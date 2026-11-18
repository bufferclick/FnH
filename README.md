<p align="center"><img src="fh/icons/fh-256.png" width="88" height="88" alt="FNH's icon: a deep-navy F and H with a black n on a light rounded tile"></p>

<h1 align="center">FNH</h1>

<p align="center">
  Fabric n Horizon — a native Windows 11 launcher for your Fabric and Horizon instances.<br>
  <a href="https://fnh.is-cool.dev"><b>fnh.is-cool.dev</b></a> ·
  <a href="https://github.com/bufferclick/fnh/releases/latest"><b>Download</b></a> ·
  <a href="fh/README.md"><b>Documentation</b></a>
</p>

---

FNH is a single-executable desktop application for Windows 10 and 11 (x64), built with the clean
Windows 11 (WinUI 3) layout and widget style of **JLac**. It manages and launches Minecraft: Java
Edition instances (with **Fabric** or vanilla) alongside native **Horizon** instances — no
installer, no embedded browser, and no background service.

## Install

1. Download `fnh-1.5.zip` from [Releases](https://github.com/bufferclick/fnh/releases/latest).
2. Unzip it anywhere you can write to — `%LOCALAPPDATA%\FNH`, a folder on your desktop, or a USB
   stick.
3. Double-click `FH.exe`.

Windows 10 or 11 on x64. Nothing else is required: no .NET, no Visual C++ redistributable, and no
separate Java install (FNH fetches the matching Mojang Java runtime when an instance needs one).

## What it does

- **Windows 11 (WinUI 3) interface.** Modeled on JLac's `windows11` style: top toolbar
  (`Add Instance...`, `Folders`, `Settings...`, `Help`, `Update...`, `Accounts`), grouped
  `InstanceView` grid in the centre, right-hand `WideBar` (`Launch`, `Kill`, `Edit...`,
  `Change Group...`, `Folder`, `Copy...`, `Delete`), and bottom status bar with total playtime.
  Follows Windows 11 Light and Dark modes automatically or can be pinned in `Settings...`.
- **Add Instance.** Create a Minecraft: Java Edition instance with Fabric Loader (or vanilla) from
  Mojang's live version list, or create a native Horizon instance with optional grouping and icon
  colour.
- **Edit Instance.** Per-instance window with **Minecraft Log**, **Version** (install, update, pin
  or remove Fabric Loader), **Mods** (enable/disable `.jar` files, remove, open folder), **Worlds**
  (mode, version, size, folder), and **Settings** (memory, JVM flags, window size, launch hooks).
- **Accounts.** Multi-account Microsoft device-code sign-in with one-click code copy and browser
  launch, plus quick account switching from the top-right toolbar menu.
- **Factory reset shortcut.** Hold `Ctrl + Shift + Alt` while clicking `Add Instance...` to delete
  all FNH data (instances, accounts, saves, mods, caches, preferences and logs) and immediately
  relaunch `FH.exe` as a fresh install.

Every launch step is recorded in `%APPDATA%\horizon-launcher\fh\fh.log`.

## How it works

FNH is written in Rust on top of [GPUI](https://gpui.rs) (`gpui-fast`) and `horizon-launcher`:
`look.rs` implements WinUI 3 (`QWindows11Style`) Light and Dark surfaces and Fluent controls,
`icons.rs` embeds a crisp 24×24 vector icon set, `app.rs` and `dialogs.rs` implement JLac's
`MainWindow` and modal dialogs, `reset.rs` handles full data wipe and clean relaunch, and `job.rs`
runs launches, process termination (`Kill`) and Microsoft login on background threads. Full details
live in [fh/README.md](fh/README.md).

## Releases

`fh/VERSION` holds `1.5`, the release tag is `fnh-v1.5`, and the download archive is `fnh-1.5.zip`.
[fnh.is-cool.dev](https://fnh.is-cool.dev) and the Download link above always resolve to the newest
release.

## Licence

MIT. FNH is not an official Minecraft product and is not approved by or associated with Mojang or
Microsoft. Horizon is made by the [Horizon Team](https://github.com/horizon-rust/horizon); FNH is a
front end for it, styled after JLac. The launcher's upstream README is [HORIZON.md](HORIZON.md).
