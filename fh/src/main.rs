//! FNH (Fabric n Horizon) v1.7 — multi-window Windows 11 launcher for Fabric, Minecraft: Java
//! Edition, and Horizon instances, modelled on Buffer's JLac (`QWindows11Style` / WinUI 3).
//!
//! Modules:
//! - `app.rs`          — Shared `Fh` state + `MainWindow` (`mainToolBar`, grouped `InstanceView`, `WideBar`, status bar)
//! - `win_instance.rs` — Standalone `InstanceWindow` (Log, Version, Mods, Packs, Shaders, Notes, Worlds, Servers, Screenshots, Settings, Other Logs)
//! - `win_settings.rs` — Standalone 9-page global `SettingsWindow` (Launcher, Appearance, Minecraft, Horizon, Java, Accounts, APIs, External Tools, Proxy)
//! - `win_dialogs.rs`  — Standalone `NewInstanceWindow`, `IconPickerWindow`, `ModBrowserWindow`, `UtilityWindow` & `CrashWindow`
//! - `content.rs`      — Isolated instance creation, Fabric/Horizon mod manager + auto-porter, Modrinth/Horizon browser, packs, notes, screenshots, shortcuts
//! - `hmods.rs`        — Upstream Horizon `v0.2` `.hmod` Content/WASM/Native subsystem, `index.json` verifier, GitHub mod installer & Native Mod SDK bridge
//! - `crash.rs`        — Fabric-Loader-style Crash & Mod Compatibility analyzer (pre-launch & post-exit diagnostics + 1-click Disable Mod)
//! - `updater.rs`      — Startup & manual GitHub Releases self-updater (`FnHLauncher.zip` download, extract, zip cleanup & live relaunch)
//! - `inst_icons.rs`   — 61 built-in 128x128 RGBA preset icons + custom icon importer & thumbnail loader
//! - `paths.rs`        — Isolated `<FnH>` data root (`%APPDATA%\FnH` / portable) so FNH never collides with other launchers
//! - `prefs.rs`        — Global `Prefs`, per-instance `InstanceCfgStore`, and `options.txt` `GameOptions`
//! - `reset.rs`        — Detached two-phase `--factory-reset` worker (`Ctrl + Shift + Alt` + `Add Instance`)
//! - `job.rs`          — Separated Fabric/Java & isolated Horizon launcher, live console ring buffers, Java/Horizon installers & MSA sign-in
//! - `look.rs`         — WinUI 3 Dark / OLED / Light / System themes + 8 accents, `fh.ico` `WM_SETICON` & shared controls
//! - `icons.rs`        — 49 embedded 24x24 Fluent UI SVGs (`gpui::AssetSource`)
//! - `log.rs`          — Rotating `<FnH>/logs/fh.log`, browser, Explorer, editor & world-tool helpers

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod content;
mod crash;
mod hmods;
mod icons;
mod inst_icons;
mod job;
mod log;
mod look;
mod paths;
mod prefs;
mod reset;
mod updater;
mod win_dialogs;
mod win_instance;
mod win_settings;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};
use std::path::PathBuf;

/// Release version from `fh/VERSION` via `build.rs` (matches `fnh-v{VERSION}` and `FnHLauncher.zip`).
pub const VERSION: &str = env!("FH_VERSION");

const HELP: &str = "\
FNH (Fabric n Horizon) — by Buffer
Your Fabric, Minecraft: Java Edition, and Horizon instances in a native multi-window Windows 11 launcher.

Hold Ctrl + Shift + Alt while clicking \"Add Instance\" to wipe all FNH launcher data and relaunch fresh.

Usage: FH [options]

Options:
  --launch <id>          Launch the given instance immediately on startup
  --launch-offline <id>  Launch the given instance in offline mode on startup
  --version, -V          Print the version and stop
  --help, -h             Print this help and stop
";

fn main() {
    let mut after_reset = false;
    let mut run_worker = false;
    let mut updated_from: Option<String> = None;
    let mut apply_update_pid: Option<u32> = None;
    let mut apply_staged: Option<PathBuf> = None;
    let mut apply_target: Option<PathBuf> = None;
    let mut apply_from: String = String::new();
    let mut auto_launch: Option<(String, bool)> = None;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                say(HELP);
                return;
            }
            "--version" | "-V" => {
                say(&format!("FNH v{VERSION}"));
                return;
            }
            reset::WORKER_FLAG => {
                run_worker = true;
            }
            reset::AFTER_RESET_FLAG => {
                after_reset = true;
            }
            "--updated-from" => {
                if let Some(v) = args.next() {
                    updated_from = Some(v);
                }
            }
            "--apply-update" => {
                if let Some(p) = args.next().and_then(|s| s.parse::<u32>().ok()) {
                    apply_update_pid = Some(p);
                }
            }
            "--staged" => {
                if let Some(p) = args.next() {
                    apply_staged = Some(PathBuf::from(p));
                }
            }
            "--target" => {
                if let Some(p) = args.next() {
                    apply_target = Some(PathBuf::from(p));
                }
            }
            "--from" => {
                if let Some(v) = args.next() {
                    apply_from = v;
                }
            }
            "--launch" => {
                if let Some(id) = args.next() {
                    auto_launch = Some((id, false));
                }
            }
            "--launch-offline" => {
                if let Some(id) = args.next() {
                    auto_launch = Some((id, true));
                }
            }
            other => {
                say(&format!("{other} is not an option of this program."));
                say("See FH.exe --help.");
                std::process::exit(2);
            }
        }
    }

    if let (Some(pid), Some(staged), Some(target)) = (apply_update_pid, apply_staged, apply_target)
    {
        std::process::exit(updater::run_apply_update_worker(
            pid,
            &staged,
            &target,
            &apply_from,
        ));
    }

    paths::init();

    if run_worker {
        std::process::exit(reset::run_worker());
    }

    std::process::exit(run(after_reset, updated_from, auto_launch));
}

fn say(text: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn run(
    after_reset: bool,
    updated_from: Option<String>,
    auto_launch: Option<(String, bool)>,
) -> i32 {
    reset::sweep_stale_stages(&paths::root());
    updater::cleanup_stale_update_files();

    let problem = horizon_launcher::data_location::startup();
    log::start(paths::logs_dir().join("fh.log"));
    log::line(&format!(
        "FNH v{VERSION} (root: {}){}.",
        paths::root().display(),
        if after_reset {
            " [fresh start after factory reset]"
        } else if updated_from.is_some() {
            " [restarted after self-update]"
        } else {
            ""
        }
    ));
    if let Some(problem) = problem {
        log::line(&problem);
    }

    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(move |cx: &mut App| {
            let shared = cx.new(|_| app::Fh::new(after_reset, updated_from.clone()));
            let bounds = Bounds::centered(None, size(px(980.), px(680.)), cx);

            let shared_for_main = shared.clone();
            let main_handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(740.), px(480.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some("FnH".into()),
                            ..Default::default()
                        }),
                        app_id: Some("fnh".into()),
                        ..Default::default()
                    },
                    move |window, cx| {
                        cx.new(|cx| app::MainWindow::new(shared_for_main, window, cx))
                    },
                )
                .expect("FNH main window");

            let main_id = main_handle.window_id();
            cx.on_window_closed(move |cx, closed_id| {
                if closed_id == main_id || cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .detach();

            if let Some((inst_id, offline)) = auto_launch.clone() {
                shared.update(cx, |fh, cx| {
                    fh.launch_instance(
                        &inst_id,
                        job::LaunchExtras {
                            offline,
                            ..job::LaunchExtras::default()
                        },
                        cx,
                    );
                });
            }

            cx.activate(true);
        });
    0
}

#[cfg(test)]
mod tests {
    use super::VERSION;

    /// `fh/VERSION` and `fh/Cargo.toml` must agree on the release number (`1.7.1` or `1.7` vs `1.7.0`).
    #[test]
    fn the_version_is_the_one_the_release_is_tagged_with() {
        assert_eq!(VERSION, VERSION.trim(), "VERSION has no padding");
        let pkg = env!("CARGO_PKG_VERSION");
        assert!(
            pkg == VERSION || pkg.starts_with(&format!("{VERSION}.")),
            "Cargo.toml's {pkg} and VERSION's {VERSION} are the same release"
        );
    }

    #[test]
    fn horizon_release_parser_resolves_stable_and_snapshot_channels() {
        let sample = serde_json::json!([
            {
                "tag_name": "launcher@0.3.19",
                "name": "Horizon Launcher 0.3.19",
                "draft": false,
                "prerelease": false,
                "assets": [
                    {
                        "name": "horizon-launcher-windows-x64.zip",
                        "size": 34057547,
                        "browser_download_url": "https://github.com/eltanschauung/horizon/releases/download/launcher@0.3.19/horizon-launcher-windows-x64.zip"
                    }
                ]
            },
            {
                "tag_name": "snapshot-2026-11-26-fd841ab",
                "name": "Snapshot 2026-11-26 · fd841ab",
                "draft": false,
                "prerelease": true,
                "assets": [
                    {
                        "name": "horizon-windows-x64.zip",
                        "size": 135020222,
                        "browser_download_url": "https://github.com/eltanschauung/horizon/releases/download/snapshot-2026-11-26-fd841ab/horizon-windows-x64.zip"
                    },
                    {
                        "name": "horizon-linux-x86_64.tar.gz",
                        "size": 133348997,
                        "browser_download_url": "https://github.com/eltanschauung/horizon/releases/download/snapshot-2026-11-26-fd841ab/horizon-linux-x86_64.tar.gz"
                    },
                    {
                        "name": "SHA256SUMS",
                        "size": 259,
                        "browser_download_url": "https://github.com/eltanschauung/horizon/releases/download/snapshot-2026-11-26-fd841ab/SHA256SUMS"
                    }
                ]
            },
            {
                "tag_name": "v0.3.5",
                "name": "Horizon 0.3.5",
                "draft": false,
                "prerelease": false,
                "assets": [
                    {
                        "name": "horizon-windows-x64.zip",
                        "size": 143849419,
                        "browser_download_url": "https://github.com/eltanschauung/horizon/releases/download/v0.3.5/horizon-windows-x64.zip"
                    },
                    {
                        "name": "horizon-linux-x86_64.tar.gz",
                        "size": 95238123,
                        "browser_download_url": "https://github.com/eltanschauung/horizon/releases/download/v0.3.5/horizon-linux-x86_64.tar.gz"
                    },
                    {
                        "name": "SHA256SUMS",
                        "size": 475,
                        "browser_download_url": "https://github.com/eltanschauung/horizon/releases/download/v0.3.5/SHA256SUMS"
                    }
                ]
            }
        ]);
        let parsed = job::parse_horizon_release_array_for_test(&sample);
        assert_eq!(parsed.len(), 2, "launcher@0.3.19 must be ignored, leaving v0.3.5 and snapshot");
        assert_eq!(
            job::pick_horizon_tag_for_test(&parsed, false).as_deref(),
            Some("v0.3.5")
        );
        assert_eq!(
            job::pick_horizon_tag_for_test(&parsed, true).as_deref(),
            Some("snapshot-2026-11-26-fd841ab")
        );
    }
}
