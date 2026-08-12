#[cfg(not(target_os = "android"))]
use std::sync::Arc;
#[cfg(not(target_os = "android"))]
use std::sync::Mutex as StdMutex;
#[cfg(not(target_os = "android"))]
use std::time::{Instant, SystemTime, UNIX_EPOCH};
#[cfg(not(target_os = "android"))]
use tokio::sync::Mutex;
#[cfg(not(target_os = "android"))]
use crate::proto::easyshare::ClipboardSync;

/// 轮询间隔与图片处理相关常量（功耗优化）。
#[cfg(not(target_os = "android"))]
const POLL_TEXT_MS: u64 = 2000; // 文本检测间隔
#[cfg(not(target_os = "android"))]
const POLL_IMAGE_MS: u64 = 5000; // 图片检测间隔（错开，避免每轮都做图片 round-trip）
#[cfg(not(target_os = "android"))]
const IMAGE_THROTTLE_SECS: u64 = 2; // 同一设备图片同步最小间隔（防连续截图刷屏）
#[cfg(not(target_os = "android"))]
const MAX_IMAGE_W: u32 = 1920; // 同步图片最大宽
#[cfg(not(target_os = "android"))]
const MAX_IMAGE_H: u32 = 1080; // 同步图片最大高

/// 剪切板监视器（Windows / Linux 端）。
///
/// - Windows / Linux 端：轮询检测文本/图片变化（`watch_polling`）。
/// - Android 端：不使用本结构轮询；由 `ClipAccessibilityService` 在后台读取，
///   再通过 JNI 调用 [`crate::android`] 的入口把变化广播出去（见阶段 3）。
pub struct ClipboardWatcher {
    #[cfg(not(target_os = "android"))]
    last_hash: Arc<Mutex<u64>>,
    #[cfg(not(target_os = "android"))]
    last_image_hash: Arc<Mutex<u64>>,
    /// 上次图片发送时刻（用于节流，避免截图场景连续推送）。
    #[cfg(not(target_os = "android"))]
    last_image_send: StdMutex<Option<Instant>>,
    #[cfg(not(target_os = "android"))]
    device_name: String,
}

impl ClipboardWatcher {
    #[cfg_attr(target_os = "android", allow(unused_variables))]
    pub fn new(device_name: String) -> Self {
        Self {
            #[cfg(not(target_os = "android"))]
            last_hash: Arc::new(Mutex::new(0)),
            #[cfg(not(target_os = "android"))]
            last_image_hash: Arc::new(Mutex::new(0)),
            #[cfg(not(target_os = "android"))]
            last_image_send: StdMutex::new(None),
            #[cfg(not(target_os = "android"))]
            device_name,
        }
    }

    /// 轮询剪切板变化（Windows / Linux 端使用）。
    ///
    /// 功耗优化（阶段 4 / 5）：
    /// - 文本约每 `POLL_TEXT_MS` 检测一次，图片约每 `POLL_IMAGE_MS` 检测一次（错开）；
    /// - 复用进程级持久剪贴板句柄（不再每轮 `Clipboard::new()`）；
    /// - `screen_on == false`（息屏）时**直接不轮询**（用户要求：息屏不询）；
    /// - `active == false`（无在线节点）时跳过实际剪贴板读取；
    /// - 图片超过 `MAX_IMAGE_*` 时降采样；PNG 用快速压缩；同一设备 2s 内只同步一次图片。
    ///
    /// 仅当内容 hash 变化时才触发 `callback`，避免把刚写入的远端内容再次回传。
    #[cfg(not(target_os = "android"))]
    pub async fn watch_polling<F>(
        &self,
        callback: F,
        active: std::sync::Arc<std::sync::atomic::AtomicBool>,
        screen_on: std::sync::Arc<std::sync::atomic::AtomicBool>,
    ) where
        F: Fn(ClipboardSync) + Send + 'static,
    {
        let mut text_ticker = tokio::time::interval(std::time::Duration::from_millis(POLL_TEXT_MS));
        let mut image_ticker =
            tokio::time::interval(std::time::Duration::from_millis(POLL_IMAGE_MS));
        loop {
            tokio::select! {
                _ = text_ticker.tick() => {
                    if online(&active, &screen_on) {
                        if let Some(text) = platform::read_text() {
                            let hash = simple_hash(&text);
                            let mut last = self.last_hash.lock().await;
                            if hash != *last {
                                *last = hash;
                                callback(ClipboardSync {
                                    device_name: self.device_name.clone(),
                                    clip_type: crate::proto::CLIP_TYPE_TEXT,
                                    data: text.into_bytes(),
                                    timestamp: now_ms(),
                                });
                            }
                        }
                    }
                }
                _ = image_ticker.tick() => {
                    if online(&active, &screen_on) {
                        if let Some(png) = platform::read_image(Some((MAX_IMAGE_W, MAX_IMAGE_H))) {
                            let hash = simple_hash_bytes(&png);
                            let mut last = self.last_image_hash.lock().await;
                            if hash != *last {
                                *last = hash;
                                // 节流：同一设备 2s 内只同步一次图片
                                let mut last_send = self.last_image_send.lock().unwrap();
                                let allow = match *last_send {
                                    Some(t) => t.elapsed().as_secs() >= IMAGE_THROTTLE_SECS,
                                    None => true,
                                };
                                if allow {
                                    *last_send = Some(Instant::now());
                                    callback(ClipboardSync {
                                        device_name: self.device_name.clone(),
                                        clip_type: crate::proto::CLIP_TYPE_IMAGE,
                                        data: png,
                                        timestamp: now_ms(),
                                    });
                                }
                                // hash 已更新（无论是否被节流），避免重复触发
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn online(
    active: &std::sync::Arc<std::sync::atomic::AtomicBool>,
    screen_on: &std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> bool {
    use std::sync::atomic::Ordering;
    screen_on.load(Ordering::Relaxed) && active.load(Ordering::Relaxed)
}

#[cfg(not(target_os = "android"))]
mod platform {
    use super::*;
    use arboard::Clipboard;
    use std::sync::OnceLock;

    /// 进程级持久剪贴板句柄。
    ///
    /// 保持所有者存活，使写入的内容可被其他进程/实例读回（X11 下剪贴板所有权
    /// 随所有者销毁而丢失；Windows 下则天然持久）。复用同一句柄也更高效。
    static PERSISTENT_CLIPBOARD: OnceLock<std::sync::Mutex<Clipboard>> = OnceLock::new();

    /// 取得进程级持久剪贴板句柄（watch_polling 与 set_text/set_image 共用）。
    ///
    /// 避免每轮轮询都新建句柄（省一次系统调用），也保证读写使用同一所有者。
    pub(crate) fn clipboard_handle() -> std::sync::MutexGuard<'static, Clipboard> {
        PERSISTENT_CLIPBOARD
            .get_or_init(|| std::sync::Mutex::new(Clipboard::new().expect("无法初始化剪贴板")))
            .lock()
            .unwrap()
    }

    /// 读取当前文本（复用持久句柄），返回 owned 字符串。
    /// 同步函数内用完即释放句柄，避免非 Send 的 `MutexGuard` 逃逸到 async 作用域。
    pub(crate) fn read_text() -> Option<String> {
        clipboard_handle().get_text().ok()
    }

    /// 读取当前图片并编码为 PNG 字节（复用持久句柄），返回 owned 数据。
    /// `max` 给定时，超出尺寸则降采样到其范围内（功耗优化：减少传输/内存）。
    pub(crate) fn read_image(max: Option<(u32, u32)>) -> Option<Vec<u8>> {
        let mut g = clipboard_handle();
        super::read_image_png(&mut *g, max)
    }

    impl ClipboardWatcher {
        /// 将远端同步来的文本写入本地剪贴板。
        ///
        /// 使用进程级持久句柄，确保写入后内容可持续被读取（跨进程/实例可见）。
        pub fn set_text(text: String) -> anyhow::Result<()> {
            let cell = PERSISTENT_CLIPBOARD.get_or_init(|| {
                std::sync::Mutex::new(Clipboard::new().expect("无法初始化剪贴板"))
            });
            let mut clip = cell.lock().unwrap();
            clip.set_text(text)?;
            Ok(())
        }

        /// 供 Android JNI 调用：在不改变本地"最后 hash"的前提下写入，
        /// 由调用方负责去重，避免回环。
        pub fn set_remote_text(text: String) -> anyhow::Result<()> {
            Self::set_text(text)
        }

        /// 将远端同步来的图片（PNG 字节）写入本地剪贴板。
        ///
        /// 先把 PNG 解码为 RGBA，再交给 arboard 写入系统剪贴板。
        /// 同样使用进程级持久句柄，确保内容可被其他实例读回。
        pub fn set_image(png: &[u8]) -> anyhow::Result<()> {
            let img = image::load_from_memory(png)
                .map_err(|e| anyhow::anyhow!("解码图片失败: {}", e))?;
            let rgba = img.to_rgba8();
            let data = arboard::ImageData {
                width: rgba.width() as usize,
                height: rgba.height() as usize,
                bytes: std::borrow::Cow::Owned(rgba.into_raw()),
            };
            let cell = PERSISTENT_CLIPBOARD.get_or_init(|| {
                std::sync::Mutex::new(Clipboard::new().expect("无法初始化剪贴板"))
            });
            let mut clip = cell.lock().unwrap();
            clip.set_image(data)?;
            Ok(())
        }
    }
}

#[cfg(not(target_os = "android"))]
fn simple_hash(s: &str) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    s.hash(&mut hasher);
    hasher.finish()
}

#[cfg(not(target_os = "android"))]
fn simple_hash_bytes(b: &[u8]) -> u64 {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    b.hash(&mut hasher);
    hasher.finish()
}

/// 从 arboard 读取当前剪贴板图片，降采样（可选）后编码为**快速压缩** PNG 字节。
/// 无图片时返回 None。
#[cfg(not(target_os = "android"))]
fn read_image_png(clip: &mut arboard::Clipboard, max: Option<(u32, u32)>) -> Option<Vec<u8>> {
    use image::codecs::png::{CompressionType, FilterType as PngFilter, PngEncoder};
    use image::imageops::FilterType as ResizeFilter;
    use image::ExtendedColorType;
    use image::ImageEncoder;

    let img = clip.get_image().ok()?;
    let rgba = image::RgbaImage::from_raw(img.width as u32, img.height as u32, img.bytes.to_vec())?;

    // 降采样：超出最大尺寸则等比缩放到范围内
    let rgba = if let Some((mw, mh)) = max {
        let (w, h) = (rgba.width(), rgba.height());
        if w > mw || h > mh {
            let scale = ((mw as f64 / w as f64).min(mh as f64 / h as f64)).min(1.0);
            let nw = (w as f64 * scale).max(1.0) as u32;
            let nh = (h as f64 * scale).max(1.0) as u32;
            image::imageops::resize(&rgba, nw, nh, ResizeFilter::Triangle)
        } else {
            rgba
        }
    } else {
        rgba
    };

    // 快速压缩，降低 CPU 峰值（功耗优化 #5）
    let mut buf = Vec::new();
    {
        let mut cursor = std::io::Cursor::new(&mut buf);
        let encoder = PngEncoder::new_with_quality(&mut cursor, CompressionType::Fast, PngFilter::NoFilter);
        if encoder
            .write_image(rgba.as_raw(), rgba.width(), rgba.height(), ExtendedColorType::Rgba8)
            .is_err()
        {
            return None;
        }
    }
    Some(buf)
}

#[cfg(not(target_os = "android"))]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64
}
