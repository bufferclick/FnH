//! The system around the bootstrap: the player's browser, the clipboard the sign-in code goes
//! to, and the log file that says what happened when the page can't.
//!
//! Nothing here opens a window. A double-clicked .exe has no console to print to, so every line
//! that matters is kept in `nolauncher.log` beside the page, and the only two things this
//! program ever starts directly are the browser and the game.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::Instant;

/// Opens `url` in the player's default browser (the shell's handler on Windows, LaunchServices on
/// macOS, the desktop's on Linux).
#[cfg(any(windows, unix))]
pub fn open(url: &str) -> Result<(), String> {
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

#[cfg(not(any(windows, unix)))]
pub fn open(_url: &str) -> Result<(), String> {
    Err("This system has no browser to open.".into())
}

#[cfg(windows)]
fn opener(url: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut command = horizon_launcher::process::command("cmd");
    // `start` hands the address to the shell's handler for it; the empty title keeps the quotes
    // of the address itself straight, and no console window opens (process::command).
    command.raw_arg(format!("/D /S /C start \"\" \"{url}\""));
    command
}

#[cfg(target_os = "macos")]
fn opener(url: &str) -> Command {
    let mut command = horizon_launcher::process::command("open");
    command.arg(url);
    command
}

#[cfg(all(unix, not(target_os = "macos")))]
fn opener(url: &str) -> Command {
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

/// `path` as the address a browser opens it by: `file:///C:/…/nolauncher.html`. A path's own
/// characters are escaped, so a user name with a space or an accent opens the same file.
pub fn file_url(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\\', "/");
    // Windows' `C:/…` and Linux's `/…` both end up after the same three slashes.
    let text = text.strip_prefix('/').unwrap_or(&text);
    format!("file:///{}", escape_path(text))
}

/// The path with everything a URL reserves escaped (`%20` for a space, UTF-8 bytes for accents).
fn escape_path(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        let plain = byte.is_ascii_alphanumeric()
            || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'/' | b':');
        if plain {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// The log of the starts: every line the work reported, with the second it came in at. It is the
/// only place a failure is explained in full — a double-clicked .exe has no terminal.
pub struct Log {
    path: PathBuf,
    started: Instant,
    file: Mutex<Option<std::fs::File>>,
}

impl Log {
    /// The log at `path`, made fresh when the last one grew past a megabyte.
    pub fn open(path: PathBuf) -> Self {
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
        Self {
            path,
            started: Instant::now(),
            file: Mutex::new(file),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// One line, with the seconds since the start. Also on a terminal's stderr, where there is
    /// one (a start from a command prompt with its output redirected, a test).
    pub fn line(&self, text: &str) {
        let line = format!("[{:>6.2}s] {}\n", self.started.elapsed().as_secs_f32(), text);
        // A terminal's stderr, where there is one. A double-clicked .exe has no console, and
        // `eprint!` panics where writing fails, so the handle is written to and ignored.
        let _ = std::io::stderr().write_all(line.as_bytes());
        if let Ok(mut file) = self.file.lock() {
            if let Some(file) = file.as_mut() {
                let _ = file.write_all(line.as_bytes());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{escape_path, file_url};
    use std::path::Path;

    /// Windows' own spelling of the page's path becomes an address a browser opens.
    #[test]
    fn a_windows_path_becomes_a_file_address() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\Buffer Name\AppData\Roaming\horizon-launcher\nolauncher\nolauncher.html")),
            "file:///C:/Users/Buffer%20Name/AppData/Roaming/horizon-launcher/nolauncher/nolauncher.html"
        );
    }

    #[test]
    fn a_unix_path_keeps_its_one_slash() {
        assert_eq!(
            file_url(Path::new("/home/buffer/.local/share/nolauncher.html")),
            "file:///home/buffer/.local/share/nolauncher.html"
        );
    }

    /// An accent is UTF-8's bytes, and the characters that would end the address are escaped.
    #[test]
    fn the_characters_a_url_reserves_are_escaped() {
        assert_eq!(escape_path("a b#c?d&e"), "a%20b%23c%3Fd%26e");
        assert_eq!(escape_path("é"), "%C3%A9");
        assert_eq!(escape_path("C:/keep/this"), "C:/keep/this");
    }
}
