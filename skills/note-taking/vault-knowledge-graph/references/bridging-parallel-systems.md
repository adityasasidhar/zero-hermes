# Bridging parallel systems (hand-written vault ↔ agent-maintained wiki)

When the same content exists in two places — a hand-written vault note and an
agent-maintained `wiki/` page — and the user asks to "connect things", the work
is bridge-wiring, not deduplication. The duplicates are usually intentional
(see `wiki/AMBIGUOUS-LINKS.md`); the user wants navigation between the two
parallel systems, not merge.

## The canonical duplicate-stem list (Arctic's vault, 2026-07-25)

There are 18 bare stems that resolve to two (or three) files. The
`wiki/AMBIGUOUS-LINKS.md` manifest is the source of truth; refresh with
`python3 wiki/build_index.py` if needed. Current list:

| Pattern | Hand-written page | Wiki counterpart | Already bridged? |
|---|---|---|---|
| `rmsnorm` | `Concepts/RMSNorm.md` | `wiki/aiml/concepts/rmsnorm.md` | ✓ both directions |
| `swiglu` | `Concepts/SwiGLU.md` | `wiki/aiml/concepts/swiglu.md` | ✓ both directions |
| `flashattention` | `Concepts/FlashAttention.md` | `wiki/aiml/concepts/flashattention.md` (also `Research/Papers/FlashAttention/flashattention.md`) | partial |
| `kimi-k2` | `Research/Papers/Kimi-K2/kimi-k2.md` | `wiki/aiml/raw/articles/kimi-k2.md` | ✓ both directions |
| `deepseek-v3-technical-report` | `Research/Papers/DeepSeek-V3-Technical-Report/...` | `wiki/aiml/raw/articles/...` | partial |
| `attention-is-all-you-need` | `Research/Papers/Attention-Is-All-You-Need/...` | `wiki/aiml/raw/articles/...` | partial |
| `muon` | `Research/Papers/Muon/muon.md` | `wiki/aiml/raw/articles/muon.md` | ✓ both directions |
| `roformer-rotary-position-embedding` | `Research/Papers/RoFormer-Rotary-Position-Embedding/...` | `wiki/aiml/raw/articles/...` | partial |
| `small-language-model` | `Github/Repos/Small-Language-Model.md` | `wiki/aiml/entities/small-language-model.md` (also `wiki/aiml/raw/articles/small-language-model.md`) | bridged 2026-07-25 |
| `build-your-own-harness` | `wiki/aiml/entities/build-your-own-harness.md` | `wiki/aiml/raw/articles/build-your-own-harness.md` | entity→raw ✓ (raw is opt-in) |
| `cotgd-reproduction` | `wiki/aiml/entities/cotgd-reproduction.md` | `wiki/aiml/raw/articles/cotgd-reproduction.md` | entity→raw ✓ (raw is opt-in) |
| `leap` | `Github/Repos/LEAP.md` | `Research/LEAP/leap.md` (0-byte stub) | NOT bridgeable |
| `papers` | `Research/Papers/Papers.md` | `Work/Papers/papers.md` | MOC collision, not a bridge target |
| `issues` | `Docling/Issues.md` | `Work/Internship/Dataobserve/employment/Issues.md` | both real, no bridge needed |
| `index` / `log` / `schema` | (multiple) | (multiple wiki/ files) | structural MOC collisions |
| `rust` | `Learning/Rust/Rust.md` | `Learning/Rust/Rust.pdf` | file-vs-asset, not a bridge |

## Wiki-link conventions (copy exactly)

When the **hand-written note** needs to add a link to its wiki counterpart:

```markdown
## See also
- [[wiki/aiml/concepts/rmsnorm|rmsnorm]] — agent-maintained wiki concept page
- [[wiki/aiml/raw/articles/kimi-k2|kimi-k2]] — agent-ingested raw source (immutable)
- [[wiki/aiml/entities/small-language-model|small-language-model]] — agent-maintained wiki entity page
```

When the **wiki note** needs to add a link back to its hand-written counterpart:

```markdown
## Counterpart

- [[Concepts/RMSNorm|RMSNorm]] — hand-written vault concept hub
- [[Research/Papers/Kimi-K2/kimi-k2|kimi-k2]] — hand-written vault paper note
- [[Github/Repos/Small-Language-Model|Small-Language-Model]] — hand-written vault repo note
```

The `## Counterpart` heading is the wiki's convention (visible in
`wiki/aiml/concepts/rmsnorm.md`, `wiki/aiml/raw/articles/kimi-k2.md`).
The `## See also` heading is the hand-written note's convention.

## Stub-sacred rule (CLAUDE.md)

Per the vault's `CLAUDE.md`, **0-byte files are deliberate placeholders**.
`Research/LEAP/leap.md` (0 bytes) is a stub reserved for a topic that hasn't
been written yet — **do not bridge to it**. The bridge can only be made when
the stub gets filled in. If the user asks to bridge a pair where one side is
a stub, surface the stub-rule and ask: fill the stub first, link to the
non-stub side only, or skip entirely.

## Raw-article opt-in rule

`wiki/aiml/raw/articles/` files are agent-ingested source snapshots. Some
self-document as **immutable** (e.g. `kimi-k2.md` carves it out in its own
`## Counterpart` comment: `(immutable; does not link back)`). The convention:

- **Entity → raw** link (`See raw source: [[raw/articles/...|raw/articles/...]]`)
  is always added by the entity-side author. Standard.
- **Raw → entity** back-link is opt-in. If the raw article already has
  curated content, add a `- [[wiki/aiml/entities/<name>|<name>]] — entity
  summary` link in its `## Related` section. If the raw article is purely
  agent-ingested with no manual edits, **don't add** — leave it as the
  immutable source of truth.

## Worked example (small-language-model, 2026-07-25)

Before: `Github/Repos/Small-Language-Model.md` had no Wiki line.
`wiki/aiml/entities/small-language-model.md` had no `## Counterpart`.

After:

In `Github/Repos/Small-Language-Model.md` (under `## Links`):
```markdown
- Related: [[mini-gpt-implementation]], [[Ant_v1]], [[neural-networks-from-scratch]], [[TinyMoe]]
- Wiki: [[wiki/aiml/entities/small-language-model|small-language-model]] — agent-maintained wiki entity page
- Part of: [[Public Repos]]
```

In `wiki/aiml/entities/small-language-model.md` (appended):
```markdown
## Counterpart

- [[Github/Repos/Small-Language-Model|Small-Language-Model]] — hand-written vault repo note
```

Verification:
```bash
python3 /home/arctic/Documents/fun/wiki/build_index.py
python3 -c "
import json
d = json.load(open('/home/arctic/Documents/fun/wiki/INDEX.json'))
sm = d['notes']['Github/Repos/Small-Language-Model.md']
ws = d['notes']['wiki/aiml/entities/small-language-model.md']
gh_to_w = any('wiki/aiml/entities/small-language-model' in t for t in [lo['target'] for lo in sm['links_out']])
w_to_gh = any('Github/Repos/Small-Language-Model' in t for t in [lo['target'] for lo in ws['links_out']])
print(f'Github→wiki: {gh_to_w}  wiki→Github: {w_to_gh}')"
# Expected: Github→wiki: True  wiki→Github: True
```

## Don't subagent when scope ≤ 1 link

When a "bridge" request yields only 1 actual link pair to do (after the
duplicate-stem audit), do it with `patch` directly. Don't dispatch 3
subagents for 1 patch — the per-subagent overhead (cold-start, brief
context, transcript streaming) exceeds the edit cost. The user
specifically redirected away from a 3-subagent plan when the work
collapsed to 1 link pair. Resume the subagent-fan-out pattern only when
the audit surfaces ≥4 link pairs to bridge.
