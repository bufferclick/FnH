//! Fabric n Horizon: your Horizon and Fabric instances in one native window, drawn the way JLac
//! draws its own — Prism Launcher's layout in JLac's dark theme, in a real window, with nothing in
//! a browser but the sign-in Microsoft makes you finish there.
//!
//! The window is `app.rs`; the launching is `job.rs` (the launcher's own library: `account` for
//! the Microsoft sign-in, `backend` for the builds, the preparation and the game's command,
//! `instances` and `launch` for the folders and wrappers); the look is `look.rs` (JLac's palette);
//! and `log.rs` is the log file a double-clicked .exe has instead of a terminal.
//!
//! Versioning (fh/VERSION, the tag the release workflow publishes): v1.3 first, then one tenth
//! per update.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod app;
mod job;
mod log;
mod look;

use gpui::{px, size, App, AppContext, Bounds, TitlebarOptions, WindowBounds, WindowOptions};

/// The version this build is: `fh/VERSION` through `build.rs`, the same string the release is
/// tagged with.
pub const VERSION: &str = env!("FH_VERSION");

const HELP: &str = "\
Fabric n Horizon: your Horizon and Fabric instances, one window.

Double-click it and the window opens on your instances. Pick one and press Play: it signs you in
with Microsoft if it has to (the code is in the window and on the clipboard), installs the build
the instance plays when it is missing, takes the game's files from your own Minecraft the way the
launcher does, and starts the game. New instance makes a Horizon one or a Minecraft: Java Edition
one — vanilla or with Fabric — and the Mods, Worlds and Logs pages show what each instance has.

Usage: FH [options]

Options:
  --version, -V    Print the version and stop
  --help, -h       Print this and stop

The data folder is the launcher's own (HORIZON_LAUNCHER_HOME overrides it), so the instances, the
builds, the worlds and the sign-in are shared with a launcher installed beside this one. The log
of everything the window does is fh/fh.log in it.
";

fn main() {
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--help" | "-h" => {
                say(HELP);
                return;
            }
            "--version" | "-V" => {
                say(&format!("Fabric n Horizon v{VERSION}"));
                return;
            }
            other => {
                say(&format!("{other} is not an option of this program."));
                say("See FH --help.");
                std::process::exit(2);
            }
        }
    }
    std::process::exit(run());
}

/// Writes where a console may be reading; a double-click has none, and that is not a failure.
fn say(text: &str) {
    use std::io::Write;
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn run() -> i32 {
    // The launcher's data folder, which a "Move to another drive" may have pointed elsewhere: the
    // same instances, builds, worlds and sign-in a launcher installed beside this one uses.
    let problem = horizon_launcher::data_location::startup();
    let dir = horizon_launcher::backend::data_dir().join("fh");
    log::start(dir.join("fh.log"));
    log::line(&format!("Fabric n Horizon v{VERSION}."));
    if let Some(problem) = problem {
        log::line(&problem);
    }
    // The player's proxy, for the sign-in, the builds and the game's files.
    horizon_launcher::proxy::announce(horizon_launcher::backend::Settings::load().proxy);
    gpui_platform::application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1180.), px(760.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                window_min_size: Some(size(px(900.), px(560.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("Fabric n Horizon".into()),
                    ..Default::default()
                }),
                app_id: Some("fabric-n-horizon".into()),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| app::Fh::new(window, cx)),
        )
        .expect("the window");
        cx.on_window_closed(|cx, _| cx.quit()).detach();
        cx.activate(true);
    });
    0
}

#[cfg(test)]
mod tests {
    use super::VERSION;

    /// `fh/VERSION` and `fh/Cargo.toml` are bumped together: the tag the release workflow creates
    /// (`fh-v1.3`) comes from the file, the .exe's own version from the manifest, and a build
    /// where they disagree is a build nobody can name.
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
