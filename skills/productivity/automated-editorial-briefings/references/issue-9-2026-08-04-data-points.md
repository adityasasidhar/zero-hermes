# THE HERMES TIMES v4 — Issue 9 (2026-08-04) data points

The 2026-08-04 run is the ninth v4 Pattern B edition. The first issue
to ship under the orchestrator's full orchestrator.md brief *as written*
(no override, no 4-reporter collapse, full 5-beat run — though the
orchestrator delegated 4 leaves here: AI Industry, Research, Messi, F1;
the HF desk was folded as a sub-beat into AI Industry exactly as in
Issue 7). New lessons this run: PDF size budget, Gmail SMTP upload
timeouts, cron-mode Telegram auto-delivery, the `delegate_task`
synchronous-batch return semantics, and the AI leaf's broken-download
recovery path.

## Edition

- 8 pages, A4, **1.96 MB PDF** (post-compression; the pre-compression
  render was 19 MB and the mail send timed out on it), 1.5 MB page-1
  PNG, 49 KB HTML, 37 KB manifest.
- Issue number 9 (monotonic from Issue 1).
- Wall-clock: ~13 minutes from cron entry to render. Faster than Issue
  7 because the AI Industry, Research, and F1 leaves all completed in
  under 6 minutes each, and the orchestrator was already running the
  renderer before the final leaf's transcript finished streaming.
- Lead: Horner-Wolff parallel Alpine bid (the strongest fresh cover
  with a real photo and a usable quote; the EU AI Act story from
  Issue 8 was deliberately *not* the lead a second day running).

## Beat → lead outcome

| Section | Lead story | Source | Hero kind |
| --- | --- | --- | --- |
| Cover (P1) | "Horner and Wolff stake out Alpine in the dark" | BBC Sport, 2026-08-01 | downloaded, BBC iChef (Horner at microphones) |
| P2 AI Industry | EU AI Act enforcement + Anthropic $20M donation to Public First Action | European Commission + Anthropic | downloaded, EU flag (Wikimedia) |
| P3 AI Industry | EU gigafactories €30B + Qwen3.8-Max 2.4T + DeepSeek V4-Flash 0731 + LG K-EXAONE 2.0 | EU Commission + Reuters + Korea Herald | 4 downloaded |
| P4 Research | Frontis-MA1 (35B agent > GPT-5.5+Codex on MLE-Bench) | arXiv 2607.28568 | downloaded, HF paper thumbnail |
| P5 Messi | Messi returns, Suárez banned, Leagues Cup opener Wed | Miami Herald + Sports Mole | downloaded, Getty via Goal CDN |
| P6 F1 | Horner-Wolff Alpine + factories dark + Zandvoort prince exits | BBC + Formula1.com + F1-Fansite | downloaded, BBC iChef (Horner) |
| P7 F1 | Zandvoort countdown + constructors' table | Formula1.com | downloaded, F1 CDN (summer break hero) |
| P8 Closing | Editorial (J. Jameson), weather, tomorrow, quote | wttr.in | reused F1 summer break hero |

## delegate_task returned synchronously in this session

The orchestrator prompt instructed `background=true` (a parameter that
the `delegate_task` tool now says is "DEPRECATED / IGNORED"). The
runtime treats all delegations as background but, in this
synchronous-render session (the cron does not have a foreground
consumer to return the result to), the tool returned the consolidated
batch result inline as the next assistant message — same shape as a
synchronous return. The dispatch notes confirm: *"background=true is
not available in this session — it cannot receive a detached
subagent result after the turn ends (a one-shot runner such as
`hermes -z`, a cron job, a Kanban worker, or a stateless HTTP
endpoint). The subagent(s) ran SYNCHRONOUSLY and the result is
included above."*

**Implication for the orchestrator:** in a cron session, treat
`delegate_task` as a *synchronous* call. Do not assume the result
arrives via a later turn. Do not poll the live transcript — wait for
the consolidated batch result. Total wall clock for a 4-leaf
synchronous batch was ~6 minutes (parallel), which is actually faster
than a typical async-with-poll pattern.

## PDF size budget: 4 MB is the Gmail SMTP ceiling (NEW)

Headless Chrome's `--print-to-pdf` embeds full-resolution images
without compression. With 8 pages of downloaded photos (cover
~620 KB, F1 photos ~580-621 KB, AI cards ~1 MB for the gigafactory
Politico image, research thumbnails ~250-380 KB each, Messi
~280 KB, plus 4 hero images) the rendered PDF ballooned to **19.1
MB**. The mail send (`send_pdf_mail.py` via Gmail SMTP + GOA
XOAUTH2) authenticated cleanly in 5 seconds, then silently hung on
the `send_message` SSL write until the 30-second per-call timeout
fired, dropping the connection with `SMTPServerDisconnected: Server
not connected`.

**Fix:** compress with Ghostscript /ebook before the mail send. The
pdfinfo page count and A4 page size are preserved; the file size
drops ~10x. Verified this run: 19.1 MB → 1.96 MB in 6 seconds.

```bash
gs -sDEVICE=pdfwrite -dCompatibilityLevel=1.4 -dPDFSETTINGS=/ebook \
   -dNOPAUSE -dBATCH \
   -sOutputFile=compressed.pdf original.pdf
mv compressed.pdf original.pdf
pdfinfo original.pdf | grep -E "Pages|Page size|File size"
```

**Why this matters:** the existing `send_pdf_mail.py` already sets
`smtplib.SMTP.timeout=120`, but that only controls the socket-level
timeout — the SSL write inside `send_message` uses a separate
per-call timeout that does not inherit from the socket timeout. The
workaround is to ship smaller PDFs, not to chase the SSL timeout.

**Future-work:** add a `--compress /ebook` flag to
`send_pdf_mail.py` so the orchestrator does not have to remember the
gs incantation. The skill should also have a one-liner
`render.py` post-hook that auto-compresses. Tracked, not done in
this run.

## Cron-mode Telegram auto-delivery skip (NEW)

The cron entry for THE HERMES TIMES has `deliver=telegram` and the
default user is Aditya's chat_id (1462067171). When the orchestrator
ran `hermes send` with the PDF inside the same cron session, the
gateway correctly detected the duplicate target and refused the
explicit send:

```json
{
  "success": true,
  "skipped": true,
  "reason": "cron_auto_delivery_duplicate_target",
  "target": "telegram:1462067171",
  "note": "Skipped send_message to telegram:1462067171. This cron job
          will already auto-deliver its final response to that same
          target. Put the intended user-facing content in your final
          response instead, or use a different target if you want an
          additional message."
}
```

This is **not a bug** and **not a silent drop** — the gateway is
doing the right thing. The paper is delivered to Aditya via the cron
auto-delivery of this very assistant response, not via a separate
`hermes send` upload. `send_all.sh` will still report `tg: FAIL
(send.sh exit 2 — see gateway.log)` because the gateway didn't
acknowledge a `sendDocument` for this PDF; that exit code is
expected in cron mode, not a failure to fix.

**Implication for the orchestrator's closing note:** do *not* claim
"Telegram delivered" when running inside a cron session with
`deliver=telegram`. State explicitly that the paper travels through
the cron auto-delivery of the final response. The mail channel
(reachnancysharma@gmail.com) is a separate subscriber and will
deliver via the normal `send_pdf_mail.py` path.

## AI leaf: one image downloaded as HTML stub (recovered)

The AI Industry leaf reported all 6 stories with valid image paths,
but the EU AI Act enforcement image at
`ai_eu_aiact_enforcement.jpg` was a 1.9 KB HTML stub
(`file` reported `HTML document, ASCII text, with very long lines
(429)`) — the source URL was a Wikimedia Commons redirect page, not
the image itself. The leaf's sibling downloads had already landed a
real 1280×854 EU Commission card at
`ai_eu_aiact_enforcement.png`, so the orchestrator `mv`'d the PNG
over the broken `.jpg` filename. The manifest already pointed at the
`.jpg` path so no JSON edit was needed.

**Lesson:** the orchestrator's pre-render existence check (the
`Missing: NONE` audit script) only catches *missing* files, not
*wrong-content* files. A 1.9 KB HTML stub is "present" by
`os.path.exists` but is not a real image. **Upgrade path:** add a
`file` check to the pre-render audit that rejects non-image MIME
types and file sizes < 20 KB. The leaf-beat-reporter-workflow.md
already documents the > 20 KB / image-MIME check; the orchestrator
audit script should mirror it.

```bash
# Pre-render audit, upgraded:
python3 -c "
import json, os, subprocess
m = json.load(open('<manifest>.json'))
paths = [m['cover']['art_uri']] if m.get('cover') else []
for s in m.get('sections', []):
    if s.get('art_uri'): paths.append(s['art_uri'])
    for b in s.get('bodies', []):
        if b.get('type') == 'split':
            for side in [b.get('left', {}), b.get('right', {})]:
                if side.get('card_art'): paths.append(side['card_art'])
        if b.get('type') in ('cards', 'paper_grid'):
            for it in b.get('items', []):
                if it.get('card_art'): paths.append(it['card_art'])
bad = []
for p in paths:
    if not p or not os.path.exists(p):
        bad.append((p, 'MISSING'))
        continue
    sz = os.path.getsize(p)
    if sz < 20_000:
        bad.append((p, f'TOO_SMALL ({sz}B)'))
        continue
    ftype = subprocess.run(['file', '--mime-type', '-b', p],
                           capture_output=True, text=True).stdout.strip()
    if not ftype.startswith('image/'):
        bad.append((p, f'NOT_IMAGE ({ftype})'))
print('Bad:', bad if bad else 'NONE')
print(f'{len(paths)} paths checked')
"
```

## send_all.sh timing: 90-120s outer timeout required

`send.sh` (the Telegram channel wrapper) takes 25-30 seconds to
exit when the gateway rejects the upload — it tails the gateway
log for 5 seconds after the `hermes send` call returns 0, then exits
2 if no `sendDocument` line appears. The mail channel
(`send_pdf_mail.py`) then runs. **Total wall clock for `send_all.sh`
is 60-90 seconds** when Telegram is involved. A 60-second outer
timeout (as the orchestrator initially set) killed the wrapper
before the mail stage began. Use 90-120s.

## Reusable framing patterns from this run

- **"The dark, the quiet, the bid."** When the F1 factories are
  legally closed and the principals are still on the record, the
  shutdown *is* the story — paired with the parallel-bid headline
  and a quote that survives the silence. The Wolff "broken glass"
  quote carried the cover; the Mercedes / Horner Alpine dual-bid
  was the factual substrate. Pattern: `[Name] and [Name] stake out
  [entity] in the dark. The factories are legally closed. The
  principals are still talking. [Vote-holder] is the swing. [Owner]
  has not moved.` Worked for P1.
- **"Hours after the rule book went live, the political bet doubled."**
  When an enforcement date and a political donation land within 72
  hours, the lab is signalling. Pattern: `Lab X's date-1 enforcement
  and date-2 donation are the same story told two ways. The
  enforcement is the stick. The donation is the public
  positioning.` Worked for the P2 EU AI Office + Anthropic pairing.
- **"Ban kicks in at the worst possible moment."** When a key
  player's suspension activates at the start of a new competition,
  the timing is the lead. Pattern: `[Player]'s [N]-game ban — for
  [incident] — becomes operative on [date], just as [he/she] had hit
  [stat]. [Coach] called it "a shame." The reshuffle is the real
  story.` Worked for the P5 Suárez-ban / Leagues Cup-opener pairing.

## What worked cleanly

- **6-minute synchronous batch** — the 4 leaves all returned before
  the orchestrator's render step. No transcript-recovery needed.
  The `delegate_task` synchronous-batch return is the right pattern
  for cron orchestrators that don't have a foreground consumer.
- **Cover photo: real, BBC iChef, FIA backdrop visible.** The image
  is dimly lit (Horner at microphones, an arm with a wristband
  visible) but the credit is honest and the file is 621 KB. The
  headless-Chrome render baked the photo dark; the cover mood
  accidentally matched the headline ("in the dark") and read well.
- **Research page held the line at 4 papers + 1 feature** — same
  shape as Issues 7 and 8. The paper_grid component with explicit
  arxiv_id + authors + sub + card_art schema worked first try.
- **Constructors' table for P7** — `wire_table_v2` with `num_col: 1`
  produced the same clean look as Issue 8's Hungary classification.
  Reusing the table data (Mercedes 379 etc.) one day later is
  fine because the source is the official F1 standings, updated
  per race.

## What broke / what was missing

- **19 MB PDF upload timeout (covered above).** The fix is in this
  reference; the skill's main SKILL.md pitfall list now carries it.
- **AI Industry leaf path inconsistency (still).** The AI leaf
  wrote its on-disk JSON to
  `~/.hermes/data/hermes-times-v4/assets/2026-08-04/ai_beat_2026-08-04.json`
  (under `assets/`, not `data/`). The orchestrator had to discover
  the path from the leaf's `local_path` field rather than guess it.
  **Same fix as Issue 7:** read the leaf's JSON from the
  delegation cache summary; treat on-disk files as advisory.
- **One image was 1.9 KB HTML stub (covered above).** The fix
  upgrades the pre-render audit script.

## Pre-render audit script (canonical, this run)

15 art paths in the manifest, all present, one was a 1.9 KB HTML
stub that the upgraded audit (not yet wired at the time of the
run) would have caught. The orchestrator caught it by `file`-ing
the EU AI Act asset and `mv`-ing the sibling PNG over it before
render.

The upgraded audit script lives in the "AI leaf: one image
downloaded as HTML stub" section above. Wire it into the
orchestrator's pre-render hook for Issue 10.
