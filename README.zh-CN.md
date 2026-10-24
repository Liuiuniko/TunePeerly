<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="192" height="192" alt="TunePeerly">
</p>

<h1 align="center">TunePeerly（声渡）</h1>

<p align="center">
  <a href="README.md">English</a> |
  简体中文
</p>

声渡播放这台电脑上已有的音乐，并分享给已配对的设备。没有账号。同一局域网里的已配对设备直接互相发现。你自己运行的中继可以用于局域网之外的连接。

> **注意：** 当前版本是 alpha 测试版，程序的实际效果不保证和[使用手册](docs/guide.zh-CN.md)的说明一致。

## 功能

- 播放本机文件，从已配对设备串流一首已公开的歌，或把原始文件下载到本机。
- 在局域网发现设备，或按 IPv4 地址和端口连接。双方都要确认。同步数据使用 libsignal 端到端加密。配对前必须打开设置 → 网络里的**发现附近设备**，否则无法发现设备。
- 最近播放按最新的 UTC 时间合并。一首歌大约播放 10 秒，或播放到一半，就会记下来。可以在这台电脑上线时再合并一次。
- 喜欢的歌曲和外观的同步和跟随一台在线设备的播放。
- 无痕模式会暂停发现、同步和播放记录。也可以拉黑一台设备，或按项目停止手动同步和跟随。
- 浅色和深色主题、中英文、滚动歌词、托盘图标、迷你播放器，以及 Windows 任务栏缩略图上的播放按钮。
- 有些设置标着**待修改**、**不要使用**或**待完善且未测试**。标签不说明具体问题。[使用手册](docs/guide.zh-CN.md)列出了每个标签对应哪些设置。跟随播放和中继标着不要使用。中继还标着待完善且未测试。

## 手册

[使用手册](docs/guide.zh-CN.md)

## 安装

GitHub 上发布的版本用 [Inno Setup](installer/TunePeerly.iss) 打包。安装程序会询问为所有用户安装，还是只为当前用户安装。默认是当前用户，目录为 `C:\Users\<用户名>\AppData\Local\Programs\TunePeerly`，不需要管理员权限。为所有用户安装时，目录为 `C:\Program Files\TunePeerly`，需要管理员权限。

数据库和缓存不跟着这个选择走，也不能单独修改。数据库始终在 `%APPDATA%\TunePeerly`。封面和界面缓存始终在 `%LOCALAPPDATA%\TunePeerly`。

下载的安装包是 `TunePeerly-0.6.0-alpha.1-x64-Setup.exe`。在这个文件所在的文件夹打开 PowerShell，算出它的 SHA256，再和发布页上这个文件的 SHA256 对照。两串一致再安装。

```powershell
Get-FileHash -Algorithm SHA256 .\TunePeerly-0.6.0-alpha.1-x64-Setup.exe
```

## 开发

需要 Node.js、npm、Rust 工具链，以及 Windows 上的 Microsoft Edge WebView2 运行库。

```powershell
npm install
npm run tauri dev
npm test
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run tauri build
```

## 安装包

在仓库根目录执行。先生成发布版程序，再编译 [installer/TunePeerly.iss](installer/TunePeerly.iss)。脚本从 `src-tauri\target\release\TunePeerly.exe` 取文件，需要已安装的 [Inno Setup 7](https://jrsoftware.org/isinfo.php)。

```powershell
npm run tauri build -- --no-bundle
& "${env:ProgramFiles}\Inno Setup 7\ISCC.exe" installer\TunePeerly.iss
```

`npm run tauri build -- --no-bundle` 只生成可执行文件，不生成安装包。安装包写到 `installer\Output\TunePeerly-0.6.0-alpha.1-x64-Setup.exe`。`installer\Output` 被 git 忽略。也可以用 Inno Setup 打开 `installer\TunePeerly.iss` 后编译。

编译停在 `EndUpdateResource failed (110)` 时，是往新安装包写入图标的时候文件被占用了。Windows Defender 实时扫描会锁住这个未签名的 exe，安全中心不会弹出提示。等几秒后重新执行上面的 `ISCC.exe` 命令，不要紧接着失败立刻再编译。如果还是 这样，把仓库里的 `installer\Output` 加进 Windows 安全中心的排除项后再编译。路径是：病毒和威胁防护 → 管理设置 → 排除项 → 添加排除项 → 文件夹。

## 贡献

贡献流程在 [CONTRIBUTING.md](CONTRIBUTING.md)。先开 issue，讨论确定后再开发。

## 项目

- [更新日志](CHANGELOG.md)
- [安全策略](SECURITY.md)

## 许可证

[GNU Affero General Public License v3.0](https://www.gnu.org/licenses/agpl-3.0.html)。关于署名和修改版的附加条款在 [ADDITIONAL_TERMS.md](ADDITIONAL_TERMS.md)。
