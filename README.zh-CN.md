[English](README.md) | 简体中文

# TunePeerly（声渡）

TunePeerly（声渡）播放这台电脑上已有的音乐，并分享给已配对的设备。没有账号。歌曲从原始文件播放。同一局域网里的已配对设备直接互相发现。你自己运行的中继可以用于局域网之外的连接。中继这个标题标着待完善且未测试，以及不要使用。

## 功能

- 播放本机文件，从已配对设备串流一首已公开的歌，或把原始文件下载到本机。
- 在局域网发现设备，或按 IPv4 地址和端口连接。双方都要确认。同步数据使用 libsignal 端到端加密。
- 喜欢的歌曲和外观按整份副本同步。自动发送把你的修改推出去。跟随从一台设备拉取之后的修改。发送和覆盖是手动操作。
- 最近播放按最新的 UTC 时间合并。一首歌大约播放 10 秒，或播放到一半，就会记下来。可以在这台电脑上线时再合并一次。
- 跟随一台在线设备的播放。底部播放条显示那台设备正在听的歌，进度跟着走。这台电脑不发声。
- 无痕模式会暂停发现、同步和播放记录。也可以拉黑一台设备，或按项目停止手动同步和跟随。
- 浅色和深色主题、中英文、滚动歌词、托盘图标、迷你播放器，以及 Windows 任务栏缩略图上的播放按钮。
- 有些设置标着**待修改**、**不要使用**或**待完善且未测试**。标签不说明具体问题。[使用手册](docs/guide.zh-CN.md)列出了每个标签对应哪些设置。跟随播放和中继标着不要使用。中继还标着待完善且未测试。

## 手册

[使用手册](docs/guide.zh-CN.md)

## 数据

数据库在 `%APPDATA%\TunePeerly\library.db`。封面在 `%LOCALAPPDATA%\TunePeerly\covers`。程序名是 `TunePeerly.exe`。

## 开发

需要 Node.js、npm、Rust 工具链，以及 Windows 上的 Microsoft Edge WebView2 运行库。

```powershell
npm install
npm run tauri dev
npm test
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run tauri build
```

`npm run tauri build` 会生成 Windows 安装包。默认是 NSIS 安装程序。装了 WiX Toolset 时还会生成 MSI。安装程序在缺少 WebView2 时可以下载它。安装程序不会登记防火墙规则。

## 项目

- [更新日志](CHANGELOG.md)
- [安全策略](SECURITY.md)

## 许可证

[GNU Affero General Public License v3.0](https://www.gnu.org/licenses/agpl-3.0.html)。关于署名和修改版的附加条款在 [ADDITIONAL_TERMS.md](ADDITIONAL_TERMS.md)。名称和图标见 [TRADEMARKS.md](TRADEMARKS.md)。
