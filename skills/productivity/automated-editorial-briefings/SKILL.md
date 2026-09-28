---
name: automated-editorial-briefings
description: "Use when building recurring visual briefings/newspapers. Source, render, verify, deliver. Covers fan-out to multiple subscribers and consent posture for real-human cron recipients."
version: 1.6.0
author: Hermes Agent
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [automation, cron, newspaper, briefing, html, pdf, telegram, editorial]
    related: [service-monitor-telegram, claude-design, iterative-image-generation, mmx-cli]
---
# Automated Editorial Briefings

Use this skill for recurring, designed briefings rather than simple text alerts: personalised newspapers, daily research digests, executive morning papers, visual market briefs, and similar scheduled editions delivered as images/PDFs.

This is an umbrella workflow. It composes:

- `service-monitor-telegram` for script-owned cron and Telegram delivery;
- `claude-design` for reference-led composition and visual QA;
- `iterative-image-generation` / `mmx-cli` for generated editorial art.

**References:**
- `references/newspaper-pipeline.md` — baseline source-integrity and rendering workflow (Pattern A).
- `references/minimax-premium-pipeline.md` — MiniMax-M3 editorial JSON, image-01 art, authenticated GitHub, final-run visual QA for a premium multi-page edition.
- `references/agent-driven-pipeline.md` — **Pattern B** orchestrator pattern: agent decides structure, correspondent sub-agents, real charts, continuity from yesterday.
- `references/personality-as-publisher.md` — pairing the paper with a named personality preset so the cron entry has a voice.
- `references/broadsheet-design.md` — broadsheet visual language (italic display, justified columns, B&W art, warm newsprint). Trigger: "make it look more like a newspaper" / "looks too web / too modern".
- `references/hermes-send-media.md` — `hermes send` Telegram payload flags, `MEDIA:` syntax, and the cron-mode delivery contract.
- `references/hermes-times-v4-architecture.md` — **working Pattern B reference** (THE HERMES TIMES v4, deployed 2026-07-26, 10-page edition shipped 2026-07-26). Agent-driven personal newspaper with 5 fixed beats (AI Industry, Research, Hugging Face, Messi, F1), hybrid real-photo + generated-illustration image strategy, 30-min wall clock, deliver=telegram. Use as the concrete template when the user says "build me a daily newspaper."
- `references/leaf-beat-reporter-workflow.md` — **leaf correspondent playbook** for any beat (AI industry, research, sports, etc.). Window discipline, primary-source discovery, image verification with `file`, JSON output contract, pitfalls. Load this when executing a leaf beat brief (the orchestrator spawns these as sub-agents in Pattern B).
- `references/f1-leaf-beat-playbook.md` — **F1 / motorsport leaf extraction specifics**. formula1.com as a Next.js app (dates in streamed JSON, hero image IDs in `og:image`/preload links, credits hidden in `__next_f.push()`), FIA.com timezone trap (CEST `+02:00`, not UTC), Cloudinary URL transforms for guaranteed JPEG downloads — **three FOM Cloudinary paths**: (1) `trackside-images/<year>/<GP>/<ID>.jpg` race-weekend photos, (2) `fom-website/<year>/<Team>/<File>.jpg` editorial uploads — both honour the `.jpg` extension as the JPEG transform, AND (3) the **archival `content/dam/fom-website/sutton/<year>/<Country>/<Day>/<hash>.webp` path** (used for historic / anniversary / lifestyle photos — does NOT honour extension-swap; convert with `ffmpeg -y -i in.webp out.jpg`). 2026 steward penalty guidelines. **Also covers:** GrandPrix247 as a parallel shutdown-beat discovery surface (4 in-window stories/day, dated bylines), the GrandPrix247 S3 image CDN (direct JPEG download, no auth), the Motorsport.com Cloudflare-UA trap (default `Mozilla/5.0` returns 919-byte HTML block; use `Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)` or skip to `web_extract`), the Liberty Media 8-K as the canonical F1 financial source, the `as-web.jp` Auto Sport Web Japanese-language pickup pattern, the **vision-verify-the-lead-image-before-composing-JSON hard rule** (a topical historic image only matches the lead if the lead actually centres that subject — confirmed F1 v4 2026-08-12), and the rank-4-is-a-fallback-not-a-forced-slot principle. Load in addition to the general leaf playbook when the beat is F1, F2, F3, or any other FIA championship.
- `references/messi-leaf-beat-playbook.md` — **soccer-star / named-player leaf extraction specifics**. Foreign-timezone match-day math (MLS 7:30 PM ET = 5:00 AM IST next day, La Liga / UCL 9:00 PM CET = 1:30 AM IST next day), preview-vs-result framing (verify with `TZ=Asia/Kolkata date` before writing the lead), the **no-result-is-still-a-lead** case (preview-only file: projected XI, manager quotes, qualification math, notable absences, broadcast info — do NOT invent a result), image-fallback chain when no fresh match photo exists (most recent prior match with same player, encoded in the slug e.g. `messi_leaguescup_goal_san_luis_aug5.jpg`), the **Miami Herald / McClatchy Zenfs CDN** pattern (`media.zenfs.com/en/<outlet-slug>_slideshows_<n>/<hash>` — default curl works, stable JPEG, no auth, discoverable via `web_extract` on Yahoo Sports slideshow pages), Bolavip credit-preservation rule (Bolavip credits the original photographer in the caption — reuse it), TyC Sports / Olé / AFA CDN patterns, the "preview + secondary" stacking pattern (preview is always rank 1; national-team window / contract / off-pitch news fills rank 2), and the **match-report fabrication** failure mode (a leaf with no `web_extract` budget for the live scoreboard will sometimes emit a plausible 2-1 recap from memory — the leaf must self-report `fixture_status` in its summary text). Load in addition to the general leaf playbook when the beat is a named soccer player (Messi, Ronaldo, Mbappé, Haaland, Bellingham, Vinícius Jr, Yamal, etc.) and the same-day fixture is in a foreign timezone.
- `references/research-beat-arxiv-extraction.md` — **Research-beat arXiv extraction specifics**. arXiv API `sortBy=lastUpdatedDate` (not `updatedDate` — that returns HTTP 400), HTTPS vs HTTP for `export.arxiv.org`, why `arxiv.org/abs/<id>` returns metadata-only via `web_extract` (no prose), the `arxiv.org/html/<id>v<N>/x1.png` figure URL as a robust Tier-1 fallback when the HF paper-thumbnail CDN times out, regex-on-bytes parsing for author/title extraction, big-lab tech-report author conventions (Kimi Team etc.). Load in addition to the general leaf playbook when the beat is Research or any other beat that pulls from arXiv.
- `references/issue-7-2026-08-01-data-points.md` — Issue 7 (2026-08-01) edition specifics: the **4-reporter override** (user says 4 reporters, brief has 5 beats — fold HF into AI Industry as sub-beat), **foreign-timezone match-day math** (7:30 PM ET = 5:00 AM IST next day → preview not result), lead timing window anchored to publication moment, leaf output path inconsistency across runs, reusable framing patterns ("two frontier labs, ten days, two real incidents", "hours after X did Y, the next vendor matched", "N points clear into the silence").
- `references/style-component-library.md` — **the broadsheet component library** that ships in `style.css` for v4. Per-component schema for `stat_block`, `rank_list`, `wire_table_v2`, `mini_chart`, `score_line`, `opinion`, `weather`, `tomorrow_list`, `paper_grid`, `editorial_aside`, attributed `quote`. When you add a component, add a row here. Load when composing a multi-page manifest.
- `references/leaf-transcript-recovery.md` — **when a beat leaf's transcript truncates the JSON** to `...(+N chars)`. The full JSON lives at `~/.hermes/cache/delegation/subagent-summary-<task>-<ts>.txt`. Do not re-spawn the leaf — read the summary file and renormalize.
- `references/render-artifact-overwrite.md` — **`render.py` writes `issues/<date>.{html,pdf,png}` unconditionally**, so a v2 render for the same date wipes v1's artifacts. Snapshot before re-rendering.
- `references/issue-10-2026-08-05-data-points.md` — Issue 10 (2026-08-05) edition specifics: AI Industry leaf **hit iteration cap and returned JSON only in summary text** (fourth case of the leaf-output-path pitfall), AI Industry leaf **fabricated a math claim** ("Connes rigidity conjecture disproof" — not in OpenAI's actual paper list), **OpenAI CDN URL returned a 409 KB blue-gradient placeholder JPEG** instead of a real pricing card, 4.99 MB PDF delivered clean through Gmail SMTP. Use this when verifying subagent technical claims and CDN image integrity.
- `references/issue-11-2026-08-06-data-points.md` — Issue 11 (2026-08-06) edition specifics: AI Industry leaf **hit the `loop_web_search_cap` after ~50 searches** without ever writing its JSON to disk (a different failure mode from `max_iterations` — assets landed but the JSON didn't), but the leaf did successfully generate 5 mmx cover-style images before timing out, so the orchestrator composed the beat JSON itself from the partial transcript + verified sources. Also: the **mmx image-01 football-kit hallucination** (fake Pirelli sponsor → American-football helmet → clean silhouette on third try) and its safe-prompt pattern. Plus: the **Telegram auto-skip is the delivery, not a failure** — `send_all.sh` exits non-zero but the cron runner carries the PDF in this final response, so the closing note must reflect that explicitly rather than retrying or reporting it as a delivery failure. Use this when an orchestrator has to recover a beat JSON from a partial transcript and when verifying any sports/people editorial illustrations.
- `references/issue-12-2026-08-07-data-points.md` — Issue 12 (2026-08-07) edition specifics: **third consecutive leaf-cap failure, second consecutive `loop_web_search_cap` on the AI Industry leaf** (Issues 10, 11, 12 all had AI Industry as the failure beat — once `max_iterations`, twice `loop_web_search_cap`). Confirmed the recovery recipe (read partial transcript, verify each candidate against primary source via `web_extract`, compose JSON orchestrator-side). Validated **two-batch dispatch pattern** (split 5 leaves into 2 batches of 3+2 when one leaf is suspiciously slow — saves wall clock over waiting for a stuck leaf). Notes that AI Industry is the repeat offender because 6 stories × ~3-5 searches per story × 2 verification rounds exceeds the cap; mitigation = either lower the count (4 not 6), pre-supply source URLs, or add an explicit search-budget hint in the leaf context. Also: **Gmail SMTP `ssl.SSL.write` timeout** on 16 MB PDFs (different failure mode from Issue-9 silent drop and Issue-10 slow success). Use this when the AI Industry leaf is the slow one and when Gmail delivery silently hangs.

**Templates:**
- `templates/orchestrator-prompt.md` — copy-pasteable cron-entry prompt for the agent-driven mode.
- `templates/preview-edition-prompt.md` — **when the kid asks for a draft to vet before tomorrow's run**, use this: build the full layout, archive to `Documents/fun/HermesTimes/<DATE>-v1/ed.html`, footer reads "Presses: HOLDING pending kid's read", stop. Do not register the cron, do not send to Telegram, do not run final QA. The preview is a read-through artifact, not the morning paper.
- `templates/send_pdf_mail.py` — **Gmail SMTP + GOA XOAUTH2 PDF-attachment fallback** for when `hermes send` silently drops the Telegram upload (gateway log shows zero outbound activity but the CLI returns 0). Reuses the GNOME Online Accounts token; no app password. Self-contained (do NOT import `~/.local/bin/mail` — it's `__main__`-guarded). Use `smtplib.SMTP.timeout=120` for >4 MB PDFs. See `references/hermes-send-media.md` "When `hermes send` silently drops the message" for the full failure-detection recipe and the pivot-not-retry rule.

**Scripts:**
- `scripts/venv-run.sh` — wrapper that invokes a project venv's python with `env -u PYTHONPATH` to dodge hermes-agent venv pollution. Use any time a project script imports a package not installed in hermes-agent venv.

## Core architecture

Two valid architectures. Pick one based on what the user actually wants — do not default to the lighter one when the user asked for editorial richness.

### Pattern A: deterministic no-agent script

Use when the content shape is fixed (one-page briefing, fixed sections, no charts, no correspondent voices) and the script owns all synthesis. Fast, cheap, fully reproducible.

```text
cron (no_agent=true, deliver=local)
  -> parallel deterministic collectors
  -> editorial LLM produces structured JSON
  -> deterministic integrity gates
  -> reference-led HTML/CSS template
  -> headless-browser PDF + PNG
  -> verify exact page count and artifact sizes
  -> script sends attachments through `hermes send`
  -> empty stdout on success
```

Why: the scheduler should not add a second LLM pass or duplicate delivery. The script owns data collection, synthesis, rendering, and the exact Telegram payload.

### Pattern B: agent-driven editorial mode

Use when the user wants the **agent itself** to decide structure, sections, visuals, and correspondent voices — not a fixed template. Triggers include any of:

- user says "the agent should do the whole thing" / "not deterministic" / "decide based on the news";
- the brief calls for **real charts/graphs** generated from the day's data (not just decorative art);
- sections should be optional, mergeable, or invented-by-the-agent (e.g. "skip the systems desk if no releases today");
- correspondent sub-agents writing individual articles in their own voice are valuable;
- continuity from yesterday's issue matters (the agent reads the previous issue).

```text
cron (no_agent=false, deliver=local, prompt-based)
  -> orchestrator agent (full toolset)
       -> reads previous issue for continuity
       -> runs parallel collectors
       -> decides today's structure: page count, sections, whether charts warranted
       -> spawns 4-8 leaf sub-agents ("correspondents") in parallel
       -> collects articles, writes manifest.json
       -> generates any charts (matplotlib inline, no API)
       -> generates any art (mmx image generate)
       -> renders HTML via thin render.py -> Chrome -> PDF -> PNG
       -> vision-QA the rendered pages, re-render if needed
       -> hermes send attachments
```

See `references/agent-driven-pipeline.md` for the full orchestrator pattern, beat templates, and the manifest schema. See `templates/orchestrator-prompt.md` for a copy-pasteable cron-entry prompt. See `references/leaf-beat-reporter-workflow.md` for the playbook each leaf correspondent follows to produce its JSON package.

Pattern A is the right answer for a simple file-watch brief or a one-page morning card. Pattern B is the right answer for a personal newspaper where the user reads it for editorial judgment, not just headlines. When the user describes the target as "a newspaper" — not "a briefing" or "a card" — Pattern B is almost always what they mean.

## Quality bar: source-backed does not mean source-text-only

For a personal newspaper meant to feel like a real morning briefing, do **not** default to a sparse broadsheet assembled from deterministic excerpts. The source pack is the factual boundary; a strong edition can still use an editorial model to select, synthesize, and frame those facts, with imagery as part of the composition.

When the user authorizes an LLM/image budget (for example MiniMax-M3 plus `image-01`):

- give the editorial model a structured source pack and require strict JSON; repair every title, author, date, URL, paper identity, and repo identity from the collected records before render;
- have it write the issue theme, lead headline/dek, concise story angles, and **text-free** editorial-art prompts;
- use generated art only as conceptual/atmospheric illustration, never as evidence; use official article imagery only with clear attribution/licensing;
- generate a cover/lead visual plus supporting section visuals large enough to carry the composition—not tiny decorations;
- commit to an intentional editorial composition (cover, briefing, research desk, systems desk) before CSS. Do not force thin copy into a fixed multi-page shell merely to hit a page count;
- pause an existing scheduled delivery during a visual rebuild, and resume only after a real rendered issue has passed visual QA and a force-run.

A complaint that the paper looks sparse, ugly, or makes the user "want to puke" is a **composition failure**, not a request for cosmetic recolouring. Rebuild hierarchy, imagery, density, and editorial voice together.

## Workflow

### 1. Ground time and lifecycle intent

Run `date` before selecting a schedule. List jobs, identify the exact job IDs, and distinguish:

- replace only a related predecessor;
- replace all jobs (only when the user explicitly says so);
- coexist with unrelated jobs.

Never guess IDs. Remove the old job only after the replacement pipeline has passed a dry run, unless the user explicitly prioritises immediate removal.

### 2. Define the source pack

Collect structured evidence, not prose impressions. Typical classes:

- current news from official feeds and reputable reporting;
- recent papers from arXiv;
- relevant repositories from GitHub Trending/API;
- personal work from session metadata, Git status/logs, GitHub contributions, and knowledge-vault edits;
- structured weather or market APIs.

Distinguish **committed work**, **uncommitted working-tree changes**, and **topics merely explored in sessions**. A session title is evidence of attention, not completion.

Run independent collectors concurrently. Scheduled jobs may have hard runtime limits; serial network collectors can consume the budget before synthesis or delivery begins.

### 3. Ask the editorial model for JSON

The model should select and write, not control rendering. Require a strict schema for lead, briefs, personal-work ledger, paper, repository, weather, sidebar, quote, and source note.

Prompt rules:

- only use facts/URLs in the source pack;
- prefer primary sources and reputable reporting;
- reject duplicates and SEO noise;
- do not invent benchmark values, dates, work completed, or attribution;
- explicitly distinguish public contribution activity from private/local work.

In Pattern B (agent-driven), the editorial-model call is replaced by a fan-out of correspondent sub-agents who each write one article, followed by an editor pass that produces the final manifest. Same source-integrity rules, different control flow.

### 4. Apply deterministic integrity gates

Never trust editorial JSON solely because it parses.

- URL allowlist: every published URL must be present in the source pack.
- Paper identity: paper title, authors, and URL must all match one arXiv candidate, not merely any allowed news URL.
- Repository identity: `owner/repo`, URL, and trend signal must match one repository candidate.
- Weather/market numbers: overwrite model prose with structured API values.
- If a selection fails, choose a scored deterministic fallback from the correct source class.

This prevents semantically wrong but syntactically valid output, such as a news article being presented as an arXiv paper.

In Pattern B, the orchestrator agent runs these gates itself; an article that fails the gate is rejected and a fallback is requested from the orchestrator.

### 5. Translate references structurally

When the user supplies a screenshot and says "make it look more like this," treat that as a design correction, not optional inspiration. Extract and reproduce:

- page aspect ratio and occupied-height ratio;
- masthead scale and type class;
- metadata strip and rule weights;
- headline case, italic posture, line height, and width;
- number and ratio of columns;
- density, alignment, boxes, captions, and image scale;
- monochrome/halftone treatment and paper texture;
- finishing motifs such as an editorial bar.

Change the **composition first**, then tokens. A palette tweak cannot fix a modern hero layout when the reference is a dense broadsheet.

For vintage newspapers, use restrained black ink, serif display/body typography, narrow justified columns, thin rules, small bylines, small grayscale/halftone imagery, and ivory newsprint texture. Avoid cards, rounded corners, gradients, gold accents, and oversized decorative hero images unless present in the reference.

In Pattern B, the agent pulls from a CSS component library (`STYLE.md` if you build one) rather than a fixed template; the layout tokens are things the agent can compose, not a single rigid page.

### 6. Generate art as supporting material

Generated imagery should occupy the role the reference assigns it. If the reference has one small portrait, do not let generated art become a dominant hero.

- request no text/logos/signatures;
- use `--out-dir`;
- vision-review for fake glyphs and crop quality;
- apply grayscale + contrast/halftone treatment in CSS or post-processing;
- maintain a known-good fallback image when generation/quota fails.

Pattern B adds: real charts (matplotlib inline) are distinct from generated art. Charts are evidence; art is atmosphere. Don't conflate them in manifest or render.

### 7. Render and verify

Render HTML to PDF with a headless browser, then rasterise page 1 for Telegram preview.

Mandatory checks:

- HTML exists and opens;
- no browser console errors;
- PDF exists and has plausible size;
- `pdfinfo` reports the intended page size and **exact requested page count**;
- PNG dimensions and file size are plausible;
- inspect the final PNG visually, including lower-page content;
- for a multi-page edition, rasterise and inspect **every page** (or representative regions on every page), not only the Telegram cover;
- audit clipping, overlap, unreadable type, accidental gibberish, and excessive empty area.

The page-count gate must follow the user's editorial brief: one page for a briefing explicitly meant as a front-page card; multiple intentional pages for a newspaper. Never compress a user-requested newspaper into one page merely because the initial template was one-page. Conversely, do not treat accidental overflow as a multi-page edition: page breaks, folios, section hierarchy, and content density must be deliberately designed.

In Pattern B, the agent runs a vision-based QA loop on its own output before delivery; failed pages get re-rendered or fixed in the manifest until they pass.

### 8. Delivery and cron verification

For script-owned Telegram delivery (Pattern A):

```text
no_agent = true
deliver = local
script = <relative script under ~/.hermes/scripts/>
```

For agent-driven delivery (Pattern B), the cron entry's `prompt` is the orchestrator agent; the agent itself calls `hermes send` with `MEDIA:<pdf_path>` in the message body. Use `no_agent=false` and `deliver=telegram` (not `local`) — the scheduler pushes a status ping so the user knows the run happened even if the agent crashes before its own send. The agent's `MEDIA:` send carries the PDF; the scheduler's telegram delivery carries the cron output log. They don't duplicate.

In both modes, a successful force-run should update artifact mtimes and show `last_status=ok`. This verifies the scheduler environment, not merely the interactive shell.

## Pitfalls

- **Defaulting to a deterministic script when the user asked for an agent-driven newspaper.** Triggers like "the agent should do the whole thing", "not deterministic", "real graphs", "spawn correspondents" all map to Pattern B, not Pattern A. A templated 4-page script that fills fixed regions is the wrong answer for that brief. Default to Pattern A only when the user described the artifact as a "briefing", "card", or "alert"; default to Pattern B when they said "newspaper", "edition", "read", or "make it yours".
- **Compressing a user-requested 10-page newspaper into a 6-page residual layout.** If the user explicitly asked for 10 pages and you have wire copy for them, build 10. A 6-page compressed paper fails the brief even if "all the beats fit." The kid reads the page count.
- **Re-rendering an edition for the same date without snapshotting the previous one.** `render.py` writes `issues/<date>.{html,pdf,png}` unconditionally, so v2 wipes v1's PDF/PNG. Snapshot first, or write each version to a dated subdirectory. See `references/render-artifact-overwrite.md`.
- **Treating Pattern A as the only canonical pipeline.** The skill previously declared the no-agent script as the one true architecture. That was wrong for editorial-grade personal newspapers. Pattern A and Pattern B are both valid; pick by trigger language.
- **Deleting every job when "existing cron job" meant one predecessor.** Inspect names and ask only when lifecycle intent is genuinely ambiguous.
- **Creating cron before the artifact works.** Dry-run first, then replace and force-run.
- **Using `deliver=telegram` while the script also sends Telegram.** This duplicates delivery; for Pattern A use `local` for the scheduler. **Exception** (Pattern B): the agent's `MEDIA:` send carries the PDF, the scheduler's telegram delivery carries the agent's reply/log — they carry different payloads, not duplicates. See the v4 reference for the working recipe.
- **Pattern B with invented beats instead of fixed beats.** Letting the agent decide which beats to cover every day produces a paper that doesn't feel like *yours*. If the user has standing interests (research, a sport, an industry), fix the beats in the brief and let the agent decide section *structure*, not section *membership*.
- **Beat-count override: user says N reporters but standing brief lists M > N beats.** When the user explicitly tells the orchestrator "spawn K reporters" but the standing brief lists more beats than K (e.g. user says "4 reporters" while brief lists AI Industry, Research, Hugging Face, Messi, F1), do NOT skip a beat and do NOT exceed K. Fold the **closest related** beat(s) into the parent: HF → AI Industry (HF is the venue for open-weight model releases; AI Industry covers lab announcements), Research → AI Industry (paper-of-the-day framing), sports sub-beats → parent sports beat. Document the fold in the parent reporter's brief ("handle HF as a SUB-BEAT since the user is running 4 reporters not 5") so its JSON package exposes the folded beat's stories alongside its own. The folded beat's source URLs and image-CDN conventions still apply (e.g. `cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png`). Confirmed in Issue 7 (2026-08-01): 4 reporters carried 5 beats cleanly; wall-clock dropped from ~30 min (5 leaves, Issue 3) to ~22 min (4 leaves, Issue 7). See `references/issue-7-2026-08-01-data-points.md` for the recipe.
- **Leaf beat output path inconsistency across runs.** Leaves have been writing their final JSON to three different paths across issues: `~/.hermes/scripts/<slug>/<beat>_2026-XX-XX.json` (Issue 7 AI Industry + Research), `~/.hermes/data/<slug>/beat_<beat>_2026-XX-XX.json` (Issue 7 Messi), or **no on-disk file at all** with the JSON living only in the delegate summary (Issue 7 F1). The orchestrator must NOT assume a fixed path. **Always read the leaf's JSON from the delegation cache summary file first** (`~/.hermes/cache/delegation/subagent-summary-<task>-<ts>_<pid>.txt`); treat any on-disk file as advisory and reconcile via the summary if paths disagree. The leaf should return the JSON as its final assistant message — on-disk writes are at best redundant and at worst contradictory. `references/leaf-beat-reporter-workflow.md` says this already; the gap is that leaves do it inconsistently in practice. **Fourth case (Issue 10):** the leaf hit `max_iterations` and returned the JSON only in the summary text, with no on-disk file at all — `exit_reason: "max_iterations"`. The orchestrator's recovery is (a) treat the summary text as authoritative, (b) write it to a canonical on-disk path before manifest build, (c) audit the JSON for the same fabrication / verification risks as if the leaf had written it itself. **Root cause:** the leaf did more tool calls than the others (10+ web searches, 4 image downloads, 4 vision checks) and ran out at the 50-call agent cap. Future-work: pass a `--max-tool-calls 35` hint in the leaf context for beats with 5+ stories to gather. **Fifth case (Issue 11):** the leaf hit a **different** guardrail — `loop_web_search_cap` after ~50 web searches that returned no progress — and never wrote the JSON at all. The leaf's own final summary explicitly recommended "change strategy instead of repeating the same call" and exited cleanly with `exit_reason: "completed"`. **Symptom:** the orchestrator sees no on-disk JSON for the beat, but the partial transcript in `~/.hermes/cache/delegation/live/deleg_*/task-N.log` shows the leaf DID identify the right 4-5 stories and DID download/generate cover-style images for them — assets on disk, no JSON. **Recovery recipe:** do NOT re-spawn the leaf. Read the partial transcript, extract the story candidates the leaf named (each one with its primary-source URL it discovered via web_search), compose the beat JSON orchestrator-side with those URLs and bodies, and trust the leaf's already-downloaded assets. Verify each story against its primary source (`web_extract` the URL, confirm headline/body match) before pasting into the manifest — the leaf's wording may have been aspirational, not verified. The orchestrator's own editorial pass replaces the missing leaf JSON, but the leaf's asset-preparation work isn't wasted. See `references/issue-11-2026-08-06-data-points.md` for the full recovery recipe with the Issue-11 example.
- **Subagent fabrication of named-theorem / technical claims.** Leaves are confident writers and may *plausibly extend* a primary source into a stronger claim than it supports. **Verified case (Issue 10, AI Industry leaf):** the leaf's draft body for the OpenAI/Astra story claimed a "disproof of the Connes embedding conjecture" — Connes was not in OpenAI's actual paper list. The leaf knew the name "Connes embedding conjecture" and associated it with "impressive open problem" and emitted both as a plausible-sounding combo. The leaf's own summary flagged the risk ("first time I'd seen 'Connes rigidity conjecture' referenced as a disproof — still recommend a human spot-check") but framed it as a polite spot-check, not a hard rule. **Orchestrator rule:** for any technical claim about a paper's results, the named theorem, the specific disproof or construction, the author list, the arXiv-ID, or the model/version cited, **verify against the primary source** (the paper page, the lab publication, the HF paper page) before pasting into the manifest. If the leaf's claim is more specific than the primary source supports, drop the specific claim. If the leaf's claim is more dramatic than the primary source supports, drop the drama. Math, science, named-theorem, and named-author claims are highest-risk; routine news claims (a result, a date, a price) are lower-risk but still worth spot-checking the first time the leaf surfaces them.
- **CDN-hosted image returned a generic placeholder, not a real image.** Verified case (Issue 10): the AI Industry leaf's pricing story pointed at `images.ctfassets.net/kftzwdyauwt9/.../Frame.png` (OpenAI Contentful CDN). The leaf `file`-checked the file: 409 KB, JPEG, "looks like a real image." The orchestrator's vision review caught it: the 2160×2160 file was a generic OpenAI brand-blue gradient with no pricing content, no typography, no real photo. **Pattern:** Contentful CDN URLs (`images.ctfassets.net`, `cdn.shopify.com`, similar) sometimes return generic placeholder images, especially for assets that have been removed or moved by the content owner. The leaf's `file` + size check is necessary but not sufficient — only `vision_analyze` reliably catches a placeholder. **Orchestrator rule:** if the leaf skipped vision-verification on any image (because it was the lowest-priority story, or because the iteration cap fired), the orchestrator must run its own vision check on every image in the manifest before render. A vision-rejected image either becomes a text-only card (drop `card_art` to `null`) or gets replaced with a real image from a fresh download. The schema explicitly allows text-only cards ("If absent, the card is text-only").
- **Treating an LLM-selected allowed URL as type-safe.** Validate paper/repository identity by source class.
- **Serial collection under a hard scheduler timeout.** Parallelise independent sources and give network calls bounded timeouts.
- **Calling a three-page PDF a one-page newspaper.** Gate with `pdfinfo` before rasterising.
- **Responding to a visual reference with cosmetic recolouring.** Recompose around the reference's hierarchy and density.
- **Using generated image text.** Request no text and inspect pixels; text-like artifacts are common.
- **Inventing work from file modification timestamps.** Use diffs, commits, and session evidence; state uncertainty.
- **Letting empty cells render before they are filled.** When a region has a fixed height and short content, the visible empty space reads as "template did not render." Fill the region with real editorial content (e.g. pour `yesterday.items` work notes into the right column) before falling back to decorative ruling. Reserve ruled/notebook filler for overflow only.
- **`column-count:2` with default `column-fill:balance` on thin copy.** Short text wrapped in two columns leaves the second column half-empty, looking broken. Either lengthen the copy or set `column-fill:auto` so a short paragraph just fills the first column instead of splitting into two sparse sub-columns.
- **Trusting `pdfinfo` + file size alone for visual completeness.** A one-page, well-sized PDF can still look like an unfinished template. Always crop the rendered PNG into the major regions (left/centre/right columns, footer) and inspect each one with vision. JSON-parses, exact-page-count, plausible-file-size are necessary but not sufficient.
- **String-concatenating optional model fields without a guard.** Editorial repos, paper fields, and trend signals often arrive empty. `value + " suffix"` then renders as `" suffix"` or as a description field glued onto the wrong slot. Always default to a fixed string ("Trending on GitHub today", "No paper selected") when the source value is empty.
- **Letting shell-path strings reach the printed page.** A literal `~/.hermes/data/...` in a footer or sidebar reads as debug noise. Use a clean phrasing like `Archive on disk · see ~/.hermes/...` (write the tilde into the layout deliberately as a path hint, not as leftover code).
- **Trusting a word-count spec alone to fill a fixed-height column.** Editorial models are lazy: a "110-170 word body" spec is routinely satisfied with 120 words even when the column needs 280 to look full. The layout reads as "incomplete" even though the JSON parsed cleanly. **Fix:** after parsing the editorial JSON, count words on every fixed-height region (lead body, briefs, yesterday items, etc.) and either (a) retry the model with an expand instruction, (b) tighten the spec to a higher minimum, or (c) pull fill content from another region (e.g. pour `yesterday.items` into the right column). One-shot retry is enough — the second pass usually lands within ±15% of the target. Pattern: `if body_words < MIN: result = expand_lead_body(parsed); return result or parsed`.
- **Pouring `yesterday.items` (work notes) into multiple regions.** The same `yesterday.items` list can populate the right-column "Your Work Reconstructed" panel and the bottom "Yesterday's Ledger" band without contradicting itself. Reusing the list kills dead space in both places and reinforces the day's narrative. Style the right-column copy slightly tighter (smaller font, no column-count) than the band so the regions don't look identical.
- **Layout verification needs vision, not just `pdfinfo`.** A one-page, well-sized PDF can still look like an unfinished template (empty sub-columns, dead ruled space, missing work items). Always crop the rendered PNG into the major regions (left/centre/right columns, footer) and inspect each with vision. JSON-parses, exact-page-count, plausible-file-size are necessary but not sufficient. Skip the vision pass on re-runs that only changed text; do it on every CSS or template change.
- **Pattern B without a manifest schema.** If the agent is composing the layout, the manifest is the contract between the orchestrator and `render.py`. Without it, every page becomes bespoke HTML and the renderer can't be reused. Define the manifest schema first, then build the renderer against it.
- **Trusting a correspondent's `published_at`, cutoff summary, or `primary` flag.** Leaf packages are drafts, not verified records. Before editing, the orchestrator must compute one explicit UTC cutoff and parse every story timestamp with timezone-aware code; reject rows before the cutoff even if the leaf says the whole beat passed. Normalize source class independently: `primary: true` is reserved for the canonical owner (vendor, regulator, team, arXiv/repo author). Reuters, WSJ, Axios, TechCrunch, MARCA, and similar original reporting are publishable third-party sources, but remain `primary: false`. The exact gate and Python snippet live in `references/leaf-beat-reporter-workflow.md`.
- **Pattern B correspondent sub-agents without a beat template.** Free-form delegation produces inconsistent articles. Each beat needs a fixed prompt template + a JSON output contract. See `references/leaf-beat-reporter-workflow.md` for the canonical leaf playbook.
- **Reading truncated leaf output from the live transcript and assuming the body is unrecoverable.** The live `task-N.log` truncates every long assistant message as `<prefix> …(+N chars)` and earlier turns do not contain the missing text. **Always check `~/.hermes/cache/delegation/subagent-summary-<task>-<ts>_<pid>.txt` first** — that file holds the leaf's complete final JSON and is the authoritative recovery source. See `references/leaf-beat-reporter-workflow.md` ("Recovering a leaf's full output from the delegation cache") for the full recovery recipe and the stale-data trap with `~/.hermes/scripts/<paper>/output/<beat>_beat.json` from prior runs.
- **Using yesterday's leaf JSON as today's.** `~/.hermes/scripts/hermes-times-v4/output/<beat>_beat.json` is from the previous day's run and contains *different headlines*; it silently re-prints yesterday's edition if used in place of today's. Always pull today's leaf output from the delegation cache summary file whose timestamp matches the current `manifest.json` `completed` field, and mtime-check the output dir before trusting it.
- **Pattern B without continuity from yesterday.** The agent re-decides structure from scratch each day and the paper feels like a series of one-offs. The first tool call should read the previous issue's HTML.
- **Pattern B without a 30-min cron budget.** The agent-driven run is 2-4× slower than the script. Schedule at 05:00 rather than 08:00, or extend the cron timeout — the user has explicitly accepted a slower morning paper in exchange for editorial richness.
- **Generated art: download the cover/hero into a slug-specific subdir, not a global `--out-dir`.** The mmx fixed-filename collision (`image_001.<ext>` overwrites per call) means a single global `--out-dir` clobbers the cover when section heroes are generated in the same session. Pattern: `mkdir -p <assets_root>/<slug>_<date> && mmx image generate --out-dir <assets_root>/<slug>_<date> --out-prefix <slug>`. Confirmed in v4 Issue 3 (2026-07-28) where cover, research hero, HF hero, Messi calendar hero, and closing-page hero each landed in their own subdir without conflict.
- **`mmx image generate` exits 0 even when the model produces an aesthetically unusable frame.** A green-blob `image-01` result for the AI cover was saved successfully but vision-rejected; the orchestrator MUST vision-review every generated image and trigger one retry with a rephrased prompt before falling back. Do not trust the JSON `saved: [...]` block as proof of usable art.
- **Two-batch leaf dispatch when one leaf is suspiciously slow.** If 4-5 leaves are dispatched in parallel and one is taking noticeably longer than the others (e.g. the AI Industry leaf is on its 30th web_search call 5 minutes in while the others returned in 70-200s), do NOT wait for the slow leaf to finish before dispatching the remaining beats. Instead: (a) let the slow leaf continue running in the background, (b) dispatch the missing beat(s) in a second batch, (c) recover the slow leaf's JSON from its partial transcript once it caps out. **Pattern validated Issue 12:** AI Industry hit `loop_web_search_cap` after ~540s with no JSON; F1 was dispatched in a second batch, returned clean JSON in ~70s while the orchestrator recovered AI Industry from the partial transcript. Total wall clock ~10 min vs. the ~540s+ you would have waited by dispatching everything in batch 1. **Trigger:** if a leaf has been running >3× the median wall clock of the other leaves in its batch, dispatch the remaining beats in batch 2 and start the recovery path for the slow leaf in parallel.
- **AI Industry leaf is the repeat-offender beat — pre-empt the cap.** Across Issues 10, 11, and 12, the AI Industry leaf was the failure-prone one in **every** case (once `max_iterations`, twice `loop_web_search_cap`). Root cause: 6 stories × ~3-5 web_search calls per story × 2 verification rounds = 36-60 calls, plus image downloads and vision-checks — exceeds the `loop_web_search_cap`. Other beats (Research, HF, Messi, F1) have narrower scopes and complete cleanly. **Mitigation, pick one per run based on news density:** (a) **lower the story count** in the leaf brief — 4 stories instead of 6; (b) **pre-supply source URLs** — "verify these 4 URLs first before searching for more: <URL1>, <URL2>, <URL3>, <URL4>"; (c) **add an explicit search-budget hint** — append "If you have not found 4 verified stories by your 25th web_search call, write what you have and stop searching." to the leaf context. All three reduce the call count without changing the editorial quality bar. See `references/issue-12-2026-08-07-data-points.md` for the full recipe.
- **Gmail SMTP `ssl.SSL.write` timeout on large PDFs is intermittent and unrelated to script timeout.** `send_pdf_mail.py` defaults to `smtplib.SMTP.timeout=120`, but the SSL write inside `send_message()` uses a per-call timeout that is NOT raised with the socket timeout. STARTTLS + XOAUTH2 auth complete in ~5s, then `ssl.SSL.write` silently hangs at the 30s default per-call timeout, the connection drops, and `smtplib.SMTP.send_message` raises `SMTPServerDisconnected`. **Symptom:** the wrapper sits at 30-120s with no output before crashing. **Different from:** Issue-9 silent drop (no upload line, exit 0 — a Telegram-side issue), Issue-10 successful-but-slow (mail delivered in 26s). **Mitigation:** pre-flight STARTTLS to fail-fast on a known-flaky connection (5s timeout instead of letting `send_message` run for 120s), or compress the PDF with Ghostscript to <4 MB before mailing (`gs -sDEVICE=pdfwrite -dCompatibilityLevel=1.4 -dPDFSETTINGS=/ebook -dNOPAUSE -dBATCH -sOutputFile=compressed.pdf original.pdf`). Confirmed Issue 12: 16 MB PDF → 120s hang → `SMTPServerDisconnected`. See `references/issue-12-2026-08-07-data-points.md` for the full failure mode.
- **Reused card image across stories reads as a packaging bug.** If the leaf ran short of time and used the same `card_art` path for two different stories (e.g. an ABSeeker thumbnail on a Mistral Shieldstral story), the orchestrator must catch this BEFORE render. Audit `card_art` paths against story content — if the same path appears twice with different headlines, either (a) generate a dedicated image with `mmx image generate`, (b) accept text-only cards by setting `card_art: null` on the duplicate, or (c) download a fresh image from a primary source. The schema allows text-only cards; a missing image is more honest than a misleading one. Confirmed Issue 12: HF leaf reused ABSeeker thumbnail on Mistral card; orchestrator accepted without flagging.

- **Vision-verify every downloaded image at the orchestrator, even when the leaf says it's verified.** Pattern observed 2026-07-28: the AI leaf's "SSI × NVIDIA partnership" image downloaded as a 1600×900 JPEG (87 KB) and `file` passed, but a 10-second `vision_analyze` revealed it was just two brand logos on black — usable only as a section opener card, not a cover. The cover art needed to be regenerated. Always vision-check at the orchestrator before placing an image into the manifest.
- **F1 second-page hero: pair the lead race-hero with a pit-wall / qualifying portrait for the F1 follow-up page.** Hungary's qualifying-day `2287599294` Cloudinary ID (Andrea Stella at the pit wall) is a stable, well-composed 3392px JPEG on `trackside-images/2026/F1_Grand_Prix_of_Hungary___Qualifying/`. Free fallback, different crop, different vibe from the lead podium shot. Avoids the F1 lead-photo trap without fabricating a new download. Confirmed in v4 Issue 3 — paired with the lead `2287727846` Norris-podium JPEG.
- **Closing-page opinion must contain exactly 2 items.** The renderer renders the opinion component as a fixed 2-column grid; a 3rd item orphans to a second row and the closing layout breaks. Always emit exactly two opinion objects: one from the masthead editor (J. Jameson), one from a desk (Research Desk, The Messi Desk, The Builder, etc.). Sigs vary; count is fixed.
- **`wire_table_v2` is the only table component to use in 2026+ manifests.** The older `wire_table` (per-row `num` array) still parses but produces uglier HTML and lacks `num_col` control. Always use `wire_table_v2` with explicit `headers: [...]`, `rows: [[...]]` (list of list of strings), and `num_col: <index>` to right-align accent-deep numerics. Negative or missing `num_col` disables alignment. Confirmed in v4 Issue 3 P9 — Hungary classification.
- **Long orchestrator prompts waste tokens; put the editorial standards in the skill, not the prompt.** The 276-line orchestrator.md is the right size — every paragraph earns its place. But when a leaf needs the same rule, do not duplicate it: have the leaf load `news-correspondent` and `automated-editorial-briefings` and read those standards from there. Saves ~5-10% of the run's tokens per leaf. Pattern: the orchestrator points each leaf to the relevant skill in its context, not the orchestrator brief.
- **Pattern B orchestrator hanging silently with no output.** A full agent run can stall for 8+ minutes with no files written, no manifest, no spawned sub-agents, and no error. Likely cause: a slow tool call (mmx image generation, blocking web fetch) with no timeout, or the agent loop spinning on a malformed instruction. **Hardening checklist before force-run:** (1) explicit per-tool timeouts in the orchestrator prompt (e.g. `mmx image generate` capped at 60s, web fetches capped at 20s); (2) a hard `max_turns` cap on the agent loop (cron-wise: `agent.max_turns` in config.yaml, or `--max-turns` flag); (3) a partial-manifest heartbeat path the agent writes every iteration (e.g. `/tmp/manifest-PREFIX.json`) so you can see progress from outside the loop; (4) a "fast path" fallback in the brief — if the news is sparse OR the agent has been running >15 min, ship a 2-page paper instead of 4. Diagnose a stuck run by checking `~/.hermes/cache/delegation/live/` for any new subagent transcripts, and `~/.hermes/logs/gateway.log` for the most recent tool call.
- **PYTHONPATH pollution across venvs.** `~/.hermes/hermes-agent/venv` is on the system PYTHONPATH for the desktop app. Any new venv (e.g. `~/.hermes/scripts/hermes-times-v3/.venv`) that imports packages unavailable in the hermes-agent venv at runtime will see those packages but with broken C extensions (e.g. `from PIL import _imaging` fails when PIL is loaded from hermes-agent's 3.11 venv but the new venv is 3.13). **Fix:** always invoke the new venv's python with `env -u PYTHONPATH ~/.hermes/scripts/<name>/.venv/bin/python …`. Bake this into the cron wrapper script and the chart helper docs.
- **Hermes `cron create` positional-arg order.** The CLI is `hermes cron create [schedule] [prompt] [--name NAME] [--skill SKILL] [--deliver TARGET]`. There is no `--prompt` flag. The schedule and prompt are positional; flags come after. If a long prompt is given after the schedule, the shell parser will treat it as additional unrecognized flags and error like `unrecognized arguments: You are …`. Recipe: `hermes cron create "30 5 * * *" "<prompt>" --name "..." --skill ... --deliver local`. Pass the prompt as a single quoted string at position 2.
- **Cron YAML config corruption auto-recovery.** When `~/.hermes/config.yaml` is written with invalid YAML (e.g. wrong indent when inserting a personalities entry), Hermes detects the parse failure, copies the broken file to `~/.hermes/config.yaml.corrupt.<timestamp>.bak`, and recovers from the previous good backup. The current `config.yaml` is fine; the previous good content lives in the `.corrupt.<timestamp>.bak` file. **Idempotency check** before any config patch: `python3 -c "import yaml; yaml.safe_load(open('~/.hermes/config.yaml'))"`. **Recovery recipe:** if you've already corrupted the file, copy the most recent `.corrupt.<timestamp>.bak` over `config.yaml` and verify selectors with `hermes config get agent.personality`.
- **Personality preset as a single-line escaped string.** When a multi-line preset is inserted into `agent.personalities` via `hermes config set`, the CLI stores it as a single-line JSON-escaped string (`"\n"` literals) rather than a YAML block scalar. This parses identically but is ugly to read/edit. Acceptable for production; for cleaner storage, write the YAML directly via `hermes config edit` and use a block scalar (`jameson: |`).
- **Personality preset named but never invoked.** "Make the newspaper a personality" requires both the preset definition AND the cron entry actually loading that preset via `/personality <name>` or `agent.personality` in config. See `references/personality-as-publisher.md` for the wiring.
- **Asking "what's on it?" after the kid says "do the paper now."** The kid has said "no full paper", "ship me a preview", "write me an edition", or "do the paper now" — those are *all* the same instruction: file a complete edition. Don't run an intake loop ("what topic?", "what desks?", "what's the visual?"). Survey the desk (recent commits, vault mtimes, recent pushes via `gh`, today's notebook entries), pick the lead from what actually has evidence, build the four pages, ship a preview. The kid's edit comes after the file is on disk; that's what a preview is for. If the desk is genuinely empty, *say so and ship a one-page "nothing-burger"* — that's a real edition, not a failure. Asking is the failure.
- **404 repos in the source pack, fabricated into the run.** `gh api user/repos` may return `Not Found` for repos that are private, deleted, or partially typed. **Never** fill a briefing slot with a guess. Spike the row, log it under "404s · spiked", name the four-or-five affected repos verbatim, and move on. A paper with a one-line "Repo the wire missed" panel is more honest than a paper with a faked MIRA / Iris / earth_guardian_ai blurb.
- **Preview edition vs. real edition — the kid wants a draft to vet before committing to a schedule.** When the kid says "issue it now so I can check / before you put it in tomorrow morning / I want to read it first," they are asking for the **preview edition** pattern: build the full layout, file it to a `/HermesTimes/<DATE>-v1/ed.html` archive, hold the presses (footer reads "Presses: HOLDING pending kid's read"), and stop. Do not also register a cron job. Do not also send via Telegram. Do not also generate any final-Run QA pass. The preview is a read-through artifact, not the morning paper. Once the kid edits / approves, then — and only then — the real edition runs. See `templates/preview-edition-prompt.md`.
- **Pre-render image-existence check is mandatory; `render.py` will throw on the first missing path.** Walk every `cover.art_uri`, `sections[*].art_uri`, and `sections[*].bodies[*].card_art` in the manifest before invoking `render.py`:
    ```bash
    python3 -c "
    import json, os
    m = json.load(open('<manifest>.json'))
    paths = [m['cover']['art_uri']] if m.get('cover') else []
    for s in m.get('sections', []):
        if s.get('art_uri'): paths.append(s['art_uri'])
        for b in s.get('bodies', []):
            if b.get('type') == 'split':
                for side in [b.get('left', {}), b.get('right', {})]:
                    if side.get('card_art'): paths.append(side['card_art'])
            if b.get('type') == 'cards':
                for it in b.get('items', []):
                    if it.get('card_art'): paths.append(it['card_art'])
    missing = [p for p in paths if not p or not os.path.exists(p)]
    print('Missing:', missing if missing else 'NONE')
    "
    ```
    If `Missing:` is non-empty, the manifest is broken *before* render. `render.py:img()` raises `FileNotFoundError` and the whole press run dies — much harder to debug than checking 5 seconds before press. Same script doubles as a duplicate-image detector:
    ```python
    from collections import Counter; dupes = {p:c for p,c in Counter(paths).items() if c > 1}
    ```
    If a path shows up twice but refers to two *different* story ranks, the leaf reused a downloaded photo for two stories — fix it (drop one `card_art`, or download a second image) before render. `kind: "generated"` paths that are *not* newer than today's manifest draft are stale (left over from prior runs) — drop or regenerate. See `references/leaf-beat-reporter-workflow.md` for the corresponding leaf-side pitfalls.

- **`hermes send` silently drops the upload and reports success.** `hermes send --to telegram -q "… MEDIA:/path/issue.pdf"` returns exit 0, the cron DB records `last_status: completed, error: None`, but `~/.hermes/logs/gateway.log` shows zero outbound Telegram activity for the send window (no `sendDocument`, `chat_action`, `uploadDocument`, or `Sending ... MEDIA:` line). The Telegram bot is alive on the wire but the adapter path drops the upload. The cron-level "delivered" signal lies — the user never sees the paper. **Detection:** snapshot the gateway log line count before the send, wait ~5s, tail from the snapshot for any line containing the PDF basename or the standard send verbs; if nothing appears, the upload was silently dropped (the `send.sh` script at `~/.hermes/scripts/hermes-times-v4/send.sh` does this and exits 2 on silent drop). **Recovery:** do NOT retry `hermes send` — it may queue indefinitely without flushing. Pivot straight to the Gmail SMTP + GOA XOAUTH2 fallback (`templates/send_pdf_mail.py`); use `smtplib.SMTP.timeout=120` for PDFs > 4 MB. **Reporting honesty:** when the Telegram path drops, say so explicitly in the cron closing note — the PDF went to email as a fallback, Telegram delivery did NOT happen. See `references/hermes-send-media.md` "When `hermes send` silently drops the message" for the full recipe and gotchas.
- **`send_all.sh` reports per-channel results separately — read the summary, not just the exit code.** `send_all.sh` walks every `recipients.conf` line, dispatches each channel, aggregates the results, and prints a fan-out summary table (one row per channel with OK/FAIL + reason). It exits non-zero if **any** channel failed. Confirmed v4 Issue 8 (2026-08-03): Telegram returned `FAIL (send.sh exit 2 — see gateway.log)` because no gateway upload line appeared, but Gmail returned `OK · 3030950 bytes delivered to reachnancysharma@gmail.com`. The orchestrator's closing note must report this honestly — do not say "delivered" when one channel failed; do not claim Telegram when only the mail channel succeeded. The wrapper's exit code alone tells you nothing about which channel worked; the printed summary is the only ground truth.

- **Multi-recipient fan-out via `send_all.sh`.** Press-run senders live at `~/.hermes/scripts/hermes-times-v4/`. The subscriber list is in `recipients.conf` (one `channel:target` per line; channels: `tg:` and `mail:`). `send_all.sh <pdf> [<caption>]` walks the list and dispatches to every channel, aggregating per-channel results and exiting non-zero if any failed. Cron uses `send_all.sh`, not `send.sh` directly — add/remove a subscriber by editing `recipients.conf`, not the orchestrator brief. To pause a recipient temporarily without deleting their line, comment it out (`# mail:...`).
- **Gmail SMTP upload times out on PDFs > 4 MB at the default `smtplib.SMTP.timeout`.** The `send_pdf_mail.py` script defaults to `timeout=120` for the SMTP socket, but the SSL write inside `send_message()` uses a per-call timeout that is not raised with the socket timeout. **Symptom:** 19 MB PDF: STARTTLS + XOAUTH2 auth complete in ~5s, then `send_message` silently times out at 30s and the connection drops with `SMTPServerDisconnected: Server not connected`. **Fix before mailing the PDF:** compress with Ghostscript to < 4 MB:
  ```bash
  gs -sDEVICE=pdfwrite -dCompatibilityLevel=1.4 -dPDFSETTINGS=/ebook \
     -dNOPAUSE -dBATCH -sOutputFile=compressed.pdf original.pdf
  # 19 MB → ~2 MB. Replace original, verify with pdfinfo (page count must not change).
  ```
  The /ebook preset balances quality vs size for an 8-page A4 broadsheet. Confirmed v4 Issue 9 (2026-08-04): 19 MB → 1.96 MB, 8 pages preserved, mail delivered in 26s.
- **Cron-mode Telegram auto-delivery skips `hermes send` with the target that triggered the cron.** When the cron job's prompt runs in this same session and the deliver target is `telegram:<aditya's chat_id>`, the gateway detects "the cron job will already auto-deliver its final response to that same target" and refuses the explicit `hermes send` call with `cron_auto_delivery_duplicate_target`. The final assistant response IS the Telegram delivery — `send_all.sh` will print `tg: FAIL (send.sh exit 2 — see gateway.log)` in this case, but that "FAIL" is the *correct* path: the cron runner will inline this final response (including any `MEDIA:<pdf>` syntax) as the Telegram message to the user. **Closing-note rule:** when the gateway skips Telegram, do NOT say "Telegram failed", do NOT retry `hermes send`, do NOT flag it as a delivery failure in the summary. State explicitly: "Telegram: delivered via this final response (cron auto-delivery). Mail: delivered to <address>." The mail channel still goes through `send_all.sh` normally and is the only one that should be reported via the send-all summary table. Confirmed v4 Issue 9 (mail-only delivery) and v4 Issue 11 (Telegram auto-skip was the entire Telegram path).
- **`hermes send` shell helper can take 25-30s before exiting when the gateway rejects the upload.** Don't wrap `send_all.sh` in a 60s timeout if the script tries both Telegram and mail channels — the Telegram stage can burn 30s of the timeout before the mail stage even starts. Use a 90-120s outer timeout and let the wrapper finish. Confirmed v4 Issue 9: a 60s outer timeout killed the run before the mail stage began.
- **Adding a real human to a daily cron broadcast.** When wiring a new email recipient into `recipients.conf` that fires every morning from cron, **always ask the user to pick the auto-send posture** (always-on vs opt-in flag vs per-run `--yes`) before writing the line. Real humans don't unsubscribe from emails they didn't subscribe to — they just ignore or filter them. Defaults to recommend: opt-in via env-flag (`HERMES_TIMES_INCLUDE_NANCY=1`) so the infrastructure is wired but no email fires until the user explicitly flips the switch. The fan-out wrapper should respect that flag. Always-on is OK if the user has explicitly subscribed the person (e.g. "add Nancy to the paper list"); never assume consent. If the user says "add X to the list" without specifying cadence/posture, ask — don't auto-default to always-on for a third party.

## Final report

Lead with the artifact and schedule. Include:

- issue PNG/PDF paths or attachments;
- schedule and next run;
- what was replaced;
- real verification facts (cron status, page count, delivery result);
- one concise caveat only if unresolved.