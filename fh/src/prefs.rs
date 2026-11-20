//! Global launcher settings (`<FnH>/settings.json`), per-instance overrides (`<FnH>/instances_cfg.json`)
//! for both Fabric/Java and Horizon instances, and non-destructive `options.txt` editing.
//!
//! Covers JLac's complete Settings suite (Launcher features/UI/console/folders, Appearance,
//! Minecraft window/time/commands/environment/tweaks, Java memory/runtime/JVM arguments,
//! Horizon engine channel/renderer/privacy/debug defaults, Accounts/offline, APIs, External Tools,
//! and Proxy) plus complete per-instance overrides.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

/// Windows 11 widget theme mode.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    #[default]
    System,
    Dark,
    OledDark,
    Light,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 4] = [
        ThemeMode::System,
        ThemeMode::Dark,
        ThemeMode::OledDark,
        ThemeMode::Light,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "Windows 11 (System)",
            ThemeMode::Dark => "Windows 11 Dark",
            ThemeMode::OledDark => "Windows 11 OLED Black",
            ThemeMode::Light => "Windows 11 Light",
        }
    }
}

/// Windows 11 Fluent accent palette (contrast-matched for light & dark surfaces).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Accent {
    #[default]
    Blue,
    Navy,
    Green,
    Purple,
    Teal,
    Gold,
    Crimson,
    Graphite,
}

impl Accent {
    pub const ALL: [Accent; 8] = [
        Accent::Blue,
        Accent::Navy,
        Accent::Green,
        Accent::Purple,
        Accent::Teal,
        Accent::Gold,
        Accent::Crimson,
        Accent::Graphite,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Accent::Blue => "Windows Blue",
            Accent::Navy => "FnH Navy",
            Accent::Green => "Emerald",
            Accent::Purple => "Iris",
            Accent::Teal => "Teal",
            Accent::Gold => "Amber",
            Accent::Crimson => "Crimson",
            Accent::Graphite => "Graphite",
        }
    }

    /// `(fill, hover, on_accent)` in `#rrggbb`.
    pub fn colours(self, dark: bool) -> (u32, u32, u32) {
        match (self, dark) {
            (Accent::Blue, false) => (0x0067c0, 0x1975c5, 0xffffff),
            (Accent::Blue, true) => (0x60cdff, 0x56b8e6, 0x000000),
            (Accent::Navy, false) => (0x1f2278, 0x2b2f8e, 0xffffff),
            (Accent::Navy, true) => (0x939bff, 0x828ae8, 0x0b0c1e),
            (Accent::Green, false) => (0x0f7b0f, 0x218721, 0xffffff),
            (Accent::Green, true) => (0x6ccb5f, 0x5fb853, 0x071206),
            (Accent::Purple, false) => (0x5c43b8, 0x6b54c2, 0xffffff),
            (Accent::Purple, true) => (0xb4a0ff, 0xa28ee8, 0x0e0a1c),
            (Accent::Teal, false) => (0x00786c, 0x0d8a7d, 0xffffff),
            (Accent::Teal, true) => (0x4dc9b0, 0x3eb59d, 0x041411),
            (Accent::Gold, false) => (0x9a5b00, 0xad6908, 0xffffff),
            (Accent::Gold, true) => (0xfce100, 0xe3cb00, 0x141100),
            (Accent::Crimson, false) => (0xc42b1c, 0xcc3f31, 0xffffff),
            (Accent::Crimson, true) => (0xff99a4, 0xe88792, 0x1a0608),
            (Accent::Graphite, false) => (0x4a545e, 0x596571, 0xffffff),
            (Accent::Graphite, true) => (0xa8b4c2, 0x96a3b2, 0x0c1014),
        }
    }
}

/// JLac's `InstSortMode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SortMode {
    #[default]
    Name,
    LastLaunched,
    PlayTime,
}

impl SortMode {
    pub const ALL: [SortMode; 3] = [SortMode::Name, SortMode::LastLaunched, SortMode::PlayTime];

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Name => "By name",
            SortMode::LastLaunched => "By last launched",
            SortMode::PlayTime => "By total time played",
        }
    }
}

/// Instance grid display density in `MainWindow`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ViewMode {
    #[default]
    Grid,
    List,
}

impl ViewMode {
    pub const ALL: [ViewMode; 2] = [ViewMode::Grid, ViewMode::List];

    pub fn label(self) -> &'static str {
        match self {
            ViewMode::Grid => "Icon grid",
            ViewMode::List => "Details list",
        }
    }
}

/// JLac mod update release channel filter.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateChannel {
    #[default]
    Release,
    ReleaseAndBeta,
    All,
}

impl UpdateChannel {
    pub const ALL: [UpdateChannel; 3] = [
        UpdateChannel::Release,
        UpdateChannel::ReleaseAndBeta,
        UpdateChannel::All,
    ];

    pub fn label(self) -> &'static str {
        match self {
            UpdateChannel::Release => "Release only",
            UpdateChannel::ReleaseAndBeta => "Release and Beta",
            UpdateChannel::All => "All (Release, Beta, Alpha)",
        }
    }
}

/// Horizon engine graphics API (`WGPU_BACKEND`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuBackend {
    #[default]
    Auto,
    Vulkan,
    Dx12,
    Gl,
}

impl GpuBackend {
    pub const ALL: [GpuBackend; 4] = [
        GpuBackend::Auto,
        GpuBackend::Vulkan,
        GpuBackend::Dx12,
        GpuBackend::Gl,
    ];

    pub fn label(self) -> &'static str {
        match self {
            GpuBackend::Auto => "Automatic (Recommended)",
            GpuBackend::Vulkan => "Vulkan",
            GpuBackend::Dx12 => "DirectX 12",
            GpuBackend::Gl => "OpenGL",
        }
    }

    pub fn env_value(self) -> Option<&'static str> {
        match self {
            GpuBackend::Auto => None,
            GpuBackend::Vulkan => Some("vulkan"),
            GpuBackend::Dx12 => Some("dx12"),
            GpuBackend::Gl => Some("gl"),
        }
    }
}

/// Horizon GPU adapter preference (`WGPU_POWER_PREF`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GpuPower {
    #[default]
    Auto,
    High,
    Low,
}

impl GpuPower {
    pub const ALL: [GpuPower; 3] = [GpuPower::Auto, GpuPower::High, GpuPower::Low];

    pub fn label(self) -> &'static str {
        match self {
            GpuPower::Auto => "System default",
            GpuPower::High => "High performance (Discrete GPU)",
            GpuPower::Low => "Power saving (Integrated GPU)",
        }
    }

    pub fn env_value(self) -> Option<&'static str> {
        match self {
            GpuPower::Auto => None,
            GpuPower::High => Some("high"),
            GpuPower::Low => Some("low"),
        }
    }
}

/// Horizon release channel preference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HorizonTrack {
    #[default]
    Default,
    Release,
    Nightly,
    Pinned,
}

impl HorizonTrack {
    pub const ALL: [HorizonTrack; 4] = [
        HorizonTrack::Default,
        HorizonTrack::Release,
        HorizonTrack::Nightly,
        HorizonTrack::Pinned,
    ];

    pub fn label(self) -> &'static str {
        match self {
            HorizonTrack::Default => "Follow global Horizon setting",
            HorizonTrack::Release => "Stable Release",
            HorizonTrack::Nightly => "Nightly (latest builds)",
            HorizonTrack::Pinned => "Pinned installed version",
        }
    }
}

/// Launcher HTTP/SOCKS5 proxy mode (`ProxyPage`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProxyMode {
    #[default]
    None,
    System,
    Http,
    Socks5,
}

impl ProxyMode {
    pub const ALL: [ProxyMode; 4] = [
        ProxyMode::None,
        ProxyMode::System,
        ProxyMode::Http,
        ProxyMode::Socks5,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ProxyMode::None => "None (direct connection)",
            ProxyMode::System => "Use system proxy settings",
            ProxyMode::Http => "HTTP proxy",
            ProxyMode::Socks5 => "SOCKS5 proxy",
        }
    }
}

/// Standard resolution presets offered in global & instance settings.
pub const WINDOW_PRESETS: &[(&str, u32, u32)] = &[
    ("854×480", 854, 480),
    ("1280×720", 1280, 720),
    ("1600×900", 1600, 900),
    ("1920×1080", 1920, 1080),
    ("2560×1440", 2560, 1440),
];

/// Standard RAM presets (MiB) offered in global & instance Java settings.
pub const MEMORY_PRESETS_MB: &[u32] = &[2048, 4096, 6144, 8192, 12288, 16384];

/// Curated 1-click JVM flag profiles in Java settings.
pub const JVM_FLAG_PRESETS: &[(&str, &str)] = &[
    (
        "Aikar's G1GC",
        "-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:+AlwaysPreTouch -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20",
    ),
    (
        "Generational ZGC (Java 21+)",
        "-XX:+UseZGC -XX:+ZGenerational -XX:+AlwaysPreTouch -XX:+DisableExplicitGC",
    ),
    (
        "Compact Low-RAM",
        "-XX:+UseG1GC -XX:MaxGCPauseMillis=100 -XX:+UseStringDeduplication",
    ),
];

/// Complete global launcher preferences (`<FnH>/settings.json`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    // --- Appearance ---
    pub theme_mode: ThemeMode,
    pub accent: Accent,
    pub console_font_size: u32,
    pub show_status_bar: bool,
    pub show_group_counts: bool,

    // --- Launcher -> User Interface ---
    pub sort_mode: SortMode,
    pub view_mode: ViewMode,
    pub icon_size: u32,
    pub double_click_edits: bool,
    pub confirm_delete: bool,
    pub confirm_kill: bool,
    pub remember_selected_instance: bool,
    pub collapsed_groups: Vec<String>,

    // --- Launcher -> Features & Tasks ---
    pub check_updates_on_start: bool,
    pub update_interval_hours: u32,
    pub download_on_create: bool,
    pub copy_template_on_create: bool,
    pub auto_install_deps: bool,
    pub track_mod_metadata: bool,
    pub detect_mod_incompat: bool,
    pub mod_update_channel: UpdateChannel,
    pub concurrent_downloads: u32,
    pub concurrent_tasks: u32,
    pub http_timeout_secs: u32,
    pub network_retries: u32,

    // --- Launcher -> Console ---
    pub console_max_lines: usize,
    pub console_stop_on_overflow: bool,
    pub show_console_on_launch: bool,
    pub close_console_on_exit: bool,
    pub show_console_on_crash: bool,
    pub console_highlight_levels: bool,
    pub console_word_wrap: bool,
    pub console_timestamps: bool,

    // --- Minecraft -> General ---
    pub start_maximized: bool,
    pub start_fullscreen: bool,
    pub window_width: u32,
    pub window_height: u32,
    pub hide_launcher_on_launch: bool,
    pub quit_launcher_on_exit: bool,
    pub record_playtime: bool,
    pub show_instance_playtime: bool,
    pub show_total_playtime: bool,
    pub playtime_always_hours: bool,
    pub show_snapshots_default: bool,
    pub show_old_versions_default: bool,

    // --- Java Defaults ---
    pub java_min_mem_mb: u32,
    pub java_max_mem_mb: u32,
    pub java_permgen_mb: u32,
    pub warn_low_memory: bool,
    pub auto_download_java: bool,
    pub auto_detect_java: bool,
    pub skip_java_compat: bool,
    pub default_java_path: String,
    pub default_jvm_args: String,

    // --- Horizon Global Defaults ---
    /// `true` = prefer Nightly builds by default; `false` = prefer stable Release builds.
    pub horizon_prefer_nightly: bool,
    pub horizon_auto_update_mods: bool,
    pub horizon_verify_assets: bool,
    pub horizon_start_creative: bool,
    pub horizon_gpu_backend: GpuBackend,
    pub horizon_gpu_power: GpuPower,
    pub horizon_rust_backtrace: bool,
    pub horizon_rust_log: String,
    pub horizon_extra_args: String,
    pub horizon_telemetry_opt_out: bool,
    pub horizon_no_ai: bool,

    // --- Accounts / Offline ---
    pub offline_player_name: String,

    // --- Global Custom Commands & Environment ---
    pub pre_launch_cmd: String,
    pub wrapper_cmd: String,
    pub post_exit_cmd: String,
    pub global_env: Vec<(String, String)>,

    // --- Global Tweaks & Native Libraries ---
    pub prefer_discrete_gpu: bool,
    pub use_zink: bool,
    pub custom_glfw_path: String,
    pub custom_openal_path: String,

    // --- APIs ---
    pub user_agent: String,
    pub modrinth_token: String,

    // --- External Tools ---
    pub text_editor_cmd: String,
    pub profiler_cmd: String,
    pub world_tool_cmd: String,

    // --- Proxy ---
    pub proxy_mode: ProxyMode,
    pub proxy_host: String,
    pub proxy_port: u16,
    pub proxy_user: String,
    pub proxy_pass: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme_mode: ThemeMode::System,
            accent: Accent::Blue,
            console_font_size: 12,
            show_status_bar: true,
            show_group_counts: true,

            sort_mode: SortMode::Name,
            view_mode: ViewMode::Grid,
            icon_size: 48,
            double_click_edits: false,
            confirm_delete: true,
            confirm_kill: false,
            remember_selected_instance: true,
            collapsed_groups: Vec::new(),

            check_updates_on_start: false,
            update_interval_hours: 24,
            download_on_create: false,
            copy_template_on_create: true,
            auto_install_deps: true,
            track_mod_metadata: true,
            detect_mod_incompat: true,
            mod_update_channel: UpdateChannel::Release,
            concurrent_downloads: 6,
            concurrent_tasks: 10,
            http_timeout_secs: 60,
            network_retries: 3,

            console_max_lines: 100_000,
            console_stop_on_overflow: false,
            show_console_on_launch: false,
            close_console_on_exit: false,
            show_console_on_crash: true,
            console_highlight_levels: true,
            console_word_wrap: false,
            console_timestamps: false,

            start_maximized: false,
            start_fullscreen: false,
            window_width: 854,
            window_height: 480,
            hide_launcher_on_launch: false,
            quit_launcher_on_exit: false,
            record_playtime: true,
            show_instance_playtime: true,
            show_total_playtime: true,
            playtime_always_hours: false,
            show_snapshots_default: false,
            show_old_versions_default: false,

            java_min_mem_mb: 512,
            java_max_mem_mb: 4096,
            java_permgen_mb: 0,
            warn_low_memory: true,
            auto_download_java: true,
            auto_detect_java: true,
            skip_java_compat: false,
            default_java_path: String::new(),
            default_jvm_args: String::new(),

            horizon_prefer_nightly: true,
            horizon_auto_update_mods: true,
            horizon_verify_assets: true,
            horizon_start_creative: false,
            horizon_gpu_backend: GpuBackend::Auto,
            horizon_gpu_power: GpuPower::Auto,
            horizon_rust_backtrace: false,
            horizon_rust_log: String::new(),
            horizon_extra_args: String::new(),
            horizon_telemetry_opt_out: false,
            horizon_no_ai: false,

            offline_player_name: "Player".into(),

            pre_launch_cmd: String::new(),
            wrapper_cmd: String::new(),
            post_exit_cmd: String::new(),
            global_env: Vec::new(),

            prefer_discrete_gpu: false,
            use_zink: false,
            custom_glfw_path: String::new(),
            custom_openal_path: String::new(),

            user_agent: "FnH/$LAUNCHER_VER".into(),
            modrinth_token: String::new(),

            text_editor_cmd: String::new(),
            profiler_cmd: String::new(),
            world_tool_cmd: String::new(),

            proxy_mode: ProxyMode::None,
            proxy_host: String::new(),
            proxy_port: 8080,
            proxy_user: String::new(),
            proxy_pass: String::new(),
        }
    }
}

impl Prefs {
    pub fn load() -> Self {
        std::fs::read_to_string(crate::paths::settings_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = crate::paths::settings_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
        self.apply_proxy_env();
    }

    pub fn is_group_collapsed(&self, group: &str) -> bool {
        self.collapsed_groups.iter().any(|g| g == group)
    }

    pub fn toggle_group(&mut self, group: &str) {
        if let Some(pos) = self.collapsed_groups.iter().position(|g| g == group) {
            self.collapsed_groups.remove(pos);
        } else {
            self.collapsed_groups.push(group.to_owned());
        }
    }

    /// Applies HTTP/SOCKS5 proxy settings to process environment for launcher network requests.
    pub fn apply_proxy_env(&self) {
        match self.proxy_mode {
            ProxyMode::None => {
                for key in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"] {
                    std::env::remove_var(key);
                }
            }
            ProxyMode::System => {}
            ProxyMode::Http | ProxyMode::Socks5 => {
                let host = self.proxy_host.trim();
                if host.is_empty() {
                    return;
                }
                let scheme = if self.proxy_mode == ProxyMode::Http {
                    "http"
                } else {
                    "socks5"
                };
                let auth = if self.proxy_user.trim().is_empty() {
                    String::new()
                } else if self.proxy_pass.is_empty() {
                    format!("{}@", self.proxy_user.trim())
                } else {
                    format!("{}:{}@", self.proxy_user.trim(), self.proxy_pass)
                };
                let url = format!("{scheme}://{auth}{host}:{}", self.proxy_port.max(1));
                std::env::set_var("HTTP_PROXY", &url);
                std::env::set_var("HTTPS_PROXY", &url);
                std::env::set_var("ALL_PROXY", &url);
            }
        }
    }

    /// Effective launch window dimensions if explicit size should be passed.
    pub fn effective_window_for(&self, inst: &InstanceCfg) -> Option<(u32, u32)> {
        let (maximized, w, h) = if inst.override_window {
            (inst.start_maximized, inst.window_width, inst.window_height)
        } else {
            (self.start_maximized, self.window_width, self.window_height)
        };
        if maximized {
            Some((1920, 1080))
        } else if w >= 320 && h >= 200 && (inst.override_window || (w != 854 || h != 480)) {
            Some((w, h))
        } else {
            None
        }
    }

    pub fn effective_fullscreen(&self, inst: &InstanceCfg) -> bool {
        if inst.override_window {
            inst.start_fullscreen
        } else {
            self.start_fullscreen
        }
    }

    pub fn effective_show_console_on_launch(&self, inst: &InstanceCfg) -> bool {
        if inst.override_console {
            inst.show_console_on_launch
        } else {
            self.show_console_on_launch
        }
    }

    pub fn effective_close_console_on_exit(&self, inst: &InstanceCfg) -> bool {
        if inst.override_console {
            inst.close_console_on_exit
        } else {
            self.close_console_on_exit
        }
    }

    pub fn effective_show_console_on_crash(&self, inst: &InstanceCfg) -> bool {
        if inst.override_console {
            inst.show_console_on_crash
        } else {
            self.show_console_on_crash
        }
    }

    pub fn effective_record_playtime(&self, inst: &InstanceCfg) -> bool {
        if inst.override_playtime {
            inst.record_playtime
        } else {
            self.record_playtime
        }
    }

    pub fn format_playtime(&self, secs: u64) -> String {
        if self.playtime_always_hours {
            let hours = secs as f64 / 3600.0;
            return format!("{hours:.1}h");
        }
        let days = secs / 86_400;
        let hours = (secs % 86_400) / 3600;
        let mins = (secs % 3600) / 60;
        let rem = secs % 60;
        if days > 0 {
            format!("{days}d {hours}h {mins}m")
        } else if hours > 0 {
            format!("{hours}h {mins}m")
        } else if mins > 0 {
            format!("{mins}m {rem}s")
        } else {
            format!("{rem}s")
        }
    }
}

/// Complete per-instance settings & overrides (`InstanceSettingsPage` for Fabric + Horizon).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceCfg {
    /// Selected icon preset (`"grass"`, `"creeper"`, `"fabricmc"`, `"horizon"`, ...) or custom
    /// imported image (`"custom:my_icon"`).
    pub icon_key: Option<String>,
    /// Unix epoch seconds of last launch (drives `"By last launched"` sorting).
    pub last_launched: u64,

    // --- General: Window overrides ---
    pub override_window: bool,
    pub start_maximized: bool,
    pub start_fullscreen: bool,
    pub window_width: u32,
    pub window_height: u32,

    // --- General: Console overrides ---
    pub override_console: bool,
    pub show_console_on_launch: bool,
    pub close_console_on_exit: bool,
    pub show_console_on_crash: bool,

    // --- General: Account & Auto-join ---
    pub override_account: bool,
    pub account_uuid: String,
    pub offline_name: String,
    pub auto_join_enabled: bool,
    pub auto_join_server: String,
    pub auto_join_world: String,

    // --- General: Game Time ---
    pub override_playtime: bool,
    pub record_playtime: bool,
    pub count_in_total: bool,

    // --- Java / Fabric overrides ---
    pub override_memory: bool,
    pub min_mem_mb: u32,
    pub max_mem_mb: u32,
    pub permgen_mb: u32,
    pub override_java: bool,
    pub java_path: String,
    pub skip_java_compat: bool,
    pub override_jvm_args: bool,
    pub jvm_args: String,
    pub extra_game_args: String,

    // --- Horizon Instance Settings ---
    pub horizon_track: HorizonTrack,
    pub horizon_pinned_version: String,
    pub horizon_start_creative: bool,
    pub horizon_friend_code: String,
    pub horizon_override_updates: bool,
    pub horizon_auto_update_mods: bool,
    pub horizon_verify_assets: bool,
    pub horizon_override_graphics: bool,
    pub horizon_gpu_backend: GpuBackend,
    pub horizon_gpu_power: GpuPower,
    pub horizon_rust_backtrace: bool,
    pub horizon_rust_log: String,
    pub horizon_extra_args: String,
    pub horizon_override_privacy: bool,
    pub horizon_telemetry_opt_out: bool,
    pub horizon_no_ai: bool,

    // --- Custom Commands override ---
    pub override_commands: bool,
    pub pre_launch_cmd: String,
    pub wrapper_cmd: String,
    pub post_exit_cmd: String,

    // --- Environment Variables override ---
    pub override_env: bool,
    pub env: Vec<(String, String)>,

    // --- Tweaks & Native Libraries override ---
    pub override_tweaks: bool,
    pub prefer_discrete_gpu: bool,
    pub use_zink: bool,
    pub custom_glfw_path: String,
    pub custom_openal_path: String,
}

impl Default for InstanceCfg {
    fn default() -> Self {
        Self {
            icon_key: None,
            last_launched: 0,

            override_window: false,
            start_maximized: false,
            start_fullscreen: false,
            window_width: 854,
            window_height: 480,

            override_console: false,
            show_console_on_launch: false,
            close_console_on_exit: false,
            show_console_on_crash: true,

            override_account: false,
            account_uuid: String::new(),
            offline_name: String::new(),
            auto_join_enabled: false,
            auto_join_server: String::new(),
            auto_join_world: String::new(),

            override_playtime: false,
            record_playtime: true,
            count_in_total: true,

            override_memory: false,
            min_mem_mb: 512,
            max_mem_mb: 4096,
            permgen_mb: 0,
            override_java: false,
            java_path: String::new(),
            skip_java_compat: false,
            override_jvm_args: false,
            jvm_args: String::new(),
            extra_game_args: String::new(),

            horizon_track: HorizonTrack::Default,
            horizon_pinned_version: String::new(),
            horizon_start_creative: false,
            horizon_friend_code: String::new(),
            horizon_override_updates: false,
            horizon_auto_update_mods: true,
            horizon_verify_assets: true,
            horizon_override_graphics: false,
            horizon_gpu_backend: GpuBackend::Auto,
            horizon_gpu_power: GpuPower::Auto,
            horizon_rust_backtrace: false,
            horizon_rust_log: String::new(),
            horizon_extra_args: String::new(),
            horizon_override_privacy: false,
            horizon_telemetry_opt_out: false,
            horizon_no_ai: false,

            override_commands: false,
            pre_launch_cmd: String::new(),
            wrapper_cmd: String::new(),
            post_exit_cmd: String::new(),

            override_env: false,
            env: Vec::new(),

            override_tweaks: false,
            prefer_discrete_gpu: false,
            use_zink: false,
            custom_glfw_path: String::new(),
            custom_openal_path: String::new(),
        }
    }
}

/// Persistent map of `instance_id -> InstanceCfg`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InstanceCfgStore {
    pub instances: BTreeMap<String, InstanceCfg>,
}

impl InstanceCfgStore {
    pub fn load() -> Self {
        std::fs::read_to_string(crate::paths::instance_cfg_file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = crate::paths::instance_cfg_file();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, json);
        }
    }

    pub fn get(&self, id: &str) -> InstanceCfg {
        self.instances.get(id).cloned().unwrap_or_default()
    }

    pub fn update(&mut self, id: &str, f: impl FnOnce(&mut InstanceCfg)) {
        let entry = self.instances.entry(id.to_owned()).or_default();
        f(entry);
        self.save();
    }

    pub fn remove(&mut self, id: &str) {
        if self.instances.remove(id).is_some() {
            self.save();
        }
    }

    pub fn clone_cfg(&mut self, from_id: &str, to_id: &str) {
        let mut copy = self.get(from_id);
        copy.last_launched = 0;
        self.instances.insert(to_id.to_owned(), copy);
        self.save();
    }
}

/// Non-destructive editor over `<instance>/options.txt` (shared format for Minecraft: Java Edition
/// and Horizon): edits video, performance, control and audio settings directly from the instance
/// Settings window while leaving every keybind and unknown line untouched.
#[derive(Clone, Debug)]
pub struct GameOptions {
    pub render_distance: u32,
    pub simulation_distance: u32,
    pub max_fps: u32,
    pub gui_scale: u32,
    pub fov_deg: u32,
    pub gamma_pct: u32,
    pub fullscreen: bool,
    pub vsync: bool,
    pub view_bobbing: bool,
    pub auto_jump: bool,
    pub raw_mouse_input: bool,
    pub pause_on_lost_focus: bool,
    pub smooth_lighting: bool,
    pub clouds: String,
    pub graphics_mode: u32,
    pub particles: u32,
    pub mipmap_levels: u32,
    pub master_volume_pct: u32,
    pub music_volume_pct: u32,
    pub exists_on_disk: bool,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            render_distance: 12,
            simulation_distance: 12,
            max_fps: 120,
            gui_scale: 0,
            fov_deg: 70,
            gamma_pct: 50,
            fullscreen: false,
            vsync: true,
            view_bobbing: true,
            auto_jump: false,
            raw_mouse_input: true,
            pause_on_lost_focus: true,
            smooth_lighting: true,
            clouds: "true".into(),
            graphics_mode: 1,
            particles: 0,
            mipmap_levels: 4,
            master_volume_pct: 100,
            music_volume_pct: 100,
            exists_on_disk: false,
        }
    }
}

impl GameOptions {
    pub fn load(home: &Path) -> Self {
        let path = home.join("options.txt");
        let Ok(text) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        let mut o = Self {
            exists_on_disk: true,
            ..Self::default()
        };
        for line in text.lines() {
            let Some((k, raw)) = line.split_once(':') else {
                continue;
            };
            let v = raw.trim().trim_matches('"');
            match k.trim() {
                "renderDistance" => o.render_distance = v.parse().unwrap_or(o.render_distance),
                "simulationDistance" => {
                    o.simulation_distance = v.parse().unwrap_or(o.simulation_distance)
                }
                "maxFps" => o.max_fps = v.parse().unwrap_or(o.max_fps),
                "guiScale" => o.gui_scale = v.parse().unwrap_or(o.gui_scale),
                "fov" => {
                    if let Ok(f) = v.parse::<f32>() {
                        o.fov_deg = if f <= 2.5 {
                            (70.0 + f * 40.0).round() as u32
                        } else {
                            f.round() as u32
                        }
                        .clamp(30, 110);
                    }
                }
                "gamma" => {
                    if let Ok(g) = v.parse::<f32>() {
                        o.gamma_pct = (g * 100.0).round().clamp(0.0, 500.0) as u32;
                    }
                }
                "fullscreen" => o.fullscreen = v == "true",
                "enableVsync" => o.vsync = v == "true",
                "bobView" => o.view_bobbing = v == "true",
                "autoJump" => o.auto_jump = v == "true",
                "rawMouseInput" => o.raw_mouse_input = v == "true",
                "pauseOnLostFocus" => o.pause_on_lost_focus = v == "true",
                "ao" => o.smooth_lighting = v != "false" && v != "0",
                "renderClouds" => o.clouds = v.to_owned(),
                "graphicsMode" => o.graphics_mode = v.parse().unwrap_or(o.graphics_mode),
                "particles" => o.particles = v.parse().unwrap_or(o.particles),
                "mipmapLevels" => o.mipmap_levels = v.parse().unwrap_or(o.mipmap_levels),
                "soundCategory_master" => {
                    if let Ok(s) = v.parse::<f32>() {
                        o.master_volume_pct = (s * 100.0).round().clamp(0.0, 100.0) as u32;
                    }
                }
                "soundCategory_music" => {
                    if let Ok(s) = v.parse::<f32>() {
                        o.music_volume_pct = (s * 100.0).round().clamp(0.0, 100.0) as u32;
                    }
                }
                _ => {}
            }
        }
        o
    }

    pub fn save(&mut self, home: &Path) -> Result<(), String> {
        std::fs::create_dir_all(home).map_err(|e| format!("{}: {e}", home.display()))?;
        let path = home.join("options.txt");
        let existing = std::fs::read_to_string(&path).unwrap_or_default();

        let fov_norm = ((self.fov_deg.clamp(30, 110) as f32) - 70.0) / 40.0;
        let clouds_val = match self.clouds.as_str() {
            "false" => "\"false\"",
            "fast" => "\"fast\"",
            _ => "\"true\"",
        };
        let updates: Vec<(&str, String)> = vec![
            ("renderDistance", self.render_distance.clamp(2, 64).to_string()),
            (
                "simulationDistance",
                self.simulation_distance.clamp(5, 32).to_string(),
            ),
            ("maxFps", self.max_fps.clamp(10, 260).to_string()),
            ("guiScale", self.gui_scale.min(6).to_string()),
            ("fov", format!("{fov_norm:.2}")),
            (
                "gamma",
                format!("{:.2}", self.gamma_pct.min(500) as f32 / 100.0),
            ),
            ("fullscreen", self.fullscreen.to_string()),
            ("enableVsync", self.vsync.to_string()),
            ("bobView", self.view_bobbing.to_string()),
            ("autoJump", self.auto_jump.to_string()),
            ("rawMouseInput", self.raw_mouse_input.to_string()),
            ("pauseOnLostFocus", self.pause_on_lost_focus.to_string()),
            ("ao", self.smooth_lighting.to_string()),
            ("renderClouds", clouds_val.to_owned()),
            ("graphicsMode", self.graphics_mode.min(2).to_string()),
            ("particles", self.particles.min(2).to_string()),
            ("mipmapLevels", self.mipmap_levels.min(4).to_string()),
            (
                "soundCategory_master",
                format!("{:.2}", self.master_volume_pct.min(100) as f32 / 100.0),
            ),
            (
                "soundCategory_music",
                format!("{:.2}", self.music_volume_pct.min(100) as f32 / 100.0),
            ),
        ];

        let mut seen = vec![false; updates.len()];
        let mut lines: Vec<String> = existing
            .lines()
            .map(|line| {
                if let Some((k, _)) = line.split_once(':') {
                    if let Some((idx, (_, val))) =
                        updates.iter().enumerate().find(|(_, (uk, _))| *uk == k.trim())
                    {
                        seen[idx] = true;
                        return format!("{}:{val}", k.trim());
                    }
                }
                line.to_owned()
            })
            .collect();

        for (idx, (k, val)) in updates.iter().enumerate() {
            if !seen[idx] {
                lines.push(format!("{k}:{val}"));
            }
        }

        let mut out = lines.join("\n");
        out.push('\n');
        std::fs::write(&path, out).map_err(|e| format!("{}: {e}", path.display()))?;
        self.exists_on_disk = true;
        Ok(())
    }
}

/// Normalizes a user-chosen Java executable or directory into a Java `home` folder containing
/// `bin/java(.exe)`, matching `horizon_launcher::java::pick`.
pub fn normalize_java_home(input: &str) -> Option<std::path::PathBuf> {
    let trimmed = input.trim().trim_matches('"');
    if trimmed.is_empty() {
        return None;
    }
    let path = std::path::PathBuf::from(trimmed);
    if path.is_file() {
        if let Some(bin) = path.parent() {
            if bin
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.eq_ignore_ascii_case("bin"))
            {
                if let Some(home) = bin.parent() {
                    return Some(home.to_path_buf());
                }
            }
        }
    }
    if path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.eq_ignore_ascii_case("bin"))
    {
        if let Some(home) = path.parent() {
            return Some(home.to_path_buf());
        }
    }
    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefs_and_instance_cfg_round_trip_json() {
        let mut p = Prefs::default();
        p.theme_mode = ThemeMode::Dark;
        p.accent = Accent::Purple;
        p.java_max_mem_mb = 8192;
        p.horizon_gpu_backend = GpuBackend::Vulkan;
        p.global_env.push(("FOO".into(), "BAR".into()));
        let raw = serde_json::to_string(&p).unwrap();
        let back: Prefs = serde_json::from_str(&raw).unwrap();
        assert_eq!(back.theme_mode, ThemeMode::Dark);
        assert_eq!(back.accent, Accent::Purple);
        assert_eq!(back.java_max_mem_mb, 8192);
        assert_eq!(back.horizon_gpu_backend, GpuBackend::Vulkan);
        assert_eq!(back.global_env, vec![("FOO".into(), "BAR".into())]);
    }

    #[test]
    fn game_options_preserves_unknown_keys_and_updates_values() {
        let dir = std::env::temp_dir().join(format!("fnh-opts-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("options.txt"),
            "key_key.jump:key.keyboard.space\nrenderDistance:8\nfov:0.0\n",
        )
        .unwrap();

        let mut opts = GameOptions::load(&dir);
        assert_eq!(opts.render_distance, 8);
        assert_eq!(opts.fov_deg, 70);
        opts.render_distance = 20;
        opts.fov_deg = 90;
        opts.vsync = false;
        opts.save(&dir).unwrap();

        let saved = std::fs::read_to_string(dir.join("options.txt")).unwrap();
        assert!(saved.contains("key_key.jump:key.keyboard.space"));
        assert!(saved.contains("renderDistance:20"));
        assert!(saved.contains("enableVsync:false"));
        let again = GameOptions::load(&dir);
        assert_eq!(again.fov_deg, 90);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn normalize_java_home_strips_bin_and_exe() {
        let home = normalize_java_home("/opt/jdk-21/bin").unwrap();
        assert_eq!(home, Path::new("/opt/jdk-21"));
    }
}
