# Wallpaper and multi-variant image packs

Use this workflow when producing several polished backgrounds or visual variants for a known display or publishing slot.

## 1. Resolve the canvas

Explicit user dimensions win. If the wallpaper targets the current local desktop and no size was given, inspect the active display mode (for example, the starred current mode from `xrandr --current`). Generate at that exact aspect ratio and resolution when the model supports it; do not upscale a random square afterward.

Choose which screen edge needs negative space for icons, widgets, or a clock. On a conventional left-icon desktop, keep roughly the left third low-contrast and move the focal subject toward the right third. This is a composition requirement, not an afterthought.

## 2. Design a concept matrix

A pack should provide genuinely different choices, not six seed variations of one scene. A useful six-image spread is:

1. cinematic architectural scene
2. dynamic character/action scene
3. abstract systems or constellation motif
4. classical/editorial reinterpretation
5. atmospheric landscape
6. ultra-minimal OLED option

Keep one brand palette or conceptual motif across the pack, while varying scene grammar and energy.

## 3. Prompt for wallpaper usability

Order each prompt as:

1. identity/concept being represented
2. primary subject and action
3. focal placement and icon-safe negative space
4. palette, lighting, atmosphere
5. medium/style and quality target
6. explicit exclusions

Example tail:

> Subject on the right third; left third calm, dark, and low contrast for desktop icons. No words, letters, fake signage, logos, watermark, interface panels, or clutter.

Do not rely on “minimal” alone: models may still fill the frame. State the empty region and its purpose explicitly.

## 4. Generate independently and safely

Give every job a unique exact `--out` destination. This makes parallel generation safe and avoids fixed-filename collisions:

```bash
mmx image generate --prompt "$P1" --width 1920 --height 1080 \
  --seed 3101 --out "$OUT/01-cinematic.jpg" --quiet --non-interactive &
mmx image generate --prompt "$P2" --width 1920 --height 1080 \
  --seed 3102 --out "$OUT/02-abstract.jpg" --quiet --non-interactive &
wait
```

Use distinct seeds and prompts. See `references/mmx-output-verification.md` before trusting extensions or packaging the results.

## 5. Vision-QA every image

Review each variant against a concrete checklist:

- wallpaper composition and focal balance
- requested icon-safe region is actually calm
- no fake text, accidental letters, logos, or watermark
- no anatomy, perspective, or geometry artifacts
- concept identity is recognizable rather than generic AI art
- score out of 10 and explicit PASS/REGENERATE

A pretty image can still fail as a wallpaper if icons become illegible. Evaluate it at the target dimensions.

## 6. Validate and package

Before delivery:

1. Count the expected outputs.
2. Inspect codec/MIME and dimensions from the actual bytes.
3. Fix extension mismatches or transcode when necessary.
4. Give files stable numbered names.
5. Build a flat archive and test it.

```bash
file "$OUT"/*
zip -j -9 "$OUT/Wallpaper-Pack.zip" "$OUT"/*.jpg
unzip -t "$OUT/Wallpaper-Pack.zip"
```

Deliver the individual images inline first so the user can compare them visually, then provide the tested archive. Brief labels help selection.

## Late completion notices

A background completion notice may arrive after the image has already been verified, renamed, and packaged. It often repeats the original requested output path and suffix. Treat it as process telemetry, not a request to regenerate. Reconcile it to the canonical verified artifact and respond briefly with that path if needed.
