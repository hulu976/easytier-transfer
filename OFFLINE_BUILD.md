# EasyTier 传输版 — Android 共存版 离线构建指南

## 包名
`com.easyshare.easytier`（与官方 `com.kkrainbow.easytier` 完全共存）

## 关于"构建后选项全没了"的诊断

如果构建出来的 App 界面只剩基础输入框（虚拟IP/网络名/密码），**高级设置、端口转发、访问控制、剪贴板同步全部消失**，按以下步骤排查：

### 第 1 步：确认是"折叠面板"还是"真的没渲染"

App 启动 → 点击绿色 **+** 按钮创建网络 → 进入编辑模式后，向下滚动查看：

- ✅ **能看到 "高级设置"、"端口转发"、"访问控制"、"剪贴板同步" 这些标题行**（左边有个小三角 ▼）→ 这是**折叠面板默认收起**，点击标题即可展开，属于正常 UI 设计
- ❌ **完全看不到这些标题行** → 前端构建有问题，继续排查

### 第 2 步：检查前端构建日志

```bash
# 查看前端构建是否报错
cat output/frontend_build.log

# 如果文件不存在或为空，重新构建看实时输出
rm -rf easytier-web/frontend-lib/dist easytier-web/frontend/dist
pnpm install
pnpm -F easytier-web build
```

常见错误和修复：

| 错误信息 | 原因 | 修复 |
|---------|------|------|
| `Cannot find module 'easytier-frontend-lib'` | workspace 链接失败 | `rm -rf node_modules && pnpm install` |
| `PrimeVue component not found` | 依赖版本不对 | 检查 `pnpm-lock.yaml` 是否存在且完整（应 8000+ 行） |
| `vue-tsc: command not found` | devDep 没装 | `pnpm install` 时查看是否有报错 |
| `Type error in Config.vue` | TypeScript 类型不匹配 | 检查 `types/network.ts` 是否完整 |

### 第 3 步：验证前端产物

```bash
# 检查构建产物是否包含关键代码
grep -r "advanced_settings" easytier-web/frontend-lib/dist/ | head -3
grep -r "port_forwards" easytier-web/frontend-lib/dist/ | head -3
grep -r "clipboard_sync" easytier-web/frontend-lib/dist/ | head -3
```

如果以上命令**没有任何输出**，说明 Config.vue 没被编译进去 → 检查 `easytier-web/frontend-lib/src/components/Config.vue` 是否存在且非空。

### 第 4 步：诊断手机上的 App

```bash
# 用 Chrome 远程调试查看手机 WebView 控制台
# 手机打开 USB 调试 → 连接电脑 → Chrome 访问 chrome://inspect
# 点击 EasyTier App → Inspect → 查看 Console 错误
```

常见运行时错误：

| 错误 | 原因 |
|------|------|
| `Cannot read properties of undefined (reading 't')` | i18n 语言文件没加载 → 检查 `locales/cn.yaml` 和 `locales/en.yaml` |
| `PrimeVue plugin not installed` | main.ts 没正确注册 PrimeVue |
| 白屏 / 全空 | JS bundle 加载失败 → 检查 `dist/` 产物是否被正确打包进 APK |

## 正常 UI 结构（构建成功后）

点击 **+** 创建网络后，应看到：

```
┌─────────────────────────────────┐
│  [easytier ▼]        [+]  [...] │  ← 网络选择 + 操作按钮
├─────────────────────────────────┤
│  编辑网络                        │
│  [编辑为文件] [导入配置] [保存] │
│  ─────────────────────────────  │
│  ▼ 基础设置                     │  ← 默认展开
│     虚拟 IPv4  [DHCP ✓]         │
│     网络名称  [________]         │
│     网络密码  [________]         │
│     初始节点  [+ 添加]           │
│  ▶ 高级设置                     │  ← 点击展开 → 20+ 个开关
│  ▶ 端口转发                     │  ← 点击展开 → 添加转发规则
│  ▶ 访问控制 (ACL)              │  ← 点击展开 → ACL 规则
│  ▶ 剪贴板同步                  │  ← 点击展开 → 启用同步/文件传输
│                                 │
│  [运行网络]  ← 绿色大按钮       │
└─────────────────────────────────┘
```

## 离线构建流程

### 在有网络的机器上（一次性准备）

```bash
unzip easytier-android-offline.zip
cd easytier-transfer-android

# 1. 检查环境
bash build.sh check

# 2. 下载 Rust 依赖（~200-300MB，5-15分钟）
bash build.sh vendor

# 3. 下载 Gradle（如果 script/ 里有 prebuilt/ 机制）
# （Gradle 通常由 Tauri 自动下载，首次需要网络）

# 4. 打包整个目录
cd ..
zip -r easytier-offline-full.zip easytier-transfer-android/
```

### 在离线构建机上

```bash
unzip easytier-offline-full.zip
cd easytier-transfer-android

# 设置环境变量
export ANDROID_HOME=$HOME/Android/Sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/25.2.9519653

# 一键构建（不访问任何网络）
bash build.sh

# 安装到手机
adb install -r output/*.apk
```

## 环境要求

| 工具 | 最低版本 | 安装方式 |
|------|---------|---------|
| Rust | 1.95 | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Node.js | 18 | https://nodejs.org/ |
| pnpm | latest | `npm install -g pnpm` |
| JDK | 17 | `sudo apt install openjdk-17-jdk` |
| Android SDK | latest | Android Studio |
| Android NDK | 25+ | SDK Manager 安装 |

## 文件清单

```
easytier-transfer-android/
├── Cargo.toml              # Workspace 根
├── Cargo.lock              # Rust 依赖锁定
├── package.json            # pnpm workspace 根
├── pnpm-workspace.yaml     # Workspace 包列表
├── pnpm-lock.yaml          # npm 依赖锁定 (8409 行)
├── rust-toolchain.toml      # Rust 1.95
├── build.sh                # 一键构建脚本（增强诊断版）
├── README.md               # 项目说明
├── OFFLINE_BUILD.md        # 本文件
│
├── easytier/              # 核心库（官方 v2.6.4 完整源码）
├── easyshare-lib/         # 🆕 传输核心（剪贴板+文件传输）
├── easytier-gui/          # Tauri GUI
│   ├── src/               # Vue 前端
│   └── src-tauri/         # Rust 后端 + Android 工程
├── easytier-web/          # 前端共享库
│   ├── frontend-lib/      # 核心组件库（Config/RemoteManagement/ACL等）
│   └── frontend/          # Web 版入口
├── easytier-rpc-build/    # protobuf 生成
├── easytier-contrib/      # ffi / uptime / android-jni
├── tauri-plugin-vpnservice/  # Android VPN Service 插件
├── .cargo/config.toml     # Cargo 配置（离线模式用）
├── vendor/                # （构建后生成）Rust 离线依赖
└── output/                # （构建后生成）APK 产物
```
