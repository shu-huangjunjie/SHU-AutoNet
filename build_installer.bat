@echo off
setlocal
cd /d "%~dp0"
if exist "%USERPROFILE%\.cargo\bin" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
if not exist "node_modules" (
    call npm ci
    if errorlevel 1 exit /b 1
)
set "RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc"
set "STATIC_VCRUNTIME=true"
call npx tauri build --bundles nsis
if errorlevel 1 exit /b 1
echo Windows installer: src-tauri\target\release\bundle\nsis\
