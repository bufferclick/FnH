#!/usr/bin/env python3
"""
Builds the universal FnH Dark-Glass Resource Pack (`fh/assets/FnH-Dark-Glass.zip`)
and shared high-res `[FnH] EDITION` banner (`fnh_edition.png`) using 100% pure
Python stdlib (`zlib` + `struct`) — zero third-party packages required.

Works across:
  - Minecraft: Java Edition 1.20.2 .. 1.21.11 (pack_format 15..46+)
  - Minecraft: Java Edition 26.1 .. 26.4+ (min_format / max_format)
  - Horizon 26.3 (Rust) (via `options.txt` resourcePacks + `edition.png` override)
"""

from __future__ import annotations

import json
import math
from pathlib import Path
import random
import struct
import zipfile
import zlib

ROOT = Path(__file__).resolve().parent.parent
OUT_ZIP = ROOT / "fh" / "assets" / "FnH-Dark-Glass.zip"
EDITION_121 = (
    ROOT / "mods" / "horizon" / "src" / "fnh-ui" / "resources" / "assets" / "fnh_ui" / "textures" / "gui" / "fnh_edition.png"
)
EDITION_26 = (
    ROOT / "mods" / "horizon" / "src" / "fnh-ui-26" / "resources" / "assets" / "fnh_ui" / "textures" / "gui" / "fnh_edition.png"
)

Color = tuple[int, int, int, int]


class Canvas:
    __slots__ = ("w", "h", "buf")

    def __init__(self, w: int, h: int, bg: Color = (0, 0, 0, 0)) -> None:
        self.w = w
        self.h = h
        self.buf = bytearray(bg) * (w * h)

    def set_px(self, x: int, y: int, c: Color) -> None:
        if 0 <= x < self.w and 0 <= y < self.h:
            i = (y * self.w + x) * 4
            self.buf[i] = c[0]
            self.buf[i + 1] = c[1]
            self.buf[i + 2] = c[2]
            self.buf[i + 3] = c[3]

    def get_px(self, x: int, y: int) -> Color:
        i = (y * self.w + x) * 4
        return (self.buf[i], self.buf[i + 1], self.buf[i + 2], self.buf[i + 3])

    def blend_px(self, x: int, y: int, c: Color) -> None:
        if 0 <= x < self.w and 0 <= y < self.h and c[3] > 0:
            i = (y * self.w + x) * 4
            sa = c[3] / 255.0
            da = self.buf[i + 3] / 255.0
            oa = sa + da * (1.0 - sa)
            if oa <= 1e-6:
                return
            self.buf[i] = int((c[0] * sa + self.buf[i] * da * (1.0 - sa)) / oa)
            self.buf[i + 1] = int((c[1] * sa + self.buf[i + 1] * da * (1.0 - sa)) / oa)
            self.buf[i + 2] = int((c[2] * sa + self.buf[i + 2] * da * (1.0 - sa)) / oa)
            self.buf[i + 3] = int(oa * 255.0)

    def fill_rect(self, x0: int, y0: int, x1: int, y1: int, c: Color) -> None:
        xa = max(0, min(x0, x1))
        xb = min(self.w - 1, max(x0, x1))
        ya = max(0, min(y0, y1))
        yb = min(self.h - 1, max(y0, y1))
        if xa > xb or ya > yb:
            return
        row_bytes = bytes(c) * (xb - xa + 1)
        span = len(row_bytes)
        w4 = self.w * 4
        xa4 = xa * 4
        for y in range(ya, yb + 1):
            start = y * w4 + xa4
            self.buf[start : start + span] = row_bytes

    def rounded_box(
        self,
        x0: int,
        y0: int,
        x1: int,
        y1: int,
        fill: Color | None,
        outline: Color | None,
        cut_corners: bool = True,
    ) -> None:
        if fill is not None:
            self.fill_rect(x0, y0, x1, y1, fill)
        if outline is not None:
            self.fill_rect(x0, y0, x1, y0, outline)
            self.fill_rect(x0, y1, x1, y1, outline)
            self.fill_rect(x0, y0, x0, y1, outline)
            self.fill_rect(x1, y0, x1, y1, outline)
        if cut_corners:
            for cx, cy in ((x0, y0), (x1, y0), (x0, y1), (x1, y1)):
                self.set_px(cx, cy, (0, 0, 0, 0))
            if outline is not None and (x1 - x0) >= 4 and (y1 - y0) >= 4:
                for cx, cy in ((x0 + 1, y0 + 1), (x1 - 1, y0 + 1), (x0 + 1, y1 - 1), (x1 - 1, y1 - 1)):
                    self.set_px(cx, cy, outline)

    def to_png(self) -> bytes:
        w, h = self.w, self.h
        stride = w * 4
        raw = bytearray((stride + 1) * h)
        for y in range(h):
            dst = y * (stride + 1)
            raw[dst] = 0
            src = y * stride
            raw[dst + 1 : dst + 1 + stride] = self.buf[src : src + stride]

        def chunk(tag: bytes, data: bytes) -> bytes:
            crc = zlib.crc32(tag + data) & 0xFFFFFFFF
            return struct.pack(">I", len(data)) + tag + data + struct.pack(">I", crc)

        ihdr = struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)
        idat = zlib.compress(bytes(raw), 9)
        return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) + chunk(b"IDAT", idat) + chunk(b"IEND", b"")


GLYPHS: dict[str, list[str]] = {
    "[": ["11110", "11000", "11000", "11000", "11000", "11000", "11110"],
    "]": ["01111", "00011", "00011", "00011", "00011", "00011", "01111"],
    "F": ["11111", "11000", "11000", "11110", "11000", "11000", "11000"],
    "n": ["00000", "11110", "11011", "11011", "11011", "11011", "11011"],
    "H": ["11011", "11011", "11011", "11111", "11011", "11011", "11011"],
    "E": ["11111", "11000", "11000", "11110", "11000", "11000", "11111"],
    "D": ["11110", "11011", "11011", "11011", "11011", "11011", "11110"],
    "I": ["11111", "00100", "00100", "00100", "00100", "00100", "11111"],
    "T": ["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
    "O": ["01110", "11011", "11011", "11011", "11011", "11011", "01110"],
    "N": ["11001", "11101", "11111", "11011", "11011", "11011", "11001"],
    " ": ["000", "000", "000", "000", "000", "000", "000"],
}


def build_edition_canvas() -> Canvas:
    """512x64 metallic `[FnH] EDITION` inside y=5..52 (fits both 14/16 UV crop & Horizon 512x64)."""
    w, h = 512, 64
    c = Canvas(w, h)
    text = "[FnH] EDITION"
    scale = 5
    gap = 5
    total_w = sum(len(GLYPHS[ch][0]) * scale for ch in text) + gap * (len(text) - 1)
    start_x = (w - total_w) // 2
    top_y = 6

    face = [[False] * w for _ in range(h)]
    cx = start_x
    for ch in text:
        rows = GLYPHS[ch]
        cw = len(rows[0])
        for ry, row in enumerate(rows):
            for rx, bit in enumerate(row):
                if bit == "1":
                    for py in range(top_y + ry * scale, top_y + (ry + 1) * scale):
                        for px in range(cx + rx * scale, cx + (rx + 1) * scale):
                            face[py][px] = True
        cx += cw * scale + gap

    ext = [[False] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            if not face[y][x]:
                for dy in range(1, 7):
                    if y - dy >= 0 and face[y - dy][x]:
                        ext[y][x] = True
                        break

    out = [[False] * w for _ in range(h)]
    for y in range(h):
        for x in range(w):
            if face[y][x] or ext[y][x]:
                continue
            for oy in range(-2, 3):
                ny = y + oy
                if 0 <= ny < h:
                    for ox in range(-2, 3):
                        nx = x + ox
                        if 0 <= nx < w and (face[ny][nx] or ext[ny][nx]):
                            out[y][x] = True
                            break
                if out[y][x]:
                    break

    fnh_end_x = start_x + sum(len(GLYPHS[ch][0]) * scale for ch in "[FnH]") + gap * 4
    for y in range(h):
        for x in range(w):
            is_brand = x <= fnh_end_x + 2
            if out[y][x]:
                c.set_px(x, y, (14, 16, 24, 246))
            elif ext[y][x]:
                c.set_px(x, y, (56, 70, 98, 255) if is_brand else (76, 80, 92, 255))
            elif face[y][x]:
                t = max(0.0, min(1.0, (y - top_y) / float(7 * scale)))
                top_edge = y == 0 or not face[y - 1][x]
                bot_edge = y + 1 >= h or not face[y + 1][x]
                if is_brand:
                    r, g, b = int(244 - 26 * t), int(248 - 20 * t), int(255 - 8 * t)
                    if top_edge:
                        r, g, b = 255, 255, 255
                    elif bot_edge:
                        r, g, b = 196, 214, 245
                else:
                    r, g, b = int(218 - 34 * t), int(222 - 32 * t), int(232 - 28 * t)
                    if top_edge:
                        r, g, b = 244, 246, 252
                    elif bot_edge:
                        r, g, b = 166, 172, 186
                c.set_px(x, y, (r, g, b, 255))
    return c


def make_nine_slice_mcmeta(w: int, h: int, border: int = 3) -> bytes:
    return json.dumps(
        {"gui": {"scaling": {"type": "nine_slice", "width": w, "height": h, "border": border}}},
        indent=2,
    ).encode("utf-8")


def build_glass_widget(
    w: int,
    h: int,
    fill_top: Color,
    fill_bot: Color,
    border_outer: Color,
    border_top_hi: Color,
) -> bytes:
    c = Canvas(w, h)
    for y in range(h):
        t = y / max(1, h - 1)
        col = (
            int(fill_top[0] + (fill_bot[0] - fill_top[0]) * t),
            int(fill_top[1] + (fill_bot[1] - fill_top[1]) * t),
            int(fill_top[2] + (fill_bot[2] - fill_top[2]) * t),
            int(fill_top[3] + (fill_bot[3] - fill_top[3]) * t),
        )
        c.fill_rect(0, y, w - 1, y, col)
    c.rounded_box(0, 0, w - 1, h - 1, None, border_outer, cut_corners=True)
    if w > 4:
        c.fill_rect(2, 1, w - 3, 1, border_top_hi)
    return c.to_png()


def build_hotbar() -> bytes:
    w, h = 182, 22
    c = Canvas(w, h)
    # Clean, minimal translucent smoked-obsidian pill (no cluttered inner slot boxes)
    c.rounded_box(0, 0, w - 1, h - 1, (10, 12, 18, 152), (255, 255, 255, 38), cut_corners=True)
    c.fill_rect(2, 1, w - 3, 1, (255, 255, 255, 16))
    # 8 subtle 1px vertical hairlines between the 9 slots
    for div_idx in range(1, 9):
        dx = 1 + div_idx * 20
        c.fill_rect(dx, 5, dx, h - 6, (255, 255, 255, 18))
    return c.to_png()


def build_hotbar_selection() -> bytes:
    w, h = 24, 23
    c = Canvas(w, h)
    # Crisp, minimal 1px pure-white slot frame flush with the 20x20 slot + soft glass sheen
    c.rounded_box(2, 1, 21, 20, (255, 255, 255, 18), (255, 255, 255, 226), cut_corners=True)
    return c.to_png()


def build_offhand(left: bool) -> bytes:
    c = Canvas(29, 24)
    ox = 0 if left else 7
    c.rounded_box(ox, 1, ox + 21, 22, (10, 12, 18, 152), (255, 255, 255, 38), cut_corners=True)
    c.fill_rect(ox + 2, 2, ox + 19, 2, (255, 255, 255, 16))
    return c.to_png()


def build_xp_bar(progress: bool) -> bytes:
    w, h = 182, 5
    c = Canvas(w, h)
    if not progress:
        c.rounded_box(0, 0, w - 1, h - 1, (10, 12, 18, 204), (48, 56, 76, 218), cut_corners=True)
    else:
        for x in range(w):
            u = x / max(1, w - 1)
            r = int(84 + 48 * u)
            g = int(228 + 18 * u)
            b = int(154 + 78 * u)
            c.set_px(x, 0, (min(255, r + 40), 255, min(255, b + 30), 250))
            for y in (1, 2, 3):
                c.set_px(x, y, (r, g, b, 250))
            c.set_px(x, 4, (max(0, r - 35), max(0, g - 50), max(0, b - 35), 245))
        for cx, cy in ((0, 0), (w - 1, 0), (0, h - 1), (w - 1, h - 1)):
            c.set_px(cx, cy, (0, 0, 0, 0))
    return c.to_png()


def draw_panel(c: Canvas, x0: int, y0: int, w: int, h: int) -> None:
    x1, y1 = x0 + w - 1, y0 + h - 1
    c.rounded_box(x0, y0, x1, y1, (20, 24, 34, 238), (84, 96, 128, 246), cut_corners=True)
    c.rounded_box(x0 + 1, y0 + 1, x1 - 1, y1 - 1, None, (156, 174, 218, 58), cut_corners=False)


def draw_header_pill(c: Canvas, x0: int, y0: int, x1: int, y1: int) -> None:
    # Subtle smoked-slate header sheen (no bright box — lang `en_us.json` + Fabric mixin render crisp white labels)
    c.fill_rect(x0, y0, x1, y1, (28, 33, 46, 238))
    c.fill_rect(x0, y1, x1, y1, (48, 56, 76, 180))


def draw_slot(c: Canvas, sx: int, sy: int, size: int = 18) -> None:
    ex, ey = sx + size - 1, sy + size - 1
    c.fill_rect(sx, sy, ex, ey, (10, 12, 19, 236))
    c.fill_rect(sx, sy, ex, sy, (4, 5, 9, 246))
    c.fill_rect(sx, sy, sx, ey, (4, 5, 9, 246))
    c.fill_rect(sx, ey, ex, ey, (54, 62, 86, 208))
    c.fill_rect(ex, sy, ex, ey, (54, 62, 86, 208))


def draw_player_inv(c: Canvas, inv_y: int) -> None:
    for r in range(3):
        for col in range(9):
            draw_slot(c, 7 + col * 18, inv_y + r * 18)
    for col in range(9):
        draw_slot(c, 7 + col * 18, inv_y + 58)


def draw_arrow(c: Canvas, x: int, y: int) -> None:
    col = (72, 82, 110, 216)
    c.fill_rect(x, y + 6, x + 14, y + 9, col)
    for i in range(7):
        c.fill_rect(x + 15 + i, y + 1 + i, x + 15 + i, y + 14 - i, col)


def build_generic_54() -> bytes:
    c = Canvas(256, 256)
    c.fill_rect(0, 0, 175, 124, (20, 24, 34, 238))
    c.fill_rect(0, 0, 175, 0, (96, 110, 144, 246))
    c.fill_rect(1, 1, 174, 1, (162, 180, 222, 62))
    c.fill_rect(0, 0, 0, 124, (84, 96, 128, 246))
    c.fill_rect(175, 0, 175, 124, (84, 96, 128, 246))
    c.set_px(0, 0, (0, 0, 0, 0))
    c.set_px(175, 0, (0, 0, 0, 0))
    draw_header_pill(c, 6, 4, 169, 15)
    for r in range(6):
        for col in range(9):
            draw_slot(c, 7 + col * 18, 17 + r * 18)

    c.fill_rect(0, 126, 175, 221, (20, 24, 34, 238))
    c.fill_rect(0, 126, 0, 221, (84, 96, 128, 246))
    c.fill_rect(175, 126, 175, 221, (84, 96, 128, 246))
    c.fill_rect(0, 221, 175, 221, (84, 96, 128, 246))
    c.set_px(0, 221, (0, 0, 0, 0))
    c.set_px(175, 221, (0, 0, 0, 0))
    draw_header_pill(c, 6, 127, 76, 137)
    draw_player_inv(c, 139)
    return c.to_png()


def build_inventory() -> bytes:
    c = Canvas(256, 256)
    draw_panel(c, 0, 0, 176, 166)
    for i in range(4):
        draw_slot(c, 7, 7 + i * 18)
    c.rounded_box(25, 7, 76, 77, (7, 9, 14, 248), (56, 66, 92, 224), cut_corners=True)
    draw_slot(c, 76, 61)
    draw_header_pill(c, 96, 4, 154, 15)
    for r in range(2):
        for col in range(2):
            draw_slot(c, 97 + col * 18, 17 + r * 18)
    draw_arrow(c, 134, 28)
    draw_slot(c, 153, 27)
    draw_player_inv(c, 83)
    return c.to_png()


def build_crafting_table() -> bytes:
    c = Canvas(256, 256)
    draw_panel(c, 0, 0, 176, 166)
    draw_header_pill(c, 27, 4, 98, 15)
    draw_header_pill(c, 6, 70, 76, 81)
    for r in range(3):
        for col in range(3):
            draw_slot(c, 29 + col * 18, 16 + r * 18)
    draw_arrow(c, 89, 34)
    draw_slot(c, 119, 30, size=26)
    draw_player_inv(c, 83)
    return c.to_png()


def build_shulker_box() -> bytes:
    c = Canvas(256, 256)
    draw_panel(c, 0, 0, 176, 167)
    draw_header_pill(c, 6, 4, 169, 15)
    draw_header_pill(c, 6, 71, 76, 82)
    for r in range(3):
        for col in range(9):
            draw_slot(c, 7 + col * 18, 17 + r * 18)
    draw_player_inv(c, 83)
    return c.to_png()


def build_hopper() -> bytes:
    c = Canvas(256, 256)
    draw_panel(c, 0, 0, 176, 133)
    draw_header_pill(c, 6, 4, 120, 15)
    draw_header_pill(c, 6, 38, 76, 49)
    for col in range(5):
        draw_slot(c, 43 + col * 18, 19)
    draw_player_inv(c, 50)
    return c.to_png()


def build_dispenser() -> bytes:
    c = Canvas(256, 256)
    draw_panel(c, 0, 0, 176, 166)
    draw_header_pill(c, 6, 4, 169, 15)
    draw_header_pill(c, 6, 70, 76, 81)
    for r in range(3):
        for col in range(3):
            draw_slot(c, 61 + col * 18, 16 + r * 18)
    draw_player_inv(c, 83)
    return c.to_png()


def build_furnace() -> bytes:
    c = Canvas(256, 256)
    draw_panel(c, 0, 0, 176, 166)
    draw_header_pill(c, 6, 4, 169, 15)
    draw_header_pill(c, 6, 70, 76, 81)
    draw_slot(c, 55, 16)
    draw_slot(c, 55, 52)
    for fy in range(13):
        half = max(1, (fy * 5) // 12)
        c.fill_rect(63 - half, 36 + fy, 63 + half, 36 + fy, (56, 64, 86, 212))
    draw_arrow(c, 79, 34)
    draw_slot(c, 111, 30, size=26)
    draw_player_inv(c, 83)
    return c.to_png()


def build_panorama_faces() -> list[bytes]:
    rng = random.Random(20261117)
    palette = [
        (255, 255, 255),
        (210, 226, 255),
        (244, 218, 255),
        (190, 212, 255),
        (255, 236, 214),
    ]
    stars: list[tuple[float, float, float, float, tuple[int, int, int]]] = []
    for _ in range(900):
        u = rng.uniform(-1.0, 1.0)
        theta = rng.uniform(0.0, math.tau)
        s = math.sqrt(max(0.0, 1.0 - u * u))
        dx, dy, dz = s * math.cos(theta), u, s * math.sin(theta)
        if dy >= -0.16:
            stars.append((dx, dy, dz, rng.uniform(0.45, 1.0), rng.choice(palette)))

    size = 192
    faces: list[bytes] = []
    for face_idx in range(6):
        c = Canvas(size, size, (8, 10, 20, 255))
        for py in range(size):
            v = 1.0 - 2.0 * ((py + 0.5) / size)
            for px_i in range(size):
                u = 2.0 * ((px_i + 0.5) / size) - 1.0
                if face_idx == 0:
                    rx, ry, rz = u, v, 1.0
                elif face_idx == 1:
                    rx, ry, rz = -1.0, v, u
                elif face_idx == 2:
                    rx, ry, rz = -u, v, -1.0
                elif face_idx == 3:
                    rx, ry, rz = 1.0, v, -u
                elif face_idx == 4:
                    rx, ry, rz = u, 1.0, -v
                else:
                    rx, ry, rz = u, -1.0, v
                inv = 1.0 / math.sqrt(rx * rx + ry * ry + rz * rz)
                dx, dy, dz = rx * inv, ry * inv, rz * inv

                az = math.atan2(dz, dx)
                ridge = (
                    -0.04
                    + 0.055 * math.sin(az * 3.0 + 0.6)
                    + 0.032 * math.sin(az * 7.0 + 1.9)
                    + 0.016 * math.sin(az * 15.0 + 0.3)
                )
                if dy < ridge:
                    c.set_px(px_i, py, (7, 9, 15, 255))
                else:
                    horizon = math.exp(-max(0.0, dy + 0.08) * 4.2)
                    band_plane = abs(0.56 * dx - 0.62 * dy + 0.55 * dz)
                    nebula = math.exp(-(band_plane * band_plane) / 0.055) * max(0.0, dy + 0.12)
                    r = min(255, int(10 + 36 * horizon + 74 * nebula))
                    g = min(255, int(14 + 32 * horizon + 48 * nebula))
                    b = min(255, int(30 + 62 * horizon + 104 * nebula))
                    c.set_px(px_i, py, (r, g, b, 255))

        for sx, sy, sz, br, col in stars:
            if face_idx == 0 and sz > 0.0:
                u, v = sx / sz, sy / sz
            elif face_idx == 1 and sx < 0.0:
                u, v = sz / (-sx), sy / (-sx)
            elif face_idx == 2 and sz < 0.0:
                u, v = (-sx) / (-sz), sy / (-sz)
            elif face_idx == 3 and sx > 0.0:
                u, v = (-sz) / sx, sy / sx
            elif face_idx == 4 and sy > 0.0:
                u, v = sx / sy, (-sz) / sy
            elif face_idx == 5 and sy < 0.0:
                u, v = sx / (-sy), sz / (-sy)
            else:
                continue
            if -1.0 <= u <= 1.0 and -1.0 <= v <= 1.0:
                az = math.atan2(sz, sx)
                ridge = (
                    -0.04
                    + 0.055 * math.sin(az * 3.0 + 0.6)
                    + 0.032 * math.sin(az * 7.0 + 1.9)
                    + 0.016 * math.sin(az * 15.0 + 0.3)
                )
                if sy <= ridge + 0.005:
                    continue
                ix = int((u + 1.0) * 0.5 * (size - 1))
                iy = int((1.0 - v) * 0.5 * (size - 1))
                cur = c.get_px(ix, iy)
                c.set_px(
                    ix,
                    iy,
                    (
                        min(255, int(cur[0] + col[0] * br)),
                        min(255, int(cur[1] + col[1] * br)),
                        min(255, int(cur[2] + col[2] * br)),
                        255,
                    ),
                )
                if br > 0.78:
                    for ox, oy in ((-1, 0), (1, 0), (0, -1), (0, 1)):
                        c.blend_px(ix + ox, iy + oy, (col[0], col[1], col[2], 95))
        faces.append(c.to_png())
    return faces


def build_pack_icon(ed: Canvas) -> bytes:
    c = Canvas(128, 128, (10, 12, 20, 255))
    c.rounded_box(4, 4, 123, 123, (18, 22, 36, 255), (150, 174, 232, 240), cut_corners=True)
    c.rounded_box(6, 6, 121, 121, None, (86, 102, 142, 180), cut_corners=False)
    # Blit `[FnH]` crop from edition banner (x=66..242, y=4..54) scaled into 104x32
    for dy in range(32):
        sy = 4 + (dy * 48) // 32
        for dx in range(104):
            sx = 66 + (dx * 176) // 104
            px = ed.get_px(sx, sy)
            if px[3] > 0:
                c.blend_px(12 + dx, 48 + dy, px)
    return c.to_png()


def main() -> None:
    ed_canvas = build_edition_canvas()
    edition_bytes = ed_canvas.to_png()

    for target in (EDITION_121, EDITION_26):
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(edition_bytes)

    OUT_ZIP.parent.mkdir(parents=True, exist_ok=True)

    btn_normal = build_glass_widget(
        200, 20, (18, 22, 34, 214), (11, 14, 22, 228), (78, 90, 120, 212), (165, 184, 228, 72)
    )
    btn_hover = build_glass_widget(
        200, 20, (34, 42, 64, 236), (22, 28, 44, 244), (208, 222, 252, 250), (245, 250, 255, 148)
    )
    btn_disabled = build_glass_widget(
        200, 20, (11, 13, 20, 144), (9, 11, 17, 154), (46, 52, 68, 128), (90, 100, 124, 32)
    )
    slider_bg = build_glass_widget(
        200, 20, (10, 12, 19, 220), (14, 17, 26, 226), (66, 76, 102, 205), (40, 46, 62, 90)
    )
    slider_handle = build_glass_widget(
        8, 20, (186, 202, 238, 242), (132, 150, 192, 246), (232, 240, 255, 252), (255, 255, 255, 210)
    )
    slider_handle_hi = build_glass_widget(
        8, 20, (230, 238, 255, 252), (176, 196, 238, 255), (255, 255, 255, 255), (255, 255, 255, 245)
    )
    text_field = build_glass_widget(
        200, 20, (8, 10, 16, 232), (11, 14, 22, 236), (74, 86, 114, 215), (30, 36, 50, 95)
    )
    text_field_hi = build_glass_widget(
        200, 20, (12, 15, 24, 242), (16, 20, 32, 245), (196, 214, 252, 248), (120, 142, 188, 95)
    )
    tab_idle = build_glass_widget(
        130, 24, (14, 18, 28, 198), (10, 13, 20, 214), (64, 74, 98, 190), (140, 158, 198, 55)
    )
    tab_sel = build_glass_widget(
        130, 24, (32, 40, 62, 238), (20, 25, 40, 244), (212, 226, 255, 250), (248, 252, 255, 165)
    )

    mcmeta_pack = {
        "pack": {
            "pack_format": 46,
            "supported_formats": [15, 999],
            "min_format": 15,
            "max_format": 999,
            "description": "FnH Dark-Glass UI — Java Edition & Horizon (made by Buffer)",
        }
    }

    transparent_16 = Canvas(16, 16).to_png()
    furnace_bytes = build_furnace()
    pano_faces = build_panorama_faces()

    with zipfile.ZipFile(OUT_ZIP, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as zf:
        zf.writestr("pack.mcmeta", json.dumps(mcmeta_pack, indent=2))
        zf.writestr("pack.png", build_pack_icon(ed_canvas))
        zf.writestr("assets/minecraft/textures/gui/title/edition.png", edition_bytes)
        zf.writestr("assets/minecraft/texts/splashes.txt", "made by Buffer\n")
        zf.writestr(
            "assets/minecraft/lang/en_us.json",
            json.dumps(
                {
                    "container.inventory": "\u00a7fInventory",
                    "container.crafting": "\u00a7fCrafting",
                    "container.chest": "\u00a7fChest",
                    "container.chestDouble": "\u00a7fLarge Chest",
                    "container.enderchest": "\u00a7fEnder Chest",
                    "container.barrel": "\u00a7fBarrel",
                    "container.furnace": "\u00a7fFurnace",
                    "container.smoker": "\u00a7fSmoker",
                    "container.blast_furnace": "\u00a7fBlast Furnace",
                    "container.shulkerBox": "\u00a7fShulker Box",
                    "container.hopper": "\u00a7fItem Hopper",
                    "container.dropper": "\u00a7fDropper",
                    "container.dispenser": "\u00a7fDispenser",
                },
                indent=2,
            ),
        )
        for i, face in enumerate(pano_faces):
            zf.writestr(f"assets/minecraft/textures/gui/title/background/panorama_{i}.png", face)
        zf.writestr("assets/minecraft/textures/gui/title/background/panorama_overlay.png", transparent_16)

        mcmeta_200_20 = make_nine_slice_mcmeta(200, 20, 3)
        mcmeta_8_20 = make_nine_slice_mcmeta(8, 20, 2)
        mcmeta_130_24 = make_nine_slice_mcmeta(130, 24, 3)
        for rel, png, meta in [
            ("widget/button", btn_normal, mcmeta_200_20),
            ("widget/button_highlighted", btn_hover, mcmeta_200_20),
            ("widget/button_disabled", btn_disabled, mcmeta_200_20),
            ("widget/slider", slider_bg, mcmeta_200_20),
            ("widget/slider_highlighted", btn_hover, mcmeta_200_20),
            ("widget/slider_handle", slider_handle, mcmeta_8_20),
            ("widget/slider_handle_highlighted", slider_handle_hi, mcmeta_8_20),
            ("widget/text_field", text_field, mcmeta_200_20),
            ("widget/text_field_highlighted", text_field_hi, mcmeta_200_20),
            ("tab/tab", tab_idle, mcmeta_130_24),
            ("tab/tab_highlighted", btn_hover, mcmeta_130_24),
            ("tab/tab_selected", tab_sel, mcmeta_130_24),
            ("tab/tab_selected_highlighted", tab_sel, mcmeta_130_24),
        ]:
            zf.writestr(f"assets/minecraft/textures/gui/sprites/{rel}.png", png)
            zf.writestr(f"assets/minecraft/textures/gui/sprites/{rel}.png.mcmeta", meta)

        zf.writestr("assets/minecraft/textures/gui/sprites/hud/hotbar.png", build_hotbar())
        zf.writestr("assets/minecraft/textures/gui/sprites/hud/hotbar_selection.png", build_hotbar_selection())
        zf.writestr("assets/minecraft/textures/gui/sprites/hud/hotbar_offhand_left.png", build_offhand(True))
        zf.writestr("assets/minecraft/textures/gui/sprites/hud/hotbar_offhand_right.png", build_offhand(False))
        zf.writestr("assets/minecraft/textures/gui/sprites/hud/experience_bar_background.png", build_xp_bar(False))
        zf.writestr("assets/minecraft/textures/gui/sprites/hud/experience_bar_progress.png", build_xp_bar(True))

        zf.writestr("assets/minecraft/textures/gui/container/generic_54.png", build_generic_54())
        zf.writestr("assets/minecraft/textures/gui/container/inventory.png", build_inventory())
        zf.writestr("assets/minecraft/textures/gui/container/crafting_table.png", build_crafting_table())
        zf.writestr("assets/minecraft/textures/gui/container/shulker_box.png", build_shulker_box())
        zf.writestr("assets/minecraft/textures/gui/container/hopper.png", build_hopper())
        zf.writestr("assets/minecraft/textures/gui/container/dispenser.png", build_dispenser())
        zf.writestr("assets/minecraft/textures/gui/container/furnace.png", furnace_bytes)
        zf.writestr("assets/minecraft/textures/gui/container/blast_furnace.png", furnace_bytes)
        zf.writestr("assets/minecraft/textures/gui/container/smoker.png", furnace_bytes)

        # GPU temporal frame-blend Motion Blur post-effect shaders (1.21.5+ / 1.21.11 / 26.x)
        fsh = """#version 330

uniform sampler2D InSampler;
uniform sampler2D PrevSampler;

in vec2 texCoord;

layout(std140) uniform MotionBlurConfig {
    float BlendFactor;
};

out vec4 fragColor;

void main() {
    vec4 curr = texture(InSampler, texCoord);
    vec4 prev = texture(PrevSampler, texCoord);
    vec3 mixed = prev.a < 0.01 ? curr.rgb : mix(curr.rgb, prev.rgb, BlendFactor);
    fragColor = vec4(mixed, 1.0);
}
"""
        zf.writestr("assets/minecraft/shaders/post/fnh_motion_blur.fsh", fsh)
        for preset_name, factor in (("low", 0.42), ("med", 0.60), ("high", 0.74)):
            post_cfg = {
                "targets": {
                    "prev": {"persistent": True},
                    "swap": {},
                },
                "passes": [
                    {
                        "vertex_shader": "minecraft:core/screenquad",
                        "fragment_shader": "minecraft:post/fnh_motion_blur",
                        "inputs": [
                            {"sampler_name": "In", "target": "minecraft:main"},
                            {"sampler_name": "Prev", "target": "prev"},
                        ],
                        "output": "swap",
                        "uniforms": {
                            "MotionBlurConfig": [
                                {"name": "BlendFactor", "type": "float", "value": factor}
                            ]
                        },
                    },
                    {
                        "vertex_shader": "minecraft:core/screenquad",
                        "fragment_shader": "minecraft:post/blit",
                        "inputs": [{"sampler_name": "In", "target": "swap"}],
                        "uniforms": {
                            "BlitConfig": [
                                {"name": "ColorModulate", "type": "vec4", "value": [1.0, 1.0, 1.0, 1.0]}
                            ]
                        },
                        "output": "prev",
                    },
                    {
                        "vertex_shader": "minecraft:core/screenquad",
                        "fragment_shader": "minecraft:post/blit",
                        "inputs": [{"sampler_name": "In", "target": "swap"}],
                        "uniforms": {
                            "BlitConfig": [
                                {"name": "ColorModulate", "type": "vec4", "value": [1.0, 1.0, 1.0, 1.0]}
                            ]
                        },
                        "output": "minecraft:main",
                    },
                ],
            }
            zf.writestr(
                f"assets/minecraft/post_effect/fnh_motion_blur_{preset_name}.json",
                json.dumps(post_cfg, indent=2),
            )

    print(f"Built {OUT_ZIP} ({OUT_ZIP.stat().st_size / 1024:.1f} KB)")

    # Also build native Horizon Shader API 0.2 per-pixel velocity Motion Blur pack (`FnH-Motion-Blur.hshader`)
    out_hshader = ROOT / "fh" / "assets" / "FnH-Motion-Blur.hshader"
    hshader_toml = """schema = 1
id = "fnh_motion_blur"
name = "FnH Motion Blur"
version = "1.9.1"
api = "0.2"
description = "Clean per-pixel camera velocity motion blur for Horizon. Made by Buffer."
authors = ["Buffer"]
license = "MIT"

[[post]]
file = "motion_blur.wgsl"
name = "Velocity motion blur"

[[options]]
id = "strength"
name = "Motion Blur Strength"
description = "Camera motion blur shutter strength."
type = "float"
default = 0.70
min = 0.0
max = 1.5
step = 0.05

[budget]
gpu_ms = 1.0
"""
    hshader_wgsl = """override strength: f32 = 0.70;

fn post(input: PostInput) -> vec4<f32> {
    if strength <= 0.01 || frame.delta_time <= 0.0 {
        return input.color;
    }
    let d = max(input.depth, 0.0002);
    let p_cam = view_position(input.uv, d);
    let prev_world = select(p_cam + frame.camera_motion, p_cam, input.depth == 0.0);
    let prev_clip = frame.previous_view_projection * vec4<f32>(prev_world, 1.0);
    if prev_clip.w <= 0.0001 {
        return input.color;
    }
    let ndc = prev_clip.xy / prev_clip.w;
    let prev_uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    let raw_vel = (prev_uv - input.uv) * (strength * 0.62);
    let vel = clamp(raw_vel, vec2<f32>(-0.038), vec2<f32>(0.038));
    if dot(vel, vel) < 0.00000025 {
        return input.color;
    }
    var acc = input.color.rgb * 1.5;
    var weight_sum = 1.5;
    for (var i = 1; i <= 6; i++) {
        let t = (f32(i) / 6.0) - 0.5;
        let sample_uv = clamp(input.uv + vel * t, vec2<f32>(0.001), vec2<f32>(0.999));
        acc += textureSampleLevel(previous, linear_sampler, sample_uv, 0.0).rgb;
        weight_sum += 1.0;
    }
    return vec4<f32>(acc / weight_sum, 1.0);
}
"""
    with zipfile.ZipFile(out_hshader, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as hzf:
        hzf.writestr("horizon-shaders.toml", hshader_toml)
        hzf.writestr("motion_blur.wgsl", hshader_wgsl)
    print(f"Built {out_hshader} ({out_hshader.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
