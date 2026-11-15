# NoLauncherHorizon

<p align="center"><img src="icons/nlh-256.png" width="96" alt="NoLauncherHorizon's icon: Horizon's horizon line with a play mark on it"></p>

**[Download NLH.zip](https://github.com/bufferclick/NoLauncherHorizon/releases)** — unzip it and
double-click `NoLauncherHorizon.exe` (Windows x64).

Horizon with the launcher stripped out. One `.exe`, double-clicked:

1. your browser opens on Microsoft's sign-in page with the code already filled in, and the tab the
   program opened behind it shows the code, a Copy button and a QR code for a phone;
2. you sign in — a Microsoft account that owns Minecraft: Java Edition, as any launcher needs;
3. the page turns into the progress: the newest Horizon build is downloaded if this computer has
   no copy of it, and the game's files are prepared from your own Minecraft exactly as the
   launcher's Play does (Mojang's own files are only downloaded when no install has them);
4. Horizon starts, and the window you were looking at closes itself. Nothing to click.

There is no launcher window, no settings screen, no version list and no mod browser: the page in
your browser is the program's only screen, and a start on a computer that is already signed in
and prepared opens no browser at all — it goes straight into the game.

It is not a second launcher: it is `horizon-launcher`'s own library (`account`, `backend`,
`instances`, `launch`) with the window replaced by the order below. Nothing is sent anywhere (unlike
the launcher, it does not even ping the play counter).

**The sign-in needs no registration from you, or from this fork's maintainers.** This build asks
Microsoft for the code as Microsoft's own Xbox Live client does — the public client the official
Minecraft Launcher signs in with, on `login.live.com` (`account::XBOX_CLIENT_ID`, and the
`xbox-sign-in` feature in `launcher/Cargo.toml`) — so "has a Microsoft account that owns Minecraft"
is the whole requirement. Everything after the code is the launcher's own chain. If you would
rather sign in with an Azure app of your own (the route `docs/microsoft-account.md` describes, and
the one Mojang approves apps for), set `HORIZON_MSA_CLIENT_ID` to its client ID before starting:
the variable wins over the built-in client, with no rebuild.

## What it shares, and where it keeps things

The data folder is the launcher's own — on Windows `%APPDATA%\horizon-launcher`, overridden by
`HORIZON_LAUNCHER_HOME` — so builds, worlds, instances, options and the sign-in are the same ones
a launcher installed beside it uses, and Minecraft files are taken from the same places
(`nolauncher/` inside it holds only this program's own two files):

| | |
|---|---|
| `nolauncher/nolauncher.log` | every start's lines, with the second they came in at: the only place a failure is explained in full |
| `nolauncher/nolauncher.html` | the page the browser shows while it works |

The instance the launcher has selected is the one that plays (its folder, its `pre-launch` and
wrapper). A Java instance is not this program's: Horizon's own instance starts instead.

## Options

```
NoLauncherHorizon [options] [-- <game arguments>]

  --tag TAG        Play this build, installing it if this computer has no copy (latest: newest)
  --no-update      Play what is installed; do not look for a newer build
  --version, -V    Print the version and stop
  --help, -h       Print this and stop
  --               Everything after this goes to the game
```

An argument it does not know is the game's, so `NoLauncherHorizon --world MyWorld` and
`NoLauncherHorizon -- --server example.org` both work.

## Versions

`nolauncher/VERSION` is the one place a version lives (`build.rs` puts it in the .exe and in
`--version`, `nolauncher/Cargo.toml` must agree — a test checks it), and the release workflow tags
the build from it: `nlh-v1.0`, `nlh-v1.1`, …

One tenth per update: **v1** (v1.0) first, then v1.1, v1.2 … v1.9, then **v1.10**, never v2.0.
Later a build worth naming after the Minecraft version it lands with may take those numbers
(v1.12.2, v1.8.9), which read the same way as the tenths.

## Building it

```
cd nolauncher
cargo build --release          # target/release/NoLauncherHorizon.exe
cargo test --release           # the command line, the page, the paths, the version
python3 icons/make_icon.py     # icons/nlh.ico from icons/nlh.svg (--png 256 to look at it)
```

`.github/workflows/nolauncher.yml` builds it on Windows, checks `--version`, wraps the .exe and
`pack/README.txt` into `NLH.zip`, uploads it as an artifact and — on `main` — publishes it as the
release `nlh-v<VERSION>` (never marked latest: the game's own releases stay the repository's
latest). Unzipping `NLH.zip` gives the .exe to double-click.

`nolauncher/` is its own Cargo workspace so the launcher's workspace and lock file are untouched;
it repeats the launcher's `[patch.crates-io]` entries for wgpu-hal, naga and libmimalloc-sys,
because it links the launcher's GPUI code and those copies are the ones Horizon ships with.

MIT, like Horizon. Not an official Minecraft product; not approved by or associated with Mojang or
Microsoft.
