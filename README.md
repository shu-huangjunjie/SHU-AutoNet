# SHU AutoNet · 上大校园网助手

上海大学校园网自动认证与断线重连工具，支持后台监测、系统托盘和开机启动。

使用 TypeScript + Rust（Tauri v2）构建，提供 Windows x64 便携版。使用者无需安装 Python、Node.js 或 Rust。

[下载便携版](https://github.com/shu-huangjunjie/SHU-AutoNet/releases/latest) · [反馈问题](https://github.com/shu-huangjunjie/SHU-AutoNet/issues)

## 功能

- 监测网络连通状态，在检测到断网后尝试校园网认证。
- 支持手动认证测试，以及启动、停止后台监测。
- 支持关闭或最小化到系统托盘。
- 支持 Windows 开机启动和最小化启动。
- 提供深色、浅色主题。
- 配置保存在 EXE 同目录下的 `config.json`，方便携带和迁移。

## 下载与使用

1. 在 [Releases](https://github.com/shu-huangjunjie/SHU-AutoNet/releases) 下载 `SHU_AutoNet_Portable_v1.0.0.zip`。
2. 解压到桌面、文档或其他可写目录，再运行 `SHU_AutoNet.exe` 或 `QuickStart.bat`。
3. 填写自己的校园网账号和密码，点击 **SAVE CONFIG** 保存。
4. 点击 **START MONITORING** 开始监测；可使用 **TEST AUTH** 进行手动认证测试。

已保存账号密码时，程序会在下次启动后自动开启监测。托盘菜单中的 **Exit** 可完整退出程序。

### 使用条件

- Windows 10/11，x64。
- 已安装 Microsoft Edge WebView2 Runtime。缺少时请前往 [微软 WebView2 页面](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) 安装 Evergreen Runtime。
- 连接适用的上海大学校园网，并使用自己的有效账号。
- 程序所在目录可写；请先解压，不要直接在压缩包内运行。

当前发布版已完成本机启动运行检查与监测启停回归测试，尚未完成不同电脑及所有校园网接入场景的验证。如遇问题，请通过 Issues 描述系统版本、网络场景和现象，并隐去账号密码。

## 配置与隐私

- 默认认证服务器为 `10.10.9.9:8080`；可在界面中调整认证服务器和高级解析参数。
- 账号密码以明文形式保存在 EXE 同目录的 `config.json`。请妥善保管，不要上传自己的配置或含凭据的截图。
- 仓库中的 `config.json` 是账号密码留空的模板，发布包也只包含空白模板。
- 移动程序位置后，如需继续开机启动，请重新保存开机启动设置。

本项目为非官方工具，与上海大学官方无隶属关系。

## 从源码构建

开发环境需要：

- Node.js 20.19+ 或 22.12+。
- Rust stable MSVC 工具链。
- Visual Studio C++ Build Tools 与 Windows SDK。

首次安装 MSVC Rust 工具链：

```powershell
rustup toolchain install stable-x86_64-pc-windows-msvc
```

在项目根目录运行：

```powershell
.\build_portable.bat
```

脚本会安装前端依赖、构建前后端，并将 Windows 便携程序输出到 `dist_portable/`。使用 MSVC 构建，并静态链接 C 运行库。

开发调试可运行 `QuickStart_Dev.bat`。

### 回归测试

```powershell
cargo +stable-x86_64-pc-windows-msvc test --locked --manifest-path src-tauri/Cargo.toml --test monitor_runtime
```

测试覆盖没有 Tokio 上下文时启动监测、重复启动、停止和重新启动，防止监测启动时闪退的问题再次出现。

## 项目结构

```text
src/                  TypeScript 界面逻辑与样式
src-tauri/src/        Rust 认证、网络检查、监测、配置与托盘逻辑
src-tauri/tests/      监测运行环境回归测试
src-tauri/icons/      应用图标
config.json           空白配置模板
build_portable.bat    Windows 便携版构建脚本
```

## 许可证

本项目采用 [MIT License](LICENSE)。
