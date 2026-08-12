@echo off
REM ═════════════════════════════════════════════════════════════
REM  推送源码到 GitHub + 触发云端构建 (Windows)
REM  前置条件：在 GitHub → Settings → SSH keys 添加 DEPLOY_KEY.pub
REM ═════════════════════════════════════════════════════════════

setlocal enabledelayedexpansion

echo.
echo ╔══════════════════════════════════════════════════╗
echo ║   EasyTier Transfer — Push to GitHub              ║
echo ╚══════════════════════════════════════════════════╝
echo.

set SCRIPT_DIR=%~dp0
set SRC_DIR=%SCRIPT_DIR%easytier-transfer-android
set SSH_KEY=%SCRIPT_DIR%DEPLOY_KEY

REM ─── 检查 SSH key ───
echo [INFO]  检查 SSH 密钥...
if not exist "%SSH_KEY%" (
    echo [FAIL]  SSH 私钥不存在: %SSH_KEY%
    echo         请确认 DEPLOY_KEY 文件存在
    pause
    exit /b 1
)
echo [ OK ]  SSH 私钥存在

REM ─── 设置 GIT_SSH_COMMAND ───
echo [INFO]  配置 SSH 命令...
where ssh >nul 2>&1
if %errorlevel% neq 0 (
    echo [WARN]  ssh 命令不可用，请安装 Git for Windows
    echo        下载: https://git-scm.com/download/win
    pause
    exit /b 1
)

set GIT_SSH_COMMAND=ssh -i %SSH_KEY% -o StrictHostKeyChecking=no -o UserKnownHostsFile=NUL
echo [ OK ]  GIT_SSH_COMMAND 已设置

REM ─── 进入源码目录 ───
cd /d "%SRC_DIR%"
if errorlevel 1 (
    echo [FAIL]  无法进入目录: %SRC_DIR%
    pause
    exit /b 1
)

REM ─── 初始化 git ───
echo.
echo [INFO]  初始化 Git 仓库...
if not exist ".git" (
    git init -b main
    echo [ OK ]  Git 仓库已初始化
) else (
    echo [ OK ]  Git 仓库已存在
)

git config user.email "hulu976@users.noreply.github.com"
git config user.name "hulu976"
echo [ OK ]  Git 用户配置完成

REM ─── 添加远程 ───
echo.
echo [INFO]  配置远程仓库...
git remote remove origin 2>nul
git remote add origin "git@github.com:hulu976/easytier-transfer-.git"
echo [ OK ]  远程仓库已设置

REM ─── 添加文件 ───
echo.
echo [INFO]  添加文件到 Git...
git add .
echo [ OK ]  文件已暂存

REM ─── 提交 ───
echo.
echo [INFO]  提交代码...
git commit -m "feat: EasyTier Transfer Android 完整源码

- EasyTier v2.6.4 官方源码完整内嵌
- easyshare-lib: 剪贴板同步 + 文件传输
- Android 共存版: com.easyshare.easytier
- GitHub Actions 自动构建 workflow
- 包名与官方 com.kkrainbow.easytier 共存
- 剪贴板同步通过 AccessibilityService 实现" 2>&1 | tail -5
echo [ OK ]  代码已提交

REM ─── 推送 ───
echo.
echo ╔══════════════════════════════════════════════════╗
echo ║  正在推送到 GitHub...                              ║
echo ╚══════════════════════════════════════════════════╝
echo.
git push -u origin main --force 2>&1
if errorlevel 1 (
    echo.
    echo [FAIL]  推送失败！
    echo.
    echo 常见原因：
    echo   1. DEPLOY_KEY.pub 没加到 GitHub → Settings → SSH keys
    echo   2. 网络不通 GitHub → 检查代理/防火墙
    echo   3. 仓库地址错误 → 检查仓库是否存在
    echo.
    echo 请打开 DEPLOY.md 查看详细排查步骤
    pause
    exit /b 1
)

echo.
echo ╔══════════════════════════════════════════════════╗
echo ║  ✅ 推送成功！                                     ║
echo ╚══════════════════════════════════════════════════╝
echo.
echo 下一步：触发云端构建
echo   1. 打开: https://github.com/hulu976/easytier-transfer-/actions
echo   2. 点 'Build Android APK'
echo   3. 点 'Run workflow' 按钮
echo   4. 选择 build_type: debug
echo   5. 点绿色 'Run workflow'
echo   6. 等待 25-40 分钟
echo   7. 下载 APK 产物
echo.
pause
