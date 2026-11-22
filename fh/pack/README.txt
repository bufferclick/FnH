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
  - Every Java, Fabric, and Horizon 26.3 instance automatically receives `FnH-Dark-Glass.zip`
    (universal Dark-Glass UI, HUD & containers), while Fabric 1.21.11 and Fabric 26.x instances also
    receive the matching `fnh-menu` dynamic UI mod (animated Milky Way sky, live Microsoft Account 3D
    skin, directional motion-blur menus & smooth hotbar glide; toggleable in Settings -> Minecraft).
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
