#!/usr/bin/env bash
# html-to-png.sh — render an HTML file (with inline SVG) to a PNG via headless Chromium.
# Default: 1280x720 (16:9 HD). Override via WIDTH/HEIGHT env vars.
#
# Usage:
#   html-to-png.sh <input.html> <output.png> [width] [height]
#
# Why this exists: the user closed the preview pane (2026-08-06). All HTML/SVG diagrams
# must be rendered to PNG and delivered inline via `MEDIA:<path>`. This is the default
# pipeline for any visual-first reply that needs an HTML/SVG source.

set -euo pipefail

INPUT="${1:?usage: html-to-png.sh <input.html> <output.png> [width] [height]}"
OUTPUT="${2:?usage: html-to-png.sh <input.html> <output.png> [width] [height]}"
WIDTH="${3:-1280}"
HEIGHT="${4:-720}"

# Pick a chromium binary if available
CHROME=""
for candidate in chromium chromium-browser google-chrome google-chrome-stable chrome; do
  if command -v "$candidate" >/dev/null 2>&1; then
    CHROME="$candidate"
    break
  fi
done

if [[ -z "$CHROME" ]]; then
  echo "error: no chromium binary found (looked for chromium, chromium-browser, google-chrome, chrome)" >&2
  exit 1
fi

mkdir -p "$(dirname "$OUTPUT")"

"$CHROME" \
  --headless \
  --disable-gpu \
  --no-sandbox \
  --hide-scrollbars \
  --default-background-color=00000000 \
  --screenshot="$OUTPUT" \
  --window-size="${WIDTH},${HEIGHT}" \
  "file://$(realpath "$INPUT")"

# Verify
if [[ ! -s "$OUTPUT" ]]; then
  echo "error: chromium produced an empty file at $OUTPUT" >&2
  exit 1
fi

echo "$OUTPUT"
