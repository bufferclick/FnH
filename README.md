<p align="center"><img src="fh/icons/fh-256.png" width="88" height="88" alt="FNH's icon: a deep-navy F and H with a black n on a light rounded tile"></p>

<h1 align="center">FNH</h1>

<p align="center">
  Fabric n Horizon — a Windows app for your Horizon and Minecraft: Java Edition instances.<br>
  <a href="https://fnh.is-cool.dev"><b>fnh.is-cool.dev</b></a> ·
  <a href="https://github.com/bufferclick/fnh/releases/latest"><b>Download</b></a> ·
  <a href="fh/README.md"><b>Documentation</b></a>
</p>

---

FNH is a desktop application for Windows 10 and 11 (x64). It lists the instances the Horizon
launcher has and starts one, and it makes new ones: Horizon instances of their own, or Minecraft:
Java Edition instances, vanilla or with Fabric. It is a single `.exe` — no installer, no browser,
no background service.

It uses the Horizon launcher's own data folder and its own code, so instances, builds, worlds,
options and the Microsoft sign-in are the same ones the launcher uses, and the two can be installed
side by side.

## Install

1. Download `fnh-1.4.zip` from [Releases](https://github.com/bufferclick/fnh/releases/latest).
2. Unzip it anywhere you can write to — `%LOCALAPPDATA%\FNH`, a folder on your desktop, a USB
   stick. There is nothing to run first.
3. Double-click `FH.exe`.

Windows 10 or 11 on x64. Nothing else is needed: no .NET, no Visual C++ redistributable, no Java
until a Minecraft: Java Edition instance needs it (FNH picks one and offers to fetch it).

## What it does

- **Instance list.** Every instance the Horizon launcher has, with its icon, what it plays and the
  build it is kept on. The one selected is the one Play starts.
- **Overview, Mods, Worlds, Logs.** For the chosen instance: what it plays and where its folder is;
  the jars in `mods/`; its saves with mode, version, size and last played; and the end of
  `logs/latest.log`. Each page opens its folder in Explorer.
- **New instance.** A Horizon instance of its own, or a Minecraft: Java Edition one — vanilla, or
  with Fabric. Its loader version is chosen when it first starts.
- **Microsoft sign-in.** The device-code flow, in the window: the code goes on the clipboard and
  Microsoft's page opens with it filled in. No app registration, no client secret, nothing to
  configure.
- **Play.** Installs the build if this computer has none, prepares its files from your own
  Minecraft install as the launcher does, and starts the game with a session file, so joining a
  server does not ask you to sign in again.

What happened, step by step, is written to `%APPDATA%\horizon-launcher\fh\fh.log`.

## How it works

FNH is a Rust program. Its window is [GPUI](https://gpui.rs) — the same toolkit the Horizon
launcher draws with — and its appearance follows **JLac**, the Prism Launcher fork this product is
modelled on: a 76-pixel toolbar, a 260-pixel instance pane, a page strip with an accent underline,
a status bar, and JLac's dark palette (`#313131` window, `#222222` base, `#96db59` highlight).
Dialogs open over a dimmed backdrop.

The launching is the launcher's own library (`horizon-launcher`): `account` for the sign-in and the
read-once session file, `backend` for the installs, the preparation and the game's command,
`instances` for the folders, `launch` for the wrappers. FNH adds the order, the window and the log.
More about the internals and the versioning: [fh/README.md](fh/README.md).

## Releases

FNH is versioned in tenths: `fh/VERSION` holds `1.4`, the release is tagged `fnh-v1.4`, and the
download is `fnh-1.4.zip`. `fnh.is-cool.dev` and the Download link above always point at the latest
release.

## Licence

MIT. FNH is not an official Minecraft product and is not approved by or associated with Mojang or
Microsoft. Horizon is made by the [Horizon Team](https://github.com/horizon-rust/horizon); FNH is a
front end for it, styled after JLac. The launcher's own README, unchanged, is [HORIZON.md](HORIZON.md).
