# THE HERMES TIMES v4 — Issue 3 (2026-07-28) data points

The 2026-07-28 run is the third successful v4 Pattern B edition. Use this file as the most recent reference for what's stable and what's brittle. Issue 1 and 2 data points are in `references/hermes-times-v4-architecture.md`; this file supersedes them on points where the third run changed practice.

## Edition

- 10 pages, A4, 3.87 MB PDF, 512 KB page-1 PNG, 42.6 KB HTML, 29.1 KB manifest.
- Issue number 3 (monotonic from Issue 1 = 42 historical numbering convention dropped at the v4 boundary).
- Wall-clock: ~30 min from cron entry to render.

## Beat → lead outcome (what landed as page heroes)

| Section | Lead story | Source | Hero kind |
| --- | --- | --- | --- |
| Cover (P1) | "Ilya's locked lab gets a machine big enough to matter" (NVIDIA ↔ SSI) | NVIDIA Newsroom, 2026-07-27 | generated, miniMax image-01 (slate-green guard-tower / computing-hall) |
| P2 AI Industry | SSI lead + 10× stat block + OpenAI task-crossover card | nvidianews.nvidia.com | downloaded, NVIDIA press kit (1600×900, 87 KB) |
| P3 AI Industry | Kimi K3 (2.8T MoE, 1M context, MXFP4) | platform.kimi.ai | downloaded, HF model thumbnail (1200×648, 43 KB) |
| P4 Research | "The Regression Tax" arXiv 2607.22520 | arxiv.org/abs/2607.22520 | generated, red-thread agent diagram (1280×720, 162 KB) |
| P5 Hugging Face | moonshotai/Kimi-K3 + Inkling + Laguna-S-2.1 | huggingface.co | generated, modular-neural-network collage (1280×720, 580 KB) |
| P6 Messi | Berterame stretchered off, discharged | intermiamicf.com | downloaded, Minas Panagiotakis / Getty (1600×1067, 198 KB) |
| P7 Messi | All-Star excuse + Cruz Azul Campeones Cup | intermiamicf.com | generated, empty pink stadium (1280×720, 161 KB) |
| P8 F1 | Stella praises Norris/Piastri respect | formula1.com | downloaded, F1 Cloudinary `2287599294` pit wall (3392×1908, 276 KB) |
| P9 F1 | Hungary race recap + classification wire table | formula1.com | downloaded, F1 Cloudinary `2287727846` Norris podium (3392×1908, 497 KB) |
| P10 Closing | Editorial (J. Jameson + Research Desk), weather, tomorrow | wttr.in + open-meteo | generated, rainy editor desk (1280×720, 255 KB) |

## What worked cleanly on this run

- **Five-leaf parallel batch with a 5–12 min wall-clock.** All five beat correspondents returned complete JSON; the slowest was the AI leaf at ~9 minutes. No leaves needed transcript recovery from `subagent-summary-*.txt`.
- **Generated art into per-slug subdirs.** Each `mmx image generate` call landed in its own `<slug>_<date>/` directory. No `image_001.jpg` collisions, no overwrites from concurrent calls. The orchestrator's pre-render existence check passed clean.
- **Two F1 section heroes from the same race weekend, different Cloudinary IDs.** `2287727846` (podium, lead hero on P9) and `2287599294` (Stella pit wall, P8 follow-up) — both downloaded at `w=3392, f_jpg`, no double-art on cards.
- **Weather strip in the closing page.** 26°C / feels-like 33°C / patchy light drizzle from `wttr.in/New_Delhi?format=j1` cross-checked with `api.open-meteo.com`. Both APIs agreed; the wttr value landed in the rendered P10.
- **Image audit at the orchestrator caught a leaf's "fine-looking" download that was actually a brand-logo card.** The AI Industry leaf's "SSI × NVIDIA" image was a 1600×900 JPEG of just two brand logos on black. `vision_analyze` saw it, the orchestrator moved it to P2 (where it works as a section opener card), and regenerated a different image for the cover. If you skip the orchestrator-level vision-review, you ship a logo card on P1.

## What broke on this run

- **Telegram delivery silently dropped.** `hermes send` exited 0, but `~/.hermes/logs/gateway.log` shows no outbound `sendDocument` / `uploadDocument` / `chat_action` for the PDF in the 5s tail window. `send.sh` correctly exited 2 and surfaced "gateway ack missing" to the editor. The same silent-drop happened on 2026-07-27 and is now a known pattern. The PDF rendered and archived on disk, but the user did not receive it on Telegram. The mail-based PDF send (`~/.local/bin/mail` + PDF attachment) is the verified-working fallback.
- **One generated frame was a green-blob.** First `mmx image generate` attempt for the AI cover returned an unusable near-monochrome green frame. `vision_analyze` rejected it; orchestrator retried with a rephrased prompt ("…vintage broadsheet editorial photograph…" rather than the original "text-free editorial news photograph…"). Second attempt landed clean. Without vision-review, this would have shipped.
- **Cron-DB will report `completed` regardless.** The `last_status: completed, error: None` line in the cron DB does NOT prove Telegram receipt. It only proves the agent returned. Verify with `send.sh`'s "gateway ack confirmed" suffix, not with the cron DB.

## New manifest schema elements exercised this issue

- `stat_block` (P2) — short accent-deep number + label + body, used once per page for the headline stat.
- `paper_grid` (P4) — three-paper grid with arxiv_id / title / authors / sub.
- `quote` (P8) — italic pull quote with attribution (`ANDREA STELLA, McLAREN TEAM PRINCIPAL`).
- `wire_table_v2` (P9) — five-row Hungary classification, `num_col: 0` right-aligns positions in accent deep.
- `editorial_aside` (P6, P9) — short editor's note with eyebrow, italic title, body, sig.
- `weather` (P10) — top of closing page only; the strip's flex layout breaks the masthead rhythm if injected into a section.
- `opinion` (P10) — exactly 2 items: J. Jameson (masthead editor) + The Research Desk. A 3rd item orphans.
- `tomorrow_list` (P7, P10) — 3-item numbered list, used twice.

## Pre-render audit script (canonical)

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
missing = [p for p in paths if not p or not os.path.exists(p)]
dupes = {p: c for p, c in Counter(paths).items() if c > 1}
print('Missing:', missing if missing else 'NONE')
print('Dupes:', dupes if dupes else 'NONE')
print('Total art paths:', len(paths), 'unique:', len(set(paths)))
"
```

Issue 3's manifest passed with: `Missing: NONE`, `Dupes: NONE`, 10 paths, 10 unique, 1 cover + 9 section art = 10 pages. The 10 unique-path figure is load-bearing: if it's lower than 10, a single image is being reused for two distinct story ranks.
