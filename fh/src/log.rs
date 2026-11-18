//! The system around the window: the log file, the player's browser for a sign-in, and the
//! clipboard a code goes to.
//!
//! A double-clicked .exe has no console, so everything the window would have printed goes to
//! `fh/fh.log` beside the launcher's data instead — including the exact command line the game was
//! started with, which is the first thing to look at when a launch says it is not signed in.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime};

static STARTED: OnceLock<Instant> = OnceLock::new();
static PATH: OnceLock<PathBuf> = OnceLock::new();
static FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Opens the log at `path`, made fresh when the last one grew past a megabyte.
pub fn start(path: PathBuf) {
    let _ = STARTED.set(Instant::now());
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if std::fs::metadata(&path).is_ok_and(|meta| meta.len() > 1 << 20) {
        let _ = std::fs::remove_file(&path);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok();
    if let Ok(mut open) = FILE.lock() {
        *open = file;
    }
    let _ = PATH.set(path);
}

/// Flushes and closes the open log file handle so a factory reset can delete `fh/` on Windows.
pub fn close() {
    if let Ok(mut open) = FILE.lock() {
        if let Some(mut file) = open.take() {
            let _ = file.flush();
        }
    }
}

/// Where the log is.
pub fn path() -> &'static Path {
    PATH.get().map(PathBuf::as_path).unwrap_or(Path::new("fh.log"))
}

/// One line, with the seconds since the start, and the day's clock beside it.
pub fn line(text: &str) {
    let since = STARTED.get().map(|start| start.elapsed().as_secs_f32()).unwrap_or(0.);
    let clock = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|since| since.as_secs() % 86_400)
        .unwrap_or(0);
    let stamp = format!(
        "{:02}:{:02}:{:02}",
        clock / 3600,
        (clock % 3600) / 60,
        clock % 60
    );
    let record = format!("[{since:>7.2}s] {stamp} {text}\n");
    let _ = std::io::stderr().write_all(record.as_bytes());
    if let Ok(mut file) = FILE.lock() {
        if let Some(file) = file.as_mut() {
            let _ = file.write_all(record.as_bytes());
        }
    }
}

/// Opens `url` in the player's default browser: the shell's handler on Windows, the desktop's
/// elsewhere. Only the sign-in ever asks for it.
pub fn open_url(url: &str) -> Result<(), String> {
    let status = opener(url)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not open {url}: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("Could not open {url}: {status}"))
    }
}

#[cfg(windows)]
fn opener(url: &str) -> std::process::Command {
    use std::os::windows::process::CommandExt;
    let mut command = horizon_launcher::process::command("cmd");
    // `start` hands the address to the shell's handler for it; the empty title keeps the quotes
    // of the address itself straight, and no console window opens (process::command).
    command.raw_arg(format!("/D /S /C start \"\" \"{url}\""));
    command
}

#[cfg(target_os = "macos")]
fn opener(url: &str) -> std::process::Command {
    let mut command = horizon_launcher::process::command("open");
    command.arg(url);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn opener(url: &str) -> std::process::Command {
    let mut command = horizon_launcher::process::command("xdg-open");
    command.arg(url);
    command
}

/// Puts `text` on the clipboard, so the sign-in code can be pasted on Microsoft's page: Windows'
/// own `clip.exe`, which keeps the clipboard after this program exits. False where it can't.
#[cfg(windows)]
pub fn copy(text: &str) -> bool {
    let mut child = match horizon_launcher::process::command("clip.exe")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    let written = child
        .stdin
        .take()
        .map(|mut stdin| stdin.write_all(text.as_bytes()).is_ok())
        .unwrap_or(false);
    written && child.wait().map(|status| status.success()).unwrap_or(false)
}

#[cfg(not(windows))]
pub fn copy(_text: &str) -> bool {
    false
}

/// Opens `path` in the system's file manager: Explorer on Windows, the desktop's elsewhere.
pub fn open_folder(path: &Path) -> Result<(), String> {
    if !path.is_dir() {
        return Err(format!("{} is not a folder on this computer.", path.display()));
    }
    #[cfg(windows)]
    let mut command = {
        let mut command = horizon_launcher::process::command("explorer.exe");
        command.arg(path);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = horizon_launcher::process::command("open");
        command.arg(path);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = horizon_launcher::process::command("xdg-open");
        command.arg(path);
        command
    };
    let status = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| format!("Could not open {}: {e}", path.display()))?;
    // Explorer answers 1 even when the window opened: only a failed start is a failure here.
    if status.success() || cfg!(windows) {
        Ok(())
    } else {
        Err(format!("Could not open {}: {status}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    /// The sign-in's page takes the code in its address, so the browser lands on it with nothing
    /// to type; any other page keeps its own address.
    #[test]
    fn the_link_page_is_opened_with_the_code_in_it() {
        assert_eq!(
            super::sign_in_url("https://www.microsoft.com/link", "72J7EFQU"),
            "https://www.microsoft.com/link?otc=72J7EFQU"
        );
        assert_eq!(
            super::sign_in_url("https://microsoft.com/devicelogin", "72J7EFQU"),
            "https://microsoft.com/devicelogin"
        );
    }
}

/// The address the sign-in opens on: Microsoft's own link page takes the code in the address
/// (`?otc=`), so the player signs in without typing it.
pub fn sign_in_url(link: &str, code: &str) -> String {
    let lower = link.trim_end_matches('/').to_ascii_lowercase();
    if code.is_empty() || lower != "https://www.microsoft.com/link" {
        return link.to_owned();
    }
    let separator = if link.contains('?') { '&' } else { '?' };
    format!("{link}{separator}otc={code}")
}
