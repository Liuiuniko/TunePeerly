# Security policy

## Supported versions

| Version | Supported |
| --- | --- |
| 0.6.x | Yes |

## Reporting a vulnerability

Report privately. Do not open a public GitHub issue, and do not post proof-of-concept details in a pull request.

Use [a private security advisory](https://github.com/zhebuilds/TunePeerly/security/advisories/new).

Include the TunePeerly version, the operating system, the steps to reproduce, and what an attacker gains. If you can, name the pairing, sync, or streaming path involved.

Please give the maintainer time to confirm and ship a fix before any public disclosure.

## In scope

- Pairing and device identity, including accepting a device the user did not confirm
- End-to-end encryption of sync and signaling traffic, including anything the configured relay can read
- Applying a sync copy, history list, or playback position from a device that is not paired and confirmed
- Streaming or downloading a song this computer did not expose
- Reading or writing files outside the library, the download folder, or the app data folder

## Out of scope

- The device name announced on the local network. Discovery sends the name only.
- A person who already confirmed the pairing on both computers.
- The Windows Firewall prompt. The app does not create that rule.
- Vulnerabilities in dependencies that do not affect TunePeerly.

## 安全策略

### 支持的版本

| 版本 | 是否支持 |
| --- | --- |
| 0.6.x | 是 |

### 如何私下报告

请私下报告。不要开公开的 GitHub issue，也不要在 pull request 里贴出利用细节。

使用[私人安全公告](https://github.com/zhebuilds/TunePeerly/security/advisories/new)。

请写上 TunePeerly 版本、操作系统、复现步骤，以及攻击者能得到什么。如果可以，请指出涉及的是配对、同步还是串流。

在维护者确认并发布修复之前，请留出时间，不要公开披露。

### 范围内

- 配对和设备身份，包括未经用户确认就接受一台设备。
- 同步和信令的端到端加密，包括你配置的中继能够读到的内容。
- 采用未配对或未确认设备发来的同步副本、最近播放或播放进度。
- 串流或下载这台电脑没有公开的歌曲。
- 读写曲库、下载文件夹和应用数据文件夹以外的文件。

### 范围外

- 局域网发现时广播的设备名。发现只发送名字。
- 已经在两台电脑上确认过配对的人。
- Windows 防火墙的询问框。程序不创建那条规则。
- 不影响 TunePeerly 的依赖漏洞。
