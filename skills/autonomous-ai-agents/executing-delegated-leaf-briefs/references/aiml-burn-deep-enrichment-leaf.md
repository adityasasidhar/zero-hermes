# Worked Example: aiml-burn Deep-Enrichment Leaf BRIEF

This is the leaf-BRIEF pattern for **concept/entity notes inside the `/home/arctic/Documents/fun/wiki/aiml/` vault**, fed by the canonical template at `/home/arctic/projects/aiml-burn-tmp/BRIEF-TEMPLATE.md`. It is structurally distinct from the repo-enrichment pattern in `deep-enrichment.md` (the playbook under `vault-knowledge-graph`), so it lives here rather than there.

## Scenario

A parent agent (the wave-N dispatcher) decomposes "deep-enrich N aiml-wiki notes" into N leafs, one note each. This leaf gets ONE note and produces ONE enriched copy in `/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md`. Parent verifies and applies. The leaf never touches the real note path.

## Inputs

- **BRIEF.md** — the canonical template, with `<NOTE_PATH>`, `<basename>`, `<waveN>`, `<BYTES>`, and a list of wikilink targets all substituted.
- **Required reads in order:**
  1. `/home/arctic/Documents/fun/wiki/aiml/SCHEMA.md` — frontmatter fields, tag taxonomy, citation-marker convention.
  2. The full current note at `<NOTE_PATH>`.
  3. Any related `raw/articles/*.md` files in the aiml wiki covering the same concept — primary sources. Read at least 2 if they exist.
  4. `~/.hermes/skills/note-taking/vault-knowledge-graph/references/deep-enrichment.md` — the 11-section schema and depth criteria.

## Output

- `/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md` — the FULL enriched note (replaces original; not appended).
- Last line MUST be either `PASS: <bytes> bytes, <count> outbound links` or `FAIL: <one-line reason>`.

## Constraints (hard)

1. **Output path is `/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md`** (durable disk), NOT `/tmp/`. `/tmp/` has been wiped mid-burn before.
2. **Preserve frontmatter verbatim** except bump `updated:` to today's date (2026-07-29 at time of writing).
3. **Preserve any existing `Part of [[...]]` line** if present.
4. **Schema**: 11-section deep-enrichment schema. For **concept** notes: Overview, History/Motivation, Mechanism, Math, Implementation, Variants, Use cases, Trade-offs, Connections, Open questions, References. For **entity** notes (labs, papers-as-objects, people), the BRIEF may explicitly substitute — e.g. "entity type — emphasize History, Notable work, Connections" — replace the standard `Variants` slot with `Notable work`. **Read the BRIEF carefully for substitutions; don't apply the concept schema to an entity note.**
5. **Citation markers**: any paragraph that synthesizes 3+ sources or traces to a specific paper must carry `^[raw/articles/<source>.md]` (or `^[Research/Papers/...]` for the research tree) at the end.
6. **Outbound wikilinks**: minimum 5, all resolvable in the aiml wiki. Run `python3 /home/arctic/Documents/fun/wiki/build_index.py --check` to confirm target basenames exist. If a target doesn't resolve, add a `## Broken links in this note` footer flagging it.

## Pitfalls That Actually Fired (Read These)

### 1. Entity notes get a different schema than concept notes

The BRIEF sometimes specifies the schema per-note-type. A 2026-07-29 leaf run on `entities/anthropic.md` received "entity type — emphasize History, Notable work, Connections". Applying the default concept schema would have put `Variants` (wrong section for an entity) and lost `Notable work` (a heavy prose section listing the lab's tracked work). **Always read the BRIEF's "Schema" or "Section list" line before drafting.**

The entity substitution pattern:

| Concept slot | Entity slot (when BRIEF says "entity type — emphasize Notable work") |
|---|---|
| Variants | **Notable work** (longer prose; lists what the entity is known for in this vault) |
| Mechanism | Mechanism (unchanged; entity mechanism = how the entity's work operates on its subject) |

### 2. `read_file` strips content on re-read (dedup behavior)

`read_file(path)` returns the file content the first time. Subsequent `read_file` calls on the same path return `{'status': 'unchanged', 'dedup': True, 'content_returned': False}` with **no `content` key** — attempting `r['content']` raises `KeyError`. This bites in-script transforms: the script reads the file, drafts the enriched body, writes it, then wants to re-read to update the PASS line — and `read_file` won't give it back.

**Workaround: read via `terminal` + base64 in the script:**

```python
from hermes_tools import terminal, write_file
import base64
r = terminal("python3 -c \"from pathlib import Path; import base64; print(base64.b64encode(Path('PATH').read_bytes()).decode())\"")
text = base64.b64decode(r['output'].strip()).decode()
```

For one-shot reads at the start, `read_file` is fine. For in-script re-reads after writing, always use `terminal`+base64.

### 3. Byte-budget discipline: write to the ceiling, not past it

The BRIEF specifies a tight **6,000–10,000 byte** target. Dense LaTeX (`$...$`), em-dashes, Greek letters, and citation markers all push the byte count above the char count. **Plan section sizes against bytes, not chars.**

A 2026-07-29 leaf run on `entities/anthropic.md` hit the trim cycle:

| Write # | Bytes | Notes |
|---|---|---|
| 1 | 14,822 | first draft, too long |
| 2 | 11,585 | trimmed Math section |
| 3 | 11,216 | trimmed Notable work |
| 4 | 11,004 | trimmed Use cases |
| 5 | 10,400 | converted Trade-offs to prose paragraph |
| 6 | 10,183 | removed one redundant sentence |
| 7 | 10,025 | removed Circuit-evidence-localizes line |
| 8 | 9,918 | removed one final sentence — finally in range |

That is 7 extra `write_file` calls plus 7 tool turns. Pre-budget instead:

- 11 sections × ~900 bytes avg = ~10k target.
- Longest section (Notable work / Connections / Math) → 1.5–2k bytes.
- Shortest (Open questions / Variants / Use cases) → 250–500 bytes.
- Account for ~30 bytes per `$ ... $` LaTeX pair beyond the visible char count.
- Aim the FIRST `write_file` to land at `ceiling - 300 bytes`. One trim pass is acceptable; seven is not.

### 4. PASS line is read-only after the final write — emit it last

The PASS line must match `wc -c` of the file exactly. Use the **fixed-point loop** from the parent skill §10, not a mental estimate:

```python
import re
text = body_without_pass_line
targets = set(re.findall(r'\[\[([^\]|]+)(?:\|[^\]]+)?\]\]', text))
line = 'PASS: TBD'
for _ in range(10):
    final = text + line + '\n'
    n = len(final.encode())
    new = f'PASS: {n} bytes, {len(targets)} outbound links'
    if new == line: break
    line = new
final = text + line + '\n'
write_file(path, final)
```

Use `TBD` as the placeholder, NOT `PASS: 0 bytes, 0 outbound links`. A numeric placeholder can survive into the delivered file if the agent runs out of tool budget mid-trim (a documented 2026-07-29 failure mode).

### 5. "Outbound links" semantics: unique targets, not occurrences

The leaf BRIEF's PASS contract uses `count of unique outbound wikilink targets` in body content. The reliable pattern:

```python
targets = set(re.findall(r'\[\[([^\]|]+)(?:\|[^\]]+)?\]\]', text))
```

Note the **non-greedy `+` in `[^\]|]+`** so that piped links (`[[Target|display]]`) correctly capture `Target` as the bare name, not `Target|display`.

Citation markers like `^[raw/articles/x.md]` are NOT wikilinks and do NOT count. Frontmatter `sources:` and `tags:` lines are also excluded (no `[[...]]`).

### 6. Cross-vault bridge wikilinks are forbidden in body content

The aiml BRIEF explicitly forbids a class of wikilinks that *do* resolve under `build_index.py --check` but cross out of the aiml wiki into the personal vault's concept hubs. Common forbidden patterns:

- `[[Concepts/OCR|OCR]]`
- `[[Concepts/Native-Resolution Vision|Native-Resolution Vision]]`
- `[[Concepts/Vision-Language Model|Vision-Language Model]]`

The agent's job is to **describe these concepts in prose**, not link them. If the existing note has a `[[Concepts/X|X]]` link in the body, rewrite that line as prose (e.g. "Related OCR research in this vault..."), preserving the information without violating the cross-vault rule.

### 7. `build_index.py --check` exit code != failure for delivery

The check reports five counts plus `UNKNOWN TAGS` / `DEPRECATED TAGS` warnings. For a leaf BRIEF delivery:

- `broken=0` is the **hard gate** for new outbound links. New outbound links must not introduce broken ones.
- `ambiguous>0` is **informational, not a blocker**. A wikilink that resolves to `raw/articles/x.md` with no `concepts/x.md` is ambiguous-but-resolving and is acceptable. The aiml wiki has many such intentional duplicates.
- `unknown_tags` and `deprecated_tags` warnings are **non-blocking**. The parent surfaces them; the leaf's deliverable is the enriched file.
- The script exits with code 1 on `unknown_tags > 0` even when `broken=0`. Don't treat non-zero exit as a delivery failure — read the output and confirm `broken=0` independently.

### 8. Thin-source honesty: don't fabricate to fill the schema

When the source for a note is thin (a `Research/Papers/X/x.md` with only 30 lines stating framing but no encoder dimensions, token counts, or training data), do NOT invent specifics to fill `Implementation` / `Variants` / `Math`. The right moves:

- In **Math**: state the framing in information-theoretic terms without inventing a specific loss.
- In **Implementation**: list the *categories* of components and explicitly flag "concrete numbers should be verified against the paper before reuse".
- In **Variants** (or **Notable work** for entities): name variants evident from the *cross-link graph* and describe them in 1–2 sentences each.
- In **Open questions**: enumerate what's NOT in the source as research questions, not as fabricated answers.

A 2026-07-29 leaf run on `entities/anthropic.md` faced a thin source for the poisoning paper: the title, identifier, and numerical claim were unverified in the vault entry. The correct move was to retain the qualitative threat model, list the paper under Notable work with an explicit "bibliographic details and numerical result require verification" flag, and refuse to cite a specific number. The user BRIEF explicitly listed `Research/Papers` source refs as fine — so naming the paper is allowed; stating its claim as fact is not.

### 9. The 4 source papers form the cluster; cite them carefully

For `entities/anthropic.md` specifically, the 5 papers in `Research/Papers/` are the entity's footprint in this vault. The enriched note's Notable work section should:

- Cite each paper as the vault's stored entry, e.g. `[[Research/Papers/Alignment-Faking-in-LLMs/alignment-faking-in-llms|alignment-faking-in-llms]]`.
- Use the citation marker `^[Research/Papers/.../<basename>.md]` for synthesis paragraphs.
- NOT cite arXiv IDs, dates, or author lists as primary facts (the vault note's bibliographic surface is thin). Use formulations like "(2022)" or "the 2025 follow-up" without committing to a specific date unless the vault source confirms it.

### 10. Don't `Original (preserved)` a clean concept-note

The BRIEF says preserve original prose inside `## Original (preserved)` "if the existing note has content you can't reconcile with the 11-section schema". For a clean concept-note leaf where the existing note is a 1–2 KB overview mapping cleanly onto the schema (Overview + History/Motivation + Variants + References), the right move is **DEEPEN, not preserve-and-append** — fold original prose into the new sections and drop the verbatim section. The "preserve" rule triggers only when content doesn't fit the schema (rare for concept notes; common for cross-vault bridges where the original points at a vault hub the leaf BRIEF forbids).

## Verification Before Finishing

1. **Byte count**: `wc -c <path>` matches the PASS line's N exactly. Fixed-point loop guarantees this on final write.
2. **Link count**: PASS line's M matches `len(set(re.findall(r'\[\[([^\]|]+)(?:\|[^\]]+)?\]\]', text)))`.
3. **Section count**: 11 H2 sections (or 11 with Notable work substituted for Variants for entity notes). Verify with `grep -c '^## '` — should be ≥ 11 (the file may have H3 subsections too).
4. **Frontmatter preserved**: extract frontmatter from source and output, drop the `updated:` line from each, and assert equality. A 2026-07-29 leaf run did this via:

```python
from pathlib import Path
a = Path('/home/arctic/Documents/fun/wiki/aiml/entities/<basename>.md').read_text().split('---', 2)[1]
b = Path('/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md').read_text().split('---', 2)[1]
a = '\n'.join(x for x in a.splitlines() if not x.startswith('updated:'))
b = '\n'.join(x for x in b.splitlines() if not x.startswith('updated:'))
assert a == b
```

5. **Updated date**: `updated: 2026-07-29` is present in the output frontmatter.
6. **Required targets**: every wikilink the BRIEF listed as a cross-link target appears at least once in the body.
7. **Sentinel absent**: `PASS: TBD`, `PASS: WRITE_LAST`, `PASS: 0 bytes` patterns are NOT in the final file.

## Reporting Back

End with:

1. The output path (absolute): `/home/arctic/projects/aiml-burn-tmp/waveN/<basename>.md`.
2. Final byte count and link count.
3. The 11 section headers (or entity-substituted variants), for parent verification.
4. Any required targets from the BRIEF that did NOT make it into the body (rare; usually means a wikilink target doesn't resolve).