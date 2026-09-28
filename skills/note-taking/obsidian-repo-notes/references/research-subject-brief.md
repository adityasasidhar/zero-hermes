# Research-Subject Brief — non-repo waves (people, protocols, systems)

This is the workflow for vault-note waves whose **subject is not a GitHub repo**
but a real-world person, system, protocol, framework, or any other reference
subject. Where the repo variants (`scripts/generate_repo_notes.py` for the bulk
pass, `references/deep-enrichment-brief.md` for the code-reading pass) feed on
`gh api` and cloned trees, this variant feeds on **external authoritative URLs**
and a `Source log` table that every claim in the notes traces back to.

## When this triggers

A parent agent has scaffolded a folder of stub notes (e.g. `Protocol.md`,
`Source log.md`, `Concepts/*.md`) and now wants them substantively populated
from web sources. Typical prompt:

> "Populate the Bryan Johnson protocol data — there are scaffolded notes under
> `Personal Growth/bryan_routine/` (Protocol.md, Source log.md, Concepts/*.md).
> Write the populated versions to `/tmp/bj-wave1/Personal Growth/bryan_routine/...`
> — mirror the vault path under the staging folder. Never touch
> `/home/arctic/Documents/fun/...`. Use only external sources; cite every
> numeric claim in the source log."

The parent supplies a staging path that **mirrors the vault folder name-for-name**
(`/tmp/<wave>N/<vault-relative-path>/...`). You are a leaf subagent — do NOT
spawn further subagents.

## Hard rules

1. **Never touch the vault.** The vault path
   `~/Documents/fun/Personal Growth/<Subject>/...` is read-only for the entire
   wave. Write only to `/tmp/<wave>N/Personal Growth/<Subject>/...`. Same-name
   collision is the failure mode: a stray write to the vault path during a wave
   corrupts the parent's verify+merge step the same way it does for repo waves,
   but with the added danger that the staging mirror trick (`/tmp/<wave>N/...`)
   *looks* like a normal vault path. Verify with
   `git -C <vault_root> status --short -- Personal\ Growth/<Subject>/` (empty
   result) before printing the final summary.
2. **Source log is the only evidence channel.** Every claim in every concept
   note must trace to a row in `Source log.md`. No row = fabrication. The
   canonical row shape is:
   `| YYYY-MM-DD | https://... | summary of what was extracted | 1-5 | [[Concepts/X]], [[Concepts/Y]] |`
3. **Cross-cite, don't paraphrase.** Differences between sources are signal
   (something changed), not noise. When the canonical source and a third-party
   summary disagree, surface the disagreement in `## What we know` and write
   a new source-log row rather than overwriting the old one.
4. **No fabrication of the user's own counterpart notes.** If a note's `See also`
   references a sibling note the user hasn't written yet
   (e.g. `Personal Growth/Sleep.md`), link as
   `_(Personal Growth/Sleep not yet written)_ — your version`. Do NOT create
   the counterpart. The vault's `CLAUDE.md` makes empty files a deliberate
   placeholder convention.
5. **The `> **WARNING:**` blockquote is mandatory on every concept note when
   the subject is a person/protocol whose practices could be mimicked.**
   Biohackers, athletes, supplement regimes, wellness influencers, legal-but-risky
   treatments — every concept note opens with the resource gap
   ("Reference only. He has a $2M/year clinical team. You don't.") and the
   reminder to log n=1. This is non-negotiable; the user expects to be
   *reminded*, not assumed to know. Same shape applies to regimes with
   contraindications (pregnancy, drug interactions, fertility concerns) — call
   those out in the relevant concept note (`Supplements`, `Recovery`).

## Inputs (in priority order)

1. **The parent's BRIEF** (often inline in the user message for a one-shot wave).
2. **The scaffolded notes in the vault** at
   `~/Documents/fun/Personal Growth/<Subject>/...` — read to learn the
   schema (frontmatter, section order, warning banner shape, counterparty
   note names). Read-only.
3. **The author's own canonical source** — the subject's website, blog,
   paper, or published protocol. Use `web_search` + `web_extract` (and
   `browser_navigate` for interactive pages). Save full text via `web_extract`'s
   on-disk cache for re-reading the middle of long pages (`read_file` on the
   cached `.md`).
4. **Independent secondary sources** — third-party summaries that cite primary,
   ideally with cross-checks on numeric claims. The trust hierarchy is:
   (1) the subject's own published canonical source,
   (2) the subject's YouTube/podcast,
   (3) podcast interviews,
   (4) the subject's social media,
   (5) tabloid coverage — never use (5).
5. **A pre-snapshot** if the parent provides one (e.g.
   `/tmp/<wave>N/_pre_snapshot.json`) listing the target files with their
   pre-wave sizes — this is the parent's diff baseline.

## What to read in the staging path

If the parent dropped a pre-snapshot, read it first to learn the file list and
expected sizes. The staging folder should already be created with the right
shape; if not, mirror the vault path under `/tmp/<wave>N/` before starting.

For each concept file in the scaffold, read the vault version (`read_file`)
to learn: frontmatter schema, section order, which sections are stubbed vs
populated, and the exact language the user uses for cross-links (this vault
prefers compact cross-links like `[[bryan_routine/Protocol|Protocol]]`).

## Required body shape for concept notes

The vault's existing scaffold for a concept note (e.g.
`Personal Growth/bryan_routine/Concepts/Sleep.md`) is opinionated and should
be matched exactly:

```markdown
---
type: Concept
tags: [<topic>, <topic>, personal-growth, physical-fitness, bryan-johnson, sleep]
created: YYYY-MM-DD
last_updated: YYYY-MM-DD
status: populated | scaffolding
counterpart: null | ../Concepts/<X>
---

# <Author> — <Concept>

> **WARNING:** [one-sentence reminder of the resource gap and the n=1 logging expectation]

## What he/she/it does
[Concrete numbers, times, doses, items — every numeric claim cites a source-log row]

## What we know
[What's confidently supported, with citations inline]

## What we don't know
[Gaps in public information; things the subject does but doesn't share]

## What we'd adopt (and why)
[Ranked n=1 candidates with judgment — the user's actual decision surface]

## See also
- [[bryan_routine/Protocol|Protocol]] — high-level
- [[bryan_routine/Source log|Source log]] — provenance
- [[Concepts/<Sibling1>]] — short note on the cross-link
- [[Concepts/<Sibling2>]] — short note on the cross-link
- _<Counterpart> (Personal Growth/<X> not yet written)_ — your version
```

The Protocol note (the folder-level overview) follows a different shape: a
cadence block, a "daily shape" timeline, a "what's known to change
frequently" block, and a "how to read this folder" block — match whatever the
scaffolded `Protocol.md` already uses.

The Source log uses a table with five columns:
`| Date | Source URL | What we took from it | Trust level | Updated which notes |`.
Use `[[Concepts/X]]` wikilinks in the last column so a reader of the source
log can navigate to any updated note directly.

## Frontmatter (verbatim from scaffold + upgrades)

Match the scaffold's existing frontmatter exactly. Typical upgrades from
"scaffolding" to "populated":

- `status: scaffolding` → `status: populated`
- Add `last_updated: YYYY-MM-DD` (the wave date)
- Don't gratuitously rewrite tags that were already set in the scaffold

## Output protocol

After writing all the files, print exactly:

```
Wave: <wave-name> (e.g. bj-wave1)
Subject: <Subject>
Files written:
- /tmp/<wave>N/Personal Growth/<Subject>/Protocol.md (<bytes> bytes)
- /tmp/<wave>N/Personal Growth/<Subject>/Source log.md (<bytes> bytes)
- /tmp/<wave>N/Personal Growth/<Subject>/Concepts/<X>.md (<bytes> bytes)
- ... (one line per file)
Source log URL count: <n> (≥4 required)
Vault untouched: yes  (git status --short -- Personal Growth/<Subject>/ = empty)
```

Then STOP. Don't try to also copy the files to the vault — the parent owns
the verify+merge step. Don't write any other files. Don't edit the BRIEF.

## Verification checklist (parent runs, you self-check)

- [ ] All scaffolded files written to `/tmp/<wave>N/...`, NOT the vault
- [ ] `Source log.md` has ≥4 dated URL rows with trust levels and "Updated which notes"
- [ ] Every concept note opens with a `> **WARNING:**` blockquote (when subject is mimickable)
- [ ] Every concept note has `## What X does`, `## What we know`, `## What we don't know`, `## What we'd adopt (and why)`, `## See also`
- [ ] Every concept note's `## See also` includes both `Protocol` and `Source log`
- [ ] Every numeric claim in a concept note is traceable to a Source log row
- [ ] No counterpart note created on the user's behalf (only referenced as `(not yet written)`)
- [ ] `git -C <vault_root> status --short -- Personal\ Growth/<Subject>/` is empty
- [ ] All files non-trivially sized (3–6 KB for concept notes; ≥2 KB for the protocol overview)
- [ ] No README-raw markdown leaked (`![...](...)`, `<img>`, `[text](url)` link-only refs) — though this constraint is lighter than for repo notes since web-extracted prose is naturally cleaner

## Post-apply verification (parent-only step)

After copying the wave files into the vault, run the URL + numeric verification
described in `references/post-apply-verification.md` *before* declaring the wave
done. The brief's checklist above confirms the wave is *internally consistent*
(structure, sections, cross-links); the post-apply verification confirms the
wave is *externally grounded* (URLs resolve, numbers appear on the cited pages).
Both must pass. A wave with great structure but hallucinated URLs is a
citation costume, not a research note.

## Common pitfalls (specific to research-subject waves)

1. **Quote the URL once in the source log, not in every concept note.** A
   numeric claim cites the source log row (by date or by URL), not the URL
   itself. The source log is the single point of truth for what was extracted
   from where; a concept note that duplicates the URL in five places drifts
   when the canonical source rotates.
2. **Don't treat the canonical source as the only source.** The third-party
   summaries often catch *recent* updates (a 2026 stack change, a 2024 event
   the canonical source no longer highlights). Cross-cite at least one
   third-party per major topic — it's the only way to catch when "the latest"
   has rotated since the canonical was written.
3. **Don't collapse numeric ranges.** When the canonical says "150 minutes of
   moderate + 75 minutes of vigorous" (two distinct numbers), preserve the
   split. When it says "6 hr/week," that's a separate number, not the sum.
   Merging them loses the information the user's n=1 decision needs.
4. **Surface "what we don't know" honestly.** The subject's gaps are real
   findings — exact set/rep schemes, whether an algorithm is per-day or
   averaged across a week, what brand-versions actually contain. A note that
   pretends to know everything has fabricated parts; a note that lists gaps
   honestly is reusable.
5. **The "mirror-to-staging" path is a coordination trap.** Writing to
   `/tmp/bj-wave1/Personal Growth/bryan_routine/Source log.md` is one extra
   `cd` from the vault path `Personal Growth/bryan_routine/Source log.md`,
   and the bash completion on some systems autocompletes the vault path. Use
   absolute paths from the start (`/tmp/<wave>N/...`) and never `cd` into the
   staging folder without also verifying the current directory with `pwd`
   before any write tool.
6. **The pre-snapshot's `_pre_snapshot.json` is the parent's diff baseline, not
   an instruction.** If the parent provides a snapshot listing expected files
   and sizes, the parent's verifier checks that every file in the snapshot
   exists with reasonable size. A file the snapshot doesn't list but you
   create is harmless but invisible to the parent's verifier. A file in the
   snapshot you skip breaks the wave.
7. **Subagent URL citations can be hallucinated.** A subagent may write
   `https://blueprint.bryanjohnson.com/...` into a source-log row without
   ever fetching it — the prose around the URL reads plausible, but the
   page returns 404, the cited numbers don't actually appear on the page,
   or the URL path is wrong. This is *exactly* the failure mode the
   Source log is supposed to prevent. The parent agent must run a
   post-apply URL verification step before declaring the wave done — see
   `references/post-apply-verification.md` for the concrete recipe
   (HTTP-200 sweep over every URL in the Source log + a numeric spot-check
   that greps the primary source for the headline numbers and confirms
   they actually appear on the live page). A wave where every URL
   returns 200 AND every headline number is greppable in the live source
   is shippable; a wave where either check fails is *not*.
