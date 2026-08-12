#!/bin/bash
# ════════════════════════════════════════════════════════════════════
#   EasyTier 传输版 — Android 共存版 一键构建脚本 (增强诊断版)
#   包名: com.easyshare.easytier (与官方 com.kkrainbow.easytier 共存)
#   基于: EasyTier v2.6.4 + easyshare-lib (剪贴板同步 + 文件传输)
#
#   用法:
#     bash build.sh                 # 完整构建 (默认)
#     bash build.sh vendor          # 仅下载离线依赖（在有网的机器上跑一次）
#     bash build.sh check           # 仅检查环境
#     bash build.sh clean           # 清理构建缓存
#     bash build.sh help            # 查看帮助
#
#   离线构建流程:
#     1. 先在有网络的机器上:  bash build.sh vendor
#        (自动下载 Rust 依赖到 vendor/ + Gradle 到 prebuilt/)
#     2. 把整个目录打包带走:  zip -r easytier-offline.zip .
#     3. 在离线机器上:        bash build.sh
#
#   前置条件:
#     1. Rust >= 1.95        → curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
#     2. Node.js >= 18       → https://nodejs.org/
#     3. pnpm                → npm install -g pnpm
#     4. JDK 17+             → apt install openjdk-17-jdk
#     5. Android SDK + NDK 25+ → 设置 ANDROID_HOME 和 ANDROID_NDK_HOME
# ═════════════════════════════════════════════════════════════════════

set -e

# ─── 颜色输出 ───
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

log()  { echo -e "${BLUE}[INFO]${NC}  $*"; }
ok()   { echo -e "${GREEN}[ OK ]${NC}  $*"; }
warn() { echo -e "${YELLOW}[WARN]${NC}  $*"; }
err()  { echo -e "${RED}[FAIL]${NC}  $*"; }
step() { echo -e "\n${BOLD}${CYAN}━━━ $* ━━━${NC}"; }

# ─── 路径配置 ───
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$SCRIPT_DIR"
ANDROID_DIR="$PROJECT_ROOT/easytier-gui/src-tauri"
WEB_DIR="$PROJECT_ROOT/easytier-web"
OUTPUT_DIR="$SCRIPT_DIR/output"
VENDOR_DIR="$PROJECT_ROOT/vendor"
mkdir -p "$OUTPUT_DIR"

# ─── 打印横幅 ───
print_banner() {
    echo -e "${BOLD}${CYAN}"
    echo "  ╔══════════════════════════════════════════════════╗"
    echo "  ║   EasyTier 传输版 — Android 共存版               ║"
    echo "  ║   Package: com.easyshare.easytier               ║"
    echo "  ║   Based on EasyTier v2.6.4                      ║"
    echo "  ╚══════════════════════════════════════════════════╝"
    echo -e "${NC}"
}

# ═════════════════════════════════════════════════════════════════════
#  环境检查（增强版）
# ═════════════════════════════════════════════════════════════════════
check_env() {
    step "检查构建环境"
    local all_ok=true

    # Rust
    if command -v rustc &>/dev/null; then
        local rust_ver=$(rustc --version | awk '{print $2}')
        ok "Rust: $rust_ver"
        local major=$(echo "$rust_ver" | cut -d. -f1)
        local minor=$(echo "$rust_ver" | cut -d. -f2)
        if [ "$major" -lt 1 ] || ([ "$major" -eq 1 ] && [ "$minor" -lt 95 ]); then
            warn "Rust 版本偏低 (需要 >= 1.95)，某些 crate 可能编译失败"
            echo "  修复: rustup install 1.95 && rustup default 1.95"
            all_ok=false
        fi
    else
        err "Rust 未安装"
        echo "  安装: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        all_ok=false
    fi

    # Node.js
    if command -v node &>/dev/null; then
        ok "Node.js: $(node --version)"
        local node_major=$(node --version | sed 's/v//' | cut -d. -f1)
        if [ "$node_major" -lt 18 ]; then
            warn "Node.js 版本偏低 (推荐 >= 18)"
            all_ok=false
        fi
    else
        err "Node.js 未安装 (需要 >= 18)"
        all_ok=false
    fi

    # pnpm
    if command -v pnpm &>/dev/null; then
        ok "pnpm: $(pnpm --version)"
    else
        warn "pnpm 未安装，尝试安装..."
        npm install -g pnpm 2>/dev/null && ok "pnpm 已安装" || {
            err "pnpm 安装失败，请手动: npm install -g pnpm"
            all_ok=false
        }
    fi

    # JDK
    if command -v java &>/dev/null; then
        local java_ver=$(java -version 2>&1 | head -1 | sed 's/.*"\(.*\)".*/\1/')
        ok "JDK: $java_ver"
    else
        err "JDK 未安装 (Android 构建需要 JDK 17+)"
        echo "  安装: sudo apt install openjdk-17-jdk"
        all_ok=false
    fi

    # Android SDK
    if [ -n "$ANDROID_HOME" ] || [ -n "$ANDROID_SDK_ROOT" ]; then
        ok "Android SDK: ${ANDROID_HOME:-$ANDROID_SDK_ROOT}"
    else
        err "ANDROID_HOME 未设置"
        echo "  示例: export ANDROID_HOME=\$HOME/Android/Sdk"
        echo "        export ANDROID_SDK_ROOT=\$ANDROID_HOME"
        all_ok=false
    fi

    # Android NDK
    if [ -n "$ANDROID_NDK_HOME" ] || [ -n "$ANDROID_NDK_ROOT" ]; then
        ok "Android NDK: ${ANDROID_NDK_HOME:-$ANDROID_NDK_ROOT}"
    else
        warn "ANDROID_NDK_HOME 未设置 (需要 NDK 25+)"
        echo "  示例: export ANDROID_NDK_HOME=\$ANDROID_HOME/ndk/25.2.9519653"
        all_ok=false
    fi

    echo ""
    log "检查项目文件完整性..."

    # 关键文件清单
    local required_files=(
        "Cargo.toml"
        "easytier/Cargo.toml"
        "easyshare-lib/Cargo.toml"
        "easytier-gui/Cargo.toml"
        "easytier-gui/src-tauri/Cargo.toml"
        "easytier-gui/src-tauri/tauri.conf.json"
        "easytier-gui/src-tauri/src/lib.rs"
        "easytier-web/frontend-lib/package.json"
        "easytier-web/frontend-lib/src/components/Config.vue"
        "easytier-web/frontend-lib/src/components/RemoteManagement.vue"
        "easytier-web/frontend-lib/src/types/network.ts"
        "easytier-web/frontend-lib/src/modules/clipboardSync.ts"
        "easytier-web/frontend-lib/src/locales/cn.yaml"
        "easytier-web/frontend-lib/src/locales/en.yaml"
        "easytier-gui/src/composables/backend.ts"
        "easytier-gui/src/composables/mobile_vpn.ts"
        "easytier-gui/src/pages/index.vue"
        "easytier-gui/src/components/ModeSwitcher.vue"
        "tauri-plugin-vpnservice/Cargo.toml"
        "tauri-plugin-vpnservice/guest-js/index.ts"
        "pnpm-lock.yaml"
    )

    local missing=0
    for f in "${required_files[@]}"; do
        if [ -f "$PROJECT_ROOT/$f" ]; then
            ok "  ✓ $f"
        else
            err "  ✗ $f  【缺失！】"
            missing=$((missing+1))
            all_ok=false
        fi
    done

    # 关键目录检查
    echo ""
    log "检查关键目录..."
    local required_dirs=(
        "easytier/src"
        "easyshare-lib/src"
        "easytier-gui/src-tauri/src"
        "easytier-gui/src-tauri/gen/android"
        "easytier-web/frontend-lib/src/components"
        "easytier-web/frontend-lib/src/components/acl"
        "tauri-plugin-vpnservice/src"
        "tauri-plugin-vpnservice/android/src"
    )
    for d in "${required_dirs[@]}"; do
        if [ -d "$PROJECT_ROOT/$d" ]; then
            local count=$(find "$PROJECT_ROOT/$d" -type f 2>/dev/null | wc -l)
            if [ "$count" -gt 0 ]; then
                ok "  ✓ $d/ ($count 个文件)"
            else
                warn "  ⚠ $d/ (空目录)"
            fi
        else
            err "  ✗ $d/ 【缺失！】"
            missing=$((missing+1))
            all_ok=false
        fi
    done

    # 检查离线 vendor 目录
    echo ""
    if [ -d "$VENDOR_DIR" ] && [ -f "$VENDOR_DIR/config.toml" ]; then
        ok "离线依赖 (vendor/) 已就绪"
        local vendor_count=$(find "$VENDOR_DIR" -type f 2>/dev/null | wc -l)
        log "  vendor 目录包含 $vendor_count 个文件"
    else
        warn "离线依赖 (vendor/) 未找到"
        echo "  在有网络的机器上先运行: bash build.sh vendor"
    fi

    # 检查包名
    echo ""
    local ident=$(grep -o '"identifier": "[^"]*"' "$PROJECT_ROOT/easytier-gui/src-tauri/tauri.conf.json" 2>/dev/null | head -1)
    if echo "$ident" | grep -q "easyshare"; then
        ok "包名: $ident (共存版 ✓)"
    else
        warn "包名: ${ident:-未知} (可能不是共存版)"
    fi

    echo ""
    if $all_ok; then
        ok "环境检查全部通过 ✓"
        return 0
    else
        err "发现 $missing 个缺失项，构建很可能失败"
        echo "  请先修复上述问题再构建"
        return 1
    fi
}

# ═════════════════════════════════════════════════════════════════════
#  下载离线依赖 (cargo vendor)
# ═════════════════════════════════════════════════════════════════════
vendor_deps() {
    step "下载离线依赖 (cargo vendor)"
    
    if ! command -v cargo &>/dev/null; then
        err "cargo 未安装，无法下载依赖"
        echo "  请先安装 Rust: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        return 1
    fi

    cd "$PROJECT_ROOT"

    # 检查是否需要安装 Android targets（vendor 时需要）
    if command -v rustup &>/dev/null; then
        log "确保 Android targets 已安装（vendor 时需要）..."
        rustup target add aarch64-linux-android 2>&1 | tail -1 || true
        rustup target add armv7-linux-androideabi 2>&1 | tail -1 || true
        rustup target add x86_64-linux-android 2>&1 | tail -1 || true
        rustup target add i686-linux-android 2>&1 | tail -1 || true
    fi

    log "运行 cargo vendor（这会从 crates.io 下载所有依赖）..."
    log "预计下载 ~200-300MB，耗时 5-15 分钟，请耐心等待..."
    echo "──────────────────────────────────────"
    
    cargo vendor --locked --versioned-dirs vendor 2>&1 | tee "$OUTPUT_DIR/vendor.log" | tail -20
    echo "──────────────────────────────────────"

    if [ -d "$VENDOR_DIR" ] && [ "$(ls -A "$VENDOR_DIR" 2>/dev/null)" ]; then
        local vendor_size=$(du -sh "$VENDOR_DIR" | cut -f1)
        ok "离线依赖下载完成: $VENDOR_DIR ($vendor_size)"
        
        # 创建 .cargo/config.toml 指向 vendor 目录
        mkdir -p "$PROJECT_ROOT/.cargo"
        cat > "$PROJECT_ROOT/.cargo/config.toml" << 'EOF'
# EasyTier Transfer — 离线构建配置 (由 build.sh vendor 自动生成)
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
EOF
        ok "已生成 .cargo/config.toml (离线模式)"
        echo ""
        ok "离线依赖准备完成！现在可以打包整个目录，在离线机器上构建。"
        echo "  打包命令: cd .. && zip -r easytier-offline.zip easytier-transfer-android/"
        echo "  离线机器: 解压后直接 bash build.sh 即可"
    else
        err "vendor 目录为空，下载可能失败"
        echo "  查看日志: $OUTPUT_DIR/vendor.log"
        return 1
    fi
}

# ═════════════════════════════════════════════════════════════════════
#  安装 Rust Android targets
# ═════════════════════════════════════════════════════════════════════
install_rust_targets() {
    step "安装 Rust Android 编译目标"
    if command -v rustup &>/dev/null; then
        log "安装 aarch64-linux-android..."
        rustup target add aarch64-linux-android 2>&1 | tail -1
        log "安装 armv7-linux-androideabi..."
        rustup target add armv7-linux-androideabi 2>&1 | tail -1
        log "安装 x86_64-linux-android..."
        rustup target add x86_64-linux-android 2>&1 | tail -1
        log "安装 i686-linux-android..."
        rustup target add i686-linux-android 2>&1 | tail -1
        ok "所有 Android targets 就绪"
    else
        warn "rustup 不可用，跳过 target 安装"
        warn "如果缺失 target，请安装 rustup 后重试"
    fi
}

# ═════════════════════════════════════════════════════════════════════
#  构建前端（增强诊断版）
# ═════════════════════════════════════════════════════════════════════
build_frontend() {
    step "构建前端 (easytier-web + easytier-gui)"

    cd "$PROJECT_ROOT"

    # 检查 pnpm-lock.yaml
    if [ ! -f "pnpm-lock.yaml" ]; then
        err "pnpm-lock.yaml 不存在！前端依赖版本无法锁定"
        return 1
    fi
    ok "pnpm-lock.yaml 存在"

    # 检查关键前端源文件是否存在
    log "验证前端源文件..."
    local frontend_files=(
        "easytier-web/frontend-lib/src/components/Config.vue"
        "easytier-web/frontend-lib/src/components/RemoteManagement.vue"
        "easytier-web/frontend-lib/src/types/network.ts"
        "easytier-web/frontend-lib/src/modules/clipboardSync.ts"
        "easytier-web/frontend-lib/src/locales/cn.yaml"
        "easytier-web/frontend-lib/src/locales/en.yaml"
        "easytier-gui/src/pages/index.vue"
        "easytier-gui/src/composables/backend.ts"
    )
    for f in "${frontend_files[@]}"; do
        if [ ! -f "$f" ]; then
            err "前端文件缺失: $f"
            return 1
        fi
    done
    ok "所有关键前端源文件存在"

    # 安装 npm 依赖（显示完整输出，不用 tail 隐藏错误）
    log "安装 npm 依赖 (pnpm install)..."
    echo "──────────────────────────────────────"
    if pnpm install --prefer-offline 2>&1 | tee "$OUTPUT_DIR/pnpm_install.log"; then
        ok "pnpm install 成功"
    else
        err "pnpm install 失败！查看日志: $OUTPUT_DIR/pnpm_install.log"
        echo ""
        echo "常见原因:"
        echo "  1. Node.js 版本过低 (需要 >= 18)"
        echo "  2. 网络问题导致部分包下载失败"
        echo "  3. pnpm-lock.yaml 与 package.json 不一致"
        echo ""
        echo "尝试修复:"
        echo "  rm -rf node_modules easytier-*/node_modules"
        echo "  pnpm install --no-frozen-lockfile"
        return 1
    fi
    echo "──────────────────────────────────────"

    # 构建前端库（完整输出）
    log "构建前端库 (pnpm -F easytier-web build)..."
    echo "──────────────────────────────────────"
    if pnpm -F easytier-web build 2>&1 | tee "$OUTPUT_DIR/frontend_build.log"; then
        ok "前端库构建成功"
    else
        err "前端库构建失败！查看日志: $OUTPUT_DIR/frontend_build.log"
        echo ""
        echo "常见原因:"
        echo "  1. TypeScript 类型错误"
        echo "  2. Vue 组件语法错误"
        echo "  3. 依赖版本不兼容"
        return 1
    fi
    echo "──────────────────────────────────────"

    # 验证前端构建产物
    echo ""
    log "验证前端构建产物..."
    local dist_dirs=(
        "easytier-web/frontend-lib/dist"
        "easytier-web/frontend/dist"
    )
    for d in "${dist_dirs[@]}"; do
        if [ -d "$d" ]; then
            local size=$(du -sh "$d" | cut -f1)
            local file_count=$(find "$d" -type f | wc -l)
            ok "  ✓ $d/ ($file_count 个文件, $size)"
        else
            warn "  ⚠ $d/ 不存在（可能不影响构建）"
        fi
    done

    # 关键检查：确认 Config.vue 的内容被编译进去了
    # 搜索构建产物里是否包含高级设置等关键词
    local main_js=$(find easytier-web/frontend-lib/dist -name "*.js" -type f 2>/dev/null | head -3)
    if [ -n "$main_js" ]; then
        for js in $main_js; do
            if grep -q "advanced_settings\|port_forwards\|clipboard_sync\|acl" "$js" 2>/dev/null; then
                ok "  ✓ 前端产物包含高级设置/端口转发/剪贴板同步代码"
                break
            fi
        done
    fi

    ok "前端构建完成 ✓"
}

# ═════════════════════════════════════════════════════════════════════
#  构建 Android APK
# ═════════════════════════════════════════════════════════════════════
build_android() {
    step "构建 Android APK (共存版 com.easyshare.easytier)"
    cd "$ANDROID_DIR"

    # 确保 gradle wrapper 可执行
    chmod +x gen/android/gradlew 2>/dev/null || true

    # 检查是否使用离线 vendor 模式
    if [ -d "$VENDOR_DIR" ] && [ -f "$VENDOR_DIR/config.toml" ]; then
        ok "使用离线依赖 (vendor/ 模式)"
        export CARGO_NET_OFFLINE=true
    fi

    log "运行: pnpm tauri android build --apk"
    if [ -d "$VENDOR_DIR" ]; then
        log "(离线模式，不访问网络)"
    else
        log "(首次构建需要下载依赖，可能需要 10-30 分钟)"
    fi
    echo "──────────────────────────────────────"
    pnpm tauri android build --apk 2>&1 | tee "$OUTPUT_DIR/android_build.log"
    local build_exit_code=${PIPESTATUS[0]}
    echo "──────────────────────────────────────"

    if [ $build_exit_code -ne 0 ]; then
        err "Android 构建失败 (exit code: $build_exit_code)"
        echo "  查看完整日志: $OUTPUT_DIR/android_build.log"
        echo ""
        echo "常见原因:"
        echo "  1. ANDROID_HOME / ANDROID_NDK_HOME 未设置或路径错误"
        echo "  2. Rust target 未安装 (运行: bash build.sh check)"
        echo "  3. 前端构建失败导致 dist 目录缺失"
        echo "  4. 离线模式下 vendor/ 不完整"
        return 1
    fi

    # 查找产物
    local apk_dir="$ANDROID_DIR/gen/android/app/build/outputs/apk"
    if [ -d "$apk_dir" ]; then
        local apks=$(find "$apk_dir" -name "*.apk" 2>/dev/null)
        if [ -n "$apks" ]; then
            echo ""
            echo -e "${GREEN}━━━ APK 构建成功 ━━━${NC}"
            for apk in $apks; do
                local size=$(du -h "$apk" | cut -f1)
                echo -e "  ${GREEN}→${NC} $apk ${YELLOW}($size)${NC}"
                cp "$apk" "$OUTPUT_DIR/" 2>/dev/null || true
            done
            echo ""
            ok "产物已复制到: $OUTPUT_DIR/"
        else
            err "未找到 APK 产物"
            echo "  查看日志: $OUTPUT_DIR/android_build.log"
            return 1
        fi
    else
        err "构建输出目录不存在: $apk_dir"
        echo "  查看日志: $OUTPUT_DIR/android_build.log"
        return 1
    fi
}

# ═════════════════════════════════════════════════════════════════════
#  清理
# ═════════════════════════════════════════════════════════════════════
clean_all() {
    step "清理构建缓存"
    rm -rf "$OUTPUT_DIR"
    mkdir -p "$OUTPUT_DIR"
    cd "$PROJECT_ROOT"
    cargo clean 2>/dev/null || true
    find . -name "node_modules" -type d -exec rm -rf {} + 2>/dev/null || true
    find . -name "target" -type d -exec rm -rf {} + 2>/dev/null || true
    find . -name "dist" -type d -exec rm -rf {} + 2>/dev/null || true
    ok "清理完成"
}

# ═════════════════════════════════════════════════════════════════════
#  诊断工具：检查已构建 APK 的前端内容
# ═════════════════════════════════════════════════════════════════════
diagnose_apk() {
    step "诊断已安装的 App"
    echo "请在手机上运行 App，然后观察以下行为："
    echo ""
    echo "━━━ UI 结构说明 ━━━"
    echo "App 启动后，界面应有以下元素："
    echo ""
    echo "  [下拉框] 选择网络 (easytier 等)"
    echo "  [+]    创建新网络"
    echo "  [...]  更多操作菜单"
    echo ""
    echo "点击 [+] 后，应展开完整编辑表单："
    echo ""
    echo "  ┌─ 基础设置 ─────────────┐"
    echo "  │  虚拟 IPv4  [DHCP ✓]   │"
    echo "  │  网络名称  [________]   │"
    echo "  │  网络密码  [________]   │"
    echo "  │  初始节点  [+ 添加]     │"
    echo "  └─────────────────────────┘"
    echo "  ┌─ 高级设置 ▼ ───────────┐  ← 点击展开"
    echo "  │  标志开关  [20+ 个选项] │"
    echo "  │  主机名    [________]   │"
    echo "  │  MTU       [1100   ]   │"
    echo "  │  ...                    │"
    echo "  └─────────────────────────┘"
    echo "  ┌─ 端口转发 ▼ ───────────┐  ← 点击展开"
    echo "  │  [+ 添加端口转发]      │"
    echo "  └─────────────────────────┘"
    echo "  ┌─ 访问控制 (ACL) ▼ ─────┐  ← 点击展开"
    echo "  └─────────────────────────┘"
    echo "  ┌─ 剪贴板同步 ▼ ─────────┐  ← 点击展开"
    echo "  │  [启用剪贴板同步 ✓]    │"
    echo "  │  [启用文件传输   ✓]    │"
    echo "  │  [同步图片       ✓]    │"
    echo "  └─────────────────────────┘"
    echo "  [运行网络]  ← 绿色大按钮"
    echo ""
    echo "━━━ 如果看不到折叠面板标题 ━━━"
    echo "这说明 PrimeVue CSS 没加载。检查："
    echo "  1. 前端构建是否报错 (查看 $OUTPUT_DIR/frontend_build.log)"
    echo "  2. node_modules 是否完整 (尝试 rm -rf node_modules && pnpm install)"
    echo "  3. 手机 WebView 版本是否过低 (需要 Chrome 90+)"
    echo ""
    echo "━━━ 如果面板标题可见但展开为空 ━━━"
    echo "这说明 Vue 组件渲染失败。检查："
    echo "  1. JS 控制台错误 (Chrome 远程调试)"
    echo "  2. Config.vue 是否被正确编译进 bundle"
    echo "  3. i18n 语言文件是否加载 (cn.yaml/en.yaml)"
}

# ═════════════════════════════════════════════════════════════════════
#  主入口
# ═════════════════════════════════════════════════════════════════════
main() {
    local cmd="${1:-build}"
    print_banner

    case "$cmd" in
        vendor)
            check_env || true
            vendor_deps
            ;;
        check)
            check_env
            ;;
        clean)
            clean_all
            ;;
        diagnose)
            diagnose_apk
            ;;
        build|"")
            check_env || true
            install_rust_targets
            build_frontend || exit 1
            build_android || exit 1
            echo ""
            ok "═════════════════════════════════════════"
            ok "  🎉 构建全部完成！"
            ok "  产物位置: $OUTPUT_DIR/"
            ok "  安装命令: adb install -r $OUTPUT_DIR/*.apk"
            ok "═════════════════════════════════════════"
            ;;
        help|--help|-h)
            echo "用法: bash build.sh [命令]"
            echo ""
            echo "命令:"
            echo "  build      完整构建 Android APK (默认)"
            echo "  vendor     下载离线依赖（在有网的机器上跑一次）"
            echo "  check      仅检查构建环境"
            echo "  clean      清理构建缓存"
            echo "  diagnose   诊断已安装 App 的 UI 问题"
            echo "  help       显示帮助"
            echo ""
            echo "离线构建流程:"
            echo "  1) 有网机器: bash build.sh vendor"
            echo "  2) 打包:    zip -r easytier-offline.zip ."
            echo "  3) 离线机:  解压后 bash build.sh"
            echo ""
            echo "新增: diagnose 命令可诊断'选项全没了'等 UI 问题"
            echo ""
            echo "环境变量:"
            echo "  ANDROID_HOME      Android SDK 路径"
            echo "  ANDROID_NDK_HOME  Android NDK 路径"
            ;;
        *)
            err "未知命令: $cmd"
            echo "运行 'bash build.sh help' 查看帮助"
            exit 1
            ;;
    esac
}

main "$@"
