English | [简体中文](guide.zh-CN.md)

# TunePeerly user guide

TunePeerly (声渡) plays audio files on this computer and shares them with computers you pair. There is no account. This guide matches version 0.6.0. Version 0.6.0 is an alpha test release. The program is not guaranteed to match this guide.

## Install

The GitHub release is an Inno Setup installer. It asks whether to install for all users or only for the current user. The current user is selected by default and uses `C:\Users\<name>\AppData\Local\Programs\TunePeerly`. That choice does not need an administrator account. All users uses `C:\Program Files\TunePeerly` and does. The database and the cache stay in their own fixed folders, below, and are not changed by that choice.

The installer does not add a firewall rule. The first time the app listens for other devices, Windows Defender Firewall may ask whether to allow `TunePeerly.exe`. Allow it on the networks you use. A later copy of the program at a different path is treated as a different program and is asked about again. Uninstalling asks whether to remove the rules Windows created for that copy. Agreeing asks for an administrator account when the uninstall itself is not running as one.

## Add music

Open **Music folder** in Settings, or drop folders and audio files onto the window. The app reads the files where they are. It does not copy them and it does not transcode them.

**Scan** on the All music page, and **Rescan** in Settings, read every added folder again. New songs are added. A changed song, or a `.lrc` file you added or edited next to it, is read again. Nothing else changes.

Removing a folder only removes it from TunePeerly. The files stay on disk. Its songs stay in the library as unavailable, with their tags, covers, favorites, playlists and recently played. Add the folder again and they play again.

## Play

The bar at the bottom controls this computer's own queue: play, pause, skip, seek, shuffle, and repeat. Sound uses the system's default output device.

Right-click a song in a playlist for **Play next**, **Add to queue**, **Add to playlist**, **Save to Liked Songs**, and others.

The queue is only on this computer. It is not synced.

When a song has LRC lyrics, the lyrics button scrolls with the song.

## Window, tray, and mini player

The window has its own title bar. **When closing** in Settings → Appearance chooses what the close button does:

**Mini player when minimized** and **Mini player in the tray** leave a small bar on screen with the cover, playback controls, and the heart, when the window is minimized or hidden in the tray. Turn both off for no bar. Drag the cover, the title, or the empty part of the bar to move it. Click the cover or the title to open the main window. The cross in its corner closes only the bar; it comes back the next time the window is minimized or hidden.

The Windows taskbar thumbnail shows the whole cover, scaled down to fit, with previous, play, next, and a heart.

## Playlists and exposure

A playlist is a list of songs. Deleting a playlist does not delete the songs.

**New local songs**, in Settings → Music folder, decides whether a song that is not governed by a playlist is offered to paired devices. Each playlist can follow that default, always expose new songs, or never expose them. That choice applies when a song is added. **Expose all now** and **Stop exposing all** change songs that are already in the list.

A paired device can see and play a song only when this computer is exposing it.

## Unfinished settings

Some settings show a short mark beside the title. The mark does not describe the problem.

- **To be revised** means this version keeps the current behavior, and it will change later. The mark is on **Paired devices**, **Nearby**, **Connect by address**, **Sync**, **Auto send**, **When a sync conflicts**, **Sync recently played**, **Merge when this computer comes online**, **LAN port**, and **Device ID**.
- **Do not use** means do not use it. The feature has problems. The mark is on **Follow playback** and **Relay**.
- **Not completed yet, not tested** is also on **Relay**. A relay implementation is in this version. It is not completed yet, and it has not been tested.

The rest of this guide says what those controls are for.

## Pair a device

Open **Devices**. **Paired devices**, **Nearby**, and **Connect by address** are marked **To be revised**. Before pairing, **Discover nearby devices** in Settings → Network must be on, or devices cannot be found.

- **Discover nearby devices** announces this computer's name about every 3 seconds, over UDP multicast.
- You can also type another computer's IPv4 address and port.
- Pairing finishes only after both computers accept. Until then the connection is pending.
- After you decline a request, that device cannot ask again until **Silence after a decline** has passed. That setting is under History and privacy. The default is 60 seconds, and the allowed range is 1 to 86400. You can still connect to that device yourself.
- Block removes a device. Later data from it is dropped.

The four relay fields are under Settings → Network: address, port, the relay's certificate ID, and a token. **Relay** is marked **Not completed yet, not tested** and **Do not use**.

**LAN port** and **Device ID** are under Settings → Storage, and both are marked **To be revised**.

## Play a song that is on another computer

A song offered by an online paired device can be played from that device. **Download** saves the original file into the downloads folder and adds it to this library.

## Sync

The **Sync** heading, **Auto send**, and **When a sync conflicts** are marked **To be revised**.

Incognito cuts every device connection, stops discovery, and turns sync off.

| Control | What it does |
| --- | --- |
| Send | Push this computer's copy to every paired device that is online now. |
| Override | Ask one online device for its copy and replace this computer's copy with it. |

**Silent overwrite** replaces the copy immediately. **Ask before overwrite** waits for a confirmation. Override does not ask again, because this computer requested that copy.

Appearance sync includes the theme, the language, the two mini player switches, and the close-window choice.

## Recently played

**Sync recently played** and **Merge when this computer comes online** are marked **To be revised**.

## Follow playback

**Follow playback** is marked **Do not use**.

## Notifications

The bell opens the notification list. You can mark items read when they scroll into view, when the panel opens, or only when you open one. Clearing can ask every time, delete everything except pinned items, or delete everything.

## Where your data stays

The database is in `%APPDATA%\TunePeerly`. Covers and the window cache are in `%LOCALAPPDATA%\TunePeerly`. These two folders cannot be changed. Uninstalling asks whether to delete them.

There is no account. Paired devices on the same local network sync and stream directly. Outside that network they can meet through the relay configured under Settings → Network. **Relay** is marked **Not completed yet, not tested** and **Do not use**.
