# 🚀 最后一步：启用 GitHub Actions 自动构建

源码已全部推送到 `https://github.com/hulu976/easytier-transfer`

但 **GitHub Actions workflow 文件**由于 GitHub 的安全限制（Fine-Grained PAT 不能写入 `.github/` 目录），需要你**手动操作 30 秒**。

---

## 📋 只需 3 步

### 第 1 步：创建 workflow 目录和文件

1. 打开 → https://github.com/hulu976/easytier-transfer/new/main
2. 在 **"Name your file..."** 输入框里，**完整输入**以下路径（含斜杠）：
   ```
   .github/workflows/build-android.yml
   ```
   > ⚠️ 必须包含 `.github/workflows/` 目录，GitHub 会自动创建

### 第 2 步：粘贴 workflow 内容

把下面这个代码块里的**全部内容**复制粘贴到编辑框：

```yaml
name: Build Android APK

on:
  workflow_dispatch:
    inputs:
      build_type:
        description: 'Build type'
        required: true
        default: 'debug'
        type: choice
        options:
          - debug
          - release
  push:
    branches: [main, master]

jobs:
  build:
    runs-on: ubuntu-latest

    env:
      ANDROID_HOME: /opt/android-sdk
      ANDROID_SDK_ROOT: /opt/android-sdk
      NDK_VERSION: "25.2.9519653"
      JAVA_HOME: /usr/lib/jvm/temurin-17-jdk-amd64

    steps:
      - name: Checkout code
        uses: actions/checkout@v4
        with:
          fetch-depth: 1

      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libgtk-3-dev \
            libsoup-3.0-dev \
            libjavascriptcoregtk-4.1-dev \
            pkg-config wget unzip openjdk-17-jdk

      - name: Setup Android SDK
        run: |
          mkdir -p $ANDROID_HOME && cd $ANDROID_HOME
          wget -q https://dl.google.com/android/repository/commandlinetools-linux-11076708_latest.zip -O cmdtools.zip
          mkdir -p cmdline-tools/latest
          unzip -q cmdtools.zip -d cmdline-tools/latest
          rm cmdtools.zip
          export PATH=$ANDROID_HOME/cmdline-tools/latest/bin:$PATH
          yes | sdkmanager --licenses > /dev/null 2>&1 || true
          sdkmanager "platform-tools" "platforms;android-34" "build-tools;34.0.0"

      - name: Setup Android NDK
        run: |
          cd $ANDROID_HOME
          wget -q https://dl.google.com/android/repository/android-ndk-r25c-linux.zip -O ndk.zip
          unzip -q ndk.zip && mv android-ndk-r25c ndk && rm ndk.zip
          echo "ANDROID_NDK_HOME=$ANDROID_HOME/ndk" >> $GITHUB_ENV

      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          toolchain: "1.75"
          targets: |
            aarch64-linux-android
            armv7-linux-androideabi
            x86_64-linux-android
            i686-linux-android

      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: "20"

      - name: Install pnpm
        uses: pnpm/action-setup@v4
        with:
          version: latest

      - name: Cache Rust
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: "easytier-gui/src-tauri"
          cache-on-failure: true

      - name: Cache Gradle
        uses: gradle/gradle-build-action@v3
        with:
          gradle-version: "8.5"

      - name: Install npm deps
        run: pnpm install --frozen-lockfile

      - name: Build frontend
        run: pnpm -F easytier-web build

      - name: Verify frontend
        run: |
          ls -la easytier-web/frontend-lib/dist/ 2>/dev/null || echo "NO DIST"
          grep -l "advanced_settings\|port_forwards\|clipboard_sync" easytier-web/frontend-lib/dist/*.js 2>/dev/null || echo "WARNING: Config keywords missing!"

      - name: Build APK (debug)
        if: github.event.inputs.build_type != 'release'
        working-directory: easytier-gui/src-tauri
        run: pnpm tauri android build --apk --debug

      - name: Build APK (release)
        if: github.event.inputs.build_type == 'release'
        working-directory: easytier-gui/src-tauri
        run: pnpm tauri android build --apk

      - name: Upload APK
        uses: actions/upload-artifact@v4
        with:
          name: easytier-transfer-${{ github.event.inputs.build_type || 'debug' }}
          path: |
            easytier-gui/src-tauri/gen/android/app/build/outputs/apk/**/*.apk
          retention-days: 30
```

### 第 3 步：提交

1. 滚动到底部
2. 填写 commit message：`Add Android build workflow`
3. 点 **Commit changes**
4. 等待 2-3 秒刷新页面

---

## ▶️ 触发构建

1. 打开 → https://github.com/hulu976/easytier-transfer/actions
2. 左边点 **Build Android APK**
3. 右边点 **Run workflow** 按钮
4. 选择 `debug`（推荐先试 debug）
5. 点绿色 **Run workflow**
6. 等待 **25-40 分钟**
7. 完成后点 **Artifacts** → 下载 `easytier-transfer-debug` zip
8. 解压得到 `app-universal-debug.apk`

---

## ✅ 验证清单

构建完成后，安装 APK 检查：

- [ ] 打开 App，能看到"easytier"网络
- [ ] 点进去能看到"编辑网络"界面
- [ ] **基础设置**：虚拟IP、网络名称、网络密码、初始节点
- [ ] **高级设置**：能展开，有内容
- [ ] **端口转发**：能展开
- [ ] **访问控制**：能展开
- [ ] **剪贴板同步**：开关可见 ✅（这是你的核心功能）
- [ ] 能正常"运行网络"

---

## 🔑 剪贴板同步使用提醒

Android 上剪贴板同步需要**开启无障碍服务权限**：
1. App 内打开剪贴板同步开关
2. 会弹出"去开启无障碍权限"提示
3. 跳转到系统设置 → 找到 **EasyTier Transfer**
4. 开启无障碍服务
5. 之后跨设备复制文字会自动同步

---

## ❓ 构建失败怎么办

打开 Actions 页面 → 点失败的 run → 看红色日志：

| 错误关键词 | 解决方案 |
|-----------|---------|
| `pnpm: command not found` | workflow 会自动装 pnpm，不会出这个错 |
| `ANDROID_HOME not set` | 检查 env 配置 |
| `Config keywords missing` | 前端构建有问题，检查 `pnpm -F easytier-web build` 步骤日志 |
| `tauri: command not found` | Rust target 没装全 |
| `NDK not found` | NDK 下载步骤失败，重试 |
| `403/401` | token 过期，更新 secrets |

---

**仓库地址**: https://github.com/hulu976/easytier-transfer
**Actions 页面**: https://github.com/hulu976/easytier-transfer/actions
