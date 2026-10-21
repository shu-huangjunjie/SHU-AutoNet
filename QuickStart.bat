@echo off
cd /d "%~dp0"
if exist "%~dp0dist_portable\SHU_AutoNet.exe" (
    start /d "%~dp0dist_portable" "" "SHU_AutoNet.exe"
    exit /b 0
)
if exist "%~dp0src-tauri\target\release\shu-autonet.exe" (
    if not exist "%~dp0dist_portable" mkdir "%~dp0dist_portable"
    copy /y "%~dp0src-tauri\target\release\shu-autonet.exe" "%~dp0dist_portable\SHU_AutoNet.exe" >nul
    if not exist "%~dp0dist_portable\config.json" copy /y "%~dp0config.json" "%~dp0dist_portable\config.json" >nul
    start /d "%~dp0dist_portable" "" "SHU_AutoNet.exe"
    exit /b 0
)
call "%~dp0build_portable.bat"
if exist "%~dp0dist_portable\SHU_AutoNet.exe" (
    start /d "%~dp0dist_portable" "" "SHU_AutoNet.exe"
)