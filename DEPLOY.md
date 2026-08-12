# 🚀 部署到 GitHub + 云端构建

## ⚠️ 上次推送失败的根本原因

| 问题 | 说明 |
|------|------|
| **Token 类型错误** | 使用的是 Fine-Grained PAT (`github_pat_...`)，该类型 token **仅支持 API 只读访问**，不能用于 `git push` |
| **缺少 `.github/workflows/`** | 之前的包里没有 GitHub Actions 工作流文件，即使推送成功也无法云端构建 |
| **SSH 端口被限制** | 默认 SSH 端口 22 可能被网络防火墙拦截 |

### ✅ 本次修复

| 修复项 | 说明 |
|--------|------|
| **改用 SSH 密钥对** | `DEPLOY_KEY`（私钥）+ `DEPLOY_KEY.pub`（公钥），SSH 认证不受 PAT 限制 |
| **SSH 走 443 端口** | `ssh_config` 已配置 `Port 443`（ssh.github.com），绕过防火墙 |
| **新增 GitHub Actions workflow** | `.github/workflows/build-android.yml` 自动安装全部依赖 → 构建 APK → 上传产物 |
| **推送脚本增强** | 自动检测 SSH key、测试连接、初始化 git、推送、引导触发构建 |

---

## 第 1 步：添加 SSH Key 到 GitHub

1. 打开 https://github.com/settings/keys
2. 点 **New SSH key**
3. Title 填 `easytier-builder`
4. 打开本包的 `DEPLOY_KEY.pub` 文件，**复制全部内容**粘贴到 Key 框
5. 点 **Add SSH key**

> `DEPLOY_KEY.pub` 内容（也可直接复制）：
> ```
> ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIFdqMqX03jl7IFYwWatHq8jl/VcjH+FioSgG/5PFRRX9 yuanbao-builder
> ```

## 第 2 步：推送代码到 GitHub

### Linux / macOS：
```bash
chmod +x push_to_github.sh
bash push_to_github.sh
```

### Windows：
双击 `push_windows.bat`，或 CMD 中执行：
```
push_windows.bat
```

### 仅检查环境（不推送）：
```bash
bash push_to_github.sh check
```

## 第 3 步：触发云端构建

推送成功后：

1. 打开 https://github.com/hulu976/easytier-transfer-/actions
2. 左侧点 **Build Android APK**
3. 右侧点 **Run workflow** 按钮
4. 选择 `build_type`：
   - `debug` — 快速构建，约 25 分钟
   - `release` — 签名构建，约 40 分钟
5. 点绿色 **Run workflow**
6. 等待构建完成（Actions 页面可看实时日志）
7. 构建完成后，页面底部 **Artifacts** 区域下载 APK

## 第 4 步：安装到手机

```bash
# 下载的 APK 文件名类似：
# easytier-transfer-android-debug.zip

unzip easytier-transfer-android-debug.zip
adb install -r *.apk
```

---

## 🔍 故障排查

| 问题 | 解决方案 |
|------|----------|
| `Permission denied (publickey)` | SSH key 没加到 GitHub，回第 1 步 |
| `Connection refused` / 超时 | 网络不通 GitHub，检查代理/防火墙 |
| `ssh: connect to host github.com port 22` | 确认 `ssh_config` 中 `Port 443` 生效 |
| Actions 页面显示 "No workflows" | 代码没推成功，检查 `git log` 和远程 |
| 构建失败：`cargo build` 报错 | 查看 Actions 日志，通常是 Rust 版本问题（workflow 已固定 1.75） |
| 构建失败：`pnpm install` 报错 | 查看 Actions 日志，可能是 lockfile 不匹配 |

---

## 📁 包内文件清单

```
easytier-android-fixed/
├── DEPLOY.md                    ← 本文档
├── DEPLOY_KEY                   ← SSH 私钥（不要泄露）
├── DEPLOY_KEY.pub               ← SSH 公钥（加到 GitHub）
├── ssh_config                   ← SSH 443 端口配置
├── push_to_github.sh            ← Linux/macOS 推送脚本
├── push_windows.bat             ← Windows 推送脚本
└── easytier-transfer-android/   ← 完整源码（782 文件）
    ├── .github/workflows/
    │   └── build-android.yml    ← 🆕 GitHub Actions 构建流水线
    ├── Cargo.toml               ← Workspace（含 easyshare-lib）
    ├── rust-toolchain.toml      ← Rust 1.95
    ├── build.sh                  ← 本地构建脚本
    ├── easytier/                 ← 核心库（v2.6.4 完整源码）
    ├── easyshare-lib/           ← 🆕 传输核心（剪贴板+文件传输）
    ├── easytier-gui/            ← Tauri GUI + Android 工程
    ├── easytier-web/            ← 前端共享库
    └── ...
```

---

## 🔒 安全提醒

- `DEPLOY_KEY` 是私钥，**不要提交到公开仓库**（本仓库已是公开，建议用完后删除这个 key）
- 构建完成后去 https://github.com/settings/keys 删除 `easytier-builder` 密钥
- 之后每次构建可以重新生成新的 SSH key pair
