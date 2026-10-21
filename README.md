# winisland

A tiny Dynamic Island-style overlay for Windows. It sits flush against the top
edge of the screen as a thin strip, stays out of your way, and expands when you
hover over it.

The main goal of the project is **low memory use**. It runs as a tray app, with no
taskbar icon and no Alt+Tab entry.

## Features

- Idle state is a thin strip at the top edge of the screen; hovering expands it with an animation
- Shows the title and artist of the media that is currently playing (via Windows media controls)
- Tray icon with **Show / Hide**, **Autostart** (on/off) and **Quit**
- Starts with Windows when Autostart is enabled (registry `HKCU\...\Run`, no admin rights)
- Does not steal focus when you click it

## Status

Early and personal-scale. It works on the author's machine (Windows 11), and a lot is still untested.

Known limitations:

- Notifications are not supported yet (it depends on Windows package identity; still being investigated)
- Only the primary monitor is used; multi-monitor and high-DPI setups are untested
- Exclusive-fullscreen games are untested
- Windows 10 is untested
- If Autostart is turned off from *Settings > Apps > Startup*, the tray menu may still show "On"

Measured on the author's machine with the `dist` build (single measurements, not a benchmark):
about 7.3 MB executable, about 2.9 MB private working set at idle, 0% idle CPU.

## Build from source

Requirements:

- Windows 11
- Rust (stable, `x86_64-pc-windows-msvc` toolchain)
- *Build Tools for Visual Studio* with the **Desktop development with C++** workload (the full Visual Studio IDE is not needed)

`setup.ps1` checks all of this without changing anything:

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

The result is `target\dist\winisland.exe`. To use it day to day, quit the app first, copy the file to a
fixed location such as `%LOCALAPPDATA%\winisland\`, run it from there, then enable **Autostart** from
the tray menu. Enable Autostart only from a release/dist build in a fixed location, because the registry
entry stores the path of the running executable.

## Contributing

Contributions are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) first.

## License

The source code of winisland is released under the [MIT License](LICENSE).

winisland is built with [Slint](https://slint.dev), which is used under the
[Slint Royalty-free Desktop, Mobile, and Web Applications License, version 2.0](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md).
That license requires attribution to Slint, which is why the badge below is shown. If you distribute binaries
of this project or of a fork, you must comply with Slint's license terms as well (the Royalty-free license with
attribution, or the GPLv3 for the whole work). This is not legal advice; read the license texts.

<a href="https://slint.dev"><img src="https://raw.githubusercontent.com/slint-ui/slint/master/logo/MadeWithSlint-logo-whitebg.png" alt="Made with Slint" width="160"></a>
