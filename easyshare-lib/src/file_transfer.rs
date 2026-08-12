use tokio::fs::File;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use sha2::{Sha256, Digest};
use anyhow::Result;

/// 文件分块大小：256KB（功耗优化 #13，增大分块以减少系统调用/CPU 次数；
/// 仍属小内存占用，弱网下若需更细粒度可回退）。
pub const CHUNK_SIZE: usize = 256 * 1024;

/// 文件发送方。一个实例对应一次传输（由 `transfer_id` 标识）。
pub struct FileSender {
    transfer_id: u32,
}

impl FileSender {
    pub fn new(transfer_id: u32) -> Self {
        Self { transfer_id }
    }

    /// 计算文件 SHA-256 十六进制摘要，用于接收端完整性校验。
    pub async fn hash_file(path: &str) -> Result<String> {
        let mut file = File::open(path).await?;
        let mut hasher = Sha256::new();
        let mut buf = [0u8; CHUNK_SIZE];
        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// 分块读取并发送文件。每片包裹为 `FileChunk` 帧后通过 `client` 发出，
    /// 最后发送 `FileComplete` 通知接收端落盘校验。
    pub async fn send_file(
        &self,
        path: &str,
        client: &crate::client::EasyShareClient,
        peer_ip: &str,
        port: u16,
    ) -> Result<()> {
        let mut file = File::open(path).await?;
        let mut seq: u32 = 0;
        let mut buf = [0u8; CHUNK_SIZE];

        loop {
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break;
            }

            let chunk = crate::proto::easyshare::FileChunk {
                transfer_id: self.transfer_id,
                seq,
                data: buf[..n].to_vec(),
            };
            let frame = crate::proto::make_envelope(
                crate::proto::easyshare::MessageType::FileChunk as i32,
                &chunk,
            )?;
            client.send_to(peer_ip, port, &frame).await?;
            seq += 1;
        }

        let complete = crate::proto::easyshare::FileComplete {
            transfer_id: self.transfer_id,
            success: true,
        };
        let frame = crate::proto::make_envelope(
            crate::proto::easyshare::MessageType::FileComplete as i32,
            &complete,
        )?;
        client.send_to(peer_ip, port, &frame).await?;
        Ok(())
    }
}

/// 将已收集的分块按 `seq` 排序后写入磁盘。接收端在收到 `FileComplete` 时调用。
pub async fn receive_file(
    save_path: &str,
    chunks: Vec<crate::proto::easyshare::FileChunk>,
) -> Result<()> {
    let mut file = tokio::fs::File::create(save_path).await?;
    let mut sorted = chunks;
    sorted.sort_by_key(|c| c.seq);
    for chunk in sorted {
        file.write_all(&chunk.data).await?;
    }
    file.flush().await?;
    Ok(())
}
