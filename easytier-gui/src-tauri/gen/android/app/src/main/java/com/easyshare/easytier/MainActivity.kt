package com.easyshare.easytier

import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.provider.OpenableColumns
import android.util.Log
import java.io.File
import java.io.FileOutputStream

/**
 * 主 Activity。
 *
 * 除常规入口外，还处理系统「分享」面板的唤起（ACTION_SEND / ACTION_SEND_MULTIPLE）：
 * 用户在其他应用（相册、文件管理器等）选择文件后点「分享」→ 选 EasyTier，
 * 这里把选中的文件交给 [EasyShareBridge.sendSharedFiles] 传输给虚拟网内的对端。
 */
class MainActivity : TauriActivity() {
    private val tag = "MainActivity"

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        // 先注入应用上下文并把 JavaVM 缓存到 Rust 侧，
        // 否则远端剪贴板内容到达时无法回写系统剪贴板。
        EasyShareBridge.init(this)
        initService()
        handleShareIntent(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleShareIntent(intent)
    }

    private fun initService() {
        val serviceIntent = Intent(this, MainForegroundService::class.java)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            startForegroundService(serviceIntent)
        } else {
            startService(serviceIntent)
        }
    }

    /** 系统分享面板唤起：解析 EXTRA_STREAM 里的文件 URI 并发起传输。 */
    private fun handleShareIntent(intent: Intent?) {
        val action = intent?.action ?: return
        if (action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) return

        val uris: List<Uri> = if (action == Intent.ACTION_SEND) {
            val single = intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
            if (single != null) listOf(single) else emptyList()
        } else {
            intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM) ?: emptyList()
        }
        if (uris.isEmpty()) {
            Log.w(tag, "share intent without EXTRA_STREAM")
            return
        }

        val paths = uris.mapNotNull { uriToLocalPath(it) }
        if (paths.isNotEmpty()) {
            EasyShareBridge.sendSharedFiles(paths)
        } else {
            Log.w(tag, "share intent: none of the URIs could be resolved")
        }
    }

    /** 把分享的 URI 转为本地可读路径：file:// 直接取路径，content:// 拷贝到 cache。 */
    private fun uriToLocalPath(uri: Uri): String? {
        return try {
            if (uri.scheme == "file") {
                uri.path
            } else {
                val name = queryDisplayName(uri) ?: "share_${System.currentTimeMillis()}"
                val out = File(cacheDir, "share/$name")
                out.parentFile?.mkdirs()
                contentResolver.openInputStream(uri)?.use { input ->
                    FileOutputStream(out).use { output -> input.copyTo(output) }
                }
                out.absolutePath
            }
        } catch (e: Exception) {
            Log.w(tag, "uriToLocalPath($uri) failed: ${e.message}")
            null
        }
    }

    private fun queryDisplayName(uri: Uri): String? {
        return try {
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { c -> if (c.moveToFirst()) c.getString(0) else null }
        } catch (e: Exception) {
            null
        }
    }
}
