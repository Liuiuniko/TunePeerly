English | [简体中文](README.zh-CN.md)

# TunePeerly

TunePeerly (声渡) plays the music already on your computer and shares it with the devices you pair. There is no account. Songs play from the original files. Paired devices on the same local network find each other directly. A relay you run can be configured for connections outside that network. The Relay heading is marked not completed yet, not tested, and do not use.

## Features

- Play local files, stream an exposed song from a paired device, or download the original file.
- Find devices on the LAN or connect by IPv4 address and port. Both sides confirm. Sync traffic is end-to-end encrypted with libsignal.
- Sync liked songs and appearance as a full copy. Auto send pushes your edits out. Follow pulls later edits from one device. Send and Override are manual.
- Merge recently played by the latest UTC time. A song is recorded after about 10 seconds, or half its length. Optionally merge again when this computer comes online.
- Follow playback from one online device. The bar at the bottom shows that device's song and moves with it. This computer stays silent.
- Incognito pauses discovery, sync, and history recording. You can also block a device, or stop manual sync and following per item.
- Light and dark themes, English and Chinese, synced lyrics, a tray icon, a mini player, and playback buttons on the Windows taskbar thumbnail.
- Some settings are marked **To be revised**, **Do not use**, or **Not completed yet, not tested**. The mark does not describe the problem. The [user guide](docs/guide.md) lists which setting has which mark. Follow playback and the relay are marked do not use. The relay is also marked not completed yet and not tested.

## Guide

[User guide](docs/guide.md)

## Data

The app keeps its database at `%APPDATA%\TunePeerly\library.db`. Cover images are under `%LOCALAPPDATA%\TunePeerly\covers`. The program name is `TunePeerly.exe`.

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
