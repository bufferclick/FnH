FNH v@VERSION@ — by Buffer
==========================

Fabric n Horizon — your Fabric, Minecraft: Java Edition, and Horizon instances in a native
multi-window Windows 11 launcher modelled on JLac.

Unzip FnHLauncher.zip anywhere you can write to and double-click FH.exe. No installer, no Java setup
up front, and no background service are required. FNH checks for new releases automatically on
startup and can update itself in-place.

Quick guide:

  - Click "Add Instance" (or press Ctrl+N) to create an isolated Fabric, Vanilla, or Horizon instance.
  - Select an instance and click "Launch" (or "Launch Offline"), and "Kill" to stop a running game.
  - Click "Edit" to open a dedicated Instance Window with 11 pages: Minecraft Log, Version, Mods
    (Horizon Registry v0.2 .hmod Content/WASM/Native SDK, GitHub .hmod install, Modrinth + auto-port,
    and drag-and-drop), Resource Packs, Shader Packs, Notes, Worlds, Servers, Screenshots, Settings
    (including an options.txt tuner), and Other Logs.
  - If a mod is incompatible or crashes Minecraft/Horizon, FNH opens a Fabric-style Crash & Mod
    Compatibility window with potential solutions and 1-click "Disable Mod".
  - Fabric 1.21.11 instances automatically receive `fnh-menu-1.8.0.jar` (FnH's Dark Glass Minecraft UI,
    animated Milky Way background & 3D walk-in skin; toggleable in Settings -> Minecraft).
  - Click "Update" in the top toolbar at any time to check for or install the latest FnHLauncher.zip.
  - Hold Ctrl + Shift + Alt while clicking "Add Instance" to wipe all FNH data and relaunch fresh.

Where files live on Windows:

  %APPDATA%\FnH\fh\settings.json      global launcher settings
  %APPDATA%\FnH\fh\instance_cfg.json  per-instance settings & icon assignments
  %APPDATA%\FnH\logs\fh.log           launcher log
  %APPDATA%\FnH\game\instances\       isolated per-instance game folders, mods, worlds & screenshots

Website:  https://fnh.is-cool.dev
Source:   https://github.com/bufferclick/FnH
Options:  FH.exe --launch <id> | --launch-offline <id> | --version | --help

MIT licence — created by Buffer. Not an official Minecraft product; not approved by or associated
with Mojang or Microsoft.
