//! JLac's look: Prism Launcher's window in the dark theme this fork is drawn in — the flat greys
//! of its window, base and alternate rows, one lime highlight with black text on it, teal links,
//! 3-pixel corners, 30-pixel controls. Colours are JLac's `DarkTheme.cpp` palette; nothing here
//! reads the Horizon launcher's own theme, so the two windows look nothing alike on purpose.

use gpui::{div, prelude::*, px, rgb, Div, FontWeight, Hsla, SharedString, Stateful};

/// A colour, from the `#rrggbb` JLac writes.
pub fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

/// JLac's dark palette (`launcher/ui/themes/CatPack.cpp`, the dark one).
pub const WINDOW: u32 = 0x313131;
pub const BASE: u32 = 0x222222;
pub const ALT: u32 = 0x2a2a2a;
pub const BUTTON: u32 = 0x303030;
pub const BUTTON_HOVER: u32 = 0x3a3a3a;
pub const RULE: u32 = 0x1b1b1b;
pub const BORDER: u32 = 0x3c3c3c;
pub const INK: u32 = 0xffffff;
pub const MUTED: u32 = 0xa6a6a6;
pub const DIM: u32 = 0x8a8a8a;
pub const ACCENT: u32 = 0x96db59;
pub const ACCENT_HOVER: u32 = 0x8ad04a;
pub const ON_ACCENT: u32 = 0x101010;
pub const LINK: u32 = 0x2fa3c6;
pub const DANGER: u32 = 0xe0715f;
pub const TILE: u32 = 0x0b0b0b;

/// The three kinds of control JLac draws: the accent button, a grey one, and one with no plate.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Primary,
    Secondary,
    Quiet,
}

/// A control: 30 pixels tall, 3-pixel corners, the label centred in it.
pub fn button(
    id: impl Into<SharedString>,
    label: impl Into<SharedString>,
    kind: Kind,
) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    let (plate, ink, hover) = match kind {
        Kind::Primary => (ACCENT, ON_ACCENT, ACCENT_HOVER),
        Kind::Secondary => (BUTTON, INK, BUTTON_HOVER),
        Kind::Quiet => (BASE, MUTED, ALT),
    };
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .h(px(30.))
        .px(px(14.))
        .rounded(px(3.))
        .bg(c(plate))
        .text_color(c(ink))
        .text_size(px(12.))
        .font_weight(FontWeight::SEMIBOLD)
        .cursor_pointer()
        .hover(move |style| style.bg(c(hover)))
        .child(label)
}

/// The toolbar's Play: the same accent plate, taller and wider, because it is the one button the
/// window exists for.
pub fn play_button(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .w(px(112.))
        .h(px(38.))
        .rounded(px(3.))
        .bg(c(ACCENT))
        .text_color(c(ON_ACCENT))
        .text_size(px(14.))
        .font_weight(FontWeight::BOLD)
        .cursor_pointer()
        .hover(|style| style.bg(c(ACCENT_HOVER)))
        .child(label)
}

/// A small plate of text: what a row says under its name, or the account on the right of the bar.
pub fn chip(id: impl Into<SharedString>, label: impl Into<SharedString>) -> Stateful<Div> {
    let id: SharedString = id.into();
    let label: SharedString = label.into();
    div()
        .id(id)
        .flex()
        .flex_none()
        .items_center()
        .gap(px(6.))
        .h(px(26.))
        .px(px(10.))
        .rounded(px(3.))
        .bg(c(BUTTON))
        .text_color(c(INK))
        .text_size(px(12.))
        .cursor_pointer()
        .hover(|style| style.bg(c(BUTTON_HOVER)))
        .child(label)
}

/// A row's picture: the instance's letter in white on the colour it has, 3-pixel corners — the
/// shape JLac's own mark is drawn in, at a row's size.
pub fn tile(letter: &str, colour: Hsla, size: f32) -> Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(size))
        .rounded(px(3.))
        .bg(colour)
        .text_color(c(INK))
        .text_size(px((size * 0.5).round()))
        .font_weight(FontWeight::BOLD)
        .child(letter.to_owned())
}

/// The state dot of the status bar's left end.
pub fn dot(colour: Hsla) -> Div {
    div().flex_none().size(px(7.)).rounded(px(4.)).bg(colour)
}

/// A section's small heading: "INSTANCES", "MODS", and the like.
pub fn heading(label: &str) -> Div {
    div()
        .flex_none()
        .text_size(px(10.))
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(c(DIM))
        .child(label.to_owned())
}

/// A line of text at `size`, in the window's ink.
pub fn line(size: f32, label: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(size))
        .text_color(c(INK))
        .child(label.into())
}

/// The same, in JLac's quieter grey: what a row says under its name.
pub fn quiet(size: f32, label: impl Into<SharedString>) -> Div {
    div()
        .text_size(px(size))
        .text_color(c(MUTED))
        .child(label.into())
}

/// The letter a tile shows: the name's first letter, upper case.
pub fn initial(name: &str) -> String {
    name.trim()
        .chars()
        .next()
        .map(|first| first.to_uppercase().to_string())
        .unwrap_or_else(|| "?".to_owned())
}

/// The colour an instance's tile takes: its own where the instance names a colour, else one
/// taken from its name, so a row keeps its colour between starts.
pub fn colour_of(id: &str, icon: Option<&horizon_launcher::instances::Icon>) -> Hsla {
    use horizon_launcher::instances::Icon;
    if let Some(Icon::Colour(hex)) = icon {
        if let Some(hex) = hex.strip_prefix('#') {
            if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
                if let Ok(value) = u32::from_str_radix(hex, 16) {
                    return c(value);
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
    // Spread over the hues that read on JLac's dark panes at full saturation.
    gpui::hsla((hash % 360) as f32 / 360., 0.42, 0.46, 1.)
}

/// `text` with everything a shell would read as its own escaped, for a path shown on the page.
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
    use super::{colour_of, initial, short_path};
    use horizon_launcher::instances::Icon;

    #[test]
    fn a_row_keeps_its_letter_and_its_colour() {
        assert_eq!(initial("Horizon"), "H");
        assert_eq!(initial("  my pack"), "M");
        assert_eq!(initial("  "), "?");
        let own = Some(Icon::Colour("#3fa7d6".into()));
        assert_eq!(colour_of("own:pack", own.as_ref()), color_of_hex(0x3fa7d6));
        // A file written by hand may say anything: only a colour is taken from it.
        let odd = Some(Icon::Colour("#fff\" onload=\"x".into()));
        assert_ne!(colour_of("own:odd", odd.as_ref()), color_of_hex(0xffffff));
        assert_eq!(colour_of("own:same", None), colour_of("own:same", None));
    }

    fn color_of_hex(hex: u32) -> gpui::Hsla {
        super::c(hex)
    }

    #[test]
    fn a_long_path_keeps_its_end() {
        assert_eq!(
            short_path(r"C:\Users\Doue\AppData\Roaming\horizon", 12),
            r"…ing\horizon"
        );
        assert_eq!(short_path("short", 12), "short");
    }
}
