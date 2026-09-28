# Obsidian wikilink semantics — source of truth for `verify_wikilinks.py`

Obsidian's `[[X]]` resolution is non-obvious if you don't use Obsidian regularly. This is the **consolidated rule set** the bundled verifier implements. It's been hand-checked on the Moonshot AI Papers/ library (12 papers, ~25 notes) against Obsidian's actual behavior.

## The resolver's algorithm

Given a note at `<vault>/<path>/dir/note.md`, the link `[[X]]`:

1. Look for a file/folder named exactly `X.md` (or `X/`) **in the same directory** as the source note.
2. If not found, look **in sibling subfolders** of `dir/`. So `[[Papers]]` from `dir/kimi-k2.md` finds `dir/../Papers.md`.
3. If still not found, **walk up** to the parent, then repeat from step 1.
4. If reached vault root without a match, **declaration as broken** in Obsidian (rendered as a red link).

## Concrete examples from this vault

Source note: `Research/Papers/Kimi-K2/kimi-k2.md`

| Link | Resolves to | Notes |
|---|---|---|
| `[[Papers]]` | `Research/Papers/Papers.md` | Step 2: sibling folder of `Kimi-K2/` is `Papers/` (no, wait — `Papers/` IS the parent, so step 1 fails, step 3 walks up). Actually: it walks up to `Research/Papers/`, finds `Papers.md` directly. |
| `[[kimi-linear]]` | `Research/Papers/Kimi-Linear/kimi-linear.md` | Step 1: same folder? No (`Kimi-K2/` doesn't have `kimi-linear.md`). Step 2: sibling subfolder — yes, found in `Kimi-Linear/`. |
| `[[../Research]]` | `Research/Research.md` | Step 1 in `..` (=`Research/Papers/`)? No. Step 2: walk up one more → `Research/` → has `Research.md`. |
| `[[../Papers/Papers]]` | `Research/Papers/Papers.md` | Two `..` levels up from the *note-level* (same as filesystem here): resolves to `Research/Papers/Papers.md`. |
| `[[wikilinks]]` | n/a | Common prose mention of the *syntax*, not a link. Verifier skips. |
| `[[Research]]` (from `Papers.md`) | `Research/Research.md` | Step 1 same folder (`Papers/`)? No `Research.md`. Step 3 walks up to `Research/`, finds it. |

## What the verifier catches

`scripts/verify_wikilinks.py` walks the whole vault, builds a basename lookup, and for each `[[X]]` returns the first matching file in the vault with that basename. This **slightly over-approximates** Obsidian (it can match a same-named note in a distant folder that Obsidian wouldn't find because it doesn't walk that far), but it never misses a real link.

For robust "did I actually use Obsidian-correct links in this note?", after the bulk rewrite pass, **open the note in Obsidian** to do a final visual check on `[[...]]` items. The verifier rules out the obvious mistakes but doesn't catch every pathed-style mismatch.

## Common errors and how to spot them

| Symptom | Likely cause | Fix |
|---|---|---|
| Note contains `[[../Papers/kimi-k2]]` everywhere after a slug rename | Mass rename broke path-relative links | `scripts/bulk_rewrite_wikilinks.py` with a regex rule |
| `[[Papeers]]` (typo) | Hand-written | Search-and-replace in the source note |
| `[[Kimi K2.5]]` (with space and dot) | Wrong slug for the actual file `kimi-k2.5.md` | Rename the file or fix the link |
| `[[alignment-faking-in-LLMs]]` reports broken even though `Alignment-Faking-in-LLMs/` exists | Case-sensitivity mismatch — the folder is `Alignment-Faking-in-LLMs/` but the file inside is lowercase (`alignment-faking-in-llms.md`). Obsidian's wikilink resolution is basename-based but case-sensitive | Use `[[alignment-faking-in-llms]]` (lowercase, matching the `.md` filename), not `[[alignment-faking-in-LLMs]]` |
| `[[Papers]]` in a per-paper note and Obsidian resolves it to itself | Filesystem ambiguity — `Papers.md` is in `Research/Papers/` while the note is also in `Research/Papers/X/` | Obsidian handles it correctly; no fix needed |

## Style preference (this user)

When linking to a sibling paper note from a per-paper note, **always use the short form** (`[[kimi-k2]]`, never `[[../Papers/kimi-k2]]`). The latter works but breaks when notes move or folders get reorganized.

When linking to the parent MOC from a per-paper note, use `[[Papers|Papers]]` if you want the literal display to read `Papers`, or just `[[Papers]]` if the file is `Papers.md`. Obsidian resolves both.

## Pipelines that benefit from this rule set

1. After every bulk rename — run `verify_wikilinks.py` to confirm no orphan `[[...]]` references.
2. Before declaring a vault-collection task done — run `verify_wikilinks.py` once and report the result count to the user.
3. When debugging "why does my link show as red in Obsidian?" — run `verify_wikilinks.py` against the report.

If a wikilink is reported broken but Obsidian clearly renders it, the mismatch is usually in how the file was named on disk vs the link text — use `search_files` with the basename to confirm.
