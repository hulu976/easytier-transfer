use std::sync::Arc;
use anyhow::Result;
use prost::Message;
use tokio::sync::Mutex;
use std::collections::HashMap;

use crate::proto::easyshare::{MessageType, ClipboardSync, FileChunk, FileComplete, FileOffer};

/// 消息分发与处理中心。
///
/// 服务端每收到一帧就交给 [`MessageHandler::handle_message`]。当前实现：
/// - `ClipboardSync`：写入本地剪切板（Windows/Android 通用）。
/// - `FileOffer`：记录 `transfer_id` -> 文件名，供落盘时还原原始文件名。
/// - `FileChunk`：按 `transfer_id` 缓存分块。
/// - `FileComplete`：将缓存的分块排序落盘到 `save_dir/{文件名}`。
/// - `Heartbeat`：仅记录 trace 日志。
pub struct MessageHandler {
    _device_name: String,
    /// 文件接收缓存：`transfer_id` -> 已收到的分块。
    file_chunks: Arc<Mutex<HashMap<u32, Vec<FileChunk>>>>,
    /// 文件名缓存：`transfer_id` -> 发送端声明的原始文件名。
    file_offers: Arc<Mutex<HashMap<u32, String>>>,
    save_dir: String,
    /// 可选观察回调：收到远端剪切板时触发（供 UI 显示状态 / 测试断言）。
    on_clipboard: Option<Arc<dyn Fn(&ClipboardSync) + Send + Sync>>,
}

impl MessageHandler {
    pub fn new(device_name: String, save_dir: String) -> Self {
        Self {
            _device_name: device_name,
            file_chunks: Arc::new(Mutex::new(HashMap::new())),
            file_offers: Arc::new(Mutex::new(HashMap::new())),
            save_dir,
            on_clipboard: None,
        }
    }

    /// 注册"收到远端剪切板"观察回调（可选，不影响默认行为）。
    pub fn with_clipboard_observer(
        mut self,
        f: Arc<dyn Fn(&ClipboardSync) + Send + Sync>,
    ) -> Self {
        self.on_clipboard = Some(f);
        self
    }

    pub async fn handle_message(&self, data: &[u8]) -> Result<()> {
        let env = crate::proto::decode_envelope(data)?;

        match crate::proto::message_type_from_i32(env.r#type) {
            MessageType::ClipboardSync => {
                let msg = ClipboardSync::decode(env.payload.as_slice())?;
                // 同步频繁：仅 Debug 构建输出，Release 编译期移除（功耗优化 #6）
                #[cfg(debug_assertions)]
                log::trace!(
                    "Clipboard sync from {}: type={} {} bytes",
                    msg.device_name,
                    msg.clip_type,
                    msg.data.len()
                );

                // 可选观察回调：先确定性通知"已收到"（供 UI 状态 / 测试断言），
                // 不依赖后续写系统剪贴板的成败。
                if let Some(cb) = &self.on_clipboard {
                    cb(&msg);
                }

                // 记录"本次内容来自远端"，避免本机剪贴板监听器把它再广播回去
                // （否则两台设备会互相回声，形成同步风暴）。
                crate::api::mark_remote_write(&msg.data);

                // 按内容类型写回本地剪切板；写失败仅告警，不让整条消息处理失败。
                match msg.clip_type {
                    crate::proto::CLIP_TYPE_IMAGE => {
                        #[cfg(not(target_os = "android"))]
                        if let Err(e) =
                            crate::clipboard::ClipboardWatcher::set_image(&msg.data)
                        {
                            log::warn!("写入图片剪贴板失败: {}", e);
                        }
                        #[cfg(target_os = "android")]
                        crate::android::set_android_clipboard_image(&msg.data);
                    }
                    _ => {
                        // 文本（含未知类型统一回退为文本）
                        #[cfg(not(target_os = "android"))]
                        if let Err(e) = crate::clipboard::ClipboardWatcher::set_text(
                            String::from_utf8_lossy(&msg.data).to_string(),
                        ) {
                            log::warn!("写入文本剪贴板失败: {}", e);
                        }
                        #[cfg(target_os = "android")]
                        crate::android::set_android_clipboard(
                            &String::from_utf8_lossy(&msg.data),
                        );
                    }
                }
            }
            MessageType::FileOffer => {
                let offer = FileOffer::decode(env.payload.as_slice())?;
                let name = sanitize_filename(&offer.file_name);
                let mut offers = self.file_offers.lock().await;
                offers.insert(offer.transfer_id, name);
                log::info!("File offer: {} (transfer {})", offer.file_name, offer.transfer_id);
            }
            MessageType::FileChunk => {
                let chunk = FileChunk::decode(env.payload.as_slice())?;
                let mut chunks = self.file_chunks.lock().await;
                chunks
                    .entry(chunk.transfer_id)
                    .or_insert_with(Vec::new)
                    .push(chunk);
            }
            MessageType::FileComplete => {
                let msg = FileComplete::decode(env.payload.as_slice())?;
                if msg.success {
                    let mut chunks = self.file_chunks.lock().await;
                    if let Some(file_chunks) = chunks.remove(&msg.transfer_id) {
                        // 优先用发送端声明的文件名，回退到 transfer_id
                        let name = {
                            let offers = self.file_offers.lock().await;
                            offers.get(&msg.transfer_id).cloned()
                        };
                        let safe_name = name.unwrap_or_else(|| format!("recv_{}.bin", msg.transfer_id));
                        let save_path = format!("{}/{}", self.save_dir, safe_name);
                        if let Err(e) = crate::file_transfer::receive_file(&save_path, file_chunks).await {
                            log::error!("File receive failed: {e}");
                        } else {
                            log::info!("File received: {}", save_path);
                            // 通知宿主（Android 上弹通知并刷新媒体库）
                            crate::api::notify_file_received(&save_path);
                        }
                    }
                }
            }
            MessageType::Heartbeat => {
                log::trace!("Heartbeat received");
            }
            _ => {
                log::debug!("Unhandled message type: {:?}", env.r#type);
            }
        }
        Ok(())
    }
}

/// 把发送端给的文件名规整成安全落盘名：只保留最后一段、去掉路径分隔符，
/// 避免 `../../etc/passwd` 之类的路径穿越。
fn sanitize_filename(name: &str) -> String {
    let base = name.rsplit('/').next().unwrap_or(name).rsplit('\\').next().unwrap_or(name);
    let trimmed = base.trim();
    if trimmed.is_empty() {
        "file.bin".to_string()
    } else {
        trimmed.to_string()
    }
}
