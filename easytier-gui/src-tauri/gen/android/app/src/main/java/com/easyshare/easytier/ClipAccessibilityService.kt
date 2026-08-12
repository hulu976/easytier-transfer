package com.easyshare.easytier

import android.accessibilityservice.AccessibilityService
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.net.Uri
import android.util.Log
import android.view.accessibility.AccessibilityEvent
import java.io.ByteArrayOutputStream

/**
 * 剪贴板监听无障碍服务。
 *
 * 为什么必须用无障碍服务：Android 10（API 29）起，只有获得焦点的前台应用才能
 * 读取剪贴板；后台服务调用 `getPrimaryClip()` 一律返回空。无障碍服务是系统留
 * 的少数例外之一，所以跨设备剪贴板同步在安卓上只能走这条路。
 *
 * 职责很薄：监听 [ClipboardManager] 变化 -> 判重/防回环 -> 交给
 * [EasyShareBridge] 经 JNI 广播出去。真正的网络收发都在 Rust 侧。
 */
class ClipAccessibilityService : AccessibilityService() {

    companion object {
        private const val TAG = "ClipAccessibility"

        /** 图片同步上限，超过则跳过（避免把超大截图塞进虚拟网络）。 */
        private const val MAX_IMAGE_BYTES = 4 * 1024 * 1024

        @Volatile
        var instance: ClipAccessibilityService? = null
            private set
    }

    private var clipboardManager: ClipboardManager? = null
    private var lastText: String? = null
    private var lastImageSize: Int = -1

    private val clipListener = ClipboardManager.OnPrimaryClipChangedListener {
        onClipboardChanged()
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
        EasyShareBridge.init(this)

        clipboardManager =
            (getSystemService(Context.CLIPBOARD_SERVICE) as? ClipboardManager)?.also {
                it.addPrimaryClipChangedListener(clipListener)
            }
        Log.i(TAG, "clipboard listener attached")
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        // 只借用无障碍服务的剪贴板读取特权，不消费界面事件
    }

    override fun onInterrupt() {}

    override fun onDestroy() {
        clipboardManager?.removePrimaryClipChangedListener(clipListener)
        clipboardManager = null
        instance = null
        super.onDestroy()
    }

    private fun onClipboardChanged() {
        val cm = clipboardManager ?: return
        val clip = try {
            cm.primaryClip
        } catch (e: Throwable) {
            Log.w(TAG, "read clipboard failed: ${e.message}")
            null
        } ?: return

        if (clip.itemCount <= 0) return
        val item = clip.getItemAt(0)
        val desc: ClipDescription? = clip.description

        // 图片：URI 型剪贴项且 MIME 为 image/*
        val uri: Uri? = item.uri
        val isImage = uri != null && desc != null &&
            (0 until desc.mimeTypeCount).any { desc.getMimeType(it).startsWith("image/") }

        if (isImage && uri != null) {
            handleImage(uri)
            return
        }

        val text = item.coerceToText(this)?.toString().orEmpty()
        if (text.isEmpty()) return
        if (text == lastText || EasyShareBridge.isEchoText(text)) return

        lastText = text
        try {
            EasyShareBridge.nativeSendClipboard(text)
            Log.i(TAG, "broadcast text (${text.length} chars)")
        } catch (e: Throwable) {
            Log.w(TAG, "nativeSendClipboard failed: ${e.message}")
        }
    }

    private fun handleImage(uri: Uri) {
        try {
            val bytes = contentResolver.openInputStream(uri)?.use { input ->
                val bitmap: Bitmap? = BitmapFactory.decodeStream(input)
                if (bitmap == null) return
                ByteArrayOutputStream().use { out ->
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
                    out.toByteArray()
                }
            } ?: return

            if (bytes.size > MAX_IMAGE_BYTES) {
                Log.i(TAG, "image too large (${bytes.size} bytes), skip")
                return
            }
            if (bytes.size == lastImageSize || EasyShareBridge.isEchoImage(bytes.size)) return

            lastImageSize = bytes.size
            EasyShareBridge.nativeSendClipboardImage(bytes)
            Log.i(TAG, "broadcast image (${bytes.size} bytes)")
        } catch (e: Throwable) {
            Log.w(TAG, "handleImage failed: ${e.message}")
        }
    }
}
