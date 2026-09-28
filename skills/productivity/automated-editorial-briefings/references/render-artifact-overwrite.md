# render.py overwrites yesterday's artifacts — preserve before re-rendering

Reference for a specific failure mode in the v4 renderer: `render.py`
writes `~/.hermes/data/hermes-times-v4/issues/<issue_date>.{html,pdf,png}`
unconditionally. If you compose and render a second edition for the
same date (preview + final, or v1 + v2), the second run **wipes the
first run's rendered artifacts**. The continuity JSON at
`manifests/<date>.json` survives because `render.py` doesn't write
that path, but the PDF/PNG for the previous version do not.

This bit the v2 (10-page) build on 2026-07-26 — restoring
`manifests/2026-07-26.json` for the v1 issue required re-rendering
after every v2 render that clobbered the PDF.

## The pattern

```
Morning 22:07   cron fires → render.py v1 → issues/2026-07-26.{pdf,png,html}
Afternoon 22:30 user says "expand to 10 pages" → orchestrator composes v2
Afternoon 22:35 render.py v2 → issues/2026-07-26.{pdf,png,html}  ← v1 wiped
Afternoon 22:40 user asks "what about the original 4-page version?"
                  → must re-render v1 from the saved manifest
```

## The fix

Three options, in order of preference:

### 1. Archive each rendered edition to its own dated subdirectory

The orchestrator's Step 7 ("Archive + send") should mkdir-and-copy
the rendered artifacts into a versioned directory before the next
render can clobber them. The pattern already exists in the
Documents/fun archive:

```
~/Documents/fun/HermesTimes/<date>-v1/ed.html
~/Documents/fun/HermesTimes/<date>-v2/ed.html
```

Mirror that into the canonical data dir as well, e.g.
`~/.hermes/data/hermes-times-v4/issues/<date>-v1/ed.html`. Self-serve
looking forward.

### 2. Make `issue_date` a suffix

Patch `render.py` to write to
`issues/<issue_date>-<version>.{html,pdf,png}` and update the
`page_count` check / send.sh path. Cleaner but requires
backwards-compat handling for any caller that hard-codes
`issues/<date>.pdf`.

### 3. Restore-before-recompose ordering

Before composing a new edition, snapshot the previous artifacts:

```bash
cp ~/.hermes/data/hermes-times-v4/issues/<date>.* \
   ~/.hermes/data/hermes-times-v4/issues/<date>-prev.* 2>/dev/null || true
```

This is the simplest patch and the one we used on 2026-07-26 to undo
the v2 overwrite. Bake it into the orchestrator's Step 5 ("Press run")
at the top, so the previous edition is preserved before the new one
overwrites it.

## What NOT to do

- **Do not re-render from the manifest every time you want to check.**
  `render.py` rewrites the issues dir. If you only want to read the
  PDF, open it from the Documents/fun archive directory.
- **Do not delete the v1 archive.** When the user asks "make it more
  like a newspaper," that is a *new* edition, not a replacement.
  Both versions belong to the timeline.
- **Do not rename `.json` artifacts.** The continuity read in the
  next morning's cron looks for `issues/<date>.json`. Renaming it
  breaks the lead-pick for tomorrow.

## Pitfalls

- **The continuity JSON at `manifests/<date>.json` IS preserved across
  renders.** Only the rendered artifacts get clobbered. So the next
  morning's lead pick still works.
- **`pdfinfo` after a clobber will report v2's page count, not v1's.**
  If you are checking that yesterday's archive still has 4 pages, use
  the archive PDF, not the live `issues/<date>.pdf`.
- **The Documents/fun archive is the source of truth after v2 ships.**
  It has the versioned copy. Don't try to recover the old PDF from
  the live data dir.

## What this changes in the orchestrator prompt

Step 5 ("Press run") should add before the `render.py` call:

```bash
# Snapshot the previous edition's rendered artifacts so v(n+1) does
# not wipe v(n). Skip if no prior render exists.
BACKUP_VERSION="v$((${ISSUE_NUMBER:-0} - 1))"
if [[ -f "${ISSUES_ROOT}/${ISSUE_DATE}.pdf" ]]; then
  cp "${ISSUES_ROOT}/${ISSUE_DATE}.{html,pdf,png}" \
     "${ISSUES_ROOT}/${ISSUE_DATE}-${BACKUP_VERSION}." 2>/dev/null || true
fi
```

Or, simpler, write the snapshot into
`~/Documents/fun/HermesTimes/${ISSUE_DATE}-${BACKUP_VERSION}/` to keep
the archive aligned with the kid's read-through directory.
