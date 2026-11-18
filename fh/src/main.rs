//! FNH (Fabric n Horizon): your Fabric, Minecraft: Java Edition and Horizon instances in one
//! native Windows 11 window, modeled directly on JLac's `windows11` (WinUI 3 / `QWindows11Style`)
//! interface.
//!
//! Modules:
//! - `app.rs`     — `MainWindow` (top toolbar, grouped `InstanceView`, right `WideBar`, status bar)
//! - `dialogs.rs` — WinUI 3 dialogs (`Add Instance`, `Edit Instance`, `Settings`, `Accounts`, etc.)
//! - `look.rs`    — Windows 11 Light/Dark theme tokens, Fluent controls and FNH brand mark
//! - `icons.rs`   — Embedded 24x24 Fluent/JLac SVG icons (`gpui::AssetSource`)
//! - `prefs.rs`   — UI preferences (`fh/ui.cfg`: Windows 11 theme mode, accent, sorting, groups)
//! - `reset.rs`   — `Ctrl + Shift + Alt` + `Add Instance...` factory reset and clean relaunch
//! - `job.rs`     — Background instance launching, process termination (`Kill`) and Microsoft login
//! - `log.rs`     — Rotating `fh/fh.log`, browser, Explorer and clipboard helpers

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod dialogs;
mod icons;
mod job;
mod log;
mod look;
mod prefs;
mod reset;

use gpui::{px, size, App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};

/// Release version from `fh/VERSION` via `build.rs` (matches `fnh-v{VERSION}` and `fnh-{VERSION}.zip`).
pub const VERSION: &str = env!("FH_VERSION");

const HELP: &str = "\
FNH (Fabric n Horizon): your Fabric and Horizon instances in one Windows 11 launcher.

Open FH.exe to manage and launch Fabric, Minecraft: Java Edition and Horizon instances from a
clean WinUI 3 interface modeled on JLac's Windows 11 theme: Add Instance, Folders, Settings, Help,
Update and Accounts across the top toolbar; grouped instance grid in the centre; Launch, Kill,
Edit (Minecraft Log, Version, Mods, Worlds, Settings), Change Group, Folder, Copy and Delete on
the right; and live status plus total playtime along the bottom.

Hold Ctrl + Shift + Alt while clicking \"Add Instance...\" to wipe all launcher data (instances,
accounts, saves, mods, caches, preferences and logs) and relaunch FNH as a fresh install.

Usage: FH [options]

Options:
  --version, -V    Print the version and stop
  --help, -h       Print this help and stop
";

fn main() {
    let mut after_reset = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" | "-h" => {
                say(HELP);
                return;
            }
            "--version" | "-V" => {
                say(&format!("FNH v{VERSION}"));
                return;
            }
            reset::AFTER_RESET_FLAG => {
                after_reset = true;
            }
            other => {
                say(&format!("{other} is not an option of this program."));
                say("See FH.exe --help.");
                std::process::exit(2);
            }
        }
    }
    std::process::exit(run(after_reset));
}

/// Writes to stdout when run from a terminal; harmless when double-clicked from Explorer.
fn say(text: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn run(after_reset: bool) -> i32 {
    if after_reset {
        // Brief pause so the exiting parent releases `location.lock` before the final sweep.
        std::thread::sleep(std::time::Duration::from_millis(120));
        reset::finish_post_reset_sweep();
    }

    let problem = horizon_launcher::data_location::startup();
    let dir = horizon_launcher::backend::data_dir().join("fh");
    log::start(dir.join("fh.log"));
    log::line(&format!(
        "FNH v{VERSION}{}.",
        if after_reset { " (fresh start after factory reset)" } else { "" }
    ));
    if let Some(problem) = problem {
        log::line(&problem);
    }

    horizon_launcher::proxy::announce(horizon_launcher::backend::Settings::load().proxy);

    gpui_platform::application()
        .with_assets(icons::Assets)
        .run(move |cx: &mut App| {
            let bounds = Bounds::centered(None, size(px(920.), px(620.)), cx);
            let title = format!("FNH {VERSION}");
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(720.), px(480.))),
                    titlebar: Some(TitlebarOptions {
                        title: Some(title.into()),
                        ..Default::default()
                    }),
                    app_id: Some("fnh".into()),
                    ..Default::default()
                },
                |window, cx| cx.new(|cx| app::Fh::new(window, cx)),
            )
            .expect("FNH main window");
            cx.on_window_closed(|cx, _| cx.quit()).detach();
            cx.activate(true);
        });
    0
}

#[cfg(test)]
mod tests {
    use super::VERSION;

    /// `fh/VERSION` and `fh/Cargo.toml` must agree on the release number (`1.5` vs `1.5.0`).
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
