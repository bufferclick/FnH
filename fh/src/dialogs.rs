//! Windows 11 (WinUI 3) dialogs and `PageContainer` windows for FNH, matching JLac's
//! `NewInstanceDialog`, `InstanceWindow`, `SettingsDialog`, `AccountListPage`, `MSALoginDialog`,
//! `FabricVersionsDialog` and `AboutDialog`.

use crate::app::{
    format_bytes, mc_kind_label, Dialog, EditPage, Fh, Field, NewKind, NewLoader, SettingsPage,
    RELEASES_URL, REPO_URL, SITE_URL,
};
use crate::icons::icon;
use crate::look::{
    self, brand_mark, check_box, dialog_shell, hex, hsep, instance_icon, nav_item, push_button,
    text_box, ButtonKind, Theme, ICON_PALETTE, MONO_FONT,
};
use crate::prefs::{Accent, SortMode, ThemeMode};
use crate::{job, log, VERSION};
use gpui::{
    div, prelude::*, px, AnyElement, ClipboardItem, Context, Div, FontWeight, MouseButton,
    SharedString, Stateful,
};
use horizon_launcher::minecraft_versions::Kind as McKind;
use horizon_launcher::{backend, java};

impl Fh {
    /// Renders the active modal WinUI 3 dialog over a subtle Windows 11 smoke backdrop.
    pub(crate) fn render_dialog(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let which = self.dialog?;
        let card = match which {
            Dialog::AddInstance => self.dialog_add_instance(t, cx),
            Dialog::EditInstance => self.dialog_edit_instance(t, cx),
            Dialog::Settings => self.dialog_settings(t, cx),
            Dialog::Accounts => self.dialog_accounts(t, cx),
            Dialog::SignIn => self.dialog_sign_in(t, cx),
            Dialog::FabricVersions => self.dialog_fabric_versions(t, cx),
            Dialog::LauncherLog => self.dialog_launcher_log(t, cx),
            Dialog::Rename => self.dialog_prompt(
                "Rename Instance",
                "New instance name:",
                "Enter a name",
                "Rename",
                false,
                t,
                cx,
            ),
            Dialog::ChangeGroup => self.dialog_change_group(t, cx),
            Dialog::CopyInstance => self.dialog_prompt(
                "Copy Instance",
                "Name for the copied instance:",
                "Enter a name",
                "Copy",
                false,
                t,
                cx,
            ),
            Dialog::ConfirmDelete => self.dialog_confirm_delete(t, cx),
            Dialog::About => self.dialog_about(t, cx),
        };

        let scrim = look::hexa(0x000000, if t.dark { 0.52 } else { 0.30 });
        Some(
            div()
                .id("dialog-scrim")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .p(px(20.))
                .bg(scrim)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.field = None;
                        cx.notify();
                    }),
                )
                .child(card)
                .into_any_element(),
        )
    }

    // ----- Add Instance (JLac NewInstanceDialog) ------------------------------------------------

    fn dialog_add_instance(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let swatch_hex = ICON_PALETTE[self.new_form.colour_idx % ICON_PALETTE.len()];
        let swatch = u32::from_str_radix(&swatch_hex[1..], 16)
            .map(hex)
            .unwrap_or(t.accent);
        let preview_letter = if self.new_form.name.trim().is_empty() {
            match self.new_form.kind {
                NewKind::Custom => "F".into(),
                NewKind::Horizon => "H".into(),
            }
        } else {
            look::initial(&self.new_form.name)
        };

        // Top icon + Name + Group bar, matching JLac's NewInstanceDialog.ui.
        let top_bar = div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(14.))
            .px(px(18.))
            .py(px(14.))
            .bg(t.sidebar)
            .child(
                div()
                    .id("new-icon-btn")
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(52.))
                    .rounded(px(10.))
                    .bg(swatch)
                    .text_color(hex(0xffffff))
                    .text_size(px(22.))
                    .font_weight(FontWeight::BOLD)
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.new_form.colour_idx =
                            (this.new_form.colour_idx + 1) % ICON_PALETTE.len();
                        cx.notify();
                    }))
                    .child(preview_letter),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .gap(px(8.))
                    .child(form_row(
                        "Name:",
                        text_box(
                            "new-name",
                            &self.new_form.name,
                            match self.new_form.kind {
                                NewKind::Custom => "Instance name (e.g. 1.21.4)",
                                NewKind::Horizon => "Horizon",
                            },
                            self.field == Some(Field::NewName),
                            false,
                            t,
                        )
                        .flex_1()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.field = Some(Field::NewName);
                                cx.notify();
                            }),
                        )
                        .into_any_element(),
                        t,
                    ))
                    .child(form_row(
                        "Group:",
                        text_box(
                            "new-group",
                            &self.new_form.group,
                            "No group (optional)",
                            self.field == Some(Field::NewGroup),
                            false,
                            t,
                        )
                        .flex_1()
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, _, _, cx| {
                                cx.stop_propagation();
                                this.field = Some(Field::NewGroup);
                                cx.notify();
                            }),
                        )
                        .into_any_element(),
                        t,
                    )),
            );

        let nav = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(154.))
            .p(px(8.))
            .gap(px(2.))
            .bg(t.sidebar)
            .child(
                nav_item(
                    "new-nav-custom",
                    "fabric",
                    "Custom",
                    self.new_form.kind == NewKind::Custom,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_new_kind(NewKind::Custom);
                    if this.mc_versions.is_none() {
                        this.fetch_mc_versions(false);
                    }
                    cx.notify();
                })),
            )
            .child(
                nav_item(
                    "new-nav-horizon",
                    "horizon",
                    "Horizon",
                    self.new_form.kind == NewKind::Horizon,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.set_new_kind(NewKind::Horizon);
                    cx.notify();
                })),
            );

        let page: AnyElement = match self.new_form.kind {
            NewKind::Custom => self.new_custom_page(t, cx),
            NewKind::Horizon => self.new_horizon_page(t),
        };

        let body = div()
            .flex()
            .flex_col()
            .w_full()
            .h(px(456.))
            .child(top_bar)
            .child(hsep(t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(nav)
                    .child(div().flex_none().w(px(1.)).h_full().bg(t.divider))
                    .child(page),
            )
            .into_any_element();

        let can_create = !self.new_form.creating
            && match self.new_form.kind {
                NewKind::Horizon => true,
                NewKind::Custom => !self.new_form.mc_version.trim().is_empty(),
            };
        let status_note = if self.new_form.creating {
            Some(("Creating instance and resolving Fabric Loader…".to_owned(), false))
        } else {
            self.new_form.error.as_ref().map(|e| (e.clone(), true))
        };

        let footer = div()
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_size(px(12.))
                    .text_color(match status_note {
                        Some((_, true)) => t.danger,
                        _ => t.ink_secondary,
                    })
                    .child(status_note.map(|(s, _)| s).unwrap_or_default()),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        push_button("new-ok", "OK", ButtonKind::Accent, can_create, t).when(
                            can_create,
                            |b| {
                                b.on_click(cx.listener(|this, _, _, cx| {
                                    this.submit_add_instance();
                                    cx.notify();
                                }))
                            },
                        ),
                    )
                    .child(
                        push_button("new-cancel", "Cancel", ButtonKind::Standard, true, t)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_dialog();
                                cx.notify();
                            })),
                    ),
            );

        dialog_shell(720., body, footer, t)
    }

    fn new_custom_page(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let needle = self.new_form.filter.trim().to_ascii_lowercase();
        let mut rows: Vec<AnyElement> = Vec::new();
        if let Some(versions) = &self.mc_versions {
            for (idx, ver) in versions
                .all
                .iter()
                .filter(|v| match v.kind {
                    McKind::Release => self.new_form.show_releases,
                    McKind::Snapshot => self.new_form.show_snapshots,
                    McKind::OldBeta => self.new_form.show_betas,
                    McKind::OldAlpha => self.new_form.show_alphas,
                })
                .filter(|v| needle.is_empty() || v.id.to_ascii_lowercase().contains(&needle))
                .take(180)
                .enumerate()
            {
                let selected = self.new_form.mc_version == ver.id;
                let id = ver.id.clone();
                let date = ver.released.get(..10).unwrap_or(&ver.released).to_owned();
                let hover = t.subtle_hover;
                rows.push(
                    div()
                        .id(SharedString::from(format!("mc-row-{idx}")))
                        .flex()
                        .flex_none()
                        .items_center()
                        .h(px(27.))
                        .px(px(10.))
                        .rounded(px(3.))
                        .bg(if selected {
                            t.tile_selected
                        } else {
                            gpui::transparent_black()
                        })
                        .text_size(px(12.))
                        .text_color(t.ink)
                        .cursor_pointer()
                        .when(!selected, |r| r.hover(move |s| s.bg(hover)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.pick_new_mc_version(&id);
                            cx.notify();
                        }))
                        .child(
                            div()
                                .flex_1()
                                .font_weight(if selected {
                                    FontWeight::SEMIBOLD
                                } else {
                                    FontWeight::NORMAL
                                })
                                .child(ver.id.clone()),
                        )
                        .child(
                            div()
                                .w(px(94.))
                                .text_color(t.ink_secondary)
                                .child(date),
                        )
                        .child(
                            div()
                                .w(px(68.))
                                .text_color(t.ink_secondary)
                                .child(mc_kind_label(ver.kind)),
                        )
                        .into_any_element(),
                );
            }
        }

        let table_body: AnyElement = if self.mc_loading {
            empty_table_note("Loading Minecraft versions from Mojang…", t)
        } else if let Some(err) = &self.mc_error {
            empty_table_note(err, t)
        } else if rows.is_empty() {
            empty_table_note("No Minecraft versions match the current filter.", t)
        } else {
            div()
                .id("mc-version-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .p(px(3.))
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        let filters = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(116.))
            .gap(px(2.))
            .child(
                div()
                    .text_size(px(11.5))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.ink_secondary)
                    .mb(px(2.))
                    .child("Filter"),
            )
            .child(
                check_box("flt-rel", "Releases", self.new_form.show_releases, t).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.new_form.show_releases = !this.new_form.show_releases;
                        cx.notify();
                    }),
                ),
            )
            .child(
                check_box("flt-snap", "Snapshots", self.new_form.show_snapshots, t).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.new_form.show_snapshots = !this.new_form.show_snapshots;
                        cx.notify();
                    }),
                ),
            )
            .child(
                check_box("flt-beta", "Betas", self.new_form.show_betas, t).on_click(cx.listener(
                    |this, _, _, cx| {
                        this.new_form.show_betas = !this.new_form.show_betas;
                        cx.notify();
                    },
                )),
            )
            .child(
                check_box("flt-alpha", "Alphas", self.new_form.show_alphas, t).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.new_form.show_alphas = !this.new_form.show_alphas;
                        cx.notify();
                    }),
                ),
            )
            .child(div().flex_1())
            .child(
                push_button(
                    "mc-refresh",
                    "Refresh",
                    ButtonKind::Standard,
                    !self.mc_loading,
                    t,
                )
                .w_full()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.fetch_mc_versions(true);
                    cx.notify();
                })),
            );

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .p(px(14.))
            .gap(px(10.))
            .bg(t.layer)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(t.control_border)
                            .bg(t.input_bg)
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .flex_none()
                                    .items_center()
                                    .h(px(28.))
                                    .px(px(10.))
                                    .bg(t.sidebar)
                                    .border_b_1()
                                    .border_color(t.divider)
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.ink_secondary)
                                    .child(div().flex_1().child("Version"))
                                    .child(div().w(px(94.)).child("Released"))
                                    .child(div().w(px(68.)).child("Type")),
                            )
                            .child(table_body),
                    )
                    .child(filters),
            )
            .child(
                text_box(
                    "mc-search",
                    &self.new_form.filter,
                    "Search Minecraft versions…",
                    self.field == Some(Field::NewVersionFilter),
                    true,
                    t,
                )
                .w_full()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.field = Some(Field::NewVersionFilter);
                        cx.notify();
                    }),
                ),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .gap(px(10.))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.ink)
                            .child("Mod Loader"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                push_button(
                                    "ldr-none",
                                    "None (Vanilla)",
                                    if self.new_form.loader == NewLoader::Vanilla {
                                        ButtonKind::Accent
                                    } else {
                                        ButtonKind::Standard
                                    },
                                    true,
                                    t,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.new_form.loader = NewLoader::Vanilla;
                                    cx.notify();
                                })),
                            )
                            .child(
                                push_button(
                                    "ldr-fabric",
                                    "Fabric (latest stable)",
                                    if self.new_form.loader == NewLoader::Fabric {
                                        ButtonKind::Accent
                                    } else {
                                        ButtonKind::Standard
                                    },
                                    true,
                                    t,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.new_form.loader = NewLoader::Fabric;
                                    cx.notify();
                                })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn new_horizon_page(&self, t: &Theme) -> AnyElement {
        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .p(px(18.))
            .gap(px(12.))
            .bg(t.layer)
            .child(
                div()
                    .text_size(px(15.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(t.ink)
                    .child("Horizon Instance"),
            )
            .child(
                div()
                    .p(px(14.))
                    .rounded(px(6.))
                    .bg(t.control)
                    .border_1()
                    .border_color(t.control_border)
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(px(12.5))
                            .text_color(t.ink)
                            .child(
                                "Creates an isolated Horizon game folder with its own worlds, mods, resource packs and options.",
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(t.ink_secondary)
                            .child(
                                "On first launch, FNH downloads the latest Horizon build automatically if needed, prepares assets from your owned Minecraft copy, and starts Horizon directly.",
                            ),
                    ),
            )
            .into_any_element()
    }

    // ----- Edit Instance (JLac InstanceWindow) --------------------------------------------------

    fn dialog_edit_instance(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let Some(instance) = self.find_instance(&self.edit.instance_id) else {
            return dialog_shell(
                420.,
                div().p(px(20.)).child("Instance no longer exists.").into_any_element(),
                div().child(
                    push_button("ed-gone", "Close", ButtonKind::Accent, true, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.close_dialog();
                            cx.notify();
                        }),
                    ),
                ),
                t,
            );
        };
        let running = self.status.busy
            && self.status.instance_id.as_deref() == Some(instance.id.as_str());
        let id_launch = instance.id.clone();
        let home_open = instance.home.clone();

        let sidebar = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(178.))
            .p(px(10.))
            .gap(px(2.))
            .bg(t.sidebar)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .px(px(6.))
                    .py(px(6.))
                    .mb(px(6.))
                    .child(instance_icon(instance, 34., running, t))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(12.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.ink)
                                    .child(instance.name.clone()),
                            )
                            .child(
                                div()
                                    .truncate()
                                    .text_size(px(11.))
                                    .text_color(t.ink_secondary)
                                    .child(crate::app::instance_subtitle(instance)),
                            ),
                    ),
            )
            .child(
                nav_item(
                    "ed-nav-log",
                    "log",
                    "Minecraft Log",
                    self.edit.page == EditPage::Log,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit.page = EditPage::Log;
                    this.edit.notice = None;
                    this.field = None;
                    this.reload_edit_log();
                    cx.notify();
                })),
            )
            .child(
                nav_item(
                    "ed-nav-ver",
                    "fabric",
                    "Version",
                    self.edit.page == EditPage::Version,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit.page = EditPage::Version;
                    this.edit.notice = None;
                    this.field = None;
                    cx.notify();
                })),
            )
            .child(
                nav_item(
                    "ed-nav-mods",
                    "mods",
                    "Mods",
                    self.edit.page == EditPage::Mods,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit.page = EditPage::Mods;
                    this.edit.notice = None;
                    this.field = None;
                    this.reload_edit_mods();
                    cx.notify();
                })),
            )
            .child(
                nav_item(
                    "ed-nav-worlds",
                    "world",
                    "Worlds",
                    self.edit.page == EditPage::Worlds,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit.page = EditPage::Worlds;
                    this.edit.notice = None;
                    this.field = None;
                    this.reload_edit_worlds();
                    cx.notify();
                })),
            )
            .child(
                nav_item(
                    "ed-nav-settings",
                    "settings",
                    "Settings",
                    self.edit.page == EditPage::Settings,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.edit.page = EditPage::Settings;
                    this.edit.notice = None;
                    this.field = None;
                    cx.notify();
                })),
            );

        let content: AnyElement = match self.edit.page {
            EditPage::Log => self.edit_log_page(instance, t, cx),
            EditPage::Version => self.edit_version_page(instance, t, cx),
            EditPage::Mods => self.edit_mods_page(instance, t, cx),
            EditPage::Worlds => self.edit_worlds_page(instance, t, cx),
            EditPage::Settings => self.edit_settings_page(instance, t, cx),
        };

        let body = div()
            .flex()
            .w_full()
            .h(px(468.))
            .child(sidebar)
            .child(div().flex_none().w(px(1.)).h_full().bg(t.divider))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .min_h(px(0.))
                    .bg(t.layer)
                    .child(content)
                    .when_some(self.edit.notice.clone(), |col, (msg, is_err)| {
                        col.child(
                            div()
                                .flex_none()
                                .px(px(16.))
                                .py(px(6.))
                                .border_t_1()
                                .border_color(t.divider)
                                .text_size(px(12.))
                                .text_color(if is_err { t.danger } else { t.ink_secondary })
                                .truncate()
                                .child(msg),
                        )
                    }),
            )
            .into_any_element();

        let can_launch = !self.status.busy;
        let can_kill = job::can_kill();
        let footer = div()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        push_button("ed-launch", "Launch", ButtonKind::Accent, can_launch, t)
                            .when(can_launch, |b| {
                                b.on_click(cx.listener(move |this, _, _, cx| {
                                    this.launch_id(&id_launch);
                                    cx.notify();
                                }))
                            }),
                    )
                    .child(
                        push_button("ed-kill", "Kill", ButtonKind::Standard, can_kill, t).when(
                            can_kill,
                            |b| {
                                b.on_click(cx.listener(|this, _, _, cx| {
                                    this.kill_running();
                                    cx.notify();
                                }))
                            },
                        ),
                    )
                    .child(
                        push_button(
                            "ed-folder",
                            "Instance Folder",
                            ButtonKind::Standard,
                            true,
                            t,
                        )
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_dir(home_open.clone(), true);
                            cx.notify();
                        })),
                    ),
            )
            .child(
                push_button("ed-close", "Close", ButtonKind::Standard, true, t).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.close_dialog();
                        cx.notify();
                    }),
                ),
            );

        dialog_shell(760., body, footer, t)
    }

    fn edit_version_page(
        &self,
        instance: &horizon_launcher::instances::Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut comp_rows: Vec<AnyElement> = Vec::new();
        match &instance.java {
            Some(java) => {
                comp_rows.push(component_row(
                    "Minecraft",
                    "net.minecraft",
                    &java.minecraft,
                    true,
                    t,
                ));
                if let Some(loader) = &java.loader {
                    comp_rows.push(component_row(
                        &format!("{} Loader", loader.kind.name()),
                        "net.fabricmc.fabric-loader",
                        &loader.version,
                        false,
                        t,
                    ));
                }
            }
            None => {
                let ver = instance.version.as_deref().unwrap_or("Latest release");
                comp_rows.push(component_row(
                    "Horizon Client",
                    "horizon.client",
                    ver,
                    true,
                    t,
                ));
            }
        }

        let is_java = instance.java.is_some();
        let has_loader = instance
            .java
            .as_ref()
            .and_then(|j| j.loader.as_ref())
            .is_some();
        let id_latest = instance.id.clone();
        let id_pick = instance.id.clone();
        let id_remove = instance.id.clone();

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .p(px(16.))
            .gap(px(12.))
            .child(page_title("Version", t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(t.control_border)
                            .bg(t.input_bg)
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h(px(28.))
                                    .px(px(10.))
                                    .bg(t.sidebar)
                                    .border_b_1()
                                    .border_color(t.divider)
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.ink_secondary)
                                    .child(div().flex_1().child("Name"))
                                    .child(div().w(px(165.)).child("UID"))
                                    .child(div().w(px(110.)).child("Version")),
                            )
                            .child(div().flex().flex_col().p(px(4.)).gap(px(2.)).children(comp_rows)),
                    )
                    .when(is_java, |row| {
                        row.child(
                            div()
                                .flex()
                                .flex_col()
                                .flex_none()
                                .w(px(146.))
                                .gap(px(6.))
                                .child(
                                    push_button(
                                        "ver-install-fab",
                                        if has_loader {
                                            "Update Fabric"
                                        } else {
                                            "Install Fabric"
                                        },
                                        ButtonKind::Accent,
                                        !self.edit.working,
                                        t,
                                    )
                                    .w_full()
                                    .when(!self.edit.working, |b| {
                                        b.on_click(cx.listener(move |this, _, _, cx| {
                                            this.install_fabric_on(&id_latest, None);
                                            cx.notify();
                                        }))
                                    }),
                                )
                                .child(
                                    push_button(
                                        "ver-change-fab",
                                        "Change Version...",
                                        ButtonKind::Standard,
                                        !self.edit.working,
                                        t,
                                    )
                                    .w_full()
                                    .when(!self.edit.working, |b| {
                                        b.on_click(cx.listener(move |this, _, _, cx| {
                                            this.open_fabric_versions(Some(id_pick.clone()));
                                            cx.notify();
                                        }))
                                    }),
                                )
                                .child(
                                    push_button(
                                        "ver-remove-ldr",
                                        "Remove Loader",
                                        ButtonKind::Standard,
                                        has_loader && !self.edit.working,
                                        t,
                                    )
                                    .w_full()
                                    .when(has_loader && !self.edit.working, |b| {
                                        b.on_click(cx.listener(move |this, _, _, cx| {
                                            this.remove_loader_on(&id_remove);
                                            cx.notify();
                                        }))
                                    }),
                                ),
                        )
                    }),
            )
            .into_any_element()
    }

    fn edit_mods_page(
        &self,
        instance: &horizon_launcher::instances::Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let needle = self.edit.mod_filter.trim().to_ascii_lowercase();
        let mut rows: Vec<AnyElement> = Vec::new();
        for (idx, entry) in self
            .edit
            .mods
            .iter()
            .filter(|m| needle.is_empty() || m.display.to_ascii_lowercase().contains(&needle))
            .enumerate()
        {
            let selected = self.edit.selected_mod.as_deref() == Some(entry.file_name.as_str());
            let sel_name = entry.file_name.clone();
            let tog_name = entry.file_name.clone();
            let hover = t.subtle_hover;
            rows.push(
                div()
                    .id(SharedString::from(format!("mod-row-{idx}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .h(px(30.))
                    .px(px(10.))
                    .rounded(px(3.))
                    .bg(if selected {
                        t.tile_selected
                    } else {
                        gpui::transparent_black()
                    })
                    .cursor_pointer()
                    .when(!selected, |r| r.hover(move |s| s.bg(hover)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit.selected_mod = Some(sel_name.clone());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .id(SharedString::from(format!("mod-chk-{idx}")))
                            .flex()
                            .flex_none()
                            .items_center()
                            .justify_center()
                            .size(px(17.))
                            .rounded(px(4.))
                            .bg(if entry.enabled { t.accent } else { t.input_bg })
                            .border_1()
                            .border_color(if entry.enabled { t.accent } else { t.input_bottom })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.toggle_mod(&tog_name);
                                cx.notify();
                            }))
                            .when(entry.enabled, |b| b.child(icon("check", 12., t.on_accent))),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .truncate()
                            .text_size(px(12.))
                            .text_color(if entry.enabled {
                                t.ink
                            } else {
                                t.ink_tertiary
                            })
                            .child(entry.display.clone()),
                    )
                    .child(
                        div()
                            .w(px(72.))
                            .text_size(px(11.5))
                            .text_color(t.ink_secondary)
                            .child(format_bytes(entry.size)),
                    )
                    .into_any_element(),
            );
        }

        let sel_mod = self
            .edit
            .selected_mod
            .as_deref()
            .and_then(|name| self.edit.mods.iter().find(|m| m.file_name == name));
        let has_sel = sel_mod.is_some();
        let toggle_label = match sel_mod {
            Some(m) if m.enabled => "Disable",
            Some(_) => "Enable",
            None => "Enable / Disable",
        };
        let sel_toggle_name = sel_mod.map(|m| m.file_name.clone()).unwrap_or_default();
        let mods_dir = instance.home.join("mods");

        let list_view: AnyElement = if rows.is_empty() {
            empty_table_note(
                "No mod .jar files in mods/. Click \"View Folder\" to drop Fabric mods in.",
                t,
            )
        } else {
            div()
                .id("mods-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .p(px(3.))
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .p(px(16.))
            .gap(px(10.))
            .child(page_title(&format!("Mods ({})", self.edit.mods.len()), t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(t.control_border)
                            .bg(t.input_bg)
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(8.))
                                    .h(px(28.))
                                    .px(px(10.))
                                    .bg(t.sidebar)
                                    .border_b_1()
                                    .border_color(t.divider)
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.ink_secondary)
                                    .child(div().w(px(17.)).child(""))
                                    .child(div().flex_1().child("Name"))
                                    .child(div().w(px(72.)).child("Size")),
                            )
                            .child(list_view),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(px(136.))
                            .gap(px(6.))
                            .child(
                                push_button("mod-folder", "View Folder", ButtonKind::Accent, true, t)
                                    .w_full()
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_dir(mods_dir.clone(), true);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                push_button(
                                    "mod-toggle",
                                    toggle_label,
                                    ButtonKind::Standard,
                                    has_sel,
                                    t,
                                )
                                .w_full()
                                .when(has_sel, |b| {
                                    b.on_click(cx.listener(move |this, _, _, cx| {
                                        this.toggle_mod(&sel_toggle_name);
                                        cx.notify();
                                    }))
                                }),
                            )
                            .child(
                                push_button("mod-remove", "Remove", ButtonKind::Standard, has_sel, t)
                                    .w_full()
                                    .when(has_sel, |b| {
                                        b.on_click(cx.listener(|this, _, _, cx| {
                                            this.delete_selected_mod();
                                            cx.notify();
                                        }))
                                    }),
                            )
                            .child(div().flex_1())
                            .child(
                                push_button("mod-refresh", "Refresh", ButtonKind::Standard, true, t)
                                    .w_full()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reload_edit_mods();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                text_box(
                    "mod-search",
                    &self.edit.mod_filter,
                    "Filter installed mods…",
                    self.field == Some(Field::ModFilter),
                    true,
                    t,
                )
                .w_full()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        cx.stop_propagation();
                        this.field = Some(Field::ModFilter);
                        cx.notify();
                    }),
                ),
            )
            .into_any_element()
    }

    fn edit_worlds_page(
        &self,
        instance: &horizon_launcher::instances::Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut rows: Vec<AnyElement> = Vec::new();
        for (idx, world) in self.edit.worlds.iter().enumerate() {
            let selected = self.edit.selected_world.as_deref() == Some(world.folder.as_str());
            let folder = world.folder.clone();
            let hover = t.subtle_hover;
            rows.push(
                div()
                    .id(SharedString::from(format!("world-row-{idx}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(px(30.))
                    .px(px(10.))
                    .rounded(px(3.))
                    .bg(if selected {
                        t.tile_selected
                    } else {
                        gpui::transparent_black()
                    })
                    .text_size(px(12.))
                    .text_color(t.ink)
                    .cursor_pointer()
                    .when(!selected, |r| r.hover(move |s| s.bg(hover)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.edit.selected_world = Some(folder.clone());
                        cx.notify();
                    }))
                    .child(div().flex_1().min_w(px(0.)).truncate().child(world.name.clone()))
                    .child(
                        div()
                            .w(px(88.))
                            .text_color(t.ink_secondary)
                            .child(world.mode),
                    )
                    .child(
                        div()
                            .w(px(80.))
                            .text_color(t.ink_secondary)
                            .truncate()
                            .child(world.version.clone().unwrap_or_else(|| "—".into())),
                    )
                    .child(
                        div()
                            .w(px(72.))
                            .text_color(t.ink_secondary)
                            .child(format_bytes(world.size)),
                    )
                    .into_any_element(),
            );
        }

        let selected_path = self
            .edit
            .selected_world
            .as_deref()
            .and_then(|f| self.edit.worlds.iter().find(|w| w.folder == f))
            .map(|w| w.path.clone());
        let has_sel = selected_path.is_some();
        let saves_dir = instance.home.join("saves");

        let list_view: AnyElement = if rows.is_empty() {
            empty_table_note("No single-player worlds saved in this instance yet.", t)
        } else {
            div()
                .id("worlds-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .p(px(3.))
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .p(px(16.))
            .gap(px(10.))
            .child(page_title(&format!("Worlds ({})", self.edit.worlds.len()), t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .min_h(px(0.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(t.control_border)
                            .bg(t.input_bg)
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h(px(28.))
                                    .px(px(10.))
                                    .bg(t.sidebar)
                                    .border_b_1()
                                    .border_color(t.divider)
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.ink_secondary)
                                    .child(div().flex_1().child("Name"))
                                    .child(div().w(px(88.)).child("Game Mode"))
                                    .child(div().w(px(80.)).child("Version"))
                                    .child(div().w(px(72.)).child("Size")),
                            )
                            .child(list_view),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(px(136.))
                            .gap(px(6.))
                            .child(
                                push_button(
                                    "wld-open",
                                    "Open World",
                                    ButtonKind::Standard,
                                    has_sel,
                                    t,
                                )
                                .w_full()
                                .when_some(selected_path, |b, path| {
                                    b.on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_dir(path.clone(), false);
                                        cx.notify();
                                    }))
                                }),
                            )
                            .child(
                                push_button(
                                    "wld-saves",
                                    "View Folder",
                                    ButtonKind::Standard,
                                    true,
                                    t,
                                )
                                .w_full()
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.open_dir(saves_dir.clone(), true);
                                    cx.notify();
                                })),
                            )
                            .child(div().flex_1())
                            .child(
                                push_button("wld-refresh", "Refresh", ButtonKind::Standard, true, t)
                                    .w_full()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reload_edit_worlds();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }

    fn edit_log_page(
        &self,
        instance: &horizon_launcher::instances::Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let logs_dir = instance.home.join("logs");
        let latest = logs_dir.join("latest.log");
        let full_text = self.edit.log_lines.join("\n");

        div()
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .p(px(16.))
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(page_title("Minecraft Log", t))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                push_button(
                                    "log-copy",
                                    "Copy",
                                    ButtonKind::Standard,
                                    !self.edit.log_lines.is_empty(),
                                    t,
                                )
                                .when(!self.edit.log_lines.is_empty(), |b| {
                                    b.on_click(cx.listener(move |this, _, _, cx| {
                                        let _ = log::copy(&full_text);
                                        cx.write_to_clipboard(ClipboardItem::new_string(
                                            full_text.clone(),
                                        ));
                                        this.edit.notice =
                                            Some(("Copied log to clipboard.".into(), false));
                                        cx.notify();
                                    }))
                                }),
                            )
                            .child(
                                push_button("log-clear", "Clear", ButtonKind::Standard, true, t)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        let _ = std::fs::write(&latest, "");
                                        this.reload_edit_log();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                push_button("log-folder", "Logs Folder", ButtonKind::Standard, true, t)
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.open_dir(logs_dir.clone(), true);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                push_button("log-refresh", "Refresh", ButtonKind::Standard, true, t)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.reload_edit_log();
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(console_box("inst-log-scroll", &self.edit.log_lines, t))
            .into_any_element()
    }

    fn edit_settings_page(
        &self,
        instance: &horizon_launcher::instances::Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let is_java = instance.java.is_some();
        let inst_id = instance.id.clone();

        let swatches = div()
            .flex()
            .items_center()
            .gap(px(6.))
            .children(ICON_PALETTE.iter().enumerate().map(|(idx, raw)| {
                let fill = u32::from_str_radix(&raw[1..], 16)
                    .map(hex)
                    .unwrap_or(t.accent);
                let col_str = (*raw).to_owned();
                let id = inst_id.clone();
                div()
                    .id(SharedString::from(format!("ed-sw-{idx}")))
                    .size(px(22.))
                    .rounded(px(4.))
                    .bg(fill)
                    .border_1()
                    .border_color(t.control_border)
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.set_instance_colour(&id, &col_str);
                        cx.notify();
                    }))
            }));

        div()
            .id("ed-settings-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_h(px(0.))
            .p(px(16.))
            .gap(px(10.))
            .overflow_y_scroll()
            .child(page_title("Instance Settings", t))
            .child(form_row(
                "Name:",
                self.bound_box("ed-name", &self.edit.name, "Instance name", Field::EditName, t, cx)
                    .flex_1()
                    .into_any_element(),
                t,
            ))
            .child(form_row(
                "Group:",
                self.bound_box("ed-group", &self.edit.group, "Ungrouped", Field::EditGroup, t, cx)
                    .flex_1()
                    .into_any_element(),
                t,
            ))
            .child(form_row("Icon:", swatches.into_any_element(), t))
            .when(is_java, |col| {
                col.child(hsep(t))
                    .child(section_label("Java & Game Window", t))
                    .child(form_row(
                        "Max RAM:",
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .flex_1()
                            .child(
                                self.bound_box(
                                    "ed-mem",
                                    &self.edit.memory_mb,
                                    "4096",
                                    Field::EditMemory,
                                    t,
                                    cx,
                                )
                                .w(px(108.)),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(t.ink_secondary)
                                    .child("MiB"),
                            )
                            .children(["2048", "4096", "8192"].into_iter().map(|preset| {
                                push_button(
                                    format!("mem-{preset}"),
                                    preset,
                                    ButtonKind::Standard,
                                    true,
                                    t,
                                )
                                .min_w(px(54.))
                                .px(px(8.))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.edit.memory_mb = preset.to_owned();
                                    cx.notify();
                                }))
                            }))
                            .into_any_element(),
                        t,
                    ))
                    .child(form_row(
                        "JVM args:",
                        self.bound_box(
                            "ed-jvm",
                            &self.edit.jvm_args,
                            "Optional extra JVM flags",
                            Field::EditJvmArgs,
                            t,
                            cx,
                        )
                        .flex_1()
                        .into_any_element(),
                        t,
                    ))
                    .child(form_row(
                        "Window:",
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                self.bound_box(
                                    "ed-winw",
                                    &self.edit.win_w,
                                    "854",
                                    Field::EditWinW,
                                    t,
                                    cx,
                                )
                                .w(px(90.)),
                            )
                            .child(div().text_size(px(12.)).text_color(t.ink_secondary).child("×"))
                            .child(
                                self.bound_box(
                                    "ed-winh",
                                    &self.edit.win_h,
                                    "480",
                                    Field::EditWinH,
                                    t,
                                    cx,
                                )
                                .w(px(90.)),
                            )
                            .child(
                                div()
                                    .text_size(px(11.5))
                                    .text_color(t.ink_tertiary)
                                    .child("px (blank for default)"),
                            )
                            .into_any_element(),
                        t,
                    ))
            })
            .child(hsep(t))
            .child(section_label("Custom Commands", t))
            .child(form_row(
                "Pre-launch:",
                self.bound_box(
                    "ed-pre",
                    &self.edit.pre_launch,
                    "Command run before the game starts (optional)",
                    Field::EditPreLaunch,
                    t,
                    cx,
                )
                .flex_1()
                .into_any_element(),
                t,
            ))
            .child(form_row(
                "Wrapper:",
                self.bound_box(
                    "ed-wrap",
                    &self.edit.wrapper,
                    "Wrapper command (optional)",
                    Field::EditWrapper,
                    t,
                    cx,
                )
                .flex_1()
                .into_any_element(),
                t,
            ))
            .child(
                div().flex().justify_end().pt(px(4.)).child(
                    push_button("ed-save", "Save Settings", ButtonKind::Accent, true, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.save_edit_settings();
                            cx.notify();
                        }),
                    ),
                ),
            )
            .into_any_element()
    }

    // ----- Settings Dialog ----------------------------------------------------------------------

    fn dialog_settings(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let sidebar = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(174.))
            .p(px(10.))
            .gap(px(2.))
            .bg(t.sidebar)
            .child(
                nav_item(
                    "st-nav-ui",
                    "palette",
                    "User Interface",
                    self.settings_page == SettingsPage::Ui,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings_page = SettingsPage::Ui;
                    cx.notify();
                })),
            )
            .child(
                nav_item(
                    "st-nav-mc",
                    "cube",
                    "Minecraft",
                    self.settings_page == SettingsPage::Minecraft,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings_page = SettingsPage::Minecraft;
                    this.refresh_runtimes();
                    cx.notify();
                })),
            )
            .child(
                nav_item(
                    "st-nav-launcher",
                    "settings",
                    "Launcher",
                    self.settings_page == SettingsPage::Launcher,
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.settings_page = SettingsPage::Launcher;
                    cx.notify();
                })),
            );

        let page: AnyElement = match self.settings_page {
            SettingsPage::Ui => self.settings_ui_page(t, cx),
            SettingsPage::Minecraft => self.settings_minecraft_page(t, cx),
            SettingsPage::Launcher => self.settings_launcher_page(t, cx),
        };

        let body = div()
            .flex()
            .w_full()
            .h(px(430.))
            .child(sidebar)
            .child(div().flex_none().w(px(1.)).h_full().bg(t.divider))
            .child(page)
            .into_any_element();

        let footer = div().child(div()).child(
            push_button("st-close", "Close", ButtonKind::Accent, true, t).on_click(cx.listener(
                |this, _, _, cx| {
                    this.close_dialog();
                    cx.notify();
                },
            )),
        );

        dialog_shell(680., body, footer, t)
    }

    fn settings_ui_page(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        div()
            .id("st-ui-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .p(px(18.))
            .gap(px(14.))
            .bg(t.layer)
            .overflow_y_scroll()
            .child(page_title("User Interface", t))
            .child(
                settings_card(
                    "Windows 11 Theme",
                    "WinUI 3 widget styling matching JLac's Windows 11 style.",
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(
                            ThemeMode::ALL
                                .into_iter()
                                .enumerate()
                                .map(|(idx, mode)| {
                                    push_button(
                                        format!("st-mode-{idx}"),
                                        mode.label(),
                                        if self.prefs.theme == mode {
                                            ButtonKind::Accent
                                        } else {
                                            ButtonKind::Standard
                                        },
                                        true,
                                        t,
                                    )
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.prefs.theme = mode;
                                        this.prefs.save();
                                        cx.notify();
                                    }))
                                }),
                        )
                        .into_any_element(),
                    t,
                ),
            )
            .child(
                settings_card(
                    "Accent Colour",
                    "Applied to primary actions, focused inputs and selected navigation items.",
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(Accent::ALL.iter().copied().enumerate().map(|(idx, acc)| {
                            let (swatch_rgb, _, _) = acc.colours(t.dark);
                            let selected = self.prefs.accent == acc;
                            let hover = t.control_hover;
                            div()
                                .id(SharedString::from(format!("st-acc-{idx}")))
                                .flex()
                                .items_center()
                                .gap(px(7.))
                                .h(px(30.))
                                .px(px(10.))
                                .rounded(px(4.))
                                .bg(if selected {
                                    t.tile_selected
                                } else {
                                    t.control
                                })
                                .border_1()
                                .border_color(if selected {
                                    t.accent
                                } else {
                                    t.control_border
                                })
                                .text_size(px(12.))
                                .text_color(t.ink)
                                .cursor_pointer()
                                .when(!selected, |b| b.hover(move |s| s.bg(hover)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.prefs.accent = acc;
                                    this.prefs.save();
                                    cx.notify();
                                }))
                                .child(
                                    div()
                                        .size(px(12.))
                                        .rounded(px(6.))
                                        .bg(hex(swatch_rgb)),
                                )
                                .child(acc.label())
                        }))
                        .into_any_element(),
                    t,
                ),
            )
            .child(
                settings_card(
                    "Instance View",
                    "Sorting order and bottom status bar visibility.",
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .children(
                                    [SortMode::Name, SortMode::LastLaunched]
                                        .into_iter()
                                        .enumerate()
                                        .map(|(idx, sort)| {
                                            push_button(
                                                format!("st-sort-{idx}"),
                                                sort.label(),
                                                if self.prefs.sort == sort {
                                                    ButtonKind::Accent
                                                } else {
                                                    ButtonKind::Standard
                                                },
                                                true,
                                                t,
                                            )
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.prefs.sort = sort;
                                                this.prefs.save();
                                                this.reload_instances();
                                                cx.notify();
                                            }))
                                        }),
                                ),
                        )
                        .child(
                            check_box(
                                "st-statusbar",
                                "Show status bar",
                                self.prefs.status_bar,
                                t,
                            )
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.prefs.status_bar = !this.prefs.status_bar;
                                this.prefs.save();
                                cx.notify();
                            })),
                        )
                        .into_any_element(),
                    t,
                ),
            )
            .into_any_element()
    }

    fn settings_minecraft_page(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let rt_list: AnyElement = if self.runtimes.is_empty() {
            div()
                .text_size(px(12.))
                .text_color(t.ink_secondary)
                .child(
                    "No local Java runtime detected yet. FNH automatically downloads the matching Mojang Java runtime the first time a Java or Fabric instance launches.",
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .children(self.runtimes.iter().take(6).map(|rt| {
                    let src = match rt.source {
                        java::Source::Mojang => "Mojang",
                        java::Source::Installed => "System",
                    };
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(10.))
                        .text_size(px(12.))
                        .child(
                            div()
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_color(t.ink)
                                .child(format!("Java {} ({}, {})", rt.major, rt.version, src)),
                        )
                        .child(
                            div()
                                .truncate()
                                .text_size(px(11.5))
                                .text_color(t.ink_secondary)
                                .child(look::short_path(&rt.home.display().to_string(), 42)),
                        )
                }))
                .into_any_element()
        };

        div()
            .id("st-mc-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .p(px(18.))
            .gap(px(14.))
            .bg(t.layer)
            .overflow_y_scroll()
            .child(page_title("Minecraft & Java", t))
            .child(settings_card(
                "Detected Java Installations",
                "Per-instance memory and JVM flags can be customized in Edit... -> Settings.",
                div()
                    .flex()
                    .flex_col()
                    .gap(px(10.))
                    .child(rt_list)
                    .child(
                        div().flex().gap(px(8.)).child(
                            push_button("st-rt-refresh", "Re-scan Java", ButtonKind::Standard, true, t)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh_runtimes();
                                    cx.notify();
                                })),
                        ),
                    )
                    .into_any_element(),
                t,
            ))
            .child(settings_card(
                "Fabric Loader",
                "Inspect available Fabric Loader releases from meta.fabricmc.net.",
                div()
                    .flex()
                    .gap(px(8.))
                    .child(
                        push_button(
                            "st-open-fab",
                            "Fabric Versions...",
                            ButtonKind::Standard,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.open_fabric_versions(None);
                            cx.notify();
                        })),
                    )
                    .into_any_element(),
                t,
            ))
            .into_any_element()
    }

    fn settings_launcher_page(&self, t: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let data_dir = backend::data_dir();
        let game_dir = backend::game_home();
        let data_label = look::short_path(&data_dir.display().to_string(), 52);
        let game_label = look::short_path(&game_dir.display().to_string(), 52);

        div()
            .id("st-ln-scroll")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .p(px(18.))
            .gap(px(14.))
            .bg(t.layer)
            .overflow_y_scroll()
            .child(page_title("Launcher", t))
            .child(settings_card(
                "Folders",
                &format!("Launcher: {data_label}  ·  Instances: {game_label}"),
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(
                        push_button("st-d-open", "Launcher Folder", ButtonKind::Standard, true, t)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_dir(data_dir.clone(), true);
                                cx.notify();
                            })),
                    )
                    .child(
                        push_button("st-g-open", "Instances Folder", ButtonKind::Standard, true, t)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_dir(game_dir.clone(), true);
                                cx.notify();
                            })),
                    )
                    .child(
                        push_button(
                            "st-clear-meta",
                            "Clear Metadata Cache",
                            ButtonKind::Standard,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.clear_metadata_cache();
                            cx.notify();
                        })),
                    )
                    .into_any_element(),
                t,
            ))
            .child(settings_card(
                "Factory Reset Shortcut",
                "Hold Ctrl + Shift + Alt while clicking \"Add Instance...\" to erase every instance, account, world, mod, cache and setting, then immediately relaunch FNH as a fresh install.",
                div()
                    .text_size(px(11.5))
                    .text_color(t.ink_tertiary)
                    .child(format!("FNH v{VERSION} · Windows 11 UI"))
                    .into_any_element(),
                t,
            ))
            .into_any_element()
    }

    // ----- Accounts Dialog (JLac AccountListPage) -----------------------------------------------

    fn dialog_accounts(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let active_uuid = self.accounts.selected.as_deref();
        let rows: Vec<AnyElement> = self
            .accounts
            .accounts
            .iter()
            .enumerate()
            .map(|(idx, acc)| {
                let is_sel = self.selected_account.as_deref() == Some(acc.uuid.as_str());
                let is_default = active_uuid == Some(acc.uuid.as_str());
                let uuid = acc.uuid.clone();
                let hover = t.subtle_hover;
                div()
                    .id(SharedString::from(format!("acc-row-{idx}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(px(32.))
                    .px(px(10.))
                    .rounded(px(3.))
                    .bg(if is_sel {
                        t.tile_selected
                    } else {
                        gpui::transparent_black()
                    })
                    .text_size(px(12.5))
                    .text_color(t.ink)
                    .cursor_pointer()
                    .when(!is_sel, |r| r.hover(move |s| s.bg(hover)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.selected_account = Some(uuid.clone());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .flex_1()
                            .min_w(px(0.))
                            .child(icon(
                                if is_default { "check" } else { "account" },
                                15.,
                                if is_default { t.accent } else { t.ink_secondary },
                            ))
                            .child(
                                div()
                                    .truncate()
                                    .font_weight(if is_default {
                                        FontWeight::SEMIBOLD
                                    } else {
                                        FontWeight::NORMAL
                                    })
                                    .child(acc.name.clone()),
                            ),
                    )
                    .child(
                        div()
                            .w(px(90.))
                            .text_size(px(11.5))
                            .text_color(if is_default {
                                t.accent
                            } else {
                                t.ink_secondary
                            })
                            .child(if is_default { "Default" } else { "Ready" }),
                    )
                    .into_any_element()
            })
            .collect();

        let pick = self.selected_account.clone();
        let has_pick = pick
            .as_deref()
            .is_some_and(|u| self.accounts.accounts.iter().any(|a| a.uuid == u));
        let pick_set = pick.clone().unwrap_or_default();
        let pick_del = pick.unwrap_or_default();

        let table: AnyElement = if rows.is_empty() {
            empty_table_note(
                "No Microsoft accounts added yet. Click \"Add Microsoft\" to sign in.",
                t,
            )
        } else {
            div()
                .id("acc-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .p(px(3.))
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        let body = div()
            .flex()
            .flex_col()
            .w_full()
            .h(px(330.))
            .p(px(18.))
            .gap(px(12.))
            .child(page_title("Accounts", t))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .gap(px(12.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w(px(0.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(t.control_border)
                            .bg(t.input_bg)
                            .overflow_hidden()
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .h(px(28.))
                                    .px(px(10.))
                                    .bg(t.sidebar)
                                    .border_b_1()
                                    .border_color(t.divider)
                                    .text_size(px(11.5))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .text_color(t.ink_secondary)
                                    .child(div().flex_1().child("Account"))
                                    .child(div().w(px(90.)).child("Status")),
                            )
                            .child(table),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_none()
                            .w(px(142.))
                            .gap(px(6.))
                            .child(
                                push_button(
                                    "acc-add",
                                    "Add Microsoft",
                                    ButtonKind::Accent,
                                    !self.status.busy,
                                    t,
                                )
                                .w_full()
                                .when(!self.status.busy, |b| {
                                    b.on_click(cx.listener(|this, _, _, cx| {
                                        this.start_microsoft_sign_in();
                                        cx.notify();
                                    }))
                                }),
                            )
                            .child(
                                push_button(
                                    "acc-default",
                                    "Set Default",
                                    ButtonKind::Standard,
                                    has_pick,
                                    t,
                                )
                                .w_full()
                                .when(has_pick, |b| {
                                    b.on_click(cx.listener(move |this, _, _, cx| {
                                        this.switch_account(&pick_set);
                                        cx.notify();
                                    }))
                                }),
                            )
                            .child(
                                push_button("acc-remove", "Remove", ButtonKind::Standard, has_pick, t)
                                    .w_full()
                                    .when(has_pick, |b| {
                                        b.on_click(cx.listener(move |this, _, _, cx| {
                                            this.remove_account(&pick_del);
                                            cx.notify();
                                        }))
                                    }),
                            ),
                    ),
            )
            .into_any_element();

        let footer = div().child(div()).child(
            push_button("acc-close", "Close", ButtonKind::Standard, true, t).on_click(cx.listener(
                |this, _, _, cx| {
                    this.close_dialog();
                    cx.notify();
                },
            )),
        );

        dialog_shell(560., body, footer, t)
    }

    // ----- Microsoft Sign-In Dialog (JLac MSALoginDialog) ---------------------------------------

    fn dialog_sign_in(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let (code, link) = self
            .status
            .code
            .clone()
            .unwrap_or_else(|| (String::new(), "https://www.microsoft.com/link".into()));
        let full_url = log::sign_in_url(&link, &code);
        let code_copy = code.clone();
        let code_open = code.clone();

        let body = div()
            .flex()
            .flex_col()
            .p(px(22.))
            .gap(px(12.))
            .child(page_title("Add Microsoft Account", t))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(if self.status.failed {
                        t.danger
                    } else {
                        t.ink_secondary
                    })
                    .child(if !code.is_empty() {
                        format!(
                            "Open {link} in your browser and enter the code below (already copied to your clipboard):"
                        )
                    } else if !self.status.line.is_empty() {
                        self.status.line.clone()
                    } else {
                        "Requesting a one-time code from Microsoft…".into()
                    }),
            )
            .when(!code.is_empty(), |col| {
                col.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .py(px(14.))
                        .px(px(16.))
                        .rounded(px(6.))
                        .bg(t.control)
                        .border_1()
                        .border_color(t.control_border)
                        .font_family(MONO_FONT)
                        .text_size(px(22.))
                        .font_weight(FontWeight::BOLD)
                        .text_color(t.accent)
                        .child(code.clone()),
                )
            })
            .into_any_element();

        let has_code = !code.is_empty();
        let footer = div()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when(has_code, |row| {
                        row.child(
                            push_button(
                                "msa-open",
                                "Open Page & Copy Code",
                                ButtonKind::Accent,
                                true,
                                t,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                let _ = log::copy(&code_open);
                                cx.write_to_clipboard(ClipboardItem::new_string(
                                    code_open.clone(),
                                ));
                                this.open_web(&full_url);
                                cx.notify();
                            })),
                        )
                        .child(
                            push_button("msa-copy", "Copy Code", ButtonKind::Standard, true, t)
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    let _ = log::copy(&code_copy);
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        code_copy.clone(),
                                    ));
                                    cx.notify();
                                })),
                        )
                    }),
            )
            .child(
                push_button("msa-close", "Close", ButtonKind::Standard, true, t).on_click(
                    cx.listener(|this, _, _, cx| {
                        if this.status.busy {
                            let _ = job::kill();
                        }
                        this.close_dialog();
                        cx.notify();
                    }),
                ),
            );

        dialog_shell(480., body, footer, t)
    }

    // ----- Fabric Versions Dialog (JLac FabricVersionsDialog) -----------------------------------

    fn dialog_fabric_versions(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let rows: Vec<AnyElement> = self
            .fabric_dlg
            .versions
            .iter()
            .take(120)
            .enumerate()
            .map(|(idx, item)| {
                let selected =
                    self.fabric_dlg.selected.as_deref() == Some(item.version.as_str());
                let ver = item.version.clone();
                let hover = t.subtle_hover;
                div()
                    .id(SharedString::from(format!("fab-row-{idx}")))
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(px(28.))
                    .px(px(10.))
                    .rounded(px(3.))
                    .bg(if selected {
                        t.tile_selected
                    } else {
                        gpui::transparent_black()
                    })
                    .text_size(px(12.))
                    .text_color(t.ink)
                    .cursor_pointer()
                    .when(!selected, |r| r.hover(move |s| s.bg(hover)))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.fabric_dlg.selected = Some(ver.clone());
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex_1()
                            .font_weight(if selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .child(item.version.clone()),
                    )
                    .child(
                        div()
                            .w(px(90.))
                            .text_color(if item.stable {
                                t.success
                            } else {
                                t.ink_secondary
                            })
                            .child(if item.stable { "Stable" } else { "Beta" }),
                    )
                    .into_any_element()
            })
            .collect();

        let table_body: AnyElement = if self.fabric_dlg.loading {
            empty_table_note("Loading Fabric Loader versions…", t)
        } else if let Some(err) = &self.fabric_dlg.error {
            empty_table_note(err, t)
        } else if rows.is_empty() {
            empty_table_note("Enter a Minecraft version above and click Refresh.", t)
        } else {
            div()
                .id("fab-scroll")
                .flex()
                .flex_col()
                .flex_1()
                .min_h(px(0.))
                .p(px(3.))
                .overflow_y_scroll()
                .children(rows)
                .into_any_element()
        };

        let body = div()
            .flex()
            .flex_col()
            .w_full()
            .h(px(390.))
            .p(px(18.))
            .gap(px(10.))
            .child(page_title("Fabric Loader Versions", t))
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
                        self.bound_box(
                            "fab-mc",
                            &self.fabric_dlg.mc_version,
                            "e.g. 1.21.4",
                            Field::FabricMcVersion,
                            t,
                            cx,
                        )
                        .flex_1(),
                    )
                    .child(
                        push_button(
                            "fab-load",
                            "Refresh",
                            ButtonKind::Standard,
                            !self.fabric_dlg.loading,
                            t,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.fetch_fabric_versions();
                            cx.notify();
                        })),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(t.control_border)
                    .bg(t.input_bg)
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .h(px(28.))
                            .px(px(10.))
                            .bg(t.sidebar)
                            .border_b_1()
                            .border_color(t.divider)
                            .text_size(px(11.5))
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(t.ink_secondary)
                            .child(div().flex_1().child("Loader Version"))
                            .child(div().w(px(90.)).child("Stability")),
                    )
                    .child(table_body),
            )
            .into_any_element();

        let sel = self.fabric_dlg.selected.clone();
        let target = self.fabric_dlg.target_instance.clone();
        let back_to_edit = target.is_some();

        let footer = div()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .when_some(sel.clone(), |row, chosen| {
                        if let Some(inst_id) = target {
                            let ver = chosen.clone();
                            row.child(
                                push_button(
                                    "fab-apply",
                                    format!("Install {chosen}"),
                                    ButtonKind::Accent,
                                    !self.edit.working,
                                    t,
                                )
                                .when(!self.edit.working, |b| {
                                    b.on_click(cx.listener(move |this, _, _, cx| {
                                        this.install_fabric_on(&inst_id, Some(ver.clone()));
                                        cx.notify();
                                    }))
                                }),
                            )
                        } else {
                            row.child(
                                push_button(
                                    "fab-copy",
                                    "Copy Version",
                                    ButtonKind::Standard,
                                    true,
                                    t,
                                )
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    let _ = log::copy(&chosen);
                                    cx.write_to_clipboard(ClipboardItem::new_string(
                                        chosen.clone(),
                                    ));
                                    this.say(format!("Copied Fabric Loader {chosen}."), false);
                                    cx.notify();
                                })),
                            )
                        }
                    }),
            )
            .child(
                push_button("fab-close", "Close", ButtonKind::Standard, true, t).on_click(
                    cx.listener(move |this, _, _, cx| {
                        if back_to_edit {
                            this.dialog = Some(Dialog::EditInstance);
                            this.field = None;
                        } else {
                            this.close_dialog();
                        }
                        cx.notify();
                    }),
                ),
            );

        dialog_shell(520., body, footer, t)
    }

    // ----- Launcher Log Dialog ------------------------------------------------------------------

    fn dialog_launcher_log(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let full = self.launcher_log.join("\n");
        let body = div()
            .flex()
            .flex_col()
            .w_full()
            .h(px(400.))
            .p(px(18.))
            .gap(px(10.))
            .child(page_title("Launcher Log (fh.log)", t))
            .child(console_box("fh-log-scroll", &self.launcher_log, t))
            .into_any_element();

        let footer = div()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        push_button(
                            "ll-copy",
                            "Copy",
                            ButtonKind::Standard,
                            !self.launcher_log.is_empty(),
                            t,
                        )
                        .when(!self.launcher_log.is_empty(), |b| {
                            b.on_click(cx.listener(move |this, _, _, cx| {
                                let _ = log::copy(&full);
                                cx.write_to_clipboard(ClipboardItem::new_string(full.clone()));
                                this.say("Copied fh.log to clipboard.", false);
                                cx.notify();
                            }))
                        }),
                    )
                    .child(
                        push_button("ll-folder", "Open Folder", ButtonKind::Standard, true, t)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_dir(backend::data_dir().join("fh"), true);
                                cx.notify();
                            })),
                    )
                    .child(
                        push_button("ll-refresh", "Refresh", ButtonKind::Standard, true, t)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.launcher_log = crate::app::read_tail(log::path(), 400);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                push_button("ll-close", "Close", ButtonKind::Accent, true, t).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.close_dialog();
                        cx.notify();
                    }),
                ),
            );

        dialog_shell(660., body, footer, t)
    }

    // ----- Small WinUI 3 ContentDialogs (Rename / Group / Copy / Delete / About) ----------------

    #[allow(clippy::too_many_arguments)]
    fn dialog_prompt(
        &self,
        title: &'static str,
        label: &'static str,
        placeholder: &'static str,
        confirm: &'static str,
        allow_empty: bool,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let can_ok = allow_empty || !self.prompt_text.trim().is_empty();
        let body = div()
            .flex()
            .flex_col()
            .p(px(22.))
            .gap(px(10.))
            .child(page_title(title, t))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(t.ink_secondary)
                    .child(label),
            )
            .child(
                self.bound_box("prompt-in", &self.prompt_text, placeholder, Field::PromptText, t, cx)
                    .w_full(),
            )
            .when_some(self.prompt_error.clone(), |col, err| {
                col.child(div().text_size(px(12.)).text_color(t.danger).child(err))
            })
            .into_any_element();

        let footer = div().child(div()).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    push_button("pr-ok", confirm, ButtonKind::Accent, can_ok, t).when(
                        can_ok,
                        |b| {
                            b.on_click(cx.listener(|this, _, _, cx| {
                                this.submit_prompt();
                                cx.notify();
                            }))
                        },
                    ),
                )
                .child(
                    push_button("pr-cancel", "Cancel", ButtonKind::Standard, true, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.close_dialog();
                            cx.notify();
                        }),
                    ),
                ),
        );

        dialog_shell(420., body, footer, t)
    }

    fn dialog_change_group(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let groups = self.existing_groups();
        let body = div()
            .flex()
            .flex_col()
            .p(px(22.))
            .gap(px(10.))
            .child(page_title("Change Group", t))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(t.ink_secondary)
                    .child("Enter a group name, or leave blank to ungroup:"),
            )
            .child(
                self.bound_box(
                    "grp-in",
                    &self.prompt_text,
                    "No group (Ungrouped)",
                    Field::PromptText,
                    t,
                    cx,
                )
                .w_full(),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(6.))
                    .child(
                        push_button("grp-none", "Ungrouped", ButtonKind::Standard, true, t)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.prompt_text.clear();
                                cx.notify();
                            })),
                    )
                    .children(groups.into_iter().enumerate().map(|(idx, g)| {
                        let pick = g.clone();
                        push_button(format!("grp-chip-{idx}"), g, ButtonKind::Standard, true, t)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.prompt_text = pick.clone();
                                cx.notify();
                            }))
                    })),
            )
            .when_some(self.prompt_error.clone(), |col, err| {
                col.child(div().text_size(px(12.)).text_color(t.danger).child(err))
            })
            .into_any_element();

        let footer = div().child(div()).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    push_button("grp-ok", "OK", ButtonKind::Accent, true, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.submit_prompt();
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    push_button("grp-cancel", "Cancel", ButtonKind::Standard, true, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.close_dialog();
                            cx.notify();
                        }),
                    ),
                ),
        );

        dialog_shell(440., body, footer, t)
    }

    fn dialog_confirm_delete(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let body = div()
            .flex()
            .flex_col()
            .p(px(22.))
            .gap(px(10.))
            .child(page_title("Delete Instance", t))
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(t.ink)
                    .child(format!(
                        "Are you sure you want to delete \"{}\" and all of its saves, mods and settings?",
                        self.prompt_text
                    )),
            )
            .when_some(self.prompt_error.clone(), |col, err| {
                col.child(div().text_size(px(12.)).text_color(t.danger).child(err))
            })
            .into_any_element();

        let footer = div().child(div()).child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    push_button("del-ok", "Delete", ButtonKind::Danger, true, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.submit_prompt();
                            cx.notify();
                        }),
                    ),
                )
                .child(
                    push_button("del-cancel", "Cancel", ButtonKind::Standard, true, t).on_click(
                        cx.listener(|this, _, _, cx| {
                            this.close_dialog();
                            cx.notify();
                        }),
                    ),
                ),
        );

        dialog_shell(440., body, footer, t)
    }

    fn dialog_about(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let body = div()
            .flex()
            .flex_col()
            .p(px(22.))
            .gap(px(14.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(14.))
                    .child(brand_mark(56.))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(px(18.))
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(t.ink)
                                    .child(format!("FNH {VERSION}")),
                            )
                            .child(
                                div()
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("Fabric n Horizon — native Windows 11 launcher"),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(px(12.5))
                    .text_color(t.ink)
                    .child(
                        "Manage and launch your Fabric, Minecraft: Java Edition and Horizon instances from a clean Windows 11 interface inspired by JLac.",
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(8.))
                    .child(
                        push_button("ab-site", "Website", ButtonKind::Standard, true, t).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.open_web(SITE_URL);
                                cx.notify();
                            }),
                        ),
                    )
                    .child(
                        push_button("ab-repo", "Source Code", ButtonKind::Standard, true, t)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_web(REPO_URL);
                                cx.notify();
                            })),
                    )
                    .child(
                        push_button("ab-rel", "Releases", ButtonKind::Standard, true, t).on_click(
                            cx.listener(|this, _, _, cx| {
                                this.open_web(RELEASES_URL);
                                cx.notify();
                            }),
                        ),
                    ),
            )
            .child(
                div()
                    .text_size(px(11.))
                    .text_color(t.ink_tertiary)
                    .child(
                        "MIT Licence. Not an official Minecraft product. Not approved by or associated with Mojang or Microsoft.",
                    ),
            )
            .into_any_element();

        let footer = div().child(div()).child(
            push_button("ab-close", "Close", ButtonKind::Accent, true, t).on_click(cx.listener(
                |this, _, _, cx| {
                    this.close_dialog();
                    cx.notify();
                },
            )),
        );

        dialog_shell(480., body, footer, t)
    }

    // ----- Field binder -------------------------------------------------------------------------

    fn bound_box(
        &self,
        id: impl Into<SharedString>,
        value: &str,
        placeholder: &str,
        field: Field,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        text_box(id, value, placeholder, self.field == Some(field), false, t).on_mouse_down(
            MouseButton::Left,
            cx.listener(move |this, _, _, cx| {
                cx.stop_propagation();
                this.field = Some(field);
                cx.notify();
            }),
        )
    }
}

// ----- Shared dialog pieces ---------------------------------------------------------------------

fn page_title(title: &str, t: &Theme) -> Div {
    div()
        .text_size(px(16.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.ink)
        .child(title.to_owned())
}

fn section_label(title: &str, t: &Theme) -> Div {
    div()
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.ink)
        .child(title.to_owned())
}

fn form_row(label: &'static str, control: AnyElement, t: &Theme) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .child(
            div()
                .w(px(74.))
                .flex_none()
                .text_size(px(12.5))
                .text_color(t.ink_secondary)
                .child(label),
        )
        .child(control)
}

fn component_row(name: &str, uid: &str, version: &str, primary: bool, t: &Theme) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .h(px(30.))
        .px(px(10.))
        .rounded(px(3.))
        .text_size(px(12.))
        .text_color(t.ink)
        .child(
            div()
                .flex_1()
                .font_weight(if primary {
                    FontWeight::SEMIBOLD
                } else {
                    FontWeight::NORMAL
                })
                .child(name.to_owned()),
        )
        .child(
            div()
                .w(px(165.))
                .truncate()
                .text_color(t.ink_secondary)
                .child(uid.to_owned()),
        )
        .child(
            div()
                .w(px(110.))
                .truncate()
                .text_color(t.ink)
                .child(version.to_owned()),
        )
        .into_any_element()
}

fn settings_card(title: &str, subtitle: &str, content: AnyElement, t: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .p(px(14.))
        .rounded(px(6.))
        .bg(t.control)
        .border_1()
        .border_color(t.control_border)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.ink)
                        .child(title.to_owned()),
                )
                .child(
                    div()
                        .text_size(px(11.5))
                        .text_color(t.ink_secondary)
                        .child(subtitle.to_owned()),
                ),
        )
        .child(content)
}

fn empty_table_note(text: &str, t: &Theme) -> AnyElement {
    div()
        .flex()
        .flex_1()
        .items_center()
        .justify_center()
        .p(px(20.))
        .text_center()
        .text_size(px(12.))
        .text_color(t.ink_secondary)
        .child(text.to_owned())
        .into_any_element()
}

fn console_box(id: &'static str, lines: &[String], t: &Theme) -> AnyElement {
    if lines.is_empty() {
        return div()
            .flex()
            .flex_1()
            .min_h(px(0.))
            .items_center()
            .justify_center()
            .rounded(px(4.))
            .border_1()
            .border_color(t.control_border)
            .bg(t.input_bg)
            .text_size(px(12.))
            .text_color(t.ink_secondary)
            .child("Log is empty.")
            .into_any_element();
    }
    div()
        .id(id)
        .flex()
        .flex_col()
        .flex_1()
        .min_h(px(0.))
        .p(px(10.))
        .gap(px(1.))
        .rounded(px(4.))
        .border_1()
        .border_color(t.control_border)
        .bg(t.input_bg)
        .font_family(MONO_FONT)
        .text_size(px(11.5))
        .line_height(px(16.))
        .text_color(t.ink)
        .overflow_y_scroll()
        .children(lines.iter().map(|line| div().child(line.clone())))
        .into_any_element()
}
