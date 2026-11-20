//! FNH (Fabric n Horizon) v1.6 — multi-window Windows 11 launcher for Fabric, Minecraft: Java
//! Edition, and Horizon instances, modelled on Buffer's JLac (`QWindows11Style` / WinUI 3).
//!
//! Modules:
//! - `app.rs`          — Shared `Fh` state + `MainWindow` (`mainToolBar`, grouped `InstanceView`, `WideBar`, status bar)
//! - `win_instance.rs` — Standalone `InstanceWindow` (Log, Version, Mods, Packs, Shaders, Notes, Worlds, Servers, Screenshots, Settings, Other Logs)
//! - `win_settings.rs` — Standalone 9-page global `SettingsWindow` (Launcher, Appearance, Minecraft, Horizon, Java, Accounts, APIs, External Tools, Proxy)
//! - `win_dialogs.rs`  — Standalone `NewInstanceWindow`, `IconPickerWindow`, `ModBrowserWindow` & `UtilityWindow`
//! - `content.rs`      — Isolated instance creation, Fabric/Horizon mod manager + auto-porter, Modrinth/Horizon browser, packs, notes, screenshots, shortcuts
//! - `inst_icons.rs`   — 61 built-in 128x128 RGBA preset icons + custom icon importer & thumbnail loader
//! - `paths.rs`        — Isolated `<FnH>` data root (`%APPDATA%\FnH` / portable) so FNH never collides with other launchers
//! - `prefs.rs`        — Global `Prefs`, per-instance `InstanceCfgStore`, and `options.txt` `GameOptions`
//! - `reset.rs`        — Detached two-phase `--factory-reset` worker (`Ctrl + Shift + Alt` + `Add Instance`)
//! - `job.rs`          — Separated Fabric/Java & isolated Horizon launcher, live console ring buffers, Java/Horizon installers & MSA sign-in
//! - `look.rs`         — WinUI 3 Dark / OLED / Light / System themes + 8 accents & shared controls
//! - `icons.rs`        — 42 embedded 24x24 Fluent UI SVGs (`gpui::AssetSource`)
//! - `log.rs`          — Rotating `<FnH>/logs/fh.log`, browser, Explorer, editor & world-tool helpers

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod content;
mod icons;
mod inst_icons;
mod job;
mod log;
mod look;
mod paths;
mod prefs;
mod reset;
mod win_dialogs;
mod win_instance;
mod win_settings;

use gpui::{App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions, px, size};

/// Release version from `fh/VERSION` via `build.rs` (matches `fnh-v{VERSION}` and `fnh-{VERSION}.zip`).
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

    paths::init();

    if run_worker {
        std::process::exit(reset::run_worker());
    }

    std::process::exit(run(after_reset, auto_launch));
}

fn say(text: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn run(after_reset: bool, auto_launch: Option<(String, bool)>) -> i32 {
    reset::sweep_stale_stages(&paths::root());

    let problem = horizon_launcher::data_location::startup();
    log::start(paths::logs_dir().join("fh.log"));
    log::line(&format!(
        "FNH v{VERSION} (root: {}){}.",
        paths::root().display(),
        if after_reset {
            " [fresh start after factory reset]"
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
            let shared = cx.new(|_| app::Fh::new(after_reset));
            let bounds = Bounds::centered(None, size(px(980.), px(660.)), cx);
            let title = format!("FNH {VERSION}");

            let shared_for_main = shared.clone();
            let main_handle = cx
                .open_window(
                    WindowOptions {
                        window_bounds: Some(WindowBounds::Windowed(bounds)),
                        window_min_size: Some(size(px(740.), px(480.))),
                        titlebar: Some(TitlebarOptions {
                            title: Some(title.into()),
                            ..Default::default()
                        }),
                        app_id: Some("fnh".into()),
                        ..Default::default()
                    },
                    move |window, cx| cx.new(|cx| app::MainWindow::new(shared_for_main, window, cx)),
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

    /// `fh/VERSION` and `fh/Cargo.toml` must agree on the release number (`1.6` vs `1.6.0`).
    #[test]
    fn the_version_is_the_one_the_release_is_tagged_with() {
        assert_eq!(VERSION, VERSION.trim(), "VERSION has no padding");
        assert!(
            env!("CARGO_PKG_VERSION").starts_with(&format!("{VERSION}.")),
            "Cargo.toml's {} and VERSION's {} are the same release",
            env!("CARGO_PKG_VERSION"),
            VERSION
        );
    }
}
