---
name: visual-first-explanations
description: Use when explaining or showing concepts. Lead with visuals.
---

# Visual-First Explanations

Cross-format visual toolkit for everyday explanations. The class of task: "explain X" / "show me X" / "what does X look like" / "draw X" — concepts, papers, plans, scenes, anything where a picture beats a paragraph.

This skill is the umbrella. The actual hard techniques for each format live in their own skills:

- Mermaid — inline in chat (use markdown fence)
- HTML/SVG → render as PNG → inline `MEDIA:...` (no preview pane)
- `mmx image-01` → inline `MEDIA:...` (no preview pane)
- Markdown tables, ASCII art, inline `<img>` — all inline

Specific deep-skills to load alongside this one when the format demands:

- `iterative-image-generation` — when the image needs to match a reference style or contain multiple composable elements
- `architecture-diagram` — when the output is a dark-themed infra/system diagram that needs SVG polish
- `excalidraw` — when the user wants hand-drawn whiteboard sketch
- `manim-video` — when the explanation needs animation (3Blue1Brown-style)
- `p5js` — interactive sketches / generative art
- `pretext` / `claude-design` — when the artifact is a webpage, not a chat visual

## Decision framework

Pick a format BEFORE writing. Default order:

| Concept type | First pick | Why |
|---|---|---|
| Structure (flow, sequence, lifecycle, data model, schedule) | **Mermaid** | Renders inline, no render cost, scannable |
| "What does X look like" / concept illustration / scene / character | **`mmx image-01`** | Concrete visual ground; user explicitly wants this |
| Large or polished diagram (intricate architecture, branded visuals) | **HTML/SVG → PNG → inline `MEDIA:...`** | SVG polish + inline delivery |
| Comparison / schedule / before-after | **Markdown table** | Compact, scannable |
| Quick terminal snippet / simple sketch | **ASCII art** | Native, no tool |
| Pre-existing image asset | **Inline `<img>` URL or `MEDIA:...`** | Don't re-render |

Mix formats when it helps. Three concepts → three Mermaid nodes + one concept image → a table comparing options. The point is "show first, narrate after" — pick the formats that maximize signal density.

## When NOT to use a visual

The user-stated "don't abuse it" rule. NOT every reply needs a diagram. Skip visuals when:

- Short factual answer ("yes", "no", "184", "function-not-found")
- Code-only output (paste the code; the code IS the visual)
- Routine confirmation ("done", "saved", "fixed")
- One-line error explanation
- The visual would be smaller than the caption

**Decision rule:** if a one-line answer is clearer than a one-line diagram, prefer the line. Earn visuals by adding value.

## Delivery defaults

- **No preview pane.** HTML/SVG renders become PNG before delivery; the user wants visuals inline in the chat. (Confirmed 2026-08-06.)
- **Mermaid**: write inside a ` ```mermaid ` fence. Render is automatic.
- **PNG diagrams**: render with `chromium --headless --screenshot`, then `MEDIA:<path>`.
- **mmx images**: always `--out-dir <stable_path>` so the file is predictable, then `MEDIA:<absolute-path>`. See `mmx-cli` SKILL.md for full flag reference.
- **Tables / ASCII / inline `<img>`**: embedded directly in the markdown.

## Specific techniques captured from sessions

### mmx image-01 — detailed prompts yield better images

Vague prompts ("a nice image of a robot") yield mid images. Always specify: subject, scene, lighting, style, mood. Example working prompt:

> "DIY electronics starter kit spread out on a glowing oak workbench: solderless breadboard, Arduino Uno, jumper wires in neat loops, RGB LED matrix showing a tiny rainbow, multimeter, hot glue gun, overhead cinematic shot, soft warm key light from the left, cool blue rim light, photoreal, shallow depth of field"

More examples in `references/mmx-prompt-patterns.md`.

### Mermaid — keep it scannable

Default to left-to-right `flowchart LR` for flows. Use `sequenceDiagram` for API/agent calls. Use `stateDiagram-v2` for lifecycles. Use `classDiagram` for type hierarchies. Use `erDiagram` for data models. Use `gantt` for plans. Use `gitGraph` for branching.

Limit node count to ~10 per diagram. If you have more, split into nested diagrams or use a table.

### mmx output filename collision

Successive `mmx image generate` calls in the same `--out-dir` overwrite the same fixed filename (`image_001.jpg`). Beyond a single image, rename between calls or use one subdir per image. See `iterative-image-generation` for the full pitfall list.

### HTML/SVG → PNG pipeline

For SVG-only, render in a wrapper HTML and chromium-screenshot the wrapper. Inline the result via `MEDIA:<path>`.

Use `scripts/html-to-png.sh` for the conversion. Common headless chromium command pattern:

```bash
chromium --headless --disable-gpu --no-sandbox \
  --screenshot=/path/out.png --window-size=1280,720 \
  file:///path/to/diagram.html
```

## Pitfalls

- **Don't open HTML/SVG in the preview pane.** User explicitly closed it. Always render to PNG and deliver inline.
- **Don't paste raw HTML in the chat.** GitHub-flavored markdown sanitizes it; the user sees text. Render to PNG first.
- **Don't generate 5 images when 1 will do.** One well-prompted image beats five mediocre ones.
- **Don't explain at length before showing.** Show first, narrate after. The rule is "diagram first, short caption second".
- **Don't ask permission to generate a visual.** Default is "yes, I'll generate that". Only ask when the request is genuinely ambiguous.
- **Don't re-render a Mermaid block as an image.** It already renders inline; outputting an image is duplication.
- **Don't use a visual where text is genuinely clearer.** Initials, short commands, fixed strings — text wins.

## Verification before declaring done

- [ ] Visual is in the chat (not a preview pane)
- [ ] Format matches the concept type (table for comparison, Mermaid for flow, mmx for "what does X look like")
- [ ] If image: detailed prompt (subject, scene, lighting, style)
- [ ] If image: vision-reviewed for fake text, logolism, wrong direction
- [ ] Caption is short — narrate AFTER, not before
- [ ] No visual was generated where text would have been clearer
