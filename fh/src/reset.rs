//! Bulletproof factory reset (`Ctrl + Shift + Alt` while clicking `Add Instance...`).
//!
//! Why two phases:
//! Deleting gigabytes of Minecraft assets, Java runtimes and Horizon builds on the UI thread or
//! while `location.lock` is held hangs or fails on Windows. Instead:
//! 1. The running window stops any launched game, signs out all FNH accounts, closes `fh.log`,
//!    spawns a detached `FH.exe --factory-reset` worker (with `%TEMP%` as its working directory so
//!    nothing holds a handle on `<FnH>`), and exits immediately.
//! 2. The worker waits for the parent process to release `location.lock`, atomically moves every
//!    item inside `<FnH>` into a staging wipe directory (milliseconds on NTFS), launches a brand
//!    new `FH.exe --after-reset` immediately so FNH reopens cleanly as a fresh install, and then
//!    deletes all staged files in the background before exiting.

use crate::{job, log, paths};
use gpui::{ClickEvent, Modifiers};
use horizon_launcher::{account, process};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Worker CLI flag: waits for parent exit, wipes `<FnH>`, launches fresh `FH.exe`, cleans stage.
pub const WORKER_FLAG: &str = "--factory-reset";

/// Marker flag passed to the fresh GUI instance strictly for logging that reset completed.
pub const AFTER_RESET_FLAG: &str = "--after-reset";

/// Prefix used for temporary sibling wipe directories if cross-volume move falls back.
const WIPE_PREFIX: &str = ".fnh-wipe-";

/// Whether `modifiers` holds `Alt + Shift + Ctrl` together.
pub fn is_reset_chord(modifiers: Modifiers) -> bool {
    modifiers.alt && modifiers.shift && modifiers.control
}

/// Whether a click on `Add Instance...` was made while holding `Alt + Shift + Ctrl`.
pub fn is_reset_click(event: &ClickEvent) -> bool {
    match event {
        ClickEvent::Mouse(click) => {
            is_reset_chord(click.down.modifiers) || is_reset_chord(click.up.modifiers)
        }
        _ => false,
    }
}

/// Guard against ever deleting a filesystem root, drive root or bare user profile directory.
pub fn safe_to_remove(path: &Path) -> bool {
    let depth = path
        .components()
        .filter(|c| matches!(c, Component::Normal(_)))
        .count();
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    depth >= 2 && !name.is_empty() && name != "." && name != ".."
}

/// Clears read-only flags recursively so Windows allows deleting extracted JDK/mod files.
fn make_writable(path: &Path) {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    let mut perms = meta.permissions();
    if perms.readonly() {
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        let _ = std::fs::set_permissions(path, perms);
    }
    if meta.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                make_writable(&entry.path());
            }
        }
    }
}

/// Deletes a single file or directory tree with read-only stripping and brief Windows retry.
pub fn remove_target(path: &Path) {
    if !safe_to_remove(path) {
        return;
    }
    for attempt in 0..5 {
        let Ok(meta) = std::fs::symlink_metadata(path) else {
            return;
        };
        let ok = if meta.is_dir() {
            std::fs::remove_dir_all(path).is_ok() || {
                make_writable(path);
                std::fs::remove_dir_all(path).is_ok()
            }
        } else {
            std::fs::remove_file(path).is_ok() || {
                make_writable(path);
                std::fs::remove_file(path).is_ok()
            }
        };
        if ok || !path.exists() {
            return;
        }
        if attempt < 4 {
            std::thread::sleep(Duration::from_millis(80));
        }
    }
}

/// Signs out every Microsoft account stored in FNH's `accounts.json` and clears session files.
fn wipe_accounts() {
    let mut accounts = account::Accounts::load();
    let uuids: Vec<String> = accounts.accounts.iter().map(|a| a.uuid.clone()).collect();
    for uuid in uuids {
        let _ = accounts.remove(account::store(), &uuid);
        let _ = accounts.remove(account::keychain(), &uuid);
    }
    let _ = accounts.save();
    account::discard_unread_sessions();
}

/// Spawns the detached reset worker (`FH.exe --factory-reset`) without blocking the UI thread.
/// Once this returns `Ok(())`, the caller exits the process immediately so all file locks drop.
pub fn spawn_reset_worker() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not locate FH.exe to relaunch: {e}"))?;

    // Terminate any running game child first so world/jar handles close.
    let _ = job::kill();
    job::clear();

    // Sign out of stored accounts while our environment is configured.
    wipe_accounts();

    // Flush and close `fh.log` before handing off to the worker.
    log::close();

    let safe_cwd = std::env::temp_dir();
    let mut cmd = process::command(&exe);
    cmd.arg(WORKER_FLAG)
        .current_dir(&safe_cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    cmd.spawn()
        .map_err(|e| format!("Could not start reset worker ({}): {e}", exe.display()))?;
    Ok(())
}

/// Waits up to ~4 seconds until the exiting parent process releases `location.lock` and `fh.log`.
fn wait_for_parent_unlock(root: &Path) {
    std::thread::sleep(Duration::from_millis(180));
    let lock_candidates = [
        root.join("launcher").join("location.lock"),
        root.join("logs").join("fh.log"),
        root.join("launcher").join("fh").join("fh.log"),
    ];
    for _ in 0..40 {
        let mut busy = false;
        for lock in &lock_candidates {
            if lock.is_file() && std::fs::remove_file(lock).is_err() && lock.exists() {
                busy = true;
                break;
            }
        }
        if !busy {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Moves every entry inside `root` (never touching `protected_exe` if nested) into a temporary
/// staging folder so `root` becomes completely empty in milliseconds, returning staged paths to
/// delete afterwards.
pub fn stage_and_empty_dir(root: &Path, protected_exe: Option<&Path>) -> Vec<PathBuf> {
    if !safe_to_remove(root) || !root.is_dir() {
        return Vec::new();
    }
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let pid = std::process::id();
    let temp_stage = std::env::temp_dir().join(format!("fnh-reset-{pid}-{stamp}"));
    let _ = std::fs::create_dir_all(&temp_stage);

    let mut trash: Vec<PathBuf> = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return trash;
    };

    for (idx, entry) in entries.flatten().enumerate() {
        let path = entry.path();
        if let Some(exe) = protected_exe {
            if same_or_contains(&path, exe) {
                continue;
            }
        }
        // Try instant move into %TEMP% first; if cross-volume, rename beside root's parent.
        let stage_dst = temp_stage.join(format!("item-{idx}"));
        if std::fs::rename(&path, &stage_dst).is_ok() {
            continue;
        }
        if let Some(parent) = root.parent() {
            let sibling = parent.join(format!("{WIPE_PREFIX}{pid}-{stamp}-{idx}"));
            if std::fs::rename(&path, &sibling).is_ok() {
                trash.push(sibling);
                continue;
            }
        }
        // Last resort: delete in place before launching the fresh window.
        remove_target(&path);
    }

    if temp_stage.exists() {
        trash.push(temp_stage);
    }
    trash
}

fn same_or_contains(dir_or_file: &Path, target: &Path) -> bool {
    #[cfg(windows)]
    {
        let a = dir_or_file.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
        let b = target.to_string_lossy().replace('/', "\\").to_ascii_lowercase();
        b == a || b.starts_with(&format!("{a}\\"))
    }
    #[cfg(not(windows))]
    {
        target == dir_or_file || target.starts_with(dir_or_file)
    }
}

/// Sweeps any leftover sibling `.fnh-wipe-*` folders from an interrupted earlier reset.
pub fn sweep_stale_stages(root: &Path) {
    let Some(parent) = root.parent() else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(parent) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|s| s.starts_with(WIPE_PREFIX))
        {
            remove_target(&entry.path());
        }
    }
}

/// Runs inside `FH.exe --factory-reset`: owns zero locks, wipes all FNH data, immediately opens a
/// fresh `FH.exe`, then finishes deleting staged trees before exiting.
pub fn run_worker() -> i32 {
    let root = paths::root();
    let exe = std::env::current_exe().ok();

    wait_for_parent_unlock(&root);

    // Collect every FNH directory (including any overridden launcher/game/build roots).
    let mut roots = vec![root.clone()];
    for env_key in [
        "HORIZON_LAUNCHER_HOME",
        "HORIZON_HOME",
        "HORIZON_BUILD_DIR",
    ] {
        if let Some(val) = std::env::var_os(env_key).filter(|v| !v.is_empty()) {
            let p = PathBuf::from(val);
            if !same_or_contains(&root, &p) && safe_to_remove(&p) && !roots.contains(&p) {
                roots.push(p);
            }
        }
    }

    let mut trash = Vec::new();
    for dir in &roots {
        trash.extend(stage_and_empty_dir(dir, exe.as_deref()));
        // Remove the empty root itself if nothing protected sits inside it, then recreate clean.
        if exe.as_deref().is_none_or(|e| !same_or_contains(dir, e)) {
            let _ = std::fs::remove_dir(dir);
        }
    }

    // Relaunch a brand-new clean FNH window right away so reopening feels instantaneous.
    if let Some(exe) = &exe {
        let mut fresh = process::command(exe);
        fresh
            .arg(AFTER_RESET_FLAG)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(dir) = exe.parent().filter(|d| d.is_dir()) {
            fresh.current_dir(dir);
        }
        let _ = fresh.spawn();
    }

    // Delete all staged files completely before exiting the worker.
    for item in trash {
        remove_target(&item);
    }
    sweep_stale_stages(&root);
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reset_chord_requires_all_three_modifiers() {
        let full = Modifiers {
            control: true,
            alt: true,
            shift: true,
            ..Default::default()
        };
        assert!(is_reset_chord(full));
        assert!(!is_reset_chord(Modifiers {
            control: false,
            ..full
        }));
        assert!(!is_reset_chord(Modifiers { alt: false, ..full }));
        assert!(!is_reset_chord(Modifiers {
            shift: false,
            ..full
        }));
    }

    #[test]
    fn safe_to_remove_rejects_shallow_and_empty_paths() {
        assert!(!safe_to_remove(Path::new("")));
        assert!(!safe_to_remove(Path::new("/")));
        assert!(!safe_to_remove(Path::new("/home")));
        assert!(safe_to_remove(Path::new("/home/user/FnH")));
    }

    #[test]
    fn stage_and_empty_wipes_all_data_while_protecting_portable_exe() {
        let base = std::env::temp_dir().join(format!("fnh-reset-unit-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&base);
        let root = base.join("FnH");
        std::fs::create_dir_all(root.join("game/instances/fabric-1.21/mods")).unwrap();
        std::fs::create_dir_all(root.join("launcher/versions/nightly")).unwrap();
        std::fs::create_dir_all(root.join("icons")).unwrap();
        std::fs::write(root.join("settings.json"), "{}").unwrap();
        std::fs::write(root.join("game/instances/fabric-1.21/mods/sodium.jar"), b"jar").unwrap();
        let fake_exe = root.join("FH.exe");
        std::fs::write(&fake_exe, b"exe").unwrap();

        let trash = stage_and_empty_dir(&root, Some(&fake_exe));
        assert!(!root.join("settings.json").exists());
        assert!(!root.join("game").exists());
        assert!(!root.join("launcher").exists());
        assert!(!root.join("icons").exists());
        assert!(fake_exe.is_file(), "portable FH.exe must never be deleted");

        for item in trash {
            remove_target(&item);
        }
        let _ = std::fs::remove_dir_all(&base);
    }
}
