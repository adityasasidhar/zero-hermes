---
name: hf-trend-discovery
description: "HF trend discovery for news: trending, papers, blog, API."
version: 1.1.0
author: Hermes Agent
license: MIT
tags: [huggingface, hf, news, journalism, trend-discovery, api, thumbnails]
platforms: [linux, macos]
---

# Hugging Face Trend Discovery for News Reporting

Treat HF as its own beat, not an appendage to "Research" or "Models." When asked for the last 72 hours of Hugging Face activity, you need a repeatable recipe: pull trending surfaces, dedupe across old + new, anchor to timestamps, and ground every claim to a `huggingface.co/...` URL.

## The 7 surfaces, ranked by builder-relevance

When the user asks for "what would an ML builder click on huggingface.co today", check in this order. Each surface has a different audience and cadence.

1. **HF blog** (`/blog`) — official HF + partner announcements. Highest editorial weight. The blog index page contains NO dates in its HTML — you must fetch each post link and parse the JSON-LD `datePublished` block.
2. **Trending models** (`/models?sort=trending`) — what the community is currently downloading. The HTML page is the only source — the API endpoint `?sort=trending` returns 400. Cross-check `lastModified` vs `createdAt`: old model + recent mod = still hot, worth a story. New model + recent mod = fresh drop.
3. **Daily papers** (`/papers`, `/papers/date/<YYYY-MM-DD>`) — academic front door. The API endpoint `?date=YYYY-MM-DD` returns the same listing regardless of date — scrape the HTML page instead, then hit `/api/papers/{id}` per paper for upvotes and dates.
4. **Trending datasets** (`/datasets?sort=trending`) — corpus drops. Always check the dataset card, the splits, and the `downloads` count. The `/api/datasets/{id}` endpoint commonly returns 401 — probe with `curl -sIL` and fall back to the rendered page.
5. **Trending spaces** (`/spaces?sort=trending`) — interactive demos. Lower editorial weight unless the space is a flagship from a major lab. Use `/api/spaces?author={org}&sort=createdAt&direction=-1` to find freshly launched Spaces.
6. **HF changelog** (`/changelog`) — Hub feature releases. **Fully JS-rendered with no API.** Don't waste time probing `/api/changelog` (401) or `/changelog.json` (404). The blog is the canonical place for Hub feature announcements (e.g. "X on Hugging Face Inference Providers" pattern).
7. **HF organization timelines** (nvidia, mistralai, zai-org, Qwen, allenai, upstage, etc.) — first-party release signals. Use `/api/models?author={org}&sort=lastModified&limit=N` to find recent activity; cross-check with `createdAt` for fresh drops.

## The HTTP API quick reference

No auth required for read-only. JSON responses.

| Endpoint | Returns | Use it for |
|---|---|---|
| `GET /api/models/{repo_id}` | `createdAt`, `lastModified`, `downloads`, `tags`, `cardData`, `siblings[]` | Verify a model exists, get its story date |
| `GET /api/datasets/{repo_id}` | same + `datasetsServerInfo` | Verify a dataset. **Commonly returns 401** for gated/auth-required repos — probe with `curl -sIL` first and fall back to `https://huggingface.co/datasets/{repo_id}` |
| `GET /api/spaces/{repo_id}` | same shape | Verify a space |
| `GET /api/papers/{arxiv_id}` | `publishedAt`, `submittedOnDailyAt`, `title`, `summary`, `authors`, `upvotes` | Confirm paper date. `submittedOnDailyAt` = when HF added it to the daily list (distinct from `publishedAt` = arxiv submission). |
| `GET /api/papers?date=YYYY-MM-DD` | daily paper list | **BROKEN: returns identical default data regardless of date** — scrape the HTML page instead |
| `GET /api/blogPosts/{author}/{slug}` | full post body | **Often 404s** — the slug is not the bare basename. The reliable path is to fetch the rendered blog page and parse the JSON-LD `datePublished` block. |
| `GET /api/models?author={org}&sort=lastModified&limit=N` | org's recently-modified models | First-party release signal — better than `?sort=trending` |
| `GET /api/models?sort=trending&full=true` | — | **Returns 400** — `trending` is not a valid sort param. Only `lastModified`, `downloads`, `createdAt` are accepted in the `sort` field. To see the community trending list, scrape `/models?sort=trending` HTML. |
| `GET /api/spaces?author={org}&sort=createdAt&direction=-1&limit=N` | org's newest Spaces | Find recently-launched Spaces from a major lab |
| `GET /api/datasets?author={org}&sort=createdAt&direction=-1&limit=N` | org's newest datasets | Same pattern for datasets |

## Timestamp semantics — pick the right one

| Field | Meaning | When to use for "is it in the window?" |
|---|---|---|
| `createdAt` | Repo first pushed | Fresh drops only |
| `lastModified` | Last commit | Still-active models with recent updates |
| `publishedAt` (papers) | HF indexed the paper (often same day as arxiv) | Always |
| `submittedOnDailyAt` (papers) | When the paper hit the Daily Papers list | When the story is "this paper is trending on HF today" |
| Blog JSON-LD `datePublished` | First published | Always |
| `downloadsAllTime` | Cumulative | Never as a "in window" signal |

**Rule of thumb:** if `lastModified` is in the 72-hour window, the model is worth a story even if `createdAt` is months old — that means the maintainer is actively shipping.

**Paper dating rule:** `publishedAt` is often `T00:00:00.000Z` of the listing day (just a date, not a timestamp). `submittedOnDailyAt` is when the Daily Papers curator picked it up. For "trending paper today" stories, `submittedOnDailyAt` is the more relevant hook.

## Thumbnail URL patterns (the asset problem)

You always need a picture. The CDN is reliable and needs no auth.

| Asset type | URL pattern | Typical size | PNG? |
|---|---|---|---|
| Model social card | `https://cdn-thumbnails.huggingface.co/social-thumbnails/models/{org}/{repo}.png` | 400–450 KB | yes |
| Paper social card | `https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/{arxiv_id}.png` | 200–340 KB | yes |
| Dataset banner | `https://huggingface.co/datasets/{org}/{repo}/resolve/main/assets/banner.png` | varies, 1–2 MB if present | yes |
| Blog post hero | `https://huggingface.co/blog/assets/<slug>/<file>.png` (path is post-specific) | 100–500 KB | yes — extract from the page's JSON-LD `image` field or `<meta property="og:image">` |
| Org avatar | `https://cdn-avatars.huggingface.co/v1/production/uploads/...` | 3–5 KB | yes — **TOO SMALL, don't use for stories** |

**Model social preview discovery:** The model page HTML carries the canonical preview URL in `<meta property="og:image" content="https://cdn-thumbnails.huggingface.co/social-thumbnails/models/{org}/{repo}.png">`. If the CDN URL 404s for a fresh repo, fall back to scraping the model page for the OG image.

**Blog post hero discovery:** The blog post HTML carries the canonical hero URL in the JSON-LD `image` field (e.g. `https://huggingface.co/blog/assets/inference-providers/welcome-baseten.png`). That field is present on every post — use it instead of guessing the path.

Always verify the downloaded file is a real PNG (`head.startswith(b"\x89PNG")`) and > 20 KB. The org-avatars pattern will pass the PNG check but fail the size check; a future agent will be tempted to use them and produce a 4 KB image.

## The 72-hour-window verification loop

```
1. Pull the HTML for /models?sort=trending, /papers/date/<TODAY>, /datasets?sort=trending,
   /spaces?sort=trending, /blog. The API is unreliable for the date-filtered and trending endpoints.
2. For each candidate model/dataset/paper, hit the /api/{models,datasets,spaces,papers}/{id}
   endpoint and grab timestamps. Sort by createdAt / submittedOnDailyAt / JSON-LD datePublished.
3. For org-targeted searches, use /api/models?author={org}&sort=lastModified&limit=N to find
   recently active major-lab repos.
4. For each blog candidate, fetch the rendered post page and extract datePublished from the
   JSON-LD <script type="application/ld+json"> block — the /blog index has no dates.
5. Sort candidates by relevance score:
   - new + in window + high community interest (likes/downloads) = highest
   - in window + brand-name org = high
   - in window + has Spaces using it = higher (builder magnetism)
   - old but recently modified + trending = medium (maintenance story)
   - blog post in window from HF or major partner = high (editorial)
6. Build stories ranked by relevance, not by source surface.
```

## The blog date extraction recipe

The `/blog` index page renders titles and avatars but NO dates. To get all blog posts in the 72-hour window:

```bash
# 1. Extract unique blog link slugs from the index
curl -sL https://huggingface.co/blog | grep -oE 'href="/blog/[^"?]+' | sort -u

# 2. For each slug, fetch the rendered page and pull datePublished from JSON-LD
for slug in <slugs>; do
  curl -sL "https://huggingface.co/blog/$slug" | \
    grep -oE '"datePublished"[[:space:]]*:[[:space:]]*"[^"]+"'
done

# 3. Filter to entries within the window (e.g. >= 2026-08-05)
```

The JSON-LD block is always present on blog posts and is the canonical published date. The same block also carries the `image` field for the hero thumbnail.

## Pitfalls (learned the hard way)

- **Don't trust the trending list alone.** Trending surfaces are updated continuously; an old model with a fresh `lastModified` will sit on the list for days. Always check `createdAt` vs `lastModified` to know whether you're writing a "new release" story or a "still hot" story.
- **The `/api/papers?date=YYYY-MM-DD` endpoint is unreliable for date filtering.** It returns the same default listing (the same handful of papers with identical `publishedAt`) regardless of the date supplied — no 400, no error, just the wrong data. **Don't trust its results.** Use the **public HTML page** `https://huggingface.co/papers/date/<YYYY-MM-DD>` instead, then scrape arxiv IDs and titles from the `href="/papers/<id>"` anchors. Once you have the IDs, hit `/api/papers/{id}` (the per-ID endpoint) to get upvotes, the canonical `publishedAt`, AND `submittedOnDailyAt` — that endpoint *is* reliable.
- **The `/api/models?sort=trending` endpoint returns 400.** `trending` is not a valid sort parameter. The only way to see the community trending list is to scrape the HTML at `/models?sort=trending`. For programmatic org-targeted searches, use `?sort=lastModified` instead.
- **The `/api/blogPosts/{author}/{slug}` endpoint 404s for many posts** — the slug format is not the bare post basename. Don't trust this endpoint; fetch the rendered blog page and parse the JSON-LD block.
- **The `/api/datasets/{id}` endpoint returns 401 for many datasets** — gated repos, auth-required content, or rate-limit responses. Probe with `curl -sIL` first; if 401, use the rendered page (`/datasets/{id}`) and parse the metadata from the HTML.
- **The HF changelog (`/changelog`) is fully JS-rendered.** There is no server-side API endpoint: `/api/changelog` returns 401, `/changelog.json` returns 404. For Hub feature releases, the blog is the canonical signal (look for "X on Hugging Face Inference Providers" pattern from the partners).
- **HF daily listings lag real time by ~12–24h.** Querying `/papers/date/2026-08-01` on Aug 1 UTC returned the same listing as `/papers/date/2026-07-31`. The Jul 31 listing (38 papers) was the freshest batch until HF rotated. When the user says "last 72 hours" and today is "Aug 1," assume the freshest in-window listing is yesterday's, not today's. Cross-check the arxiv ID's `<meta name="citation_date">` (on `arxiv.org/abs/<id>`) to confirm the actual submission date — HF's `publishedAt` is just a date (`T00:00:00.000Z`), not the submission hour.
- **Fresh-drop signal: 0 downloads + high likes + recent `createdAt` = major release, not noise.** DeepSeek-V4-Flash-0731 hit 980 likes and 0 downloads within 24 hours of `createdAt = 2026-07-31T07:30:24Z`. A brand-new repo from a major lab with hundreds of likes and near-zero downloads is a *just-dropped* model — that's a rank-1 story, not a data quality issue. Cross-check `createdAt` is genuinely fresh (within 24–48h) and the org is one of the watchlist labs (deepseek-ai, moonshotai, Qwen, zai-org, microsoft, mistralai, allenai, poolside, etc.) before treating as rank-1.
- **Blog URL slugs are not predictable.** `cosmos3edge` works, but `introducing-cosmos-3-edge` 404s. Always probe with `curl -sIL` first. The reliable extraction is from the `/blog` index via `grep -oE 'href="/blog/[^"?]+'`.
- **Paper `publishedAt` is often `T00:00:00.000Z` of the listing day**, not the arxiv submission time. Treat it as a date, not a timestamp. For "when did it hit the daily papers list" use `submittedOnDailyAt`.
- **Org avatars are NOT model previews.** If you see a 3–5 KB PNG, you've got the org logo, not the model. Re-download from the social-thumbnails CDN.
- **Dataset thumbnails are repo-local.** No CDN. If `assets/banner.png` is missing on the dataset repo, you have nothing. Fall back to the paper thumbnail of a related paper, or skip the image.
- **The web_extract JSON passthrough is unreliable.** For JSON API endpoints like `/api/models/{id}`, `web_extract` may return content that fails to parse as JSON (text/plain wrapping, escape issues). Use `curl -sL` directly and `json.loads()` for API endpoints.

## Output shape for HF news stories

Every story in a "HF in the last 72h" brief should carry:

- `rank` (1-5, ordered by ML-builder click-likelihood)
- `headline` (6–12 words, declarative, includes the model/blog name)
- `dek` (15–25 words, why a builder cares — lead with the technical hook, not the org name)
- `body` (2–3 paragraphs, ~150 words, include at least one concrete number/benchmark)
- `source_url` — the canonical huggingface.co/<page> (not a CDN, not an arxiv link)
- `published_at` (ISO 8601 UTC, the actual timestamp you verified from the API)
- `source_name` (e.g. "Mistral AI via HF", "Hugging Face", "Upstage via HF")
- `image` (downloaded path, byte count, format — null only if no usable PNG exists)

## Related support files

- `references/api-endpoints.md` — exhaustive list of HF HTTP API endpoints with example JSON shapes.
- `references/api-gotchas.md` — condensed table of endpoints that return 400/401/404 or silently wrong data, with verified working alternatives (Aug 2026).
- `references/blog-date-extraction.md` — the regex + curl loop for pulling all blog post dates from `/blog`.
- `templates/story-template.json` — JSON skeleton for one HF news story.
