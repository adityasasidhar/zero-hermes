---
name: multi-wiki-layouts
description: "Maintain multiple llm-wiki instances split by domain (e.g. aiml / learning / personal / work), each with its own SCHEMA.md and tag taxonomy. Covers parent-folder layout, cross-wiki linking, and per-session routing. Use when the user has multiple wikis, asks to set up multiple wikis, or asks which wiki a source belongs to."
metadata:
  hermes:
    tags: [wiki, knowledge-base, obsidian, multi-vault, organization]
    category: research
    related_skills: [llm-wiki, obsidian, vault-knowledge-graph]
---

# Multi-Wiki Layouts

When the user keeps separate knowledge bases by domain (research / work /
personal / learning), use one **llm-wiki instance per domain** under a
shared parent folder. Each instance is self-contained — own schema, own tag
taxonomy, own index. This skill captures the *layout, routing, and linking*
across instances. The mechanics of running a single wiki (ingest, query,
lint) live in the `llm-wiki` skill.

## When This Skill Activates

- User has or wants more than one wiki (one per domain)
- User asks to set up multiple wikis for different scopes
- User asks which wiki a source / question belongs to
- User asks to migrate a single wiki into a multi-wiki layout
- An existing setup mixes personal, work, and research content in one
  schema and the schemas are clearly diverging

## When NOT to Use

- One wiki, one domain, ~50 pages or fewer → just use `llm-wiki` directly
- Single-vault Obsidian workflow with mixed domains → use `obsidian` and
  lean on tags / folders for separation instead

## Why Multi-Wiki Beats One Big Schema

| Concern | Single schema | Per-domain wikis |
|---|---|---|
| Tag taxonomy | sprawling, ~50 tags, half unused per topic | 15–25 per wiki, all relevant |
| Frontmatter fields | one-size-fits-all (no `arxiv_id` in personal) | domain-shaped (`arxiv_id`, `status:`, `people:`, `private:`) |
| Privacy / scoping | `.gitignore` is coarse | per-folder coarse control |
| Cross-domain capture | forced into one tag | pointers + prose, no tag-collision |
| Index navigation | big section counts, slow | small per-section count |

The 50-page rule of thumb: until you hit ~50 pages, the overhead of multiple
schemas/indexes isn't worth it. Past that, per-domain wins.

## Standard Layout

```
<WIKI_ROOT>/                         ← parent folder, e.g. ~/wikis
├── README.md                        ← orientation map (one wiki per line)
├── <wiki-a>/                        ← e.g. aiml/
│   ├── SCHEMA.md                    ← domain-specific conventions + taxonomy
│   ├── index.md                     ← catalog (sectioned)
│   ├── log.md                       ← append-only action log
│   ├── _archive/                    ← optional, retired pages
│   └── raw/
│       ├── articles/                ← web articles, clippings
│       ├── papers/                  ← PDFs, arXiv
│       ├── transcripts/             ← meeting notes, interviews
│       └── assets/                  ← images, diagrams
│   ├── entities/                    ← layer 2: per-entity pages
│   ├── concepts/                    ← layer 2: per-concept pages
│   ├── comparisons/                 ← layer 2: side-by-side
│   └── queries/                     ← layer 2: filed query results
└── <wiki-b>/                        ← e.g. work/
    └── …
```

**WIKI_ROOT** is the parent folder. Each subfolder is a full wiki. The
`README.md` at the parent is the orientation map.

## Top-Level README.md Template

The parent README is **not** a duplicate of any single wiki's schema. It's
an orientation map only.

```markdown
# Wikis

> N llm-wiki instances under this folder, one per domain.
> Schemas and tag taxonomies differ per wiki — what fits one doesn't fit all.

| Wiki | Path | Domain | When to use it |
|---|---|---|---|
| <name> | <path> | <one-line domain> | <one-line when-to-use> |

## Cross-wiki linking

Wikilinks resolve inside ONE wiki folder. Cross-wiki references use:

- `[[<wiki>/<page>]]` (qualified) — works in Obsidian if WIKI_ROOT is the
  vault root; works in plain editors too
- Prose with a wikilink — works everywhere, less clickable

Pick ONE convention and write it here so future agents don't guess.
```

## Per-Wiki Setup (repeat for each subfolder)

For mechanics of one wiki (`SCHEMA.md`, `index.md`, `log.md`, raw layout),
load the `llm-wiki` skill. The differences in a multi-wiki setup are:

1. **Folder lives under `WIKI_ROOT/<name>/`** instead of `~/<name>/`
2. **`README.md` at WIKI_ROOT replaces per-wiki orientation** — but each
   wiki still has its own `SCHEMA.md`, `index.md`, `log.md`
3. **Tag taxonomies are explicitly distinct** — don't copy-paste between
   wikis. Add only tags you'd actually use in that domain
4. **Frontmatter fields vary by domain** — `arxiv_id:` only in research
   wikis; `status:` + `people:` + `repo:` only in work; `private:`
   default true in personal
5. **`log.md` gets its own creation entry** — not a shared log

## Cross-Wiki Linking

Wikilinks resolve within a single wiki folder. Pick one convention and stick
to it (document in `WIKI_ROOT/README.md`):

| Convention | Pros | Cons |
|---|---|---|
| `[[<wiki>/<page>]]` (qualified) | Clickable in Obsidian when WIKI_ROOT is the vault root, readable everywhere | Slightly verbose |
| Prose: "see [[page]] in the `<wiki>` wiki" | Works in any editor | Not clickable in plain editors |
| Drop cross-references entirely | Simpler; you write the link in prose | Loses one of the wiki's main affordances |

**Default recommendation:** qualified form `[[aiml/transformer-architecture]]`
when `WIKI_ROOT` is also the Obsidian vault root.

## Per-Session Routing

When ingesting a source or answering a question:

1. Read `WIKI_ROOT/README.md` first to map the wiki landscape
2. For each candidate wiki, read its `SCHEMA.md` to match domain
3. Pick the wiki whose domain statement matches the source/topic
4. Inside that wiki, follow normal `llm-wiki` mechanics (orient, search,
   create/update)
5. If the source legitimately spans domains, ingest into the PRIMARY wiki
   and add a one-line pointer in the others — don't full-duplicate

**When ambiguous:** ask the user which wiki. Don't guess. Bad routing
causes page moves later.

## Resuming in a Multi-Wiki Setup

You do NOT have to orient into every wiki on every session. Orient into
only the wikis you'll touch:

1. Skim `WIKI_ROOT/README.md` to identify candidates
2. For each candidate wiki you'll touch:
   - Read `SCHEMA.md`
   - Read `index.md`
   - Scan last 20–30 lines of `log.md`
3. For larger setups (5+ wikis, 100+ pages each), also `search_files` for
   the topic at hand before creating anything new

**Cost:** a session that touches 3 wikis pays 3 orientation reads, not N+1.

## Common Wikis to Pre-Provision

When setting up multiple wikis for a new user, common splits:

- **aiml / research** — papers, models, techniques, benchmarks
- **learning** — courses, books, study notes, TILs
- **work** — projects, meetings, coworkers, tooling
- **personal** — life, ideas, side projects, capture

Not all four are needed. Start with 2–3, add more only when domains
genuinely diverge.

## Wikis Living Inside an Obsidian Vault

When `WIKI_ROOT` is a subfolder of an existing Obsidian vault (e.g.
`<vault>/wikis/` inside the personal vault), the wikis inherit the vault's
environment. This is a real, supported configuration — some users merge
the two so Obsidian browses both personal notes and research wikis in one
place. (This skill's author has run this since 2026-07-18: four llm-wiki
instances live at `~/Documents/fun/wikis/` inside the personal Obsidian
vault.)

**What changes vs a standalone `WIKI_ROOT`:**

- **Obsidian features apply to wikis automatically.** Wikilinks, Graph
  View, backlinks pane, Dataview, Excalidraw all work on wiki pages with
  no extra setup. Wikis appear in the vault's file browser.
- **Graph View includes wiki nodes.** A vault with 100 personal notes + 4
  wikis of 50 notes each shows ~300 nodes by default. To isolate: Graph
  View settings → Filters → add a path filter excluding `wikis/` (or
  include only the wikis).
- **Git tracking is shared.** A wiki edit becomes a vault edit. Every
  wiki `log.md` append, index update, etc. is a vault commit. Decide
  upfront whether wiki history belongs in the vault's repo. If not, add
  `wikis/` to the vault's `.gitignore`.
- **`raw/` lives inside the vault.** PDFs, audio, images downloaded as
  sources sit inside the vault. They count toward any attachment sync,
  file-size budget, or git history.
- **Cross-wiki linking becomes natural.** `[[<wiki>/<page>]]` resolves
  in Obsidian because `WIKI_ROOT` is inside the vault root. Example:
  `[[aiml/transformer-architecture]]` from a personal note clicks
  through. This makes the qualified form in [Cross-Wiki
  Linking](#cross-wiki-linking) the natural choice rather than a workaround.
- **Vault-wide settings apply to wikis.** Plugins, Obsidian Sync, file
  watchers, attachment folder — all vault-scoped. A wiki that wants
  different behavior from the rest of the vault must be a *separate*
  Obsidian vault, not a subfolder.

**Setup steps:**

1. Decide whether `wikis/` is gitignored or tracked (one decision, not
   per-wiki). If gitignored, add `wikis/` to the vault's `.gitignore`.
2. In `wikis/README.md`, document the cross-wiki convention you're using.
   Qualified `[[<wiki>/<page>]]` is the natural fit when wikis live
   inside a vault.
3. If you want Graph View to filter wikis in or out, configure path
   filters under vault Settings → Graph View → Filters.

**When to do this:**

- You want one Obsidian app window with everything.
- The vault is not git-tracked, or you're fine with wiki churn in its
  history.
- You don't need different Obsidian plugins per wiki.

**When NOT to do this:**

- The vault is heavily git-tracked AND wiki churn would create noisy
  history. Keep wikis at a separate path (`~/wikis/`) and open as a
  second Obsidian vault instead.
- You need different Obsidian Sync configurations per wiki (separate
  vaults have independent sync).
- You want different plugins per wiki (plugins are vault-wide).

## Pitfalls

See `references/cross-wiki-ingest-pitfalls.md` for the full list with session
evidence — the "move files into canonical subfolders?" decision rule,
cross-wiki citation patterns (qualified wikilink vs absolute path), the
source-backed-vs-fabricated TIL hard rule with `raw/articles/<source>-tracker.md`
as staging area, the `patch` tool's section-boundary bug, total-pages counter
gotchas, bidirectional concept cross-linking, and same-day log entries.

Brief highlights:

- **Tag pollution across wikis.** A tag valid in one wiki (`rlhf`)
  doesn't exist in another. Don't copy page templates between wikis without
  rewriting tags to the destination's `SCHEMA.md` taxonomy.
- **Schema copy-paste.** Tag taxonomy and frontmatter fields MUST be
  customized per domain. A copy-pasted SCHEMA.md defeats the whole purpose.
- **Cross-wiki wikilinks silently break.** In plain editors, `[[page]]`
  looks the same whether `page` exists or not. Periodically lint for
  broken cross-references.
- **Going multi-wiki too early.** Two wikis with five pages each is
  overhead. Wait until ~50 pages or schemas genuinely diverge.
- **Bad routing causes page moves.** If you put a work-flavored page in
  the personal wiki, you'll move it later — that breaks links. Ask when
  routing is ambiguous.
- **Shared `log.md`** at the parent folder breaks the per-wiki action
  history. Keep logs per-wiki.
- **Forgetting the parent README.** Without `WIKI_ROOT/README.md`, future
  agents (and future you) have to re-discover the layout. Write it once.

## Related Skills

- **`llm-wiki`** — single-wiki mechanics (ingest, query, lint). Multi-wiki
  setup uses this per subfolder.
- **`obsidian`** — Obsidian-specific vault operations. If `WIKI_ROOT` is
  also an Obsidian vault, these compose.
- **`vault-knowledge-graph`** — bulk enrichment of a single vault. Pair
  with multi-wiki layout when scaling past one vault.
