#!/usr/bin/env python3
"""Writes src/qr.rs again: Microsoft's link page as a QR code, dark modules on white.

The page is a file with no server behind it and the code the player types changes every start, so
the only QR worth drawing is the address the code is typed at — and that is a constant. This
script renders it once, here, and the program carries the drawing.

    python3 icons/make_qr.py            # from nolauncher/, rewrites src/qr.rs
    python3 -c 'import segno' || pip install segno
"""

import io
import pathlib

import segno

#: What the page's QR opens.
LINK_PAGE = "https://www.microsoft.com/link"
#: Q survives a phone held at an angle to a screen; the address fits a 29 × 29 symbol at it.
ERROR = "q"
BORDER = 2

HEAD = '''//! The QR code on the sign-in page: Microsoft's own link page, dark modules on white.
//!
//! It is a constant, not a drawing: the page is a file in the player's browser with no server and
//! no library behind it, the code the player types changes every start, and the page the QR opens
//! is the same one every time. `icons/make_qr.py` prints this file again from `segno` (the module
//! matrix here is that generator's, checked by decoding it back to the address).

/// Microsoft's link page: where the sign-in's code is typed, for either kind of Microsoft app.
pub const LINK_PAGE: &str = "{link}";

/// [`LINK_PAGE`] as a {size} × {size} QR code, error correction {error}, with a two-module quiet zone.
const LINK: &str = r##"{svg}"##;

/// The QR code for `link`: it is there when the address is Microsoft's link page, and left out
/// for any other verification page (the page shows that address to type instead).
pub fn for_link(link: &str) -> Option<&'static str> {{
    if link.trim_end_matches('/').eq_ignore_ascii_case(LINK_PAGE) {{
        Some(LINK)
    }} else {{
        None
    }}
}}

#[cfg(test)]
mod tests {{
    use super::{{for_link, LINK, LINK_PAGE}};

    /// The link page has one, and it is a drawing of a QR code: an SVG with dark modules on a
    /// white square, so it scans off a dark page.
    #[test]
    fn the_link_page_has_a_qr_code_beside_it() {{
        let qr = for_link(LINK_PAGE).expect("the link page has a QR code");
        assert!(qr.starts_with("<svg") && qr.ends_with("</svg>"), "{{qr}}");
        assert!(qr.contains("fill=\\"#fff\\""), "no white background: {{qr}}");
        assert!(qr.contains("stroke=\\"#000"), "no dark modules: {{qr}}");
        assert!(qr.contains("width=\\"33\\" height=\\"33\\""), "{{qr}}");
    }}

    /// A page that is not the link page gets none — and neither does a look-alike address.
    #[test]
    fn only_microsofts_link_page_has_one() {{
        assert!(for_link("https://microsoft.com/devicelogin").is_none());
        assert!(for_link("https://www.microsoft.com/link.example.net").is_none());
        assert!(for_link("https://www.microsoft.com/link/").is_some());
        assert!(for_link("HTTPS://WWW.MICROSOFT.COM/LINK").is_some());
        assert_eq!(for_link(LINK_PAGE), Some(LINK));
    }}
}}
'''


def svg() -> tuple[str, int]:
    """The QR code as an SVG, and the width of the symbol with its quiet zone."""
    qr = segno.make(LINK_PAGE, error=ERROR)
    buffer = io.BytesIO()
    qr.save(
        buffer,
        kind="svg",
        scale=1,
        border=BORDER,
        dark="#000000",
        light="#ffffff",
        xmldecl=False,
        svgclass=None,
        lineclass=None,
        nl=False,
    )
    # The symbol without its quiet zone: the modules the drawing is made of.
    return buffer.getvalue().decode(), qr.symbol_size(border=0)[0]


def main() -> None:
    drawing, size = svg()
    out = pathlib.Path(__file__).resolve().parent.parent / "src" / "qr.rs"
    out.write_text(
        HEAD.format(link=LINK_PAGE, svg=drawing, size=size, error=ERROR.upper())
    )
    print(f"{out}: {size} × {size} modules, {len(drawing)} bytes of SVG")


if __name__ == "__main__":
    main()
