//! Standalone secondary Windows 11 OS windows matching JLac's dialog suite:
//! - `NewInstanceWindow`  (`"New Instance - FNH"`)
//! - `IconPickerWindow`   (`"Select Icon - FNH"` — 61 JLac presets + any custom image)
//! - `ModBrowserWindow`   (`"Download Mods / Resource Packs / Shader Packs - FNH"`)
//! - `UtilityWindow`      (`Rename`, `Change Group`, `Copy`, `Delete`, `Create Shortcut`,
//!                         `Fabric Versions`, `Launcher Log`, and `About FNH` — made by Buffer)

use crate::app::{Fh, IconTarget};
use crate::content::{self, ShortcutTarget};
use crate::icons::icon;
use crate::inst_icons::{self, IconCategory, IconEntry};
use crate::look::{
    accent_button, apply_text_key, apply_window_icon, brand_mark, checkbox_row, chip,
    danger_button, group_box, log_line_color, page_nav_row, push_button, side_action_button,
    status_banner, sub_tab, text_input, Theme, MONO_FONT, UI_FONT,
};
use crate::prefs::HorizonTrack;
use crate::win_instance::InstancePage;
use crate::{crash, hmods, job, log, paths, updater, VERSION};
use gpui::{
    div, prelude::*, px, size, App, Bounds, ClickEvent, ClipboardItem, Context, Entity,
    ExternalPaths, FocusHandle, FontWeight, InteractiveElement, KeyDownEvent, PathPromptOptions,
    Render, SharedString, StatefulInteractiveElement, TitlebarOptions, Window, WindowBounds,
    WindowHandle, WindowOptions,
};
use horizon_launcher::backend;
use horizon_launcher::browser::{details::Sort, Kind as BrowseKind, Project, SourceId};
use horizon_launcher::instances::{self as inst, Instance};
use horizon_launcher::loaders::{self, Kind as LoaderKind, Loader};
use horizon_launcher::minecraft_versions::{self, Version as McVersion};
use std::sync::{Arc, Mutex};

/// Standard native Windows 11 options for a secondary FNH window.
pub fn secondary_window_options(
    title: impl Into<SharedString>,
    w: f32,
    h: f32,
    min_w: f32,
    min_h: f32,
    cx: &App,
) -> WindowOptions {
    let bounds = Bounds::centered(None, size(px(w), px(h)), cx);
    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        window_min_size: Some(size(px(min_w), px(min_h))),
        titlebar: Some(TitlebarOptions {
            title: Some(title.into()),
            ..Default::default()
        }),
        app_id: Some("fnh".into()),
        ..Default::default()
    }
}

// ================================================================================================
// 1. Select Icon Window (JLac `IconPickerDialog`: 61 JLac presets + custom user images)
// ================================================================================================

pub struct IconPickerWindow {
    app: Entity<Fh>,
    target: IconTarget,
    focus: FocusHandle,
    category: IconCategory,
    search: String,
    search_focused: bool,
    selected_key: String,
    notice: Option<(String, bool)>,
}

impl IconPickerWindow {
    pub fn open(
        app: Entity<Fh>,
        target: IconTarget,
        initial_key: String,
        cx: &mut App,
    ) -> Option<WindowHandle<Self>> {
        let opts = secondary_window_options("Select Icon - FNH", 720., 540., 540., 400., cx);
        let res = cx
            .open_window(opts, move |window, cx| {
                cx.new(|cx| {
                    cx.observe(&app, |_: &mut Self, _, cx| cx.notify()).detach();
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Self {
                        app,
                        target,
                        focus,
                        category: IconCategory::Any,
                        search: String::new(),
                        search_focused: false,
                        selected_key: if initial_key.is_empty() {
                            "default".into()
                        } else {
                            initial_key
                        },
                        notice: None,
                    }
                })
            })
            .ok();
        apply_window_icon();
        res
    }

    fn prompt_add_icons(&mut self, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select Icon Image".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.import_paths(&paths, cx);
            });
        })
        .detach();
    }

    fn import_paths(&mut self, files: &[std::path::PathBuf], cx: &mut Context<Self>) {
        let mut last_key = None;
        let mut count = 0usize;
        let mut first_err = None;
        for path in files {
            match inst_icons::import_custom_icon(path) {
                Ok(key) => {
                    last_key = Some(key);
                    count += 1;
                }
                Err(e) => {
                    if first_err.is_none() {
                        first_err = Some(e);
                    }
                }
            }
        }
        if let Some(key) = last_key {
            self.selected_key = key;
            self.notice = Some((
                format!(
                    "Imported {count} custom icon{}. Click OK to apply.",
                    if count == 1 { "" } else { "s" }
                ),
                false,
            ));
            self.app.update(cx, |_, cx| cx.notify());
        } else if let Some(err) = first_err {
            self.notice = Some((err, true));
        }
        cx.notify();
    }

    fn apply_and_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.selected_key.clone();
        let target = self.target.clone();
        self.app.update(cx, |app, cx| {
            app.apply_picked_icon(target, key, cx);
        });
        window.remove_window();
    }
}

impl Render for IconPickerWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.app.read(cx).prefs.clone();
        let t = Theme::resolve(prefs.theme_mode, prefs.accent, window.appearance());
        let entries: Vec<IconEntry> = inst_icons::list_icons(self.category, &self.search);
        let can_remove = inst_icons::is_custom_key(&self.selected_key);

        let mut tabs = div().flex().items_center().gap(px(4.));
        for cat in IconCategory::ALL {
            tabs = tabs.child(
                sub_tab(
                    format!("icon-cat-{}", cat.label()),
                    cat.label(),
                    self.category == cat,
                    &t,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.category = cat;
                    this.search_focused = false;
                    cx.notify();
                })),
            );
        }

        let top = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .px(px(14.))
            .pt(px(8.))
            .pb(px(6.))
            .bg(t.window)
            .border_b_1()
            .border_color(t.divider)
            .child(tabs)
            .child(
                div().w(px(210.)).child(
                    text_input(
                        "icon-search",
                        &self.search,
                        "Filter icons…",
                        self.search_focused,
                        &t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.search_focused = true;
                        cx.notify();
                    })),
                ),
            );

        let mut grid = div().flex().flex_wrap().gap(px(8.)).p(px(12.));
        for (idx, item) in entries.iter().enumerate() {
            let sel = item.key == self.selected_key;
            let key = item.key.clone();
            let hover = t.subtle_hover;
            grid = grid.child(
                div()
                    .id(SharedString::from(format!("icon-cell-{idx}")))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(5.))
                    .w(px(92.))
                    .px(px(6.))
                    .py(px(8.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(if sel {
                        t.tile_selected_border
                    } else {
                        gpui::transparent_black()
                    })
                    .when(sel, |d| d.bg(t.tile_selected))
                    .when(!sel, |d| d.hover(move |s| s.bg(hover)))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
                        this.selected_key = key.clone();
                        this.search_focused = false;
                        if matches!(ev, ClickEvent::Mouse(m) if m.up.click_count >= 2) {
                            this.apply_and_close(window, cx);
                        } else {
                            cx.notify();
                        }
                    }))
                    .child(inst_icons::render_icon(&item.key, 48.))
                    .child(
                        div()
                            .w_full()
                            .text_center()
                            .text_size(px(11.5))
                            .text_color(t.ink)
                            .child(item.label.clone()),
                    ),
            );
        }

        let body = div()
            .id("icon-grid-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(t.layer)
            .overflow_y_scroll()
            .drag_over::<ExternalPaths>(move |s, _, _, _| {
                s.border_2().border_color(t.accent).bg(t.tile_selected)
            })
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.import_paths(paths.paths(), cx);
            }))
            .when_some(self.notice.clone(), |d, (msg, err)| {
                d.child(div().px(px(12.)).pt(px(10.)).child(status_banner(&msg, err, &t)))
            })
            .when(entries.is_empty(), |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap(px(6.))
                        .py(px(48.))
                        .text_size(px(12.5))
                        .text_color(t.ink_secondary)
                        .child("No icons match this filter.")
                        .child("Click 'Add Icon…' or drop any PNG, JPG, WEBP, GIF, BMP or ICO image here."),
                )
            })
            .child(grid)
            .child(
                div()
                    .px(px(14.))
                    .pb(px(10.))
                    .text_size(px(11.5))
                    .text_color(t.ink_tertiary)
                    .child("Tip: Click 'Add Icon…' or drag-and-drop any image from Explorer into this window."),
            );

        let footer = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(14.))
            .py(px(10.))
            .bg(t.footer)
            .border_t_1()
            .border_color(t.divider)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .child(
                        push_button("icon-add", "Add Icon…", true, &t)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.prompt_add_icons(cx);
                            })),
                    )
                    .child(
                        push_button("icon-remove", "Remove Icon", can_remove, &t).when(
                            can_remove,
                            |b| {
                                b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    match inst_icons::remove_custom_icon(&this.selected_key) {
                                        Ok(()) => {
                                            this.selected_key = "default".into();
                                            this.notice =
                                                Some(("Custom icon removed.".into(), false));
                                            this.app.update(cx, |_, cx| cx.notify());
                                        }
                                        Err(e) => this.notice = Some((e, true)),
                                    }
                                    cx.notify();
                                }))
                            },
                        ),
                    )
                    .child(
                        push_button("icon-folder", "Open Folder", true, &t).on_click(cx.listener(
                            |this, _: &ClickEvent, _, cx| {
                                let dir = paths::icons_dir();
                                if let Err(e) = log::open_folder(&dir) {
                                    this.notice = Some((e, true));
                                    cx.notify();
                                }
                            },
                        )),
                    )
                    .child(
                        push_button("icon-reload", "Refresh", true, &t).on_click(cx.listener(
                            |_, _: &ClickEvent, _, cx| {
                                inst_icons::clear_cache();
                                cx.notify();
                            },
                        )),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        accent_button("icon-ok", "OK", true, &t).on_click(cx.listener(
                            |this, _: &ClickEvent, window, cx| {
                                this.apply_and_close(window, cx);
                            },
                        )),
                    )
                    .child(
                        push_button("icon-cancel", "Cancel", true, &t)
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            );

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let key = ev.keystroke.key.as_str();
                if key == "escape" {
                    window.remove_window();
                } else if key == "enter" && !this.search_focused {
                    this.apply_and_close(window, cx);
                } else if this.search_focused && apply_text_key(&mut this.search, ev, false, cx) {
                    cx.notify();
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(t.window)
            .font_family(UI_FONT)
            .text_color(t.ink)
            .child(top)
            .child(body)
            .child(footer)
    }
}

// ================================================================================================
// 2. New Instance Window (JLac `NewInstanceDialog` — separate native Windows 11 window)
// ================================================================================================

#[derive(Clone, Copy, PartialEq, Eq)]
enum NewKind {
    Fabric,
    Horizon,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum NewFocus {
    None,
    Name,
    Group,
    Search,
}

pub struct NewInstanceWindow {
    app: Entity<Fh>,
    focus: FocusHandle,
    field: NewFocus,
    kind: NewKind,
    icon_key: String,
    icon_customized: bool,
    name: String,
    name_customized: bool,
    group: String,
    search: String,
    show_releases: bool,
    show_snapshots: bool,
    show_historical: bool,
    versions: Arc<Mutex<Option<Result<Vec<McVersion>, String>>>>,
    picked_mc: String,
    want_fabric: bool,
    fabric_versions: Arc<Mutex<Option<(String, Result<Vec<String>, String>)>>>,
    picked_fabric: Option<String>,
    horizon_track: HorizonTrack,
    picked_horizon_tag: String,
    copy_template: bool,
    download_now: bool,
    error: Option<String>,
}

impl NewInstanceWindow {
    pub fn open(app: Entity<Fh>, cx: &mut App) -> Option<WindowHandle<Self>> {
        let opts = secondary_window_options("New Instance - FNH", 840., 600., 680., 480., cx);
        cx.open_window(opts, move |window, cx| {
            cx.new(|cx| {
                cx.observe(&app, |_: &mut Self, _, cx| cx.notify()).detach();
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                let prefs = app.read(cx).prefs.clone();
                let versions: Arc<Mutex<Option<Result<Vec<McVersion>, String>>>> =
                    Arc::new(Mutex::new(None));
                let v_slot = versions.clone();
                std::thread::spawn(move || {
                    let res = minecraft_versions::load().map(|l| l.all);
                    if let Ok(mut s) = v_slot.lock() {
                        *s = Some(res);
                    }
                });
                let fabric_versions = Arc::new(Mutex::new(None));

                cx.spawn(async move |this, cx| loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(200))
                        .await;
                    let alive = this
                        .update(cx, |this, cx| {
                            this.sync_loaded_versions();
                            cx.notify();
                        })
                        .is_ok();
                    if !alive {
                        break;
                    }
                })
                .detach();

                Self {
                    app,
                    focus,
                    field: NewFocus::Name,
                    kind: NewKind::Fabric,
                    icon_key: "fabricmc".into(),
                    icon_customized: false,
                    name: String::new(),
                    name_customized: false,
                    group: String::new(),
                    search: String::new(),
                    show_releases: true,
                    show_snapshots: prefs.show_snapshots_default,
                    show_historical: prefs.show_old_versions_default,
                    versions,
                    picked_mc: String::new(),
                    want_fabric: true,
                    fabric_versions,
                    picked_fabric: None,
                    horizon_track: if prefs.horizon_prefer_nightly {
                        HorizonTrack::Nightly
                    } else {
                        HorizonTrack::Release
                    },
                    picked_horizon_tag: backend::installed().into_iter().next().unwrap_or_default(),
                    copy_template: prefs.copy_template_on_create,
                    download_now: prefs.download_on_create,
                    error: None,
                }
            })
        })
        .ok()
        .inspect(|_| apply_window_icon())
    }

    pub fn set_icon(&mut self, key: String, cx: &mut Context<Self>) {
        self.icon_key = key;
        self.icon_customized = true;
        cx.notify();
    }

    fn sync_loaded_versions(&mut self) {
        if self.picked_mc.is_empty() {
            let found = self.versions.lock().ok().and_then(|slot| {
                slot.as_ref()
                    .and_then(|r| r.as_ref().ok())
                    .and_then(|list| {
                        list.iter()
                            .find(|v| v.kind == minecraft_versions::Kind::Release)
                            .or(list.first())
                            .map(|v| v.id.clone())
                    })
            });
            if let Some(id) = found {
                self.select_mc(id);
            }
        }
    }

    fn select_mc(&mut self, mc: String) {
        self.picked_mc = mc.clone();
        self.picked_fabric = None;
        if !self.name_customized && self.kind == NewKind::Fabric {
            self.name = mc.clone();
        }
        let slot = self.fabric_versions.clone();
        std::thread::spawn(move || {
            let res = loaders::versions(LoaderKind::Fabric, &mc, &backend::get_bytes)
                .map(|list| list.into_iter().map(|v| v.version).collect::<Vec<_>>());
            if let Ok(mut s) = slot.lock() {
                *s = Some((mc, res));
            }
        });
    }

    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        let fallback_name = match self.kind {
            NewKind::Fabric => {
                if self.picked_mc.is_empty() {
                    "Fabric".to_owned()
                } else {
                    self.picked_mc.clone()
                }
            }
            NewKind::Horizon => "Horizon".to_owned(),
        };
        let raw_name = if self.name.trim().is_empty() {
            fallback_name
        } else {
            self.name.trim().to_owned()
        };

        let created = match self.kind {
            NewKind::Fabric => {
                if self.picked_mc.trim().is_empty() {
                    self.error = Some("Pick a Minecraft version first.".into());
                    cx.notify();
                    return;
                }
                let mc = self.picked_mc.clone();
                match content::create_java_instance(&raw_name, &mc) {
                    Ok(inst) => {
                        if self.want_fabric {
                            let loader_ver = self.picked_fabric.clone().or_else(|| {
                                self.fabric_versions
                                    .lock()
                                    .ok()
                                    .and_then(|s| s.as_ref().and_then(|(m, r)| {
                                        (m == &mc).then(|| r.as_ref().ok()).flatten().and_then(|v| v.first().cloned())
                                    }))
                            });
                            let resolved = match loader_ver {
                                Some(v) => Ok(Loader {
                                    kind: LoaderKind::Fabric,
                                    version: v,
                                }),
                                None => loaders::choose("fabric", &mc, None),
                            };
                            match resolved {
                                Ok(loader) => {
                                    let _ = inst::edit(&inst.id, |meta| {
                                        if let Some(j) = meta.java.as_mut() {
                                            j.loader = Some(loader);
                                        }
                                    });
                                }
                                Err(e) => {
                                    log::line(&format!("Fabric version fetch deferred: {e}"));
                                }
                            }
                        }
                        inst::find(&inst.id)
                    }
                    Err(e) => Err(e),
                }
            }
            NewKind::Horizon => match content::create_horizon_instance(&raw_name) {
                Ok(inst) => {
                    if self.horizon_track == HorizonTrack::Pinned
                        && !self.picked_horizon_tag.trim().is_empty()
                    {
                        let pin = self.picked_horizon_tag.trim().to_owned();
                        let _ = inst::edit(&inst.id, |m| m.version = Some(pin));
                    }
                    inst::find(&inst.id)
                }
                Err(e) => Err(e),
            },
        };

        match created {
            Ok(instance) => {
                let grp = self.group.trim().to_owned();
                if !grp.is_empty() {
                    let _ = inst::edit(&instance.id, |m| m.group = Some(grp));
                }
                if self.copy_template {
                    paths::apply_template(&instance.home);
                }
                let icon_key = self.icon_key.clone();
                let hz_track = self.horizon_track;
                let hz_pin = self.picked_horizon_tag.clone();
                let is_hz = self.kind == NewKind::Horizon;
                let download_now = self.download_now;
                let id = instance.id.clone();

                self.app.update(cx, |app, cx| {
                    app.cfgs.update(&id, |cfg| {
                        cfg.icon_key = Some(icon_key);
                        if is_hz {
                            cfg.horizon_track = hz_track;
                            if hz_track == HorizonTrack::Pinned {
                                cfg.horizon_pinned_version = hz_pin;
                            }
                        }
                    });
                    app.reload_instances();
                    app.select_instance(&id);
                    app.notice = Some(format!("Created instance '{}'.", instance.name));
                    if download_now {
                        if let Some(fresh) = app.instances.iter().find(|i| i.id == id).cloned() {
                            let cfg = app.cfgs.get(&id);
                            job::prefetch_instance(fresh, app.prefs.clone(), cfg);
                        }
                    }
                    cx.notify();
                });
                window.remove_window();
            }
            Err(e) => {
                self.error = Some(e);
                cx.notify();
            }
        }
    }
}

impl Render for NewInstanceWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.app.read(cx).prefs.clone();
        let groups = self.app.read(cx).existing_groups();
        let t = Theme::resolve(prefs.theme_mode, prefs.accent, window.appearance());

        // Top JLac identity header: 52x52 clickable Icon button + Name + Group.
        let icon_hover = t.control_hover;
        let top = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(14.))
            .px(px(16.))
            .py(px(12.))
            .bg(t.window)
            .border_b_1()
            .border_color(t.divider)
            .child(
                div()
                    .id("new-inst-icon-btn")
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .size(px(64.))
                    .rounded(px(6.))
                    .bg(t.control)
                    .border_1()
                    .border_color(t.control_border)
                    .cursor_pointer()
                    .hover(move |s| s.bg(icon_hover))
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        let app = this.app.clone();
                        let cur = this.icon_key.clone();
                        let me = cx.entity().downgrade();
                        IconPickerWindow::open(app, IconTarget::NewInstance(me), cur, cx);
                    }))
                    .child(inst_icons::render_icon(&self.icon_key, 48.)),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .w(px(56.))
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("Name:"),
                            )
                            .child(
                                div().flex_1().child(
                                    text_input(
                                        "new-inst-name",
                                        &self.name,
                                        match self.kind {
                                            NewKind::Fabric => "e.g. 1.21.11",
                                            NewKind::Horizon => "e.g. Horizon",
                                        },
                                        self.field == NewFocus::Name,
                                        &t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.field = NewFocus::Name;
                                        cx.notify();
                                    })),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(10.))
                            .child(
                                div()
                                    .w(px(56.))
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("Group:"),
                            )
                            .child(
                                div().flex_1().child(
                                    text_input(
                                        "new-inst-group",
                                        &self.group,
                                        "No group (optional)",
                                        self.field == NewFocus::Group,
                                        &t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.field = NewFocus::Group;
                                        cx.notify();
                                    })),
                                ),
                            )
                            .children(groups.into_iter().take(3).enumerate().map(|(i, g)| {
                                let active = self.group == g;
                                let g_click = g.clone();
                                chip(format!("new-grp-chip-{i}"), g, active, &t).on_click(
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.group = if this.group == g_click {
                                            String::new()
                                        } else {
                                            g_click.clone()
                                        };
                                        cx.notify();
                                    }),
                                )
                            })),
                    ),
            );

        // Left sidebar matching JLac `PageContainer`: Fabric/Java first, Horizon second.
        let sidebar = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(175.))
            .gap(px(2.))
            .p(px(8.))
            .bg(t.sidebar)
            .border_r_1()
            .border_color(t.divider)
            .child(
                page_nav_row(
                    "new-nav-fabric",
                    "fabric",
                    "Custom (Fabric)",
                    self.kind == NewKind::Fabric,
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.kind = NewKind::Fabric;
                    if !this.icon_customized {
                        this.icon_key = if this.want_fabric {
                            "fabricmc".into()
                        } else {
                            "grass".into()
                        };
                    }
                    if !this.name_customized {
                        this.name = this.picked_mc.clone();
                    }
                    cx.notify();
                })),
            )
            .child(
                page_nav_row(
                    "new-nav-horizon",
                    "horizon",
                    "Horizon",
                    self.kind == NewKind::Horizon,
                    &t,
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                    this.kind = NewKind::Horizon;
                    if !this.icon_customized {
                        this.icon_key = "horizon".into();
                    }
                    if !this.name_customized {
                        this.name = "Horizon".into();
                    }
                    cx.notify();
                })),
            );

        let content_pane = match self.kind {
            NewKind::Fabric => self.render_fabric_pane(&t, cx),
            NewKind::Horizon => self.render_horizon_pane(&t, cx),
        };

        let footer = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .px(px(16.))
            .py(px(10.))
            .bg(t.footer)
            .border_t_1()
            .border_color(t.divider)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        checkbox_row(
                            "new-copy-tpl",
                            "Copy template",
                            None,
                            self.copy_template,
                            true,
                            &t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.copy_template = !this.copy_template;
                            cx.notify();
                        })),
                    )
                    .child(
                        checkbox_row(
                            "new-dl-now",
                            "Download files now",
                            None,
                            self.download_now,
                            true,
                            &t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.download_now = !this.download_now;
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        accent_button("new-ok", "OK", true, &t).on_click(cx.listener(
                            |this, _: &ClickEvent, window, cx| {
                                this.create(window, cx);
                            },
                        )),
                    )
                    .child(
                        push_button("new-cancel", "Cancel", true, &t)
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            );

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let key = ev.keystroke.key.as_str();
                if key == "escape" {
                    window.remove_window();
                    return;
                }
                if key == "enter" {
                    this.create(window, cx);
                    return;
                }
                let changed = match this.field {
                    NewFocus::Name => {
                        let ok = apply_text_key(&mut this.name, ev, false, cx);
                        if ok {
                            this.name_customized = !this.name.trim().is_empty();
                        }
                        ok
                    }
                    NewFocus::Group => apply_text_key(&mut this.group, ev, false, cx),
                    NewFocus::Search => apply_text_key(&mut this.search, ev, false, cx),
                    NewFocus::None => false,
                };
                if changed {
                    cx.notify();
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(t.window)
            .font_family(UI_FONT)
            .text_color(t.ink)
            .child(top)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(sidebar)
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .bg(t.layer)
                            .when_some(self.error.clone(), |d, e| {
                                d.child(
                                    div()
                                        .px(px(14.))
                                        .pt(px(10.))
                                        .child(status_banner(&e, true, &t)),
                                )
                            })
                            .child(content_pane),
                    ),
            )
            .child(footer)
    }
}

impl NewInstanceWindow {
    fn render_fabric_pane(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let needle = self.search.trim().to_ascii_lowercase();
        let loaded = self.versions.lock().ok().and_then(|s| s.clone());

        let mut list_el = div()
            .id("new-mc-list")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .rounded(px(4.))
            .border_1()
            .border_color(t.control_border)
            .bg(t.input_bg)
            .overflow_y_scroll();

        match loaded {
            None => {
                list_el = list_el.child(
                    div()
                        .p(px(16.))
                        .text_size(px(12.5))
                        .text_color(t.ink_secondary)
                        .child("Loading Minecraft version manifest…"),
                );
            }
            Some(Err(e)) => {
                list_el = list_el.child(
                    div()
                        .p(px(16.))
                        .text_size(px(12.5))
                        .text_color(t.danger)
                        .child(format!("Could not load versions: {e}")),
                );
            }
            Some(Ok(versions)) => {
                let filtered: Vec<&McVersion> = versions
                    .iter()
                    .filter(|v| match v.kind {
                        minecraft_versions::Kind::Release => self.show_releases,
                        minecraft_versions::Kind::Snapshot => self.show_snapshots,
                        minecraft_versions::Kind::OldBeta
                        | minecraft_versions::Kind::OldAlpha => self.show_historical,
                    })
                    .filter(|v| needle.is_empty() || v.id.to_ascii_lowercase().contains(&needle))
                    .take(160)
                    .collect();
                for (i, ver) in filtered.into_iter().enumerate() {
                    let sel = ver.id == self.picked_mc;
                    let vid = ver.id.clone();
                    let hover = t.subtle_hover;
                    list_el = list_el.child(
                        div()
                            .id(SharedString::from(format!("mc-ver-{i}")))
                            .flex()
                            .items_center()
                            .justify_between()
                            .h(px(26.))
                            .px(px(10.))
                            .text_size(px(12.5))
                            .bg(if sel {
                                t.tile_selected
                            } else {
                                gpui::transparent_black()
                            })
                            .when(!sel, |d| d.hover(move |s| s.bg(hover)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.select_mc(vid.clone());
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .font_weight(if sel {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .child(ver.id.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(t.ink_tertiary)
                                    .child(match ver.kind {
                                        minecraft_versions::Kind::Release => "Release",
                                        minecraft_versions::Kind::Snapshot => "Snapshot",
                                        minecraft_versions::Kind::OldBeta => "Beta",
                                        minecraft_versions::Kind::OldAlpha => "Alpha",
                                    }),
                            ),
                    );
                }
            }
        }

        // Right filter column (Releases, Snapshots, Historical) + Mod Loader section (Fabric / None).
        let fab_list: Vec<String> = self
            .fabric_versions
            .lock()
            .ok()
            .and_then(|s| {
                s.as_ref().and_then(|(m, r)| {
                    (m == &self.picked_mc)
                        .then(|| r.as_ref().ok().cloned())
                        .flatten()
                })
            })
            .unwrap_or_default();

        let loader_box = div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .pt(px(8.))
            .border_t_1()
            .border_color(t.divider)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Mod Loader"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(chip("ldr-none", "None (Vanilla)", !self.want_fabric, t).on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.want_fabric = false;
                                    if !this.icon_customized {
                                        this.icon_key = "grass".into();
                                    }
                                    cx.notify();
                                }),
                            ))
                            .child(chip("ldr-fabric", "Fabric", self.want_fabric, t).on_click(
                                cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.want_fabric = true;
                                    if !this.icon_customized {
                                        this.icon_key = "fabricmc".into();
                                    }
                                    cx.notify();
                                }),
                            )),
                    ),
            )
            .when(self.want_fabric, |d| {
                d.child(
                    div()
                        .id("fab-ver-chips")
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap(px(5.))
                        .child(
                            div()
                                .text_size(px(11.5))
                                .text_color(t.ink_secondary)
                                .child("Loader version:"),
                        )
                        .children(fab_list.iter().take(6).enumerate().map(|(i, fv)| {
                            let active = match &self.picked_fabric {
                                Some(p) => p == fv,
                                None => i == 0,
                            };
                            let label = if i == 0 {
                                format!("{fv} ★")
                            } else {
                                fv.clone()
                            };
                            let v_copy = fv.clone();
                            chip(format!("new-fv-{i}"), label, active, t).on_click(cx.listener(
                                move |this, _: &ClickEvent, _, cx| {
                                    this.picked_fabric = Some(v_copy.clone());
                                    cx.notify();
                                },
                            ))
                        }))
                        .when(fab_list.is_empty(), |row| {
                            row.child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(t.ink_tertiary)
                                    .child("Recommended latest Fabric Loader (selected automatically)"),
                            )
                        }),
                )
            });

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(8.))
            .p(px(14.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                checkbox_row("flt-rel", "Releases", None, self.show_releases, true, t)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.show_releases = !this.show_releases;
                                        cx.notify();
                                    })),
                            )
                            .child(
                                checkbox_row(
                                    "flt-snap",
                                    "Snapshots",
                                    None,
                                    self.show_snapshots,
                                    true,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.show_snapshots = !this.show_snapshots;
                                    cx.notify();
                                })),
                            )
                            .child(
                                checkbox_row(
                                    "flt-old",
                                    "Betas / Alphas",
                                    None,
                                    self.show_historical,
                                    true,
                                    t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    this.show_historical = !this.show_historical;
                                    cx.notify();
                                })),
                            ),
                    )
                    .child(
                        div().w(px(180.)).child(
                            text_input(
                                "new-mc-search",
                                &self.search,
                                "Search version…",
                                self.field == NewFocus::Search,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.field = NewFocus::Search;
                                cx.notify();
                            })),
                        ),
                    ),
            )
            .child(list_el)
            .child(loader_box)
    }

    fn render_horizon_pane(&self, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let installed = backend::installed();
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("new-horizon-scroll")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h_0()
                    .gap(px(12.))
                    .p(px(14.))
                    .overflow_y_scroll()
                    .child(
                        group_box(
                            "Horizon Instance (Isolated)",
                            Some("Gets its own dedicated saves, horizon-mods, resourcepacks, servers.dat, screenshots and options.txt — never mixed with any other instance or launcher."),
                            t,
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .gap(px(6.))
                                .children(
                                    [
                                        (HorizonTrack::Nightly, "Nightly (latest)"),
                                        (HorizonTrack::Release, "Stable Release"),
                                        (HorizonTrack::Default, "Follow Global Setting"),
                                        (HorizonTrack::Pinned, "Pin Installed Version"),
                                    ]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, (tr, lbl))| {
                                        chip(format!("new-hz-tr-{i}"), lbl, self.horizon_track == tr, t)
                                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                this.horizon_track = tr;
                                                cx.notify();
                                            }))
                                    }),
                                ),
                        ),
                    )
                    .when(self.horizon_track == HorizonTrack::Pinned, |d| {
                        d.child(
                            group_box(
                                "Installed Horizon Builds",
                                Some("Select a local Horizon build to pin to this instance."),
                                t,
                            )
                            .when(installed.is_empty(), |g| {
                                g.child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(t.ink_secondary)
                                        .child("No Horizon builds installed yet — pick Nightly or Stable Release and FNH will download and prepare it automatically on first launch."),
                                )
                            })
                            .children(installed.into_iter().enumerate().map(|(i, tag)| {
                                let active = self.picked_horizon_tag == tag;
                                let tag_c = tag.clone();
                                chip(format!("new-hz-pin-{i}"), tag, active, t).on_click(cx.listener(
                                    move |this, _: &ClickEvent, _, cx| {
                                        this.picked_horizon_tag = tag_c.clone();
                                        cx.notify();
                                    },
                                ))
                            })),
                        )
                    })
                    .child(
                        group_box(
                            "Mods & Customization",
                            Some("After creating the instance, open Edit… -> Mods to browse the Horizon Mod Registry, add .hmod / .hnmod files, or drag-and-drop mods straight in. Configure wgpu graphics, creative launch, privacy, game options and hooks under Edit… -> Settings -> Horizon."),
                            t,
                        ),
                    ),
            )
    }
}

// ================================================================================================
// 3. Mod / Pack / Shader Browser Window (Modrinth + Horizon Registry, separate OS window)
// ================================================================================================

pub struct ModBrowserWindow {
    app: Entity<Fh>,
    instance_id: String,
    kind: BrowseKind,
    source: SourceId,
    github_mode: bool,
    sort: Sort,
    focus: FocusHandle,
    query: String,
    search_focused: bool,
    results: Arc<Mutex<Option<Result<Vec<Project>, String>>>>,
    selected: usize,
    installing: Arc<Mutex<Option<String>>>,
    notice: Arc<Mutex<Option<(String, bool)>>>,
}

impl ModBrowserWindow {
    pub fn open(
        app: Entity<Fh>,
        instance: Instance,
        kind: BrowseKind,
        cx: &mut App,
    ) -> Option<WindowHandle<Self>> {
        let title = format!("Download {} ({}) - FNH", kind.label(), instance.name);
        let opts = secondary_window_options(title, 880., 620., 680., 460., cx);
        let default_source = if instance.java.is_some() {
            SourceId::Modrinth
        } else {
            SourceId::Horizon
        };
        let inst_id = instance.id.clone();

        let res = cx
            .open_window(opts, move |window, cx| {
                cx.new(|cx| {
                    cx.observe(&app, |_: &mut Self, _, cx| cx.notify()).detach();
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    let mut win = Self {
                        app,
                        instance_id: inst_id,
                        kind,
                        source: default_source,
                        github_mode: false,
                        sort: Sort::Downloads,
                        focus,
                        query: String::new(),
                        search_focused: true,
                        results: Arc::new(Mutex::new(None)),
                        selected: 0,
                        installing: Arc::new(Mutex::new(None)),
                        notice: Arc::new(Mutex::new(None)),
                    };
                    win.run_search(cx);
                    cx.spawn(async move |this, cx| loop {
                        cx.background_executor()
                            .timer(std::time::Duration::from_millis(220))
                            .await;
                        if this.update(cx, |_, cx| cx.notify()).is_err() {
                            break;
                        }
                    })
                    .detach();
                    win
                })
            })
            .ok();
        apply_window_icon();
        res
    }

    fn current_instance(&self, cx: &App) -> Option<Instance> {
        self.app
            .read(cx)
            .instances
            .iter()
            .find(|i| i.id == self.instance_id)
            .cloned()
    }

    fn run_search(&mut self, cx: &mut Context<Self>) {
        let Some(inst) = self.current_instance(cx) else {
            return;
        };
        let src = self.source;
        let kind = self.kind;
        let q = self.query.clone();
        let sort = self.sort;
        let target = inst.java.as_ref().map(|j| {
            (
                j.minecraft.clone(),
                j.loader.as_ref().map(|l| l.kind.id().to_owned()),
            )
        });
        self.selected = 0;
        if let Ok(mut r) = self.results.lock() {
            *r = None;
        }
        let slot = self.results.clone();
        std::thread::spawn(move || {
            let tref = target.as_ref().map(|(m, l)| (m.as_str(), l.as_deref()));
            let res = content::search_projects(src, kind, &q, sort, tref);
            if let Ok(mut s) = slot.lock() {
                *s = Some(res);
            }
        });
        cx.notify();
    }

    fn install_selected(&mut self, project: Project, cx: &mut Context<Self>) {
        if self.installing.lock().ok().is_some_and(|s| s.is_some()) {
            return;
        }
        let Some(inst) = self.current_instance(cx) else {
            return;
        };
        if let Ok(mut b) = self.installing.lock() {
            *b = Some(format!("Installing {}…", project.title));
        }
        let busy = self.installing.clone();
        let note = self.notice.clone();
        let app = self.app.clone();
        std::thread::spawn(move || {
            let busy_p = busy.clone();
            let res = content::install_project(&inst, &project, &|msg, _| {
                if let Ok(mut b) = busy_p.lock() {
                    *b = Some(msg.to_owned());
                }
            });
            if let Ok(mut b) = busy.lock() {
                *b = None;
            }
            if let Ok(mut n) = note.lock() {
                *n = Some(match res {
                    Ok(ok) => (ok, false),
                    Err(e) => (e, true),
                });
            }
        });
        app.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn install_github(&mut self, cx: &mut Context<Self>) {
        if self.installing.lock().ok().is_some_and(|s| s.is_some()) {
            return;
        }
        let Some(inst) = self.current_instance(cx) else {
            return;
        };
        let spec = self.query.trim().to_owned();
        if spec.is_empty() {
            if let Ok(mut n) = self.notice.lock() {
                *n = Some((
                    "Enter a GitHub repository (owner/repo or https://github.com/owner/repo)."
                        .into(),
                    true,
                ));
            }
            cx.notify();
            return;
        }
        if let Ok(mut b) = self.installing.lock() {
            *b = Some(format!("Downloading Horizon mod from {spec}…"));
        }
        let busy = self.installing.clone();
        let note = self.notice.clone();
        let home = inst.home.clone();
        let app = self.app.clone();
        std::thread::spawn(move || {
            let res = hmods::install_from_github(&home, &spec);
            if let Ok(mut b) = busy.lock() {
                *b = None;
            }
            if let Ok(mut n) = note.lock() {
                *n = Some(match res {
                    Ok(info) => (
                        format!(
                            "Installed {} v{} ({}) from GitHub!",
                            info.name,
                            info.version,
                            info.runtime.badge()
                        ),
                        false,
                    ),
                    Err(e) => (e, true),
                });
            }
        });
        app.update(cx, |_, cx| cx.notify());
        cx.notify();
    }
}

impl Render for ModBrowserWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.app.read(cx).prefs.clone();
        let t = Theme::resolve(prefs.theme_mode, prefs.accent, window.appearance());
        let is_java = self
            .current_instance(cx)
            .is_some_and(|i| i.java.is_some());

        let busy_text = self.installing.lock().ok().and_then(|b| b.clone());
        let note = self.notice.lock().ok().and_then(|n| n.clone());
        let loaded = self.results.lock().ok().and_then(|r| r.clone());
        let gh_mode = self.github_mode && !is_java;

        let top = div()
            .flex()
            .flex_col()
            .flex_none()
            .gap(px(8.))
            .px(px(14.))
            .py(px(10.))
            .bg(t.window)
            .border_b_1()
            .border_color(t.divider)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div().flex_1().child(
                            text_input(
                                "mb-query",
                                &self.query,
                                if gh_mode {
                                    "owner/repo or https://github.com/owner/repo (or direct .hmod URL)…"
                                } else {
                                    "Search by name or keyword, then press Enter…"
                                },
                                self.search_focused,
                                &t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.search_focused = true;
                                cx.notify();
                            })),
                        ),
                    )
                    .child(
                        accent_button(
                            "mb-search-btn",
                            if gh_mode {
                                "Install from GitHub"
                            } else {
                                "Search"
                            },
                            busy_text.is_none(),
                            &t,
                        )
                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                            if gh_mode {
                                this.install_github(cx);
                            } else {
                                this.run_search(cx);
                            }
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .when(!is_java, |d| {
                                d.child(
                                    chip(
                                        "mb-src-hz",
                                        "Horizon Registry",
                                        !gh_mode && self.source == SourceId::Horizon,
                                        &t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.github_mode = false;
                                        this.source = SourceId::Horizon;
                                        this.run_search(cx);
                                    })),
                                )
                            })
                            .when(is_java || self.kind != BrowseKind::Shader, |d| {
                                d.child(
                                    chip(
                                        "mb-src-mr",
                                        if is_java {
                                            "Modrinth"
                                        } else {
                                            "Modrinth (Auto-Port)"
                                        },
                                        !gh_mode && self.source == SourceId::Modrinth,
                                        &t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.github_mode = false;
                                        this.source = SourceId::Modrinth;
                                        this.run_search(cx);
                                    })),
                                )
                            })
                            .when(!is_java && self.kind == BrowseKind::Mod, |d| {
                                d.child(
                                    chip("mb-src-gh", "GitHub (.hmod)", gh_mode, &t).on_click(
                                        cx.listener(|this, _: &ClickEvent, _, cx| {
                                            this.github_mode = true;
                                            this.query.clear();
                                            cx.notify();
                                        }),
                                    ),
                                )
                            }),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(5.))
                            .children(
                                [Sort::Relevance, Sort::Downloads, Sort::Updated, Sort::Newest]
                                    .into_iter()
                                    .enumerate()
                                    .map(|(i, s)| {
                                        chip(
                                            format!("mb-sort-{i}"),
                                            s.label(),
                                            self.sort == s,
                                            &t,
                                        )
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            this.sort = s;
                                            this.run_search(cx);
                                        }))
                                    }),
                            ),
                    ),
            );

        let mut list = div()
            .id("mb-results-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .bg(t.layer)
            .overflow_y_scroll();

        let mut picked_project: Option<Project> = None;

        if gh_mode {
            list = list.child(
                div()
                    .p(px(22.))
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(icon("github", 18., t.accent))
                            .child(
                                div()
                                    .text_size(px(14.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Install Horizon Mod Directly from GitHub"),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(t.ink_secondary)
                            .child("Paste a GitHub repository (`owner/repo`, `https://github.com/owner/repo`, a release tag URL, or a direct `.hmod` download link) above and click Install from GitHub."),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.ink_tertiary)
                            .child("• Content & WebAssembly (`.hmod`) mods install to `horizon-mods/` and load immediately.\n• Native SDK (`.hmod`) mods install to `horizon-native-mods/` and link automatically via Horizon's Native Mod SDK on Play."),
                    ),
            );
        } else {
        match loaded {
            None => {
                list = list.child(
                    div()
                        .p(px(20.))
                        .text_size(px(12.5))
                        .text_color(t.ink_secondary)
                        .child("Searching…"),
                );
            }
            Some(Err(e)) => {
                list = list.child(div().p(px(14.)).child(status_banner(&e, true, &t)));
            }
            Some(Ok(projects)) if projects.is_empty() => {
                list = list.child(
                    div()
                        .p(px(20.))
                        .text_size(px(12.5))
                        .text_color(t.ink_secondary)
                        .child("No compatible results found for this instance."),
                );
            }
            Some(Ok(projects)) => {
                picked_project = projects.get(self.selected).cloned();
                for (i, p) in projects.into_iter().enumerate() {
                    let sel = i == self.selected;
                    let hover = t.subtle_hover;
                    let p_install = p.clone();
                    list = list.child(
                        div()
                            .id(SharedString::from(format!("mb-row-{i}")))
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(10.))
                            .px(px(12.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(t.divider)
                            .bg(if sel {
                                t.tile_selected
                            } else {
                                gpui::transparent_black()
                            })
                            .when(!sel, |d| d.hover(move |s| s.bg(hover)))
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, ev: &ClickEvent, _, cx| {
                                this.selected = i;
                                if matches!(ev, ClickEvent::Mouse(m) if m.up.click_count >= 2) {
                                    this.install_selected(p_install.clone(), cx);
                                } else {
                                    cx.notify();
                                }
                            }))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .child(
                                                div()
                                                    .text_size(px(13.))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .child(p.title.clone()),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .text_color(t.ink_tertiary)
                                                    .child(format!("by {}", p.author)),
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(t.ink_secondary)
                                            .child(p.summary.clone()),
                                    ),
                            )
                            .child(
                                div()
                                    .flex_none()
                                    .text_size(px(11.5))
                                    .text_color(t.ink_tertiary)
                                    .child(format!("{} dl", content::format_count(p.downloads))),
                            ),
                    );
                }
            }
        }
        }

        let can_install = !gh_mode && busy_text.is_none() && picked_project.is_some();
        let proj_for_btn = picked_project.clone();
        let page_url = picked_project.as_ref().map(|p| p.page.clone());

        let footer = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(10.))
            .px(px(14.))
            .py(px(10.))
            .bg(t.footer)
            .border_t_1()
            .border_color(t.divider)
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.))
                    .text_color(if note.as_ref().is_some_and(|(_, e)| *e) {
                        t.danger
                    } else {
                        t.ink_secondary
                    })
                    .child(
                        busy_text
                            .or_else(|| note.map(|(m, _)| m))
                            .unwrap_or_else(|| "Select a project and click Install (required dependencies install automatically).".into()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when_some(page_url.filter(|u| !u.is_empty()), |d, url| {
                        d.child(
                            push_button("mb-open-page", "Project Page", true, &t).on_click(
                                move |_, _, _| {
                                    let _ = log::open_url(&url);
                                },
                            ),
                        )
                    })
                    .child(
                        accent_button("mb-install", "Install", can_install, &t).when_some(
                            proj_for_btn.filter(|_| can_install),
                            |b, p| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.install_selected(p.clone(), cx);
                                }))
                            },
                        ),
                    )
                    .child(
                        push_button("mb-close", "Close", true, &t).on_click(cx.listener(
                            |this, _: &ClickEvent, window, cx| {
                                this.app.update(cx, |_, cx| cx.notify());
                                window.remove_window();
                            },
                        )),
                    ),
            );

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let key = ev.keystroke.key.as_str();
                if key == "escape" {
                    window.remove_window();
                } else if key == "enter" {
                    if this.github_mode && !is_java {
                        this.install_github(cx);
                    } else {
                        this.run_search(cx);
                    }
                } else if this.search_focused && apply_text_key(&mut this.query, ev, false, cx) {
                    cx.notify();
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(t.window)
            .font_family(UI_FONT)
            .text_color(t.ink)
            .child(top)
            .child(list)
            .child(footer)
    }
}

// ================================================================================================
// 4. Utility Windows (Rename, Group, Copy, Delete, Shortcut, Fabric Versions, Launcher Log, About)
// ================================================================================================

#[derive(Clone)]
pub enum UtilityMode {
    Rename { id: String, value: String },
    Group { id: String, value: String },
    Copy { id: String, value: String },
    Delete { id: String, name: String },
    Shortcut {
        id: String,
        name: String,
        target: ShortcutTarget,
        offline: bool,
    },
    FabricVersions {
        mc: String,
        list: Arc<Mutex<Option<Result<Vec<String>, String>>>>,
    },
    LauncherLog,
    Updater,
    About { tab: usize },
}

pub struct UtilityWindow {
    app: Entity<Fh>,
    focus: FocusHandle,
    mode: UtilityMode,
    error: Option<String>,
}

impl UtilityWindow {
    pub fn open(app: Entity<Fh>, mode: UtilityMode, cx: &mut App) -> Option<WindowHandle<Self>> {
        let (title, w, h) = match &mode {
            UtilityMode::Rename { .. } => ("Rename Instance - FNH", 430., 210.),
            UtilityMode::Group { .. } => ("Change Group - FNH", 450., 260.),
            UtilityMode::Copy { .. } => ("Copy Instance - FNH", 450., 220.),
            UtilityMode::Delete { .. } => ("Confirm Deletion - FNH", 450., 210.),
            UtilityMode::Shortcut { .. } => ("Create Shortcut - FNH", 460., 290.),
            UtilityMode::FabricVersions { .. } => ("Fabric Loader Versions - FNH", 540., 460.),
            UtilityMode::LauncherLog => ("Launcher Log - FNH", 820., 540.),
            UtilityMode::Updater => ("Update FNH", 540., 390.),
            UtilityMode::About { .. } => ("About FNH", 560., 420.),
        };
        let opts = secondary_window_options(title, w, h, w.min(400.), h.min(200.), cx);
        let res = cx
            .open_window(opts, move |window, cx| {
                cx.new(|cx| {
                    cx.observe(&app, |_: &mut Self, _, cx| cx.notify()).detach();
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    if matches!(
                        mode,
                        UtilityMode::FabricVersions { .. }
                            | UtilityMode::LauncherLog
                            | UtilityMode::Updater
                    ) {
                        let poll_ms = if matches!(mode, UtilityMode::Updater) {
                            120
                        } else {
                            350
                        };
                        cx.spawn(async move |this, cx| loop {
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(poll_ms))
                                .await;
                            if this.update(cx, |_, cx| cx.notify()).is_err() {
                                break;
                            }
                        })
                        .detach();
                    }
                    Self {
                        app,
                        focus,
                        mode,
                        error: None,
                    }
                })
            })
            .ok();
        apply_window_icon();
        res
    }

    pub fn open_fabric_versions(app: Entity<Fh>, cx: &mut App) -> Option<WindowHandle<Self>> {
        let slot = Arc::new(Mutex::new(None));
        let slot_bg = slot.clone();
        let mc = "1.21.11".to_owned();
        let mc_bg = mc.clone();
        std::thread::spawn(move || {
            let res = loaders::versions(LoaderKind::Fabric, &mc_bg, &backend::get_bytes)
                .map(|v| v.into_iter().map(|x| x.version).collect());
            if let Ok(mut s) = slot_bg.lock() {
                *s = Some(res);
            }
        });
        Self::open(app, UtilityMode::FabricVersions { mc, list: slot }, cx)
    }

    fn submit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.error = None;
        match self.mode.clone() {
            UtilityMode::Rename { id, value } => {
                let name = value.trim().to_owned();
                if name.is_empty() {
                    self.error = Some("Name cannot be empty.".into());
                    cx.notify();
                    return;
                }
                match inst::edit(&id, |m| m.name = Some(name.clone())) {
                    Ok(_) => {
                        self.app.update(cx, |app, cx| {
                            app.reload_instances();
                            app.notice = Some(format!("Renamed to '{name}'."));
                            cx.notify();
                        });
                        window.remove_window();
                    }
                    Err(e) => {
                        self.error = Some(e);
                        cx.notify();
                    }
                }
            }
            UtilityMode::Group { id, value } => {
                let grp = value.trim().to_owned();
                match inst::edit(&id, |m| {
                    m.group = (!grp.is_empty()).then(|| grp.clone());
                }) {
                    Ok(_) => {
                        self.app.update(cx, |app, cx| {
                            app.reload_instances();
                            app.notice = Some(if grp.is_empty() {
                                "Cleared instance group.".into()
                            } else {
                                format!("Moved to group '{grp}'.")
                            });
                            cx.notify();
                        });
                        window.remove_window();
                    }
                    Err(e) => {
                        self.error = Some(e);
                        cx.notify();
                    }
                }
            }
            UtilityMode::Copy { id, value } => {
                let name = value.trim().to_owned();
                if name.is_empty() {
                    self.error = Some("Enter a name for the copied instance.".into());
                    cx.notify();
                    return;
                }
                match inst::create(&name, Some(&id)) {
                    Ok(copy) => {
                        self.app.update(cx, |app, cx| {
                            app.cfgs.clone_cfg(&id, &copy.id);
                            app.reload_instances();
                            app.select_instance(&copy.id);
                            app.notice = Some(format!("Copied to '{}'.", copy.name));
                            cx.notify();
                        });
                        window.remove_window();
                    }
                    Err(e) => {
                        self.error = Some(e);
                        cx.notify();
                    }
                }
            }
            UtilityMode::Delete { id, name } => match inst::delete(&id) {
                Ok(_) => {
                    self.app.update(cx, |app, cx| {
                        app.cfgs.remove(&id);
                        app.close_instance_window(&id, cx);
                        app.reload_instances();
                        app.notice = Some(format!("Deleted '{name}'."));
                        cx.notify();
                    });
                    window.remove_window();
                }
                Err(e) => {
                    self.error = Some(e);
                    cx.notify();
                }
            },
            UtilityMode::Shortcut {
                id,
                name,
                target,
                offline,
            } => {
                let Some(instance) = self
                    .app
                    .read(cx)
                    .instances
                    .iter()
                    .find(|i| i.id == id)
                    .cloned()
                else {
                    window.remove_window();
                    return;
                };
                match content::create_instance_shortcut(&instance, &name, target, offline) {
                    Ok(path) => {
                        self.app.update(cx, |app, cx| {
                            app.notice = Some(format!("Created shortcut: {}", path.display()));
                            cx.notify();
                        });
                        window.remove_window();
                    }
                    Err(e) => {
                        self.error = Some(e);
                        cx.notify();
                    }
                }
            }
            _ => window.remove_window(),
        }
    }
}

impl Render for UtilityWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.app.read(cx).prefs.clone();
        let groups = self.app.read(cx).existing_groups();
        let t = Theme::resolve(prefs.theme_mode, prefs.accent, window.appearance());

        let (body, ok_label, destructive): (gpui::Div, Option<&'static str>, bool) =
            match &self.mode {
                UtilityMode::Rename { value, .. } => (
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .p(px(16.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Enter a new instance name:"),
                        )
                        .child(text_input("util-rename", value, "Instance name", true, &t)),
                    Some("OK"),
                    false,
                ),
                UtilityMode::Group { value, .. } => (
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .p(px(16.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Instance group (leave blank for Ungrouped):"),
                        )
                        .child(text_input("util-group", value, "Ungrouped", true, &t))
                        .when(!groups.is_empty(), |d| {
                            d.child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(6.))
                                    .children(groups.into_iter().enumerate().map(|(i, g)| {
                                        let active = value == &g;
                                        let g_c = g.clone();
                                        chip(format!("ug-{i}"), g, active, &t).on_click(
                                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                                if let UtilityMode::Group { value, .. } =
                                                    &mut this.mode
                                                {
                                                    *value = g_c.clone();
                                                }
                                                cx.notify();
                                            }),
                                        )
                                    })),
                            )
                        }),
                    Some("OK"),
                    false,
                ),
                UtilityMode::Copy { value, .. } => (
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(10.))
                        .p(px(16.))
                        .child(
                            div()
                                .text_size(px(13.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child("Name for the duplicated instance:"),
                        )
                        .child(text_input("util-copy", value, "Copy name", true, &t)),
                    Some("Copy"),
                    false,
                ),
                UtilityMode::Delete { name, .. } => (
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .p(px(16.))
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(format!("Permanently delete '{name}'?")),
                        )
                        .child(
                            div()
                                .text_size(px(12.5))
                                .text_color(t.ink_secondary)
                                .child("This removes the instance's saves, mods, resource packs, screenshots and configuration. This cannot be undone."),
                        ),
                    Some("Delete"),
                    true,
                ),
                UtilityMode::Shortcut {
                    name,
                    target,
                    offline,
                    ..
                } => {
                    let cur_target = *target;
                    let cur_offline = *offline;
                    (
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .p(px(16.))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("Shortcut name:"),
                            )
                            .child(text_input("util-sc-name", name, "Shortcut name", true, &t))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .text_color(t.ink_secondary)
                                            .child("Target:"),
                                    )
                                    .child(
                                        chip(
                                            "sc-desk",
                                            "Desktop",
                                            cur_target == ShortcutTarget::Desktop,
                                            &t,
                                        )
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, _, cx| {
                                                if let UtilityMode::Shortcut { target, .. } =
                                                    &mut this.mode
                                                {
                                                    *target = ShortcutTarget::Desktop;
                                                }
                                                cx.notify();
                                            },
                                        )),
                                    )
                                    .child(
                                        chip(
                                            "sc-start",
                                            "Start Menu",
                                            cur_target == ShortcutTarget::StartMenu,
                                            &t,
                                        )
                                        .on_click(cx.listener(
                                            |this, _: &ClickEvent, _, cx| {
                                                if let UtilityMode::Shortcut { target, .. } =
                                                    &mut this.mode
                                                {
                                                    *target = ShortcutTarget::StartMenu;
                                                }
                                                cx.notify();
                                            },
                                        )),
                                    ),
                            )
                            .child(
                                checkbox_row(
                                    "sc-offline",
                                    "Launch instance in offline mode",
                                    None,
                                    cur_offline,
                                    true,
                                    &t,
                                )
                                .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                    if let UtilityMode::Shortcut { offline, .. } = &mut this.mode {
                                        *offline = !*offline;
                                    }
                                    cx.notify();
                                })),
                            ),
                        Some("Create"),
                        false,
                    )
                }
                UtilityMode::FabricVersions { mc, list } => {
                    let loaded = list.lock().ok().and_then(|s| s.clone());
                    let mc_copy = mc.clone();
                    let list_arc = list.clone();
                    let mut rows = div()
                        .id("fv-scroll")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_h_0()
                        .rounded(px(4.))
                        .border_1()
                        .border_color(t.control_border)
                        .bg(t.input_bg)
                        .overflow_y_scroll();
                    match loaded {
                        None => {
                            rows = rows.child(
                                div()
                                    .p(px(14.))
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("Loading Fabric Meta…"),
                            );
                        }
                        Some(Err(e)) => {
                            rows = rows.child(div().p(px(12.)).child(status_banner(&e, true, &t)));
                        }
                        Some(Ok(vs)) => {
                            for (i, v) in vs.into_iter().enumerate() {
                                rows = rows.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .justify_between()
                                        .px(px(12.))
                                        .py(px(5.))
                                        .border_b_1()
                                        .border_color(t.divider)
                                        .text_size(px(12.5))
                                        .child(v)
                                        .when(i == 0, |d| {
                                            d.child(
                                                div()
                                                    .text_size(px(11.5))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(t.accent)
                                                    .child("Latest Recommended"),
                                            )
                                        }),
                                );
                            }
                        }
                    }
                    (
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .gap(px(8.))
                            .p(px(14.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .text_color(t.ink_secondary)
                                            .child("Minecraft:"),
                                    )
                                    .child(
                                        div()
                                            .flex_1()
                                            .child(text_input("fv-mc", mc, "1.21.11", true, &t)),
                                    )
                                    .child(
                                        push_button("fv-fetch", "Refresh", true, &t).on_click(
                                            cx.listener(move |_, _: &ClickEvent, _, cx| {
                                                if let Ok(mut s) = list_arc.lock() {
                                                    *s = None;
                                                }
                                                let slot = list_arc.clone();
                                                let m = mc_copy.clone();
                                                std::thread::spawn(move || {
                                                    let res = loaders::versions(
                                                        LoaderKind::Fabric,
                                                        &m,
                                                        &backend::get_bytes,
                                                    )
                                                    .map(|v| {
                                                        v.into_iter()
                                                            .map(|x| x.version)
                                                            .collect()
                                                    });
                                                    if let Ok(mut s) = slot.lock() {
                                                        *s = Some(res);
                                                    }
                                                });
                                                cx.notify();
                                            }),
                                        ),
                                    ),
                            )
                            .child(rows),
                        None,
                        false,
                    )
                }
                UtilityMode::LauncherLog => {
                    let log_path = log::path().to_path_buf();
                    let lines = content::read_log_tail(&log_path, 400);
                    let full = lines.join("\n");
                    let p_open = log_path.clone();
                    let editor = prefs.text_editor_cmd.clone();
                    (
                        div()
                            .flex()
                            .flex_1()
                            .min_h_0()
                            .gap(px(10.))
                            .p(px(12.))
                            .child(
                                div()
                                    .id("launcher-log-scroll")
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .min_h_0()
                                    .p(px(10.))
                                    .rounded(px(4.))
                                    .border_1()
                                    .border_color(t.control_border)
                                    .bg(t.input_bg)
                                    .font_family(MONO_FONT)
                                    .text_size(px(prefs.console_font_size as f32))
                                    .overflow_y_scroll()
                                    .children(lines.into_iter().map(|line| {
                                        let c = log_line_color(
                                            &line,
                                            prefs.console_highlight_levels,
                                            &t,
                                        );
                                        div().text_color(c).child(line)
                                    })),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .w(px(136.))
                                    .gap(px(6.))
                                    .child(
                                        side_action_button("ll-copy", Some("copy"), "Copy", true, false, &t)
                                            .on_click(move |_, _, cx| {
                                                cx.write_to_clipboard(ClipboardItem::new_string(
                                                    full.clone(),
                                                ));
                                            }),
                                    )
                                    .child(
                                        side_action_button("ll-open", Some("log"), "Open File", true, false, &t)
                                            .on_click(move |_, _, _| {
                                                let _ = log::open_file(&p_open, &editor);
                                            }),
                                    )
                                    .child(
                                        side_action_button("ll-dir", Some("folder"), "Logs Folder", true, false, &t)
                                            .on_click(|_, _, _| {
                                                let _ = log::open_folder(&paths::logs_dir());
                                            }),
                                    ),
                            ),
                        None,
                        false,
                    )
                }
                UtilityMode::Updater => {
                    let st = updater::state();
                    let header = div()
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(brand_mark(46.))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(px(17.))
                                        .font_weight(FontWeight::BOLD)
                                        .child(match &st {
                                            updater::UpdateState::Available(_) => {
                                                "Do you want to update FnH?".to_owned()
                                            }
                                            updater::UpdateState::Downloading { version, .. } => {
                                                format!("Updating FnH to v{version}…")
                                            }
                                            updater::UpdateState::Applying { version } => {
                                                format!("Restarting into FnH v{version}…")
                                            }
                                            updater::UpdateState::UpToDate { current } => {
                                                format!("FnH v{current} is up to date")
                                            }
                                            updater::UpdateState::Checking
                                            | updater::UpdateState::Idle => {
                                                "Checking for FnH updates…".to_owned()
                                            }
                                            updater::UpdateState::Failed(_) => {
                                                "Could not complete update check".to_owned()
                                            }
                                        }),
                                )
                                .child(
                                    div()
                                        .text_size(px(12.))
                                        .text_color(t.ink_secondary)
                                        .child(format!(
                                            "Installed version: v{VERSION}  •  Package: {}",
                                            updater::PREFERRED_ZIP_NAME
                                        )),
                                ),
                        );

                    let content_area = match st {
                        updater::UpdateState::Available(rel) => {
                            let rel_yes = rel.clone();
                            let rel_url = rel.html_url.clone();
                            let dismiss_ver = rel.version.clone();
                            let notes_text = if rel.notes.is_empty() {
                                format!(
                                    "{} ({}) is available on GitHub Releases.\n\nClicking 'Yes, Update FnH' downloads {}, extracts the new FH.exe, deletes the downloaded .zip, and restarts FnH automatically.",
                                    rel.title, rel.tag, rel.zip_name
                                )
                            } else {
                                rel.notes.clone()
                            };
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_h_0()
                                .gap(px(10.))
                                .child(
                                    div()
                                        .text_size(px(13.))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(t.accent)
                                        .child(format!(
                                            "New release available: v{}  (current: v{VERSION})",
                                            rel.version
                                        )),
                                )
                                .child(
                                    div()
                                        .id("upd-notes-scroll")
                                        .flex_1()
                                        .min_h_0()
                                        .p(px(10.))
                                        .rounded(px(4.))
                                        .border_1()
                                        .border_color(t.control_border)
                                        .bg(t.input_bg)
                                        .text_size(px(12.))
                                        .text_color(t.ink_secondary)
                                        .overflow_y_scroll()
                                        .child(notes_text),
                                )
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(px(8.))
                                        .child(
                                            accent_button(
                                                "upd-yes",
                                                "Yes, Update FnH",
                                                true,
                                                &t,
                                            )
                                            .on_click(cx.listener(
                                                move |_, _: &ClickEvent, _, cx| {
                                                    updater::start_install_update(rel_yes.clone());
                                                    cx.notify();
                                                },
                                            )),
                                        )
                                        .child(
                                            push_button("upd-not-now", "Not Now", true, &t)
                                                .on_click(move |_, window, _| {
                                                    updater::dismiss_version(&dismiss_ver);
                                                    window.remove_window();
                                                }),
                                        )
                                        .child(
                                            push_button("upd-rel-page", "Release Page", true, &t)
                                                .on_click(move |_, _, _| {
                                                    let _ = log::open_url(&rel_url);
                                                }),
                                        ),
                                )
                        }
                        updater::UpdateState::Downloading {
                            percent, message, ..
                        } => {
                            let frac = (percent as f32 / 100.0).clamp(0.04, 1.0);
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(10.))
                                .pt(px(8.))
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .text_color(t.ink)
                                        .child(message),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .h(px(8.))
                                        .rounded(px(4.))
                                        .bg(t.control_border)
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .h_full()
                                                .w(gpui::relative(frac))
                                                .bg(t.accent),
                                        ),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(t.ink_tertiary)
                                        .child("FnHLauncher.zip will be deleted automatically once extracted, and FnH will restart."),
                                )
                        }
                        updater::UpdateState::Applying { .. } => div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .pt(px(8.))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .child("Applying update and relaunching FH.exe…"),
                            ),
                        updater::UpdateState::UpToDate { .. } => div()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .pt(px(6.))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("You are already running the newest published build of FnH."),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        accent_button("upd-recheck", "Check Again", true, &t)
                                            .on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                                                updater::check_in_background(false);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        push_button("upd-gh-latest", "View Releases", true, &t)
                                            .on_click(|_, _, _| {
                                                let _ = log::open_url(
                                                    "https://github.com/bufferclick/FnH/releases/latest",
                                                );
                                            }),
                                    ),
                            ),
                        updater::UpdateState::Checking | updater::UpdateState::Idle => div()
                            .flex()
                            .flex_col()
                            .gap(px(8.))
                            .pt(px(6.))
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("Contacting GitHub Releases (`bufferclick/FnH`)…"),
                            ),
                        updater::UpdateState::Failed(err) => div()
                            .flex()
                            .flex_col()
                            .gap(px(10.))
                            .pt(px(6.))
                            .child(status_banner(&err, true, &t))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        accent_button("upd-retry", "Try Again", true, &t).on_click(
                                            cx.listener(|_, _: &ClickEvent, _, cx| {
                                                updater::check_in_background(false);
                                                cx.notify();
                                            }),
                                        ),
                                    )
                                    .child(
                                        push_button(
                                            "upd-manual-zip",
                                            "Download FnHLauncher.zip in Browser",
                                            true,
                                            &t,
                                        )
                                        .on_click(|_, _, _| {
                                            let _ = log::open_url(updater::FALLBACK_ZIP_URL);
                                        }),
                                    ),
                            ),
                    };

                    (
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .gap(px(12.))
                            .p(px(18.))
                            .child(header)
                            .child(content_area),
                        None,
                        false,
                    )
                }
                UtilityMode::About { tab } => {
                    let cur = *tab;
                    (
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_h_0()
                            .gap(px(12.))
                            .p(px(18.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(14.))
                                    .child(brand_mark(54.))
                                    .child(
                                        div()
                                            .flex()
                                            .flex_col()
                                            .gap(px(2.))
                                            .child(
                                                div()
                                                    .text_size(px(18.))
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(format!("FNH v{VERSION}")),
                                            )
                                            .child(
                                                div()
                                                    .text_size(px(12.5))
                                                    .font_weight(FontWeight::SEMIBOLD)
                                                    .text_color(t.accent)
                                                    .child("Fabric n Horizon — Made by Buffer"),
                                            ),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(4.))
                                    .border_b_1()
                                    .border_color(t.divider)
                                    .children(["About", "Credits", "License"].into_iter().enumerate().map(
                                        |(i, lbl)| {
                                            sub_tab(format!("about-tab-{i}"), lbl, cur == i, &t)
                                                .on_click(cx.listener(
                                                    move |this, _: &ClickEvent, _, cx| {
                                                        if let UtilityMode::About { tab } =
                                                            &mut this.mode
                                                        {
                                                            *tab = i;
                                                        }
                                                        cx.notify();
                                                    },
                                                ))
                                        },
                                    )),
                            )
                            .child(match cur {
                                0 => div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(8.))
                                    .text_size(px(12.5))
                                    .child("A native Windows 11 launcher built in JLac's image for both Fabric (Minecraft: Java Edition) and Horizon instances — completely isolated, multi-window, fast, and customizable.")
                                    .child(
                                        div()
                                            .text_size(px(12.))
                                            .text_color(t.ink_secondary)
                                            .child(format!(
                                                "Version: v{VERSION}  •  Author: Buffer  •  Platform: {} ({})",
                                                std::env::consts::OS,
                                                std::env::consts::ARCH
                                            )),
                                    )
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap(px(8.))
                                            .pt(px(4.))
                                            .child(
                                                push_button("ab-site", "fnh.is-cool.dev", true, &t)
                                                    .on_click(|_, _, _| {
                                                        let _ = log::open_url("https://fnh.is-cool.dev");
                                                    }),
                                            )
                                            .child(
                                                push_button("ab-gh", "View on GitHub", true, &t)
                                                    .on_click(|_, _, _| {
                                                        let _ = log::open_url(
                                                            "https://github.com/bufferclick/fnh",
                                                        );
                                                    }),
                                            )
                                            .child(
                                                push_button("ab-rel", "Releases", true, &t)
                                                    .on_click(|_, _, _| {
                                                        let _ = log::open_url(
                                                            "https://github.com/bufferclick/fnh/releases/latest",
                                                        );
                                                    }),
                                            ),
                                    ),
                                1 => div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .text_size(px(12.5))
                                    .child("• FNH (Fabric n Horizon) — created by Buffer")
                                    .child("• JLac UI & design — created by Buffer (Prism Launcher / MultiMC lineage)")
                                    .child("• Horizon engine & runtime — Horizon contributors")
                                    .child("• Fabric Loader — FabricMC contributors")
                                    .child(
                                        div()
                                            .pt(px(4.))
                                            .text_size(px(11.5))
                                            .text_color(t.ink_tertiary)
                                            .child("Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft."),
                                    ),
                                _ => div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(6.))
                                    .text_size(px(12.))
                                    .text_color(t.ink_secondary)
                                    .child("FNH — Copyright (c) Buffer. Released under the MIT License.")
                                    .child("Permission is hereby granted, free of charge, to any person obtaining a copy of this software to use, copy, modify, merge, publish and distribute it, provided the above notice is retained.")
                                    .child("Bundled classic MultiMC / Prism / JLac instance preset icons retain their respective upstream open-source licenses."),
                            }),
                        None,
                        false,
                    )
                }
            };

        let footer = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_end()
            .gap(px(8.))
            .px(px(14.))
            .py(px(10.))
            .bg(t.footer)
            .border_t_1()
            .border_color(t.divider)
            .when_some(ok_label, |d, lbl| {
                let btn = if destructive {
                    danger_button("util-ok", lbl, true, &t)
                } else {
                    accent_button("util-ok", lbl, true, &t)
                };
                d.child(btn.on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                    this.submit(window, cx);
                })))
            })
            .child(
                push_button(
                    "util-close",
                    if ok_label.is_some() { "Cancel" } else { "Close" },
                    true,
                    &t,
                )
                .on_click(|_, window, _| window.remove_window()),
            );

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                let k = ev.keystroke.key.as_str();
                if k == "escape" {
                    window.remove_window();
                    return;
                }
                if k == "enter" {
                    if !matches!(
                        this.mode,
                        UtilityMode::LauncherLog
                            | UtilityMode::Updater
                            | UtilityMode::About { .. }
                            | UtilityMode::FabricVersions { .. }
                    ) {
                        this.submit(window, cx);
                    }
                    return;
                }
                let edited = match &mut this.mode {
                    UtilityMode::Rename { value, .. }
                    | UtilityMode::Group { value, .. }
                    | UtilityMode::Copy { value, .. }
                    | UtilityMode::Shortcut { name: value, .. }
                    | UtilityMode::FabricVersions { mc: value, .. } => {
                        apply_text_key(value, ev, false, cx)
                    }
                    _ => false,
                };
                if edited {
                    cx.notify();
                }
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(t.layer)
            .font_family(UI_FONT)
            .text_color(t.ink)
            .when_some(self.error.clone(), |d, e| {
                d.child(
                    div()
                        .px(px(14.))
                        .pt(px(10.))
                        .child(status_banner(&e, true, &t)),
                )
            })
            .child(div().flex().flex_col().flex_1().min_h_0().child(body))
            .child(footer)
    }
}

/// Suppress unused-import warnings when optional icons are swapped.
#[allow(dead_code)]
fn _glyph_ref(t: &Theme) {
    let _ = icon("info", 12., t.ink);
}

// ================================================================================================
// 5. Fabric-Style Crash & Mod Compatibility Window (`CrashWindow`)
// ================================================================================================

pub struct CrashWindow {
    app: Entity<Fh>,
    focus: FocusHandle,
    diag: crash::CrashDiagnosis,
    feedback: Option<(String, bool)>,
}

impl CrashWindow {
    pub fn open(
        app: Entity<Fh>,
        diag: crash::CrashDiagnosis,
        cx: &mut App,
    ) -> Option<WindowHandle<Self>> {
        let title = format!("{} — {} - FNH", diag.headline, diag.instance_name);
        let opts = secondary_window_options(title, 800., 620., 620., 460., cx);
        let res = cx
            .open_window(opts, move |window, cx| {
                cx.new(|cx| {
                    cx.observe(&app, |_: &mut Self, _, cx| cx.notify()).detach();
                    let focus = cx.focus_handle();
                    window.focus(&focus, cx);
                    Self {
                        app,
                        focus,
                        diag,
                        feedback: None,
                    }
                })
            })
            .ok();
        apply_window_icon();
        res
    }

    fn instance_home(&self, cx: &App) -> Option<std::path::PathBuf> {
        self.app
            .read(cx)
            .instances
            .iter()
            .find(|i| i.id == self.diag.instance_id)
            .map(|i| i.home.clone())
            .or_else(|| self.diag.mods_dir.parent().map(|p| p.to_path_buf()))
    }

    fn disable_mod_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(home) = self.instance_home(cx) else {
            self.feedback = Some(("Could not locate instance directory.".into(), true));
            cx.notify();
            return;
        };
        let Some(entry) = self.diag.offending_mods.get(index).cloned() else {
            return;
        };
        match crash::disable_suspect_mod(&home, &entry) {
            Ok(msg) => {
                if let Some(slot) = self.diag.offending_mods.get_mut(index) {
                    slot.disabled = true;
                }
                self.feedback = Some((msg, false));
                self.app.update(cx, |a, cx| {
                    a.reload_instances();
                    cx.notify();
                });
            }
            Err(e) => {
                self.feedback = Some((e, true));
            }
        }
        cx.notify();
    }
}

impl Render for CrashWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let prefs = self.app.read(cx).prefs.clone();
        let t = Theme::resolve(prefs.theme_mode, prefs.accent, window.appearance());

        let header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(12.))
            .px(px(18.))
            .py(px(14.))
            .bg(t.window)
            .border_b_1()
            .border_color(t.divider)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(12.))
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .pt(px(2.))
                            .child(icon("warning", 26., t.danger)),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(3.))
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .child(
                                        div()
                                            .px(px(6.))
                                            .py(px(1.))
                                            .rounded(px(3.))
                                            .bg(t.danger)
                                            .text_color(gpui::white())
                                            .text_size(px(10.5))
                                            .font_weight(FontWeight::BOLD)
                                            .child(self.diag.kind.badge()),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(t.ink_tertiary)
                                            .child(format!(
                                                "{}  •  {}",
                                                self.diag.instance_name, self.diag.engine_label
                                            )),
                                    ),
                            )
                            .child(
                                div()
                                    .text_size(px(16.))
                                    .font_weight(FontWeight::BOLD)
                                    .child(self.diag.headline.clone()),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child(self.diag.summary.clone()),
                            ),
                    ),
            )
            .child(brand_mark(30.));

        let mut scroll = div()
            .id("crash-body-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .gap(px(12.))
            .p(px(16.))
            .bg(t.layer)
            .overflow_y_scroll();

        if let Some((ref msg, is_err)) = self.feedback {
            scroll = scroll.child(status_banner(msg, is_err, &t));
        }

        // 1. Fabric-Loader-style "A potential solution has been determined:" card.
        if !self.diag.solutions.is_empty() {
            let mut sol_box = group_box("A potential solution has been determined", &t);
            for (i, sol) in self.diag.solutions.iter().enumerate() {
                sol_box = sol_box.child(
                    div()
                        .id(SharedString::from(format!("crash-sol-{i}")))
                        .flex()
                        .items_start()
                        .gap(px(8.))
                        .child(
                            div()
                                .pt(px(2.))
                                .child(icon("wrench", 14., t.accent)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_size(px(12.5))
                                .text_color(t.ink)
                                .child(sol.clone()),
                        ),
                );
            }
            scroll = scroll.child(sol_box);
        }

        // 2. Suspect / Incompatible Mods card with 1-click Disable buttons!
        if !self.diag.offending_mods.is_empty() {
            let mut mod_box = group_box("Suspect / Incompatible Mods", &t);
            for (idx, m) in self.diag.offending_mods.iter().enumerate() {
                let disabled = m.disabled;
                let can_disable = !disabled && (m.path.is_some() || m.is_horizon);
                mod_box = mod_box.child(
                    div()
                        .id(SharedString::from(format!("crash-mod-{idx}")))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(10.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(4.))
                        .border_1()
                        .border_color(t.divider)
                        .bg(t.input_bg)
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_1()
                                .min_w_0()
                                .gap(px(1.))
                                .child(
                                    div()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .child(m.display_name.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(px(11.5))
                                        .text_color(t.ink_secondary)
                                        .child(m.reason.clone()),
                                ),
                        )
                        .child(if disabled {
                            div()
                                .px(px(8.))
                                .py(px(3.))
                                .text_size(px(11.5))
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.accent)
                                .child("✓ Disabled")
                                .into_any_element()
                        } else if can_disable {
                            danger_button(format!("crash-dis-{idx}"), "Disable Mod", true, &t)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.disable_mod_at(idx, cx);
                                }))
                                .into_any_element()
                        } else {
                            div().into_any_element()
                        }),
                );
            }
            scroll = scroll.child(mod_box);
        }

        // 3. Raw stack trace / crash-report viewer.
        let report_copy = format!(
            "=== FNH v{VERSION} Crash & Mod Compatibility Report ===\nInstance: {} ({})\nStatus: {}\nSummary: {}\n\n{}\n",
            self.diag.instance_name,
            self.diag.engine_label,
            self.diag.headline,
            self.diag.summary,
            self.diag.raw_details
        );

        scroll = scroll.child(
            group_box("Diagnostic Details & Stack Trace", &t).child(
                div()
                    .id("crash-raw-trace")
                    .max_h(px(210.))
                    .p(px(10.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(t.control_border)
                    .bg(t.input_bg)
                    .font_family(MONO_FONT)
                    .text_size(px(11.5))
                    .text_color(t.ink_secondary)
                    .overflow_y_scroll()
                    .child(if self.diag.raw_details.trim().is_empty() {
                        self.diag.summary.clone()
                    } else {
                        self.diag.raw_details.clone()
                    }),
            ),
        );

        let inst_id_browse = self.diag.instance_id.clone();
        let inst_id_log = self.diag.instance_id.clone();
        let inst_id_relaunch = self.diag.instance_id.clone();
        let has_inst = !self.diag.instance_id.is_empty();
        let mods_dir = self.diag.mods_dir.clone();

        let footer = div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(8.))
            .px(px(14.))
            .py(px(10.))
            .bg(t.footer)
            .border_t_1()
            .border_color(t.divider)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(6.))
                    .when(has_inst, |d| {
                        d.child(
                            push_button("crash-browse-mods", "Browse Mods", true, &t).on_click(
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let iid = inst_id_browse.clone();
                                    this.app.update(cx, |a, cx| {
                                        if let Some(inst) =
                                            a.instances.iter().find(|i| i.id == iid).cloned()
                                        {
                                            a.open_mod_browser(inst, BrowseKind::Mod, cx);
                                        }
                                    });
                                }),
                            ),
                        )
                    })
                    .child(
                        push_button("crash-open-mods", "Open Mods Folder", true, &t).on_click(
                            move |_, _, _| {
                                let _ = std::fs::create_dir_all(&mods_dir);
                                let _ = log::open_folder(&mods_dir);
                            },
                        ),
                    )
                    .when(has_inst, |d| {
                        d.child(
                            push_button("crash-view-log", "View Full Log", true, &t).on_click(
                                cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let iid = inst_id_log.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.open_instance_window(&iid, InstancePage::Log, cx);
                                    });
                                }),
                            ),
                        )
                    })
                    .child(
                        push_button("crash-copy", "Copy Crash Report", true, &t).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    report_copy.clone(),
                                ));
                                this.feedback =
                                    Some(("Copied crash report to clipboard.".into(), false));
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when(has_inst, |d| {
                        d.child(
                            accent_button("crash-relaunch", "Relaunch", true, &t).on_click(
                                cx.listener(move |this, _: &ClickEvent, window, cx| {
                                    let iid = inst_id_relaunch.clone();
                                    this.app.update(cx, |a, cx| {
                                        a.launch_instance(&iid, job::LaunchExtras::default(), cx);
                                    });
                                    window.remove_window();
                                }),
                            ),
                        )
                    })
                    .child(
                        push_button("crash-close", "Close", true, &t)
                            .on_click(|_, window, _| window.remove_window()),
                    ),
            );

        div()
            .track_focus(&self.focus)
            .on_key_down(|ev: &KeyDownEvent, window, _| {
                if ev.keystroke.key.as_str() == "escape" {
                    window.remove_window();
                }
            })
            .flex()
            .flex_col()
            .size_full()
            .bg(t.window)
            .font_family(UI_FONT)
            .text_color(t.ink)
            .child(header)
            .child(scroll)
            .child(footer)
    }
}

