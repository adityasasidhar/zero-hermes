---
name: iterative-image-generation
description: Iterate on AI-generated images (mmx image-01, similar models) until they match a target style or contain specific elements. Use when the user asks for a blog hero, marketing visual, or any artwork that needs to match a reference aesthetic and contain multiple specific compositional elements. Covers prompt design, the realistic ceiling on multi-element compositions, chart-direction pitfalls, output-location gotchas, and the per-panel fallback when iteration plateaus.
---

# Iterative Image Generation — Workflow & Pitfalls

A practical guide for getting AI-generated images to match a reference style (editorial blog hero, brand aesthetic, etc.) when they need to contain **multiple specific compositional elements**. Applies to `mmx image generate` (model `image-01`) and similar text-to-image models with comparable ceilings.

The fundamental tension: image models are good at style transfer but bad at multi-element composition. Specifying "two stacks side by side" or "a downward-sloping loss curve" frequently fails despite multiple prompt rewrites. This skill documents the realistic ceiling and the fallback when you hit it.

## Cross-reference

When the agent loads `mmx-cli` first (the most common path — agents reach for `mmx image generate` directly), it should also load this skill. The `mmx-cli` SKILL.md has a "For blog heroes / multi-element compositions" note pointing here.

## When to load this skill

- User asks for a hero image that must match a reference style (e.g. "match my existing post hero")
- User asks for a wallpaper, background, or multi-variant image pack that needs target-screen composition and delivery QA
- The target image needs multiple distinct compositional elements (3+ panels, paired figures, charts with specific directions)
- You find yourself iterating on the same prompt more than twice with mixed results

## Workflow

### 1. Inspect the reference first

Before writing any prompt, look at the existing reference image with `vision_analyze`. Extract:
- Color palette (specific hex/region colors, not just "warm")
- Composition (3-panel? single? what proportions?)
- Style (photoreal / illustrated / hand-drawn / sketchy / flat)
- Texture (paper, glass, gradients, flat color)
- Typography if any (hand-lettered vs. printed)
- Lighting / mood

The point: be specific in your prompt, not vague. "Hand-drawn editorial collage style, three equal torn-paper panels, lavender/cream/terracotta palette" lands; "a nice looking blog hero" doesn't.

### 2. Write the prompt for ONE focused element first

If the target has multiple distinct elements (e.g. 3 panels each with a different diagram), start by getting ONE element right in isolation. Don't try to nail all three in the first prompt. Verify it, then build up.

### 3. Pin every output path, then verify the actual encoding

For a single image, prefer an exact destination with the current CLI:

```bash
mmx image generate --prompt "..." --aspect-ratio 16:9 \
  --out /path/to/assets/<slug>.jpg --quiet --non-interactive
```

For several independent calls, give every call a unique `--out` path or an isolated per-image `--out-dir`. Never send repeated calls into one shared directory with the default filename; later calls can silently overwrite earlier ones.

After generation, inspect the bytes rather than trusting the requested suffix or the `saved` JSON field:

```bash
file --brief --mime-type /path/to/assets/<slug>.*
file /path/to/assets/<slug>.*   # also confirms dimensions on common Linux setups
```

`mmx image-01` can save **JPEG-encoded bytes at a path ending in `.png`** when `--out ...png` is requested. The response may still report that `.png` path. If the bytes are JPEG, rename the file to `.jpg` before delivery; transcode only when a real PNG is required. This matters for wallpaper packs, web asset pipelines, and any consumer that trusts extensions.

**The reverse case also happens with downloaded images.** When you `curl` an image from a CDN that doesn't honour the extension (Bloomberg's `bwbx.io` images, certain Getty caches, social-media CDNs), a `--output image.jpg` or `<slug>.jpg` may end up holding **WebP/AVIF bytes** even though the filename says JPEG. `file` reports it as `RIFF (little-endian) data, Web/P` instead of `JPEG image data`. Browsers tolerate this in `<img>` tags, but Chrome's headless `--print-to-pdf` renderer used by some PDF pipelines (e.g. `hermes-times-v4/render.py`) does NOT — it silently produces a blank box where the image should be, and the rendered PDF/HTML has no visible error.

**Verification recipe (handles both cases):**

```bash
file --brief --mime-type /path/to/assets/<slug>.<ext>
# Expect: image/jpeg OR image/png — anything else (image/webp, image/avif, text/html) is wrong
file /path/to/assets/<slug>.<ext>   # also confirms dimensions
```

If the MIME doesn't match the extension, re-encode in place with `ffmpeg` (always present on Linux/Mac; small static binary):

```bash
ffmpeg -y -i input.webp output.jpg   # extension on output is what ffmpeg uses to choose codec
ffmpeg -y -i input.jpg output.png    # transcode to real PNG if you actually need PNG bytes
```

Then verify with `file` again. Do this BEFORE rendering, not after — the renderer's silent failure mode is "blank box" with no log line.

### 4. Always vision-review with a concrete checklist

Generic "how is this?" prompts miss concrete failure modes. Use a checklist tailored to the brief:

> "Be brutally honest. (1) Does the left panel show X? (2) Does the middle panel show Y with correct direction? (3) Any garbage/fake text? (4) Overall 1-10 rating."

This catches "chart points the wrong way" and "fake text on the third panel" that generic reviews miss.

### 5. Know the iteration ceiling and respect it

For a multi-element composition with specific requirements:
- **Realistic ceiling**: 1 panel perfect + 2 panels adequate. Trying to get all 3 panels perfect in a single prompt is unrealistic for current image models.
- **Retry pattern**: each retry tends to fix one element while regressing another. Track which element each version got right and pivot to that one if iteration plateaus.
- **Stop rule**: if 3+ retries haven't improved the composite, the model has hit its ceiling. Switch strategies.

### 6. When you hit the ceiling: per-panel composite

Generate each panel as a separate focused prompt where the model only needs to nail ONE element. Then composite them onto a paper background using PIL or ImageMagick. This is more work but solves "two stacks in one panel" cleanly because each generation has only one element to get right.

```python
from PIL import Image

bg = Image.open("background.jpg")
left = Image.open("left_panel.png").resize((400, 600))
middle = Image.open("middle_panel.png").resize((400, 600))
right = Image.open("right_panel.png").resize((400, 600))

# paste at known coordinates, optionally with rotation/torn-edge masks
bg.paste(left, (50, 100), left)
bg.paste(middle, (500, 100), middle)
bg.paste(right, (950, 100), right)
bg.save("hero.jpg", quality=92)
```

Torn-paper-edge effect: PIL `ImageDraw` with `filter=ImageFilter.GaussianBlur` on a mask of jagged polygons. Or use a CSS overlay if the composite goes on a website.

## Pitfalls (image-01 / similar models)

### Prompt budget: 1500 characters

Exceeding returns `invalid params, prompt length must be less than 1500`. Surprisingly useful — forces you to be selective about what to specify.

### Multi-element compositions collapse

When you ask for "two of the same thing side-by-side" (two block stacks, two charts, two figures), the model often renders ONE. It treats the second as redundancy and omits it. Observed across repeated attempts. If you need paired/triplicate elements, the per-panel composite is the reliable path.

### Output filename collision across multiple calls

Every `mmx image generate` call writes to the same fixed filename (`image_001.jpg` / `image_002.png` / etc.) inside the `--out-dir`. Successive calls **overwrite** the previous output silently — `--quiet` does not warn, and the JSON response only reports what was saved this round, not what was clobbered. Real cost: a script that does `for prompt in prompts: mmx image generate --prompt "$prompt" --out-dir assets/ --quiet` ends up with only the last image on disk.

**Fixes (pick one):**
- **Rename between calls** — the simplest fix: `mmx image generate ... && mv assets/image_001.jpg assets/<slug>.jpg`. Add the slug to a known manifest so the renderer can find it.
- **One subdir per image** — `mmx image generate --out-dir assets/<slug>/ ...`. The fixed filename then lives in its own dir and doesn't collide. Best when you'll iterate per-image.
- **Per-image wrapper script** — if you're calling from a build pipeline, write a small helper that takes `--prompt`, `--slug`, `--out-dir`, calls mmx, and renames in one step.

If you don't pick one and you need more than one image, you will lose all but the last. **Verify after each call** that `ls --out-dir` shows the expected files, not just one.

### Chart direction is unreliable

"Loss curves going down" gets rendered upward ~80% of the time. The model has a strong prior for "training curves go up over time" (true for accuracy/F1) and doesn't invert reliably for loss.

**Prompt patterns that finally worked:**
- Specify y-axis position explicitly: `y-axis labeled 'LOSS' with higher numbers at the top and lower numbers at the bottom`
- Use a directional arrow that physically points down: `a downward arrow descending from top-left to bottom-right` lands better than `curves descend`

### Bar counts are unreliable

Specifying "four bars in two pairs" can yield 3, 4, or 5 bars depending on the run. If exact bar count matters, generate the bar chart separately and composite.

### Fake text artifacts

Models will produce plausible-looking fake words ("Pnsthsk", "BEMA 2:12S↓", "irwedd"). Always vision-check for these and either accept them as paper-noise (hand-drawn reference style absorbs them) or regenerate with stricter "no text" constraints.

### Sports kit / jersey hallucination (esp. football/soccer)

When the prompt asks for a footballer, image-01 confidently fabricates kit details that aren't true and that you cannot safely keep:

- **Fake sponsor logos on the jersey** — even with "no logos" in the prompt, the model will sometimes paint a believable fake brand wordmark ("Pirelli", "Beko", "Etihad", etc.) across the chest. This violates editorial standards because it reads as a real sponsor that didn't pay to be there. **Vision-check every sports image at the size it'll be displayed; reject any jersey with legible text/logos.**
- **Wrong-sport helmet / pads** — "football player celebrating" defaults to American football (helmet + shoulder pads) unless you explicitly say "soccer", "association football", and "bareheaded". Two attempts may produce a helmet before the third produces the soccer silhouette. **Always include "bareheaded soccer player (NOT American football, no helmet)" in the prompt.**
- **Generic training kit instead of match kit** — the model often picks a blue/red training top or no shirt at all. Specify "in a team jersey of a single saturated colour, no visible text or logos".

**Safe-prompt pattern for a "hero celebrating" editorial image (verified working):**

> "Editorial silhouette photograph of a bareheaded male football player viewed from behind with both fists raised in celebration at night, soccer stadium floodlights creating dramatic god rays, pink-orange dusk Miami sky, palm trees at the stadium edges, blurred crowd, jersey has no visible logos or text, pure silhouette against the lit sky, photojournalism aesthetic, 16:9 aspect ratio"

Key elements: **silhouette** (kills logo hallucination), **viewed from behind** (no face/jersey detail to fabricate), **bareheaded soccer player (not American football, no helmet)**, **no visible logos or text**. Vision-QA prompt: "Be brutally honest. (1) Is this a single football/soccer player (NOT American football) with fists raised? (2) Any fake logos, fake sponsor text, fake jerseys with visible writing? (3) Does it look like a real photo? (4) Rate 1-10 as a newspaper cover hero."

### Style consistency degrades with prompt length

Long prompts with many style modifiers (palette + texture + lighting + 4 elements + 6 labels) tend to drop the style cohesion. The model keeps the most recent instructions better than the earliest. Put the most important style requirements at the END of the prompt if anything, or ruthlessly prioritize.

## Anti-patterns

- **Retrying the same prompt with minor wording changes** — burns quota, no real signal.
- **Adding more detail to fix a missing element** — usually makes the prompt worse. Remove instead of add.
- **Trying to get all 3+ panels perfect simultaneously** — out of model scope. Aim for "good enough across all panels" and iterate from there.
- **Trusting the `saved` path, suffix, or completion notice as format verification** — they prove where bytes were written, not their codec. Inspect MIME/type and dimensions before delivery. In particular, image-01 may write JPEG bytes to a requested `.png` path; rename the suffix or transcode.
- **Using one shared `--out-dir` for parallel generations** — fixed filenames collide. Assign one exact unique `--out` path per job or isolate each job in its own directory.

## Verification checklist before declaring done

- [ ] Output uses an exact unique path (`--out`) or an isolated directory; no parallel filename collisions
- [ ] Actual codec/MIME matches the filename extension (inspect bytes; do not trust `saved` JSON) — applies to BOTH mmx-generated images AND downloaded images (CDNs often ignore extensions)
- [ ] Dimensions match the target display or publishing slot
- [ ] Style matches the reference (palette, texture, illustration type)
- [ ] All required compositional elements present
- [ ] No orientation/direction errors (charts point the right way, bars are the right count)
- [ ] No garbage text artifacts that would look broken at the target size
- [ ] At target dimensions (use `--aspect-ratio` or `--width`/`--height`)
- [ ] Vision-reviewed with a concrete checklist, not a generic "looks good?"

## Scripts

`scripts/per-panel-composite.py` — minimal PIL script for compositing 3 panel images onto a paper background with torn-edge effect. Use when iteration hits the ceiling.

## References

`references/mmx-output-verification.md` — verified recipe for unique parallel output paths, detecting JPEG/PNG suffix mismatches, correcting them, and validating a batch before packaging or delivery.

`references/wallpaper-pack-workflow.md` — target-display sizing, concept diversification, icon-safe composition, per-variant vision QA, archive verification, and handling late background completion notices.