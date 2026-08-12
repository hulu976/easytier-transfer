//! 进程内运行时 API：供宿主（EasyTier GUI / Tauri 移动端）直接调用。
//!
//! 与旧的"侧车二进制"模式不同，本模块把 EasyShare 作为**库**嵌入宿主进程：
//! - 宿主调用 [`start`] 拉起 TCP 服务端（跑在独立的 tokio 运行时线程上）
//! - 宿主周期性调用 [`update_peers`] 把 EasyTier 路由表里的在线虚拟 IP 喂进来
//!   （Android 上没有 `easytier-cli`，无法用子进程发现节点）
//! - 本地剪贴板变化时，宿主调用 [`broadcast_text`] / [`broadcast_image`] 广播
//!
//! 所有函数都是非阻塞的：内部要么 spawn 到常驻运行时，要么只写全局状态。

use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::{Arc, LazyLock, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::runtime::Runtime;

/// 常驻 tokio 运行时（服务端 + 广播共用，避免每次广播新建运行时的开销）。
static RUNTIME: OnceLock<Runtime> = OnceLock::new();
/// 服务是否已启动（重复调用 [`start`] 时只更新配置，不重复监听）。
static STARTED: AtomicBool = AtomicBool::new(false);
/// 同步开关：关闭后收发都停止（比杀线程更安全，也便于随时恢复）。
static ENABLED: AtomicBool = AtomicBool::new(false);
/// 文件传输开关：与剪贴板同步相互独立，由宿主注入（分享面板发送前会校验）。
static FILE_TRANSFER: AtomicBool = AtomicBool::new(false);
/// 服务端口。
static PORT: AtomicU16 = AtomicU16::new(crate::DEFAULT_PORT);
/// 是否同步图片剪贴板。
static SYNC_IMAGES: AtomicBool = AtomicBool::new(true);
/// 本机设备名（随消息携带，供对端识别来源）。
static DEVICE_NAME: LazyLock<RwLock<String>> =
    LazyLock::new(|| RwLock::new("device".to_string()));
/// 宿主注入的在线节点虚拟 IP 列表（替代 `easytier-cli route --json`）。
static PEERS: LazyLock<RwLock<Vec<String>>> = LazyLock::new(|| RwLock::new(Vec::new()));
/// 最近一次由远端写入本地剪贴板的内容指纹，用于抑制回环（A->B->A）。
static LAST_REMOTE: LazyLock<RwLock<Vec<u8>>> = LazyLock::new(|| RwLock::new(Vec::new()));

/// 文件接收回调：宿主（桌面端 GUI）注册，用于在文件落盘后弹通知 / 向前端发事件。
static FILE_RECEIVED_CB: LazyLock<RwLock<Option<Box<dyn Fn(String) + Send + Sync>>>> =
    LazyLock::new(|| RwLock::new(None));

fn runtime() -> &'static Runtime {
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .thread_name("easyshare")
            .build()
            .expect("easyshare: failed to build tokio runtime")
    })
}

/// 当前设备名。
pub fn device_name() -> String {
    DEVICE_NAME
        .read()
        .map(|s| s.clone())
        .unwrap_or_else(|_| "device".to_string())
}

/// 当前服务端口。
pub fn port() -> u16 {
    PORT.load(Ordering::SeqCst)
}

/// 是否同步图片。
pub fn sync_images() -> bool {
    SYNC_IMAGES.load(Ordering::SeqCst)
}

/// 同步功能当前是否启用。
pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::SeqCst)
}

/// 宿主注入文件传输开关（与剪贴板同步相互独立）。
pub fn set_file_transfer(v: bool) {
    FILE_TRANSFER.store(v, Ordering::SeqCst);
}

/// 文件传输当前是否启用（分享面板发送前会校验）。
pub fn file_transfer_enabled() -> bool {
    FILE_TRANSFER.load(Ordering::SeqCst)
}

/// 宿主注入在线节点虚拟 IP 列表（每次调用整体替换）。
pub fn update_peers(peers: Vec<String>) {
    if let Ok(mut g) = PEERS.write() {
        *g = peers;
    }
}

/// 读取当前在线节点列表。
pub fn peers() -> Vec<String> {
    PEERS.read().map(|p| p.clone()).unwrap_or_default()
}

/// 记录"刚由远端写入本地剪贴板"的内容，用于抑制广播回环。
pub fn mark_remote_write(data: &[u8]) {
    if let Ok(mut g) = LAST_REMOTE.write() {
        *g = data.to_vec();
    }
}

/// 判断某段内容是否就是刚刚由远端写入的（若是则不该再广播出去）。
pub fn is_echo_of_remote(data: &[u8]) -> bool {
    LAST_REMOTE
        .read()
        .map(|g| !g.is_empty() && g.as_slice() == data)
        .unwrap_or(false)
}

/// 启动（或重配置）EasyShare 服务。
///
/// - `bind`：监听地址，传空则 `0.0.0.0`
/// - `port`：监听端口，传 0 则使用 [`crate::DEFAULT_PORT`]
/// - `device_name`：本机显示名
/// - `sync_images`：是否同步图片剪贴板
/// - `recv_dir`：文件接收落盘目录
///
/// 重复调用是安全的：只有第一次会真正创建监听 task。
pub fn start(bind: &str, port: u16, device_name: &str, sync_images: bool, recv_dir: &str) {
    let port = if port > 0 { port } else { crate::DEFAULT_PORT };
    PORT.store(port, Ordering::SeqCst);
    SYNC_IMAGES.store(sync_images, Ordering::SeqCst);
    if let Ok(mut g) = DEVICE_NAME.write() {
        *g = device_name.to_string();
    }
    ENABLED.store(true, Ordering::SeqCst);

    if STARTED.swap(true, Ordering::SeqCst) {
        log::info!("easyshare: already running, config updated (port={port})");
        return;
    }

    // 监听地址：移动端虚拟网卡地址可能晚于服务启动才就绪，统一绑 0.0.0.0 更稳
    let bind = if bind.is_empty() || bind == "0.0.0.0" {
        "0.0.0.0".to_string()
    } else {
        bind.to_string()
    };
    let recv_dir = recv_dir.to_string();
    let dev = device_name.to_string();

    runtime().spawn(async move {
        let handler = Arc::new(crate::handler::MessageHandler::new(dev, recv_dir));
        let server = crate::server::EasyShareServer::new(handler);
        log::info!("easyshare: TCP server starting on {bind}:{port}");
        if let Err(e) = server.run(&bind, port).await {
            log::error!("easyshare: server exited: {e}");
        }
    });
}

/// 停止同步（保留监听 socket，仅关闭收发行为，便于快速恢复）。
pub fn stop() {
    ENABLED.store(false, Ordering::SeqCst);
    log::info!("easyshare: sync disabled");
}

/// 把一帧数据广播给所有在线节点。
fn broadcast_frame(frame: Vec<u8>) {
    let port = PORT.load(Ordering::SeqCst);
    let targets = peers();
    if targets.is_empty() {
        log::debug!("easyshare: no online peers, skip broadcast");
        return;
    }
    runtime().spawn(async move {
        let client = crate::client::EasyShareClient::new();
        for ip in targets {
            if let Err(e) = client.send_to(&ip, port, &frame).await {
                log::debug!("easyshare: send to {ip} failed: {e}");
            }
        }
    });
}

/// 广播文本剪贴板内容。
pub fn broadcast_text(text: &str) {
    if !is_enabled() {
        return;
    }
    let bytes = text.as_bytes();
    if bytes.is_empty() || is_echo_of_remote(bytes) {
        return;
    }
    let sync = crate::proto::easyshare::ClipboardSync {
        device_name: device_name(),
        clip_type: 0,
        data: bytes.to_vec(),
        timestamp: now_millis(),
    };
    match crate::proto::make_envelope(
        crate::proto::easyshare::MessageType::ClipboardSync as i32,
        &sync,
    ) {
        Ok(frame) => broadcast_frame(frame),
        Err(e) => log::warn!("easyshare: encode text frame failed: {e}"),
    }
}

/// 广播图片剪贴板内容（PNG 字节）。
pub fn broadcast_image(png: Vec<u8>) {
    if !is_enabled() || !sync_images() || png.is_empty() {
        return;
    }
    if is_echo_of_remote(&png) {
        return;
    }
    let sync = crate::proto::easyshare::ClipboardSync {
        device_name: device_name(),
        clip_type: crate::proto::CLIP_TYPE_IMAGE,
        data: png,
        timestamp: now_millis(),
    };
    match crate::proto::make_envelope(
        crate::proto::easyshare::MessageType::ClipboardSync as i32,
        &sync,
    ) {
        Ok(frame) => broadcast_frame(frame),
        Err(e) => log::warn!("easyshare: encode image frame failed: {e}"),
    }
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 生成一个随机 transfer_id（用于一次文件传输），避免碰撞即可。
fn transfer_id() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    (nanos ^ (nanos >> 32)) as u32
}

/// 注册文件接收回调（桌面端 GUI 用来弹通知 / 向前端发事件）。
pub fn set_file_received_callback<F>(f: F)
where
    F: Fn(String) + Send + Sync + 'static,
{
    if let Ok(mut g) = FILE_RECEIVED_CB.write() {
        *g = Some(Box::new(f));
    }
}

/// 收到远端文件后通知宿主。
#[cfg(target_os = "android")]
pub fn notify_file_received(path: &str) {
    crate::android::on_file_received(path);
}

#[cfg(not(target_os = "android"))]
pub fn notify_file_received(path: &str) {
    if let Ok(g) = FILE_RECEIVED_CB.read() {
        if let Some(cb) = g.as_ref() {
            cb(path.to_string());
        }
    }
}

/// 通过 EasyShare 把本地文件发给所有在线节点（文件传输功能）。
///
/// 发送前先广播 `FileOffer` 携带原始文件名，接收端据此还原文件名落盘；
/// 随后对每个在线节点分块发送文件内容，最后发送 `FileComplete`。
pub fn send_file(path: &str) {
    if !is_enabled() || !file_transfer_enabled() {
        log::warn!("easyshare: file transfer not enabled, skip send_file");
        return;
    }
    let name = std::path::Path::new(path)
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file.bin".to_string());
    let tid = transfer_id();
    let port = PORT.load(Ordering::SeqCst);
    let targets = peers();
    if targets.is_empty() {
        log::warn!("easyshare: no online peers, cannot send file {name}");
        return;
    }

    // 1) 广播 FileOffer，让所有对端缓存 transfer_id -> 文件名
    let offer = crate::proto::easyshare::FileOffer {
        file_name: name.clone(),
        file_size: 0,
        file_hash: String::new(),
        transfer_id: tid,
    };
    if let Ok(frame) = crate::proto::make_envelope(
        crate::proto::easyshare::MessageType::FileOffer as i32,
        &offer,
    ) {
        broadcast_frame(frame);
    }

    // 2) 对每个在线节点发送文件内容（借用 path 的所有权进入异步任务）
    let owned_path = path.to_string();
    runtime().spawn(async move {
        let client = crate::client::EasyShareClient::new();
        let sender = crate::file_transfer::FileSender::new(tid);
        for ip in targets {
            if let Err(e) = sender.send_file(&owned_path, &client, &ip, port).await {
                log::warn!("easyshare: send file to {ip} failed: {e}");
            }
        }
    });
}
