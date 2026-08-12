//! EasyShare 应用层服务
//!
//! 一个完全独立于 EasyTier 的 crate，通过虚拟 IP 上的 TCP 通信实现
//! 跨设备的文件传输与剪切板同步。与 EasyTier 零代码耦合。
//!
//! 模块划分：
//! - [`proto`]: Protobuf 编解码与消息帧封装（[4字节长度][payload]）
//! - [`server`]: TCP 服务端，监听虚拟 IP:12000
//! - [`client`]: TCP 客户端，向对端虚拟 IP 发送消息（连接复用）
//! - [`clipboard`]: 剪切板监听与写入（Windows 端轮询 / Android 端无障碍服务）
//! - [`file_transfer`]: 文件分块传输与 SHA-256 校验
//! - [`peer_discovery`]: 获取在线节点（宿主注入 / `easytier-cli route --json`）
//! - [`handler`]: 消息分发与处理中心
//! - [`api`]: 进程内运行时 API，供宿主（EasyTier GUI）嵌入式调用

pub mod api;
pub mod proto;
pub mod server;
pub mod client;
pub mod clipboard;
pub mod file_transfer;
/// 通过 `easytier-cli route --json` 发现在线节点。仅桌面端可用 —— Android 上
/// 没有 CLI 可执行文件，改由宿主经 [`api::update_peers`] 注入路由表。
#[cfg(not(target_os = "android"))]
pub mod peer_discovery;
pub mod handler;

/// Android 端 JNI 桥接（仅 Android 编译）。为宿主 App 提供 `nativeSendClipboard`
/// 等 JNI 入口，以及 [`android::set_android_clipboard`] 供 handler 回写系统剪贴板。
#[cfg(target_os = "android")]
pub mod android;

/// 应用层服务默认监听端口（运行在 EasyTier 虚拟网络之上）。
pub const DEFAULT_PORT: u16 = 12000;
