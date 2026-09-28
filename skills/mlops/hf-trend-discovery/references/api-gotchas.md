# HF API Gotchas — verified Aug 2026

A condensed reference of the Hugging Face HTTP API surface for trend discovery. Each entry lists the trap, the actual returned behaviour, and the working alternative. **Probe every endpoint with `curl -sIL` before relying on it** — these are the surprises that bit me, not a complete catalogue.

## Endpoints that return 400 / 401 / 404

| Endpoint | What it returns | What to do instead |
|---|---|---|
| `GET /api/models?sort=trending` | `400 — Invalid sort parameter: trending` | `trending` is not a valid sort param. Allowed: `lastModified`, `downloads`, `createdAt`. To see the community trending list, scrape `/models?sort=trending` HTML. |
| `GET /api/models?sort=modifiedAt` | `400 — Invalid sort parameter: modifiedAt` | Use `lastModified` (the real field name) |
| `GET /api/datasets/{id}` (many repos) | `401 Unauthorized` | Gated/auth/rate-limited. Probe with `curl -sIL` first; fall back to scraping the rendered `/datasets/{id}` page. |
| `GET /api/blogPosts/{author}/{slug}` | `404` for most posts | Slug is not the bare basename. Fetch the rendered blog page and parse the JSON-LD `<script type="application/ld+json">` block — that's the canonical source. |
| `GET /changelog.json` | `404` | The changelog is JS-rendered. No API endpoint exists. Use the blog for Hub feature releases. |
| `GET /api/changelog` | `401 Invalid username or password` | Same — no API. Look for "<Partner> on Hugging Face Inference Providers" blog posts. |

## Endpoints that return WRONG data (silent failures)

| Endpoint | What it returns | What to do instead |
|---|---|---|
| `GET /api/papers?date=YYYY-MM-DD` | The same default listing for every date queried | Scrape `/papers/date/<YYYY-MM-DD>` HTML, then hit `/api/papers/{id}` per-paper for upvotes and `submittedOnDailyAt`. |

## The blog index has NO dates

`/blog` HTML renders titles and avatars but no published dates. To find posts in the 72h window:

```bash
curl -sL https://huggingface.co/blog | grep -oE 'href="/blog/[^"?]+' | sort -u > /tmp/blog_slugs.txt
for slug in $(cat /tmp/blog_slugs.txt); do
  url="https://huggingface.co/blog/$slug"
  date=$(curl -sL "$url" | grep -oE '"datePublished"[[:space:]]*:[[:space:]]*"[^"]+"' | head -1)
  echo "$date  $slug"
done | grep -E '2026-08-0[5-8]'
```

Each post page always carries the JSON-LD block with `datePublished`, `image`, and `author` fields — and the `image` field is the canonical hero thumbnail URL.

## Per-paper API fields worth knowing

`GET /api/papers/{arxiv_id}` returns:

```json
{
  "id": "2608.05466",
  "publishedAt": "2026-08-05T00:00:00.000Z",        // arxiv submission
  "submittedOnDailyAt": "2026-08-06T00:00:00.000Z", // when it hit the Daily Papers list
  "title": "...",
  "summary": "...",
  "upvotes": 212,
  "thumbnailUrl": null,
  "authors": [
    {"name": "Zhongzhi Li", "status": "claimed_verified", ...},
    ...
  ]
}
```

- **`publishedAt`** = arxiv submission date (often `T00:00:00.000Z` of the listing day — treat as a date, not a timestamp)
- **`submittedOnDailyAt`** = when the HF Daily Papers curator picked it up. Use this for "trending paper today" stories.
- **`thumbnailUrl`** is often `null` — the canonical paper thumbnail is `https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/{arxiv_id}.png` (340 KB), and that's the path to use for the story image.

## Per-model API fields worth knowing

`GET /api/models/{owner}/{repo}` returns:

```json
{
  "id": "deepgrove/maple-preview",
  "createdAt": "2026-08-04T20:50:54.000Z",    // first push
  "lastModified": "2026-08-04T20:51:36.000Z", // last commit (NOT a download count)
  "downloads": 686,
  "likes": 226,
  "pipeline_tag": "text-generation",
  "library_name": "transformers",
  "tags": ["transformers", "safetensors", "causal-lm", "mixture-of-experts", "reasoning", "ternary", "custom-code"],
  "siblings": [{"rfilename": "..."}, ...],
  "cardData": {...}
}
```

- **`createdAt`** + 0 downloads + high likes = fresh drop, major release
- **`lastModified`** in window + `createdAt` old = maintenance/refresh story (quant drop, paper version, license update)
- **`siblings[]`** lists every file in the repo — useful for spotting config files, model code (`modeling_*.py`), and pretrained weights
- **`cardData`** is the YAML front-matter of the model card — license, language, library_name, eval results

## Per-org search pattern (the first-party release signal)

```bash
# Recently modified models from a major lab
curl -sL "https://huggingface.co/api/models?author=nvidia&sort=lastModified&limit=10" | \
  python3 -c "import json,sys; [print(m['lastModified'], m['id'], m['downloads'], m['likes']) for m in json.load(sys.stdin)]"

# Newly created datasets
curl -sL "https://huggingface.co/api/datasets?author=nvidia&sort=createdAt&direction=-1&limit=10"

# Newly launched Spaces
curl -sL "https://huggingface.co/api/spaces?author=tencent&sort=createdAt&direction=-1&limit=10"
```

Watchlist orgs to seed these searches: `nvidia`, `mistralai`, `Qwen`, `deepseek-ai`, `moonshotai`, `zai-org`, `LGAI-EXAONE`, `microsoft`, `allenai`, `google`, `MiniMaxAI`, `baidu`, `LiquidAI`, `inclusionAI`, `deepgrove`, `black-forest-labs`, `Audio8`, `lodestones`, `Kwaipilot`, `thinkingmachines`, `tencent`, `bytedance`, `alibaba-pai`, `CohereForAI`, `google-deepmind`, `KIEFERSA`.

## The web_extract JSON passthrough is unreliable

`web_extract` may return content for JSON API endpoints that fails to parse as JSON — text wrapping, escape issues, or a non-JSON prelude. Pattern that works consistently:

```python
import json, urllib.request
url = "https://huggingface.co/api/models/deepgrove/maple-preview"
with urllib.request.urlopen(url, timeout=15) as r:
    data = json.loads(r.read())
```

For shell, `curl -sL ... | python3 -c "import json,sys; ..."` is the reliable pattern.

## Thumbnail discovery patterns

| Need | How to find it |
|---|---|
| Model social preview | `curl -sL https://huggingface.co/{org}/{repo} \| grep -oE 'og:image"\s+content="[^"]+"'` — extract the URL. The path is predictable: `social-thumbnails/models/{org}/{repo}.png`. |
| Paper thumbnail | Direct CDN: `https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/{arxiv_id}.png` (340 KB). |
| Blog post hero | `curl -sL https://huggingface.co/blog/{slug} \| grep -oE '"image":"[^"]+"'` — extract from JSON-LD. Path is post-specific (e.g. `blog/assets/inference-providers/welcome-baseten.png`). |
| Dataset banner | `https://huggingface.co/datasets/{org}/{repo}/resolve/main/assets/banner.png` — repo-local, may not exist. Fall back to the linked paper's thumbnail. |

Always verify the downloaded file with the PNG header check (`b'\x89PNG\r\n\x1a\n'`) and a size > 20 KB before locking it in.
