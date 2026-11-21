# FNH — Fabric n Horizon (`v1.7`)

<p align="center"><img src="icons/fh-256.png" width="88" height="88" alt="FNH's icon: a deep-navy F and H with a black n on a light rounded tile"></p>

**[Download FnHLauncher.zip](https://github.com/bufferclick/FnH/releases/latest/download/FnHLauncher.zip)** — unzip it and
double-click `FH.exe` (Windows 10/11, x64). **Website: [fnh.is-cool.dev](https://fnh.is-cool.dev).**

Created by **Buffer** — your Fabric, Minecraft: Java Edition, and Horizon instances in a native
multi-window Windows 11 launcher modelled directly on **JLac**'s `windows11` (WinUI 3 /
`QWindows11Style`) interface.

## Highlights in v1.7

1. **Multi-window desktop shell & crisp 9×9 branding (`app.rs`, `win_instance.rs`, `win_settings.rs`, `win_dialogs.rs`, `look.rs`).**
   - `MainWindow` (`"FNH 1.7"`): crisp 9×9 `FnH` monogram, top toolbar (`Add Instance`, `Folders`,
     `Settings`, `View`, `Help`, `Update`, `Accounts`), collapsible grouped `InstanceView`, right
     `WideBar`, and bottom status bar with total playtime.
   - `InstanceWindow` (`"Edit Instance (<Name>) - FNH"`): 11 JLac pages — **Minecraft Log**,
     **Version**, **Mods**, **Resource Packs**, **Shader Packs**, **Notes**, **Worlds**, **Servers**,
     **Screenshots**, **Settings**, and **Other Logs**.
   - `SettingsWindow` (`"Settings - FNH"`): 9 global pages — **Launcher**, **Appearance**,
     **Minecraft**, **Horizon**, **Java**, **Accounts**, **APIs**, **External Tools**, and **Proxy**.
   - `CrashWindow`, `IconPickerWindow`, `NewInstanceWindow`, `ModBrowserWindow`, and `UtilityWindow`,
     all stamped with `fh.ico` via Win32 `WM_SETICON`.
2. **Upstream Horizon `v0.2` mod subsystem (`hmods.rs`, `content.rs`, `job.rs`).**
   - Verifies Horizon Registry `index.json` (`api = "0.2"`, `SHA-256`, `minisign`, `revoked` checks).
   - Routes Content & sandboxed WASM `.hmod` packages to `<home>/horizon-mods/<id>.hmod`, and Native
     Rust `.hmod` packages to `<home>/horizon-native-mods/<id>.hmod` (linked on launch via official
     `horizon-launcher native-build`), syncing `mod.<id>:true|false` in `<home>/horizon-mods.txt`.
   - Direct GitHub `.hmod` installation (`owner/repo` or URL) + Modrinth `.jar` auto-porting via
     `horizon-port-pack`.
3. **Fabric-style Crash & Mod Compatibility Window (`crash.rs`, `CrashWindow`).**
   - Pre-launch and post-exit detection of Fabric `Incompatible mod set!` / `FormattedException`,
     Forge/NeoForge `.jar` mismatches, missing `fabric-api` (with Jar-in-Jar awareness),
     `crash-reports/crash-*.txt`, and unsupported Horizon mods, with **1-click Disable Mod**.
4. **Startup Self-Updater (`updater.rs`).**
   - Checks `bufferclick/FnH` releases on startup and via `Update`, prompts *"Do you want to update
     FnH?"*, downloads `FnHLauncher.zip`, extracts `FH.exe`, deletes `FnHLauncher.zip`, swaps the
     executable, and restarts cleanly.

## Building

```sh
cd fh
cargo build --release      # builds FH.exe
cargo test --release       # runs unit tests across icons, hmods, crash, updater, paths, prefs, content & reset
```

`.github/workflows/fh.yml` builds and tests `FH.exe` on Windows x64, verifies its GUI subsystem and
clean DLL imports, and publishes `FnHLauncher.zip` under release tag `fnh-v<version>`.

## Versioning

`fh/VERSION` holds the version (`1.7`), matching `fnh-v1.7`, `FnHLauncher.zip`, the `.exe` version
resource, `FH.exe --version` (`FNH v1.7`), and `fh/Cargo.toml` (`1.7.0`).

## Licence

MIT — by **Buffer**. Not an official Minecraft product; not approved by or associated with Mojang or
Microsoft. Horizon is made by the Horizon Team; FNH is styled after Buffer's JLac.
