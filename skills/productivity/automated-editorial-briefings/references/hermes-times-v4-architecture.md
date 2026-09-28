# THE HERMES TIMES v4 — Working Pattern B Architecture

**Status:** Production since 2026-07-26. Scheduled at 06:00 IST daily.
Replaces the v3 agent-driven architecture (which hung silently on its first
force-run — see the v3-architecture reference for the autopsy).

This is the canonical worked example of Pattern B (agent-driven editorial
mode) for a personal daily newspaper. Use it as the template when the
user says "build me a daily paper / newsletter / morning briefing."

## Why v4, not v3

v3 failed because:
- Single monolithic agent doing everything → no parallelism, easy to stall.
- No per-tool timeouts → mmx image generation could hang the whole loop.
- No manifest schema → bespoke HTML per page, renderer not reusable.
- No continuity contract → agent re-decided structure from scratch each day.
- deliver=local but the script also sent → duplicate delivery potential.

v4 fixes all five: **5 parallel beat reporters** (added Hugging Face
in the v2 build — see Issue 2 data points below), explicit timeouts,
manifest schema first, yesterday's-archive read at start,
deliver=telegram with agent-owned send.

## Page-count target

Default target: **10 pages per edition** (cover + 9 interior). That
covers a full house: lead + 1-2 intros per beat + a The Wire data
page + a Closing weather/tomorrow/opinion page. The 11 new style
components (`stat_block`, `rank_list`, `wire_table_v2`, `mini_chart`,
`score_line`, `opinion`, `weather`, `tomorrow_list`, `paper_grid`,
`editorial_aside`, attributed `quote`) are documented in
`references/style-component-library.md`. Drop a beat's continuation
page if it has nothing real — a 7-page honest paper beats a 10-page
filler.

## The 5-beat pattern

For a personal newspaper where the user has fixed interests (this user:
AI/ML, research, Hugging Face, Messi, F1), **fix the beats**. Don't
let the agent invent beats every day — that produces inconsistent feel.
Beats become the spine; the agent decides section structure, lead
story, and depth per issue.

Hugging Face was added as a 5th desk in Issue 2 (2026-07-26). It is
its own beat, not a Research appendage: trending papers, trending
models, dataset drops, Spaces launches, Hub feature releases, OSS
model releases by major labs. HF paper thumbnails are usable as
`card_art` but not as section heroes (text-heavy).

Each beat gets:
- A fixed prompt template (see `templates/beat-briefs.md` below)
- A JSON output contract (ranked stories, source URLs, image metadata)
- A primary-source allowlist (Getty, Reuters, team press, arxiv.org,
  huggingface.co)
- A time budget (10-15 min per beat)

Spawn all 5 in one `delegate_task` batch (parallel). Each returns a
JSON package, not prose. Wall-clock is the slowest leaf, typically
~7-10 min for a 5-beat run.

## Hybrid image strategy

This is the design choice that matters most for a news-driven paper:

| Use case | Strategy | Why |
| --- | --- | --- |
| Messi match photo | **Download** real Getty/MLS photo via curl | Generated images get faces wrong, fail at jersey numbers, don't match the moment |
| F1 car/track photo | **Download** real Getty/F1 official photo | Same — racing moments are too specific for image-01 |
| AI lab announcement | **Download** if available, else generate | Real lab photos exist; fall back if no canonical image |
| arXiv paper hero | **Download** paper figure from arXiv HTML or repo README | Paper figures ARE the data; never synthesize |
| Pull-quote art | **Generate** with mmx image-01 | Decorative, no factual content to get wrong |
| Section opener mood shot | **Generate** with mmx image-01 | Atmospheric, not evidence |
| Cover art (no real photo) | **Generate** with mmx image-01 | Acceptable when a real photo isn't available |

Rule: every downloaded image records its `art_source_url` and `art_credit`
in the manifest. Generated images carry the prompt for reproducibility.

The renderer (`render.py`) validates every `art_uri` exists on disk before
producing HTML. A missing image is a render error, not a partial output.

## Cron entry — the working recipe

```bash
hermes cron create \
  "0 6 * * *" \
  "<orchestrator prompt, see templates/orchestrator-prompt.md>" \
  --name "hermes-times-v4" \
  --skill hermes-personalities \
  --skill iterative-image-generation \
  --deliver telegram \
  --workdir /home/arctic/.hermes/scripts/hermes-times-v4
```

Key choices:
- **`deliver=telegram`**: the orchestrator's prompt ends with the PDF send
  via `hermes send`, but the scheduler also pushes a status ping so the
  user knows if the run failed. Not duplicate delivery — the agent's send
  is the PDF; the scheduler's delivery is the cron output (agent's
  reply/log).
- **`workdir=...`**: cron sessions start in this dir so relative paths in
  the prompt work without a `cd`. Beats the PYTHONPATH pollution pattern.
- **Two skills, not four**: `hermes-personalities` (for the J. Jameson
  voice wiring) and `iterative-image-generation` (for mmx iteration
  discipline). Don't preload more — it dilutes prompt attention.

## Wall-clock budget

30 minutes for an end-to-end run. Realistic phase split:

| Phase | Time | What |
| --- | --- | --- |
| Continuity read + structure decision | 2 min | Orchestrator reads yesterday's issue, picks lead, picks beats |
| Parallel beat reporting | 5-12 min | 4 leaf sub-agents in one batch; wall-clock is the slowest one |
| Image downloads/generation | concurrent with reporting | Beat reporters handle downloads; cover/section art may need extra mmx call |
| Editor composition | 5 min | Orchestrator writes manifest, validates art paths |
| Render + vision-QA | 2-3 min | render.py + pdftoppm + vision_analyze on page 1 |
| Archive + send | 1 min | Copy manifest to issues/<date>.json, send.sh to Telegram |

A run that uses 18-22 minutes is healthy. Under 12 minutes means the
agent cut corners; over 28 minutes means it's stuck — kill and inspect.

## Orchestrator prompt structure

The brief lives in `prompts/orchestrator.md` (checked into the script
dir). The cron entry's prompt is a one-liner that points at the file
plus the date substitution.

Structure of the brief:

1. **Identity block** — who the agent is, who reads the paper, the voice.
2. **Editorial standards** — hard rules (sources, no invented facts,
   image citation hygiene).
3. **Today's steps** — 8 explicit phases with time budgets.
4. **Beat reporter contract** — JSON shape, image strategy per beat.
5. **Pitfalls / never-do list** — embedded inline so it's hard to skip.
6. **Tooling notes** — which tools for which job, no config touch.

The brief is ~9 KB. That's deliberate — long enough to encode standards,
short enough that the agent reads it once and acts.

## What v4 still doesn't solve

- **mmx image generate filename collision**: every call writes
  `image_001.jpg` in the output dir, overwriting the previous one. If you
  need multiple images in one script, rename between calls (the v4
  build script does this manually). Or call with `--out-dir` per image
  to different subdirs. See `iterative-image-generation` pitfall.
- **Cron wrapper inherits PYTHONPATH pollution**: solved in v3 with
  `env -u PYTHONPATH`; v4 doesn't use a venv at all (pure stdlib
  renderer), so the issue doesn't arise. Worth knowing if you add a
  chart helper that needs matplotlib.
- **Manifest versioning**: v4 has no schema version field. If you evolve
  the manifest, the renderer silently breaks on old issues. Add a
  `schema_version` field the next time around.

## Files (canonical paths)

```
~/.hermes/scripts/hermes-times-v4/
├── prompts/orchestrator.md          # the editor's brief (~9 KB)
├── prompts/beat_briefs.md           # instructions for the 4 reporters
├── render.py                        # manifest → HTML → PDF (stdlib only)
├── style.css                        # broadsheet component library
├── cron_jameson.py                  # 30-min wrapper (alternative to cron prompt)
├── send.sh                          # hermes send + MEDIA: syntax
├── MANIFEST_SCHEMA.md               # the orchestrator↔renderer contract
└── fixtures/smoke.json              # verified end-to-end fixture

~/.hermes/data/hermes-times-v4/
├── assets/                          # all images land here
├── issues/<date>.{html,pdf,png}     # rendered artifacts
└── issues/<date>.json               # archived manifest (continuity source)
```

## Verification checklist before declaring v4 done

- [x] Renderer smoke test: fixture → 5-page PDF, ~2.4 MB
- [x] Page 1 vision-QA: cover art visible, masthead legible, layout clean
- [x] Interior vision-QA: hero image embedded, cards laid out cleanly
- [x] Telegram target resolved: `telegram:Aditya Sasidhar [1462067171]`
- [x] send.sh syntax verified: `MEDIA:<path>` in message body, `--to telegram`
- [x] End-to-end agent force-run — **complete (2026-07-26 21:55 IST,
      Issue 1)**. 6-page PDF, 17.3 MB, 4 beat reporters in parallel,
      17 ranked stories (AI:5, Research:5, Messi:3, F1:4), 18
      downloaded images all > 70 KB, all primary-sourced, all
      art_source_url + art_credit recorded. Vision-QA pass on cover
      and 4 interior pages. PDF delivered to Telegram via send.sh.

## First-run data points (Issue 1, 2026-07-26) — superseded by Issue 2 below

| Beat | Stories | Real photos downloaded | Notes |
| --- | --- | --- | --- |
| AI Industry | 5 | 5 | AMD Helios (lead), Opus 5, AMD↔Anthropic split, FLUX 3, Anduril |
| Research | 5 | 5 (HF paper thumbnails) | SkillOpt paper-of-week, Xiaomi-Robotics-1, AlayaWorld, Mage-Flow, Unlimited OCR |
| Messi | 3 | 1 (lead only) | World Cup final loss, Montreal match, Casemiro signing |
| F1 | 4 | 1 (lead reused for section hero) | Hungary pole lead, Russell fix, Hamilton penalty, Spa debrief |

Pitfalls observed and resolved on this run — load-bearing for the next agent run:

- **Smoke-fixture contamination on day-1 launch.** On the first cron
  run, `~/.hermes/data/hermes-times-v4/issues/2026-07-26.json` exists
  because it was written during the renderer's smoke test, and the
  byte-for-byte content is identical to `fixtures/smoke.json`. If
  today's run tries to read that file as "yesterday's issue" for
  continuity, it will read smoke data and produce a worse lead
  choice. **Detection recipe** (run before reading for continuity):

  ```bash
  diff -q ~/.hermes/data/hermes-times-v4/issues/<yesterday>.json \
          ~/.hermes/scripts/hermes-times-v4/fixtures/smoke.json
  # If identical → smoke contamination, skip the continuity read.
  ```

  Tomorrow's run will read the new `2026-07-26.json` (real content,
  17.5 KB) and this won't matter again.

- **`send.sh` already appends `MEDIA:<pdf>`.** Looking at `send.sh`:
  the caption is concatenated with `\n\nMEDIA:${PDF}` unconditionally.
  If the orchestrator's caption ALSO contains `MEDIA:/path/...`, the
  message body reaches `hermes send` with two `MEDIA:` lines and
  Telegram's parser keeps one (usually the last) — works but ugly.
  **Rule:** the orchestrator's caption string must NOT contain
  `MEDIA:`. send.sh owns that token.

- **F1 leaf used one downloaded photo for all 4 stories.** This is
  acceptable for the section hero (re-use is fine — `art_uri` is one
  field per section, not per story) but is wrong if the orchestrator
  reuses the same path for every story's `card_art`. The
  orchestrator must set `card_art` only on the lead story of a beat
  that ships a downloaded hero, and leave subsequent cards text-only.

- **Anduril card-art overlapped the source/date eyebrow.** The AI
  Industry page rendered with the soldier-with-drone photo bleeding
  over the "REUTERS · JUL 24" line. Not breaking, but ugly.
  **Cause:** `card_art` is rendered at a fixed 28mm height with no
  margin-top on the eyebrow block when the photo aspect ratio is very
  wide (landscape soldier + drone shot). Orchestrator options: (a)
  crop the source image before download to a 4:3 or 1:1, (b) drop
  `card_art` for that story and let the card be text-only, (c) accept
  the visual seam and ship. For day-1 shipping, (c) is fine.

- **HF paper thumbnails are text-heavy as section heroes.** The
  Research beat used
  `cdn-thumbnails.huggingface.co/social-thumbnails/papers/<id>.png`
  for both card art and the Xiaomi section hero. They show the paper
  title page — readable, accurate, but visually dense. **Use as
  card_art, not section hero.** For a hero, prefer a paper's own
  figure (extract from arXiv HTML) or a generated conceptual
  illustration.

## Issue 2 data points (2026-07-26, 10-page edition)

After shipping Issue 1 as a 6-page preview earlier in the day, the
user asked for "more content, up to 10 pages, and news on Hugging
Face." Issue 2 (16:18 IST, 10-page PDF, 9.99 MB) validates the
expanded architecture.

| Beat | Stories | Real photos downloaded | Notes |
| --- | --- | --- | --- |
| AI Industry | 5 | 5 | AMD ↔ Anthropic $5B / 2GW lead, Opus 5, Etched $10.3B, OpenAI/HF cyber chain, WH vs Moonshot |
| Research | 5 | 5 (HF paper thumbnails) | SANA-Video 2.0 hybrid linear+softmax lead, OpenForgeRL, emergent-misalignment solo paper, WorkBuddy Bench, DONDO African ASR |
| Hugging Face | 5 | 4 (solar-open2, laguna, pocket, robostral) | Solar Open 2 250B-A15B MoE lead, Laguna-S-2.1 SWE-bench top, POCKET 35B CPU llama.cpp, AMD MI455X 99.5% pass-rate, Mistral Robostral Navigate |
| Messi | 3 | 1 (lead hero only) | Miami 1-0 Montreal sixth straight (Suárez 81' pen), Paredes farewell speculation, MLS All-Star exemption under WC rest agreement |
| F1 | 4 | 1 (lead hero) | Norris wins Hungary after Piastri gearbox, qualifying pole, Aston 16-update package, Sepang replaces Bahrain |

Pitfalls observed and resolved on this run — load-bearing for the
next agent run:

- **Leaf transcript truncation** (`...(+N chars)` marker on final
  assistant message). Two of five leaves (Messi, F1) lost their
  body text in the visible log. The full JSON is preserved at
  `~/.hermes/cache/delegation/subagent-summary-<N>-<ts>.txt`. Do
  not re-spawn the leaf — read the summary file, renormalize the
  `image` block to `{ kind, local_path, source_url, credit }`, and
  write to `~/.hermes/data/hermes-times-v4/<beat>.json` for the
  orchestrator to pick up. ~1-2 minutes vs. a full re-spawn. See
  `references/leaf-transcript-recovery.md`.

- **Render clobbers yesterday's artifacts.** `render.py` writes
  `issues/<date>.{html,pdf,png}` unconditionally. A v2 render for
  the same date wiped v1's PDF/PNG (continuity JSON at
  `manifests/<date>.json` survived). Snapshot or version-rotate
  the output dir before re-rendering. See
  `references/render-artifact-overwrite.md`.

- **Hugging Face thumbnails are usable as `card_art`, not as section
  heroes.** The HF Solar Open 2 hero was a generated model card
  image (text-free, gradient backdrop). Falling back to a
  downloaded HF Hub social card as a section hero reads as
  marketing collateral — keep section heroes photographic or
  figure-derived.

- **`mini_chart` pct is relative width, not the value.** Use `pct:
  95` to mean "bar fills 95% of the container"; the actual number
  goes in the `value` field next to the label. The Anthropic
  compute-stack chart on P9 uses this.

- **`stat_block` overflow at >6 chars.** A stat like "$4.2B +62%"
  overflows the accent-deep number slot. Truncate to 6-8 chars; longer
  numeric series go in a `wire_table_v2` row.

- **Cards on Hugging Face page clipped until font was tightened.**
  Original CSS `font-size: 10px` and `card-art height: 28mm` made
  the bottom-card body text overlap the date eyebrow on P5. Fixed
  to `font-size: 9.5px`, `min-width: 0; overflow: hidden` on
  `.card`, and `card-art height: 24mm`. Cards now respect ~220-char
  body limit.

- **Opinion column expects exactly 2 items.** It is a 2-column
  grid by design. A third opinion renders as an orphan.

- **Wire-table `num_col` is the column index to right-align in
  accent deep**, not a column count. Pass `-1` or omit to disable.