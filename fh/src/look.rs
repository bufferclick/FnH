//! Windows 11 (WinUI 3) theme and widgets, matching Qt 6's `QWindows11Style` as JLac draws it:
//! Mica-tinted window chrome (`#f3f3f3` light / `#202020` dark), elevated central view layer
//! (`#ffffff` light / `#272727` dark), 4-pixel control corners, 8-pixel flyout and `ContentDialog`
//! corners, 1-pixel `ControlStrokeColorDefault` borders, bottom-accented `TextBox` inputs, left
//! 3x16-pixel accent selection pills in `PageContainer` sidebars, and Fluent Blue (`#0067c0`
//! light / `#60cdff` dark) as the default system accent.

use crate::icons::icon;
use crate::prefs::{Accent, ThemeMode};
use gpui::{
    div, prelude::*, px, rgb, AnyElement, Div, FontWeight, Hsla, SharedString, Stateful,
    WindowAppearance,
};
use horizon_launcher::instances::{Icon, Instance};
use std::time::Duration;

/// Windows 11 system UI and console font stacks.
pub const UI_FONT: &str = "Segoe UI";
pub const MONO_FONT: &str = "Consolas";

/// FNH's own logo colours (`icons/fh.svg`): porcelain tile, navy F & H, black n.
pub const MARK_TILE: u32 = 0xf8f9fc;
pub const MARK_RING: u32 = 0xd8dae5;
pub const MARK_NAVY: u32 = 0x131254;
pub const MARK_INK: u32 = 0x000000;

/// Curated instance tile colours when the user clicks an instance icon to recolour it.
pub const ICON_PALETTE: &[&str] = &[
    "#0067c0", "#131254", "#0f7b0f", "#5c43b8", "#b146c2", "#c42b1c", "#ca5010", "#038387",
    "#498205", "#4a5459",
];

/// Converts `#rrggbb` to GPUI `Hsla`.
pub fn hex(value: u32) -> Hsla {
    rgb(value).into()
}

/// Converts `#rrggbb` with `alpha` (`0.0..=1.0`) to GPUI `Hsla`.
pub fn hexa(value: u32, alpha: f32) -> Hsla {
    let mut colour: Hsla = rgb(value).into();
    colour.a = alpha;
    colour
}

/// Resolved WinUI 3 surface, stroke, text and accent tokens for one frame.
#[derive(Clone, Copy)]
pub struct Theme {
    pub dark: bool,
    /// Mica window chrome: top toolbar, right instance bar, status bar.
    pub window: Hsla,
    /// Central `InstanceView` viewport and primary content pane.
    pub layer: Hsla,
    /// Secondary panel / sidebar surface inside `PageContainer` dialogs.
    pub sidebar: Hsla,
    /// Standard resting control fill (`ControlFillColorDefault`).
    pub control: Hsla,
    /// Hovered standard control fill (`ControlFillColorSecondary`).
    pub control_hover: Hsla,
    /// Standard 1px control outline (`ControlStrokeColorDefault`).
    pub control_border: Hsla,
    /// Bottom elevation line on standard buttons.
    pub control_bottom: Hsla,
    /// Subtle hover fill for toolbar buttons, menu rows and instance tiles.
    pub subtle_hover: Hsla,
    /// Subtle selected fill for sidebar items, checked tool buttons and table rows.
    pub subtle_selected: Hsla,
    /// Selected instance tile background and border in `InstanceView`.
    pub tile_selected: Hsla,
    pub tile_selected_border: Hsla,
    /// `TextBox` background and resting bottom underline (`ControlStrongStrokeColorDefault`).
    pub input_bg: Hsla,
    pub input_bottom: Hsla,
    /// 1px structural divider (`DividerStrokeColorDefault`).
    pub divider: Hsla,
    /// Popup menu surface and stroke (`FlyoutPresenter`).
    pub flyout: Hsla,
    pub flyout_border: Hsla,
    /// `ContentDialog` body and distinct bottom command-bar surface.
    pub dialog_bg: Hsla,
    pub dialog_footer: Hsla,
    /// WinUI 3 text hierarchy (`Primary`, `Secondary`, `Tertiary`, `Disabled`).
    pub ink: Hsla,
    pub ink_secondary: Hsla,
    pub ink_tertiary: Hsla,
    pub ink_disabled: Hsla,
    /// Active Fluent accent fill, hover fill and foreground text.
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub on_accent: Hsla,
    /// Semantic status colours (`Critical`, `Success`).
    pub danger: Hsla,
    pub danger_hover: Hsla,
    pub on_danger: Hsla,
    pub success: Hsla,
}

impl Theme {
    /// Resolves WinUI 3 tokens for `mode`, `accent` and the OS window appearance.
    pub fn resolve(mode: ThemeMode, accent: Accent, system: WindowAppearance) -> Self {
        let dark = match mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            ThemeMode::System => {
                !matches!(system, WindowAppearance::Light | WindowAppearance::VibrantLight)
            }
        };
        let (acc, acc_hover, on_acc) = accent.colours(dark);
        if dark {
            Self {
                dark: true,
                window: hex(0x202020),
                layer: hex(0x272727),
                sidebar: hex(0x202020),
                control: hex(0x2d2d2d),
                control_hover: hex(0x343434),
                control_border: hex(0x3a3a3a),
                control_bottom: hex(0x303030),
                subtle_hover: hex(0x2e2e2e),
                subtle_selected: hex(0x353535),
                tile_selected: hexa(acc, 0.16),
                tile_selected_border: hexa(acc, 0.65),
                input_bg: hex(0x252525),
                input_bottom: hex(0x9a9a9a),
                divider: hex(0x323232),
                flyout: hex(0x2c2c2c),
                flyout_border: hex(0x404040),
                dialog_bg: hex(0x2b2b2b),
                dialog_footer: hex(0x1f1f1f),
                ink: hex(0xffffff),
                ink_secondary: hex(0xc6c6c6),
                ink_tertiary: hex(0x8f8f8f),
                ink_disabled: hex(0x6d6d6d),
                accent: hex(acc),
                accent_hover: hex(acc_hover),
                on_accent: hex(on_acc),
                danger: hex(0xc42b1c),
                danger_hover: hex(0xd13b2c),
                on_danger: hex(0xffffff),
                success: hex(0x6ccb5f),
            }
        } else {
            Self {
                dark: false,
                window: hex(0xf3f3f3),
                layer: hex(0xffffff),
                sidebar: hex(0xf3f3f3),
                control: hex(0xfbfbfb),
                control_hover: hex(0xf5f5f5),
                control_border: hex(0xe2e2e2),
                control_bottom: hex(0xcccccc),
                subtle_hover: hex(0xe8e8e8),
                subtle_selected: hex(0xe2e2e2),
                tile_selected: hexa(acc, 0.11),
                tile_selected_border: hexa(acc, 0.55),
                input_bg: hex(0xffffff),
                input_bottom: hex(0x868686),
                divider: hex(0xe3e3e3),
                flyout: hex(0xfcfcfc),
                flyout_border: hex(0xd8d8d8),
                dialog_bg: hex(0xffffff),
                dialog_footer: hex(0xf3f3f3),
                ink: hex(0x1b1b1b),
                ink_secondary: hex(0x5c5c5c),
                ink_tertiary: hex(0x767676),
                ink_disabled: hex(0x9e9e9e),
                accent: hex(acc),
                accent_hover: hex(acc_hover),
                on_accent: hex(on_acc),
                danger: hex(0xc42b1c),
                danger_hover: hex(0xb02315),
                on_danger: hex(0xffffff),
                success: hex(0x0f7b0f),
            }
        }
    }
}

/// Visual style of a WinUI 3 `PushButton`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonKind {
    Accent,
    Standard,
    Subtle,
    Danger,
}

/// WinUI 3 `PushButton`: 30px tall, 4px corners, 1px elevation stroke.
pub fn push_button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    kind: ButtonKind,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let (bg, hover, border, bottom, fg) = if !enabled {
        (
            t.control.opacity(0.55),
            t.control.opacity(0.55),
            t.control_border.opacity(0.6),
            t.control_border.opacity(0.6),
            t.ink_disabled,
        )
    } else {
        match kind {
            ButtonKind::Accent => (t.accent, t.accent_hover, t.accent, t.accent_hover, t.on_accent),
            ButtonKind::Standard => (
                t.control,
                t.control_hover,
                t.control_border,
                t.control_bottom,
                t.ink,
            ),
            ButtonKind::Subtle => (
                gpui::transparent_black(),
                t.subtle_hover,
                gpui::transparent_black(),
                gpui::transparent_black(),
                t.ink,
            ),
            ButtonKind::Danger => (t.danger, t.danger_hover, t.danger, t.danger_hover, t.on_danger),
        }
    };
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .gap(px(6.))
        .h(px(30.))
        .min_w(px(76.))
        .px(px(14.))
        .rounded(px(4.))
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_color(fg)
        .text_size(px(12.5))
        .font_weight(if kind == ButtonKind::Accent {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .overflow_hidden()
        .when(enabled, |el| el.cursor_pointer().hover(move |s| s.bg(hover)))
        .when(!enabled, |el| el.cursor_default())
        .when(kind == ButtonKind::Standard && enabled, |el| {
            el.child(
                div()
                    .absolute()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .h(px(1.))
                    .bg(bottom),
            )
        })
        .child(label)
}

/// Top `mainToolBar` `QToolButton` (`ToolButtonTextBesideIcon`): 30px tall, 4px corners, 16px icon
/// beside label, and a subtle chevron when the button opens a popup menu.
pub fn tool_button(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    label: impl Into<SharedString>,
    has_menu: bool,
    open: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let hover = t.subtle_hover;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(30.))
        .px(px(9.))
        .rounded(px(4.))
        .bg(if open {
            t.subtle_selected
        } else {
            gpui::transparent_black()
        })
        .text_color(t.ink)
        .text_size(px(12.5))
        .cursor_pointer()
        .when(!open, |el| el.hover(move |s| s.bg(hover)))
        .child(icon(icon_name, 16., t.ink))
        .child(div().whitespace_nowrap().child(label))
        .when(has_menu, |el| {
            el.child(icon("chevron-down", 12., t.ink_secondary))
        })
}

/// Right-hand `instanceToolBar` (`WideBar`) button: full width, left-aligned 16px icon + text.
pub fn wide_action(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    label: &'static str,
    primary: bool,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    if primary && enabled {
        let hover = t.accent_hover;
        return div()
            .id(id)
            .flex()
            .flex_none()
            .items_center()
            .gap(px(8.))
            .w_full()
            .h(px(30.))
            .px(px(10.))
            .rounded(px(4.))
            .bg(t.accent)
            .text_color(t.on_accent)
            .text_size(px(12.5))
            .font_weight(FontWeight::SEMIBOLD)
            .cursor_pointer()
            .hover(move |s| s.bg(hover))
            .child(icon(icon_name, 15., t.on_accent))
            .child(label);
    }
    let fg = if enabled { t.ink } else { t.ink_disabled };
    let ic = if enabled {
        t.ink_secondary
    } else {
        t.ink_disabled
    };
    let hover = t.subtle_hover;
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .w_full()
        .h(px(29.))
        .px(px(10.))
        .rounded(px(4.))
        .text_color(fg)
        .text_size(px(12.5))
        .when(enabled, |el| el.cursor_pointer().hover(move |s| s.bg(hover)))
        .when(!enabled, |el| el.cursor_default())
        .child(icon(icon_name, 15., ic))
        .child(label)
}

/// Vertical 1px separator in the top toolbar (`QToolBar::separator`).
pub fn vsep(t: &Theme) -> Div {
    div()
        .flex_none()
        .w(px(1.))
        .h(px(20.))
        .mx(px(4.))
        .bg(t.divider)
}

/// Horizontal 1px divider (`DividerStrokeColorDefault`).
pub fn hsep(t: &Theme) -> Div {
    div().flex_none().w_full().h(px(1.)).bg(t.divider)
}

/// WinUI 3 `NavigationView` / JLac `PageContainer` sidebar row, with the signature 3x16-pixel
/// rounded vertical accent bar on the selected page.
pub fn nav_item(
    id: impl Into<SharedString>,
    icon_name: &'static str,
    label: &'static str,
    selected: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let hover = t.subtle_hover;
    div()
        .id(id)
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(10.))
        .w_full()
        .h(px(34.))
        .pl(px(12.))
        .pr(px(10.))
        .rounded(px(4.))
        .bg(if selected {
            t.subtle_selected
        } else {
            gpui::transparent_black()
        })
        .text_color(t.ink)
        .text_size(px(12.5))
        .font_weight(if selected {
            FontWeight::SEMIBOLD
        } else {
            FontWeight::NORMAL
        })
        .cursor_pointer()
        .when(!selected, |el| el.hover(move |s| s.bg(hover)))
        .when(selected, |el| {
            el.child(
                div()
                    .absolute()
                    .left(px(2.))
                    .top(px(9.))
                    .w(px(3.))
                    .h(px(16.))
                    .rounded(px(2.))
                    .bg(t.accent),
            )
        })
        .child(icon(
            icon_name,
            16.,
            if selected { t.accent } else { t.ink_secondary },
        ))
        .child(label)
}

/// WinUI 3 `TextBox` (`QLineEdit`): 32px tall, 4px corners, 1px resting bottom line that expands
/// to a 2px Fluent accent indicator when focused.
pub fn text_box(
    id: impl Into<SharedString>,
    value: &str,
    placeholder: &str,
    focused: bool,
    search_icon: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let empty = value.is_empty();
    let shown = if empty {
        placeholder.to_owned()
    } else {
        value.to_owned()
    };
    div()
        .id(id)
        .relative()
        .flex()
        .items_center()
        .gap(px(7.))
        .h(px(32.))
        .px(px(10.))
        .rounded(px(4.))
        .bg(t.input_bg)
        .border_1()
        .border_color(if focused { t.accent } else { t.control_border })
        .overflow_hidden()
        .cursor_text()
        .when(search_icon, |el| {
            el.child(icon("search", 14., t.ink_tertiary))
        })
        .child(
            div()
                .flex()
                .items_center()
                .flex_1()
                .min_w(px(0.))
                .text_size(px(12.5))
                .text_color(if empty { t.ink_tertiary } else { t.ink })
                .child(div().truncate().child(shown))
                .when(focused && !empty, |el| {
                    el.child(
                        div()
                            .flex_none()
                            .w(px(1.5))
                            .h(px(15.))
                            .ml(px(1.))
                            .bg(t.ink),
                    )
                })
                .when(focused && empty, |el| {
                    el.child(
                        div()
                            .absolute()
                            .left(px(if search_icon { 31. } else { 10. }))
                            .top(px(8.))
                            .w(px(1.5))
                            .h(px(15.))
                            .bg(t.ink),
                    )
                }),
        )
        .child(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom_0()
                .h(px(if focused { 2. } else { 1. }))
                .bg(if focused { t.accent } else { t.input_bottom }),
        )
}

/// WinUI 3 `CheckBox`: 18x18 4px-rounded box filled in the Fluent accent when checked.
pub fn check_box(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    checked: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(28.))
        .cursor_pointer()
        .child(
            div()
                .flex()
                .flex_none()
                .items_center()
                .justify_center()
                .size(px(18.))
                .rounded(px(4.))
                .bg(if checked { t.accent } else { t.input_bg })
                .border_1()
                .border_color(if checked { t.accent } else { t.input_bottom })
                .when(checked, |box_el| box_el.child(icon("check", 13., t.on_accent))),
        )
        .child(
            div()
                .text_size(px(12.5))
                .text_color(t.ink)
                .whitespace_nowrap()
                .child(label),
        )
}

/// WinUI 3 popup `QMenu` (`FlyoutPresenter`): 8px outer corners, subtle border and drop shadow.
pub fn flyout_card(width: f32, children: Vec<AnyElement>, t: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .w(px(width))
        .p(px(4.))
        .gap(px(1.))
        .rounded(px(8.))
        .bg(t.flyout)
        .border_1()
        .border_color(t.flyout_border)
        .shadow_lg()
        .children(children)
}

/// One row inside a WinUI 3 popup menu.
pub fn menu_item(
    id: impl Into<SharedString>,
    icon_name: Option<&'static str>,
    label: impl Into<SharedString>,
    shortcut: Option<&'static str>,
    checked: bool,
    enabled: bool,
    t: &Theme,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let fg = if enabled { t.ink } else { t.ink_disabled };
    let ic = if checked {
        t.accent
    } else if enabled {
        t.ink_secondary
    } else {
        t.ink_disabled
    };
    let hover = t.subtle_hover;
    let lead = if checked { Some("check") } else { icon_name };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .w_full()
        .h(px(30.))
        .px(px(10.))
        .rounded(px(4.))
        .bg(if checked {
            t.subtle_selected
        } else {
            gpui::transparent_black()
        })
        .text_color(fg)
        .text_size(px(12.5))
        .when(enabled, |el| el.cursor_pointer().hover(move |s| s.bg(hover)))
        .when(!enabled, |el| el.cursor_default())
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(9.))
                .min_w(px(0.))
                .child(
                    div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(16.))
                        .when_some(lead, |slot, name| slot.child(icon(name, 15., ic))),
                )
                .child(div().truncate().child(label)),
        )
        .when_some(shortcut, |el, key| {
            el.child(
                div()
                    .flex_none()
                    .text_size(px(11.))
                    .text_color(t.ink_tertiary)
                    .child(key),
            )
        })
}

/// Horizontal divider inside a WinUI 3 popup menu.
pub fn menu_sep(t: &Theme) -> AnyElement {
    div()
        .flex_none()
        .h(px(1.))
        .mx(px(8.))
        .my(px(4.))
        .bg(t.divider)
        .into_any_element()
}

/// WinUI 3 `ContentDialog` container: 8px corners, content surface on top, command bar below.
pub fn dialog_shell(width: f32, body: AnyElement, footer: Div, t: &Theme) -> Div {
    div()
        .flex()
        .flex_col()
        .w(px(width))
        .max_h(px(580.))
        .rounded(px(8.))
        .bg(t.dialog_bg)
        .border_1()
        .border_color(t.flyout_border)
        .shadow_lg()
        .overflow_hidden()
        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(body)
        .child(hsep(t))
        .child(
            footer
                .flex()
                .flex_none()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .px(px(20.))
                .py(px(14.))
                .bg(t.dialog_footer),
        )
}

/// WinUI 3 linear progress indicator (`ProgressBar`).
pub fn progress_bar(fraction: f32, width: f32, t: &Theme) -> Div {
    let fill = (width * fraction.clamp(0., 1.)).max(0.);
    div()
        .flex()
        .flex_none()
        .w(px(width))
        .h(px(4.))
        .rounded(px(2.))
        .bg(t.divider)
        .overflow_hidden()
        .child(div().w(px(fill)).h_full().rounded(px(2.)).bg(t.accent))
}

/// FNH's own 9x9 monogram (`icons/fh.svg`): deep-navy F & H and black n on a porcelain tile.
pub fn brand_mark(size: f32) -> Div {
    let cell = ((size * 0.625) / 9.).floor().max(1.);
    let span = cell * 9.;
    let radius = (size * (14. / 64.)).round().max(3.);
    let bar = |x: f32, y: f32, w: f32, h: f32, colour: u32| {
        div()
            .absolute()
            .left(px(x * cell))
            .top(px(y * cell))
            .w(px(w * cell))
            .h(px(h * cell))
            .bg(hex(colour))
    };
    let glyph = div()
        .relative()
        .flex_none()
        .size(px(span))
        .child(bar(0., 0., 1., 9., MARK_NAVY))
        .child(bar(1., 0., 4., 1., MARK_NAVY))
        .child(bar(1., 3., 2., 1., MARK_NAVY))
        .child(bar(2., 5., 1., 4., MARK_INK))
        .child(bar(3., 5., 1., 1., MARK_INK))
        .child(bar(4., 5., 1., 4., MARK_INK))
        .child(bar(6., 0., 1., 9., MARK_NAVY))
        .child(bar(7., 4., 1., 1., MARK_NAVY))
        .child(bar(8., 0., 1., 9., MARK_NAVY));
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(radius))
        .bg(hex(MARK_TILE))
        .border_1()
        .border_color(hex(MARK_RING))
        .child(glyph)
}

/// Square instance emblem (48px in `InstanceView`, 56px in `instanceToolBar`), with a small
/// loader badge in the bottom-right and a green dot when the instance is running.
pub fn instance_icon(instance: &Instance, size: f32, running: bool, t: &Theme) -> Div {
    let plate = colour_of(&instance.id, instance.icon.as_ref());
    let radius = (size * 0.18).round().max(6.);
    let is_fabric = instance
        .java
        .as_ref()
        .and_then(|java| java.loader.as_ref())
        .is_some_and(|loader| loader.kind == horizon_launcher::loaders::Kind::Fabric);
    let tag = if is_fabric {
        Some("F")
    } else if instance.java.is_none() {
        Some("H")
    } else {
        None
    };
    div()
        .relative()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(radius))
        .bg(plate)
        .text_color(hex(0xffffff))
        .text_size(px((size * 0.44).round()))
        .font_weight(FontWeight::BOLD)
        .child(initial(&instance.name))
        .when_some(tag, |el, badge| {
            el.child(
                div()
                    .absolute()
                    .right(px(-2.))
                    .bottom(px(-2.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px((size * 0.34).round().max(15.)))
                    .rounded(px(4.))
                    .bg(t.dialog_bg)
                    .border_1()
                    .border_color(t.control_border)
                    .text_size(px(9.5))
                    .font_weight(FontWeight::BOLD)
                    .text_color(t.ink)
                    .child(badge),
            )
        })
        .when(running, |el| {
            el.child(
                div()
                    .absolute()
                    .right(px(-2.))
                    .top(px(-2.))
                    .size(px(11.))
                    .rounded(px(6.))
                    .bg(t.success)
                    .border_1()
                    .border_color(t.dialog_bg),
            )
        })
}

/// First letter of `name`, upper-cased, for an instance icon.
pub fn initial(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map(|first| first.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_owned())
}

/// Colour of an instance's icon tile: explicit `#rrggbb` if set, otherwise a stable Fluent hue.
pub fn colour_of(id: &str, icon: Option<&Icon>) -> Hsla {
    if let Some(Icon::Colour(raw)) = icon {
        if let Some(digits) = raw.strip_prefix('#') {
            if digits.len() == 6 && digits.chars().all(|c| c.is_ascii_hexdigit()) {
                if let Ok(value) = u32::from_str_radix(digits, 16) {
                    return hex(value);
                }
            }
        }
    }
    let seed = match icon {
        Some(Icon::Texture(texture)) => format!("{texture}{id}"),
        _ => id.to_owned(),
    };
    let mut hash: u32 = 2_166_136_261;
    for byte in seed.bytes() {
        hash = (hash ^ byte as u32).wrapping_mul(16_777_619);
    }
    let pick = ICON_PALETTE[(hash as usize) % ICON_PALETTE.len()];
    u32::from_str_radix(&pick[1..], 16)
        .map(hex)
        .unwrap_or_else(|_| hex(0x0067c0))
}

/// Next colour in `ICON_PALETTE` after `current`.
pub fn next_palette_colour(current: Option<&Icon>) -> String {
    let now = match current {
        Some(Icon::Colour(raw)) => raw.as_str(),
        _ => "",
    };
    let idx = ICON_PALETTE
        .iter()
        .position(|entry| entry.eq_ignore_ascii_case(now))
        .map(|i| (i + 1) % ICON_PALETTE.len())
        .unwrap_or(0);
    ICON_PALETTE[idx].to_owned()
}

/// JLac status-bar duration format: `"0s"`, `"42s"`, `"12m 5s"`, `"3h 14m"`.
pub fn playtime(duration: Duration) -> String {
    let secs = duration.as_secs();
    if secs < 60 {
        return format!("{secs}s");
    }
    let mins = secs / 60;
    if mins < 60 {
        let rem = secs % 60;
        return if rem == 0 {
            format!("{mins}m")
        } else {
            format!("{mins}m {rem}s")
        };
    }
    let hours = mins / 60;
    let rem_mins = mins % 60;
    if rem_mins == 0 {
        format!("{hours}h")
    } else {
        format!("{hours}h {rem_mins}m")
    }
}

/// Truncates the front of a long path with an ellipsis.
pub fn short_path(text: &str, keep: usize) -> String {
    if text.chars().count() <= keep {
        return text.to_owned();
    }
    let tail: String = text
        .chars()
        .rev()
        .take(keep.saturating_sub(1))
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("…{tail}")
}

#[cfg(test)]
mod tests {
    use super::{colour_of, hex, initial, next_palette_colour, playtime, short_path, Theme};
    use crate::prefs::{Accent, ThemeMode};
    use gpui::WindowAppearance;
    use horizon_launcher::instances::Icon;
    use std::time::Duration;

    #[test]
    fn windows11_theme_follows_system_and_explicit_modes() {
        let light = Theme::resolve(ThemeMode::System, Accent::Blue, WindowAppearance::Light);
        let dark = Theme::resolve(ThemeMode::System, Accent::Blue, WindowAppearance::Dark);
        assert!(!light.dark);
        assert!(dark.dark);
        assert_eq!(light.window, hex(0xf3f3f3));
        assert_eq!(light.layer, hex(0xffffff));
        assert_eq!(light.accent, hex(0x0067c0));
        assert_eq!(dark.window, hex(0x202020));
        assert_eq!(dark.layer, hex(0x272727));
        assert_eq!(dark.accent, hex(0x60cdff));

        let forced = Theme::resolve(ThemeMode::Light, Accent::Navy, WindowAppearance::Dark);
        assert!(!forced.dark);
    }

    #[test]
    fn icons_colours_playtime_and_paths_behave() {
        assert_eq!(initial("Horizon"), "H");
        assert_eq!(initial("  fabric pack"), "F");
        assert_eq!(initial("   "), "?");
        let custom = Some(Icon::Colour("#3fa7d6".into()));
        assert_eq!(colour_of("pack", custom.as_ref()), hex(0x3fa7d6));
        assert_eq!(
            next_palette_colour(Some(&Icon::Colour("#0067c0".into()))),
            "#131254"
        );
        assert_eq!(playtime(Duration::ZERO), "0s");
        assert_eq!(playtime(Duration::from_secs(125)), "2m 5s");
        assert_eq!(playtime(Duration::from_secs(7500)), "2h 5m");
        assert_eq!(
            short_path(r"C:\Users\Doue\AppData\Roaming\horizon", 12),
            r"…ing\horizon"
        );
    }
}
