//! The window, drawn the way JLac draws its own: the bar across the top with the chosen
//! instance's mark, its name and one lime Play; the instance list down the left in a darker pane;
//! the instance's own pages on the right (Overview, Mods, Worlds, Logs); a status bar along the
//! bottom; and dialogs over all of it for a new instance, the accounts and a sign-in.
//!
//! Everything here is the launcher's data, read live: `instances::list` for the rows, the
//! instance's own folders for its mods, worlds and log, `account::Accounts` for who plays. The
//! window never writes an instance's files itself — Play, install and preparation are
//! `job.rs`'s, on its own thread.

use crate::job;
use crate::log;
use crate::look::{
    self, button, c, chip, colour_of, dot, heading, initial, play_button, quiet, short_path, Kind,
};
use gpui::{
    div, prelude::*, px, AnyElement, ClickEvent, Context, Div, FontWeight, SharedString, Stateful,
    Window,
};
use horizon_launcher::account::Accounts;
use horizon_launcher::instances::{self as inst, Instance, Kind as InstanceKind};
use horizon_launcher::minecraft_versions::{self, Versions};
use horizon_launcher::worlds;
use horizon_launcher::loaders;
use std::time::Duration;

/// The instance's page the window shows.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Mods,
    Worlds,
    Logs,
}

impl Tab {
    const ALL: [Tab; 4] = [Tab::Overview, Tab::Mods, Tab::Worlds, Tab::Logs];

    fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Mods => "Mods",
            Tab::Worlds => "Worlds",
            Tab::Logs => "Logs",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Tab::Overview => "tab-overview",
            Tab::Mods => "tab-mods",
            Tab::Worlds => "tab-worlds",
            Tab::Logs => "tab-logs",
        }
    }
}

/// What is open over the window.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dialog {
    NewInstance,
    Accounts,
    SignIn,
}

/// What a new instance plays: Horizon, plain Minecraft, or Minecraft with Fabric.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Plan {
    Horizon,
    Vanilla,
    Fabric,
}

impl Plan {
    const ALL: [Plan; 3] = [Plan::Horizon, Plan::Vanilla, Plan::Fabric];

    fn label(self) -> &'static str {
        match self {
            Plan::Horizon => "Horizon",
            Plan::Vanilla => "Vanilla",
            Plan::Fabric => "Fabric",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Plan::Horizon => "plan-horizon",
            Plan::Vanilla => "plan-vanilla",
            Plan::Fabric => "plan-fabric",
        }
    }
}

/// Mojang's version list as the new-instance dialog has it.
enum List {
    NotAsked,
    Loading,
    Ready(Versions),
    Failed(String),
}

/// The window's whole state.
pub struct Fh {
    tab: Tab,
    instances: Vec<Instance>,
    chosen: Option<String>,
    dialog: Option<Dialog>,
    plan: Plan,
    /// The Minecraft version the dialog makes, when one was picked.
    mc: Option<String>,
    versions: List,
    accounts: Accounts,
    /// A line under a dialog's controls: what the last action said.
    message: Option<String>,
    creating: bool,
    /// Whether a launch was running at the last tick, so the window reloads once when it ends.
    was_busy: bool,
}

impl Fh {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let instances = inst::list();
        let chosen = instances
            .iter()
            .find(|instance| instance.kind != InstanceKind::Default)
            .or_else(|| instances.first())
            .map(|instance| instance.id.clone());
        let view = Self {
            tab: Tab::Overview,
            instances,
            chosen,
            dialog: None,
            plan: Plan::Fabric,
            mc: None,
            versions: List::NotAsked,
            accounts: Accounts::load(),
            message: None,
            creating: false,
            was_busy: false,
        };
        view.tick_forever(cx);
        view
    }

    /// Looks at the launch four times a second while the window is open: the status bar follows the
    /// step it is on, a sign-in's code appears as Microsoft gives it, and the instance list is read
    /// again when a launch ends (its play time changed).
    fn tick_forever(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
            if this
                .update(cx, |this, cx| {
                    let busy = job::busy();
                    if this.was_busy && !busy {
                        this.instances = inst::list();
                    }
                    this.was_busy = busy;
                    cx.notify();
                })
                .is_err()
            {
                break;
            }
        })
        .detach();
    }

    fn current(&self) -> Option<&Instance> {
        let chosen = self.chosen.as_deref()?;
        self.instances
            .iter()
            .find(|instance| instance.id == chosen)
            .or_else(|| self.instances.first())
    }

    fn choose(&mut self, id: String, cx: &mut Context<Self>) {
        if let Err(e) = inst::select(&id) {
            log::line(&format!("Could not make {id} the one Play starts: {e}"));
        }
        self.chosen = Some(id);
        self.tab = Tab::Overview;
        cx.notify();
    }

    fn signed_in(&self) -> Option<String> {
        self.accounts
            .selected()
            .map(|account| account.name.clone())
    }

    /// The top bar: the chosen instance's mark and name, the account, and Play.
    fn toolbar(&mut self, cx: &mut Context<Self>) -> Div {
        let instance = self.current().cloned();
        let name = instance
            .as_ref()
            .map(|instance| instance.name.clone())
            .unwrap_or_else(|| "No instance".to_owned());
        let detail = instance.as_ref().map(detail_of).unwrap_or_default();
        let mark = match &instance {
            Some(instance) => look::tile(
                &initial(&instance.name),
                colour_of(&instance.id, instance.icon.as_ref()),
                44.,
            ),
            None => look::tile("?", c(look::ALT), 44.),
        };
        let account = self
            .signed_in()
            .unwrap_or_else(|| "Not signed in".to_owned());
        let play = match instance {
            Some(instance) => play_button("play", "▶  Play")
                .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                    job::play(instance.clone());
                    this.was_busy = true;
                    cx.notify();
                }))
                .into_any_element(),
            None => button("play", "Play", Kind::Primary).into_any_element(),
        };
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(12.))
            .h(px(76.))
            .px(px(16.))
            .bg(c(look::WINDOW))
            .border_b_1()
            .border_color(c(look::BORDER))
            .child(mark)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(3.))
                    .child(look::line(16., name).font_weight(FontWeight::SEMIBOLD))
                    .child(quiet(12., detail)),
            )
            .child(
                chip("account", format!("{account}  ▾")).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.dialog = Some(Dialog::Accounts);
                        cx.notify();
                    },
                )),
            )
            .child(play)
    }

    /// The left pane: every instance, then the button that makes another.
    fn sidebar(&mut self, cx: &mut Context<Self>) -> Div {
        let list = self.instances.clone();
        let rows: Vec<AnyElement> = list
            .iter()
            .map(|instance| self.instance_row(instance, cx))
            .collect();
        let count = list.len();
        let new = button("new-instance", "+  New instance", Kind::Secondary).on_click(
            cx.listener(|this, _: &ClickEvent, _, cx| {
                this.dialog = Some(Dialog::NewInstance);
                this.message = None;
                this.ask_versions(cx);
                cx.notify();
            }),
        );
        div()
            .flex()
            .flex_none()
            .flex_col()
            .w(px(260.))
            .h_full()
            .bg(c(look::BASE))
            .border_r_1()
            .border_color(c(look::RULE))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_between()
                    .h(px(34.))
                    .px(px(12.))
                    .child(heading("INSTANCES"))
                    .child(quiet(11., format!("{count}"))),
            )
            .child(
                div()
                    .id("instance-list")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .px(px(6.))
                    .pb(px(6.))
                    .children(rows),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .p(px(8.))
                    .border_t_1()
                    .border_color(c(look::RULE))
                    .child(div().flex().w_full().justify_center().child(new)),
            )
    }

    fn instance_row(&self, instance: &Instance, cx: &mut Context<Self>) -> AnyElement {
        let chosen = self.chosen.as_deref() == Some(instance.id.as_str());
        let id = SharedString::from(format!("row:{}", instance.id));
        let click_id = instance.id.clone();
        let ink = if chosen { c(look::ON_ACCENT) } else { c(look::INK) };
        let under = if chosen { c(0x20400a) } else { c(look::MUTED) };
        div()
            .id(id)
            .flex()
            .items_center()
            .gap(px(9.))
            .h(px(42.))
            .px(px(8.))
            .rounded(px(3.))
            .bg(if chosen {
                c(look::ACCENT)
            } else {
                gpui::transparent_black()
            })
            .when(!chosen, |row| row.hover(|style| style.bg(c(look::ALT))))
            .cursor_pointer()
            .child(look::tile(
                &initial(&instance.name),
                colour_of(&instance.id, instance.icon.as_ref()),
                28.,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .child(
                        div()
                            .text_size(px(13.))
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(ink)
                            .child(instance.name.clone()),
                    )
                    .child(
                        div()
                            .text_size(px(11.))
                            .text_color(under)
                            .child(kind_line(instance)),
                    ),
            )
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.choose(click_id.clone(), cx);
            }))
            .into_any_element()
    }

    /// The right side: the instance's tabs and the tab's page.
    fn main_area(&mut self, cx: &mut Context<Self>) -> Div {
        let tabs: Vec<AnyElement> = Tab::ALL
            .iter()
            .map(|tab| self.tab_button(*tab, cx))
            .collect();
        let body = match self.tab {
            Tab::Overview => self.overview(cx),
            Tab::Mods => self.mods(cx),
            Tab::Worlds => self.worlds(cx),
            Tab::Logs => self.logs(cx),
        };
        div()
            .flex()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex_col()
            .bg(c(look::BASE))
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .h(px(40.))
                    .px(px(8.))
                    .gap(px(2.))
                    .border_b_1()
                    .border_color(c(look::RULE))
                    .children(tabs),
            )
            .child(
                div()
                    .id("page")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_y_scroll()
                    .p(px(20.))
                    .child(body),
            )
    }

    fn tab_button(&self, tab: Tab, cx: &mut Context<Self>) -> AnyElement {
        let active = self.tab == tab;
        div()
            .id(tab.id())
            .flex()
            .items_center()
            .h(px(38.))
            .px(px(14.))
            .text_size(px(12.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(if active { c(look::INK) } else { c(look::MUTED) })
            .border_b_2()
            .border_color(if active {
                c(look::ACCENT)
            } else {
                gpui::transparent_black()
            })
            .cursor_pointer()
            .when(!active, |tab| tab.hover(|style| style.text_color(c(look::INK))))
            .child(tab.label())
            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                this.tab = tab;
                cx.notify();
            }))
            .into_any_element()
    }

    /// The instance's page: what it plays, where it keeps it, and what can be done with it.
    fn overview(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(instance) = self.current().cloned() else {
            return empty("No instance yet: make one with the button in the list.");
        };
        let folder = instance.home.clone();
        let open = button("open-folder", "Open folder", Kind::Secondary).on_click(
            cx.listener(move |_this, _: &ClickEvent, _, _cx| {
                if let Err(e) = log::open_folder(&folder) {
                    log::line(&e);
                }
            }),
        );
        let mut info = div().flex().flex_col().gap(px(8.));
        for (label, value) in [
            ("Plays", kind_line(&instance)),
            ("Build", build_line(&instance)),
            ("Last played", played_line(&instance)),
            ("Game folder", instance.home.display().to_string()),
        ] {
            info = info.child(field(label, value));
        }
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(4.))
                    .child(look::line(20., instance.name.clone()).font_weight(FontWeight::SEMIBOLD))
                    .child(quiet(12., detail_of(&instance))),
            )
            .child(info)
            .child(div().flex().items_center().gap(px(8.)).child(open))
            .into_any_element()
    }

    /// What is in the instance's `mods/`: the jars it loads, newest name order.
    fn mods(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(instance) = self.current().cloned() else {
            return empty("No instance yet: make one with the button in the list.");
        };
        let mods = jar_list(&instance.home.join("mods"));
        let folder = instance.home.join("mods");
        let open = button("open-mods", "Open mods folder", Kind::Secondary).on_click(
            cx.listener(move |_this, _: &ClickEvent, _, _cx| {
                if let Err(e) = log::open_folder(&folder) {
                    log::line(&e);
                }
            }),
        );
        let mut body = div().flex().flex_col().gap(px(10.)).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(look::line(16., format!("{} mods", mods.len())).font_weight(FontWeight::SEMIBOLD))
                .child(open),
        );
        if mods.is_empty() {
            body = body.child(quiet(
                12.,
                "No mods yet. Drop .jar files in the mods folder, or let a modpack put them there.",
            ));
        } else {
            for (name, bytes) in mods {
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .px(px(10.))
                        .py(px(6.))
                        .rounded(px(3.))
                        .bg(c(look::ALT))
                        .child(div().text_size(px(12.)).child(name))
                        .child(quiet(11., megabytes(bytes))),
                );
            }
        }
        body.into_any_element()
    }

    /// The instance's saves: name, mode, version, size and when it was last played.
    fn worlds(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(instance) = self.current().cloned() else {
            return empty("No instance yet: make one with the button in the list.");
        };
        let saves = worlds::list(&instance.home);
        let folder = instance.home.join("saves");
        let open = button("open-saves", "Open saves folder", Kind::Secondary).on_click(
            cx.listener(move |_this, _: &ClickEvent, _, _cx| {
                if let Err(e) = log::open_folder(&folder) {
                    log::line(&e);
                }
            }),
        );
        let mut body = div().flex().flex_col().gap(px(10.)).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(look::line(16., format!("{} worlds", saves.len())).font_weight(FontWeight::SEMIBOLD))
                .child(open),
        );
        if saves.is_empty() {
            body = body.child(quiet(12., "No worlds yet: Play makes one."));
        } else {
            for world in saves {
                let version = world.version.clone().unwrap_or_else(|| "—".to_owned());
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(12.))
                        .px(px(10.))
                        .py(px(7.))
                        .rounded(px(3.))
                        .bg(c(look::ALT))
                        .child(
                            div()
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(div().text_size(px(12.)).child(world.name.clone()))
                                .child(quiet(
                                    11.,
                                    format!("{} · {} · {}", world.mode, version, megabytes(world.size)),
                                )),
                        )
                        .child(quiet(11., when(world.last_played))),
                );
            }
        }
        body.into_any_element()
    }

    /// The end of the instance's own log: what the game said when it last ran.
    fn logs(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let Some(instance) = self.current().cloned() else {
            return empty("No instance yet: make one with the button in the list.");
        };
        let path = instance.home.join("logs").join("latest.log");
        let lines = tail(&path, 400);
        let folder = instance.home.join("logs");
        let open = button("open-logs", "Open log folder", Kind::Secondary).on_click(
            cx.listener(move |_this, _: &ClickEvent, _, _cx| {
                if let Err(e) = log::open_folder(&folder) {
                    log::line(&e);
                }
            }),
        );
        let showing = if lines.is_empty() {
            "No log yet: the game writes one when it first runs.".to_owned()
        } else {
            format!("The last {} lines of {}", lines.len(), short_path(&path.display().to_string(), 64))
        };
        let mut body = div().flex().flex_col().gap(px(10.)).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(quiet(11., showing))
                .child(open),
        );
        if !lines.is_empty() {
            let mut log_box = div().flex().flex_col().gap(px(1.)).p(px(10.)).rounded(px(3.)).bg(c(look::RULE));
            for line in lines {
                log_box = log_box.child(
                    div()
                        .text_size(px(11.))
                        .text_color(c(look::MUTED))
                        .child(line),
                );
            }
            body = body.child(log_box);
        }
        body.into_any_element()
    }

    /// The bottom bar: what the launch is doing, how far it got, and how to dismiss it.
    fn status_bar(&mut self, cx: &mut Context<Self>) -> Div {
        let status = job::status();
        let (colour, text) = match &status {
            Some(status) if status.failed => (c(look::DANGER), status.line.clone()),
            Some(status) if status.busy => (c(look::ACCENT), format!("{} — {}", status.title, status.line)),
            Some(status) => (c(look::DIM), status.line.clone()),
            None => (
                c(look::DIM),
                format!(
                    "Ready · {} instances · FNH v{}",
                    self.instances.len(),
                    crate::VERSION
                ),
            ),
        };
        let bar = status.as_ref().and_then(|status| status.fraction).map(progress);
        let dismiss = status
            .as_ref()
            .filter(|status| !status.busy)
            .map(|_| {
                button("dismiss", "Dismiss", Kind::Quiet)
                    .on_click(cx.listener(|_this, _: &ClickEvent, _, cx| {
                        job::clear();
                        cx.notify();
                    }))
                    .into_any_element()
            });
        div()
            .flex()
            .flex_none()
            .items_center()
            .gap(px(10.))
            .h(px(30.))
            .px(px(12.))
            .bg(c(look::WINDOW))
            .border_t_1()
            .border_color(c(look::RULE))
            .text_size(px(11.))
            .child(dot(colour))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_w(px(0.))
                    .text_color(c(look::MUTED))
                    .child(text),
            )
            .when_some(bar, |row, bar| row.child(bar))
            .when_some(dismiss, |row, dismiss| row.child(dismiss))
    }

    /// Asks Mojang for the Minecraft versions once, for the new-instance dialog.
    fn ask_versions(&mut self, cx: &mut Context<Self>) {
        if !matches!(self.versions, List::NotAsked) {
            return;
        }
        self.versions = List::Loading;
        let task = cx
            .background_executor()
            .spawn(async { minecraft_versions::load() });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.versions = match result {
                    Ok(versions) => List::Ready(versions),
                    Err(e) => List::Failed(e),
                };
                cx.notify();
            });
        })
        .detach();
    }

    /// The dialog over the window, on a dimmed backdrop.
    fn dialog_panel(&mut self, dialog: Dialog, cx: &mut Context<Self>) -> AnyElement {
        let body = match dialog {
            Dialog::NewInstance => self.new_instance(cx),
            Dialog::Accounts => self.accounts_panel(cx),
            Dialog::SignIn => self.sign_in_panel(cx),
        };
        div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .bg(gpui::hsla(0., 0., 0., 0.55))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .w(px(540.))
                    .max_h(px(540.))
                    .gap(px(12.))
                    .p(px(16.))
                    .rounded(px(4.))
                    .bg(c(look::WINDOW))
                    .border_1()
                    .border_color(c(look::BORDER))
                    .child(body),
            )
            .into_any_element()
    }

    fn close_row(&self, title: &str, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .child(look::line(15., title.to_owned()).font_weight(FontWeight::SEMIBOLD))
            .child(
                button("close", "Close", Kind::Quiet).on_click(cx.listener(
                    |this, _: &ClickEvent, _, cx| {
                        this.dialog = None;
                        this.message = None;
                        cx.notify();
                    },
                )),
            )
    }

    /// A new instance: what it plays, which Minecraft version, and the button that makes it.
    fn new_instance(&mut self, cx: &mut Context<Self>) -> Div {
        let mut card = div().flex().flex_col().gap(px(12.)).child(self.close_row("New instance", cx));
        let picks: Vec<AnyElement> = Plan::ALL
            .iter()
            .map(|plan| {
                let active = self.plan == *plan;
                let picked = *plan;
                div()
                    .id(plan.id())
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .h(px(34.))
                    .rounded(px(3.))
                    .bg(if active { c(look::ACCENT) } else { c(look::BUTTON) })
                    .text_color(if active { c(look::ON_ACCENT) } else { c(look::INK) })
                    .text_size(px(12.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .cursor_pointer()
                    .when(!active, |plate| plate.hover(|style| style.bg(c(look::BUTTON_HOVER))))
                    .child(plan.label())
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        this.plan = picked;
                        this.mc = None;
                        if picked != Plan::Horizon {
                            this.ask_versions(cx);
                        }
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .collect();
        card = card.child(div().flex().items_center().gap(px(6.)).children(picks));
        if self.plan != Plan::Horizon {
            card = card.child(quiet(
                11.,
                "Its Minecraft version, newest first. Fabric's loader version comes with the first Play.",
            ));
            card = card.child(self.version_list(cx));
        } else {
            card = card.child(quiet(
                12.,
                "A Horizon instance of its own: worlds, options and packs, started on Horizon.",
            ));
        }
        let create = if self.creating {
            button("create", "Making it…", Kind::Primary)
        } else {
            button("create", "Create", Kind::Primary).on_click(cx.listener(
                |this, _: &ClickEvent, _, cx| {
                    this.create(cx);
                },
            ))
        };
        let message = self
            .message
            .clone()
            .map(|message| quiet(11., message).text_color(c(look::DANGER)));
        card.child(div().flex().items_center().justify_between().gap(px(12.)).child(create).when_some(message, |row, message| row.child(message)))
    }

    fn version_list(&mut self, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut list = div()
            .id("versions")
            .flex()
            .flex_col()
            .gap(px(2.))
            .max_h(px(240.))
            .overflow_y_scroll()
            .p(px(6.))
            .rounded(px(3.))
            .bg(c(look::BASE));
        match &self.versions {
            List::NotAsked | List::Loading => {
                list = list.child(quiet(12., "Asking Mojang for the list…"));
            }
            List::Failed(e) => {
                list = list.child(quiet(12., format!("Mojang's list could not be read: {e}")));
            }
            List::Ready(versions) => {
                let latest = versions.latest_release.clone();
                let chosen = self.mc.clone().unwrap_or_else(|| latest.clone());
                let rows: Vec<(String, bool)> = versions
                    .all
                    .iter()
                    .filter(|version| matches!(version.kind, minecraft_versions::Kind::Release))
                    .take(80)
                    .map(|version| (version.id.clone(), version.id == chosen))
                    .collect();
                let mut built: Vec<AnyElement> = Vec::with_capacity(rows.len());
                for (version, picked) in rows {
                    let id = SharedString::from(format!("mc:{version}"));
                    let click = version.clone();
                    built.push(
                        div()
                            .id(id)
                            .flex()
                            .items_center()
                            .h(px(26.))
                            .px(px(8.))
                            .rounded(px(3.))
                            .bg(if picked {
                                c(look::ACCENT)
                            } else {
                                gpui::transparent_black()
                            })
                            .text_color(if picked { c(look::ON_ACCENT) } else { c(look::INK) })
                            .text_size(px(12.))
                            .cursor_pointer()
                            .when(!picked, |row| row.hover(|style| style.bg(c(look::ALT))))
                            .child(version)
                            .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                                this.mc = Some(click.clone());
                                cx.notify();
                            }))
                            .into_any_element(),
                    );
                }
                list = list.children(built);
            }
        }
        list
    }

    /// Makes the instance the dialog chose, on a background thread (creating a Java instance asks
    /// the loader's service for its version, and that is a download).
    fn create(&mut self, cx: &mut Context<Self>) {
        if self.creating {
            return;
        }
        let plan = self.plan;
        let minecraft = match plan {
            Plan::Horizon => None,
            _ => {
                let picked = self.mc.clone().or_else(|| match &self.versions {
                    List::Ready(versions) => Some(versions.latest_release.clone()),
                    _ => None,
                });
                match picked {
                    Some(mc) => Some(mc),
                    None => {
                        self.message = Some("Mojang's version list isn't here yet.".to_owned());
                        return;
                    }
                }
            }
        };
        let taken: Vec<String> = self
            .instances
            .iter()
            .map(|instance| instance.name.clone())
            .collect();
        let name = match &minecraft {
            None => free_name("My Horizon".to_owned(), &taken),
            Some(mc) => free_name(
                match plan {
                    Plan::Fabric => format!("Fabric {mc}"),
                    _ => format!("Vanilla {mc}"),
                },
                &taken,
            ),
        };
        self.creating = true;
        self.message = Some(format!("Making {name}…"));
        let made_name = name.clone();
        let task = cx.background_executor().spawn(async move {
            match minecraft {
                None => inst::create(&made_name, None),
                Some(mc) => {
                    let made = inst::create_java(&made_name, &mc)?;
                    if plan == Plan::Fabric {
                        let loader = loaders::choose("fabric", &mc, None)?;
                        return inst::edit(&made.id, |meta| {
                            meta.java.get_or_insert_with(Default::default).loader = Some(loader)
                        });
                    }
                    Ok(made)
                }
            }
        });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                this.creating = false;
                match result {
                    Ok(made) => {
                        log::line(&format!("Made {} ({}).", made.name, made.id));
                        this.chosen = Some(made.id.clone());
                        this.instances = inst::list();
                        this.dialog = None;
                        this.message = None;
                        this.tab = Tab::Overview;
                    }
                    Err(e) => {
                        log::line(&format!("Could not make an instance: {e}"));
                        this.message = Some(e);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// The accounts this computer has, and the one Play signs in as.
    fn accounts_panel(&mut self, cx: &mut Context<Self>) -> Div {
        let list = self.accounts.accounts.clone();
        let chosen = self.accounts.selected.clone();
        let mut rows: Vec<AnyElement> = Vec::with_capacity(list.len());
        for account in list {
            let picked = chosen.as_deref() == Some(account.uuid.as_str());
            let id = SharedString::from(format!("account:{}", account.uuid));
            let uuid = account.uuid.clone();
            let name = account.name.clone();
            rows.push(
                div()
                    .id(id)
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .h(px(38.))
                    .px(px(10.))
                    .rounded(px(3.))
                    .bg(if picked { c(look::ALT) } else { c(look::BASE) })
                    .border_1()
                    .border_color(if picked { c(look::ACCENT) } else { c(look::RULE) })
                    .cursor_pointer()
                    .hover(|style| style.bg(c(look::ALT)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(div().text_size(px(12.)).child(name))
                            .child(quiet(11., short_path(&account.uuid, 18))),
                    )
                    .child(quiet(11., if picked { "playing" } else { "use" }))
                    .on_click(cx.listener(move |this, _: &ClickEvent, _, cx| {
                        let played = this.accounts.play_as(&uuid);
                        let result = played.and_then(|()| this.accounts.save());
                        this.message = Some(match result {
                            Ok(()) => "That account plays now.".to_owned(),
                            Err(e) => e,
                        });
                        cx.notify();
                    }))
                    .into_any_element(),
            );
        }
        let mut card = div().flex().flex_col().gap(px(12.)).child(self.close_row("Accounts", cx));
        if rows.is_empty() {
            card = card.child(quiet(
                12.,
                "No Microsoft account on this computer yet. Sign in and the game starts as you.",
            ));
        } else {
            card = card.child(div().flex().flex_col().gap(px(6.)).children(rows));
        }
        let message = self.message.clone().map(|message| quiet(11., message));
        card.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .child(
                    button("sign-in", "Sign in with Microsoft", Kind::Primary).on_click(
                        cx.listener(|this, _: &ClickEvent, _, cx| {
                            this.dialog = Some(Dialog::SignIn);
                            this.message = None;
                            job::sign_in_now();
                            cx.notify();
                        }),
                    ),
                )
                .when_some(message, |row, message| row.child(message)),
        )
    }

    /// The sign-in's own page: the code Microsoft gave, where to type it, and how it is going.
    fn sign_in_panel(&mut self, cx: &mut Context<Self>) -> Div {
        let status = job::status();
        let code = status.as_ref().and_then(|status| status.code.clone());
        let line = status
            .as_ref()
            .map(|status| status.line.clone())
            .unwrap_or_else(|| "Asking Microsoft for a sign-in code…".to_owned());
        let failed = status.as_ref().is_some_and(|status| status.failed);
        let mut card = div().flex().flex_col().gap(px(12.)).child(self.close_row("Sign in", cx));
        if let Some((code, link)) = code {
            let copy_code = code.clone();
            card = card
                .child(quiet(12., "Enter this code on Microsoft's page:"))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(10.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .h(px(48.))
                                .px(px(16.))
                                .rounded(px(3.))
                                .bg(c(look::BASE))
                                .border_1()
                                .border_color(c(look::BORDER))
                                .text_size(px(26.))
                                .font_weight(FontWeight::BOLD)
                                .child(code),
                        )
                        .child(button("copy", "Copy code", Kind::Secondary).on_click(
                            cx.listener(move |_this, _: &ClickEvent, _, _cx| {
                                if !log::copy(&copy_code) {
                                    log::line("The code could not be put on the clipboard.");
                                }
                            }),
                        )),
                )
                .child(quiet(12., format!("It opens {link} in your browser as well.")));
        }
        card.child(
            div()
                .text_size(px(12.))
                .text_color(if failed { c(look::DANGER) } else { c(look::MUTED) })
                .child(line),
        )
    }
}

impl Render for Fh {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let dialog = self.dialog.map(|dialog| self.dialog_panel(dialog, cx));
        div()
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(c(look::WINDOW))
            .text_color(c(look::INK))
            .text_size(px(13.))
            .child(self.toolbar(cx))
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .child(self.sidebar(cx))
                    .child(self.main_area(cx)),
            )
            .child(self.status_bar(cx))
            .when_some(dialog, |root, dialog| root.child(dialog))
    }
}

/// `base`, or "base 2", "base 3"… when an instance already has it (names ignore case, as
/// `instances::create` does).
fn free_name(base: String, taken: &[String]) -> String {
    let free = |name: &String| !taken.iter().any(|other| other.eq_ignore_ascii_case(name));
    if free(&base) {
        return base;
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(free)
        .unwrap_or(base)
}

/// What an instance plays, in one line: "Horizon", "Minecraft 1.21.4 · Fabric".
fn kind_line(instance: &Instance) -> String {
    match (&instance.java, instance.kind) {
        (Some(java), _) => match &java.loader {
            Some(loader) => format!(
                "Minecraft {} · {} {}",
                java.minecraft,
                loader.kind.name(),
                loader.version
            ),
            None => format!("Minecraft {}", java.minecraft),
        },
        (None, InstanceKind::Default) => "Horizon".to_owned(),
        (None, InstanceKind::Own) => "Horizon instance".to_owned(),
        (None, InstanceKind::Port) => "Ported pack".to_owned(),
        (None, InstanceKind::Pack) => "Modpack".to_owned(),
    }
}

/// The bar's second line: what it plays, and the build it is kept on when it is pinned.
fn detail_of(instance: &Instance) -> String {
    match (&instance.version, instance.java.is_some()) {
        (Some(build), false) => format!("{} · {build}", kind_line(instance)),
        _ => kind_line(instance),
    }
}

/// The Overview's build row.
fn build_line(instance: &Instance) -> String {
    match (&instance.version, &instance.java) {
        (Some(build), _) => build.clone(),
        (None, Some(java)) => match &java.loader {
            Some(loader) => format!("{} {} (managed)", loader.kind.name(), loader.version),
            None => format!("Minecraft {}", java.minecraft),
        },
        (None, None) => "The newest installed Horizon".to_owned(),
    }
}

fn played_line(instance: &Instance) -> String {
    match instance.last_played {
        Some(_) => format!("{} played", inst::play_time(instance.played)),
        None => "Not played yet".to_owned(),
    }
}

/// A row of the Overview: a fixed-width label and its value.
fn field(label: &str, value: String) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(6.))
        .border_b_1()
        .border_color(c(look::RULE))
        .child(div().flex().flex_none().w(px(96.)).child(quiet(11., label.to_owned())))
        .child(div().flex().flex_1().min_w(px(0.)).text_size(px(12.)).child(value))
}

/// The page of an instance that has nothing to show.
fn empty(text: &str) -> AnyElement {
    div()
        .flex()
        .items_center()
        .justify_center()
        .p(px(24.))
        .child(quiet(12., text.to_owned()))
        .into_any_element()
}

/// The status bar's progress bar: the accent filling as far as the download got.
fn progress(fraction: f32) -> AnyElement {
    let width = (120. * fraction.clamp(0., 1.)).max(0.);
    div()
        .flex()
        .flex_none()
        .w(px(120.))
        .h(px(5.))
        .rounded(px(3.))
        .bg(c(look::BUTTON))
        .child(div().w(px(width)).h_full().rounded(px(3.)).bg(c(look::ACCENT)))
        .into_any_element()
}

/// The jars in `folder`, by name.
fn jar_list(folder: &std::path::Path) -> Vec<(String, u64)> {
    let mut jars: Vec<(String, u64)> = std::fs::read_dir(folder)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.path().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("jar"))
        })
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            Some((name, size))
        })
        .collect();
    jars.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    jars
}

/// The last `count` lines of `path`, or none when it isn't there.
fn tail(path: &std::path::Path, count: usize) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let lines: Vec<String> = text.lines().map(|line| line.to_owned()).collect();
    let skip = lines.len().saturating_sub(count);
    lines.into_iter().skip(skip).collect()
}

/// `bytes` as the megabytes a person reads.
fn megabytes(bytes: u64) -> String {
    if bytes >= 1 << 30 {
        format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64)
    } else if bytes >= 1 << 20 {
        format!("{:.1} MB", bytes as f64 / (1u64 << 20) as f64)
    } else if bytes >= 1024 {
        format!("{} KB", bytes / 1024)
    } else {
        format!("{bytes} B")
    }
}

/// `when` as a person reads it: today, yesterday, or how many days ago.
fn when(at: std::time::SystemTime) -> String {
    let Ok(age) = std::time::SystemTime::now().duration_since(at) else {
        return "just now".to_owned();
    };
    let days = age.as_secs() / 86_400;
    match days {
        0 => "today".to_owned(),
        1 => "yesterday".to_owned(),
        _ => format!("{days} days ago"),
    }
}

#[cfg(test)]
mod tests {
    use super::{free_name, jar_list, megabytes, tail};

    #[test]
    fn a_name_is_free_and_numbered_when_it_is_taken() {
        let taken = vec!["Fabric 1.21.4".to_owned(), "my horizon".to_owned()];
        assert_eq!(free_name("Vanilla 1.21.4".to_owned(), &taken), "Vanilla 1.21.4");
        assert_eq!(
            free_name("Fabric 1.21.4".to_owned(), &taken),
            "Fabric 1.21.4 2"
        );
        assert_eq!(free_name("My Horizon".to_owned(), &taken), "My Horizon 2");
    }

    #[test]
    fn a_mods_folder_lists_only_jars() {
        let dir = std::env::temp_dir().join(format!("fh-mods-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        std::fs::write(dir.join("sodium.jar"), b"1234").unwrap();
        std::fs::write(dir.join("notes.txt"), b"hi").unwrap();
        let listed = jar_list(&dir);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].0, "sodium.jar");
        assert_eq!(listed[0].1, 4);
    }

    #[test]
    fn a_log_shows_its_end() {
        let dir = std::env::temp_dir().join(format!("fh-log-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("latest.log");
        std::fs::write(&path, "one\ntwo\nthree\n").unwrap();
        assert_eq!(tail(&path, 2), vec!["two".to_owned(), "three".to_owned()]);
        assert!(tail(&dir.join("missing.log"), 5).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn sizes_read_as_a_person_reads_them() {
        assert_eq!(megabytes(512), "512 B");
        assert_eq!(megabytes(2048), "2 KB");
        assert_eq!(megabytes(5 << 20), "5.0 MB");
        assert_eq!(megabytes(2 << 30), "2.0 GB");
    }
}
