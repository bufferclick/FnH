//! Built-in 24x24 vector icons for FNH's Windows 11 toolbar, right-hand `WideBar`, popup menus
//! and `PageContainer` sidebars.
//!
//! Matches the iconography JLac draws on Windows 11 (`flat` in light mode, `flat_white` in dark
//! mode): GPUI tints every SVG to `.text_color(...)`, so one crisp vector set serves light, dark,
//! accent and disabled states with zero external files.

use gpui::{prelude::*, px, svg, Hsla, SharedString, Svg};
use std::borrow::Cow;

/// Every embedded icon `(name, svg_markup)`.
pub const ALL: &[(&str, &str)] = &[
    (
        "add",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M5.5 3A2.5 2.5 0 0 0 3 5.5v13A2.5 2.5 0 0 0 5.5 21h13a2.5 2.5 0 0 0 2.5-2.5v-13A2.5 2.5 0 0 0 18.5 3h-13ZM12 7a.9.9 0 0 1 .9.9v3.2h3.2a.9.9 0 1 1 0 1.8h-3.2v3.2a.9.9 0 1 1-1.8 0v-3.2H7.9a.9.9 0 1 1 0-1.8h3.2V7.9A.9.9 0 0 1 12 7Z"/></svg>"#,
    ),
    (
        "folder",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M4.25 4.5A2.25 2.25 0 0 0 2 6.75v10.5A2.25 2.25 0 0 0 4.25 19.5h15.5A2.25 2.25 0 0 0 22 17.25v-8A2.25 2.25 0 0 0 19.75 7h-7.69L9.94 5.06a1.75 1.75 0 0 0-1.24-.56H4.25Z"/></svg>"#,
    ),
    (
        "settings",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M10.3 2.2h3.4l.45 2.55c.62.22 1.2.55 1.72.97l2.45-.92 1.7 2.94-2 1.68c.08.38.12.78.12 1.18s-.04.8-.12 1.18l2 1.68-1.7 2.94-2.45-.92c-.52.42-1.1.75-1.72.97l-.45 2.55h-3.4l-.45-2.55a7.2 7.2 0 0 1-1.72-.97l-2.45.92-1.7-2.94 2-1.68A6.8 6.8 0 0 1 5.86 10.6c0-.4.04-.8.12-1.18l-2-1.68 1.7-2.94 2.45.92c.52-.42 1.1-.75 1.72-.97l.45-2.55ZM12 8.75a3.25 3.25 0 1 0 0 6.5 3.25 3.25 0 0 0 0-6.5Z"/></svg>"#,
    ),
    (
        "help",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 14.5a1.2 1.2 0 1 1 0 2.4 1.2 1.2 0 0 1 0-2.4Zm.15-10.1c2.15 0 3.6 1.25 3.6 3.1 0 1.28-.66 2.13-1.82 2.86-.9.56-1.18.96-1.18 1.78v.26h-1.8v-.38c0-1.26.48-2.02 1.62-2.74.88-.55 1.32-1.02 1.32-1.78 0-.96-.72-1.56-1.78-1.56-1.08 0-1.78.64-1.88 1.68H8.38c.14-1.92 1.62-3.22 3.77-3.22Z"/></svg>"#,
    ),
    (
        "update",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 3a9 9 0 1 0 9 9 9 9 0 0 0-9-9Zm1 5v4.59l2.3-2.3 1.4 1.42L12 16.41l-4.7-4.7 1.4-1.42 2.3 2.3V8h2Z"/></svg>"#,
    ),
    (
        "refresh",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M17.65 6.35A7.96 7.96 0 0 0 12 4a8 8 0 1 0 7.73 10h-2.08A6 6 0 1 1 12 6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35Z"/></svg>"#,
    ),
    (
        "account",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 3.5a4 4 0 1 1 0 8 4 4 0 0 1 0-8Zm0 10c4.2 0 7.5 2.05 7.5 4.75V20a.75.75 0 0 1-.75.75H5.25A.75.75 0 0 1 4.5 20v-1.75C4.5 15.55 7.8 13.5 12 13.5Z"/></svg>"#,
    ),
    (
        "play",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M6.5 4.3a1.3 1.3 0 0 1 1.97-1.11l11.2 6.7a1.3 1.3 0 0 1 0 2.22l-11.2 6.7A1.3 1.3 0 0 1 6.5 17.7V4.3Z"/></svg>"#,
    ),
    (
        "kill",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm3.54 12.12a1 1 0 0 1-1.42 1.42L12 13.41l-2.12 2.13a1 1 0 0 1-1.42-1.42L10.59 12 8.46 9.88a1 1 0 0 1 1.42-1.42L12 10.59l2.12-2.13a1 1 0 0 1 1.42 1.42L13.41 12l2.13 2.12Z"/></svg>"#,
    ),
    (
        "edit",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M16.86 3.5a2.1 2.1 0 0 1 2.97 0l.67.67a2.1 2.1 0 0 1 0 2.97L9.3 18.34 4 20l1.66-5.3L16.86 3.5Z"/></svg>"#,
    ),
    (
        "group",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M3 5a2 2 0 0 1 2-2h6.17a2 2 0 0 1 1.42.59l8.24 8.24a2 2 0 0 1 0 2.82l-6.18 6.18a2 2 0 0 1-2.82 0L3.59 12.59A2 2 0 0 1 3 11.17V5Zm4.25 2.75a1.5 1.5 0 1 0 0 3 1.5 1.5 0 0 0 0-3Z"/></svg>"#,
    ),
    (
        "copy",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M8.5 3A2.5 2.5 0 0 0 6 5.5v10A2.5 2.5 0 0 0 8.5 18h10a2.5 2.5 0 0 0 2.5-2.5v-10A2.5 2.5 0 0 0 18.5 3h-10ZM4.5 7.5A1.5 1.5 0 0 0 3 9v9.5A2.5 2.5 0 0 0 5.5 21H15a1.5 1.5 0 0 0 1.5-1.5H5.5a1 1 0 0 1-1-1V7.5Z"/></svg>"#,
    ),
    (
        "delete",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M9.5 3h5l1 1.5H19a.9.9 0 1 1 0 1.8H5a.9.9 0 0 1 0-1.8h3.5L9.5 3ZM6.5 8h11l-.7 10.6A2.2 2.2 0 0 1 14.6 20.7H9.4a2.2 2.2 0 0 1-2.2-2.1L6.5 8Z"/></svg>"#,
    ),
    (
        "mods",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M10.5 2a2.5 2.5 0 0 1 2.5 2.5V6h3.5A2 2 0 0 1 18.5 8v3.5H20a2.2 2.2 0 1 1 0 4.4h-1.5V19a2 2 0 0 1-2 2h-3.6v-1.4a2.4 2.4 0 1 0-4.8 0V21H4.5a2 2 0 0 1-2-2v-3.6h1.4a2.4 2.4 0 1 0 0-4.8H2.5V8a2 2 0 0 1 2-2H8V4.5A2.5 2.5 0 0 1 10.5 2Z"/></svg>"#,
    ),
    (
        "world",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm-1 17.93A8 8 0 0 1 4.07 13H7l2 2v2l2 2v.93Zm6.9-2.54A7.96 7.96 0 0 1 13 19.93V18a1.5 1.5 0 0 0-1.5-1.5H9v-2.2l-3.8-3.8A8 8 0 0 1 14 4.26V6a1.5 1.5 0 0 0 1.5 1.5H18l1.73 2.6a8 8 0 0 1-1.83 7.29Z"/></svg>"#,
    ),
    (
        "log",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M6 2.5A2 2 0 0 0 4 4.5v15a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8.5l-6-6H6Zm7 1.7L18.3 9.5H13V4.2ZM8 12.2h8v1.6H8v-1.6Zm0 3.6h8v1.6H8v-1.6Z"/></svg>"#,
    ),
    (
        "cube",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 2.2 3.2 6.8v10.4L12 21.8l8.8-4.6V6.8L12 2.2Zm0 2.04 6.4 3.34L12 10.92 5.6 7.58 12 4.24Zm-7 5.08 6.1 3.18v6.84L5 16.16V9.32Zm8.1 10.02V12.5L19.2 9.32v6.84l-6.1 3.18Z"/></svg>"#,
    ),
    (
        "fabric",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 3 2.5 8 12 13l9.5-5L12 3Zm-7.6 8.3L2.5 12.3 12 17.3l9.5-5-1.9-1L12 15.3l-7.6-4Zm0 4.2L2.5 16.5 12 21.5l9.5-5-1.9-1L12 19.5l-7.6-4Z"/></svg>"#,
    ),
    (
        "horizon",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M5 4h2.5v6.75h9V4H19v16h-2.5v-6.75h-9V20H5V4Z"/></svg>"#,
    ),
    (
        "palette",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 2.5a9.5 9.5 0 0 0 0 19h1.2a2.3 2.3 0 0 0 1.72-3.83 1.3 1.3 0 0 1 .98-2.17H17.5a4 4 0 0 0 4-4C21.5 6.53 17.25 2.5 12 2.5Zm-4.5 9a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3Zm3-4a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3Zm5 0a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3Zm2.5 4a1.5 1.5 0 1 1 0-3 1.5 1.5 0 0 1 0 3Z"/></svg>"#,
    ),
    (
        "info",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm-1 5h2v2.2h-2V7Zm0 4h2v6h-2v-6Z"/></svg>"#,
    ),
    (
        "chevron-down",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M6.4 8.8 12 14.4l5.6-5.6 1.4 1.4-7 7-7-7 1.4-1.4Z"/></svg>"#,
    ),
    (
        "chevron-right",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="m8.8 6.4 1.4-1.4 7 7-7 7-1.4-1.4 5.6-5.6-5.6-5.6Z"/></svg>"#,
    ),
    (
        "check",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="m9.4 16.2-4.2-4.2-1.5 1.5 5.7 5.7L20.3 8.3l-1.5-1.5L9.4 16.2Z"/></svg>"#,
    ),
    (
        "search",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M10.5 3.5a7 7 0 1 0 4.38 12.46l4.33 4.33 1.42-1.42-4.33-4.33A7 7 0 0 0 10.5 3.5Zm0 2a5 5 0 1 1 0 10 5 5 0 0 1 0-10Z"/></svg>"#,
    ),
    (
        "external",
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24"><path fill="currentColor" d="M14 4h6v6h-2V7.41l-7.29 7.3-1.42-1.42 7.3-7.29H14V4ZM5 5h6v2H6v11h11v-5h2v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V6a1 1 0 0 1 1-1Z"/></svg>"#,
    ),
];

/// GPUI asset source serving `icons/<name>.svg`.
pub struct Assets;

impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> gpui::Result<Option<Cow<'static, [u8]>>> {
        let name = path
            .strip_prefix("icons/")
            .and_then(|rest| rest.strip_suffix(".svg"));
        let found = name.and_then(|wanted| {
            ALL.iter()
                .find(|(key, _)| *key == wanted)
                .map(|(_, data)| Cow::Borrowed(data.as_bytes()))
        });
        Ok(found)
    }

    fn list(&self, _path: &str) -> gpui::Result<Vec<SharedString>> {
        Ok(ALL
            .iter()
            .map(|(name, _)| SharedString::from(format!("icons/{name}.svg")))
            .collect())
    }
}

/// Renders a built-in vector icon at `size` pixels tinted with `colour`.
pub fn icon(name: &'static str, size: f32, colour: Hsla) -> Svg {
    svg()
        .path(format!("icons/{name}.svg"))
        .flex_none()
        .size(px(size))
        .text_color(colour)
}

#[cfg(test)]
mod tests {
    use super::{Assets, ALL};
    use gpui::AssetSource;

    #[test]
    fn every_builtin_icon_is_valid_svg_and_loadable() {
        let assets = Assets;
        assert_eq!(assets.list("icons").unwrap().len(), ALL.len());
        for (name, raw) in ALL {
            assert!(raw.starts_with("<svg ") && raw.ends_with("</svg>"), "{name}");
            let loaded = assets
                .load(&format!("icons/{name}.svg"))
                .unwrap()
                .expect(name);
            assert_eq!(loaded.as_ref(), raw.as_bytes());
        }
        assert!(assets.load("icons/missing.svg").unwrap().is_none());
    }
}
