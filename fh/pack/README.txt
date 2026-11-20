FNH v@VERSION@ — by Buffer
==========================

Fabric n Horizon — your Fabric, Minecraft: Java Edition, and Horizon instances in a native
multi-window Windows 11 launcher modelled on JLac.

Unzip this folder anywhere you can write to and double-click FH.exe. No installer, no Java setup
up front, and no background service are required.

Quick guide:

  - Click "Add Instance" (or press Ctrl+N) to create an isolated Fabric, Vanilla, or Horizon instance.
  - Select an instance and click "Launch" (or "Launch Offline"), and "Kill" to stop a running game.
  - Click "Edit" to open a dedicated Instance Window with 11 pages: Minecraft Log, Version, Mods
    (with Modrinth + Horizon Registry browser & drag-and-drop), Resource Packs, Shader Packs, Notes,
    Worlds (datapacks, .zip backup, duplicate, external tool), Servers (servers.dat), Screenshots,
    Settings (including an options.txt tuner), and Other Logs.
  - Click the instance icon in the sidebar or New Instance dialog to pick from 61 built-in preset
    icons or import your own image.
  - Click "Settings" (Ctrl+,) for 9 global settings pages: Launcher, Appearance (Dark, OLED Dark,
    Light, System + 8 accents), Minecraft, Horizon, Java (one-click Java 21/17/8 download),
    Accounts, APIs, External Tools, and Proxy.
  - Hold Ctrl + Shift + Alt while clicking "Add Instance" to wipe all FNH data and relaunch fresh.

Where files live on Windows:

  %APPDATA%\FnH\fh\settings.json      global launcher settings
  %APPDATA%\FnH\fh\instance_cfg.json  per-instance settings & icon assignments
  %APPDATA%\FnH\logs\fh.log           launcher log
  %APPDATA%\FnH\game\instances\       isolated per-instance game folders, mods, worlds & screenshots

Website:  https://fnh.is-cool.dev
Source:   https://github.com/bufferclick/fnh
Options:  FH.exe --launch <id> | --launch-offline <id> | --version | --help

MIT licence — created by Buffer. Not an official Minecraft product; not approved by or associated
with Mojang or Microsoft.
