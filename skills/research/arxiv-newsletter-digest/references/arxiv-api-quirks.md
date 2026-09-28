# arXiv API Gotchas for Newsletter Work

Concrete quirks learned while building digests. Distilled from production runs.

## Date filtering

The arXiv Atom API (`export.arxiv.org/api/query`) accepts these query params:

| Param | Values | Notes |
|-------|--------|-------|
| `search_query` | free text | No native date range |
| `sortBy` | `relevance`, `lastUpdatedDate`, `submittedDate` | Use `submittedDate` for fresh-paper discovery |
| `sortOrder` | `ascending`, `descending` | |
| `max_results` | 1–30000 | Default 10; pull 30+ for a 72-hour window |
| `start` | 0-indexed offset | Use for pagination if 30 isn't enough |

There is **no** `dateRange` param. To get "last 72 hours":

1. Query `sortBy=submittedDate&sortOrder=descending&max_results=30`
2. Filter client-side by parsing `<published>` (UTC, ISO 8601 with `Z` suffix)
3. For windows > 72h, paginate with `start=30`, `start=60`, etc.

## ID parsing

The API returns IDs as `http://arxiv.org/abs/2607.21553v1`. To extract the bare ID:

```python
raw = entry.find('a:id', ns).text.strip()
arxiv_id = raw.split('/abs/')[-1]   # '2607.21553v1'
# strip version for canonical
canonical = arxiv_id.split('v')[0]  # '2607.21553'
```

The version (`v1`, `v2`, …) is meaningful — a paper can substantively change between versions. For citation hygiene, preserve the version you actually read; for ranking and headline writing, use the bare ID.

## Categories

For AI/ML newsletters, four categories cover the bulk of submissions:

| Category | Coverage |
|----------|----------|
| `cs.AI` | Artificial Intelligence (broad) |
| `cs.LG` | Machine Learning (the bulk) |
| `cs.CL` | Computation & Language (NLP, agents, evals) |
| `cs.CV` | Computer Vision (multimodal, video) |

Optional fifth: `stat.ML` for stats-flavored ML work.

Do NOT pull `cs.CR` (security), `cs.RO` (robotics), `eess.AS` (audio) unless the user's beat explicitly calls for them — they add noise without much gain for a general AI digest.

## Abstract extraction

```python
ns = {'a': 'http://www.w3.org/2005/Atom'}
root = ET.parse(sys.stdin).getroot()
for entry in root.findall('a:entry', ns):
    summary = entry.find('a:summary', ns).text.strip().replace('\n', ' ')
    # summary may contain LaTeX-ish fragments; for editorial writing,
    # extract sentences only with a regex
```

**Don't use `web_extract` on `arxiv.org/abs/{id}`** for the abstract. That page returns only the metadata scaffold (title, authors, comments, categories) — the abstract body lives in the Atom `<summary>` element. `web_extract` on the abs page is fine for confirming submission history, comments, and license, just not for the abstract.

## License

`<link rel="license" href="..."/>` and the license badge (CC-BY, CC-BY-NC, arXiv non-exclusive) tell you whether you can quote, reuse, or screenshot figures. Most AI papers are CC-BY or arXiv non-exclusive; a small minority (often industry lab papers) are CC-BY-NC-ND, which restricts derivatives.

## Common pitfalls

- **First-page doesn't have v2 updates.** If a paper was revised on day 4, `submittedDate` still returns day 1. Use `lastUpdatedDate` if revision-relevance matters (rare for digest work).
- **`max_results=30` returns at most ~30 per query.** arXiv's API batches internally; if you need more, paginate.
- **Author lists can be huge.** arXiv-style multi-author papers regularly have 20+ authors. Truncate to first 3-5 in editorial output and use "et al." for the rest.
- **Comments field is gold.** It contains page counts, figure counts, conference acceptance ("Accepted at NeurIPS 2026"), and workshop placement. Always include in metadata extraction.
- **The 3-second rate limit.** arXiv asks for ~1 request per 3 seconds. Batch your calls: pull all needed categories in one `curl` with `;` separators, or use the multiple-id-list endpoint (`id_list=ID1,ID2,ID3`) when fetching specific papers.