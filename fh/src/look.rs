//! Windows 11 (WinUI 3) theme tokens and reusable controls, matching JLac's `QWindows11Style`
//! (`SystemTheme` `"windows11"`): Mica window chrome (`#f3f3f3` light / `#202020` dark /
//! `#0c0c0c` OLED), elevated central panes (`#ffffff` / `#272727`), 4px control corners,
//! bottom-accented `TextBox` inputs, left 3x16px accent selection pills on `PageContainer` rows,
//! `QTabWidget` sub-tab strips, WinUI 3 settings cards, and FNH's 9x9 pixel brand mark.

use crate::icons::icon;
use crate::prefs::{Accent, ThemeMode};
use gpui::{
    div, prelude::*, px, rgb, Div, FontWeight, Hsla, SharedString, Stateful, WindowAppearance,
};

pub const UI_FONT: &str = "Segoe UI";
pub const MONO_FONT: &str = "Consolas";

/// FNH logo colours (`fh/icons/fh.svg`): porcelain tile, navy F & H, black n.
pub const MARK_TILE: u32 = 0xf8f9fc;
pub const MARK_RING: u32 = 0xd8dae5;
pub const MARK_NAVY: u32 = 0x131254;
pub const MARK_INK: u32 = 0x000000;

pub fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

pub fn hexa(value: u32, alpha: f32) -> Hsla {
    let mut c: Hsla = rgb(value).into();
    c.a = alpha;
    c
}

/// Resolved WinUI 3 tokens for the current frame.
#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    pub window: Hsla,
    pub layer: Hsla,
    pub card: Hsla,
    pub sidebar: Hsla,
    pub control: Hsla,
    pub control_hover: Hsla,
    pub control_border: Hsla,
    pub control_bottom: Hsla,
    pub subtle_hover: Hsla,
    pub subtle_selected: Hsla,
    pub tile_selected: Hsla,
    pub tile_selected_border: Hsla,
    pub input_bg: Hsla,
    pub input_bottom: Hsla,
    pub divider: Hsla,
    pub flyout: Hsla,
    pub flyout_border: Hsla,
    pub footer: Hsla,
    pub ink: Hsla,
    pub ink_secondary: Hsla,
    pub ink_tertiary: Hsla,
    pub ink_disabled: Hsla,
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub on_accent: Hsla,
    pub warn: Hsla,
    pub danger: Hsla,
    pub danger_hover: Hsla,
    pub on_danger: Hsla,
    pub success: Hsla,
}

impl Theme {
    pub fn resolve(mode: ThemeMode, accent: Accent, system: WindowAppearance) -> Self {
        let (dark, oled) = match mode {
            ThemeMode::Dark => (true, false),
            ThemeMode::OledDark => (true, true),
            ThemeMode::Light => (false, false),
            ThemeMode::System => (
                !matches!(system, WindowAppearance::Light | WindowAppearance::VibrantLight),
                false,
            ),
        };
        let (acc, acc_hover, on_acc) = accent.colours(dark);
        if oled {
            Self {
                dark: true,
                window: hex(0x0b0b0c),
                layer: hex(0x121214),
                card: hex(0x17171a),
                sidebar: hex(0x0b0b0c),
                control: hex(0x1d1d21),
                control_hover: hex(0x26262b),
                control_border: hex(0x2c2c32),
                control_bottom: hex(0x222227),
                subtle_hover: hex(0x1c1c20),
                subtle_selected: hex(0x25252b),
                tile_selected: hexa(acc, 0.18),
                tile_selected_border: hexa(acc, 0.72),
                input_bg: hex(0x141417),
                input_bottom: hex(0x888892),
                divider: hex(0x222228),
                flyout: hex(0x18181c),
                flyout_border: hex(0x303038),
                footer: hex(0x09090a),
                ink: hex(0xffffff),
                ink_secondary: hex(0xc6c6cc),
                ink_tertiary: hex(0x8d8d96),
                ink_disabled: hex(0x5c5c64),
                accent: hex(acc),
                accent_hover: hex(acc_hover),
                on_accent: hex(on_acc),
                warn: hex(0xfce100),
                danger: hex(0xff99a4),
                danger_hover: hex(0xd13438),
                on_danger: hex(0xffffff),
                success: hex(0x6ccb5f),
            }
        } else if dark {
            Self {
                dark: true,
                window: hex(0x202020),
                layer: hex(0x272727),
                card: hex(0x2c2c2c),
                sidebar: hex(0x202020),
                control: hex(0x2f2f2f),
                control_hover: hex(0x373737),
                control_border: hex(0x3d3d3d),
                control_bottom: hex(0x333333),
                subtle_hover: hex(0x2e2e2e),
                subtle_selected: hex(0x363636),
                tile_selected: hexa(acc, 0.16),
                tile_selected_border: hexa(acc, 0.65),
                input_bg: hex(0x232323),
                input_bottom: hex(0x9a9a9a),
                divider: hex(0x333333),
                flyout: hex(0x2c2c2c),
                flyout_border: hex(0x424242),
                footer: hex(0x1c1c1c),
                ink: hex(0xffffff),
                ink_secondary: hex(0xc8c8c8),
                ink_tertiary: hex(0x969696),
                ink_disabled: hex(0x666666),
                accent: hex(acc),
                accent_hover: hex(acc_hover),
                on_accent: hex(on_acc),
                warn: hex(0xfce100),
                danger: hex(0xff99a4),
                danger_hover: hex(0xd13438),
                on_danger: hex(0xffffff),
                success: hex(0x6ccb5f),
            }
        } else {
            Self {
                dark: false,
                window: hex(0xf3f3f3),
                layer: hex(0xffffff),
                card: hex(0xfbfbfb),
                sidebar: hex(0xf3f3f3),
                control: hex(0xfefefe),
                control_hover: hex(0xf6f6f6),
                control_border: hex(0xe2e2e2),
                control_bottom: hex(0xcccccc),
                subtle_hover: hex(0xe9e9e9),
                subtle_selected: hex(0xe2e2e2),
                tile_selected: hexa(acc, 0.11),
                tile_selected_border: hexa(acc, 0.58),
                input_bg: hex(0xffffff),
                input_bottom: hex(0x868686),
                divider: hex(0xe5e5e5),
                flyout: hex(0xfcfcfc),
                flyout_border: hex(0xdadada),
                footer: hex(0xeeeeee),
                ink: hex(0x1a1a1a),
                ink_secondary: hex(0x5c5c5c),
                ink_tertiary: hex(0x787878),
                ink_disabled: hex(0xa2a2a2),
                accent: hex(acc),
                accent_hover: hex(acc_hover),
                on_accent: hex(on_acc),
                warn: hex(0x9d5d00),
                danger: hex(0xc42b1c),
                danger_hover: hex(0xb12316),
                on_danger: hex(0xffffff),
                success: hex(0x0f7b0f),
            }
        }
    }
}

/// FNH's crisp 9x9 pixel-grid brand logo (`fh/icons/fh.svg` / `fh/icons/fh.ico`), rendered
/// directly from the supersampled embedded `default.png` icon so every corner, navy stem, and
/// black `n` stays razor-sharp at every window DPI and size.
pub fn brand_mark(size: f32) -> Div {
    let _ = (MARK_TILE, MARK_RING, MARK_NAVY, MARK_INK);
    crate::inst_icons::render_icon("default", size)
}

/// Applies FNH's embedded `fh.ico` (`ICON_SMALL` + `ICON_BIG`) to every OS window created on the
/// UI thread on Windows so secondary windows (`Settings`, `Instance`, `Add Instance`, `Select Icon`,
/// `Mod Browser`, `Crash Report`, `Updater`) always show FNH's crisp 9x9 icon in the titlebar and
/// Alt-Tab switcher.
#[cfg(windows)]
pub fn apply_window_icon() {
    use std::sync::OnceLock;
    use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::System::Threading::GetCurrentThreadId;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        EnumThreadWindows, GetSystemMetrics, LoadImageW, SendMessageW, ICON_BIG, ICON_SMALL,
        IMAGE_ICON, LR_SHARED, SM_CXICON, SM_CXSMICON, SM_CYICON, SM_CYSMICON, WM_SETICON,
    };

    static ICONS: OnceLock<(isize, isize)> = OnceLock::new();
    let (small, big) = *ICONS.get_or_init(|| unsafe {
        let hmod = GetModuleHandleW(std::ptr::null());
        if hmod.is_null() {
            return (0, 0);
        }
        // `winresource::WindowsResource::set_icon` embeds `fh.ico` at resource ID 1.
        let res_id = 1usize as *const u16;
        let sm = LoadImageW(
            hmod,
            res_id,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXSMICON),
            GetSystemMetrics(SM_CYSMICON),
            LR_SHARED,
        ) as isize;
        let lg = LoadImageW(
            hmod,
            res_id,
            IMAGE_ICON,
            GetSystemMetrics(SM_CXICON),
            GetSystemMetrics(SM_CYICON),
            LR_SHARED,
        ) as isize;
        (sm, lg)
    });

    if small == 0 && big == 0 {
        return;
    }

    unsafe extern "system" fn set_on_hwnd(hwnd: HWND, lparam: LPARAM) -> i32 {
        let pair = unsafe { &*(lparam as *const (isize, isize)) };
        unsafe {
            if pair.0 != 0 {
                SendMessageW(hwnd, WM_SETICON, ICON_SMALL as WPARAM, pair.0 as LPARAM);
            }
            if pair.1 != 0 {
                SendMessageW(hwnd, WM_SETICON, ICON_BIG as WPARAM, pair.1 as LPARAM);
            }
        }
        1
    }

    let pair = (small, big);
    unsafe {
        EnumThreadWindows(
            GetCurrentThreadId(),
            Some(set_on_hwnd),
            (&pair as *const (isize, isize)) as LPARAM,
        );
    }
}

#[cfg(not(windows))]
pub fn apply_window_icon() {}

/// Top toolbar button (`QToolButton` ToolButtonTextBesideIcon, WinUI 3 subtle style).
pub fn toolbar_button(
    id: impl Into<SharedString>,
    glyph: &'static str,
    label: &str,
    dropdown: bool,
    active: bool,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    let fg = if enabled { t.ink } else { t.ink_disabled };
    let hover = t.subtle_hover;
    let mut row = div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(30.))
        .px(px(8.))
        .rounded(px(4.))
        .text_size(px(12.5))
        .text_color(fg)
        .when(active, |d| d.bg(t.subtle_selected))
        .when(enabled && !active, |d| {
            d.cursor_pointer().hover(move |s| s.bg(hover))
        })
        .when(enabled && active, |d| d.cursor_pointer())
        .child(icon(glyph, 16., fg))
        .child(label.to_owned());
    if dropdown {
        row = row.child(icon(
            "chevron-down",
            11.,
            if enabled { t.ink_secondary } else { t.ink_disabled },
        ));
    }
    row
}

pub fn toolbar_separator(t: &Theme) -> Div {
    div()
        .flex_none()
        .w(px(1.))
        .h(px(20.))
        .mx(px(3.))
        .bg(t.divider)
}

/// Right-hand `WideBar` action button matching JLac's `instanceToolBar`.
pub fn wide_bar_action(
    id: impl Into<SharedString>,
    glyph: &'static str,
    label: &str,
    enabled: bool,
    primary: bool,
    t: &Theme,
) -> Stateful<Div> {
    if primary && enabled {
        let hover = t.accent_hover;
        return div()
            .id(id.into())
            .flex()
            .items_center()
            .gap(px(8.))
            .w_full()
            .h(px(30.))
            .px(px(10.))
            .rounded(px(4.))
            .bg(t.accent)
            .text_size(px(12.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.on_accent)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(icon(glyph, 15., t.on_accent))
            .child(label.to_owned());
    }
    let fg = if enabled { t.ink } else { t.ink_disabled };
    let hover = t.subtle_hover;
    div()
        .id(id.into())
        .flex()
        .items_center()
        .gap(px(8.))
        .w_full()
        .h(px(28.))
        .px(px(10.))
        .rounded(px(4.))
        .text_size(px(12.5))
        .text_color(fg)
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
        .child(icon(glyph, 15., fg))
        .child(label.to_owned())
}

pub fn wide_bar_separator(t: &Theme) -> Div {
    div()
        .flex_none()
        .w_full()
        .h(px(1.))
        .my(px(4.))
        .bg(t.divider)
}

/// Standard WinUI 3 push button (`ControlFillColorDefault`, 4px radius, 1px stroke + bottom bevel).
pub fn push_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    let hover = t.control_hover;
    let fg = if enabled { t.ink } else { t.ink_disabled };
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .min_w(px(74.))
        .h(px(30.))
        .px(px(12.))
        .rounded(px(4.))
        .bg(t.control)
        .border_1()
        .border_color(t.control_border)
        .text_size(px(12.5))
        .text_color(fg)
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
        .child(label.into())
}

/// Right-column action button inside JLac `InstanceWindow` pages (`Mods`, `Worlds`, `Servers`...).
pub fn side_action_button(
    id: impl Into<SharedString>,
    glyph: Option<&'static str>,
    label: impl Into<SharedString>,
    enabled: bool,
    accent: bool,
    t: &Theme,
) -> Stateful<Div> {
    if accent && enabled {
        let hover = t.accent_hover;
        let mut b = div()
            .id(id.into())
            .flex()
            .items_center()
            .gap(px(7.))
            .w_full()
            .h(px(30.))
            .px(px(10.))
            .rounded(px(4.))
            .bg(t.accent)
            .text_size(px(12.5))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.on_accent)
            .cursor_pointer()
            .hover(move |s| s.bg(hover));
        if let Some(g) = glyph {
            b = b.child(icon(g, 14., t.on_accent));
        }
        return b.child(label.into());
    }
    let hover = t.control_hover;
    let fg = if enabled { t.ink } else { t.ink_disabled };
    let mut b = div()
        .id(id.into())
        .flex()
        .items_center()
        .gap(px(7.))
        .w_full()
        .h(px(29.))
        .px(px(10.))
        .rounded(px(4.))
        .bg(t.control)
        .border_1()
        .border_color(t.control_border)
        .text_size(px(12.5))
        .text_color(fg)
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)));
    if let Some(g) = glyph {
        b = b.child(icon(g, 14., fg));
    }
    b.child(label.into())
}

/// Primary Fluent accent button (`OK`, `Launch`, `Install`, `Save`).
pub fn accent_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    if !enabled {
        return push_button(id, label, false, t);
    }
    let hover = t.accent_hover;
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .min_w(px(80.))
        .h(px(30.))
        .px(px(14.))
        .rounded(px(4.))
        .bg(t.accent)
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.on_accent)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label.into())
}

/// Destructive WinUI 3 button (`Delete`, `Remove`).
pub fn danger_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    if !enabled {
        return push_button(id, label, false, t);
    }
    let bg = if t.dark { hex(0xc42b1c) } else { t.danger };
    let hover = t.danger_hover;
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .min_w(px(80.))
        .h(px(30.))
        .px(px(14.))
        .rounded(px(4.))
        .bg(bg)
        .text_size(px(12.5))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(t.on_danger)
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label.into())
}

/// Compact selectable pill / preset button (`Aikar's G1GC`, `1920×1080`, `4096 MiB`, etc.).
pub fn chip(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    selected: bool,
    t: &Theme,
) -> Stateful<Div> {
    let hover = if selected {
        t.accent_hover
    } else {
        t.control_hover
    };
    div()
        .id(id.into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(26.))
        .px(px(10.))
        .rounded(px(4.))
        .border_1()
        .border_color(if selected { t.accent } else { t.control_border })
        .bg(if selected { t.accent } else { t.control })
        .text_size(px(12.))
        .font_weight(if selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(if selected { t.on_accent } else { t.ink })
        .cursor_pointer()
        .hover(move |s| s.bg(hover))
        .child(label.into())
}

/// Horizontal `QTabWidget` sub-tab inside `Settings` and `Instance Settings` pages.
pub fn sub_tab(
    id: impl Into<SharedString>,
    label: &str,
    selected: bool,
    t: &Theme,
) -> Stateful<Div> {
    let hover = t.subtle_hover;
    div()
        .id(id.into())
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .h(px(32.))
        .px(px(12.))
        .rounded_t(px(4.))
        .text_size(px(12.5))
        .font_weight(if selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .text_color(if selected { t.ink } else { t.ink_secondary })
        .cursor_pointer()
        .when(selected, |d| d.bg(t.subtle_selected))
        .when(!selected, |d| d.hover(move |s| s.bg(hover)))
        .child(label.to_owned())
        .when(selected, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(8.))
                    .right(px(8.))
                    .bottom(px(0.))
                    .h(px(2.5))
                    .rounded(px(1.5))
                    .bg(t.accent),
            )
        })
}

/// Left sidebar navigation row in `PageContainer` (`Settings`, `InstanceWindow`, `NewInstance`),
/// with WinUI 3's signature 3x16px left accent pill.
pub fn page_nav_row(
    id: impl Into<SharedString>,
    glyph: &'static str,
    label: &str,
    selected: bool,
    t: &Theme,
) -> Stateful<Div> {
    let hover = t.subtle_hover;
    div()
        .id(id.into())
        .relative()
        .flex()
        .items_center()
        .gap(px(9.))
        .w_full()
        .h(px(32.))
        .px(px(12.))
        .rounded(px(4.))
        .text_size(px(12.5))
        .text_color(t.ink)
        .font_weight(if selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .cursor_pointer()
        .when(selected, |d| d.bg(t.subtle_selected))
        .when(!selected, |d| d.hover(move |s| s.bg(hover)))
        .when(selected, |d| {
            d.child(
                div()
                    .absolute()
                    .left(px(3.))
                    .top(px(8.))
                    .w(px(3.))
                    .h(px(16.))
                    .rounded(px(1.5))
                    .bg(t.accent),
            )
        })
        .child(icon(glyph, 16., if selected { t.accent } else { t.ink }))
        .child(label.to_owned())
}

/// WinUI 3 `TextBox` with 1px->2px Fluent accent bottom underline when focused.
pub fn text_input(
    id: impl Into<SharedString>,
    value: &str,
    placeholder: &str,
    focused: bool,
    t: &Theme,
) -> Stateful<Div> {
    let shown = if value.is_empty() { placeholder } else { value };
    let color = if value.is_empty() {
        t.ink_tertiary
    } else {
        t.ink
    };
    div()
        .id(id.into())
        .flex()
        .items_center()
        .w_full()
        .h(px(30.))
        .px(px(10.))
        .rounded(px(4.))
        .bg(t.input_bg)
        .border_1()
        .border_b_2()
        .border_color(if focused { t.accent } else { t.control_border })
        .text_size(px(12.5))
        .text_color(color)
        .cursor_text()
        .overflow_x_hidden()
        .child(
            div()
                .flex()
                .items_center()
                .child(shown.to_owned())
                .when(focused && !value.is_empty(), |d| {
                    d.child(div().w(px(1.5)).h(px(14.)).ml(px(1.)).bg(t.accent))
                })
                .when(focused && value.is_empty(), |d| {
                    d.child(div().w(px(1.5)).h(px(14.)).ml(px(2.)).bg(t.accent))
                }),
        )
}

/// WinUI 3 `CheckBox` row (with optional secondary description).
pub fn checkbox_row(
    id: impl Into<SharedString>,
    label: &str,
    hint: Option<&str>,
    checked: bool,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    let hover = t.subtle_hover;
    let box_el = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(17.))
        .mt(px(if hint.is_some() { 1. } else { 0. }))
        .rounded(px(3.5))
        .border_1()
        .border_color(if checked && enabled {
            t.accent
        } else {
            t.input_bottom
        })
        .bg(if checked && enabled {
            t.accent
        } else {
            t.input_bg
        })
        .when(checked, |d| {
            d.child(icon(
                "check",
                12.,
                if enabled { t.on_accent } else { t.ink_disabled },
            ))
        });

    let mut text_col = div().flex().flex_col().gap(px(1.)).child(
        div()
            .text_size(px(12.5))
            .text_color(if enabled { t.ink } else { t.ink_disabled })
            .child(label.to_owned()),
    );
    if let Some(h) = hint {
        text_col = text_col.child(
            div()
                .text_size(px(11.5))
                .text_color(t.ink_tertiary)
                .child(h.to_owned()),
        );
    }

    div()
        .id(id.into())
        .flex()
        .items_start()
        .gap(px(9.))
        .px(px(6.))
        .py(px(4.))
        .rounded(px(4.))
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
        .child(box_el)
        .child(text_col)
}

/// Titled `QGroupBox` / WinUI 3 settings card container.
pub fn group_box(title: &str, subtitle: Option<&str>, t: &Theme) -> Div {
    let mut header = div().flex().flex_col().gap(px(2.)).child(
        div()
            .text_size(px(13.))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(t.ink)
            .child(title.to_owned()),
    );
    if let Some(sub) = subtitle {
        header = header.child(
            div()
                .text_size(px(11.5))
                .text_color(t.ink_secondary)
                .child(sub.to_owned()),
        );
    }
    div()
        .flex()
        .flex_col()
        .gap(px(8.))
        .w_full()
        .p(px(12.))
        .rounded(px(6.))
        .bg(t.card)
        .border_1()
        .border_color(t.control_border)
        .child(header)
}

/// Popup menu row (`FlyoutPresenter` item).
pub fn menu_row(
    id: impl Into<SharedString>,
    glyph: Option<&'static str>,
    label: &str,
    shortcut: Option<&str>,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    let fg = if enabled { t.ink } else { t.ink_disabled };
    let hover = t.subtle_hover;
    let icon_slot = match glyph {
        Some(g) => div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(px(16.))
            .child(icon(g, 15., fg)),
        None => div().flex_none().size(px(16.)),
    };
    let mut row = div()
        .id(id.into())
        .flex()
        .items_center()
        .gap(px(9.))
        .w_full()
        .h(px(28.))
        .px(px(10.))
        .rounded(px(4.))
        .text_size(px(12.5))
        .text_color(fg)
        .when(enabled, |d| d.cursor_pointer().hover(move |s| s.bg(hover)))
        .child(icon_slot)
        .child(div().flex_1().child(label.to_owned()));
    if let Some(sc) = shortcut {
        row = row.child(
            div()
                .flex_none()
                .text_size(px(11.5))
                .text_color(t.ink_tertiary)
                .child(sc.to_owned()),
        );
    }
    row
}

pub fn menu_separator(t: &Theme) -> Div {
    div()
        .flex_none()
        .w_full()
        .h(px(1.))
        .my(px(4.))
        .bg(t.divider)
}

/// Flyout popup container (`8px` radius, `1px` border).
pub fn menu_frame(width: f32, t: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .w(px(width))
        .p(px(4.))
        .rounded(px(8.))
        .bg(t.flyout)
        .border_1()
        .border_color(t.flyout_border)
}

/// Inline status banner inside a page (`is_err` switches between success/info and danger tint).
pub fn status_banner(message: &str, is_err: bool, t: &Theme) -> Div {
    let fg = if is_err { t.danger } else { t.accent };
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .w_full()
        .px(px(10.))
        .py(px(6.))
        .rounded(px(4.))
        .bg(t.card)
        .border_1()
        .border_color(fg)
        .text_size(px(12.))
        .text_color(if is_err { t.danger } else { t.ink })
        .child(icon(if is_err { "kill" } else { "info" }, 14., fg))
        .child(div().flex_1().child(message.to_owned()))
}

/// Colour-codes a Minecraft / Fabric / Horizon log line matching JLac's `LogView`.
pub fn log_line_color(line: &str, highlight: bool, t: &Theme) -> Hsla {
    if !highlight {
        return t.ink;
    }
    let upper = line.to_ascii_uppercase();
    if upper.contains("/FATAL")
        || upper.contains("[FATAL]")
        || upper.contains("/ERROR")
        || upper.contains("[ERROR]")
        || upper.contains("EXCEPTION")
        || line.starts_with("\tat ")
        || upper.contains("CAUSED BY:")
    {
        t.danger
    } else if upper.contains("/WARN") || upper.contains("[WARN]") || upper.contains("WARNING:") {
        t.warn
    } else if line.starts_with("[FNH]") {
        t.accent
    } else if upper.contains("/DEBUG") || upper.contains("[DEBUG]") {
        t.ink_tertiary
    } else {
        t.ink
    }
}

/// Shared keyboard text-editing helper for single-line and multi-line inputs across all windows.
/// Handles typing, Backspace, Ctrl+Backspace (delete word), Ctrl+A/Delete clear, Enter/newline
/// (when `multiline`), and `Ctrl+V` clipboard paste!
pub fn apply_text_key(
    buf: &mut String,
    event: &gpui::KeyDownEvent,
    multiline: bool,
    cx: &gpui::App,
) -> bool {
    let ks = &event.keystroke;
    let key = ks.key.as_str();
    let ctrl = ks.modifiers.control || ks.modifiers.platform;

    if ctrl {
        match key {
            "v" => {
                if let Some(item) = cx.read_from_clipboard() {
                    if let Some(text) = item.text() {
                        if multiline {
                            buf.push_str(&text.replace("\r\n", "\n"));
                        } else {
                            let one_line: String = text
                                .chars()
                                .filter(|c| !matches!(c, '\r' | '\n'))
                                .collect();
                            buf.push_str(&one_line);
                        }
                        return true;
                    }
                }
                return false;
            }
            "backspace" => {
                let trimmed = buf.trim_end_matches(' ');
                let cut = trimmed
                    .rfind(|c: char| c.is_whitespace() || matches!(c, '/' | '\\' | '-'))
                    .map(|i| i + 1)
                    .unwrap_or(0);
                buf.truncate(cut);
                return true;
            }
            "u" => {
                buf.clear();
                return true;
            }
            _ => return false,
        }
    }

    match key {
        "backspace" => {
            buf.pop();
            true
        }
        "delete" => {
            buf.clear();
            true
        }
        "space" => {
            buf.push(' ');
            true
        }
        "enter" if multiline => {
            buf.push('\n');
            true
        }
        _ => {
            if let Some(ch) = &ks.key_char {
                if !ch.chars().any(char::is_control) {
                    buf.push_str(ch);
                    return true;
                }
            } else if key.len() == 1 && !ks.modifiers.alt {
                let mut s = key.to_owned();
                if ks.modifiers.shift {
                    s = s.to_ascii_uppercase();
                }
                buf.push_str(&s);
                return true;
            }
            false
        }
    }
}

/// Concise subtitle describing what an instance runs (shown in status bar, list view & headers).
pub fn instance_summary(instance: &horizon_launcher::instances::Instance) -> String {
    match &instance.java {
        Some(java) => match &java.loader {
            Some(loader) => format!(
                "Minecraft {} · {} {}",
                java.minecraft,
                loader.kind.name(),
                loader.version
            ),
            None => format!("Minecraft {} (Vanilla)", java.minecraft),
        },
        None => match &instance.version {
            Some(v) => format!("Horizon · {v}"),
            None => "Horizon".into(),
        },
    }
}
