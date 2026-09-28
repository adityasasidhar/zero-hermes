# Hugging Face Beat — Discovery & Image Patterns

The HF beat's discovery is concentrated on `huggingface.co` itself (not external news outlets). The leaf should hit these pages first, then cross-reference the originating labs' own channels for timestamps and primary statements.

## Primary discovery pages

| Surface | URL | What it shows |
|---|---|---|
| Daily papers | `https://huggingface.co/papers` | AK's curated top 10–12 trending papers, refreshed daily. Single best signal for "what ML papers are getting attention today" |
| Daily papers (per-day) | `https://huggingface.co/papers/date/<YYYY-MM-DD>` | The full per-day listing (30–40 papers) as rendered HTML. **Use this for date-anchored discovery** — the `/api/papers?date=` endpoint is broken (returns identical default data regardless of date); scrape the HTML instead. |
| Trending models | `https://huggingface.co/models?sort=trending` | New uploads and big repos rising on the Hub |
| Trending datasets | `https://huggingface.co/datasets?sort=trending` | New dataset drops |
| Changelog | `https://huggingface.co/changelog` | Hub feature releases (MCP server, new filters, new tools, token presets) |
| HF Blog | `https://huggingface.co/blog` | In-depth lab releases written by HF staff, with a model or dataset lead |
| Trending Papers (7-day) | `https://huggingface.co/papers/trending` | Rolling 7-day list, useful for catching missed stories |
| Monthly papers | `https://huggingface.co/papers/month/2026-07` | All curated papers for the current month |

**Discovery order:** daily papers → trending models → changelog → HF blog. That order surfaces 4–5 candidate stories in 3–4 minutes, which is the bulk of the 12-minute HF beat budget.

## Lab channels worth checking (most post their HF first)

The HF model card often goes up *before* the lab's blog post, so cross-reference timing by checking both. The leaf should already be on these org pages from the trending models pull.

| Lab | HF org | Own blog / X |
|---|---|---|
| Moonshot AI | `huggingface.co/moonshotai` (Kimi line) | `kimi.com/blog`, `@Kimi_Moonshot` |
| Mistral AI | `huggingface.co/mistralai` | `mistral.ai/news` |
| Microsoft Research | `huggingface.co/microsoft` | `microsoft.com/en-us/research` |
| DeepSeek | `huggingface.co/deepseek-ai` | `github.com/deepseek-ai` |
| Alibaba Qwen | `huggingface.co/Qwen` | `qwenlm.github.io` |
| Zhipu / Z.ai | `huggingface.co/zai-org` (also `THUDM`) | `z.ai` |
| Allen AI | `huggingface.co/allenai` | `allenai.org` |
| Thinking Machines | `huggingface.co/thinkingmachines` | `thinkingmachines.ai/news` |
| Poolside | `huggingface.co/poolside` | `docs.poolside.ai/release-notes` |
| Baidu | `huggingface.co/baidu` | — |
| MiniMax (Hailuo AI) | `huggingface.co/MiniMaxAI` — omni-modal video + audio; ships H3, M3, VTP encoders; ~90 team members on the org page, well-staffed, releases `MiniMax` Articles on the Hub blog | `minimax.io`, `hailuoai.video` (app), `platform.minimax.io` (API) |
| Nanbeige LLM Lab | `huggingface.co/Nanbeige` — compact (3B/4B) agentic models with LoopSplit, mHC, n-gram embeddings; ships vLLM/SGLang/llama.cpp/Ollama branches under `Nanbeige/<fork>.git -b <branch>` | `nanbeige@kanzhun.com`, technical reports on arXiv |
| ByteDance | papers only — no first-party HF org for most releases; entries surface via HF daily papers under ByteDance / ByteDance-Seed / Seed avatars | `bytedance.com` |

**Note on identifying a real Hub-native lab vs a thin alias:** a synthetic / squat org sometimes shows up at the top of trending with thousands of likes on a single model and otherwise no followership. **Quick legitimacy test:** open `huggingface.co/<org>` and verify (a) ≥ 5 distinct repos with varied `createdAt` dates spanning weeks/months, (b) a team-member count in the dozens or avatars of named staff visible, and (c) prior org-level Articles or blog posts on the org page. `MiniMaxAI` passes all three (19 models, 7 datasets, 90 team members, "Why We Built VIBE Bench" and "M2.1" articles predate the H3 drop). If a candidate fails any of these checks, treat the rank-1 slot with suspicion — it's a viral single-drop that may not be a real lab you can bet the beat on.

## Image patterns for HF beat

The brief allows paper thumbnails, model cover images, or generated illustrations. Order of preference: model cover (real photo from the repo) → paper thumbnail (real, AK-curated) → generated.

### Paper thumbnail (preferred for paper stories)

```text
https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<arxiv_id>.png
```

1200x648, ~200–400KB PNG. Always `curl -sI` first to confirm `200 OK` and `content-length > 20000` before downloading. Casing on the arxiv ID doesn't matter (`2607.25857` or `2607.25857v1` both work — the trailing version suffix is optional).

### Dataset thumbnail (preferred for dataset stories)

```text
https://cdn-thumbnails.huggingface.co/social-thumbnails/datasets/<org>/<repo>.png
```

Same CDN, same 1200x648 size, ~400–450KB PNG. Verified for `moonshotai/PerceptionBench` (430KB) and `HuggingFaceCode/stack-v3-train` (433KB). Dataset cards rarely ship a `resolve/main/assets/banner.png`; the social-thumbnails CDN is the canonical fallback. Same `curl -sI` + `200 OK` + `content-length > 20000` check applies.

### Space thumbnail (preferred for Space stories)

```text
https://cdn-thumbnails.huggingface.co/social-thumbnails/spaces/<org>/<repo>.png
```

Same CDN pattern, works for any Space regardless of SDK (Gradio, Streamlit, Docker). `content-length` is typically 350–450KB PNG. Useful when a trending Space has no `README.md` preview image; the social-thumbnails CDN fills the slot the same way it does for models and datasets.

### Model card cover image (preferred for model stories)

Probed at multiple paths — not all model cards ship one. Try in this order:

```text
https://huggingface.co/<org>/<repo>/resolve/main/cover.png
https://huggingface.co/<org>/<repo>/resolve/main/assets/<name>-cover.png
https://huggingface.co/<repo>/resolve/main/assets/cover.jpg
https://huggingface.co/<org>/<repo>/resolve/main/social-preview.png
https://cdn-thumbnails.huggingface.co/social-thumbnails/models/<org>/<repo>.png
```

Microsoft, Allen AI, and other well-staffed orgs typically include a `cover.png` or `assets/<name>-cover.png` (Microsoft Mage-VL ships at `…/assets/mage-vl-cover.png`, 2.7MB PNG). Smaller lab uploads often don't — fall back to the paper thumbnail (if the model has a companion paper) or `mmx image generate`.

**The last line is the canonical fallback.** The HF CDN at `cdn-thumbnails.huggingface.co/social-thumbnails/models/<org>/<repo>.png` reliably returns a 400–450KB PNG for almost every model repo, even if the model card itself ships without `cover.png`. It is what `hf-trend-discovery` recommends for any model without a repo-local image, and the 1200×648 size works for both `kind: "downloaded"` blocks and editorial layouts. Last observed examples: `deepseek-ai/DeepSeek-V4-Flash-0731` (422KB), `moonshotai/Kimi-K3` (411KB), `microsoft/Mage-VL` (411KB).

**Trap:** the `assets/` path is org-specific. The only reliable way to know if a cover exists is to probe each path with `curl -sI`. A 404 is expected on at least one of these paths per model — that's not a failure, it's the cost of working with an open platform.

**Don't try to compose the social-preview URL by guessing — pull it from the page's `og:image` meta tag.** The HF page renderer stamps the social-preview PNG as the page's `og:image` and `twitter:image` hrefs, so the canonical URL is on the page itself if you know where to look. `web_extract` strips image meta tags from its markdown output, so use a one-line `urllib` + regex fetch on the model page HTML:

```python
import urllib.request, re
html = urllib.request.urlopen(urllib.request.Request(
    "https://huggingface.co/<org>/<repo>",
    headers={"User-Agent": "Mozilla/5.0"}
), timeout=15).read().decode("utf-8", errors="ignore")
m = re.search(r'<meta[^>]+property="og:image"[^>]+content="([^"]+)"', html)
print(m.group(1) if m else "NOT FOUND")
# → https://cdn-thumbnails.huggingface.co/social-thumbnails/models/<org>/<repo>.png
```

This is the gold-standard resolution path: the same URL the og-meta wires up is what you should download. Confirmed working for `meta-models/Muse-Glimmer-30B`, `Motif-Technologies/Motif-3`, `mindlab-research/Macaron-V1-Venti`, `deepseek-ai/DeepSeek-V4-Flash-0731`, and `moonshotai/Kimi-K3`. The CDN mirrors a 1200×648 PNG with the repo's org/name and the huggingface.co URL stamped on it — it is the "card you would see as a Twitter preview" and is the right `kind: "downloaded"` image for an HF model story.

**The org avatar CDN is a separate beast and is NOT a hero image.** `https://cdn-avatars.huggingface.co/v1/production/uploads/<upload-id>/<hash>.png` returns the org's small logo (3–15KB, often 64×64 or 128×128). Don't use it as a story image — it fails the 20KB floor and is visually an avatar, not a card. The avatar CDN is only useful for credit context (e.g. "credit: <lab> via Hugging Face" with the org logo as a small badge).

### Organization avatar (last-resort / not for lead)

```text
https://cdn-avatars.huggingface.co/v1/production/uploads/<upload-id>/<filename>.png
```

These are typically small (10–30KB) WebP/PNG thumbnails. Useful for credit context but not as lead photo.

## Beat-specific gotchas

- **arxiv ID format for 2026** is `26MM.NNNNN` (year + month + 5-digit sequence). `2607.xxxxx` = July 2026, `2608.xxxxx` = August 2026. The 5-digit suffix is sequential within the month and roughly date-ordered. Use the arxiv ID's first 4 digits plus the daily papers page to confirm whether a paper is in-window.
- **`/api/papers?date=YYYY-MM-DD` returns identical default data regardless of date.** As of late Jul 2026, the endpoint silently returns the same handful of papers (same titles, same `publishedAt`) whether you ask for `2026-07-29`, `2026-07-31`, or `2026-08-01` — no 400 error, just wrong results. **Do not use it for date filtering.** Use `/api/papers/{arxiv_id}` (the per-ID endpoint, which IS reliable) or scrape the public HTML page `/papers/date/<YYYY-MM-DD>` for the `href="/papers/<id>"` anchors.
- **HF daily listings lag real time by ~12–24h.** Querying `/papers/date/2026-08-01` on Aug 1 UTC returned the same listing as `/papers/date/2026-07-31`. When the user says "last 72 hours" and today is "Aug 1," assume the freshest in-window listing is yesterday's. For each candidate paper, cross-check the actual submission date with `<meta name="citation_date" content="YYYY/MM/DD">` on `arxiv.org/abs/<id>` — HF's `publishedAt` is a date (`T00:00:00.000Z`), not the submission hour.
- **Fresh-drop signal: 0 downloads + high likes + recent `createdAt` = major release, not noise.** DeepSeek-V4-Flash-0731 hit 980 likes with 0 downloads within 24 hours of `createdAt = 2026-07-31T07:30:24Z`. MiniMax-H3 hit 2,000+ likes with 0 downloads within 24 hours of its Aug 4 drop. A brand-new repo from a major lab with hundreds of likes and near-zero downloads is a *just-dropped* model — that's a rank-1 story, not a data quality issue. **For freshness on a model/dataset you've never seen before, prefer `lastModified` over `createdAt`** — `lastModified` reflects the most recent push (README update, file revision, license tweak) and stays accurate even if the initial `createdAt` is older; combining `lastModified ∈ last 72h` with `downloads ~ 0 / likes ≫ 100` is the strongest in-window fresh-drop signal. Cross-check the org is one of the watchlist labs (deepseek-ai, moonshotai, Qwen, zai-org, microsoft, mistralai, allenai, poolside, **MiniMaxAI**, **Nanbeige**, etc.) before treating as rank-1 — see the legitimacy test in the watchlist section. The same heuristic applies to fresh dataset and Space drops.
- **"Trending model" ≠ "trending in your window."** A model on the trending list might have been uploaded days earlier. Always check the model card's "last updated" date (visible in the page metadata) before ranking. Lag of 1–3 days between upload and trending is common — Kimi K3's API launched July 16, the weight drop was July 27, and the model only hit the trending top spot on the 28th.
- **Hub feature releases ship in the changelog without a blog post.** The changelog at `huggingface.co/changelog` is the canonical source. Each entry has a permalink like `huggingface.co/changelog/<slug>` and shows the publication date inline.
- **MCP Server releases also go to GitHub first** (`github.com/huggingface/hf-mcp-server/releases`). The GitHub release page shows UTC timestamps to the minute — use that for `published_at` and the changelog page for the editorial writeup.
- **Mistral papers and similar often surface as named collections on the Hub** (e.g. "AI censorship", "AI safety"). Check `huggingface.co/collections/` for newly-added items if a paper has no obvious model or dataset.
- **HF is the primary home for several major-lab releases** (Moonshot Kimi, Microsoft Mage, Mistral Magistral, Zhipu GLM, Poolside Laguna, DeepSeek V3/V4). The HF model card is the canonical source; the lab blog is the secondary source. For these, source_url the HF page, not the lab blog.
- **Some model cards lack benchmark sections at upload time** — Moonshot and Microsoft ship the full eval table in the README on day one, but smaller labs (Poolside, Nanbeige) often publish to a separate "evals" page that lives outside the model card. If the model card has no numbers, check the lab's release notes blog or the model collection page.
- **Hub feature releases are dated by adjacent date markers in the changelog HTML, not a JSON field.** The changelog page renders dates inline next to each entry as `<div class="text-md mb-2 mt-1 text-gray-400">Mmm DD, YY</div>` (two-digit year, US-style ordering). To get a reliable date for a changelog entry, parse the raw HTML in order and map each date to the entry that follows it. As of late July / early Aug 2026 the order is mcp-improvements-jul-26 (Jul 22), egress (Jul 22), spaces-with-agents (Jul 16), token-presets (Jul 14), filter-models-by-hardware (Jun 30). Anything before mcp-improvements-jul-26 is out of the 72-hour window — skip.
- **`lastModified` is the right freshness signal for dataset and Space stories, not `createdAt`.** A dataset updated on Aug 1 may have been created weeks earlier — the edit is the story, not the upload. For Spaces, `lastModified` reflects the most recent Space SDK/repo push; that is the timestamp to use for `published_at`. Example: `cinderholm/wan2-2-i2v-v3` Space has `createdAt = 2026-07-19T13:14:01Z` (a week old) but `lastModified = 2026-08-02T16:40:54Z` (in-window) — the latter is the story.
- **`/api/spaces/<org>/<repo>` can return `null` for createdAt/lastModified on Spaces whose IDs only appear on the trending list with no API visibility.** The `_id` field exists but the three timestamp fields may be null. Fall back to scraping the Space page's README for the most recent commit timestamp, or skip the data point. As of 2026-08-03, the trending Spaces list contains community demos like `prithivMLmods/Qwen-Image-Edit-2511-LoRAs-Fast` (created 2025-12-19, lastModified 2026-07-30) — both within the 72-hour window after you cross-check `lastModified`.
- **The Hugging Face CDN sometimes returns 401 on `cdn-thumbnails.huggingface.co/social-thumbnails/spaces/<org>/<repo>.png` for community Spaces, but the same URL succeeds on retry.** Likely intermittent auth/cache glitch — retry once before scraping og:image from the Space page. Last observed transient: `cinderholm/wan2-2-i2v-v3` returned 401 then 200 with 366KB PNG on a recheck.
- **Changelog goes silent for days at a time — that's normal, not a bug.** Confirmed 11-day gap with no entries between the 22 Jul 2026 `mcp-improvements-jul-26` release and the next documented entry. When the 72-hour window contains zero changelog entries, **don't fabricate a hub-feature rank.** Substitute the slot with a Hub-first dataset drop (e.g. `HuggingFaceCode/*`, `HuggingFaceH4/*`, `HuggingFaceFW/*`), which carry the same "HF shipped this themselves" provenance. As of the 2026-08-05 window, the most recent in-window Hub-org artifact was `HuggingFaceCode/stack-v3-train` (a dataset upload by HF Code Research) — the slot is leasable by a fresh Hub-org dataset drop, not by stretching the changelog to two-week-old entries.
- **The `MiniMaxAI` org name is a watchlist trap.** Org names that look like minor LLM-lab aliases (`MiniMaxAI`, `OpenBMB`, `internlm`, etc.) sometimes belong to major video/multimodal labs; the watchlist is sorted by HF org page recency, not visibility. When an unknown org sits at #1 trending with thousands of likes, do the legitimacy test before dismissing it as noise.

## In-window size constraint

The brief is "4–5 stories from the last 72 hours." For the HF beat that translates to: 1–2 trending model releases, 1–2 trending papers, 0–1 hub feature release, 0–1 dataset drop. **Do not pad with non-trending older paper** just to hit 4 — the brief explicitly says "would an ML builder actually click into it on huggingface.co today." A day where the trending page only has 2 strong candidates is a 2-story day, not a 4-story day.

**When the changelog is silent and no HF-org blog post drops in-window**, the 0–1 hub-feature slot can be carried by a HF-org dataset drop instead (see gotcha above). The goal is to land 4–5 in-window stories without stretching the window, not to honour a fixed category template.

## Rank 1 candidate heuristic

For the lead slot, the strongest candidates are usually:

1. A new major-lab open-weights release (Moonshot Kimi, Microsoft Mage, Mistral new flagship, Zhipu GLM, DeepSeek V-series, Poolside Laguna) — biggest "click" weight
2. A trending paper that came with a same-day model or dataset drop — combined story

Hub feature releases and dataset-only drops are usually rank 4–5. Lead with model releases when one is in window; lead with the freshest paper when no major model dropped.

## Example: 5-story beat for 2026-07-30

A real 5-story mix that hit the bar:

| # | Category | Story | Source |
|---|---|---|---|
| 1 | Major-lab model | Moonshot Kimi K3 weights drop (2.8T, native MXFP4) | `huggingface.co/moonshotai/Kimi-K3` |
| 2 | Major-lab model + paper | Microsoft Mage-VL codec-native streaming VLM (Apache-2.0) | `huggingface.co/microsoft/Mage-VL` |
| 3 | Trending paper | Mistral Shieldstral 3B safety classifier | `huggingface.co/papers/2607.25857` |
| 4 | Trending paper | Adobe Wonder video world model (16 FPS minute-scale) | `huggingface.co/papers/2607.26037` |
| 5 | Hub feature release | HF MCP Server v0.4.2 + Sandboxes + hf_fs | `github.com/huggingface/hf-mcp-server/releases/tag/v0.4.2` |

3 categories covered, 4 if you count the changelog as a separate category. Lead was the biggest open-weights event in the window; the rest were paper / hub-feature depth.
