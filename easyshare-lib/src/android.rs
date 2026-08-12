//! Android 端 JNI 桥接（仅 Android 编译）。
//!
//! 宿主是官方 EasyTier App（Tauri，包名 `com.easyshare.easytier`）。服务的
//! 启停由 Rust 侧 [`crate::api`] 直接完成，本模块只负责两个方向的跨语言调用：
//!
//! - **Kotlin → Rust**：`EasyShareBridge.nativeSendClipboard(text)` /
//!   `nativeSendClipboardImage(png)`，把本机剪贴板变化广播给在线节点。
//! - **Rust → Kotlin**：[`set_android_clipboard`] / [`set_android_clipboard_image`]，
//!   把远端来的内容写回 Android 系统剪贴板（经 `EasyShareBridge` 静态方法）。
//!
//! JNI 函数名必须与 Kotlin 侧 `external fun` 的包名/类名/方法名严格对应：
//! `com.easyshare.easytier.EasyShareBridge` → `Java_com_easyshare_easytier_EasyShareBridge_*`。

use std::sync::OnceLock;

use jni::objects::{JByteArray, JClass, JObject, JString, JValue};
use jni::sys::{jboolean, JNI_FALSE, JNI_TRUE};
use jni::{JNIEnv, JavaVM};

/// 桥接类的 JNI 内部名（用于 `find_class`）。
const BRIDGE_CLASS: &str = "com/easyshare/easytier/EasyShareBridge";

/// 缓存 JavaVM，供后续（非 JNI 调用线程上）回写系统剪贴板使用。
static JVM: OnceLock<JavaVM> = OnceLock::new();

/// 由 Kotlin 在进程启动时调用一次，缓存 JavaVM。
///
/// 必须先于任何 [`set_android_clipboard`] 调用发生，否则远端内容无法回写。
#[no_mangle]
pub extern "C" fn Java_com_easyshare_easytier_EasyShareBridge_nativeInit(
    env: JNIEnv,
    _class: JClass,
) {
    if let Ok(vm) = env.get_java_vm() {
        let _ = JVM.set(vm);
        log::info!("easyshare: JavaVM cached");
    }
}

/// Kotlin 侧检测到本机剪贴板文本变化时调用，广播给所有在线节点。
#[no_mangle]
pub extern "C" fn Java_com_easyshare_easytier_EasyShareBridge_nativeSendClipboard(
    mut env: JNIEnv,
    _class: JClass,
    text: JString,
) {
    let text: String = env.get_string(&text).map(String::from).unwrap_or_default();
    if text.is_empty() {
        return;
    }
    crate::api::broadcast_text(&text);
}

/// Kotlin 侧检测到本机剪贴板图片变化时调用，广播 PNG 字节给所有在线节点。
#[no_mangle]
pub extern "C" fn Java_com_easyshare_easytier_EasyShareBridge_nativeSendClipboardImage(
    env: JNIEnv,
    _class: JClass,
    png: JByteArray,
) {
    let png: Vec<u8> = match env.convert_byte_array(png) {
        Ok(b) => b,
        Err(e) => {
            log::warn!("easyshare: convert_byte_array failed: {e}");
            return;
        }
    };
    crate::api::broadcast_image(png);
}

/// Kotlin 侧由系统分享面板入口调用：把本地文件发给所有在线节点。
#[no_mangle]
pub extern "C" fn Java_com_easyshare_easytier_EasyShareBridge_nativeSendFile(
    mut env: JNIEnv,
    _class: JClass,
    path: JString,
) {
    let path: String = env.get_string(&path).map(String::from).unwrap_or_default();
    if path.is_empty() {
        return;
    }
    crate::api::send_file(&path);
}

/// Kotlin 侧查询文件传输是否已启用（分享前提示用户去开启）。
#[no_mangle]
pub extern "C" fn Java_com_easyshare_easytier_EasyShareBridge_nativeIsFileTransferEnabled(
    _env: JNIEnv,
    _class: JClass,
) -> jboolean {
    if crate::api::file_transfer_enabled() {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

/// 通用的"调 Kotlin 桥接类静态方法"辅助函数。
fn call_bridge_static<F>(method: &str, sig: &str, build_arg: F)
where
    F: for<'a> FnOnce(&mut JNIEnv<'a>) -> Option<JObject<'a>>,
{
    let Some(vm) = JVM.get() else {
        log::warn!("easyshare: JVM not initialized, cannot call {method}");
        return;
    };
    let mut env = match vm.attach_current_thread() {
        Ok(env) => env,
        Err(e) => {
            log::warn!("easyshare: attach_current_thread failed: {e}");
            return;
        }
    };

    let Some(arg) = build_arg(&mut env) else {
        return;
    };

    let class = match env.find_class(BRIDGE_CLASS) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("easyshare: find_class {BRIDGE_CLASS} failed: {e}");
            let _ = env.exception_clear();
            return;
        }
    };

    let args = [JValue::Object(&arg)];
    if let Err(e) = env.call_static_method(class, method, sig, &args) {
        log::warn!("easyshare: call {method} failed: {e}");
        let _ = env.exception_clear();
    }
}

/// 远端文本剪贴板到达时，回写到 Android 系统剪贴板。
pub fn set_android_clipboard(text: &str) {
    let text = text.to_string();
    call_bridge_static(
        "setRemoteClipboard",
        "(Ljava/lang/String;)V",
        move |env| match env.new_string(&text) {
            Ok(s) => Some(s.into()),
            Err(e) => {
                log::warn!("easyshare: new_string failed: {e}");
                None
            }
        },
    );
}

/// 远端文件接收完成（已落盘到 `path`）时，通知 Android 侧弹通知并刷新媒体库。
pub fn on_file_received(path: &str) {
    let path = path.to_string();
    call_bridge_static(
        "onFileReceived",
        "(Ljava/lang/String;)V",
        move |env| match env.new_string(&path) {
            Ok(s) => Some(s.into()),
            Err(e) => {
                log::warn!("easyshare: new_string failed: {e}");
                None
            }
        },
    );
}

/// 打开系统「无障碍」设置页，引导用户开启 EasyShare 剪贴板监听服务。
///
/// Android 10 起后台应用无法读取剪贴板，只有无障碍服务例外，因此这是
/// 移动端剪贴板同步能真正工作的前提。
pub fn open_accessibility_settings() {
    let Some(vm) = JVM.get() else {
        log::warn!("easyshare: JVM not initialized");
        return;
    };
    let Ok(mut env) = vm.attach_current_thread() else {
        return;
    };
    let Ok(class) = env.find_class(BRIDGE_CLASS) else {
        let _ = env.exception_clear();
        return;
    };
    if let Err(e) = env.call_static_method(class, "openAccessibilitySettings", "()V", &[]) {
        log::warn!("easyshare: openAccessibilitySettings failed: {e}");
        let _ = env.exception_clear();
    }
}

/// 查询 EasyShare 的无障碍服务当前是否已被用户开启。
pub fn is_accessibility_enabled() -> bool {
    let Some(vm) = JVM.get() else {
        return false;
    };
    let Ok(mut env) = vm.attach_current_thread() else {
        return false;
    };
    let Ok(class) = env.find_class(BRIDGE_CLASS) else {
        let _ = env.exception_clear();
        return false;
    };
    match env.call_static_method(class, "isAccessibilityEnabled", "()Z", &[]) {
        Ok(v) => v.z().unwrap_or(false),
        Err(e) => {
            log::warn!("easyshare: isAccessibilityEnabled failed: {e}");
            let _ = env.exception_clear();
            false
        }
    }
}

/// 远端图片剪贴板到达时，回写到 Android 系统剪贴板。
pub fn set_android_clipboard_image(png: &[u8]) {
    let png = png.to_vec();
    call_bridge_static("setRemoteClipboardImage", "([B)V", move |env| {
        match env.byte_array_from_slice(&png) {
            Ok(a) => Some(a.into()),
            Err(e) => {
                log::warn!("easyshare: byte_array_from_slice failed: {e}");
                None
            }
        }
    });
}
