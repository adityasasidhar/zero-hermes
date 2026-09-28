# Figures and Thumbnails for arXiv Papers

When you need an image to accompany an arXiv paper in a report, newsletter, or beat package, the usual candidates are (in order of reliability):

## 1. arXiv HTML page figures — **canonical fallback, recommended**

Most modern papers (cs.AI, cs.CL, cs.LG, cs.CV from 2025 onward) have an HTML rendering at `https://arxiv.org/html/<id>v1`. The figures live under `extracted/...` or inline as `<img src="<id>v1/x1.png">`.

### How to find the right figure

```bash
# List all image sources in the rendered HTML page
curl -sS -L --max-time 30 "https://arxiv.org/html/<ID>v1" \
  | grep -oE '(src|href)="[^"]*\.(jpg|png|jpeg|gif)"' \
  | grep -vE 'icon|logo|static|favicon' \
  | head -10
```

Typical figure file patterns:

- `<id>v1/x1.png`, `x2.png`, ... — auto-generated numbered figures (usually the first one is the framework/teaser)
- `<id>v1/figures/framework.png` or `<id>v1/figures/<name>.png` — named figures (often the main architecture diagram)
- `<id>v1/fig1_*.png`, `fig2_*.png`, ... — manually named figures

### How to download

```bash
curl -sS -L --max-time 45 -o "figure.png" \
  "https://arxiv.org/html/<ID>v1/x1.png"
file --brief --mime-type figure.png   # must be image/png or image/jpeg
```

If the file starts with `<` it's an error page (HTML 404), not an image — try the next figure in the listing.

## 2. Hugging Face paper thumbnails — **often flaky**

Hugging Face hosts social-card-style thumbnails at:

```
https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png
```

They look great (1200×648 PNGs, ~400KB), but the CDN is **intermittently unreliable** — `504 Gateway Timeout`, `curl: (28) Failed to connect`, or hangs indefinitely. Empirically observed in 2026: timeouts on every attempt across 5 consecutive papers. Do not block on them.

### Workaround

Set a strict timeout (~10s) per request and **fall back to arxiv.org/html page figures immediately** if it fails:

```bash
timeout 10 curl -sS -L -o thumb.png -w "%{http_code}\n" \
  "https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png"
# If HTTP != 200 OR file is not a valid PNG, use arxiv.org/html fallback
```

## 3. Generated images — **last resort**

If neither source works (rare — almost always one of the above succeeds for any paper submitted in the last ~3 years), generate a hero image with `mmx image-01`, `bfl_flux3_*`, or similar. Reserve this for cover/hero use, not per-paper figures.

## Size requirement (newsletter / beat packages)

If the consumer of the image requires `>20KB image/png` or similar, verify with:

```bash
file --brief --mime-type figure.png
ls -la figure.png   # size in bytes
```

Real arXiv figures are typically 100KB–2MB. If your file is < 20KB it's probably an error page disguised as an image.

## Canonical URL patterns (for source attribution)

When crediting an image in a publication:

- arXiv HTML figure: `https://arxiv.org/html/<id>v1/<figure-filename>` (use the exact path you downloaded from)
- HF thumbnail: `https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png`
- Generated: source the prompt/model

Credit format: `arXiv <id> / <first author> et al.`

## Hugging Face daily papers as a discovery layer

Because arXiv listings lag 2–3 days, when you need **same-day coverage** of new papers, supplement arXiv with:

```
https://huggingface.co/papers
https://huggingface.co/papers?q=<topic>
```

HF's daily papers feed surfaces submissions within ~24h of arXiv posting, often before the arXiv listing API reflects them. Cross-reference the arXiv ID (visible on each HF paper page) when writing up the story, and pull the figure from `arxiv.org/html/<id>v1` as described above.