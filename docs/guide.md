English | [简体中文](guide.zh-CN.md)

# TunePeerly user guide

TunePeerly (声渡) plays audio files on this computer and shares them with computers you pair. There is no account. This guide matches version 0.6.0.

## Install

Run the NSIS installer, or the MSI if you built one. The NSIS installer installs for the current Windows user and does not need an administrator account. The MSI installs into Program Files and does ask for administrator permission.

Windows 11 already includes the WebView2 runtime the window needs. On Windows 10 the installer downloads it when it is missing and the computer is online.

The installer does not add a firewall rule. The first time the app listens for other devices, Windows Defender Firewall may ask whether to allow `TunePeerly.exe`. Allow it on the networks you use. A later copy of the program at a different path is treated as a different program and is asked about again. Search the firewall list for `tunepeerly.exe`.

## Add music

Open **Library** in Settings, or drop folders and audio files onto the window. The app reads the files where they are. It does not copy them and it does not transcode them.

Removing a folder from the library only removes it from TunePeerly. The files stay on disk.

**Show unavailable music** keeps songs from offline devices in the library and in search. Those rows are dimmed.

## Play

The bar at the bottom is the player. Play, pause, skip, seek, shuffle, and repeat apply to the queue on this computer. Sound uses the system's default output device.

Right-click a song for **Play next**, **Add to queue**, **Add to playlist**, and **Save to Liked Songs**.

The queue is only on this computer. It is not synced.

Open the lyrics button to follow an LRC file when the song has one. The heart on the player saves the current song to Liked Songs.

## Window, tray, and mini player

The window has its own title bar. **When closing** in Settings → Appearance chooses what the close button does:

- **Ask me** shows Quit or Minimize to tray each time.
- **Minimize to tray** leaves playback running. Use the tray menu to show the window again or quit.
- **Quit** exits.

**Mini player when minimized** and **Mini player in the tray** leave a small bar on screen with the cover, playback controls, and the heart, when the window is minimized or hidden in the tray. Turn both off for no bar. Drag the cover, the title, or the empty part of the bar to move it. Click the cover or the title to open the main window. The cross in its corner closes only the bar; it comes back the next time the window is minimized or hidden.

The Windows taskbar thumbnail shows the whole cover, scaled down to fit, with previous, play, next, and a heart. The heart is white when the song is not liked and red when it is. Hovering that thumbnail shows a picture of this window. After the window is minimized, the picture is the one taken just before it was minimized.

## Playlists and exposure

A playlist is a list of songs. Deleting a playlist does not delete the songs.

**New local songs**, in Settings → Library, decides whether a song that is not governed by a playlist is offered to paired devices. Each playlist can follow that default, always expose new songs, or never expose them. That choice applies when a song is added. **Expose all now** and **Stop exposing all** change songs that are already in the list.

A paired device can see and play a song only when this computer is exposing it. Songs without a content id stay on this computer.

## Unfinished settings

Some settings show a short mark beside the title. The mark does not describe the problem.

- **To be revised** means this version keeps the current behavior, and it will change later. The mark is on **Paired devices**, **Nearby**, **Connect by address**, **Sync**, **Auto send**, **When a sync conflicts**, **Sync recently played**, **Merge when this computer comes online**, **LAN port**, and **Device ID**.
- **Do not use** means do not use it. The feature has problems. The mark is on **Follow playback** and **Relay**.
- **Not completed yet, not tested** is also on **Relay**. The relay client is in this version. It is not completed yet, and it has not been tested.

The rest of this guide says what those controls are for.

## Pair a device

Open **Devices**. **Paired devices**, **Nearby**, and **Connect by address** are marked **To be revised**.

- **Discover nearby devices** announces this computer's name about every 3 seconds, by UDP multicast and by a broadcast on each local subnet. Song names are not in that announcement.
- You can also type another computer's IPv4 address and port.
- Pairing finishes only after both computers accept. Until then the connection is pending.
- After you decline a request, that device cannot ask again until **Silence after a decline** has passed. That setting is under History and privacy. The default is 60 seconds, and the allowed range is 1 to 86400. You can still connect to that device yourself.
- Block removes a device. Its later traffic is dropped.

The four relay fields are under Settings → Network: address, port, the relay's certificate ID, and a token. When all four are set, this computer connects to that [strelaysrv](https://github.com/syncthing/relaysrv) relay, and paired devices can reach each other through it. The devices encrypt that traffic. **Relay** is marked **Not completed yet, not tested** and **Do not use**.

**LAN port** and **Device ID** are under Settings → Storage, and both are marked **To be revised**. The port is saved and reused. The allowed range is 1024–65535. If that port is busy at startup, the app listens on another port for this run and shows a warning. The saved port is not changed.

## Play a song that is on another computer

A song offered by an online paired device can be played from that device. The player shows that device's name. **Download** saves the original file into the downloads folder and adds it to this library. The folder is chosen the first time you download.

## Sync

The **Sync** heading, **Auto send**, and **When a sync conflicts** are marked **To be revised**.

Incognito cuts every device connection, stops discovery, and turns sync off. Turn it off again before using this section.

Liked songs and appearance each sync as one whole copy. A copy is the current list, not a list of individual edits. **Auto send** only offers those two. Playlists stay on this computer. Recently played has its own switches, below.

| Control | What it does |
| --- | --- |
| Auto send | After you change a checked item, send that copy to every paired device that is online. |
| Follow | When the chosen device changes that item, take its copy automatically. |
| Send | Push this computer's copy to every paired device that is online now. |
| Override | Ask one online device for its copy and replace this computer's copy with it. |
| No manual | This computer does not push its copy with Send. A copy another device sends here still applies. If another device asks for this computer's copy, it still receives it. |
| No following | Ignore automatic copies from the device selected under Follow. |
| Stop all | Do not push and do not follow. A copy another device sends here can still apply. |

Incognito still blocks a copy from leaving or arriving. The copy you receive is the other computer's current copy.

**When others overwrite this computer** applies when another device presses Send. **Silent** replaces the copy immediately. **Ask** waits for a confirmation. Override does not ask again, because this computer requested that copy.

**When a sync conflicts** applies when an automatic copy arrives and this computer still has changes that were not sent. **Always keep mine** drops the incoming copy. **Always use theirs** replaces the local copy. **Ask me** waits.

Appearance sync includes the theme, the language, the two mini player switches, and the close-window choice.

## Recently played

**Sync recently played** and **Merge when this computer comes online** are marked **To be revised**.

A song is added after it has played for about 10 seconds, or for half its length when that is shorter. A song that ends after more than one second of playback is also added. **Pause recording** and Incognito stop this. **Songs kept in Recently Played** is the maximum length of the list, from 1 to 10000.

**Sync recently played** sends this computer's list to paired devices that are online. The other device merges it only when its own switch is on. For each song, the later UTC time is kept. An equal time leaves the entry that was already there.

**Merge when this computer comes online** asks each paired device that is online. It waits up to 8 seconds for replies, merges them into this computer's list, then sends that list. A device with **Sync recently played** off does not answer and does not record a notification.

## Follow playback

**Follow playback** is marked **Do not use**.

In Settings → Network, **Follow playback** lists paired devices that are online. Choose one.

The bar at the bottom then shows that device's current song. The progress bar moves while that device is playing and stops when it pauses. The play button stays on play. This computer does not play sound, and the playback buttons on this bar do not start playback here.

Choose **Don't follow** to use this computer's player again.

Both computers need a build that includes this feature. The computer you follow sends its position about once a second while it has a song loaded. Incognito stops that exchange.

## Notifications

The bell opens the notification list. You can mark items read when they scroll into view, when the panel opens, or only when you open one. Clearing can ask every time, delete everything except pinned items, or delete everything.

## Where your data stays

`%APPDATA%\TunePeerly\library.db` holds the library, playlists, likes, history, pairings, and settings. Cover images are under `%LOCALAPPDATA%\TunePeerly\covers`. Uninstalling the app does not delete either folder.

There is no account. Paired devices on the same local network sync and stream directly. Outside that network they can meet through the relay configured under Settings → Network. **Relay** is marked **Not completed yet, not tested** and **Do not use**.
