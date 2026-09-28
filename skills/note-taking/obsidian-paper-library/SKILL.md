---
name: obsidian-paper-library
description: Build and maintain a chronological research paper library inside an Obsidian vault — one folder per paper, PDF + slug-matched reading-note per entry, an MOC with reading-order lineages (chronology + theme), and a wikilink resolver that actually mimics Obsidian's recursive-name resolution. Use when the user says things like "set up a papers folder", "every paper by <lab>", "read these into my vault", "ingest arXiv collection", "use the original PDF name like Books", "match the Books convention", or wants to extend an existing `Research/Papers/`.
version: 1.1.0
author: Hermes Agent (built for arctic / adityasasidhar)
license: MIT
platforms: [linux, macos, windows]
metadata:
  hermes:
    tags: [Obsidian, vault, knowledge-graph, papers, arxiv, research, note-taking]
    related_skills: [obsidian, obsidian-repo-notes]
---

# Obsidian Paper Library

Workflow for **populating an Obsidian vault with a scholarly paper library**: PDFs, per-paper index notes, and a chronological map-of-content (MOC) with reading-order lineages. Differs from `obsidian-repo-notes` (which handles **code repos** — stars/forks schema) — this is for **research papers** (arXiv IDs, authorship, technical reading notes).

## When to use

- "set up a papers folder in my vault"
- "download **every** paper by <lab>" / "find papers by <author>"
- "ingest these arXiv listings into my vault"
- "build me a reading list for <area>"
- "use the original PDF name like Books" / "match the Books convention" / "rename `paper.pdf` to real titles" — see **Books-convention variant** below
- existing `Research/Papers/` needs the next batch, or the MOC is stale
- existing vault has a separate paper stash (`Work/Papers/`, `Downloads/`, etc.) and the user wants **one** canonical papers folder — consolidate into `Research/Papers/`

Don't use for: code repos (use `obsidian-repo-notes`), single-paper summaries (use `obsidian` directly), random research jots.

## Books-convention variant (PDFs keep their original title filename)

This vault's `Books/` folder uses a different convention than the default `paper.pdf` scheme: PDFs sit alongside a flat wikilink index, each PDF keeps its **original download filename** (e.g. `Deep Learning (Ian Goodfellow, Yoshua Bengio, Aaron Courville) (Z-Library).pdf`), and the index note `Books/books.md` lists them as flat `[[filename.pdf]]` entries with no nested folders.

When the user says *"each paper should have its original name, like Books"*, or *"match the Books convention"*, or *"don't rename to paper.pdf, attach to graph"*, switch to this variant. Two layouts are common; pick with the user (default to IN-PLACE unless they explicitly want a flat tree):

### Layout A — IN-PLACE (default, lower risk)

Keep the existing `<Slug>/` folder and `<slug>.md` reading note. Just rename the PDF inside each folder from `paper.pdf` to `<Title>.pdf` (title extracted from page 1 via `pdftotext`), then wire the PDF into the graph:

```
Research/Papers/<Slug>/
  ├── <slug>.md                # reading note (unchanged)
  └── <Title>.pdf              # renamed from paper.pdf → original paper title
```

Each reading note gets:
- `> PDF: ./<Title>.pdf` updated (and ideally converted to a clickable `> PDF: [[<Title>.pdf]]` wikilink in the blockquote).
- A `## PDF` section right after the metadata blockquote containing `![[<Title>.pdf]]` — this is what makes the PDF appear as a **graph node** in Obsidian's graph view, not just a link.

A flat Books-style index `Research/Papers/papers.md` is added at the collection root: one section per lab/bucket (mirroring the existing `Papers.md` MOC structure) plus a final "All PDFs (alphabetical)" section. Each line is `- [[<Title>.pdf]]`. This becomes the second entry-point into the graph alongside the chronological `Papers.md`.

### Layout B — FLATTEN (Books-mirror, higher risk)

Move all PDFs into `Research/Papers/papers/` (flat, no per-paper folder), keep `<slug>.md` notes side-by-side at `Research/Papers/<slug>.md`. **This orphans the folder→note relationship** and forces wikilink rewrites across `Papers.md` and every cross-link in other notes. Only do this if the user explicitly asks for it ("completely flatten", "match Books exactly"). Reserve for fresh rebuilds, not extensions of an existing `Research/Papers/`.

### Title-extraction recipe

The hard part is getting the real paper title to use as a filename. `pdftotext -f 1 -l 1` is the start, but raw output is messy:

1. **Skip the arXiv banner** (line starting with `arXiv:YYMM.NNNNN`). Some PDFs put it on the right margin, others inline — `grep -m1` for `arXiv:` and start after.
2. **Skip `Provided proper attribution is provided, …`** (Vaswani 2017 boilerplate).
3. **Take the first 1–4 short non-empty lines** as title candidates, breaking on: lines containing `*` or `†` (author markers), URLs, `@email`, "Abstract", "1 Introduction", or any line that starts an author block (uppercase + comma + uppercase).
4. **Collapse letter-spaced caps.** PDFs often render titles with extra spacing like `A LIGNMENT FAKING IN LARGE LANGUAGE MODELS` (every char/short-fragment separated by a space). Detect: >40% of "words" are 1–3 char all-caps fragments → collapse consecutive fragments into one word. Don't apply this to normal-case titles (you'll corrupt them).
5. **Strip trailing author fragments** at the end of the title — `Kimi Team`, `DeepSeek-AI`, `Adam Tauman Kalai∗`, multi-name runs like `Yanda Chen Joe Bento`.
6. **Have manual overrides ready.** Even the best extraction fails on ~10% of papers (Vaswani 2017's copyright block hides the title; some Kimi papers have letter-spacing that doesn't match the heuristic). Maintain a small `MANUAL = {"Folder-Name": "Real Title"}` dict in your script and use it as the final word. This is faster and more reliable than trying to fix the heuristic for every edge case.

Use the bundled `scripts/extract_pdf_titles.py` for a one-shot extractor that produces a `{"folder": "title", ...}` dict and prints a dry-run table. Pipe the output through `clarify` for user approval before any renames.

### Wiring the graph

The point of renaming is so each PDF becomes a **first-class node** in Obsidian's graph view (otherwise PDFs are invisible graph nodes, just files referenced by markdown). Three writes per paper:

1. `> PDF: ./<Title>.pdf` → `> PDF: [[<Title>.pdf]]` (clickable wikilink, **not** embed — the blockquote doesn't render `![[]]`)
2. Insert a `## PDF` section after the metadata blockquote: `![[<Title>.pdf]]` (this is the graph-node embed)
3. Add `- [[<Title>.pdf]]` to `Research/Papers/papers.md` (Books-style flat index)

Idempotency: check for `## PDF` or `![[<Title>.pdf]]` or `[[<Title>.pdf]]` in each note before editing. Re-running the script must not duplicate the embed.

### `re.escape()` pitfall when bulk-rewriting paths

When substituting PDF paths with regex, **do not** wrap the filename in `re.escape()` and then `sub` it into a path that becomes file content. `re.escape()` over-escapes spaces, dots, colons, and hyphens (`Kimi K2: Open Agentic Intelligence.pdf` → `Kimi\ K2\:\ Open\ Agentic\ Intelligence\.pdf`). Obsidian cannot resolve `./Kimi\ K2.pdf` — the backslashes break the path. Build the path by direct string concatenation (`f"./{pdf_name}"`), not by regex sub on the escaped filename. If you must use regex (because the suffix ` (or direct: ...)` varies), apply the substitution **without** escaping the literal portion and only escape the variable parts separately.

## Single-papers-folder rule

When the user says "there should be only one paper dir" / "put all my research papers in that folder itself", treat the canonical location as the load-bearing constraint. The plan:

1. **Identify the canonical folder** — usually an existing `Research/Papers/` (already populated with per-paper folders) or one the user just names. Don't create a parallel structure.
2. **Inventory every PDF everywhere in the vault** (every `.pdf` under `<vault>/`, excluding `.obsidian/`, etc.). Classify each as:
   - **Already canonical**: sits in the target folder's convention (`<TitleSlug>/paper.pdf` next to `<TitleSlug>/<slug>.md`). Leave alone.
   - **Stray in a paper-stash**: in `Work/Papers/papers/<subdir>/`, `Downloads/`, `~/Desktop/`, etc. Candidate for migration.
   - **Reference / unrelated**: in `Books/`, `Learning/`, etc. — usually leave alone unless the user says otherwise.
3. **For every stray PDF, identify the title**. The on-disk filename is often misleading (arXiv IDs, truncated names, vendor names like `paper.pdf` or `LEAP.pdf`). Use `pdftotext -f 1 -l 1` to grab page 1, take the first non-empty non-arXiv-banner line:
   ```bash
   pdftotext -f 1 -l 1 -q "<pdf>" - | head -40
   ```
   Cleanup: skip arXiv submission-banners (`arXiv:YYMM.NNNNNvN [cs.LG] DD Mon YYYY`), skip `Provided proper attribution is provided, …` (Vaswani 2017 boilerplate), and look for the actual paper title in caps or normal-case. If page 1 is ambiguous, try `-f 1 -l 2`.
4. **Slug it** per the conventions below; create `<Slug>/paper.pdf` *under the canonical folder*; verify the destination doesn't already exist (don't clobber a same-slug paper).
5. **Delete the source file** only after the destination is verified (size > 0). Move first, then `rm`, never `mv` on the same filesystem with no verification in between if the source path is the user's only copy.
6. **Update the source folder's index `.md`** to reflect the move (or delete it if it becomes empty placeholder content).
7. **Re-run verification** (existing `scripts/verify_wikilinks.py` if present, or `graph_stats.py` from `vault-knowledge-graph` sibling) to confirm no broken `![[...paper.pdf]]` embeds appeared.

**Important**: arXiv ID filenames (`2506.02153v2.pdf`, `2512.15943v1.pdf`) and org-stash subdirectories (`papers/anthropic/`, `papers/deepseek/`) are **organizational hints, not titles**. Don't infer the title from the folder name alone — always run `pdftotext` on the first page.

## Diagnose, propose, then execute (workflow preference)

Before mutating any PDFs / notes on a real filesystem, do this:

1. **Inventory first.** Walk the vault and print per-PDF: relative path, size, and adjacent-note status (does it sit next to a `.md` with matching slug? is its dir enumerated by a folder index?). Don't edit anything yet.
2. **Categorize the bulk**: usually 3-4 buckets (already-canonical, stray-but-movable, reference/unrelated, suspicious non-PDF co-located files). Present a table.
3. **Surface decisions the user needs to make**: deletions (empty files, mystery files like `rockyou.txt`), renames (truncated titles), folder naming conventions (camelCase vs kebab-case), what to do with the now-empty old folder (delete, tombstone, leave). Don't assume.
4. **Show a dry-run move table**: proposed `<source>` → `<dest>` for every file, with the proposed slug. Get explicit yes/no before any file moves.
5. **Execute step-by-step, verify after each batch**, and re-run the diagnostic at the end to confirm zero new broken wikilinks and zero unindexed PDFs.

The user has redirected bulk vault work mid-flight several times ("there should be only one paper dir"); they prefer the strategy table over surprises.

## Vault path resolution

Same rule as `obsidian-repo-notes`: `OBSIDIAN_VAULT_PATH` env var → `~/Documents/Obsidian Vault` → user-supplied path. Resolve to an **absolute path** before calling file tools; they don't expand `$VAR` or `~`.

Two storage conventions you'll see in this vault:

1. **Top-level papers collection** — `Research/Papers/` (this repo's convention; matches the existing `[[Papers]]` link in `Research/Research.md`)
2. **Per-lab sub-collection** — e.g. `Research/Papers/Moonshot AI/` if you want to scope a library to one lab

Pick the one the user implies, or default to (1).

## Authoritative source for a paper

Every paper must be **verified to actually belong to the lab/author** named. Author lists on arXiv abstracts can be ambiguous (different `Kimi Team` papers exist; some are independent). Use this verification chain:

1. **PDF header is ground truth.** The first ~500 chars of `https://arxiv.org/pdf/<ID>` will list authors + affiliations. Search for the affiliation string (e.g. `Moonshot AI`, `MIT`, `DeepSeek`) or author-team label (`Kimi Team`).
2. **Subtitle patterns.** Moonshot's papers often have `TECHNICAL REPORT OF <model>` in caps at the top of v2 PDFs.
3. **GitHub repo pointer** in the comments field on the abstract page is a strong corroborator: `arxiv.org/abs/<ID>` → look for the abstract page's "Comments" → `Github: https://github.com/<owner>/<repo>`.
4. **Red flags** to gate on:
   - arXiv abstract page shows the title but no author affiliations (common) — pull the PDF.
   - A paper **named** with a lab's brand but author list is different people (e.g. `K2-Think` was an independent academic group that named a model `K2-Think`; not Moonshot).
   - External follow-up analyses by other groups (e.g. `Optimizing Mixture of Block Attention` from MIT+NVIDIA isn't a Moonshot paper even though it builds on their `MoBA`).

Don't add papers to a lab's collection unless the affiliation is confirmed in the PDF or the arXiv comments field has a lab-internal repo.

## Folder + note naming convention

For every paper, create:

```
<vault>/<path>/Papers/<Slug>/<slug>.md
<vault>/<path>/Papers/<Slug>/paper.pdf
```

Where `<Slug>` is **kebab-case, ASCII-only, no spaces, no dots past the version**. Examples for Moonshot:

| Paper | Slug | Note filename |
|---|---|---|
| Kimi K1.5 | `Kimi-K1.5` | `kimi-k1.5.md` |
| Kimi K2 | `Kimi-K2` | `kimi-k2.md` |
| Kimi K2.5 | `Kimi-K2.5` | `kimi-k2.5.md` |
| Kimi-VL | `Kimi-VL` | `kimi-vl.md` |
| Kimi Linear | `Kimi-Linear` | `kimi-linear.md` |
| MoBA | `MoBA` | `moba.md` |
| Mooncake | `Mooncake` | `mooncake.md` |

**Slug rules**:
- Match the user's existing convention when extending (`Research/Papers/Kimi K1.5/` is what was here — keep the same shape for consistency, but use kebab-case elsewhere).
- If making a new `Papers/` from scratch, prefer kebab-case (no spaces) for predictable sort + Obsidian resolution.

## Note schema

Each per-paper note follows this skeleton:

```markdown
# <Display Name>

> arXiv: **[<ID>](https://arxiv.org/abs/<ID>)** — <Mon YYYY> (<vN> if not v1)
> Repo: <github URL>
> HF: <huggingface URL> (optional)
> PDF: ./paper.pdf

## The headline

One paragraph — what the paper says in 1–2 sentences. Cite the *paper's own claim*, not your interpretation.

## (Optional) Architecture / Mechanism / Reported numbers / etc.

Sections depend on paper type:
- **Model papers** → architecture, training recipe, reported numbers
- **Method papers** → mechanism (with a tiny ASCII diagram), reported numbers
- **Serving/systems papers** → architecture diagram, throughput / latency claims
- **Benchmarks** → what's measured, where the data comes from, baseline scores

## Connection to <lab> family / reading recommendations

Link to adjacent papers in the same collection. This is what builds the lineage.

## Related entries in this vault

- `[[Papers|Papers]]` — folder MOC (or `<lab-name>` MOC if scoped)
- One or two cross-refs to the most strongly related papers

## Open questions

Questions to revisit when you next read the paper. Helps the note earn its keep.

Part of [[Research]] (or whatever the parent MOC is).
```

Keep each note ~80–150 lines. Detail-rich beats prose-heavy.

## MOC structure

`Papers.md` (or `<lab-name>.md`) at the root of the collection:

1. **Numbered chronological list** of `[[wikilinks]]`, grouped by year.
2. **Reading-order lineages by topic** — separate `[[wikilinks]]` sections for architecture lineage, training-method lineage, etc. This is the durable value of the MOC.
3. **Convention notes** at the bottom — slug rules, how to add a paper, paper-not-a-paper policy.

**DO NOT** auto-cross-link between per-paper notes via `..` paths. Write `[[kimi-k2]]` (Obsidian resolves via recursive search). Path-prefixed links (`[[../Papers/kimi-k2]]`) work but break when notes move.

## Bulk download pattern

When the user says "download every paper by <lab>":

1. **Discover via the lab's blog page or GitHub org** (`https://<org>/blog`, `https://github.com/<org>`). Lab blogs (e.g. Kimi's research page, Anthropic's research page) are typically complete.
2. **Cross-reference arXiv**: for each candidate, find the arXiv ID via the lab website's `arxiv.org/abs/<ID>` links or by `site:arxiv.org <lab>` searches.
3. **Verify authorship per the chain above**.
4. **Concurrent curl downloads** — single batch script, not serial per-folder calls:

```bash
mkdir -p "<vault>/Research/Papers"
for pair in "Mooncake|2407.00079" "Muon|2502.16982"; do
  name="${pair%|*}"; id="${pair#*|}"
  mkdir -p "<vault>/Research/Papers/$name"
  curl -sSL -o "<vault>/Research/Papers/$name/paper.pdf" "https://arxiv.org/pdf/$id"
done
```

`curl` to arXiv's PDF endpoint is fast and reliable. Don't bother with `arxiv` Python libs unless they have a feature you need. See `scripts/fetch_arxiv_papers.py` for a Python equivalent with politeness delay.

## Wikilink verification — Obsidian semantics

The single biggest pitfall: writing notes that look fine to you but Obsidian can't resolve because you used filesystem paths.

**Obsidian's resolution model** (verified by hand-walking error cases):
- `[[X]]` looks for a file named `X.md` or folder `X/`:
  1. **Same folder** as the source note.
  2. **Sibling subfolders** under the source's parent.
  3. **Walks up** the folder chain until match or vault root.
- `[[../X]]` skips one level up (the **note-level** up, which usually ≈ filesystem `..`).
- **Aliases**: `[[Page Name|display text]]` works the same; resolution uses the part before `|`.

**Pitfall**: a naive `os.path.normpath` based checker will report false positives / negatives. Use the bundled `scripts/verify_wikilinks.py` which mimics Obsidian's recursive-name search.

**Bulk rewrite pattern**: when you need to fix wikilinks across many files (e.g. after slug rename), use `execute_code` with regex substitution in a single batch — NOT 50 individual `patch` calls. See `scripts/bulk_rewrite_wikilinks.py`.

```python
import os, re
base = "<vault>/Research/Papers"
for root, _, files in os.walk(base):
    for f in files:
        if not f.endswith(".md"): continue
        p = os.path.join(root, f)
        with open(p) as fh: content = fh.read()
        new = re.sub(r"\[\[\.\./Papers/Papers\]\]", "[[Papers]]", content)
        new = re.sub(r"\[\[\.\./papers/([\w\-\.]+)\]\]", r"[[\1]]", new)
        if new != content:
            with open(p, "w") as fh: fh.write(new)
```

This is dramatically faster than per-file edits and observes the user's convention.

## Pitfalls

- **ArXiv `YYMM.NNNNN` IDs encode year + month** — `2507 = Jul 2025`, `2602 = Feb 2026`, `2603 = Mar 2026`. Use this for chronological sorting without hitting arXiv for metadata.
- **PDF page-count reports from `file`/`pdfinfo` are accurate** but the **abstract-page number on HF papers-page** is often wildly different. Trust the PDF's actual page count.
- **"Preview" / "Letter" / "Tech Report"** suffixes in titles matter. A 3-page "Muon is Scalable" letter is the right one to download — there's no full paper edition.
- **Joint-author papers** (e.g. `Kimina-Prover` is Numina + Kimi Team) still belong in the lab's collection, with a note about the joint authorship.
- **Concurrency**: `curl -sSL` to arXiv is fine; arXiv tolerates ~1 RPS sustained. Don't parallelize with `&` for >10 simultaneous downloads. The bundled fetcher respects a 1.1 s delay.
- **De-duplicate by arXiv ID**, not by title. Same paper can have multiple versions (`v1`, `v2`, `v3`) — download the latest.
- **Don't auto-commit**: the vault may have no commits yet, or its git state may be unusual (detached HEAD, worktree). Leave the commit to the user unless they asked.
- **Verify authorship BEFORE downloading** — avoid burning bandwidth on decoys (e.g. academic clones like `K2-Think` that don't belong to the target lab).
- **Existing-paper-stash consolidation is a migration, not a download.** When the vault *already* has the PDFs (just in a junk-drawer location like `Work/Papers/papers/<org>/`), don't re-download from arXiv. Extract title from each on-disk PDF via `pdftotext -f 1 -l 1`, slug it, move to the canonical folder, and verify file size + first-page text matches expectations before deleting the original.
- **`pdftotext -f 1 -l 1` strips cleanly but page-1 titles can be misleading.** Some papers put the title in caps on page 2 (after the arXiv banner + abstract), some bury it under `Provided proper attribution is provided, …` copyright blocks. If page 1 is ambiguous, run `-f 1 -l 2` and look for the second non-banner block. Don't guess and slug it anyway — a wrong slug means a renamed PDF you can't grep for later.
- **Consolidation can pull non-paper artifacts into view.** A paper-stash folder often contains `*.zip`, `*.txt`, or whole non-PDF files (`rockyou.txt`, `10th Class-...zip` in one vault's deepseek stash). Surface these explicitly to the user as "found alongside the PDFs — also move or keep?" — don't decide unilaterally; they're often deletable noise but some are intentional (e.g. dataset files). After asking, treat them as out-of-scope for the paper-library workflow.

## Reference files

- `references/arxiv-author-verification.md` — patterns for identifying lab authorship from PDF headers across major Chinese / US labs (Moonshot, DeepSeek, Tsinghua, MIT-HAN-Lab, Allen AI, etc.)
- `references/obsidian-wikilink-semantics.md` — detailed resolution rules with worked examples; the source-of-truth for `verify_wikilinks.py`.

## Scripts

- `scripts/verify_wikilinks.py` — recursive walker that mimics Obsidian's `[[X]]` resolution across a vault subtree. Reports broken links + the file each broken link is in. Use this before telling the user "all wikilinks resolve".
- `scripts/bulk_rewrite_wikilinks.py` — regex-based bulk wikilink rewriter; used after slug renames across many notes.
- `scripts/fetch_arxiv_papers.py` — batch arXiv PDF downloader with concurrency control (≤1 RPS to arXiv) and per-paper PDF-magic-byte verification.
