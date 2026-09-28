---
name: arxiv-newsletter-digest
description: "Ranked JSON digest from arXiv + HF within a sliding window."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [research, newsletter, digest, arxiv, huggingface, editorial]
    related_skills: [arxiv, huggingface-hub, github-pr-workflow]
---

# arXiv Newsletter / Digest Correspondent

Class-level skill for the recurring editorial task of producing a **ranked JSON list of recent AI research stories** within a sliding time window (typically 48–168 hours). Examples: "today's top 5 papers", "weekly agent roundup", "what would I open today", "research correspondent output".

The output shape is usually the same: 4–6 ranked stories, each with `headline`, `dek`, `body`, `arxiv_id` or `github_repo`, `authors`, `source_url`, `published_at` (ISO 8601 UTC), and an `image` object pointing to a verified PNG ≥ 20 KB.

This skill captures the workflow that reliably produces that output without fabricated metadata.

## Quick Reference

| Step | Command / Tool |
|------|---------------|
| Discover recent arXiv papers | `curl "https://export.arxiv.org/api/query?search_query=cat:cs.AI&sortBy=submittedDate&sortOrder=descending&max_results=30"` (repeat for cs.LG, cs.CL, cs.CV) |
| Cross-reference with HF community signal | `curl "https://huggingface.co/api/daily_papers?date=YYYY-MM-DD"` |
| Read full abstract | `curl "https://export.arxiv.org/api/query?id_list=ID"` and parse `<summary>` |
| Get paper HTML figure | `curl -sLo /tmp/x.png "https://arxiv.org/html/IDv1/x1.png"` |
| Get HF thumbnail | `curl -sLo /tmp/x.png "https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/ID.png"` |
| Verify image | `file /tmp/x.png` → must report `PNG image data` and size > 20 KB |

## Workflow (7 steps)

### 1. Define the window

Convert "last 72 hours" to a UTC date range, but query arXiv without a date filter — arXiv API doesn't support `dateRange`. Instead pull `max_results=30` sorted by `submittedDate descending` and filter client-side by parsing the `<published>` field. Cross-check against HF `daily_papers` for the 3 days in the window — the HF feed is dated per-day and reliable for the last 7 days.

### 2. Pull from multiple categories in parallel

Make one call per category with `cat:cs.AI`, `cat:cs.LG`, `cat:cs.CL`, `cat:cs.CV` — these four cover ~95% of relevant papers. Run them as parallel `terminal()` calls in a single assistant turn; the runtime executes them concurrently.

### 3. Cross-reference with Hugging Face daily papers

`https://huggingface.co/api/daily_papers?date=YYYY-MM-DD` returns up to ~30 papers/day sorted by community upvotes, with the same arXiv IDs. This is your **importance signal**. A paper that appears in HF daily papers within 24h of arXiv submission is almost always worth including.

### 4. Read the abstracts

The arXiv API's `<summary>` element has the full abstract. `web_extract` on `arxiv.org/abs/{id}` returns only the metadata scaffold, not the abstract body — don't rely on it for editorial writing. Parse the API XML directly.

### 5. Harvest images (with CDN fallback)

For each paper you'll include:

1. **First attempt**: `curl -sLo <path>.png "https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png"`
2. **If size 0 or HTML**: `curl -sLo <path>.png "https://arxiv.org/html/<id>v1/x1.png"` — Figure 1 of the paper's HTML render. Try `x2.png`, `x3.png` if needed.
3. **Always verify**: `file <path>` must report `PNG image data` and `stat` must report > 20 KB. If verification fails, regenerate or skip — never ship an HTML error page disguised as a PNG.

### 6. Rank by "would I actually open this PDF today?"

Editorial ranking is judgment, not formula. Use these heuristics:

- **Decisive mechanism**: papers that name a specific causal mechanism (e.g. "recruits a pre-existing persona subspace") outrank incremental benchmark bumps
- **Open-source artifacts**: a released model, dataset, or benchmark that the community can use immediately
- **Cross-category surprise**: a paper that's strong in 2+ of the beat's categories (e.g. agents + evals)
- **Author / lab reputation**: NVIDIA Labs, DeepMind, Anthropic, Meta FAIR, Microsoft Research signal a baseline quality floor
- **Red flags**: small delta on a saturated benchmark, narrow domain, no artifacts released

### 7. Write the JSON

Match the exact shape the consumer expects. Common fields:

```json
{
  "rank": 1,
  "headline": "6–12 words, declarative, mechanism-bearing",
  "dek": "1 sentence — the mechanism or finding, not a summary",
  "body": "2–3 paragraphs: what they did, what they found, why it matters",
  "arxiv_id": "2607.21553",
  "github_repo": "org/repo",
  "authors": "Author A, Author B, Author C",
  "source_url": "https://arxiv.org/abs/...",
  "published_at": "2026-07-23T17:36:05Z",
  "image": {
    "path": "/abs/path/research_<slug>.png",
    "source": "https://cdn-thumbnails.huggingface.co/.../papers/<id>.png",
    "credit": "arxiv.org/abs/<id>"
  }
}
```

## Pitfalls

- **Never fabricate metadata.** If you can't read an abstract, don't write one. If an image fails to download, skip the image field rather than invent one. The "Finishing the job" rule in the system prompt applies here: a real download > a plausible-looking substitute.
- **`web_extract` on arxiv abs pages is a trap.** It returns the page metadata scaffold without the abstract body. Use the arXiv API (`export.arxiv.org/api/query`) when you need the abstract for editorial writing.
- **HF CDN 504s are common for fresh papers.** Always have the arxiv.org/html fallback ready. Don't retry the HF CDN more than twice — it's not your flaky network, it's theirs.
- **Verify every image with `file`.** `curl -sLo foo.png` will silently save an HTML error page with the `.png` extension. A 504 looks identical to a successful download until you check `file`.
- **arXiv dates are UTC, not local.** The `<published>` field is `YYYY-MM-DDTHH:MM:SSZ`. Convert the user's "last 72 hours IST" to a UTC range before filtering.
- **HF daily papers is per-day, not cumulative.** Pull each day in the window separately. Day boundaries are UTC.
- **The arXiv API returns IDs as `http://arxiv.org/abs/XXXXv1`**, not bare IDs. Split on `/abs/` to extract.
- **Sort by `submittedDate` not `lastUpdatedDate`.** `lastUpdatedDate` will surface revised v2s of old papers as if they were new.
- **Don't include more than 5–6 stories.** The editorial contract is "would I open this today" — beyond that, ranking becomes noise. If you have 8 strong candidates, drop the bottom 3 ruthlessly.

## Verification (run before returning)

```bash
# 1. JSON parses
python3 -c "import json; json.load(open('output.json'))"

# 2. Every image exists and is a real PNG
for img in $(jq -r '.stories[].image.path' output.json); do
  file "$img" | grep -q "PNG image data" || echo "FAIL: $img"
done

# 3. Every image is > 20 KB
for img in $(jq -r '.stories[].image.path' output.json); do
  [ $(stat -c%s "$img") -gt 20480 ] || echo "TOO SMALL: $img"
done

# 4. Every arxiv_id is well-formed (4 digits . 5 digits, optionally vN)
jq -r '.stories[].arxiv_id // empty' output.json | grep -vE '^[0-9]{4}\.[0-9]{4,5}(v[0-9]+)?$'

# 5. Every published_at is ISO 8601 UTC
jq -r '.stories[].published_at' output.json | grep -vE '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$'
```

## Support Files

- `references/arxiv-api-quirks.md` — concrete gotchas on the arXiv API, including date filtering, ID parsing, and category coverage
- `references/hf-cdn-fallback.md` — the full CDN-failover protocol with worked examples for when each branch triggers
- `templates/digest-output.json` — the canonical output shape, copy-and-fill
- `scripts/fetch-window.sh` — given a UTC date range, pulls arXiv Atom XML for cs.AI/cs.LG/cs.CL/cs.CV and HF daily papers in parallel; outputs a unified JSON list sorted by `published_at`
- `scripts/verify-digest.sh` — runs the five verification checks above against a digest JSON file