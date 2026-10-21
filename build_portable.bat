@echo off
setlocal
title SHU AutoNet Pro - Portable Builder
cd /d "%~dp0"
for /f "tokens=2*" %%a in ('reg query "HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment" /v Path 2^>nul') do set "SYS_PATH=%%b"
for /f "tokens=2*" %%a in ('reg query "HKCU\Environment" /v Path 2^>nul') do set "USER_PATH=%%b"
set "PATH=%USER_PATH%;%SYS_PATH%;%PATH%"
if exist "%USERPROFILE%\.cargo\bin" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
where node >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Node.js not detected. Please install Node.js: https://nodejs.org/
    pause
    exit /b 1
)
where cargo >nul 2>nul
if %errorlevel% neq 0 (
    echo [ERROR] Cargo not detected. Please install Rustup: https://rustup.rs/
    pause
    exit /b 1
)
echo [1/3] Checking dependencies...
if not exist "node_modules" (
    call npm install
    if %errorlevel% neq 0 (
        echo [ERROR] npm install failed.
        pause
        exit /b 1
    )
)
echo [2/3] Building release binary with Tauri...
rem Use MSVC so WebView2Loader and the C runtime are linked into the portable EXE.
set "RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc"
set "STATIC_VCRUNTIME=true"
call npx tauri build --no-bundle
if %errorlevel% neq 0 (
    echo [ERROR] Tauri build failed.
    pause
    exit /b 1
)
echo [3/3] Packaging portable release...
if not exist "dist_portable" mkdir "dist_portable"
copy /y "src-tauri\target\release\shu-autonet.exe" "dist_portable\SHU_AutoNet.exe" >nul
if not exist "dist_portable\config.json" copy /y "config.json" "dist_portable\config.json" >nul
echo ========================================================
echo [SUCCESS] Portable release created in dist_portable directory!
echo ========================================================
pause
