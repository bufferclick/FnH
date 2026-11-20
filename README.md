<p align="center"><img src="fh/icons/fh-256.png" width="88" height="88" alt="FNH's icon: a deep-navy F and H with a black n on a light rounded tile"></p>

<h1 align="center">FNH</h1>

<p align="center">
  Fabric n Horizon — a native multi-window Windows 11 launcher by <b>Buffer</b> for your Fabric and Horizon instances.<br>
  <a href="https://fnh.is-cool.dev"><b>fnh.is-cool.dev</b></a> ·
  <a href="https://github.com/bufferclick/fnh/releases/latest"><b>Download</b></a> ·
  <a href="fh/README.md"><b>Documentation</b></a>
</p>

---

FNH (`v1.6`) is a single-executable desktop application for Windows 10 and 11 (x64) by **Buffer**,
modelled on **JLac**'s Windows 11 (WinUI 3 / `QWindows11Style`) desktop launcher. It manages and
launches Minecraft: Java Edition instances (with **Fabric** or vanilla) alongside isolated native
**Horizon** instances — no installer, no embedded browser, and no background service.

## Install

1. Download `fnh-1.6.zip` from [Releases](https://github.com/bufferclick/fnh/releases/latest).
2. Unzip it anywhere you can write to — `%LOCALAPPDATA%\FNH`, a folder on your desktop, or a USB
   stick.
3. Double-click `FH.exe`.

Windows 10 or 11 on x64. Nothing else is required: no .NET, no Visual C++ redistributable, and no
separate Java install (FNH automatically fetches the matching Mojang Java runtime when needed).

## What's in v1.6

- **True multi-window Windows 11 shell.** Separate OS windows for **Settings** (9 global pages:
  Launcher, Appearance, Minecraft, Horizon, Java, Accounts, APIs, External Tools, Proxy), per-instance
  **Edit Instance** (11 pages: Minecraft Log, Version, Mods, Resource Packs, Shader Packs, Notes,
  Worlds, Servers, Screenshots, Settings, Other Logs), **New Instance**, **Icon Picker** (61 built-in
  128×128 icons + custom image import), and **Download Mods / Packs / Shaders**.
- **Isolated `<FnH>` data root.** Stores instances, Java runtimes, Horizon builds, icons, logs and
  settings under `%APPDATA%\FnH` so FNH never collides with or inherits stray instances from other
  launchers.
- **Modrinth + Horizon Registry & auto-porting.** Browse and install mods, resource packs and shaders
  directly inside FNH, or drag-and-drop `.jar`, `.hmod`, `.hnmod` and mod folders onto any instance
  (Fabric `.jar` packs dropped onto a Horizon instance are ported automatically via `horizon-port-pack`).
- **Live game console & `options.txt` tuner.** Real-time stdout/stderr capture with level filtering,
  search and crash auto-open, plus per-instance `options.txt`, `servers.dat`, world datapack/backup
  tools, and Windows `.lnk` shortcuts.
- **Two-phase factory reset.** Hold `Ctrl + Shift + Alt` while clicking `Add Instance` to spawn a
  detached reset worker that cleanly wipes `%APPDATA%\FnH` after handles close and relaunches `FH.exe`
  as a fresh install.

Every launch step is recorded in `%APPDATA%\FnH\logs\fh.log`.

## Releases

`fh/VERSION` holds `1.6`, the release tag is `fnh-v1.6`, and the download archive is `fnh-1.6.zip`.
[fnh.is-cool.dev](https://fnh.is-cool.dev) and the Download link above always resolve to the newest
release.

## Licence

MIT — created by **Buffer**. FNH is not an official Minecraft product and is not approved by or
associated with Mojang or Microsoft. Horizon is made by the Horizon Team; FNH is a front end for
Fabric and Horizon styled after Buffer's JLac. The upstream Horizon workspace README is
[HORIZON.md](HORIZON.md).
