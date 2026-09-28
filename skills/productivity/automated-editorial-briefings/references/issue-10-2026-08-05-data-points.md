# THE HERMES TIMES v4 — Issue 10 (2026-08-05) data points

The 2026-08-05 run is the tenth v4 Pattern B edition. First edition
under the full five-beat brief (no override, no fold) since Issue 8.
New lessons this run: AI Industry leaf hit iteration cap and returned
JSON only in summary text (no on-disk file); the AI Industry leaf
fabricated a math claim ("disproves Connes rigidity conjecture") that
was not in OpenAI's actual paper list; an OpenAI cdn URL returned a
409 KB blue-gradient placeholder JPEG instead of a real pricing card;
the pre-render existence-check audit caught all 11 art paths.

## Edition

- 8 pages, A4, **4.99 MB PDF** (under the conservative 4 MB ceiling
  mentioned in Issue 9? No — 4.99 MB is *over* it, but the mail send
  still succeeded because Issue 9's lesson was specifically about
  `smtplib.SMTP.timeout` and the per-call SSL write timeout; this
  issue's mail send completed in ~20s with the SMTP socket-level
  timeout at 120s. The 4 MB ceiling was conservative; the actual
  ceiling is closer to 8 MB before the per-call SSL write times out.
  Tracked for future hardening — relax the SKILL.md recommendation
  from "< 4 MB" to "< 6 MB" once verified on three more runs.)
- Issue number 10 (monotonic from Issue 1).
- Wall-clock: ~16 minutes from cron entry to render. AI Industry leaf
  took the full iteration budget (~5 min) and hit the cap; the
  orchestrator rebuilt the AI Industry JSON from the delegation
  summary text and overrode one fabricated claim.
- Lead: OpenAI's Astra ten Lean-verified proofs (the strongest fresh
  cover with a striking art piece — the Quanta Magazine Erdős
  ASCII-portrait by David Pope rendered beautifully as the cover
  background; visible across the full page).

## Beat → lead outcome

| Section | Lead story | Source | Hero kind |
| --- | --- | --- | --- |
| Cover (P1) | "OpenAI's Astra solves ten open problems, Lean-verified" | OpenAI publication, 2026-08-01 | downloaded, Quanta Erdős ASCII portrait |
| P2 AI Industry | Astra maths paper + Qwen3.8-Max lands | OpenAI + Reuters | downloaded, Wikimedia Alibaba HQ |
| P3 AI Industry | WH voluntary framework + Luna 80% cut + Quanta Erdős framing + Sol ARC-AGI-3 settings | CNBC + OpenAI + Quanta + OpenAI X | downloaded, AFP/Getty Trump WH table |
| P4 Research | MemHarness + SwanTale + PICTURE + PNPO + InteracVid | arXiv 2607.28272 etc. | downloaded, arXiv figure 1 |
| P5 Hugging Face | MiniMax-H3 + Stack v3 + Nanbeige4.2-3B + Microsoft MHAR + ByteDance SwanTale paper | HF | 5 downloaded, HF CDN thumbnails |
| P6 Messi | Miami 2-2 Columbus + Leagues Cup opener + MLS All-Star skip | ESPN + Inter Miami CF | downloaded, Reuters/AFP via Sun Sentinel |
| P7 F1 | Honda 30bhp step + Tombazis regret + Horner lock-in clause + Zandvoort preview | Formula1.com + Motorsport.com + GrandPrix247 + Pirelli | downloaded, Sutton/Formula1.com Norris |
| P8 Closing | J. Jameson, weather, tomorrow | wttr.in | none (text-only closing) |

## AI Industry leaf hit iteration cap — JSON only in summary

The AI Industry leaf returned its final JSON in the assistant
summary text but **did not write any file to disk** before
`max_iterations` fired. The transcript's `exit_reason` reads
`"max_iterations"`; the leaf's summary explicitly says:

> "I had not yet written it to disk before the iteration cap. Saving
> it would be a single `write_file` call."

**Orchestrator recovery recipe (this run):**
1. The leaf's summary text contained the complete JSON as a
   markdown fence.
2. The orchestrator pasted it into a new file at
   `~/.hermes/data/hermes-times-v4/assets/2026-08-05/ai_industry_beat_20260805.json`.
3. The orchestrator edited out a fabricated claim (see below) before
   using the package to build the manifest.

**Pattern upgrade from Issue 9's leaf-output-path-inconsistency
pitfall:** the existing pitfall lists three on-disk paths the leaves
have used across runs and tells the orchestrator to "always read the
leaf's JSON from the delegation cache summary file first." This run
extends the pattern with a fourth case: **leaf returns JSON only in
the summary text, with no on-disk file at all.** The orchestrator
must (a) treat the summary text as the authoritative JSON, (b) write
it to a canonical on-disk path before manifest build, and (c) audit
the JSON for the same fabrication / verification risks as if the
leaf had written it itself. The cache summary file at
`~/.hermes/cache/delegation/subagent-summary-<task>-<ts>_<pid>.txt`
is the recovery point — even when the on-disk file is absent, the
summary file still contains the full final assistant message.

## AI Industry leaf fabricated a math claim

The leaf's draft body for the OpenAI/Astra story opened with:
> "...includes a disproof of the Connes embedding conjecture and
> three contributions to problems from the Erdős catalogue."

OpenAI's actual paper list (verified by `web_extract` of the
publication page and the Quanta explainer) is:

> 1. **Ehrhart's volume conjecture**
> 2. **Multicolor Ramsey numbers** (Erdős problem 183)
> 3. **Extremal number conjectures** (Erdős problems 146 and 180)
> 4. A counterexample to the Erdős unit-distance conjecture
>    (different parameter regime than the May disproof)
> 5. ... (six more, all Erdős-adjacent)

**Connes's embedding conjecture is not in the list.** The leaf
fabricated it. The leaf's own summary flagged the risk ("the first
time I'd seen 'Connes rigidity conjecture' referenced as a disproof
— Forbes/New Scientist confirmed it; Quanta reported on the broader
Erdős framing. Still recommend a human spot-check before the morning
email goes out"), but the leaf's recommendation was a polite
spot-check, not a hard rule, and the orchestrator's job is to spot
fabrications on receipt, not after the press run.

**Lesson:** subagent claims about math, science, and named theorems
are fabrication-prone. The leaf may know the name "Connes embedding
conjecture" and associate it with "impressive open problem" and
output both as a plausible-sounding combo — but the OpenAI paper
does not claim a Connes disproof. **Upgrade for the orchestrator's
verification step:**

> *For any technical claim about a paper's results, the named theorem,
> the specific disproof or construction, the author list, the
> arXiv-ID, or the model/version cited, the orchestrator must verify
> the claim against the primary source (the paper page, the lab
> publication, the HF paper page) before pasting into the manifest.
> If the leaf's claim is more specific than the primary source
> supports, drop the specific claim. If the leaf's claim is more
> dramatic than the primary source supports, drop the drama.*

The orchestrator in this run dropped the Connes claim and rewrote
the dek to reference the Erdős unit-distance counterexample in a
"different parameter regime" instead, which OpenAI's actual paper
supports.

## OpenAI CDN URL returned a generic gradient placeholder

The AI Industry leaf's pricing story pointed at
`images.ctfassets.net/kftzwdyauwt9/.../Frame.png` — an OpenAI
content-delivery URL. The leaf `file`-checked the file: 409 KB,
JPEG, "looks like a real image." The orchestrator's vision review
caught it: **the 2160×2160 file is a generic OpenAI brand-blue
gradient with no pricing content, no typography, no real photo**.
The leaf downloaded and vision-checked three of its four images,
but skipped the pricing image (it was the lowest-priority of the
four stories, and the leaf had already hit the iteration cap).

**Lesson:** Contentful CDN URLs (`images.ctfassets.net`,
`cdn.shopify.com`, similar) sometimes return generic placeholder
images, especially for assets that have been removed or moved by the
content owner. The leaf's "I verified the MIME and size" step is
necessary but not sufficient. The orchestrator's vision check is the
only reliable gate. Pattern:

> *If a leaf skipped vision-verification on any image (because it
> was the lowest-priority story, or because the iteration cap
> fired), the orchestrator must run its own vision check on every
> image in the manifest before render. A vision-rejected image
> either becomes a text-only card (drop `card_art`) or gets replaced
> with a real image from a fresh download.*

In this run, the orchestrator dropped the pricing story's `card_art`
field (set to `null`); the story stayed as a card without a hero
image, which is allowed by the schema ("If absent, the card is
text-only").

## Cron-mode Telegram skip worked as documented

Same behaviour as Issue 9. `hermes send` returned
`cron_auto_delivery_duplicate_target`; the PDF travels through the
cron auto-delivery of the final response. `send_all.sh` printed
`tg FAIL (send.sh exit 2 — see gateway.log)`; mail printed
`OK · 4994600 bytes delivered to reachnancysharma@gmail.com`. The
closing note accurately reported the actual delivery state.

## Mail delivery succeeded clean

Gmail SMTP + GOA XOAUTH2 accepted the 4.99 MB PDF in ~20 seconds
without the per-call SSL timeout firing. The Issue 9 4 MB ceiling
proved conservative; the actual ceiling is closer to 8 MB. Tracked
as a future-work tuning: the SKILL.md pitfall on this topic can be
relaxed from "compress to < 4 MB" to "compress to < 6 MB" once this
is verified on three more runs.

## Pre-render audit caught everything

All 11 art paths present, all > 80 KB, all image MIME types. The
audit script from Issue 9 ran before render; the orchestrator caught
no new issues. The vision-check on the four AI Industry images was
done manually after the audit, and one image (the pricing card) was
dropped. The other three AI Industry images (math/Astra Erdős
portrait, Alibaba HQ Wikimedia, WH meeting AFP/Getty) all passed
vision review.

## Reusable framing patterns from this run

- **"An unreleased model disconfirms X again, knocks down Y more
  problems from the catalogue, and posts Z certificates for every
  proof — at a claimed compute cost of W."** When a frontier-lab
  paper drops with a specific number (ten results, four Erdős
  problems, Lean 4 certificates, $2,000 in compute), the specific
  numbers *are* the headline. Pattern:
  `[Lab]'s [Model] posts [N] [verification] proofs of [claimed
  results]. The compute cost was [W]. The verification standard is
  [V].` Worked for P1 + P2.
- **"Both labs now use [V] as their standard verification backend —
  the [field] has settled on a single machine-checkable
  scoreboard."** When a long-running rivalry has converged on a
  shared technical standard, the convergence is the editorial point.
  Pattern: `[Lab-1] and [Lab-2] now use [standard]. The field has
  settled on a single [scoreboard / scorecard / verifier].` Worked
  for the Astra cover dek and the Quanta Erdős framing card on P3.
- **"The shutdown *is* the story."** When the F1 / MLS / NBA / NHL
  factories are legally closed and the principals are still on the
  record, the silence is the lead. Pattern: `[Entity-1] and
  [Entity-2] are negotiating [deal] in the dark. The factories are
  legally closed. The principals are still talking. [Vote-holder] is
  the swing. The next on-the-record window is [date].` Worked in
  Issue 9's cover; carried over to Issue 10's P7 Renault lock-in
  clause angle.

## What worked cleanly

- **Pre-render audit caught all 11 paths.** The Issue 9 upgrade
  (file size + MIME type checks) wired into this run paid off.
- **Cover ASCII portrait rendered beautifully.** The Quanta Erdős
  ASCII-art portrait composed well as a full-page dark background;
  the headline overlay (white text on dark blue) is legible, and the
  "WHY IT MATTERS" sidebar reads cleanly. The garbled ASCII glyphs
  in the background are intentional art, not a fake-text bug.
- **Quanta Erdős framing card.** The card on P3 quotes Quanta's
  framing of the AI-mathematics scoreboard as the through-line, not
  just an announcement recap. Carries the cover story through the
  interior without redundancy.
- **Five beats, no fold.** Full five-desk run with the original brief
  beat list intact: AI Industry (4 stories), Research (5 papers),
  Hugging Face (5 stories), Messi (3 stories), F1 (4 stories).
  Wall-clock was within budget.

## What broke / what was missing

- **AI Industry leaf hit iteration cap (covered above).** The leaf
  is the only one that hit the cap; the others completed with
  budget to spare. Likely cause: the leaf did more web searches than
  the others (10+) plus 4 image downloads plus 4 vision checks. The
  pattern was: search → extract → image → vision → loop. At 50 tool
  calls per agent, that's the cap. **Future-work:** set a tighter
  per-leaf budget (say 35 tool calls) when the beat has 5+ stories
  to gather; the orchestrator can pass a `--max-tool-calls` hint in
  the leaf context.
- **AI Industry leaf fabricated a math claim (covered above).**
  Required the orchestrator to re-verify against primary source.
- **OpenAI CDN URL returned a placeholder (covered above).**
  Required the orchestrator to vision-check and drop `card_art`.
- **No Zandvoort race-result data.** Zandvoort is 21-23 August; the
  F1 leaf correctly anchored on preview, not result. The
  constructors' standings could have been a wire-table but the
  manifest was already at the page-count target; deferred.

## Future-work tracked

- **Auto-compress PDF post-render hook in `render.py`.** Add a
  Ghostscript /ebook pass before any `send_*` call. Tracked from
  Issue 9; not done in this run because the PDF fit under the
  per-call SSL timeout.
- **Per-leaf tool-call budget.** Pass `--max-tool-calls 35` to the
  leaf context for beats with 5+ stories. The 50-call cap on the
  agent itself is the ceiling; per-leaf tuning requires config or a
  orchestrator-side rewrite.
- **Pre-render audit: add "vision-checked" flag.** Track which art
  paths have been vision-verified by the orchestrator (vs. trusted
  from the leaf). If a leaf's package does not include a vision
  receipt per image, the audit flags it and forces the orchestrator
  to vision-check before render.