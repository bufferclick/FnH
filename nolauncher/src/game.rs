//! The game itself: a build of Horizon, its resources, and the process.
//!
//! All of it is the launcher's: `backend::install_tag` downloads and checks the build (signed
//! checksums, Nightly titles) and keeps a few versions on disk; `backend::preparer` takes the
//! game's files from the player's own Minecraft — the official launcher, Prism, MultiMC, the
//! Modrinth App, ATLauncher, GDLauncher — exactly as Play does, and only downloads them from
//! Mojang when no install has them and the signed-in account owns the game; `backend::client_as`
//! runs the owner gate, writes the read-once session file and builds the command; `instances` and
//! `launch` put it in the instance the player picked and through their wrapper.
//!
//! What is added here is the order, the page's progress and what happens when the game ends
//! badly.

use crate::host;
use crate::page::Page;
use crate::Options;
use horizon_launcher::account;
use horizon_launcher::backend;
use horizon_launcher::instances::{self, Instance};
use horizon_launcher::launch;
use horizon_launcher::{mojang, process};
use std::process::Stdio;
use std::sync::atomic::Ordering;
use std::time::SystemTime;

/// The instance Play starts, as the launcher's own `play` reads it. A Java instance plays
/// Mojang's game through Java, which this program does not run: Horizon's own instance is
/// started instead, and the log says so.
pub fn instance(log: &host::Log) -> Instance {
    let selected = instances::selected();
    if selected.java.is_none() {
        return selected;
    }
    log.line(&format!(
        "{} plays Minecraft: Java Edition, not Horizon: starting Horizon instead.",
        selected.name
    ));
    instances::find("Horizon").unwrap_or(selected)
}

/// The installed build to play: the one `--tag` names, the one the instance is kept on, else the
/// newest published build — installing it now when this computer has no copy.
pub fn tag(options: &Options, instance: &Instance, page: &Page, log: &host::Log) -> Result<String, String> {
    let installed = backend::installed();
    if let Some(wanted) = options.tag.clone().or_else(|| instance.version.clone()) {
        if installed.contains(&wanted) {
            log.line(&format!("Playing the installed {wanted}."));
            return Ok(wanted);
        }
        return install(&wanted, page, log);
    }
    if options.no_update {
        return backend::resolve_version(None);
    }
    // The newest build GitHub publishes for this platform. Without a list (no network, GitHub
    // answering nothing) what is installed still plays: a launcher's job is to start the game.
    let releases = match backend::releases() {
        Ok(releases) => releases,
        Err(e) => {
            log.line(&format!(
                "No list of Horizon builds ({e}): playing what is installed."
            ));
            // Nothing installed and no list is nothing to play: GitHub's rate limit is the usual
            // reason, and it passes.
            return backend::resolve_version(None).map_err(|_| {
                format!(
                    "No Horizon build is installed on this computer yet, and GitHub could not be \
                     asked for one ({e}). Start it again in a few minutes."
                )
            });
        }
    };
    match backend::latest(&releases) {
        Some(latest) if installed.contains(&latest.tag) => {
            log.line(&format!("{} is the newest build, and it is installed.", latest.tag));
            Ok(latest.tag.clone())
        }
        Some(latest) => {
            let newest = latest.tag.clone();
            install(&newest, page, log)
        }
        None => backend::resolve_version(None),
    }
}

fn install(tag: &str, page: &Page, log: &host::Log) -> Result<String, String> {
    log.line(&format!("Installing {tag}."));
    page.step("Getting Horizon", &format!("Downloading {tag}…"), Some(0.0));
    let progress = |fraction: f32| page.progress(fraction);
    let tag = backend::install_tag(tag, false, progress)?;
    page.progress(1.0);
    log.line(&format!("Installed {tag}."));
    Ok(tag)
}

/// Prepares `tag`'s resources when it has none: from the player's own Minecraft install, as the
/// launcher's Play does on a build it has never prepared — the files, Java, the sounds and
/// horizon-prepare's own work, each reported on the page.
pub fn prepare(tag: &str, page: &Page, log: &host::Log) -> Result<(), String> {
    if backend::check_prepared(tag) {
        log.line(&format!("{tag} is prepared."));
        return Ok(());
    }
    log.line(&format!("Preparing {tag}."));
    mojang::CANCEL.store(false, Ordering::Relaxed);
    page.step(
        "Getting the game's files",
        "Looking for your Minecraft install…",
        None,
    );
    let minecraft = backend::minecraft_dir(None)?;
    let lines = |line: String| {
        log.line(&line);
        page.line(&line);
    };
    let (preparer, minecraft) = backend::preparer(
        tag,
        &minecraft,
        backend::MinecraftFrom::Install,
        &lines,
    )?;
    // The sounds come beside the preparation (horizon-prepare sounds): a version without them
    // plays silently, which is a line in the log, never a failed start.
    backend::sounds(tag, &minecraft, &lines);
    backend::set_prepare_step(backend::PrepareStep::Prepare);
    page.step(
        "Preparing the game",
        "Building its tables from your Minecraft files…",
        None,
    );
    match preparer {
        backend::Preparer::InProcess(options) => backend::prepare_in_process(&options, &lines)?,
        backend::Preparer::Process(cmd) => run(cmd, page, log)?,
    }
    // The answer `check_prepared` kept is from before the preparation.
    backend::forget_prepared();
    if !backend::check_prepared(tag) {
        return Err(format!(
            "{tag} is still not prepared: the preparation stopped before it finished. The log \
             says where."
        ));
    }
    log.line(&format!("{tag} is prepared."));
    Ok(())
}

/// A preparer this build did not link in (an older version's own binary, or its Python script),
/// run to the end: its lines go to the page, its exit decides.
fn run(cmd: std::process::Command, page: &Page, log: &host::Log) -> Result<(), String> {
    let (tx, rx) = std::sync::mpsc::channel();
    backend::spawn(backend::Job::Prepare, cmd, tx)?;
    loop {
        match rx.recv() {
            Ok(backend::Msg::Line(backend::Job::Prepare, line)) => {
                log.line(&line);
                page.line(&line);
            }
            Ok(backend::Msg::Exit(backend::Job::Prepare, result)) => return result,
            Ok(_) => {}
            Err(_) => return Err("The preparation stopped without saying why.".into()),
        }
    }
}

/// Starts the game and stays until it ends, as the launcher does: the Windows runtime slot it
/// staged must outlive the game, and a crash is worth the page's words.
pub fn play(
    tag: &str,
    options: &Options,
    instance: &Instance,
    page: &Page,
    log: &host::Log,
) -> Result<(), String> {
    page.step(
        "Starting Horizon",
        "Reading your sign-in and the game's files…",
        None,
    );
    // The owner gate, the session file and the command, then the instance's own folder and
    // launch wrapper (`pre-launch`, a wrapper program, its variables), as `play` does.
    let mut cmd = backend::client_as(tag, None)?;
    instances::apply(&mut cmd, instance)?;
    cmd.args(&options.game);
    let cmd = launch::wrap(cmd, &instance.launch);
    let (mut cmd, runtime) = backend::prepare_command(cmd)?;
    process::hide_console(&mut cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(e) = account::take_unsaved() {
        log.line(&e);
    }
    log.line(&format!(
        "Starting {tag} in {}.",
        instance.home.display()
    ));
    let started = SystemTime::now();
    page.starting();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start {:?}: {e}", cmd.get_program()))?;
    let status = child
        .wait()
        .map_err(|e| format!("Horizon could not be waited for: {e}"))?;
    drop(runtime);
    log.line(&format!("Horizon ended: {status}."));
    if status.success() {
        return Ok(());
    }
    // A crash report says what went wrong; without one, the game's own log is the place.
    match backend::crash_report_since(&backend::game_home(), started) {
        Some(report) => {
            let summary = std::fs::read_to_string(&report)
                .ok()
                .and_then(|text| backend::crash_summary(&text));
            Err(match summary {
                Some(summary) => format!("Horizon crashed: {summary} — {}", report.display()),
                None => format!("Horizon crashed ({status}): {}", report.display()),
            })
        }
        None => Err(format!(
            "Horizon ended ({status}). Its log is in {}.",
            backend::logs_dir().display()
        )),
    }
}
