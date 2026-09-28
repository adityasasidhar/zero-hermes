---
name: markdown-to-pdf
description: Render Markdown files (Obsidian notes, READMEs, docs) to print-ready PDF without pandoc or LaTeX. Uses python-markdown for parsing, Pygments for syntax highlighting, and headless Chrome for print-to-PDF. Works for any MD source — vault notes, technical docs, tutorials, reports.
platforms: [linux, macos, windows]
---

# Markdown → PDF

Render a Markdown file to a clean, print-ready PDF with syntax-highlighted code blocks, tables, and GitHub-style typography — without installing pandoc or a TeX toolchain.

## When to use this

- "Save this note / README / tutorial as a PDF"
- "Render my Obsidian vault note as a PDF"
- "Make a print-ready version of this doc"
- Any task where the deliverable is a PDF and the source is Markdown (or MD-flavored like Obsidian).

## Pipeline (at a glance)

1. **Parse** Markdown → HTML with `python-markdown` (extensions: `fenced_code`, `tables`, `sane_lists`, `toc`, `nl2br`).
2. **Colorize** every fenced code block with Pygments (substitute placeholders so the markdown parser doesn't mangle the code).
3. **Wrap** the HTML body in a print-tuned stylesheet (GitHub typography, A4 page, header/footer suppression, page-break rules, code blocks that don't split across pages).
4. **Print** to PDF via `google-chrome --headless=new --print-to-pdf --no-pdf-header-footer`.

The full working script is at `scripts/render.py` in this skill — copy it, point it at your MD file, run.

## Setup (one-time, per machine)

Chrome is the only hard requirement. Everything else installs via uv:

```bash
# Check Chrome
google-chrome --version

# Install Python deps into a project venv (PEP 668 — don't pip install bare)
mkdir -p ~/hermes/md-pdf && cd ~/hermes/md-pdf
uv venv .venv --python 3.11
uv pip install --python .venv/bin/python markdown pygments
```

If Chrome isn't installed: `sudo apt install google-chrome-stable` (Linux) — chromium also works, swap the binary name in the script.

## Quick start

1. Copy `scripts/render.py` to your working dir.
2. Edit the three path constants at the top (`MD_PATH`, `PDF_PATH`, `HTML_PATH`).
3. Run: `.venv/bin/python render.py && google-chrome --headless=new --no-sandbox --disable-gpu --no-pdf-header-footer --print-to-pdf=<PDF_PATH> file://<HTML_PATH>`

The script handles MD→HTML, Pygments colorization, and Obsidian-flavor cleanup (frontmatter strip, trailing `#tag` lines). Chrome handles the rest.

## Pitfalls

### Chrome flag confusion — `--print-to-pdf-no-header` does NOT work in modern Chrome

The flag you want is `--no-pdf-header-footer`. The older `--print-to-pdf-no-header` is silently ignored in Chrome ≥130 — the PDF still gets a "date, title, URL" header at the top of every page. Verified on Chrome 150.

### Use `--headless=new`, not `--headless`

The old `--headless` mode is deprecated and prints differently (and will be removed). Always use `--headless=new` for HTML→PDF.

### Pandoc is not the only path

If the system has pandoc + LaTeX, that's fine for academic PDFs (better math, references). For a 10–30 page technical doc, this Chrome-based pipeline produces comparable output without the 500MB LaTeX install. Default to this pipeline unless the user specifically needs TeX features.

### Obsidian frontmatter leaks into the PDF

YAML between `---` markers at the top of a note will render as a literal table. Strip it before HTML conversion:

```python
md_text = re.sub(r"^---\n.*?\n---\n", "", md_text, count=1, flags=re.DOTALL)
```

### Trailing `#tag` lines render as headings

Obsidian notes often end with `#rust #learning`. They render as a giant H1 in the PDF. Strip with:

```python
md_text = re.sub(r"\n#[\w#/][\w#/ \-]*\s*\Z", "", md_text)
```

### Code blocks split across pages

Add `page-break-inside: avoid; break-inside: avoid;` to `pre` in the stylesheet so each snippet stays together. For very long code blocks (>40 lines), this isn't physically possible — accept the split, or trim the block.

### Math isn't rendered

This pipeline doesn't render LaTeX math (`$...$`, `$$...$$`). If the source has heavy math, switch to pandoc + LaTeX or add MathJax/KaTeX to the HTML head before printing (extra step, not in the default script).

### Wikilinks `[[Note]]` become literal text

That's actually fine — they show up as bracketed text in the PDF, useful as a breadcrumb. If you want them as blue links, post-process the HTML to convert `[[X|Y]]` → `<a href="#">Y</a>`.

## Verification

After rendering, always verify the PDF visually before reporting done:

```bash
# Render page 1 (and optionally page N) to PNG for inspection
pdftoppm -png -r 90 -f 1 -l 1 /path/to/out.pdf /tmp/preview

# Pass to vision_analyze to QA layout
# Look for: text overflow, missing syntax colors, header/footer noise, broken tables
```

`pdftoppm` is part of `poppler-utils` (preinstalled on most Linux systems).

### Visual verification checklist (via `vision_analyze`)

Inspect at least two pages: **page 1** (TOC + intro, catches stylesheet regressions) and a **content page with code blocks** (catches syntax-highlighting breaks). Ask vision: "Are there any header/footer (date, title, page numbers, file URLs) at the top/bottom margins? Is the code colored? Does text overflow the page margins?"

Common things vision catches that text-based QA misses:
- Chrome silently adding a `MM/DD/YY, HH:MM | Title` header when `--no-pdf-header-footer` is missing.
- A `file:///path/to/print.html` URL footer.
- Light-on-dark code blocks where the inline `<code>` chip CSS bleeds into the block background.
- Tables that wrap and lose their borders.
- Wikilinks rendering as `<code>tags</code>` because the markdown parser mistook `[[X]]` for HTML.

## Files in this skill

- `scripts/render.py` — full working MD→HTML script with Obsidian cleanup, Pygments colorization, and print stylesheet. Copy + edit paths at top.
