//! Shared multi-window application state (`Fh`) and primary launcher shell (`MainWindow`),
//! modelled on JLac's `MainWindow` (`mainToolBar` + collapsible grouped `InstanceView` + right
//! `WideBar` + `QStatusBar`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, ClickEvent, ClipboardItem, Context, Entity, ExternalPaths, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, MouseButton, MouseDownEvent,
    ParentElement, Render, SharedString, StatefulInteractiveElement, Styled, WeakEntity, Window,
    WindowHandle, div, px,
};
use horizon_launcher::account;
use horizon_launcher::browser::Kind as BrowseKind;
use horizon_launcher::instances::{self as inst, Instance};

use crate::content::{self, ShortcutTarget};
use crate::icons;
use crate::inst_icons;
use crate::job::{self, LaunchExtras};
use crate::log;
use crate::look::{
    MONO_FONT, Theme, UI_FONT, accent_button, apply_inline_rename_key, apply_text_key,
    apply_window_icon, brand_mark, chip, danger_button, inline_rename_input, instance_summary,
    menu_frame, menu_row, menu_separator, push_button, status_banner, text_input, toolbar_button,
    toolbar_separator, wide_bar_action, wide_bar_separator,
};
use crate::paths;
use crate::prefs::{InstanceCfgStore, Prefs, SortMode, ViewMode};
use crate::reset;
use crate::win_dialogs::{
    CrashWindow, IconPickerWindow, ModBrowserWindow, NewInstanceWindow, UtilityMode, UtilityWindow,
};
use crate::win_instance::{InstancePage, InstanceWindow};
use crate::win_settings::{SettingsPage, SettingsWindow};
use crate::{crash, updater};

/// Destination when `IconPickerWindow` confirms an icon key.
#[derive(Clone)]
pub enum IconTarget {
    Instance(String),
    NewInstance(WeakEntity<NewInstanceWindow>),
}

/// Shared launcher state observed by `MainWindow` and every secondary OS window.
pub struct Fh {
    pub prefs: Prefs,
    pub cfgs: InstanceCfgStore,
    pub instances: Vec<Instance>,
    pub selected_id: Option<String>,
    pub notice: Option<String>,

    instance_windows: HashMap<String, WindowHandle<InstanceWindow>>,
    settings_window: Option<WindowHandle<SettingsWindow>>,
    new_instance_window: Option<WindowHandle<NewInstanceWindow>>,
    icon_picker_window: Option<WindowHandle<IconPickerWindow>>,
    mod_browser_windows: HashMap<String, WindowHandle<ModBrowserWindow>>,
    utility_window: Option<WindowHandle<UtilityWindow>>,
    crash_window: Option<WindowHandle<CrashWindow>>,

    seen_event_seq: u64,
    was_busy: bool,
}

impl Fh {
    pub fn new(after_reset: bool, updated_from: Option<String>) -> Self {
        let prefs = Prefs::load();
        prefs.apply_proxy_env();
        let cfgs = InstanceCfgStore::load();
        let startup_notice = if after_reset {
            Some("Factory reset complete — FNH has been restored to a clean slate.".to_owned())
        } else {
            updated_from.map(|old| {
                format!(
                    "Updated FnH from v{} to v{}!",
                    updater::normalize_version(&old),
                    crate::VERSION
                )
            })
        };
        let mut s = Self {
            prefs,
            cfgs,
            instances: Vec::new(),
            selected_id: None,
            notice: startup_notice,
            instance_windows: HashMap::new(),
            settings_window: None,
            new_instance_window: None,
            icon_picker_window: None,
            mod_browser_windows: HashMap::new(),
            utility_window: None,
            crash_window: None,
            seen_event_seq: 0,
            was_busy: false,
        };
        s.reload_instances();
        if s.prefs.remember_selected_instance {
            let sel = inst::selected();
            if s.instances.iter().any(|i| i.id == sel.id) {
                s.selected_id = Some(sel.id);
            }
        }
        // Automatically check GitHub Releases for a newer version of FnH on startup!
        updater::check_in_background(true);
        s
    }

    pub fn reload_instances(&mut self) {
        self.instances = content::user_instances();
        self.sort_instances();
        let valid = self
            .selected_id
            .as_ref()
            .is_some_and(|id| self.instances.iter().any(|i| &i.id == id));
        if !valid {
            self.selected_id = self.instances.first().map(|i| i.id.clone());
        }
    }

    pub fn sort_instances(&mut self) {
        let mode = self.prefs.sort_mode;
        let cfgs = &self.cfgs;
        self.instances.sort_by(|a, b| match mode {
            SortMode::Name => a
                .name
                .to_ascii_lowercase()
                .cmp(&b.name.to_ascii_lowercase()),
            SortMode::LastLaunched => {
                let la = cfgs.get(&a.id).last_launched;
                let lb = cfgs.get(&b.id).last_launched;
                lb.cmp(&la)
                    .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
            }
            SortMode::PlayTime => b
                .played
                .cmp(&a.played)
                .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase())),
        });
    }

    pub fn select_instance(&mut self, id: &str) {
        if self.instances.iter().any(|i| i.id == id) {
            self.selected_id = Some(id.to_owned());
            if self.prefs.remember_selected_instance {
                let _ = inst::select(id);
            }
        }
    }

    pub fn selected_instance(&self) -> Option<Instance> {
        let id = self.selected_id.as_deref()?;
        self.instances.iter().find(|i| i.id == id).cloned()
    }

    pub fn existing_groups(&self) -> Vec<String> {
        let mut groups: Vec<String> = self
            .instances
            .iter()
            .filter_map(|i| i.group.as_ref().map(|g| g.trim().to_owned()))
            .filter(|g| !g.is_empty())
            .collect();
        groups.sort_by_key(|g| g.to_ascii_lowercase());
        groups.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        groups
    }

    pub fn apply_picked_icon(
        &mut self,
        target: IconTarget,
        key: String,
        cx: &mut Context<Self>,
    ) {
        match target {
            IconTarget::Instance(id) => {
                self.cfgs.update(&id, |c| c.icon_key = Some(key));
                cx.notify();
            }
            IconTarget::NewInstance(weak) => {
                let _ = weak.update(cx, |win, cx| win.set_icon(key, cx));
            }
        }
    }

    pub fn launch_instance(
        &mut self,
        id: &str,
        extras: LaunchExtras,
        cx: &mut Context<Self>,
    ) {
        if job::busy() {
            self.notice = Some("A game or background task is already running.".into());
            cx.notify();
            return;
        }
        let Some(instance) = self
            .instances
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .or_else(|| inst::find(id).ok())
        else {
            return;
        };

        self.select_instance(&instance.id);
        let now = content::now_secs();
        self.cfgs.update(&instance.id, |c| c.last_launched = now);
        self.sort_instances();

        let cfg = self.cfgs.get(&instance.id);
        let show_console = self.prefs.effective_show_console_on_launch(&cfg);
        let inst_id = instance.id.clone();

        job::play(instance, self.prefs.clone(), cfg, extras);

        if show_console {
            self.open_instance_window(&inst_id, InstancePage::Log, cx);
        }
        cx.notify();
    }

    pub fn open_instance_window(
        &mut self,
        id: &str,
        page: InstancePage,
        cx: &mut Context<Self>,
    ) {
        if let Some(handle) = self.instance_windows.get(id).copied() {
            let ok = handle.update(cx, |view, window, cx| {
                view.set_page(page, cx);
                window.activate_window();
            });
            if ok.is_ok() {
                return;
            }
            self.instance_windows.remove(id);
        }
        let me = cx.entity().clone();
        let id_owned = id.to_owned();
        cx.defer(move |cx| {
            if let Some(h) = InstanceWindow::open(me.clone(), id_owned.clone(), page, cx) {
                me.update(cx, |app, _| {
                    app.instance_windows.insert(id_owned, h);
                });
            }
        });
    }

    pub fn close_instance_window(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(handle) = self.instance_windows.remove(id) {
            let _ = handle.update(cx, |_, window, _| {
                window.remove_window();
            });
        }
    }

    pub fn open_settings_window(&mut self, page: SettingsPage, cx: &mut Context<Self>) {
        if let Some(handle) = self.settings_window {
            let ok = handle.update(cx, |view, window, cx| {
                view.set_page(page, cx);
                window.activate_window();
            });
            if ok.is_ok() {
                return;
            }
            self.settings_window = None;
        }
        let me = cx.entity().clone();
        cx.defer(move |cx| {
            if let Some(h) = SettingsWindow::open(me.clone(), page, cx) {
                me.update(cx, |app, _| {
                    app.settings_window = Some(h);
                });
            }
        });
    }

    pub fn open_new_instance_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.new_instance_window {
            let ok = handle.update(cx, |_, window, _| {
                window.activate_window();
            });
            if ok.is_ok() {
                return;
            }
            self.new_instance_window = None;
        }
        let me = cx.entity().clone();
        cx.defer(move |cx| {
            if let Some(h) = NewInstanceWindow::open(me.clone(), cx) {
                me.update(cx, |app, _| {
                    app.new_instance_window = Some(h);
                });
            }
        });
    }

    pub fn open_icon_picker(
        &mut self,
        target: IconTarget,
        current_key: &str,
        cx: &mut Context<Self>,
    ) {
        if let Some(handle) = self.icon_picker_window.take() {
            let _ = handle.update(cx, |_, window, _| {
                window.remove_window();
            });
        }
        let me = cx.entity().clone();
        let init = current_key.to_owned();
        cx.defer(move |cx| {
            if let Some(h) = IconPickerWindow::open(me.clone(), target, init, cx) {
                me.update(cx, |app, _| {
                    app.icon_picker_window = Some(h);
                });
            }
        });
    }

    pub fn open_mod_browser(
        &mut self,
        instance: Instance,
        kind: BrowseKind,
        cx: &mut Context<Self>,
    ) {
        let key = format!("{}:{:?}", instance.id, kind);
        if let Some(handle) = self.mod_browser_windows.get(&key).copied() {
            let ok = handle.update(cx, |_, window, _| {
                window.activate_window();
            });
            if ok.is_ok() {
                return;
            }
            self.mod_browser_windows.remove(&key);
        }
        let me = cx.entity().clone();
        cx.defer(move |cx| {
            if let Some(h) = ModBrowserWindow::open(me.clone(), instance, kind, cx) {
                me.update(cx, |app, _| {
                    app.mod_browser_windows.insert(key, h);
                });
            }
        });
    }

    pub fn open_utility_window(&mut self, mode: UtilityMode, cx: &mut Context<Self>) {
        if let Some(handle) = self.utility_window.take() {
            let _ = handle.update(cx, |_, window, _| {
                window.remove_window();
            });
        }
        let me = cx.entity().clone();
        cx.defer(move |cx| {
            if let Some(h) = UtilityWindow::open(me.clone(), mode, cx) {
                me.update(cx, |app, _| {
                    app.utility_window = Some(h);
                });
            }
        });
    }

    pub fn open_fabric_versions_window(&mut self, cx: &mut Context<Self>) {
        if let Some(handle) = self.utility_window.take() {
            let _ = handle.update(cx, |_, window, _| {
                window.remove_window();
            });
        }
        let me = cx.entity().clone();
        cx.defer(move |cx| {
            if let Some(h) = UtilityWindow::open_fabric_versions(me.clone(), cx) {
                me.update(cx, |app, _| {
                    app.utility_window = Some(h);
                });
            }
        });
    }

    pub fn open_crash_window(&mut self, diag: crash::CrashDiagnosis, cx: &mut Context<Self>) {
        if let Some(handle) = self.crash_window.take() {
            let _ = handle.update(cx, |_, window, _| {
                window.remove_window();
            });
        }
        let me = cx.entity().clone();
        cx.defer(move |cx| {
            if let Some(h) = CrashWindow::open(me.clone(), diag, cx) {
                me.update(cx, |app, _| {
                    app.crash_window = Some(h);
                });
            }
        });
    }

    pub fn poll_job_events(&mut self, cx: &mut Context<Self>) {
        let st = job::status();
        let now_busy = st.as_ref().is_some_and(|s| s.busy);
        let mut changed = now_busy != self.was_busy;
        self.was_busy = now_busy;

        // 1. Pop open "Do you want to update FnH?" if startup check discovered a newer GitHub release!
        if updater::take_auto_prompt().is_some() {
            self.open_utility_window(UtilityMode::Updater, cx);
            changed = true;
        }

        // 2. Pop open Fabric-style Crash & Mod Compatibility Window for any pending diagnosis!
        let pending_crashes = crash::take_pending_diagnoses();
        if let Some(latest_diag) = pending_crashes.into_iter().last() {
            self.open_crash_window(latest_diag, cx);
            changed = true;
        }

        if let Some(s) = st {
            if s.event_seq != self.seen_event_seq {
                self.seen_event_seq = s.event_seq;
                changed = true;

                if s.done {
                    self.reload_instances();
                    if let Some(inst_id) = s.instance_id.as_deref() {
                        let cfg = self.cfgs.get(inst_id);
                        if s.crashed && self.prefs.effective_show_console_on_crash(&cfg) {
                            self.open_instance_window(inst_id, InstancePage::Log, cx);
                        } else if !s.crashed
                            && !s.failed
                            && self.prefs.effective_close_console_on_exit(&cfg)
                        {
                            self.close_instance_window(inst_id, cx);
                        }
                        if !s.crashed && !s.failed && self.prefs.quit_launcher_on_exit {
                            cx.quit();
                        }
                    }
                }
            }
        }

        if changed {
            cx.notify();
        }
    }
}

// ================================================================================================
// MainWindow ("FnH")
// ================================================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum TopFlyout {
    Folders,
    View,
    Help,
    Account,
}

pub struct MainWindow {
    app: Entity<Fh>,
    focus: FocusHandle,
    search: String,
    search_focused: bool,
    flyout: Option<TopFlyout>,
    /// Prism-style inline rename `(instance_id, text_buffer, select_all)`.
    inline_rename: Option<(String, String, bool)>,
    /// Right-click context menu `(instance_id, x, y)`.
    ctx_menu: Option<(String, f32, f32)>,
}

impl MainWindow {
    pub fn new(app: Entity<Fh>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&app, |_: &mut Self, _, cx| cx.notify()).detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        apply_window_icon();

        let app_poll = app.clone();
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(200))
                    .await;
                let ok = this.update(cx, |_, cx| {
                    app_poll.update(cx, |fh, cx| fh.poll_job_events(cx));
                    if job::busy() {
                        cx.notify();
                    }
                });
                if ok.is_err() {
                    break;
                }
            }
        })
        .detach();

        Self {
            app,
            focus,
            search: String::new(),
            search_focused: false,
            flyout: None,
            inline_rename: None,
            ctx_menu: None,
        }
    }

    fn toggle_flyout(&mut self, which: TopFlyout, cx: &mut Context<Self>) {
        self.ctx_menu = None;
        if self.flyout == Some(which) {
            self.flyout = None;
        } else {
            self.flyout = Some(which);
        }
        cx.notify();
    }

    fn start_inline_rename(
        &mut self,
        inst: &Instance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.commit_inline_rename(cx);
        self.flyout = None;
        self.ctx_menu = None;
        self.search_focused = false;
        let id = inst.id.clone();
        self.app.update(cx, |a, _| a.select_instance(&id));
        self.inline_rename = Some((inst.id.clone(), inst.name.clone(), true));
        window.focus(&self.focus, cx);
        cx.notify();
    }

    fn commit_inline_rename(&mut self, cx: &mut Context<Self>) {
        let Some((id, buf, _)) = self.inline_rename.take() else {
            return;
        };
        let trimmed = buf.trim().to_owned();
        if !trimmed.is_empty() {
            let changed = inst::find(&id)
                .ok()
                .is_none_or(|cur| cur.name.trim() != trimmed);
            if changed {
                match inst::edit(&id, |m| m.name = Some(trimmed.clone())) {
                    Ok(_) => {
                        self.app.update(cx, |a, cx| {
                            a.reload_instances();
                            a.select_instance(&id);
                            cx.notify();
                        });
                    }
                    Err(e) => {
                        self.app.update(cx, |a, cx| {
                            a.notice = Some(e);
                            cx.notify();
                        });
                    }
                }
            }
        }
        cx.notify();
    }

    fn step_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        let q = self.search.trim().to_ascii_lowercase();
        let ids: Vec<String> = self
            .app
            .read(cx)
            .instances
            .iter()
            .filter(|i| {
                q.is_empty()
                    || i.name.to_ascii_lowercase().contains(&q)
                    || i.group
                        .as_deref()
                        .is_some_and(|g| g.to_ascii_lowercase().contains(&q))
                    || instance_summary(i).to_ascii_lowercase().contains(&q)
            })
            .map(|i| i.id.clone())
            .collect();
        if ids.is_empty() {
            return;
        }
        let cur_idx = self
            .app
            .read(cx)
            .selected_id
            .as_deref()
            .and_then(|sel| ids.iter().position(|id| id == sel))
            .unwrap_or(0);
        let next = (cur_idx as isize + delta).clamp(0, ids.len() as isize - 1) as usize;
        let next_id = ids[next].clone();
        self.app.update(cx, |a, cx| {
            a.select_instance(&next_id);
            cx.notify();
        });
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;

        // Global Ctrl+K kills any running game or active preparation immediately!
        if ctrl && key.eq_ignore_ascii_case("k") {
            if job::can_kill() {
                job::kill();
                self.app.update(cx, |a, cx| {
                    a.notice = Some("Stopped running game.".into());
                    cx.notify();
                });
            }
            return;
        }

        // Prism-style inline rename box active:
        if self.inline_rename.is_some() {
            if key == "escape" {
                self.inline_rename = None;
                cx.notify();
                return;
            }
            if key == "enter" {
                self.commit_inline_rename(cx);
                return;
            }
            if let Some((_, ref mut buf, ref mut sel_all)) = self.inline_rename {
                if apply_inline_rename_key(buf, sel_all, ev, cx) {
                    cx.notify();
                }
            }
            return;
        }

        if key == "escape" {
            if self.ctx_menu.is_some() {
                self.ctx_menu = None;
                cx.notify();
            } else if self.flyout.is_some() {
                self.flyout = None;
                cx.notify();
            } else if self.search_focused {
                self.search_focused = false;
                cx.notify();
            }
            return;
        }

        if ctrl && key.eq_ignore_ascii_case("n") {
            self.app.update(cx, |a, cx| a.open_new_instance_window(cx));
            return;
        }
        if ctrl && key == "," {
            self.app
                .update(cx, |a, cx| a.open_settings_window(SettingsPage::Launcher, cx));
            return;
        }
        if ctrl && key.eq_ignore_ascii_case("f") {
            self.search_focused = true;
            self.flyout = None;
            self.ctx_menu = None;
            cx.notify();
            return;
        }
        if ctrl && key.eq_ignore_ascii_case("e") {
            if let Some(sel) = self.app.read(cx).selected_instance() {
                self.app.update(cx, |a, cx| {
                    a.open_instance_window(&sel.id, InstancePage::Version, cx);
                });
            }
            return;
        }
        if ctrl && key.eq_ignore_ascii_case("m") {
            if let Some(sel) = self.app.read(cx).selected_instance() {
                self.app.update(cx, |a, cx| {
                    a.open_instance_window(&sel.id, InstancePage::Mods, cx);
                });
            }
            return;
        }
        if ctrl && key.eq_ignore_ascii_case("o") {
            if let Some(sel) = self.app.read(cx).selected_instance() {
                let _ = std::fs::create_dir_all(&sel.home);
                let _ = log::open_folder(&sel.home);
            }
            return;
        }

        if self.search_focused {
            if apply_text_key(&mut self.search, ev, false, cx) {
                cx.notify();
            }
            return;
        }

        if key.eq_ignore_ascii_case("f2") {
            if let Some(sel) = self.app.read(cx).selected_instance() {
                self.start_inline_rename(&sel, window, cx);
            }
            return;
        }

        if key == "left" || key == "up" {
            self.step_selection(-1, cx);
            return;
        }
        if key == "right" || key == "down" {
            self.step_selection(1, cx);
            return;
        }

        if key == "enter" {
            if let Some(sel) = self.app.read(cx).selected_instance() {
                self.app.update(cx, |a, cx| {
                    a.launch_instance(&sel.id, LaunchExtras::default(), cx);
                });
            }
        } else if key == "delete" {
            if let Some(sel) = self.app.read(cx).selected_instance() {
                self.request_delete(&sel, cx);
            }
        }
    }

    fn request_delete(&mut self, inst: &Instance, cx: &mut Context<Self>) {
        let confirm = self.app.read(cx).prefs.confirm_delete;
        if confirm {
            let mode = UtilityMode::Delete {
                id: inst.id.clone(),
                name: inst.name.clone(),
            };
            self.app.update(cx, |a, cx| a.open_utility_window(mode, cx));
        } else {
            let id = inst.id.clone();
            let name = inst.name.clone();
            let _ = inst::delete(&id);
            self.app.update(cx, |a, cx| {
                a.cfgs.remove(&id);
                a.close_instance_window(&id, cx);
                a.reload_instances();
                a.notice = Some(format!("Deleted '{name}'."));
                cx.notify();
            });
        }
    }

    fn render_top_toolbar(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let accs = account::Accounts::load();
        let active_acc = accs
            .selected
            .as_deref()
            .and_then(|u| accs.accounts.iter().find(|a| a.uuid == u))
            .or_else(|| accs.accounts.first())
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Accounts".to_owned());

        div()
            .h(px(44.))
            .px(px(10.))
            .bg(t.window)
            .border_b_1()
            .border_color(t.divider)
            .flex()
            .items_center()
            .gap(px(4.))
            .child(
                div()
                    .px(px(6.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(brand_mark(22.))
                    .child(
                        div()
                            .text_size(px(13.5))
                            .font_weight(FontWeight::BOLD)
                            .text_color(t.ink)
                            .child(format!("FNH {}", crate::VERSION)),
                    ),
            )
            .child(toolbar_separator(t))
            .child(
                toolbar_button(
                    "tb-add-inst",
                    "add",
                    "Add Instance",
                    false,
                    false,
                    true,
                    t,
                )
                .on_click(cx.listener(|this, ev: &ClickEvent, _, cx| {
                    this.flyout = None;
                    if reset::is_reset_click(ev) {
                        match reset::spawn_reset_worker() {
                            Ok(()) => std::process::exit(0),
                            Err(e) => {
                                this.app.update(cx, |a, cx| {
                                    a.notice = Some(format!("Factory reset failed: {e}"));
                                    cx.notify();
                                });
                            }
                        }
                        return;
                    }
                    this.app.update(cx, |a, cx| a.open_new_instance_window(cx));
                })),
            )
            .child(toolbar_separator(t))
            .child(
                toolbar_button(
                    "tb-folders",
                    "folder",
                    "Folders",
                    true,
                    self.flyout == Some(TopFlyout::Folders),
                    true,
                    t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_flyout(TopFlyout::Folders, cx);
                })),
            )
            .child(
                toolbar_button("tb-settings", "settings", "Settings", false, false, true, t)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.flyout = None;
                        this.app.update(cx, |a, cx| {
                            a.open_settings_window(SettingsPage::Launcher, cx);
                        });
                    })),
            )
            .child(
                toolbar_button(
                    "tb-view",
                    "grid",
                    "View",
                    true,
                    self.flyout == Some(TopFlyout::View),
                    true,
                    t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_flyout(TopFlyout::View, cx);
                })),
            )
            .child(
                toolbar_button(
                    "tb-help",
                    "help",
                    "Help",
                    true,
                    self.flyout == Some(TopFlyout::Help),
                    true,
                    t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_flyout(TopFlyout::Help, cx);
                })),
            )
            .child({
                let upd_state = updater::state();
                let (upd_lbl, upd_active) = match upd_state {
                    updater::UpdateState::Available(ref u) => {
                        (format!("Update (v{}!)", u.version), true)
                    }
                    updater::UpdateState::Downloading { percent, .. } => {
                        (format!("Updating {percent}%"), true)
                    }
                    _ => ("Update".to_owned(), false),
                };
                toolbar_button("tb-update", "download", &upd_lbl, false, upd_active, true, t)
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.flyout = None;
                        if !matches!(
                            updater::state(),
                            updater::UpdateState::Available(_)
                                | updater::UpdateState::Downloading { .. }
                                | updater::UpdateState::Applying { .. }
                        ) {
                            updater::check_in_background(false);
                        }
                        this.app.update(cx, |a, cx| {
                            a.open_utility_window(UtilityMode::Updater, cx);
                        });
                    }))
            })
            .child(div().flex_1())
            .child(
                div().w(px(210.)).child(
                    text_input(
                        "main-search",
                        &self.search,
                        "Search instances (Ctrl+F)...",
                        self.search_focused,
                        t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                        this.search_focused = true;
                        this.flyout = None;
                        window.focus(&this.focus, cx);
                        cx.notify();
                    })),
                ),
            )
            .child(toolbar_separator(t))
            .child(
                toolbar_button(
                    "tb-account",
                    "account",
                    &active_acc,
                    true,
                    self.flyout == Some(TopFlyout::Account),
                    true,
                    t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.toggle_flyout(TopFlyout::Account, cx);
                })),
            )
    }

    fn render_flyout(&self, which: TopFlyout, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let prefs = self.app.read(cx).prefs.clone();
        let (left_px, right_align, frame) = match which {
            TopFlyout::Folders => (
                210.,
                false,
                menu_frame(240., t)
                    .child(
                        menu_row("fl-inst", Some("folder"), "View Instances Folder", None, true, t)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                let _ = log::open_folder(&paths::instances_dir());
                                cx.notify();
                            })),
                    )
                    .child(
                        menu_row("fl-icons", Some("image"), "View Custom Icons Folder", None, true, t)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                let _ = log::open_folder(&paths::icons_dir());
                                cx.notify();
                            })),
                    )
                    .child(
                        menu_row("fl-java", Some("java"), "View Java Runtimes Folder", None, true, t)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                let _ = log::open_folder(&paths::java_dir());
                                cx.notify();
                            })),
                    )
                    .child(
                        menu_row("fl-logs", Some("log"), "View Launcher Logs Folder", None, true, t)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                let _ = log::open_folder(&paths::logs_dir());
                                cx.notify();
                            })),
                    )
                    .child(menu_separator(t))
                    .child(
                        menu_row("fl-root", Some("folder"), "Open FNH Root Folder", None, true, t)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                let _ = log::open_folder(&paths::root());
                                cx.notify();
                            })),
                    ),
            ),
            TopFlyout::View => (
                360.,
                false,
                menu_frame(230., t)
                    .child(
                        menu_row(
                            "vw-grid",
                            Some("grid"),
                            if prefs.view_mode == ViewMode::Grid {
                                "✓ Grid View"
                            } else {
                                "Grid View"
                            },
                            None,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flyout = None;
                            this.app.update(cx, |a, cx| {
                                a.prefs.view_mode = ViewMode::Grid;
                                a.prefs.save();
                                cx.notify();
                            });
                        })),
                    )
                    .child(
                        menu_row(
                            "vw-list",
                            Some("list"),
                            if prefs.view_mode == ViewMode::List {
                                "✓ List View"
                            } else {
                                "List View"
                            },
                            None,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flyout = None;
                            this.app.update(cx, |a, cx| {
                                a.prefs.view_mode = ViewMode::List;
                                a.prefs.save();
                                cx.notify();
                            });
                        })),
                    )
                    .child(menu_separator(t))
                    .children(SortMode::ALL.into_iter().enumerate().map(|(i, sm)| {
                        let active = prefs.sort_mode == sm;
                        let lbl = if active {
                            format!("✓ Sort: {}", sm.label())
                        } else {
                            format!("Sort: {}", sm.label())
                        };
                        menu_row(format!("vw-sort-{i}"), None, &lbl, None, true, t).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                this.app.update(cx, |a, cx| {
                                    a.prefs.sort_mode = sm;
                                    a.prefs.save();
                                    a.sort_instances();
                                    cx.notify();
                                });
                            }),
                        )
                    }))
                    .child(menu_separator(t))
                    .child(
                        menu_row(
                            "vw-appr",
                            Some("appearance"),
                            "Appearance & Icon Size...",
                            None,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flyout = None;
                            this.app.update(cx, |a, cx| {
                                a.open_settings_window(SettingsPage::Appearance, cx);
                            });
                        })),
                    ),
            ),
            TopFlyout::Help => (
                425.,
                false,
                menu_frame(240., t)
                    .child(
                        menu_row("hlp-llog", Some("log"), "View Launcher Log", None, true, t)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                this.app.update(cx, |a, cx| {
                                    a.open_utility_window(UtilityMode::LauncherLog, cx);
                                });
                            })),
                    )
                    .child(
                        menu_row(
                            "hlp-fabv",
                            Some("fabric"),
                            "Fabric Loader Versions",
                            None,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flyout = None;
                            this.app
                                .update(cx, |a, cx| a.open_fabric_versions_window(cx));
                        })),
                    )
                    .child(
                        menu_row(
                            "hlp-rel",
                            Some("download"),
                            "Check for FNH Updates...",
                            None,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flyout = None;
                            updater::check_in_background(false);
                            this.app.update(cx, |a, cx| {
                                a.open_utility_window(UtilityMode::Updater, cx);
                            });
                        })),
                    )
                    .child(
                        menu_row("hlp-site", Some("globe"), "FNH Website", None, true, t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                let _ = log::open_url("https://fnh.is-cool.dev");
                                cx.notify();
                            }),
                        ),
                    )
                    .child(menu_separator(t))
                    .child(
                        menu_row("hlp-about", Some("info"), "About FNH", None, true, t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                this.app.update(cx, |a, cx| {
                                    a.open_utility_window(UtilityMode::About { tab: 0 }, cx);
                                });
                            }),
                        ),
                    ),
            ),
            TopFlyout::Account => {
                let accs = account::Accounts::load();
                let mut f = menu_frame(240., t);
                for (i, a) in accs.accounts.iter().enumerate() {
                    let is_sel = accs.selected.as_deref() == Some(a.uuid.as_str());
                    let lbl = if is_sel {
                        format!("✓ {}", a.name)
                    } else {
                        a.name.clone()
                    };
                    let uuid_pick = a.uuid.clone();
                    f = f.child(
                        menu_row(format!("acc-m-{i}"), Some("account"), &lbl, None, true, t)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.flyout = None;
                                let mut cur = account::Accounts::load();
                                cur.selected = Some(uuid_pick.clone());
                                let _ = cur.save();
                                cx.notify();
                            })),
                    );
                }
                if !accs.accounts.is_empty() {
                    f = f.child(menu_separator(t));
                }
                f = f
                    .child(
                        menu_row(
                            "acc-signin",
                            Some("add"),
                            "Sign in with Microsoft",
                            None,
                            !job::busy(),
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flyout = None;
                            if !job::busy() {
                                job::sign_in_now();
                            }
                            cx.notify();
                        })),
                    )
                    .child(
                        menu_row(
                            "acc-manage",
                            Some("settings"),
                            "Manage Accounts...",
                            None,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flyout = None;
                            this.app.update(cx, |a, cx| {
                                a.open_settings_window(SettingsPage::Accounts, cx);
                            });
                        })),
                    );
                (0., true, f)
            }
        };

        let mut pos = div().absolute().top(px(46.)).child(frame);
        pos = if right_align {
            pos.right(px(10.))
        } else {
            pos.left(px(left_px))
        };
        pos
    }

    fn render_instances_pane(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let fh = self.app.read(cx);
        let prefs = fh.prefs.clone();
        let selected_id = fh.selected_id.clone();
        let q = self.search.trim().to_ascii_lowercase();

        let visible: Vec<Instance> = fh
            .instances
            .iter()
            .filter(|i| {
                q.is_empty()
                    || i.name.to_ascii_lowercase().contains(&q)
                    || i.group
                        .as_deref()
                        .is_some_and(|g| g.to_ascii_lowercase().contains(&q))
                    || instance_summary(i).to_ascii_lowercase().contains(&q)
            })
            .cloned()
            .collect();

        if visible.is_empty() {
            let has_any = !fh.instances.is_empty();
            return div()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .bg(t.card)
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap(px(12.))
                .p(px(24.))
                .child(brand_mark(48.))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.ink)
                        .child(if has_any {
                            "No instances match your search"
                        } else {
                            "Welcome to FnH"
                        }),
                )
                .child(
                    div()
                        .max_w(px(420.))
                        .text_size(px(12.5))
                        .text_color(t.ink_secondary)
                        .child(if has_any {
                            "Try clearing the search box in the top right."
                        } else {
                            "Create an isolated Fabric, Vanilla, or Horizon instance to get started. Each instance has its own mods, worlds, screenshots, settings, and live console window."
                        }),
                )
                .when(!has_any, |d| {
                    d.child(
                        accent_button("empty-add-inst", "Add Instance...", true, t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.app.update(cx, |a, cx| a.open_new_instance_window(cx));
                            }),
                        ),
                    )
                });
        }

        // Group instances cleanly (named groups alphabetically, then "Ungrouped")
        let mut bucket_map: HashMap<String, Vec<Instance>> = HashMap::new();
        for inst_item in visible {
            let grp = inst_item
                .group
                .as_deref()
                .map(str::trim)
                .filter(|g| !g.is_empty())
                .unwrap_or("Ungrouped")
                .to_owned();
            bucket_map.entry(grp).or_default().push(inst_item);
        }
        let mut group_names: Vec<String> = bucket_map.keys().cloned().collect();
        group_names.sort_by(|a, b| match (a == "Ungrouped", b == "Ungrouped") {
            (true, false) => std::cmp::Ordering::Greater,
            (false, true) => std::cmp::Ordering::Less,
            _ => a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase()),
        });

        let icon_sz = prefs.icon_size.clamp(32, 128) as f32;
        let is_grid = prefs.view_mode == ViewMode::Grid;
        let active_job_inst = job::active_instance();

        div().flex_1().min_w_0().min_h_0().flex().flex_col().child(div()
            .id("main-instances-scroll")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .bg(t.card)
            .p(px(14.))
            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                this.commit_inline_rename(cx);
                this.flyout = None;
                this.ctx_menu = None;
                this.search_focused = false;
                window.focus(&this.focus, cx);
                cx.notify();
            }))
            .flex()
            .flex_col()
            .gap(px(14.))
            .children(group_names.into_iter().enumerate().map(|(gi, gname)| {
                let items = bucket_map.remove(&gname).unwrap_or_default();
                let collapsed = prefs.is_group_collapsed(&gname);
                let g_toggle = gname.clone();
                let hdr_text = if prefs.show_group_counts {
                    format!("{gname} ({})", items.len())
                } else {
                    gname.clone()
                };

                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .id(SharedString::from(format!("grp-hdr-{gi}")))
                            .cursor_pointer()
                            .py(px(3.))
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let g = g_toggle.clone();
                                this.app.update(cx, |a, cx| {
                                    a.prefs.toggle_group(&g);
                                    a.prefs.save();
                                    cx.notify();
                                });
                            }))
                            .child(icons::icon(
                                if collapsed {
                                    "chevron-right"
                                } else {
                                    "chevron-down"
                                },
                                14.,
                                t.ink_secondary,
                            ))
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.ink_secondary)
                                    .child(hdr_text),
                            )
                            .child(div().flex_1().h(px(1.)).bg(t.divider)),
                    )
                    .when(!collapsed, |sec| {
                        if is_grid {
                            let tile_w = (icon_sz + 56.).max(108.);
                            sec.child(
                                div().flex().flex_wrap().gap(px(10.)).children(
                                    items.into_iter().enumerate().map(|(ii, item)| {
                                        let is_sel =
                                            selected_id.as_deref() == Some(item.id.as_str());
                                        let is_running = active_job_inst.as_deref()
                                            == Some(item.id.as_str())
                                            && job::busy();
                                        let cfg = self.app.read(cx).cfgs.get(&item.id);
                                        let icon_key = cfg.icon_key.unwrap_or_else(|| {
                                            inst_icons::default_key_for(&item).to_owned()
                                        });
                                        let id_click = item.id.clone();
                                        let id_ctx = item.id.clone();
                                        let item_name_click = item.clone();
                                        let item_drop = item.clone();
                                        let dbl_edits = prefs.double_click_edits;
                                        let show_pt = prefs.show_instance_playtime
                                            && item.played.as_secs() > 0;
                                        let pt_str =
                                            prefs.format_playtime(item.played.as_secs());
                                        let renaming_state = self
                                            .inline_rename
                                            .as_ref()
                                            .filter(|(rid, _, _)| rid == &item.id)
                                            .map(|(_, buf, sa)| (buf.clone(), *sa));

                                        div()
                                            .id(SharedString::from(format!(
                                                "inst-tile-{gi}-{ii}"
                                            )))
                                            .w(px(tile_w))
                                            .p(px(8.))
                                            .rounded(px(6.))
                                            .cursor_pointer()
                                            .border_1()
                                            .border_color(if is_sel {
                                                t.tile_selected_border
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .bg(if is_sel {
                                                t.tile_selected
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .hover(|s| {
                                                if is_sel {
                                                    s
                                                } else {
                                                    s.bg(t.subtle_hover)
                                                }
                                            })
                                            .drag_over::<ExternalPaths>({
                                                let acc = t.accent;
                                                let bg = t.tile_selected;
                                                move |s, _, _, _| s.border_color(acc).bg(bg)
                                            })
                                            .on_drop(cx.listener(
                                                move |this, paths: &ExternalPaths, _, cx| {
                                                    let files: Vec<PathBuf> =
                                                        paths.paths().to_vec();
                                                    if files.len() == 1
                                                        && inst_icons::is_image_file(&files[0])
                                                    {
                                                        if let Ok(key) =
                                                            inst_icons::import_custom_icon(
                                                                &files[0],
                                                            )
                                                        {
                                                            let id_t = item_drop.id.clone();
                                                            this.app.update(cx, |a, cx| {
                                                                a.apply_picked_icon(
                                                                    IconTarget::Instance(id_t),
                                                                    key,
                                                                    cx,
                                                                );
                                                                a.notice = Some(format!(
                                                                    "Updated icon for '{}'.",
                                                                    item_drop.name
                                                                ));
                                                            });
                                                            return;
                                                        }
                                                    }
                                                    let res =
                                                        content::add_mod_files(&item_drop, &files);
                                                    this.app.update(cx, |a, cx| {
                                                        a.notice = Some(match res {
                                                            Ok(m) => m,
                                                            Err(e) => e,
                                                        });
                                                        cx.notify();
                                                    });
                                                },
                                            ))
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(
                                                    move |this, ev: &MouseDownEvent, window, cx| {
                                                        cx.stop_propagation();
                                                        this.commit_inline_rename(cx);
                                                        this.flyout = None;
                                                        this.search_focused = false;
                                                        let id_r = id_ctx.clone();
                                                        this.app.update(cx, |a, _| {
                                                            a.select_instance(&id_r);
                                                        });
                                                        this.ctx_menu = Some((
                                                            id_r,
                                                            f32::from(ev.position.x),
                                                            f32::from(ev.position.y),
                                                        ));
                                                        window.focus(&this.focus, cx);
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .on_click(cx.listener(
                                                move |this, ev: &ClickEvent, window, cx| {
                                                    cx.stop_propagation();
                                                    if this
                                                        .inline_rename
                                                        .as_ref()
                                                        .is_some_and(|(rid, _, _)| rid != &id_click)
                                                    {
                                                        this.commit_inline_rename(cx);
                                                    } else if this.inline_rename.is_some() {
                                                        this.commit_inline_rename(cx);
                                                    }
                                                    this.flyout = None;
                                                    this.ctx_menu = None;
                                                    this.search_focused = false;
                                                    window.focus(&this.focus, cx);
                                                    let dbl = matches!(
                                                        ev,
                                                        ClickEvent::Mouse(m)
                                                            if m.up.click_count >= 2
                                                    );
                                                    let id_c = id_click.clone();
                                                    this.app.update(cx, |a, cx| {
                                                        a.select_instance(&id_c);
                                                        if dbl {
                                                            if dbl_edits {
                                                                a.open_instance_window(
                                                                    &id_c,
                                                                    InstancePage::Version,
                                                                    cx,
                                                                );
                                                            } else {
                                                                a.launch_instance(
                                                                    &id_c,
                                                                    LaunchExtras::default(),
                                                                    cx,
                                                                );
                                                            }
                                                        }
                                                        cx.notify();
                                                    });
                                                },
                                            ))
                                            .flex()
                                            .flex_col()
                                            .items_center()
                                            .gap(px(6.))
                                            .child(inst_icons::render_icon(&icon_key, icon_sz))
                                            .child(if let Some((buf, sel_all)) = renaming_state {
                                                inline_rename_input(
                                                    format!("tile-ren-{gi}-{ii}"),
                                                    &buf,
                                                    sel_all,
                                                    true,
                                                    t,
                                                )
                                                .on_mouse_down(
                                                    MouseButton::Left,
                                                    |_, _, cx| cx.stop_propagation(),
                                                )
                                                .on_click(cx.listener(
                                                    |this, _: &ClickEvent, window, cx| {
                                                        cx.stop_propagation();
                                                        if let Some((_, _, sa)) =
                                                            &mut this.inline_rename
                                                        {
                                                            *sa = false;
                                                        }
                                                        window.focus(&this.focus, cx);
                                                        cx.notify();
                                                    },
                                                ))
                                                .into_any_element()
                                            } else {
                                                div()
                                                    .id(SharedString::from(format!(
                                                        "inst-tile-name-{gi}-{ii}"
                                                    )))
                                                    .w_full()
                                                    .px(px(3.))
                                                    .py(px(1.))
                                                    .rounded(px(3.))
                                                    .cursor_text()
                                                    .hover(|s| s.bg(t.subtle_hover))
                                                    .text_size(px(12.5))
                                                    .font_weight(if is_sel {
                                                        FontWeight::SEMIBOLD
                                                    } else {
                                                        FontWeight::NORMAL
                                                    })
                                                    .text_color(t.ink)
                                                    .truncate()
                                                    .flex()
                                                    .justify_center()
                                                    .on_click(cx.listener(
                                                        move |this, _: &ClickEvent, window, cx| {
                                                            cx.stop_propagation();
                                                            this.start_inline_rename(
                                                                &item_name_click,
                                                                window,
                                                                cx,
                                                            );
                                                        },
                                                    ))
                                                    .child(item.name.clone())
                                                    .into_any_element()
                                            })
                                            .when(is_running, |d| {
                                                d.child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .font_weight(FontWeight::SEMIBOLD)
                                                        .text_color(t.accent)
                                                        .child("Running"),
                                                )
                                            })
                                            .when(!is_running && show_pt, |d| {
                                                d.child(
                                                    div()
                                                        .text_size(px(10.5))
                                                        .text_color(t.ink_tertiary)
                                                        .child(pt_str),
                                                )
                                            })
                                    }),
                                ),
                            )
                        } else {
                            // List View
                            let row_icon = icon_sz.clamp(24., 48.);
                            sec.child(
                                div().flex().flex_col().gap(px(3.)).children(
                                    items.into_iter().enumerate().map(|(ii, item)| {
                                        let is_sel =
                                            selected_id.as_deref() == Some(item.id.as_str());
                                        let cfg = self.app.read(cx).cfgs.get(&item.id);
                                        let icon_key = cfg.icon_key.unwrap_or_else(|| {
                                            inst_icons::default_key_for(&item).to_owned()
                                        });
                                        let id_click = item.id.clone();
                                        let id_ctx = item.id.clone();
                                        let item_name_click = item.clone();
                                        let dbl_edits = prefs.double_click_edits;
                                        let summary = instance_summary(&item);
                                        let pt = prefs.format_playtime(item.played.as_secs());
                                        let renaming_state = self
                                            .inline_rename
                                            .as_ref()
                                            .filter(|(rid, _, _)| rid == &item.id)
                                            .map(|(_, buf, sa)| (buf.clone(), *sa));

                                        div()
                                            .id(SharedString::from(format!(
                                                "inst-row-{gi}-{ii}"
                                            )))
                                            .px(px(10.))
                                            .py(px(6.))
                                            .rounded(px(4.))
                                            .cursor_pointer()
                                            .border_1()
                                            .border_color(if is_sel {
                                                t.tile_selected_border
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .bg(if is_sel {
                                                t.tile_selected
                                            } else {
                                                gpui::transparent_black()
                                            })
                                            .hover(|s| {
                                                if is_sel {
                                                    s
                                                } else {
                                                    s.bg(t.subtle_hover)
                                                }
                                            })
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                cx.listener(
                                                    move |this, ev: &MouseDownEvent, window, cx| {
                                                        cx.stop_propagation();
                                                        this.commit_inline_rename(cx);
                                                        this.flyout = None;
                                                        this.search_focused = false;
                                                        let id_r = id_ctx.clone();
                                                        this.app.update(cx, |a, _| {
                                                            a.select_instance(&id_r);
                                                        });
                                                        this.ctx_menu = Some((
                                                            id_r,
                                                            f32::from(ev.position.x),
                                                            f32::from(ev.position.y),
                                                        ));
                                                        window.focus(&this.focus, cx);
                                                        cx.notify();
                                                    },
                                                ),
                                            )
                                            .on_click(cx.listener(
                                                move |this, ev: &ClickEvent, window, cx| {
                                                    cx.stop_propagation();
                                                    if this.inline_rename.is_some() {
                                                        this.commit_inline_rename(cx);
                                                    }
                                                    this.flyout = None;
                                                    this.ctx_menu = None;
                                                    this.search_focused = false;
                                                    window.focus(&this.focus, cx);
                                                    let dbl = matches!(
                                                        ev,
                                                        ClickEvent::Mouse(m)
                                                            if m.up.click_count >= 2
                                                    );
                                                    let id_c = id_click.clone();
                                                    this.app.update(cx, |a, cx| {
                                                        a.select_instance(&id_c);
                                                        if dbl {
                                                            if dbl_edits {
                                                                a.open_instance_window(
                                                                    &id_c,
                                                                    InstancePage::Version,
                                                                    cx,
                                                                );
                                                            } else {
                                                                a.launch_instance(
                                                                    &id_c,
                                                                    LaunchExtras::default(),
                                                                    cx,
                                                                );
                                                            }
                                                        }
                                                        cx.notify();
                                                    });
                                                },
                                            ))
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
                                                    .child(inst_icons::render_icon(
                                                        &icon_key, row_icon,
                                                    ))
                                                    .child(
                                                        div()
                                                            .min_w_0()
                                                            .flex()
                                                            .flex_col()
                                                            .child(
                                                                if let Some((buf, sel_all)) =
                                                                    renaming_state
                                                                {
                                                                    div()
                                                                        .w(px(220.))
                                                                        .child(
                                                                            inline_rename_input(
                                                                                format!(
                                                                                    "row-ren-{gi}-{ii}"
                                                                                ),
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
                                                                                    )) =
                                                                                        &mut this
                                                                                            .inline_rename
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
                                                                        .id(SharedString::from(
                                                                            format!(
                                                                                "inst-row-name-{gi}-{ii}"
                                                                            ),
                                                                        ))
                                                                        .px(px(3.))
                                                                        .rounded(px(3.))
                                                                        .cursor_text()
                                                                        .hover(|s| {
                                                                            s.bg(t.subtle_hover)
                                                                        })
                                                                        .text_size(px(13.))
                                                                        .font_weight(
                                                                            FontWeight::SEMIBOLD,
                                                                        )
                                                                        .text_color(t.ink)
                                                                        .truncate()
                                                                        .on_click(cx.listener(
                                                                            move |this,
                                                                                  _: &ClickEvent,
                                                                                  window,
                                                                                  cx| {
                                                                                cx.stop_propagation();
                                                                                this.start_inline_rename(
                                                                                    &item_name_click,
                                                                                    window,
                                                                                    cx,
                                                                                );
                                                                            },
                                                                        ))
                                                                        .child(item.name.clone())
                                                                        .into_any_element()
                                                                },
                                                            )
                                                            .child(
                                                                div()
                                                                    .text_size(px(11.5))
                                                                    .text_color(t.ink_secondary)
                                                                    .truncate()
                                                                    .child(summary),
                                                            ),
                                                    ),
                                            )
                                            .when(prefs.show_instance_playtime, |d| {
                                                d.child(
                                                    div()
                                                        .text_size(px(11.5))
                                                        .text_color(t.ink_tertiary)
                                                        .child(pt),
                                                )
                                            })
                                    }),
                                ),
                            )
                        }
                    })
            })))
    }

    fn render_wide_bar(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let sel = self.app.read(cx).selected_instance();
        let has = sel.is_some();
        let is_busy = job::busy();
        let can_launch = has && !is_busy;
        let can_kill = job::can_kill();

        let header = match &sel {
            Some(inst) => {
                let cfg = self.app.read(cx).cfgs.get(&inst.id);
                let icon_key = cfg
                    .icon_key
                    .unwrap_or_else(|| inst_icons::default_key_for(inst).to_owned());
                let id_icon = inst.id.clone();
                let key_open = icon_key.clone();
                let inst_for_name = inst.clone();
                let renaming_wb = self
                    .inline_rename
                    .as_ref()
                    .filter(|(rid, _, _)| rid == &inst.id)
                    .map(|(_, buf, sa)| (buf.clone(), *sa));

                div()
                    .p(px(4.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(4.))
                    .child(
                        div()
                            .id("wb-inst-icon")
                            .p(px(4.))
                            .rounded(px(6.))
                            .cursor_pointer()
                            .hover(|s| s.bg(t.subtle_hover))
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let id_c = id_icon.clone();
                                let k_c = key_open.clone();
                                this.app.update(cx, |a, cx| {
                                    a.open_icon_picker(IconTarget::Instance(id_c), &k_c, cx);
                                });
                            }))
                            .child(inst_icons::render_icon(&icon_key, 48.)),
                    )
                    .child(if let Some((buf, sel_all)) = renaming_wb {
                        inline_rename_input("wb-ren-input", &buf, sel_all, true, t)
                            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                if let Some((_, _, sa)) = &mut this.inline_rename {
                                    *sa = false;
                                }
                                window.focus(&this.focus, cx);
                                cx.notify();
                            }))
                            .into_any_element()
                    } else {
                        div()
                            .id("wb-inst-name")
                            .w_full()
                            .px(px(4.))
                            .py(px(1.))
                            .rounded(px(3.))
                            .cursor_text()
                            .hover(|s| s.bg(t.subtle_hover))
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.ink)
                            .truncate()
                            .flex()
                            .justify_center()
                            .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                this.start_inline_rename(&inst_for_name, window, cx);
                            }))
                            .child(inst.name.clone())
                            .into_any_element()
                    })
                    .child(
                        div()
                            .w_full()
                            .text_size(px(11.))
                            .text_color(t.ink_tertiary)
                            .truncate()
                            .flex()
                            .justify_center()
                            .child(instance_summary(inst)),
                    )
            }
            None => div()
                .p(px(10.))
                .text_size(px(12.))
                .text_color(t.ink_tertiary)
                .child("No instance selected"),
        };

        div()
            .w(px(196.))
            .flex_shrink_0()
            .h_full()
            .min_h_0()
            .flex()
            .flex_col()
            .child(
                div()
                    .id("widebar-scroll")
                    .flex_1()
                    .min_h_0()
                    .w(px(196.))
                    .flex_shrink_0()
                    .overflow_y_scroll()
                    .bg(t.sidebar)
                    .border_l_1()
                    .border_color(t.divider)
                    .p(px(6.))
                    .flex()
                    .flex_col()
                    .gap(px(1.5))
                    .child(header)
                    .child(wide_bar_separator(t))
                    .child({
                        let s_l = sel.clone();
                        wide_bar_action("wb-launch", "play", "Launch", can_launch, true, t)
                            .when_some(s_l.filter(|_| can_launch), |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    window.focus(&this.focus, cx);
                                    let id = inst.id.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.launch_instance(&id, LaunchExtras::default(), cx);
                                    });
                                }))
                            })
                    })
                    .child({
                        let s_lo = sel.clone();
                        wide_bar_action(
                            "wb-offline",
                            "account",
                            "Launch Offline",
                            can_launch,
                            false,
                            t,
                        )
                        .when_some(s_lo.filter(|_| can_launch), |b, inst| {
                            b.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                window.focus(&this.focus, cx);
                                let id = inst.id.clone();
                                this.app.update(cx, |a, cx| {
                                    a.launch_instance(
                                        &id,
                                        LaunchExtras {
                                            offline: true,
                                            ..LaunchExtras::default()
                                        },
                                        cx,
                                    );
                                });
                            }))
                        })
                    })
                    .when(can_kill, |d| {
                        d.child(
                            danger_button("wb-kill", "Kill (Ctrl+K)", true, t).on_click(
                                cx.listener(|this, _: &ClickEvent, window, cx| {
                                    window.focus(&this.focus, cx);
                                    job::kill();
                                    this.app.update(cx, |a, cx| {
                                        a.notice = Some("Stopped running game.".into());
                                        cx.notify();
                                    });
                                }),
                            ),
                        )
                    })
                    .child(wide_bar_separator(t))
                    .child({
                        let s_ed = sel.clone();
                        wide_bar_action("wb-edit", "edit", "Edit", has, false, t).when_some(
                            s_ed,
                            |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let id = inst.id.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.open_instance_window(&id, InstancePage::Version, cx);
                                    });
                                }))
                            },
                        )
                    })
                    .child({
                        let s_log = sel.clone();
                        wide_bar_action("wb-log", "log", "Minecraft Log", has, false, t)
                            .when_some(s_log, |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let id = inst.id.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.open_instance_window(&id, InstancePage::Log, cx);
                                    });
                                }))
                            })
                    })
                    .child({
                        let s_mods = sel.clone();
                        wide_bar_action("wb-mods", "mods", "Mods", has, false, t).when_some(
                            s_mods,
                            |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let id = inst.id.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.open_instance_window(&id, InstancePage::Mods, cx);
                                    });
                                }))
                            },
                        )
                    })
                    .child({
                        let s_wld = sel.clone();
                        wide_bar_action("wb-worlds", "world", "Worlds", has, false, t).when_some(
                            s_wld,
                            |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let id = inst.id.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.open_instance_window(&id, InstancePage::Worlds, cx);
                                    });
                                }))
                            },
                        )
                    })
                    .child({
                        let s_cfg = sel.clone();
                        wide_bar_action("wb-instcfg", "settings", "Instance Settings", has, false, t)
                            .when_some(s_cfg, |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let id = inst.id.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.open_instance_window(&id, InstancePage::Settings, cx);
                                    });
                                }))
                            })
                    })
                    .child(wide_bar_separator(t))
                    .child({
                        let s_grp = sel.clone();
                        wide_bar_action("wb-group", "group", "Change Group", has, false, t)
                            .when_some(s_grp, |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let mode = UtilityMode::Group {
                                        id: inst.id.clone(),
                                        value: inst.group.clone().unwrap_or_default(),
                                    };
                                    this.app.update(cx, |a, cx| a.open_utility_window(mode, cx));
                                }))
                            })
                    })
                    .child({
                        let s_ren = sel.clone();
                        wide_bar_action("wb-rename", "edit", "Rename", has, false, t).when_some(
                            s_ren,
                            |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    cx.stop_propagation();
                                    this.start_inline_rename(&inst, window, cx);
                                }))
                            },
                        )
                    })
                    .child({
                        let s_fld = sel.clone();
                        wide_bar_action("wb-folder", "folder", "Folder", has, false, t).when_some(
                            s_fld,
                            |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let _ = std::fs::create_dir_all(&inst.home);
                                    if let Err(e) = log::open_folder(&inst.home) {
                                        this.app.update(cx, |a, cx| {
                                            a.notice = Some(e);
                                            cx.notify();
                                        });
                                    }
                                }))
                            },
                        )
                    })
                    .child({
                        let s_cp = sel.clone();
                        wide_bar_action("wb-copy", "copy", "Copy Instance", has, false, t)
                            .when_some(s_cp, |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let mode = UtilityMode::Copy {
                                        id: inst.id.clone(),
                                        value: format!("{} (Copy)", inst.name),
                                    };
                                    this.app.update(cx, |a, cx| a.open_utility_window(mode, cx));
                                }))
                            })
                    })
                    .child({
                        let s_sc = sel.clone();
                        wide_bar_action("wb-shortcut", "shortcut", "Create Shortcut", has, false, t)
                            .when_some(s_sc, |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let mode = UtilityMode::Shortcut {
                                        id: inst.id.clone(),
                                        name: inst.name.clone(),
                                        target: ShortcutTarget::Desktop,
                                        offline: false,
                                    };
                                    this.app.update(cx, |a, cx| a.open_utility_window(mode, cx));
                                }))
                            })
                    })
                    .child({
                        let s_del = sel;
                        wide_bar_action("wb-delete", "delete", "Delete", has, false, t).when_some(
                            s_del,
                            |b, inst| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.request_delete(&inst, cx);
                                }))
                            },
                        )
                    }),
            )
    }

    fn render_context_menu(
        &self,
        inst_id: &str,
        x: f32,
        y: f32,
        vp_w: f32,
        vp_h: f32,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Div> {
        let inst = self
            .app
            .read(cx)
            .instances
            .iter()
            .find(|i| i.id == inst_id)
            .cloned()?;
        let can_launch = !job::busy();
        let can_kill = job::can_kill();

        let menu_w = 224.0_f32;
        let menu_h = 330.0_f32;
        let left = x.clamp(8.0, (vp_w - menu_w - 8.0).max(8.0));
        let top = y.clamp(8.0, (vp_h - menu_h - 8.0).max(8.0));

        let i_launch = inst.clone();
        let i_off = inst.clone();
        let i_edit = inst.clone();
        let i_mods = inst.clone();
        let i_log = inst.clone();
        let i_ren = inst.clone();
        let i_icon = inst.clone();
        let i_fld = inst.clone();
        let i_cp = inst.clone();
        let i_del = inst;

        let frame = menu_frame(menu_w, t)
            .child(
                menu_row("ctx-launch", Some("play"), "Launch", Some("Enter"), can_launch, t)
                    .when(can_launch, |r| {
                        r.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ctx_menu = None;
                            let id = i_launch.id.clone();
                            this.app.update(cx, |a, cx| {
                                a.launch_instance(&id, LaunchExtras::default(), cx);
                            });
                        }))
                    }),
            )
            .child(
                menu_row("ctx-offline", Some("account"), "Launch Offline", None, can_launch, t)
                    .when(can_launch, |r| {
                        r.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.ctx_menu = None;
                            let id = i_off.id.clone();
                            this.app.update(cx, |a, cx| {
                                a.launch_instance(
                                    &id,
                                    LaunchExtras {
                                        offline: true,
                                        ..LaunchExtras::default()
                                    },
                                    cx,
                                );
                            });
                        }))
                    }),
            )
            .when(can_kill, |f| {
                f.child(
                    menu_row("ctx-kill", Some("kill"), "Kill Running Game", Some("Ctrl+K"), true, t)
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.ctx_menu = None;
                            job::kill();
                            this.app.update(cx, |a, cx| {
                                a.notice = Some("Stopped running game.".into());
                                cx.notify();
                            });
                        })),
                )
            })
            .child(menu_separator(t))
            .child(
                menu_row("ctx-edit", Some("edit"), "Edit Instance...", Some("Ctrl+E"), true, t)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ctx_menu = None;
                        let id = i_edit.id.clone();
                        this.app.update(cx, |a, cx| {
                            a.open_instance_window(&id, InstancePage::Version, cx);
                        });
                    })),
            )
            .child(
                menu_row("ctx-mods", Some("mods"), "Mods...", Some("Ctrl+M"), true, t).on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ctx_menu = None;
                        let id = i_mods.id.clone();
                        this.app.update(cx, |a, cx| {
                            a.open_instance_window(&id, InstancePage::Mods, cx);
                        });
                    }),
                ),
            )
            .child(
                menu_row("ctx-log", Some("log"), "Minecraft Log...", None, true, t).on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ctx_menu = None;
                        let id = i_log.id.clone();
                        this.app.update(cx, |a, cx| {
                            a.open_instance_window(&id, InstancePage::Log, cx);
                        });
                    }),
                ),
            )
            .child(menu_separator(t))
            .child(
                menu_row("ctx-ren", Some("edit"), "Rename", Some("F2"), true, t).on_click(
                    cx.listener(move |this, _: &ClickEvent, window, cx| {
                        this.ctx_menu = None;
                        this.start_inline_rename(&i_ren, window, cx);
                    }),
                ),
            )
            .child(
                menu_row("ctx-icon", Some("image"), "Change Icon...", None, true, t).on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ctx_menu = None;
                        let key = this
                            .app
                            .read(cx)
                            .cfgs
                            .get(&i_icon.id)
                            .icon_key
                            .unwrap_or_else(|| inst_icons::default_key_for(&i_icon).to_owned());
                        let id = i_icon.id.clone();
                        this.app.update(cx, |a, cx| {
                            a.open_icon_picker(IconTarget::Instance(id), &key, cx);
                        });
                    }),
                ),
            )
            .child(
                menu_row("ctx-folder", Some("folder"), "Open Folder", Some("Ctrl+O"), true, t)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ctx_menu = None;
                        let _ = std::fs::create_dir_all(&i_fld.home);
                        let _ = log::open_folder(&i_fld.home);
                        cx.notify();
                    })),
            )
            .child(
                menu_row("ctx-copy", Some("copy"), "Copy Instance...", None, true, t).on_click(
                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ctx_menu = None;
                        let mode = UtilityMode::Copy {
                            id: i_cp.id.clone(),
                            value: format!("{} (Copy)", i_cp.name),
                        };
                        this.app.update(cx, |a, cx| a.open_utility_window(mode, cx));
                    }),
                ),
            )
            .child(menu_separator(t))
            .child(
                menu_row("ctx-del", Some("delete"), "Delete Instance...", Some("Del"), true, t)
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.ctx_menu = None;
                        this.request_delete(&i_del, cx);
                    })),
            );

        Some(
            div()
                .absolute()
                .left(px(left))
                .top(px(top))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(frame),
        )
    }

    fn render_status_bar(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let fh = self.app.read(cx);
        let prefs = &fh.prefs;
        let count = fh.instances.len();
        let total_secs: u64 = fh.instances.iter().map(|i| i.played.as_secs()).sum();
        let total_str = prefs.format_playtime(total_secs);
        let sel_summary = fh
            .selected_instance()
            .map(|i| format!("{} — {}", i.name, instance_summary(&i)))
            .unwrap_or_else(|| "Ready".to_owned());

        let st = job::status();
        let left_el = match st {
            Some(s) if s.busy || s.failed => {
                let msg = if let Some(frac) = s.fraction {
                    format!(
                        "{}: {} ({:.0}%)",
                        s.title,
                        s.line,
                        (frac.clamp(0.0, 1.0) * 100.0)
                    )
                } else {
                    format!("{}: {}", s.title, s.line)
                };
                let failed_inst = s.instance_id.clone();
                div()
                    .min_w_0()
                    .flex_1()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .min_w_0()
                            .truncate()
                            .text_size(px(11.5))
                            .text_color(if s.failed { t.danger } else { t.accent })
                            .child(msg),
                    )
                    .when_some(s.code, |d, (code, link)| {
                        let c_cp = code.clone();
                        let url = log::sign_in_url(&link, &code);
                        d.child(
                            chip("sb-copy-code", format!("Code {code} (Copy & Open)"), true, t)
                                .on_click(move |_, _, cx| {
                                    cx.write_to_clipboard(ClipboardItem::new_string(c_cp.clone()));
                                    let _ = log::open_url(&url);
                                }),
                        )
                    })
                    .when(s.busy && job::can_kill(), |d| {
                        d.child(
                            danger_button("sb-kill-run", "Kill (Ctrl+K)", true, t).on_click(
                                cx.listener(|this, _: &ClickEvent, window, cx| {
                                    window.focus(&this.focus, cx);
                                    job::kill();
                                    this.app.update(cx, |a, cx| {
                                        a.notice = Some("Stopped running game.".into());
                                        cx.notify();
                                    });
                                }),
                            ),
                        )
                    })
                    .when_some(failed_inst.filter(|_| s.failed), |d, inst_id| {
                        d.child(
                            push_button("sb-view-log", "View Log", true, t).on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    let id = inst_id.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.open_instance_window(&id, InstancePage::Log, cx);
                                    });
                                },
                            )),
                        )
                    })
                    .when(s.failed, |d| {
                        d.child(
                            push_button("sb-dismiss-err", "Dismiss", true, t)
                                .on_click(|_, _, _| job::clear()),
                        )
                    })
            }
            _ => div()
                .text_size(px(11.5))
                .text_color(t.ink_secondary)
                .child(sel_summary),
        };

        div()
            .h(px(32.))
            .px(px(12.))
            .bg(t.footer)
            .border_t_1()
            .border_color(t.divider)
            .flex()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .child(left_el)
            .child(
                div()
                    .flex_none()
                    .font_family(MONO_FONT)
                    .text_size(px(11.))
                    .text_color(t.ink_tertiary)
                    .child(if prefs.show_total_playtime {
                        format!("{count} instance(s)  ·  Total played: {total_str}")
                    } else {
                        format!("{count} instance(s)")
                    }),
            )
    }
}

impl Focusable for MainWindow {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for MainWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.app.read(cx).prefs.clone();
        let notice = self.app.read(cx).notice.clone();
        let t = Theme::resolve(prefs.theme_mode, prefs.accent, window.appearance());
        let vp = window.viewport_size();
        let vp_w = f32::from(vp.width);
        let vp_h = f32::from(vp.height);
        let ctx_menu_el = self
            .ctx_menu
            .as_ref()
            .and_then(|(id, x, y)| self.render_context_menu(id, *x, *y, vp_w, vp_h, &t, cx));

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _, cx| {
                    if this.ctx_menu.take().is_some() {
                        cx.notify();
                    }
                }),
            )
            .relative()
            .size_full()
            .bg(t.window)
            .font_family(UI_FONT)
            .text_color(t.ink)
            .flex()
            .flex_col()
            .child(self.render_top_toolbar(&t, cx))
            .when_some(notice, |d, msg| {
                d.child(
                    div()
                        .px(px(12.))
                        .py(px(6.))
                        .bg(t.layer)
                        .border_b_1()
                        .border_color(t.divider)
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(div().flex_1().child(status_banner(&msg, false, &t)))
                        .child(
                            push_button("dismiss-notice", "Dismiss", true, &t).on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.app.update(cx, |a, cx| {
                                        a.notice = None;
                                        cx.notify();
                                    });
                                }),
                            ),
                        ),
                )
            })
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(self.render_instances_pane(&t, cx))
                    .child(self.render_wide_bar(&t, cx)),
            )
            .when(prefs.show_status_bar, |d| d.child(self.render_status_bar(&t, cx)))
            .when_some(self.flyout, |d, which| {
                d.child(self.render_flyout(which, &t, cx))
            })
            .when_some(ctx_menu_el, |d, el| d.child(el))
    }
}
