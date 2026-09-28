# Issue 12 — 2026-08-07 data points

Issue 12 (Thursday morning edition) is the **third consecutive run where a leaf reporter hit a tool-cap failure mode** — and the second consecutive where it was the AI Industry leaf with `loop_web_search_cap`. The orchestrator recovery playbook held cleanly, but the pattern is now strong enough that it should shape how the AI Industry leaf brief is written going forward.

## What happened (verified facts from this run)

- **AI Industry leaf** was spawned at 06:01 with a full 6-story target (EU AI Act, Qwen3.8-Max, Anthropic-Volta follow-up, IndiaAI compute, Meta Muse Spark, Nvidia Vera Rubin). Hit `loop_web_search_cap` after **~50 web_search calls** (`exit_reason: "completed"` — not `max_iterations`, but the cap-message-style stop). The leaf DID successfully generate 7 mmx cover-style images before timing out (`ai_eu_ai_act_enforcement.jpg`, `ai_anthropic_volta_lead.webp`, `ai_india_sarvam_ibm.jpg`, `ai_qwen_3_8_max.jpg`, `ai_volta_bitdeer_tydal.jpg`, `ai_meta_muse_spark.webp`, `ai_nvidia_chip.jpg`) in `2026-08-07/`. Partial transcript in `~/.hermes/cache/delegation/live/deleg_4f28882d/task-0.log` named all 6 story candidates with primary-source URLs the leaf had discovered.
- **Research, Hugging Face, Messi leaves** all completed cleanly in this run.
- **F1 leaf** was spawned in a **second delegation batch** after the first batch timed out on AI Industry. Returned clean JSON in ~70s. Pattern validated: 2-batch dispatch beats waiting on a slow leaf.
- **Orchestrator recovery**: read the partial transcript, extracted the 6 story candidates, verified each against its primary source via `web_extract`, composed the beat JSON orchestrator-side, written to `/tmp/ai_beat_2026-08-07.json` then `/home/arctic/.hermes/data/hermes-times-v4/manifests/2026-08-07.json`.
- **7-page PDF rendered cleanly** (16 MB), vision-QA'd on page 1, archive + send_all attempted.
- **Telegram**: documented cron auto-skip path (the final response IS the delivery; `send.sh` exit 2 with "skipped duplicate target" — do not retry, do not flag as failure).
- **Gmail SMTP**: failed at the `ssl.SSL.write` step after XOAUTH2 auth succeeded — `TimeoutError: The write operation timed out` after 120s, then `SMTPServerDisconnected`. PDF on disk for manual recovery. This is **different from** the Issue-9 silent-drop and the Issue-10 successful-but-slow cases — Gmail's STARTTLS+write path is intermittently flaky on large PDFs (16 MB here vs 4 MB in Issue 9 vs 1.96 MB compressed in Issue 9).

## Recovery recipe (refined from Issue 11, validated again here)

1. **Don't re-spawn the leaf.** It will hit the same cap.
2. **Read the partial transcript** at `~/.hermes/cache/delegation/live/deleg_*/task-N.log`. Look for the assistant's turn-by-turn thinking blocks — they name story candidates with URLs the leaf actually verified via web_search.
3. **Extract candidates**: for each story the leaf named, the leaf has usually done at least one web_extract on its source URL. The URLs are real, the headlines are usually close, but the bodies may be aspirational — verify before pasting.
4. **Re-verify each candidate's primary source** with `web_extract` (URL → content). Confirm: (a) headline matches the source's actual headline, (b) facts/quotes/figures match, (c) the source is genuinely primary (not a roundup), (d) timestamp is in the last 72 hours.
5. **Compose the beat JSON orchestrator-side** — write to `/tmp/<beat>_beat_<date>.json` first, then promote to the manifest path after validation.
6. **Trust the leaf's already-downloaded assets** — they're on disk in `assets/<date>/`, vision-verify them, and they're ready for the manifest.
7. **Document the recovery in the orchestrator's closing note** so the next editor knows it was orchestrator-recovered (the JSON shape stays normal).

## New observation: AI Industry leaf is the repeat offender

Across Issues 10, 11, 12, the AI Industry leaf has been the failure-prone one in **every single case** — once `max_iterations`, twice `loop_web_search_cap`. Research, HF, Messi, and F1 leaves have each completed cleanly across these three runs.

**Why**: AI Industry is the broadest beat. 6 stories × ~3-5 web_search calls per story × 2 verification rounds = 36-60 calls. Add in image downloads and vision-checks and you blow through the cap. Other beats have narrower scopes:
- Research: arXiv listings + 2-3 cross-references per paper
- HF: model cards + a couple of comparisons
- Messi: 1-2 match reports + team announcement
- F1: post-race interviews + driver market

**Action for the next orchestrator prompt**: when writing the AI Industry leaf brief, set an explicit search budget. Either:
- **Lower the story count**: 4 stories instead of 6 — same beat, less search load.
- **Pre-supply source URLs**: tell the leaf "verify these 4 URLs first before searching for more: <URL1>, <URL2>, <URL3>, <URL4>." — caps the search space at ~4 calls per story × 4 stories = ~16 calls total.
- **Add a search-budget hint**: in the leaf context, append "If you have not found 4 verified stories by your 25th web_search call, write what you have and stop searching."

Don't try all three — pick one per run based on what the orchestrator knows about the day's news density.

## Two-batch dispatch pattern (validated here, worth encoding)

When the first batch of leaves is too wide, **split into 2 batches of 2-3 leaves each**. The orchestrator can dispatch batch 2 once batch 1's results are in hand (sequentially, but in less wall clock than waiting for a single slow leaf to finish). In Issue 12:
- Batch 1: AI Industry, Research, HF, Messi (parallel).
- AI Industry timed out (~540s with no JSON).
- Batch 2: F1 (alone, dispatched after batch 1 completed).
- Orchestrator then recovered AI Industry from the partial transcript while F1 was running.

Total wall clock: ~10 min, well under the 30-min budget. If F1 had been in batch 1, the orchestrator would have waited 540s for AI Industry before getting F1's clean result. Two-batch dispatch is faster AND more reliable.

## What worked (carry forward)

- The orchestrator's parallel leaf dispatch worked: 4 leaves completed in ~70-200s each, all returning valid JSON. No serial bottleneck.
- The `mmx image generate` calls for F1 card illustrations worked first try (Mercedes, Cadillac, Williams). Used safe prompts that explicitly forbade logos/jersey text — no fake text hallucination this time.
- Vision-QA on the page-1 PNG caught nothing wrong (paper rendered clean).
- WebP vs JPEG suffix detection worked: the orchestrator renamed `.webp`-bytes-but-`.jpg`-suffix files (F1 Hungary photo, Anthropic Volta, Meta Muse Spark) before render. Image verification loop held.

## What didn't work (one note for next time)

- The HF leaf's rank-5 image (`hf_abseeker.png`) was re-used for the Mistral Shieldstral story because the leaf ran out of time to grab a separate Mistral card. **Visual honesty**: the ABSeeker thumbnail showing on the Mistral story is misleading. Future orchestrator should audit `card_art` paths against story content before render — if the same thumbnail is reused for two different stories, the leaf ran short and the orchestrator should either generate a dedicated image or accept text-only cards.
- The orchestrator did not detect the Gmail SMTP failure mode in advance. The script does not pre-check Gmail's SMTP server responsiveness; it just attempts the send and times out after 120s. Could add a 5-second SMTP-STARTTLS preflight to send_all.sh to fail-fast on a known-flaky connection and skip the mail channel entirely (so the user gets a clear "Gmail SMTP unavailable, PDF on disk for manual send" instead of a 120s hang + silent fail).

## Data points for next issue's continuity

Issue 13 (2026-08-08) lead candidates:
- **Inter Miami vs CF Monterrey** (8 PM ET Friday = 5 AM IST Saturday). Messi will play. If Miami win, the Suárez-ban-depth story gets a clean coda.
- **Zandvoort FP1** opens if F1's summer break ends (Aug 22).
- **EU AI Office formal guidance on GPAI copyright opt-outs** is the next regulatory drumbeat.

Issue 12 cover: Norris + Piastri on Hungaroring podium. Tomorrow's cover is the Inter Miami-Monterrey result if they win, or the Anthropic-Bitdeer Tydal build-progress story if the AI desk finds a real update.
