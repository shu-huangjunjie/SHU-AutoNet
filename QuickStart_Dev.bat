@echo off
title SHU AutoNet - Development Mode
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
if not exist "node_modules" (
    echo [INFO] Installing dependencies...
    call npm install
)
echo [INFO] Starting Tauri development server with hot-reload...
call npm run tauri dev
pause