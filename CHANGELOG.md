# Changelog

This file describes each released version. 0.6.0 is the first release under the name TunePeerly (声渡).

## [0.6.0] - 2026-10-19

TunePeerly plays local audio from the original file, streams an exposed song from a paired device, and can download that file into the library. Pairing is by LAN discovery or by IPv4 address and port. Both computers must accept. Sync traffic is end-to-end encrypted.

Liked songs and appearance can sync as a full copy. Recently played is recorded after about 10 seconds, or half the song, and can merge by UTC time. Incognito cuts device connections and stops discovery, sync, and history recording. Device block and per-item sync blocks are included. The app has light and dark themes, English and Chinese, lyrics, a tray icon, a mini player, and playback buttons on the Windows taskbar thumbnail.

The program is `TunePeerly.exe`. The database is in `%APPDATA%\TunePeerly`. Covers and the window cache are in `%LOCALAPPDATA%\TunePeerly`.

Some settings are marked because they are not finished. **To be revised** means this version keeps the current behavior. **Do not use** means do not use it, because the feature has problems. **Not completed yet, not tested** and **Do not use** are on Relay. Its client is included. It is not completed yet, and it has not been tested. The guide lists which setting has which mark. The mark does not describe the problem. Follow playback is also marked do not use.

## 更新日志

这里说明每个发布版本的情况。0.6.0 是以 TunePeerly（声渡）这个名字发布的第一个版本。

## [0.6.0] - 2026-10-19

TunePeerly 从原始文件播放本机音频，可以从已配对设备串流已公开的歌曲，也可以把那个文件下载进曲库。配对通过局域网发现，或通过 IPv4 地址和端口。两台电脑都要接受。同步数据端到端加密。

喜欢的歌曲和外观可以按整份副本同步。最近播放在大约 10 秒或歌曲一半之后记下，并可以按 UTC 时间合并。无痕模式会断开设备连接，并停止发现、同步和播放记录。包含拉黑设备，以及按项目阻止同步。程序有浅色和深色主题、中英文、歌词、托盘图标、迷你播放器，以及 Windows 任务栏缩略图上的播放按钮。

程序名是 `TunePeerly.exe`。数据库位于 `%APPDATA%\TunePeerly\library.db`。封面在 `%LOCALAPPDATA%\TunePeerly\covers`。

有些设置因为还没完成而标了标签。**待修改**表示这个版本先保持现在的行为。**不要使用**就是不要使用，这个功能有问题。**待完善且未测试**和**不要使用**标在中继上：客户端在这个版本里，但还没完善，也还没测试。手册列出了每个标签对应哪些设置。标签不说明具体问题。跟随播放也标着不要使用。
