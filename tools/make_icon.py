#!/usr/bin/env python3
"""Generate the app icon (SVG, PNGs, ICO, ICNS) from one set of shapes.

The shapes mirror `src/ui/logo.rs` (512 grid). Run from the repo root:
    python3 tools/make_icon.py
Needs Pillow.
"""
from pathlib import Path

from PIL import Image, ImageDraw

GOLD = (0xF0, 0xB3, 0x5A, 255)
INK = (0x14, 0x16, 0x1C, 255)
BG_RADIUS = 112

# (x, y, w, h, corner radius) on a 512 grid. Same list as src/ui/logo.rs.
SHAPES = [
    (150, 112, 62, 288, 31),   # F stem
    (150, 112, 212, 62, 31),   # F top bar
    (150, 222, 150, 56, 28),   # F middle bar
    (286, 314, 88, 88, 44),    # note head
    (356, 232, 18, 126, 9),    # note stem
]

OUT = Path(__file__).resolve().parent.parent / "assets" / "icon"


def svg() -> str:
    rects = "\n".join(
        f'  <rect x="{x}" y="{y}" width="{w}" height="{h}" rx="{r}" fill="#14161C"/>'
        for x, y, w, h, r in SHAPES
    )
    return (
        '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 512 512" width="512" height="512">\n'
        '  <title>Faith Tabernacle Hymn Book</title>\n'
        f'  <rect width="512" height="512" rx="{BG_RADIUS}" fill="#F0B35A"/>\n'
        f"{rects}\n</svg>\n"
    )


def render(size: int) -> Image.Image:
    """Draw at 8x and downsample for clean edges."""
    big = size * 8
    k = big / 512
    im = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    d = ImageDraw.Draw(im)
    d.rounded_rectangle((0, 0, big - 1, big - 1), radius=BG_RADIUS * k, fill=GOLD)
    for x, y, w, h, r in SHAPES:
        d.rounded_rectangle((x * k, y * k, (x + w) * k - 1, (y + h) * k - 1), radius=r * k, fill=INK)
    return im.resize((size, size), Image.LANCZOS)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    (OUT / "icon.svg").write_text(svg())
    for s in (16, 32, 48, 64, 128, 256, 512, 1024):
        render(s).save(OUT / f"icon-{s}.png")
    master = render(1024)
    master.save(OUT / "icon.ico", sizes=[(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)])
    master.save(OUT / "icon.icns")
    print("wrote", sorted(p.name for p in OUT.iterdir()))


if __name__ == "__main__":
    main()
