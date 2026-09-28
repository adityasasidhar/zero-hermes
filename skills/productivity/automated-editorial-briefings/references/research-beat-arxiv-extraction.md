# Research beat — arXiv extraction recipe

Companion to `leaf-beat-reporter-workflow.md`. Load this in addition to
the general leaf playbook when the beat is `research` (or any beat that
pulls from arXiv). Covers the operational quirks of `export.arxiv.org`,
`arxiv.org/abs/...`, and `arxiv.org/html/...` that are easy to trip over
and not documented in one place.

## 1. Discovery: arXiv API query parameters that matter

The arXiv API at `http://export.arxiv.org/api/query` accepts a few
sort/filter parameters. The non-obvious one is `sortBy`. The valid
values are exactly `relevance`, `lastUpdatedDate`, and `submittedDate`.
**There is no `updatedDate` value** — calling the API with
`sortBy=updatedDate` returns HTTP 400, not an empty result. Future leaves
will reliably hit this trap because "last updated" is the natural thing
to type.

For a daily Research beat you want `lastUpdatedDate`, not
`submittedDate`: a paper submitted 9 days ago that got a substantial
v2 yesterday shows up under `lastUpdatedDate` (the version of the paper
a reader currently sees) but not under `submittedDate`. This is the
same `updated` vs `published` pitfall called out in the general
leaf playbook under "arXiv date field" — the API field name is
`lastUpdatedDate`, but the *meaning* is the per-entry `<updated>`.

```bash
# CORRECT — what you want for a 72-hour window
http://export.arxiv.org/api/query?search_query=cat:cs.AI&start=0&max_results=120&sortBy=lastUpdatedDate&sortOrder=descending

# WRONG — returns HTTP 400 (confirmed 2026-07-29)
https://export.arxiv.org/api/query?search_query=cat:cs.AI&start=0&max_results=80&sortBy=updatedDate&sortOrder=descending
```

## 2. Discovery: HTTPS vs HTTP for the API

The arXiv API listens on both `http://` and `https://`. **HTTPS works
for some query shapes but returns HTTP 400 for `search_query=cat:...`
with sortBy options** — even though it accepts the same query on plain
HTTP. The failure mode is silent: HTTP 400 with no useful body.

Always hit the API over `http://export.arxiv.org`. Use HTTPS only for
the public-facing `arxiv.org/abs/<id>` and `arxiv.org/html/<id>` pages.

```bash
# Works
curl -A "Mozilla/5.0" "http://export.arxiv.org/api/query?search_query=cat:cs.LG&start=0&max_results=50&sortBy=lastUpdatedDate&sortOrder=descending"

# Same query on HTTPS — HTTP 400
curl "https://export.arxiv.org/api/query?search_query=cat:cs.LG&..."
```

## 3. Window filter: use `<updated>`, not `<published>`

When parsing the Atom feed, filter entries by the `<updated>` field,
not `<published>`. `<published>` only fires on the original v1
submission; `<updated>` reflects the current live version. This is
the difference between "papers submitted in the last 72 hours" and
"papers that became meaningfully news in the last 72 hours" (a v2
revision of an important paper belongs in the latter).

```python
from datetime import datetime, timezone
CUTOFF = datetime(2026, 7, 26, 0, 31, 0, tzinfo=timezone.utc)
for entry in entries:
    updated = entry.findtext('a:updated', namespaces=ns)
    if datetime.fromisoformat(updated.replace('Z', '+00:00')) >= CUTOFF:
        ...  # in window
```

## 4. Author extraction: regex beats ElementTree

The arXiv API XML is technically well-formed but namespace-prefix
matching via `xml.etree.ElementTree` is fragile enough that it is easy
to silently read the *feed-level* `<title>` (which says
`arXiv Query: search_query=...`) instead of the entry's own
`<title>`. The general leaf playbook already flags this; the
practical workaround that works is to **regex-extract on the raw bytes**:

```python
import urllib.request, re

data = urllib.request.urlopen(url).read()
entries = re.findall(rb'<entry>(.*?)</entry>', data, re.DOTALL)
for entry in entries:
    arxiv_id = re.search(rb'<id>http://arxiv\.org/abs/(\d+\.\d+)(v\d+)?</id>', entry).group(1).decode()
    title = ' '.join(re.search(rb'<title>(.*?)</title>', entry, re.DOTALL).group(1).decode().split())
    summary = ' '.join(re.search(rb'<summary>(.*?)</summary>', entry, re.DOTALL).group(1).decode().split())
    authors = [a.decode().strip() for a in re.findall(rb'<author>\s*<name>(.*?)</name>', entry)]
```

For very large author lists (e.g. Kimi Team with 300+ named
contributors), the conventional citation is the consortium name
("Kimi Team"), not the individual list. The API returns both — pick
the first author (the consortium) and acknowledge the rest in prose.

## 5. Abstract content: arxiv.org/abs vs arxiv.org/html

The leaf playbook recommends `web_extract` on `arxiv.org/abs/<id>`
for "clean markdown with title, authors, abstract." This only works
for the metadata block (title, authors, subjects, comments). **The
abs page does NOT contain the paper abstract as visible content** —
`web_extract` on `https://arxiv.org/abs/<id>` returns a page with
title + metadata but no prose. The actual abstract lives behind a
different path.

For prose you have three routes, in order of preference:

1. **`https://arxiv.org/html/<id>v<N>`** — if the source is HTML/LaTeX
   the paper has a full HTML rendering with the abstract in the
   `<section class="abstract">`. ~40% of recent CS papers have this.
   `web_extract` on the html URL gives you the full abstract +
   intro + figures.
2. **arXiv API with `<id>` filter** — parse the summary from the Atom
   feed as in section 4. Always works but the summary is plain text
   wrapped in one `<summary>` tag (no section headers, no LaTeX
   formatting).
3. **`web_extract` on `https://arxiv.org/pdf/<id>`** — for papers that
   are PDF-only and have no HTML rendering. Falls back to the PDF
   text extraction; quality is variable.

A `404` from `https://arxiv.org/html/<id>` means the paper has no
HTML rendering — use route 2.

## 6. Image strategy for Research-beat papers

The leaf playbook's image cheat sheet already lists the HF paper
thumbnail CDN (`cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png`)
as the primary choice. **The CDN is flaky in practice** — roughly
30-40% of arXiv IDs return HTTP 504 / connection timeout even when
the paper exists and HF has indexed it. Per the playbook, retry 2-3
times then fall back. Three clean fallback tiers, in order:

### Tier 1: arxiv.org/html/<id>v<N>/x1.png

The lead figure of any paper with HTML rendering is at a stable URL:

```text
https://arxiv.org/html/<id>v<N>/x1.png
```

This is the figure rendered above the abstract in the HTML version.
Verified working 2026-07-29 for `2607.22465v2` (TRACE-Router) and
`2607.19363v2` (AdaRoPE). 136KB and 71KB respectively. No special
headers needed.

### Tier 2: arxiv.org/html/<id>v<N>/figures/.../*.png

Sub-figures live at `<id>v<N>/figures/<subdir>/<name>.png`. To
discover them, fetch the HTML once and grep:

```python
import urllib.request, re
html = urllib.request.urlopen('https://arxiv.org/html/2607.19363v2').read().decode()
figure_urls = re.findall(r'(?:src|href)="([^"]*\.png)"', html)
figure_urls = [u for u in figure_urls if not u.startswith('/static/') and not u.startswith('/icons/')]
```

For AdaRoPE, the first figure is
`figures/synthetic/synthetic_standard_rope_utilization.png` — a clean,
self-contained plot that's better than the lead figure for a leaf
context (the lead figure often shows the model architecture which
requires caption context to parse).

### Tier 3: `mmx image generate`

For papers with no HTML rendering AND a flaky HF thumbnail, generate
an editorial illustration via `mmx image generate`. The pattern that
worked:

```bash
mmx image generate \
  --prompt "Editorial illustration for an interpretability research paper on sparse autoencoders (SAEs) in large language models. A 3D wireframe neural network with sparse glowing pathways in deep blue and teal, surrounded by floating geometric feature-vectors in cyan and amber. Show feature-effect geometry: a stylized 'logit cloud' of small translucent tetrahedra hovering around the network. Dark navy background. Editorial science style, clean, sophisticated, similar to MIT Technology Review covers." \
  --aspect-ratio 16:9 \
  --out-dir /tmp/sae_art/ \
  --out-prefix sae_fega
```

Output defaults to JPEG with `_NNN.jpg` suffix (e.g.
`sae_fega_001.jpg`). The leaf must `mv` or `cp` the generated file
into `~/.hermes/data/hermes-times-v4/assets/<slug>.<ext>` and set
`kind: "generated"` with an honest credit string that names mmx as
the generator. **Always vision-review** the generated frame for
distorted text / mangled shapes before recording it — the leaf
playbook flags this and it bit in the 2026-07-28 v4 Issue 3 (AI
cover green-blob rejection).

### Tier 4: omit

If even mmx is unavailable or the prompt produces garbage, drop the
story. A filler image is worse than a missing image.

## 7. Discovery recipe that works end-to-end

Putting it all together, the recipe that produced a clean Research
beat package on 2026-07-29:

```python
import urllib.request, re
from datetime import datetime, timezone

CUTOFF = datetime(2026, 7, 26, 0, 31, 0, tzinfo=timezone.utc)
ns_atom = b'http://www.w3.org/2005/Atom'

# Step 1: Pull listings over HTTP, not HTTPS
all_entries = {}
for cat in ['cs.AI', 'cs.LG', 'cs.CL', 'cs.CV', 'cs.RO']:
    url = f'http://export.arxiv.org/api/query?search_query=cat:{cat}&start=0&max_results=120&sortBy=lastUpdatedDate&sortOrder=descending'
    data = urllib.request.urlopen(url, timeout=60).read()
    for entry in re.findall(rb'<entry>(.*?)</entry>', data, re.DOTALL):
        arxiv_id = re.search(rb'<id>http://arxiv\.org/abs/(\d+\.\d+)(v\d+)?</id>', entry).group(1).decode()
        updated = re.search(rb'<updated>(.*?)</updated>', entry).group(1).decode()
        if datetime.fromisoformat(updated.replace('Z', '+00:00')) < CUTOFF:
            continue
        all_entries[arxiv_id] = {
            'id': arxiv_id,
            'updated': updated,
            'title': ' '.join(re.search(rb'<title>(.*?)</title>', entry, re.DOTALL).group(1).decode().split()),
            'authors': [a.decode().strip() for a in re.findall(rb'<author>\s*<name>(.*?)</name>', entry)],
        }

# Step 2: Filter out yesterday's coverage (dedupe against prior run)
# Step 3: Pick 5 candidates that hit at least 2 of the 6 categories
# Step 4: web_extract on arxiv.org/html/<id>v<N> for prose
# Step 5: image — try HF thumbnail, then arxiv html x1.png, then mmx
# Step 6: body 130-180 words, 2-3 paragraphs, no invented numbers
```

## 8. Pitfalls specific to the Research beat

- **arXiv IDs are not stable URLs for the latest version.** A v2 of a
  paper has ID `2607.12345v2`; the canonical abstract URL is
  `arxiv.org/abs/2607.12345` (no version). Always strip the version
  suffix before linking in the manifest.
- **The "comments" field is gold.** Most arXiv submissions include
  venue / workshop / page-count / figure-count in the `<arxiv:comment>`
  field. For a leaf, this is the cheapest way to flag "ICML 2026
  acceptance" or "8 pages, 4 figures" without opening the PDF.
- **Tech-report papers have hundreds of authors.** The Kimi Team, the
  Llama Team, etc. The conventional citation is the team name (one
  author slot) plus optionally "et al." The leaf playbook's author
  formatting ("Author1, Author2, Author3, dropping et al.") is for
  papers with 1-6 authors; tech reports follow a different norm.
  Document the choice in the leaf's report-back notes.
- **The Kimi K3 / DeepSeek / Qwen-style update pattern is recurring.**
  Big Chinese-lab releases land with a v1 the day before and a v2
  the morning after with minor corrections. Filtering by `<updated>`
  catches both, which is what you want — the news is the v1 release,
  and the v2 update is metadata confirming the paper is still active.
- **HF "Papers" page often 404s for the ID.** `huggingface.co/papers/<id>`
  is not a stable URL — it only exists for papers HF has manually
  curated into the daily list. Even if the paper is in HF's model
  hub, `huggingface.co/papers/<id>` may 404. Treat the URL as a
  fallback source link, not a canonical one. The canonical is
  always `arxiv.org/abs/<id>`.

## 9. What to record in the report-back

Per the general leaf playbook, a Research beat report-back should
include:

1. Total stories + 1-line headline per rank.
2. Image status: N real downloaded (with source URL), M generated
   (with mmx prompt).
3. **Window filter result:** how many candidates were in the 72-hour
   window before deduplication, how many after, and how many were
   excluded as duplicates of yesterday's coverage. This lets the
   editor spot if the window is too narrow or the dedupe is too
   aggressive.
4. Categories covered (mechanistic interp, agents/RL, arch, evals,
   multimodal, open-source releases) — confirm ≥2 covered.
5. Set of emitted `local_path`s (the leaf's responsibility per the
   general playbook).
