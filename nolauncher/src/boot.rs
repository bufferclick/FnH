//! One start, in the order the launcher does the same things: the data folder, the sign-in, the
//! build, its resources, the game.

use crate::page::Page;
use crate::{game, host, sign_in, Options};
use horizon_launcher::{account, backend, data_location, proxy};

/// The whole start. 0 when the game played, 1 when it did not (the page and the log say why).
pub fn run(options: Options) -> i32 {
    // The launcher's data folder, which a "Move to another drive" may have pointed elsewhere: the
    // same builds, worlds, instances and sign-in a launcher installed beside this one uses.
    let problem = data_location::startup();
    let dir = backend::data_dir().join("nolauncher");
    let log = host::Log::open(dir.join("nolauncher.log"));
    if let Some(problem) = problem {
        log.line(&problem);
    }
    log.line(&format!("NoLauncherHorizon v{}.", crate::VERSION));
    let page = Page::new(&dir);
    page.footer(&footer(&log, None));
    match start(&options, &page, &log) {
        Ok(()) => 0,
        Err(e) => {
            log.line(&format!("Stopped: {e}"));
            page.error(&e);
            1
        }
    }
}

fn start(options: &Options, page: &Page, log: &host::Log) -> Result<(), String> {
    // The player's proxy, for every download: the same setting the launcher reads.
    proxy::announce(backend::Settings::load().proxy);
    // Without the game's public client there is no sign-in to do, and nothing to play.
    if account::client_id().is_none() {
        return Err(account::NOT_CONFIGURED.to_string());
    }
    sign_in::ensure(page, log)?;
    let instance = game::instance(log);
    let tag = game::tag(options, &instance, page, log)?;
    page.footer(&footer(log, Some(&tag)));
    game::prepare(&tag, page, log)?;
    game::play(&tag, options, &instance, page, log)
}

/// What the page's foot says: this build, the game's build once it is known, the log's path.
fn footer(log: &host::Log, tag: Option<&str>) -> String {
    match tag {
        Some(tag) => format!(
            "NoLauncherHorizon v{} · Horizon {tag} · log: {}",
            crate::VERSION,
            log.path().display()
        ),
        None => format!(
            "NoLauncherHorizon v{} · log: {}",
            crate::VERSION,
            log.path().display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::footer;
    use crate::host::Log;

    /// The foot names what was started and where the log is, which is all a player has when the
    /// page says "it did not start".
    #[test]
    fn the_foot_names_the_build_the_game_and_the_log() {
        let dir = std::env::temp_dir().join("nolauncher-footer-test");
        let log = Log::open(dir.join("nolauncher.log"));
        let foot = footer(&log, Some("nightly-2026-11-15-abc1234"));
        assert!(foot.contains(&format!("v{}", crate::VERSION)), "{foot}");
        assert!(foot.contains("nightly-2026-11-15-abc1234"), "{foot}");
        assert!(foot.contains("nolauncher.log"), "{foot}");
        let early = footer(&log, None);
        assert!(!early.contains("Horizon nightly"), "{early}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
