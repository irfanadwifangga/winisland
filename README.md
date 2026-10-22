# winisland

A tiny Dynamic Island-style overlay for Windows. It sits flush against the top edge of the screen as a thin strip, stays out of your way, and expands when you hover over it.

The main goal of the project is **low memory use and near-zero idle CPU**. It runs as a lightweight tray app with no taskbar icon and no Alt+Tab entry.

## Features

- **Idle strip & animated expand:** Idle state is a subtle 4 px strip at the top edge of the screen; hovering expands it smoothly into a dynamic island pill.
- **Now Playing media controls:** Tracks media title, artist, and playback state in real time via Windows SMTC events. Includes on-screen Previous, Play/Pause, and Next buttons.
- **Action Center notifications:** Reads active toast notifications, grouping duplicates and cleaning text. A badge shows the notification count; clicking it switches between media and notifications. Automatically shows notifications on hover if nothing is playing.
- **Notification privacy switch:** By default, only app names, titles, and age are shown (message bodies are never fetched). Enable "Notification body: Shown" from the tray menu to display message contents.
- **Multi-monitor & DPI-aware placement:** Accurately places the overlay at the top-center of the active screen across mixed DPIs and negative coordinates. Cycle displays directly from the tray menu.
- **Fullscreen app detection:** Automatically suppresses the overlay when playing fullscreen games or watching fullscreen videos (YouTube, media players) to prevent distraction or accidental clicks. Can be toggled on/off in the tray menu.
- **System tray menu:**
  - **Show / Hide** — toggle overlay visibility manually
  - **Display: Primary / 2 / ...** — cycle target monitor
  - **Autostart: On / Off** — launch on Windows startup (`HKCU\...\Run`, no admin rights required)
  - **Notification body: Hidden / Shown** — toggle notification message body privacy
  - **Hide in fullscreen: On / Off** — toggle auto-hide during games and fullscreen media
  - **Quit** — close the application
- **Non-activating window:** Does not steal focus from your active game, browser, or editor when clicked.

## Status

Developed and tested on Windows 11.

Known behaviors & notes:
- **Notification polling:** Notifications are fetched when the pill expands. Windows does not allow unpackaged Win32 apps to register for `NotificationChanged` events, so notifications do not push updates on their own.
- **Memory footprint:** In the `dist` build, executable size is ~7.4 MB. RAM usage is ~2.9 MB private working set at startup and ~4.3 MB after reading notifications (due to WinRT runtime activation). Idle CPU is 0%.
- **Windows 10:** Untested (Windows 11 recommended).

## Build from source

Requirements:

- Windows 11
- Rust (stable, `x86_64-pc-windows-msvc` toolchain)
- _Build Tools for Visual Studio_ with the **Desktop development with C++** workload (the full Visual Studio IDE is not needed)

`setup.ps1` checks all requirements without modifying anything:

```powershell
.\setup.ps1          # environment check
.\setup.ps1 -Check   # also runs `cargo check`
```

Run during development:

```powershell
cargo run
```

Build the small, optimized binary:

```powershell
cargo build --profile dist
```

The resulting binary is located at `target\dist\winisland.exe`. To use it daily:
1. Quit any running debug instance.
2. Copy `target\dist\winisland.exe` to a permanent folder (such as `%LOCALAPPDATA%\winisland\`).
3. Run it from that location and enable **Autostart** from the tray menu.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first.

## License

The source code of winisland is released under the [MIT License](LICENSE).

winisland is built with [Slint](https://slint.dev), which is used under the [Slint Royalty-free Desktop, Mobile, and Web Applications License, version 2.0](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md). That license requires attribution to Slint, which is why the badge below is shown. If you distribute binaries of this project or of a fork, you must comply with Slint's license terms as well (the Royalty-free license with attribution, or the GPLv3 for the whole work). This is not legal advice; read the license texts.

<a href="https://slint.dev"><img src="https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png" alt="Made with Slint" width="160"></a>
