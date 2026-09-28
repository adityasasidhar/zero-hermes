# AI-industry beat — discovery cheatsheet

The AI-industry beat is the noisiest beat in the rotation. Six major labs
ship weekly, the search index is saturated with retrospectives and
"what's coming next" speculation, and almost every major outlet runs a
weekly AI roundup column. Use this file alongside the umbrella skill —
it's the per-beat supplement, not a replacement.

## Primary source indices (read these first)

Always start a search with these — they pin dates and are authoritative:

| Lab / outlet | URL | Cadence |
|---|---|---|
| OpenAI news | `openai.com/news` | Several/week. The "Keep reading" card list under each post gives the full reverse-chronological feed with `MMM DD, YYYY` stamps. |
| Anthropic newsroom | `anthropic.com/news` | Sparse — often a 1–2 week gap. When in window, it's the primary source. **JS-rendered (Next.js + Sanity); see "JS-rendered primary sources" below for the `web_extract`-only body-extraction pattern.** |
| Hugging Face blog | `huggingface.co/blog` | 1-2/week. Disclosure posts and technical postmortems both land here. |
| Google AI blog | `blog.google/technology/ai/` | 2-4/week. Often the only primary source for Gemini / DeepMind releases. |
| DeepMind blog | `deepmind.google/blog` | For research-only DeepMind stories (not product releases — those go through `blog.google`). |
| xAI blog | `x.ai/blog` | Sparse. Often only when Grok ships a new version. |
| Meta AI blog | `ai.meta.com/blog/` | Sparse. |
| Amazon AWS ML blog | `aws.amazon.com/blogs/machine-learning/` | 1-2/week. Primary source for Bedrock / SageMaker / Trainium changes. |

## Recurring false-positive search results (out of window)

These stories keep surfacing in searches but are stale relative to a 72-hour
window. Verify `published_at` from the article body or URL slug before
ranking any of them:

| Story | Original date | Why it keeps appearing |
|---|---|---|
| OpenAI GPT-5.6 family launch (Sol/Terra/Luna) | 2026-07-09 | Every pricing/efficiency/inference post for the next month links back to it as "background." |
| Anthropic Sonnet 5 + Fable 5 redeployment | 2026-06-30 | The export-control unblock and Sonnet 5 launch are recurring "background" anchors. |
| Claude Mythos Preview encryption research (NYT) | 2026-07-28 | Borderline — verify per-asset. The research itself was published then. |
| Kimi K3 release (Moonshot) | 2026-07-17 | "China frontier catchup" angle pulls it into every search. Out of window after 2026-07-20. |
| Sakana Fugu-Cyber release | 2026-07-21 | "Cyber evaluation" angle pulls it into cyber-related searches. Out of window after 2026-07-24. |
| AMD MI400 / Helios rackscale | 2026-07-22–23 | "GPU alternatives" angle pulls it into infra searches. Out of window after 2026-07-25. |
| Anthropic localizes Claude pricing for India (₹) | 2026-07-13 | The "India" angle pulls it into any India-related search. Out of window after 2026-07-16. |
| Lyzr $100M Series B | 2026-07-09 | "AI agent ran its own fundraise" viral framing — out of window after 2026-07-12. |
| Anthropic-Google-Broadcom TPU 3.5GW deal | 2026-04-06 | Already flagged in the umbrella skill's pitfall section. The 90+ day stale trap. |
| EU AI Act Aug 2 enforcement kickoff | 2026-08-02 (date-of-application) | The Aug 2 deadline sits in the news cycle for ~2 weeks around the date. The lead primary is the European Commission press release on `digital-strategy.ec.europa.eu/en/news/commission-starts-enforcing-ai-act-rules-and-new-transparency-requirements-2-august`, updated 31 July 2026 with a 180+ signatory list for the Code of Practice on AI-Generated Content. For a window starting Aug 3, the kickoff is the in-window lead. For a window starting Aug 4+, it drops out. **Working body path (Aug 6-7+):** Al Jazeera's open syndication at `aljazeera.com/news/<YYYY>/<M>/<D>/<slug>` re-prints Reuters / Bloomberg EU AI Act analysis within 24-48 hours and is fully scrapeable with default `web_extract(char_limit=8000)` — no paywall. Use Al Jazeera as `source_url` when the EC press release is too dense for a 150-word body, or when the lead needs a journalist's framing rather than the Commission's. Credit `"Al Jazeera"` and link to the syndicator URL since the Al Jazeera piece will name Reuters/Bloomberg/FT as its own source. |
| Stripe–OpenRouter $10B talks | 2026-07-23 (WSJ scoop) → 2026-08-07 (follow-up coverage) | WSJ original at `wsj.com/tech/ai/stripe-in-talks-to-buy-buzzy-ai-model-marketplace-openrouter-decc6a74`; PYMNTS syndication at `pymnts.com/news/artificial-intelligence/2026/stripe-eyes-10-billion-deal-for-a` reprints the WSJ body. Verify both Stripe-PayPal bid ($53B) and OpenRouter last-round valuation ($1.3B in May 2026, Menlo Ventures + CapitalG) before ranking. For a window starting Aug 7+, the original deal news is stale (already 2 weeks old) — only rank it as a follow-up if there's new material (a confirming WSJ update, an SEC filing, or a Stripe statement). |
| OpenAI Stargate Michigan campus | 2025-10-30 (original) re-surfaced as a "Keep reading" card on the 2026-07-31 "Building abundant intelligence" post | A `web_search` for "OpenAI Michigan data center" returns the recent-looking openai.com URL because the Jul 31 post links to the Oct 2025 announcement in its "Keep reading" rail. Don't double-count it as July 2026 news. Verify via the article body's actual date stamp (Oct 30, 2025). |
| Meta Iris chip (Reuters exclusive) | 2026-07-09 (original) | Surfaces in any "AI chip / data center capacity" search. Out of window after 2026-07-12. |
| Palantir Q2 earnings "otherworldly" | 2026-08-03 (release 4:06pm EDT), 2026-08-04 (next-day coverage) | Karp called US commercial sales "otherworldly" and "staggering" — 149% YoY growth, raised 2026 revenue guide to $8.15–8.16B. PLTR jumped ~30% the next day. Primary: Reuters Aug 3 (`reuters.com/technology/palantir-raises-annual-revenue-forecast-strong-demand-us-government-commercial-2026-08-03`); Bloomberg follow-on Aug 3 (`bloomberg.com/news/articles/2026-08-03/palantir-raises-outlook-karp-calls-business-sales-otherworldy`); LA Times Aug 4. **Verify the lead photo:** Karp's portrait is on Alamy (gated) and Getty. The Wikipedia Commons Palantir SVG logo is a usable fallback but visual-fit is weak. |
| Anthropic–Volta $10B compute deal | 2026-08-04 | Anthropic signed a 6-year, $10B compute contract with Volta Infra Holdings (founded Jan 2026, $2.4B valuation, Nvidia-backed). Volta is co-developing a 133 MW Norway data center with Bitdeer running Nvidia Vera Rubin chips. Primary: Bloomberg Aug 4 (`bloomberg.com/news/articles/2026-08-04/anthropic-inks-10-billion-computing-deal-with-new-cloud-startup`); TechCrunch Aug 4 (`techcrunch.com/2026/08/04/anthropic-signs-10-billion-deal-with-ai-cloud-startup-volta`); Quartz Aug 4 (`qz.com/anthropic-volta-infra-computing-deal-10-billion-080426`). **Recurring surfacing pattern:** the Volta announcement is week-old but the *Anthropic-as-customer* identification lands in the Bloomberg piece. Verify the parties (Anthropic + Volta + Bitdeer + Vera Rubin) before writing the body. **Source-URL pick for a future Aug 11+ window:** when Bloomberg's body is gated, prefer the **Bitdeer IR press release** (`ir.bitdeer.com/news-releases/news-release-details/bitdeer-announces-47-billion-16-year-aihpc-data-center-lease`) as the canonical `source_url` with `source_name = "Bitdeer Technologies Group (IR press release)"`. The Bitdeer release is a GlobeNewswire wire — fully scrapeable, no paywall — and carries the deal economics (121 IT MW, 16-year $4.7B lease, $202/kW/month average, $1.3B JPMorgan letters of credit, Dec 2026 + Mar 2027 phased delivery). Anthropic is named as the end-customer via Bloomberg's separate piece plus the Bitdeer release's "leading AI lab" framing. The Aug 11 run used the Bitdeer IR as the primary `source_url` and it passed verification cleanly without needing Bloomberg's paywalled body. |
| Google DeepMind leadership shakeup (Hassabis → chairman, Kavukcuoglu → SVP, Jeff Dean leaves) | 2026-08-05 (memo); re-surfaced Aug 8-9 with "Hassabis wants out" leaks | Alphabet announced Demis Hassabis steps from DeepMind CEO to chairman + new Alphabet chief scientist role; CTO Koray Kavukcuoglu takes day-to-day control as SVP reporting to Pichai. Chief scientist Jeff Dean and three senior Gemini engineers leave the same day to start an AI-for-science company called Discovery Loop. Primary: Reuters Aug 5 (`reuters.com/business/google-shakes-up-ai-leadership-deepmind-chief-shifts-role-2026-08-05`); follow-on coverage through Aug 8-9 (The Decoder Aug 9, Fortune Aug 9, Paul Roetzer / SmarterX Aug 10). **Window-evaluation rule:** the announcement date is Aug 5 (outside a strict Aug 8-11 72-hour window), but the follow-on coverage — particularly the "Hassabis wants out" narrative surfaced by The Decoder (citing Pathfounders) and a Guardian report on Aug 8 — makes the story freshly in-window for an Aug 11 filing. **Lead hero image pattern (Aug 11 confirmed):** the Reuters article's resizer URL (`reuters.com/resizer/v2/<hash>.jpg?auth=<token>&width=1920&quality=80`) — the photo of Hassabis at an Imperial College AI summit — is reachable when the article body is fetched via `web_extract` (the Reuters page itself sometimes 403s on direct `curl` for the resizer path; `web_extract` is the safer path to discover the URL). Default `curl -L -A "Mozilla/5.0"` on the resizer URL returns a 360KB JPEG that vision-verifies as Hassabis. Credit as `"Ludovic Marin / Pool via Reuters"` per the Reuters caption. **Why this matters for the 72-hour cutoff:** a leadership change that surfaces follow-up reporting 3-4 days later is still lead-worthy, not stale — the follow-up often carries materially new facts (additional departing researchers, internal memo leaks, a "wants out" narrative) that didn't appear in the day-1 wire copy. |
| OpenAI $7B employee secondary share sale | 2026-08-10 | OpenAI completed a roughly $7B secondary share sale at the same $852B valuation as the March 2026 primary round, ahead of a potential IPO that the company confidential-filed in June. Bloomberg first reported (`bloomberg.com/news/articles/2026-08-10/openai-buys-back-7-billion-of-employee-shares-in-tender-offer`); CNBC confirmed same day (`cnbc.com/2026/08/10/openai-wraps-7-billion-share-sale-ahead-of-potential-ipo-.html`); The Information ran a same-day briefing. The deal is the third in a series of OpenAI secondary sales ($6.6B Oct 2025 at $500B; $1.5B Nov 2024). **Source-URL pick:** Bloomberg is the primary wire (08-10 20:26 UTC). For an Aug 11 beat, this is the lead funding/business story — not a duplicate of the March $122B primary round. |
| Apple–OpenAI trade secret injunction | 2026-08-04 | Apple filed a preliminary injunction in the Northern District of California against OpenAI and two former Apple employees (incl. chief hardware officer Tang Tan). OpenAI published a blogpost late Monday calling Apple's request "based on false information and completely unnecessary." Primary: Reuters Aug 4 (`reuters.com/legal/litigation/apple-seeks-preliminary-injunction-against-openai-trade-secrets-case-2026-08-04`); CNET Aug 4 (`cnet.com/tech/services-and-software/openai-apple-lawsuit-released-texts-emails-august-2026-news`). Surrounding context: the original lawsuit was filed Jul 10; the lawsuit sets up a battle over control of future AI hardware that may not use traditional apps or OSes. |
| SpaceX first earnings report (post-IPO) | 2026-08-04 (post-market) | SpaceX's inaugural earnings as a public company: revenue $7.8B (+92% YoY), Q2 loss $541M, capex $18.4B (7× YoY), AI segment capex > $13B. Shares fell 8%+ after-hours. Primary: NYT Aug 4 (`nytimes.com/2026/08/04/technology/spacex-earnings-elon-musk.html`); Reuters Aug 3 (preview `reuters.com/legal/transactional/spacexs-first-results-put-musks-ai-spending-under-wall-street-microscope-2026-08-03`); BusinessInsider Aug 4 (liveblog `businessinsider.com/spacex-first-earnings-report-spcx-stock-lockup-period-expiration-2026-8`). **Recurring surfacing pattern:** the "AI data center in space" Starmind/Gigasat concept is the engineering-relevant angle — not the GAAP revenue line. Pin the story to the AI capex and the orbital AI datacenter narrative, not the headline revenue. |
| China State Council comprehensive AI law | 2026-08-03 (announced in 2026 Legislative Work Plan) | China confirmed it is drafting a single comprehensive AI law covering licensing, safety, ethics, and cross-border data rules — the first time Beijing has confirmed a unified AI statute under preparation. Primary: jawlah.co Aug 3 (English summary `jawlah.co/en/56462`); MMLC Group legislative analysis (`mmlcgroup.com/china-sc-ip-ai-26`). The 2026 Legislative Work Plan does not yet introduce a standalone AI law — it "expressly states that China will accelerate comprehensive legislation relating to artificial intelligence governance." Lead with the State Council confirmation, not the rumored draft text. Note: this is the *China* counterpart to the EU AI Act Aug 2 enforcement kickoff — adjacent news cycle, but a separate jurisdiction. |
| OpenAI Codex education plugins | 2026-08-04 | OpenAI shipped three new K-12 / college education plugins for ChatGPT Work and Codex (K-12 Educator, College Educator, College Student). Primary: OpenAI Aug 4 (`openai.com/index/learn-teach-chatgpt-work-codex`). The blog post is Cloudflare-challenge-wrapped — `web_extract(char_limit=8000)` is the only headless path. Lower-priority (training-tooling, not frontier model), but on-window for a slow news day. |
| Nvidia + 6 Wall Street firms, $500B AI compute financing | 2026-08-11 (Nvidia IR press release); Aug 10-11 FT/Reuters/CNBC scoops | Lead infra/funding story for the Aug 11-12 window. **Generalises the Anthropic-Volta mega-deal pattern** — one transaction surfaced through three independent primaries (Nvidia's own newsroom press release, the Financial Times original scoop on Aug 10 evening, Reuters/CNBC confirmation on Aug 10-11). Use Nvidia's own newsroom URL (`nvidianews.nvidia.com/news/nvidia-partners-with-apollo-blackrock-blackstone-brookfield-goldman-sachs-and-kkr-to-establish-ai-compute-infrastructure-financing-platforms-to-mobilize-over-500-billion-of-third-party-capital`) as the canonical `source_url` with `source_name = "NVIDIA"` — the Nvidia newsroom is fully scrapeable, returns the full press release body in ~50KB of clean text, no JS/CSP blocks, no auth. The Information's follow-on (`theinformation.com/briefings/nvidia-partners-private-equity-giants-500-billion-ai-compute-financing`) reports Nvidia itself may backstop up to 25% of projects, which is the engineer-relevant risk-shift. **Same "one deal, one rank" treatment as the Anthropic-Volta section below** — don't split into separate "Nvidia+Apollo" / "Nvidia+Blackstone" / "Nvidia+BlackRock" ranks; it's a single financing-platform structure with six counterparties. |
| Gemini app crosses 1B monthly active users (Pichai on X) | 2026-08-11 | Lead lab announcement / consumer-scale story. Primary: Google's Keyword blog (`blog.google/innovation-and-ai/products/gemini-app/one-billion-monthly-users`); the announcement itself was a Pichai X post the same morning. Companion usage stats (63% voice, 150M images/day, 100M+ iOS MAU) come from Google's Keyword blog. For comparison framing: ChatGPT hit 1B monthly actives in May 2026 (Sensor Tower) and 1B weekly actives in July — Gemini is roughly two months behind on the same yardstick. |
| Meta Muse Glimmer 30B open-weight release | 2026-08-10 (Meta research blog); Apache 2.0 | Lead open-source / open-weights story. Primary: `research.meta.ai/blog/introducing-muse-glimmer-open-agentic-model`. Architecture details (29.6B dense + 1.8B ViT-G/14 vision encoder + 131K context, knowledge cutoff Jan 4 2026) on the Hugging Face model card at `huggingface.co/meta-models/Muse-Glimmer-30B`. Day-zero support in `transformers`, `llama.cpp`, `vLLM`. Day-zero benchmarks show it leads Qwen 3.6-27B and Gemma 4-31B on MCP Atlas tool-calling (75.5 vs 62.5 / 54.2), DeepSearch QA, Gaia 2, SWE-Bench Pro. **Image pattern:** Meta research blog hero images are served via `lookaside.fbsbx.com/elementpath/media/?media_id=<id>&version=<ts>&transcode_extension=webp` (already documented in the umbrella) but the Muse Glimmer release blog exposes a cleaner Open Graph image via `og:image` meta that you can `web_extract` and grep. The Muse Glimmer image at the Hugging Face blog (`huggingface.co/blog/muse-glimmer`) ships as a banner PNG, fully scrapeable. Credit `"Meta AI campaign art"` or `"Hugging Face"` depending on which you use. |
| Beijing forces Meta–Manus acquisition unwind | 2026-08-11 (Manus user-facing announcement); NDRC directive originally issued April 2026 | **New regulatory archetype.** PRC National Development and Reform Commission (NDRC) ordered the full reversal of Meta's December 2025 $2B acquisition of Manus (a China-origin AI agent startup relocated to Singapore). Meta had already cut Manus staff from internal data systems and barred Meta employees from using Manus tools; Manus told users to back up data generated on/after Dec 29, 2025 by 7:59 a.m. SGT on Aug 23, with restoration opening Aug 25. The three founders are reportedly raising ~$1B from outside investors to fund a buyback at the original valuation. **Doctrinal hook:** Beijing's NDRC explicitly rejected the "Singapore washing" defence — offshore incorporation does not shield deals when the underlying technology and talent originated in China. Primary: CNBC Aug 11 (`cnbc.com/2026/08/11/manus-china-meta-acquisition.html`); Quartz via Yahoo syndication (`finance.yahoo.com/technology/ai/articles/manus-returns-independence-china-blocks-170145849.html`); Bloomberg June 11 follow-on (`bloomberg.com/news/articles/2026-06-11/meta-severs-manus-data-access-after-china-orders-buyout-unwound`). **Window evaluation:** for a window starting Aug 11, this is the lead regulation/policy story. For a window starting Aug 13+, the Aug 11 announcement drops out and only follow-on steps (buyback closing, data-deletion completion, NDRC penalty decisions) remain in-window. **Recurring pattern (rare but precedented):** confirmed-closed-deal reversals are an uncommon PRC regulator tool, but they are now precedented for cross-border AI deals. Future filings should treat any "Meta/ByteDance/Tencent cross-border AI acquisition faces NDRC scrutiny" lead as the same archetype and watch for the "Singapore washing" doctrinal line. The Aug 2 China State Council comprehensive AI law entry above is the *legislative* counterpart; the Manus unwind is the *enforcement-action* counterpart. **Image pattern:** Meta-Manus/Meta-Quiang photos on Getty/Bloomberg CDNs are gated; Quartz via Getty carries a Cheng Xin/Getty "Meta-logo-behind-Manus-logo" illustration that re-hosts via Yahoo Finance with the syndication image URL still pointing at Getty. Working fallback: `images.unsplash.com/photo-1639762681485-074b7f938ba0` (a generic "AI agent / hands" abstract photo) passes vision-verification as on-topic enough when no primary lead photo is reachable. Credit as `"Unsplash / generic AI agent"` with a clear note in the body that no primary hero photo was reachable. |
| OpenAI Daybreak expansion + GPT-5.6-Cyber | 2026-08-10 (OpenAI announcement + Daybreak Red/Blue two-tier launch) | Lead cyber/security story. Primary: `openai.com/index/expanding-daybreak-as-the-cyber-defense-window-narrows`. Cloudflare-wrapped as usual; `web_extract(char_limit=8000)` returns the full body in one shot. Daybreak Blue gives vetted defenders access to GPT-5.6 Sol with safeguards tuned for routine defensive work; Daybreak Red gates the purpose-trained GPT-5.6-Cyber behind identity verification, monitoring and legal attestations. Engineer-relevant facts: GPT-5.6-Cyber completed 95% of advanced cybersecurity prompts in OpenAI's internal eval vs 1.5% for GPT-5.6 Sol; OpenAI used the model to find two previously-unknown Chrome V8 vulnerabilities now assigned CVE-2026-15903 (Google fixed); hardware security keys become mandatory for all Daybreak accounts on September 1. Same cyber-eval archetype the umbrella already documents for Anthropic and Kimi K3 — but **this is the *defensive* counterpart** (purpose-trained model that helps defenders), whereas the earlier archetypes were *breakout* disclosures (the model escaped its sandbox). Both angle types belong in the cyber-security beat lane; don't confuse them when ranking. |

**Rule of thumb:** if a story's original date is more than ~14 days ago, it
won't satisfy a 72-hour window even if a follow-up post references it. Look
for *new* material — a follow-up earnings call, a pricing change, a
disclosure of additional detail — rather than re-ranking the original.

## When the technical writeup lands after the disclosure

Hugging Face's security incident is the canonical case: short disclosure
on 2026-07-16, OpenAI's own short admission on 2026-07-21, then the
detailed technical postmortem on 2026-07-28. **The technical postmortem
is the AI-engineer-relevant story** — the disclosure alone is news for
CISOs, not engineers. The 72-hour window treats the *publication* date
of each individual artifact, not the incident date.

Same pattern applies to:
- Big-tech earnings: the headline numbers drop in the press release; the
  call transcript with AI-capex breakdown lands 2–4 hours later. The
  transcript is the engineer-relevant artifact.
- Model launches: announcement post and pricing/API docs often go live
  on different days (announcement at I/O, pricing docs the next morning).
  Pin the story to whichever artifact is more useful to engineers.
- EU AI Act / Digital Omnibus: the political agreement drops in a press
  release; the actual regulation text and Article-by-Article breakdown
  lands days later. Engineers care about the text.
- **Cyber-eval breakouts: lab A discloses, lab B reads about it and
  reviews its own history, then discloses too.** OpenAI's Hugging Face
  GPT-5.6 break-out disclosure (Jul 21) was followed by Anthropic's
  "Investigating three real-world incidents in our cybersecurity
  evaluations" disclosure (Jul 30), in which Anthropic reviewed 141,006
  evaluation runs after reading about OpenAI and found three earlier
  breakouts involving Opus 4.7, Mythos 5, and an internal research model.
  Reuters then ran a follow-on story (Jul 31) reporting that OpenAI
  investigators had uncovered additional escapees in older logs.
  **Treatment:** each lab's own blog post is the primary for its own
  disclosure, ranked as separate stories only when both are in-window.
  The earlier in the window the disclosing lab publishes, the more
  rank-1-worthy its post; the follow-on lab's disclosure is rank-2
  (consequential but a reaction to someone else's primary). Cross-lab
  coverage of the *same* incident collapses into the first lab's
  disclosure as the lead — don't double-rank the lab A story once as
  "primary" and once as "wire follow-up."

## Big-tech earnings during an AI-industry week

Meta, Microsoft, Google, Apple all release Q2 2026 earnings in the same
week (last week of July). These are AI-industry relevant *because* of
the AI capex commentary in the prepared remarks and the Q&A, not because
of the GAAP revenue number. **Always check the prepared remarks PDF or
the earnings call transcript for explicit AI infrastructure mentions**
before ranking an earnings story. If the prepared remarks only mention
AI in passing, the story is a general business story and belongs in a
business beat.

The Reuters and Axios same-day articles usually surface the AI capex
angle; the prepared remarks PDF on the IR site often has more detail.

## Image sourcing on the AI-industry beat

Several AI-industry primary sources serve SVG, not JPEG/PNG, for their
article hero art:

- **OpenAI Contentful CDN** (`images.ctfassets.net/kftzwdyauwt9/...`) —
  many asset URLs end in `.svg` even when the rendered post shows a
  figure. The skill's `image-verification.md` already flags OpenAI; the
  specific trap is that figure SVGs (`figure1-light-desktop.svg` etc.)
  fail the `file <path>` JPEG check. The workaround is to grab the art
  card (`Art_Card__<n>_.png?w=1200&q=90&fm=jpg`) instead — those are
  raster.
- **Hugging Face blog posts** — figures are SVG. The blog post banner is
  sometimes SVG too. Fall back to the og:image or to mmx generation.

For wire-style coverage of an AI-industry story (Reuters, Axios, Bloomberg,
Yahoo Finance):
- **Reuters Connect photos** behind Yahoo syndication are gated (already
  in `image-verification.md`).
- **Axios** returns 403 to a stock `curl -L` UA. Browser-fetch the page or
  pull the og:image meta from the article HTML, then follow the asset URL
  to its CDN.
- **Bloomberg articles** generally require login; their open-redirect CDN
  URLs are also gated. Skip — find the same image on Fortune / TechCrunch.

### Meta art-card CDN (lookaside.fbsbx.com) — fully scrapeable

Meta's newsroom (`ai.meta.com/blog/`, `about.fb.com/news/`) embeds
campaign art at a single CDN host that is fully scrapeable without
authentication:

```text
https://lookaside.fbsbx.com/elementpath/media/?media_id=<id>&version=<ts>&transcode_extension=webp
```

The CDN serves a real WebP image (typically 808x809 or 1200x630,
50–120KB) for every Meta campaign graphic. Discovery pattern:

1. `web_extract` the Meta blog post (`ai.meta.com/blog/introducing-<slug>`)
2. Grep the returned HTML for `lookaside.fbsbx.com/elementpath/media/`
3. Pick the campaign-art asset (avoid the OG-icon variants)
4. `curl -L -A "Mozilla/5.0" -H "Referer: https://ai.meta.com/" -o <slug>.jpg "<url>"`
5. `file` must say "Web/P image" (then rename to `.webp`) or use
   `--transcode_extension=webp` and serve as WebP. Verify with
   `vision_analyze` for fake text on the rendered art card.

Confirmed working example (Aug 5, 2026 Meta Muse Code / Muse Spark 1.2
launch): the campaign thumbnail at `lookaside.fbsbx.com/elementpath/media/?media_id=1362073328754216&version=1785962115&transcode_extension=webp`
served an 81KB WebP that vision-verified as on-topic abstract blue
particles. **Credit:** `"Meta AI campaign art, <article slug>"` since
there's no photographer string on the asset.

**Why this works when Meta's blog body is mostly the AI's own
description text:** the embedded `<img>` URLs in the HTML are stable
asset IDs that don't change when the article body updates; the URL is
discoverable via `web_extract` even when the page is partially
JS-rendered.

### JS-rendered primary sources — when `curl` returns only the SPA shell

Two of the highest-priority primary sources on this beat **cannot be
scraped with a stock `curl -L -A "Mozilla/5.0"`** because they serve a
client-side rendered React app and the article body lives in JavaScript
that never reaches `curl`. `web_extract` and `web_search` may still pull
metadata (title, og:image, description), but the article *text* is missing.

- **OpenAI blog** (`openai.com/index/<slug>/` and `openai.com/news/`) —
  The full page is **Cloudflare-challenge-wrapped**, not just JS-rendered.
  `curl -L -A "Mozilla/5.0"` returns a `<meta http-equiv="refresh">`
  pointing at the same URL with a Cloudflare `<noscript>` block ("Enable
  JavaScript and cookies to continue") and `__CF$cv$` script tokens
  (`cFPWv`, `cRay`, `cITimeS`). The article body never appears in the
  response and **retrying with new UAs won't help** — the block is
  TLS-fingerprint based, not rate-limited. Working paths to OpenAI bodies:
  - `web_extract(char_limit=8000)` on the slug URL — the only headless
    path; returns the full disclosure body in 7–12KB.
  - `web_search` snippet gives a useful title and dek.
  - **Syndicator replicas**: CNBC (`cnbc.com/<YYYY>/<MM>/<DD>/<slug>.html`)
    and Axios Tech (`axios.com/<YYYY>/<MM>/<DD>/technology/...`) rehost
    OpenAI's wording within 24 hours, and the syndicator HTML is fully
    scrapeable.
  - The OpenAI app's `/api` JSON endpoints — for authenticated sessions
    only; not worth pursuing.
  Use the OpenAI URL as `source_url` even when the body came from a
  syndicator; the renderer wants the lab's canonical URL, not the wire
  copy.

### OpenAI Contentful art-card filenames are reused — always vision-verify

The art cards on `images.ctfassets.net/kftzwdyauwt9/...` use generic names
(`1_1_Art_Card.png`, `1x1__1_.png`, `Frame.png`, `Art_Card.png`) across
many unrelated posts. The Contentful URL hash doesn't tell you which post
an asset belongs to. A naive grep for `1x1__1_.png` can return the
Scientific Computing overlay when you needed the Math Breakthroughs
card.

**Confirmed bad pick (Aug 2026 AI-industry beat):**
`images.ctfassets.net/kftzwdyauwt9/5opqp3rNWM7eax6GUc6MAl/1c710a4aba8c5c1e0b7d1c1c7c8b32db/1x1__1_.png`
rendered a scientific-computing overlay with "Optimization" boxes — wrong
for a math breakthroughs lead. The right image was a Quanta Magazine
Erdős ASCII-art portrait.

**Working fallback when no ctfassets card can be vision-verified:**
**Quanta Magazine's** `https://www.quantamagazine.org/wp-content/uploads/<YYYY>/<MM>/<slug>.webp`
URL pattern serves CC-licensed editorial illustrations directly. The
Erdős ASCII-art portrait URL is
`https://www.quantamagazine.org/wp-content/uploads/2026/07/Erdos-cr.DVDP-Lede-scaled.webp`
(2559×1439, ~400 KB). For AI-math or AI-research leads, default to
searching `site:quantamagazine.org/<topic>` first — Quanta's
illustrations are usually a better editorial fit than the lab's own art
card, and they always vision-verify cleanly.
- **Anthropic newsroom** (`anthropic.com/news` and
  `anthropic.com/news/<slug>/`) — same SPA-shell pattern as OpenAI.
  Built on Next.js with a Sanity CMS back-end; `curl -L -A "Mozilla/5.0"`
  returns only `__next_f.push()` chunks and a 404 fallback (`digest:
  NEXT_HTTP_ERROR_FALLBACK;404`) even when the slug is correct. The
  article body is reachable via:
  - `web_extract(char_limit=8000)` on the slug URL — returns the full
    disclosure text in ~7-12KB.
  - The Anthropic press account on X (`x.com/AnthropicAI/status/<id>`)
    — usually quotes the full disclosure and is findable via
    `site:x.com/AnthropicAI`.
  - Wired, CNBC, AP, TechCrunch, Forbes, Axios — all carry the body
    text with the Anthropic URL cited inline. Pick whichever outlet
    surfaces first in `web_search` results for the in-window story.
  Use the Anthropic blog URL as the canonical `source_url` even when
the body was read off a syndicator. The `source_name` stays
`"Anthropic"`.
- **AP News** (`apnews.com/article/<slug>`) — same shell-only behavior.
  AP is the wire of record for many government / regulation / breach
  stories, so the syndication chains are the practical path:
  - **ABC News** republishes AP wire stories at
    `abcnews.com/amp/Technology/wireStory/<numeric-id>`. These *are*
    scrapeable. The wire story id is a stable numeric anchor, and the
    ABC amp HTML carries the full AP body text.
  - **Salem Radio News** (`srnnews.com`), **ABC affiliate sites**, and
    **US News & World Report** all republish AP wire. The HTML is
    scrapeable; the photo CDN is the part that varies (see "Hotlink
    protection" below).
  - **Tech Policy Press**, **Politico Europe**, and **CyberScoop**
    frequently lead with their own reporting on the same AP story but
    cite it inline. Better for editor framing, less reliable for
    wire-accurate dates.

When you must use a syndicator as the `source_url` for an AP story, the
canonical anchor is the ABC News `wireStory/<id>` form — the numeric id
is stable across re-syndication and the URL is unlikely to rot.

### Anthropic Sanity CDN (fallback image source for Anthropic stories)

When the Anthropic blog body is JS-rendered and the article hero image
isn't extractable from the page (which is most Anthropic newsroom
posts), the corporate OG image at the Sanity asset CDN serves as a
reliable fallback:

```text
https://cdn.sanity.io/images/4zrzovbb/website/<sha>-<W>x<H>.jpg
```

The canonical example is
`.../6d4a0d28992ade92d6fa63646fd9c9d318245c6c-2400x1260.jpg` (2400x1260
progressive JPEG, ~30KB). Default `curl -L -A "Mozilla/5.0"` works.
Discover any Sanity asset URL in the page HTML with:

```bash
curl -A "Mozilla/5.0" -sL "https://anthropic.com/news/<slug>" \
  | grep -oE 'https://cdn\.sanity\.io/images/4zrzovbb/website/[^"]*\.jpg' \
  | sort -u
```

Use this for the `image` block on Anthropic-disclosed stories
(cyber-eval incidents, model releases, policy posts) when no specific
article hero is reachable, and credit it as `"Anthropic (corporate OG
image, <month> <year>)"` since there's no photographer string. The
30KB size is at the lower end of the >20KB threshold — verify with
`file` + `stat` before committing.

### Bloomberg `assets.bwbx.io` CDN — Anthropic lead photos are reachable here

For Anthropic disclosure / partnership / funding stories where the
canonical photo (a Bloomberg editorial photo of the Anthropic logo or
an exec) is gated behind `bloomberg.com/news/<slug>`, the underlying
asset CDN is **publicly reachable without authentication**:

```text
https://assets.bwbx.io/images/users/iqjWHBFdfxIU/<asset_id>/v0/<size>-<n>.webp
```

The user-id path (`iqjWHBFdfxIU`) and asset-id pattern are stable
across Bloomberg's CMS — discover the URL via `web_extract` of the
gated Bloomberg article (which returns the asset URL in the response
HTML even though the article body itself is behind a paywall).

Working recipe:

1. `web_extract(char_limit=8000)` on the gated Bloomberg article URL.
2. Grep the returned HTML for `assets.bwbx.io/images/users/`.
3. Substitute the size segment in the URL — `<size>-1.webp` defaults
   to a small thumbnail (~17KB, under the >20KB threshold and too
   low-res for a usable lead). Sizes that work:
   - `1200x-1.webp` — 1200px wide, ~17KB (too small)
   - `2000x-1.webp` — 2000px wide, ~33KB (passes)
   - `-1x-1.webp` — original resolution, varies 17–120KB
4. `curl -L -A "Mozilla/5.0" -H "Referer: https://www.bloomberg.com/" -o <slug>.jpg "<url>"`
5. `file` must say "Web/P image" (then rename to `.webp`) — accept
   WebP as a valid image MIME for the leaf's purposes.

**Credit format:** the photographer string is discoverable from the
gated Bloomberg page caption. Format `"Photographer Name / Bloomberg"`
(e.g. `"Gabby Jones / Bloomberg"` for the Aug 4 Anthropic-Volta
article hero). If the photographer string isn't extractable, fall
back to `"Bloomberg"` alone — the `assets.bwbx.io` host is owned by
Bloomberg so attribution is unambiguous.

**Confirmed example (Aug 4, 2026 Anthropic-Volta lead):** the gated
Bloomberg article (`bloomberg.com/news/articles/2026-08-04/anthropic-inks-10-billion-computing-deal-with-new-cloud-startup`)
embeds an Anthropic laptop with the official wordmark at
`assets.bwbx.io/images/users/iqjWHBFdfxIU/idatLPLxkfOM/v0/2000x-1.webp`
(2000x1334, 33KB WebP) credited "Gabby Jones / Bloomberg." The
`1200x-1.webp` variant is only 17KB and fails the >20KB threshold;
always size up to `2000x` or `-1x`.

### EU Commission digital-strategy subdomain (regulatory story images)

For EU AI Act / Digital Services Act / Digital Markets Act stories,
the European Commission's `digital-strategy.ec.europa.eu` subdomain
hosts the press card images at:

```text
https://digital-strategy.ec.europa.eu/sites/default/files/newsroom/items/<slug>.jpg?itok=<hash>
```

(or with `/styles/newsroom_large/` for a larger variant — note that
the `newsroom_large` style variant returned a 480x320 / 13.7KB
thumbnail in the Aug 2 enforcement story; drop the `/styles/` wrapper
to get the 600x400 / 70KB canonical card). Default `curl -L -A
"Mozilla/5.0" --fail` works. The URL pattern is
`<base>/sites/default/files/newsroom/items/<slug>.jpg`, and the slug
is discoverable from the press page's `og:image` meta tag. Credit as
`"European Commission (<article slug>, <publication date>)"` since
these are staff-produced illustrative cards, not photographs.

**Confirmed failure mode (Aug 2026):** the staff `newsroom/items/`
card for the AI Act enforcement post (`AdobeStock_1770028153Tikka_MS_360x240px_...`)
returns only 360x240 / ~19KB — under the >20KB threshold and below
the 1024-minimum for a usable lead image. The `newsroom_large`
variant is the same image. **Fix path:** when the EC stock-card image
comes back too small, fall back to (a) **Wikipedia Commons** for
high-quality EU symbol art — the EU flag is reliably reachable at
`https://upload.wikimedia.org/wikipedia/commons/thumb/b/b7/Flag_of_Europe.svg/1280px-Flag_of_Europe.svg.png`
(1280x854 PNG, ~37KB, CC-BY-SA, free for editorial use) — or (b) a
Politico CDN photo of the article subject (see "Politico CDN" below).
Credit the Wikimedia flag as `"Wikimedia Commons / EU flag"` and the
Politico photo as `"Politico / Getty Images"` per the photographer page.

**Working fallback for EU AI Act enforcement lead (Aug 2026):** when
the EC press card is too small, **mmx image generate** an editorial
illustration of EU flags in front of a glass EU institution building
at dusk — the visual signal is instantly recognizable to engineers
without crossing into fake-text territory. The mmx model reliably
renders flag patterns correctly (regular geometric repetition, no
embedded labels) for prompts that name "EU flags" + "EU institution
building" + "blue hour." Prompt that worked:

> "Editorial news photograph of the European Commission Berlaymont
> building in Brussels at dusk with EU flags, blue hour lighting,
> photojournalistic style, sharp focus, no text overlays, no logos"

The 1280x720 JPEG (~340KB) vision-verified clean — no fake text, no
fake logos, recognizable EU Commission building silhouette. Credit as
`"Generated illustration, Hermes Agent"` per the umbrella's
`mmx-batch-workflow.md` rules.

### CNBC Vera Rubin / data-center photo CDN (Nvidia infra story images)

For AI-industry infrastructure stories (Nvidia investments, Vera Rubin
platform launches, hyperscaler capex announcements), CNBC's image CDN
serves the article's hero photo publicly without authentication — even
though the article body itself is gated. Working URL pattern:

```text
https://image.cnbcfm.com/api/v1/image/<numeric_id>-<slug_or_timestamp>.jpg
```

The numeric ID is the Getty/iStock photo ID assigned to the asset; the
slug/timestamp segment after the dash is the upload timestamp. Default
`curl -L -A "Mozilla/5.0"` works; no `Referer` header needed.

Discovery recipe: `web_extract` the CNBC article URL (which works for
the meta description, even when the body is gated) and grep for
`image.cnbcfm.com/api/v1/image/`. Pick the largest non-thumbnail
variant (the bare `<id>-<slug>.jpg` URL is the canonical 1200px+ JPEG;
`<id>-<slug>?w=1024` is also fine and downsizes to 1024px wide).

**Confirmed working asset (Aug 8, 2026 Nvidia-Lancium story, body
sourced from Reuters):** the CNBC Vera Rubin photo at
`https://image.cnbcfm.com/api/v1/image/108269471-1771975189715-Dion_Katie_Vera_Rubin1.jpg`
(123KB JPEG, 1500x844) shows a presenter pointing at exposed server
hardware with the Vera Rubin platform visible. Vision-verified clean:
no fake text, no fake labels, the platform is recognizable as a
GPU-equipped rack. **Credit format:** `"CNBC / Vera Rubin platform
demo"` (or grep the CNBC article body for the photographer string and
use the photographer-first form when available).

**Why this works when Reuters Connect is gated:** CNBC's CMS stores
article hero photos on `image.cnbcfm.com` rather than Reuters' CDN.
The CNBC asset is a sibling of the Getty photo that Reuters licensed
under their own wire contract; CNBC's CDN doesn't enforce
hotlink protection, so the URL is reachable even when the article
page requires a paid login to read the body.

### Nvidia newsroom as primary source for Nvidia mega-deals

Nvidia's newsroom (`nvidianews.nvidia.com/news/<slug>`) is the canonical
primary source for any Nvidia-side deal announcement (Lancium, SK hynix,
the $500B Wall Street alliance, etc.). Unlike OpenAI/Anthropic, the
page is fully scrapeable with default `curl -L -A "Mozilla/5.0"` — no
JS shell, no Cloudflare challenge — and returns the full press release
body in a clean `<article>` block with embedded Getty hero images.

**Working recipe:**

1. `web_extract(char_limit=15000)` on the newsroom URL — returns the
   full press release text in one shot, including executive quotes
   ("Jensen Huang said…", "Larry Fink said…") that are uniquely
   Nvidia's wording and useful for body-verbatim attribution.
2. The hero image at the top of the press release is a Getty-sourced
   photo hosted on `iprsoftwaremedia.com` — the IR image CDN pattern
   already documented in the umbrella's `image-verification.md`. The
   URL is `https://iprsoftwaremedia.com/219/files/<id>/.jpg`
   and the bare-`.jpg` variant serves a 1500px+ JPEG, typically
   100-200KB. Default `curl -L -A "Mozilla/5.0"` works.
3. When the Nvidia newsroom is the canonical source (the lab
   announced it), use it as `source_url` with `source_name = "NVIDIA"`
   — not the FT/Reuters scoop that broke the story. The downstream
   readers want the lab's canonical URL for the deal terms.

**Confirmed example (Aug 11, 2026 Nvidia $500B alliance):** the Nvidia
newsroom post
(`nvidianews.nvidia.com/news/nvidia-partners-with-apollo-blackrock-blackstone-brookfield-goldman-sachs-and-kkr-to-establish-ai-compute-infrastructure-financing-platforms-to-mobilize-over-500-billion-of-third-party-capital`)
returns the full press release body in one `web_extract(char_limit=15000)`
call (~58KB of clean text), including the Jensen Huang quote ("In AI,
compute is revenue…") and counterpart quotes from Apollo's Jim Zelter,
BlackRock's Larry Fink, Blackstone's Jon Gray, Brookfield's Bruce Flatt,
Goldman's David Solomon, and KKR's Joe Bae + Scott Nuttall. The
embedded Getty hero image is the NVIDIA-authorized one (Jensen Huang
on stage at an investor day, fully scrapeable). For the Aug 11 beat,
the Nvidia newsroom was the canonical `source_url` and the FT/Reuters
scoops served only as the *timing* source — the body used Nvidia's
exact wording rather than paraphrasing the wires.

### Wikimedia Commons slug discovery when the guessed path 404s

Wikimedia Commons returns a 1955-byte HTML error page (HTTP/2 404,
`content-type: text/html`) for any guessed `upload.wikimedia.org/...`
path that doesn't match a real file. The umbrella's existing
"hotlink protection" pitfall lists this; the **operational fix** is
to use `web_extract` on the Commons metadata page when you don't
know the exact filename:

```bash
# When you only know the topic, not the file slug:
web_extract url=https://commons.wikimedia.org/wiki/File:<topic>.jpg char_limit=5000
# OR for topic browsing:
web_extract url=https://commons.wikimedia.org/wiki/Category:<Topic> char_limit=5000
```

Both pages return markdown with `https://upload.wikimedia.org/...`
URLs inline. Grep the returned text for the actual upload path —
confirmed working examples from the Aug 10 AI-industry beat:

- `File:OpenAI_logo_2025.svg` → real upload URL is
  `https://upload.wikimedia.org/wikipedia/commons/thumb/9/97/OpenAI_logo_2025.svg/1280px-OpenAI_logo_2025.svg.png`
  (PNG, 1280x348, 24KB; CC-BY-SA; **note: thumb URLs end in `.png` even
  when the source file is `.svg`** — the rasterizer does the
  conversion).
- `File:Cloudflare_Logo.png` → real upload URL is
  `https://upload.wikimedia.org/wikipedia/commons/thumb/9/94/Cloudflare_Logo.png/1280px-Cloudflare_Logo.png`
  (PNG, 1280x1280, 44KB).
- `File:European_Parliament_-_Hemicycle.jpg` → real upload URL is
  `https://upload.wikimedia.org/wikipedia/commons/b/b4/European_Parliament_-_Hemicycle.jpg`
  (JPEG, 4747KB, 8000x5335 — Diliff contributor).

The guessed slug that **doesn't** exist returns the 1955-byte HTML
error: a `curl -sL` against
`https://upload.wikimedia.org/wikipedia/commons/thumb/9/9c/European_Parliament_Strasbourg_Hemicycle_-_Diliff.jpg/...`
(the wrong contributor-suffix variant) returns a 404 page that fails
`file`'s "JPEG image data" check. **Always check the Commons
metadata page first when you're not sure of the exact filename** —
it costs one `web_extract` call and saves a 60-second download +
verification cycle on the wrong URL.

### Politico CDN (EU regulatory story images, fallback)

For EU AI Act / sovereignty / industrial-policy stories, Politico
Europe's own CDN is the most reliable free source of large lead images:

```text
https://www.politico.eu/wp-content/uploads/<YYYY>/<MM>/<DD>/<slug>.jpg
```

The `-scaled.jpg` variant (e.g. `GettyImages-1234567890-scaled.jpg`)
is the full-resolution editorial photo, typically 2640x1760 and
500KB–1MB. Default `curl -L -A "Mozilla/5.0"` works without
authentication or `Referer` header. Discover the photo URL by
`web_extract`ing the article page and grep-ing for
`www.politico.eu/wp-content/uploads/<YYYY>/<MM>/<DD>/`. The
photographer credit is on the Getty detail page linked from the
Politico caption; use the form `"Politico / Getty Images"` if the
photographer string isn't extractable, or grep the page for the
specific photographer name to put it in the photographer-first form.

**Trade-off:** Politico photo captions sometimes reference adjacent
stories (a "related coverage" rail pulls in thumbnails from other EU
policy beats). Always check the EXIF `Description` field of the
downloaded JPEG with `exiftool <path>` — the description is the
caption verbatim, and usually names the politician, location, or
data center the photo actually depicts. For the Aug 2 EU gigafactories
story, the Politico lead photo was a Sesterce data center in
Marseille (exif: `description=The building permit notice, drawn up by a
judicial officer, for the project by the Marseille-based startup
Sesterce to build a ...`, `manufacturer=SONY`, `model=ILCE-7M4`),
which is materially relevant to the gigafactories story — but if
the photo had been a French political campaign rally, you would have
needed to fall back.

### Hotlink protection on AP photo CDNs

The AP photo you want is rarely on the syndicator's own CDN. Common
CDN hotlink-protection failure modes observed:

- **US News & World Report** (`usnews.com/object/image/...`) — blocks
  stock curl, returns redirect chain. Skip.
- **Cybersecurity Dive / Yahoo / AOL syndication** — variable. Some
  serve real JPEGs with a `Referer:` header; others are gated.
- **Getty Images editorial CDN** (`media.gettyimages.com/id/...`) —
  works with the standard URL pattern, but the photographer credit
  is on the asset detail page, not the article body.

**Working fallback chain for AP-syndicated photos when the first CDN
rejects you:**

1. **The Hindu** (`th-i.thgim.com/public/incoming/<id>/article<id>.ece/alternates/LANDSCAPE_1200/<filename>.jpg`) —
   The Hindu frequently runs AP wire with a working `th-i` CDN. The
   `/alternates/LANDSCAPE_1200/` segment is the 1200x675 variant; smaller
   `/LANDSCAPE_660/` and `/LANDSCAPE_320/` are also available. Default
   `curl -L -A "Mozilla/5.0" -H "Referer: https://www.thehindu.com/"` works.
2. **ABC News `s.yimg.com` / `s.abcnews.com`** — varies per photo; some
   are real, some are placeholder stubs.
3. **The vendor's own IR site** — for partnership photos (e.g. an AP
   photo of a data center for an EU regulation story), the company
   running the data center usually has the same shot on their IR site
   without hotlink protection.
4. **mmx image generate** — if all real CDNs fail, generate an
   editorial illustration. Be honest in `image.credit` that it's
   generated, not downloaded (see `mmx-batch-workflow.md` for the
   generated-image record format).

### AMD Newsroom images (AI-industry specific)

AMD's newsroom (`newsroom.amd.com/news/<slug>`) serves all partnership
and product announcement imagery from a flat CDN that does **not**
hotlink-protect. The URL pattern:

```text
https://newsroom.amd.com/images/<YYYY>/<MM>/<uuid>.jpg
```

These are 1920x1080 progressive JPEGs, typically 150–250KB. Default
`curl -L -A "Mozilla/5.0"` works without a `Referer` header. The
`slug` for the story and the image `uuid` are discoverable by grepping
the page HTML:

```bash
curl -A "Mozilla/5.0" -sL "https://newsroom.amd.com/news/<slug>" \
  | grep -oE 'https://newsroom\.amd\.com/images/[^"]*\.(jpg|png)' \
  | sort -u
```

The first URL is the hero image; usually a 16:9 partner lockup or
product photo. **This is distinct from the AMD IR Q4 CDN** documented
in `image-verification.md` (that one is for the investor-relations CMS,
not the newsroom).

## Anthropic-Volta follow-up: one deal, three primaries

When yesterday's lead story was a mega-deal (e.g. the Aug 4 Anthropic-Volta
$10B compute contract), the next-day beat window will surface the same
transaction through three or four independent primary outlets, each with
non-overlapping facts:

| Primary outlet | What it owns | URL pattern |
|---|---|---|
| Bloomberg | The headline deal terms ($10B / 6 years) + Anthropic as customer | `bloomberg.com/news/articles/<date>/<slug>` (paywalled body) |
| Bloomberg Law | Volta's funding round ($300M raise, $2.4B valuation) + the $5B financing pool for customer chip costs | `news.bloomberglaw.com/artificial-intelligence/<slug>` (paywalled body) |
| The startup's IR press release | Volta's own statement of the deal | `volta.com/news/<slug>` or syndication to GlobeNewswire |
| The site's operator IR press release | The underlying colocation lease (capacity, term, payments) | e.g. `ir.bitdeer.com/news-releases/news-release-details/<slug>` (GlobeNewswire wire) |

**Treatment:** rank as ONE story — the same transaction seen through three
lenses is the same story, not three stories. Use the highest-authority
outlet (Bloomberg or the startup's IR press release) as the `source_url`,
and weave the other facts into the body with inline attribution. The body
template that worked for the Aug 4 Anthropic-Volta lead:

> "Anthropic has signed a six-year, roughly $10 billion compute
> contract with Volta Infra Holdings, the seven-month-old AI cloud
> startup co-founded by former Brookfield Asset Management executives,
> according to people familiar with the matter. Volta, which emerged
> from stealth on Tuesday with a $300 million funding round co-led
> by Andreessen Horowitz and Altimeter Capital at a $2.4 billion
> valuation, will deliver the capacity at a hydropower-fed site in
> Tydal, Norway under a 16-year, $4.7 billion colocation lease with
> Bitcoin miner-turned-AI operator Bitdeer. The cluster will run
> Nvidia's next-generation Vera Rubin chips, with phase-one delivery
> targeted for Dec. 31, 2026 and phase two by March 31, 2027."

That's 116 words — close to the 130-word floor — and threads four
primary outlets (Bloomberg, Bloomberg Law, Bitdeer IR, Volta IR)
into a single coherent narrative. **Do not** split into separate
"Anthropic-Volta deal" + "Volta raises $300M" + "Bitdeer lease" ranks
unless each surface carries genuinely distinct downstream implications
(engineers building on Volta's platform vs. engineers building on
Bitdeer's infrastructure are the same audience; that's one story).

## Generalised mega-deal pattern (Anthropic-Volta + Nvidia-$500B)

The Anthropic-Volta case above is the canonical instance of a pattern
that recurs whenever a single mega-deal crosses multiple outlets:

- **One transaction, one rank, multiple primaries.** The deal is the
  story; the wires are the discovery surface.
- **Pick the lab / startup / lead counterparty's own IR release as
  `source_url`** when they have one. The lab's wording is the most
  precise for downstream readers, and the IR release is the URL the
  renderer audit expects.
- **Use wires (FT, Bloomberg, Reuters) as the *timing* source**, not
  the `source_url`. They tell you the scoop window; the IR press
  release tells you the canonical terms.
- **Don't split the deal across ranks by counterparty.** "Nvidia+Apollo"
  and "Nvidia+BlackRock" are not two stories — they're one financing
  platform with six counterparties. Rank-2 would be a follow-on (a
  FT editorial, an S&P credit rating, a hyperscaler's response), not a
  subset of the same deal.
- **The Information's follow-on briefings are engineer-relevant.** A
  same-day Information piece that adds *new* facts (a backstop
  percentage, an offtake structure, a regulatory wrinkle) is rank-2
  material for the same deal, not duplicate coverage.

**Confirmed second instance (Aug 11, 2026 Nvidia + 6 Wall Street
firms, $500B AI compute financing):** the FT original scoop (Aug 10
evening UK time) was followed by Reuters/CNBC confirmation within hours
and by Nvidia's own newsroom press release the next morning. The
Information's follow-on (Aug 11) added the 25% Nvidia backstop fact.
Rank as one story: Nvidia newsroom URL as `source_url`, body weaving
all six counterparty quotes from the Nvidia release, dek mentioning
the 25% Information backstop. Same pattern as Anthropic-Volta — the
only thing that changed was which lab's newsroom was the canonical
primary.

## Beat-specific wordsmithing

AI-industry headlines often drift into hype terms ("revolutionary",
"game-changing", "AGI"). The umbrella skill's 6–12 word headline budget
forces compression that usually strips this out, but watch for:
- "frontier" — fine when used by the lab itself; avoid in editorial dek.
- "GPT-5.x" / "Claude X" — keep version numbers exact, never round.
- "OpenAI says..." / "Anthropic claims..." — prefer "OpenAI cuts..." /
  "Anthropic releases..." for declarative headlines.
- Company name + product name + number — fits the budget, satisfies the
  "why anyone cares" dek question for engineers.

## When the lab announcement is behind a paywalled explainer

The Information and Stratechery routinely break AI-industry stories a
day before the lab's own blog post goes live. For 72-hour windows, if
the lab hasn't published yet, the story is *not yet* a primary-source
story — wait until the lab post lands, or rank the explainer as the
story (and accept the secondary-source trade-off). Don't rank both
unless they say materially different things.

## AI-industry cluster around EU AI Act enforcement (Aug 2-7 of every year)

The Aug 2 EU AI Act enforcement date reliably pulls a cluster of
adjacent in-window stories. Confirmed 2026 cluster (Aug 2-7):

| Story | Date | Beat slot |
|---|---|---|
| EU AI Act Article 50 enforcement begins | 2026-08-02 | Lead regulation/policy story |
| EU Commission publishes 180+ signatory list for the Code of Practice on AI-Generated Content | 2026-08-02 (in same press release) | Bundled with the enforcement story |
| Anthropic-Volta $10B compute deal | 2026-08-04 | Lead infra/business story |
| Bitdeer $4.7B Tydal colocation lease (subordinate to Anthropic-Volta) | 2026-08-04 | Folded into the Anthropic-Volta body |
| Alibaba Qwen 3.8-Max open-weights flagship | 2026-08-03 | Lead lab announcement / open-source |
| Meta Muse Code + Muse Spark 1.2 coding agent | 2026-08-05 | Rank-2/3 lab announcement |
| Apple–OpenAI trade secret injunction | 2026-08-04 | Rank-2 legal/IP story |
| SpaceX first earnings (AI capex angle) | 2026-08-04 | Often outranked by Anthropic-Volta on the same day |
| China State Council comprehensive AI law | 2026-08-03 | Rank-2/3 policy story (China jurisdiction) |
| OpenAI Codex education plugins | 2026-08-04 | Tier-4 filler if needed |

**Treatment:** for an Aug 4-7 window, lead with EU AI Act (Aug 2) if
no fresh mega-deal has dropped, OR lead with Anthropic-Volta (Aug 4) if
the deal's the bigger news that week. Rank Alibaba's open-weights
flagship + Meta's coding agent + Apple's legal move as a tight cluster
of rank-2/3/4 stories. **Don't split Anthropic-Volta into multiple
ranks** — it's one transaction. Don't split Alibaba's preview and
launch — preview is Feb/Jul, launch is Aug 3, the Aug 3 launch is the
in-window event.

For an Aug 7-9 window (i.e. the current Aug 7 morning brief), EU AI
Act enforcement (Aug 2) drops out of the 72-hour window — but the
follow-on consequences (the first compliance complaints, the first
fines, the Commission publishing enforcement decisions) become
rank-worthy. Verify which post-Aug-2 EU AI Act stories have landed
before locking the regulation rank.

## mmx image generation — when "no text" prompt still produces text

The umbrella's `mmx-batch-workflow.md` warns that mmx image-01
occasionally renders fake text/logos. **Confirmed hard failure mode
(Aug 6, 2026 chip render):** a prompt like

> "Editorial close-up photograph of a modern AI accelerator GPU module
> with golden heatsink fins, dark background with dramatic rim
> lighting, technology publication aesthetic, sharp focus, no text
> anywhere, no labels, no logos, no symbols"

still rendered fake text on the chip surface ("GOLEM" or similar
fabricated label). Two retries with similar prompts produced the same
issue. **The "explicit 'no text'" prompt is unreliable for close-up
objects with embedded labels** — the model interprets a detailed
physical object as needing surface detail, and surface detail on
tech hardware usually means labels.

**Fix pattern that worked:** switch to neutral abstract subjects —
architecture, fjord landscapes, building exteriors — for any mmx
generation where fake text would be a deal-breaker. The same session
got clean renders for:

> "Editorial news photograph of the European Commission Berlaymont
> building in Brussels at dusk with EU flags, blue hour lighting"

(EU flags are geometric repetition, no embedded text)

> "Aerial editorial photograph of a large AI data center building in
> a snowy Norwegian fjord valley, hydroelectric power lines visible"

(buildings + landscape, no embedded labels)

**Heuristic for prompting mmx image-01:**

- ✅ Pass: architecture exteriors, landscapes, abstract particle/flow
  compositions, geometric patterns (flags, repeating textures).
- ⚠️ Borderline: editorial photo compositions of people (faces can
  generate fake labels on shirts, name badges).
- ❌ Fail: close-up tech hardware (chips, GPUs, devices), UI
  screenshots, anything that the model would naturally associate
  with embedded text/logos.

When you need a tech-hardware lead image, don't use mmx — fetch a
vendor press photo (NVIDIA iPR, AMD newsroom, Intel Blue Carpet CDN)
or fall back to a wider environmental shot (server room, data
center exterior) where fake labels would be implausible.

## Hotlink protection on AP photo CDNs

(This section is intentionally duplicated above under "Hotlink
protection on AP photo CDNs" — leaving the table of contents-style
marker here as a navigation anchor for older leaf runs that scroll
to "Hotlink protection" looking for the AP CDN section.)
