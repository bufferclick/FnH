# FNH — Fabric n Horizon (`v1.6`)

<p align="center"><img src="icons/fh-256.png" width="88" height="88" alt="FNH's icon: a deep-navy F and H with a black n on a light rounded tile"></p>

**[Download fnh-1.6.zip](https://github.com/bufferclick/fnh/releases/latest)** — unzip it and
double-click `FH.exe` (Windows 10/11, x64). **Website: [fnh.is-cool.dev](https://fnh.is-cool.dev).**

Created by **Buffer** — your Fabric, Minecraft: Java Edition, and Horizon instances in a native
multi-window Windows 11 launcher modelled directly on **JLac**'s `windows11` (WinUI 3 /
`QWindows11Style`) interface.

## Highlights in v1.6

1. **Multi-window desktop shell (`app.rs`, `win_instance.rs`, `win_settings.rs`, `win_dialogs.rs`).**
   - `MainWindow` (`"FNH 1.6"`): top toolbar (`Add Instance`, `Folders`, `Settings`, `View`, `Help`,
     `Accounts`), collapsible grouped `InstanceView` (Grid & List views, 32–128 px icons, drag-drop
     mods/icons onto tiles), right `WideBar` (`Launch`, `Launch Offline`, `Kill`, `Edit`,
     `Minecraft Log`, `Mods`, `Worlds`, `Instance Settings`, `Change Group`, `Rename`, `Folder`,
     `Copy Instance`, `Create Shortcut`, `Delete`), and bottom status bar with total playtime.
   - `InstanceWindow` (`"Edit Instance (<Name>) - FNH"`): 11 JLac pages — **Minecraft Log**,
     **Version**, **Mods**, **Resource Packs**, **Shader Packs**, **Notes**, **Worlds**, **Servers**,
     **Screenshots**, **Settings** (General, Java / Horizon, Game Options `options.txt`, Custom
     Commands, Environment, Tweaks), and **Other Logs**.
   - `SettingsWindow` (`"Settings - FNH"`): 9 global pages — **Launcher**, **Appearance**
     (Dark / OLED Dark / Light / System + 8 WinUI accents), **Minecraft**, **Horizon**, **Java**
     (Java 21/17/8 installer & tester), **Accounts**, **APIs**, **External Tools**, and **Proxy**.
   - `IconPickerWindow` (61 built-in 128×128 RGBA preset icons + custom PNG/JPG/WebP/ICO import),
     `NewInstanceWindow`, `ModBrowserWindow` (Modrinth + Horizon Registry), and `UtilityWindow`.
2. **Separated Fabric/Java & isolated Horizon launch (`job.rs`, `paths.rs`, `content.rs`).**
   - Dedicated `%APPDATA%\FnH` root so FNH never collides with other launchers.
   - Clean `build_java_command` vs `build_horizon_command` separation with live stdout/stderr ring
     buffers, pre-launch / wrapper / post-exit hooks, and automatic Mojang asset fallback.
   - Drag-and-drop or browse mods across Modrinth and Horizon Registry, with automatic Fabric `.jar`
     porting on Horizon instances via `horizon-port-pack`.
3. **Detached two-phase factory reset (`reset.rs`).**
   - Hold `Ctrl + Shift + Alt` while clicking `Add Instance` to spawn `FH.exe --factory-reset`,
     release locks/handles cleanly, stage-wipe `%APPDATA%\FnH`, and relaunch `FH.exe --after-reset`.

## Building

```sh
cd fh
cargo build --release      # builds FH.exe
cargo test --release       # runs unit tests across icons, paths, prefs, content, reset, logs & version sync
```

`.github/workflows/fh.yml` builds and tests `FH.exe` on Windows x64, verifies its GUI subsystem and
clean DLL imports, and publishes `fnh-<version>.zip` under release tag `fnh-v<version>`.

## Versioning

`fh/VERSION` holds the version (`1.6`), matching `fnh-v1.6`, `fnh-1.6.zip`, the `.exe` version
resource, `FH.exe --version` (`FNH v1.6`), and `fh/Cargo.toml` (`1.6.0`).

## Licence

MIT — by **Buffer**. Not an official Minecraft product; not approved by or associated with Mojang or
Microsoft. Horizon is made by the Horizon Team; FNH is styled after Buffer's JLac.
