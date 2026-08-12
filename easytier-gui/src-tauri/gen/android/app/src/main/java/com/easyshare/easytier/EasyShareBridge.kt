package com.easyshare.easytier

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.ClipData
import android.content.ClipboardManager
import android.content.ComponentName
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.MediaStore
import android.provider.Settings
import android.text.TextUtils
import android.util.Log
import android.webkit.MimeTypeMap
import androidx.core.app.NotificationCompat
import androidx.core.content.FileProvider
import java.io.File
import java.io.FileOutputStream

/**
 * EasyShare 与 Rust 侧的 JNI 桥接。
 *
 * Rust 侧符号：`Java_com_easyshare_easytier_EasyShareBridge_*`（见 easyshare-lib/src/android.rs）。
 * 两个方向：
 *  - Kotlin -> Rust：本机剪贴板变化时调 [nativeSendClipboard] / [nativeSendClipboardImage]；
 *                    系统分享面板选中文件时调 [nativeSendFile]
 *  - Rust -> Kotlin：远端内容到达时回调 [setRemoteClipboard] / [setRemoteClipboardImage] /
 *                    [onFileReceived]
 *
 * 注意：所有被 Rust 反射调用的方法都必须是 `@JvmStatic`，且在 proguard 中 keep。
 */
object EasyShareBridge {
    private const val TAG = "EasyShareBridge"

    /** 应用上下文，由 [init] 注入，用于访问剪贴板与启动设置页。 */
    @Volatile
    private var appContext: Context? = null

    /** 最近一次由远端写入的内容指纹，用于抑制 A->B->A 回环。 */
    @Volatile
    private var lastRemoteText: String? = null

    @Volatile
    private var lastRemoteImageSize: Int = -1

    init {
        System.loadLibrary("app_lib")
    }

    /** 由 [MainActivity] 在进程启动时调用一次。 */
    fun init(context: Context) {
        appContext = context.applicationContext
        try {
            nativeInit()
        } catch (e: Throwable) {
            Log.w(TAG, "nativeInit failed: ${e.message}")
        }
    }

    fun context(): Context? = appContext

    /** 缓存 JavaVM 到 Rust 侧，使远端内容可以回写系统剪贴板。 */
    @JvmStatic
    external fun nativeInit()

    /** 把本机剪贴板文本广播给所有在线节点。 */
    @JvmStatic
    external fun nativeSendClipboard(text: String)

    /** 把本机剪贴板图片（PNG 字节）广播给所有在线节点。 */
    @JvmStatic
    external fun nativeSendClipboardImage(png: ByteArray)

    /** 把本地文件发给所有在线节点（由系统分享面板入口触发）。 */
    @JvmStatic
    external fun nativeSendFile(path: String)

    /** 文件传输是否已启用（服务运行 + 用户开启「启用文件传输」）。 */
    @JvmStatic
    external fun nativeIsFileTransferEnabled(): Boolean

    /** 判断某段文本是否刚由远端写入（是则不应再广播，避免回环）。 */
    fun isEchoText(text: String): Boolean = text == lastRemoteText

    /** 判断某张图是否刚由远端写入。 */
    fun isEchoImage(size: Int): Boolean = size == lastRemoteImageSize

    /**
     * 系统分享面板入口：把选中的文件发给所有在线节点。
     * 未启用文件传输（或网络未运行）时给出明确提示。
     */
    fun sendSharedFiles(paths: List<String>) {
        val ctx = appContext ?: return
        if (!nativeIsFileTransferEnabled()) {
            showNotification(
                ctx, "文件传输未启用",
                "请先在「传输与同步」中开启「启用文件传输」，并运行网络后重试"
            )
            return
        }
        var ok = 0
        for (p in paths) {
            try {
                nativeSendFile(p)
                ok++
            } catch (e: Throwable) {
                Log.w(TAG, "nativeSendFile($p) failed: ${e.message}")
            }
        }
        showNotification(
            ctx, "文件传输",
            if (ok > 0) "已向在线设备发送 $ok 个文件"
            else "发送失败：请确认网络已运行且至少一台设备在线"
        )
    }

    /**
     * 远端文本到达：写入本机系统剪贴板。由 Rust 反射调用。
     */
    @JvmStatic
    fun setRemoteClipboard(text: String) {
        val ctx = appContext ?: run {
            Log.w(TAG, "context not ready, drop remote clipboard")
            return
        }
        lastRemoteText = text
        runOnMain {
            try {
                val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("EasyShare", text))
                Log.i(TAG, "remote clipboard applied (${text.length} chars)")
            } catch (e: Throwable) {
                Log.w(TAG, "setRemoteClipboard failed: ${e.message}")
            }
        }
    }

    /**
     * 远端图片到达：落盘到私有目录后经 FileProvider 写入系统剪贴板。
     * 由 Rust 反射调用。
     */
    @JvmStatic
    fun setRemoteClipboardImage(png: ByteArray) {
        val ctx = appContext ?: return
        lastRemoteImageSize = png.size
        runOnMain {
            try {
                val bitmap: Bitmap? = BitmapFactory.decodeByteArray(png, 0, png.size)
                if (bitmap == null) {
                    Log.w(TAG, "decode remote image failed")
                    return@runOnMain
                }
                val dir = File(ctx.cacheDir, "easyshare")
                if (!dir.exists()) dir.mkdirs()
                val file = File(dir, "clip_${System.currentTimeMillis()}.png")
                FileOutputStream(file).use { out ->
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
                }
                val uri: Uri = FileProvider.getUriForFile(
                    ctx,
                    "${ctx.packageName}.fileprovider",
                    file
                )
                val cm = ctx.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                cm.setPrimaryClip(ClipData.newUri(ctx.contentResolver, "EasyShare", uri))
                Log.i(TAG, "remote image applied (${png.size} bytes)")
            } catch (e: Throwable) {
                Log.w(TAG, "setRemoteClipboardImage failed: ${e.message}")
            }
        }
    }

    /**
     * 远端文件接收完成：把文件复制到公共「下载/EasyTier」目录并通知用户。
     * 由 Rust 反射调用（落盘发生在应用私有目录，这里转存到用户可见的位置）。
     */
    @JvmStatic
    fun onFileReceived(path: String) {
        val ctx = appContext ?: return
        runOnMain {
            try {
                val src = File(path)
                if (!src.exists() || !src.isFile) return@runOnMain
                val name = src.name
                val mime = guessMime(name)
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                    // Android 10+：走 MediaStore，写入公共 Downloads/EasyTier 无需存储权限
                    val values = ContentValues().apply {
                        put(MediaStore.MediaColumns.DISPLAY_NAME, name)
                        put(MediaStore.MediaColumns.MIME_TYPE, mime)
                        put(MediaStore.MediaColumns.RELATIVE_PATH, Environment.DIRECTORY_DOWNLOADS + "/EasyTier")
                    }
                    val uri = ctx.contentResolver.insert(MediaStore.Downloads.EXTERNAL_CONTENT_URI, values)
                    if (uri != null) {
                        ctx.contentResolver.openOutputStream(uri)?.use { out ->
                            src.inputStream().use { it.copyTo(out) }
                        }
                    }
                } else {
                    // Android 9-：直接写公共下载目录（清单里声明了 maxSdkVersion=28 的存储权限）
                    val dir = File(
                        Environment.getExternalStoragePublicDirectory(Environment.DIRECTORY_DOWNLOADS),
                        "EasyTier"
                    )
                    if (!dir.exists()) dir.mkdirs()
                    src.copyTo(File(dir, name), overwrite = true)
                }
                showNotification(ctx, "文件已接收", "已保存到 下载/EasyTier/$name")
            } catch (e: Throwable) {
                Log.w(TAG, "onFileReceived failed: ${e.message}")
            }
        }
    }

    /** 简易 MIME 猜测（用扩展名查系统表，兜底 application/octet-stream）。 */
    private fun guessMime(name: String): String {
        val ext = MimeTypeMap.getFileExtensionFromUrl(name).lowercase()
        return MimeTypeMap.getSingleton().getMimeTypeFromExtension(ext) ?: "application/octet-stream"
    }

    /** 通知渠道统一入口（文件接收/分享结果提示）。 */
    private fun showNotification(ctx: Context, title: String, text: String) {
        try {
            val nm = ctx.getSystemService(Context.NOTIFICATION_SERVICE) as NotificationManager
            val channelId = "easyshare_transfer"
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                nm.createNotificationChannel(
                    NotificationChannel(channelId, "传输与同步", NotificationManager.IMPORTANCE_DEFAULT)
                )
            }
            val notification = NotificationCompat.Builder(ctx, channelId)
                .setSmallIcon(android.R.drawable.stat_sys_download_done)
                .setContentTitle(title)
                .setContentText(text)
                .setAutoCancel(true)
                .build()
            nm.notify((title + text).hashCode(), notification)
        } catch (e: Throwable) {
            Log.w(TAG, "showNotification failed: ${e.message}")
        }
    }

    /**
     * 打开系统「无障碍」设置页。由 Rust 反射调用（前端点击"授权"按钮触发）。
     *
     * Android 10 起后台应用读不到剪贴板，只有无障碍服务例外，因此这是移动端
     * 剪贴板同步能真正工作的前提。
     */
    @JvmStatic
    fun openAccessibilitySettings() {
        val ctx = appContext ?: return
        try {
            val intent = Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)
            intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            ctx.startActivity(intent)
        } catch (e: Throwable) {
            Log.w(TAG, "openAccessibilitySettings failed: ${e.message}")
        }
    }

    /** 查询本应用的无障碍服务是否已被用户开启。由 Rust 反射调用。 */
    @JvmStatic
    fun isAccessibilityEnabled(): Boolean {
        val ctx = appContext ?: return false
        return try {
            val expected = ComponentName(ctx, ClipAccessibilityService::class.java)
                .flattenToString()
            val enabled = Settings.Secure.getString(
                ctx.contentResolver,
                Settings.Secure.ENABLED_ACCESSIBILITY_SERVICES
            ) ?: return false
            val splitter = TextUtils.SimpleStringSplitter(':')
            splitter.setString(enabled)
            var found = false
            while (splitter.hasNext()) {
                val item = splitter.next()
                if (item.equals(expected, ignoreCase = true)) {
                    found = true
                    break
                }
            }
            found
        } catch (e: Throwable) {
            Log.w(TAG, "isAccessibilityEnabled failed: ${e.message}")
            false
        }
    }

    /** 剪贴板写入必须在主线程，Rust 回调来自 tokio 工作线程，这里统一切回去。 */
    private fun runOnMain(block: () -> Unit) {
        android.os.Handler(android.os.Looper.getMainLooper()).post(block)
    }
}
