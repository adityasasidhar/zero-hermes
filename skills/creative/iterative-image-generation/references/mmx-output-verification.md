# MMX image output verification

Use this after `mmx image generate`, especially for batches, wallpaper packs, and publishing pipelines.

## Observed mismatch

A generation requested with:

```bash
mmx image generate --width 1920 --height 1080 \
  --out /path/wallpaper.png --quiet --non-interactive
```

returned `{"saved":["/path/wallpaper.png"]}`, while `file` identified the bytes as a 1920×1080 JFIF/JPEG. The completion notice repeated the requested path; neither response verified the codec.

Treat this as a **possible mismatch**, not a claim that image-01 always emits JPEG.

## Safe single-output pattern

```bash
out=/path/to/assets/wallpaper.jpg
mmx image generate --prompt "$PROMPT" --width 1920 --height 1080 \
  --out "$out" --quiet --non-interactive
file --brief --mime-type "$out"
file "$out"
```

Expected MIME for `.jpg`: `image/jpeg`; for `.png`: `image/png`.

If the codec and suffix differ:

- Rename when that codec is acceptable: `mv wallpaper.png wallpaper.jpg`.
- Transcode when a true PNG is required; then inspect the transcoded file again.
- Never merely leave JPEG bytes under `.png`: some consumers sniff bytes, while others trust suffixes, producing inconsistent behavior.

## Safe batch/parallel pattern

Assign every job a unique exact path:

```bash
mmx image generate ... --out "$OUT/01-gateway.jpg" &
mmx image generate ... --out "$OUT/02-courier.jpg" &
wait
```

Then verify all artifacts before packaging:

```bash
file "$OUT"/*
```

Do not use a shared `--out-dir` with default filenames for concurrent or repeated calls. Fixed names such as `image_001.jpg` can overwrite one another.

## Delivery checklist

1. Count outputs.
2. Inspect MIME/codec and pixel dimensions.
3. Correct suffix mismatches.
4. Vision-review each image for prompt-specific failures.
5. Build the archive only after verification, then test the archive itself.
