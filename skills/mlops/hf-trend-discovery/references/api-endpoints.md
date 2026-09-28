# Hugging Face HTTP API — read-only endpoints for trend discovery

No auth required for these. JSON in, JSON out. All `GET` requests.

## Repository metadata

| Endpoint | Returns | Notes |
|---|---|---|
| `GET /api/models/{org}/{repo}` | `createdAt`, `lastModified`, `downloads`, `downloadsAllTime`, `likes`, `tags[]`, `cardData{}`, `siblings[]` (file list), `spaces[]`, `pipeline_tag`, `library_name`, `safetensors{}` (size + dtype), `model-index` (eval results) | Workhorse. Use `lastModified` for "is this in window?" |
| `GET /api/datasets/{org}/{repo}` | same shape + `datasetsServerInfo{numRows, libraries, formats, modalities}` + `description` | For corpus drops; `numRows` is the headline number |
| `GET /api/spaces/{org}/{repo}` | same shape | For Space launches; check `runtime` and `sdk` |

## Org-scoped listing (full catalog with dates)

| Endpoint | Returns | Notes |
|---|---|---|
| `GET /api/models?author={org}&full=true&limit=100` | Array of model objects, newest first | The `full=true` flag expands `cardData` and `siblings`. Paginated; loop until empty. |
| `GET /api/datasets?author={org}&full=true` | Same for datasets | Useful for "what did Mistral drop this week" |
| `GET /api/spaces?author={org}&sort=likes` | Space list | Lower signal than models for news |

## Papers

| Endpoint | Returns | Notes |
|---|---|---|
| `GET /api/papers` | Array of today's trending papers (full HTML list) | Use this, not `/papers?date=today` (that 400s) |
| `GET /api/papers?date=YYYY-MM-DD` | Daily list | **Date must be ≤ yesterday UTC** — server returns 400 with "expected date to be <=Fri Jul 24 2026 00:00:00 GMT+0000" otherwise |
| `GET /api/papers/{arxiv_id}` | Single paper: `id`, `title`, `summary`, `authors[]`, `publishedAt`, `submittedBy`, `submittedAt` | `publishedAt` is usually `T00:00:00.000Z` of the listing day — treat as a date |

## Blog posts

| Endpoint | Returns | Notes |
|---|---|---|
| `GET /api/blogPosts/{author}/{slug}` | Full post body + metadata | `publishedAt` is the truth. Slugs are NOT predictable from the title — probe with `curl -sIL` first. |
| HTML scrape `GET /blog` | List of post links | Pull `datePublished` from JSON-LD on each link |

## Other

| Endpoint | Returns | Notes |
|---|---|---|
| `GET /api/datasets/{org}/{repo}/parquet` | List of parquet file URLs | Useful for "how big is the drop" |
| `GET /api/models/{org}/{repo}/eval_results` | Inline eval results | Rare; usually just embedded in `cardData` |

## Example — get the last 10 model updates from NVIDIA

```bash
curl -s 'https://huggingface.co/api/models?author=nvidia&full=false&limit=10' | \
  jq -r '.[] | "\(.lastModified) \(.id)  \(.downloads // 0) dls  \(.pipeline_tag // "?")"'
```

## Example — verify a model is in the 72-hour window

```bash
model=upstage/Solar-Open2-250B
created=$(curl -s "https://huggingface.co/api/models/$model" | jq -r '.createdAt')
mod=$(curl -s "https://huggingface.co/api/models/$model" | jq -r '.lastModified')
echo "created: $created"
echo "mod:     $mod"
```

## Headers / auth

No auth needed for public reads. For private repos or higher rate limits, set `Authorization: Bearer $HF_TOKEN`. Default rate limit is generous enough for one-off scraping but if you hit 429s, sleep 1s between calls.
