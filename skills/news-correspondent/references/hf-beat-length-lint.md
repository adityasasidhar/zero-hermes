# HF Beat — Length & Field Lint (assertion pattern)

Codified 2026-08-05 after two over-band drafts (a dek at 27 words, a body at
206 words) were caught before JSON emission. Run this in `execute_code` after
you have assembled the `stories` list and *before* you emit or write the JSON.
A failed band should fail the script and force a fix — never silently emit
over-band text.

```python
import json

def cw(s): return len(s.split())

BANDS = {
    "headline": (6, 12),
    "dek":      (15, 25),
    "body":     (130, 200),
}

def lint_stories(stories, source_name_map=None):
    """Validate every story in the package against the band rules."""
    failures = []
    for s in stories:
        for field, (lo, hi) in BANDS.items():
            val = s.get(field, "")
            n = cw(val)
            tag = "OK"
            if not (lo <= n <= hi):
                tag = f"FAIL ({n} not in [{lo},{hi}])"
                failures.append((s.get("rank"), field, n, val[:60]))
            print(f"Rank {s.get('rank')}: {field}={n}w [{tag}]")
        # Hard invariants
        for required in ("rank", "headline", "dek", "body", "source_url",
                         "source_name", "published_at"):
            if not s.get(required):
                failures.append((s.get("rank"), f"missing:{required}", 0, ""))
        if s.get("rank") == 1 and not s.get("image", {}).get("local_path"):
            # Lead image is required by the contract — fail loudly
            failures.append((1, "missing:lead image", 0, ""))
    return failures

# Usage at the end of any beat package:
#
#   failures = lint_stories(stories)
#   assert not failures, f"length/field lint failed: {failures}"
#   package = {"beat": "<beat>", "stories": stories}
#   print(json.dumps(package, indent=2))   # or write to disk
```

## Band cheat sheet

| Field       | Min | Max | Notes                                                       |
|-------------|-----|-----|-------------------------------------------------------------|
| `headline`  | 6   | 12  | Declarative, no padding words (don't reach for "actually")  |
| `dek`       | 15  | 25  | One sentence. Why a builder would care                      |
| `body`      | 130 | 200 | 2–3 paragraphs. ~150 is the sweet spot. Trim paragraph 2 first |

**The HF beat's orchestrator-supplied brief sometimes widens the body cap to 100–220 words** for technical/heavy-paper stories where benchmark tables and architecture names need room (e.g. a 314B MoE with a 256K context needs >150 words to mention the architecture, the pretraining tokens, the benchmarks, and the deployment story). When the brief uses the wider band, override the lint to `(100, 220)` and aim for ~180–190 words; the renderer tolerates up to 220 but tightens more aggressively over 200. When the brief uses the standard `(130, 200)` band (default), keep the tight ceiling. Check the brief's literal numbers before linting — paste the brief's band into the `BANDS` dict, don't assume.

## Other field invariants worth linting in the same pass

- Every story has `rank`, `headline`, `dek`, `body`, `source_url`, `source_name`, `published_at`.
- Rank 1 carries a `kind: "downloaded"` (or `"generated"`, if you ran `mmx`) image
  block with a real `local_path` that `ls`'d to ≥ 20 KB between the download and
  the JSON write — don't punt on the lead photo.
- `image.local_path` is unique per rank — see umbrella pitfalls for the "same
  thumbnail used for rank 1 and rank 2" trap.
- `source_name` is non-empty and follows the lab+venue pattern (e.g.
  `"MiniMax via HF"`, `"Microsoft Research via HF"`, `"ByteDance via HF"`,
  `"Hugging Face Code Research via HF"`); never `"arxiv.org"`, never `"huggingface.co"`.
- `published_at` is ISO-8601 UTC (`YYYY-MM-DDTHH:MM:SSZ`); never a bare date.
- All `image.local_path` files actually exist on disk — re-run `ls -la` immediately
  before `write_file` if a parallel leaf could have wiped the dir.

A short lint loop is much cheaper than an editor cutting a 230-word body down to
195 mid-sentence downstream.
