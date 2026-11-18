//! One launch, from the window: the sign-in when it is needed, the build, its files, the game —
//! and the same steps for a Minecraft: Java Edition instance (Fabric, Quilt, Forge, NeoForge or
//! none), which the launcher drives through Java.
//!
//! Tracks the running instance's id and honours `kill()` (`Ctrl+K` or JLac's right-hand `Kill`
//! button, as well as the factory-reset chord) so a running instance can be stopped cleanly from
//! the window.

use crate::log;
use horizon_launcher::account::{self, SignIn, SignInState};
use horizon_launcher::instances::{self as inst, Instance};
use horizon_launcher::{backend, launch, process};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// What the window draws for the running (or most recent) task.
#[derive(Clone, Default, PartialEq)]
pub struct Status {
    pub instance_id: Option<String>,
    pub title: String,
    pub line: String,
    pub fraction: Option<f32>,
    pub code: Option<(String, String)>,
    pub busy: bool,
    /// True once the game child process itself has spawned and can be terminated by `kill()`.
    pub running_game: bool,
    pub failed: bool,
    pub done: bool,
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);
static KILL_REQUESTED: AtomicBool = AtomicBool::new(false);

/// The status of the running (or last) launch, if one has been asked for.
pub fn status() -> Option<Status> {
    STATUS.lock().ok().and_then(|status| status.clone())
}

/// Whether a launch or a sign-in is running.
pub fn busy() -> bool {
    status().is_some_and(|status| status.busy)
}

/// Whether a game process is actively running and can be killed.
pub fn can_kill() -> bool {
    status().is_some_and(|status| status.busy && status.running_game)
}

/// The id of the instance currently launching or running, if any.
pub fn active_instance() -> Option<String> {
    status()
        .filter(|status| status.busy)
        .and_then(|status| status.instance_id)
}

/// Requests termination of the running game process (returns true if one was active).
pub fn kill() -> bool {
    let active = busy();
    if active {
        KILL_REQUESTED.store(true, Ordering::Release);
        horizon_launcher::mojang::CANCEL.store(true, Ordering::Relaxed);
    }
    active
}

/// Forgets the last launch's status (the window's Dismiss).
pub fn clear() {
    KILL_REQUESTED.store(false, Ordering::Release);
    if let Ok(mut status) = STATUS.lock() {
        *status = None;
    }
}

fn set(change: impl FnOnce(&mut Status)) {
    if let Ok(mut status) = STATUS.lock() {
        let mut current = status.clone().unwrap_or_default();
        change(&mut current);
        *status = Some(current);
    }
}

fn begin(instance_id: Option<String>, title: String, line: String) {
    KILL_REQUESTED.store(false, Ordering::Release);
    set(|status| {
        *status = Status {
            instance_id,
            title,
            line,
            fraction: None,
            code: None,
            busy: true,
            running_game: false,
            failed: false,
            done: false,
        };
    });
}

fn step(title: &str, line: String) {
    set(|status| {
        status.title = title.to_owned();
        status.line = line;
        status.code = None;
    });
}

fn fraction(value: f32) {
    set(|status| status.fraction = Some(value.clamp(0., 1.)));
}

/// Launches `instance` on its own background thread so the window stays responsive.
pub fn play(instance: Instance) {
    if busy() {
        return;
    }
    begin(
        Some(instance.id.clone()),
        format!("Launching {}", instance.name),
        "Checking Microsoft account and game files…".to_owned(),
    );
    log::line(&format!("Launch: {} ({}).", instance.name, instance.id));
    std::thread::spawn(move || {
        let result = run(&instance);
        match result {
            Ok(note) => {
                log::line(&note);
                set(|status| {
                    status.busy = false;
                    status.running_game = false;
                    status.done = true;
                    status.code = None;
                    status.fraction = None;
                    status.title = format!("{} finished", instance.name);
                    status.line = note;
                });
            }
            Err(error) => {
                log::line(&format!("{} did not start: {error}", instance.name));
                set(|status| {
                    status.busy = false;
                    status.running_game = false;
                    status.failed = true;
                    status.code = None;
                    status.fraction = None;
                    status.title = format!("{} stopped", instance.name);
                    status.line = error;
                });
            }
        }
    });
}

/// The whole launch. Ok: what to say when the game ended; Err: why it did not start.
fn run(instance: &Instance) -> Result<String, String> {
    sign_in(false)?;
    if KILL_REQUESTED.load(Ordering::Acquire) {
        return Ok(format!("{} launch cancelled.", instance.name));
    }
    let mut cmd = if instance.java.is_some() {
        step(
            &format!("Preparing {}", instance.name),
            "Resolving Minecraft, Fabric loader and libraries…".to_owned(),
        );
        backend::java_game(instance, None, &|line| log::line(&line))?
    } else {
        let tag = version(instance)?;
        prepare(&tag)?;
        let cmd = backend::client_as(&tag, None)?;
        if !cmd.get_args().any(|arg| arg == "--session-file") {
            log::line(
                "No --session-file on the command: this launch has no Microsoft session and \
                 plays offline (the launcher's own reason is above).",
            );
        }
        cmd
    };
    inst::apply(&mut cmd, instance)?;
    let vars = instance.vars(cmd.get_program());
    cmd.envs(vars.iter().map(|(key, value)| (key, value)));
    let cmd = launch::wrap(cmd, &instance.launch);
    let (mut cmd, runtime) = backend::prepare_command(cmd)?;
    process::hide_console(&mut cmd);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(e) = account::take_unsaved() {
        log::line(&e);
    }
    log::line(&format!(
        "Running: {:?} {}",
        cmd.get_program(),
        cmd.get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    ));
    let started = SystemTime::now();
    let clock = Instant::now();
    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Could not start {:?}: {e}", cmd.get_program()))?;
    set(|status| {
        status.running_game = true;
        status.fraction = None;
        status.title = format!("{} (running)", instance.name);
        status.line = "Instance is running.".to_owned();
    });
    let mut killed = false;
    let exit_status = loop {
        if KILL_REQUESTED.swap(false, Ordering::AcqRel) {
            log::line(&format!("Killing {} at user request.", instance.name));
            let _ = child.kill();
            killed = true;
            break child
                .wait()
                .map_err(|e| format!("Waiting for terminated process: {e}"))?;
        }
        match child.try_wait() {
            Ok(Some(done)) => break done,
            Ok(None) => std::thread::sleep(Duration::from_millis(100)),
            Err(e) => return Err(format!("Lost track of running process: {e}")),
        }
    };
    drop(runtime);
    let played = clock.elapsed();
    if let Err(e) = inst::record_play(&instance.id, played) {
        log::line(&format!("Play time not saved: {e}"));
    }
    if !killed {
        if let Some(mut after) = launch::after_exit(&instance.launch, &instance.home, &vars) {
            match after.status() {
                Ok(s) if s.success() => {}
                Ok(s) => log::line(&format!("The post-exit command failed ({s}).")),
                Err(e) => log::line(&format!("The post-exit command didn't run: {e}")),
            }
        }
    }
    log::line(&format!("The game ended: {exit_status}."));
    if killed {
        return Ok(format!(
            "{} killed after {}.",
            instance.name,
            inst::play_time(played)
        ));
    }
    if exit_status.success() {
        return Ok(format!(
            "{} exited cleanly after {}.",
            instance.name,
            inst::play_time(played)
        ));
    }
    match backend::crash_report_since(&instance.home, started)
        .or_else(|| backend::crash_report_since(&backend::game_home(), started))
    {
        Some(report) => {
            let summary = std::fs::read_to_string(&report)
                .ok()
                .and_then(|text| backend::crash_summary(&text));
            Err(match summary {
                Some(summary) => format!("Game crashed: {summary} — {}", report.display()),
                None => format!("Game crashed ({exit_status}): {}", report.display()),
            })
        }
        None => Err(format!(
            "Game exited with {exit_status}. Log: {}.",
            instance.home.join("logs").display()
        )),
    }
}

/// The Microsoft sign-in, skipped on launch (`force == false`) when an owning account is ready.
fn sign_in(force: bool) -> Result<(), String> {
    if !force {
        let accounts = account::Accounts::load();
        if !accounts.accounts.is_empty() && accounts.has_owner(account::store()) {
            log::line("Signed in already.");
            return Ok(());
        }
    }
    log::line("Signing in with Microsoft.");
    step(
        "Signing in with Microsoft",
        "Requesting device code from Microsoft…".to_owned(),
    );
    let sign_in = SignIn::start();
    let mut opened = false;
    loop {
        if KILL_REQUESTED.load(Ordering::Acquire) {
            return Err("Sign-in cancelled.".into());
        }
        match sign_in.state() {
            SignInState::Starting => {}
            SignInState::Code(code) => {
                let (user_code, link) = (code.user_code.clone(), code.verification_uri.clone());
                set(|status| status.code = Some((user_code.clone(), link.clone())));
                if !opened {
                    opened = true;
                    let url = log::sign_in_url(&link, &user_code);
                    log::line(&format!("Signing in at {url} with the code {user_code}."));
                    if !log::copy(&user_code) {
                        log::line("The code could not be put on the clipboard; it is in the window.");
                    }
                    if let Err(e) = log::open_url(&url) {
                        log::line(&format!("{e} Open {url} in a browser yourself."));
                    }
                }
            }
            SignInState::Working(phase) => {
                step("Signing in with Microsoft", phase.label().to_owned())
            }
            SignInState::Done(Ok(account)) => {
                log::line(&format!("Signed in as {}.", account.name));
                return Ok(());
            }
            SignInState::Done(Err(e)) => return Err(e),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Resolves or installs the Horizon build `instance` plays.
fn version(instance: &Instance) -> Result<String, String> {
    if let Some(pinned) = instance.version.clone() {
        if backend::installed().contains(&pinned) {
            log::line(&format!("{} plays the installed {pinned}.", instance.name));
            return Ok(pinned);
        }
        return install(&pinned);
    }
    match backend::resolve_version(None) {
        Ok(tag) => {
            log::line(&format!("Playing the installed {tag}."));
            Ok(tag)
        }
        Err(_) => {
            step("Downloading Horizon", "Checking GitHub for the latest release…".to_owned());
            let releases = backend::releases().map_err(|e| {
                format!(
                    "No Horizon build is installed yet, and GitHub could not be reached ({e})."
                )
            })?;
            let latest = backend::latest(&releases)
                .ok_or("GitHub lists no Horizon build for this computer.")?;
            let tag = latest.tag.clone();
            install(&tag)
        }
    }
}

fn install(tag: &str) -> Result<String, String> {
    log::line(&format!("Installing {tag}."));
    step("Downloading Horizon", format!("Downloading {tag}…"));
    let tag = backend::install_tag(tag, false, fraction)?;
    fraction(1.);
    log::line(&format!("Installed {tag}."));
    Ok(tag)
}

/// Prepares `tag`'s resources from the player's own Minecraft installation when needed.
fn prepare(tag: &str) -> Result<(), String> {
    if backend::check_prepared(tag) {
        log::line(&format!("{tag} is prepared."));
        return Ok(());
    }
    log::line(&format!("Preparing {tag}."));
    horizon_launcher::mojang::CANCEL.store(false, Ordering::Relaxed);
    step(
        "Preparing game assets",
        "Locating your Minecraft installation…".to_owned(),
    );
    let minecraft = backend::minecraft_dir(None)?;
    let lines = |line: String| log::line(&line);
    let (preparer, minecraft) =
        backend::preparer(tag, &minecraft, backend::MinecraftFrom::Install, &lines)?;
    backend::sounds(tag, &minecraft, &lines);
    backend::set_prepare_step(backend::PrepareStep::Prepare);
    step(
        "Preparing Horizon",
        "Building data tables from your Minecraft files…".to_owned(),
    );
    match preparer {
        backend::Preparer::InProcess(options) => backend::prepare_in_process(&options, &lines)?,
        backend::Preparer::Process(cmd) => {
            let (tx, rx) = std::sync::mpsc::channel();
            backend::spawn(backend::Job::Prepare, cmd, tx)?;
            loop {
                match rx.recv() {
                    Ok(backend::Msg::Line(backend::Job::Prepare, line)) => log::line(&line),
                    Ok(backend::Msg::Exit(backend::Job::Prepare, result)) => result?,
                    Ok(_) => {}
                    Err(_) => return Err("Preparation stopped unexpectedly.".into()),
                }
            }
        }
    }
    backend::forget_prepared();
    if !backend::check_prepared(tag) {
        return Err(format!(
            "{tag} is still not prepared: check the launcher log for details."
        ));
    }
    log::line(&format!("{tag} is prepared."));
    Ok(())
}

/// Starts a standalone Microsoft device-code sign-in from Accounts.
pub fn sign_in_now() {
    begin(
        None,
        "Signing in with Microsoft".to_owned(),
        "Requesting device code from Microsoft…".to_owned(),
    );
    std::thread::spawn(|| match sign_in(true) {
        Ok(()) => {
            let name = account::Accounts::load()
                .selected()
                .map(|account| account.name.clone())
                .unwrap_or_default();
            set(|status| {
                status.busy = false;
                status.running_game = false;
                status.done = true;
                status.code = None;
                status.title = "Signed in".to_owned();
                status.line = format!("Signed in as {name}.");
            });
        }
        Err(error) => {
            log::line(&format!("Sign-in failed: {error}"));
            set(|status| {
                status.busy = false;
                status.running_game = false;
                status.failed = true;
                status.code = None;
                status.title = "Sign-in failed".to_owned();
                status.line = error;
            });
        }
    });
}
