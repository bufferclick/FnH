//! Persistent UI preferences (`fh/ui.cfg` in the launcher's data folder), styled after JLac's
//! own INI settings: which Windows 11 appearance mode and accent to draw, whether the status bar
//! is shown, how instances sort, and which instance groups are collapsed.

use std::path::PathBuf;

/// How the Windows 11 (WinUI 3) widget style resolves light or dark surfaces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ThemeMode {
    /// Follow Windows 11's own app mode (`window.appearance()`).
    #[default]
    System,
    /// Always draw WinUI 3 Light (`#f3f3f3` Mica base, `#ffffff` view layer).
    Light,
    /// Always draw WinUI 3 Dark (`#202020` Mica base, `#272727` view layer).
    Dark,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [ThemeMode::System, ThemeMode::Dark, ThemeMode::Light];

    pub fn id(self) -> &'static str {
        match self {
            ThemeMode::System => "windows11",
            ThemeMode::Dark => "windows11-dark",
            ThemeMode::Light => "windows11-light",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::System => "Windows 11 (System)",
            ThemeMode::Dark => "Windows 11 Dark",
            ThemeMode::Light => "Windows 11 Light",
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value.trim() {
            "windows11-dark" | "dark" => ThemeMode::Dark,
            "windows11-light" | "light" => ThemeMode::Light,
            _ => ThemeMode::System,
        }
    }
}

/// Windows 11 Fluent accent swatches (paired light/dark values meeting WinUI 3 contrast).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Accent {
    /// Windows 11 default Fluent Blue (`#0067c0` light / `#60cdff` dark).
    #[default]
    Blue,
    /// FNH brand deep indigo-navy (`#1f2478` light / `#929cff` dark).
    Navy,
    /// Windows 11 Emerald (`#0f7b0f` light / `#6ccb5f` dark).
    Green,
    /// Windows 11 Iris (`#5c43b8` light / `#b4a0ff` dark).
    Purple,
    /// Windows 11 Gold (`#9d5d00` light / `#fce100` dark).
    Gold,
    /// Windows 11 Crimson (`#c42b1c` light / `#ff99a4` dark).
    Crimson,
}

impl Accent {
    pub const ALL: [Accent; 6] = [
        Accent::Blue,
        Accent::Navy,
        Accent::Green,
        Accent::Purple,
        Accent::Gold,
        Accent::Crimson,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Accent::Blue => "blue",
            Accent::Navy => "navy",
            Accent::Green => "green",
            Accent::Purple => "purple",
            Accent::Gold => "gold",
            Accent::Crimson => "crimson",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Accent::Blue => "Windows Blue",
            Accent::Navy => "FnH Navy",
            Accent::Green => "Emerald",
            Accent::Purple => "Iris",
            Accent::Gold => "Amber",
            Accent::Crimson => "Crimson",
        }
    }

    /// `(fill, hover, on_accent)` in `#rrggbb` for light or dark WinUI 3 surfaces.
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
            (Accent::Gold, false) => (0x9a5b00, 0xad6908, 0xffffff),
            (Accent::Gold, true) => (0xfce100, 0xe3cb00, 0x141100),
            (Accent::Crimson, false) => (0xc42b1c, 0xcc3f31, 0xffffff),
            (Accent::Crimson, true) => (0xff99a4, 0xe88792, 0x1a0608),
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value.trim() {
            "navy" => Accent::Navy,
            "green" => Accent::Green,
            "purple" => Accent::Purple,
            "gold" => Accent::Gold,
            "crimson" => Accent::Crimson,
            _ => Accent::Blue,
        }
    }
}

/// JLac's `InstSortMode`: alphabetical or most recently launched first.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SortMode {
    #[default]
    Name,
    LastLaunched,
}

impl SortMode {
    pub fn id(self) -> &'static str {
        match self {
            SortMode::Name => "Name",
            SortMode::LastLaunched => "LastLaunch",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SortMode::Name => "By name",
            SortMode::LastLaunched => "By last launched",
        }
    }

    pub fn from_id(value: &str) -> Self {
        match value.trim() {
            "LastLaunch" => SortMode::LastLaunched,
            _ => SortMode::Name,
        }
    }
}

/// All saved window preferences.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Prefs {
    pub theme: ThemeMode,
    pub accent: Accent,
    pub sort: SortMode,
    pub status_bar: bool,
    pub collapsed: Vec<String>,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            theme: ThemeMode::System,
            accent: Accent::Blue,
            sort: SortMode::Name,
            status_bar: true,
            collapsed: Vec::new(),
        }
    }
}

impl Prefs {
    pub fn path() -> PathBuf {
        horizon_launcher::backend::data_dir().join("fh").join("ui.cfg")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(path, self.encode());
    }

    pub fn is_collapsed(&self, group: &str) -> bool {
        self.collapsed.iter().any(|item| item == group)
    }

    pub fn toggle_group(&mut self, group: &str) {
        if let Some(index) = self.collapsed.iter().position(|item| item == group) {
            self.collapsed.remove(index);
        } else {
            self.collapsed.push(group.to_owned());
        }
    }

    pub fn parse(text: &str) -> Self {
        let mut prefs = Self::default();
        for raw in text.lines() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            match key.trim() {
                "ApplicationTheme" => prefs.theme = ThemeMode::from_id(value),
                "AccentColor" => prefs.accent = Accent::from_id(value),
                "InstSortMode" => prefs.sort = SortMode::from_id(value),
                "StatusBarVisible" => prefs.status_bar = value.trim() != "false",
                "CollapsedGroup" => {
                    let group = value.trim();
                    if !group.is_empty() && !prefs.is_collapsed(group) {
                        prefs.collapsed.push(group.to_owned());
                    }
                }
                _ => {}
            }
        }
        prefs
    }

    pub fn encode(&self) -> String {
        let mut out = format!(
            "ApplicationTheme={}\nAccentColor={}\nInstSortMode={}\nStatusBarVisible={}\n",
            self.theme.id(),
            self.accent.id(),
            self.sort.id(),
            if self.status_bar { "true" } else { "false" },
        );
        for group in &self.collapsed {
            let clean = group.replace(['\r', '\n'], " ");
            out.push_str(&format!("CollapsedGroup={clean}\n"));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::{Accent, Prefs, SortMode, ThemeMode};

    #[test]
    fn preferences_round_trip_cleanly() {
        let mut prefs = Prefs {
            theme: ThemeMode::Dark,
            accent: Accent::Navy,
            sort: SortMode::LastLaunched,
            status_bar: false,
            collapsed: Vec::new(),
        };
        prefs.toggle_group("Fabric Packs");
        prefs.toggle_group("Snapshots");
        let parsed = Prefs::parse(&prefs.encode());
        assert_eq!(parsed, prefs);
        assert!(parsed.is_collapsed("Fabric Packs"));
        prefs.toggle_group("Fabric Packs");
        assert!(!prefs.is_collapsed("Fabric Packs"));
    }
}
