# EasyTier 传输版 — Android 共存版

基于 **EasyTier v2.6.4** 改造，增加 **剪贴板同步 + 文件传输** 功能。

## 包名

```
com.easyshare.easytier
```

与官方 `com.kkrainbow.easytier` **完全共存**，可同时安装互不干扰。

## 快速构建

```bash
# 1. 设置环境变量
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/25.2.9519653

# 2. 一键构建
bash build.sh

# 3. 安装到设备
adb install -r output/*.apk
```

## 前置依赖

| 工具 | 最低版本 | 安装方式 |
|------|----------|----------|
| Rust | 1.95 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Node.js | 18 | https://nodejs.org/ |
| pnpm | latest | `npm install -g pnpm` |
| JDK | 17 | `sudo apt install openjdk-17-jdk` |
| Android SDK | 34+ | Android Studio → SDK Manager |
| Android NDK | 25+ | SDK Manager → NDK (Side by side) |

## 项目结构

```
easytier-transfer-android/
├── build.sh                  ← 一键构建脚本
├── Cargo.toml                ← Rust workspace 配置
├── rust-toolchain.toml       ← Rust 1.95
├── .cargo/config.toml        ← 仅 Android target 配置
├── package.json              ← pnpm workspace
├── pnpm-workspace.yaml
│
├── easytier/                 ← 核心库 (官方 v2.6.4 完整源码)
├── easyshare-lib/            ← 🆕 传输核心 (剪贴板+文件传输)
│   ├── src/
│   │   ├── lib.rs            ← API 入口: start/stop/send_file
│   │   ├── api.rs            ← 对外 API (JNI 调用入口)
│   │   ├── server.rs         ← TCP 传输服务端
│   │   ├── client.rs         ← TCP 传输客户端
│   │   ├── clipboard.rs      ← 剪贴板监听/广播
│   │   ├── file_transfer.rs  ← 文件分块传输
│   │   ├── peer_discovery.rs ← 节点发现 (从路由表读取)
│   │   ├── handler.rs        ← 消息处理器
│   │   ├── proto.rs          ← protobuf 编解码
│   │   └── android.rs        ← Android JNI 桥接
│   ├── proto/easyshare.proto ← protobuf 定义
│   └── Cargo.toml
│
├── easytier-gui/             ← Tauri GUI + Android 工程
│   ├── src/                  ← 前端 Vue/TS
│   │   └── composables/
│   │       └── backend.ts    ← 后端通信
│   └── src-tauri/            ← Rust 后端
│       ├── tauri.conf.json   ← identifier: com.easyshare.easytier
│       ├── src/lib.rs        ← 进程内启动 easyshare
│       └── gen/android/      ← Android 原生工程
│           └── app/src/main/java/com/easyshare/easytier/
│               ├── MainActivity.kt
│               ├── ClipAccessibilityService.kt
│               └── EasyShareBridge.kt
│
├── easytier-web/             ← 前端共享库
│   └── frontend-lib/src/
│       ├── modules/clipboardSync.ts
│       └── locales/{cn,en}.yaml
│
├── easytier-rpc-build/       ← protobuf 代码生成
├── easytier-contrib/
│   ├── easytier-android-jni/ ← Android JNI 桥
│   ├── easytier-ffi/
│   └── easytier-uptime/
└── tauri-plugin-vpnservice/  ← VPN Service 插件 (Android 必需)
```

## 构建脚本用法

```bash
bash build.sh           # 完整构建 APK (默认)
bash build.sh check     # 仅检查环境是否就绪
bash build.sh clean     # 清理所有构建缓存
bash build.sh help      # 查看帮助
```

## 关于"创建网络"问题

旧版 easyshare 启动时绑定虚拟 IP（如 `10.144.144.1`），但此时 tun 网卡尚未就绪，导致监听失败、整个传输服务挂掉，前端表现为"创建网络失败"。

**当前版本已修复**：`easyshare::api::start()` 改为绑定 `0.0.0.0`，不依赖虚拟网卡状态。tun 就绪后自动可通。

## 已知限制

- 剪贴板同步需要 Android 无障碍服务权限（首次使用需在系统设置中手动开启）
- 文件传输依赖同一 EasyTier 网络内的节点发现
- 仅支持 Android 8.0+ (API 26+)

## License

与 EasyTier 一致 (Apache-2.0 / MIT)
