# Research Beat — Discovery & Sourcing Patterns

The research beat surfaces arXiv papers and academic releases that landed in the last 72 hours. Unlike the HF beat (which centers on `huggingface.co` itself), the research beat treats **arXiv as the canonical source** and Hugging Face as the **discovery surface** — papers get on the HF daily listing ~12–24h after arXiv submission, and HF upvotes serve as a community-significance signal but never as the citation source.

## Primary discovery surfaces (in order)

| Surface | URL | What it gives you |
|---|---|---|
| HF daily papers (HTML) | `https://huggingface.co/papers/date/<YYYY-MM-DD>` | The freshest in-window curated listing (10–40 papers per day). This is the single best signal — AK's curation already filtered for "what ML builders are clicking." |
| HF daily papers index | `https://huggingface.co/papers` | The current default listing; the per-day page is more useful for date-anchored work |
| arXiv listing API | `https://export.arxiv.org/api/query?search_query=cat:cs.{AI,CL,LG,CV}&sortBy=submittedDate&sortOrder=descending&max_results=20` | Raw, unfiltered, freshest-by-submission-time. Pull from `cs.AI`, `cs.CL`, `cs.LG`, `cs.CV` for ML research; add `cs.RO` for robotics, `cs.IR` for retrieval. |
| Semantic Scholar paper details | `https://api.semanticscholar.org/graph/v1/paper/arXiv:<id>?fields=title,authors,citationCount,influentialCitationCount,year` | Citation counts (run after the 72-hour window — a brand-new paper has ~0 citations, which is normal). |
| HF `/api/papers/{arxiv_id}` | `https://huggingface.co/api/papers/2607.28227` | Upvote count, canonical title, `publishedAt` date. Reliable for per-ID lookups. |

**Discovery order:** HF daily papers page → arXiv API for freshest raw feed → cross-reference both → verify each candidate with `web_extract` on `arxiv.org/html/<id>v1`.

## What arXiv ID format means in 2026

arXiv IDs are `YYMM.NNNNN` where YY is year, MM is month (zero-padded), and NNNNN is a 5-digit sequence within the month. The 5-digit suffix is **roughly date-ordered within the month** — papers submitted in early August have lower 5-digit suffixes than papers submitted late August. Within a given month, **the suffix is a strong freshness signal**: papers with `2608.09XXX` and `2608.10XXX` were almost certainly submitted in the last 1–3 days of August 2026. Use this when filtering search results — if the task brief says "last 72 hours" and today is Aug 12, a paper with ID `2608.04419` (Aug 4) is automatically out-of-window and shouldn't be ranked, while `2608.09888` (Aug 10) is comfortably in-window.

Examples from this session's window:
- `2608.09888` (BDH-CQ) = Aug 10 2026 — in 72h window anchored Aug 12
- `2608.09867` (Stealing Reasoning Traces) = Aug 10 2026 — in window
- `2608.09119` (Motif 3) = Aug 10 2026 — in window
- `2608.09853` (RynnValue) = Aug 10 2026 — in window
- `2608.07594` (Steerling) = Aug 6 2026 — borderline (5 days); keep only with strong editorial justification

**Quick rejection filter:** before reading an abstract, check the first 4 digits of the arxiv ID against the window month and the 5-digit suffix against "is this week's submission range." If the suffix is more than ~1500 below the maximum seen on today's HF listing, drop it without reading. This saves 10–15 minutes of per-paper abstract extraction on obviously out-of-window candidates.

When evaluating "is this paper in the 72-hour window?", parse the first 4 digits of the arxiv ID to get `YY-MM`, then verify with the arxiv API `<published>` field. **Do not trust HF's `publishedAt` for window evaluation** — it's the day HF indexed the paper (often 12–24h after arxiv submission), not the submission day itself.

## Pinning `published_at` and authors via the arxiv API

`arxiv.org/abs/<id>` does NOT expose its submission date to `web_extract`'s HTML parser — the meta tags and submission-history block stay hidden. For a reliable ISO-8601 submission timestamp AND author list, query the arxiv API directly:

```bash
# Batched: up to ~10 IDs per call, returns one Atom XML feed with <entry> per paper
curl -sL "https://export.arxiv.org/api/query?id_list=2608.09888,2608.09119,2608.09853,2608.09867,2608.07594" \
  | python3 -c "
import sys, re
xml = sys.stdin.read()
entries = re.split(r'<entry>', xml)
for e in entries[1:]:
    title = re.search(r'<title>([^<]+)</title>', e)
    pub = re.search(r'<published>([^<]+)</published>', e)
    authors = re.findall(r'<name>([^<]+)</name>', e)[:5]
    aid = re.search(r'abs/([^\"v]+)', e)
    n_total = len(re.findall(r'<name>', e))
    print(f'{aid.group(1) if aid else \"?\"} | {pub.group(1) if pub else \"?\"}')
    print(f'  title: {title.group(1) if title else \"?\"}')
    print(f'  authors: {\", \".join(authors)}{\"...\" if n_total>5 else \"\"}  ({n_total} total)')
"
```

The API returns Atom XML; each `<entry>` has an `<id>` (`http://arxiv.org/abs/<id>v1`), an ISO-8601 `<published>` field already in UTC, and a full `<author><name>` list per paper. **Use this for both window evaluation, the JSON's `published_at`, AND the `authors` field** — the API is the canonical source, and it's batch-friendly (up to ~10 IDs per call). This avoids two pitfalls at once: the existing one where leaves copy HF's `publishedAt` and end up off by 12-24h, and the new one where leaves hand-type an author list from the abstract and miss a contributor.

For very long author lists (e.g. corporate technical reports with 30+ contributors), truncate to the first 4–5 names plus an organizational lead-in for the JSON's `authors` field — confirmed pattern (research beat 2026-08-12, Motif 3 / 2608.09119): `"Motif Technologies (Junghwan Lim, Joon Son Chung, Sungmin Lee, Wai Ting Cheung et al.)"`.

## Source hierarchy for research stories

1. **arXiv abs page** (`https://arxiv.org/abs/<id>`) — primary source for the paper itself. Use this as `source_url` unless the paper has an institutional project page that includes benchmarks, demos, or model weights not on arXiv.
2. **arXiv HTML** (`https://arxiv.org/html/<id>v1`) — for verifying body facts, benchmark numbers, and architecture details before writing the body.
3. **Project pages** (`<lab>.github.io/<project>`) — secondary source, use as `source_url` only if it adds material not on arXiv (interactive demos, larger eval tables, code repos).
4. **HF daily papers page** — discovery only. Cite it in the body if relevant ("#3 on HF's daily papers"), but never as `source_url`.
5. **Lab blog posts** — secondary; cite only if the lab published an extended blog with benchmarks beyond the paper.
6. **Never** cite Wikipedia, "Top 10" roundups, aggregator articles, or marketing pages as `source_url`.

For the research beat, `source_name` follows the pattern `"<Lab or Lead Institution> via arXiv"` — e.g. `"Alibaba Tongyi Lab via arXiv"`, `"MemTensor via arXiv"`, `"CASIA via arXiv"`, `"Microsoft Research via arXiv"`. The `via arXiv` suffix is the contractual source-type marker; the lab name is the editorial attribution.

## The HF listing-lag trap (applies here too)

HF daily listings lag real time by ~12–24h. Querying `/papers/date/2026-08-01` on Aug 1 returned the same listing as `/papers/date/2026-07-31`. **For a 72-hour window anchored on today**, the freshest in-window batch is usually yesterday's HF listing, not today's. The `published_at` in the JSON must come from arXiv's `<meta name="citation_date">`, not from HF's `publishedAt`. If you copy HF's date, you'll date the story one day late and it may drop out of the window for downstream reads.

**The arXiv listing API also lags.** `export.arxiv.org/api/query?sortBy=submittedDate&sortOrder=descending` returns papers submitted ~2–3 days ago at the top of the listing, not today's. If the task brief says "find papers from the last 72 hours" and today is Aug 9, expect the newest visible papers to be dated Aug 6 or Aug 7 — that's normal, the API is not broken. For same-day coverage, the HF daily papers page is the most current signal.

**Don't trust the `/api/papers?date=YYYY-MM-DD` endpoint.** As of late Jul 2026 it returned the same default data regardless of date (no 400 error, just wrong results). Use the per-day HTML page or `/api/papers/{id}` (which is reliable).

## Image patterns for research stories

**Default (most reliable AND usually best-looking): the paper's own figures on `arxiv.org/html/<id>v1`.** Modern LaTeXML-rendered arxiv HTML pages have inline images under predictable paths, e.g. `<id>v1/x1.png`, `<id>v1/x2.png`, ... for auto-numbered figures, or named figures like `<id>v1/figures/framework.png`, `<id>v1/fig1_architecture.png`. These are the actual figures the authors submitted — no curation, no CDN, served directly by arxiv.org. **In practice the arxiv figure is almost always better art than the HF CDN title card** because it's the paper's actual concept/framework diagram rather than a screenshot of the title page with author affiliations stamped on it. Confirmed pattern (research beat 2026-08-11): SkillProx `2608.07449`'s `x1.png` was a clean colored forward/backward loop diagram that vision-verified as on-topic, while the HF thumbnail for the same paper returned a 504 and would have been a generic first-page screenshot anyway.

**Default recipe — try arxiv first, fall back to HF CDN:**

```bash
# 1. Probe arxiv HTML to discover the figure filename (often x1.png but varies)
curl -sS -L --max-time 30 "https://arxiv.org/html/<ID>v1" \
  | grep -oE '(src|href)="[^"]*\.(jpg|png|jpeg|gif)"' \
  | grep -vE 'icon|logo|static|favicon|funder' \
  | head -10

# 2. Download the first non-decoration figure
curl -sS -L --max-time 45 -o research_<id>.png "https://arxiv.org/html/<ID>v1/x1.png"
file --brief --mime-type research_<id>.png    # must be image/png
stat -c '%s' research_<id>.png                 # must be > 20480 (~20KB)

# 3. ONLY if arxiv HTML didn't render or x1.png is missing/corrupt, try the HF CDN
timeout 10 curl -sS -L -o research_<id>.png -w "%{http_code}\n" \
  "https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png"
```

**Secondary: paper thumbnails from the Hugging Face CDN** — `https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<arxiv_id>.png`. These render the paper's title card as a 1200×648 PNG (200–450 KB). They are fine when they work but the CDN is **intermittently unreliable** as of 2026-08 — observed failures: HTTP 504 on every request across consecutive papers in the same beat, plus full hangs. Don't block on them; treat as a fallback. The HF title card also reads as a screenshot of the abstract page rather than a teaser figure, which is a weaker visual for a research-beat card.

**Reliability update (research beat 2026-08-12):** the HF CDN worked 5/5 times for the Aug 12 beat — every paper returned a real PNG in the 200-450 KB range with no 504s. The "intermittent flakiness" caveat still applies as a hedge, but the CDN should be treated as the default rather than a last-resort fallback. Confirmed working recipe: `curl -sSL -o <slug>.png "https://cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png"` followed by `file --brief --mime-type <slug>.png` (must say `image/png`) and `stat -c '%s' <slug>.png` (must be > 20480). When the CDN works it returns a ~250-400 KB PNG in well under 5 seconds.

**Fallback: `mmx image generate`** with a prompt describing the paper's visual concept (architecture diagram, benchmark chart, etc.) — only when both arxiv figures and HF thumbnails fail, which should be rare.

## Body-length calibration for research beats

The umbrella's 130–200 word band holds, but research beats naturally run to the upper end (~190–200 words) because **concrete benchmark numbers carry the story**. A research body that names specific percentages, parameter counts, and dataset names will read tighter than a vague "researchers propose a new approach" body at the same word count. Use the second paragraph for benchmark numbers — that's where readers scan.

**Pattern that worked for this beat:**
- **Para 1 (~70 words):** what the paper does, in plain terms, with the headline number.
- **Para 2 (~70 words):** how it compares — specific competitor numbers and the gap (e.g. "vs Sora 2's 26.5, PhiZero scores 41.2").
- **Para 3 (~50 words):** what's released (code, weights, dataset) and what the next move is.

Trim the third paragraph first if you overshoot 200 words; trim the first if you need to drop redundant framing.

**Reality check from the 2026-08-12 beat:** first drafts came in at 207, 224, 198, 232, 224 words across five stories — every story ran at or above the 200-word cap. The skill's existing umbrella pitfall ("research-beat drafts run 30-60w over the 200-word cap even after one honest trim") is conservative; **a single honest trim pass is not enough for security-flavored or mechanism-explanation stories that need a 3-paragraph "what's deployed / why it matters" arc.** Two-pass trim discipline is required:

1. **Pass 1 (drop redundant numbers):** every comparative benchmark row you can cut without losing the headline gap, cut. One benchmark row per story; the rest is editorial padding.
2. **Pass 2 (compress paragraph 3):** "what's released and what the next move is" reads tighter as one sentence than as a paragraph. If you need a third paragraph for narrative flow, keep it under 40 words.

**Re-lint with `len(body.split())` after each pass.** Confirmed working final counts from the Aug 12 beat after two passes: 184, 191, 178, 195, 184 — all comfortably inside the 130–200 band. The visual-eyeballing step undercounts by 5–15w because the writer is already adapted to seeing the long version; trust the count, not the eye.

## Rank-1 candidate heuristic

For the research beat lead slot, strongest candidates are usually:
1. **Security/alignment paper with concrete attack vectors and downstream impact.** Highest urgency when the paper demonstrates an exploit that affects deployed systems and quantifies real-world damage (PII recovered, credentials harvested, models bypassed). The "would I open this today" bar comes from the deployment consequence, not the academic novelty. Confirmed pattern (research beat 2026-08-12): Stealing Reasoning Traces from Proprietary LLM APIs (2608.09867) led the package because it shows encrypted CoT from Anthropic/OpenAI/Google is cross-model decodable, with 367 PII artifacts and 182 credentials harvested from public repos — every agentic system shipping encrypted reasoning traces needs to know about this today.
2. **Training-free / fix-a-known-bug paper from a major lab.** Highest "would I open this today" weight for builders because the paper addresses a pain point builders already feel — no new training, just a better implementation that drops into existing pipelines. Confirmed pattern (research beat 2026-08-11): NVIDIA's WorldTrace (2608.07408) led the package because it's a training-free fix for an obvious failure mode (long-horizon KV-cache addressability in video world models) and every builder running video diffusion has hit that exact failure mode.
3. **Major-lab paper with same-day open release** (Alibaba, Microsoft, DeepSeek, Moonshot, Mistral, Allen AI). Biggest "click" weight because builders will look for weights/repro.
4. **Cross-cutting eval/benchmark paper** that resets a field's leaderboard (e.g. an "X beats GPT-5 at Y" headline).
5. **Architectural innovation with concrete numbers** (e.g. memory foundation model, new attention variant, novel training objective).
6. **Hot-topic community signal** (most-upvoted paper on HF daily listing that day).

If a major-lab paper exists in window, lead with it. If the only thing fresh is a benchmarking paper from a smaller lab, lead with that and rank the major-lab work at #2. **Security-flavored papers with concrete exploit numbers override the major-lab preference** — a paper that names a vulnerability affecting deployed systems is more time-sensitive than one that ships new capabilities.

## What research beats should NOT lead on

- **A long arxiv-only list with no clear single winner.** If five papers are roughly equivalent in upvotes/impact, do not invent a rank-1 by editorial gravity — pick the one with the clearest "why a builder cares" hook and rank accordingly. If none has a hook, return fewer stories.
- **A paper that hasn't been verified.** If you can't pull the abstract and at least one benchmark number, don't rank it. A research story without numbers is just a press release.
- **A paper whose window is ambiguous.** If a paper was submitted Jul 28 and HF indexed it Aug 1, treat the arxiv date as authoritative for window evaluation. If you can't pin a submission date, treat it as out of window.

## Example: 5-story research beat for 2026-08-03 (this session's output)

| # | Category | Story | `source_url` | `source_name` |
|---|---|---|---|---|
| 1 | Agents / tool use | Qwen-UI-Agent (Tongyi) beats frontier on real-device mobile | `arxiv.org/abs/2607.28227` | "Alibaba Tongyi Lab via arXiv" |
| 2 | Model architecture | Metis Memory Foundation Model (native memory in backbone) | `arxiv.org/abs/2607.26760` | "MemTensor via arXiv" |
| 3 | Multimodal / video | PhiZero (CASIA) reasons in physical language | `arxiv.org/abs/2607.28624` | "CASIA via arXiv" |
| 4 | Benchmarks / evals | BM25 Wins at Scale (USTC/Metastone) | `arxiv.org/abs/2607.26497` | "USTC / Metastone via arXiv" |
| 5 | Agents / RL | Echoverse (Microsoft Research) co-evolves envs | `arxiv.org/abs/2607.28074` | "Microsoft Research via arXiv" |

4 categories covered (agents/RL, model architecture, multimodal, benchmarks). All 5 papers submitted arXiv 2026-07-30, indexed by HF 2026-08-01, in the 72-hour window anchored on 2026-08-03 IST.

## Example: 5-story research beat for 2026-08-11 (confirmed working)

| # | Category | Story | `source_url` |
|---|---|---|---|
| 1 | Multimodal / video | NVIDIA WorldTrace (training-free fix for long-horizon video world models) | `arxiv.org/abs/2608.07408` |
| 2 | Architecture / training efficiency | Meta FAIR Skaling (coupled scaling-law exponent) | `arxiv.org/abs/2608.07222` |
| 3 | Agents / tool use | UT Austin + Snowflake ReASearch (agent-internalized optimizer) | `arxiv.org/abs/2608.06714` |
| 4 | Agents / RL | HKUST SkillProx (proximal-gradient skill evolution) | `arxiv.org/abs/2608.07449` |
| 5 | Multimodal / driving | HUST + Dongfeng SimWAM (video-as-training-signal driving planner) | `arxiv.org/abs/2608.07468` |

3 categories covered; all in the 72-hour window anchored on 2026-08-11 IST; none overlap yesterday's edition (Aug 10 used IDs up to 2608.06301). Image mix: 4 HF CDN title cards + 1 arxiv HTML figure (SkillProx `x1.png`, because HF CDN returned a 504 for that paper).

## Example: 5-story research beat for 2026-08-12 (confirmed working)

| # | Category | Story | `source_url` |
|---|---|---|---|
| 1 | Security / alignment | UK AISI Stealing Reasoning Traces (encrypted CoT cross-model decryption jailbreak) | `arxiv.org/abs/2608.09867` |
| 2 | Open-source model release | Motif 3 (314B MoE with GDLA, weights on HF) | `arxiv.org/abs/2608.09119` |
| 3 | Robotics | Alibaba DAMO RynnValue (temporal-distance value foundation model) | `arxiv.org/abs/2608.09853` |
| 4 | Interpretability | Guide Labs Scaling Inherently Interpretable LMs (Steerling-8B) | `arxiv.org/abs/2608.07594` |
| 5 | Architecture / training efficiency | Pathway BDH-CQ (recurrent latent reasoning + ARC-AGI-1 cost frontier) | `arxiv.org/abs/2608.09888` |

5 categories covered (security, open-source, robotics, interpretability, architecture); all 5 in the 72-hour window anchored on 2026-08-12 IST; all 5 IDs in `2608.09XXX` range (Aug 10 submission) or `2608.07XXX` (Aug 6 borderline). All 5 images came from the HF CDN successfully (no arxiv HTML fallback needed). `published_at` for all 5 came from a single batched arxiv API call returning Atom XML.

## Notes on cross-referencing the related skills

- `arxiv` skill — the Python search helper (`scripts/search_arxiv.py`) is the fastest path for an unfiltered freshest feed. Use `--sort date --max 20` to get the latest 20 in a category.
- `hf-trend-discovery` skill — for the "trending model just dropped" sub-class of research story (a paper that shipped with weights on HF). Most research-beat papers are weightless, but when one has a paired model card (Alibaba Tongyi Lab frequently does), check the model repo with `curl -s "https://huggingface.co/api/models?author=<org>&full=false"`.