# FNH — Fabric n Horizon

<p align="center"><img src="icons/fh-256.png" width="88" alt="FNH's icon: a white F and H on a black rounded tile"></p>

**[Download fnh-1.4.zip](https://github.com/bufferclick/fnh/releases/latest)** — unzip it and
double-click `FH.exe` (Windows 10/11, x64). **Website: [fnh.is-cool.dev](https://fnh.is-cool.dev).**

Your Horizon and Minecraft: Java Edition instances, one window. It is a native application, not a
page in a browser: it draws its own window with the toolkit the Horizon launcher is built on, and
every control in it is JLac's — the Prism Launcher fork this product is modelled on — down to its
palette, its 30-pixel controls, its 3-pixel corners and its lime highlight with black text on it.

```
┌──────────────────────────────────────────────────────────────────────────────────┐
│ ▣  My Horizon          Minecraft 1.21.4 · Fabric 0.16.9   bufferclick  ▾  ▶ Play │
├────────────────────┬─────────────────────────────────────────────────────────────┤
│ INSTANCES      3   │ Overview   Mods   Worlds   Logs                             │
│ ┌────────────────┐ │                                                             │
│ │ M  My Horizon  │ │ My Horizon                                                  │
│ │ H  Horizon     │ │ Minecraft 1.21.4 · Fabric 0.16.9                            │
│ │ V  Vanilla 1.21│ │ Plays          Minecraft 1.21.4 · Fabric 0.16.9             │
│ └────────────────┘ │ Build          Fabric 0.16.9 (managed)                      │
│ +  New instance    │ Last played    3 h 12 min played                            │
├────────────────────┴─────────────────────────────────────────────────────────────┤
│ ● Ready · 3 instances · FNH v1.4                                                 │
└──────────────────────────────────────────────────────────────────────────────────┘
```

## What it does

1. **Play.** The chosen instance starts. If this computer has no signed-in Microsoft account that
   owns Minecraft: Java Edition, the window shows the sign-in's code (in the window, on the
   clipboard, and Microsoft's own page opens once with it filled in). The build the instance plays
   is installed when there is no copy, its resources are prepared from your own Minecraft exactly
   as the launcher's Play does, and the game starts **with its session file**, so joining a server
   does not ask you to sign in again.
2. **New instance.** A Horizon instance of its own, or a Minecraft: Java Edition one — vanilla, or
   with **Fabric**. `instances::create_java` makes it and `loaders::choose` gives it Fabric's
   loader for that Minecraft version.
3. **Overview** — what the instance plays, its build, when it was last played, its game folder,
   and the button that opens it in Explorer.
4. **Mods** — every `.jar` in the instance's `mods/`, by name and size, and a button that opens
   the folder; modpacks put their jars there and FNH lists them.
5. **Worlds** — the instance's `saves/`: name, mode, the version that saved it, its size and when
   it was last opened, read through the launcher's own `worlds` module.
6. **Logs** — the end of the instance's `logs/latest.log`, and its folder.
7. **Accounts** — every Microsoft account on this computer, the one Play uses, and a sign-in that
   can be started from the window at any time.

Nothing is downloaded behind your back and nothing is written to an instance that the launcher
would not write itself: the work is `horizon-launcher`'s own code, called in the order its Play
calls it, and a log line is written for every step.

## Why it looks like this

The window is GPUI — the same `gpui-fast` the Horizon launcher draws with — and `look.rs` is JLac's
palette written out as constants (`DarkTheme.cpp`: window `#313131`, base `#222222`, alternate
`#2a2a2a`, buttons `#303030`, rules `#1b1b1b`, borders `#3c3c3c`, ink `#ffffff`, muted `#a6a6a6`,
the highlight `#96db59` with black text on it, links `#2fa3c6`). `app.rs` is the window: a
76-pixel toolbar, a 260-pixel instance pane, a 40-pixel page strip with the accent underline under
the open page, a 30-pixel status bar, and dialogs on a dimmed backdrop — JLac's own layout, not an
imitation of it.

There is no webview, no embedded browser and no HTTP server: FNH is one `.exe` with one window.

## What it shares, and where it keeps things

The data folder is the launcher's own — on Windows `%APPDATA%\horizon-launcher`, overridden by
`HORIZON_LAUNCHER_HOME` — so instances, builds, worlds, options and the sign-in are the same ones
the launcher uses. `fh/fh.log` in it is this window's log: a double-clicked `.exe` has no console,
so every step is written there, including the game's command line.

## Building it

```sh
cd fh
cargo build --release      # FH.exe
cargo test                 # the look, the log, the launch, and the window's helpers
```

Windows x64 only, which is the only platform this product exists for. `.github/workflows/fh.yml`
builds and tests it on a Windows runner, publishes `fnh-<version>.zip` as the release
`fnh-v<version>`, and checks that the `.exe` is a GUI-subsystem image linking only system libraries.

## Versioning and the zip

`fh/VERSION` holds the version — 1.4, and one tenth per update (1.5 … 1.9, then 1.10). It is the
release tag (`fnh-v1.4`), the name of the download (`fnh-1.4.zip`), the `.exe`'s version resource,
and `FH.exe --version` (`FNH v1.4`); a test checks that `Cargo.toml` agrees with it.

The website at [fnh.is-cool.dev](https://fnh.is-cool.dev) is the static site in [`site/`](../site),
and its Download button asks GitHub for the newest release's `fnh-*.zip` — so a release is all a
publish needs.

## Licence

MIT. Not an official Minecraft product; not approved by or associated with Mojang or Microsoft.
Horizon is made by the Horizon Team; FNH is a front end for it, styled after JLac.
