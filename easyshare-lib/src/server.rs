use tokio::net::TcpListener;
use tokio::io::AsyncReadExt;
use anyhow::Result;
use std::sync::Arc;
use crate::handler::MessageHandler;

/// 应用层 TCP 服务端。
///
/// 监听在 EasyTier 分配的虚拟 IP 上（如 `10.144.144.1:12000`）。每个接入的
/// 连接在一个独立 task 中处理：循环读取 `[4字节长度][payload]` 帧，
/// 交给 [`MessageHandler`] 分发。
pub struct EasyShareServer {
    handler: Arc<MessageHandler>,
}

impl EasyShareServer {
    pub fn new(handler: Arc<MessageHandler>) -> Self {
        Self { handler }
    }

    /// 绑定 `virtual_ip:port` 并开始接受连接。此调用会一直阻塞。
    pub async fn run(&self, virtual_ip: &str, port: u16) -> Result<()> {
        let addr = format!("{}:{}", virtual_ip, port);
        let listener = TcpListener::bind(&addr).await?;
        log::info!("EasyShare listening on {}", addr);

        loop {
            let (mut stream, peer_addr) = listener.accept().await?;
            log::debug!("Connection from {}", peer_addr);
            let handler = self.handler.clone();

            tokio::spawn(async move {
                let mut len_buf = [0u8; 4];
                loop {
                    // 读取 4 字节长度前缀
                    match stream.read_exact(&mut len_buf).await {
                        Ok(_) => {}
                        Err(_) => break, // 连接关闭或读取出错
                    }
                    let len = u32::from_be_bytes(len_buf) as usize;

                    // 读取 payload
                    let mut payload = vec![0u8; len];
                    if stream.read_exact(&mut payload).await.is_err() {
                        break;
                    }

                    // 处理消息
                    if let Err(e) = handler.handle_message(&payload).await {
                        log::warn!("Handle message error: {}", e);
                    }
                }
            });
        }
    }
}
