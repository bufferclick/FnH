//! Standalone per-instance editor window (`"Edit Instance (<Name>) - FNH"`), modelled on JLac's
//! `InstanceWindow` with all 11 sidebar pages:
//! - Log (live console stream, search, level filters, word wrap, copy, clear, folder)
//! - Version (Fabric MC/Loader switcher + Download All; Horizon track/build switcher + Install/Verify)
//! - Mods (drag-drop `.jar`/`.hmod`/`.hnmod`/folder with auto-porting on Horizon, ModBrowserWindow,
//!   Add file, Enable/Disable, Remove, Configs, Folder, search)
//! - Resource Packs
//! - Shader Packs
//! - Notes
//! - Worlds (Launch, Add Datapack, Datapacks Folder, Backup `.zip`, Duplicate, World Tool, Delete)
//! - Servers (`servers.dat` Join, Add, Copy IP, Remove)
//! - Screenshots (thumbnail grid, Open, Set as Instance Icon, Copy Path, Delete, Folder)
//! - Settings (General, Java / Horizon, Game Options `options.txt`, Custom Commands, Environment, Tweaks)
//! - Other Logs (`logs/` + `crash-reports/` tail viewer)

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui::prelude::*;
use gpui::{
    App, ClickEvent, ClipboardItem, Context, Entity, ExternalPaths, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement,
    PathPromptOptions, Render, SharedString, StatefulInteractiveElement, Styled, Window,
    WindowHandle, div, img, px,
};
use horizon_launcher::backend;
use horizon_launcher::browser::Kind as BrowseKind;
use horizon_launcher::instances::{self as inst, Instance};
use horizon_launcher::java;
use horizon_launcher::loaders::{self, Kind as LoaderKind, Loader};
use horizon_launcher::minecraft_versions::{self, Version as McVersion};
use horizon_launcher::server_list;
use horizon_launcher::worlds;

use crate::app::{Fh, IconTarget};
use crate::content::{self, LogFileItem, ModItem, PackItem, ScreenshotItem};
use crate::icons;
use crate::inst_icons;
use crate::job::{self, LaunchExtras};
use crate::log;
use crate::look::{
    MONO_FONT, Theme, UI_FONT, accent_button, apply_inline_rename_key, apply_text_key,
    checkbox_row, chip, danger_button, group_box, inline_rename_input, instance_summary,
    log_line_color, page_nav_row, push_button, side_action_button, status_banner, sub_tab,
    text_input,
};
use crate::prefs::{GameOptions, GpuBackend, GpuPower, HorizonTrack};
use crate::win_dialogs::secondary_window_options;


fn gbox(title: &str, sub: Option<&str>, t: &Theme, body: gpui::Div) -> gpui::Div {
    group_box(title, sub, t).child(body)
}

fn chk(
    id: impl Into<SharedString>,
    checked: bool,
    label: &str,
    hint: Option<&str>,
    t: &Theme,
) -> gpui::Stateful<gpui::Div> {
    checkbox_row(id, label, hint, checked, true, t)
}

fn side_btn(
    id: impl Into<SharedString>,
    glyph: Option<&'static str>,
    label: impl Into<SharedString>,
    enabled: bool,
    t: &Theme,
) -> gpui::Stateful<gpui::Div> {
    side_action_button(id, glyph, label, enabled, false, t)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstancePage {
    Log,
    Version,
    Mods,
    ResourcePacks,
    ShaderPacks,
    Notes,
    Worlds,
    Servers,
    Screenshots,
    Settings,
    OtherLogs,
}

impl InstancePage {
    pub const ALL: [InstancePage; 11] = [
        Self::Log,
        Self::Version,
        Self::Mods,
        Self::ResourcePacks,
        Self::ShaderPacks,
        Self::Notes,
        Self::Worlds,
        Self::Servers,
        Self::Screenshots,
        Self::Settings,
        Self::OtherLogs,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Log => "Minecraft Log",
            Self::Version => "Version",
            Self::Mods => "Mods",
            Self::ResourcePacks => "Resource Packs",
            Self::ShaderPacks => "Shader Packs",
            Self::Notes => "Notes",
            Self::Worlds => "Worlds",
            Self::Servers => "Servers",
            Self::Screenshots => "Screenshots",
            Self::Settings => "Settings",
            Self::OtherLogs => "Other Logs",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Version => "cube",
            Self::Mods => "mods",
            Self::ResourcePacks => "pack",
            Self::ShaderPacks => "shader",
            Self::Notes => "note",
            Self::Worlds => "world",
            Self::Servers => "server",
            Self::Screenshots => "camera",
            Self::Settings => "settings",
            Self::OtherLogs => "terminal",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActiveField {
    LogSearch,
    ModSearch,
    PackSearch,
    ShaderSearch,
    Notes,
    ServerName,
    ServerIp,
    InstName,
    InstGroup,
    WinW,
    WinH,
    OfflineName,
    AutoJoinServer,
    MinMem,
    MaxMem,
    JavaPath,
    JvmArgs,
    ExtraGameArgs,
    HzPinnedTag,
    HzFriendCode,
    HzRustLog,
    HzExtraArgs,
    OptRenderDist,
    OptSimDist,
    OptMaxFps,
    OptGuiScale,
    OptFov,
    OptGamma,
    OptMasterVol,
    OptMusicVol,
    PreCmd,
    WrapCmd,
    PostCmd,
    EnvKey,
    EnvVal,
    GlfwPath,
    OpenAlPath,
}

pub struct InstanceWindow {
    app: Entity<Fh>,
    pub instance_id: String,
    page: InstancePage,
    focus: FocusHandle,
    active_field: Option<ActiveField>,
    notice: Option<(String, bool)>,

    // Log page
    log_search: String,
    log_show_error: bool,
    log_show_warn: bool,
    log_show_info: bool,
    log_show_other: bool,
    log_wrap: bool,
    seen_console_rev: u64,

    // Version page (Fabric/Java)
    mc_versions: Arc<Mutex<Option<Result<Vec<McVersion>, String>>>>,
    fabric_versions: Arc<Mutex<Option<(String, Result<Vec<String>, String>)>>>,
    ver_show_snapshots: bool,
    ver_show_old: bool,

    // Mods / Packs / Shaders
    mod_search: String,
    selected_mod: usize,
    renaming_mod: Option<(ModItem, String, bool)>,
    pack_search: String,
    selected_pack: usize,
    shader_search: String,
    selected_shader: usize,

    // Notes
    notes_text: String,
    notes_dirty: bool,

    // Worlds
    selected_world: usize,

    // Servers
    selected_server: usize,
    new_server_name: String,
    new_server_ip: String,

    // Screenshots
    selected_shot: usize,

    // Settings sub-tab (0..6)
    settings_tab: usize,
    edit_name: String,
    edit_group: String,
    game_opts: GameOptions,
    new_env_key: String,
    new_env_val: String,

    // Other Logs
    selected_log_file: usize,
}

impl InstanceWindow {
    pub fn open(
        app: Entity<Fh>,
        instance_id: String,
        initial: InstancePage,
        cx: &mut App,
    ) -> Option<WindowHandle<Self>> {
        let inst_opt = app
            .read(cx)
            .instances
            .iter()
            .find(|i| i.id == instance_id)
            .cloned()
            .or_else(|| inst::find(&instance_id).ok());
        let instance = inst_opt?;
        let title = format!("Edit Instance ({}) - FnH", instance.name);
        let opts = secondary_window_options(title, 980., 680., 760., 520., cx);

        cx.open_window(opts, move |window, cx| {
            crate::look::apply_window_icon();
            cx.new(|cx| {
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                cx.observe(&app, |this: &mut Self, app_ent, cx| {
                    if let Some(inst) = app_ent
                        .read(cx)
                        .instances
                        .iter()
                        .find(|i| i.id == this.instance_id)
                    {
                        if this.active_field != Some(ActiveField::InstName) {
                            this.edit_name = inst.name.clone();
                        }
                        if this.active_field != Some(ActiveField::InstGroup) {
                            this.edit_group = inst.group.clone().unwrap_or_default();
                        }
                    }
                    cx.notify();
                })
                .detach();

                let prefs = app.read(cx).prefs.clone();
                let notes_text = content::load_notes(&instance.home);
                let game_opts = GameOptions::load(&instance.home);

                let mc_versions: Arc<Mutex<Option<Result<Vec<McVersion>, String>>>> =
                    Arc::new(Mutex::new(None));
                let fabric_versions: Arc<Mutex<Option<(String, Result<Vec<String>, String>)>>> =
                    Arc::new(Mutex::new(None));

                if let Some(j) = instance.java.as_ref() {
                    let mc_slot = mc_versions.clone();
                    std::thread::spawn(move || {
                        let res = minecraft_versions::load().map(|v| v.all);
                        if let Ok(mut s) = mc_slot.lock() {
                            *s = Some(res);
                        }
                    });
                    let mc_cur = j.minecraft.clone();
                    let fab_slot = fabric_versions.clone();
                    std::thread::spawn(move || {
                        let res = loaders::versions(
                            LoaderKind::Fabric,
                            &mc_cur,
                            &backend::get_bytes,
                        )
                        .map(|list| list.into_iter().map(|x| x.version).collect::<Vec<_>>());
                        if let Ok(mut s) = fab_slot.lock() {
                            *s = Some((mc_cur, res));
                        }
                    });
                }

                // Live ticker for console log + background task updates
                let id_tick = instance.id.clone();
                cx.spawn(async move |this, cx| {
                    loop {
                        cx.background_executor()
                            .timer(Duration::from_millis(200))
                            .await;
                        let rev = job::console_revision(&id_tick);
                        let is_busy = job::busy();
                        let alive = this.update(cx, |view, cx| {
                            if rev != view.seen_console_rev || is_busy {
                                view.seen_console_rev = rev;
                                cx.notify();
                            }
                        });
                        if alive.is_err() {
                            break;
                        }
                    }
                })
                .detach();

                Self {
                    app,
                    instance_id: instance.id.clone(),
                    page: initial,
                    focus,
                    active_field: None,
                    notice: None,
                    log_search: String::new(),
                    log_show_error: true,
                    log_show_warn: true,
                    log_show_info: true,
                    log_show_other: true,
                    log_wrap: prefs.console_word_wrap,
                    seen_console_rev: job::console_revision(&instance.id),
                    mc_versions,
                    fabric_versions,
                    ver_show_snapshots: prefs.show_snapshots_default,
                    ver_show_old: prefs.show_old_versions_default,
                    mod_search: String::new(),
                    selected_mod: 0,
                    renaming_mod: None,
                    pack_search: String::new(),
                    selected_pack: 0,
                    shader_search: String::new(),
                    selected_shader: 0,
                    notes_text,
                    notes_dirty: false,
                    selected_world: 0,
                    selected_server: 0,
                    new_server_name: String::new(),
                    new_server_ip: String::new(),
                    selected_shot: 0,
                    settings_tab: 0,
                    edit_name: instance.name.clone(),
                    edit_group: instance.group.clone().unwrap_or_default(),
                    game_opts,
                    new_env_key: String::new(),
                    new_env_val: String::new(),
                    selected_log_file: 0,
                }
            })
        })
        .ok()
    }

    pub fn set_page(&mut self, page: InstancePage, cx: &mut Context<Self>) {
        self.page = page;
        self.active_field = None;
        cx.notify();
    }

    fn current_instance(&self, cx: &App) -> Option<Instance> {
        self.app
            .read(cx)
            .instances
            .iter()
            .find(|i| i.id == self.instance_id)
            .cloned()
            .or_else(|| inst::find(&self.instance_id).ok())
    }

    fn update_cfg(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut crate::prefs::InstanceCfg)) {
        let id = self.instance_id.clone();
        self.app.update(cx, |app, cx| {
            app.cfgs.update(&id, f);
            cx.notify();
        });
        cx.notify();
    }

    fn fetch_fabric_for(&mut self, mc: String) {
        let slot = self.fabric_versions.clone();
        if let Ok(mut s) = slot.lock() {
            *s = None;
        }
        std::thread::spawn(move || {
            let res = loaders::versions(LoaderKind::Fabric, &mc, &backend::get_bytes)
                .map(|list| list.into_iter().map(|x| x.version).collect::<Vec<_>>());
            if let Ok(mut s) = slot.lock() {
                *s = Some((mc, res));
            }
        });
    }

    fn launch_now(&mut self, extras: LaunchExtras, cx: &mut Context<Self>) {
        if job::busy() {
            self.notice = Some(("Another task or instance is already running.".into(), true));
            cx.notify();
            return;
        }
        let Some(instance) = self.current_instance(cx) else {
            return;
        };
        let id = instance.id.clone();
        self.app.update(cx, |app, cx| {
            app.launch_instance(&id, extras, cx);
        });
        cx.notify();
    }

    fn start_mod_rename(&mut self, item: &ModItem, window: &mut Window, cx: &mut Context<Self>) {
        if self
            .renaming_mod
            .as_ref()
            .is_some_and(|(cur, _, _)| cur.path != item.path)
        {
            self.commit_mod_rename(cx);
        }
        self.active_field = None;
        self.renaming_mod = Some((item.clone(), item.name.clone(), true));
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn commit_mod_rename(&mut self, cx: &mut Context<Self>) {
        let Some((item, raw_name, _)) = self.renaming_mod.take() else {
            return;
        };
        let trimmed = raw_name.trim();
        if trimmed.is_empty() || trimmed == item.name {
            cx.notify();
            return;
        }
        if let Some(inst) = self.current_instance(cx) {
            match content::rename_mod(&inst.home, &item, trimmed) {
                Ok(new_name) => {
                    self.notice = Some((format!("Renamed mod to '{new_name}'."), false));
                }
                Err(e) => self.notice = Some((e, true)),
            }
        }
        cx.notify();
    }

    fn filtered_mods(&self, instance: &Instance) -> Vec<ModItem> {
        let all_mods = content::list_mods(instance);
        let q = self.mod_search.trim().to_ascii_lowercase();
        all_mods
            .into_iter()
            .filter(|m| {
                q.is_empty()
                    || m.name.to_ascii_lowercase().contains(&q)
                    || m.id.to_ascii_lowercase().contains(&q)
                    || m.file_label.to_ascii_lowercase().contains(&q)
            })
            .collect()
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let km = &ev.keystroke.modifiers;

        // Global Ctrl+K kills any running game or active prepare process
        if (km.control || km.platform) && key.eq_ignore_ascii_case("k") {
            if job::can_kill() {
                job::kill();
                self.notice = Some(("Stopped running game.".into(), false));
                cx.notify();
            }
            return;
        }

        // Prism-style inline mod rename mode
        if self.renaming_mod.is_some() {
            match key {
                "escape" => {
                    self.renaming_mod = None;
                    cx.notify();
                    return;
                }
                "enter" => {
                    self.commit_mod_rename(cx);
                    return;
                }
                _ => {}
            }
            if let Some((_, buf, select_all)) = &mut self.renaming_mod {
                if apply_inline_rename_key(buf, select_all, ev, cx) {
                    cx.notify();
                }
            }
            return;
        }

        if key == "escape" {
            if self.active_field.is_some() {
                self.active_field = None;
                cx.notify();
            } else {
                window.remove_window();
            }
            return;
        }

        let Some(field) = self.active_field else {
            if self.page == InstancePage::Mods {
                if let Some(inst) = self.current_instance(cx) {
                    let mods = self.filtered_mods(&inst);
                    if !mods.is_empty() {
                        let sel = self.selected_mod.min(mods.len() - 1);
                        match key {
                            "f2" => {
                                let m = mods[sel].clone();
                                self.start_mod_rename(&m, window, cx);
                                return;
                            }
                            "space" => {
                                let m = &mods[sel];
                                if let Err(e) = content::set_mod_enabled(&inst.home, m, !m.enabled)
                                {
                                    self.notice = Some((e, true));
                                }
                                cx.notify();
                                return;
                            }
                            "delete" => {
                                let m = &mods[sel];
                                match content::remove_mod(&inst.home, m) {
                                    Ok(()) => {
                                        self.notice =
                                            Some((format!("Removed '{}'.", m.name), false));
                                    }
                                    Err(e) => self.notice = Some((e, true)),
                                }
                                cx.notify();
                                return;
                            }
                            "up" => {
                                self.selected_mod = sel.saturating_sub(1);
                                cx.notify();
                                return;
                            }
                            "down" => {
                                self.selected_mod = (sel + 1).min(mods.len() - 1);
                                cx.notify();
                                return;
                            }
                            _ => {}
                        }
                    }
                }
            }
            return;
        };

        if field == ActiveField::Notes {
            let mods = &ev.keystroke.modifiers;
            if (mods.control || mods.platform) && key.eq_ignore_ascii_case("s") {
                if let Some(inst) = self.current_instance(cx) {
                    match content::save_notes(&inst.home, &self.notes_text) {
                        Ok(()) => {
                            self.notes_dirty = false;
                            self.notice = Some(("Saved instance notes.".into(), false));
                        }
                        Err(e) => self.notice = Some((e, true)),
                    }
                    cx.notify();
                }
                return;
            }
            if key == "enter" {
                self.notes_text.push('\n');
                self.notes_dirty = true;
                cx.notify();
                return;
            }
            if apply_text_key(&mut self.notes_text, ev, true, cx) {
                self.notes_dirty = true;
                cx.notify();
            }
            return;
        }

        // Local string fields
        match field {
            ActiveField::LogSearch => {
                if apply_text_key(&mut self.log_search, ev, false, cx) {
                    cx.notify();
                }
                return;
            }
            ActiveField::ModSearch => {
                if apply_text_key(&mut self.mod_search, ev, false, cx) {
                    self.selected_mod = 0;
                    cx.notify();
                }
                return;
            }
            ActiveField::PackSearch => {
                if apply_text_key(&mut self.pack_search, ev, false, cx) {
                    self.selected_pack = 0;
                    cx.notify();
                }
                return;
            }
            ActiveField::ShaderSearch => {
                if apply_text_key(&mut self.shader_search, ev, false, cx) {
                    self.selected_shader = 0;
                    cx.notify();
                }
                return;
            }
            ActiveField::ServerName => {
                if apply_text_key(&mut self.new_server_name, ev, false, cx) {
                    cx.notify();
                }
                return;
            }
            ActiveField::ServerIp => {
                if apply_text_key(&mut self.new_server_ip, ev, false, cx) {
                    cx.notify();
                }
                return;
            }
            ActiveField::InstName => {
                if key == "enter" {
                    self.commit_name_and_group(cx);
                    self.active_field = None;
                    cx.notify();
                    return;
                }
                if apply_text_key(&mut self.edit_name, ev, false, cx) {
                    cx.notify();
                }
                return;
            }
            ActiveField::InstGroup => {
                if key == "enter" {
                    self.commit_name_and_group(cx);
                    self.active_field = None;
                    cx.notify();
                    return;
                }
                if apply_text_key(&mut self.edit_group, ev, false, cx) {
                    cx.notify();
                }
                return;
            }
            ActiveField::EnvKey => {
                if apply_text_key(&mut self.new_env_key, ev, false, cx) {
                    cx.notify();
                }
                return;
            }
            ActiveField::EnvVal => {
                if apply_text_key(&mut self.new_env_val, ev, false, cx) {
                    cx.notify();
                }
                return;
            }
            _ => {}
        }

        // GameOptions numeric fields
        if matches!(
            field,
            ActiveField::OptRenderDist
                | ActiveField::OptSimDist
                | ActiveField::OptMaxFps
                | ActiveField::OptGuiScale
                | ActiveField::OptFov
                | ActiveField::OptGamma
                | ActiveField::OptMasterVol
                | ActiveField::OptMusicVol
        ) {
            let mut cur = match field {
                ActiveField::OptRenderDist => self.game_opts.render_distance.to_string(),
                ActiveField::OptSimDist => self.game_opts.simulation_distance.to_string(),
                ActiveField::OptMaxFps => self.game_opts.max_fps.to_string(),
                ActiveField::OptGuiScale => self.game_opts.gui_scale.to_string(),
                ActiveField::OptFov => self.game_opts.fov_deg.to_string(),
                ActiveField::OptGamma => self.game_opts.gamma_pct.to_string(),
                ActiveField::OptMasterVol => self.game_opts.master_volume_pct.to_string(),
                ActiveField::OptMusicVol => self.game_opts.music_volume_pct.to_string(),
                _ => String::new(),
            };
            if apply_text_key(&mut cur, ev, false, cx) {
                let digits: String = cur.chars().filter(|c| c.is_ascii_digit()).collect();
                let n = digits.parse::<u32>().unwrap_or(0);
                match field {
                    ActiveField::OptRenderDist => self.game_opts.render_distance = n.min(64),
                    ActiveField::OptSimDist => self.game_opts.simulation_distance = n.min(32),
                    ActiveField::OptMaxFps => self.game_opts.max_fps = n.min(260),
                    ActiveField::OptGuiScale => self.game_opts.gui_scale = n.min(6),
                    ActiveField::OptFov => self.game_opts.fov_deg = n.min(110),
                    ActiveField::OptGamma => self.game_opts.gamma_pct = n.min(500),
                    ActiveField::OptMasterVol => self.game_opts.master_volume_pct = n.min(100),
                    ActiveField::OptMusicVol => self.game_opts.music_volume_pct = n.min(100),
                    _ => {}
                }
                cx.notify();
            }
            return;
        }

        // InstanceCfg backed fields
        let cfg = self.app.read(cx).cfgs.get(&self.instance_id);
        let mut s = match field {
            ActiveField::WinW => cfg.window_width.to_string(),
            ActiveField::WinH => cfg.window_height.to_string(),
            ActiveField::OfflineName => cfg.offline_name.clone(),
            ActiveField::AutoJoinServer => cfg.auto_join_server.clone(),
            ActiveField::MinMem => cfg.min_mem_mb.to_string(),
            ActiveField::MaxMem => cfg.max_mem_mb.to_string(),
            ActiveField::JavaPath => cfg.java_path.clone(),
            ActiveField::JvmArgs => cfg.jvm_args.clone(),
            ActiveField::ExtraGameArgs => cfg.extra_game_args.clone(),
            ActiveField::HzPinnedTag => cfg.horizon_pinned_version.clone(),
            ActiveField::HzFriendCode => cfg.horizon_friend_code.clone(),
            ActiveField::HzRustLog => cfg.horizon_rust_log.clone(),
            ActiveField::HzExtraArgs => cfg.horizon_extra_args.clone(),
            ActiveField::PreCmd => cfg.pre_launch_cmd.clone(),
            ActiveField::WrapCmd => cfg.wrapper_cmd.clone(),
            ActiveField::PostCmd => cfg.post_exit_cmd.clone(),
            ActiveField::GlfwPath => cfg.custom_glfw_path.clone(),
            ActiveField::OpenAlPath => cfg.custom_openal_path.clone(),
            _ => return,
        };

        if apply_text_key(&mut s, ev, false, cx) {
            let digits_only = matches!(
                field,
                ActiveField::WinW | ActiveField::WinH | ActiveField::MinMem | ActiveField::MaxMem
            );
            let parsed_u32 = if digits_only {
                let d: String = s.chars().filter(|c| c.is_ascii_digit()).collect();
                d.parse::<u32>().unwrap_or(0)
            } else {
                0
            };
            self.update_cfg(cx, |c| match field {
                ActiveField::WinW => c.window_width = parsed_u32.min(7680),
                ActiveField::WinH => c.window_height = parsed_u32.min(4320),
                ActiveField::OfflineName => c.offline_name = s,
                ActiveField::AutoJoinServer => c.auto_join_server = s,
                ActiveField::MinMem => c.min_mem_mb = parsed_u32.min(65536),
                ActiveField::MaxMem => c.max_mem_mb = parsed_u32.min(65536),
                ActiveField::JavaPath => c.java_path = s,
                ActiveField::JvmArgs => c.jvm_args = s,
                ActiveField::ExtraGameArgs => c.extra_game_args = s,
                ActiveField::HzPinnedTag => c.horizon_pinned_version = s,
                ActiveField::HzFriendCode => c.horizon_friend_code = s,
                ActiveField::HzRustLog => c.horizon_rust_log = s,
                ActiveField::HzExtraArgs => c.horizon_extra_args = s,
                ActiveField::PreCmd => c.pre_launch_cmd = s,
                ActiveField::WrapCmd => c.wrapper_cmd = s,
                ActiveField::PostCmd => c.post_exit_cmd = s,
                ActiveField::GlfwPath => c.custom_glfw_path = s,
                ActiveField::OpenAlPath => c.custom_openal_path = s,
                _ => {}
            });
        }
    }

    fn commit_name_and_group(&mut self, cx: &mut Context<Self>) {
        let name = self.edit_name.trim().to_owned();
        if name.is_empty() {
            self.notice = Some(("Instance name cannot be empty.".into(), true));
            return;
        }
        let grp = self.edit_group.trim().to_owned();
        let id = self.instance_id.clone();
        match inst::edit(&id, |m| {
            m.name = Some(name.clone());
            m.group = (!grp.is_empty()).then_some(grp);
        }) {
            Ok(_) => {
                self.app.update(cx, |app, cx| {
                    app.reload_instances();
                    cx.notify();
                });
                self.notice = Some((format!("Saved '{}' metadata.", name), false));
            }
            Err(e) => self.notice = Some((e, true)),
        }
    }

    fn render_field(
        &self,
        id: impl Into<SharedString>,
        value: &str,
        placeholder: &str,
        field: ActiveField,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Stateful<gpui::Div> {
        let focused = self.active_field == Some(field);
        text_input(id, value, placeholder, focused, t).on_click(cx.listener(
            move |this, _: &ClickEvent, window, cx| {
                this.active_field = Some(field);
                window.focus(&this.focus, cx);
                cx.notify();
            },
        ))
    }

    // ============================================================================================
    // Page 1: Minecraft / Horizon Live Log
    // ============================================================================================
    fn render_log_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let prefs = self.app.read(cx).prefs.clone();
        let (lines, total_count) = job::console_snapshot(&instance.id, 600);
        let q = self.log_search.trim().to_ascii_lowercase();

        let filtered: Vec<String> = lines
            .into_iter()
            .filter(|line| {
                let up = line.to_ascii_uppercase();
                let is_err = up.contains("ERROR")
                    || up.contains("FATAL")
                    || up.contains("EXCEPTION")
                    || line.starts_with("[FNH] Error");
                let is_warn = !is_err && up.contains("WARN");
                let is_info = !is_err && !is_warn && (up.contains("INFO") || line.starts_with("[FNH]"));
                let level_ok = if is_err {
                    self.log_show_error
                } else if is_warn {
                    self.log_show_warn
                } else if is_info {
                    self.log_show_info
                } else {
                    self.log_show_other
                };
                if !level_ok {
                    return false;
                }
                if !q.is_empty() && !line.to_ascii_lowercase().contains(&q) {
                    return false;
                }
                true
            })
            .collect();

        let font_sz = prefs.console_font_size.clamp(9, 20) as f32;
        let highlight = prefs.console_highlight_levels;
        let wrap = self.log_wrap;

        let id_copy = instance.id.clone();
        let id_clear = instance.id.clone();
        let logs_dir = instance.home.join("logs");

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div().w(px(220.)).child(self.render_field(
                            "log-search",
                            &self.log_search,
                            "Filter console lines...",
                            ActiveField::LogSearch,
                            t,
                            cx,
                        )),
                    )
                    .child(chip("lvl-err", "Errors", self.log_show_error, t).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.log_show_error = !this.log_show_error;
                            cx.notify();
                        }),
                    ))
                    .child(chip("lvl-warn", "Warnings", self.log_show_warn, t).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.log_show_warn = !this.log_show_warn;
                            cx.notify();
                        }),
                    ))
                    .child(chip("lvl-info", "Info", self.log_show_info, t).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.log_show_info = !this.log_show_info;
                            cx.notify();
                        }),
                    ))
                    .child(chip("lvl-other", "Other", self.log_show_other, t).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.log_show_other = !this.log_show_other;
                            cx.notify();
                        }),
                    ))
                    .child(chip("log-wrap", "Wrap", self.log_wrap, t).on_click(cx.listener(
                        |this, _: &ClickEvent, _, cx| {
                            this.log_wrap = !this.log_wrap;
                            cx.notify();
                        },
                    )))
                    .child(div().flex_1())
                    .child(
                        push_button("log-copy", "Copy", true, t).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                let full = job::console_full_text(&id_copy);
                                cx.write_to_clipboard(ClipboardItem::new_string(full));
                                this.notice = Some(("Copied console log to clipboard.".into(), false));
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        push_button("log-clear", "Clear", true, t).on_click(cx.listener(
                            move |_, _: &ClickEvent, _, cx| {
                                job::clear_console(&id_clear);
                                cx.notify();
                            },
                        )),
                    )
                    .child(
                        push_button("log-folder", "Logs Folder", true, t).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&logs_dir);
                                if let Err(e) = log::open_folder(&logs_dir) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .id("console-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(t.divider)
                    .bg(t.card)
                    .p(px(10.))
                    .font_family(MONO_FONT)
                    .text_size(px(font_sz))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .when(filtered.is_empty(), |d| {
                        d.child(
                            div().text_color(t.ink_tertiary).child(if total_count == 0 {
                                "No console output yet. Click Launch below to stream stdout/stderr live."
                            } else {
                                "No console lines match your current filter."
                            }),
                        )
                    })
                    .children(filtered.into_iter().map(|line| {
                        let col = log_line_color(&line, highlight, t);
                        let row = div().text_color(col).child(line);
                        if wrap {
                            row
                        } else {
                            row.whitespace_nowrap()
                        }
                    })),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.ink_tertiary)
                    .child(format!(
                        "{} buffered line(s)  ·  Working dir: {}",
                        total_count,
                        instance.home.display()
                    )),
            )
    }

    // ============================================================================================
    // Page 2: Version (Fabric MC/Loader switcher OR Horizon build/channel switcher)
    // ============================================================================================
    fn render_version_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let is_busy = job::busy();
        if let Some(java_cfg) = instance.java.as_ref() {
            let cur_mc = java_cfg.minecraft.clone();
            let cur_loader = java_cfg.loader.clone();

            let mc_list = self
                .mc_versions
                .lock()
                .ok()
                .and_then(|g| g.clone());
            let fab_list = self
                .fabric_versions
                .lock()
                .ok()
                .and_then(|g| g.clone());

            let show_snaps = self.ver_show_snapshots;
            let show_old = self.ver_show_old;

            let inst_prefetch = instance.clone();

            div().flex_1().min_h_0().flex().flex_col().child(div()
                .id("ver-java-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(gbox(
                    "Active Stack",
                    Some("Components resolved when launching this instance"),
                    t,
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(4.))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(t.ink)
                                        .child(format!("Minecraft {cur_mc}")),
                                )
                                .child(
                                    div().text_size(px(12.)).text_color(t.ink_secondary).child(
                                        match &cur_loader {
                                            Some(l) => format!(
                                                "{} Loader {}",
                                                l.kind.name(),
                                                l.version
                                            ),
                                            None => "Vanilla (no mod loader)".to_owned(),
                                        },
                                    ),
                                ),
                        )
                        .child(
                            accent_button("ver-dl-all", "Download All", !is_busy, t).when(
                                !is_busy,
                                |b| {
                                    b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        let prefs = this.app.read(cx).prefs.clone();
                                        let cfg = this.app.read(cx).cfgs.get(&inst_prefetch.id);
                                        job::prefetch_instance(inst_prefetch.clone(), prefs, cfg);
                                        this.notice = Some((
                                            "Prefetching libraries, assets & Java runtime...".into(),
                                            false,
                                        ));
                                        cx.notify();
                                    }))
                                },
                            ),
                        ),
                ))
                .child(gbox(
                    "Fabric Loader",
                    Some("Toggle Fabric or switch loader version"),
                    t,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    chip("ver-ldr-none", "None (Vanilla)", cur_loader.is_none(), t)
                                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            let id = this.instance_id.clone();
                                            let _ = inst::edit(&id, |m| {
                                                if let Some(j) = m.java.as_mut() {
                                                    j.loader = None;
                                                }
                                            });
                                            this.app.update(cx, |a, cx| {
                                                a.reload_instances();
                                                cx.notify();
                                            });
                                            cx.notify();
                                        })),
                                )
                                .child(
                                    chip("ver-ldr-fab", "Fabric", cur_loader.is_some(), t).on_click(
                                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            let id = this.instance_id.clone();
                                            let mc_for_ldr = cur_mc.clone();
                                            let latest_fab = this
                                                .fabric_versions
                                                .lock()
                                                .ok()
                                                .and_then(|s| {
                                                    s.as_ref().and_then(|(m, r)| {
                                                        (m == &mc_for_ldr)
                                                            .then(|| r.as_ref().ok())
                                                            .flatten()
                                                            .and_then(|v| v.first().cloned())
                                                    })
                                                })
                                                .unwrap_or_else(|| "latest".to_owned());
                                            let chosen = if latest_fab == "latest" {
                                                loaders::choose("fabric", &mc_for_ldr, None).ok()
                                            } else {
                                                Some(Loader {
                                                    kind: LoaderKind::Fabric,
                                                    version: latest_fab,
                                                })
                                            };
                                            if let Some(ldr) = chosen {
                                                let _ = inst::edit(&id, |m| {
                                                    if let Some(j) = m.java.as_mut() {
                                                        j.loader = Some(ldr);
                                                    }
                                                });
                                                this.app.update(cx, |a, cx| {
                                                    a.reload_instances();
                                                    cx.notify();
                                                });
                                            }
                                            cx.notify();
                                        }),
                                    ),
                                ),
                        )
                        .when(cur_loader.is_some(), |d| match fab_list {
                            None => d.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.ink_tertiary)
                                    .child("Loading Fabric Loader versions..."),
                            ),
                            Some((_, Err(e))) => d.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.danger)
                                    .child(format!("Could not fetch Fabric versions: {e}")),
                            ),
                            Some((_, Ok(vers))) => d.child(
                                div()
                                    .id("ver-fab-list")
                                    .max_h(px(140.))
                                    .overflow_y_scroll()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(5.))
                                    .children(vers.into_iter().take(24).enumerate().map(
                                        |(i, fv)| {
                                            let active = cur_loader
                                                .as_ref()
                                                .is_some_and(|l| l.version == fv);
                                            let fv_set = fv.clone();
                                            chip(format!("fv-{i}"), fv, active, t).on_click(
                                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                    let id = this.instance_id.clone();
                                                    let fv_val = fv_set.clone();
                                                    let _ = inst::edit(&id, |m| {
                                                        if let Some(j) = m.java.as_mut() {
                                                            j.loader = Some(Loader {
                                                                kind: LoaderKind::Fabric,
                                                                version: fv_val,
                                                            });
                                                        }
                                                    });
                                                    this.app.update(cx, |a, cx| {
                                                        a.reload_instances();
                                                        cx.notify();
                                                    });
                                                    cx.notify();
                                                }),
                                            )
                                        },
                                    )),
                            ),
                        }),
                ))
                .child(gbox(
                    "Minecraft Version",
                    Some("Switch target Minecraft release or snapshot"),
                    t,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    chip("mc-flt-snap", "Snapshots", show_snaps, t).on_click(
                                        cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.ver_show_snapshots = !this.ver_show_snapshots;
                                            cx.notify();
                                        }),
                                    ),
                                )
                                .child(
                                    chip("mc-flt-old", "Old Alpha/Beta", show_old, t).on_click(
                                        cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.ver_show_old = !this.ver_show_old;
                                            cx.notify();
                                        }),
                                    ),
                                ),
                        )
                        .child(match mc_list {
                            None => div()
                                .text_size(px(12.))
                                .text_color(t.ink_tertiary)
                                .child("Loading Mojang version manifest..."),
                            Some(Err(e)) => div()
                                .text_size(px(12.))
                                .text_color(t.danger)
                                .child(format!("Could not load versions: {e}")),
                            Some(Ok(all)) => {
                                let cur_mc_cmp = java_cfg.minecraft.clone();
                                let filtered: Vec<McVersion> = all
                                    .into_iter()
                                    .filter(|v| match v.kind {
                                        minecraft_versions::Kind::Release => true,
                                        minecraft_versions::Kind::Snapshot => show_snaps,
                                        minecraft_versions::Kind::OldBeta
                                        | minecraft_versions::Kind::OldAlpha => show_old,
                                    })
                                    .take(80)
                                    .collect();
                                div().child(div()
                                    .id("ver-mc-list")
                                    .max_h(px(220.))
                                    .overflow_y_scroll()
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(t.divider)
                                    .bg(t.window)
                                    .p(px(4.))
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .children(filtered.into_iter().enumerate().map(|(i, mv)| {
                                        let selected = mv.id == cur_mc_cmp;
                                        let id_pick = mv.id.clone();
                                        let kind_lbl = match mv.kind {
                                            minecraft_versions::Kind::Release => "Release",
                                            minecraft_versions::Kind::Snapshot => "Snapshot",
                                            minecraft_versions::Kind::OldBeta => "Beta",
                                            minecraft_versions::Kind::OldAlpha => "Alpha",
                                        };
                                        div()
                                            .id(SharedString::from(format!("mc-row-{i}")))
                                            .h(px(28.))
                                            .px(px(10.))
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .flex()
                                            .items_center()
                                            .justify_between()
                                            .bg(if selected {
                                                t.tile_selected
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .hover(|s| s.bg(t.subtle_hover))
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _, cx| {
                                                    let id = this.instance_id.clone();
                                                    let new_mc = id_pick.clone();
                                                    let _ = inst::edit(&id, |m| {
                                                        if let Some(j) = m.java.as_mut() {
                                                            j.minecraft = new_mc.clone();
                                                            if j.loader.is_some() {
                                                                j.loader = loaders::choose(
                                                                    "fabric", &new_mc, None,
                                                                )
                                                                .ok();
                                                            }
                                                        }
                                                    });
                                                    this.fetch_fabric_for(new_mc.clone());
                                                    this.app.update(cx, |a, cx| {
                                                        a.reload_instances();
                                                        cx.notify();
                                                    });
                                                    this.notice = Some((
                                                        format!("Switched to Minecraft {new_mc}."),
                                                        false,
                                                    ));
                                                    cx.notify();
                                                },
                                            ))
                                            .child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .text_color(t.ink)
                                                    .child(mv.id),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(t.ink_tertiary)
                                                    .child(format!("{kind_lbl} · {}", mv.released)),
                                            )
                                    })))
                            }
                        }),
                )))
        } else {
            // Horizon Version Page
            let cfg = self.app.read(cx).cfgs.get(&instance.id);
            let installed = backend::installed();
            let inst_prefetch = instance.clone();

            div().flex_1().min_h_0().flex().flex_col().child(div()
                .id("ver-hz-scroll")
                .flex_1()
                .min_h_0()
                .overflow_y_scroll()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(gbox(
                    "Horizon Track",
                    Some("Choose which Horizon channel or pinned build this instance launches"),
                    t,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .child(
                            div().flex().flex_wrap().gap(px(6.)).children(
                                HorizonTrack::ALL.into_iter().enumerate().map(|(i, tr)| {
                                    chip(
                                        format!("hz-tr-{i}"),
                                        tr.label(),
                                        cfg.horizon_track == tr,
                                        t,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _, cx| {
                                            this.update_cfg(cx, |c| c.horizon_track = tr);
                                        },
                                    ))
                                }),
                            ),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    accent_button("hz-inst-rel", "Install Latest Release", !is_busy, t)
                                        .when(!is_busy, |b| {
                                            b.on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| {
                                                    job::install_horizon_channel(false);
                                                    this.notice = Some((
                                                        "Installing latest Horizon stable build..."
                                                            .into(),
                                                        false,
                                                    ));
                                                    cx.notify();
                                                },
                                            ))
                                        }),
                                )
                                .child(
                                    push_button("hz-inst-nig", "Install Nightly", !is_busy, t)
                                        .when(!is_busy, |b| {
                                            b.on_click(cx.listener(
                                                |this, _: &ClickEvent, _, cx| {
                                                    job::install_horizon_channel(true);
                                                    this.notice = Some((
                                                        "Installing latest Horizon nightly build..."
                                                            .into(),
                                                        false,
                                                    ));
                                                    cx.notify();
                                                },
                                            ))
                                        }),
                                )
                                .child(
                                    push_button("hz-verify", "Prepare / Verify Assets", !is_busy, t)
                                        .when(!is_busy, |b| {
                                            b.on_click(cx.listener(
                                                move |this, _: &ClickEvent, _, cx| {
                                                    let prefs = this.app.read(cx).prefs.clone();
                                                    let cfg = this
                                                        .app
                                                        .read(cx)
                                                        .cfgs
                                                        .get(&inst_prefetch.id);
                                                    job::prefetch_instance(
                                                        inst_prefetch.clone(),
                                                        prefs,
                                                        cfg,
                                                    );
                                                    this.notice = Some((
                                                        "Preparing & verifying Horizon assets..."
                                                            .into(),
                                                        false,
                                                    ));
                                                    cx.notify();
                                                },
                                            ))
                                        }),
                                ),
                        ),
                ))
                .child(gbox(
                    "Installed Horizon Builds",
                    Some("Click Pin to lock this instance to a specific installed tag"),
                    t,
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .when(installed.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.ink_tertiary)
                                    .child("No Horizon builds installed yet. Click 'Install Latest Release' above or simply Launch."),
                            )
                        })
                        .children(installed.into_iter().enumerate().map(|(i, tag)| {
                            let is_pinned = cfg.horizon_track == HorizonTrack::Pinned
                                && cfg.horizon_pinned_version == tag;
                            let prep = backend::is_prepared(&tag);
                            let tag_pin = tag.clone();
                            div()
                                .px(px(10.))
                                .py(px(7.))
                                .rounded(px(4.))
                                .border_1()
                                .border_color(if is_pinned { t.accent } else { t.divider })
                                .bg(if is_pinned { t.tile_selected } else { t.window })
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .flex()
                                        .flex_col()
                                        .child(
                                            div()
                                                .text_size(px(12.5))
                                                .font_weight(FontWeight::MEDIUM)
                                                .text_color(t.ink)
                                                .child(tag),
                                        )
                                        .child(
                                            div()
                                                .text_size(px(11.))
                                                .text_color(t.ink_tertiary)
                                                .child(if prep {
                                                    "Assets prepared"
                                                } else {
                                                    "Prepares automatically on launch"
                                                }),
                                        ),
                                )
                                .child(
                                    push_button(
                                        format!("hz-pin-{i}"),
                                        if is_pinned { "Pinned" } else { "Pin Build" },
                                        true,
                                        t,
                                    )
                                    .on_click(cx.listener(
                                        move |this, _: &ClickEvent, _, cx| {
                                            let tp = tag_pin.clone();
                                            let id = this.instance_id.clone();
                                            let _ = inst::edit(&id, |m| {
                                                m.version = Some(tp.clone());
                                            });
                                            this.update_cfg(cx, |c| {
                                                c.horizon_track = HorizonTrack::Pinned;
                                                c.horizon_pinned_version = tp;
                                            });
                                            this.app.update(cx, |a, cx| {
                                                a.reload_instances();
                                                cx.notify();
                                            });
                                        },
                                    )),
                                )
                        })),
                )))
        }
    }

    // ============================================================================================
    // Page 3: Mods (Fabric `.jar` + Horizon `.hmod`/`.hnmod`/folder/auto-port `.jar`)
    // ============================================================================================
    fn render_mods_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let mods = self.filtered_mods(instance);

        let sel = self.selected_mod.min(mods.len().saturating_sub(1));
        let cur_mod = mods.get(sel).cloned();

        let inst_drop = instance.clone();
        let inst_add = instance.clone();
        let inst_dl = instance.clone();
        let mods_dir = content::mods_folder(instance);
        let cfg_dir = instance.home.join("config");
        let home_toggle = instance.home.clone();
        let home_en_all = instance.home.clone();
        let home_dis_all = instance.home.clone();
        let mods_en_all = mods.clone();
        let mods_dis_all = mods.clone();
        let home_rm = instance.home.clone();
        let is_java = instance.java.is_some();

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(px(10.))
            .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                let files: Vec<PathBuf> = paths.paths().to_vec();
                match content::add_mod_files(&inst_drop, &files) {
                    Ok(msg) => this.notice = Some((msg, false)),
                    Err(e) => this.notice = Some((e, true)),
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div().flex_1().child(self.render_field(
                                    "mod-search",
                                    &self.mod_search,
                                    if is_java {
                                        "Filter Fabric mods (.jar)..."
                                    } else {
                                        "Filter Horizon mods (.hmod / .hnmod / folder / ported .jar)..."
                                    },
                                    ActiveField::ModSearch,
                                    t,
                                    cx,
                                )),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(t.ink_tertiary)
                                    .child(format!("{} mod(s)", mods.len())),
                            ),
                    )
                    .child(
                        div()
                            .id("mods-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(t.divider)
                            .bg(t.window)
                            .p(px(6.))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.commit_mod_rename(cx);
                                this.active_field = None;
                                window.focus(&this.focus, cx);
                                cx.notify();
                            }))
                            .drag_over::<ExternalPaths>({
                                let accent = t.accent;
                                let sel_bg = t.tile_selected;
                                move |s, _, _, _| s.border_color(accent).bg(sel_bg)
                            })
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .when(mods.is_empty(), |d| {
                                d.child(
                                    div()
                                        .p(px(16.))
                                        .text_size(px(12.5))
                                        .text_color(t.ink_tertiary)
                                        .child(if is_java {
                                            "No mods in this instance yet. Click 'Download Mods' or drag & drop .jar files here."
                                        } else {
                                            "No mods in this Horizon instance yet. Click 'Download Mods' or drag & drop .hmod, .hnmod, mod folders, or Fabric .jar packs (auto-ported via horizon-port-pack)."
                                        }),
                                )
                            })
                            .children(mods.iter().enumerate().map(|(i, m)| {
                                let selected = i == sel;
                                let home_chk = instance.home.clone();
                                let item_chk = m.clone();
                                let item_ren = m.clone();
                                let row_path = m.path.clone();
                                let renaming_state = self
                                    .renaming_mod
                                    .as_ref()
                                    .filter(|(rm, _, _)| rm.path == m.path)
                                    .map(|(_, buf, sa)| (buf.clone(), *sa));

                                div()
                                    .id(SharedString::from(format!("mod-row-{i}")))
                                    .px(px(10.))
                                    .py(px(7.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(if selected {
                                        t.accent
                                    } else {
                                        gpui::transparent_black()
                                    })
                                    .bg(if selected {
                                        t.tile_selected
                                    } else {
                                        t.layer
                                    })
                                    .hover(|s| s.bg(t.subtle_hover))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                        cx.stop_propagation();
                                        if this
                                            .renaming_mod
                                            .as_ref()
                                            .is_some_and(|(rm, _, _)| rm.path != row_path)
                                        {
                                            this.commit_mod_rename(cx);
                                        } else if this.renaming_mod.is_some() {
                                            this.commit_mod_rename(cx);
                                        }
                                        this.active_field = None;
                                        this.selected_mod = i;
                                        window.focus(&this.focus, cx);
                                        cx.notify();
                                    }))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.))
                                            .min_w_0()
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!("mod-en-{i}")))
                                                    .w(px(16.))
                                                    .h(px(16.))
                                                    .rounded(px(3.))
                                                    .border_1()
                                                    .border_color(if m.enabled {
                                                        t.accent
                                                    } else {
                                                        t.control_border
                                                    })
                                                    .bg(if m.enabled { t.accent } else { t.window })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, window, cx| {
                                                            cx.stop_propagation();
                                                            this.commit_mod_rename(cx);
                                                            this.active_field = None;
                                                            window.focus(&this.focus, cx);
                                                            if let Err(e) = content::set_mod_enabled(
                                                                &home_chk,
                                                                &item_chk,
                                                                !item_chk.enabled,
                                                            ) {
                                                                this.notice = Some((e, true));
                                                            }
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .when(m.enabled, |b| {
                                                        b.child(icons::icon(
                                                            "check",
                                                            12.,
                                                            t.on_accent,
                                                        ))
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .min_w_0()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        if let Some((buf, sel_all)) = renaming_state
                                                        {
                                                            div()
                                                                .w(px(260.))
                                                                .child(
                                                                    inline_rename_input(
                                                                        format!("mod-ren-{i}"),
                                                                        &buf,
                                                                        sel_all,
                                                                        false,
                                                                        t,
                                                                    )
                                                                    .on_mouse_down(
                                                                        MouseButton::Left,
                                                                        |_, _, cx| {
                                                                            cx.stop_propagation()
                                                                        },
                                                                    )
                                                                    .on_click(cx.listener(
                                                                        |this,
                                                                         _: &ClickEvent,
                                                                         window,
                                                                         cx| {
                                                                            cx.stop_propagation();
                                                                            if let Some((
                                                                                _,
                                                                                _,
                                                                                sa,
                                                                            )) = &mut this
                                                                                .renaming_mod
                                                                            {
                                                                                *sa = false;
                                                                            }
                                                                            window.focus(
                                                                                &this.focus,
                                                                                cx,
                                                                            );
                                                                            cx.notify();
                                                                        },
                                                                    )),
                                                                )
                                                                .into_any_element()
                                                        } else {
                                                            div()
                                                                .id(SharedString::from(format!(
                                                                    "mod-name-{i}"
                                                                )))
                                                                .px(px(3.))
                                                                .rounded(px(3.))
                                                                .cursor_text()
                                                                .hover(|s| s.bg(t.subtle_hover))
                                                                .text_size(px(12.5))
                                                                .font_weight(FontWeight::MEDIUM)
                                                                .text_color(if m.enabled {
                                                                    t.ink
                                                                } else {
                                                                    t.ink_tertiary
                                                                })
                                                                .truncate()
                                                                .on_click(cx.listener(
                                                                    move |this,
                                                                          _: &ClickEvent,
                                                                          window,
                                                                          cx| {
                                                                        cx.stop_propagation();
                                                                        this.selected_mod = i;
                                                                        this.start_mod_rename(
                                                                            &item_ren, window, cx,
                                                                        );
                                                                    },
                                                                ))
                                                                .child(m.name.clone())
                                                                .into_any_element()
                                                        },
                                                    )
                                                    .child(
                                                        div()
                                                            .text_size(px(11.))
                                                            .text_color(t.ink_tertiary)
                                                            .truncate()
                                                            .child(format!(
                                                                "{}  ·  {}",
                                                                m.version, m.file_label
                                                            )),
                                                    ),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .px(px(6.))
                                            .py(px(2.))
                                            .rounded(px(3.))
                                            .bg(t.card)
                                            .text_size(px(10.5))
                                            .text_color(t.ink_secondary)
                                            .child(m.format.badge()),
                                    )
                            })),
                    ),
            )
            // Right action column (JLac ModFolderPage sidebar)
            .child(
                div()
                    .id("mods-side-col")
                    .w(px(176.))
                    .flex_shrink_0()
                    .min_h_0()
                    .overflow_y_scroll()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        accent_button("mod-dl", "Download Mods", true, t).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                let inst_c = inst_dl.clone();
                                this.app.update(cx, |app, cx| {
                                    app.open_mod_browser(inst_c, BrowseKind::Mod, cx);
                                });
                            },
                        )),
                    )
                    .child(
                        side_btn("mod-add", Some("add"), "Add File...", true, t).on_click(
                            cx.listener(move |_, _: &ClickEvent, _, cx| {
                                let rx = cx.prompt_for_paths(PathPromptOptions {
                                    files: true,
                                    directories: !is_java,
                                    multiple: true,
                                    prompt: Some("Select mod file(s)".into()),
                                });
                                let inst_target = inst_add.clone();
                                cx.spawn(async move |view, cx| {
                                    if let Ok(Ok(Some(paths))) = rx.await {
                                        let res = content::add_mod_files(&inst_target, &paths);
                                        let _ = view.update(cx, |v, cx| {
                                            match res {
                                                Ok(m) => v.notice = Some((m, false)),
                                                Err(e) => v.notice = Some((e, true)),
                                            }
                                            cx.notify();
                                        });
                                    }
                                })
                                .detach();
                            }),
                        ),
                    )
                    .child({
                        let m_ren = cur_mod.clone();
                        side_btn("mod-rename", Some("edit"), "Rename (F2)", m_ren.is_some(), t)
                            .when_some(m_ren, |b, m| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.start_mod_rename(&m, window, cx);
                                }))
                            })
                    })
                    .child({
                        let m_tog = cur_mod.clone();
                        let lbl = match &m_tog {
                            Some(m) if m.enabled => "Disable",
                            _ => "Enable",
                        };
                        side_btn("mod-toggle", None, lbl, m_tog.is_some(), t).when_some(
                            m_tog,
                            |b, m| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if let Err(e) =
                                        content::set_mod_enabled(&home_toggle, &m, !m.enabled)
                                    {
                                        this.notice = Some((e, true));
                                    }
                                    cx.notify();
                                }))
                            },
                        )
                    })
                    .child(
                        side_btn("mod-en-all", None, "Enable All", !mods_en_all.is_empty(), t)
                            .when(!mods_en_all.is_empty(), |b| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    for m in &mods_en_all {
                                        if !m.enabled {
                                            let _ = content::set_mod_enabled(&home_en_all, m, true);
                                        }
                                    }
                                    cx.notify();
                                    let _ = this;
                                }))
                            }),
                    )
                    .child(
                        side_btn("mod-dis-all", None, "Disable All", !mods_dis_all.is_empty(), t)
                            .when(!mods_dis_all.is_empty(), |b| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    for m in &mods_dis_all {
                                        if m.enabled {
                                            let _ =
                                                content::set_mod_enabled(&home_dis_all, m, false);
                                        }
                                    }
                                    cx.notify();
                                    let _ = this;
                                }))
                            }),
                    )
                    .child({
                        let m_rm = cur_mod.clone();
                        danger_button("mod-rm", "Remove", m_rm.is_some(), t).when_some(
                            m_rm,
                            |b, m| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    match content::remove_mod(&home_rm, &m) {
                                        Ok(()) => {
                                            this.notice =
                                                Some((format!("Removed '{}'.", m.name), false));
                                        }
                                        Err(e) => this.notice = Some((e, true)),
                                    }
                                    cx.notify();
                                }))
                            },
                        )
                    })
                    .child(div().h(px(1.)).bg(t.divider).my(px(4.)))
                    .child(
                        side_btn("mod-cfgs", Some("folder"), "View Configs", true, t)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&cfg_dir);
                                if let Err(e) = log::open_folder(&cfg_dir) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            })),
                    )
                    .child(
                        side_btn("mod-folder", Some("folder"), "View Folder", true, t)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&mods_dir);
                                if let Err(e) = log::open_folder(&mods_dir) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }

    // ============================================================================================
    // Pages 4 & 5: Resource Packs & Shader Packs
    // ============================================================================================
    fn render_packs_page(
        &self,
        instance: &Instance,
        shaders: bool,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let folder = if shaders {
            "shaderpacks"
        } else {
            "resourcepacks"
        };
        let browse_kind = if shaders {
            BrowseKind::Shader
        } else {
            BrowseKind::ResourcePack
        };
        let search_val = if shaders {
            &self.shader_search
        } else {
            &self.pack_search
        };
        let search_field = if shaders {
            ActiveField::ShaderSearch
        } else {
            ActiveField::PackSearch
        };

        let q = search_val.trim().to_ascii_lowercase();
        let packs: Vec<PackItem> = content::list_packs(&instance.home, folder)
            .into_iter()
            .filter(|p| q.is_empty() || p.name.to_ascii_lowercase().contains(&q))
            .collect();

        let sel_idx = if shaders {
            self.selected_shader
        } else {
            self.selected_pack
        }
        .min(packs.len().saturating_sub(1));
        let cur_pack = packs.get(sel_idx).cloned();

        let home_drop = instance.home.clone();
        let home_add = instance.home.clone();
        let dir_open = instance.home.join(folder);
        let inst_dl = instance.clone();
        let folder_s = folder.to_owned();
        let folder_add = folder.to_owned();

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(px(10.))
            .on_drop(cx.listener(move |this, paths: &ExternalPaths, _, cx| {
                let files: Vec<PathBuf> = paths.paths().to_vec();
                match content::add_pack_files(&home_drop, &folder_s, &files) {
                    Ok(m) => this.notice = Some((m, false)),
                    Err(e) => this.notice = Some((e, true)),
                }
                cx.notify();
            }))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(self.render_field(
                        if shaders { "shd-search" } else { "rp-search" },
                        search_val,
                        if shaders {
                            "Filter shader packs..."
                        } else {
                            "Filter resource packs..."
                        },
                        search_field,
                        t,
                        cx,
                    ))
                    .child(
                        div()
                            .id(SharedString::from(format!("packs-list-{folder}")))
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(t.divider)
                            .bg(t.window)
                            .p(px(6.))
                            .drag_over::<ExternalPaths>({
                                let accent = t.accent;
                                let sel_bg = t.tile_selected;
                                move |s, _, _, _| s.border_color(accent).bg(sel_bg)
                            })
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .when(packs.is_empty(), |d| {
                                d.child(
                                    div()
                                        .p(px(16.))
                                        .text_size(px(12.5))
                                        .text_color(t.ink_tertiary)
                                        .child("None installed yet. Click 'Download' or drag & drop .zip packs here."),
                                )
                            })
                            .children(packs.iter().enumerate().map(|(i, p)| {
                                let selected = i == sel_idx;
                                let p_chk = p.clone();
                                div()
                                    .id(SharedString::from(format!("pk-{folder}-{i}")))
                                    .px(px(10.))
                                    .py(px(7.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(if selected {
                                        t.accent
                                    } else {
                                        gpui::transparent_black()
                                    })
                                    .bg(if selected {
                                        t.tile_selected
                                    } else {
                                        t.layer
                                    })
                                    .hover(|s| s.bg(t.subtle_hover))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        if shaders {
                                            this.selected_shader = i;
                                        } else {
                                            this.selected_pack = i;
                                        }
                                        cx.notify();
                                    }))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(10.))
                                            .child(
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "pk-en-{folder}-{i}"
                                                    )))
                                                    .w(px(16.))
                                                    .h(px(16.))
                                                    .rounded(px(3.))
                                                    .border_1()
                                                    .border_color(if p.enabled {
                                                        t.accent
                                                    } else {
                                                        t.control_border
                                                    })
                                                    .bg(if p.enabled { t.accent } else { t.window })
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, _, cx| {
                                                            if let Err(e) =
                                                                content::set_pack_enabled(
                                                                    &p_chk,
                                                                    !p_chk.enabled,
                                                                )
                                                            {
                                                                this.notice = Some((e, true));
                                                            }
                                                            cx.notify();
                                                        },
                                                    ))
                                                    .when(p.enabled, |b| {
                                                        b.child(icons::icon(
                                                            "check",
                                                            12.,
                                                            t.on_accent,
                                                        ))
                                                    }),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .text_color(if p.enabled {
                                                        t.ink
                                                    } else {
                                                        t.ink_tertiary
                                                    })
                                                    .child(p.name.clone()),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(t.ink_tertiary)
                                            .child(content::format_bytes(p.size_bytes)),
                                    )
                            })),
                    ),
            )
            .child(
                div()
                    .w(px(176.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(
                        accent_button("pk-dl", "Download", true, t).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                let inst_c = inst_dl.clone();
                                this.app.update(cx, |app, cx| {
                                    app.open_mod_browser(inst_c, browse_kind, cx);
                                });
                            },
                        )),
                    )
                    .child(
                        side_btn("pk-add", Some("add"), "Add File...", true, t).on_click(
                            cx.listener(move |_, _: &ClickEvent, _, cx| {
                                let rx = cx.prompt_for_paths(PathPromptOptions {
                                    files: true,
                                    directories: true,
                                    multiple: true,
                                    prompt: Some("Select pack(s)".into()),
                                });
                                let h = home_add.clone();
                                let fld = folder_add.clone();
                                cx.spawn(async move |view, cx| {
                                    if let Ok(Ok(Some(paths))) = rx.await {
                                        let res = content::add_pack_files(&h, &fld, &paths);
                                        let _ = view.update(cx, |v, cx| {
                                            match res {
                                                Ok(m) => v.notice = Some((m, false)),
                                                Err(e) => v.notice = Some((e, true)),
                                            }
                                            cx.notify();
                                        });
                                    }
                                })
                                .detach();
                            }),
                        ),
                    )
                    .child({
                        let p_tog = cur_pack.clone();
                        let lbl = match &p_tog {
                            Some(p) if p.enabled => "Disable",
                            _ => "Enable",
                        };
                        side_btn("pk-tog", None, lbl, p_tog.is_some(), t).when_some(
                            p_tog,
                            |b, p| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if let Err(e) = content::set_pack_enabled(&p, !p.enabled) {
                                        this.notice = Some((e, true));
                                    }
                                    cx.notify();
                                }))
                            },
                        )
                    })
                    .child({
                        let p_rm = cur_pack;
                        danger_button("pk-rm", "Remove", p_rm.is_some(), t).when_some(
                            p_rm,
                            |b, p| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    match content::remove_pack(&p) {
                                        Ok(()) => {
                                            this.notice =
                                                Some((format!("Removed '{}'.", p.name), false));
                                        }
                                        Err(e) => this.notice = Some((e, true)),
                                    }
                                    cx.notify();
                                }))
                            },
                        )
                    })
                    .child(div().h(px(1.)).bg(t.divider).my(px(4.)))
                    .child(
                        side_btn("pk-fld", Some("folder"), "View Folder", true, t)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&dir_open);
                                if let Err(e) = log::open_folder(&dir_open) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }

    // ============================================================================================
    // Page 6: Notes
    // ============================================================================================
    fn render_notes_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let focused = self.active_field == Some(ActiveField::Notes);
        let home_save = instance.home.clone();
        let notes_file = instance.home.join("fh-notes.txt");
        let editor_cmd = self.app.read(cx).prefs.text_editor_cmd.clone();

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.ink_secondary)
                            .child(if self.notes_dirty {
                                "Unsaved changes — press Ctrl+S or click Save Notes"
                            } else {
                                "Per-instance notes (saved in fh-notes.txt)"
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                push_button("notes-ext", "Open in Editor", true, t).on_click(
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        let _ = content::save_notes(&notes_file.parent().unwrap_or(&notes_file), &this.notes_text);
                                        if let Err(e) = log::open_file(&notes_file, &editor_cmd) {
                                            this.notice = Some((e, true));
                                            cx.notify();
                                        }
                                    }),
                                ),
                            )
                            .child(
                                accent_button("notes-save", "Save Notes", true, t).on_click(
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        match content::save_notes(&home_save, &this.notes_text) {
                                            Ok(()) => {
                                                this.notes_dirty = false;
                                                this.notice =
                                                    Some(("Saved instance notes.".into(), false));
                                            }
                                            Err(e) => this.notice = Some((e, true)),
                                        }
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
            )
            .child(
                div()
                    .id("notes-area")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(if focused { t.accent } else { t.divider })
                    .bg(t.input_bg)
                    .p(px(12.))
                    .cursor_text()
                    .font_family(UI_FONT)
                    .text_size(px(13.))
                    .text_color(if self.notes_text.is_empty() {
                        t.ink_tertiary
                    } else {
                        t.ink
                    })
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.active_field = Some(ActiveField::Notes);
                        window.focus(&this.focus, cx);
                        cx.notify();
                    }))
                    .child(if self.notes_text.is_empty() {
                        "Click here to jot down modlist notes, seed coordinates, JVM flags, or build plans...".to_owned()
                    } else if focused {
                        format!("{}|", self.notes_text)
                    } else {
                        self.notes_text.clone()
                    }),
            )
    }

    // ============================================================================================
    // Page 7: Worlds (Launch, Datapacks, Backup, Duplicate, World Tool, Delete)
    // ============================================================================================
    fn render_worlds_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let all_worlds = worlds::list(&instance.home);
        let sel = self.selected_world.min(all_worlds.len().saturating_sub(1));
        let cur_world = all_worlds.get(sel).cloned();
        let saves_dir = instance.home.join("saves");
        let home_backup = instance.home.clone();
        let world_tool = self.app.read(cx).prefs.world_tool_cmd.clone();
        let is_busy = job::busy();

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(px(10.))
            .child(
                div()
                    .id("worlds-list")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(t.divider)
                    .bg(t.window)
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .when(all_worlds.is_empty(), |d| {
                        d.child(
                            div()
                                .p(px(16.))
                                .text_size(px(12.5))
                                .text_color(t.ink_tertiary)
                                .child("No singleplayer worlds saved in this instance yet."),
                        )
                    })
                    .children(all_worlds.iter().enumerate().map(|(i, w)| {
                        let selected = i == sel;
                        div()
                            .id(SharedString::from(format!("wld-row-{i}")))
                            .px(px(10.))
                            .py(px(8.))
                            .rounded(px(4.))
                            .cursor_pointer()
                            .border_1()
                            .border_color(if selected {
                                t.accent
                            } else {
                                gpui::transparent_black()
                            })
                            .bg(if selected {
                                t.tile_selected
                            } else {
                                t.layer
                            })
                            .hover(|s| s.bg(t.subtle_hover))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.selected_world = i;
                                cx.notify();
                            }))
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(t.ink)
                                            .child(w.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.))
                                            .text_color(t.ink_tertiary)
                                            .child(format!(
                                                "{}  ·  Folder: {}",
                                                w.mode, w.folder
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(11.))
                                    .text_color(t.ink_secondary)
                                    .child(content::format_bytes(w.size)),
                            )
                    })),
            )
            .child(
                div()
                    .w(px(176.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child({
                        let w_join = cur_world.clone();
                        accent_button("wld-join", "Join World", w_join.is_some() && !is_busy, t)
                            .when_some(w_join.filter(|_| !is_busy), |b, w| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.launch_now(
                                        LaunchExtras {
                                            world: Some(w.folder.clone()),
                                            server: None,
                                            offline: false,
                                        },
                                        cx,
                                    );
                                }))
                            })
                    })
                    .child({
                        let w_dp = cur_world.clone();
                        side_btn(
                            "wld-add-dp",
                            Some("add"),
                            "Add Datapack...",
                            w_dp.is_some(),
                            t,
                        )
                        .when_some(w_dp, |b, w| {
                            b.on_click(cx.listener(move |_, _: &ClickEvent, _, cx| {
                                let rx = cx.prompt_for_paths(PathPromptOptions {
                                    files: true,
                                    directories: false,
                                    multiple: true,
                                    prompt: Some("Select datapack .zip file(s)".into()),
                                });
                                let w_path = w.path.clone();
                                let dp_dir = w_path.join("datapacks");
                                cx.spawn(async move |view, cx| {
                                    if let Ok(Ok(Some(paths))) = rx.await {
                                        let res =
                                            content::add_pack_files(&w_path, "datapacks", &paths);
                                        let _ = std::fs::create_dir_all(&dp_dir);
                                        let _ = view.update(cx, |v, cx| {
                                            match res {
                                                Ok(m) => v.notice = Some((m, false)),
                                                Err(e) => v.notice = Some((e, true)),
                                            }
                                            cx.notify();
                                        });
                                    }
                                })
                                .detach();
                            }))
                        })
                    })
                    .child({
                        let w_dp_fld = cur_world.clone();
                        side_btn(
                            "wld-dp-fld",
                            Some("folder"),
                            "Datapacks Folder",
                            w_dp_fld.is_some(),
                            t,
                        )
                        .when_some(w_dp_fld, |b, w| {
                            b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let dp = w.path.join("datapacks");
                                let _ = std::fs::create_dir_all(&dp);
                                if let Err(e) = log::open_folder(&dp) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            }))
                        })
                    })
                    .child({
                        let w_bak = cur_world.clone();
                        side_btn("wld-bak", None, "Backup (.zip)", w_bak.is_some(), t)
                            .when_some(w_bak, |b, w| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    match worlds::backup(&home_backup, &w) {
                                        Ok(zip_path) => {
                                            this.notice = Some((
                                                format!("Backed up to {}.", zip_path.display()),
                                                false,
                                            ));
                                        }
                                        Err(e) => this.notice = Some((e, true)),
                                    }
                                    cx.notify();
                                }))
                            })
                    })
                    .child({
                        let w_dup = cur_world.clone();
                        side_btn("wld-dup", Some("copy"), "Duplicate", w_dup.is_some(), t)
                            .when_some(w_dup, |b, w| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    match worlds::copy(&w) {
                                        Ok(nw) => {
                                            this.notice = Some((
                                                format!("Duplicated as '{}'.", nw.name),
                                                false,
                                            ));
                                        }
                                        Err(e) => this.notice = Some((e, true)),
                                    }
                                    cx.notify();
                                }))
                            })
                    })
                    .child({
                        let w_tool = cur_world.clone();
                        side_btn("wld-tool", None, "Open in Tool", w_tool.is_some(), t)
                            .when_some(w_tool, |b, w| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if let Err(e) = log::run_world_tool(&world_tool, &w.path) {
                                        this.notice = Some((e, true));
                                        cx.notify();
                                    }
                                }))
                            })
                    })
                    .child({
                        let w_del = cur_world;
                        danger_button("wld-del", "Delete World", w_del.is_some(), t).when_some(
                            w_del,
                            |b, w| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    match worlds::delete(&w) {
                                        Ok(()) => {
                                            this.notice =
                                                Some((format!("Deleted '{}'.", w.name), false));
                                        }
                                        Err(e) => this.notice = Some((e, true)),
                                    }
                                    cx.notify();
                                }))
                            },
                        )
                    })
                    .child(div().h(px(1.)).bg(t.divider).my(px(4.)))
                    .child(
                        side_btn("wld-saves", Some("folder"), "Saves Folder", true, t)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&saves_dir);
                                if let Err(e) = log::open_folder(&saves_dir) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }

    // ============================================================================================
    // Page 8: Servers (`servers.dat`)
    // ============================================================================================
    fn render_servers_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let list = server_list::List::load(&instance.home);
        let servers = list.servers();
        let sel = self.selected_server.min(servers.len().saturating_sub(1));
        let cur_srv = servers.get(sel).cloned();
        let home_add = instance.home.clone();
        let home_rm = instance.home.clone();
        let is_busy = job::busy();

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(px(10.))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div().w(px(180.)).child(self.render_field(
                                    "srv-name",
                                    &self.new_server_name,
                                    "Server name",
                                    ActiveField::ServerName,
                                    t,
                                    cx,
                                )),
                            )
                            .child(
                                div().flex_1().child(self.render_field(
                                    "srv-ip",
                                    &self.new_server_ip,
                                    "play.example.net:25565",
                                    ActiveField::ServerIp,
                                    t,
                                    cx,
                                )),
                            )
                            .child(
                                accent_button("srv-add-btn", "Add Server", true, t).on_click(
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        let mut lst = server_list::List::load(&home_add);
                                        let nm = if this.new_server_name.trim().is_empty() {
                                            "Minecraft Server"
                                        } else {
                                            this.new_server_name.trim()
                                        };
                                        match lst.add(nm, this.new_server_ip.trim()) {
                                            Ok(()) => match lst.save(&home_add) {
                                                Ok(()) => {
                                                    this.new_server_name.clear();
                                                    this.new_server_ip.clear();
                                                    this.notice =
                                                        Some(("Added server to servers.dat.".into(), false));
                                                }
                                                Err(e) => this.notice = Some((e, true)),
                                            },
                                            Err(e) => this.notice = Some((e, true)),
                                        }
                                        cx.notify();
                                    }),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .id("servers-list")
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .rounded(px(6.))
                            .border_1()
                            .border_color(t.divider)
                            .bg(t.window)
                            .p(px(6.))
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .when(servers.is_empty(), |d| {
                                d.child(
                                    div()
                                        .p(px(16.))
                                        .text_size(px(12.5))
                                        .text_color(t.ink_tertiary)
                                        .child("No saved multiplayer servers in servers.dat yet."),
                                )
                            })
                            .children(servers.iter().enumerate().map(|(i, s)| {
                                let selected = i == sel;
                                div()
                                    .id(SharedString::from(format!("srv-row-{i}")))
                                    .px(px(10.))
                                    .py(px(8.))
                                    .rounded(px(4.))
                                    .cursor_pointer()
                                    .border_1()
                                    .border_color(if selected {
                                        t.accent
                                    } else {
                                        gpui::transparent_black()
                                    })
                                    .bg(if selected {
                                        t.tile_selected
                                    } else {
                                        t.layer
                                    })
                                    .hover(|st| st.bg(t.subtle_hover))
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.selected_server = i;
                                        cx.notify();
                                    }))
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(t.ink)
                                            .child(s.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .font_family(MONO_FONT)
                                            .text_size(px(11.5))
                                            .text_color(t.ink_secondary)
                                            .child(s.ip.clone()),
                                    )
                            })),
                    ),
            )
            .child(
                div()
                    .w(px(176.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child({
                        let s_join = cur_srv.clone();
                        accent_button("srv-join", "Join Server", s_join.is_some() && !is_busy, t)
                            .when_some(s_join.filter(|_| !is_busy), |b, s| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.launch_now(
                                        LaunchExtras {
                                            world: None,
                                            server: Some(s.ip.clone()),
                                            offline: false,
                                        },
                                        cx,
                                    );
                                }))
                            })
                    })
                    .child({
                        let s_cp = cur_srv.clone();
                        side_btn("srv-copy", Some("copy"), "Copy Address", s_cp.is_some(), t)
                            .when_some(s_cp, |b, s| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(s.ip.clone()));
                                    this.notice =
                                        Some((format!("Copied '{}'.", s.ip), false));
                                    cx.notify();
                                }))
                            })
                    })
                    .child({
                        let has_sel = cur_srv.is_some();
                        danger_button("srv-rm", "Remove", has_sel, t).when(has_sel, |b| {
                            b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let mut lst = server_list::List::load(&home_rm);
                                match lst.remove(sel).and_then(|_| lst.save(&home_rm)) {
                                    Ok(()) => {
                                        this.notice = Some(("Removed server.".into(), false));
                                    }
                                    Err(e) => this.notice = Some((e, true)),
                                }
                                cx.notify();
                            }))
                        })
                    }),
            )
    }

    // ============================================================================================
    // Page 9: Screenshots (thumbnail grid, Open, Set as Instance Icon, Copy Path, Delete)
    // ============================================================================================
    fn render_screenshots_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let shots: Vec<ScreenshotItem> = content::list_screenshots(&instance.home);
        let sel = self.selected_shot.min(shots.len().saturating_sub(1));
        let cur_shot = shots.get(sel).cloned();
        let shots_dir = instance.home.join("screenshots");

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .gap(px(10.))
            .child(
                div()
                    .id("shots-grid")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(t.divider)
                    .bg(t.window)
                    .p(px(10.))
                    .flex()
                    .flex_wrap()
                    .gap(px(10.))
                    .when(shots.is_empty(), |d| {
                        d.child(
                            div()
                                .p(px(12.))
                                .text_size(px(12.5))
                                .text_color(t.ink_tertiary)
                                .child("No screenshots captured yet (press F2 in-game)."),
                        )
                    })
                    .children(shots.iter().enumerate().map(|(i, sh)| {
                        let selected = i == sel;
                        let thumb = inst_icons::load_thumbnail(&sh.path, 220);
                        div()
                            .id(SharedString::from(format!("shot-card-{i}")))
                            .w(px(176.))
                            .p(px(6.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .border_1()
                            .border_color(if selected { t.accent } else { t.divider })
                            .bg(if selected {
                                t.tile_selected
                            } else {
                                t.layer
                            })
                            .hover(|s| s.bg(t.subtle_hover))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.selected_shot = i;
                                cx.notify();
                            }))
                            .flex()
                            .flex_col()
                            .gap(px(6.))
                            .child(
                                div()
                                    .w_full()
                                    .h(px(100.))
                                    .rounded(px(4.))
                                    .bg(t.window)
                                    .overflow_hidden()
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(match thumb {
                                        Some(im) => img(im).w_full().h_full().into_any_element(),
                                        None => icons::icon("camera", 24., t.ink_tertiary)
                                            .into_any_element(),
                                    }),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(t.ink)
                                    .truncate()
                                    .child(sh.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(10.5))
                                    .text_color(t.ink_tertiary)
                                    .child(content::format_bytes(sh.size_bytes)),
                            )
                    })),
            )
            .child(
                div()
                    .w(px(176.))
                    .flex_shrink_0()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child({
                        let s_open = cur_shot.clone();
                        accent_button("shot-open", "Open Image", s_open.is_some(), t).when_some(
                            s_open,
                            |b, sh| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if let Err(e) = log::open_file(&sh.path, "") {
                                        this.notice = Some((e, true));
                                        cx.notify();
                                    }
                                }))
                            },
                        )
                    })
                    .child({
                        let s_icon = cur_shot.clone();
                        side_btn("shot-icon", None, "Set as Icon", s_icon.is_some(), t)
                            .when_some(s_icon, |b, sh| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    match inst_icons::import_custom_icon(&sh.path) {
                                        Ok(key) => {
                                            let id = this.instance_id.clone();
                                            this.app.update(cx, |app, cx| {
                                                app.apply_picked_icon(
                                                    IconTarget::Instance(id),
                                                    key,
                                                    cx,
                                                );
                                            });
                                            this.notice = Some((
                                                "Applied screenshot as instance icon.".into(),
                                                false,
                                            ));
                                        }
                                        Err(e) => this.notice = Some((e, true)),
                                    }
                                    cx.notify();
                                }))
                            })
                    })
                    .child({
                        let s_cp = cur_shot.clone();
                        side_btn("shot-cp", Some("copy"), "Copy Path", s_cp.is_some(), t)
                            .when_some(s_cp, |b, sh| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        sh.path.display().to_string(),
                                    ));
                                    this.notice =
                                        Some(("Copied screenshot path.".into(), false));
                                    cx.notify();
                                }))
                            })
                    })
                    .child({
                        let s_del = cur_shot;
                        danger_button("shot-del", "Delete", s_del.is_some(), t).when_some(
                            s_del,
                            |b, sh| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    match std::fs::remove_file(&sh.path) {
                                        Ok(()) => {
                                            this.notice =
                                                Some((format!("Deleted '{}'.", sh.name), false));
                                        }
                                        Err(e) => {
                                            this.notice =
                                                Some((format!("{}: {e}", sh.name), true));
                                        }
                                    }
                                    cx.notify();
                                }))
                            },
                        )
                    })
                    .child(div().h(px(1.)).bg(t.divider).my(px(4.)))
                    .child(
                        side_btn("shot-fld", Some("folder"), "View Folder", true, t)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&shots_dir);
                                if let Err(e) = log::open_folder(&shots_dir) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            })),
                    ),
            )
    }

    // ============================================================================================
    // Page 10: Instance Settings (General, Java/Horizon, Game Options, Commands, Env, Tweaks)
    // ============================================================================================
    fn render_settings_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let is_java = instance.java.is_some();
        let cfg = self.app.read(cx).cfgs.get(&instance.id);
        let tabs: [(&str, usize); 6] = [
            ("General", 0),
            (if is_java { "Java" } else { "Horizon" }, 1),
            ("Game Options", 2),
            ("Custom Commands", 3),
            ("Environment", 4),
            ("Tweaks", 5),
        ];

        let body = match self.settings_tab {
            0 => {
                let icon_key = cfg
                    .icon_key
                    .clone()
                    .unwrap_or_else(|| inst_icons::default_key_for(instance).to_owned());
                let id_icon = instance.id.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(gbox(
                        "Identity",
                        Some("Instance icon, display name, and group"),
                        t,
                        div()
                            .flex()
                            .items_center()
                            .gap(px(12.))
                            .child(
                                div()
                                    .id("inst-cfg-icon")
                                    .w(px(56.))
                                    .h(px(56.))
                                    .rounded(px(6.))
                                    .border_1()
                                    .border_color(t.divider)
                                    .bg(t.window)
                                    .cursor_pointer()
                                    .hover(|s| s.border_color(t.accent))
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        let id_t = id_icon.clone();
                                        let cur = icon_key.clone();
                                        this.app.update(cx, |app, cx| {
                                            app.open_icon_picker(
                                                IconTarget::Instance(id_t),
                                                &cur,
                                                cx,
                                            );
                                        });
                                    }))
                                    .child(inst_icons::render_icon(
                                        cfg.icon_key
                                            .as_deref()
                                            .unwrap_or_else(|| inst_icons::default_key_for(instance)),
                                        40.,
                                    )),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .child(self.render_field(
                                        "cfg-name",
                                        &self.edit_name,
                                        "Instance name",
                                        ActiveField::InstName,
                                        t,
                                        cx,
                                    ))
                                    .child(self.render_field(
                                        "cfg-group",
                                        &self.edit_group,
                                        "Group (optional)",
                                        ActiveField::InstGroup,
                                        t,
                                        cx,
                                    )),
                            )
                            .child(
                                accent_button("cfg-save-meta", "Apply", true, t).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.commit_name_and_group(cx);
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ))
                    .child(gbox(
                        "Game Window",
                        None,
                        t,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                chk(
                                    "ovr-win",
                                    cfg.override_window,
                                    "Override global game window settings",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| c.override_window = !c.override_window);
                                })),
                            )
                            .when(cfg.override_window, |d| {
                                d.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(
                                            div().w(px(110.)).child(self.render_field(
                                                "cfg-winw",
                                                &cfg.window_width.to_string(),
                                                "854",
                                                ActiveField::WinW,
                                                t,
                                                cx,
                                            )),
                                        )
                                        .child(div().text_color(t.ink_tertiary).child("×"))
                                        .child(
                                            div().w(px(110.)).child(self.render_field(
                                                "cfg-winh",
                                                &cfg.window_height.to_string(),
                                                "480",
                                                ActiveField::WinH,
                                                t,
                                                cx,
                                            )),
                                        ),
                                )
                                .child(
                                    chk(
                                        "cfg-max",
                                        cfg.start_maximized,
                                        "Start window maximized",
                                        None,
                                        t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.update_cfg(cx, |c| {
                                            c.start_maximized = !c.start_maximized
                                        });
                                    })),
                                )
                                .child(
                                    chk(
                                        "cfg-fs",
                                        cfg.start_fullscreen,
                                        "Launch directly in fullscreen",
                                        None,
                                        t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.update_cfg(cx, |c| {
                                            c.start_fullscreen = !c.start_fullscreen
                                        });
                                    })),
                                )
                            }),
                    ))
                    .child(gbox(
                        "Console, Offline & Auto-Join",
                        None,
                        t,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                chk(
                                    "ovr-con",
                                    cfg.override_console,
                                    "Override global console behaviour",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.override_console = !c.override_console
                                    });
                                })),
                            )
                            .when(cfg.override_console, |d| {
                                d.child(
                                    chk(
                                        "cfg-con-launch",
                                        cfg.show_console_on_launch,
                                        "Show console window when game launches",
                                        None,
                                        t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.update_cfg(cx, |c| {
                                            c.show_console_on_launch = !c.show_console_on_launch
                                        });
                                    })),
                                )
                                .child(
                                    chk(
                                        "cfg-con-crash",
                                        cfg.show_console_on_crash,
                                        "Show console window if game crashes",
                                        None,
                                        t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.update_cfg(cx, |c| {
                                            c.show_console_on_crash = !c.show_console_on_crash
                                        });
                                    })),
                                )
                            })
                            .child(
                                chk(
                                    "ovr-acc",
                                    cfg.override_account,
                                    "Override offline player name for this instance",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.override_account = !c.override_account
                                    });
                                })),
                            )
                            .when(cfg.override_account, |d| {
                                d.child(self.render_field(
                                    "cfg-off-name",
                                    &cfg.offline_name,
                                    "Offline player name",
                                    ActiveField::OfflineName,
                                    t,
                                    cx,
                                ))
                            })
                            .child(
                                chk(
                                    "cfg-autojoin",
                                    cfg.auto_join_enabled,
                                    "Auto-join multiplayer server on launch",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.auto_join_enabled = !c.auto_join_enabled
                                    });
                                })),
                            )
                            .when(cfg.auto_join_enabled, |d| {
                                d.child(self.render_field(
                                    "cfg-aj-srv",
                                    &cfg.auto_join_server,
                                    "server.example.net:25565",
                                    ActiveField::AutoJoinServer,
                                    t,
                                    cx,
                                ))
                            })
                            .child(
                                chk(
                                    "ovr-pt",
                                    cfg.override_playtime,
                                    "Override playtime tracking",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.override_playtime = !c.override_playtime
                                    });
                                })),
                            )
                            .when(cfg.override_playtime, |d| {
                                d.child(
                                    chk(
                                        "cfg-rec-pt",
                                        cfg.record_playtime,
                                        "Record playtime for this instance",
                                        None,
                                        t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.update_cfg(cx, |c| {
                                            c.record_playtime = !c.record_playtime
                                        });
                                    })),
                                )
                            }),
                    ))
            }
            1 if is_java => {
                let runtimes = java::installed();
                let cur_jp = cfg.java_path.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(gbox(
                        "Java Installation",
                        Some("Override Java runtime for this Fabric/Java instance"),
                        t,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                chk(
                                    "ovr-java",
                                    cfg.override_java,
                                    "Use a custom Java installation",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| c.override_java = !c.override_java);
                                })),
                            )
                            .when(cfg.override_java, |d| {
                                d.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(6.))
                                        .child(
                                            div().flex_1().child(self.render_field(
                                                "cfg-jpath",
                                                &cfg.java_path,
                                                "Path to java/javaw or JAVA_HOME",
                                                ActiveField::JavaPath,
                                                t,
                                                cx,
                                            )),
                                        )
                                        .child(
                                            push_button("cfg-jbrowse", "Browse...", true, t)
                                                .on_click(cx.listener(
                                                    |_, _: &ClickEvent, _, cx| {
                                                        let rx = cx.prompt_for_paths(
                                                            PathPromptOptions {
                                                                files: true,
                                                                directories: true,
                                                                multiple: false,
                                                                prompt: Some(
                                                                    "Select Java executable or home"
                                                                        .into(),
                                                                ),
                                                            },
                                                        );
                                                        cx.spawn(async move |view, cx| {
                                                            if let Ok(Ok(Some(paths))) = rx.await {
                                                                if let Some(p) = paths.first() {
                                                                    let s = p
                                                                        .to_string_lossy()
                                                                        .into_owned();
                                                                    let _ = view.update(
                                                                        cx,
                                                                        |v, cx| {
                                                                            v.update_cfg(cx, |c| {
                                                                                c.java_path = s;
                                                                            });
                                                                        },
                                                                    );
                                                                }
                                                            }
                                                        })
                                                        .detach();
                                                    },
                                                )),
                                        )
                                        .child(
                                            push_button("cfg-jtest", "Test", true, t).on_click(
                                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                    match job::test_java_installation(&cur_jp) {
                                                        Ok(info) => {
                                                            this.notice = Some((info, false));
                                                        }
                                                        Err(e) => {
                                                            this.notice = Some((e, true));
                                                        }
                                                    }
                                                    cx.notify();
                                                }),
                                            ),
                                        ),
                                )
                                .when(!runtimes.is_empty(), |d2| {
                                    d2.child(
                                        div()
                                            .flex()
                                            .flex_wrap()
                                            .gap(px(5.))
                                            .children(runtimes.into_iter().take(6).enumerate().map(
                                                |(i, rt)| {
                                                    let p_str =
                                                        rt.home.to_string_lossy().into_owned();
                                                    let lbl =
                                                        format!("Java {} ({})", rt.major, rt.version);
                                                    chip(
                                                        format!("rt-pick-{i}"),
                                                        lbl,
                                                        cfg.java_path == p_str,
                                                        t,
                                                    )
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, _, cx| {
                                                            let v = p_str.clone();
                                                            this.update_cfg(cx, |c| {
                                                                c.java_path = v
                                                            });
                                                        },
                                                    ))
                                                },
                                            )),
                                    )
                                })
                            }),
                    ))
                    .child(gbox(
                        "Memory & Arguments",
                        None,
                        t,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                chk(
                                    "ovr-mem",
                                    cfg.override_memory,
                                    "Override Java heap memory allocation (MiB)",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| c.override_memory = !c.override_memory);
                                })),
                            )
                            .when(cfg.override_memory, |d| {
                                d.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(div().text_size(px(12.)).text_color(t.ink_secondary).child("Min:"))
                                        .child(
                                            div().w(px(110.)).child(self.render_field(
                                                "cfg-minm",
                                                &cfg.min_mem_mb.to_string(),
                                                "512",
                                                ActiveField::MinMem,
                                                t,
                                                cx,
                                            )),
                                        )
                                        .child(div().text_size(px(12.)).text_color(t.ink_secondary).child("Max:"))
                                        .child(
                                            div().w(px(110.)).child(self.render_field(
                                                "cfg-maxm",
                                                &cfg.max_mem_mb.to_string(),
                                                "4096",
                                                ActiveField::MaxMem,
                                                t,
                                                cx,
                                            )),
                                        ),
                                )
                            })
                            .child(
                                chk(
                                    "ovr-jvm",
                                    cfg.override_jvm_args,
                                    "Override JVM arguments",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.override_jvm_args = !c.override_jvm_args
                                    });
                                })),
                            )
                            .when(cfg.override_jvm_args, |d| {
                                d.child(self.render_field(
                                    "cfg-jvmargs",
                                    &cfg.jvm_args,
                                    "-XX:+UseG1GC ...",
                                    ActiveField::JvmArgs,
                                    t,
                                    cx,
                                ))
                            })
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.ink_secondary)
                                    .child("Extra game CLI arguments:"),
                            )
                            .child(self.render_field(
                                "cfg-gameargs",
                                &cfg.extra_game_args,
                                "--demo ...",
                                ActiveField::ExtraGameArgs,
                                t,
                                cx,
                            )),
                    ))
            }
            1 => {
                // Horizon Instance Settings
                let home_open = instance.home.clone();
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(gbox(
                        "Horizon Channel, Gameplay & Multiplayer",
                        Some("Isolated Horizon settings stored strictly for this instance"),
                        t,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div().flex().flex_wrap().gap(px(6.)).children(
                                    HorizonTrack::ALL.into_iter().enumerate().map(|(i, tr)| {
                                        chip(
                                            format!("cfg-hz-tr-{i}"),
                                            tr.label(),
                                            cfg.horizon_track == tr,
                                            t,
                                        )
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                this.update_cfg(cx, |c| c.horizon_track = tr);
                                            },
                                        ))
                                    }),
                                ),
                            )
                            .when(cfg.horizon_track == HorizonTrack::Pinned, |d| {
                                d.child(self.render_field(
                                    "cfg-hz-pin",
                                    &cfg.horizon_pinned_version,
                                    "Pinned Horizon release tag",
                                    ActiveField::HzPinnedTag,
                                    t,
                                    cx,
                                ))
                            })
                            .child(
                                chk(
                                    "cfg-hz-cre",
                                    cfg.horizon_start_creative,
                                    "Start Horizon in Creative mode (--creative)",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.horizon_start_creative = !c.horizon_start_creative
                                    });
                                })),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.ink_secondary)
                                    .child("Auto-join Horizon friend code (optional):"),
                            )
                            .child(self.render_field(
                                "cfg-hz-friend",
                                &cfg.horizon_friend_code,
                                "Friend invite code",
                                ActiveField::HzFriendCode,
                                t,
                                cx,
                            ))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.ink_secondary)
                                    .child("Extra Horizon CLI arguments:"),
                            )
                            .child(self.render_field(
                                "cfg-hz-args",
                                &cfg.horizon_extra_args,
                                "Additional flags passed to Horizon",
                                ActiveField::HzExtraArgs,
                                t,
                                cx,
                            )),
                    ))
                    .child(gbox(
                        "Graphics (wgpu), Diagnostics & Privacy",
                        None,
                        t,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                chk(
                                    "cfg-hz-ovrgfx",
                                    cfg.horizon_override_graphics,
                                    "Override global wgpu backend & power preference",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.horizon_override_graphics = !c.horizon_override_graphics
                                    });
                                })),
                            )
                            .when(cfg.horizon_override_graphics, |d| {
                                d.child(
                                    div().flex().flex_wrap().gap(px(5.)).children(
                                        GpuBackend::ALL.into_iter().enumerate().map(|(i, b)| {
                                            chip(
                                                format!("cfg-wgpu-{i}"),
                                                b.label(),
                                                cfg.horizon_gpu_backend == b,
                                                t,
                                            )
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _, cx| {
                                                    this.update_cfg(cx, |c| {
                                                        c.horizon_gpu_backend = b
                                                    });
                                                },
                                            ))
                                        }),
                                    ),
                                )
                                .child(
                                    div().flex().flex_wrap().gap(px(5.)).children(
                                        GpuPower::ALL.into_iter().enumerate().map(|(i, p)| {
                                            chip(
                                                format!("cfg-wpwr-{i}"),
                                                p.label(),
                                                cfg.horizon_gpu_power == p,
                                                t,
                                            )
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _, cx| {
                                                    this.update_cfg(cx, |c| {
                                                        c.horizon_gpu_power = p
                                                    });
                                                },
                                            ))
                                        }),
                                    ),
                                )
                            })
                            .child(
                                chk(
                                    "cfg-hz-bt",
                                    cfg.horizon_rust_backtrace,
                                    "Enable RUST_BACKTRACE=1 for crash diagnostics",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.horizon_rust_backtrace = !c.horizon_rust_backtrace
                                    });
                                })),
                            )
                            .child(self.render_field(
                                "cfg-hz-rlog",
                                &cfg.horizon_rust_log,
                                "RUST_LOG filter (e.g. info,wgpu=warn)",
                                ActiveField::HzRustLog,
                                t,
                                cx,
                            ))
                            .child(
                                chk(
                                    "cfg-hz-ovrpriv",
                                    cfg.horizon_override_privacy,
                                    "Override Horizon telemetry & No-AI flags",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.update_cfg(cx, |c| {
                                        c.horizon_override_privacy = !c.horizon_override_privacy
                                    });
                                })),
                            )
                            .when(cfg.horizon_override_privacy, |d| {
                                d.child(
                                    chk(
                                        "cfg-hz-tel",
                                        cfg.horizon_telemetry_opt_out,
                                        "Opt out of Horizon telemetry",
                                        None,
                                        t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.update_cfg(cx, |c| {
                                            c.horizon_telemetry_opt_out =
                                                !c.horizon_telemetry_opt_out
                                        });
                                    })),
                                )
                                .child(
                                    chk(
                                        "cfg-hz-noai",
                                        cfg.horizon_no_ai,
                                        "Disable Horizon AI features (No-AI)",
                                        None,
                                        t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.update_cfg(cx, |c| {
                                            c.horizon_no_ai = !c.horizon_no_ai
                                        });
                                    })),
                                )
                            })
                            .child(
                                push_button("cfg-hz-home", "Open Instance Folder", true, t)
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        let _ = std::fs::create_dir_all(&home_open);
                                        if let Err(e) = log::open_folder(&home_open) {
                                            this.notice = Some((e, true));
                                            cx.notify();
                                        }
                                    })),
                            ),
                    ))
            }
            2 => {
                // Game Options (`options.txt` tuner)
                let home_save = instance.home.clone();
                let o = &self.game_opts;
                div()
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(gbox(
                        "options.txt Tuner",
                        Some("Edit video, audio & input options before launching"),
                        t,
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .w(px(130.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_secondary)
                                                    .child("Render Dist (2-64)"),
                                            )
                                            .child(self.render_field(
                                                "opt-rd",
                                                &o.render_distance.to_string(),
                                                "12",
                                                ActiveField::OptRenderDist,
                                                t,
                                                cx,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(130.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_secondary)
                                                    .child("Sim Dist (5-32)"),
                                            )
                                            .child(self.render_field(
                                                "opt-sd",
                                                &o.simulation_distance.to_string(),
                                                "12",
                                                ActiveField::OptSimDist,
                                                t,
                                                cx,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(130.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_secondary)
                                                    .child("Max FPS (260=Unlim)"),
                                            )
                                            .child(self.render_field(
                                                "opt-fps",
                                                &o.max_fps.to_string(),
                                                "120",
                                                ActiveField::OptMaxFps,
                                                t,
                                                cx,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(110.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_secondary)
                                                    .child("FOV (30-110)"),
                                            )
                                            .child(self.render_field(
                                                "opt-fov",
                                                &o.fov_deg.to_string(),
                                                "70",
                                                ActiveField::OptFov,
                                                t,
                                                cx,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(110.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_secondary)
                                                    .child("GUI Scale (0=Auto)"),
                                            )
                                            .child(self.render_field(
                                                "opt-gui",
                                                &o.gui_scale.to_string(),
                                                "0",
                                                ActiveField::OptGuiScale,
                                                t,
                                                cx,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(110.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_secondary)
                                                    .child("Gamma %"),
                                            )
                                            .child(self.render_field(
                                                "opt-gam",
                                                &o.gamma_pct.to_string(),
                                                "100",
                                                ActiveField::OptGamma,
                                                t,
                                                cx,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(110.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_secondary)
                                                    .child("Master Vol %"),
                                            )
                                            .child(self.render_field(
                                                "opt-mvol",
                                                &o.master_volume_pct.to_string(),
                                                "100",
                                                ActiveField::OptMasterVol,
                                                t,
                                                cx,
                                            )),
                                    )
                                    .child(
                                        div()
                                            .w(px(110.))
                                            .flex()
                                            .flex_col()
                                            .gap(px(3.))
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_secondary)
                                                    .child("Music Vol %"),
                                            )
                                            .child(self.render_field(
                                                "opt-muvol",
                                                &o.music_volume_pct.to_string(),
                                                "100",
                                                ActiveField::OptMusicVol,
                                                t,
                                                cx,
                                            )),
                                    ),
                            )
                            .child(
                                chk("opt-vsync", o.vsync, "Enable VSync", None, t)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.game_opts.vsync = !this.game_opts.vsync;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                chk(
                                    "opt-fs",
                                    o.fullscreen,
                                    "Fullscreen in options.txt",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.game_opts.fullscreen = !this.game_opts.fullscreen;
                                    cx.notify();
                                })),
                            )
                            .child(
                                chk(
                                    "opt-raw",
                                    o.raw_mouse_input,
                                    "Raw Mouse Input",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.game_opts.raw_mouse_input =
                                        !this.game_opts.raw_mouse_input;
                                    cx.notify();
                                })),
                            )
                            .child(
                                chk("opt-bob", o.view_bobbing, "View Bobbing", None, t)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.game_opts.view_bobbing = !this.game_opts.view_bobbing;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                chk("opt-aj", o.auto_jump, "Auto-Jump", None, t).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.game_opts.auto_jump = !this.game_opts.auto_jump;
                                        cx.notify();
                                    }),
                                ),
                            )
                            .child(
                                chk(
                                    "opt-pause",
                                    o.pause_on_lost_focus,
                                    "Pause on Lost Focus",
                                    None,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.game_opts.pause_on_lost_focus =
                                        !this.game_opts.pause_on_lost_focus;
                                    cx.notify();
                                })),
                            )
                            .child(
                                div().child(
                                    accent_button("opt-save", "Save to options.txt", true, t)
                                        .on_click(cx.listener(
                                            move |this, _: &ClickEvent, _, cx| {
                                                match this.game_opts.save(&home_save) {
                                                    Ok(()) => {
                                                        this.notice = Some((
                                                            "Saved options.txt.".into(),
                                                            false,
                                                        ));
                                                    }
                                                    Err(e) => this.notice = Some((e, true)),
                                                }
                                                cx.notify();
                                            },
                                        )),
                                ),
                            ),
                    ))
            }
            3 => div().flex().flex_col().gap(px(12.)).child(gbox(
                "Custom Commands",
                Some(
                    "Variables: $INST_NAME, $INST_ID, $INST_DIR, $INST_MC_DIR, $INST_JAVA, $INST_JAVA_ARGS",
                ),
                t,
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        chk(
                            "ovr-cmd",
                            cfg.override_commands,
                            "Override global custom commands",
                            None,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.update_cfg(cx, |c| c.override_commands = !c.override_commands);
                        })),
                    )
                    .when(cfg.override_commands, |d| {
                        d.child(
                            div()
                                .text_size(px(12.))
                                .text_color(t.ink_secondary)
                                .child("Pre-launch command:"),
                        )
                        .child(self.render_field(
                            "cfg-precmd",
                            &cfg.pre_launch_cmd,
                            "Runs before the game starts",
                            ActiveField::PreCmd,
                            t,
                            cx,
                        ))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(t.ink_secondary)
                                .child("Wrapper command:"),
                        )
                        .child(self.render_field(
                            "cfg-wrapcmd",
                            &cfg.wrapper_cmd,
                            "Prefix wrapper around game executable",
                            ActiveField::WrapCmd,
                            t,
                            cx,
                        ))
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(t.ink_secondary)
                                .child("Post-exit command:"),
                        )
                        .child(self.render_field(
                            "cfg-postcmd",
                            &cfg.post_exit_cmd,
                            "Runs after the game exits",
                            ActiveField::PostCmd,
                            t,
                            cx,
                        ))
                    }),
            )),
            4 => div().flex().flex_col().gap(px(12.)).child(gbox(
                "Environment Variables",
                Some("Per-instance environment variables applied to the game process"),
                t,
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        chk(
                            "ovr-env",
                            cfg.override_env,
                            "Apply custom environment variables for this instance",
                            None,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.update_cfg(cx, |c| c.override_env = !c.override_env);
                        })),
                    )
                    .when(cfg.override_env, |d| {
                        d.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div().w(px(180.)).child(self.render_field(
                                        "env-k",
                                        &self.new_env_key,
                                        "KEY",
                                        ActiveField::EnvKey,
                                        t,
                                        cx,
                                    )),
                                )
                                .child(
                                    div().flex_1().child(self.render_field(
                                        "env-v",
                                        &self.new_env_val,
                                        "VALUE",
                                        ActiveField::EnvVal,
                                        t,
                                        cx,
                                    )),
                                )
                                .child(accent_button("env-add", "Add", true, t).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        let k = this.new_env_key.trim().to_owned();
                                        let v = this.new_env_val.clone();
                                        if !k.is_empty() {
                                            this.update_cfg(cx, |c| {
                                                c.env.retain(|(ek, _)| ek != &k);
                                                c.env.push((k, v));
                                            });
                                            this.new_env_key.clear();
                                            this.new_env_val.clear();
                                        }
                                        cx.notify();
                                    }),
                                )),
                        )
                        .children(cfg.env.iter().cloned().enumerate().map(|(i, (k, v))| {
                            let k_rm = k.clone();
                            div()
                                .px(px(10.))
                                .py(px(5.))
                                .rounded(px(4.))
                                .border_1()
                                .border_color(t.divider)
                                .bg(t.window)
                                .flex()
                                .items_center()
                                .justify_between()
                                .child(
                                    div()
                                        .font_family(MONO_FONT)
                                        .text_size(px(12.))
                                        .text_color(t.ink)
                                        .child(format!("{k} = {v}")),
                                )
                                .child(
                                    push_button(format!("env-rm-{i}"), "Remove", true, t).on_click(
                                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            let kr = k_rm.clone();
                                            this.update_cfg(cx, |c| {
                                                c.env.retain(|(ek, _)| ek != &kr);
                                            });
                                        }),
                                    ),
                                )
                        }))
                    }),
            )),
            _ => div().flex().flex_col().gap(px(12.)).child(gbox(
                "GPU & Native Library Tweaks",
                None,
                t,
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        chk(
                            "ovr-twk",
                            cfg.override_tweaks,
                            "Override global GPU & native library tweaks",
                            None,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.update_cfg(cx, |c| c.override_tweaks = !c.override_tweaks);
                        })),
                    )
                    .when(cfg.override_tweaks, |d| {
                        d.child(
                            chk(
                                "twk-dgpu",
                                cfg.prefer_discrete_gpu,
                                "Prefer dedicated discrete GPU",
                                None,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.update_cfg(cx, |c| {
                                    c.prefer_discrete_gpu = !c.prefer_discrete_gpu
                                });
                            })),
                        )
                        .child(
                            chk(
                                "twk-zink",
                                cfg.use_zink,
                                "Use Mesa Zink (OpenGL-on-Vulkan)",
                                None,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.update_cfg(cx, |c| c.use_zink = !c.use_zink);
                            })),
                        )
                        .child(self.render_field(
                            "twk-glfw",
                            &cfg.custom_glfw_path,
                            "Custom GLFW library path (optional)",
                            ActiveField::GlfwPath,
                            t,
                            cx,
                        ))
                        .child(self.render_field(
                            "twk-oal",
                            &cfg.custom_openal_path,
                            "Custom OpenAL library path (optional)",
                            ActiveField::OpenAlPath,
                            t,
                            cx,
                        ))
                    })
                    .when(is_java, |d| {
                        d.child(
                            chk(
                                "twk-skipjava",
                                cfg.skip_java_compat,
                                "Skip Java version compatibility check",
                                None,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.update_cfg(cx, |c| {
                                    c.skip_java_compat = !c.skip_java_compat
                                });
                            })),
                        )
                    }),
            )),
        };

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(4.))
                    .border_b_1()
                    .border_color(t.divider)
                    .pb(px(4.))
                    .children(tabs.into_iter().map(|(lbl, idx)| {
                        sub_tab(
                            format!("inst-stab-{idx}"),
                            lbl,
                            self.settings_tab == idx,
                            t,
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.settings_tab = idx;
                            this.active_field = None;
                            cx.notify();
                        }))
                    })),
            )
            .child(
                div()
                    .id("inst-settings-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .child(body),
            )
    }

    // ============================================================================================
    // Page 11: Other Logs (`logs/` + `crash-reports/`)
    // ============================================================================================
    fn render_other_logs_page(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let files: Vec<LogFileItem> = content::list_log_files(&instance.home);
        let sel = self.selected_log_file.min(files.len().saturating_sub(1));
        let cur_file = files.get(sel).cloned();
        let tail = cur_file
            .as_ref()
            .map(|f| content::read_log_tail(&f.path, 400))
            .unwrap_or_default();

        let editor_cmd = self.app.read(cx).prefs.text_editor_cmd.clone();
        let logs_dir = instance.home.join("logs");
        let crash_dir = instance.home.join("crash-reports");

        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .gap(px(8.))
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap(px(6.))
                    .children(files.iter().take(8).enumerate().map(|(i, lf)| {
                        chip(format!("olf-{i}"), lf.label.clone(), i == sel, t).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.selected_log_file = i;
                                cx.notify();
                            }),
                        )
                    }))
                    .child(div().flex_1())
                    .when_some(cur_file.clone(), |d, lf| {
                        let p_cp = lf.path.clone();
                        let p_ed = lf.path.clone();
                        d.child(
                            push_button("olf-copy", "Copy", true, t).on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    let text = content::read_log_tail(&p_cp, 2000).join("\n");
                                    cx.write_to_clipboard(ClipboardItem::new_string(text));
                                    this.notice = Some(("Copied log tail.".into(), false));
                                    cx.notify();
                                },
                            )),
                        )
                        .child(
                            push_button("olf-open", "Open in Editor", true, t).on_click(
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    if let Err(e) = log::open_file(&p_ed, &editor_cmd) {
                                        this.notice = Some((e, true));
                                        cx.notify();
                                    }
                                }),
                            ),
                        )
                    })
                    .child(
                        push_button("olf-logs", "Logs Folder", true, t).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&logs_dir);
                                if let Err(e) = log::open_folder(&logs_dir) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            },
                        )),
                    )
                    .child(
                        push_button("olf-crash", "Crash Reports", true, t).on_click(cx.listener(
                            move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&crash_dir);
                                if let Err(e) = log::open_folder(&crash_dir) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .id("olf-tail-scroll")
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .rounded(px(6.))
                    .border_1()
                    .border_color(t.divider)
                    .bg(t.card)
                    .p(px(10.))
                    .font_family(MONO_FONT)
                    .text_size(px(11.5))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .when(tail.is_empty(), |d| {
                        d.child(
                            div()
                                .text_color(t.ink_tertiary)
                                .child("No log or crash report files on disk yet."),
                        )
                    })
                    .children(tail.into_iter().map(|ln| {
                        let col = log_line_color(&ln, true, t);
                        div().text_color(col).child(ln)
                    })),
            )
    }
}

impl Focusable for InstanceWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for InstanceWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.app.read(cx).prefs.clone();
        let t = Theme::resolve(prefs.theme_mode, prefs.accent, window.appearance());

        let Some(instance) = self.current_instance(cx) else {
            return div()
                .size_full()
                .bg(t.window)
                .font_family(UI_FONT)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .child(
                    div()
                        .text_size(px(14.))
                        .text_color(t.ink_secondary)
                        .child("This instance was deleted."),
                )
                .child(
                    push_button("inst-gone-close", "Close", true, &t).on_click(cx.listener(
                        |_, _: &ClickEvent, window, _| window.remove_window(),
                    )),
                );
        };

        let cfg = self.app.read(cx).cfgs.get(&instance.id);
        let icon_key = cfg
            .icon_key
            .clone()
            .unwrap_or_else(|| inst_icons::default_key_for(&instance).to_owned());

        let page_content = match self.page {
            InstancePage::Log => self.render_log_page(&instance, &t, cx),
            InstancePage::Version => self.render_version_page(&instance, &t, cx),
            InstancePage::Mods => self.render_mods_page(&instance, &t, cx),
            InstancePage::ResourcePacks => self.render_packs_page(&instance, false, &t, cx),
            InstancePage::ShaderPacks => self.render_packs_page(&instance, true, &t, cx),
            InstancePage::Notes => self.render_notes_page(&instance, &t, cx),
            InstancePage::Worlds => self.render_worlds_page(&instance, &t, cx),
            InstancePage::Servers => self.render_servers_page(&instance, &t, cx),
            InstancePage::Screenshots => self.render_screenshots_page(&instance, &t, cx),
            InstancePage::Settings => self.render_settings_page(&instance, &t, cx),
            InstancePage::OtherLogs => self.render_other_logs_page(&instance, &t, cx),
        };

        let is_busy = job::busy();
        let active_inst = job::active_instance();
        let this_running = active_inst.as_deref() == Some(instance.id.as_str()) && job::can_kill();
        let st = job::status();
        let home_help = instance.home.clone();

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .bg(t.window)
            .font_family(UI_FONT)
            .text_color(t.ink)
            .flex()
            .flex_col()
            // Main split: left sidebar + right page container
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        // Left JLac PageContainer sidebar
                        div()
                            .w(px(208.))
                            .flex_shrink_0()
                            .bg(t.layer)
                            .border_r_1()
                            .border_color(t.divider)
                            .p(px(10.))
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .child(
                                div()
                                    .p(px(8.))
                                    .rounded(px(6.))
                                    .bg(t.card)
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(inst_icons::render_icon(&icon_key, 32.))
                                    .child(
                                        div()
                                            .min_w_0()
                                            .flex()
                                            .flex_col()
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(t.ink)
                                                    .truncate()
                                                    .child(instance.name.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.))
                                                    .text_color(t.ink_tertiary)
                                                    .truncate()
                                                    .child(instance_summary(&instance)),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .id("inst-nav-scroll")
                                    .flex_1()
                                    .min_h_0()
                                    .overflow_y_scroll()
                                    .flex()
                                    .flex_col()
                                    .gap(px(2.))
                                    .children(InstancePage::ALL.into_iter().enumerate().map(
                                        |(i, p)| {
                                            page_nav_row(
                                                format!("inst-nav-{i}"),
                                                p.icon(),
                                                p.label(),
                                                self.page == p,
                                                &t,
                                            )
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _, cx| {
                                                    this.set_page(p, cx);
                                                },
                                            ))
                                        },
                                    )),
                            ),
                    )
                    .child(
                        // Active page content
                        div()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .p(px(16.))
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .justify_between()
                                    .child(
                                        div()
                                            .text_size(px(16.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(t.ink)
                                            .child(self.page.label()),
                                    )
                                    .when_some(st.filter(|s| s.instance_id.as_deref() == Some(instance.id.as_str())), |d, s| {
                                        d.child(
                                            div()
                                                .text_size(px(11.5))
                                                .text_color(if s.failed { t.danger } else { t.accent })
                                                .child(format!("{} — {}", s.title, s.line)),
                                        )
                                    }),
                            )
                            .when_some(self.notice.clone(), |d, (msg, is_err)| {
                                d.child(status_banner(&msg, is_err, &t))
                            })
                            .child(page_content),
                    ),
            )
            // Bottom JLac footer bar: Help | Launch | Launch Offline | Kill | Close
            .child(
                div()
                    .h(px(48.))
                    .px(px(14.))
                    .bg(t.layer)
                    .border_t_1()
                    .border_color(t.divider)
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        push_button("inst-ft-folder", "Instance Folder", true, &t).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let _ = std::fs::create_dir_all(&home_help);
                                if let Err(e) = log::open_folder(&home_help) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            }),
                        ),
                    )
                    .child(
                        push_button("inst-ft-help", "Help", true, &t).on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                if let Err(e) = log::open_url("https://github.com/bufferclick/fnh")
                                {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            },
                        )),
                    )
                    .child(div().flex_1())
                    .child(
                        accent_button("inst-ft-launch", "Launch", !is_busy, &t).when(!is_busy, |b| {
                            b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.launch_now(LaunchExtras::default(), cx);
                            }))
                        }),
                    )
                    .child(
                        push_button("inst-ft-offline", "Launch Offline", !is_busy, &t).when(
                            !is_busy,
                            |b| {
                                b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.launch_now(
                                        LaunchExtras {
                                            offline: true,
                                            ..LaunchExtras::default()
                                        },
                                        cx,
                                    );
                                }))
                            },
                        ),
                    )
                    .child(
                        danger_button("inst-ft-kill", "Kill", this_running, &t).when(
                            this_running,
                            |b| {
                                b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    job::kill();
                                    this.notice =
                                        Some(("Terminated game process.".into(), false));
                                    cx.notify();
                                }))
                            },
                        ),
                    )
                    .child(
                        push_button("inst-ft-close", "Close", true, &t).on_click(cx.listener(
                            |_, _: &ClickEvent, window, _| {
                                window.remove_window();
                            },
                        )),
                    ),
            )
    }
}
