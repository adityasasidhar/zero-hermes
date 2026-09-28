# mmx image generate — batch workflow

When a leaf needs multiple editorial illustrations (e.g. AI-industry beat
running 5 stories, none of which have a downloadable primary photo), the
canonical recipe is to call `mmx image generate` once per story, rename the
fixed output, and verify before the next call. mmx writes to
`image_<seq>.jpg` in the `--out-dir` (default sequence `_001`), so concurrent
calls would clobber each other — do them strictly in sequence.

## The loop

```bash
ASSETS=/home/arctic/.hermes/data/hermes-times-v4/assets/<date>

# Story 1
mmx image generate \
  --prompt "<prompt-1, <1500 chars, 16:9 editorial photo>" \
  --aspect-ratio 16:9 \
  --out-dir "$ASSETS" --quiet
# File lands as either image_001.jpg (default prefix) or
# <prefix>_001.jpg (when --out-prefix is set). Either way:
mv "$ASSETS/<actual_filename>.jpg" "$ASSETS/ai_<slug-1>.jpg"
file "$ASSETS/ai_<slug-1>.jpg"   # must say "JPEG image data"
stat -c '%s' "$ASSETS/ai_<slug-1>.jpg"  # must be > 20480

# Story 2
mmx image generate --prompt "..." --aspect-ratio 16:9 \
  --out-dir "$ASSETS" --quiet
mv "$ASSETS/<actual_filename_2>.jpg" "$ASSETS/ai_<slug-2>.jpg"
file "$ASSETS/ai_<slug-2>.jpg"
stat -c '%s' "$ASSETS/ai_<slug-2>.jpg"
# ... repeat
```

Run `mmx image generate` one at a time, not in parallel — the API writes
to a fixed-name file, so parallel calls race and one will overwrite the
other's output.

**`--out-prefix` does NOT remove the trailing sequence number.** Passing
`--out-prefix ai_nvidia_rubin` produces `ai_nvidia_rubin_001.jpg`, not
`ai_nvidia_rubin.jpg`. The rename step is still required after every
call regardless of whether `--out-prefix` is set. The default prefix is
`image`, so without it you get `image_001.jpg`. If you want clean slug
filenames you still need `mv` after every call (the prefix only changes
the *base*, the `_001` suffix is hard-coded). Confirmed 2026-08-09
AI-industry beat: passed `--out-prefix ai_nvidia_rubin` and the file
landed as `ai_nvidia_rubin_001.jpg`; the rename step in the existing
loop recipe still applies.

## Timeouts

A 16:9 editorial generation typically completes in 60–90 seconds. Under
load it can run to the 90–120 second timeout. If a call hits timeout:

1. Check whether the output file was actually written before the timeout
   fired — sometimes the file lands even though the CLI process was killed
   by the parent shell's timeout. List the out-dir with `ls -la` after
   every call, before assuming the call failed.
2. If the file is there and `file` + `stat` pass, proceed — that's your
   illustration for that story.
3. If the file is missing or empty, simply re-run the same `mmx image
   generate` command. Don't switch tools; mmx is the canonical image
   backend and a retry usually lands in well under 90 seconds.

## Prompt constraints

- Hard limit: **< 1500 characters per prompt**. The CLI silently truncates
  beyond that, and the truncated prompt often produces a generic /
  off-topic illustration.
- Lead with the editorial intent ("Editorial illustration of...", "Editorial
  photo of...") rather than bare noun phrases — bare prompts produce
  abstract art that doesn't read as news photography.
- Specify the visual context (Capitol Hill, datacenter, trading floor,
  server room) plus the mood/tone (dramatic, photojournalism, professional).
- Aspect ratio `--aspect-ratio 16:9` is the schema requirement for
  Hermes Times v4 cards.

## Pitfall — do NOT request readable text in the generated image

The image model garbles most text strings. Asking for "a digital price tag
showing '$0.20 per 1M tokens'" produced an illustration with the literal
text "/usr./bin/bash.20" rendered onto the price tag — a hallucinated
file path. The same trap fires for "$X" figures, ticker symbols,
sentences, names, or any alphanumeric combination the model has to
*spell*. Patterns observed:

- Dollar amounts → "/usr./bin/bash.<digits>" or arbitrary mangled glyphs
- Ticker symbols (e.g. "NVDA") → random consonant clusters
- Multi-word captions → first word rendered, rest replaced with nonsense
- Numbers in general → digits appear but rarely the right ones
- Brand wordmarks in prompt ("Stripe", "Google", "Muse Code") → model
  produces something *evocative* of the brand (blue gradient, geometric
  G shapes) but the literal spelling is unreliable. Confirmed 2026-08-09
  AI-industry beat: prompts asking for "Stripe logo" or "Muse Code
  wordmark" rendered plausible abstract brand silhouettes but no
  readable text — that's fine for editorial illustration (the visual
  signal is what matters) but breaks if the story is about typography,
  a product launch where the wordmark matters, or any caption with
  required accuracy.

**Fix:** describe the *visual symbol* and leave the text out. For pricing
stories, use a downward arrow piercing a price tag with no caption; for
regulation stories, use a clipboard / magnifying glass / government
building silhouette with no label. If the story genuinely requires text
(e.g. a chart), generate a clean version with mmx and add the text in
post with `ffmpeg` drawtext — don't try to get the model to spell it.

Also avoid prompts that combine many text-like symbols: the model treats
long alphanumeric runs as a single token and corrupts them. Keep prompts
to visual nouns, colors, lighting, and composition cues.

## When the prompt is too generic and the render comes back off-topic

A prompt like "AI data center" or "executive at a conference" is too
abstract — mmx will return either an interior that doesn't read as a
real datacenter or a person whose face/setting has nothing to do with
the story. The harder check (already in the umbrella skill) is **topic
fit, not just file-format**. Run `vision_analyze` on the local path
after every generation with the question:

> "Does this image visually match a story about <topic>? List the
> visible elements (people, places, objects, text)."

If the answer is "no, this is a generic <other>" or the visible elements
don't include the anchors you asked for (no logo, no recognizable
building, no distinguishing prop), regenerate with a more specific
prompt. Confirmed 2026-08-09 AI-industry beat: the Stripe/OpenRouter
prompt landed as a recognizable stock-exchange floor with both brand
silhouettes visible; the NVIDIA/Rubin prompt produced a clearly
datacenter-aisle interior with a labeled rack; the EU AI Act prompt
returned a Berlaymont-style building with EU flags — all vision-
verified clean on first pass. Compare to the Aug 6 chip prompt which
landed fake text labels on the chip surface and required a retry with
a different (architecture) subject.

## Verification

Same rules as a downloaded photo:

```bash
file <path>           # must say "JPEG image data, ... baseline ..."
stat -c '%s' <path>   # must be > 20480
```

Plus a quick `vision_analyze` / `mmx vision describe` to confirm the image
isn't blank or generic — the prompt's "Editorial illustration of..." phrasing
sometimes produces a literal illustration (watercolor, cartoon) instead of
photojournalism. If the result is non-photographic, rewrite the prompt to
include "photojournalism" or "professional photography" and retry.

## Recording the result

Each generated image still needs the four fields the orchestrator expects:

```json
"image": {
  "kind": "generated",
  "local_path": "/home/arctic/.hermes/data/hermes-times-v4/assets/<date>/ai_<slug>.jpg",
  "source_url": "<the canonical primary URL the story is sourced to>",
  "credit": "Editorial illustration / <primary source attribution>"
}
```

`source_url` should still be the story's primary source URL (Reuters,
Bloomberg, etc.) — not an `mmx://` URI — so the orchestrator's citation
hygiene stays consistent. The `credit` line distinguishes generated art
from downloaded photography in the print/HTML renderer's caption.

## When ALL real photo sources fail and every rank is generated

The image-source hierarchy in the umbrella skill is "real photo first,
generated only as last resort." In some runs — particularly AI-industry
beats when every wire CDN is gated and every primary article has no
extractable hero — every rank may end up using `kind: "generated"`.
That is acceptable **only when** every attempt at a real source has
been documented in the run log, and the resulting JSON still cites
the canonical primary URL as `image.source_url`. **Anti-pattern:**
emitting `kind: "generated"` for a story you didn't even try to find a
real photo for, just because mmx is fast. The umbrella's pitfall
"don't ship a generated image you didn't actually need" applies —
mmx is the fallback, not the default.

Confirmed 2026-08-09 AI-industry beat: all five ranks used
`kind: "generated"` because Reuters, Getty, Bloomberg bwbx.io,
Wikimedia Commons, Alamy, and the Anthropic Sanity CDN were all
blocked from this environment for the in-window stories. The JSON
still cited the canonical primary URL (The Information, Google Blog,
TrendForce, Meta AI Blog, European Commission) as `image.source_url`,
and each `credit` was `"Generated illustration / Hermes"` so the
renderer could distinguish the art from a real photo at caption time.
