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

**Mini player when minimized** and **Mini player in the tray** leave a small bar on screen with the cover, playback controls, and the heart, when the window is minimized or hidden in the tray. Turn both off for no bar. Drag the empty area to move it. Click the cover or the title to open the main window. The cross in its corner closes only the bar; it comes back the next time the window is minimized or hidden.

The Windows taskbar thumbnail shows the whole cover, scaled to fit, with previous, play, next, and a heart. The heart is white when the song is not liked and red when it is. Hovering the thumbnail previews the window itself.

## Playlists and exposure

A playlist is a list of songs. Deleting a playlist does not delete the songs.

**New local songs**, in Settings → Library, decides whether a song that is not governed by a playlist is offered to paired devices. Each playlist can follow that default, always expose new songs, or never expose them. That choice applies when a song is added. **Expose all now** and **Stop exposing all** change songs that are already in the list.

A paired device can see and play a song only when this computer is exposing it. Songs without a content id stay on this computer.

## Pair a device

Open **Devices**.

- **Discover nearby devices** announces this computer's name on the local network a few times a minute. Song names are not in that announcement.
- You can also type another computer's address and port, or paste a pairing code.
- Pairing finishes only after both computers accept. Until then the connection is pending.
- After you decline a request, that device cannot ask again until **Silence after a decline** has passed. The default is 60 seconds. You can still connect to that device yourself.
- Block removes a device. Its later traffic is dropped.

Outside the local network, fill in all four relay fields under Settings → Network. The relay only forwards traffic between your devices. The devices encrypt that traffic, so the relay cannot read it.

**LAN port** is saved and reused. The allowed range is 1024–65535. If that port is busy at startup, the app listens on a temporary port and shows a warning. The saved port is not changed.

## Play a song that is on another computer

A song offered by an online paired device can be played from that device. The player shows that device's name. **Download** saves the original file into the downloads folder and adds it to this library. The folder is chosen the first time you download.

## Sync

Incognito turns sync off. Turn it off again before using this section.

Liked songs and appearance each sync as one whole copy. A copy contains the current list, not a list of individual edits. Playlists stay on this computer.

| Control | What it does |
| --- | --- |
| Auto send | After you change a checked item, send that copy to every paired device that is online. |
| Follow | When the chosen device changes that item, take its copy automatically. |
| Send | Push this computer's copy to every paired device that is online now. |
| Override | Ask one online device for its copy and replace this computer's copy with it. |
| No manual | This computer does not send, and it does not answer another device's Override. |
| No following | Ignore automatic copies. |
| Stop all | Do not send and do not follow. |

Override ignores **No manual**, **No following**, and **Stop all** on both computers. Incognito still blocks it. The copy you receive is the other computer's current copy.

**When others overwrite this computer** applies when another device presses Send. **Silent** replaces your copy immediately. **Ask** waits for you to confirm. Override does not ask again, because you requested that copy.

**When a sync conflicts** applies when you follow a device and this computer also has changes that have not been sent. You can keep yours, take theirs, or be asked each time.

Appearance sync includes the theme, the language, the two mini player switches, and the close-window choice.

## Recently played

A song is recorded when it finishes, unless **Pause recording** or Incognito is on. **Songs kept in Recently Played** is the maximum length of the list, from 1 to 10000.

**Sync recently played** sends that list to paired devices that are online. A device applies the list only when its own switch is on. The two lists merge by the latest UTC time for each song. Equal times keep one entry.

**Merge when this computer comes online** asks every paired device for its list, merges those replies, and sends the result. Devices that do not sync history ignore the request.

## Follow playback

In Settings → Network, **Follow playback** lists paired devices that are online. Choose one.

The bar at the bottom then shows that device's current song. The progress bar moves while that device is playing and stops when it pauses. The play button stays on play. This computer does not play sound, and the playback buttons on this bar do not start playback here.

Choose **Don't follow** to use this computer's player again.

Both computers need a build that includes this feature. The computer you follow sends its position about once a second while it has a song loaded. Incognito stops that exchange.

## Notifications

The bell opens the notification list. You can mark items read when they scroll into view, when the panel opens, or only when you open one. Clearing can ask every time, delete everything except pinned items, or delete everything.

## Where your data stays

`%AppData%\Roaming\TunePeerly\library.db` holds the library, playlists, likes, history, pairings, and settings. Uninstalling the app does not delete that folder.

There is no account. Away from the local network, sync and streaming go through the relay you configured. The relay forwards the encrypted traffic and cannot read it.
