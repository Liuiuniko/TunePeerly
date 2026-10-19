English | [简体中文](README.zh-CN.md)

# TunePeerly

TunePeerly (声渡) plays the music already on your computer and shares it with the devices you pair. There is no account. Songs play from the original files. Away from your local network, paired devices reconnect through a relay you run yourself.

## Features

- Play local files, stream an exposed song from a paired device, or download the original file.
- Find devices on the LAN, connect by address, or pair by pasting a code. Both sides confirm. Sync traffic is end-to-end encrypted with libsignal.
- Sync liked songs and appearance as a full copy. Auto send pushes your edits out. Follow pulls later edits from one device. Send and Override are manual.
- Merge recently played by the latest UTC time. Optionally merge again when this computer comes online.
- Follow playback from one online device. The bar at the bottom shows that device's song and moves with it. This computer stays silent.
- Incognito pauses discovery, sync, and history recording. You can also block a device, or stop manual sync and following per item.
- Light and dark themes, English and Chinese, synced lyrics, a tray icon, a mini player, and playback buttons on the Windows taskbar thumbnail.

## Guide

[User guide](docs/guide.md)

## Data

The app keeps its database at `%AppData%\Roaming\TunePeerly\library.db`. The program name is `TunePeerly.exe`.

## Develop

You need Node.js, npm, a Rust toolchain, and the Microsoft Edge WebView2 Runtime on Windows.

```powershell
npm install
npm run tauri dev
npm test
cargo test --manifest-path src-tauri/Cargo.toml --lib
npm run tauri build
```

`npm run tauri build` produces the Windows installers. The NSIS installer is the default. An MSI is also produced when the WiX Toolset is installed. The installer can download WebView2 if it is missing. It does not register a firewall rule.

## Project

- [Changelog](CHANGELOG.md)
- [Security policy](SECURITY.md)

## License

[GNU Affero General Public License v3.0](https://www.gnu.org/licenses/agpl-3.0.html). Additional terms for attribution and modified versions are in [ADDITIONAL_TERMS.md](ADDITIONAL_TERMS.md). Names and the logo are covered by [TRADEMARKS.md](TRADEMARKS.md).
