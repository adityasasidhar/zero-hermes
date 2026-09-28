# Deep Enrichment: Fallback Chain When Sources Are Missing

Real repos in this vault routinely have multiple data sources missing
simultaneously. This file documents the verified fallback chain when
enriching a per-repo note in the wave workflow.

## The fallback chain (in priority order)

For each fact, walk down this chain until one yields a value:

1. **GitHub API** — `gh api repos/OWNER/SLUG`
   - `description`, `language`, `topics`, `created_at`, `pushed_at`, `stargazers_count`,
     `forks_count`, `visibility`, `size`, `license`, `default_branch`, `open_issues_count`
   - The GitHub `description` is the most-reliable one-line summary. **If it's null,
     don't fall back to inventing one — explicitly write `description: ""` in
     frontmatter and note "no GitHub description" in the body.**
2. **README** — `gh api repos/OWNER/SLUG/readme | base64 -d`
   - Returns 404 + `{"message":"Not Found"}` when there's no README. **Don't
     keep retrying with different flags** — that's the answer.
   - When present, clean it: strip badges, images, HTML, link-only refs
     (see `references/readme-cleaning.md`).
3. **Code-level signals** — read the actual source.
   - The `requirements.txt` / `pyproject.toml` / `package.json` is the ground
     truth for dependencies, even when the README's "Built with X" prose lies.
   - Module/class docstrings, README-as-comment in source, and `__doc__`
     strings sometimes hold a one-liner that functions as the project
     description.
   - **There is no universal "description fallback" inside the repo** —
     `pyproject.toml`'s `description` field is one signal, but bare-script
     repos (no `pyproject.toml`, just `requirements.txt`) have nothing.
4. **Existing vault note** — `~/Documents/.../Repos/<slug>.md`
   - The `description:` frontmatter field is whatever the parent generator
     put there (often empty for stub notes). Don't echo an empty field as
     if it were content.
   - The body of an existing note IS evidence — preserve any accurate claims
     verbatim, even if you can't find them in the README.

## Three observed patterns from this vault

### Pattern A: README 404 + GitHub description null (rare, observed in `lol-image-enhancement`)

- `gh api .../readme` → 404
- `gh api .../repos/...` → `description: null`
- No `pyproject.toml` exists
- The repo has a `requirements.txt` with three deps and nothing else

**What to do:** frontmatter `description:` stays empty. The Overview and
What-this-project-is-about sections are derived entirely from code-reading.
Quote real code (class names, file paths, training hyperparameters) and
call out the missing README explicitly so the reader knows the gap is
real, not a scraping failure.

### Pattern B: README present + GitHub description present (the common case)

- GitHub description → frontmatter `description:` (cleaned of any markdown)
- README first paragraph → blockquote tagline + Overview
- README "Features" / bullets → Key Features
- README install/run → Getting Started
- Code confirms all of the above; reconcile discrepancies explicitly

### Pattern C: README present but extremely thin (single paragraph, no install section)

- Infer install commands from the manifests (e.g. `pip install -r requirements.txt`)
- Build `### How to run it` and `## Getting Started` from manifests + code,
  not from a missing README section. Add a note: "README does not document
  install; commands inferred from requirements.txt and entry-point scripts."

## Verification before declaring a fallback complete

After falling back, the final note MUST:

- Have `Part of: [[Public Repos]]` or `[[Private Repos]]` in `## Links` (required).
- Explicitly mention what was missing ("no README", "no GitHub description",
  "no license file") in either `## Status` or `### Notable details` — not
  silently absent.
- Use real code-derived facts (file sizes, function/class names, exact
  hyperparameters) wherever possible. Even a 4-file, 100-line repo yields
  several pages of accurate content from code-reading.

### Pattern D: README 404 + GitHub description null + `pyproject.toml` IS the only doc (observed in `LEAP`)

- `gh api .../readme` → 404 (no README on `main`)
- `gh api .../repos/...` → `description: null`
- `pyproject.toml` has the canonical project description AND console-script table

**What to do:** the `pyproject.toml` `description` field goes verbatim
into the frontmatter `description:` (e.g. `"LEAP Protocol - Lightweight
Edge Agent Protocol + Baseline ReAct Agent"`). The `[project.scripts]`
table becomes the source for `### How to run it` and `## Getting Started`
— each `<entry> = "<module>:<func>"` becomes a runnable binary name
(`uv run <entry>`). This is the ONLY case where Getting Started
commands come from a manifest rather than a README; flag it explicitly
in `### Notable details` ("no README; run commands derived from
`pyproject.toml` `[project.scripts]` — `<name>` → `<module>:<func>`")
so the reader knows the provenance.

The Overview and "What this project is about" sections are still
derived from code-reading (class docstrings, module names, the
`LEAP.pdf` writeup if present) — never paraphrase the `description`
into multi-sentence prose without code confirmation. The brief
allows reconstruction from code, not from prose.

## Why this matters

The brief explicitly says "No fabrication." When the README is missing,
the temptation is to write prose that *sounds* like a description but is
actually invented. The fallback chain above forces you to either find a
real source or write a visibly empty/null field — never both-and-neither.
