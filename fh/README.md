# FNH — Fabric n Horizon

<p align="center"><img src="icons/fh-256.png" width="88" height="88" alt="FNH's icon: a deep-navy F and H with a black n on a light rounded tile"></p>

**[Download fnh-1.5.zip](https://github.com/bufferclick/fnh/releases/latest)** — unzip it and
double-click `FH.exe` (Windows 10/11, x64). **Website: [fnh.is-cool.dev](https://fnh.is-cool.dev).**

Your Fabric, Minecraft: Java Edition and Horizon instances in one native Windows 11 window, modeled
directly on JLac's `windows11` (WinUI 3 / Qt `QWindows11Style`) interface.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ + Add Instance... │ 📁 Folders ▾  ⚙ Settings...  ? Help ▾  ↓ Update...  👤 Accounts ▾ │
├────────────────────────────────────────────────────────────────┬─────────────────┤
│ ▾ Fabric (2) ───────────────────────────────────────────────── │      ┌───┐      │
│   ┌────┐        ┌────┐                                         │      │ F │      │
│   │ 1  │        │ M  │                                         │      └───┘      │
│   └────┘        └────┘                                         │     1.21.4      │
│   1.21.4       Modded SMP                                      │ 1.21.4 · Fabric │
│                                                                │ ─────────────── │
│ ▾ Horizon (1) ──────────────────────────────────────────────── │  ▶ Launch       │
│   ┌────┐                                                       │  ✕ Kill         │
│   │ H  │                                                       │ ─────────────── │
│   └────┘                                                       │  ✎ Edit...      │
│   Horizon                                                      │  🏷 Change Group │
│                                                                │  📁 Folder      │
│                                                                │  ⧉ Copy...      │
│                                                                │  🗑 Delete       │
├────────────────────────────────────────────────────────────────┴─────────────────┤
│ Minecraft 1.21.4 (Fabric 0.16.10), played for 3h 12m     │ Total playtime: 3h 12m│
└──────────────────────────────────────────────────────────────────────────────────┘
```

## Features

1. **MainWindow (`app.rs`).** JLac's top toolbar (`Add Instance...`, `Folders`, `Settings...`,
   `Help`, `Update...`, and right-aligned `Accounts`), central grouped `InstanceView` (collapsible
   groups, single-click select, double-click launch, right-click context menu, `Ctrl+N`, `Ctrl+I`,
   `Ctrl+G`, `Ctrl+D`, `Ctrl+K`, `F2`, `F5`, `Del`, `Enter`), right-docked `WideBar`
   `instanceToolBar`, task progress banner, and bottom `QStatusBar` with total playtime.
2. **Windows 11 theme (`look.rs`, `icons.rs`, `prefs.rs`).** Authentic WinUI 3 Light (`#f3f3f3`
   Mica chrome, `#ffffff` view layer, `#0067c0` accent) and Dark (`#202020` Mica chrome, `#272727`
   view layer, `#60cdff` accent) surfaces, following Windows automatically (`Windows 11 (System)`)
   or selectable as `Windows 11 Light` / `Windows 11 Dark` alongside six Fluent accent swatches.
3. **Add Instance (`dialogs.rs`).** Icon swatch, `Name:` and `Group:` header, live Mojang version
   table with `Releases`, `Snapshots`, `Betas` and `Alphas` filters, one-click Fabric Loader or
   vanilla setup, and native Horizon instance creation.
4. **Edit Instance (`dialogs.rs`).** Sidebar `PageContainer` with **Minecraft Log**, **Version**
   (install, update, pick any Fabric Loader release from `meta.fabricmc.net`, or remove loader),
   **Mods** (enable/disable `.jar` ↔ `.jar.disabled`, remove, search, open folder), **Worlds**
   (game mode, version, size, folder), and **Settings** (memory, JVM args, window size, pre-launch
   and wrapper commands).
5. **Accounts & Sign-In (`job.rs`, `dialogs.rs`).** Multi-account Microsoft device-code sign-in,
   quick switcher menu in the top-right toolbar, and full `Accounts` manager dialog.
6. **Factory reset (`reset.rs`).** Hold `Ctrl + Shift + Alt` while clicking `Add Instance...` to
   kill any running instance, sign out of every stored account, delete all instances, worlds, mods,
   caches, UI preferences (`fh/ui.cfg`) and logs (`fh/fh.log`), and immediately relaunch `FH.exe`
   into a completely clean fresh-install state.

## Building

```sh
cd fh
cargo build --release      # builds FH.exe
cargo test                 # tests theme, icons, preferences, reset, logs and version sync
```

`.github/workflows/fh.yml` builds and tests `FH.exe` on Windows x64, verifies its GUI subsystem and
clean DLL imports, and publishes `fnh-<version>.zip` under release tag `fnh-v<version>`.

## Versioning

`fh/VERSION` holds the version (`1.5`), matching `fnh-v1.5`, `fnh-1.5.zip`, the `.exe` version
resource, `FH.exe --version` (`FNH v1.5`), and `fh/Cargo.toml` (`1.5.0`).

## Licence

MIT. Not an official Minecraft product; not approved by or associated with Mojang or Microsoft.
Horizon is made by the Horizon Team; FNH is a front end for it, styled after JLac.
