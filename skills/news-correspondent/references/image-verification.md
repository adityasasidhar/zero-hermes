# Image Verification & Source-Specific URL Patterns

This reference captures the CDN and press-kit patterns a leaf needs to
fetch real photos quickly. Every leaf should follow the recipe below
for the minimum `curl` + `file` + `stat` checks, then use the cheat
sheet for source-specific URL transforms.

## Verification recipe (canonical)

```bash
mkdir -p <asset_dir>
curl -L --fail -sS -o <target_path> "<cdn_url>"
file <target_path>                    # must NOT be HTML; JPEG, PNG, or WebP all pass
stat -c '%s' <target_path>            # must be > 20480 (~20KB)
```

The orchestrator accepts JPEG, PNG, and WebP. The previous "must say
`JPEG image data`" check was too strict — `assets.bwbx.io` (Bloomberg's
CDN) and various AP News CDN paths return WebP at full resolution, and
forcing them through `ffmpeg` to JPEG loses resolution for no
benefit. The new rule: `file` must report an image MIME (`JPEG image
data`, `PNG image data`, or `Web/P image`), and `file` must NOT
report `HTML document`, `ASCII text`, or `empty`. The 20KB size
threshold is unchanged. If `file` says "HTML document" you got a
404/login wall — try a different URL or a different source. If `stat`
is under 20KB it's a thumbnail — try a larger size parameter, or
move to a different CDN path.

**Optional content check (vision_analyze):** the `curl + file + stat`
recipe confirms the file is a real JPEG over 20KB, but not that you
downloaded the *right* image. Syndication chains (Getty → MARCA,
Reuters → Yahoo Finance, AP → local affiliates) sometimes re-host the
wrong photo, or attach a previous-game photo to a current-game caption.
After downloading a player-photo lead, run:

```python
vision_analyze(
    image_url=<local_path>,
    question="Who is in this photo? Is this <expected player> in <expected kit> at <expected venue>?"
)
```

If the model reports a different player, a different team kit, or an
empty stadium when you expected a packed one, re-download from a
different source. Cheap insurance against mis-attributed syndication
— particularly relevant for player-photo leads where the wrong subject
silently makes it into the paper.

For PNG files only (the renderer accepts JPEG and WebP natively):
if you specifically need a JPEG download, convert:

```bash
ffmpeg -y -i in.png -q:v 3 out.jpg
```

Skip this for WebP — the orchestrator accepts WebP directly.

## Source-specific URL patterns

### Getty Images

```text
https://media.gettyimages.com/id/<NUMERIC_ID>/photo/<slug>.jpg?s=612x612&w=gi&k=20&c=<HASH>=
```

The 612px variant comes in around 30–40KB — comfortably above the 20KB
threshold. The article page URL is
`https://www.gettyimages.com/detail/news-photo/.../<id>` and the
photographer is usually visible in the metadata table (look for
"Credit:" row).

### NVIDIA Newsroom (iPR Software)

NVIDIA press releases live on `nvidianews.nvidia.com/news/<slug>`. Images
are served from `iprsoftwaremedia.com` and the page lists several
sizes — the *cache-busted `_prv.jpg`* URL returns the largest usable
file (typically **1600x900, ~80–100KB**).

**Pattern:**

```text
https://iprsoftwaremedia.com/219/files/<YYYYMM>/<content-hash>/<size-slug>/<slug>_<cache-buster>-prv.jpg?v=<cache-buster>
```

Where:
- `<YYYYMM>` is the publication month (e.g. `202607`)
- `<content-hash>` is a short hex hash assigned by iPR
- `<size-slug>` is one of `_s`, `_mid`, `_l`, or no-suffix
- `<cache-buster>` is a UUID-like token (e.g.
  `c11a6442-e59a-4f26-b4a9-a08fb34f2181`)

**Size probe order (try the highest first):**

```bash
for suffix in "_prv.jpg?v=<cache>" "_mid.jpg?v=<cache>" ".jpg" "_s.jpg"; do
  STATUS=$(curl -A "Mozilla/5.0" -sI "<url-with-suffix>" | head -1 | awk '{print $2}')
  SIZE=$(curl -A "Mozilla/5.0" -sI "<url-with-suffix>" | grep -i content-length | awk '{print $2}' | tr -d '\r')
  echo "${suffix}: status=$STATUS, size=$SIZE"
done
```

**Discover URLs from a NVIDIA newsroom page:**

```bash
curl -A "Mozilla/5.0" -sL "https://nvidianews.nvidia.com/news/<slug>" \
  | grep -oE 'https?://iprsoftwaremedia\.com[^"]*' | sort -u
```

The page HTML often shows the filename *truncated in the middle* (e.g.
`sk-nvi..._prv.jpg`) — the actual filename is longer. Either copy
from the `og:image` meta tag (always full) or probe suffixes against
the iPR CDN with the right `?v=<token>`.

**Cache-buster is required.** Without the `?v=<token>` querystring the
iPR CDN may serve a 404 or an HTML error page for the `_prv` variant.
The token is stable for a given press release — copy it from any of
the other iPR image URLs on the same page.

### Hugging Face CDN thumbnails

Reliable as `card_art` for HF desk stories; usable as lead photo for
research beat when no paper figure is available.

| Asset | URL pattern | Notes |
| --- | --- | --- |
| Paper thumbnail | `https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<arxiv_id>.png` | 1200x648, ~250–400KB PNG; convert to JPEG |
| Model thumbnail | `https://cdn-thumbnails.huggingface.co/social-thumbnails/models/<org>/<repo>.png` | 1200x648, ~250–400KB PNG; convert to JPEG. Verify with `curl -sI` first — private/deprecated models return a small stub |
| Dataset thumbnail | `https://cdn-thumbnails.huggingface.co/social-thumbnails/datasets/<org>/<repo>.png` | Same conventions as models. |

**Trap:** the URL segment uses the literal `org/repo` pair, not a
URL-encoded form. Casing matters — `moonshotai/Kimi-K3.png` works,
`MoonshotAI/kimi-k3.png` may 404 silently. Always `curl -sI` first
to confirm `200 OK` and `content-length > 20000` before downloading.

### OpenAI Contentful CDN

```text
https://images.ctfassets.net/kftzwdyauwt9/<asset-id>/<hash>/<filename>.png?w=<width>&q=<quality>
```

Default curl works without UA spoofing. Width `w=2400` and quality
`q=80` returns a ~400KB image. Files are typically PNG even when the
URL ends `.png`; convert with `ffmpeg`.

**Discover image URLs from any OpenAI post:**

```bash
curl -A "Mozilla/5.0" -sL "https://openai.com/index/<slug>/" \
  | grep -oE 'https://images\.ctfassets\.net/[^"]*'
```

### TechCrunch uploads

```bash
curl -A "Mozilla/5.0" -sSL "<tc-article-url>" \
  | grep -oE 'https://techcrunch\.com/wp-content/uploads/[^"]*\.(jpg|jpeg|png|webp)' \
  | sort -u
```

The first URL is usually the hero photo. Width variants via
`?resize=W,H` produce smaller copies.

### MARCA (English edition)

```text
https://e00-xlk-ue-marca-en.uecdn.es/uploads/<YYYY>/<MM>/<DD>/<hash>.jpeg
```

MARCA's English edition CDN serves direct JPEGs at 1980x1320 (~400–500KB).
Default `curl -L -A "Mozilla/5.0"` works without auth. Files come through
with paint.net EXIF metadata so `file` reports `"paint.net 5.0.13"` in the
EXIF segment — that's normal, not a corruption sign. The article URL slug
encodes the publish date (`/YYYY/MM/DD/<slug>.html`) — use it to pin
`published_at`.

**Trap — credit laundering:** MARCA (and AS, Olé) almost always re-host
Getty Images without changing the URL or the photographer credit. The
original photographer attribution lives on the Getty caption page, not
on the MARCA article body. If you need a precise credit string, follow the
image back to its Getty asset page; otherwise credit "Getty via MARCA"
or just the syndicator. Don't invent a photographer name from the URL
hash.

**Use case:** MARCA English often has fresher editorial framing on
Spanish-speaking players (Messi, Vinícius, Yamal) than the English wire,
and the English CDN is faster to scrape than MARCA Spanish. Worth a
parallel `web_search` for Messi/AFA/La Liga stories.

### AMD Investor Relations (Q4 hash CDN)

```text
https://d1io3yog0oux5.cloudfront.net/_<q4hash>/amd/db/<id>/image_resized.jpg
```

These are the AMD Q4 IR CMS images. Default curl works.

### GlobeNewswire / PR Newswire

```text
https://ml.globenewswire.com/Resource/Download/<id>.<ext>
```

Returns 204 with empty body for gated assets. Always run `file` after
download; empty body means the asset is gated — fall back to the
press release HTML itself or the company's own CDN.

### AP News (`assets.apnews.com`)

AP News articles publish real photos at the canonical asset URL:

```text
https://assets.apnews.com/<XX>/<YY>/<content-hash>/<hash-or-no-tail>
```

These URLs serve the **original full-resolution image** (typically
4–6000px wide, 5–10MB), work without UA spoofing, and skip the
`dims.apnews.com` resize CDN entirely. They are reachable from the
AP News article HTML two ways:

1. **Direct grep** — the asset hash appears verbatim in the article's
   `<picture>`/`<img>` markup, e.g.
   `https://assets.apnews.com/87/a1/bc84dcd5ce07dd2107e3792a3c7c/832b72d01c5249bf91848fc9f77baaab`.
2. **Decoded from `dims.apnews.com` resize URLs** — the `?url=` query
   parameter is a URL-encoded `assets.apnews.com` URL. The
   `dims.apnews.com` resize variants often return 350x350 placeholder
   thumbnails when the resize variant isn't built yet; extracting the
   `?url=` value gives you the real image.

**Recipe:**

```bash
curl -A "Mozilla/5.0" -sSL "https://apnews.com/article/<slug>" \
  | grep -oE 'https://assets\.apnews\.com/[^"]+\.(jpg|jpeg|png)' \
  | sort -u
```

The downloaded file's EXIF `Description` field usually carries the
caption text (e.g. `"Masayoshi Son, Chairman and CEO of SoftBank Group
Corp., left, and Howard Lutnick..."`) — that string is the **exact
photographer/credit** to write into `art_credit`, plus it confirms you
have the right image (not a stock substitute).

**Trap:** the `dims.apnews.com` CDN can be misleading. URLs like
`https://dims.apnews.com/dims4/default/087a28b/.../resize/980x653/...`
look like the photo but return a 350x350 grayscale placeholder when
the resize variant isn't built yet. Always probe the direct
`assets.apnews.com/<hash>/` URL, never trust the resize URL
unless you've confirmed a real pixel-sized download with `file`.

### Yahoo Finance syndicated Reuters photos (BLOCKED)

Yahoo Finance syndication (`ca.finance.yahoo.com/news/...`,
`finance.yahoo.com/news/...`) renders Reuters article text but the
accompanying Reuters Connect photos are gated behind a Yahoo login
wall. Two attempts both fail:

1. The Yahoo image CDN URL
   `https://s.yimg.com/ny/api/res/1.2/.../media.zenfs.com/en/reuters.com/<hash>`
   returns an 11-byte ASCII stub (the redirect/login HTML).
2. The page itself redirects to the login flow even when curled with a
   browser UA — Yahoo treats Reuters Connect content as premium.

**Workaround:** find the same event photo on an unsyndicated third
party. For Anduril Farnborough coverage, use the original photo on
`defensenews.com`, the drone-celebrity outlet that shot it, or a press
release from Anduril's own site. For general Reuters wire photos, try
`reutersconnect.com` directly (sometimes open), `defensenews.com` /
`breakingdefense.com` (defense beat), or the vendor's own press kit.
Failing that, generate with `mmx image generate` rather than ship a
gated image.

**Symptom:** `curl` returns `200 OK` with content-length ~11, `file`
says `ASCII text`. Move on, don't waste cycles probing size variants.

### MLS / Inter Miami (Cloudinary-served)

`images.mlssoccer.com` Cloudinary pipeline returns JPEGs. Pattern of
the URL is arbitrary, but the file size is reliable. If a size
modifier exists (`?w=...&h=...`), use the larger one. Same for
`assets1.afa.com.ar`.

**Whose image is it?** The Cloudinary URL alone is anonymous — pair it
with the photographer credit visible in the surrounding article HTML
or the credit overlay on the photo itself. A bare `images.mlssoccer.com`
URL with no credit is a sourcing hole; reject it before downloading.

### The Athletic (`static01.nyt.com`)

The Athletic's CDN serves image objects at:

```text
https://static01.nyt.com/athletic/uploads/wp/<YYYY>/<MM>/<DDHHMMSS>/<filename>
```

The path encodes the **publish time in UTC** — the `DDHHMMSS` segment
is the article upload timestamp. This is gold for window pinning: a URL
like `/2026/07/26070632/` is July 26 at 07:06:32 UTC, which means the
story is in any window whose cutoff is after that.

Default `curl -L --fail -sS` works without UA spoofing. Bump resolution
and quality with query params:

```text
?width=1600&quality=80&auto=webp
```

`width=1600` returns ~100–200 KB JPEGs (the CDN re-encodes), well
above the 20 KB verification threshold. The `auto=webp` parameter is
honoured on download — pass `--output-extension` if you're piping to
`file` (it returns `JPEG image data` because the byte stream is JPEG
despite the parameter).

**Trap:** the image's filename and dimensions are arbitrary — a
portrait crop and an action shot can both live under the same publish
timestamp directory. Pick the right image by reading the figure caption
in the article body, not by guessing from the URL.

**Discover all images from an Athletic article:**

```bash
curl -A "Mozilla/5.0" -sL "<athletic-article-url>" \
  | grep -oE 'https://static01\.nyt\.com/athletic/uploads/[^"]*\.(jpg|jpeg|png|webp)' \
  | sort -u
```

The caption / credit string lives in the surrounding HTML as a
`<p class="caption">` or as inline text below the `<figure>`. Always
read the caption — it carries the photographer attribution you'll put
in `credit`.

### Bloomberg CDN (`assets.bwbx.io`)

Bloomberg article hero images live on `assets.bwbx.io` and serve at
**WebP** (not JPEG), at 2000×1334 or larger. The orchestrator accepts
WebP; if you downloaded a real WebP ≥ 20KB from this CDN, the
verification already passed — do not re-encode to JPEG.

```text
https://assets.bwbx.io/images/users/<userid>/<assetid>/v<int>/<-1x-1|...>.webp
```

Discovery recipe from a `bloomberg.com/news/articles/<id>` URL:

```bash
curl -A "Mozilla/5.0" -sL "<bloomberg-article-url>" \
  | grep -oE 'https://assets\.bwbx\.io/images/[^"?]+\.(jpg|jpeg|png|webp)' \
  | sort -u
```

The first URL is usually the `<img>`-tagged hero image. The
`vi=-1x-1` segment is a cache-buster, not a size modifier — the
returned image is always full-resolution. Photographer credit
appears in the page `<meta property="article:author">` or as a
caption under the image; if neither is parseable, credit as
`"Bloomberg / <first-credit-string-visible>"` or just `"Bloomberg"`.

**Trap:** the bare domain `www.bloomberg.com/news/articles/<id>` is
**gated** (returns 13-byte HTML stub to `curl`). The image CDN
itself is open. Always use the `assets.bwbx.io` URL, never the article
page as the `image.source_url`.

### Al Jazeera (Reuters-embedded via `wp-content/uploads`)

Al Jazeera re-hosts Reuters Pictures' photo IDs directly on its
own WordPress CDN. The URL pattern is:

```text
https://www.aljazeera.com/wp-content/uploads/<YYYY>/<MM>/<YYYY-MM-DD>T<HHMMSS>Z_<vendorid>_<pictureid>_<...>_<caption-slug>.jpg?<resize-and-quality>
```

Example:
`https://www.aljazeera.com/wp-content/uploads/2026/08/2026-03-05T153442Z_489233570_RC29YJAEJOQF_RTRMADP_3_TECH-USA-GERMANY-GOOGLE-1786017171.jpg?resize=770%2C513&quality=80`

The picture ID (`1786017171` in the example) is the Reuters
Pictures catalogue number — the same ID is reachable on
`reutersconnect.com` and is the canonical source. The filename
prefix (`2026-08-05T153442Z`) is the **Reuters capture time**,
not the Al Jazeera upload time — useful for the `published_at`
anchor when the article body is the source.

**Recipe:**

```bash
curl -A "Mozilla/5.0" -sL "<aljazeera-article-url>" \
  | grep -oE 'https://www\.aljazeera\.com/wp-content/uploads/[0-9]{4}/[0-9]{2}/[^"?]+\.jpg' \
  | sort -u
```

`?resize=770,513` or `?resize=1920,1080` are honoured — the larger
one returns ~60–120KB and easily passes the 20KB threshold. Credit
as `"Reuters / <Al Jazeera photographer>"` if the caption names
one, otherwise `"Reuters via Al Jazeera"`.

**Trap:** the `YYYY-MM-DD` segment in the filename is the **capture
date**, not the article date. A photographer shooting on Mar 5
2026 may file the picture that day and Al Jazeera may not publish
the article until August. Always use the article's published date
in `published_at`, not the filename date.

### Filename-encoded timestamps (general technique)

Several publishers put the publish time directly into the image URL.
Use this to **pin a story's `published_at` to the minute**, not just
to the day:

| Publisher | URL segment | Decodes as |
| --- | --- | --- |
| The Athletic (`static01.nyt.com/athletic/uploads/wp/`) | `YYYY/MM/DDHHMMSS` | UTC upload timestamp |
| Inter Miami / MLS Cloudinary | `WhatsApp Image YYYY-MM-DD at H.MM.SS PM` in filename or alt text | Local (EDT) capture time |
| NVIDIA iPR | `YYYYMM` in the path (e.g. `202607`) | Publication month only |

When a URL tells you the minute, you don't need to scrape the page for
`script[type="application/ld+json"]` — the URL IS the timestamp.
Cross-check by reading the article body for any timezone hint and
adjust to UTC before writing `published_at`.

## Common failure modes

| Symptom | Cause | Fix |
|---|---|---|
| `file` says `HTML document` | URL went to a 404 or paywall page | Re-check the URL; try the article detail page instead |
| `stat` shows < 5KB | Got a thumbnail or sprite | Bump the size parameter (`?s=2048x2048` on Getty, `_prv.jpg?v=<token>` on NVIDIA iPR) |
| 403 Forbidden | Getty is enforcing token auth | Use the `media.gettyimages.com/id/` form, not the `www.gettyimages.com/detail/` form |
| 999 / 401 | Rate-limited | Wait, or use a different image source |
| 404 on iPR `_prv.jpg` | Missing the `?v=<token>` cache-buster | Re-grep the press release page for an iPR URL, copy the full querystring |
| NVIDIA iPR page shows `sk-nvi..._prv.jpg` truncated | iPR truncates display HTML | Read the `og:image` meta tag instead — it always has the full URL |

## Don't forget the credit

The `art_credit` field is the photographer string as it appears on the
original page, not the agency name. For press-kit logo images (NVIDIA
iPR, Hugging Face CDN), the credit is the joint attribution between
the two companies named in the URL (e.g. "NVIDIA / Safe
Superintelligence Inc."), not "Getty Images" and not the filename
slug.
