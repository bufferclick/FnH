<p align="center"><img src="fh/icons/fh-256.png" width="88" height="88" alt="FNH's icon: a deep-navy F and H with a black n on a light rounded tile"></p>

<h1 align="center">FNH</h1>

<p align="center">
  Fabric n Horizon — a native multi-window Windows 11 launcher by <b>Buffer</b> for your Fabric and Horizon instances.<br>
  <a href="https://fnh.is-cool.dev"><b>fnh.is-cool.dev</b></a> ·
  <a href="https://github.com/bufferclick/FnH/releases/latest/download/FnHLauncher.zip"><b>Download FnHLauncher.zip</b></a> ·
  <a href="fh/README.md"><b>Documentation</b></a>
</p>

---

FNH (`v1.7`) is a single-executable desktop application for Windows 10 and 11 (x64) by **Buffer**,
modelled on **JLac**'s Windows 11 (WinUI 3 / `QWindows11Style`) desktop launcher. It manages and
launches Minecraft: Java Edition instances (with **Fabric** or vanilla) alongside isolated native
**Horizon** instances — no installer, no embedded browser, and no background service.

## Install & Auto-Update

1. Download [`FnHLauncher.zip`](https://github.com/bufferclick/FnH/releases/latest/download/FnHLauncher.zip) from [Releases](https://github.com/bufferclick/FnH/releases/latest).
2. Unzip it anywhere you can write to — `%LOCALAPPDATA%\FNH`, a folder on your desktop, or a USB
   stick.
3. Double-click `FH.exe`. When newer versions release, FNH prompts **"Do you want to update FnH?"**
   on startup, downloads `FnHLauncher.zip`, extracts the update, deletes the zip, and restarts
   automatically.

Windows 10 or 11 on x64. Nothing else is required: no .NET, no Visual C++ redistributable, and no
separate Java install (FNH automatically fetches the matching Mojang Java runtime when needed).

## What's in v1.7

- **Crisp 9×9 FnH icon & complete Fluent SVG suite.** Restored the exact 9×9 `FnH` monogram across
  the toolbar, About/Updater/Crash windows, `default.png` preset icon, and Windows `WM_SETICON`
  titlebar/Alt-Tab icons, plus fixed every UI glyph across `MainWindow` and `InstanceWindow`.
- **Full upstream Horizon `v0.2` mod support (`hmods.rs`).** Supports Horizon Registry `index.json`
  (`api = "0.2"` with `SHA-256` + `minisign` verification), Content & WebAssembly `.hmod` packages
  in `horizon-mods/`, Native Rust `.hmod` packages in `horizon-native-mods/` (automatically linked
  via Horizon's official Native Mod SDK `horizon-launcher native-build`), direct **GitHub** `.hmod`
  installation (`owner/repo`), and Modrinth `.jar` auto-porting via `horizon-port-pack`.
- **Fabric-style Crash & Mod Compatibility Window (`crash.rs`, `CrashWindow`).** Pre-launch and
  post-exit analyzer for Fabric `Incompatible mod set!` / `FormattedException`, Forge/NeoForge
  `.jar` mismatches, missing `fabric-api`, `crash-reports/crash-*.txt`, and unsupported Horizon mods
  — complete with *Potential Solutions* and **1-click Disable Mod**.
- **Built-in startup self-updater (`updater.rs`).** Checks GitHub Releases on launch (and via the
  top toolbar `Update` button), asks *"Do you want to update FnH?"*, downloads `FnHLauncher.zip`,
  extracts `FH.exe`, deletes `FnHLauncher.zip`, and restarts into the updated build.
- **True multi-window Windows 11 shell.** Separate OS windows for **Settings** (9 pages),
  **Edit Instance** (11 pages), **New Instance**, **Icon Picker** (61 icons + custom import),
  **Download Mods / Packs / Shaders**, and **Crash & Mod Compatibility**.

Every launch step is recorded in `%APPDATA%\FnH\logs\fh.log`.

## Releases

`fh/VERSION` holds `1.7`, the release tag is `fnh-v1.7`, and the download archive is always named
`FnHLauncher.zip`. [fnh.is-cool.dev](https://fnh.is-cool.dev) and the Download link above always
resolve to the newest release.

## Licence

MIT — created by **Buffer**. FNH is not an official Minecraft product and is not approved by or
associated with Mojang or Microsoft. Horizon is made by the Horizon Team; FNH is a front end for
Fabric and Horizon styled after Buffer's JLac. The upstream Horizon workspace README is
[HORIZON.md](HORIZON.md).
