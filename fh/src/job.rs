//! One launch, from the window: the sign-in when it is needed, the build, its files, the game —
//! and the same steps for a Minecraft: Java Edition instance (Fabric, Quilt, Forge, NeoForge or
//! none), which the launcher drives through Java.
//!
//! The work is `horizon-launcher`'s own, in the order its Play takes it: `account` for the
//! Microsoft sign-in and the owner gate, `backend` for the install, the preparation and the game's
//! command (or `backend::java_game` for a Java instance), `instances` and `launch` for the
//! instance's folder and wrapper. What is added here is the order, the status the window draws,
//! and a log line for every step — including the command line itself, because "not signed in" in
//! the game is nearly always a launch that never got a `--session-file`.

use crate::log;
use horizon_launcher::account::{self, SignIn, SignInState};
use horizon_launcher::instances::{self as inst, Instance};
use horizon_launcher::{backend, launch, process};
use std::process::Stdio;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime};

/// What the window's status bar draws: the step, how far it got, and the sign-in's code while one
/// is waiting. `busy`: a launch or a sign-in is running; `failed`/`done`: how the last one ended.
#[derive(Clone, Default, PartialEq)]
pub struct Status {
    pub title: String,
    pub line: String,
    pub fraction: Option<f32>,
    pub code: Option<(String, String)>,
    pub busy: bool,
    pub failed: bool,
    pub done: bool,
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);

/// The status of the running (or last) launch, if one has been asked for.
pub fn status() -> Option<Status> {
    STATUS.lock().ok().and_then(|status| status.clone())
}

/// Whether a launch or a sign-in is running.
pub fn busy() -> bool {
    status().is_some_and(|status| status.busy)
}

/// Forgets the last launch's status (the window's Dismiss).
pub fn clear() {
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

fn begin(title: String, line: String) {
    set(|status| {
        *status = Status {
            title,
            line,
            fraction: None,
            code: None,
            busy: true,
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

/// One instance's Play, on its own thread: the window stays live while it works.
pub fn play(instance: Instance) {
    begin(
        format!("Starting {}", instance.name),
        "Reading your sign-in and the game's files…".to_owned(),
    );
    log::line(&format!("Play: {} ({}).", instance.name, instance.id));
    std::thread::spawn(move || {
        let result = run(&instance);
        match result {
            Ok(note) => {
                log::line(&note);
                set(|status| {
                    status.busy = false;
                    status.done = true;
                    status.code = None;
                    status.fraction = None;
                    status.title = format!("{} ended", instance.name);
                    status.line = note;
                });
            }
            Err(error) => {
                log::line(&format!("{} did not start: {error}", instance.name));
                set(|status| {
                    status.busy = false;
                    status.failed = true;
                    status.code = None;
                    status.fraction = None;
                    status.title = format!("{} did not start", instance.name);
                    status.line = error;
                });
            }
        }
    });
}

/// The whole launch. Ok: what to say when the game ended; Err: why it did not start.
fn run(instance: &Instance) -> Result<String, String> {
    sign_in()?;
    let mut cmd = if instance.java.is_some() {
        backend::java_game(instance, None, &|line| log::line(&line))?
    } else {
        let tag = version(instance)?;
        prepare(&tag)?;
        let cmd = backend::client_as(&tag, None)?;
        // Every launch of a Horizon instance carries the read-once session file the game signs
        // in with; one without it plays offline, and its multiplayer asks for a sign-in again.
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
    step(
        &format!("{} is starting", instance.name),
        "Horizon is starting: the game's window comes in a moment.".to_owned(),
    );
    let started = SystemTime::now();
    let clock = Instant::now();
    let status = cmd
        .spawn()
        .and_then(|mut child| child.wait())
        .map_err(|e| format!("Could not start {:?}: {e}", cmd.get_program()))?;
    drop(runtime);
    let played = clock.elapsed();
    if let Err(e) = inst::record_play(&instance.id, played) {
        log::line(&format!("Play time not saved: {e}"));
    }
    if let Some(mut after) = launch::after_exit(&instance.launch, &instance.home, &vars) {
        match after.status() {
            Ok(s) if s.success() => {}
            Ok(s) => log::line(&format!("The post-exit command failed ({s}).")),
            Err(e) => log::line(&format!("The post-exit command didn't run: {e}")),
        }
    }
    log::line(&format!("The game ended: {status}."));
    if status.success() {
        return Ok(format!(
            "{} ended after {}.",
            instance.name,
            inst::play_time(played)
        ));
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

/// The Microsoft sign-in, unless this computer has an account that owns the game already.
fn sign_in() -> Result<(), String> {
    let accounts = account::Accounts::load();
    if !accounts.accounts.is_empty() && accounts.has_owner(account::store()) {
        log::line("Signed in already.");
        return Ok(());
    }
    log::line("Signing in with Microsoft.");
    step(
        "Signing in with Microsoft",
        "Asking Microsoft for a sign-in code…".to_owned(),
    );
    let sign_in = SignIn::start();
    let mut opened = false;
    loop {
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
            SignInState::Working(phase) => step("Signing in with Microsoft", phase.label().to_owned()),
            SignInState::Done(Ok(account)) => {
                log::line(&format!("Signed in as {}.", account.name));
                return Ok(());
            }
            SignInState::Done(Err(e)) => return Err(e),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// The build the instance plays: its own pinned build, installed when this computer has no copy,
/// else the launcher's choice from what is installed, and the newest published build when nothing
/// is installed at all.
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
            step("Getting Horizon", "Asking GitHub for the newest build…".to_owned());
            let releases = backend::releases().map_err(|e| {
                format!(
                    "No Horizon build is installed on this computer yet, and GitHub could not be \
                     asked for one ({e})."
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
    step("Getting Horizon", format!("Downloading {tag}…"));
    let tag = backend::install_tag(tag, false, fraction)?;
    fraction(1.);
    log::line(&format!("Installed {tag}."));
    Ok(tag)
}

/// Prepares `tag`'s resources when it has none: from the player's own Minecraft install, as the
/// launcher's Play does on a build it has never prepared.
fn prepare(tag: &str) -> Result<(), String> {
    if backend::check_prepared(tag) {
        log::line(&format!("{tag} is prepared."));
        return Ok(());
    }
    log::line(&format!("Preparing {tag}."));
    horizon_launcher::mojang::CANCEL.store(false, std::sync::atomic::Ordering::Relaxed);
    step(
        "Getting the game's files",
        "Looking for your Minecraft install…".to_owned(),
    );
    let minecraft = backend::minecraft_dir(None)?;
    let lines = |line: String| log::line(&line);
    let (preparer, minecraft) =
        backend::preparer(tag, &minecraft, backend::MinecraftFrom::Install, &lines)?;
    backend::sounds(tag, &minecraft, &lines);
    backend::set_prepare_step(backend::PrepareStep::Prepare);
    step(
        "Preparing the game",
        "Building its tables from your Minecraft files…".to_owned(),
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
                    Err(_) => return Err("The preparation stopped without saying why.".into()),
                }
            }
        }
    }
    backend::forget_prepared();
    if !backend::check_prepared(tag) {
        return Err(format!(
            "{tag} is still not prepared: the preparation stopped before it finished. The log \
             says where."
        ));
    }
    log::line(&format!("{tag} is prepared."));
    Ok(())
}

/// A sign-in on its own, from the window's Accounts page: the code shows there and goes on the
/// clipboard, and Microsoft's page opens once.
pub fn sign_in_now() {
    begin(
        "Signing in with Microsoft".to_owned(),
        "Asking Microsoft for a sign-in code…".to_owned(),
    );
    std::thread::spawn(|| match sign_in() {
        Ok(()) => {
            let name = account::Accounts::load()
                .selected()
                .map(|account| account.name.clone())
                .unwrap_or_default();
            set(|status| {
                status.busy = false;
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
                status.failed = true;
                status.code = None;
                status.title = "Sign-in failed".to_owned();
                status.line = error;
            });
        }
    });
}
