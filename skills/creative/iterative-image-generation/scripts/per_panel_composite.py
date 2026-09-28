#!/usr/bin/env python3
"""
per_panel_composite.py — composite 3 panel images onto a paper-style background.

Use when iterating AI-generated hero images hits its ceiling: generate each
panel as a separate focused prompt (where the model only needs to nail ONE
element), then this script drops them onto a cream paper background with
optional torn-edge and drop-shadow effects.

Usage:
    python3 per_panel_composite.py \\
        --left left_panel.jpg --middle middle_panel.jpg --right right_panel.jpg \\
        --out hero.jpg --width 1376 --height 768

Requires: Pillow (`uv pip install pillow` or `pip install pillow`).
"""

import argparse
from pathlib import Path

from PIL import Image, ImageDraw, ImageFilter


def torn_edge_mask(size, jaggedness=8, seed=0):
    """Build a soft alpha mask with torn-paper edges. Used as a layer mask."""
    w, h = size
    img = Image.new("L", (w, h), 255)
    draw = ImageDraw.Draw(img)
    # crude irregular polygon around the perimeter
    import random
    rng = random.Random(seed)
    # top/bottom
    for x in range(0, w, jaggedness * 2):
        draw.line([(x, rng.randint(0, 4)), (x + jaggedness, rng.randint(0, 6))], fill=255)
        draw.line([(x, h - rng.randint(0, 4)), (x + jaggedness, h - rng.randint(0, 6))], fill=255)
    # left/right
    for y in range(0, h, jaggedness * 2):
        draw.line([(rng.randint(0, 4), y), (rng.randint(0, 6), y + jaggedness)], fill=255)
        draw.line([(w - rng.randint(0, 4), y), (w - rng.randint(0, 6), y + jaggedness)], fill=255)
    # soften edges
    return img.filter(ImageFilter.GaussianBlur(1.5))


def paper_background(w, h, base=(247, 245, 239), fleck_count=80, seed=0):
    """Cream paper texture with subtle flecks. base is the cream color (R,G,B)."""
    import random
    rng = random.Random(seed)
    img = Image.new("RGB", (w, h), base)
    draw = ImageDraw.Draw(img)
    for _ in range(fleck_count):
        x = rng.randint(0, w - 1)
        y = rng.randint(0, h - 1)
        r = rng.randint(1, 3)
        # subtle brown fleck
        c = (rng.randint(170, 200), rng.randint(160, 190), rng.randint(140, 170))
        draw.ellipse([(x, y), (x + r, y + r)], fill=c)
    return img


def paste_with_shadow(bg, panel, x, y, shadow_offset=4, shadow_blur=8):
    """Paste `panel` onto `bg` at (x, y) with a soft drop-shadow underneath."""
    # build shadow
    shadow = Image.new("RGBA", bg.size, (0, 0, 0, 0))
    shadow.paste(panel, (x + shadow_offset, y + shadow_offset))
    shadow = shadow.filter(ImageFilter.GaussianBlur(shadow_blur))
    bg_rgba = bg.convert("RGBA")
    bg_rgba = Image.alpha_composite(bg_rgba, shadow)
    # paste actual panel
    if panel.mode != "RGBA":
        panel = panel.convert("RGBA")
    bg_rgba.paste(panel, (x, y), panel)
    return bg_rgba


def main():
    ap = argparse.ArgumentParser(description="Composite 3 panel images onto a paper background.")
    ap.add_argument("--left", required=True, help="path to left panel image")
    ap.add_argument("--middle", required=True, help="path to middle panel image")
    ap.add_argument("--right", required=True, help="path to right panel image")
    ap.add_argument("--out", required=True, help="output image path")
    ap.add_argument("--width", type=int, default=1376)
    ap.add_argument("--height", type=int, default=768)
    ap.add_argument("--gap", type=int, default=40, help="horizontal gap between panels")
    ap.add_argument("--top-bottom", type=int, default=80, help="vertical padding")
    ap.add_argument("--no-torn-edges", action="store_true", help="skip torn-edge mask (clean rectangles)")
    ap.add_argument("--no-shadow", action="store_true", help="skip drop-shadow")
    args = ap.parse_args()

    panels = [Image.open(p) for p in [args.left, args.middle, args.right]]
    bg = paper_background(args.width, args.height)

    # compute panel size: 3 panels + 2 gaps + 2 side margins
    margin = args.gap * 2
    total_gap = args.gap * 2
    panel_w = (args.width - 2 * margin - total_gap) // 3
    panel_h = args.height - 2 * args.top_bottom

    # resize each panel to (panel_w, panel_h) — preserves aspect, padded if needed
    resized = []
    for p in panels:
        p.thumbnail((panel_w, panel_h), Image.LANCZOS)
        # center onto canvas of exact panel size
        canvas = Image.new("RGBA", (panel_w, panel_h), (0, 0, 0, 0))
        off_x = (panel_w - p.width) // 2
        off_y = (panel_h - p.height) // 2
        canvas.paste(p.convert("RGBA"), (off_x, off_y))
        resized.append(canvas)

    if not args.no_torn_edges:
        resized = [Image.composite(p, Image.new("RGBA", p.size, (0, 0, 0, 0)), torn_edge_mask(p.size)) for p in resized]

    # paste
    out = bg.convert("RGBA")
    x = margin
    y = args.top_bottom
    for p in resized:
        if args.no_shadow:
            out.paste(p, (x, y), p)
        else:
            out = paste_with_shadow(out, p, x, y)
        x += panel_w + args.gap

    out.convert("RGB").save(args.out, quality=92)
    print(f"wrote {args.out}  ({args.width}x{args.height})")


if __name__ == "__main__":
    main()