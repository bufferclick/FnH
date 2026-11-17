#!/usr/bin/env python3
"""Draws Fabric n Horizon's icon (fh/icons/fh.svg) and writes fh.ico.

No image library is involved: the rounded porcelain tile and its hairline ring are rasterised with
4x4 supersampling, while the 9x9 FnH pixel monogram (deep-navy F and H with a black lowercase n
tucked under the F's arm) is snapped to an integer cell size at every icon resolution so stems and
counters stay razor-sharp down to 16x16. Frames are written as the .ico container Windows reads:
32-bit BMP frames up to 64 pixels, PNG frames above (what Explorer and the task bar take for the
large sizes).

    python3 fh/icons/make_icon.py            # writes fh/icons/fh.ico
    python3 fh/icons/make_icon.py --png 256  # also writes fh-256.png, to look at it
"""

import argparse
import pathlib
import struct
import zlib

# The 9x9 FnH monogram: `F` is painted in NAVY (#131254, used for both F and H), `n` in INK (#000000).
GRID = (
    "FFFFF.F.F",
    "F.....F.F",
    "F.....F.F",
    "FFF...F.F",
    "F.....FFF",
    "F.nnn.F.F",
    "F.n.n.F.F",
    "F.n.n.F.F",
    "F.n.n.F.F",
)

NAVY = (0x13, 0x12, 0x54)
INK = (0x00, 0x00, 0x00)
TILE_TOP = (0xFF, 0xFF, 0xFF)
TILE_BOTTOM = (0xF6, 0xF7, 0xFB)
TILE_RING = (0xD8, 0xDA, 0xE5)

RADIUS_RATIO = 14.0 / 64.0
RING_RATIO = 0.85 / 64.0

# Integer pixel cell per icon size so every bar and gap of the 9x9 mark stays exact.
CELL_SIZES = {
    16: 1,
    20: 1,
    24: 2,
    32: 2,
    40: 3,
    48: 3,
    64: 4,
    128: 9,
    256: 18,
}

SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
SUPERSAMPLE = 4


def cell_for(size):
    """The integer pixel side of one cell in the 9x9 mark for a frame of `size`."""
    return CELL_SIZES.get(size, max(1, round(size * (18.0 / 256.0))))


def tile_distance(x, y, size, radius):
    """Signed distance to the rounded square [0, size]^2 (negative inside, positive outside)."""
    cx = min(max(x, radius), size - radius)
    cy = min(max(y, radius), size - radius)
    dx = x - cx
    dy = y - cy
    if dx == 0.0 and dy == 0.0:
        return -min(x, size - x, y, size - y)
    return (dx * dx + dy * dy) ** 0.5 - radius


def plate_colour(t):
    """Vertical porcelain tint from TILE_TOP (t=0) to TILE_BOTTOM (t=1)."""
    return (
        TILE_TOP[0] + (TILE_BOTTOM[0] - TILE_TOP[0]) * t,
        TILE_TOP[1] + (TILE_BOTTOM[1] - TILE_TOP[1]) * t,
        TILE_TOP[2] + (TILE_BOTTOM[2] - TILE_TOP[2]) * t,
    )


def frame(size):
    """The RGBA pixels of `size` square, top row first."""
    radius = size * RADIUS_RATIO
    ring = max(0.9, size * RING_RATIO)
    cell = cell_for(size)
    span = cell * 9
    origin = (size - span) // 2
    samples = SUPERSAMPLE * SUPERSAMPLE

    rows = []
    for py in range(size):
        mark_row = GRID[(py - origin) // cell] if origin <= py < origin + span else None
        row = []
        for px in range(size):
            if mark_row is not None and origin <= px < origin + span:
                glyph = mark_row[(px - origin) // cell]
                if glyph == "F":
                    row.append((*NAVY, 255))
                    continue
                if glyph == "n":
                    row.append((*INK, 255))
                    continue

            red = green = blue = alpha = 0.0
            for sy in range(SUPERSAMPLE):
                y = py + (sy + 0.5) / SUPERSAMPLE
                fill = plate_colour(y / size)
                for sx in range(SUPERSAMPLE):
                    x = px + (sx + 0.5) / SUPERSAMPLE
                    dist = tile_distance(x, y, size, radius)
                    if dist > 0.0:
                        continue
                    colour = TILE_RING if dist > -ring else fill
                    red += colour[0]
                    green += colour[1]
                    blue += colour[2]
                    alpha += 1.0
            if alpha == 0.0:
                row.append((0, 0, 0, 0))
            else:
                row.append(
                    (
                        round(red / alpha),
                        round(green / alpha),
                        round(blue / alpha),
                        round(255 * alpha / samples),
                    )
                )
        rows.append(row)
    return rows


def bmp_frame(pixels):
    """A 32-bit BMP frame: the size and the doubled height, rows bottom-up, then the AND mask."""
    height = len(pixels)
    width = len(pixels[0])
    header = struct.pack(
        "<IiiHHIIiiII",
        40,
        width,
        height * 2,
        1,
        32,
        0,
        width * height * 4,
        0,
        0,
        0,
        0,
    )
    body = bytearray()
    for row in reversed(pixels):
        for (r, g, b, a) in row:
            body += bytes((b, g, r, a))
    # The AND mask: every pixel the alpha already makes transparent, one bit each, padded to 32 bits.
    stride = ((width + 31) // 32) * 4
    mask = bytearray()
    for row in reversed(pixels):
        bits = bytearray(stride)
        for x, (_, _, _, a) in enumerate(row):
            if a == 0:
                bits[x // 8] |= 1 << (7 - x % 8)
        mask += bits
    return header + bytes(body) + bytes(mask)


def png_frame(pixels):
    """The same frame as a PNG, which is what the large sizes hold."""
    height = len(pixels)
    width = len(pixels[0])

    def chunk(kind, data):
        return (
            struct.pack(">I", len(data))
            + kind
            + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    raw = bytearray()
    for row in pixels:
        raw.append(0)
        for pixel in row:
            raw += bytes(pixel)
    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )


def ico(frames):
    """The .ico container: a header, one entry per frame, then the frames' bytes."""
    header = struct.pack("<HHH", 0, 1, len(frames))
    offset = len(header) + 16 * len(frames)
    entries = bytearray()
    body = bytearray()
    for size, data in frames:
        entries += struct.pack(
            "<BBBBHHII",
            0 if size >= 256 else size,
            0 if size >= 256 else size,
            0,
            0,
            1,
            32,
            len(data),
            offset,
        )
        body += data
        offset += len(data)
    return header + bytes(entries) + bytes(body)


def write_png(path, pixels):
    path.write_bytes(png_frame(pixels))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--png",
        type=int,
        action="append",
        default=[],
        help="also write fh-<size>.png beside the .ico, to look at",
    )
    default = pathlib.Path(__file__).parent / "fh.ico"
    parser.add_argument("--out", default=str(default))
    args = parser.parse_args()

    frames = []
    for size in SIZES:
        pixels = frame(size)
        data = bmp_frame(pixels) if size <= 64 else png_frame(pixels)
        frames.append((size, data))
    out = pathlib.Path(args.out)
    out.write_bytes(ico(frames))
    print(f"{out}: {len(SIZES)} frames, {out.stat().st_size} bytes")
    for size in args.png:
        path = out.with_name(f"fh-{size}.png")
        rows = frame(size)
        write_png(path, rows)
        print(f"{path}: {path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
