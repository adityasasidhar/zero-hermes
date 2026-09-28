# Pattern B leaf-transcript recovery — when the LLM log truncates the JSON

Reference for the failure mode where a beat correspondent returns its
final JSON in the assistant's last message, but Hermes's transcript
capture truncates the message to its first ~68 chars with an ellipsis
indicator (`...(+68 chars)`). The leaf *did* write the full JSON; it's
just lost from the visible transcript. The full JSON is usually on disk
in `~/.hermes/cache/delegation/subagent-summary-N-<timestamp>.txt`
where N is the task index. This happened in the v2 (10-page) run on
2026-07-26 with the Messi and F1 leaves.

## The trigger

Symptom: the leaf's final `status=completed summary: ...` line ends
with `"...(+68 chars)"` instead of a closing `}`. Search the transcript
for the substring `+NN chars` to confirm.

```bash
grep '+[0-9]\+ chars' /home/arctic/.hermes/cache/delegation/live/<deleg_id>/task-N.log | tail
```

If you see it, the body of one or more stories is gone from the visible
transcript. Other story fields (rank, headline, dek, source_url) are
usually preserved; only the `body` field is dropped from the rendered
view (it is the last big string in the JSON).

## The on-disk recovery path

Hermes persists the full leaf reply — including the JSON — in
`subagent-summary-N-<timestamp>.txt`. Naming is:

```
~/.hermes/cache/delegation/subagent-summary-<task_index>-<YYYYMMDD>_<HHMMSS>_<microseconds>.txt
```

For a 5-leaf batch with `delegation_id = deleg_xxx`, the files are:

```
subagent-summary-0-<ts>.txt   # AI industry
subagent-summary-1-<ts>.txt   # research
subagent-summary-2-<ts>.txt   # huggingface
subagent-summary-3-<ts>.txt   # messi
subagent-summary-4-<ts>.txt   # f1
```

`json.loads()` on these files returns the full untruncated package.

## The recovery recipe

Spawn a tiny leaf (no tools beyond `terminal` and `read_file`) to:

1. Open each `subagent-summary-N-*.txt` for the truncated leaves.
2. `json.loads()` and verify `body` is the full string (typically
   140-180 words for a 150-word target).
3. Renormalize the `image` block to the canonical schema:
   `{ kind, local_path, source_url, credit }`. The leaf's transcript
   summary sometimes uses a different shape (`path`, `art_source_url`,
   `art_credit`) that needs to be flattened.
4. Write to `~/.hermes/data/hermes-times-v4/<beat>.json` so the
   orchestrator can pick it up alongside the on-disk packages the
   other leaves wrote directly.

In practice the recovery takes ~1-2 minutes — faster than re-spawning
the leaf and avoiding the cost of a second full wire-search pass.

## Why the summary-line JSON is truncated

Hermes's per-message log capture appears to apply a length budget to the
*visible* last-message line, replacing the tail with `+N chars` once a
threshold is crossed (around 80-100 chars of display). The full message
*was* received by the orchestrator and *was* used to drive the leaf's
`status=completed`; only the human-readable transcript gets clipped.
This means the JSON is in the agent's effective context — only the
*displayed* transcript loses it.

So: never re-spawn a leaf to recover truncated JSON. Use the on-disk
summary file.

## Pitfalls

- **Do not trust the per-line `summary:` field** in the transcript —
  it's already truncated to ~68 chars. Treat the on-disk file as
  canonical.
- **Task index maps to leaf order**, not to output filename. The leaf
  that wrote `manifests/inbox_2026-07-26/<beat>.json` may not have the
  same task index as the one that wrote `<beat>.json` to the data
  root. Read the leaf's transcript to see what it did.
- **Recovery leaf image schema drift.** The summary-leaf shape uses
  `{ "path": "...", "art_source_url": "...", "art_credit": "..." }`
  for downloaded images. The orchestrator canonical shape is
  `{ "kind": "downloaded", "local_path": "...", "source_url": "...",
  "credit": "..." }`. The recovery leaf must flatten these or
  `img()` will fail when looking up the asset path.
- **Bodies may be shorter than target.** A leaf that hit the truncation
  budget on its first try may have written 140-word bodies instead of
  the 150-word target. That is still readable in the layout. Do not
  re-run the leaf just to lengthen by 10 words.
- **`subagent-summary-<N>-<ts>.txt` is not guaranteed to exist.** In
  v4 Issue 8 (2026-08-03) the cache held only `live/<deleg_id>/task-N.log`
  and `live/<deleg_id>/manifest.json` — no per-leaf summary file was
  persisted. The recovery recipe above must degrade gracefully when the
  summary file is absent. See "The six paths a leaf writes to" below
  for the full scan order.
- **Asset-vs-story mismatch in leaf output.** A leaf may download more
  images than it includes in the final JSON package. Confirmed v4
  Issue 8: the HF leaf reported 3 stories in its JSON but the assets
  directory contained 7 images (changelog_mcp.png, inkling.png,
  perception.png, stack.png, wan.png, plus two thumbnails the leaf
  did not name in any story). The orchestrator must trust the
  **JSON package**, not the asset directory. Cross-referenced images
  that don't appear in any `image.local_path` field are orphan downloads
  — leave them on disk for a future story but never invent a story to
  consume the extra assets. Do not silently drop a story because its
  asset is missing from the JSON either.

## What this changes in the orchestrator prompt

The orchestrator brief currently treats the leaf's on-disk JSON as
canonical. After the v2 run, the orchestrator brief needs a sub-step
inside Step 4 ("Editor composition") that says:

> Before composing the manifest, for every leaf whose transcript has
> the `+N chars` truncation marker, load the JSON from
> `~/.hermes/cache/delegation/subagent-summary-<N>-*.txt` and write
> it to `~/.hermes/data/hermes-times-v4/<beat>.json` if a full-body
> copy is not already on disk. This is faster than re-running the
> leaf and uses the same primary-source URL set.

## The six paths a leaf writes to

Across v4 issues, leaves have used six different on-disk locations for
their final JSON. The orchestrator must scan ALL of them before
deciding to re-spawn a leaf — re-running costs a full ~10-minute
wire-search pass, and the JSON is almost always recoverable:

| Path | Seen in | When to expect it |
|---|---|---|
| `~/.hermes/cache/delegation/subagent-summary-<N>-<ts>.txt` | v2 (2026-07-26) | If the leaf's final message was long enough to persist a summary. **Not guaranteed.** |
| `~/.hermes/cache/delegation/live/<deleg_id>/task-<N>.log` | v4 Issue 8 | **Always present**, but long `assistant|` turns are truncated to `…(+N chars)` at display time. Recoverable fields: rank, headline, dek, source_url, image.local_path, arxiv_id. **Lost**: body text. |
| `<beat>_<YYYY-MM-DD>.json` under the leaf's CWD (often `/tmp/`) | v4 Issue 8 AI industry | Leaf writes here voluntarily. Most leaves skip this. |
| `~/.hermes/data/hermes-times-v4/<YYYY-MM-DD>/<beat>.json` | v4 Issue 8 Messi | Hard-coded convention for the Messi beat — uses a per-date subdir. |
| `~/.hermes/data/hermes-times-v4/<beat>.json` (no date) | v4 Issue 8 Messi (alt) | Messi also writes this layout in some runs. |
| `~/.hermes/scripts/hermes-times-v4/<beat>_beat_<YYYY-MM-DD>.json` | v4 Issues 1, 7, 8 F1 | F1 convention — lives next to the renderer. |

**Recovery scan (in order, try each before re-spawning):**

```bash
# 1. On-disk summary file (if present)
ls ~/.hermes/cache/delegation/subagent-summary-<N>-*.txt 2>/dev/null

# 2. Leaf's own write-to-disk path (varies per leaf — see table)
ls -la ~/.hermes/scripts/hermes-times-v4/*_beat_$(date +%Y-%m-%d).json 2>/dev/null
ls -la ~/.hermes/data/hermes-times-v4/<YYYY-MM-DD>/*.json 2>/dev/null
ls -la ~/.hermes/data/hermes-times-v4/*.json 2>/dev/null
ls -la /tmp/*_$(date +%Y-%m-%d).json 2>/dev/null

# 3. Live transcript — partial recovery
grep -E "assistant\|" ~/.hermes/cache/delegation/live/<deleg_id>/task-<N>.log
#    Each assistant| line is ~80-100 chars of display; concatenate
#    prefix chunks to recover fields other than body. Body text is
#    GONE if the truncation marker is present.
```

**Confusion-of-authority pattern.** When the on-disk summary, the
leaf's own write-to-disk file, AND the live transcript all disagree on
the JSON contents, prefer the on-disk summary > leaf's own file >
transcript. The transcript is the least trustworthy because it is the
display layer that is subject to truncation; the on-disk files are
what the leaf actually wrote to durable storage. (Confirmed v4 Issue 8
2026-08-03: the AI industry leaf's `/tmp/ai_industry_2026-08-03.json`
was identical to its inline-JSON `summary:` text but the live
`task-0.log` showed only truncated fragments.)
