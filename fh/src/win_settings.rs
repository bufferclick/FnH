//! Standalone global `Settings` window (`"Settings - FNH"`), modeled on JLac's `PageDialog` with
//! all 9 sidebar pages: Launcher, Appearance, Minecraft, Horizon, Java, Accounts, APIs, External
//! Tools and Proxy. Changes persist immediately and update every open FNH window in real time.

use crate::app::Fh;
use crate::look::{
    accent_button, apply_text_key, checkbox_row, chip, danger_button, group_box, hex,
    log_line_color, page_nav_row, push_button, side_action_button, status_banner, sub_tab,
    text_input, Theme, MONO_FONT, UI_FONT,
};
use crate::prefs::{
    Accent, GpuBackend, GpuPower, Prefs, ProxyMode, SortMode, ThemeMode, UpdateChannel, ViewMode,
    JVM_FLAG_PRESETS, MEMORY_PRESETS_MB, WINDOW_PRESETS,
};
use crate::win_dialogs::secondary_window_options;
use crate::{inst_icons, job, log, paths, VERSION};
use gpui::{
    div, prelude::*, px, App, ClickEvent, ClipboardItem, Context, Entity, FocusHandle, FontWeight,
    InteractiveElement, KeyDownEvent, PathPromptOptions, Render, SharedString,
    StatefulInteractiveElement, Window, WindowHandle,
};
use horizon_launcher::{account, backend, java};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SettingsPage {
    Launcher,
    Appearance,
    Minecraft,
    Horizon,
    Java,
    Accounts,
    Apis,
    ExternalTools,
    Proxy,
}

impl SettingsPage {
    const ALL: [SettingsPage; 9] = [
        SettingsPage::Launcher,
        SettingsPage::Appearance,
        SettingsPage::Minecraft,
        SettingsPage::Horizon,
        SettingsPage::Java,
        SettingsPage::Accounts,
        SettingsPage::Apis,
        SettingsPage::ExternalTools,
        SettingsPage::Proxy,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsPage::Launcher => "Launcher",
            SettingsPage::Appearance => "Appearance",
            SettingsPage::Minecraft => "Minecraft",
            SettingsPage::Horizon => "Horizon",
            SettingsPage::Java => "Java",
            SettingsPage::Accounts => "Accounts",
            SettingsPage::Apis => "APIs",
            SettingsPage::ExternalTools => "External Tools",
            SettingsPage::Proxy => "Proxy",
        }
    }

    fn glyph(self) -> &'static str {
        match self {
            SettingsPage::Launcher => "settings",
            SettingsPage::Appearance => "palette",
            SettingsPage::Minecraft => "cube",
            SettingsPage::Horizon => "horizon",
            SettingsPage::Java => "java",
            SettingsPage::Accounts => "account",
            SettingsPage::Apis => "key",
            SettingsPage::ExternalTools => "tools",
            SettingsPage::Proxy => "network",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Field {
    None,
    UpdateHours,
    ConcDownloads,
    ConcTasks,
    HttpTimeout,
    NetRetries,
    LogLimit,
    WinWidth,
    WinHeight,
    PreLaunch,
    Wrapper,
    PostExit,
    EnvKey,
    EnvVal,
    GlfwPath,
    OpenAlPath,
    JavaMinMem,
    JavaMaxMem,
    JavaPermGen,
    JavaPath,
    JvmArgs,
    HzRustLog,
    HzExtraArgs,
    OfflineName,
    UserAgent,
    ModrinthToken,
    TextEditor,
    Profiler,
    WorldTool,
    ProxyHost,
    ProxyPort,
    ProxyUser,
    ProxyPass,
}

pub struct SettingsWindow {
    app: Entity<Fh>,
    focus: FocusHandle,
    page: SettingsPage,
    tab: usize,
    field: Field,
    num_buf: String,
    env_key: String,
    env_val: String,
    selected_account: usize,
    runtimes: Vec<java::Runtime>,
    notice: Option<(String, bool)>,
}

impl SettingsWindow {
    pub fn open(
        app: Entity<Fh>,
        initial: SettingsPage,
        cx: &mut App,
    ) -> Option<WindowHandle<Self>> {
        let opts = secondary_window_options("Settings - FNH", 940., 660., 740., 500., cx);
        cx.open_window(opts, move |window, cx| {
            crate::look::apply_window_icon();
            cx.new(|cx| {
                cx.observe(&app, |_: &mut Self, _, cx| cx.notify()).detach();
                let focus = cx.focus_handle();
                window.focus(&focus, cx);
                let runtimes = collect_runtimes();
                cx.spawn(async move |this, cx| loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(300))
                        .await;
                    if this.update(cx, |_, cx| cx.notify()).is_err() {
                        break;
                    }
                })
                .detach();
                Self {
                    app,
                    focus,
                    page: initial,
                    tab: 0,
                    field: Field::None,
                    num_buf: String::new(),
                    env_key: String::new(),
                    env_val: String::new(),
                    selected_account: 0,
                    runtimes,
                    notice: None,
                }
            })
        })
        .ok()
    }

    pub fn set_page(&mut self, page: SettingsPage, cx: &mut Context<Self>) {
        self.page = page;
        self.tab = 0;
        self.field = Field::None;
        self.notice = None;
        cx.notify();
    }

    fn mutate_prefs(&mut self, cx: &mut Context<Self>, f: impl FnOnce(&mut Prefs)) {
        self.app.update(cx, |app, cx| {
            f(&mut app.prefs);
            app.prefs.save();
            app.sort_instances();
            cx.notify();
        });
        cx.notify();
    }

    fn focus_num(&mut self, field: Field, current: impl ToString, cx: &mut Context<Self>) {
        self.field = field;
        self.num_buf = current.to_string();
        cx.notify();
    }

    fn num_text<'a>(&'a self, field: Field, fallback: String) -> String {
        if self.field == field {
            self.num_buf.clone()
        } else {
            fallback
        }
    }

    fn browse_path(&mut self, target: Field, cx: &mut Context<Self>) {
        let prompt = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select File".into()),
        });
        cx.spawn(async move |this, cx| {
            let Ok(Ok(Some(paths))) = prompt.await else {
                return;
            };
            let Some(chosen) = paths.into_iter().next() else {
                return;
            };
            let text = chosen.to_string_lossy().into_owned();
            let _ = this.update(cx, |this, cx| {
                this.mutate_prefs(cx, |p| match target {
                    Field::JavaPath => p.default_java_path = text,
                    Field::GlfwPath => p.custom_glfw_path = text,
                    Field::OpenAlPath => p.custom_openal_path = text,
                    Field::TextEditor => p.text_editor_cmd = text,
                    Field::Profiler => p.profiler_cmd = text,
                    Field::WorldTool => p.world_tool_cmd = text,
                    _ => {}
                });
            });
        })
        .detach();
    }

    fn handle_key(&mut self, ev: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        if ev.keystroke.key.as_str() == "escape" {
            if self.field != Field::None {
                self.field = Field::None;
                cx.notify();
            } else {
                window.remove_window();
            }
            return;
        }
        let field = self.field;
        match field {
            Field::None => {}
            Field::EnvKey => {
                if apply_text_key(&mut self.env_key, ev, false, cx) {
                    cx.notify();
                }
            }
            Field::EnvVal => {
                if apply_text_key(&mut self.env_val, ev, false, cx) {
                    cx.notify();
                }
            }
            Field::UpdateHours
            | Field::ConcDownloads
            | Field::ConcTasks
            | Field::HttpTimeout
            | Field::NetRetries
            | Field::LogLimit
            | Field::WinWidth
            | Field::WinHeight
            | Field::JavaMinMem
            | Field::JavaMaxMem
            | Field::JavaPermGen
            | Field::ProxyPort => {
                if apply_text_key(&mut self.num_buf, ev, false, cx) {
                    self.num_buf.retain(|c| c.is_ascii_digit());
                    if let Ok(n) = self.num_buf.parse::<u64>() {
                        self.mutate_prefs(cx, |p| match field {
                            Field::UpdateHours => p.update_interval_hours = (n as u32).min(720),
                            Field::ConcDownloads => {
                                p.concurrent_downloads = (n as u32).clamp(1, 64)
                            }
                            Field::ConcTasks => p.concurrent_tasks = (n as u32).clamp(1, 64),
                            Field::HttpTimeout => p.http_timeout_secs = (n as u32).clamp(5, 600),
                            Field::NetRetries => p.network_retries = (n as u32).min(20),
                            Field::LogLimit => {
                                p.console_max_lines = (n as usize).clamp(200, 500_000)
                            }
                            Field::WinWidth => p.window_width = (n as u32).clamp(1, 7680),
                            Field::WinHeight => p.window_height = (n as u32).clamp(1, 4320),
                            Field::JavaMinMem => p.java_min_mem_mb = (n as u32).min(65_536),
                            Field::JavaMaxMem => p.java_max_mem_mb = (n as u32).min(65_536),
                            Field::JavaPermGen => p.java_permgen_mb = (n as u32).min(16_384),
                            Field::ProxyPort => p.proxy_port = (n.min(65_535)) as u16,
                            _ => {}
                        });
                    } else {
                        cx.notify();
                    }
                }
            }
            other => {
                let mut cur = {
                    let p = &self.app.read(cx).prefs;
                    match other {
                        Field::PreLaunch => p.pre_launch_cmd.clone(),
                        Field::Wrapper => p.wrapper_cmd.clone(),
                        Field::PostExit => p.post_exit_cmd.clone(),
                        Field::GlfwPath => p.custom_glfw_path.clone(),
                        Field::OpenAlPath => p.custom_openal_path.clone(),
                        Field::JavaPath => p.default_java_path.clone(),
                        Field::JvmArgs => p.default_jvm_args.clone(),
                        Field::HzRustLog => p.horizon_rust_log.clone(),
                        Field::HzExtraArgs => p.horizon_extra_args.clone(),
                        Field::OfflineName => p.offline_player_name.clone(),
                        Field::UserAgent => p.user_agent.clone(),
                        Field::ModrinthToken => p.modrinth_token.clone(),
                        Field::TextEditor => p.text_editor_cmd.clone(),
                        Field::Profiler => p.profiler_cmd.clone(),
                        Field::WorldTool => p.world_tool_cmd.clone(),
                        Field::ProxyHost => p.proxy_host.clone(),
                        Field::ProxyUser => p.proxy_user.clone(),
                        Field::ProxyPass => p.proxy_pass.clone(),
                        _ => String::new(),
                    }
                };
                if apply_text_key(&mut cur, ev, false, cx) {
                    self.mutate_prefs(cx, |p| match other {
                        Field::PreLaunch => p.pre_launch_cmd = cur,
                        Field::Wrapper => p.wrapper_cmd = cur,
                        Field::PostExit => p.post_exit_cmd = cur,
                        Field::GlfwPath => p.custom_glfw_path = cur,
                        Field::OpenAlPath => p.custom_openal_path = cur,
                        Field::JavaPath => p.default_java_path = cur,
                        Field::JvmArgs => p.default_jvm_args = cur,
                        Field::HzRustLog => p.horizon_rust_log = cur,
                        Field::HzExtraArgs => p.horizon_extra_args = cur,
                        Field::OfflineName => p.offline_player_name = cur,
                        Field::UserAgent => p.user_agent = cur,
                        Field::ModrinthToken => p.modrinth_token = cur,
                        Field::TextEditor => p.text_editor_cmd = cur,
                        Field::Profiler => p.profiler_cmd = cur,
                        Field::WorldTool => p.world_tool_cmd = cur,
                        Field::ProxyHost => p.proxy_host = cur,
                        Field::ProxyUser => p.proxy_user = cur,
                        Field::ProxyPass => p.proxy_pass = cur,
                        _ => {}
                    });
                }
            }
        }
    }
}

fn collect_runtimes() -> Vec<java::Runtime> {
    let mut all = java::mojang_runtimes(&paths::java_dir());
    for sys in java::installed() {
        if !all.iter().any(|r| r.home == sys.home) {
            all.push(sys);
        }
    }
    all.sort_by(|a, b| b.major.cmp(&a.major).then_with(|| a.version.cmp(&b.version)));
    all
}

impl Render for SettingsWindow {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = self.app.read(cx).prefs.clone();
        let t = Theme::resolve(p.theme_mode, p.accent, window.appearance());

        let mut sidebar = div()
            .flex()
            .flex_col()
            .flex_none()
            .w(px(188.))
            .gap(px(2.))
            .p(px(8.))
            .bg(t.sidebar)
            .border_r_1()
            .border_color(t.divider);

        for pg in SettingsPage::ALL {
            sidebar = sidebar.child(
                page_nav_row(
                    format!("set-nav-{}", pg.label()),
                    pg.glyph(),
                    pg.label(),
                    self.page == pg,
                    &t,
                )
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    this.set_page(pg, cx);
                })),
            );
        }

        let content = match self.page {
            SettingsPage::Launcher => self.render_launcher(&p, &t, cx),
            SettingsPage::Appearance => self.render_appearance(&p, &t, cx),
            SettingsPage::Minecraft => self.render_minecraft(&p, &t, cx),
            SettingsPage::Horizon => self.render_horizon(&p, &t, cx),
            SettingsPage::Java => self.render_java(&p, &t, cx),
            SettingsPage::Accounts => self.render_accounts(&p, &t, cx),
            SettingsPage::Apis => self.render_apis(&p, &t, cx),
            SettingsPage::ExternalTools => self.render_tools(&p, &t, cx),
            SettingsPage::Proxy => self.render_proxy(&p, &t, cx),
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
                    .gap(px(10.))
                    .child(
                        push_button("set-reset-defaults", "Restore Defaults", true, &t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| *p = Prefs::default());
                                this.notice =
                                    Some(("Restored default global settings.".into(), false));
                            }),
                        ),
                    )
                    .child(
                        div()
                            .text_size(px(11.5))
                            .text_color(t.ink_tertiary)
                            .child(format!("FNH v{VERSION} • Made by Buffer • Changes save automatically")),
                    ),
            )
            .child(
                accent_button("set-close", "Close", true, &t)
                    .on_click(|_, window, _| window.remove_window()),
            );

        div()
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, window, cx| {
                this.handle_key(ev, window, cx);
            }))
            .flex()
            .flex_col()
            .size_full()
            .bg(t.window)
            .font_family(UI_FONT)
            .text_color(t.ink)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h_0()
                    .child(sidebar)
                    .child(
                        div()
                            .id("set-page-scroll")
                            .flex()
                            .flex_col()
                            .flex_1()
                            .min_w_0()
                            .min_h_0()
                            .gap(px(12.))
                            .p(px(16.))
                            .bg(t.layer)
                            .overflow_y_scroll()
                            .child(
                                div()
                                    .text_size(px(17.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(self.page.label()),
                            )
                            .when_some(self.notice.clone(), |d, (msg, err)| {
                                d.child(status_banner(&msg, err, &t))
                            })
                            .child(content),
                    ),
            )
            .child(footer)
    }
}

impl SettingsWindow {
    fn tab_bar(&self, labels: &[&'static str], t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .items_center()
            .gap(px(4.))
            .border_b_1()
            .border_color(t.divider)
            .children(labels.iter().enumerate().map(|(i, lbl)| {
                sub_tab(format!("set-subtab-{i}"), lbl, self.tab == i, t).on_click(cx.listener(
                    move |this, _: &ClickEvent, _, cx| {
                        this.tab = i;
                        this.field = Field::None;
                        cx.notify();
                    },
                ))
            }))
    }

    fn labeled_input(
        &mut self,
        id: &'static str,
        label: &str,
        value: String,
        placeholder: &str,
        field: Field,
        width: Option<f32>,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> gpui::Div {
        let focused = self.field == field;
        let cur_for_num = value.clone();
        let inp = text_input(id, &value, placeholder, focused, t).on_click(cx.listener(
            move |this, _: &ClickEvent, _, cx| {
                this.field = field;
                this.num_buf = cur_for_num.clone();
                cx.notify();
            },
        ));
        div()
            .flex()
            .items_center()
            .gap(px(10.))
            .child(
                div()
                    .w(px(175.))
                    .flex_none()
                    .text_size(px(12.5))
                    .text_color(t.ink_secondary)
                    .child(label.to_owned()),
            )
            .child(match width {
                Some(w) => div().w(px(w)).child(inp),
                None => div().flex_1().child(inp),
            })
    }

    fn render_launcher(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let tabs = self.tab_bar(&["Features", "User Interface", "Console", "Folders"], t, cx);
        let body = match self.tab {
            0 => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    group_box("Updater & Instance Creation", None, t)
                        .child(
                            checkbox_row(
                                "lf-upd",
                                "Check for launcher updates automatically",
                                None,
                                p.check_updates_on_start,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.check_updates_on_start = !p.check_updates_on_start
                                });
                            })),
                        )
                        .child(self.labeled_input(
                            "lf-upd-hrs",
                            "Update check interval (hours):",
                            self.num_text(Field::UpdateHours, p.update_interval_hours.to_string()),
                            "24",
                            Field::UpdateHours,
                            Some(110.),
                            t,
                            cx,
                        ))
                        .child(
                            checkbox_row(
                                "lf-dlcreate",
                                "Download game files during instance creation",
                                Some("Disable to fetch Minecraft, Java, Fabric or Horizon files on first launch instead."),
                                p.download_on_create,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.download_on_create = !p.download_on_create
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "lf-tpl",
                                "Copy Instance Template into new instances by default",
                                Some("Files placed inside the Template folder (such as options.txt) copy into new instances."),
                                p.copy_template_on_create,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.copy_template_on_create = !p.copy_template_on_create
                                });
                            })),
                        ),
                )
                .child(
                    group_box("Mods and Modpacks", None, t)
                        .child(
                            checkbox_row(
                                "lf-deps",
                                "Install required mod dependencies automatically",
                                Some("Automatically resolves and installs Fabric API and library mods when downloading."),
                                p.auto_install_deps,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| p.auto_install_deps = !p.auto_install_deps);
                            })),
                        )
                        .child(
                            checkbox_row(
                                "lf-meta",
                                "Keep track of installed mod metadata",
                                Some("Stores Modrinth & Horizon Registry version info for easy mod updates."),
                                p.track_mod_metadata,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.track_mod_metadata = !p.track_mod_metadata
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "lf-incompat",
                                "Detect and warn about incompatible mod formats",
                                None,
                                p.detect_mod_incompat,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.detect_mod_incompat = !p.detect_mod_incompat
                                });
                            })),
                        )
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .w(px(175.))
                                        .text_size(px(12.5))
                                        .text_color(t.ink_secondary)
                                        .child("Mod update channel:"),
                                )
                                .children(UpdateChannel::ALL.into_iter().enumerate().map(
                                    |(i, ch)| {
                                        chip(
                                            format!("lf-ch-{i}"),
                                            ch.label(),
                                            p.mod_update_channel == ch,
                                            t,
                                        )
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            this.mutate_prefs(cx, |p| p.mod_update_channel = ch);
                                        }))
                                    },
                                )),
                        ),
                )
                .child(
                    group_box("Network & Concurrent Tasks", None, t)
                        .child(self.labeled_input(
                            "lf-cdl",
                            "Concurrent download limit:",
                            self.num_text(Field::ConcDownloads, p.concurrent_downloads.to_string()),
                            "6",
                            Field::ConcDownloads,
                            Some(110.),
                            t,
                            cx,
                        ))
                        .child(self.labeled_input(
                            "lf-ctask",
                            "Concurrent task limit:",
                            self.num_text(Field::ConcTasks, p.concurrent_tasks.to_string()),
                            "10",
                            Field::ConcTasks,
                            Some(110.),
                            t,
                            cx,
                        ))
                        .child(self.labeled_input(
                            "lf-timeout",
                            "HTTP timeout (seconds):",
                            self.num_text(Field::HttpTimeout, p.http_timeout_secs.to_string()),
                            "60",
                            Field::HttpTimeout,
                            Some(110.),
                            t,
                            cx,
                        ))
                        .child(self.labeled_input(
                            "lf-retry",
                            "Network retry limit:",
                            self.num_text(Field::NetRetries, p.network_retries.to_string()),
                            "3",
                            Field::NetRetries,
                            Some(110.),
                            t,
                            cx,
                        )),
                ),
            1 => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    group_box("Instance Sorting & Layout", None, t)
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .w(px(160.))
                                        .text_size(px(12.5))
                                        .text_color(t.ink_secondary)
                                        .child("Sort instances:"),
                                )
                                .children(SortMode::ALL.into_iter().enumerate().map(|(i, sm)| {
                                    chip(format!("ui-sort-{i}"), sm.label(), p.sort_mode == sm, t)
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            this.mutate_prefs(cx, |p| p.sort_mode = sm);
                                        }))
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .w(px(160.))
                                        .text_size(px(12.5))
                                        .text_color(t.ink_secondary)
                                        .child("View mode:"),
                                )
                                .children(ViewMode::ALL.into_iter().enumerate().map(|(i, vm)| {
                                    chip(format!("ui-vm-{i}"), vm.label(), p.view_mode == vm, t)
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            this.mutate_prefs(cx, |p| p.view_mode = vm);
                                        }))
                                })),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .child(
                                    div()
                                        .w(px(160.))
                                        .text_size(px(12.5))
                                        .text_color(t.ink_secondary)
                                        .child("Instance icon size:"),
                                )
                                .children(
                                    [(36u32, "Small (36px)"), (48, "Default (48px)"), (64, "Large (64px)")]
                                        .into_iter()
                                        .map(|(sz, lbl)| {
                                            chip(
                                                format!("ui-isz-{sz}"),
                                                lbl,
                                                p.icon_size == sz,
                                                t,
                                            )
                                            .on_click(cx.listener(
                                                move |this, _: &ClickEvent, _, cx| {
                                                    this.mutate_prefs(cx, |p| p.icon_size = sz);
                                                },
                                            ))
                                        }),
                                ),
                        ),
                )
                .child(
                    group_box("Interaction & Safety Prompts", None, t)
                        .child(
                            checkbox_row(
                                "ui-dbl",
                                "Open Edit Instance window on double-click (instead of launching)",
                                None,
                                p.double_click_edits,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.double_click_edits = !p.double_click_edits
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "ui-cfmdel",
                                "Ask for confirmation before deleting an instance",
                                None,
                                p.confirm_delete,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| p.confirm_delete = !p.confirm_delete);
                            })),
                        )
                        .child(
                            checkbox_row(
                                "ui-remsel",
                                "Remember selected instance across restarts",
                                None,
                                p.remember_selected_instance,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.remember_selected_instance = !p.remember_selected_instance
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "ui-status",
                                "Show bottom status bar",
                                None,
                                p.show_status_bar,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| p.show_status_bar = !p.show_status_bar);
                            })),
                        )
                        .child(
                            checkbox_row(
                                "ui-gcnt",
                                "Show instance count beside group headers",
                                None,
                                p.show_group_counts,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.show_group_counts = !p.show_group_counts
                                });
                            })),
                        ),
                ),
            2 => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    group_box("Console Window & Log Output", None, t)
                        .child(self.labeled_input(
                            "con-limit",
                            "Log history limit (lines):",
                            self.num_text(Field::LogLimit, p.console_max_lines.to_string()),
                            "100000",
                            Field::LogLimit,
                            Some(130.),
                            t,
                            cx,
                        ))
                        .child(
                            checkbox_row(
                                "con-ovf",
                                "Stop logging when log history limit is reached",
                                None,
                                p.console_stop_on_overflow,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.console_stop_on_overflow = !p.console_stop_on_overflow
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "con-open",
                                "Show console window automatically when a game is launched",
                                None,
                                p.show_console_on_launch,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.show_console_on_launch = !p.show_console_on_launch
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "con-close",
                                "Automatically close console window when game exits cleanly",
                                None,
                                p.close_console_on_exit,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.close_console_on_exit = !p.close_console_on_exit
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "con-crash",
                                "Show console window automatically when the game crashes",
                                None,
                                p.show_console_on_crash,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.show_console_on_crash = !p.show_console_on_crash
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "con-hl",
                                "Colour-code log levels (INFO / WARN / ERROR / FATAL)",
                                None,
                                p.console_highlight_levels,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.console_highlight_levels = !p.console_highlight_levels
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "con-ts",
                                "Prepend HH:MM:SS timestamps to console lines",
                                None,
                                p.console_timestamps,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.console_timestamps = !p.console_timestamps
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "con-wrap",
                                "Wrap long lines in console viewer",
                                None,
                                p.console_word_wrap,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.console_word_wrap = !p.console_word_wrap
                                });
                            })),
                        ),
                ),
            _ => {
                let folders = [
                    ("FNH Data Root", paths::root()),
                    ("Instances", paths::instances_dir()),
                    ("Instance Icons (Custom)", paths::icons_dir()),
                    ("Instance Template", paths::template_dir()),
                    ("Auto-Downloaded Java", paths::java_dir()),
                    ("Minecraft Assets & Libraries", paths::minecraft_dir()),
                    ("Horizon Engine Builds", paths::versions_dir()),
                    ("Launcher Logs", paths::logs_dir()),
                ];
                group_box(
                    "FNH Folders",
                    Some("All FNH data lives cleanly in its own folder, isolated from other launchers. Tip: create portable.txt next to FH.exe for fully portable USB mode."),
                    t,
                )
                .children(folders.into_iter().enumerate().map(|(i, (lbl, dir))| {
                    let shown = dir.display().to_string();
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div()
                                .w(px(185.))
                                .flex_none()
                                .text_size(px(12.5))
                                .text_color(t.ink_secondary)
                                .child(lbl),
                        )
                        .child(
                            div()
                                .flex_1()
                                .px(px(8.))
                                .py(px(5.))
                                .rounded(px(4.))
                                .bg(t.input_bg)
                                .border_1()
                                .border_color(t.control_border)
                                .font_family(MONO_FONT)
                                .text_size(px(11.5))
                                .text_color(t.ink)
                                .overflow_x_hidden()
                                .child(shown),
                        )
                        .child(
                            push_button(format!("fld-open-{i}"), "Open", true, t).on_click(
                                move |_, _, _| {
                                    let _ = std::fs::create_dir_all(&dir);
                                    let _ = log::open_folder(&dir);
                                },
                            ),
                        )
                }))
            }
        };
        div().flex().flex_col().gap(px(12.)).child(tabs).child(body)
    }

    fn render_appearance(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                group_box(
                    "Widget Theme (JLac Windows 11 / WinUI 3)",
                    Some("Applies live to the main launcher and every open instance or settings window."),
                    t,
                )
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(ThemeMode::ALL.into_iter().enumerate().map(|(i, mode)| {
                            chip(format!("ap-mode-{i}"), mode.label(), p.theme_mode == mode, t)
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.mutate_prefs(cx, |p| p.theme_mode = mode);
                                }))
                        })),
                ),
            )
            .child(
                group_box("Fluent Accent Colour", None, t).child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(8.))
                        .children(Accent::ALL.into_iter().enumerate().map(|(i, acc)| {
                            let (swatch, _, _) = acc.colours(t.dark);
                            let sel = p.accent == acc;
                            div()
                                .id(SharedString::from(format!("ap-acc-{i}")))
                                .flex()
                                .items_center()
                                .gap(px(7.))
                                .h(px(30.))
                                .px(px(10.))
                                .rounded(px(4.))
                                .bg(if sel { t.tile_selected } else { t.control })
                                .border_1()
                                .border_color(if sel { t.accent } else { t.control_border })
                                .cursor_pointer()
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.mutate_prefs(cx, |p| p.accent = acc);
                                }))
                                .child(
                                    div()
                                        .size(px(14.))
                                        .rounded(px(7.))
                                        .bg(hex(swatch)),
                                )
                                .child(div().text_size(px(12.)).child(acc.label()))
                        })),
                ),
            )
            .child(
                group_box("Console Font & Icons", None, t)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(6.))
                            .child(
                                div()
                                    .w(px(150.))
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("Console font size:"),
                            )
                            .children([10u32, 11, 12, 13, 14, 16].into_iter().map(|pt| {
                                chip(
                                    format!("ap-fnt-{pt}"),
                                    format!("{pt} pt"),
                                    p.console_font_size == pt,
                                    t,
                                )
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.mutate_prefs(cx, |p| p.console_font_size = pt);
                                }))
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                push_button("ap-icons-dir", "Open Custom Icons Folder", true, t)
                                    .on_click(|_, _, _| {
                                        let _ = log::open_folder(&paths::icons_dir());
                                    }),
                            )
                            .child(
                                push_button("ap-icons-reload", "Reload All Icons", true, t)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        inst_icons::clear_cache();
                                        this.app.update(cx, |_, cx| cx.notify());
                                        this.notice =
                                            Some(("Reloaded all instance icons.".into(), false));
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .child(
                group_box("Console Log Preview", None, t).child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .p(px(10.))
                        .rounded(px(4.))
                        .bg(t.input_bg)
                        .border_1()
                        .border_color(t.control_border)
                        .font_family(MONO_FONT)
                        .text_size(px(p.console_font_size as f32))
                        .children(
                            [
                                format!("[FnH] FnH v{} (made by Buffer) — ready.", crate::VERSION),
                                "[12:00:01] [Render thread/INFO]: Loading Minecraft 1.21.11 with Fabric Loader 0.20.1".to_owned(),
                                "[12:00:02] [Worker-Main/WARN]: Resource pack declares newer pack_format".to_owned(),
                                "[12:00:03] [Render thread/ERROR]: Example error highlight preview".to_owned(),
                            ]
                            .into_iter()
                            .map(|l| {
                                let color = log_line_color(&l, p.console_highlight_levels, t);
                                div()
                                    .text_color(color)
                                    .child(l)
                            }),
                        ),
                ),
            )
    }

    fn render_minecraft(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let tabs = self.tab_bar(
            &["General", "Custom Commands", "Environment Variables", "Tweaks"],
            t,
            cx,
        );
        let body = match self.tab {
            0 => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    group_box(
                        "Game Window",
                        Some("Global default resolution and display mode (each instance can override in its own Settings)."),
                        t,
                    )
                    .child(
                        checkbox_row(
                            "mc-max",
                            "Start Minecraft maximized",
                            None,
                            p.start_maximized,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.mutate_prefs(cx, |p| p.start_maximized = !p.start_maximized);
                        })),
                    )
                    .child(
                        checkbox_row(
                            "mc-fs",
                            "Start in fullscreen (--fullscreen)",
                            None,
                            p.start_fullscreen,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.mutate_prefs(cx, |p| p.start_fullscreen = !p.start_fullscreen);
                        })),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .w(px(175.))
                                    .text_size(px(12.5))
                                    .text_color(t.ink_secondary)
                                    .child("Window size (W × H):"),
                            )
                            .child(
                                div().w(px(95.)).child(
                                    text_input(
                                        "mc-w",
                                        &self.num_text(Field::WinWidth, p.window_width.to_string()),
                                        "854",
                                        self.field == Field::WinWidth,
                                        t,
                                    )
                                    .on_click(cx.listener({
                                        let w = p.window_width;
                                        move |this, _: &ClickEvent, _, cx| {
                                            this.focus_num(Field::WinWidth, w, cx);
                                        }
                                    })),
                                ),
                            )
                            .child("×")
                            .child(
                                div().w(px(95.)).child(
                                    text_input(
                                        "mc-h",
                                        &self
                                            .num_text(Field::WinHeight, p.window_height.to_string()),
                                        "480",
                                        self.field == Field::WinHeight,
                                        t,
                                    )
                                    .on_click(cx.listener({
                                        let h = p.window_height;
                                        move |this, _: &ClickEvent, _, cx| {
                                            this.focus_num(Field::WinHeight, h, cx);
                                        }
                                    })),
                                ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .children(WINDOW_PRESETS.iter().enumerate().map(|(i, (lbl, w, h))| {
                                let (w, h) = (*w, *h);
                                chip(
                                    format!("mc-wp-{i}"),
                                    *lbl,
                                    !p.start_maximized && p.window_width == w && p.window_height == h,
                                    t,
                                )
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.field = Field::None;
                                    this.mutate_prefs(cx, |p| {
                                        p.start_maximized = false;
                                        p.window_width = w;
                                        p.window_height = h;
                                    });
                                }))
                            })),
                    ),
                )
                .child(
                    group_box("Launcher Behaviour & Game Time", None, t)
                        .child(
                            checkbox_row(
                                "mc-quit",
                                "Quit FNH automatically when the game closes",
                                None,
                                p.quit_launcher_on_exit,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.quit_launcher_on_exit = !p.quit_launcher_on_exit
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "mc-recpt",
                                "Record time spent playing instances",
                                None,
                                p.record_playtime,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| p.record_playtime = !p.record_playtime);
                            })),
                        )
                        .child(
                            checkbox_row(
                                "mc-showpt",
                                "Show per-instance time played",
                                None,
                                p.show_instance_playtime,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.show_instance_playtime = !p.show_instance_playtime
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "mc-totpt",
                                "Show total playtime across instances in status bar",
                                None,
                                p.show_total_playtime,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.show_total_playtime = !p.show_total_playtime
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "mc-hrs",
                                "Always format durations in hours",
                                None,
                                p.playtime_always_hours,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.playtime_always_hours = !p.playtime_always_hours
                                });
                            })),
                        )
                        .child(
                            checkbox_row(
                                "mc-snaps",
                                "Show Snapshot versions by default in New Instance",
                                None,
                                p.show_snapshots_default,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.show_snapshots_default = !p.show_snapshots_default
                                });
                            })),
                        ),
                ),
            1 => group_box(
                "Global Custom Commands",
                Some("Run around every Fabric or Horizon launch unless overridden per instance. Exposed env vars: $INST_NAME, $INST_ID, $INST_DIR, $INST_MC_DIR, $INST_JAVA."),
                t,
            )
            .child(self.labeled_input(
                "cmd-pre",
                "Pre-launch command:",
                p.pre_launch_cmd.clone(),
                "Runs before the game starts",
                Field::PreLaunch,
                None,
                t,
                cx,
            ))
            .child(self.labeled_input(
                "cmd-wrap",
                "Wrapper command:",
                p.wrapper_cmd.clone(),
                "Program wrapping the game executable",
                Field::Wrapper,
                None,
                t,
                cx,
            ))
            .child(self.labeled_input(
                "cmd-post",
                "Post-exit command:",
                p.post_exit_cmd.clone(),
                "Runs after the game exits",
                Field::PostExit,
                None,
                t,
                cx,
            )),
            2 => group_box(
                "Global Environment Variables",
                Some("Passed to every launched Fabric and Horizon instance (unless overridden by the instance)."),
                t,
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div().w(px(190.)).child(
                            text_input(
                                "env-k",
                                &self.env_key,
                                "VARIABLE_NAME",
                                self.field == Field::EnvKey,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.field = Field::EnvKey;
                                cx.notify();
                            })),
                        ),
                    )
                    .child(
                        div().flex_1().child(
                            text_input(
                                "env-v",
                                &self.env_val,
                                "Value",
                                self.field == Field::EnvVal,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.field = Field::EnvVal;
                                cx.notify();
                            })),
                        ),
                    )
                    .child(
                        accent_button("env-add", "Add", !self.env_key.trim().is_empty(), t)
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                let k = this.env_key.trim().to_owned();
                                let v = this.env_val.clone();
                                if k.is_empty() {
                                    return;
                                }
                                this.env_key.clear();
                                this.env_val.clear();
                                this.mutate_prefs(cx, |p| {
                                    p.global_env.retain(|(ek, _)| ek != &k);
                                    p.global_env.push((k, v));
                                });
                            })),
                    )
                    .child(
                        push_button("env-clr", "Clear All", !p.global_env.is_empty(), t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| p.global_env.clear());
                            }),
                        ),
                    ),
            )
            .children(p.global_env.iter().enumerate().map(|(i, (k, v))| {
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px(px(10.))
                    .py(px(5.))
                    .rounded(px(4.))
                    .bg(t.input_bg)
                    .border_1()
                    .border_color(t.control_border)
                    .font_family(MONO_FONT)
                    .text_size(px(12.))
                    .child(format!("{k} = {v}"))
                    .child(
                        push_button(format!("env-rm-{i}"), "Remove", true, t).on_click(
                            cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    if i < p.global_env.len() {
                                        p.global_env.remove(i);
                                    }
                                });
                            }),
                        ),
                    )
            })),
            _ => group_box("Performance & Native Library Overrides", None, t)
                .child(
                    checkbox_row(
                        "tw-dgpu",
                        "Prefer discrete / dedicated GPU",
                        Some("Requests high-performance NVIDIA / AMD GPU instead of integrated graphics."),
                        p.prefer_discrete_gpu,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mutate_prefs(cx, |p| p.prefer_discrete_gpu = !p.prefer_discrete_gpu);
                    })),
                )
                .child(
                    checkbox_row(
                        "tw-zink",
                        "Use Zink (OpenGL over Vulkan)",
                        Some("Sets MESA_LOADER_DRIVER_OVERRIDE=zink where supported."),
                        p.use_zink,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mutate_prefs(cx, |p| p.use_zink = !p.use_zink);
                    })),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div().flex_1().child(self.labeled_input(
                                "tw-glfw",
                                "Custom GLFW library:",
                                p.custom_glfw_path.clone(),
                                "Default bundled LWJGL GLFW",
                                Field::GlfwPath,
                                None,
                                t,
                                cx,
                            )),
                        )
                        .child(push_button("tw-glfw-br", "Browse…", true, t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.browse_path(Field::GlfwPath, cx);
                            }),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .child(
                            div().flex_1().child(self.labeled_input(
                                "tw-oal",
                                "Custom OpenAL library:",
                                p.custom_openal_path.clone(),
                                "Default bundled LWJGL OpenAL",
                                Field::OpenAlPath,
                                None,
                                t,
                                cx,
                            )),
                        )
                        .child(push_button("tw-oal-br", "Browse…", true, t).on_click(
                            cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.browse_path(Field::OpenAlPath, cx);
                            }),
                        )),
                ),
        };
        div().flex().flex_col().gap(px(12.)).child(tabs).child(body)
    }

    fn render_horizon(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let tabs = self.tab_bar(
            &["Channel & Builds", "Graphics & Launch", "Privacy & Debug"],
            t,
            cx,
        );
        let installed = backend::installed();
        let is_busy = job::busy();
        let body = match self.tab {
            0 => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    group_box(
                        "Default Horizon Release Channel",
                        Some("Used by Horizon instances set to 'Follow global Horizon setting'."),
                        t,
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                chip("hz-ch-n", "Nightly (latest)", p.horizon_prefer_nightly, t)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.mutate_prefs(cx, |p| p.horizon_prefer_nightly = true);
                                    })),
                            )
                            .child(
                                chip("hz-ch-r", "Stable Release", !p.horizon_prefer_nightly, t)
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.mutate_prefs(cx, |p| p.horizon_prefer_nightly = false);
                                    })),
                            )
                            .child(
                                push_button("hz-dl-n", "Download Latest Nightly", !is_busy, t)
                                    .when(!is_busy, |b| {
                                        b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            job::install_horizon_channel(true);
                                            this.notice = Some((
                                                "Downloading latest Horizon Nightly in the background…".into(),
                                                false,
                                            ));
                                            cx.notify();
                                        }))
                                    }),
                            )
                            .child(
                                push_button("hz-dl-r", "Download Latest Release", !is_busy, t)
                                    .when(!is_busy, |b| {
                                        b.on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                            job::install_horizon_channel(false);
                                            this.notice = Some((
                                                "Downloading latest Horizon Release in the background…".into(),
                                                false,
                                            ));
                                            cx.notify();
                                        }))
                                    }),
                            ),
                    )
                    .child(
                        checkbox_row(
                            "hz-aum",
                            "Auto-update Horizon Registry mods on launch",
                            None,
                            p.horizon_auto_update_mods,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.mutate_prefs(cx, |p| {
                                p.horizon_auto_update_mods = !p.horizon_auto_update_mods
                            });
                        })),
                    )
                    .child(
                        checkbox_row(
                            "hz-va",
                            "Verify & complete Horizon built-in assets and sounds before launch",
                            None,
                            p.horizon_verify_assets,
                            true,
                            t,
                        )
                        .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.mutate_prefs(cx, |p| {
                                p.horizon_verify_assets = !p.horizon_verify_assets
                            });
                        })),
                    ),
                )
                .child(
                    group_box(
                        "Installed Horizon Engine Builds",
                        Some("Shared read-only across your isolated Horizon instances."),
                        t,
                    )
                    .when(installed.is_empty(), |g| {
                        g.child(
                            div()
                                .text_size(px(12.))
                                .text_color(t.ink_secondary)
                                .child("No Horizon builds downloaded yet — click Download above or launch any Horizon instance."),
                        )
                    })
                    .children(installed.into_iter().enumerate().map(|(i, tag)| {
                        let prep = backend::is_prepared(&tag);
                        let tag_rm = tag.clone();
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(4.))
                            .bg(t.input_bg)
                            .border_1()
                            .border_color(t.control_border)
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap(px(10.))
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(tag),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(11.5))
                                            .text_color(if prep { t.success } else { t.ink_tertiary })
                                            .child(if prep {
                                                "Prepared & ready"
                                            } else {
                                                "Prepares on launch"
                                            }),
                                    ),
                            )
                            .child(
                                push_button(format!("hz-rm-{i}"), "Remove", !is_busy, t).when(
                                    !is_busy,
                                    |b| {
                                        b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            match backend::remove(&tag_rm) {
                                                Ok(()) => {
                                                    this.notice = Some((
                                                        format!("Removed {tag_rm}."),
                                                        false,
                                                    ));
                                                }
                                                Err(e) => this.notice = Some((e, true)),
                                            }
                                            cx.notify();
                                        }))
                                    },
                                ),
                            )
                    })),
                ),
            1 => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    group_box(
                        "Horizon Graphics Backend (wgpu)",
                        Some("Default graphics API and GPU power preference for Horizon instances (overridable per instance)."),
                        t,
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .children(GpuBackend::ALL.into_iter().enumerate().map(|(i, b)| {
                                chip(
                                    format!("hz-gpu-{i}"),
                                    b.label(),
                                    p.horizon_gpu_backend == b,
                                    t,
                                )
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.mutate_prefs(cx, |p| p.horizon_gpu_backend = b);
                                }))
                            })),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .children(GpuPower::ALL.into_iter().enumerate().map(|(i, pw)| {
                                chip(
                                    format!("hz-pwr-{i}"),
                                    pw.label(),
                                    p.horizon_gpu_power == pw,
                                    t,
                                )
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.mutate_prefs(cx, |p| p.horizon_gpu_power = pw);
                                }))
                            })),
                    ),
                )
                .child(
                    group_box("Default Horizon Launch Arguments", None, t)
                        .child(
                            checkbox_row(
                                "hz-cre",
                                "Start Horizon in Creative mode by default (--creative)",
                                None,
                                p.horizon_start_creative,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| {
                                    p.horizon_start_creative = !p.horizon_start_creative
                                });
                            })),
                        )
                        .child(self.labeled_input(
                            "hz-xargs",
                            "Extra Horizon CLI flags:",
                            p.horizon_extra_args.clone(),
                            "Optional flags passed to horizon-client",
                            Field::HzExtraArgs,
                            None,
                            t,
                            cx,
                        )),
                ),
            _ => group_box("Privacy & Crash Diagnostics", None, t)
                .child(
                    checkbox_row(
                        "hz-tel",
                        "Opt out of Horizon telemetry (HORIZON_TELEMETRY_URL=off)",
                        None,
                        p.horizon_telemetry_opt_out,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mutate_prefs(cx, |p| {
                            p.horizon_telemetry_opt_out = !p.horizon_telemetry_opt_out
                        });
                    })),
                )
                .child(
                    checkbox_row(
                        "hz-noai",
                        "Enable Horizon No-AI preference in instances",
                        None,
                        p.horizon_no_ai,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mutate_prefs(cx, |p| p.horizon_no_ai = !p.horizon_no_ai);
                    })),
                )
                .child(
                    checkbox_row(
                        "hz-bt",
                        "Enable full Rust backtraces on crash (RUST_BACKTRACE=1)",
                        None,
                        p.horizon_rust_backtrace,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.mutate_prefs(cx, |p| {
                            p.horizon_rust_backtrace = !p.horizon_rust_backtrace
                        });
                    })),
                )
                .child(self.labeled_input(
                    "hz-rlog",
                    "Horizon log filter (RUST_LOG):",
                    p.horizon_rust_log.clone(),
                    "Default (e.g. info, warn, horizon_client=debug)",
                    Field::HzRustLog,
                    None,
                    t,
                    cx,
                )),
        };
        div().flex().flex_col().gap(px(12.)).child(tabs).child(body)
    }

    fn render_java(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let tabs = self.tab_bar(&["Memory & JVM", "Installations"], t, cx);
        let is_busy = job::busy();
        let body = match self.tab {
            0 => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    group_box(
                        "Memory Allocation",
                        Some("Global default heap size for Fabric / Minecraft: Java Edition instances."),
                        t,
                    )
                    .child(self.labeled_input(
                        "jv-min",
                        "Minimum memory (-Xms, MiB):",
                        self.num_text(Field::JavaMinMem, p.java_min_mem_mb.to_string()),
                        "512",
                        Field::JavaMinMem,
                        Some(120.),
                        t,
                        cx,
                    ))
                    .child(self.labeled_input(
                        "jv-max",
                        "Maximum memory (-Xmx, MiB):",
                        self.num_text(Field::JavaMaxMem, p.java_max_mem_mb.to_string()),
                        "4096",
                        Field::JavaMaxMem,
                        Some(120.),
                        t,
                        cx,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(6.))
                            .children(MEMORY_PRESETS_MB.iter().map(|&mb| {
                                chip(
                                    format!("jv-mp-{mb}"),
                                    format!("{} GiB ({mb} MiB)", mb / 1024),
                                    p.java_max_mem_mb == mb,
                                    t,
                                )
                                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    this.field = Field::None;
                                    this.mutate_prefs(cx, |p| p.java_max_mem_mb = mb);
                                }))
                            })),
                    )
                    .child(self.labeled_input(
                        "jv-perm",
                        "MaxMetaspaceSize (MiB, 0=auto):",
                        self.num_text(Field::JavaPermGen, p.java_permgen_mb.to_string()),
                        "0",
                        Field::JavaPermGen,
                        Some(120.),
                        t,
                        cx,
                    )),
                )
                .child(
                    group_box("Java Runtime & Arguments", None, t)
                        .child(
                            checkbox_row(
                                "jv-autodl",
                                "Auto-download Mojang's recommended Java build automatically",
                                Some("Automatically installs Java 8, 17, 21 or 25 matching each Minecraft version."),
                                p.auto_download_java,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| p.auto_download_java = !p.auto_download_java);
                            })),
                        )
                        .child(
                            checkbox_row(
                                "jv-autodet",
                                "Auto-detect installed system Java versions",
                                None,
                                p.auto_detect_java,
                                true,
                                t,
                            )
                            .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                this.mutate_prefs(cx, |p| p.auto_detect_java = !p.auto_detect_java);
                            })),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div().flex_1().child(self.labeled_input(
                                        "jv-path",
                                        "Default Java path:",
                                        p.default_java_path.clone(),
                                        "Auto (pick matching Java automatically)",
                                        Field::JavaPath,
                                        None,
                                        t,
                                        cx,
                                    )),
                                )
                                .child(push_button("jv-br", "Browse…", true, t).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.browse_path(Field::JavaPath, cx);
                                    }),
                                ))
                                .child(push_button("jv-test", "Test", true, t).on_click({
                                    let jp = p.default_java_path.clone();
                                    cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        this.notice = Some(match job::test_java_installation(&jp) {
                                            Ok(ok) => (ok, false),
                                            Err(e) => (e, true),
                                        });
                                        cx.notify();
                                    })
                                })),
                        )
                        .child(self.labeled_input(
                            "jv-args",
                            "Global JVM arguments:",
                            p.default_jvm_args.clone(),
                            "Optional JVM flags (e.g. -XX:+UseG1GC …)",
                            Field::JvmArgs,
                            None,
                            t,
                            cx,
                        ))
                        .child(
                            div()
                                .flex()
                                .flex_wrap()
                                .items_center()
                                .gap(px(6.))
                                .children(JVM_FLAG_PRESETS.iter().enumerate().map(
                                    |(i, (lbl, flags))| {
                                        let fl = (*flags).to_owned();
                                        chip(
                                            format!("jv-preset-{i}"),
                                            *lbl,
                                            p.default_jvm_args.trim() == *flags,
                                            t,
                                        )
                                        .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                            let val = fl.clone();
                                            this.mutate_prefs(cx, |p| p.default_jvm_args = val);
                                        }))
                                    },
                                ))
                                .child(
                                    push_button(
                                        "jv-clr-args",
                                        "Clear Flags",
                                        !p.default_jvm_args.is_empty(),
                                        t,
                                    )
                                    .on_click(cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.mutate_prefs(cx, |p| p.default_jvm_args.clear());
                                    })),
                                ),
                        ),
                ),
            _ => div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    group_box(
                        "Download Mojang Java Runtimes",
                        Some("Install official OpenJDK builds managed inside FNH's Java folder."),
                        t,
                    )
                    .child(
                        div()
                            .flex()
                            .flex_wrap()
                            .gap(px(8.))
                            .children(["21", "17", "8"].into_iter().map(|maj| {
                                push_button(
                                    format!("jv-dl-{maj}"),
                                    format!("Download Java {maj}"),
                                    !is_busy,
                                    t,
                                )
                                .when(!is_busy, |b| {
                                    b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        job::download_mojang_java(maj.to_owned());
                                        this.notice = Some((
                                            format!("Downloading Mojang Java {maj}…"),
                                            false,
                                        ));
                                        cx.notify();
                                    }))
                                })
                            }))
                            .child(
                                push_button("jv-ref", "Refresh Detected", true, t).on_click(
                                    cx.listener(|this, _: &ClickEvent, _, cx| {
                                        this.runtimes = collect_runtimes();
                                        cx.notify();
                                    }),
                                ),
                            ),
                    ),
                )
                .child(
                    group_box(
                        "Detected Java Installations",
                        Some("Click 'Use as Default' to select a runtime, or leave blank to auto-match per version."),
                        t,
                    )
                    .when(self.runtimes.is_empty(), |g| {
                        g.child(
                            div()
                                .text_size(px(12.))
                                .text_color(t.ink_secondary)
                                .child("No Java runtimes downloaded yet — FNH automatically downloads Java 21 when you launch Fabric, or click Download Java 21 above."),
                        )
                    })
                    .children(self.runtimes.clone().into_iter().enumerate().map(|(i, rt)| {
                        let home_str = rt.home.display().to_string();
                        let src_lbl = match rt.source {
                            java::Source::Mojang => "FNH Managed (Mojang)",
                            java::Source::Installed => "System Installed",
                        };
                        let use_path = home_str.clone();
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .gap(px(8.))
                            .px(px(10.))
                            .py(px(6.))
                            .rounded(px(4.))
                            .bg(t.input_bg)
                            .border_1()
                            .border_color(t.control_border)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .flex_1()
                                    .min_w_0()
                                    .child(
                                        div()
                                            .text_size(px(12.5))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(format!(
                                                "Java {} (major {}) — {src_lbl}",
                                                rt.version, rt.major
                                            )),
                                    )
                                    .child(
                                        div()
                                            .font_family(MONO_FONT)
                                            .text_size(px(11.))
                                            .text_color(t.ink_tertiary)
                                            .child(home_str),
                                    ),
                            )
                            .child(
                                push_button(format!("jv-use-{i}"), "Use Default", true, t)
                                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                        let v = use_path.clone();
                                        this.mutate_prefs(cx, |p| p.default_java_path = v);
                                        this.notice = Some((
                                            format!("Selected Java {} as default.", rt.version),
                                            false,
                                        ));
                                    })),
                            )
                    })),
                ),
        };
        div().flex().flex_col().gap(px(12.)).child(tabs).child(body)
    }

    fn render_accounts(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let accs = account::Accounts::load();
        let st = job::status();
        let signing_in = st.as_ref().is_some_and(|s| s.busy && s.instance_id.is_none());
        let picked = accs.accounts.get(self.selected_account).cloned();

        let code_card = st.as_ref().and_then(|s| s.code.clone()).map(|(code, link)| {
            let url = log::sign_in_url(&link, &code);
            let c1 = code.clone();
            group_box(
                "Microsoft Sign-In Code",
                Some("Paste this code in the browser window that just opened to sign in."),
                t,
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(
                        div()
                            .px(px(14.))
                            .py(px(6.))
                            .rounded(px(4.))
                            .bg(t.input_bg)
                            .border_1()
                            .border_color(t.accent)
                            .font_family(MONO_FONT)
                            .text_size(px(18.))
                            .font_weight(FontWeight::BOLD)
                            .text_color(t.accent)
                            .child(code),
                    )
                    .child(
                        accent_button("acc-copy-code", "Copy Code & Open Browser", true, t)
                            .on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(c1.clone()));
                                let _ = log::open_url(&url);
                            }),
                    )
                    .child(
                        push_button("acc-cancel", "Cancel", true, t).on_click(|_, _, _| {
                            let _ = job::kill();
                        }),
                    ),
            )
        });

        let list = div()
            .flex()
            .flex_1()
            .gap(px(10.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(200.))
                    .rounded(px(4.))
                    .border_1()
                    .border_color(t.control_border)
                    .bg(t.input_bg)
                    .when(accs.accounts.is_empty(), |d| {
                        d.child(
                            div()
                                .p(px(16.))
                                .text_size(px(12.5))
                                .text_color(t.ink_secondary)
                                .child("No Microsoft accounts added yet. Click 'Add Microsoft' on the right to sign in."),
                        )
                    })
                    .children(accs.accounts.iter().enumerate().map(|(i, a)| {
                        let is_def = accs.selected.as_deref() == Some(a.uuid.as_str());
                        let sel = i == self.selected_account;
                        div()
                            .id(SharedString::from(format!("acc-row-{i}")))
                            .flex()
                            .items_center()
                            .justify_between()
                            .px(px(12.))
                            .py(px(8.))
                            .border_b_1()
                            .border_color(t.divider)
                            .bg(if sel {
                                t.tile_selected
                            } else {
                                gpui::transparent_black()
                            })
                            .cursor_pointer()
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.selected_account = i;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(a.name.clone()),
                                    )
                                    .child(
                                        div()
                                            .font_family(MONO_FONT)
                                            .text_size(px(11.))
                                            .text_color(t.ink_tertiary)
                                            .child(format!("MSA • {}", a.uuid)),
                                    ),
                            )
                            .when(is_def, |d| {
                                d.child(
                                    div()
                                        .text_size(px(11.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(t.accent)
                                        .child("Default"),
                                )
                            })
                    })),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(155.))
                    .gap(px(6.))
                    .child(
                        side_action_button(
                            "acc-add",
                            Some("add"),
                            "Add Microsoft",
                            !signing_in,
                            true,
                            t,
                        )
                        .when(!signing_in, |b| {
                            b.on_click(cx.listener(|_, _: &ClickEvent, _, cx| {
                                job::sign_in_now();
                                cx.notify();
                            }))
                        }),
                    )
                    .child(
                        side_action_button(
                            "acc-def",
                            Some("check"),
                            "Set Default",
                            picked.is_some(),
                            false,
                            t,
                        )
                        .when_some(picked.clone(), |b, a| {
                            b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                let mut all = account::Accounts::load();
                                let _ = all.play_as(&a.uuid);
                                let _ = all.save();
                                this.app.update(cx, |_, cx| cx.notify());
                                cx.notify();
                            }))
                        }),
                    )
                    .child(
                        side_action_button(
                            "acc-uuid",
                            Some("copy"),
                            "Copy UUID",
                            picked.is_some(),
                            false,
                            t,
                        )
                        .when_some(picked.clone(), |b, a| {
                            b.on_click(move |_, _, cx| {
                                cx.write_to_clipboard(ClipboardItem::new_string(a.uuid.clone()));
                            })
                        }),
                    )
                    .child(
                        danger_button("acc-rm", "Remove", picked.is_some(), t).when_some(
                            picked,
                            |b, a| {
                                b.on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                    let mut all = account::Accounts::load();
                                    let _ = all.remove(account::store(), &a.uuid);
                                    let _ = all.save();
                                    this.selected_account = 0;
                                    this.app.update(cx, |_, cx| cx.notify());
                                    cx.notify();
                                }))
                            },
                        ),
                    ),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .when_some(code_card, |d, c| d.child(c))
            .child(list)
            .child(
                group_box(
                    "Offline Launch Name",
                    Some("Used when clicking 'Launch Offline' after signing in with an account that owns Minecraft."),
                    t,
                )
                .child(self.labeled_input(
                    "acc-offname",
                    "Offline player name:",
                    p.offline_player_name.clone(),
                    "Player",
                    Field::OfflineName,
                    Some(240.),
                    t,
                    cx,
                )),
            )
    }

    fn render_apis(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(
                group_box(
                    "API Keys & User Agent",
                    Some("Public Modrinth and Horizon Registry downloads work without any key. Only enter a Modrinth token to access private drafts."),
                    t,
                )
                .child(self.labeled_input(
                    "api-ua",
                    "Custom User-Agent:",
                    p.user_agent.clone(),
                    "FnH/$LAUNCHER_VER",
                    Field::UserAgent,
                    None,
                    t,
                    cx,
                ))
                .child(self.labeled_input(
                    "api-mr",
                    "Modrinth API Token:",
                    p.modrinth_token.clone(),
                    "Optional personal access token",
                    Field::ModrinthToken,
                    None,
                    t,
                    cx,
                )),
            )
            .child(
                group_box("Metadata Cache", None, t).child(
                    push_button("api-clr-meta", "Clear Metadata Cache", true, t).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            let n = paths::clear_metadata_cache();
                            this.notice = Some((
                                format!("Cleared metadata cache ({n} cached item(s) removed)."),
                                false,
                            ));
                            cx.notify();
                        }),
                    ),
                ),
            )
    }

    fn render_tools(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        group_box(
            "External Editors, Profilers & World Tools",
            Some("Optional programs launched from Logs, Game Options and Worlds. World Tool supports ${WORLD_PATH}."),
            t,
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div().flex_1().child(self.labeled_input(
                        "tl-ed",
                        "Text / JSON Editor:",
                        p.text_editor_cmd.clone(),
                        "System default (e.g. notepad.exe or code.exe)",
                        Field::TextEditor,
                        None,
                        t,
                        cx,
                    )),
                )
                .child(push_button("tl-ed-br", "Browse…", true, t).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.browse_path(Field::TextEditor, cx);
                    }),
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div().flex_1().child(self.labeled_input(
                        "tl-prof",
                        "Java Profiler (VisualVM):",
                        p.profiler_cmd.clone(),
                        "Optional path to visualvm / jprofiler",
                        Field::Profiler,
                        None,
                        t,
                        cx,
                    )),
                )
                .child(push_button("tl-prof-br", "Browse…", true, t).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.browse_path(Field::Profiler, cx);
                    }),
                )),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div().flex_1().child(self.labeled_input(
                        "tl-wt",
                        "World Tool Command:",
                        p.world_tool_cmd.clone(),
                        "e.g. MCA Selector / Amulet with ${WORLD_PATH}",
                        Field::WorldTool,
                        None,
                        t,
                        cx,
                    )),
                )
                .child(push_button("tl-wt-br", "Browse…", true, t).on_click(
                    cx.listener(|this, _: &ClickEvent, _, cx| {
                        this.browse_path(Field::WorldTool, cx);
                    }),
                )),
        )
    }

    fn render_proxy(&mut self, p: &Prefs, t: &Theme, cx: &mut Context<Self>) -> gpui::Div {
        let custom = matches!(p.proxy_mode, ProxyMode::Http | ProxyMode::Socks5);
        group_box(
            "Network Proxy",
            Some("Applies to FNH launcher downloads and metadata requests."),
            t,
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(px(6.))
                .children(ProxyMode::ALL.into_iter().enumerate().map(|(i, m)| {
                    chip(format!("px-m-{i}"), m.label(), p.proxy_mode == m, t).on_click(
                        cx.listener(move |this, _: &ClickEvent, _, cx| {
                            this.mutate_prefs(cx, |p| p.proxy_mode = m);
                        }),
                    )
                })),
        )
        .when(custom, |g| {
            g.child(self.labeled_input(
                "px-host",
                "Proxy host / address:",
                p.proxy_host.clone(),
                "127.0.0.1",
                Field::ProxyHost,
                Some(260.),
                t,
                cx,
            ))
            .child(self.labeled_input(
                "px-port",
                "Proxy port:",
                self.num_text(Field::ProxyPort, p.proxy_port.to_string()),
                "8080",
                Field::ProxyPort,
                Some(110.),
                t,
                cx,
            ))
            .child(self.labeled_input(
                "px-user",
                "Username (optional):",
                p.proxy_user.clone(),
                "None",
                Field::ProxyUser,
                Some(220.),
                t,
                cx,
            ))
            .child(self.labeled_input(
                "px-pass",
                "Password (optional):",
                if self.field == Field::ProxyPass {
                    p.proxy_pass.clone()
                } else {
                    "•".repeat(p.proxy_pass.chars().count())
                },
                "None",
                Field::ProxyPass,
                Some(220.),
                t,
                cx,
            ))
        })
    }
}
