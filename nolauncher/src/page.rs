//! The one thing the player sees: a local HTML page in their browser, rewritten as the work
//! moves on. It says what to type on Microsoft's page — the code, a QR code of that page and a
//! button that copies the code — then how far the install and the preparation are, then that
//! Horizon is starting; a failing start says why and where the log is.
//!
//! It is a file, not a window: no framework, no launcher, nothing to click. The page reloads
//! itself once a second (`<meta http-equiv="refresh">`), so the tab the sign-in opened becomes
//! the progress it is waiting for, and it is only ever opened when there is something to wait
//! for — a start on a computer that is already signed in and prepared opens no browser at all.

use crate::host;
use crate::VERSION;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// How often the page is rewritten at most, so a download's progress does not stroke the disk
/// hundreds of times a second. Phases (`step`, `sign_in`, `starting`) are written at once.
const THROTTLE: Duration = Duration::from_millis(200);

/// How long a phase may run before the browser is opened for it. A start with nothing to do takes
/// a second or two and opens nothing at all; a download or a preparation takes minutes, and the
/// player is not left looking at a double-click that seems to have done nothing.
const SLOW: Duration = Duration::from_secs(3);

/// What the page says.
#[derive(Clone, Debug, PartialEq)]
pub enum Shown {
    /// Nothing worth showing yet.
    Idle,
    /// Waiting for the player to enter `code` on Microsoft's page (`link`).
    SignIn { code: String, link: String },
    /// Work under way: what it is, how far it has gone (0 to 1 when known), the line it is at.
    Step {
        title: String,
        line: String,
        fraction: Option<f32>,
    },
    /// The game was started; the tab has done its job.
    Starting,
    /// It stopped: why.
    Failed(String),
}

/// The page file and the state it shows.
pub struct Page {
    path: PathBuf,
    state: Mutex<Shown>,
    /// The foot of the page: this build, the game's build once known, the log.
    footer: Mutex<String>,
    /// The HTML last written and when, so an unchanged page is not written again.
    written: Mutex<Option<(String, Instant)>>,
    /// Whether the browser was already pointed at the page.
    opened: AtomicBool,
    /// When the phase on show began, for [`SLOW`].
    phase_started: Mutex<Instant>,
}

impl Page {
    /// The page kept in `dir`. Nothing is shown until [`Page::show`] (the sign-in, a failure).
    pub fn new(dir: &Path) -> Self {
        Self {
            path: dir.join("nolauncher.html"),
            state: Mutex::new(Shown::Idle),
            footer: Mutex::new(format!("NoLauncherHorizon v{}", VERSION)),
            written: Mutex::new(None),
            opened: AtomicBool::new(false),
            phase_started: Mutex::new(Instant::now()),
        }
    }

    /// The address a browser opens the page by.
    pub fn url(&self) -> String {
        host::file_url(&self.path)
    }

    /// What the page's foot says.
    pub fn footer(&self, text: &str) {
        if let Ok(mut footer) = self.footer.lock() {
            *footer = text.to_owned();
        }
        self.write(true);
    }

    /// Writes the page and opens it in the player's browser, once: later calls leave the tab
    /// alone (it reloads itself). The error is what to tell the player when no browser opened.
    pub fn show(&self) -> Result<(), String> {
        self.write(true);
        if self.opened.swap(true, Ordering::SeqCst) {
            return Ok(());
        }
        host::open(&self.url())
            .map_err(|e| format!("{e} The page is {}.", self.path.display()))
    }

    /// The sign-in code to type on Microsoft's page.
    pub fn sign_in(&self, code: &str, link: &str) {
        self.set(
            Shown::SignIn {
                code: code.to_owned(),
                link: link.to_owned(),
            },
            true,
        );
    }

    /// A new phase of the work.
    pub fn step(&self, title: &str, line: &str, fraction: Option<f32>) {
        if let Ok(mut started) = self.phase_started.lock() {
            *started = Instant::now();
        }
        self.set(
            Shown::Step {
                title: title.to_owned(),
                line: line.to_owned(),
                fraction,
            },
            true,
        );
    }

    /// The line the phase is at now (a preparer's output, a download's file).
    pub fn line(&self, line: &str) {
        if let Ok(mut state) = self.state.lock() {
            if let Shown::Step { line: at, .. } = &mut *state {
                *at = line.to_owned();
            }
        }
        self.write(false);
        self.show_if_slow();
    }

    /// How far the phase has gone, 0 to 1.
    pub fn progress(&self, fraction: f32) {
        if let Ok(mut state) = self.state.lock() {
            if let Shown::Step {
                fraction: at, ..
            } = &mut *state
            {
                *at = Some(fraction.clamp(0., 1.));
            }
        }
        self.write(false);
        self.show_if_slow();
    }

    /// Opens the page when the phase has run long enough to be worth watching (`SLOW`): the
    /// downloads and the preparations do, a start that is already prepared does not.
    fn show_if_slow(&self) {
        if self.opened.load(Ordering::SeqCst) {
            return;
        }
        let slow = self
            .phase_started
            .lock()
            .map(|started| started.elapsed() >= SLOW)
            .unwrap_or(false);
        if slow {
            let _ = self.show();
        }
    }

    /// The game started: the page says so and stops reloading.
    pub fn starting(&self) {
        self.set(Shown::Starting, true);
    }

    /// It stopped: the page says why, and opens if it was never shown.
    pub fn error(&self, text: &str) {
        self.set(Shown::Failed(text.to_owned()), true);
        let _ = self.show();
    }

    fn set(&self, shown: Shown, force: bool) {
        if let Ok(mut state) = self.state.lock() {
            *state = shown;
        }
        self.write(force);
    }

    /// Renders the page and writes it, unless it is the same page that is already there or the
    /// last write was a moment ago (`force`: written anyway).
    fn write(&self, force: bool) {
        let shown = match self.state.lock() {
            Ok(state) => state.clone(),
            Err(_) => return,
        };
        let footer = self
            .footer
            .lock()
            .map(|footer| footer.clone())
            .unwrap_or_default();
        let html = render(&shown, &footer);
        match self.written.lock() {
            Ok(mut written) => {
                if let Some((last, at)) = written.as_ref() {
                    if *last == html {
                        return;
                    }
                    if !force && at.elapsed() < THROTTLE {
                        return;
                    }
                }
                *written = Some((html.clone(), Instant::now()));
            }
            Err(_) => return,
        }
        if let Some(dir) = self.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        // Written beside and renamed into place, so a reload never reads half a page. A browser
        // holding the file for a moment fails the rename rather than the write: try again.
        let part = self.path.with_extension("html.part");
        for attempt in 0..4 {
            if std::fs::write(&part, &html).is_ok() && std::fs::rename(&part, &self.path).is_ok() {
                return;
            }
            if attempt < 3 {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
    }
}

/// The page for `shown`.
pub fn render(shown: &Shown, footer: &str) -> String {
    // Everything but the end of the work reloads itself: the tab is the progress bar.
    let refresh = match shown {
        Shown::Starting | Shown::Failed(_) => "",
        _ => "<meta http-equiv=\"refresh\" content=\"1\">",
    };
    let (title, body) = match shown {
        Shown::Idle => (
            "Getting ready".to_owned(),
            "<p class=\"note\">Checking your sign-in and the game's files.</p>".to_owned(),
        ),
        Shown::SignIn { code, link } => (
            "Sign in with Microsoft".to_owned(),
            sign_in_body(code, link),
        ),
        Shown::Step {
            title,
            line,
            fraction,
        } => (
            title.clone(),
            format!(
                "<p class=\"note\">{line}</p>{bar}",
                line = escape(line),
                bar = match fraction {
                    Some(fraction) => format!(
                        "<div class=\"bar\"><i style=\"width:{:.0}%\"></i></div>",
                        fraction.clamp(0., 1.) * 100.
                    ),
                    None => String::new(),
                },
            ),
        ),
        Shown::Starting => (
            "Horizon is starting".to_owned(),
            "<p class=\"note\">You can close this tab.</p>".to_owned(),
        ),
        Shown::Failed(why) => (
            "Horizon did not start".to_owned(),
            format!(
                "<p class=\"note danger\">{why}</p>\
                 <p class=\"note\">Start it again once the problem is fixed; the log says more.</p>",
                why = escape(why),
            ),
        ),
    };
    let mut html = format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         {refresh}\n<title>{title}</title>\n<style>{css}</style>\n</head>\n<body>\n<main>\n\
         <p class=\"brand\">NoLauncher<span>Horizon</span></p>\n<h1>{title}</h1>\n{body}\n\
         <p class=\"stamp\">{footer}</p>\n</main>\n",
        title = escape(&title),
        css = CSS,
        footer = escape(footer),
    );
    // The script has braces of its own, so it is appended rather than formatted.
    html.push_str(SCRIPT);
    html.push_str("\n</body>\n</html>\n");
    html
}

/// The sign-in page: the code, a button that copies it, the QR code of Microsoft's page and the
/// address itself — nothing else, and no clicking needed to start (the browser is already there).
fn sign_in_body(code: &str, link: &str) -> String {
    let mut body = format!(
        "<p class=\"note\">Enter this code on Microsoft's page:</p>\
         <p class=\"row\"><span class=\"code\">{code}</span>\
         <button class=\"copy\" type=\"button\" data-code=\"{code}\" \
         onclick=\"copyCode(this)\">Copy</button></p>",
        code = escape(code),
    );
    if let Some(drawing) = crate::qr::for_link(link) {
        body.push_str("<p class=\"qr\">");
        body.push_str(drawing);
        body.push_str("</p>");
    }
    body.push_str(&format!(
        "<p class=\"note\">Open <a href=\"{open}\" target=\"_blank\" rel=\"noreferrer\">\
         {link}</a> or scan the QR and enter the code above if needed.</p>\
         <p class=\"wait\">Waiting for you to finish signing in…</p>",
        open = escape(&open_url(link, code)),
        link = escape(link),
    ));
    body
}

/// The address the sign-in opens on: Microsoft's link page takes the code in the address
/// (`?otc=`), so the player signs in without typing it. Any other verification page gets the
/// address as it is.
pub fn open_url(link: &str, code: &str) -> String {
    if code.is_empty() || !crate::qr::is_link_page(link) {
        return link.to_owned();
    }
    let separator = if link.contains('?') { '&' } else { '?' };
    format!("{link}{separator}otc={}", encode(code))
}

/// `text` with everything but letters, digits, dashes and stops percent-encoded: a code is eight
/// of those, and anything a service sends stays safe in an address.
fn encode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'.' || byte == b'_' {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

/// `text` with the characters HTML reads as markup (a log line, an error, the version).
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// The look: plain. One column in the middle of the window, Horizon's night behind it, the code
/// in the player's own mono, and the QR code on the white a camera can read.
const CSS: &str = "\
:root{color-scheme:dark}\
*{box-sizing:border-box}\
body{margin:0;min-height:100vh;display:grid;place-items:center;background:#16181f;\
color:#ecedf3;font:16px/1.5 system-ui,\"Segoe UI\",sans-serif;padding:28px}\
main{width:min(420px,100%);text-align:center}\
.brand{margin:0 0 22px;font-size:12px;letter-spacing:.16em;text-transform:uppercase;\
color:#8b8f9e}\
.brand span{color:#9a86ff}\
h1{margin:0 0 20px;font-size:21px;font-weight:600}\
.note{margin:0 0 14px;color:#b7bcc9}\
.note a{color:#c9bcff}\
.row{display:flex;align-items:center;justify-content:center;gap:10px;margin:0 0 18px}\
.code{padding:10px 16px;border-radius:10px;border:1px solid #2c3142;background:#1e2130;\
color:#fff;letter-spacing:.12em;font:600 30px/1.15 ui-monospace,Consolas,monospace}\
.copy{padding:8px 12px;border-radius:8px;border:1px solid #343a4b;background:#232733;\
color:#ecedf3;font:500 13px/1 system-ui,\"Segoe UI\",sans-serif;cursor:pointer}\
.qr{margin:0 0 18px}\
.qr svg{width:168px;height:168px;border-radius:6px}\
.wait{margin:6px 0 0;color:#6f7484;font-size:13px}\
.bar{height:6px;margin:0 0 16px;border-radius:999px;background:#22262f;overflow:hidden}\
.bar i{display:block;height:100%;border-radius:999px;background:#9a86ff}\
.danger{color:#ff9a8a}\
.stamp{margin:24px 0 0;font-size:12px;color:#6f7484;word-break:break-word}";

/// The page's only script: the Copy button. The code is on the clipboard already (the program put
/// it there); this is for a player who pasted something else in between, and it works on a
/// `file://` page by falling back to a selected text area when the clipboard API is not allowed.
const SCRIPT: &str = r#"<script>
function copyCode(button) {
  var code = button.getAttribute('data-code');
  var copied = function () {
    button.textContent = 'Copied';
    setTimeout(function () { button.textContent = 'Copy'; }, 1500);
  };
  var fallback = function () {
    var area = document.createElement('textarea');
    area.value = code;
    document.body.appendChild(area);
    area.select();
    try { document.execCommand('copy'); copied(); } catch (e) {}
    document.body.removeChild(area);
  };
  if (navigator.clipboard && navigator.clipboard.writeText) {
    navigator.clipboard.writeText(code).then(copied, fallback);
  } else {
    fallback();
  }
}
</script>"#;

#[cfg(test)]
mod tests {
    use super::{escape, render, Shown};
    use crate::VERSION;

    fn page(shown: &Shown) -> String {
        render(shown, &format!("NoLauncherHorizon v{}", VERSION))
    }

    #[test]
    fn the_code_is_shown_where_the_player_types_it() {
        let html = page(&Shown::SignIn {
            code: "ABCD-1234".into(),
            link: "https://www.microsoft.com/link".into(),
        });
        assert!(html.contains("ABCD-1234"), "{html}");
        // The address carries the code, so signing in needs no typing at all.
        assert!(
            html.contains("href=\"https://www.microsoft.com/link?otc=ABCD-1234\""),
            "{html}"
        );
        // The tab reloads while the player is on Microsoft's page: it becomes the progress.
        assert!(html.contains("http-equiv=\"refresh\""));
    }

    /// The sign-in page is the code, a button that copies it, the QR code of Microsoft's page and
    /// that address — as plain as that.
    #[test]
    fn the_sign_in_page_is_the_code_the_qr_and_nothing_else() {
        let html = page(&Shown::SignIn {
            code: "72J7EFQU".into(),
            link: "https://www.microsoft.com/link".into(),
        });
        assert!(html.contains("data-code=\"72J7EFQU\""), "{html}");
        assert!(html.contains("onclick=\"copyCode(this)\""), "{html}");
        assert!(html.contains("function copyCode("), "{html}");
        let qr = crate::qr::for_link("https://www.microsoft.com/link").unwrap();
        assert!(html.contains(qr), "the QR code is not on the page");
        assert!(
            html.contains("https://www.microsoft.com/link</a>"),
            "{html}"
        );
    }

    /// The address the browser is sent to carries the code; one that is not the link page keeps
    /// its own address, and so does a code-less page.
    #[test]
    fn the_link_page_is_opened_with_the_code_in_it() {
        assert_eq!(
            super::open_url("https://www.microsoft.com/link", "72J7EFQU"),
            "https://www.microsoft.com/link?otc=72J7EFQU"
        );
        assert_eq!(
            super::open_url("https://www.microsoft.com/link/", "72J7EFQU"),
            "https://www.microsoft.com/link/?otc=72J7EFQU"
        );
        assert_eq!(
            super::open_url("https://www.microsoft.com/link?a=1", "72J7EFQU"),
            "https://www.microsoft.com/link?a=1&otc=72J7EFQU"
        );
        // Another page, and a page with no code, are left alone.
        assert_eq!(
            super::open_url("https://microsoft.com/devicelogin", "72J7EFQU"),
            "https://microsoft.com/devicelogin"
        );
        assert_eq!(
            super::open_url("https://www.microsoft.com/link", ""),
            "https://www.microsoft.com/link"
        );
    }

    /// A page with no QR code (any address but the link page) still says where to type the code.
    #[test]
    fn another_verification_page_shows_its_address_and_no_qr() {
        let html = page(&Shown::SignIn {
            code: "ABCDEFGH".into(),
            link: "https://microsoft.com/devicelogin".into(),
        });
        assert!(
            html.contains("href=\"https://microsoft.com/devicelogin\""),
            "{html}"
        );
        assert!(!html.contains("<svg"), "{html}");
        // And the code stays on the page: it is what the player types there.
        assert!(html.contains("ABCDEFGH"), "{html}");
    }

    /// A code is escaped like anything else, and encoded in the address it is put in.
    #[test]
    fn the_code_is_encoded_where_it_goes_into_an_address() {
        assert_eq!(super::encode("72J7EFQU"), "72J7EFQU");
        assert_eq!(super::encode("ABCD-1234"), "ABCD-1234");
        assert_eq!(super::encode("a b&c"), "a%20b%26c");
        let html = page(&Shown::SignIn {
            code: "<b>".into(),
            link: "https://www.microsoft.com/link".into(),
        });
        assert!(!html.contains("data-code=\"<b>\""), "{html}");
        assert!(html.contains("otc=%3Cb%3E"), "{html}");
    }

    #[test]
    fn the_progress_bar_is_the_fraction_it_was_given() {
        let html = page(&Shown::Step {
            title: "Getting Horizon".into(),
            line: "Downloading".into(),
            fraction: Some(0.42),
        });
        assert!(html.contains("width:42%"), "{html}");
        // A phase with nothing to measure shows its line and no bar.
        let html = page(&Shown::Step {
            title: "Getting Horizon".into(),
            line: "Downloading".into(),
            fraction: None,
        });
        assert!(!html.contains("class=\"bar\""), "{html}");
    }

    /// The end of the work is not reloaded, and a failure keeps what went wrong on the screen.
    #[test]
    fn a_finished_page_stops_reloading_and_says_what_happened() {
        let starting = page(&Shown::Starting);
        assert!(!starting.contains("http-equiv=\"refresh\""));
        let failed = page(&Shown::Failed("No Horizon release is installed.".into()));
        assert!(!failed.contains("http-equiv=\"refresh\""));
        assert!(failed.contains("No Horizon release is installed."));
    }

    /// A log line is text from files and services, never markup.
    #[test]
    fn text_from_the_game_is_escaped() {
        assert_eq!(escape("<b>&\"'"), "&lt;b&gt;&amp;&quot;&#39;");
        let html = page(&Shown::Failed("<script>alert(1)</script>".into()));
        assert!(!html.contains("<script>alert(1)</script>"), "{html}");
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }
}
