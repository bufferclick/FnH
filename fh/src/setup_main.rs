//! `FnHSetup.exe` — Official standalone Windows installer for FnH (Fabric n Horizon), made by Buffer.
//!
//! Downloads the latest `FnHLauncher.zip` directly from GitHub Releases (`bufferclick/FnH`),
//! installs `FH.exe` and its companion assets into `%LOCALAPPDATA%\Programs\FnH`, lets the user
//! choose whether to create a **Desktop shortcut** and/or a **Start Menu shortcut**, and launches
//! FnH immediately when finished.

#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use gpui::{
    div, prelude::*, px, rgb, size, App, AppContext, Bounds, Context, FontWeight, Hsla,
    SharedString, Stateful, TitlebarOptions, Window, WindowBounds, WindowOptions,
};
use serde::Deserialize;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

pub const SETUP_VERSION: &str = env!("FH_VERSION");

const LATEST_API_URL: &str = "https://api.github.com/repos/bufferclick/FnH/releases/latest";
const FALLBACK_ZIP_URL: &str =
    "https://github.com/bufferclick/FnH/releases/latest/download/FnHLauncher.zip";

#[derive(Clone, Debug)]
struct LatestReleaseInfo {
    version_label: String,
    zip_url: String,
    size_bytes: Option<u64>,
}

#[derive(Clone, Debug)]
enum SetupPhase {
    Ready,
    Installing { percent: u8, detail: String },
    Finished { exe_path: PathBuf },
    Failed(String),
}

#[derive(Clone, Debug)]
struct SharedSetupState {
    release: Option<LatestReleaseInfo>,
    phase: SetupPhase,
}

#[derive(Deserialize)]
struct GhRelease {
    #[serde(default)]
    tag_name: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    #[serde(default)]
    name: String,
    #[serde(default)]
    browser_download_url: String,
    #[serde(default)]
    size: u64,
}

fn hex(v: u32) -> Hsla {
    rgb(v).into()
}

fn hexa(v: u32, a: f32) -> Hsla {
    let mut c: Hsla = rgb(v).into();
    c.a = a;
    c
}

fn default_install_dir() -> PathBuf {
    if let Some(override_dir) = std::env::var_os("FNH_SETUP_DIR").filter(|s| !s.is_empty()) {
        return PathBuf::from(override_dir);
    }
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Programs")
        .join("FnH")
}

fn start_menu_programs_dir() -> Option<PathBuf> {
    if let Some(appdata) = std::env::var_os("APPDATA").filter(|s| !s.is_empty()) {
        return Some(
            PathBuf::from(appdata)
                .join("Microsoft")
                .join("Windows")
                .join("Start Menu")
                .join("Programs"),
        );
    }
    dirs::data_dir().map(|d| {
        d.join("Microsoft")
            .join("Windows")
            .join("Start Menu")
            .join("Programs")
    })
}

fn fetch_latest_release_info() -> LatestReleaseInfo {
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(8))
        .timeout_read(Duration::from_secs(15))
        .user_agent(&format!("bufferclick-FnHSetup/{SETUP_VERSION}"))
        .build();

    if let Ok(resp) = agent
        .get(LATEST_API_URL)
        .set("Accept", "application/vnd.github+json")
        .call()
    {
        if let Ok(rel) = resp.into_json::<GhRelease>() {
            let chosen = rel
                .assets
                .iter()
                .find(|a| a.name.eq_ignore_ascii_case("FnHLauncher.zip"))
                .or_else(|| {
                    rel.assets
                        .iter()
                        .find(|a| a.name.to_ascii_lowercase().ends_with(".zip"))
                });
            let zip_url = chosen
                .map(|a| a.browser_download_url.clone())
                .filter(|u| u.starts_with("https://"))
                .unwrap_or_else(|| FALLBACK_ZIP_URL.to_owned());
            let size_bytes = chosen.map(|a| a.size).filter(|&s| s > 0);
            let raw_ver = rel
                .tag_name
                .trim()
                .trim_start_matches("fnh-")
                .trim_start_matches('v')
                .to_owned();
            let version_label = if !raw_ver.is_empty() {
                format!("v{raw_ver}")
            } else if !rel.name.trim().is_empty() {
                rel.name.trim().to_owned()
            } else {
                "Latest Release".to_owned()
            };
            return LatestReleaseInfo {
                version_label,
                zip_url,
                size_bytes,
            };
        }
    }

    LatestReleaseInfo {
        version_label: format!("v{SETUP_VERSION}+ (Latest)"),
        zip_url: FALLBACK_ZIP_URL.to_owned(),
        size_bytes: None,
    }
}

fn perform_install(
    info: LatestReleaseInfo,
    install_dir: &Path,
    make_desktop_shortcut: bool,
    make_start_menu_shortcut: bool,
    progress: &dyn Fn(u8, String),
) -> Result<PathBuf, String> {
    progress(4, format!("Connecting to GitHub ({})…", info.version_label));

    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(12))
        .timeout_read(Duration::from_secs(120))
        .user_agent(&format!("bufferclick-FnHSetup/{SETUP_VERSION}"))
        .build();

    let resp = agent
        .get(&info.zip_url)
        .call()
        .map_err(|e| format!("Download failed: {e}"))?;

    let expected_total = resp
        .header("Content-Length")
        .and_then(|s| s.parse::<u64>().ok())
        .or(info.size_bytes)
        .unwrap_or(18 * 1024 * 1024);

    let mut reader = resp.into_reader().take(250 * 1024 * 1024);
    let mut zip_bytes = Vec::with_capacity((expected_total as usize).min(64 * 1024 * 1024));
    let mut chunk = [0u8; 65536];
    let mut last_pct = 4u8;

    loop {
        let n = reader
            .read(&mut chunk)
            .map_err(|e| format!("Network read interrupted: {e}"))?;
        if n == 0 {
            break;
        }
        zip_bytes.extend_from_slice(&chunk[..n]);
        let ratio = (zip_bytes.len() as f64 / expected_total.max(1) as f64).clamp(0.0, 1.0);
        let pct = 6 + (ratio * 72.0).round() as u8;
        if pct != last_pct {
            last_pct = pct;
            let mb = zip_bytes.len() as f64 / (1024.0 * 1024.0);
            progress(pct, format!("Downloading FnH {} ({mb:.1} MB)…", info.version_label));
        }
    }

    if zip_bytes.len() < 4096 || !zip_bytes.starts_with(b"PK") {
        return Err("Downloaded archive was incomplete or invalid.".into());
    }

    progress(82, format!("Extracting to {}…", install_dir.display()));
    fs::create_dir_all(install_dir)
        .map_err(|e| format!("Could not create {}: {e}", install_dir.display()))?;

    let mut archive = zip::ZipArchive::new(Cursor::new(zip_bytes))
        .map_err(|e| format!("Could not open FnHLauncher.zip: {e}"))?;

    let mut extracted_exe: Option<PathBuf> = None;

    for idx in 0..archive.len() {
        let mut entry = archive
            .by_index(idx)
            .map_err(|e| format!("Archive entry error: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        let Some(safe_rel) = entry.enclosed_name().map(|p| p.to_path_buf()) else {
            continue;
        };
        // Flatten single top-level directory if present, keeping files beside FH.exe.
        let file_name = match safe_rel.file_name() {
            Some(n) => PathBuf::from(n),
            None => continue,
        };
        let dst = install_dir.join(&file_name);
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("Extracting {}: {e}", file_name.display()))?;

        let tmp = dst.with_extension("setup-tmp");
        fs::write(&tmp, &buf).map_err(|e| format!("Writing {}: {e}", dst.display()))?;
        if let Err(_) = fs::rename(&tmp, &dst) {
            // If FH.exe is currently running, move old copy aside first on Windows.
            let old = dst.with_extension("old-setup");
            let _ = fs::remove_file(&old);
            let _ = fs::rename(&dst, &old);
            fs::rename(&tmp, &dst)
                .map_err(|e| format!("Close any running FH.exe and try again ({e})."))?;
            let _ = fs::remove_file(&old);
        }

        #[cfg(unix)]
        if file_name.to_string_lossy().eq_ignore_ascii_case("FH")
            || file_name.to_string_lossy().eq_ignore_ascii_case("FH.exe")
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&dst, fs::Permissions::from_mode(0o755));
        }

        if file_name.to_string_lossy().eq_ignore_ascii_case("FH.exe")
            || file_name.to_string_lossy().eq_ignore_ascii_case("FH")
        {
            extracted_exe = Some(dst);
        }
    }

    let fh_exe = extracted_exe.unwrap_or_else(|| install_dir.join("FH.exe"));
    if !fh_exe.is_file() {
        return Err("FH.exe was not found inside the downloaded FnHLauncher.zip.".into());
    }

    progress(93, "Configuring shortcuts…".into());
    let comment = "FnH — Fabric & Horizon Launcher (made by Buffer)";

    if make_desktop_shortcut {
        if let Some(desktop) = horizon_launcher::shortcut::desktop_dir() {
            let _ = create_platform_shortcut(&desktop, &fh_exe, comment);
        }
    }

    if make_start_menu_shortcut {
        if let Some(programs) = start_menu_programs_dir() {
            let _ = create_platform_shortcut(&programs, &fh_exe, comment);
        }
    }

    progress(100, "FnH installed! Launching now…".into());
    Ok(fh_exe)
}

fn create_platform_shortcut(dir: &Path, exe: &Path, comment: &str) -> Result<(), String> {
    let _ = fs::create_dir_all(dir);
    #[cfg(windows)]
    {
        let lnk = dir.join("FnH.lnk");
        horizon_launcher::shortcut::write_windows_link(&lnk, exe, "", comment)
    }
    #[cfg(not(windows))]
    {
        let desktop_file = dir.join("FnH.desktop");
        let body = format!(
            "[Desktop Entry]\nType=Application\nName=FnH\nComment={comment}\nExec=\"{}\"\nTerminal=false\nCategories=Game;\n",
            exe.display()
        );
        fs::write(&desktop_file, body).map_err(|e| e.to_string())
    }
}

fn spawn_installed_launcher(exe: &Path) {
    let mut cmd = Command::new(exe);
    if let Some(parent) = exe.parent() {
        cmd.current_dir(parent);
    }
    horizon_launcher::process::hide_console(&mut cmd);
    let _ = cmd.spawn();
}

#[cfg(target_os = "windows")]
fn apply_setup_window_icon() {
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumThreadWindows, GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL,
        IMAGE_ICON, LR_SHARED, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
    };

    static ICONS: OnceLock<(isize, isize)> = OnceLock::new();
    let (small, big) = *ICONS.get_or_init(|| unsafe {
        let hmod = GetModuleHandleW(std::ptr::null());
        if hmod.is_null() {
            return (0, 0);
        }
        let res_id = 1usize as *const u16;
        let sm = LoadImageW(
            hmod,
            res_id,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_SHARED,
        ) as isize;
        let lg = LoadImageW(
            hmod,
            res_id,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXICON),
            GetSystemMetrics(SM_CYICON),
            LR_SHARED,
        ) as isize;
        (sm, lg)
    });
    if small == 0 && big == 0 {
        return;
    }

    unsafe extern "system" fn set_icon_proc(hwnd: HWND, lparam: LPARAM) -> i32 {
        let &(sm, lg) = unsafe { &*(lparam as *const (isize, isize)) };
        unsafe {
            if sm != 0 {
                SendMessageW(hwnd, WM_SETICON, ICON_SMALL as WPARAM, sm as LPARAM);
            }
            if lg != 0 {
                SendMessageW(hwnd, WM_SETICON, ICON_BIG as WPARAM, lg as LPARAM);
            }
        }
        1
    }

    let pair = (small, big);
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(set_icon_proc),
            (&pair as *const (isize, isize)) as LPARAM,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn apply_setup_window_icon() {}

struct SetupWindow {
    install_dir: PathBuf,
    desktop_shortcut: bool,
    start_menu_shortcut: bool,
    shared: Arc<Mutex<SharedSetupState>>,
    launched: bool,
}

impl SetupWindow {
    fn new(shared: Arc<Mutex<SharedSetupState>>, cx: &mut Context<Self>) -> Self {
        apply_setup_window_icon();
        let poll_shared = shared.clone();
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(90))
                .await;
            let finished_exe = poll_shared
                .lock()
                .ok()
                .and_then(|s| match &s.phase {
                    SetupPhase::Finished { exe_path } => Some(exe_path.clone()),
                    _ => None,
                });
            let mut should_quit = false;
            let alive = this
                .update(cx, |view, cx| {
                    if let Some(exe) = finished_exe {
                        if !view.launched {
                            view.launched = true;
                            spawn_installed_launcher(&exe);
                            should_quit = true;
                        }
                    }
                    cx.notify();
                })
                .is_ok();
            if should_quit {
                cx.background_executor()
                    .timer(Duration::from_millis(450))
                    .await;
                let _ = cx.update(|cx| cx.quit());
                break;
            }
            if !alive {
                break;
            }
        })
        .detach();

        Self {
            install_dir: default_install_dir(),
            desktop_shortcut: true,
            start_menu_shortcut: true,
            shared,
            launched: false,
        }
    }

    fn start_install(&mut self, cx: &mut Context<Self>) {
        let desktop = self.desktop_shortcut;
        let start_menu = self.start_menu_shortcut;
        let dir = self.install_dir.clone();
        let shared = self.shared.clone();

        let existing_rel = shared.lock().ok().and_then(|mut s| {
            if matches!(s.phase, SetupPhase::Installing { .. }) {
                return None;
            }
            s.phase = SetupPhase::Installing {
                percent: 2,
                detail: "Preparing installer…".into(),
            };
            Some(s.release.clone())
        });
        let Some(maybe_rel) = existing_rel else {
            return;
        };
        cx.notify();

        std::thread::spawn(move || {
            let rel = maybe_rel.unwrap_or_else(fetch_latest_release_info);
            if let Ok(mut s) = shared.lock() {
                s.release = Some(rel.clone());
            }
            let shared_prog = shared.clone();
            let res = perform_install(
                rel,
                &dir,
                desktop,
                start_menu,
                &|pct, detail| {
                    if let Ok(mut s) = shared_prog.lock() {
                        s.phase = SetupPhase::Installing {
                            percent: pct,
                            detail,
                        };
                    }
                },
            );
            if let Ok(mut s) = shared.lock() {
                s.phase = match res {
                    Ok(exe_path) => SetupPhase::Finished { exe_path },
                    Err(err) => SetupPhase::Failed(err),
                };
            }
        });
    }
}

fn checkbox_row(
    id: impl Into<SharedString>,
    checked: bool,
    enabled: bool,
    title: &str,
    subtitle: &str,
) -> Stateful<gpui::Div> {
    let box_bg = if checked {
        hex(0x60cdff)
    } else {
        hexa(0xffffff, 0.05)
    };
    let box_border = if checked {
        hex(0x60cdff)
    } else {
        hexa(0xffffff, 0.28)
    };

    div()
        .id(id.into())
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(14.))
        .py(px(10.))
        .rounded(px(6.))
        .bg(hexa(0xffffff, 0.035))
        .border_1()
        .border_color(hexa(0xffffff, 0.08))
        .when(enabled, |d| {
            d.cursor_pointer()
                .hover(|s| s.bg(hexa(0xffffff, 0.07)).border_color(hexa(0xffffff, 0.16)))
        })
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .w(px(18.))
                .h(px(18.))
                .rounded(px(4.))
                .bg(box_bg)
                .border_1()
                .border_color(box_border)
                .text_size(px(12.))
                .font_weight(FontWeight::BOLD)
                .text_color(hex(0x06101a))
                .child(if checked { "✓" } else { "" }),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(1.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(hex(0xf4f6fb))
                        .child(title.to_owned()),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(hexa(0xd5dceb, 0.68))
                        .child(subtitle.to_owned()),
                ),
        )
}

impl Render for SetupWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let snap = self
            .shared
            .lock()
            .map(|s| s.clone())
            .unwrap_or(SharedSetupState {
                release: None,
                phase: SetupPhase::Ready,
            });

        let ver_badge = snap
            .release
            .as_ref()
            .map(|r| format!("Latest: {}", r.version_label))
            .unwrap_or_else(|| "Checking GitHub…".into());

        let busy = matches!(
            snap.phase,
            SetupPhase::Installing { .. } | SetupPhase::Finished { .. }
        );

        let header = div()
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_center()
                            .w(px(42.))
                            .h(px(42.))
                            .rounded(px(9.))
                            .bg(hex(0xf8f9fc))
                            .border_1()
                            .border_color(hex(0xd8dae5))
                            .text_size(px(13.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(hex(0x131254))
                            .child("FnH"),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(hex(0xffffff))
                                    .child("FnH Setup"),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(hexa(0xd5dceb, 0.72))
                                    .child("Fabric & Horizon Launcher · Made by Buffer"),
                            ),
                    ),
            )
            .child(
                div()
                    .px(px(9.))
                    .py(px(4.))
                    .rounded(px(99.))
                    .bg(hexa(0x60cdff, 0.14))
                    .border_1()
                    .border_color(hexa(0x60cdff, 0.35))
                    .text_size(px(11.5))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(hex(0x8adfff))
                    .child(ver_badge),
            );

        let shortcuts_section = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(hexa(0xd5dceb, 0.75))
                    .child("SHORTCUTS"),
            )
            .child(
                checkbox_row(
                    "chk-desktop",
                    self.desktop_shortcut,
                    !busy,
                    "Create a Desktop shortcut",
                    "Adds FnH to your Windows Desktop for quick one-click launch",
                )
                .when(!busy, |d| {
                    d.on_click(cx.listener(|this, _, _, cx| {
                        this.desktop_shortcut = !this.desktop_shortcut;
                        cx.notify();
                    }))
                }),
            )
            .child(
                checkbox_row(
                    "chk-startmenu",
                    self.start_menu_shortcut,
                    !busy,
                    "Create a Start Menu shortcut",
                    "Adds FnH to Windows Start Menu & Windows Search",
                )
                .when(!busy, |d| {
                    d.on_click(cx.listener(|this, _, _, cx| {
                        this.start_menu_shortcut = !this.start_menu_shortcut;
                        cx.notify();
                    }))
                }),
            );

        let path_note = div()
            .flex()
            .items_center()
            .justify_between()
            .px(px(12.))
            .py(px(7.))
            .rounded(px(5.))
            .bg(hexa(0x000000, 0.28))
            .border_1()
            .border_color(hexa(0xffffff, 0.06))
            .text_size(px(11.5))
            .text_color(hexa(0xd5dceb, 0.66))
            .child(format!("Install folder: {}", self.install_dir.display()));

        let status_area = match &snap.phase {
            SetupPhase::Ready => div()
                .text_size(px(12.))
                .text_color(hexa(0xd5dceb, 0.70))
                .child("Downloads the newest FnH release straight from GitHub (no admin rights needed)."),
            SetupPhase::Installing { percent, detail } => {
                let pct = (*percent).min(100) as f32 / 100.0;
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .text_size(px(12.))
                            .text_color(hex(0xeaf0fb))
                            .child(detail.clone())
                            .child(format!("{percent}%")),
                    )
                    .child(
                        div()
                            .w_full()
                            .h(px(6.))
                            .rounded(px(99.))
                            .bg(hexa(0xffffff, 0.10))
                            .overflow_hidden()
                            .child(
                                div()
                                    .h_full()
                                    .w(gpui::relative(pct))
                                    .rounded(px(99.))
                                    .bg(hex(0x60cdff)),
                            ),
                    )
            }
            SetupPhase::Finished { .. } => div()
                .text_size(px(12.5))
                .font_weight(FontWeight::MEDIUM)
                .text_color(hex(0x6ccb5f))
                .child("FnH is installed! Starting launcher…"),
            SetupPhase::Failed(err) => div()
                .text_size(px(12.))
                .text_color(hex(0xff8080))
                .child(format!("Error: {err}")),
        };

        let primary_label = match &snap.phase {
            SetupPhase::Ready => "Install & Launch FnH",
            SetupPhase::Installing { .. } => "Installing…",
            SetupPhase::Finished { .. } => "Launching…",
            SetupPhase::Failed(_) => "Retry Install",
        };

        let footer = div()
            .flex()
            .items_center()
            .justify_end()
            .gap(px(10.))
            .child(
                div()
                    .id("btn-cancel")
                    .px(px(16.))
                    .h(px(34.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .bg(hexa(0xffffff, 0.06))
                    .border_1()
                    .border_color(hexa(0xffffff, 0.12))
                    .text_size(px(12.5))
                    .text_color(hex(0xe6ebf5))
                    .when(!busy, |d| {
                        d.cursor_pointer()
                            .hover(|s| s.bg(hexa(0xffffff, 0.11)))
                            .on_click(|_, _, cx| cx.quit())
                    })
                    .child("Cancel"),
            )
            .child(
                div()
                    .id("btn-install")
                    .px(px(20.))
                    .h(px(34.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(px(5.))
                    .bg(if busy {
                        hexa(0x60cdff, 0.42)
                    } else {
                        hex(0x60cdff)
                    })
                    .text_size(px(13.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(hex(0x06101a))
                    .when(!busy, |d| {
                        d.cursor_pointer()
                            .hover(|s| s.bg(hex(0x7dd7ff)))
                            .on_click(cx.listener(|this, _, _, cx| this.start_install(cx)))
                    })
                    .child(primary_label),
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .justify_between()
            .p(px(22.))
            .bg(hex(0x0f121a))
            .font_family("Segoe UI")
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(16.))
                    .child(header)
                    .child(shortcuts_section)
                    .child(path_note)
                    .child(status_area),
            )
            .child(footer)
    }
}

fn main() {
    let mut quiet = false;
    let mut desktop = true;
    let mut start_menu = true;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--version" | "-V" => {
                use std::io::Write;
                let _ = writeln!(std::io::stdout(), "FnHSetup v{SETUP_VERSION}");
                return;
            }
            "--help" | "-h" => {
                use std::io::Write;
                let _ = writeln!(
                    std::io::stdout(),
                    "FnHSetup v{SETUP_VERSION} — Downloads & installs the latest FnH launcher.\n\
                     Usage: FnHSetup [--quiet] [--no-desktop] [--no-start-menu]"
                );
                return;
            }
            "--quiet" => quiet = true,
            "--no-desktop" => desktop = false,
            "--no-start-menu" => start_menu = false,
            _ => {}
        }
    }

    if quiet {
        let info = fetch_latest_release_info();
        let dir = default_install_dir();
        match perform_install(info, &dir, desktop, start_menu, &|_, _| {}) {
            Ok(_) => return,
            Err(e) => {
                eprintln!("FnHSetup failed: {e}");
                std::process::exit(1);
            }
        }
    }

    let shared = Arc::new(Mutex::new(SharedSetupState {
        release: None,
        phase: SetupPhase::Ready,
    }));
    let bg_shared = shared.clone();
    std::thread::spawn(move || {
        let rel = fetch_latest_release_info();
        if let Ok(mut s) = bg_shared.lock() {
            s.release = Some(rel);
        }
    });

    gpui_platform::application().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(376.)), cx);
        let shared_for_win = shared.clone();
        let handle = cx
            .open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    window_min_size: Some(size(px(500.), px(376.))),
                    is_resizable: false,
                    titlebar: Some(TitlebarOptions {
                        title: Some("FnH Setup".into()),
                        ..Default::default()
                    }),
                    app_id: Some("fnh-setup".into()),
                    ..Default::default()
                },
                move |_window, cx| cx.new(|cx| SetupWindow::new(shared_for_win, cx)),
            )
            .expect("FnH Setup window");

        let win_id = handle.window_id();
        cx.on_window_closed(move |cx, closed_id| {
            if closed_id == win_id || cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        cx.activate(true);
    });
}
