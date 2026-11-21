//! FNH's isolated data directories (`%APPDATA%\FnH` on Windows, `~/.local/share/FnH` on Linux,
//! `~/Library/Application Support/FnH` on macOS, or `<exe_dir>/data` when `portable.txt` sits
//! beside `FH.exe`).
//!
//! Keeps all FNH instances, mods, accounts, Horizon builds, Fabric profiles, icons, logs and
//! settings strictly separate from any standalone Horizon Launcher or Prism/JLac installation.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static ROOT: OnceLock<PathBuf> = OnceLock::new();

/// Computes and registers FNH's data root before any `horizon_launcher` call runs.
pub fn init() -> &'static Path {
    ROOT.get_or_init(|| {
        let root = resolve_root();
        let _ = std::fs::create_dir_all(&root);
        set_default_env("HORIZON_LAUNCHER_HOME", &root.join("launcher"));
        set_default_env("HORIZON_HOME", &root.join("game"));
        set_default_env("HORIZON_BUILD_DIR", &root.join("build"));
        set_default_env("HORIZON_LAUNCHER_LOCATION_FILE", &root.join("location.json"));
        root
    })
    .as_path()
}

fn resolve_root() -> PathBuf {
    if let Some(custom) = std::env::var_os("FNH_HOME").filter(|v| !v.is_empty()) {
        return PathBuf::from(custom);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join("portable.txt").is_file() {
                return dir.join("data");
            }
        }
    }
    dirs::data_dir()
        .or_else(dirs::config_dir)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("FnH")
}

fn set_default_env(key: &str, value: &Path) {
    if std::env::var_os(key).is_none_or(|v| v.is_empty()) {
        std::env::set_var(key, value);
    }
}

/// Root folder holding all FNH data (`%APPDATA%\FnH` by default).
pub fn root() -> PathBuf {
    ROOT.get().cloned().unwrap_or_else(resolve_root)
}

/// Internal launcher store (`accounts.json`, `instances.json`, `versions/`, `minecraft/`).
pub fn launcher_dir() -> PathBuf {
    horizon_launcher::backend::data_dir()
}

/// Base game root (individual instances live under `instances_dir()`).
pub fn game_root() -> PathBuf {
    horizon_launcher::backend::game_home()
}

/// Folder containing all created Fabric and Horizon instance folders (`<FnH>/game/instances`).
pub fn instances_dir() -> PathBuf {
    let dir = game_root().join("instances");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Folder containing custom user instance icons (`<FnH>/icons`).
pub fn icons_dir() -> PathBuf {
    let dir = root().join("icons");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Optional template folder copied into newly created instances when enabled (`<FnH>/template`).
pub fn template_dir() -> PathBuf {
    let dir = root().join("template");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Launcher logs folder (`<FnH>/logs`).
pub fn logs_dir() -> PathBuf {
    let dir = root().join("logs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Ephemeral download & self-update staging folder (`<FnH>/cache`).
pub fn cache_dir() -> PathBuf {
    let dir = root().join("cache");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Auto-downloaded Mojang Java runtimes folder.
pub fn java_dir() -> PathBuf {
    let dir = horizon_launcher::java::runtimes_dir();
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Shared Minecraft jars, libraries and assets folder (`<FnH>/launcher/minecraft`).
pub fn minecraft_dir() -> PathBuf {
    let dir = launcher_dir().join("minecraft");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Installed Horizon engine builds folder (`<FnH>/launcher/versions`).
pub fn versions_dir() -> PathBuf {
    let dir = launcher_dir().join("versions");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

/// Global settings file (`<FnH>/settings.json`).
pub fn settings_file() -> PathBuf {
    root().join("settings.json")
}

/// Per-instance overrides, custom icon bindings and Horizon settings (`<FnH>/instances_cfg.json`).
pub fn instance_cfg_file() -> PathBuf {
    root().join("instances_cfg.json")
}

/// Copies all files and subdirectories from `<FnH>/template` into a freshly created instance.
pub fn apply_template(instance_home: &Path) {
    let tpl = root().join("template");
    if !tpl.is_dir() {
        return;
    }
    copy_tree_missing(&tpl, instance_home);
}

fn copy_tree_missing(from: &Path, to: &Path) {
    let Ok(entries) = std::fs::read_dir(from) else {
        return;
    };
    let _ = std::fs::create_dir_all(to);
    for entry in entries.flatten() {
        let src = entry.path();
        let dst = to.join(entry.file_name());
        if src.is_dir() {
            copy_tree_missing(&src, &dst);
        } else if src.is_file() && !dst.exists() {
            let _ = std::fs::copy(&src, &dst);
        }
    }
}

/// Clears cached version manifests and mod-browser metadata, returning the count of removed items.
pub fn clear_metadata_cache() -> usize {
    let mut removed = 0usize;
    let launcher = launcher_dir();
    for rel in [
        "minecraft/version_manifest_v2.json",
        "mod-browser",
        "fabric-loader-cache",
        "links.json",
        "releases.json",
    ] {
        let p = launcher.join(rel);
        if p.is_dir() && std::fs::remove_dir_all(&p).is_ok() {
            removed += 1;
        } else if p.is_file() && std::fs::remove_file(&p).is_ok() {
            removed += 1;
        }
    }
    removed
}
