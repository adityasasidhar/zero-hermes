# Generating blog figures with PIL in a uv ephemeral env

When you need to produce a clean bar chart / scatter for a blog post and
you don't want to depend on matplotlib, use PIL directly. PIL is portable,
ships with most fonts already, and avoids the matplotlib + numpy
`_multiarray_umath` import error that bites `uv run --with` invocations.

## The exact failure pattern (avoid)

```bash
# This often breaks in uv ephemeral envs:
uv run --quiet --with matplotlib --with numpy python script.py
# -> ImportError: No module named 'numpy._core._multiarray_umath'
#    Original error was: No module named 'numpy._core._multiarray_umath'
```

Or with the system PIL:

```python
from PIL import Image
# ImportError: cannot import name '_imaging' from 'PIL'
#   (hermes internal venv has a broken PIL install)
```

## Working pattern

1. Write the script as a plain `.py` file under `/tmp/`.
2. Run with `uv run --no-project --python 3.11 --with pillow python /tmp/script.py`.
   The `--no-project` and explicit python version avoid inheriting a
   broken site-packages.
3. `Pillow` (with the L) is the right package name on PyPI; `PIL` is the
   import name.

```python
import os
from PIL import Image, ImageDraw, ImageFont

OUT = "/home/arctic/projects/<blog>/src/assets"

def get_font(size, bold=False):
    candidates = [
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf" if bold
            else "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Bold.ttf" if bold
            else "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
    ]
    for p in candidates:
        if os.path.exists(p):
            return ImageFont.truetype(p, size)
    return ImageFont.load_default()

def draw_rotated(d, text, font, fill, anchor_xy, angle=90):
    """Render text rotated 90° so the Y-axis label reads bottom-to-top."""
    bbox = d.textbbox((0, 0), text, font=font)
    w, h = bbox[2] - bbox[0], bbox[3] - bbox[1]
    tmp = Image.new("RGBA", (w + 8, h + 8), (255, 255, 255, 0))
    ImageDraw.Draw(tmp).text((4, 4 - bbox[1]), text, font=font, fill=fill)
    rotated = tmp.rotate(angle, expand=True, resample=Image.BICUBIC)
    rw, rh = rotated.size
    d.bitmap((anchor_xy[0] - rw // 2, anchor_xy[1] - rh // 2), rotated, fill=fill)
```

Y-axis labels must use `draw_rotated` — `d.text()` does not support a
free rotation angle, and a horizontal "BLiMP accuracy (%)" label will
overlap the leftmost bar group if the plot is drawn at standard chart
proportions.

## Layout numbers that look right (1280×720 PNG)

- Title: 22pt bold, top-left, padding 40px
- Plot area: left margin 130–150px (room for rotated Y label + tick labels),
  top margin 80–90px, bottom margin 60px (room for x labels)
- Bar group width = `plot_width / n_categories`
- Bar width within a group = `group_width / (n_series + 1)`
- Color choice: black for the baseline/reference, two warm shades
  (orange family) for recursive variants, two cool shades (slate-blue
  family) for the non-recursive twins — keeps the matched-pair story
  visually obvious

## When to prefer matplotlib instead

If the chart needs many small series, complex legends, log scales, error
bars, or any axis tick math that hand-coding would be tedious, install
matplotlib into the project's venv (`uv add matplotlib`) and use it from
there — the failure mode is specific to ephemeral `--with` invocations,
not to matplotlib itself.

## Verifying visually

Always preview the generated PNG before referencing it from the post —
overlapping axis labels and cut-off bars are easy to miss in code but
glaring in the rendered page. Use `vision_analyze` with a specific
question ("are any labels overlapping?") rather than just opening the
file.
