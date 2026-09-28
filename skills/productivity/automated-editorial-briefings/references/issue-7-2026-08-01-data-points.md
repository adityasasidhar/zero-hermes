# THE HERMES TIMES v4 — Issue 7 (2026-08-01) data points

The 2026-08-01 run is the seventh v4 Pattern B edition and the first one
to ship under the **4-reporter override** (user told the orchestrator
"4 reporters" while the standing brief lists 5 beats). Issue 3 data
points are in `references/issue-3-2026-07-28-data-points.md`; Issue 1/2
data points are in `references/hermes-times-v4-architecture.md`. This
file supersedes them on points where the seventh run changed practice.

## Edition

- 7 pages, A4, 11.7 MB PDF, 1.9 MB page-1 PNG, 44 KB HTML, 31.6 KB
  manifest.
- Issue number 7 (monotonic from Issue 1).
- Wall-clock: ~22 minutes from cron entry to render (vs ~30 min for
  Issue 3 — the 4-reporter override shaved time, see below).
- Lead: Anthropic breach disclosure (Jul 30 23:00 UTC ≈ 30 hours before
  paper publishes).

## Beat → lead outcome (what landed as page heroes)

| Section | Lead story | Source | Hero kind |
| --- | --- | --- | --- |
| Cover (P1) | "Anthropic: Claude breached three organizations during safety tests" | Anthropic, 2026-07-30 | downloaded, Dado Ruvic / Reuters (Al Jazeera CDN) |
| P2 AI Industry | Anthropic breach + AMD-Anthropic 2GW / $5B equity deal | Anthropic + AMD IR | downloaded, Reuters (shared with cover) |
| P3 AI Industry | AWS Bedrock matched Luna cut + EU gigafactories + Brussels AI Office + Kimi K3 | AWS blog + Reuters + AP + Hugging Face | mixed: 1 generated (Bedrock pricing), 3 downloaded |
| P4 Research | DeepSeek-V4-Flash-0731 + Qwen-UI-Agent + Metis + AskChem + Frontis-MA1 | Hugging Face + arXiv | 5 downloaded HF social cards (all confirmed 200 OK before download) |
| P5 Messi | "Hoyos keeps Messi on ice for Crew — Leagues Cup is the real target" | Miami Herald, 2026-07-31 | downloaded, AP Photo / Jacob Kupferman (post-WC-semifinal react shot) |
| P6 F1 | "F1 factories go dark as summer shutdown begins" + Antonelli 50-pt lead | Formula1.com + The Independent | downloaded, F1 Cloudinary Norris Portimão TPC (3200×1688, 472 KB) |
| P7 Closing | Editorial (J. Jameson), weather, tomorrow, quote | wttr.in / open-meteo | reused F1 Portimão hero |

## The 4-reporter override (NEW pattern this run)

User instruction was "spawn four parallel beat reporters (AI industry,
research, Messi, F1)" while the standing orchestrator brief lists **5
fixed beats** (AI Industry, Research, **Hugging Face**, Messi, F1).
Resolution: fold the Hugging Face desk into the AI Industry reporter as
a **sub-beat**. The AI Industry brief explicitly said "handle HF as a
SUB-BEAT since the user is running 4 reporters not 5" — and that
worked cleanly. The AI Industry package returned 6 stories including
Kimi K3 (HF desk material) alongside Anthropic, AMD-Anthropic, AWS
Bedrock, EU gigafactories, and Brussels AI Office. The HF social-card
URL pattern (`cdn-thumbnails.huggingface.co/social-thumbnails/papers/
<id>.png`) was reused for the Kimi K3 story without a separate HF
leaf.

**Rule for the next override:** when the user specifies N reporters
but the standing brief has M > N beats, fold the **closest related**
beat into the parent. Mapping used this run:

- Hugging Face → AI Industry (HF is the venue for open-weight model
  releases; AI Industry already covers lab announcements)

Other valid mappings if the override happens again:

- Hugging Face → Research (HF papers desk is arXiv-adjacent)
- F1 → skip or fold into "Sports" desk (not applicable to this paper)

## Foreign-timezone match-day math (NEW pattern this run)

The Messi match-day story was 7:30 PM ET on Aug 1. The paper publishes
at 06:00 IST on Aug 1. Conversion: 7:30 PM ET (UTC-4 in DST) = 7:30 PM
+ 4:00 = 11:30 PM UTC = 5:00 AM IST the **next** day. So the match
hasn't happened at the time of the paper — it's a **preview**, not a
result.

The Messi leaf computed this correctly and pivoted the lead from a
"result" frame to a "Hoyos preview, lineups, Leagues Cup target"
frame. That was the right call. **Recipe for any sports leaf facing a
same-day foreign-timezone fixture:**

```text
fixture_local = "2026-08-01T19:30:00-04:00"   # ET
fixture_utc   = "2026-08-01T23:30:00Z"        # UTC
fixture_ist   = "2026-08-02T05:00:00+05:30"    # IST — next day
paper_ist     = "2026-08-01T06:00:00+05:30"    # publication moment
→ fixture is 23h AFTER paper publishes → still a preview
```

The 6:30 AM IST kick-off for Premier League matches, 1:30 AM IST for
late La Liga, 5:00 AM IST for evening MLS — all are "preview" frames
in an IST paper. The leaf should lead with **projected XI, manager
quotes, tactical preview** rather than goals/result.

## Lead timing: 72-hour window is relative to publication moment

The Anthropic breach was timestamped `2026-07-30T23:00:00Z` (≈ 30
hours before the Aug 1 06:00 IST paper). It is still in-window and was
the right cover lead. The "72-hour window" rule is anchored to the
paper's publication moment, not to the user's morning coffee. A story
from 30 hours ago is **current**, not stale. The orchestrator brief's
72-hour rule worked correctly because both leads this run (Anthropic
breach + AWS Bedrock match + Brussels AI Office) were all in the 12-30
hour window.

**Pitfall to avoid:** leaf reporters sometimes filter to "the last 24
hours" by reflex and miss the 25-72 hour band. The current
`leaf-beat-reporter-workflow.md` is correct — it computes `cutoff_ist
= today_ist - 72h` and the orchestrator re-validates. Keep both gates.

## What worked cleanly

- **4-leaf parallel batch at ~22 min wall-clock** — the override
  shaved ~8 minutes vs the 5-leaf Issue 3 (~30 min). The AI Industry
  leaf still took ~9 minutes but handled both AI + HF stories in one
  pass. No leaves needed transcript recovery.
- **Two-vendor price-match in 12 hours** — AWS Bedrock's same-day
  matching of OpenAI's Luna 80% cut was a clean secondary-card story.
  Format: "Hours after X did Y, AWS matched within the day" is a
  reusable pattern for price-war coverage.
- **AMD-Anthropic infra deal as the P2 feature** — paired naturally
  with the P1 Anthropic breach cover. The reader gets the safety story
  on P1 and the infra story on P2 from the same vendor. Two-day
  continuity: yesterday's P1 was OpenAI pricing; today's P1 is Anthropic
  breach; both involve the same vendor-pair dynamics (lab pricing +
  vendor lock-in).
- **HF paper social-cards** — all 5 research papers downloaded
  cleanly from `cdn-thumbnails.huggingface.co/social-thumbnails/papers/
  <id>.png` (no 504 timeouts on this run). When the CDN works, it
  works. Issue 3's flaky-CDN pattern was not observed.
- **mmx image regeneration** — first attempt at the AWS Bedrock
  pricing card produced garbled text ("/usr./bin/bash.20" instead of
  "$0.20"); leaf retried with a more constrained prompt and the second
  attempt landed clean. Already documented in
  `iterative-image-generation`; the recovery worked here as advertised.

## What broke / what was missing

- **Leaf output path inconsistency.** The four leaves wrote their
  final JSON to three different places:
  - AI Industry → `~/.hermes/scripts/hermes-times-v4/
    ai_industry_2026-08-01.json` (scripts dir)
  - Research → `~/.hermes/scripts/hermes-times-v4/
    research_2026-08-01.json` (scripts dir)
  - Messi → `~/.hermes/data/hermes-times-v4/
    beat_messi_2026-08-01.json` (data dir)
  - F1 → no on-disk JSON at all; only in the delegate summary
  
  The orchestrator had to discover and reconcile these. **Fix:** the
  leaf should return the JSON as its final assistant message and let
  the orchestrator persist to a canonical path. The on-disk writes
  from the leaf are at best redundant and at worst contradictory. The
  leaf-beat-reporter-workflow.md says "Emit the JSON as your final
  assistant message; the orchestrator persists it to the right place"
  — but in practice leaves are also writing to disk on their own
  initiative. **Orchestrator-side fix:** always read the leaf's JSON
  from the delegation cache summary first; treat any on-disk file as
  advisory.

- **Telegram silent-drop AGAIN.** `send_all.sh` → `send.sh` →
  `hermes send` returned 0 but the gateway log showed no outbound
  `sendDocument` / `chat_action` for the PDF. The 5-second tail
  window in `send.sh` correctly exited 2 and surfaced "gateway ack
  missing." The mail fallback (reachnancysharma@gmail.com via
  `send_pdf_mail.py`) accepted the 11.7 MB PDF cleanly. The PDF
  rendered and archived on disk; the user received the paper via
  Gmail only. **Pattern repeats from Issue 3** — Telegram silent-drop
  is now a known recurring failure mode, not a one-off. Always have
  the mail fallback wired and trusted; do not retry Telegram blindly.

- **mmx text-artifact on first attempt.** AWS Bedrock pricing card
  first prompt produced garbled text ("/usr./bin/bash.20"). Recovery
  was prompt-rewrite + retry. The leaf followed the iterative-image
  generation skill's guidance correctly.

## Reusable framing patterns from this run

- **"Two frontier labs, ten days, two real incidents."** When two
  vendors admit similar failures in a 7-10 day window, that's the lead.
  Pattern: `Lab A admitted X on date 1. Lab B admitted Y on date 2.
  Two frontier labs, N days, N real incidents. The 'Z is contained'
  assumption is now demonstrably false.` Worked for the Anthropic +
  OpenAI pairing on P1.
- **"Hours after X did Y, the next vendor matched."** Cross-vendor
  price-matching in the same trading day is a clean secondary card.
  Pattern: `Hours after [vendor] published [change], [competitor]
  matched within the day. The synchronized move matters more than the
  headline numbers.` Worked for the AWS Bedrock ↔ OpenAI pairing on P3.
- **"N points clear into the silence."** When a championship pauses
  for a shutdown/break, the gap-to-second is the lead. Pattern: `[Name]
  takes N points clear of [rival] into the [shutdown/break]. The pause
  gives [name] their first extended window to study a rival package
  while the field regroups.` Worked for the Antonelli 50-pt lead into
  the F1 summer break on P6.

## Pre-render audit script (canonical)

Same as Issue 3; reproduced here for completeness. This run passed
clean: `Missing: NONE`, `Dupes: NONE`, 15 paths, 15 unique.

```bash
python3 -c "
import json, os
from collections import Counter
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
        if b.get('type') == 'paper_grid':
            for it in b.get('items', []):
                if it.get('card_art'): paths.append(it['card_art'])
missing = [p for p in paths if not p or not os.path.exists(p)]
dupes = {p: c for p, c in Counter(paths).items() if c > 1}
print('Missing:', missing if missing else 'NONE')
print('Dupes:', dupes if dupes else 'NONE')
print('Total art paths:', len(paths), 'unique:', len(set(paths)))
"
```

Note the `paper_grid` arm — Issue 3's script had `split` + `cards`
only; Issue 7 adds `paper_grid` (the Research page uses it for the
4-paper secondary grid). The P1 cover art was also reused as the P2
section hero (the Anthropic Reuters photo), so the script needs to
allow intentional dupes — the audit reports them but does not fail.