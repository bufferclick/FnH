//! Complete factory reset (`Ctrl + Shift + Alt` while clicking `Add Instance...`): removes every
//! account, instance, world, mod, cached version, preference and log FNH holds, then starts a
//! fresh copy of `FH.exe` and closes the current window so the launcher comes back like a brand
//! new install.

use crate::{job, log};
use gpui::{ClickEvent, Modifiers};
use horizon_launcher::{account, backend, data_location};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

/// Hidden command-line flag passed to the relaunched process so it can remove any file handle
/// (such as `location.lock`) that only unlocked once the old process exited.
pub const AFTER_RESET_FLAG: &str = "--after-reset";

/// Real files or folders created inside `game_home` once the default Horizon instance has been
/// configured or played; their absence (with no `"horizon"` key in `instances.json`) means the
/// default folder is untouched and should not appear as a phantom instance on a fresh install.
const DEFAULT_HOME_MARKERS: &[&str] = &[
    "options.txt",
    "servers.dat",
    "horizon-extras.txt",
    "saves",
    "mods",
    "logs",
    "resourcepacks",
    "shaderpacks",
    "crash-reports",
    "screenshots",
    "backups",
];

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

/// Whether the synthetic default `"Horizon"` entry from `instances::list` actually exists on disk
/// or in `instances.json` (false on a fresh install or after a factory reset).
pub fn has_default_horizon_data(data_dir: &Path, game_home: &Path) -> bool {
    if let Ok(text) = std::fs::read_to_string(data_dir.join("instances.json")) {
        if text.contains("\"horizon\": {") || text.contains("\"horizon\":{") {
            return true;
        }
    }
    DEFAULT_HOME_MARKERS
        .iter()
        .any(|entry| game_home.join(entry).exists())
}

/// Removes the default `"horizon"` instance's files and `instances.json` entry while keeping any
/// sibling instances stored under `game_home/instances/`.
pub fn delete_default_horizon() -> Result<(), String> {
    delete_default_horizon_in(&backend::data_dir(), &backend::game_home())
}

fn delete_default_horizon_in(data_dir: &Path, game_home: &Path) -> Result<(), String> {
    for marker in DEFAULT_HOME_MARKERS {
        remove_target(&game_home.join(marker));
    }
    let store = data_dir.join("instances.json");
    if let Ok(text) = std::fs::read_to_string(&store) {
        let updated = remove_horizon_key(&text);
        std::fs::write(&store, updated).map_err(|e| format!("instances.json: {e}"))?;
    }
    Ok(())
}

/// Strips the `"horizon": { ... }` property from a pretty-printed `instances.json` document while
/// leaving every other instance entry intact.
fn remove_horizon_key(json: &str) -> String {
    let needle = "\"horizon\"";
    let bytes = json.as_bytes();
    let Some(key_start) = json.find(needle) else {
        return json.to_owned();
    };
    let mut i = key_start + needle.len();
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if bytes.get(i) != Some(&b':') {
        return json.to_owned();
    }
    i += 1;
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    if bytes.get(i) != Some(&b'{') {
        return json.to_owned();
    }
    let mut depth = 0usize;
    let mut in_str = false;
    let mut escape = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_str {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_str = false;
            }
        } else {
            match b {
                b'"' => in_str = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        i += 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }
    let mut start = key_start;
    while start > 0 && matches!(bytes[start - 1], b' ' | b'\t' | b'\r' | b'\n') {
        start -= 1;
    }
    let mut end = i;
    while end < bytes.len() && matches!(bytes[end], b' ' | b'\t') {
        end += 1;
    }
    if bytes.get(end) == Some(&b',') {
        end += 1;
    } else if start > 0 && bytes[start - 1] == b',' {
        start -= 1;
    }
    let mut out = String::with_capacity(json.len());
    out.push_str(&json[..start]);
    out.push_str(&json[end..]);
    out
}

/// Guard against ever deleting a filesystem root, drive root or bare user profile directory.
pub fn safe_to_remove(path: &Path) -> bool {
    let depth = path
        .components()
        .filter(|c| matches!(c, std::path::Component::Normal(_)))
        .count();
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return false;
    };
    depth >= 2 && !name.is_empty() && name != "." && name != ".."
}

/// Every directory or file FNH's data lives in on this computer, deduplicated.
pub fn collect_targets() -> Vec<PathBuf> {
    let mut targets: Vec<PathBuf> = Vec::new();
    let mut push = |candidate: PathBuf| {
        if safe_to_remove(&candidate) && !targets.iter().any(|existing| existing == &candidate) {
            targets.push(candidate);
        }
    };

    let data = backend::data_dir();
    let had_data = data.exists();
    push(data);
    push(backend::game_home());
    if had_data {
        push(backend::build_dir());
    }

    if let Ok(Some(roots)) = data_location::configured_roots() {
        push(roots.launcher);
        push(roots.game);
        push(roots.build);
    }

    let isolated = std::env::var_os("HORIZON_LAUNCHER_HOME").is_some_and(|v| !v.is_empty())
        || std::env::var_os("HORIZON_HOME").is_some_and(|v| !v.is_empty());

    if !isolated {
        if let Some(appdata) = std::env::var_os("APPDATA").filter(|v| !v.is_empty()) {
            let roaming = PathBuf::from(appdata);
            push(roaming.join("horizon-launcher"));
            push(roaming.join("horizon"));
            push(roaming.join("horizon-launcher-location"));
        }
    }

    if let Ok(bootstrap) = data_location::bootstrap_path() {
        let custom_pointer =
            std::env::var_os("HORIZON_LAUNCHER_LOCATION_FILE").is_some_and(|v| !v.is_empty());
        if !isolated || custom_pointer {
            if let Some(parent) = bootstrap.parent() {
                if parent
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n == "horizon-launcher-location")
                {
                    push(parent.to_path_buf());
                } else {
                    push(bootstrap);
                }
            }
        }
    }

    targets
}

/// Clears read-only flags under `path` so Windows permits deleting extracted Java or mod files.
fn make_writable(path: &Path) {
    if let Ok(meta) = std::fs::symlink_metadata(path) {
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
}

/// Removes a single target file or folder tree, clearing read-only attributes if needed.
pub fn remove_target(path: &Path) {
    if !safe_to_remove(path) {
        return;
    }
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return;
    };
    if meta.is_dir() {
        if std::fs::remove_dir_all(path).is_err() {
            make_writable(path);
            let _ = std::fs::remove_dir_all(path);
        }
    } else if std::fs::remove_file(path).is_err() {
        make_writable(path);
        let _ = std::fs::remove_file(path);
    }
}

/// Removes every path in `targets`.
pub fn wipe_paths(targets: &[PathBuf]) {
    for target in targets {
        remove_target(target);
    }
}

/// Removes every stored Microsoft account from the launcher's secret stores and session files.
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

/// Deletes all FNH & Horizon data on this computer and spawns a fresh instance of `FH.exe`.
/// Caller immediately quits the current GPUI app after `Ok(())`.
pub fn wipe_and_relaunch() -> Result<(), String> {
    let exe = std::env::current_exe()
        .map_err(|e| format!("Could not locate the running executable to relaunch: {e}"))?;

    // Stop any launched game first so it releases world and log files.
    if job::kill() {
        std::thread::sleep(Duration::from_millis(250));
    }
    job::clear();

    // Sign out of every stored account (clears keychain credentials as well as files).
    wipe_accounts();

    let targets = collect_targets();

    // Release our own handle on `fh/fh.log` before deleting the data directory.
    log::close();

    wipe_paths(&targets);

    let mut command = std::process::Command::new(&exe);
    command
        .arg(AFTER_RESET_FLAG)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = exe.parent().filter(|dir| dir.is_dir()) {
        command.current_dir(dir);
    }
    command
        .spawn()
        .map_err(|e| format!("Data cleared, but could not relaunch {}: {e}", exe.display()))?;
    Ok(())
}

/// Runs at the very start of the relaunched `FH.exe` process so any file locked by the exiting
/// parent process (such as `location.lock` or `fh.log`) is swept away before startup begins.
pub fn finish_post_reset_sweep() {
    let targets = collect_targets();
    for attempt in 0..10 {
        wipe_paths(&targets);
        if targets.iter().all(|path| !path.exists()) {
            break;
        }
        if attempt < 9 {
            std::thread::sleep(Duration::from_millis(60));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{has_default_horizon_data, is_reset_chord, safe_to_remove, wipe_paths};
    use gpui::Modifiers;
    use std::path::Path;

    #[test]
    fn reset_chord_requires_all_three_modifiers() {
        let all = Modifiers {
            control: true,
            alt: true,
            shift: true,
            ..Modifiers::default()
        };
        assert!(is_reset_chord(all));
        assert!(!is_reset_chord(Modifiers {
            alt: false,
            ..all
        }));
        assert!(!is_reset_chord(Modifiers {
            shift: false,
            ..all
        }));
        assert!(!is_reset_chord(Modifiers {
            control: false,
            ..all
        }));
    }

    #[test]
    fn wipe_removes_instances_preferences_logs_and_readonly_files() {
        let root = std::env::temp_dir().join(format!("fh-reset-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let data = root.join("horizon-launcher");
        let game = root.join("horizon");

        std::fs::create_dir_all(data.join("fh")).unwrap();
        std::fs::create_dir_all(game.join("instances").join("fabric-1.21.4").join("mods")).unwrap();
        std::fs::write(data.join("instances.json"), r#"{"instances":{"horizon":{}}}"#).unwrap();
        std::fs::write(data.join("fh").join("ui.cfg"), "ApplicationTheme=windows11\n").unwrap();
        let ro = game
            .join("instances")
            .join("fabric-1.21.4")
            .join("mods")
            .join("fabric-api.jar");
        std::fs::write(&ro, b"jar").unwrap();
        let mut perms = std::fs::metadata(&ro).unwrap().permissions();
        perms.set_readonly(true);
        let _ = std::fs::set_permissions(&ro, perms);

        assert!(has_default_horizon_data(&data, &game));
        assert!(safe_to_remove(&data));
        assert!(!safe_to_remove(Path::new("/")));

        wipe_paths(&[data.clone(), game.clone()]);
        assert!(!data.exists());
        assert!(!game.exists());
        assert!(!has_default_horizon_data(&data, &game));

        // Creating only an own instance under `game/instances/` does not resurrect default Horizon.
        std::fs::create_dir_all(game.join("instances").join("my-fabric")).unwrap();
        assert!(!has_default_horizon_data(&data, &game));

        // Real Horizon data (e.g. `saves/`) makes the default Horizon instance visible again.
        std::fs::create_dir_all(game.join("saves")).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(
            data.join("instances.json"),
            "{\n  \"instances\": {\n    \"horizon\": {\n      \"name\": \"Horizon\"\n    },\n    \"my-fabric\": {\n      \"name\": \"My Fabric\"\n    }\n  }\n}",
        )
        .unwrap();
        assert!(has_default_horizon_data(&data, &game));

        // Deleting default Horizon keeps sibling instances untouched.
        super::delete_default_horizon_in(&data, &game).unwrap();
        assert!(!has_default_horizon_data(&data, &game));
        assert!(game.join("instances").join("my-fabric").is_dir());
        let kept = std::fs::read_to_string(data.join("instances.json")).unwrap();
        assert!(kept.contains("\"my-fabric\""));

        let _ = std::fs::remove_dir_all(&root);
    }
}
