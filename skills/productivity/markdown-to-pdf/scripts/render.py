#!/usr/bin/env python3
"""
Render Markdown (Obsidian-flavored) to a print-ready HTML page that headless
Chrome can convert to PDF.

Usage:
  1. Edit the three path constants below (MD_PATH, PDF_PATH, HTML_PATH).
  2. .venv/bin/python render.py
  3. google-chrome --headless=new --no-sandbox --disable-gpu \
        --no-pdf-header-footer --print-to-pdf=<PDF_PATH> file://<HTML_PATH>

Customize the CSS in HTML_DOC if you want different typography.
"""
from __future__ import annotations

import re
from pathlib import Path

import markdown as md
from pygments import highlight
from pygments.formatters import HtmlFormatter
from pygments.lexers import get_lexer_by_name, guess_lexer, TextLexer
from pygments.util import ClassNotFound

# --- Edit these three paths ------------------------------------------------
MD_PATH = Path("/home/arctic/Documents/fun/Learning/Rust/Rust.md")
PDF_PATH = Path("/home/arctic/Documents/fun/Learning/Rust/Rust.pdf")
HTML_PATH = Path("/home/arctic/hermes/md-pdf/Rust.print.html")
# -------------------------------------------------------------------------

md_text = MD_PATH.read_text(encoding="utf-8")

# Strip Obsidian YAML frontmatter so it doesn't render as a literal table.
md_text = re.sub(r"^---\n.*?\n---\n", "", md_text, count=1, flags=re.DOTALL)

# Strip trailing Obsidian tags ("#rust #learning") — they become H1 headings.
md_text = re.sub(r"\n#[\w#/][\w#/ \-]*\s*\Z", "", md_text)

# Stash code blocks, colorize with Pygments, swap back in after MD parsing.
CODE_FENCE = re.compile(r"^```(\w+)\s*\n(.*?)^```\s*$", re.MULTILINE | re.DOTALL)
PYGMENTS_FORMATTER = HtmlFormatter(
    style="github-dark",
    cssclass="codehilite",
    linenos=False,
    wrapcode=True,
    noclasses=False,
)


def colorize(lang: str, code: str) -> str:
    try:
        lexer = get_lexer_by_name(lang)
    except ClassNotFound:
        lexer = guess_lexer(code) or TextLexer()
    return highlight(code, lexer, PYGMENTS_FORMATTER)


colorized_blocks: list[str] = []


def stash(m: re.Match) -> str:
    block = colorize(m.group(1), m.group(2))
    colorized_blocks.append(block)
    return f"@@CODEBLOCK_{len(colorized_blocks) - 1}@@"


md_text = CODE_FENCE.sub(stash, md_text)

HTML_BODY = md.markdown(
    md_text,
    extensions=["fenced_code", "tables", "sane_lists", "toc", "nl2br"],
    output_format="html5",
)
for i, block in enumerate(colorized_blocks):
    HTML_BODY = HTML_BODY.replace(f"@@CODEBLOCK_{i}@@", block)

PYGMENTS_CSS = PYGMENTS_FORMATTER.get_style_defs(".codehilite")

HTML_DOC = f"""<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Rendered Document</title>
<style>
  @page {{ size: A4; margin: 18mm 16mm 20mm 16mm; }}
  * {{ box-sizing: border-box; }}
  html, body {{
    margin: 0; padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto,
                 "Helvetica Neue", Arial, "Noto Sans", sans-serif;
    font-size: 11pt;
    line-height: 1.55;
    color: #1f2328;
    background: #ffffff;
  }}
  body {{ padding: 4mm 2mm; }}
  h1, h2, h3, h4 {{
    color: #0d1117; line-height: 1.25;
    margin-top: 1.6em; margin-bottom: 0.5em;
    page-break-after: avoid; break-after: avoid;
  }}
  h1 {{ font-size: 26pt; border-bottom: 2px solid #d0d7de; padding-bottom: 6pt; margin-top: 0; }}
  h2 {{ font-size: 18pt; border-bottom: 1px solid #d0d7de; padding-bottom: 4pt; }}
  h3 {{ font-size: 14pt; color: #1f2328; }}
  h4 {{ font-size: 12pt; color: #57606a; }}
  p, li {{ text-align: justify; hyphens: auto; }}
  code {{
    font-family: "JetBrains Mono", "Fira Code", "SF Mono", Menlo, Consolas,
                 "Liberation Mono", monospace;
    font-size: 9.5pt;
  }}
  :not(pre) > code {{
    background: #eff1f3; color: #1f2328;
    padding: 1.5pt 4pt; border-radius: 4pt; font-size: 9pt;
  }}
  pre {{
    background: #0d1117; color: #e6edf3;
    padding: 10pt 12pt; border-radius: 6pt;
    overflow-x: auto; font-size: 9pt; line-height: 1.45;
    page-break-inside: avoid; break-inside: avoid;
    margin: 10pt 0;
  }}
  pre code {{ background: transparent; color: inherit; padding: 0; font-size: inherit; white-space: pre-wrap; word-break: break-word; }}
  table {{ border-collapse: collapse; width: 100%; margin: 10pt 0; font-size: 10pt; page-break-inside: avoid; }}
  th, td {{ border: 1px solid #d0d7de; padding: 5pt 8pt; text-align: left; vertical-align: top; }}
  th {{ background: #f6f8fa; }}
  blockquote {{
    border-left: 4px solid #d0d7de; background: #f6f8fa; color: #57606a;
    padding: 6pt 12pt; margin: 8pt 0; border-radius: 0 4pt 4pt 0;
  }}
  hr {{ border: 0; border-top: 1px solid #d0d7de; margin: 18pt 0; }}
  a {{ color: #0969da; text-decoration: none; }}
  ul, ol {{ padding-left: 22pt; }}
  {PYGMENTS_CSS}
  h1 + *, h2 + *, h3 + * {{ page-break-before: avoid; }}
</style>
</head>
<body>
{HTML_BODY}
</body>
</html>
"""

HTML_PATH.parent.mkdir(parents=True, exist_ok=True)
HTML_PATH.write_text(HTML_DOC, encoding="utf-8")
print(f"HTML written: {HTML_PATH} ({len(HTML_DOC)} bytes)")
print(f"Next: google-chrome --headless=new --no-sandbox --disable-gpu --no-pdf-header-footer --print-to-pdf={PDF_PATH} file://{HTML_PATH}")
