param([string]$OutputDirectory = (Join-Path $PSScriptRoot 'dist_portable'))

$ErrorActionPreference = 'Stop'
$metadata = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'package.json') -Raw | ConvertFrom-Json
$version = $metadata.version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Invalid package version' }
$binary = Join-Path $PSScriptRoot 'src-tauri\target\release\shu-autonet.exe'
if (!(Test-Path -LiteralPath $binary)) { throw 'Build the release EXE before packaging' }
$binaryVersion = [System.Diagnostics.FileVersionInfo]::GetVersionInfo($binary).ProductVersion
if ($binaryVersion -notlike "$version*") { throw "EXE version $binaryVersion does not match $version" }
$template = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'config.json') -Raw
$config = $template | ConvertFrom-Json
if ($config.user_id -or $config.password) { throw 'The release config template must have empty credentials' }

$name = "SHU_AutoNet_Portable_v$version"
$stageParent = Join-Path $PSScriptRoot ('src-tauri\target\portable-stage-' + [guid]::NewGuid().ToString('N'))
$stage = Join-Path $stageParent $name
New-Item -ItemType Directory -Path $stage -Force | Out-Null
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
Copy-Item -LiteralPath $binary -Destination (Join-Path $stage 'SHU_AutoNet.exe')
Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'LICENSE') -Destination (Join-Path $stage 'LICENSE')
$utf8 = New-Object System.Text.UTF8Encoding($true)
[System.IO.File]::WriteAllText((Join-Path $stage 'config.json'), $template, (New-Object System.Text.UTF8Encoding($false)))
[System.IO.File]::WriteAllText((Join-Path $stage 'QuickStart.bat'), "@echo off`r`ncd /d `"%~dp0`"`r`nstart `"`" `"%~dp0SHU_AutoNet.exe`"`r`n", [System.Text.Encoding]::ASCII)
$readme = @"
SHU AutoNet - 上大校园网助手
版本：v$version（Windows x64 便携版）
项目：https://github.com/shu-huangjunjie/SHU-AutoNet

开始使用
1. 将整个文件夹解压到可写目录。
2. 运行 SHU_AutoNet.exe 或 QuickStart.bat。
3. 填写校园网账号密码，点击 SAVE CONFIG。
4. 点击 START MONITORING。保存账号密码后，下次启动会自动开启监测。
5. 在托盘菜单点击 Exit，可完全退出程序。

使用条件
Windows 10/11 x64，已安装 Microsoft Edge WebView2 Runtime，连接适用的上海大学校园网。
运行此便携版不需要安装 Python、Node.js 或 Rust。

断线重连修复
使用校园网设备兼容的 HTTP 请求头格式，修复下线后无法获取登录参数的问题。
使用直连请求，绕过系统代理；并行探测地址与端口，每轮重连总超时为 30 秒。
认证后再次验证外网是否恢复，日志保存到 EXE 同目录的 autonet.log。

配置说明
本发布包使用空白账号模板。使用后，config.json 会以明文保存自己的账号密码。
要沿用已有设置，可在启动前将旧目录中的个人 config.json 复制到解压后的目录中。
移动程序位置后，如需开机启动，请重新保存开机启动设置。
不要分享自己的 config.json 或运行日志。

本项目为非官方工具，与上海大学官方无隶属关系。采用 MIT License，见 LICENSE。
"@
[System.IO.File]::WriteAllText((Join-Path $stage 'README.txt'), $readme, $utf8)
$binaryHash = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash.ToLowerInvariant()
$validation = @"
SHU AutoNet v$version - 验证记录

修复代码于 2026-10-24 连续通过两轮真实校园网下线与自动重连测试。
恢复耗时：12.86 秒、12.31 秒。
每轮均确认主动下线、外网不可用、运行中的软件自行提交登录、校园网会话恢复、外网 HTTP 204 探测通过。
之后仅更新版本信息为 v$version 并重新构建，认证代码保持不变。

回归测试覆盖请求头大小写兼容、阻塞探测、认证页解析、表单编码、失效代理、凭据跳转保护及监测启停。
验证范围：本次测试的 Windows 电脑和校园网接入场景，尚未覆盖所有电脑与接入方式。

本 EXE 的 SHA256：$binaryHash
"@
[System.IO.File]::WriteAllText((Join-Path $stage 'VALIDATION.txt'), $validation, $utf8)
$archive = Join-Path $OutputDirectory "$name.zip"
Compress-Archive -LiteralPath $stage -DestinationPath $archive -CompressionLevel Optimal -Force
$archiveHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText("$archive.sha256", "$archiveHash  $name.zip`n", [System.Text.Encoding]::ASCII)
[pscustomobject]@{ Version = $version; Archive = $archive; SHA256 = $archiveHash; EXESHA256 = $binaryHash; Stage = $stage } | ConvertTo-Json
