# Add project specific ProGuard rules here.
# You can control the set of applied configuration files using the
# proguardFiles setting in build.gradle.
#
# For more details, see
#   http://developer.android.com/guide/developing/tools/proguard.html

# If your project uses WebView with JS, uncomment the following
# and specify the fully qualified class name to the JavaScript interface
# class:
#-keepclassmembers class fqcn.of.javascript.interface.for.webview {
#   public *;
#}

# Uncomment this to preserve the line number information for
# debugging stack traces.
#-keepattributes SourceFile,LineNumberTable

# If you keep the line number information, uncomment this to
# hide the original source file name.
#-renamesourcefileattribute SourceFile

# ---- EasyShare 剪贴板同步 ----
# Rust 侧通过 JNI 反射按名字查找这些类与方法，混淆后会找不到，必须整体保留。
-keep class com.kkrainbow.easytier.EasyShareBridge { *; }
-keep class com.kkrainbow.easytier.EasyShareBridge$* { *; }
-keep class com.kkrainbow.easytier.ClipAccessibilityService { *; }
# 所有 native 方法及其声明类
-keepclasseswithmembernames class * {
    native <methods>;
}
