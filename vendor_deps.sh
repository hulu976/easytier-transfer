#!/bin/bash
# ═════════════════════════════════════════════════════════════════════
#   vendor_deps.sh — 离线依赖下载辅助脚本
#
#   用途: 在有网络的机器上运行，下载所有 Rust 依赖到 vendor/
#   之后整个目录可以打包带走，在离线机器上构建
#
#   用法: bash vendor_deps.sh
#
#   前置条件: Rust >= 1.95, 能访问 crates.io
# ═════════════════════════════════════════════════════════════════════

set -e

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
BLUE='\033[0;34m'; CYAN='\033[0;36m'; BOLD='\033[1m'; NC='\033[0m'
log()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()   { echo -e "${GREEN}[ OK ]${NC}  $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()  { echo -e "${RED}[FAIL]${NC}  $*"; }

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo -e "${BOLD}${CYAN}"
echo "  ╔════════════════════════════════════════════════════╗"
echo "  ║   EasyTier 传输版 — 离线依赖下载工具               ║"
echo "  ║   将 crates.io 依赖下载到 vendor/ 目录             ║"
echo "  ╚════════════════════════════════════════════════════╝"
echo -e "${NC}"

# ─── 1. 检查 Rust ───
step() { echo -e "\n${BOLD}${CYAN}━━━ $* ━━━${NC}"; }

step "检查 Rust 工具链"
if ! command -v cargo &>/dev/null; then
    err "cargo 未找到！请先安装 Rust:"
    echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    echo "  source \$HOME/.cargo/env"
    exit 1
fi
ok "cargo: $(cargo --version)"

# 检查 cargo-vendor 是否可用（Rust 1.61+ 内置）
if ! cargo vendor --help &>/dev/null; then
    err "cargo vendor 不可用，需要 Rust >= 1.61"
    echo "  更新 Rust: rustup update"
    exit 1
fi
ok "cargo vendor 可用"

# ─── 2. 安装 Android targets ───
step "安装 Android 编译目标"
if command -v rustup &>/dev/null; then
    for target in aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android; do
        log "安装 $target..."
        rustup target add "$target" 2>&1 | tail -1
    done
    ok "所有 Android targets 就绪"
else
    warn "rustup 不可用，跳过 target 安装"
    warn "如果构建时缺少 target，请安装 rustup"
fi

# ─── 3. 清理旧的 vendor 目录 ───
step "准备 vendor 和 prebuilt 目录"
if [ -d "vendor" ]; then
    warn "发现旧的 vendor/ 目录，删除重建..."
    rm -rf vendor
fi
mkdir -p vendor
mkdir -p prebuilt
ok "vendor/ 和 prebuilt/ 目录已就绪"

# ─── 3b. 下载 Gradle 离线包 ───
step "下载 Gradle 8.14.3 离线包"
GRADLE_ZIP="gradle-8.14.3-bin.zip"
GRADLE_URL="https://services.gradle.org/distributions/${GRADLE_ZIP}"
if [ -f "prebuilt/${GRADLE_ZIP}" ]; then
    ok "Gradle 离线包已存在，跳过下载"
else
    log "下载 ${GRADLE_ZIP} (~160MB)..."
    if command -v curl &>/dev/null; then
        curl -L --max-time 300 -o "prebuilt/${GRADLE_ZIP}" "${GRADLE_URL}" 2>&1 | tail -5
    elif command -v wget &>/dev/null; then
        wget --timeout=300 -O "prebuilt/${GRADLE_ZIP}" "${GRADLE_URL}" 2>&1 | tail -5
    else
        warn "未找到 curl 或 wget，跳过 Gradle 下载"
        warn "请手动下载: ${GRADLE_URL}"
        warn "放到 prebuilt/${GRADLE_ZIP}"
    fi
fi
if [ -f "prebuilt/${GRADLE_ZIP}" ] && [ -s "prebuilt/${GRADLE_ZIP}" ]; then
    GRADLE_SIZE=$(du -sh "prebuilt/${GRADLE_ZIP}" | cut -f1)
    ok "Gradle 离线包就绪: ${GRADLE_SIZE}"
else
    warn "Gradle 下载失败，构建时会尝试在线下载"
fi

# ─── 4. 运行 cargo vendor ───
step "下载所有 Rust 依赖 (从 crates.io)"
log "这可能需要 5-15 分钟，取决于网速..."
log "预计下载 ~200-300MB 到 vendor/ 目录"
echo ""
echo "─────────────────────────────────────"

# 使用 --locked 确保版本与 Cargo.lock 一致
# 使用 --versioned-dirs 让目录名包含版本号（与 .cargo/config.toml 匹配）
cargo vendor --locked --versioned-dirs vendor 2>&1 | tee output/vendor.log
echo "─────────────────────────────────────"

# ─── 5. 验证下载结果 ───
step "验证下载结果"
if [ -f "vendor/config.toml" ]; then
    ok "vendor/config.toml 已生成"
else
    err "vendor/config.toml 未生成，下载可能失败"
    echo "  查看日志: output/vendor.log"
    exit 1
fi

vendor_count=$(find vendor -maxdepth 2 -name "*.crate" 2>/dev/null | wc -l)
vendor_size=$(du -sh vendor | cut -f1)
ok "vendor 目录大小: $vendor_size"
log "包含约 $vendor_count 个 .crate 文件"

# ─── 6. 生成 .cargo/config.toml ───
step "生成离线构建配置"
mkdir -p .cargo
cat > .cargo/config.toml << 'CFGEOF'
# EasyTier Transfer — 离线构建配置 (由 vendor_deps.sh 自动生成)
# 此配置使 cargo 从本地 vendor/ 目录读取依赖，不访问网络

[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "../vendor"

# region Android
[target.aarch64-linux-android]
ar = "aarch64-linux-android-ar"
linker = "aarch64-linux-android-clang"

[target.armv7-linux-androideabi]
ar = "armv7-linux-androideabi-ar"
linker = "armv7-linux-androideabi-clang"

[target.x86_64-linux-android]
ar = "x86_64-linux-android-ar"
linker = "x86_64-linux-android-clang"

[target.i686-linux-android]
ar = "i686-linux-android-ar"
linker = "i686-linux-android-clang"
# endregion
CFGEOF
ok ".cargo/config.toml 已生成 (离线模式)"

# ─── 7. 打包指令 ───
step "全部完成！"
echo ""
ok "离线依赖准备完毕！"
echo ""
echo "  下一步操作："
echo "  1) 打包:  cd .. && zip -r easytier-offline.zip easytier-transfer-android/"
echo "  2) 拷贝:  把 easytier-offline.zip 拷到离线构建机"
echo "  3) 构建:  unzip → cd easytier-transfer-android → bash build.sh"
echo ""
echo "  注意: 离线构建机仍需安装 Rust >= 1.95 和 Android SDK/NDK"
echo "        vendor/ 只解决 crates.io 依赖，不解决编译工具链"
