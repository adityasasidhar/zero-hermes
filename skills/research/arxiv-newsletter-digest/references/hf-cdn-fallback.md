# HuggingFace CDN Image Fallback Protocol

Hugging Face serves paper thumbnails at a single CDN URL pattern. It's fast and high-quality when it works, but it lags arXiv submission by 12–72 hours and routinely returns HTTP 504 for brand-new papers. This file is the working fallback protocol.

## Primary URL

```
https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<ARXIV_ID>.png
```

Returns a 1200×648 PNG, typically 150–400 KB. HTTP 200 means you have your image.

## Known failure modes

### Mode 1: 504 Gateway Timeout

The HF CDN backend (`awselb/2.0`) times out. The `curl` response is HTML, ~130 bytes:

```html
<html><head><title>504 Gateway Time-out</title></head>...
```

curl will silently save this as `<your-path>.png` if you used `-sLo`. The file passes a size check (> 0 bytes) but is not a PNG.

### Mode 2: Empty file

Sometimes the CDN returns 200 with `content-length: 0`. Same trap — `-sLo` saves an empty file.

### Mode 3: Paper not yet indexed

HF daily papers only includes a paper once it's been submitted to their pipeline. There's typically a 12–48h lag from arXiv posting to HF CDN availability.

## Fallback: arXiv HTML render

For papers published in the last ~6 months, arXiv now serves an HTML version at:

```
https://arxiv.org/html/<ARXIV_ID>v1
```

Individual figures are at:

```
https://arxiv.org/html/<ARXIV_ID>v1/x1.png
https://arxiv.org/html/<ARXIV_ID>v1/x2.png
https://arxiv.org/html/<ARXIV_ID>v1/x3.png
```

`x1.png` is Figure 1, `x2.png` Figure 2, etc. Resolution is typically 794×335 to 1200×800, file size 50–250 KB. Verify with `file` — the HTML render infrastructure occasionally returns placeholder PNGs for figures that didn't render.

## Fallback: arXiv PDF first page

If the HTML render is unavailable:

```
https://arxiv.org/pdf/<ARXIV_ID>v1
```

Render the first page to PNG with `pdftoppm` or `poppler-utils`. Lower quality, but always works:

```bash
curl -sLo /tmp/p.pdf "https://arxiv.org/pdf/2607.21553v1"
pdftoppm -f 1 -l 1 -r 96 -png /tmp/p.pdf /tmp/p
mv /tmp/p-1.png /home/arctic/.hermes/data/.../research_<slug>.png
```

## The verification gate

After every download, regardless of source:

```bash
file <path>.png
# Must say: "PNG image data, ..."

stat -c%s <path>.png
# Must be > 20480 bytes (20 KB)
```

If either check fails, the file is not a real image. Do not ship it. Either retry the next fallback or skip the `image` field in the JSON output.

## Worked example (the 2026-07-23 case)

| Paper ID | HF CDN | arXiv HTML | PDF page 1 | Resolution |
|----------|--------|------------|------------|------------|
| 2607.21553 | ✅ 389 KB | ✅ | ✅ | use HF |
| 2607.21557 | ✅ 278 KB | ✅ | ✅ | use HF |
| 2607.21356 | ❌ 504 | ✅ 193 KB | ✅ | use arxiv.org/html/x1.png |
| 2607.20911 | ✅ 184 KB | ✅ | ✅ | use HF |
| 2607.21540 | ✅ 192 KB | ✅ | ✅ | use HF |

The 504 was hit on a paper where HF had just indexed it but the CDN cache was stale. The arxiv.org/html fallback saved it.

## Don't retry the CDN more than twice

The 504 is not your network. It's HF's pipeline. Two attempts with `sleep 2` between is the maximum reasonable retry budget — after that, switch to the fallback. Spending more time on HF is wasted effort for a digest where you'll never ship the difference.

## Anti-patterns

- **Don't `xdg-open` or `feh` the result to "verify"** — you can't tell a real PNG from a 130-byte HTML by looking at file size alone. Always use `file`.
- **Don't trust HTTP status alone.** `curl -sI` (HEAD) on the CDN can return 200 while GET returns 504. Always do a full GET with `-sLo`.
- **Don't use the arXiv abs page screenshot.** The abs page has chrome that doesn't make a good newsletter thumbnail.
- **Don't skip the image field if every fallback fails.** It's better to omit the image than to ship a broken one. The consumer can render with a placeholder if needed.