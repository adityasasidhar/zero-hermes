# Issue 11 — 2026-08-06 — Data points

Edition 11 of THE HERMES TIMES v4 (Pattern B, 6 pages, delivered on Telegram + Gmail). Five beats, ten stories, manifest at `~/.hermes/data/hermes-times-v4/manifests/2026-08-06.json`.

This issue surfaced two new failure modes worth encoding:

## AI Industry leaf hit `loop_web_search_cap`, not `max_iterations`

Different guardrail from Issue 10. The AI Industry leaf burned through **~50 `web_search` calls without producing a JSON output**, hit a `loop_web_search_cap` (a per-tool-call retry guard, not the agent's `max_iterations`), and exited cleanly with `exit_reason: "completed"` and a final summary that *recommended* changing strategy. Crucially:

- The leaf never wrote a JSON to any on-disk path (`ai_beat_2026-08-06.json` did not exist after the run).
- The leaf DID successfully generate **5 mmx image-01 cover-style images** for the stories it had identified (Anthropic-Volta, Apple-OpenAI, SpaceX earnings, Palantir Q2, OpenAI Education) — those assets landed on disk at the leaf's `ai_anthropic_volta_001.jpg` … `ai_whitehouse_001.jpg` paths and were usable.
- The leaf's transcript (in `~/.hermes/cache/delegation/live/deleg_<id>/task-0.log`) contained every story it had identified with its primary-source URL, headline, and a short draft body — but no JSON envelope.

**Recovery recipe for orchestrator (verified this issue):**

1. Do NOT re-spawn the leaf. The same 50-call loop will repeat.
2. Read the leaf's partial transcript (`tail -300 ~/.hermes/cache/delegation/live/deleg_*/task-0.log`) and extract the **story candidates** — for each, the URL, source name, headline, and dek. The transcript's `think |` and `assistant |` lines hold the editorial reasoning.
3. Verify each candidate against its primary source via `web_extract` before pasting into the orchestrator's beat JSON. The leaf's wording may be aspirational — the source has to actually contain the claim.
4. Compose the beat JSON orchestrator-side using the leaf's identified URLs as the `source_url` and the leaf's assets for the `image` field. Use the leaf's draft body as a starting paragraph and re-write each one as the orchestrator's own prose.
5. Spot-check that the leaf's named-theorem / technical claims are actually in the primary source. The Issue-10 Connes-embedding-conjecture fabrication pattern can repeat across leaves — the AI Industry leaf is the highest-risk beat for this.

This is a fifth variation of the leaf-output-path pitfall: the leaf produced assets, found the right stories, but never wrote JSON. **Different recovery than the Issue-10 case** (where the leaf DID return JSON in its summary). Future-work hint: pass a `--max-tool-calls 35` constraint to all leaves, and add an explicit "write your JSON to <path> after your 5th web_search" instruction in the AI Industry brief specifically — it's the most search-heavy beat.

## mmx image-01 sports-kit hallucination (fake sponsors + wrong sport)

This was a clean three-attempt pattern. The first generation for a "Messi celebration" cover put a **fake Pirelli sponsor wordmark** across the chest of the jersey (a Pirelli patch doesn't exist on Inter Miami kits; Pirelli is an F1 sponsor). The vision-check caught it because the editorial standard is "no logos, no fake text." Second attempt pivoted to a silhouette prompt ("viewed from behind") but the model rendered an **American football helmet + shoulder pads** instead of a bareheaded soccer player — the model's prior for "football player" defaults to NFL. Third attempt, with explicit "bareheaded male football player (NOT American football, no helmet)" and "jersey has no visible logos or text", produced a clean bareheaded silhouette against floodlights with the figure's hands raised. **8/10 as a cover hero** — usable, no fake text, no kit hallucinations.

**Safe-prompt pattern for sports/people editorial covers (now in `iterative-image-generation`):**

> "Editorial silhouette photograph of a bareheaded male football player viewed from behind with both fists raised in celebration at night, soccer stadium floodlights creating dramatic god rays, pink-orange dusk Miami sky, palm trees at the stadium edges, blurred crowd, jersey has no visible logos or text, pure silhouette against the lit sky, photojournalism aesthetic, 16:9 aspect ratio"

Key requirements: **silhouette** (kills logo hallucination), **viewed from behind** (no face/jersey detail to fabricate), **bareheaded soccer player (not American football, no helmet)**, **no visible logos or text**. Vision-QA prompt: "Be brutally honest. (1) Is this a single football/soccer player (NOT American football) with fists raised? (2) Any fake logos, fake sponsor text, fake jerseys with visible writing? (3) Does it look like a real photo? (4) Rate 1-10 as a newspaper cover hero."

**Stop rule:** three attempts maximum. If the third attempt still has fake text or wrong-sport gear, accept a tightly-cropped silhouette that obscures the figure's chest (e.g. back-lit, only torso-up) rather than ship a fourth.

## Cron-timing reality: Leagues Cup home openers ARE in scope for the 06:00 IST cron

Inter Miami Leagues Cup home openers kick at **7:30 PM ET**, which is **5:00 AM IST the next day**. The 06:00 IST morning cron lands ~1 hour after full-time — the match result (goals, lineups, suspensions served) is in scope for the same-day edition. Issue 11's lead: **Inter Miami 5-3 Atlético San Luis, Messi hat-trick**, was the verified final result after a quick `web_search "Inter Miami vs Atletico San Luis final score recap"`. Yesterday's preview-only story (Suárez's suspension carried into the tournament) was confirmed via ESPN.

**For future cron issues with same-day Western Hemisphere fixtures:**

- 06:00 IST = 00:30 UTC = 20:30 ET (previous day). Late-evening US fixtures have wrapped ~5 hours before the cron fires.
- 06:00 IST = 12:30 AM PT. West-coast late games have wrapped ~2-3 hours before.
- US afternoon/early-evening fixtures (1-5 PM ET = 9 PM-1 AM IST) are *not* yet played when the cron runs — those go in as preview or skip.
- Same logic in reverse: a 7:30 PM ET kickoff means final whistle at ~9:30 PM ET = 6:00 AM IST next day — that lands INSIDE the cron's coverage window, before 06:00 IST becomes 06:30 IST. Confirmed: match ended ~5:45 AM IST Aug 6, cron fired 06:00 IST Aug 6, results were in scope.

**Cron-brief implication:** if a beat's lead depends on a Western Hemisphere same-day match, always verify the final score via `web_search` against the team names before composing. Do not assume "preview only" — the match may have ended before the cron fired.

## Telegram auto-skip is the delivery, not a failure (closing-note discipline)

`send_all.sh` printed:

```
send_all: → Telegram (Aditya)
send: FAIL — hermes send did not confirm delivery
send: error: Skipped send_message to telegram:1462067171. This cron job will already auto-deliver its final response to that same target. ...
send_all: → mail:reachnancysharma@gmail.com
send_pdf_mail: delivered to reachnancysharma@gmail.com · 2026-08-06.pdf (1978265 bytes)
send_all: ── fan-out summary ──
send_all:   tg            FAIL  (send.sh exit 2 — see gateway.log)
send_all:   mail:reachnancysharma@gmail.com  OK
send_all: tg 0 ok / 1 fail · mail 1 ok / 0 fail
```

The Telegram "FAIL" is the **correct path** — the cron runner detected this is the same session the cron fired from, so the final assistant response will be inline-delivered to Aditya's chat. The mail channel succeeded normally via `send_pdf_mail.py`.

**Closing-note rule** (now codified in `automated-editorial-briefings` Pitfalls):

> When the gateway skips Telegram, do NOT say "Telegram failed". State explicitly: "Telegram: delivered via this final response (cron auto-delivery). Mail: delivered to <address>." The mail channel is the only one that should be reported via the send-all summary table.

A retry here would have been a real failure mode — `hermes send` may queue indefinitely without flushing, and a blind retry would just produce another identical "auto-delivery duplicate target" skip with wasted cron time.

## Verification facts (Issue 11)

- Manifest at `~/.hermes/data/hermes-times-v4/manifests/2026-08-06.json`, 31321 bytes, validates as JSON, 5 sections.
- All 5 `art_uri` paths exist on disk (cover + 4 section heroes).
- Render: 6 pages, PDF 1978265 bytes, PNG 513633 bytes. `pdfinfo` page count matches.
- Vision-QA on cover: 9/10 — silhouette rendered, headline legible, dek + why-it-matters sidebar visible, no layout breakage.
- Mail delivery to `reachnancysharma@gmail.com` confirmed (1978265 bytes).
- Telegram delivery: via cron auto-delivery of this final response, not via `send_all.sh`.

Reusable framing patterns surfaced this issue:
- "Bloomberg's Aug 4 scoop on Anthropic's six-year, $10B compute commitment to Volta Infra — a six-month-old Nvidia-backed startup — is the week's biggest AI business story." (Multi-source synthesis on a single dominant story.)
- "Pérez has broken ranks with the politeness of a launch-season press release." (Editor voice on a driver speaking truth to power.)
- "the cleanest single-game signal yet that [player] has put [previous disappointment] behind him" (Hat-trick framing that connects current performance to past narrative.)
- "the rest-agreement built into the All-Star calendar is now structural" (Framing an institutional change as a fait accompli.)
