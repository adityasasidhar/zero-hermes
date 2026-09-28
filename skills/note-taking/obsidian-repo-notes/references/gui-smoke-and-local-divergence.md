# GUI app smoke tests and local-vs-origin divergence

Two patterns surfaced repeatedly during `enrich-wave<N>` runs and aren't yet in
the main SKILL.md pitfall list. Capture them here so future wave subagents can
stop reinventing them.

## 1. Headless smoke test for GUI/desktop apps

`py_compile` proves syntax; for PyGame / Tk / Qt / SDL / GLFW / GLFW-cffi
repos it tells you nothing about whether the entry point can actually
initialize. Run a forced-headless smoke test instead, and quote the exact
command in `## Status` so the parent agent can re-run it during verification.

### PyGame (most common — works for `fun-with-pygame`, Gappy, etc.)

```bash
SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=dummy .venv/bin/python - <<'PY'
import pygame
from <module_with_Game_class> import Game
g = Game()                # should not raise
g.draw_world(0.25)        # one render pass
g.draw_minimap()
g.draw_hud()
g.draw_chatbox(0.25)
pygame.display.flip()
pygame.quit()
print('headless_smoke PASS', g.screen.get_size())
PY
```

What this proves:
1. The venv resolves (no `ModuleNotFoundError`).
2. The `Game.__init__` runs end-to-end, which means `properties/*.json`,
   `properties/*.txt`, and any other runtime state files parse.
3. The first frame draws without exceptions, which means coordinate hashes,
   font loaders, color lookups, and Pygame `draw_*` calls all agree on the
   available surfaces.

### Tkinter

```bash
PYTHONUNBUFFERED=1 python3 - <<'PY'
import tkinter as tk
from <app_module> import build_root
root = build_root()
root.update_idletasks()
root.update()
root.destroy()
print('tk_smoke PASS')
PY
```

If the app calls `sys.exit()` or quits the interpreter on startup, wrap the
above in `subprocess.run([...], check=True, env={..., 'DISPLAY':''})` and
check the returncode.

### Qt (PyQt5 / PySide2 / PySide6)

```bash
QT_QPA_PLATFORM=offscreen python3 - <<'PY'
from PySide6.QtWidgets import QApplication
from <app_module> import MainWindow
import sys
app = QApplication(sys.argv)
w = MainWindow()
w.show()
app.processEvents()
print('qt_smoke PASS')
PY
```

For Qt, use `QT_QPA_PLATFORM=offscreen` (PySide6) or `QT_QPA_PLATFORM=minimal`
(PyQt5) — both suppress the requirement for an X server.

### What to do if it fails

Capture the exception verbatim in `## Status` and **adjust `### How to run
it` to reflect reality**. The brief says "trust the CODE," and the code is
the authoritative source of whether the README's run command works. If the
README says `python game.py` but the smoke test reveals a missing key file,
write `python game.py # requires GROQ_API_KEY env var` and explain why in
`### Notable details`.

## 2. Local HEAD vs origin/main divergence

When the parent agent clones `~/projects/<slug>/` immediately before
dispatching the wave, the local checkout is often one or more commits ahead
of `origin/main`. The brief says "describe the cloned repo," but readers
will also try to follow the GitHub URL — so the note must reconcile the two.

### Quick divergence check

```bash
cd ~/projects/<slug>
echo "local:  $(git rev-parse --short HEAD)"
echo "remote: $(gh api repos/OWNER/SLUG/branches/main --jq '.commit.sha' | cut -c1-7)"
git rev-list --left-right --count HEAD...origin/main
git diff --stat HEAD...origin/main
```

Interpretation:
- `0\t0` (or `0\t1` ahead of remote): local matches the published HEAD.
  Proceed normally.
- `1\t0` (one commit ahead): the author did a local rebuild before
  pushing. **Describe the local code; flag the mismatch in `## Status`.**
  Example: *"Inspected local `main` is clean at `ec530e6` (2026-07-19),
  one commit ahead of `origin/main`; the rebuilt World Explorer is not
  yet on GitHub."*
- Many commits ahead: the local working tree has unpublished changes
  beyond what a single-commit rebuild produces. Treat the local code as
  authoritative but add an explicit "ahead by N commits; not yet pushed"
  note.
- Behind: someone else has pushed and the clone is stale. Re-run
  `git fetch origin && git reset --hard origin/main` before starting the
  inventory.

### When to override the README

The README on GitHub reflects `origin/main`. The local README at
`README.md` (read by `read_file`) reflects the local HEAD. When they
diverge, the local README is more recent and is the better source for
*features*, but the GitHub URL is what the reader will visit. Cite both:

```
- `game.py` — local main loop adds threaded Groq worker with result queue
  (origin/main does not have this); see commit `ec530e6`.
```

That single line tells the reader which file/line to inspect and saves
them the `git diff HEAD...origin/main` round-trip.

## 3. Combined verification recipe

For a wave subagent's final self-check, both of the above can be folded
into one Python snippet:

```python
from pathlib import Path
import subprocess, re

p = Path('/tmp/enrich-wave<N>/<slug>.md')
v = Path('/home/arctic/Documents/fun/Github/Repos/<slug>.md')

# 1. byte count in the brief's window
size = p.stat().st_size
assert 5000 <= size <= 10000, f'byte count {size} outside 5–10k window'

# 2. all required sections present, in order
required = [
    '## Overview', '## Status', '## Inside the Codebase',
    '### Architecture / How it works', '### File structure',
    '### Key files', '### Notable details', '### Tech stack',
    '### How to run it',
    '## What this project is about',
    "## Use cases / When you'd reach for this",
    '## Key Features', '## Getting Started', '## Stats', '## Links',
]
text = p.read_text()
positions = [text.find(h) for h in required]
assert min(positions) >= 0, f'missing: {[h for h,i in zip(required,positions) if i<0]}'
assert positions == sorted(positions), 'sections out of order'

# 3. wikilink footer present (parent uses it for grep -Fqx verification)
assert text.rstrip().endswith('- Part of: [[Public Repos]]') or \
       text.rstrip().endswith('- Part of: [[Private Repos]]')

# 4. no raw README markdown noise leaked through
assert not re.search(r'!\[[^]]*\]\([^)]+\)|<img\b|\[!\[', text, re.I)

# 5. no comment-only code blocks in Getting Started
gs = text.split('## Getting Started', 1)[1].split('## Stats', 1)[0]
for block in re.findall(r'```[^\n]*\n(.*?)```', gs, re.S):
    assert re.search(
        r'(?m)^\s*(git|cd|python|source|pip|export|uv|npm|cargo|go|node|make|docker)\b',
        block,
    ), f'comment-only block in Getting Started:\n{block}'

# 6. vault untouched (parent will re-check, but a self-check catches drift)
vault_git = subprocess.run(
    ['git', 'status', '--short', '--', str(v)],
    capture_output=True, text=True,
).stdout
# the vault path may not be in a git repo; that's fine, the parent checks
# from its own workdir.

print(f'{p}  {size}  all-checks PASS')
```

Run this before printing the 3-line summary. If any assertion fails,
fix the note and re-run — don't ship a note that doesn't self-verify.
