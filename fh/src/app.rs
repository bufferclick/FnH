//! JLac's `MainWindow` built natively for FNH in the Windows 11 (WinUI 3 / `QWindows11Style`)
//! theme: top `mainToolBar` (`Add Instance...`, `Folders`, `Settings...`, `Help`, `Update...`, and
//! right-aligned `Accounts`), central grouped `InstanceView`, right-docked `WideBar`
//! `instanceToolBar` (icon, instance title, `Launch`, `Kill`, `Edit...`, `Change Group...`,
//! `Folder`, `Copy...`, `Delete`), live task strip, and bottom `QStatusBar` with instance summary
//! and `"Total playtime: ..."`.
//!
//! Holding `Ctrl + Shift + Alt` while clicking `Add Instance...` wipes all launcher data (every
//! instance, world, mod, account, cache, preference and log) and immediately relaunches `FH.exe`
//! to a clean fresh-install state.

use crate::icons::icon;
use crate::job::Status;
use crate::look::{
    self, hsep, instance_icon, menu_item, menu_sep, progress_bar, push_button, tool_button, vsep,
    wide_action, ButtonKind, Theme, ICON_PALETTE, UI_FONT,
};
use crate::prefs::{Prefs, SortMode};
use crate::{job, log, reset};
use gpui::{
    div, prelude::*, px, AnyElement, ClickEvent, ClipboardItem, Context, Div, FocusHandle,
    FontWeight, KeyDownEvent, MouseButton, MouseDownEvent, Render, SharedString, Subscription,
    Window,
};
use horizon_launcher::account::{self, Accounts};
use horizon_launcher::backend;
use horizon_launcher::instances::{self as inst, Icon, Instance, Kind as InstanceKind};
use horizon_launcher::java;
use horizon_launcher::loaders::{self, Kind as LoaderKind, Listed, Loader};
use horizon_launcher::minecraft_versions::{self, Kind as McKind, Versions};
use horizon_launcher::worlds::{self, World};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, UNIX_EPOCH};

pub(crate) const SITE_URL: &str = "https://fnh.is-cool.dev";
pub(crate) const REPO_URL: &str = "https://github.com/bufferclick/fnh";
pub(crate) const RELEASES_URL: &str = "https://github.com/bufferclick/fnh/releases/latest";

/// Popup menu currently open over `MainWindow`.
#[derive(Clone, PartialEq)]
pub(crate) enum Menu {
    Folders,
    Help,
    Accounts,
    Instance { id: String, x: f32, y: f32 },
}

/// Active modal dialog over `MainWindow`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialog {
    AddInstance,
    EditInstance,
    Settings,
    Accounts,
    SignIn,
    FabricVersions,
    LauncherLog,
    Rename,
    ChangeGroup,
    CopyInstance,
    ConfirmDelete,
    About,
}

/// Sidebar tab in `Add Instance...` (`NewInstanceDialog`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NewKind {
    Custom,
    Horizon,
}

/// Loader choice for a new Minecraft: Java Edition instance.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum NewLoader {
    Fabric,
    Vanilla,
}

/// Sidebar page inside `Edit...` (`InstanceWindow`).
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum EditPage {
    Log,
    Version,
    Mods,
    Worlds,
    Settings,
}

/// Sidebar page inside `Settings...`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettingsPage {
    Ui,
    Minecraft,
    Launcher,
}

/// Which `text_box` currently receives typed characters and clipboard paste.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Field {
    NewName,
    NewGroup,
    NewVersionFilter,
    ModFilter,
    EditName,
    EditGroup,
    EditMemory,
    EditJvmArgs,
    EditWinW,
    EditWinH,
    EditPreLaunch,
    EditWrapper,
    PromptText,
    FabricMcVersion,
}

/// One mod file inside an instance's `mods/` directory.
#[derive(Clone)]
pub(crate) struct ModEntry {
    pub file_name: String,
    pub display: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub size: u64,
}

/// Async result slot shared with short metadata-loading threads.
type AsyncSlot<T> = Arc<Mutex<Option<Result<T, String>>>>;

/// State of the `Add Instance...` dialog.
pub(crate) struct NewInstanceForm {
    pub kind: NewKind,
    pub name: String,
    pub name_touched: bool,
    pub group: String,
    pub colour_idx: usize,
    pub mc_version: String,
    pub filter: String,
    pub show_releases: bool,
    pub show_snapshots: bool,
    pub show_betas: bool,
    pub show_alphas: bool,
    pub loader: NewLoader,
    pub fabric_version: Option<String>,
    pub creating: bool,
    pub error: Option<String>,
}

impl Default for NewInstanceForm {
    fn default() -> Self {
        Self {
            kind: NewKind::Custom,
            name: String::new(),
            name_touched: false,
            group: String::new(),
            colour_idx: 0,
            mc_version: String::new(),
            filter: String::new(),
            show_releases: true,
            show_snapshots: false,
            show_betas: false,
            show_alphas: false,
            loader: NewLoader::Fabric,
            fabric_version: None,
            creating: false,
            error: None,
        }
    }
}

/// State of the `Edit...` (`InstanceWindow`) dialog.
#[derive(Default)]
pub(crate) struct EditState {
    pub instance_id: String,
    pub page: EditPage,
    pub mods: Vec<ModEntry>,
    pub selected_mod: Option<String>,
    pub mod_filter: String,
    pub worlds: Vec<World>,
    pub selected_world: Option<String>,
    pub log_lines: Vec<String>,
    pub name: String,
    pub group: String,
    pub memory_mb: String,
    pub jvm_args: String,
    pub win_w: String,
    pub win_h: String,
    pub pre_launch: String,
    pub wrapper: String,
    pub working: bool,
    pub notice: Option<(String, bool)>,
}

impl Default for EditPage {
    fn default() -> Self {
        Self::Version
    }
}

/// State of the `Fabric Versions` dialog (`Help -> Fabric Versions...` or `Change Version...`).
#[derive(Default)]
pub(crate) struct FabricDialogState {
    pub mc_version: String,
    pub target_instance: Option<String>,
    pub versions: Vec<Listed>,
    pub selected: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
}

/// The FNH Windows 11 launcher view.
pub struct Fh {
    pub(crate) focus: FocusHandle,
    _appearance: Subscription,
    pub(crate) prefs: Prefs,
    pub(crate) instances: Vec<Instance>,
    pub(crate) selected_id: Option<String>,
    pub(crate) accounts: Accounts,
    pub(crate) selected_account: Option<String>,
    pub(crate) menu: Option<Menu>,
    pub(crate) dialog: Option<Dialog>,
    pub(crate) field: Option<Field>,
    pub(crate) status: Status,
    pub(crate) notice: Option<(String, bool)>,
    last_code_seen: String,
    was_busy: bool,

    // Dialog states.
    pub(crate) new_form: NewInstanceForm,
    pub(crate) edit: EditState,
    pub(crate) settings_page: SettingsPage,
    pub(crate) runtimes: Vec<java::Runtime>,
    pub(crate) fabric_dlg: FabricDialogState,
    pub(crate) launcher_log: Vec<String>,
    pub(crate) prompt_target: String,
    pub(crate) prompt_text: String,
    pub(crate) prompt_error: Option<String>,

    // Metadata caches and background channels.
    pub(crate) mc_versions: Option<Versions>,
    pub(crate) mc_loading: bool,
    pub(crate) mc_error: Option<String>,
    mc_slot: AsyncSlot<Versions>,
    fabric_slot: AsyncSlot<(String, Vec<Listed>)>,
    create_slot: AsyncSlot<Instance>,
    loader_slot: AsyncSlot<Instance>,
}

impl Fh {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus = cx.focus_handle();
        window.focus(&focus, cx);
        let appearance = cx.observe_window_appearance(window, |_, _, cx| cx.notify());

        let prefs = Prefs::load();
        let instances = load_visible_instances(prefs.sort);
        let saved_pick = backend::Settings::load().instance;
        let selected_id = saved_pick
            .filter(|id| instances.iter().any(|inst| &inst.id == id))
            .or_else(|| instances.first().map(|inst| inst.id.clone()));
        let accounts = Accounts::load();
        let selected_account = accounts
            .selected()
            .map(|a| a.uuid.clone())
            .or_else(|| accounts.accounts.first().map(|a| a.uuid.clone()));

        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;
            if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                break;
            }
        })
        .detach();

        Self {
            focus,
            _appearance: appearance,
            prefs,
            instances,
            selected_id,
            accounts,
            selected_account,
            menu: None,
            dialog: None,
            field: None,
            status: job::status().unwrap_or_default(),
            notice: None,
            last_code_seen: String::new(),
            was_busy: false,
            new_form: NewInstanceForm::default(),
            edit: EditState::default(),
            settings_page: SettingsPage::Ui,
            runtimes: Vec::new(),
            fabric_dlg: FabricDialogState::default(),
            launcher_log: Vec::new(),
            prompt_target: String::new(),
            prompt_text: String::new(),
            prompt_error: None,
            mc_versions: None,
            mc_loading: false,
            mc_error: None,
            mc_slot: Arc::new(Mutex::new(None)),
            fabric_slot: Arc::new(Mutex::new(None)),
            create_slot: Arc::new(Mutex::new(None)),
            loader_slot: Arc::new(Mutex::new(None)),
        }
    }

    /// Periodic poll for launch status, Microsoft device-code prompts and metadata fetches.
    fn tick(&mut self, cx: &mut Context<Self>) {
        let mut dirty = false;

        let next = job::status().unwrap_or_default();
        if next != self.status {
            if let Some((code, _)) = next.code.as_ref() {
                if code != &self.last_code_seen {
                    self.last_code_seen = code.clone();
                    let _ = log::copy(code);
                    cx.write_to_clipboard(ClipboardItem::new_string(code.clone()));
                    self.dialog = Some(Dialog::SignIn);
                    self.menu = None;
                }
            } else {
                self.last_code_seen.clear();
            }
            if self.was_busy && !next.busy {
                self.reload_instances();
                self.reload_accounts();
                if self.dialog == Some(Dialog::SignIn) && !next.failed {
                    self.dialog = None;
                    if let Some(acc) = self.accounts.selected() {
                        self.say(format!("Signed in as {}.", acc.name), false);
                    }
                } else if next.done && !next.line.is_empty() {
                    self.say(next.line.clone(), false);
                }
                if self.dialog == Some(Dialog::EditInstance)
                    && self.edit.page == EditPage::Log
                {
                    self.reload_edit_log();
                }
            }
            self.was_busy = next.busy;
            self.status = next;
            dirty = true;
        }

        if let Some(outcome) = take_slot(&self.mc_slot) {
            self.mc_loading = false;
            match outcome {
                Ok(list) => {
                    if self.new_form.mc_version.is_empty() {
                        let pick = list.latest_release.clone();
                        self.pick_new_mc_version(&pick);
                    }
                    if self.fabric_dlg.mc_version.is_empty() {
                        self.fabric_dlg.mc_version = list.latest_release.clone();
                        if self.dialog == Some(Dialog::FabricVersions) {
                            self.fetch_fabric_versions();
                        }
                    }
                    self.mc_versions = Some(list);
                    self.mc_error = None;
                }
                Err(err) => self.mc_error = Some(err),
            }
            dirty = true;
        }

        if let Some(outcome) = take_slot(&self.fabric_slot) {
            self.fabric_dlg.loading = false;
            match outcome {
                Ok((mc, list)) => {
                    if mc == self.fabric_dlg.mc_version {
                        self.fabric_dlg.selected = list
                            .iter()
                            .find(|l| l.stable)
                            .or_else(|| list.first())
                            .map(|l| l.version.clone());
                        self.fabric_dlg.versions = list;
                        self.fabric_dlg.error = None;
                    }
                }
                Err(err) => {
                    self.fabric_dlg.versions.clear();
                    self.fabric_dlg.error = Some(err);
                }
            }
            dirty = true;
        }

        if let Some(outcome) = take_slot(&self.create_slot) {
            self.new_form.creating = false;
            match outcome {
                Ok(created) => {
                    let name = created.name.clone();
                    let id = created.id.clone();
                    self.reload_instances();
                    self.select_instance(Some(id));
                    self.dialog = None;
                    self.field = None;
                    self.say(format!("Created instance \"{name}\"."), false);
                }
                Err(err) => self.new_form.error = Some(err),
            }
            dirty = true;
        }

        if let Some(outcome) = take_slot(&self.loader_slot) {
            self.edit.working = false;
            match outcome {
                Ok(updated) => {
                    let msg = match updated.java.as_ref().and_then(|j| j.loader.as_ref()) {
                        Some(loader) => {
                            format!("Installed {} {}.", loader.kind.name(), loader.version)
                        }
                        None => "Updated instance loader.".into(),
                    };
                    self.reload_instances();
                    self.edit.notice = Some((msg.clone(), false));
                    if self.dialog == Some(Dialog::FabricVersions) {
                        self.dialog = Some(Dialog::EditInstance);
                    }
                    self.say(msg, false);
                }
                Err(err) => {
                    self.edit.notice = Some((err.clone(), true));
                    if self.dialog == Some(Dialog::FabricVersions) {
                        self.fabric_dlg.error = Some(err);
                    }
                }
            }
            dirty = true;
        }

        if dirty {
            cx.notify();
        }
    }

    // ----- Data helpers -------------------------------------------------------------------------

    pub(crate) fn say(&mut self, text: impl Into<String>, is_error: bool) {
        self.notice = Some((text.into(), is_error));
    }

    pub(crate) fn reload_instances(&mut self) {
        self.instances = load_visible_instances(self.prefs.sort);
        if let Some(sel) = &self.selected_id {
            if !self.instances.iter().any(|i| &i.id == sel) {
                self.selected_id = self.instances.first().map(|i| i.id.clone());
            }
        }
    }

    pub(crate) fn reload_accounts(&mut self) {
        self.accounts = Accounts::load();
        if let Some(sel) = &self.selected_account {
            if !self.accounts.accounts.iter().any(|a| &a.uuid == sel) {
                self.selected_account = self
                    .accounts
                    .selected()
                    .map(|a| a.uuid.clone())
                    .or_else(|| self.accounts.accounts.first().map(|a| a.uuid.clone()));
            }
        } else {
            self.selected_account = self
                .accounts
                .selected()
                .map(|a| a.uuid.clone())
                .or_else(|| self.accounts.accounts.first().map(|a| a.uuid.clone()));
        }
    }

    pub(crate) fn selected(&self) -> Option<&Instance> {
        let id = self.selected_id.as_deref()?;
        self.instances.iter().find(|i| i.id == id)
    }

    pub(crate) fn find_instance(&self, id: &str) -> Option<&Instance> {
        self.instances.iter().find(|i| i.id == id)
    }

    pub(crate) fn select_instance(&mut self, id: Option<String>) {
        self.selected_id = id.clone();
        self.notice = None;
        if let Some(id) = id {
            let _ = inst::select(&id);
        }
    }

    pub(crate) fn existing_groups(&self) -> Vec<String> {
        let mut groups: Vec<String> = Vec::new();
        for instance in &self.instances {
            if let Some(group) = instance.group.as_deref().map(str::trim).filter(|g| !g.is_empty())
            {
                if !groups.iter().any(|g| g.eq_ignore_ascii_case(group)) {
                    groups.push(group.to_owned());
                }
            }
        }
        groups.sort_by_key(|g| g.to_ascii_lowercase());
        groups
    }

    // ----- Actions ------------------------------------------------------------------------------

    /// Handles a click on `Add Instance...`: `Ctrl + Shift + Alt` performs a full factory reset
    /// and relaunches `FH.exe`; any ordinary click opens `NewInstanceDialog`.
    pub(crate) fn on_add_instance_click(&mut self, event: &ClickEvent, cx: &mut Context<Self>) {
        self.menu = None;
        if reset::is_reset_click(event) {
            log::line("Factory reset requested (Ctrl+Shift+Alt + Add Instance).");
            match reset::wipe_and_relaunch() {
                Ok(()) => {
                    cx.quit();
                    return;
                }
                Err(err) => {
                    self.say(err, true);
                    cx.notify();
                    return;
                }
            }
        }
        self.open_add_instance();
        cx.notify();
    }

    pub(crate) fn open_add_instance(&mut self) {
        self.menu = None;
        let colour_idx = self.instances.len() % ICON_PALETTE.len();
        let mut form = NewInstanceForm {
            colour_idx,
            ..NewInstanceForm::default()
        };
        if let Some(list) = &self.mc_versions {
            form.mc_version = list.latest_release.clone();
            form.name = list.latest_release.clone();
        } else {
            self.fetch_mc_versions(false);
        }
        self.new_form = form;
        self.dialog = Some(Dialog::AddInstance);
        self.field = Some(Field::NewName);
    }

    pub(crate) fn pick_new_mc_version(&mut self, version: &str) {
        self.new_form.mc_version = version.to_owned();
        self.new_form.fabric_version = None;
        if !self.new_form.name_touched || self.new_form.name.trim().is_empty() {
            self.new_form.name = version.to_owned();
        }
    }

    pub(crate) fn set_new_kind(&mut self, kind: NewKind) {
        self.new_form.kind = kind;
        self.new_form.error = None;
        if !self.new_form.name_touched || self.new_form.name.trim().is_empty() {
            self.new_form.name = match kind {
                NewKind::Horizon => "Horizon".into(),
                NewKind::Custom => self.new_form.mc_version.clone(),
            };
        }
    }

    pub(crate) fn fetch_mc_versions(&mut self, force: bool) {
        if self.mc_loading {
            return;
        }
        if force {
            let cache = backend::data_dir()
                .join("minecraft")
                .join("version_manifest_v2.json");
            let _ = std::fs::remove_file(cache);
        }
        self.mc_loading = true;
        self.mc_error = None;
        let slot = Arc::clone(&self.mc_slot);
        std::thread::spawn(move || {
            let res = minecraft_versions::load();
            if let Ok(mut out) = slot.lock() {
                *out = Some(res);
            }
        });
    }

    pub(crate) fn submit_add_instance(&mut self) {
        if self.new_form.creating {
            return;
        }
        let kind = self.new_form.kind;
        let fallback = match kind {
            NewKind::Horizon => "Horizon".to_owned(),
            NewKind::Custom => self.new_form.mc_version.clone(),
        };
        let raw_name = self.new_form.name.trim();
        let name = if raw_name.is_empty() {
            fallback
        } else {
            raw_name.to_owned()
        };
        if name.trim().is_empty() {
            self.new_form.error = Some("Enter an instance name first.".into());
            return;
        }
        let group = Some(self.new_form.group.trim().to_owned()).filter(|g| !g.is_empty());
        let icon = Some(Icon::Colour(
            ICON_PALETTE[self.new_form.colour_idx % ICON_PALETTE.len()].to_owned(),
        ));

        match kind {
            NewKind::Horizon => {
                let default_active =
                    reset::has_default_horizon_data(&backend::data_dir(), &backend::game_home());
                let result = if !default_active && name.eq_ignore_ascii_case("Horizon") {
                    let _ = std::fs::create_dir_all(backend::game_home());
                    inst::edit("horizon", |meta| {
                        meta.name = Some("Horizon".into());
                        meta.group = group;
                        meta.icon = icon;
                    })
                } else {
                    inst::create(&name, None).and_then(|made| {
                        inst::edit(&made.id, |meta| {
                            meta.group = group;
                            meta.icon = icon;
                        })
                    })
                };
                match result {
                    Ok(created) => {
                        let created_name = created.name.clone();
                        let created_id = created.id.clone();
                        self.reload_instances();
                        self.select_instance(Some(created_id));
                        self.dialog = None;
                        self.field = None;
                        self.say(format!("Created instance \"{created_name}\"."), false);
                    }
                    Err(err) => self.new_form.error = Some(err),
                }
            }
            NewKind::Custom => {
                let mc = self.new_form.mc_version.trim().to_owned();
                if mc.is_empty() {
                    self.new_form.error = Some("Select a Minecraft version from the list.".into());
                    return;
                }
                let loader = self.new_form.loader;
                let fabric_ver = self.new_form.fabric_version.clone();
                self.new_form.creating = true;
                self.new_form.error = None;
                let slot = Arc::clone(&self.create_slot);
                std::thread::spawn(move || {
                    let outcome = (|| -> Result<Instance, String> {
                        let chosen_loader: Option<Loader> = match loader {
                            NewLoader::Vanilla => None,
                            NewLoader::Fabric => {
                                Some(loaders::choose("fabric", &mc, fabric_ver.as_deref())?)
                            }
                        };
                        let made = inst::create_java(&name, &mc)?;
                        inst::edit(&made.id, |meta| {
                            meta.group = group;
                            meta.icon = icon;
                            if let Some(java) = meta.java.as_mut() {
                                java.loader = chosen_loader;
                            }
                        })
                    })();
                    if let Ok(mut out) = slot.lock() {
                        *out = Some(outcome);
                    }
                });
            }
        }
    }

    pub(crate) fn launch_id(&mut self, id: &str) {
        self.menu = None;
        if self.status.busy {
            self.say("An instance or task is already running.", true);
            return;
        }
        let Some(instance) = self.find_instance(id).cloned() else {
            return;
        };
        let _ = inst::select(&instance.id);
        self.selected_id = Some(instance.id.clone());
        job::clear();
        job::play(instance);
        self.status = job::status().unwrap_or_default();
        self.was_busy = self.status.busy;
    }

    pub(crate) fn launch_selected(&mut self) {
        if let Some(id) = self.selected_id.clone() {
            self.launch_id(&id);
        }
    }

    pub(crate) fn kill_running(&mut self) {
        self.menu = None;
        if job::kill() {
            self.status = job::status().unwrap_or_default();
            self.say("Stopping the running instance…", false);
        }
    }

    pub(crate) fn open_edit(&mut self, id: &str, page: EditPage) {
        self.menu = None;
        let Some(instance) = self.find_instance(id).cloned() else {
            return;
        };
        self.selected_id = Some(instance.id.clone());
        let (memory_mb, jvm_args, win_w, win_h) = match &instance.java {
            Some(java) => {
                let (w, h) = java
                    .window
                    .map(|(w, h)| (w.to_string(), h.to_string()))
                    .unwrap_or_default();
                (
                    java.memory_mb
                        .unwrap_or(inst::DEFAULT_MEMORY_MB)
                        .to_string(),
                    java.jvm_args.clone(),
                    w,
                    h,
                )
            }
            None => (String::new(), String::new(), String::new(), String::new()),
        };
        self.edit = EditState {
            instance_id: instance.id.clone(),
            page,
            mods: list_mods(&instance.home),
            selected_mod: None,
            mod_filter: String::new(),
            worlds: worlds::list(&instance.home),
            selected_world: None,
            log_lines: read_tail(&instance.home.join("logs").join("latest.log"), 400),
            name: instance.name.clone(),
            group: instance.group.clone().unwrap_or_default(),
            memory_mb,
            jvm_args,
            win_w,
            win_h,
            pre_launch: instance.launch.pre_launch.clone().unwrap_or_default(),
            wrapper: instance.launch.wrapper.clone().unwrap_or_default(),
            working: false,
            notice: None,
        };
        self.dialog = Some(Dialog::EditInstance);
        self.field = None;
    }

    pub(crate) fn reload_edit_mods(&mut self) {
        if let Some(home) = self.find_instance(&self.edit.instance_id).map(|i| i.home.clone()) {
            self.edit.mods = list_mods(&home);
        }
    }

    pub(crate) fn reload_edit_worlds(&mut self) {
        if let Some(home) = self.find_instance(&self.edit.instance_id).map(|i| i.home.clone()) {
            self.edit.worlds = worlds::list(&home);
        }
    }

    pub(crate) fn reload_edit_log(&mut self) {
        if let Some(home) = self.find_instance(&self.edit.instance_id).map(|i| i.home.clone()) {
            self.edit.log_lines = read_tail(&home.join("logs").join("latest.log"), 400);
        }
    }

    pub(crate) fn toggle_mod(&mut self, file_name: &str) {
        let Some(entry) = self.edit.mods.iter().find(|m| m.file_name == file_name).cloned() else {
            return;
        };
        let next_name = if entry.enabled {
            format!("{}.disabled", entry.file_name)
        } else {
            entry
                .file_name
                .strip_suffix(".disabled")
                .unwrap_or(&entry.file_name)
                .to_owned()
        };
        let target = entry.path.with_file_name(&next_name);
        match std::fs::rename(&entry.path, &target) {
            Ok(()) => {
                self.edit.selected_mod = Some(next_name);
                self.reload_edit_mods();
            }
            Err(err) => self.edit.notice = Some((format!("Could not toggle mod: {err}"), true)),
        }
    }

    pub(crate) fn delete_selected_mod(&mut self) {
        let Some(sel) = self.edit.selected_mod.clone() else {
            return;
        };
        let Some(entry) = self.edit.mods.iter().find(|m| m.file_name == sel).cloned() else {
            return;
        };
        match std::fs::remove_file(&entry.path) {
            Ok(()) => {
                self.edit.selected_mod = None;
                self.reload_edit_mods();
                self.edit.notice = Some((format!("Removed {}.", entry.display), false));
            }
            Err(err) => self.edit.notice = Some((format!("Could not remove mod: {err}"), true)),
        }
    }

    pub(crate) fn install_fabric_on(&mut self, id: &str, version: Option<String>) {
        if self.edit.working {
            return;
        }
        let Some(mc) = self
            .find_instance(id)
            .and_then(|i| i.java.as_ref())
            .map(|j| j.minecraft.clone())
        else {
            return;
        };
        let id = id.to_owned();
        self.edit.working = true;
        self.edit.notice = Some(("Fetching Fabric Loader metadata…".into(), false));
        let slot = Arc::clone(&self.loader_slot);
        std::thread::spawn(move || {
            let outcome = loaders::choose("fabric", &mc, version.as_deref()).and_then(|chosen| {
                inst::edit(&id, |meta| {
                    if let Some(java) = meta.java.as_mut() {
                        java.loader = Some(chosen);
                    }
                })
            });
            if let Ok(mut out) = slot.lock() {
                *out = Some(outcome);
            }
        });
    }

    pub(crate) fn remove_loader_on(&mut self, id: &str) {
        match inst::edit(id, |meta| {
            if let Some(java) = meta.java.as_mut() {
                java.loader = None;
            }
        }) {
            Ok(_) => {
                self.reload_instances();
                self.edit.notice = Some(("Removed mod loader (instance is now Vanilla).".into(), false));
            }
            Err(err) => self.edit.notice = Some((err, true)),
        }
    }

    pub(crate) fn save_edit_settings(&mut self) {
        let id = self.edit.instance_id.clone();
        let Some(current) = self.find_instance(&id).cloned() else {
            return;
        };
        let name = self.edit.name.trim().to_owned();
        if name.is_empty() {
            self.edit.notice = Some(("Instance name cannot be empty.".into(), true));
            return;
        }
        let parsed_memory: Option<u32> = if current.java.is_some() {
            let raw = self.edit.memory_mb.trim();
            if raw.is_empty() {
                None
            } else {
                match raw.parse::<u32>() {
                    Ok(mb) if (256..=65_536).contains(&mb) => Some(mb),
                    _ => {
                        self.edit.notice = Some((
                            "Memory must be a number between 256 and 65536 MiB.".into(),
                            true,
                        ));
                        return;
                    }
                }
            }
        } else {
            None
        };
        let parsed_window: Option<(u32, u32)> = if current.java.is_some() {
            let w = self.edit.win_w.trim();
            let h = self.edit.win_h.trim();
            if w.is_empty() && h.is_empty() {
                None
            } else {
                match (w.parse::<u32>(), h.parse::<u32>()) {
                    (Ok(w), Ok(h)) if w >= 320 && h >= 200 => Some((w, h)),
                    _ => {
                        self.edit.notice = Some((
                            "Window size must be width × height in pixels (at least 320 × 200), or blank."
                                .into(),
                            true,
                        ));
                        return;
                    }
                }
            }
        } else {
            None
        };
        let group = Some(self.edit.group.trim().to_owned()).filter(|g| !g.is_empty());
        let jvm_args = self.edit.jvm_args.trim().to_owned();
        let pre_launch = Some(self.edit.pre_launch.trim().to_owned()).filter(|s| !s.is_empty());
        let wrapper = Some(self.edit.wrapper.trim().to_owned()).filter(|s| !s.is_empty());

        match inst::edit(&id, |meta| {
            meta.name = Some(name);
            meta.group = group;
            meta.launch.pre_launch = pre_launch;
            meta.launch.wrapper = wrapper;
            if let Some(java) = meta.java.as_mut() {
                java.memory_mb = parsed_memory;
                java.jvm_args = jvm_args;
                java.window = parsed_window;
            }
        }) {
            Ok(_) => {
                self.reload_instances();
                self.edit.notice = Some(("Saved instance settings.".into(), false));
            }
            Err(err) => self.edit.notice = Some((err, true)),
        }
    }

    pub(crate) fn cycle_instance_colour(&mut self, id: &str) {
        let Some(current) = self.find_instance(id) else {
            return;
        };
        let next = look::next_palette_colour(current.icon.as_ref());
        if inst::edit(id, |meta| meta.icon = Some(Icon::Colour(next))).is_ok() {
            self.reload_instances();
        }
    }

    pub(crate) fn set_instance_colour(&mut self, id: &str, colour: &str) {
        if inst::edit(id, |meta| meta.icon = Some(Icon::Colour(colour.to_owned()))).is_ok() {
            self.reload_instances();
        }
    }

    pub(crate) fn open_prompt(&mut self, dialog: Dialog, id: &str) {
        self.menu = None;
        let Some(instance) = self.find_instance(id).cloned() else {
            return;
        };
        self.selected_id = Some(instance.id.clone());
        self.prompt_target = instance.id.clone();
        self.prompt_error = None;
        self.prompt_text = match dialog {
            Dialog::Rename => instance.name,
            Dialog::ChangeGroup => instance.group.unwrap_or_default(),
            Dialog::CopyInstance => format!("{} (Copy)", instance.name),
            Dialog::ConfirmDelete => instance.name,
            _ => String::new(),
        };
        self.dialog = Some(dialog);
        self.field = match dialog {
            Dialog::ConfirmDelete => None,
            _ => Some(Field::PromptText),
        };
    }

    pub(crate) fn submit_prompt(&mut self) {
        let Some(kind) = self.dialog else {
            return;
        };
        let id = self.prompt_target.clone();
        match kind {
            Dialog::Rename => {
                let name = self.prompt_text.trim().to_owned();
                match inst::edit(&id, |meta| meta.name = Some(name)) {
                    Ok(updated) => {
                        let label = updated.name;
                        self.reload_instances();
                        self.dialog = None;
                        self.field = None;
                        self.say(format!("Renamed instance to \"{label}\"."), false);
                    }
                    Err(err) => self.prompt_error = Some(err),
                }
            }
            Dialog::ChangeGroup => {
                let group = Some(self.prompt_text.trim().to_owned()).filter(|g| !g.is_empty());
                match inst::edit(&id, |meta| meta.group = group) {
                    Ok(updated) => {
                        let msg = match updated.group {
                            Some(g) => format!("Moved \"{}\" to group \"{g}\".", updated.name),
                            None => format!("Moved \"{}\" to Ungrouped.", updated.name),
                        };
                        self.reload_instances();
                        self.dialog = None;
                        self.field = None;
                        self.say(msg, false);
                    }
                    Err(err) => self.prompt_error = Some(err),
                }
            }
            Dialog::CopyInstance => {
                let name = self.prompt_text.trim().to_owned();
                match inst::create(&name, Some(&id)) {
                    Ok(copy) => {
                        let copy_id = copy.id.clone();
                        let copy_name = copy.name.clone();
                        self.reload_instances();
                        self.select_instance(Some(copy_id));
                        self.dialog = None;
                        self.field = None;
                        self.say(format!("Copied instance to \"{copy_name}\"."), false);
                    }
                    Err(err) => self.prompt_error = Some(err),
                }
            }
            Dialog::ConfirmDelete => {
                let Some(target) = self.find_instance(&id).cloned() else {
                    self.dialog = None;
                    return;
                };
                let outcome = if target.kind == InstanceKind::Default {
                    reset::delete_default_horizon().map(|()| target.name.clone())
                } else {
                    inst::delete(&id).map(|gone| gone.name)
                };
                match outcome {
                    Ok(gone) => {
                        self.reload_instances();
                        self.dialog = None;
                        self.field = None;
                        self.say(format!("Deleted instance \"{gone}\"."), false);
                    }
                    Err(err) => self.prompt_error = Some(err),
                }
            }
            _ => {}
        }
    }

    pub(crate) fn open_settings(&mut self, page: SettingsPage) {
        self.menu = None;
        self.settings_page = page;
        self.refresh_runtimes();
        self.dialog = Some(Dialog::Settings);
        self.field = None;
    }

    pub(crate) fn refresh_runtimes(&mut self) {
        let mut all = java::mojang_runtimes(&java::runtimes_dir());
        for rt in java::installed() {
            if !all.iter().any(|existing| existing.home == rt.home) {
                all.push(rt);
            }
        }
        self.runtimes = all;
    }

    pub(crate) fn open_accounts_dialog(&mut self) {
        self.menu = None;
        self.reload_accounts();
        self.dialog = Some(Dialog::Accounts);
        self.field = None;
    }

    pub(crate) fn start_microsoft_sign_in(&mut self) {
        self.menu = None;
        if self.status.busy {
            if self.status.code.is_some() {
                self.dialog = Some(Dialog::SignIn);
            } else {
                self.say("Wait for the current task to finish before signing in.", true);
            }
            return;
        }
        job::clear();
        job::sign_in_now();
        self.status = job::status().unwrap_or_default();
        self.was_busy = true;
        self.dialog = Some(Dialog::SignIn);
        self.field = None;
    }

    pub(crate) fn switch_account(&mut self, uuid: &str) {
        self.menu = None;
        let mut accounts = Accounts::load();
        if let Err(err) = accounts.play_as(uuid).and_then(|()| accounts.save()) {
            self.say(err, true);
            return;
        }
        self.reload_accounts();
        self.selected_account = Some(uuid.to_owned());
        if let Some(acc) = self.accounts.selected() {
            self.say(format!("Active account: {}.", acc.name), false);
        }
    }

    pub(crate) fn remove_account(&mut self, uuid: &str) {
        let mut accounts = Accounts::load();
        let name = accounts
            .accounts
            .iter()
            .find(|a| a.uuid == uuid)
            .map(|a| a.name.clone())
            .unwrap_or_default();
        if let Err(err) = accounts
            .remove(account::store(), uuid)
            .and_then(|()| accounts.save())
        {
            self.say(err, true);
            return;
        }
        let _ = accounts.remove(account::keychain(), uuid);
        self.reload_accounts();
        self.say(format!("Removed account {name}."), false);
    }

    pub(crate) fn open_fabric_versions(&mut self, target_instance: Option<String>) {
        self.menu = None;
        let mc = target_instance
            .as_deref()
            .and_then(|id| self.find_instance(id))
            .and_then(|i| i.java.as_ref())
            .map(|j| j.minecraft.clone())
            .or_else(|| {
                self.selected()
                    .and_then(|i| i.java.as_ref())
                    .map(|j| j.minecraft.clone())
            })
            .or_else(|| self.mc_versions.as_ref().map(|v| v.latest_release.clone()))
            .unwrap_or_default();
        self.fabric_dlg = FabricDialogState {
            mc_version: mc,
            target_instance,
            versions: Vec::new(),
            selected: None,
            loading: false,
            error: None,
        };
        self.dialog = Some(Dialog::FabricVersions);
        self.field = Some(Field::FabricMcVersion);
        if self.fabric_dlg.mc_version.is_empty() {
            self.fetch_mc_versions(false);
        } else {
            self.fetch_fabric_versions();
        }
    }

    pub(crate) fn fetch_fabric_versions(&mut self) {
        let mc = self.fabric_dlg.mc_version.trim().to_owned();
        if mc.is_empty() || self.fabric_dlg.loading {
            return;
        }
        self.fabric_dlg.loading = true;
        self.fabric_dlg.error = None;
        let slot = Arc::clone(&self.fabric_slot);
        std::thread::spawn(move || {
            let res = loaders::versions(LoaderKind::Fabric, &mc, &backend::get_bytes)
                .map(|list| (mc, list));
            if let Ok(mut out) = slot.lock() {
                *out = Some(res);
            }
        });
    }

    pub(crate) fn clear_metadata_cache(&mut self) {
        self.menu = None;
        let cache = backend::data_dir()
            .join("minecraft")
            .join("version_manifest_v2.json");
        let _ = std::fs::remove_file(cache);
        self.mc_versions = None;
        self.mc_error = None;
        self.fabric_dlg.versions.clear();
        self.say("Cleared cached Minecraft and Fabric version metadata.", false);
    }

    pub(crate) fn open_launcher_log(&mut self) {
        self.menu = None;
        self.launcher_log = read_tail(log::path(), 400);
        self.dialog = Some(Dialog::LauncherLog);
        self.field = None;
    }

    pub(crate) fn open_dir(&mut self, path: PathBuf, create: bool) {
        self.menu = None;
        if create {
            let _ = std::fs::create_dir_all(&path);
        }
        if let Err(err) = log::open_folder(&path) {
            self.say(err, true);
        }
    }

    pub(crate) fn open_web(&mut self, url: &str) {
        self.menu = None;
        if let Err(err) = log::open_url(url) {
            self.say(err, true);
        }
    }

    pub(crate) fn close_dialog(&mut self) {
        self.dialog = None;
        self.field = None;
        self.prompt_error = None;
    }

    // ----- Keyboard input -----------------------------------------------------------------------

    fn active_field_mut(&mut self) -> Option<&mut String> {
        let field = self.field?;
        match field {
            Field::NewName => Some(&mut self.new_form.name),
            Field::NewGroup => Some(&mut self.new_form.group),
            Field::NewVersionFilter => Some(&mut self.new_form.filter),
            Field::ModFilter => Some(&mut self.edit.mod_filter),
            Field::EditName => Some(&mut self.edit.name),
            Field::EditGroup => Some(&mut self.edit.group),
            Field::EditMemory => Some(&mut self.edit.memory_mb),
            Field::EditJvmArgs => Some(&mut self.edit.jvm_args),
            Field::EditWinW => Some(&mut self.edit.win_w),
            Field::EditWinH => Some(&mut self.edit.win_h),
            Field::EditPreLaunch => Some(&mut self.edit.pre_launch),
            Field::EditWrapper => Some(&mut self.edit.wrapper),
            Field::PromptText => Some(&mut self.prompt_text),
            Field::FabricMcVersion => Some(&mut self.fabric_dlg.mc_version),
        }
    }

    fn on_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let ctrl = k.modifiers.control || k.modifiers.platform;

        if k.key == "escape" {
            if self.menu.take().is_some() {
                cx.notify();
                return;
            }
            if self.field.take().is_some() {
                cx.notify();
                return;
            }
            if self.dialog.is_some() {
                self.close_dialog();
                cx.notify();
            }
            return;
        }

        // Focused text box input.
        if let Some(which) = self.field {
            if k.key == "enter" {
                match self.dialog {
                    Some(Dialog::AddInstance) => self.submit_add_instance(),
                    Some(Dialog::Rename | Dialog::ChangeGroup | Dialog::CopyInstance) => {
                        self.submit_prompt()
                    }
                    Some(Dialog::FabricVersions) => self.fetch_fabric_versions(),
                    Some(Dialog::EditInstance) if self.edit.page == EditPage::Settings => {
                        self.save_edit_settings()
                    }
                    _ => {}
                }
                cx.notify();
                return;
            }
            if k.key == "backspace" {
                if ctrl {
                    if let Some(buf) = self.active_field_mut() {
                        buf.clear();
                    }
                } else if let Some(buf) = self.active_field_mut() {
                    buf.pop();
                }
                if which == Field::NewName {
                    self.new_form.name_touched = true;
                }
                cx.notify();
                return;
            }
            if ctrl && k.key == "v" {
                if let Some(clip) = cx.read_from_clipboard().and_then(|c| c.text()) {
                    let clean: String = clip
                        .chars()
                        .filter(|c| !c.is_control())
                        .take(260)
                        .collect();
                    if let Some(buf) = self.active_field_mut() {
                        buf.push_str(&clean);
                    }
                    if which == Field::NewName {
                        self.new_form.name_touched = true;
                    }
                    cx.notify();
                }
                return;
            }
            if !ctrl && !k.modifiers.alt {
                let typed = k.key_char.as_deref().or_else(|| {
                    if k.key == "space" {
                        Some(" ")
                    } else if k.key.len() == 1 {
                        Some(k.key.as_str())
                    } else {
                        None
                    }
                });
                if let Some(ch) = typed {
                    let clean: String = ch.chars().filter(|c| !c.is_control()).collect();
                    if !clean.is_empty() {
                        if let Some(buf) = self.active_field_mut() {
                            if buf.chars().count() < 260 {
                                buf.push_str(&clean);
                            }
                        }
                        if which == Field::NewName {
                            self.new_form.name_touched = true;
                        }
                        cx.notify();
                        return;
                    }
                }
            }
            return;
        }

        // Dialog-level Enter when no field is focused.
        if self.dialog.is_some() {
            if k.key == "enter" {
                match self.dialog {
                    Some(Dialog::AddInstance) => self.submit_add_instance(),
                    Some(Dialog::ConfirmDelete) => self.submit_prompt(),
                    _ => {}
                }
                cx.notify();
            }
            return;
        }

        // JLac MainWindow shortcuts.
        if ctrl && !k.modifiers.alt && !k.modifiers.shift {
            match k.key.as_str() {
                "n" => {
                    self.open_add_instance();
                    cx.notify();
                }
                "k" => {
                    self.kill_running();
                    cx.notify();
                }
                "i" => {
                    if let Some(id) = self.selected_id.clone() {
                        self.open_edit(&id, EditPage::Version);
                        cx.notify();
                    }
                }
                "g" => {
                    if let Some(id) = self.selected_id.clone() {
                        self.open_prompt(Dialog::ChangeGroup, &id);
                        cx.notify();
                    }
                }
                "d" => {
                    if let Some(id) = self.selected_id.clone() {
                        self.open_prompt(Dialog::CopyInstance, &id);
                        cx.notify();
                    }
                }
                _ => {}
            }
            return;
        }

        match k.key.as_str() {
            "enter" => {
                self.launch_selected();
                cx.notify();
            }
            "f2" => {
                if let Some(id) = self.selected_id.clone() {
                    self.open_prompt(Dialog::Rename, &id);
                    cx.notify();
                }
            }
            "f5" => {
                self.reload_instances();
                self.reload_accounts();
                self.say("Refreshed instances.", false);
                cx.notify();
            }
            "delete" => {
                if let Some(id) = self.selected_id.clone() {
                    self.open_prompt(Dialog::ConfirmDelete, &id);
                    cx.notify();
                }
            }
            _ => {}
        }
    }

    // ----- MainWindow rendering -----------------------------------------------------------------

    fn render_top_toolbar(&self, t: &Theme, cx: &mut Context<Self>) -> Div {
        let account_label = self
            .accounts
            .selected()
            .map(|a| a.name.clone())
            .unwrap_or_else(|| "Accounts".into());

        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(4.))
            .w_full()
            .h(px(40.))
            .px(px(6.))
            .bg(t.window)
            .border_b_1()
            .border_color(t.divider)
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(2.))
                    .child(
                        tool_button("tb-add", "add", "Add Instance...", false, false, t)
                            .on_click(cx.listener(Self::on_add_instance_click)),
                    )
                    .child(vsep(t))
                    .child(
                        tool_button(
                            "tb-folders",
                            "folder",
                            "Folders",
                            true,
                            self.menu == Some(Menu::Folders),
                            t,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.menu = if this.menu == Some(Menu::Folders) {
                                None
                            } else {
                                Some(Menu::Folders)
                            };
                            cx.notify();
                        })),
                    )
                    .child(
                        tool_button("tb-settings", "settings", "Settings...", false, false, t)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_settings(SettingsPage::Ui);
                                cx.notify();
                            })),
                    )
                    .child(
                        tool_button(
                            "tb-help",
                            "help",
                            "Help",
                            true,
                            self.menu == Some(Menu::Help),
                            t,
                        )
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.menu = if this.menu == Some(Menu::Help) {
                                None
                            } else {
                                Some(Menu::Help)
                            };
                            cx.notify();
                        })),
                    )
                    .child(
                        tool_button("tb-update", "update", "Update...", false, false, t)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.open_web(RELEASES_URL);
                                cx.notify();
                            })),
                    ),
            )
            .child(
                tool_button(
                    "tb-accounts",
                    "account",
                    account_label,
                    true,
                    self.menu == Some(Menu::Accounts),
                    t,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.reload_accounts();
                    this.menu = if this.menu == Some(Menu::Accounts) {
                        None
                    } else {
                        Some(Menu::Accounts)
                    };
                    cx.notify();
                })),
            )
    }

    fn render_instance_view(&self, t: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        if self.instances.is_empty() {
            return div()
                .id("instance-view-empty")
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .min_h(px(0.))
                .items_center()
                .justify_center()
                .gap(px(12.))
                .p(px(28.))
                .bg(t.layer)
                .child(look::brand_mark(52.))
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.ink)
                        .child("Welcome to FNH"),
                )
                .child(
                    div()
                        .max_w(px(360.))
                        .text_center()
                        .text_size(px(12.5))
                        .text_color(t.ink_secondary)
                        .child(
                            "No instances yet. Click \"Add Instance...\" to create a Fabric or Horizon instance.",
                        ),
                )
                .child(
                    div().mt(px(4.)).child(
                        push_button(
                            "empty-add-instance",
                            "Add Instance...",
                            ButtonKind::Accent,
                            true,
                            t,
                        )
                        .on_click(cx.listener(Self::on_add_instance_click)),
                    ),
                );
        }

        let sections = group_instances(&self.instances);
        let has_named_groups = sections.iter().any(|(name, _)| !name.is_empty());

        let mut children: Vec<AnyElement> = Vec::new();
        for (idx, (group_name, items)) in sections.into_iter().enumerate() {
            if has_named_groups {
                let label = if group_name.is_empty() {
                    "Ungrouped".to_owned()
                } else {
                    group_name.clone()
                };
                let key = label.clone();
                let collapsed = self.prefs.is_collapsed(&key);
                let toggle_key = key.clone();
                let hover = t.subtle_hover;
                children.push(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(
                            div()
                                .id(SharedString::from(format!("grp-hdr-{idx}")))
                                .flex()
                                .items_center()
                                .gap(px(6.))
                                .h(px(26.))
                                .px(px(6.))
                                .rounded(px(4.))
                                .cursor_pointer()
                                .hover(move |s| s.bg(hover))
                                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.prefs.toggle_group(&toggle_key);
                                    this.prefs.save();
                                    cx.notify();
                                }))
                                .child(icon(
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
                                        .flex_none()
                                        .text_size(px(12.5))
                                        .font_weight(FontWeight::SEMIBOLD)
                                        .text_color(t.ink)
                                        .child(label),
                                )
                                .child(
                                    div()
                                        .flex_none()
                                        .text_size(px(11.5))
                                        .text_color(t.ink_tertiary)
                                        .child(format!("({})", items.len())),
                                )
                                .child(div().flex_1().h(px(1.)).ml(px(4.)).bg(t.divider)),
                        )
                        .when(!collapsed, |sec| {
                            sec.child(
                                div()
                                    .flex()
                                    .flex_wrap()
                                    .gap(px(6.))
                                    .px(px(4.))
                                    .children(
                                        items
                                            .into_iter()
                                            .map(|inst| self.render_tile(inst, t, cx)),
                                    ),
                            )
                        })
                        .into_any_element(),
                );
            } else {
                children.push(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap(px(6.))
                        .children(items.into_iter().map(|inst| self.render_tile(inst, t, cx)))
                        .into_any_element(),
                );
            }
        }

        div()
            .id("instance-view")
            .flex()
            .flex_col()
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .gap(px(12.))
            .p(px(10.))
            .bg(t.layer)
            .overflow_y_scroll()
            .on_click(cx.listener(|this, _, _, cx| {
                if this.menu.take().is_some() {
                    cx.notify();
                    return;
                }
                this.select_instance(None);
                cx.notify();
            }))
            .children(children)
    }

    fn render_tile(
        &self,
        instance: &Instance,
        t: &Theme,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let selected = self.selected_id.as_deref() == Some(instance.id.as_str());
        let running = self.status.busy
            && self.status.instance_id.as_deref() == Some(instance.id.as_str());
        let click_id = instance.id.clone();
        let right_id = instance.id.clone();
        let hover = t.subtle_hover;

        div()
            .id(SharedString::from(format!("inst-{}", instance.id)))
            .flex()
            .flex_col()
            .flex_none()
            .items_center()
            .gap(px(6.))
            .w(px(104.))
            .pt(px(10.))
            .pb(px(8.))
            .px(px(6.))
            .rounded(px(4.))
            .bg(if selected {
                t.tile_selected
            } else {
                gpui::transparent_black()
            })
            .border_1()
            .border_color(if selected {
                t.tile_selected_border
            } else {
                gpui::transparent_black()
            })
            .cursor_pointer()
            .when(!selected, |el| el.hover(move |s| s.bg(hover)))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(move |this, event: &ClickEvent, _, cx| {
                cx.stop_propagation();
                this.menu = None;
                let double = match event {
                    ClickEvent::Mouse(m) => m.down.click_count >= 2,
                    _ => false,
                };
                if double {
                    this.launch_id(&click_id);
                } else {
                    this.select_instance(Some(click_id.clone()));
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.select_instance(Some(right_id.clone()));
                    let vw = f32::from(window.viewport_size().width);
                    let vh = f32::from(window.viewport_size().height);
                    let x = f32::from(event.position.x).clamp(6., (vw - 210.).max(6.));
                    let y = f32::from(event.position.y).clamp(6., (vh - 330.).max(6.));
                    this.menu = Some(Menu::Instance { id: right_id.clone(), x, y });
                    cx.notify();
                }),
            )
            .child(instance_icon(instance, 48., running, t))
            .child(
                div()
                    .w_full()
                    .max_h(px(33.))
                    .overflow_hidden()
                    .text_center()
                    .text_size(px(12.))
                    .line_height(px(15.5))
                    .font_weight(if selected {
                        FontWeight::SEMIBOLD
                    } else {
                        FontWeight::NORMAL
                    })
                    .text_color(t.ink)
                    .child(instance.name.clone()),
            )
            .into_any_element()
    }

    fn render_right_toolbar(&self, t: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = self.selected();
        let has_sel = selected.is_some();
        let can_launch = has_sel && !self.status.busy;
        let can_kill = job::can_kill();
        let running_selected = selected.is_some_and(|inst| {
            self.status.busy && self.status.instance_id.as_deref() == Some(inst.id.as_str())
        });
        let hover = t.subtle_hover;

        let header: AnyElement = if let Some(inst) = selected {
            let colour_id = inst.id.clone();
            let rename_id = inst.id.clone();
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(6.))
                .w_full()
                .pt(px(6.))
                .pb(px(4.))
                .child(
                    div()
                        .id("sb-icon")
                        .p(px(4.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.cycle_instance_colour(&colour_id);
                            cx.notify();
                        }))
                        .child(instance_icon(inst, 56., running_selected, t)),
                )
                .child(
                    div()
                        .id("sb-rename")
                        .w_full()
                        .px(px(6.))
                        .py(px(3.))
                        .rounded(px(4.))
                        .cursor_pointer()
                        .hover(move |s| s.bg(hover))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.open_prompt(Dialog::Rename, &rename_id);
                            cx.notify();
                        }))
                        .text_center()
                        .text_size(px(12.5))
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(t.ink)
                        .child(inst.name.clone()),
                )
                .child(
                    div()
                        .w_full()
                        .px(px(6.))
                        .text_center()
                        .text_size(px(11.))
                        .text_color(t.ink_secondary)
                        .truncate()
                        .child(instance_subtitle(inst)),
                )
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(8.))
                .w_full()
                .pt(px(10.))
                .pb(px(6.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(56.))
                        .rounded(px(10.))
                        .bg(t.control)
                        .border_1()
                        .border_color(t.control_border)
                        .child(icon("cube", 26., t.ink_tertiary)),
                )
                .child(
                    div()
                        .w_full()
                        .px(px(6.))
                        .text_center()
                        .text_size(px(12.))
                        .text_color(t.ink_secondary)
                        .child("No Instance Selected"),
                )
                .into_any_element()
        };

        div()
            .id("instance-toolbar")
            .flex()
            .flex_col()
            .flex_none()
            .w(px(166.))
            .min_h(px(0.))
            .p(px(6.))
            .gap(px(2.))
            .bg(t.window)
            .overflow_y_scroll()
            .child(header)
            .child(div().my(px(4.)).child(hsep(t)))
            .child(
                wide_action("act-launch", "play", "Launch", true, can_launch, t).when(
                    can_launch,
                    |b| {
                        b.on_click(cx.listener(|this, _, _, cx| {
                            this.launch_selected();
                            cx.notify();
                        }))
                    },
                ),
            )
            .child(
                wide_action("act-kill", "kill", "Kill", false, can_kill, t).when(can_kill, |b| {
                    b.on_click(cx.listener(|this, _, _, cx| {
                        this.kill_running();
                        cx.notify();
                    }))
                }),
            )
            .child(div().my(px(4.)).child(hsep(t)))
            .child(
                wide_action("act-edit", "edit", "Edit...", false, has_sel, t).when(has_sel, |b| {
                    b.on_click(cx.listener(|this, _, _, cx| {
                        if let Some(id) = this.selected_id.clone() {
                            this.open_edit(&id, EditPage::Version);
                            cx.notify();
                        }
                    }))
                }),
            )
            .child(
                wide_action("act-group", "group", "Change Group...", false, has_sel, t).when(
                    has_sel,
                    |b| {
                        b.on_click(cx.listener(|this, _, _, cx| {
                            if let Some(id) = this.selected_id.clone() {
                                this.open_prompt(Dialog::ChangeGroup, &id);
                                cx.notify();
                            }
                        }))
                    },
                ),
            )
            .child(
                wide_action("act-folder", "folder", "Folder", false, has_sel, t).when(
                    has_sel,
                    |b| {
                        b.on_click(cx.listener(|this, _, _, cx| {
                            if let Some(home) = this.selected().map(|i| i.home.clone()) {
                                this.open_dir(home, true);
                                cx.notify();
                            }
                        }))
                    },
                ),
            )
            .child(
                wide_action("act-copy", "copy", "Copy...", false, has_sel, t).when(has_sel, |b| {
                    b.on_click(cx.listener(|this, _, _, cx| {
                        if let Some(id) = this.selected_id.clone() {
                            this.open_prompt(Dialog::CopyInstance, &id);
                            cx.notify();
                        }
                    }))
                }),
            )
            .child(
                wide_action("act-delete", "delete", "Delete", false, has_sel, t).when(
                    has_sel,
                    |b| {
                        b.on_click(cx.listener(|this, _, _, cx| {
                            if let Some(id) = this.selected_id.clone() {
                                this.open_prompt(Dialog::ConfirmDelete, &id);
                                cx.notify();
                            }
                        }))
                    },
                ),
            )
    }

    fn render_task_banner(&self, t: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        if !self.status.busy && !self.status.failed {
            return None;
        }
        let is_err = self.status.failed;
        let message = if self.status.line.is_empty() {
            self.status.title.clone()
        } else if self.status.title.is_empty() || is_err {
            self.status.line.clone()
        } else {
            format!("{} — {}", self.status.title, self.status.line)
        };

        Some(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_between()
                .gap(px(12.))
                .w_full()
                .px(px(12.))
                .py(px(6.))
                .bg(t.control)
                .border_t_1()
                .border_color(t.divider)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .flex_1()
                        .min_w(px(0.))
                        .child(icon(
                            if is_err { "info" } else { "play" },
                            14.,
                            if is_err { t.danger } else { t.accent },
                        ))
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .truncate()
                                .text_size(px(12.))
                                .text_color(if is_err { t.danger } else { t.ink })
                                .child(message),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .gap(px(8.))
                        .when_some(self.status.fraction, |row, frac| {
                            row.child(progress_bar(frac, 140., t))
                        })
                        .when(self.status.code.is_some(), |row| {
                            row.child(
                                push_button("banner-code", "Sign In...", ButtonKind::Accent, true, t)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.dialog = Some(Dialog::SignIn);
                                        cx.notify();
                                    })),
                            )
                        })
                        .when(job::can_kill(), |row| {
                            row.child(
                                push_button("banner-kill", "Kill", ButtonKind::Standard, true, t)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.kill_running();
                                        cx.notify();
                                    })),
                            )
                        })
                        .when(is_err, |row| {
                            row.child(
                                push_button("banner-log", "View Log", ButtonKind::Standard, true, t)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.open_launcher_log();
                                        cx.notify();
                                    })),
                            )
                            .child(
                                push_button(
                                    "banner-dismiss",
                                    "Dismiss",
                                    ButtonKind::Subtle,
                                    true,
                                    t,
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    job::clear();
                                    this.status = job::status().unwrap_or_default();
                                    cx.notify();
                                })),
                            )
                        }),
                ),
        )
    }

    fn render_status_bar(&self, t: &Theme) -> Div {
        let total: Duration = self.instances.iter().map(|i| i.played).sum();
        let (left_text, left_colour) = if let Some((msg, err)) = &self.notice {
            (msg.clone(), if *err { t.danger } else { t.ink_secondary })
        } else if let Some(inst) = self.selected() {
            (instance_status_line(inst), t.ink_secondary)
        } else {
            ("No instance selected".to_owned(), t.ink_secondary)
        };

        div()
            .flex()
            .flex_none()
            .items_center()
            .justify_between()
            .gap(px(12.))
            .w_full()
            .h(px(26.))
            .px(px(10.))
            .bg(t.window)
            .border_t_1()
            .border_color(t.divider)
            .text_size(px(11.5))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .truncate()
                    .text_color(left_colour)
                    .child(left_text),
            )
            .child(
                div()
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .child(div().w(px(1.)).h(px(14.)).bg(t.divider))
                    .child(
                        div()
                            .whitespace_nowrap()
                            .text_color(t.ink_secondary)
                            .child(format!("Total playtime: {}", look::playtime(total))),
                    ),
            )
    }

    fn render_menu_overlay(&self, t: &Theme, cx: &mut Context<Self>) -> Option<AnyElement> {
        let menu = self.menu.as_ref()?;
        let has_sel = self.selected().is_some();

        let positioned = match menu {
            Menu::Folders => {
                let rows = vec![
                    menu_item(
                        "m-f-inst",
                        Some("folder"),
                        "View Instance Folder",
                        None,
                        false,
                        has_sel,
                        t,
                    )
                    .when(has_sel, |el| {
                        el.on_click(cx.listener(|this, _, _, cx| {
                            if let Some(home) = this.selected().map(|i| i.home.clone()) {
                                this.open_dir(home, true);
                                cx.notify();
                            }
                        }))
                    })
                    .into_any_element(),
                    menu_item(
                        "m-f-mods",
                        Some("mods"),
                        "View Instance Mods Folder",
                        None,
                        false,
                        has_sel,
                        t,
                    )
                    .when(has_sel, |el| {
                        el.on_click(cx.listener(|this, _, _, cx| {
                            if let Some(home) = this.selected().map(|i| i.home.clone()) {
                                this.open_dir(home.join("mods"), true);
                                cx.notify();
                            }
                        }))
                    })
                    .into_any_element(),
                    menu_item(
                        "m-f-saves",
                        Some("world"),
                        "View Instance Saves Folder",
                        None,
                        false,
                        has_sel,
                        t,
                    )
                    .when(has_sel, |el| {
                        el.on_click(cx.listener(|this, _, _, cx| {
                            if let Some(home) = this.selected().map(|i| i.home.clone()) {
                                this.open_dir(home.join("saves"), true);
                                cx.notify();
                            }
                        }))
                    })
                    .into_any_element(),
                    menu_sep(t),
                    menu_item(
                        "m-f-root",
                        Some("folder"),
                        "View Launcher Root Folder",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_dir(backend::data_dir(), true);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "m-f-game",
                        Some("cube"),
                        "View Instances Root Folder",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_dir(backend::game_home(), true);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "m-f-logs",
                        Some("log"),
                        "View Launcher Logs Folder",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_dir(backend::data_dir().join("fh"), true);
                        cx.notify();
                    }))
                    .into_any_element(),
                ];
                div()
                    .absolute()
                    .top(px(38.))
                    .left(px(134.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(look::flyout_card(236., rows, t))
            }
            Menu::Help => {
                let rows = vec![
                    menu_item(
                        "m-h-cache",
                        Some("refresh"),
                        "Clear Metadata Cache",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.clear_metadata_cache();
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "m-h-fabric",
                        Some("fabric"),
                        "Fabric Versions...",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_fabric_versions(None);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "m-h-log",
                        Some("log"),
                        "View Launcher Log...",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_launcher_log();
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_sep(t),
                    menu_item(
                        "m-h-site",
                        Some("external"),
                        "FNH Website",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_web(SITE_URL);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "m-h-repo",
                        Some("external"),
                        "Source Code (GitHub)",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_web(REPO_URL);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_sep(t),
                    menu_item("m-h-about", Some("info"), "About FNH", None, false, true, t)
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.menu = None;
                            this.dialog = Some(Dialog::About);
                            this.field = None;
                            cx.notify();
                        }))
                        .into_any_element(),
                ];
                div()
                    .absolute()
                    .top(px(38.))
                    .left(px(310.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(look::flyout_card(224., rows, t))
            }
            Menu::Accounts => {
                let mut rows: Vec<AnyElement> = Vec::new();
                if self.accounts.accounts.is_empty() {
                    rows.push(
                        menu_item(
                            "m-a-empty",
                            Some("account"),
                            "No Default Account",
                            None,
                            false,
                            false,
                            t,
                        )
                        .into_any_element(),
                    );
                } else {
                    let active = self.accounts.selected.as_deref();
                    for (idx, acc) in self.accounts.accounts.iter().enumerate() {
                        let uuid = acc.uuid.clone();
                        let is_active = active == Some(acc.uuid.as_str());
                        rows.push(
                            menu_item(
                                format!("m-a-{idx}"),
                                Some("account"),
                                acc.name.clone(),
                                None,
                                is_active,
                                true,
                                t,
                            )
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.switch_account(&uuid);
                                cx.notify();
                            }))
                            .into_any_element(),
                        );
                    }
                }
                rows.push(menu_sep(t));
                rows.push(
                    menu_item(
                        "m-a-add",
                        Some("add"),
                        "Add Microsoft...",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.start_microsoft_sign_in();
                        cx.notify();
                    }))
                    .into_any_element(),
                );
                rows.push(
                    menu_item(
                        "m-a-manage",
                        Some("settings"),
                        "Manage Accounts...",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.open_accounts_dialog();
                        cx.notify();
                    }))
                    .into_any_element(),
                );
                div()
                    .absolute()
                    .top(px(38.))
                    .right(px(6.))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(look::flyout_card(224., rows, t))
            }
            Menu::Instance { id, x, y } => {
                let id_launch = id.clone();
                let id_edit = id.clone();
                let id_mods = id.clone();
                let id_group = id.clone();
                let id_rename = id.clone();
                let id_colour = id.clone();
                let id_folder = id.clone();
                let id_copy = id.clone();
                let id_delete = id.clone();
                let can_launch = !self.status.busy;
                let can_kill = job::can_kill()
                    && job::active_instance().as_deref() == Some(id.as_str());
                let rows = vec![
                    menu_item(
                        "ctx-launch",
                        Some("play"),
                        "Launch",
                        Some("Enter"),
                        false,
                        can_launch,
                        t,
                    )
                    .when(can_launch, |el| {
                        el.on_click(cx.listener(move |this, _, _, cx| {
                            this.launch_id(&id_launch);
                            cx.notify();
                        }))
                    })
                    .into_any_element(),
                    menu_item(
                        "ctx-kill",
                        Some("kill"),
                        "Kill",
                        Some("Ctrl+K"),
                        false,
                        can_kill,
                        t,
                    )
                    .when(can_kill, |el| {
                        el.on_click(cx.listener(|this, _, _, cx| {
                            this.kill_running();
                            cx.notify();
                        }))
                    })
                    .into_any_element(),
                    menu_sep(t),
                    menu_item(
                        "ctx-edit",
                        Some("edit"),
                        "Edit...",
                        Some("Ctrl+I"),
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_edit(&id_edit, EditPage::Version);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "ctx-mods",
                        Some("mods"),
                        "Manage Mods...",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_edit(&id_mods, EditPage::Mods);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "ctx-group",
                        Some("group"),
                        "Change Group...",
                        Some("Ctrl+G"),
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_prompt(Dialog::ChangeGroup, &id_group);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "ctx-rename",
                        Some("edit"),
                        "Rename...",
                        Some("F2"),
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_prompt(Dialog::Rename, &id_rename);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "ctx-colour",
                        Some("palette"),
                        "Change Icon Colour",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.menu = None;
                        this.cycle_instance_colour(&id_colour);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_sep(t),
                    menu_item(
                        "ctx-folder",
                        Some("folder"),
                        "Folder",
                        None,
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if let Some(home) = this.find_instance(&id_folder).map(|i| i.home.clone()) {
                            this.open_dir(home, true);
                            cx.notify();
                        }
                    }))
                    .into_any_element(),
                    menu_item(
                        "ctx-copy",
                        Some("copy"),
                        "Copy...",
                        Some("Ctrl+D"),
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_prompt(Dialog::CopyInstance, &id_copy);
                        cx.notify();
                    }))
                    .into_any_element(),
                    menu_item(
                        "ctx-delete",
                        Some("delete"),
                        "Delete",
                        Some("Del"),
                        false,
                        true,
                        t,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_prompt(Dialog::ConfirmDelete, &id_delete);
                        cx.notify();
                    }))
                    .into_any_element(),
                ];
                div()
                    .absolute()
                    .left(px(*x))
                    .top(px(*y))
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .child(look::flyout_card(206., rows, t))
            }
        };

        Some(
            div()
                .id("menu-backdrop")
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        this.menu = None;
                        cx.notify();
                    }),
                )
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(|this, _, _, cx| {
                        this.menu = None;
                        cx.notify();
                    }),
                )
                .child(positioned)
                .into_any_element(),
        )
    }
}

impl Render for Fh {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let t = Theme::resolve(self.prefs.theme, self.prefs.accent, window.appearance());

        let top = self.render_top_toolbar(&t, cx);
        let center = self.render_instance_view(&t, cx);
        let right = self.render_right_toolbar(&t, cx);
        let banner = self.render_task_banner(&t, cx);
        let status = self.prefs.status_bar.then(|| self.render_status_bar(&t));
        let menu = self.render_menu_overlay(&t, cx);
        let dialog = self.render_dialog(&t, cx);

        div()
            .id("fh-root")
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|this, ev: &KeyDownEvent, _, cx| this.on_key(ev, cx)))
            .relative()
            .flex()
            .flex_col()
            .size_full()
            .bg(t.window)
            .text_color(t.ink)
            .font_family(UI_FONT)
            .overflow_hidden()
            .child(top)
            .child(
                div()
                    .flex()
                    .flex_1()
                    .min_h(px(0.))
                    .min_w(px(0.))
                    .child(center)
                    .child(div().flex_none().w(px(1.)).h_full().bg(t.divider))
                    .child(right),
            )
            .when_some(banner, |root, bar| root.child(bar))
            .when_some(status, |root, bar| root.child(bar))
            .when_some(menu, |root, overlay| root.child(overlay))
            .when_some(dialog, |root, modal| root.child(modal))
    }
}

// ----- Free helpers -----------------------------------------------------------------------------

fn take_slot<T>(slot: &AsyncSlot<T>) -> Option<Result<T, String>> {
    slot.lock().ok().and_then(|mut guard| guard.take())
}

/// Lists all real instances on disk (hiding the untouched synthetic default entry on fresh
/// installs and after a factory reset) and applies the user's sort order.
fn load_visible_instances(sort: SortMode) -> Vec<Instance> {
    let show_default = reset::has_default_horizon_data(&backend::data_dir(), &backend::game_home());
    let mut all: Vec<Instance> = inst::list()
        .into_iter()
        .filter(|i| show_default || i.kind != InstanceKind::Default)
        .collect();
    match sort {
        SortMode::Name => {
            all.sort_by_key(|i| i.name.to_ascii_lowercase());
        }
        SortMode::LastLaunched => {
            all.sort_by(|a, b| {
                let ta = a
                    .last_played
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let tb = b
                    .last_played
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                tb.cmp(&ta)
                    .then_with(|| a.name.to_ascii_lowercase().cmp(&b.name.to_ascii_lowercase()))
            });
        }
    }
    all
}

/// Groups instances into ordered `(group_name, items)` buckets, with named groups A–Z followed by
/// `""` (`Ungrouped`).
fn group_instances(instances: &[Instance]) -> Vec<(String, Vec<&Instance>)> {
    let mut named: Vec<(String, Vec<&Instance>)> = Vec::new();
    let mut ungrouped: Vec<&Instance> = Vec::new();
    for inst in instances {
        match inst.group.as_deref().map(str::trim).filter(|g| !g.is_empty()) {
            Some(group) => {
                if let Some((_, bucket)) = named
                    .iter_mut()
                    .find(|(existing, _)| existing.eq_ignore_ascii_case(group))
                {
                    bucket.push(inst);
                } else {
                    named.push((group.to_owned(), vec![inst]));
                }
            }
            None => ungrouped.push(inst),
        }
    }
    named.sort_by_key(|(name, _)| name.to_ascii_lowercase());
    if !ungrouped.is_empty() {
        named.push((String::new(), ungrouped));
    }
    named
}

/// Short version label shown under the selected instance in `instanceToolBar`.
pub(crate) fn instance_subtitle(instance: &Instance) -> String {
    match &instance.java {
        Some(java) => match &java.loader {
            Some(loader) => format!("{} · {}", java.minecraft, loader.kind.name()),
            None => format!("Minecraft {}", java.minecraft),
        },
        None => match &instance.version {
            Some(pinned) => format!("Horizon {pinned}"),
            None => "Horizon".into(),
        },
    }
}

/// JLac status-bar line describing the selected instance.
pub(crate) fn instance_status_line(instance: &Instance) -> String {
    let kind = match &instance.java {
        Some(java) => match &java.loader {
            Some(loader) => format!(
                "Minecraft {} ({} {})",
                java.minecraft,
                loader.kind.name(),
                loader.version
            ),
            None => format!("Minecraft {} (Vanilla)", java.minecraft),
        },
        None => match &instance.version {
            Some(pinned) => format!("Horizon ({pinned})"),
            None => "Horizon (latest)".into(),
        },
    };
    if instance.played.is_zero() {
        format!("{kind}, never played")
    } else {
        format!("{kind}, played for {}", look::playtime(instance.played))
    }
}

/// Lists `.jar` and `.jar.disabled` mods inside `home/mods`.
pub(crate) fn list_mods(home: &Path) -> Vec<ModEntry> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(home.join("mods")).into_iter().flatten().flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
            continue;
        };
        let lower = file_name.to_ascii_lowercase();
        let (enabled, display) = if lower.ends_with(".jar") {
            (true, file_name[..file_name.len() - 4].to_owned())
        } else if lower.ends_with(".jar.disabled") {
            (false, file_name[..file_name.len() - 13].to_owned())
        } else {
            continue;
        };
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        out.push(ModEntry {
            file_name,
            display,
            path,
            enabled,
            size,
        });
    }
    out.sort_by_key(|m| m.display.to_ascii_lowercase());
    out
}

/// Reads the trailing `keep` lines of a text file for console/log tabs.
pub(crate) fn read_tail(path: &Path, keep: usize) -> Vec<String> {
    let Ok(bytes) = std::fs::read(path) else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let all: Vec<&str> = text.lines().collect();
    let start = all.len().saturating_sub(keep);
    all[start..].iter().map(|line| (*line).to_owned()).collect()
}

/// Human-readable file size (`412 KB`, `3.4 MB`).
pub(crate) fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let b = bytes as f64;
    if bytes < 1024 {
        format!("{bytes} B")
    } else if b < MB {
        format!("{:.0} KB", b / KB)
    } else if b < GB {
        format!("{:.1} MB", b / MB)
    } else {
        format!("{:.2} GB", b / GB)
    }
}

/// Human-readable Minecraft release category for the `Add Instance` version table.
pub(crate) fn mc_kind_label(kind: McKind) -> &'static str {
    match kind {
        McKind::Release => "Release",
        McKind::Snapshot => "Snapshot",
        McKind::OldBeta => "Beta",
        McKind::OldAlpha => "Alpha",
    }
}
