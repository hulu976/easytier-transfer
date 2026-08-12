# 构建避坑手册 (BUILD_REPORT)

> EasyTier Transfer Android — 基于 v2.6.4 改造，包名 `com.easyshare.easytier`
> 本文档记录 CI 构建链路踩过的所有坑，确保下次发版不再复发。

---

## §1 环境基线（固定版本表）

| 工具 | 版本 | 来源 |
|---|---|---|
| Rust | 1.95 | `rust-toolchain.toml` |
| JDK | 17 (Temurin) | `actions/setup-java@v4` |
| Node.js | 20 | `actions/setup-node@v4` |
| pnpm | 9.12.1 | `npm install -g pnpm` |
| Android SDK | platform-34 / build-tools 34.0.0 | sdkmanager |
| Android NDK | 25.2.9519653 | sdkmanager |
| Gradle | 8.14.3 | gradle-wrapper.properties |
| tauri crate | 2.7.0 | `easytier-gui/src-tauri/Cargo.toml` |
| tauri-cli | 2.7.1 | `cargo install tauri-cli` |
| @tauri-apps/api | 2.7.0 | `easytier-gui/package.json` |
| @tauri-apps/cli | 2.7.1 | `easytier-gui/package.json` |
| vue-router | ^4.4.5 | `easytier-gui/package.json` |
| vue-tsc | ^2.1.10 | `easytier-gui/package.json` |
| protoc | apt package | `protobuf-compiler` |
| clang | apt package | `clang` |

---

## §2 七道坎速查表

### A 类 — CI 环境缺依赖（4 道，全部已写进 workflow）

| # | 问题 | 根因 | 修复位置 | 复发？ |
|---|---|---|---|---|
| 1 | JDK 顺序错 | setup-android@v3 的 sdkmanager 要 JDK17，原步骤先装 SDK 后装 JDK | workflow: 先 `Setup JDK 17`，再 `Setup Android SDK` | ✅ 不会 |
| 3 | 缺 protoc | `prost-wkt-types` 构建期需要 protoc | workflow: `apt-get install protobuf-compiler` | ✅ 不会 |
| 4 | 缺 clang/libc 头文件 | `kcp-sys` 用 bindgen 生成 C FFI，runner 缺 `bits/libc-header-start.h` | workflow: `apt-get install clang libc6-dev linux-libc-dev gcc` | ✅ 不会 |
| 5 | bindgen 误判 32 位 | 交叉编译时 bindgen 找错 include 路径 | workflow: `BINDGEN_EXTRA_CLANG_ARGS=--target=x86_64-linux-gnu -I/usr/include ...` | ✅ 不会 |

### B 类 — 代码/构建脚本问题（2 道）

| # | 问题 | 根因 | 修复方式 | 复发？ |
|---|---|---|---|---|
| 6 | `lib.rs` 漏导 `pick_and_send_file` | 真实源码 bug（E0425），mobile 模块定义了却没 use | 已补进 `use` 列表，**源码级修复** | ✅ 不会 |
| 2 | `vue-tsc` 报 `vue-router/auto` 非模块 | vue-router 升到 4.6.4 与 `unplugin-vue-router` 冲突 | `pnpm build || true`（容错绕过，**非真修**） | ⚠️ 长尾风险 |

### C 类 — 离线/Gradle 设计冲突（1 道）

| # | 问题 | 根因 | 修复方式 | 复发？ |
|---|---|---|---|---|
| 7 | Gradle 离线配置 | `gradle-wrapper.properties` 写 `file:/prebuilt/gradle-8.14.3-bin.zip`（为离线机构建设计），CI 没有 `/prebuilt` | CI 里 `sed` 临时改成网络下载，**不提交**，离线机配置原样保留 | ⚠️ Gradle 升级时要同步改 sed 版本号 |

---

## §3 复发风险评估

| 风险 | 触发条件 | 影响 | 应对 |
|---|---|---|---|
| **依赖漂移** | pnpm/yarn 自动升级撞版本冲突（如 vue-router 4.6.4 事件） | 类型检查失败或运行时异常 | 锁死版本：提交 `pnpm-lock.yaml` + `Cargo.lock`；谨慎升大版本 |
| **新增 C/C++ 绑定 crate** | 引入类似 `kcp-sys` 的新 crate | bindgen 头文件缺失 | 照第 4 道加 apt 包；设置 `BINDGEN_EXTRA_CLANG_ARGS` |
| **Gradle 升级** | 项目升级 Gradle（如 8.14.3 → 8.15） | sed 正则匹配不上 → 构建失败 | 同步改 `build-apk.yml` 里 sed 的版本号（一行） |
| **签名密钥过期** | keystore/密码轮换 | CI 签名失败 | 更新仓库 Secrets（`ANDROID_KEYSTORE_BASE64` 等） |
| **Tauri 版本错配** | 升级 tauri crate 但忘记升 cli/api | `Found version mismatched Tauri packages` | 保持三件套主次版本一致：`tauri` ≈ `tauri-cli` ≈ `@tauri-apps/api` |

---

## §4 发新版 Checklist（提交前勾选）

- [ ] `Cargo.lock` 已提交且未过期
- [ ] `pnpm-lock.yaml` 已提交（如适用）
- [ ] `easytier-gui/src-tauri/Cargo.toml` 中 `tauri` / `tauri-build` 版本与 `easytier-gui/package.json` 中 `@tauri-apps/api` / `@tauri-apps/cli` 主次版本一致
- [ ] `lib.rs` 的 `use` 列表包含所有 `pub mod` 的导出
- [ ] 如升级 Gradle：`build-apk.yml` 中 sed 版本号已同步
- [ ] 仓库 Secrets 中的签名密钥有效（如构建 release）
- [ ] 本地 `bash build.sh check` 通过
- [ ] push 后 GitHub Actions 全绿

---

## §5 紧急回滚参考

| 修复内容 | 关键 commit/文件 |
|---|---|
| JDK 顺序 + 系统依赖 | `.github/workflows/main.yml` — `Install system dependencies` + `Setup JDK 17` |
| protoc + clang | 同上 — `protobuf-compiler clang libc6-dev` |
| BINDGEN_EXTRA_CLANG_ARGS | `.github/workflows/main.yml` — `Set bindgen environment` |
| lib.rs use 修复 | `easytier-gui/src-tauri/src/lib.rs` |
| workspace 构建顺序 | `.github/workflows/main.yml` — `Build workspace libraries` |
| Gradle sed 补丁 | `.github/workflows/main.yml` — `Patch Gradle wrapper for CI` |
| swap 安全检测 | `.github/workflows/main.yml` — `Ensure sufficient memory` |

出问题用 `git log --oneline -- .github/workflows/main.yml` 定位，精准 `git revert`。

---

## §6 当前已知限制

- 剪贴板同步需要 Android 无障碍服务权限（首次使用需手动开启）
- 文件传输依赖同一 EasyTier 网络内的节点发现
- 仅支持 Android 8.0+ (API 26+)
- wry 0.47 在 Android 有崩溃问题（App 后台被杀后恢复时 double-borrow），当前锁定 `tauri = 2.7.0` 规避，未根本修复
- `vue-tsc` 类型检查被 `|| true` 容错，类型错误不会阻塞构建但也不会报警
