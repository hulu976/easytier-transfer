use tokio::net::TcpStream;
use tokio::io::AsyncWriteExt;
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::Mutex;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use socket2::{SockRef, TcpKeepalive};

/// 连接池中的一条连接及其最近 activity 时间戳。
struct Conn {
    stream: TcpStream,
    last: Instant,
}

/// 应用层 TCP 客户端。
///
/// 维护到每个对端虚拟 IP 的连接池，所有消息复用同一连接，避免每次发送都新建
/// TCP 连接。连接以 `ip:port` 为 key 缓存。
///
/// 功耗优化（阶段 4）：连接空闲超过 `idle_timeout` 后由后台清扫任务关闭，使
/// 无线射频得以休眠；下次发送时按需重连（以极小的延迟换取续航）。
pub struct EasyShareClient {
    connections: Arc<Mutex<HashMap<String, Conn>>>,
}

impl EasyShareClient {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// 向指定对端发送一帧完整消息（已含长度前缀）。
    ///
    /// 若到该对端的连接不存在则新建并缓存；后续调用复用同一连接。
    pub async fn send_to(&self, peer_ip: &str, port: u16, data: &[u8]) -> Result<()> {
        let addr = format!("{}:{}", peer_ip, port);
        let mut conns = self.connections.lock().await;

        if !conns.contains_key(&addr) {
            let stream = TcpStream::connect(&addr).await?;
            // TCP keepalive：用 OS 级保活替代应用层心跳，减少唤醒次数（功耗优化 #4）。
            // tokio 1.x 的 TcpStream 未直接暴露 set_keepalive，这里用 socket2 跨平台设置
            // （Unix 走 AsRawFd、Windows 走 AsRawSocket，SockRef::from 均可解析）。
            let sock = SockRef::from(&stream);
            let ka = TcpKeepalive::new().with_time(Duration::from_secs(120));
            if let Err(e) = sock.set_tcp_keepalive(&ka) {
                log::warn!("设置 TCP keepalive 失败（不影响发送）: {}", e);
            }
            log::debug!("Connected to {}", addr);
            conns.insert(
                addr.clone(),
                Conn {
                    stream,
                    last: Instant::now(),
                },
            );
        }

        let conn = conns.get_mut(&addr).unwrap();
        conn.stream.write_all(data).await?;
        conn.stream.flush().await?;
        conn.last = Instant::now();
        Ok(())
    }

    /// 启动后台空闲清扫：周期性关闭空闲超过 `idle_timeout` 的连接，使无线射频
    /// 得以休眠。仅在长生命周期的客户端（如 Windows 常驻进程）上调用；短生命
    /// 周期客户端（如安卓每次 JNI 调用新建的）依赖自身 drop 即可。
    pub fn start_idle_sweeper(&self, idle_timeout: Duration) {
        // 清扫间隔取 idle/4，夹在 5s~30s，兼顾及时性与自身开销。
        let mut sweep = idle_timeout / 4;
        if sweep < Duration::from_secs(5) {
            sweep = Duration::from_secs(5);
        } else if sweep > Duration::from_secs(30) {
            sweep = Duration::from_secs(30);
        }
        let conns = self.connections.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(sweep);
            loop {
                ticker.tick().await;
                let mut conns = conns.lock().await;
                let now = Instant::now();
                let before = conns.len();
                conns.retain(|_, c| now.duration_since(c.last) < idle_timeout);
                let closed = before - conns.len();
                if closed > 0 {
                    log::debug!("Idle connection sweeper closed {} connection(s)", closed);
                }
            }
        });
    }

    /// 当前活跃连接数（调试 / 测试用）。
    pub async fn active_connection_count(&self) -> usize {
        self.connections.lock().await.len()
    }
}

impl Default for EasyShareClient {
    fn default() -> Self {
        Self::new()
    }
}
