# 🚀 构建 APK — 操作步骤

## 问题
GitHub 不允许通过 API/PAT 修改 `.github/workflows/` 目录（安全限制）。
所以你需要**手动**把构建脚本复制到正确位置。

## 操作步骤（30秒）

### 第 1 步：打开构建脚本
👉 https://github.com/hulu976/easytier-transfer/blob/main/build-android-final.yml

全选复制**全部内容**（Ctrl+A → Ctrl+C）

### 第 2 步：创建 workflow 文件
👉 https://github.com/hulu976/easytier-transfer/new/main

在文件名输入框输入：
```
.github/workflows/main.yml
```

把刚才复制的内容**粘贴**到编辑器里。

页面底部：
- Commit message 填：`fix: 最终修复版构建脚本`
- 点绿色 **Commit changes**

### 第 3 步：触发构建
👉 https://github.com/hulu976/easytier-transfer/actions

1. 左侧点 **Build Android APK**
2. 右侧点 **Run workflow** 按钮
3. 分支选 `main` → 点 **Run workflow**
4. ⏳ 等待 25-40 分钟

### 第 4 步：下载 APK
构建完成后：
1. 点进绿色的构建记录
2. 页面底部 **Artifacts** 区域
3. 点 `easytier-transfer-android` 下载
4. 安装：`adb install -r easytier-transfer-android.apk`

## 这个 workflow 修复了什么

| 之前的错误 | 修复方法 |
|-----------|---------|
| `sdkmanager: 找不到命令` | 手动下载 cmdline-tools + 显式加 PATH |
| `ERR_PNPM_OUTDATED_LOCKFILE` | 加 `--no-frozen-lockfile` |
| Rust targets 缺失 | 显式 `rustup target add` 4 个架构 |
| 前端编译失败静默 | 加验证步骤，grep 关键词 |
| OOM 内存不足 | 只编译 arm64（最快最省内存）|
| NDK 路径不对 | 硬编码 `ANDROID_NDK_HOME` 环境变量 |

## 如果还是失败

把**红色步骤展开后的日志截图**发给我，我继续修。
