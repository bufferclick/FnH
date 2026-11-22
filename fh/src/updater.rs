//! Self-updater for FNH (`FH.exe`): checks GitHub Releases (`bufferclick/FnH`) on startup and on
//! demand, prompts *"Do you want to update FnH?"* whenever a newer release (`v1.6` -> `v1.7`, etc.)
//! is published, downloads `FnHLauncher.zip`, extracts the new `FH.exe`, deletes the downloaded
//! `.zip`, swaps the executable cleanly on Windows, and restarts into the updated launcher.

use crate::log;
use serde::Deserialize;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

/// Latest release endpoint on GitHub's public API.
pub const LATEST_API_URL: &str = "https://api.github.com/repos/bufferclick/FnH/releases/latest";
/// Canonical direct download URL for the renamed release zip.
pub const FALLBACK_ZIP_URL: &str =
    "https://github.com/bufferclick/FnH/releases/latest/download/FnHLauncher.zip";
/// Canonical release archive filename (`FnHLauncher.zip`).
pub const PREFERRED_ZIP_NAME: &str = "FnHLauncher.zip";

/// Metadata for an available (or current) GitHub release of FNH.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReleaseUpdate {
    /// Normalized version string (`"1.7"`, `"1.8"`, ...).
    pub version: String,
    /// Git tag (`"fnh-v1.7"`).
    pub tag: String,
    /// Human-readable release title (`"FNH v1.7"`).
    pub title: String,
    /// Markdown / plain release notes body.
    pub notes: String,
    /// Browser URL for the release page.
    pub html_url: String,
    /// Chosen zip asset filename (`"FnHLauncher.zip"`).
    pub zip_name: String,
    /// Direct HTTPS download URL for the zip archive.
    pub zip_url: String,
    /// Asset byte size if reported by GitHub.
    pub size_bytes: Option<u64>,
    /// True when `version` is strictly newer than `crate::VERSION`.
    pub newer_than_current: bool,
}

/// Live state of the FNH self-updater.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpdateState {
    Idle,
    Checking,
    UpToDate {
        current: String,
    },
    Available(ReleaseUpdate),
    Downloading {
        version: String,
        percent: u8,
        message: String,
    },
    Applying {
        version: String,
    },
    Failed(String),
}

struct SharedUpdater {
    state: UpdateState,
    /// Populated once when a background check discovers a newer release so `MainWindow` can pop open
    /// the *"Do you want to update FnH?"* prompt automatically.
    pending_prompt: Option<ReleaseUpdate>,
    /// Version dismissed via *"Not Now"* during this session so we don't nag repeatedly.
    dismissed_version: Option<String>,
}

static UPDATER: OnceLock<Mutex<SharedUpdater>> = OnceLock::new();

fn shared() -> &'static Mutex<SharedUpdater> {
    UPDATER.get_or_init(|| {
        Mutex::new(SharedUpdater {
            state: UpdateState::Idle,
            pending_prompt: None,
            dismissed_version: None,
        })
    })
}

/// Current updater state snapshot.
pub fn state() -> UpdateState {
    shared()
        .lock()
        .map(|s| s.state.clone())
        .unwrap_or(UpdateState::Idle)
}

/// Takes any newly discovered update that should pop up *"Do you want to update FnH?"* on startup.
pub fn take_auto_prompt() -> Option<ReleaseUpdate> {
    let mut guard = shared().lock().ok()?;
    let candidate = guard.pending_prompt.take()?;
    if guard.dismissed_version.as_deref() == Some(candidate.version.as_str()) {
        return None;
    }
    Some(candidate)
}

/// Remembers that the user clicked *"Not Now"* for `version` during this session.
pub fn dismiss_version(version: &str) {
    if let Ok(mut guard) = shared().lock() {
        guard.dismissed_version = Some(normalize_version(version));
        guard.pending_prompt = None;
    }
}

/// Normalizes a tag or version string (`"fnh-v1.7"`, `"v1.7.0"`, `"FNH v1.8"`) into `"1.7"`.
pub fn normalize_version(raw: &str) -> String {
    let mut s = raw.trim();
    for prefix in ["FNH ", "fnh ", "fnh-v", "FNH-v", "fnh-", "FNH-", "v", "V"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim();
        }
    }
    s.trim().to_owned()
}

/// Parses dot-separated numeric version components (`"1.7"` -> `[1, 7]`, `"1.10.0"` -> `[1, 10]`).
pub fn parse_version_numbers(raw: &str) -> Vec<u64> {
    let clean = normalize_version(raw);
    let core = clean.split(['-', '+']).next().unwrap_or(&clean);
    let mut nums: Vec<u64> = core
        .split('.')
        .filter(|part| !part.is_empty())
        .filter_map(|part| part.parse::<u64>().ok())
        .collect();
    while nums.len() > 2 && nums.last() == Some(&0) {
        nums.pop();
    }
    nums
}

/// Returns `true` iff `candidate` is strictly newer than `current` (e.g. `"1.6"` -> `"1.7"`,
/// `"1.9"` -> `"1.10"`).
pub fn is_newer_version(current: &str, candidate: &str) -> bool {
    let cur = parse_version_numbers(current);
    let cand = parse_version_numbers(candidate);
    if cur.is_empty() || cand.is_empty() {
        return false;
    }
    let len = cur.len().max(cand.len());
    for i in 0..len {
        let a = cur.get(i).copied().unwrap_or(0);
        let b = cand.get(i).copied().unwrap_or(0);
        if b > a {
            return true;
        }
        if b < a {
            return false;
        }
    }
    false
}

/// Removes leftover `FH.exe.old` and temporary update staging directories from a previous update.
pub fn cleanup_stale_update_files() {
    if let Ok(exe) = std::env::current_exe() {
        let old = old_backup_path(&exe);
        if old.exists() {
            let _ = fs::remove_file(&old);
        }
    }
    let stage_root = crate::paths::cache_dir().join("update");
    if stage_root.exists() {
        let _ = fs::remove_dir_all(&stage_root);
    }
}

fn old_backup_path(exe: &Path) -> PathBuf {
    let mut os = exe.as_os_str().to_os_string();
    os.push(".old");
    PathBuf::from(os)
}

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    body: String,
    #[serde(default)]
    html_url: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

/// Fetches the latest published release from GitHub and compares it against `crate::VERSION`.
pub fn fetch_latest_release() -> Result<ReleaseUpdate, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(20))
        .build();
    let resp = agent
        .get(LATEST_API_URL)
        .set("User-Agent", &format!("FNH/{}", crate::VERSION))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| match e {
            ureq::Error::Status(404, _) => "No published FNH releases found yet.".to_owned(),
            ureq::Error::Status(code, _) => format!("GitHub Releases returned HTTP {code}."),
            ureq::Error::Transport(t) => format!("Could not reach GitHub Releases: {t}"),
        })?;

    let rel: GhRelease = resp
        .into_json()
        .map_err(|e| format!("Invalid release metadata from GitHub: {e}"))?;
    if rel.draft {
        return Err("Latest release is still a draft.".into());
    }

    build_release_update(crate::VERSION, rel)
}

fn build_release_update(current_version: &str, rel: GhRelease) -> Result<ReleaseUpdate, String> {
    let version = normalize_version(&rel.tag_name);
    if version.is_empty() {
        return Err("Release tag did not contain a version number.".into());
    }

    // Prefer `FnHLauncher.zip`, falling back to any `.zip` asset on the release, or the canonical URL.
    let picked = rel
        .assets
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(PREFERRED_ZIP_NAME))
        .or_else(|| {
            rel.assets
                .iter()
                .find(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
        });

    let (zip_name, zip_url, size_bytes) = match picked {
        Some(a) => (
            a.name.clone(),
            a.browser_download_url.clone(),
            (a.size > 0).then_some(a.size),
        ),
        None => (
            PREFERRED_ZIP_NAME.to_owned(),
            FALLBACK_ZIP_URL.to_owned(),
            None,
        ),
    };

    let html_url = if rel.html_url.starts_with("https://") {
        rel.html_url
    } else {
        "https://github.com/bufferclick/fnh/releases/latest".to_owned()
    };

    let title = if rel.name.trim().is_empty() {
        format!("FNH v{version}")
    } else {
        rel.name.trim().to_owned()
    };

    Ok(ReleaseUpdate {
        newer_than_current: is_newer_version(current_version, &version),
        version,
        tag: rel.tag_name,
        title,
        notes: rel.body.trim().to_owned(),
        html_url,
        zip_name,
        zip_url,
        size_bytes,
    })
}

/// Checks GitHub Releases on a background thread. When `auto_prompt_on_newer` is `true` and a newer
/// version exists, `take_auto_prompt()` will yield it so FNH automatically asks
/// *"Do you want to update FnH?"*.
pub fn check_in_background(auto_prompt_on_newer: bool) {
    if let Ok(mut guard) = shared().lock() {
        if matches!(
            guard.state,
            UpdateState::Checking | UpdateState::Downloading { .. } | UpdateState::Applying { .. }
        ) {
            return;
        }
        guard.state = UpdateState::Checking;
    }

    std::thread::spawn(move || match fetch_latest_release() {
        Ok(info) => {
            log::line(&format!(
                "Update check: current v{}, latest v{} (newer={}).",
                crate::VERSION,
                info.version,
                info.newer_than_current
            ));
            if let Ok(mut guard) = shared().lock() {
                if info.newer_than_current {
                    if auto_prompt_on_newer
                        && guard.dismissed_version.as_deref() != Some(info.version.as_str())
                    {
                        guard.pending_prompt = Some(info.clone());
                    }
                    guard.state = UpdateState::Available(info);
                } else {
                    guard.state = UpdateState::UpToDate {
                        current: crate::VERSION.to_owned(),
                    };
                }
            }
        }
        Err(err) => {
            log::line(&format!("Update check failed: {err}"));
            if let Ok(mut guard) = shared().lock() {
                if auto_prompt_on_newer {
                    // Silent startup check when offline: don't leave a scary red error banner.
                    guard.state = UpdateState::Idle;
                } else {
                    guard.state = UpdateState::Failed(err);
                }
            }
        }
    });
}

fn allowed_download_url(url: &str) -> bool {
    let lower = url.to_ascii_lowercase();
    lower.starts_with("https://github.com/")
        || lower.starts_with("https://objects.githubusercontent.com/")
        || lower.starts_with("https://release-assets.githubusercontent.com/")
}

/// Downloads `update.zip_url`, extracts `FH.exe`, deletes the `.zip`, replaces the running
/// binary, and relaunches into the new version!
pub fn start_install_update(update: ReleaseUpdate) {
    if let Ok(mut guard) = shared().lock() {
        if matches!(
            guard.state,
            UpdateState::Downloading { .. } | UpdateState::Applying { .. }
        ) {
            return;
        }
        guard.pending_prompt = None;
        guard.state = UpdateState::Downloading {
            version: update.version.clone(),
            percent: 0,
            message: format!("Connecting to GitHub for {}…", update.zip_name),
        };
    }

    std::thread::spawn(move || {
        if let Err(err) = perform_download_and_apply(&update) {
            log::line(&format!("Self-update failed: {err}"));
            if let Ok(mut guard) = shared().lock() {
                guard.state = UpdateState::Failed(err);
            }
        }
    });
}

fn set_progress(version: &str, percent: u8, message: impl Into<String>) {
    if let Ok(mut guard) = shared().lock() {
        guard.state = UpdateState::Downloading {
            version: version.to_owned(),
            percent: percent.min(100),
            message: message.into(),
        };
    }
}

fn perform_download_and_apply(update: &ReleaseUpdate) -> Result<(), String> {
    if !allowed_download_url(&update.zip_url) {
        return Err(format!(
            "Refusing update download from untrusted host: {}",
            update.zip_url
        ));
    }

    let current_exe = std::env::current_exe()
        .map_err(|e| format!("Could not locate running FH.exe: {e}"))?;

    let update_dir = crate::paths::cache_dir().join("update");
    let _ = fs::remove_dir_all(&update_dir);
    fs::create_dir_all(&update_dir)
        .map_err(|e| format!("Could not create update staging folder: {e}"))?;

    let part_path = update_dir.join(format!("{PREFERRED_ZIP_NAME}.part"));
    let zip_path = update_dir.join(PREFERRED_ZIP_NAME);
    let staged_dir = update_dir.join("staged");

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(15))
        .timeout_read(Duration::from_secs(180))
        .build();
    let resp = agent
        .get(&update.zip_url)
        .set("User-Agent", &format!("FNH/{}", crate::VERSION))
        .call()
        .map_err(|e| format!("Failed to download {}: {e}", update.zip_name))?;

    let total = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .or(update.size_bytes)
        .unwrap_or(0);

    {
        let mut reader = resp.into_reader();
        let mut out = fs::File::create(&part_path)
            .map_err(|e| format!("Could not write {}: {e}", part_path.display()))?;
        let mut buf = vec![0u8; 64 * 1024];
        let mut downloaded = 0u64;
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("Download interrupted: {e}"))?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n])
                .map_err(|e| format!("Disk write error: {e}"))?;
            downloaded += n as u64;
            if downloaded > (256 << 20) {
                let _ = fs::remove_file(&part_path);
                return Err("Update archive exceeded maximum 256 MiB safety limit.".into());
            }
            let pct = if total > 0 {
                ((downloaded.saturating_mul(85)) / total).min(85) as u8
            } else {
                45
            };
            set_progress(
                &update.version,
                pct,
                format!(
                    "Downloading {} ({:.1} MB)…",
                    update.zip_name,
                    downloaded as f64 / (1024.0 * 1024.0)
                ),
            );
        }
        out.flush().map_err(|e| format!("Flush failed: {e}"))?;
    }

    fs::rename(&part_path, &zip_path)
        .map_err(|e| format!("Could not finalize {}: {e}", zip_path.display()))?;

    set_progress(&update.version, 90, "Extracting FnHLauncher.zip…");
    let extracted_exe = extract_update_zip(&zip_path, &staged_dir)?;

    // Always delete the downloaded `.zip` immediately after extraction!
    let _ = fs::remove_file(&zip_path);

    set_progress(&update.version, 98, "Installing update and restarting FnH…");
    if let Ok(mut guard) = shared().lock() {
        guard.state = UpdateState::Applying {
            version: update.version.clone(),
        };
    }

    apply_and_relaunch(&extracted_exe, &staged_dir, &current_exe, crate::VERSION)
}

/// Extracts `FH.exe` (and optional `README.txt`) from `zip_path` into `staged_dir`, verifying a
/// valid Windows PE header (`MZ`).
pub fn extract_update_zip(zip_path: &Path, staged_dir: &Path) -> Result<PathBuf, String> {
    let _ = fs::remove_dir_all(staged_dir);
    fs::create_dir_all(staged_dir)
        .map_err(|e| format!("Could not create {}: {e}", staged_dir.display()))?;

    let file =
        fs::File::open(zip_path).map_err(|e| format!("Could not open {}: {e}", zip_path.display()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Invalid update zip archive: {e}"))?;

    let mut found_exe = false;
    for idx in 0..archive.len() {
        let mut entry = archive
            .by_index(idx)
            .map_err(|e| format!("Corrupt zip entry: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let raw_name = entry
            .name()
            .map_err(|e| format!("Invalid zip path: {e}"))?
            .replace('\\', "/");
        let leaf = raw_name.rsplit('/').next().unwrap_or(&raw_name);
        if leaf.eq_ignore_ascii_case("FH.exe") {
            let dst = staged_dir.join("FH.exe");
            let mut out =
                fs::File::create(&dst).map_err(|e| format!("Could not write staged FH.exe: {e}"))?;
            std::io::copy(&mut entry, &mut out)
                .map_err(|e| format!("Failed to extract FH.exe: {e}"))?;
            found_exe = true;
        } else if leaf.eq_ignore_ascii_case("README.txt")
            || leaf.eq_ignore_ascii_case("FnH-Dark-Glass.zip")
            || (leaf.starts_with("fnh-menu-") && leaf.ends_with(".jar"))
        {
            let dst = staged_dir.join(leaf);
            if let Ok(mut out) = fs::File::create(&dst) {
                let _ = std::io::copy(&mut entry, &mut out);
            }
        }
    }

    if !found_exe {
        return Err("Update archive did not contain FH.exe.".into());
    }

    let exe_path = staged_dir.join("FH.exe");
    let meta = fs::metadata(&exe_path).map_err(|e| format!("Missing staged FH.exe: {e}"))?;
    if meta.len() < 4096 {
        return Err("Extracted FH.exe is unexpectedly small / corrupt.".into());
    }
    let mut magic = [0u8; 2];
    fs::File::open(&exe_path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .map_err(|e| format!("Could not verify FH.exe header: {e}"))?;
    if &magic != b"MZ" {
        return Err("Extracted FH.exe is not a valid Windows executable.".into());
    }

    Ok(exe_path)
}

fn apply_and_relaunch(
    staged_exe: &Path,
    staged_dir: &Path,
    current_exe: &Path,
    old_version: &str,
) -> Result<(), String> {
    let old_backup = old_backup_path(current_exe);
    let _ = fs::remove_file(&old_backup);

    // Strategy A: Windows allows renaming a currently executing `.exe` in-place!
    if fs::rename(current_exe, &old_backup).is_ok() {
        match fs::copy(staged_exe, current_exe) {
            Ok(_) => {
                if let Some(parent) = current_exe.parent() {
                    let staged_readme = staged_dir.join("README.txt");
                    let existing_readme = parent.join("README.txt");
                    if staged_readme.is_file() && existing_readme.is_file() {
                        let _ = fs::copy(&staged_readme, &existing_readme);
                    }
                }
                let _ = fs::remove_dir_all(staged_dir);
                log::line("In-place executable swap succeeded; launching updated FH.exe.");
                log::close();
                spawn_detached(current_exe, &["--updated-from", old_version])?;
                std::process::exit(0);
            }
            Err(copy_err) => {
                // Roll back rename before falling back to detached worker.
                let _ = fs::rename(&old_backup, current_exe);
                log::line(&format!(
                    "Direct copy after rename hit error ({copy_err}); using detached update worker."
                ));
            }
        }
    }

    // Strategy B: Detached `--apply-update` worker (waits for our PID to exit, swaps FH.exe, relaunches).
    let pid = std::process::id();
    let worker_exe = std::env::temp_dir().join(format!("fnh-updater-{pid}.exe"));
    fs::copy(staged_exe, &worker_exe)
        .map_err(|e| format!("Could not stage update helper {}: {e}", worker_exe.display()))?;

    let mut cmd = Command::new(&worker_exe);
    cmd.arg("--apply-update")
        .arg(pid.to_string())
        .arg("--staged")
        .arg(staged_dir)
        .arg("--target")
        .arg(current_exe)
        .arg("--from")
        .arg(old_version);
    configure_detached(&mut cmd);

    log::line("Spawning detached --apply-update worker and exiting.");
    log::close();
    cmd.spawn()
        .map_err(|e| format!("Could not launch update worker: {e}"))?;
    std::process::exit(0);
}

fn configure_detached(cmd: &mut Command) {
    cmd.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
    }
}

fn spawn_detached(exe: &Path, args: &[&str]) -> Result<(), String> {
    let mut cmd = Command::new(exe);
    cmd.args(args);
    if let Some(parent) = exe.parent() {
        cmd.current_dir(parent);
    }
    configure_detached(&mut cmd);
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Could not relaunch {}: {e}", exe.display()))
}

/// Runs in the detached `--apply-update` helper process before any log or instance lock is opened.
pub fn run_apply_update_worker(
    parent_pid: u32,
    staged_dir: &Path,
    target_exe: &Path,
    from_version: &str,
) -> i32 {
    // 1. Wait up to 20s for old `FH.exe` to exit cleanly.
    for _ in 0..100 {
        if !pid_alive(parent_pid) {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    std::thread::sleep(Duration::from_millis(150));

    let staged_exe = staged_dir.join("FH.exe");
    if !staged_exe.is_file() {
        return 2;
    }

    // 2. Replace `target_exe`, retrying briefly in case Windows Defender scans the file handle.
    let mut placed = false;
    for _ in 0..30 {
        if fs::copy(&staged_exe, target_exe).is_ok() {
            placed = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    if !placed {
        return 3;
    }

    if let Some(parent) = target_exe.parent() {
        let staged_readme = staged_dir.join("README.txt");
        let target_readme = parent.join("README.txt");
        if staged_readme.is_file() && target_readme.is_file() {
            let _ = fs::copy(&staged_readme, &target_readme);
        }
    }

    let _ = fs::remove_dir_all(staged_dir);
    let _ = fs::remove_file(old_backup_path(target_exe));

    let _ = spawn_detached(target_exe, &["--updated-from", from_version]);
    0
}

#[cfg(windows)]
fn pid_alive(pid: u32) -> bool {
    use windows_sys::Win32::Foundation::{
        CloseHandle, GetLastError, ERROR_ACCESS_DENIED, STILL_ACTIVE,
    };
    use windows_sys::Win32::System::Threading::{
        GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };
    unsafe {
        let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if handle.is_null() {
            return GetLastError() == ERROR_ACCESS_DENIED;
        }
        let mut code = 0u32;
        let running = GetExitCodeProcess(handle, &mut code) != 0 && code as i32 == STILL_ACTIVE;
        CloseHandle(handle);
        running
    }
}

#[cfg(not(windows))]
fn pid_alive(pid: u32) -> bool {
    Path::new(&format!("/proc/{pid}")).exists()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_and_compares_fnh_versions_accurately() {
        assert_eq!(normalize_version("fnh-v1.7"), "1.7");
        assert_eq!(normalize_version("v1.8.0"), "1.8.0");
        assert_eq!(normalize_version("FNH v1.10"), "1.10");

        assert!(is_newer_version("1.6", "1.7"));
        assert!(is_newer_version("1.6", "fnh-v1.7"));
        assert!(is_newer_version("1.9", "1.10"));
        assert!(is_newer_version("1.7", "1.7.1"));
        assert!(!is_newer_version("1.7", "1.7"));
        assert!(!is_newer_version("1.7", "1.7.0"));
        assert!(!is_newer_version("1.8", "1.7"));
    }

    #[test]
    fn extracts_fnh_launcher_zip_and_verifies_pe_header() {
        let tmp = std::env::temp_dir().join(format!("fnh-upd-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp);
        fs::create_dir_all(&tmp).unwrap();

        let zip_path = tmp.join(PREFERRED_ZIP_NAME);
        {
            let f = fs::File::create(&zip_path).unwrap();
            let mut z = zip::ZipWriter::new(f);
            let opts = zip::write::SimpleFileOptions::default();
            z.start_file("fnh/FH.exe", opts).unwrap();
            let mut fake_pe = vec![b'M', b'Z'];
            fake_pe.resize(8192, 0x90);
            z.write_all(&fake_pe).unwrap();
            z.start_file("fnh/README.txt", opts).unwrap();
            z.write_all(b"FNH v1.7").unwrap();
            z.finish().unwrap();
        }

        let staged = tmp.join("staged");
        let exe = extract_update_zip(&zip_path, &staged).unwrap();
        assert!(exe.is_file());
        assert!(staged.join("README.txt").is_file());

        // Simulate deleting the zip right after unzip, matching `perform_download_and_apply`.
        fs::remove_file(&zip_path).unwrap();
        assert!(!zip_path.exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn prefers_fnh_launcher_zip_asset_when_selecting_download() {
        let rel = GhRelease {
            tag_name: "fnh-v1.8".into(),
            name: "FNH v1.8".into(),
            body: "Huge update".into(),
            html_url: "https://github.com/bufferclick/FnH/releases/tag/fnh-v1.8".into(),
            draft: false,
            assets: vec![
                GhAsset {
                    name: "other.zip".into(),
                    browser_download_url:
                        "https://github.com/bufferclick/FnH/releases/download/fnh-v1.8/other.zip"
                            .into(),
                    size: 100,
                },
                GhAsset {
                    name: "FnHLauncher.zip".into(),
                    browser_download_url:
                        "https://github.com/bufferclick/FnH/releases/download/fnh-v1.8/FnHLauncher.zip"
                            .into(),
                    size: 15_000_000,
                },
            ],
        };
        let parsed = build_release_update("1.7", rel).unwrap();
        assert!(parsed.newer_than_current);
        assert_eq!(parsed.version, "1.8");
        assert_eq!(parsed.zip_name, "FnHLauncher.zip");
        assert!(parsed.zip_url.ends_with("/FnHLauncher.zip"));
    }
}
