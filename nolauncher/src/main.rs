//! NoLauncherHorizon: Horizon with the launcher stripped out. One .exe, double-clicked: the
//! browser opens on Microsoft's sign-in, the game's files come from the player's own Minecraft
//! exactly as the launcher took them (no Azure app of your own, no settings screen), the newest
//! build of Horizon is installed when there is one, and the game starts. There is no window, no
//! page to click through, and nothing to configure.
//!
//! The work itself is the launcher's own, from `horizon-launcher`'s library: `account` for the
//! Microsoft sign-in and the owner gate, `backend` for the release list, the install, the
//! preparation (`horizon-prepare`) and the game's command, `instances` and `launch` for the
//! instance the player picked in the launcher. What this crate adds is the order, the page the
//! browser shows while it works (`page.rs`) and the system around it (`host.rs`).
//!
//! Versioning (nolauncher/VERSION, the tag the release workflow publishes): v1.0 first, then one
//! tenth per update — v1.1, …, v1.9, v1.10 (never v2.0 for an update) — and Minecraft's own
//! numbers later where a build is worth naming after them, v1.12.2.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod boot;
mod game;
mod host;
mod page;
mod sign_in;

use std::ffi::OsString;
use std::io::Write;

/// The version this build is: `nolauncher/VERSION` through `build.rs`, the same string the
/// release is tagged with.
pub const VERSION: &str = env!("NLH_VERSION");

const HELP: &str = "\
NoLauncherHorizon: Horizon with no launcher.

Double-click it and Horizon starts. It signs you in with Microsoft in your browser, takes the
game's files from your own Minecraft the way the launcher does, installs the newest build of
Horizon when there is one, and starts the game. Nothing else opens: the page in your browser is
this program's only window, and it shows the sign-in code or how far the work is.

Usage: NoLauncherHorizon [options] [-- <game arguments>]

Options:
  --tag TAG        Play this build, installing it if this computer has no copy (latest: newest)
  --no-update      Play what is installed; do not look for a newer build
  --version, -V    Print the version and stop
  --help, -h       Print this and stop
  --               Everything after this goes to the game: --world NAME, --server HOST, and so on

The data folder is the launcher's own (HORIZON_LAUNCHER_HOME overrides it), so builds, worlds and
the sign-in are shared with a launcher installed beside it. The log of the last starts is
nolauncher/nolauncher.log in it.
";

/// What the command line asked for; everything else is the game's.
#[derive(Debug, Default, PartialEq)]
pub struct Options {
    pub help: bool,
    pub version: bool,
    /// The build to play (`--tag`): installed when missing, instead of the newest.
    pub tag: Option<String>,
    /// Play what is installed, without asking GitHub for a newer build.
    pub no_update: bool,
    /// The game's own arguments, in the order they were given (`--world`, `--server`, …).
    pub game: Vec<OsString>,
}

impl Options {
    /// The command line, from the arguments after the program's name. An argument this program
    /// does not know is the game's: the launcher's `play` takes the same ones, so
    /// `NoLauncherHorizon --world W` works without a `--` in front of it.
    pub fn parse(mut args: impl Iterator<Item = OsString>) -> Result<Self, String> {
        let mut options = Self::default();
        while let Some(arg) = args.next() {
            let text = arg.to_string_lossy().into_owned();
            match text.as_str() {
                "--help" | "-h" => options.help = true,
                "--version" | "-V" => options.version = true,
                "--no-update" => options.no_update = true,
                "--tag" => {
                    let tag = args
                        .next()
                        .map(|tag| tag.to_string_lossy().into_owned())
                        .filter(|tag| !tag.is_empty())
                        .ok_or("--tag needs a build: --tag latest, or one this computer has.")?;
                    options.tag = Some(tag);
                }
                _ if text.starts_with("--tag=") => {
                    options.tag = Some(text["--tag=".len()..].to_owned());
                }
                "--" => {
                    options.game.extend(args);
                    break;
                }
                _ => options.game.push(arg),
            }
        }
        Ok(options)
    }
}

/// Writes where a console may be reading; a double-click has none, and that is not a failure.
fn say(text: &str) {
    let _ = writeln!(std::io::stdout(), "{text}");
}

fn main() {
    let options = match Options::parse(std::env::args_os().skip(1)) {
        Ok(options) => options,
        Err(e) => {
            say(&e);
            say("See NoLauncherHorizon --help.");
            std::process::exit(2);
        }
    };
    if options.help {
        say(HELP);
        return;
    }
    if options.version {
        say(&format!("NoLauncherHorizon v{}", VERSION));
        return;
    }
    std::process::exit(boot::run(options));
}

#[cfg(test)]
mod tests {
    use super::{Options, VERSION};
    use std::ffi::OsString;

    fn parse(args: &[&str]) -> Result<Options, String> {
        Options::parse(args.iter().map(|arg| OsString::from(*arg)))
    }

    fn ok(args: &[&str]) -> Options {
        parse(args).expect("a command line this program takes")
    }

    #[test]
    fn the_game_gets_the_arguments_this_program_does_not_know() {
        let options = ok(&["--world", "My World", "--server", "friend:25565", "--creative"]);
        assert_eq!(
            options.game,
            [
                OsString::from("--world"),
                OsString::from("My World"),
                OsString::from("--server"),
                OsString::from("friend:25565"),
                OsString::from("--creative"),
            ]
        );
        assert!(!options.help && !options.version && !options.no_update);
        assert_eq!(options.tag, None);
    }

    #[test]
    fn a_double_dash_hands_everything_after_it_to_the_game() {
        let options = ok(&["--no-update", "--", "--version", "--tag", "1.2"]);
        assert!(options.no_update);
        assert!(!options.version, "the game's --version is not this one's");
        assert_eq!(options.tag, None);
        assert_eq!(
            options.game,
            [
                OsString::from("--version"),
                OsString::from("--tag"),
                OsString::from("1.2"),
            ]
        );
    }

    #[test]
    fn the_tag_is_named_with_a_space_or_an_equals_sign() {
        assert_eq!(ok(&["--tag", "latest"]).tag.as_deref(), Some("latest"));
        assert_eq!(ok(&["--tag=latest"]).tag.as_deref(), Some("latest"));
        assert!(parse(&["--tag"]).is_err(), "--tag alone names no build");
        assert!(
            parse(&["--tag", ""]).is_err(),
            "an empty tag installs nothing"
        );
    }

    #[test]
    fn help_and_version_are_short_and_longform() {
        assert!(ok(&["-h"]).help && ok(&["--help"]).help);
        assert!(ok(&["-V"]).version && ok(&["--version"]).version);
    }

    /// `nolauncher/VERSION` and `nolauncher/Cargo.toml` are bumped together: the tag the release
    /// workflow creates (`nlh-v1.0`) comes from the file, the .exe's own version from the
    /// manifest, and a build where they disagree is a build nobody can name.
    #[test]
    fn the_version_is_the_one_the_release_is_tagged_with() {
        assert_eq!(VERSION, VERSION.trim(), "VERSION has no padding");
        assert!(
            env!("CARGO_PKG_VERSION").starts_with(&format!("{}.", VERSION)),
            "Cargo.toml's {} and VERSION's {} are the same release",
            env!("CARGO_PKG_VERSION"),
            VERSION
        );
    }
}
