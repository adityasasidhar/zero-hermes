# Leaf Beat-Reporter Workflow

The Pattern B orchestrator spawns one leaf sub-agent per beat (e.g. AI
Industry, Research, Messi, F1 in THE HERMES TIMES v4). The leaf executes
the beat's instructions and returns a JSON package — never prose, never a
rendered artifact. This reference is the worked playbook a leaf needs to
ship a beat on time without fabrication.

A leaf that follows these rules produces a JSON package that the
orchestrator can drop into `manifest.json` without rework. A leaf that
ignores them forces the editor to re-run, re-image, or strip the beat.

## What a leaf receives

- The beat brief (e.g. `prompts/beat_briefs.md` — `## AI Industry beat`
  section). Contains: window, categories, story schema, image rules,
  output JSON contract.
- The orchestrator's already-decided lead/section structure (so the
  leaf doesn't second-guess page hierarchy).
- The output dir (usually `~/.hermes/data/hermes-times-v4/` or
  `issues/<date>/beats/<beat>/`).

What a leaf does **not** receive and should not request: the other
beats' briefs, yesterday's rendered HTML, or render/CSS knowledge. The
leaf is a content producer, not a renderer.

## The 6-step leaf loop (use it in order)

### 1. Verify the window before searching

The window is almost always "last 72 hours from today's IST". The
common mistake is to grab "this week" stories that are 5-7 days old.

**Hard cutoff calculation:**

```text
today_ist = date -u + 4 hours     # UTC → IST (or check `TZ=Asia/Kolkata date`)
cutoff_ist = today_ist - 72h
```

**Strict rule:** a story whose primary source is timestamped *before*
`cutoff_ist` is out, even if it dominated the news cycle. Document
excluded candidates in the JSON (with reason) so the editor can
override knowingly.

**Soft rule:** if a major story broke at 73-96 hours, mention it in a
note, do not rank it. The editor can promote if they want.

### 2. Find stories via primary sources, not aggregators

Bad path: search "AI news July 25 2026" → get a YouTube roundup.
Good path: search the specific vendor name + the date → find their
official blog/press release.

**Vendor primary sources to remember:**

| Beat | Primary URLs to try first |
| --- | --- |
| AI lab announcements | `ir.<vendor>.com/news-events/press-releases/...`, `<vendor>.com/news/`, `<vendor>.com/blog/` |
| AI industry news | `reuters.com/technology/...`, `techcrunch.com/...`, `cnbc.com/...`, `theinformation.com`, `aa.com.tr/...` (Anadolu for non-Western) |

**Image-CDN cheat sheet (use when the primary source has no inline photo):**

| CDN | URL pattern | UA / headers needed |
| --- | --- | --- |
| OpenAI Contentful | `images.ctfassets.net/kftzwdyauwt9/<id>/<slug>.<ext>` | None — default curl works |
| Anthropic blog | `www-cdn.anthropic.com/images/<hash>/website/<slug>.<ext>` | None — but the file is **PNG** even when the URL ends `.png`; convert with `ffmpeg -i in.png -q:v 3 out.jpg` |
| TechCrunch uploads | `techcrunch.com/wp-content/uploads/<year>/<month>/<slug>.{jpg,webp}` | None — alt sizes via `?resize=W,H` |
| CNBC CDN | `image.cnbcfm.com/api/v1/image/<id>-<id>-gettyimages-<n>-<caption>.jpeg` | **Requires** `User-Agent: Mozilla/5.0 ...` AND `Referer: https://www.cnbc.com/` — default curl returns 403 |
| AMD IR press releases | `d1io3yog0oux5.cloudfront.net/_<q4hash>/amd/db/<id>/image_resized.jpg` | None — these are the Q4 IR CMS images |
| GlobeNewswire | `ml.globenewswire.com/Resource/Download/<id>.<ext>` | Try first; if 204, the asset is gated and the press release HTML is the fallback |
| Anadolu Agency | `web-cdnprod.aa.com.tr/uploads/Contents/<YYYY>/<MM>/<DD>/thumbs_b_c_<hash>.jpg` | None — `thumbs_b_c_*` is the big lead photo, `thumbs_m_c_*` are mid-story images |
| Indian Express Group CDN | `cf-images.assettype.com/<outlet-slug>%2F<YYYY-MM-DD>%2F<random-slug>%2F<filename>.jpg?w=1200&auto=format%2Ccompress&fit=max` | None — default curl works. Used by Indulge Express, The Hindu, Quint, and other Indian Express Group outlets. Path uses URL-encoded `%2F` between outlet/date/slug segments. Often hosts Reuters / AFP wire photos republished with the outlet's caption. Sized at 1200x675 with `?w=1200`; bump to `?w=1920` for cover-sized needs. Verify the photo matches the story's subject with `vision_analyze` — secondary outlets frequently mis-tag stock images. |
| Reuters Connect video | `reutersconnect.com/item/...` | Video, not photo — useful only when the lead asset is a video |
| HF paper thumbnail | `cdn-thumbnails.huggingface.co/social-thumbnails/papers/<arxiv_id>.png` | None — usually >20 KB PNG, 1200×648; primary choice for `desk: "research"` paper stories. **Flaky in practice:** the CDN returns 504/timeout for ~30-40% of arXiv IDs even when the paper exists. Don't fail the whole beat on one bad download — keep a counter, attempt ≥2 retries with `--retry 2 --max-time 30`, and fall back to the paper's first PDF figure (`https://arxiv.org/pdf/<id>` first-page render) or `mmx image generate` if the thumbnail never lands. Document the missing thumbnail in the leaf's `image` block as `null`, never fabricate a `local_path` to an empty file |
| HF model thumbnail | `cdn-thumbnails.huggingface.co/social-thumbnails/models/<org>/<name>.png` | None — same CDN as papers but the URL segment is `models`, not `papers`. The path uses the *org/repo* pair, not the URL-safe slug. Verify >20 KB and `file <out>` says PNG — the file silently 404s for private/deprecated models and you get a stub; `curl -sI` for `200` before downloading if unsure |
| HF dataset thumbnail | `cdn-thumbnails.huggingface.co/social-thumbnails/datasets/<org>/<name>.png` | Same CDN convention — segment is `datasets`. Useful for `desk: "huggingface"` dataset drops |
| Formula1.com hero | `media.formula1.com/image/upload/t_16by9North/c_lfill,w_3392/q_auto/v1740000001/trackside-images/<year>/<gp_slug>/<id>.webp` | None — directly downloads; the w=3392 variant is the right size for an A4 broadsheet cover. Cloudinary transforms in the path are stable across editions |
| NVIDIA Newsroom (iPR) | `iprsoftwaremedia.com/219/files/<YYYYMM>/<hash>/<slug>_<cache>-prv.jpg?v=<cache>` | None — but the **`?v=<cache-buster>` querystring is required** for the `_prv` variant or it 404s. `_prv.jpg` returns 1600x900 (~80-100KB); `_mid.jpg` returns only 700x393 (~30KB). The page HTML often displays the filename *truncated* in the middle (`sk-nvi..._prv.jpg`) — read the `og:image` meta tag for the fully-formed URL. Cache-buster is stable for a given press release; copy it from any iPR URL on the same page |

**How to discover TechCrunch upload URLs from a TC article:**

```bash
curl -A "Mozilla/5.0" -sSL "https://techcrunch.com/<year>/<month>/<day>/<slug>/" \
  | grep -oE 'https://techcrunch\.com/wp-content/uploads/[^"]*\.(jpg|jpeg|png|webp)' \
  | sort -u
```

The first URL is usually the article hero photo; alt sizes appended via `?resize=W,H` are downscaled thumbnails.
| Research papers | `arxiv.org/abs/<id>`, `<lab>.com/research/`, `huggingface.co/papers` |

**arXiv discovery recipe that works.** For the Research beat:

1. **Pull the daily new-submission list by category**:
   `https://export.arxiv.org/api/query?search_query=cat:cs.AI&start=0&max_results=100&sortBy=submittedDate&sortOrder=descending`. Do the same for `cat:cs.LG`, `cat:cs.CL`, `cat:cs.CV`, `cat:cs.RO`. Dedup by `id` across categories — many papers are cross-listed. The window filter should compare against `a:updated` (catches v2 resubmissions of important work) **not** `a:published` (which only fires on original submission and misses papers from 4-7 days ago that just got a substantial revision). 2-3 days of recent activity is usually enough for a daily paper.
2. **For clean metadata (title, authors, abstract), prefer `web_extract` on `https://arxiv.org/abs/<id>`** over hand-parsing the API XML. The arXiv API XML is technically valid but its `<title>` / `<summary>` elements contain LaTeX and entity-encoded text that fights most parsers; in particular, when you query via `id_list=…` and walk the returned entries with `xml.etree.ElementTree`, it is easy to silently read the *feed-level* `<title>` (which says `arXiv Query: search_query=…`) instead of the entry's own `<title>`. `web_extract` gives you clean markdown with the actual paper title, authors as a list, and the abstract as plain prose — and it's idempotent, so a retry is cheap. Reserve the arXiv API for filtering by date/category; reserve `web_extract` for per-paper metadata.
3. **Authors come back as a list, not a single string.** Format as `Author1, Author2, Author3` in the JSON, dropping "et al." since you'll only have 1-6 names anyway.
| Sports | official league/club site, Getty Images via the team's own newsroom |
| Defense AI | `defensenews.com`, `breakingdefense.com`, `reuters.com/business/aerospace-defense/` |

**Trap:** many stories appear in vendor blog indexes with stale or
incomplete listings (Anthropic's `/news` page missed the Opus 5 launch
when checked on the same day; the actual post was at
`anthropic.com/news/claude-opus-5`). If a vendor is suspected to have
launched something, check the homepage hero banner AND guess the slug
directly.

### 3. Rank by the user's bar, not by news volume

The beat brief states the bar (e.g. "would a working AI engineer in
New Delhi care?"). Apply it strictly:

- A small but real funding round for a niche AI infra startup → not
  newsworthy for a generalist paper unless the lead is "AI infra funding
  dried up this quarter."
- A model release by a top-4 lab → always newsworthy.
- A regulatory decision with 6+ month enforcement delay → usually skip
  unless the user follows policy specifically.
- A leak/rumor from anonymous sources → only if it's the only available
  lead on a real event.

Cover **at least two of the five standard categories** in the brief
(lab announcements, funding/business, infra/hardware, regulation/policy,
open-source releases). A beat that lands all 5 stories in one category
is unbalanced — replace the weakest with a category-missing candidate.

### 4. Extract primary content; quote only what's in the source

Use `web_extract` (fast, no LLM) for the press release / blog post.
Use `browser_navigate` only if the page is JS-rendered and `web_extract`
returns nothing useful.

**Quote rules:**

- Direct quotes only from primary sources. CEO quotes from press
  releases count as primary.
- Paraphrase for everything else; never invent a quote.
- Numbers (dollars, GPU counts, dates) must appear in the source. If
  the source says "up to $5 billion", you can write "$5B". If the source
  doesn't give a number, do not include one in the headline.

**Date rules:**

- Use the primary source's own timestamp. Don't normalize to "today".
- For roundup-style stories that cite a vendor's announcement: cite
  the vendor's announcement date, not the roundup's.

### 5. Get images — real photo first, generate only as last resort

The beat brief's image strategy is usually "real photo: lab logo +
product, executive portrait, server room, conference stage. Generate
only if no real photo works."

**Real photo discovery pattern (in order of preference):**

1. The primary source itself (lab press release often has a hero
   image). Extract with `web_extract`; the markdown `![](url)` lines
   are the asset URLs.
2. Vendor's media kit / press page (`/press`, `/press-kit`,
   `/newsroom`).
3. Reuters / TechCrunch / The Information story that covers the same
   event — image URLs from their HTML are usually CDN-hosted and
   openly downloadable.
4. Search engines for `<vendor name> <event> <date>` — look for images
   on CDN domains (`cdn.mos.cms.futurecdn.net`, `ml.globenewswire.com`,
   `cdn.sanity.io`, `techcrunch.com/wp-content/uploads/...`).

**Download verification:**

```bash
curl -A "Mozilla/5.0" -sSL "<url>" -o /path/to/asset.<ext>
file /path/to/asset.<ext>      # must say "JPEG image data" or "PNG image data"
ls -l /path/to/asset.<ext>     # must be > 20 KB (per the orchestrator's
                               # MIN_ASSET_BYTES rule)
```

**If the file is PNG or WebP, the renderer needs JPEG.** Convert with:
```bash
ffmpeg -y -i /path/to/asset.png -q:v 3 /path/to/asset.jpg
```

Anthropic blog hero images in particular are always PNG even when the URL ends `.png`. Always run `file` and convert to JPEG before recording the asset in the JSON.

**CDN-specific curl flags that prevent wasted cycles:**

- **CNBC** (`image.cnbcfm.com/...`): default curl returns 403. Need both `User-Agent: Mozilla/5.0 ...` AND `Referer: https://www.cnbc.com/` headers. Without those, the extracted URL is dead.
- **GlobeNewswire** (`ml.globenewswire.com/Resource/Download/...`): returns 204 with empty body for gated assets. Always check `file` after download; an empty file means the asset is gated and the press release HTML is the fallback.
- **OpenAI Contentful** (`images.ctfassets.net/...`): default curl works. No special flags needed.
- **Anadolu Agency** (`web-cdnprod.aa.com.tr/uploads/...`): default curl works. Path is `thumbs_b_c_<hash>.jpg` for the lead photo, `thumbs_m_c_<hash>.jpg` for mid-story images.
- **Hugging Face CDN** (`cdn-thumbnails.huggingface.co/...`): paths that use `models/` or `datasets/` segments silently fall back to a generic stub for private or deprecated repos. `curl -sI "<url>"` first; if the response is `200 OK` and `content-length > 20000`, the thumbnail is real. If `404`, the path is wrong (check `org/repo` casing and `/` vs URL-encoded `%2F`). The `papers/` segment has its own failure mode: ~30-40% of arXiv IDs return **HTTP 504 / connection timeout** even when the paper exists on the HF papers page. This is CDN-level, not a path bug. Retry with `--retry 2 --max-time 30`; if it still fails, fall back to the paper's first PDF figure, `mmx image generate`, or omit `image` (use `image: null` in the manifest) — never write an empty file with a fake `local_path`.
- **Wikipedia Commons**: as of 2026 the upload servers reject default curl requests with a 2-KB HTML 404 page even when the URL is canonical. The thumbnail `/wikipedia/commons/thumb/a/ab/<file>/<width>px-<file>` path is correct, but you must send `User-Agent: Mozilla/5.0 (X11; Linux) Hermes/<version>` and accept `text/html` (some intermediate redirects return `application/json` and curl aborts). For recent news subjects where Commons is the only source, prefer the **lead's homepage hero** or a vendor press photo over Wikimedia.
- **McClatchy image CDN** (Miami Herald, Sacramento Bee, Fort Worth Star-Telegram, etc.): URLs under `https://www.{paper}.com/public/latest-news/<id>/picture<n>/alternates/{size}/<slug>.jpg` (and the `LANDSCAPE_1200` variant) frequently fail direct downloads with `curl: (92) HTTP/2 stream 1 was not closed cleanly: INTERNAL_ERROR (err 2)`, even with custom `--http1.1`, `User-Agent`, and `Referer` headers. This appears to be HTTP/2 negotiation failure on McClatchy's edge. Don't burn cycles retrying — switch to the underlying `images.mcclatchy.com` host if visible in the HTML, or pick a different outlet's coverage of the same event (Herald story → try the Miami-based AP/Reuters wire photo on the corresponding APNews or Yahoo Sports page).

**Trap:** a `curl` 200 response with 35 bytes is a redirect/HTML
placeholder, not an image. Always run `file` after download. If `file`
says "HTML document" or "GIF image data, 1 x 1", the URL was wrong —
try the original URL with `?w=1920&q=80` query params, or find a
different CDN path.

**Filename convention:** `<beat>_<story-slug>.<ext>`. Slug = 1-3 word
topic, lowercase, hyphens. Example: `ai_amd_anthropic_chip.jpg`,
`f1_norris_pole.jpg`. The renderer looks for exact paths in the
manifest, so the filename is the contract.

**Record in JSON:**

```json
"image": {
  "kind": "downloaded",
  "local_path": "/home/arctic/.hermes/data/hermes-times-v4/assets/ai_amd_helios_rack.jpg",
  "source_url": "https://cdn.mos.cms.futurecdn.net/gvXCCQAKgWmrVhBgZmEQzG.jpg",
  "credit": "ITPro / Jane McCallion"
}
```

`source_url` + `credit` are mandatory citation hygiene. Without them
the orchestrator's renderer refuses to use the image.

### 6. Return the JSON package — nothing else

The leaf's final message is a JSON object (no prose, no preamble, no
markdown wrapper). Schema is usually:

```json
{
  "beat": "<beat_name>",
  "stories": [
    {
      "rank": 1,
      "headline": "...",
      "dek": "...",
      "body": "...",
      "source_url": "...",
      "source_name": "...",
      "published_at": "<ISO 8601 UTC>",
      "primary": true,
      "image": { "kind": "downloaded", "local_path": "...", "source_url": "...", "credit": "..." }
    }
  ]
}
```

Field rules:

- `headline`: 6-12 words, declarative, no hedging.
- `dek`: 15-25 words, why a reader should care.
- `body`: 2-3 short paragraphs, ~150 words total. No "according to
  sources" — name them.
- `published_at`: ISO 8601 with `Z`. Always UTC, not IST.
- `primary`: `true` if the source is the vendor's own publication;
  `false` if it's a roundup or third-party report.
- `image.kind`: `"downloaded"` or `"generated"`. Never both.

## Pitfalls a leaf commonly hits

- **Trusting the leaf's cutoff claim instead of checking its timestamps.** A leaf can return valid JSON, confidently say every story is inside the window, and still include an older item because its date arithmetic is wrong. The orchestrator must parse every `published_at` itself and compare it against one explicit UTC cutoff before editing the beat. Reject out-of-window rows even when the summary says they passed. Use timezone-aware comparison, not date-string prefixes:
  ```python
  from datetime import datetime
  cutoff = datetime.fromisoformat("2026-07-26T00:31:00+00:00")
  kept = [s for s in stories if datetime.fromisoformat(s["published_at"].replace("Z", "+00:00")) >= cutoff]
  ```
  Also normalize `primary`: `true` means the canonical owner published it (vendor, regulator, team, arXiv/repository author). Reuters, WSJ, Axios, TechCrunch, MARCA, and other original reporting are still third-party sources and therefore `primary: false` under this contract. A reputable outlet can be publishable without being primary.

- **Searching by date and trusting the aggregators.** Most
  search-results pages surface the *coverage* date, not the *event*
  date. The event is older than you think. Click through to the primary
  source to confirm.
- **Quote from a secondary source treated as primary.** If the only
  place a CEO quote appears is a TechCrunch article, the TechCrunch
  article is primary for the quote, but the **event** is whatever the
  vendor's press release says it is. Cite the vendor's URL for the
  event; the secondary URL is for the quote attribution.
- **Image URL is the "share card" not the article photo.** Press
  release hero images often sit at `/wp-content/uploads/2026/MM/share.jpg`
  while the actual photo is at `/wp-content/uploads/2026/MM/photo.jpg`.
  Try both.
- **The `/news` index is missing the most recent story.** Vendor
  indexes update lazily. If you suspect a recent launch (homepage hero,
  social post, conference talk), guess the URL slug directly:
  `<vendor>.com/news/<product-name>-launch` or `<vendor>.com/blog/<year>/<month>/<title>/`.
- **Two stories that share a vendor get merged.** "AMD Helios rack
  ships" and "AMD signs $5B Anthropic deal" are *different* stories.
  Rank them separately. Combining them into one mega-story loses
  narrative density and confuses the editor.
- **Body word count is a soft minimum, not a ceiling.** Aim for 130-160
  words for the lead body. Short bodies leave dead air in the layout;
  long bodies force the editor to cut. Write the body, then trim.
- **Body count overruns are an AI over-write failure mode.** First-draft
  bodies from the model routinely come in at 230–280 words against a
  ~150 target. The padding hides in the second paragraph (the
  "implications" paragraph). Before returning the JSON, count words
  on every body and trim the second paragraph by a third. Hard cap
  is 200 words per body — anything past that is editorial bloat the
  orchestrator will cut mid-sentence and damage the layout.
- **Missing `art_source_url` + `art_credit`.** Without these two
  fields, the renderer refuses the image and the story runs without
  art. Double-check before returning the JSON.
- **Generating art as a fallback when a real photo exists.** The brief
  says "real photo preferred; generate only if none works." A real
  photo of a CEO at a podium or a chip in a rack is almost always
  findable. Spend 3 more minutes searching before falling back to
  mmx image generate.
- **One downloaded photo, N stories — the F1 lead-photo trap.** If
  only one real photo is downloadable for a beat (e.g. Formula 1
  races that ran a week ago have a podium shot on the lead driver
  and that's it), it is acceptable to attach that single photo to
  the lead story's `image` block AND let the orchestrator reuse
  the same path as the section hero. Do **not** attach the same
  `local_path` to every story in the beat — the renderer will then
  show the same photo on every card, which reads as a broken
  layout. Pattern that works: download one photo → put it on
  `rank: 1` only → omit `image` on `rank: 2..N`. The orchestrator
  then uses the rank-1 photo as the section `art_uri` and the
  remaining cards are text-only.

- **One photo, two stories — the silent reuse trap.** Distinct from
  the F1 lead-photo trap: a leaf downloads a single thumbnail
  (e.g. an arXiv social-hero or a HF model card thumbnail) and
  references the same `local_path` for **two different story ranks**
  in the same beat. The renderer doesn't crash — it just shows the
  same hero image twice, which reads as duplicate art or a
  duplicated story. **Always keep a set of emitted `local_path`s**
  while drafting the JSON; each non-omitted `image` block must
  reference a unique path. If you only have one image, attach it
  to `rank: 1` and drop the `image` field on `rank: 2..N` (the
  F1 lead-photo trap above). Cheaper than failing the
  orchestrator's pre-render existence/duplicate check.

- **`image: ""` (empty string) is a contract break.** The canonical
  "no photo" sentinel is `image: {}` or the field omitted entirely.
  Some leaves return a literal empty string, which is easy to miss
  while drafting JSON because Python's `len(json_dict)` still counts
  the key. **Treat any `image.kind` that is not exactly `"downloaded"`
  or `"generated"` as broken.** Either download a real photo, call
  `mmx image generate` and rewrite the field with `kind: "generated"`,
  or omit the `image` key on that story entirely. Never ship an empty
  image block — the orchestrator's manifest cannot distinguish it
  from `image: {}`, and the renderer silently renders a missing
  picture.

- **`kind: "generated"` requires proof you actually called mmx.**
  The renderer trusts `kind` — `downloaded` and `generated` both
  render. A leaf short on time sometimes emits a `generated` block
  pointing at a path that exists from a previous day's run, without
  ever invoking `mmx image generate`. The file size, the JPEG format,
  the `kind` field all look fine — but the image has nothing to do
  with the story. **The leaf must call `mmx image generate --prompt
  <NEW_PROMPT_FOR_THIS_STORY> --out-dir ~/.hermes/data/hermes-times-v4/assets/`**
  before writing `kind: "generated"`. The orchestrator's pre-render
  audit catches this (file mtime is older than today's manifest
  draft, but the slug is `generated`-tagged), so the leaf saves
  itself a re-render by being honest: if you didn't call mmx, omit
  `kind: "generated"` and either drop the story, leave `image`
  empty, or stop and run mmx yourself.
- **Don't write the beat JSON to `output/<beat>_beat.json` from inside the leaf.**
  That path is reserved for the orchestrator's consolidated manifest
  (and for the previous-day archive of yesterday's run, per the
  "yesterday's leaf JSON" pitfall below). A leaf that writes there
  clobbers yesterday's archive and confuses continuity reads. Emit
  the JSON as your final assistant message; the orchestrator persists
  it to the right place. The leaf's only filesystem writes should
  be to `~/.hermes/data/hermes-times-v4/assets/`.
- **Same-vendor ≠ same-story.** When a single AI Summit drops multiple
  deals (e.g. NVIDIA + SK, NVIDIA + NAVER, NVIDIA + SSI on the same
  day), rank them as separate stories. The "two stories share a
  vendor" rule applies when the same deal is being reported twice
  (recap vs press release), not when one vendor sits at the centre of
  multiple distinct transactions with different counterparties. Each
  deal is its own agreement and gets its own rank.
- **arXiv API XML can hand you the feed title instead of the entry
  title.** When you query `export.arxiv.org/api/query?id_list=<id>`
  and walk entries with `xml.etree.ElementTree`, it is very easy to
  silently pull the *feed-level* `<title>` (which is the literal
  string `arXiv Query: search_query=...&id_list=...`) instead of the
  entry's own `<title>`. The XML is well-formed; the bug is in how
  your code asks for it — namespace-prefix matching via
  `e.findtext('a:title', namespaces=ns)` can return the wrong node
  depending on how the parser binds the default Atom namespace. The
  leaf-visible symptom is a paper whose headline looks like an HTTP
  URL with `search_query=` and `id_list=` query strings. **Fix:**
  never trust the arXiv API XML for titles/abstracts/authors. Use
  `web_extract` on `https://arxiv.org/abs/<id>` to get the clean
  metadata. The arXiv API is fine for *filtering* (date range,
  category) but not for *parsing*. Same applies to OAI-PMH and the
  RSS feed — both embed entity-encoded LaTeX that needs
  post-processing.
- **arXiv date field: `updated` vs `published`.** When filtering the
  daily arXiv listings for a 72-hour window, compare against
  `<entry>/<updated>` not `<entry>/<published>`. `published` only
  fires on the *original* submission; a paper submitted 9 days ago
  that got a substantial v2 yesterday shows `updated` = yesterday and
  `published` = 9 days ago. For a daily paper you want
  "recently-active" coverage, so `updated` is the correct filter.
  Without this, a high-profile revision can fall outside your window
  and you lose the story.

## What the leaf does NOT do

- Spawn further sub-agents. You are a leaf. If the beat seems too
  large for one agent, the orchestrator sliced wrong — return what you
  have and flag it.
- Touch `render.py` / `style.css` / `manifest.json`. The orchestrator
  composes those.
- Send to Telegram. The orchestrator owns delivery.
- Modify any path outside `~/.hermes/data/hermes-times-v4/`. The
  repo dir at `~/.hermes/scripts/hermes-times-v4/` is read-only for
  you.

## Recovering a leaf's full output from the delegation cache

If a previous leaf run failed mid-write, finished but the JSON went
into the wrong place, or you need to inspect what a leaf actually
returned (e.g. while debugging an editor step), the leaf's **complete
JSON lives in the orchestrator's per-task summary file**:

```text
~/.hermes/cache/delegation/subagent-summary-<task_index>-<timestamp>_<pid>.txt
```

Filename pattern: `subagent-summary-{task_index}-{YYYYMMDD_HHMMSS}_{pid}.txt`.
The file is plain JSON (usually pretty-printed across many lines) and
is the **exact final assistant message** of the leaf, captured before
the orchestrator moves on. When in doubt, this file is more
authoritative than `task-N.log` in the same directory.

**When to use this vs. the live transcript:**

| Need | Source |
| --- | --- |
| Full leaf JSON, all stories, all bodies, all image blocks | `subagent-summary-<n>-<ts>_<pid>.txt` |
| Tool call sequence + how the leaf reasoned | `live/deleg_<id>/task-<n>.log` |
| Goal / kickoff text + exit reason | `live/deleg_<id>/manifest.json` |

**The live transcript is truncated.** Every long assistant message is
saved as `<prefix> …(+N chars)` where N is the hidden character count.
Body content of 3000+ characters is routinely lost this way. The
truncation is *not* recoverable from earlier assistant messages in the
same transcript — earlier turns do not contain the full body either.
**Always check the subagent-summary file before declaring a leaf's
output unrecoverable.**

**Don't confuse today's run with yesterday's.** Previous runs leave
their leaf outputs at:

```text
~/.hermes/scripts/hermes-times-v4/output/<beat>_beat.json
```

Those files have the leaf schema (which may differ slightly from
today's contract) and *different headlines* — they reflect whatever the
beat was on the previous day's run, not today. Before reading them,
check the file mtime against `deleg_*/manifest.json` `started` for
today's delegation. If the delegation timestamp is later than the file
mtime, the file is stale and **must not be used** as today's leaf
output — it will silently introduce yesterday's stories into today's
edition. The right source for today's leaf output is the
subagent-summary file whose timestamp matches the delegation's
`completed` field.

**Leaf-to-orchestrator image schema mapping.** Leaves have historically
emitted images under two slightly different schemas. Map them to the
canonical orchestrator schema `{kind, local_path, source_url, credit}`
before writing the orchestrator's manifest:

| Leaf emits | Canonical field |
| --- | --- |
| `path` | `local_path` |
| `art_source_url` | `source_url` |
| `art_credit` | `credit` |
| `caption` | (drop — render layer composes captions from headline + credit) |
| `image: ""` (empty string) | use the strict mapping: this is a *broken* sentinel, not a missing image. Either re-download, regenerate, or drop the `image` key from that story. Never propagate `""` into the manifest. |
| `image: {}` (empty object) | emit `"image": null` in the manifest (renderer handles absent art) |
| missing `image` key | emit `"image": null` in the manifest (renderer handles absent art) |

Stories without an `image` block in the leaf output should become
`"image": null` in the orchestrator manifest, not omitted — keeping
the key stable means the renderer can iterate `stories[*].image`
without null-checks.

## F1 / motorsport leaves

When the beat is F1 (or any FIA championship — F2, F3, WEC), load
`references/f1-leaf-beat-playbook.md` first. It covers:

- formula1.com as a Next.js app (dates in streamed JSON, hero image IDs
  in `og:image` / preload links, credits hidden in `__next_f.push()`)
- FIA.com Drupal extraction and the **timezone trap** (CEST `+02:00`,
  not UTC)
- Cloudinary URL transforms for guaranteed JPEG downloads
- Standard 2026 steward penalty guidelines

The general 6-step loop above still applies; the playbook adds the
beat-specific extraction layer on top.

## Reporting back

Print (in this order):

1. Total stories returned + 1-line headline per rank.
2. Image status: N real downloaded, M generated (and which).
3. Stories excluded from the window (with reason), so the editor can
   override knowingly.
4. Anything ambiguous the editor should decide (e.g. "AMD Helios and
   AMD-Anthropic are two separate stories or one?").
5. **Set of `local_path` values emitted across this beat** — duplicate
   detection belongs to the leaf, not the orchestrator. If you
   referenced the same path twice (across ranks or within one story
   + the section hero), say so explicitly so the editor knows before
   the pre-render check.
