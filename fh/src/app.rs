//! Shared multi-window application state (`Fh`) and primary launcher shell (`MainWindow`),
//! modelled on JLac's `MainWindow` (`mainToolBar` + collapsible grouped `InstanceView` + right
//! `WideBar` + `QStatusBar`).

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use gpui::prelude::FluentBuilder;
use gpui::{
    App, ClickEvent, ClipboardItem, Context, Entity, ExternalPaths, FocusHandle, Focusable,
    FontWeight, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Render, SharedString,
    StatefulInteractiveElement, Styled, WeakEntity, Window, WindowHandle, div, px,
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
    MONO_FONT, Theme, UI_FONT, accent_button, apply_text_key, brand_mark, chip, instance_summary,
    menu_frame, menu_row, menu_separator, push_button, status_banner, text_input, toolbar_button,
    toolbar_separator, wide_bar_action, wide_bar_separator,
};
use crate::paths;
use crate::prefs::{InstanceCfgStore, Prefs, SortMode, ViewMode};
use crate::reset;
use crate::win_dialogs::{
    IconPickerWindow, ModBrowserWindow, NewInstanceWindow, UtilityMode, UtilityWindow,
};
use crate::win_instance::{InstancePage, InstanceWindow};
use crate::win_settings::{SettingsPage, SettingsWindow};

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

    seen_event_seq: u64,
    was_busy: bool,
}

impl Fh {
    pub fn new(after_reset: bool) -> Self {
        let prefs = Prefs::load();
        prefs.apply_proxy_env();
        let cfgs = InstanceCfgStore::load();
        let mut s = Self {
            prefs,
            cfgs,
            instances: Vec::new(),
            selected_id: None,
            notice: after_reset.then(|| {
                "Factory reset complete — FNH has been restored to a clean slate.".to_owned()
            }),
            instance_windows: HashMap::new(),
            settings_window: None,
            new_instance_window: None,
            icon_picker_window: None,
            mod_browser_windows: HashMap::new(),
            utility_window: None,
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

    pub fn poll_job_events(&mut self, cx: &mut Context<Self>) {
        let st = job::status();
        let now_busy = st.as_ref().is_some_and(|s| s.busy);
        let mut changed = now_busy != self.was_busy;
        self.was_busy = now_busy;

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
// MainWindow ("FNH 1.6")
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
}

impl MainWindow {
    pub fn new(app: Entity<Fh>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        cx.observe(&app, |_, _, cx| cx.notify()).detach();
        let focus = cx.focus_handle();
        window.focus(&focus, cx);

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
        }
    }

    fn toggle_flyout(&mut self, which: TopFlyout, cx: &mut Context<Self>) {
        if self.flyout == Some(which) {
            self.flyout = None;
        } else {
            self.flyout = Some(which);
        }
        cx.notify();
    }

    fn on_key_down(&mut self, ev: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let key = ev.keystroke.key.as_str();
        let ctrl = ev.keystroke.modifiers.control || ev.keystroke.modifiers.platform;

        if key == "escape" {
            if self.flyout.is_some() {
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
            cx.notify();
            return;
        }

        if self.search_focused {
            if apply_text_key(&mut self.search, ev, false, cx) {
                cx.notify();
            }
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
                            "Check Latest FNH Release",
                            None,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.flyout = None;
                            let _ = log::open_url("https://github.com/bufferclick/fnh/releases/latest");
                            cx.notify();
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
                            "Welcome to FNH 1.6"
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

        div()
            .id("main-instances-scroll")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .overflow_y_scroll()
            .bg(t.card)
            .p(px(14.))
            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                if this.flyout.is_some() || this.search_focused {
                    this.flyout = None;
                    this.search_focused = false;
                    cx.notify();
                }
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
                                    "chevron_right"
                                } else {
                                    "chevron_down"
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
                                        let item_drop = item.clone();
                                        let dbl_edits = prefs.double_click_edits;
                                        let show_pt = prefs.show_instance_playtime
                                            && item.played.as_secs() > 0;
                                        let pt_str =
                                            prefs.format_playtime(item.played.as_secs());

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
                                                gpui::rgba(0x0000_0000)
                                            })
                                            .bg(if is_sel {
                                                t.tile_selected
                                            } else {
                                                gpui::rgba(0x0000_0000)
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
                                            .on_click(cx.listener(
                                                move |this, ev: &ClickEvent, _, cx| {
                                                    this.flyout = None;
                                                    this.search_focused = false;
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
                                            .child(
                                                div()
                                                    .w_full()
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
                                                    .child(item.name.clone()),
                                            )
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
                                        let dbl_edits = prefs.double_click_edits;
                                        let summary = instance_summary(&item);
                                        let pt = prefs.format_playtime(item.played.as_secs());

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
                                                gpui::rgba(0x0000_0000)
                                            })
                                            .bg(if is_sel {
                                                t.tile_selected
                                            } else {
                                                gpui::rgba(0x0000_0000)
                                            })
                                            .hover(|s| {
                                                if is_sel {
                                                    s
                                                } else {
                                                    s.bg(t.subtle_hover)
                                                }
                                            })
                                            .on_click(cx.listener(
                                                move |this, ev: &ClickEvent, _, cx| {
                                                    this.flyout = None;
                                                    this.search_focused = false;
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
                                                                div()
                                                                    .text_size(px(13.))
                                                                    .font_weight(
                                                                        FontWeight::SEMIBOLD,
                                                                    )
                                                                    .text_color(t.ink)
                                                                    .truncate()
                                                                    .child(item.name.clone()),
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
            }))
    }

    fn render_wide_bar(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let sel = self.app.read(cx).selected_instance();
        let has = sel.is_some();
        let is_busy = job::busy();
        let can_launch = has && !is_busy;
        let can_kill = sel
            .as_ref()
            .is_some_and(|i| job::active_instance().as_deref() == Some(i.id.as_str()) && job::can_kill());

        let header = match &sel {
            Some(inst) => {
                let cfg = self.app.read(cx).cfgs.get(&inst.id);
                let icon_key = cfg
                    .icon_key
                    .unwrap_or_else(|| inst_icons::default_key_for(inst).to_owned());
                let id_icon = inst.id.clone();
                let key_open = icon_key.clone();
                div()
                    .p(px(8.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        div()
                            .id("wb-inst-icon")
                            .p(px(6.))
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
                            .child(inst_icons::render_icon(&icon_key, 56.)),
                    )
                    .child(
                        div()
                            .w_full()
                            .text_size(px(13.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.ink)
                            .truncate()
                            .flex()
                            .justify_center()
                            .child(inst.name.clone()),
                    )
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
                .p(px(12.))
                .text_size(px(12.))
                .text_color(t.ink_tertiary)
                .child("No instance selected"),
        };

        div()
            .id("widebar-scroll")
            .w(px(196.))
            .flex_shrink_0()
            .overflow_y_scroll()
            .bg(t.sidebar)
            .border_l_1()
            .border_color(t.divider)
            .p(px(8.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(header)
            .child(wide_bar_separator(t))
            .child({
                let s_l = sel.clone();
                wide_bar_action("wb-launch", "play", "Launch", can_launch, true, t).when_some(
                    s_l.filter(|_| can_launch),
                    |b, inst| {
                        b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let id = inst.id.clone();
                            this.app.update(cx, |a, cx| {
                                a.launch_instance(&id, LaunchExtras::default(), cx);
                            });
                        }))
                    },
                )
            })
            .child({
                let s_lo = sel.clone();
                wide_bar_action("wb-offline", "account", "Launch Offline", can_launch, false, t)
                    .when_some(s_lo.filter(|_| can_launch), |b, inst| {
                        b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
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
                    wide_bar_action("wb-kill", "stop", "Kill", true, false, t).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
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
                wide_bar_action("wb-log", "log", "Minecraft Log", has, false, t).when_some(
                    s_log,
                    |b, inst| {
                        b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let id = inst.id.clone();
                            this.app.update(cx, |a, cx| {
                                a.open_instance_window(&id, InstancePage::Log, cx);
                            });
                        }))
                    },
                )
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
                wide_bar_action("wb-worlds", "worlds", "Worlds", has, false, t).when_some(
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
                wide_bar_action("wb-group", "tag", "Change Group", has, false, t).when_some(
                    s_grp,
                    |b, inst| {
                        b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let mode = UtilityMode::Group {
                                id: inst.id.clone(),
                                value: inst.group.clone().unwrap_or_default(),
                            };
                            this.app.update(cx, |a, cx| a.open_utility_window(mode, cx));
                        }))
                    },
                )
            })
            .child({
                let s_ren = sel.clone();
                wide_bar_action("wb-rename", "rename", "Rename", has, false, t).when_some(
                    s_ren,
                    |b, inst| {
                        b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let mode = UtilityMode::Rename {
                                id: inst.id.clone(),
                                value: inst.name.clone(),
                            };
                            this.app.update(cx, |a, cx| a.open_utility_window(mode, cx));
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
                wide_bar_action("wb-copy", "copy", "Copy Instance", has, false, t).when_some(
                    s_cp,
                    |b, inst| {
                        b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            let mode = UtilityMode::Copy {
                                id: inst.id.clone(),
                                value: format!("{} (Copy)", inst.name),
                            };
                            this.app.update(cx, |a, cx| a.open_utility_window(mode, cx));
                        }))
                    },
                )
            })
            .child({
                let s_sc = sel.clone();
                wide_bar_action("wb-shortcut", "link", "Create Shortcut", has, false, t)
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
            })
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
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
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
            .h(px(28.))
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

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(Self::on_key_down))
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
    }
}
