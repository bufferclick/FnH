//! Background game launching, live stdout/stderr console capture, Microsoft device-code sign-in,
//! Horizon version downloads, and Java runtime management.
//!
//! Fixes the `v1.5` Java/Fabric launch bug (`instances::apply` is strictly Horizon-only and is
//! never called on Java/Fabric instances) and streams live game + download output straight into
//! each instance's `Minecraft Log` / `Horizon Log` page in `InstanceWindow`.

use crate::log;
use crate::prefs::{HorizonTrack, InstanceCfg, Prefs};
use horizon_launcher::account::{self, SignIn, SignInState};
use horizon_launcher::instances::{self as inst, Instance};
use horizon_launcher::{backend, launch, process};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime};

/// Extra launch modifiers (direct world join, direct server join, offline mode).
#[derive(Clone, Debug, Default)]
pub struct LaunchExtras {
    pub world: Option<String>,
    pub server: Option<String>,
    pub offline: bool,
}

/// Current state of the active (or most recent) background task.
#[derive(Clone, Default, PartialEq)]
pub struct Status {
    pub instance_id: Option<String>,
    pub title: String,
    pub line: String,
    pub fraction: Option<f32>,
    pub code: Option<(String, String)>,
    pub busy: bool,
    pub running_game: bool,
    pub failed: bool,
    pub crashed: bool,
    pub done: bool,
    /// Set once when a game launch exits so windows can apply close/quit/crash policies.
    pub event_seq: u64,
}

struct ConsoleBuf {
    lines: VecDeque<String>,
    overflowed: bool,
    revision: u64,
}

impl Default for ConsoleBuf {
    fn default() -> Self {
        Self {
            lines: VecDeque::new(),
            overflowed: false,
            revision: 0,
        }
    }
}

static STATUS: Mutex<Option<Status>> = Mutex::new(None);
static KILL_REQUESTED: AtomicBool = AtomicBool::new(false);
static EVENT_SEQ: AtomicU64 = AtomicU64::new(1);
static CONSOLES: OnceLock<Mutex<HashMap<String, ConsoleBuf>>> = OnceLock::new();

fn consoles() -> &'static Mutex<HashMap<String, ConsoleBuf>> {
    CONSOLES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Appends one line to `instance_id`'s live console ring buffer.
pub fn push_console(instance_id: &str, line: impl Into<String>, max_lines: usize, stop_overflow: bool) {
    let limit = max_lines.clamp(200, 500_000);
    if let Ok(mut map) = consoles().lock() {
        let buf = map.entry(instance_id.to_owned()).or_default();
        if stop_overflow && buf.lines.len() >= limit {
            if !buf.overflowed {
                buf.overflowed = true;
                buf.lines.push_back(
                    "[FNH] Log history limit reached; further output paused (configurable in Settings -> Launcher -> Console).".into(),
                );
                buf.revision = buf.revision.wrapping_add(1);
            }
            return;
        }
        buf.lines.push_back(line.into());
        while buf.lines.len() > limit {
            buf.lines.pop_front();
        }
        buf.revision = buf.revision.wrapping_add(1);
    }
}

/// Clears `instance_id`'s live console buffer.
pub fn clear_console(instance_id: &str) {
    if let Ok(mut map) = consoles().lock() {
        if let Some(buf) = map.get_mut(instance_id) {
            buf.lines.clear();
            buf.overflowed = false;
            buf.revision = buf.revision.wrapping_add(1);
        }
    }
}

/// Returns the revision number of `instance_id`'s console so views only rebuild when new lines arrive.
pub fn console_revision(instance_id: &str) -> u64 {
    consoles()
        .lock()
        .ok()
        .and_then(|map| map.get(instance_id).map(|b| b.revision))
        .unwrap_or(0)
}

/// Returns up to the latest `tail` lines from `instance_id`'s console plus total buffered line count.
pub fn console_snapshot(instance_id: &str, tail: usize) -> (Vec<String>, usize) {
    let Ok(map) = consoles().lock() else {
        return (Vec::new(), 0);
    };
    let Some(buf) = map.get(instance_id) else {
        return (Vec::new(), 0);
    };
    let total = buf.lines.len();
    let skip = total.saturating_sub(tail);
    let out = buf.lines.iter().skip(skip).cloned().collect();
    (out, total)
}

/// Returns the complete console text for clipboard copy.
pub fn console_full_text(instance_id: &str) -> String {
    let Ok(map) = consoles().lock() else {
        return String::new();
    };
    let Some(buf) = map.get(instance_id) else {
        return String::new();
    };
    let mut out = String::new();
    for line in &buf.lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

pub fn status() -> Option<Status> {
    STATUS.lock().ok().and_then(|s| s.clone())
}

pub fn busy() -> bool {
    status().is_some_and(|s| s.busy)
}

pub fn can_kill() -> bool {
    status().is_some_and(|s| s.busy)
}

pub fn active_instance() -> Option<String> {
    status().filter(|s| s.busy).and_then(|s| s.instance_id)
}

pub fn kill() -> bool {
    let active = busy();
    if active {
        KILL_REQUESTED.store(true, Ordering::Release);
        horizon_launcher::mojang::CANCEL.store(true, Ordering::Relaxed);
    }
    active
}

pub fn clear() {
    KILL_REQUESTED.store(false, Ordering::Release);
    if let Ok(mut s) = STATUS.lock() {
        *s = None;
    }
}

fn set(change: impl FnOnce(&mut Status)) {
    if let Ok(mut slot) = STATUS.lock() {
        let mut cur = slot.clone().unwrap_or_default();
        change(&mut cur);
        *slot = Some(cur);
    }
}

fn begin(instance_id: Option<String>, title: String, line: String) {
    KILL_REQUESTED.store(false, Ordering::Release);
    horizon_launcher::mojang::CANCEL.store(false, Ordering::Relaxed);
    set(|s| {
        *s = Status {
            instance_id,
            title,
            line,
            fraction: None,
            code: None,
            busy: true,
            running_game: false,
            failed: false,
            crashed: false,
            done: false,
            event_seq: 0,
        };
    });
}

fn step(title: &str, line: String) {
    set(|s| {
        s.title = title.to_owned();
        s.line = line;
        s.code = None;
    });
}

fn fraction(value: f32) {
    set(|s| s.fraction = Some(value.clamp(0., 1.)));
}

/// Launches `instance` (Fabric/Java or Horizon) with global `prefs`, per-instance `cfg` and `extras`.
pub fn play(instance: Instance, prefs: Prefs, cfg: InstanceCfg, extras: LaunchExtras) {
    if busy() {
        return;
    }
    clear_console(&instance.id);
    let max_lines = prefs.console_max_lines;
    let stop_ovf = prefs.console_stop_on_overflow;
    push_console(
        &instance.id,
        format!(
            "[FNH] FNH v{} (made by Buffer) — launching '{}' ({})",
            crate::VERSION,
            instance.name,
            instance.id
        ),
        max_lines,
        stop_ovf,
    );
    begin(
        Some(instance.id.clone()),
        format!("Launching {}", instance.name),
        "Checking account and game files…".into(),
    );
    log::line(&format!("Launch: {} ({}).", instance.name, instance.id));

    std::thread::spawn(move || match run_launch(&instance, &prefs, &cfg, &extras) {
        Ok(note) => {
            log::line(&note);
            push_console(&instance.id, format!("[FNH] {note}"), max_lines, stop_ovf);
            let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
            set(|s| {
                s.busy = false;
                s.running_game = false;
                s.done = true;
                s.crashed = false;
                s.code = None;
                s.fraction = None;
                s.title = format!("{} finished", instance.name);
                s.line = note;
                s.event_seq = seq;
            });
        }
        Err((error, crashed)) => {
            log::line(&format!("{} failed: {error}", instance.name));
            push_console(
                &instance.id,
                format!("[ERROR] {error}"),
                max_lines,
                stop_ovf,
            );
            let seq = EVENT_SEQ.fetch_add(1, Ordering::Relaxed);
            set(|s| {
                s.busy = false;
                s.running_game = false;
                s.failed = true;
                s.crashed = crashed;
                s.code = None;
                s.fraction = None;
                s.title = format!("{} stopped", instance.name);
                s.line = error;
                s.event_seq = seq;
            });
        }
    });
}

fn run_launch(
    instance: &Instance,
    prefs: &Prefs,
    cfg: &InstanceCfg,
    extras: &LaunchExtras,
) -> Result<String, (String, bool)> {
    let max_lines = prefs.console_max_lines;
    let stop_ovf = prefs.console_stop_on_overflow;
    let emit = |msg: String| {
        log::line(&msg);
        push_console(&instance.id, msg.clone(), max_lines, stop_ovf);
        step(&format!("Preparing {}", instance.name), msg);
    };

    sign_in(false, Some(&emit)).map_err(|e| (e, false))?;

    if KILL_REQUESTED.load(Ordering::Acquire) {
        return Err(("Launch cancelled.".into(), false));
    }

    // Resolve server / world targets (explicit LaunchExtras override configured auto-join).
    let target_server = extras.server.clone().or_else(|| {
        (cfg.auto_join_enabled && !cfg.auto_join_server.trim().is_empty())
            .then(|| cfg.auto_join_server.trim().to_owned())
    });
    let target_world = extras.world.clone().or_else(|| {
        (cfg.auto_join_enabled
            && target_server.is_none()
            && !cfg.auto_join_world.trim().is_empty())
        .then(|| cfg.auto_join_world.trim().to_owned())
    });

    // Build effective per-instance account override (temporarily selecting account if needed for Java).
    let account_who = (cfg.override_account && !cfg.account_uuid.trim().is_empty())
        .then(|| cfg.account_uuid.trim().to_owned());

    // Build the game Command cleanly: Java/Fabric vs Horizon NEVER cross paths.
    let mut cmd = if instance.java.is_some() {
        build_java_command(
            instance,
            prefs,
            cfg,
            account_who.as_deref(),
            target_server.as_deref(),
            target_world.as_deref(),
            extras.offline,
            &emit,
        )
        .map_err(|e| (e, false))?
    } else {
        build_horizon_command(
            instance,
            prefs,
            cfg,
            account_who.as_deref(),
            target_server.as_deref(),
            target_world.as_deref(),
            extras.offline,
            &emit,
        )
        .map_err(|e| (e, false))?
    };

    // Merge global & per-instance environment variables, GPU tweaks, and custom commands.
    let effective_launch = build_effective_launch(instance, prefs, cfg);
    let vars = instance.vars(cmd.get_program());
    cmd.envs(vars.iter().map(|(k, v)| (k, v)));
    cmd.envs(effective_launch.env.iter().map(|(k, v)| (k, v)));

    let wrapped = launch::wrap(cmd, &effective_launch);
    let (mut ready, _runtime_guard) =
        backend::prepare_command(wrapped).map_err(|e| (e, false))?;
    process::hide_console(&mut ready);

    log::line(&format!("Starting: {}", describe_command(&ready)));
    push_console(
        &instance.id,
        format!("[FNH] Process: {}", ready.get_program().to_string_lossy()),
        max_lines,
        stop_ovf,
    );

    set(|s| {
        s.title = format!("Running {}", instance.name);
        s.line = "Running — click Kill (Ctrl+K) to stop.".into();
        s.fraction = None;
        s.code = None;
        s.running_game = true;
    });

    let started_sys = SystemTime::now() - Duration::from_secs(2);
    let started = Instant::now();
    let mut child = ready
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            (
                format!("Could not start {}: {e}", instance.name),
                false,
            )
        })?;

    // Stream live stdout and stderr into the instance's Console log window!
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        let id = instance.id.clone();
        let timestamps = prefs.console_timestamps;
        readers.push(std::thread::spawn(move || {
            let reader = BufReader::new(out);
            for line in reader.lines().map_while(Result::ok) {
                let formatted = if timestamps {
                    format!("{} {line}", clock_hms())
                } else {
                    line
                };
                push_console(&id, formatted, max_lines, stop_ovf);
            }
        }));
    }
    if let Some(err) = child.stderr.take() {
        let id = instance.id.clone();
        let timestamps = prefs.console_timestamps;
        readers.push(std::thread::spawn(move || {
            let reader = BufReader::new(err);
            for line in reader.lines().map_while(Result::ok) {
                let formatted = if timestamps {
                    format!("{} {line}", clock_hms())
                } else {
                    line
                };
                push_console(&id, formatted, max_lines, stop_ovf);
            }
        }));
    }

    let mut killed_by_user = false;
    let exit_status = loop {
        if KILL_REQUESTED.swap(false, Ordering::AcqRel) {
            killed_by_user = true;
            log::line(&format!("Killing {} at user request.", instance.name));
            push_console(
                &instance.id,
                "[FNH] Kill requested — terminating game process…",
                max_lines,
                stop_ovf,
            );
            let _ = child.kill();
            break child
                .wait()
                .map_err(|e| (format!("Stopped game: {e}"), false))?;
        }
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => std::thread::sleep(Duration::from_millis(120)),
            Err(e) => {
                return Err((
                    format!("Lost track of {}: {e}", instance.name),
                    false,
                ))
            }
        }
    };

    for handle in readers {
        let _ = handle.join();
    }

    let played = started.elapsed();
    if prefs.effective_record_playtime(cfg) {
        if let Err(e) = inst::record_play(&instance.id, played) {
            log::line(&format!("Playtime not saved: {e}"));
        }
    }

    // Run optional post-exit command (JLac Custom Commands).
    if let Some(mut post) = launch::after_exit(&effective_launch, &instance.home, &vars) {
        process::hide_console(&mut post);
        push_console(
            &instance.id,
            format!("[FNH] Running post-exit command: {}", effective_launch.post_exit),
            max_lines,
            stop_ovf,
        );
        let _ = post
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }

    if let Some(unsaved) = account::take_unsaved() {
        log::line(&unsaved);
    }

    if killed_by_user {
        return Ok(format!(
            "{} stopped after {}.",
            instance.name,
            inst::play_time(played)
        ));
    }
    if exit_status.success() {
        return Ok(format!(
            "{} exited cleanly after {} (exit code 0).",
            instance.name,
            inst::play_time(played)
        ));
    }

    // Non-zero exit: check for crash report and surface details directly in Console + status bar.
    match backend::crash_report_since(&instance.home, started_sys) {
        Some(report) => {
            let summary = std::fs::read_to_string(&report)
                .ok()
                .and_then(|t| backend::crash_summary(&t));
            Err((
                match summary {
                    Some(s) => format!("Game crashed: {s} ({})", report.display()),
                    None => format!("Game crashed ({exit_status}): {}", report.display()),
                },
                true,
            ))
        }
        None => Err((
            format!("Game exited with {exit_status} — open Minecraft Log for full output."),
            true,
        )),
    }
}

/// Builds the complete Java Edition / Fabric `Command` with all global & instance overrides.
#[allow(clippy::too_many_arguments)]
fn build_java_command(
    instance: &Instance,
    prefs: &Prefs,
    cfg: &InstanceCfg,
    account_who: Option<&str>,
    server: Option<&str>,
    world: Option<&str>,
    _offline: bool,
    emit: &dyn Fn(String),
) -> Result<Command, String> {
    let mut effective = instance.clone();
    if let Some(java) = effective.java.as_mut() {
        // 1. Memory (-Xms, -Xmx, -XX:MaxMetaspaceSize).
        let (min_mb, max_mb, perm_mb) = if cfg.override_memory {
            (cfg.min_mem_mb, cfg.max_mem_mb, cfg.permgen_mb)
        } else {
            let max = java.memory_mb.unwrap_or(prefs.java_max_mem_mb);
            (prefs.java_min_mem_mb, max, prefs.java_permgen_mb)
        };
        let safe_max = max_mb.clamp(512, 65_536);
        let safe_min = min_mb.clamp(128, safe_max);
        java.memory_mb = Some(safe_max);

        // 2. Java installation override (normalizing `.../bin/javaw.exe` -> JDK home).
        let raw_java = if cfg.override_java {
            Some(cfg.java_path.as_str())
        } else if java.home.is_none() && !prefs.default_java_path.trim().is_empty() {
            Some(prefs.default_java_path.as_str())
        } else {
            None
        };
        if let Some(raw) = raw_java {
            java.home = crate::prefs::normalize_java_home(raw);
        }

        // 3. Combined JVM arguments (min memory, metaspace, LWJGL native overrides, user flags).
        let mut jvm_parts: Vec<String> = Vec::new();
        let base_flags = if cfg.override_jvm_args {
            cfg.jvm_args.trim()
        } else if !java.jvm_args.trim().is_empty() {
            java.jvm_args.trim()
        } else {
            prefs.default_jvm_args.trim()
        };
        if !base_flags.contains("-Xms") {
            jvm_parts.push(format!("-Xms{safe_min}M"));
        }
        if perm_mb >= 64 && !base_flags.contains("MaxMetaspaceSize") {
            jvm_parts.push(format!("-XX:MaxMetaspaceSize={perm_mb}M"));
        }
        let (glfw, openal) = if cfg.override_tweaks {
            (cfg.custom_glfw_path.trim(), cfg.custom_openal_path.trim())
        } else {
            (prefs.custom_glfw_path.trim(), prefs.custom_openal_path.trim())
        };
        if !glfw.is_empty() {
            jvm_parts.push(format!("-Dorg.lwjgl.glfw.libname={glfw}"));
        }
        if !openal.is_empty() {
            jvm_parts.push(format!("-Dorg.lwjgl.openal.libname={openal}"));
        }
        if !base_flags.is_empty() {
            jvm_parts.push(base_flags.to_owned());
        }
        java.jvm_args = jvm_parts.join(" ");

        // 4. Window dimensions.
        if let Some(win) = prefs.effective_window_for(cfg) {
            java.window = Some(win);
        }
    }

    // Temporarily switch selected account if instance overrides default account, then restore.
    let previous_account = if let Some(uuid) = account_who {
        let mut accs = account::Accounts::load();
        let prev = accs.selected.clone();
        if accs.play_as(uuid).is_ok() {
            let _ = accs.save();
            prev
        } else {
            None
        }
    } else {
        None
    };

    std::fs::create_dir_all(&effective.home)
        .map_err(|e| format!("{}: {e}", effective.home.display()))?;
    std::fs::create_dir_all(effective.home.join("mods")).ok();

    let built = backend::java_game(&effective, server, emit);

    if let Some(prev) = previous_account {
        let mut accs = account::Accounts::load();
        accs.selected = Some(prev);
        let _ = accs.save();
    }

    let mut cmd = built?;
    cmd.current_dir(&effective.home);

    if prefs.effective_fullscreen(cfg) {
        cmd.arg("--fullscreen");
    }
    if let Some(w) = world.filter(|_| server.is_none()) {
        cmd.arg("--quickPlaySingleplayer").arg(w);
    }
    if !cfg.extra_game_args.trim().is_empty() {
        cmd.args(backend::split_args(&cfg.extra_game_args));
    }

    Ok(cmd)
}

/// Builds the complete Horizon `Command` with strict instance isolation & all Horizon settings.
#[allow(clippy::too_many_arguments)]
fn build_horizon_command(
    instance: &Instance,
    prefs: &Prefs,
    cfg: &InstanceCfg,
    account_who: Option<&str>,
    server: Option<&str>,
    world: Option<&str>,
    offline: bool,
    emit: &dyn Fn(String),
) -> Result<Command, String> {
    let tag = resolve_horizon_version(instance, prefs, cfg, emit)?;
    prepare_horizon_version(&tag, emit)?;

    let verify = if cfg.horizon_override_updates {
        cfg.horizon_verify_assets
    } else {
        prefs.horizon_verify_assets
    };
    if verify {
        emit("Verifying Horizon game assets…".into());
        backend::complete_assets(&tag);
    }

    // Guarantee instance folder & isolated offline name exist before spawning.
    std::fs::create_dir_all(&instance.home)
        .map_err(|e| format!("{}: {e}", instance.home.display()))?;
    let chosen_offline = if !cfg.offline_name.trim().is_empty() {
        Some(cfg.offline_name.trim())
    } else if !prefs.offline_player_name.trim().is_empty() {
        Some(prefs.offline_player_name.trim())
    } else {
        None
    };
    if let Some(name) = chosen_offline {
        if offline || !instance.home.join("horizon-player.txt").is_file() {
            let _ = std::fs::write(instance.home.join("horizon-player.txt"), format!("{name}\n"));
        }
    }

    // Apply No-AI preference per instance / global.
    let no_ai = if cfg.horizon_override_privacy {
        cfg.horizon_no_ai
    } else {
        prefs.horizon_no_ai
    };
    let no_ai_file = instance.home.join("no-ai");
    if no_ai {
        let _ = std::fs::write(&no_ai_file, "1\n");
    }

    // Create command & apply strict per-instance `HORIZON_HOME` isolation.
    let mut cmd = backend::client_as(&tag, account_who)?;
    inst::apply(&mut cmd, instance)?;
    // Always enforce explicit isolation on `instance.home` so mods/saves never leak across instances.
    cmd.current_dir(&instance.home)
        .env("HORIZON_HOME", &instance.home);

    // Telemetry opt-out.
    let opt_out = if cfg.horizon_override_privacy {
        cfg.horizon_telemetry_opt_out
    } else {
        prefs.horizon_telemetry_opt_out
    };
    if opt_out {
        cmd.env("HORIZON_TELEMETRY_URL", "off");
    }

    // Graphics backend & GPU power preference (`wgpu`).
    let (gpu_backend, gpu_power) = if cfg.horizon_override_graphics {
        (cfg.horizon_gpu_backend, cfg.horizon_gpu_power)
    } else {
        (prefs.horizon_gpu_backend, prefs.horizon_gpu_power)
    };
    if let Some(val) = gpu_backend.env_value() {
        cmd.env("WGPU_BACKEND", val);
    }
    if let Some(val) = gpu_power.env_value() {
        cmd.env("WGPU_POWER_PREF", val);
    }

    // Debug diagnostics (`RUST_BACKTRACE`, `RUST_LOG`).
    if cfg.horizon_rust_backtrace || prefs.horizon_rust_backtrace {
        cmd.env("RUST_BACKTRACE", "1");
    }
    let rust_log = if !cfg.horizon_rust_log.trim().is_empty() {
        Some(cfg.horizon_rust_log.trim())
    } else if !prefs.horizon_rust_log.trim().is_empty() {
        Some(prefs.horizon_rust_log.trim())
    } else {
        None
    };
    if let Some(filter) = rust_log {
        cmd.env("RUST_LOG", filter);
    }

    // Launch mode flags: friend code, world, server, creative, extra CLI flags.
    if !cfg.horizon_friend_code.trim().is_empty() && server.is_none() && world.is_none() {
        backend::join_friend(&mut cmd, cfg.horizon_friend_code.trim())?;
    } else if let Some(ip) = server {
        cmd.arg("--server").arg(ip);
    } else if let Some(w) = world {
        cmd.arg("--world").arg(w);
    }

    if cfg.horizon_start_creative || prefs.horizon_start_creative {
        cmd.arg("--creative");
    }

    for extra in [prefs.horizon_extra_args.trim(), cfg.horizon_extra_args.trim()] {
        if !extra.is_empty() {
            cmd.args(backend::split_args(extra));
        }
    }

    Ok(cmd)
}

/// Merges global & per-instance custom commands, env vars and GPU tweaks.
fn build_effective_launch(instance: &Instance, prefs: &Prefs, cfg: &InstanceCfg) -> launch::Launch {
    let (pre, wrap, post) = if cfg.override_commands {
        (
            cfg.pre_launch_cmd.clone(),
            cfg.wrapper_cmd.clone(),
            cfg.post_exit_cmd.clone(),
        )
    } else if !instance.launch.pre_launch.is_empty()
        || !instance.launch.wrapper.is_empty()
        || !instance.launch.post_exit.is_empty()
    {
        (
            instance.launch.pre_launch.clone(),
            instance.launch.wrapper.clone(),
            instance.launch.post_exit.clone(),
        )
    } else {
        (
            prefs.pre_launch_cmd.clone(),
            prefs.wrapper_cmd.clone(),
            prefs.post_exit_cmd.clone(),
        )
    };

    let mut env: BTreeMap<String, String> = BTreeMap::new();
    let (dgpu, zink) = if cfg.override_tweaks {
        (cfg.prefer_discrete_gpu, cfg.use_zink)
    } else {
        (prefs.prefer_discrete_gpu, prefs.use_zink)
    };
    if dgpu {
        env.insert("SHIM_MCCOMPAT".into(), "0x800000001".into());
        env.insert("__NV_PRIME_RENDER_OFFLOAD".into(), "1".into());
        env.insert("__GLX_VENDOR_LIBRARY_NAME".into(), "nvidia".into());
        env.insert("DRI_PRIME".into(), "1".into());
    }
    if zink {
        env.insert("MESA_LOADER_DRIVER_OVERRIDE".into(), "zink".into());
    }
    if !cfg.override_env {
        for (k, v) in &prefs.global_env {
            if !k.trim().is_empty() {
                env.insert(k.trim().to_owned(), v.clone());
            }
        }
    }
    for (k, v) in &instance.launch.env {
        if !k.trim().is_empty() {
            env.insert(k.trim().to_owned(), v.clone());
        }
    }
    for (k, v) in &cfg.env {
        if !k.trim().is_empty() {
            env.insert(k.trim().to_owned(), v.clone());
        }
    }

    launch::Launch {
        pre_launch: pre,
        wrapper: wrap,
        post_exit: post,
        env,
        dedicated_gpu: dgpu,
        ..instance.launch.clone()
    }
}

fn resolve_horizon_version(
    instance: &Instance,
    prefs: &Prefs,
    cfg: &InstanceCfg,
    emit: &dyn Fn(String),
) -> Result<String, String> {
    let installed = backend::installed();

    // 1. Explicit per-instance pin.
    if cfg.horizon_track == HorizonTrack::Pinned && !cfg.horizon_pinned_version.trim().is_empty() {
        let pin = cfg.horizon_pinned_version.trim().to_owned();
        if installed.contains(&pin) {
            emit(format!("Using pinned Horizon build {pin}."));
            return Ok(pin);
        }
        return install_horizon_tag(&pin, emit);
    }
    if let Some(pin) = instance.version.as_ref().filter(|s| !s.is_empty()) {
        if installed.contains(pin) {
            emit(format!("Using pinned Horizon build {pin}."));
            return Ok(pin.clone());
        }
        return install_horizon_tag(pin, emit);
    }

    // 2. Channel preference (Nightly vs Stable Release).
    let want_nightly = match cfg.horizon_track {
        HorizonTrack::Nightly => true,
        HorizonTrack::Release => false,
        HorizonTrack::Default | HorizonTrack::Pinned => prefs.horizon_prefer_nightly,
    };

    if let Some(matching) = installed
        .iter()
        .find(|t| t.starts_with("nightly") == want_nightly)
    {
        emit(format!("Using installed Horizon build {matching}."));
        return Ok(matching.clone());
    }
    if let Ok(any) = backend::resolve_version(None) {
        emit(format!("Using installed Horizon build {any}."));
        return Ok(any);
    }

    // 3. Download latest build for chosen track from GitHub.
    emit(format!(
        "Fetching Horizon {} channel from GitHub…",
        if want_nightly { "Nightly" } else { "Release" }
    ));
    let releases = backend::releases().map_err(|e| {
        format!("No Horizon build is installed yet, and GitHub could not be reached: {e}")
    })?;
    let picked = releases
        .iter()
        .find(|r| r.asset.is_some() && r.nightly.is_some() == want_nightly && !r.prerelease)
        .or_else(|| backend::latest(&releases))
        .ok_or("No compatible Horizon build was found for this platform.")?;
    let download_tag = picked.nightly.as_deref().unwrap_or(&picked.tag);
    install_horizon_tag(download_tag, emit)
}

fn install_horizon_tag(tag: &str, emit: &dyn Fn(String)) -> Result<String, String> {
    emit(format!("Downloading Horizon {tag}…"));
    let installed = backend::install_tag(tag, false, fraction)?;
    fraction(1.0);
    emit(format!("Installed Horizon {installed}."));
    Ok(installed)
}

/// Prepares Horizon assets from local `.minecraft` if present, automatically falling back to
/// direct Mojang asset download for the signed-in Minecraft owner so fresh FNH setups always work.
fn prepare_horizon_version(tag: &str, emit: &dyn Fn(String)) -> Result<(), String> {
    if backend::check_prepared(tag) {
        return Ok(());
    }
    emit(format!("Preparing Horizon {tag} assets…"));
    horizon_launcher::mojang::CANCEL.store(false, Ordering::Relaxed);

    let mc_dir = backend::minecraft_dir(None)
        .unwrap_or_else(|_| crate::paths::minecraft_dir());
    let (preparer, mc_used) = match backend::preparer(
        tag,
        &mc_dir,
        backend::MinecraftFrom::Install,
        emit,
    ) {
        Ok(ok) => ok,
        Err(_) => {
            emit("Downloading required Minecraft assets from Mojang…".into());
            let own_mc = crate::paths::minecraft_dir();
            let _ = std::fs::create_dir_all(&own_mc);
            backend::preparer(tag, &own_mc, backend::MinecraftFrom::Mojang, emit)?
        }
    };

    backend::sounds(tag, &mc_used, emit);
    backend::set_prepare_step(backend::PrepareStep::Prepare);
    emit("Building Horizon data tables…".into());

    match preparer {
        backend::Preparer::InProcess(options) => backend::prepare_in_process(&options, emit)?,
        backend::Preparer::Process(cmd) => {
            let (tx, rx) = std::sync::mpsc::channel();
            backend::spawn(backend::Job::Prepare, cmd, tx)?;
            loop {
                match rx.recv() {
                    Ok(backend::Msg::Line(backend::Job::Prepare, line)) => emit(line),
                    Ok(backend::Msg::Exit(backend::Job::Prepare, res)) => {
                        res?;
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => return Err("Horizon preparation stopped unexpectedly.".into()),
                }
            }
        }
    }

    backend::forget_prepared();
    if !backend::check_prepared(tag) {
        return Err(format!(
            "{tag} did not finish preparing — check the console log for details."
        ));
    }
    emit(format!("Horizon {tag} is ready."));
    Ok(())
}

/// Pre-downloads & prepares an instance's game files without launching the game (`Download All`).
pub fn prefetch_instance(instance: Instance, prefs: Prefs, cfg: InstanceCfg) {
    if busy() {
        return;
    }
    let max_lines = prefs.console_max_lines;
    let stop_ovf = prefs.console_stop_on_overflow;
    begin(
        Some(instance.id.clone()),
        format!("Downloading {}", instance.name),
        "Checking files…".into(),
    );
    std::thread::spawn(move || {
        let emit = |msg: String| {
            log::line(&msg);
            push_console(&instance.id, msg.clone(), max_lines, stop_ovf);
            step(&format!("Downloading {}", instance.name), msg);
        };
        let res = sign_in(false, Some(&emit)).and_then(|()| {
            if instance.java.is_some() {
                build_java_command(&instance, &prefs, &cfg, None, None, None, false, &emit)
                    .map(|_| format!("{} files are downloaded and ready.", instance.name))
            } else {
                let tag = resolve_horizon_version(&instance, &prefs, &cfg, &emit)?;
                prepare_horizon_version(&tag, &emit)?;
                backend::complete_assets(&tag);
                Ok(format!("{} ({tag}) is downloaded and prepared.", instance.name))
            }
        });
        match res {
            Ok(msg) => {
                push_console(&instance.id, format!("[FNH] {msg}"), max_lines, stop_ovf);
                set(|s| {
                    s.busy = false;
                    s.done = true;
                    s.fraction = None;
                    s.title = "Ready".into();
                    s.line = msg;
                });
            }
            Err(err) => {
                push_console(&instance.id, format!("[ERROR] {err}"), max_lines, stop_ovf);
                set(|s| {
                    s.busy = false;
                    s.failed = true;
                    s.fraction = None;
                    s.title = "Download failed".into();
                    s.line = err;
                });
            }
        }
    });
}

/// Installs the latest Horizon Release or Nightly from `Settings -> Horizon`.
pub fn install_horizon_channel(nightly: bool) {
    if busy() {
        return;
    }
    let label = if nightly { "Nightly" } else { "Release" };
    begin(
        None,
        format!("Installing Horizon {label}"),
        "Contacting GitHub releases…".into(),
    );
    std::thread::spawn(move || {
        let emit = |msg: String| {
            log::line(&msg);
            step(&format!("Installing Horizon {label}"), msg);
        };
        let res = (|| -> Result<String, String> {
            let releases = backend::releases()?;
            let picked = releases
                .iter()
                .find(|r| r.asset.is_some() && r.nightly.is_some() == nightly && !r.prerelease)
                .or_else(|| backend::latest(&releases))
                .ok_or("No matching Horizon release found for this platform.")?;
            let tag = picked.nightly.as_deref().unwrap_or(&picked.tag);
            let installed = install_horizon_tag(tag, &emit)?;
            Ok(format!("Installed Horizon {installed}."))
        })();
        match res {
            Ok(msg) => set(|s| {
                s.busy = false;
                s.done = true;
                s.fraction = None;
                s.title = "Horizon updated".into();
                s.line = msg;
            }),
            Err(err) => set(|s| {
                s.busy = false;
                s.failed = true;
                s.fraction = None;
                s.title = "Horizon download failed".into();
                s.line = err;
            }),
        }
    });
}

/// Downloads a Mojang Java runtime (e.g. `"21"`, `"17"`, `"8"`) from `Settings -> Java`.
pub fn download_mojang_java(major: String) {
    if busy() {
        return;
    }
    begin(
        None,
        format!("Downloading Java {major}"),
        "Fetching Mojang runtime manifest…".into(),
    );
    std::thread::spawn(move || {
        let res = horizon_launcher::java::Need::named(&major).and_then(|need| {
            horizon_launcher::java::install(&need, &|line| {
                log::line(&line);
                step(&format!("Downloading Java {major}"), line);
            })
        });
        match res {
            Ok(rt) => set(|s| {
                s.busy = false;
                s.done = true;
                s.fraction = None;
                s.title = format!("Java {} installed", rt.version);
                s.line = format!("Installed Mojang Java {} at {}.", rt.version, rt.home.display());
            }),
            Err(err) => set(|s| {
                s.busy = false;
                s.failed = true;
                s.fraction = None;
                s.title = "Java download failed".into();
                s.line = err;
            }),
        }
    });
}

/// Tests a Java installation by executing `java -version` and returning its output line.
pub fn test_java_installation(path_or_home: &str) -> Result<String, String> {
    let trimmed = path_or_home.trim().trim_matches('"');
    let java_bin: PathBuf = if trimmed.is_empty() {
        if let Some(rt) = horizon_launcher::java::mojang_runtimes(&crate::paths::java_dir())
            .into_iter()
            .chain(horizon_launcher::java::installed())
            .max_by_key(|r| r.major)
        {
            rt.home
                .join("bin")
                .join(if cfg!(windows) { "java.exe" } else { "java" })
        } else {
            PathBuf::from(if cfg!(windows) { "java.exe" } else { "java" })
        }
    } else {
        let home = crate::prefs::normalize_java_home(trimmed)
            .ok_or_else(|| "Enter a valid Java path.".to_owned())?;
        let candidate = home
            .join("bin")
            .join(if cfg!(windows) { "java.exe" } else { "java" });
        if candidate.is_file() {
            candidate
        } else {
            PathBuf::from(trimmed)
        }
    };

    let out = process::command(&java_bin)
        .arg("-version")
        .output()
        .map_err(|e| format!("Could not run {}: {e}", java_bin.display()))?;
    let combined = format!(
        "{}\n{}",
        String::from_utf8_lossy(&out.stderr),
        String::from_utf8_lossy(&out.stdout)
    );
    let lines: Vec<&str> = combined
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .take(2)
        .collect();
    if out.status.success() && !lines.is_empty() {
        Ok(format!("Java OK: {}", lines.join(" — ")))
    } else if !lines.is_empty() {
        Err(format!("Java check failed: {}", lines.join(" ")))
    } else {
        Err(format!("Java exited with {}.", out.status))
    }
}

/// Microsoft device-code sign-in; skipped on launch when an owning account is already present.
fn sign_in(force: bool, emit: Option<&dyn Fn(String)>) -> Result<(), String> {
    if !force {
        let accounts = account::Accounts::load();
        if !accounts.accounts.is_empty() && accounts.has_owner(account::store()) {
            return Ok(());
        }
    }
    log::line("Signing in with Microsoft.");
    if let Some(cb) = emit {
        cb("Signing in with Microsoft — requesting device code…".into());
    }
    step(
        "Signing in with Microsoft",
        "Requesting device code from Microsoft…".into(),
    );
    let flow = SignIn::start();
    let mut opened = false;
    loop {
        if KILL_REQUESTED.load(Ordering::Acquire) {
            return Err("Sign-in cancelled.".into());
        }
        match flow.state() {
            SignInState::Starting => {}
            SignInState::Code(code) => {
                let (user_code, link) = (code.user_code.clone(), code.verification_uri.clone());
                set(|s| {
                    s.title = "Microsoft Sign-In".into();
                    s.line = format!("Enter code {user_code} at {link} (copied to clipboard).");
                    s.code = Some((user_code.clone(), link.clone()));
                });
                if !opened {
                    opened = true;
                    let url = log::sign_in_url(&link, &user_code);
                    log::line(&format!("Signing in at {url} with code {user_code}."));
                    let _ = log::copy(&user_code);
                    let _ = log::open_url(&url);
                }
            }
            SignInState::Working(phase) => {
                step("Signing in with Microsoft", phase.label().to_owned());
            }
            SignInState::Done(Ok(acc)) => {
                log::line(&format!("Signed in as {}.", acc.name));
                if let Some(cb) = emit {
                    cb(format!("Signed in as {}.", acc.name));
                }
                return Ok(());
            }
            SignInState::Done(Err(e)) => return Err(e),
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Starts a standalone Microsoft sign-in from Accounts.
pub fn sign_in_now() {
    if busy() {
        return;
    }
    begin(
        None,
        "Signing in with Microsoft".into(),
        "Requesting device code from Microsoft…".into(),
    );
    std::thread::spawn(|| match sign_in(true, None) {
        Ok(()) => {
            let name = account::Accounts::load()
                .selected()
                .map(|a| a.name.clone())
                .unwrap_or_default();
            set(|s| {
                s.busy = false;
                s.done = true;
                s.code = None;
                s.title = "Signed in".into();
                s.line = format!("Signed in as {name}.");
            });
        }
        Err(err) => {
            log::line(&format!("Sign-in failed: {err}"));
            set(|s| {
                s.busy = false;
                s.failed = true;
                s.code = None;
                s.title = "Sign-in failed".into();
                s.line = err;
            });
        }
    });
}

fn describe_command(cmd: &Command) -> String {
    let mut parts = vec![format!("{:?}", cmd.get_program())];
    let mut redact = false;
    for arg in cmd.get_args() {
        if redact {
            parts.push("\"<redacted>\"".into());
            redact = false;
            continue;
        }
        let s = arg.to_string_lossy();
        if s == "--accessToken" {
            redact = true;
        }
        parts.push(format!("{arg:?}"));
    }
    parts.join(" ")
}

fn clock_hms() -> String {
    let secs = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs() % 86_400)
        .unwrap_or(0);
    format!(
        "[{:02}:{:02}:{:02}]",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}
