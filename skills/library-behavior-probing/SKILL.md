---
name: library-behavior-probing
description: "Empirically investigate the actual behavior of a fast-moving open-source library, CLI tool, or emerging open standard — instead of trusting the launch blog post, the README, or the marketing page. Covers the full probe loop: install the real binary into an isolated venv, write a minimal end-to-end script that exercises the feature, validate output against the claimed spec, and report a verdict table with caveats. Use when the user asks 'does X really do Y', 'how well does X handle Y format', 'is X production-ready', or when the stack is <1 year old, has 404'd docs, or where marketing claims exceed implementation reality. Loads alongside domain-specific skills (Docling, vLLM, llama.cpp, etc.) for context, but this is the methodology."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos]
metadata:
  hermes:
    tags: [Probing, Verification, OpenSource, Empirical, Research, Docs]
    related_skills: [live-model-benchmarking, spike, paper-equation-implementation, ocr-and-documents]
---

# Library / Tool Behavior Probing

When the user asks *"does X really do Y"* about a fast-moving open-source
project — emerging standards (DocLang, OpenTelemetry profiles), libraries
in active development (Docling, llama.cpp, vLLM), or any tool where the
launch blog is fresher than the README — **probe the real binary** and
report what it actually does. Don't paraphrase the docs.

This is the library/tool equivalent of `live-model-benchmarking` (which
covers the same philosophy for LLM providers). The two share a core
belief: **measure, don't trust vendor claims.** They differ in artifact
shape (a probe script + caveats table for libraries; streaming harness
+ tok/s for models).

## When to use

Trigger phrases:
- "does X really do Y", "how well does X handle Y format"
- "is X production-ready", "what does X actually do"
- "I want to research X", "check whether X supports Y"
- "compare X and Y" (when one is new enough that comparisons should be
  empirical, not just doc-based)

## The probe loop (5 steps)

### 1. Clarify the question direction before burning 20 tool calls

Ambiguous phrasing like *"how well does A's backend handle B"* can mean
three different things:
- A is **input**, B is **output** (does A produce B?)
- B is **input**, A is **output** (does A accept B?)
- **Side-by-side** comparison

Use `clarify` with the actual options BEFORE starting. A wrong direction
will burn time on the wrong conversion. (User confirmed this flow 2026-07
during the Docling/DocLang/LaTeX investigation.)

### 2. Install into an isolated venv, NOT the system Python

Use `uv venv` + `uv pip install` into a scratch dir. **Never** use the
active project's venv — you don't want probe artifacts polluting a real
codebase. Aditya's hard rule (from `live-model-benchmarking`): scratch
test artifacts go in `/home/arctic/hermes/` or `/tmp/`, never in project
repos.

```bash
cd /tmp && uv venv probe-venv --python 3.11
cd probe-venv
uv pip install <package1> <package2> ...    # note: uv venv IS the dir, no .venv subdir
```

**`uv venv` quirk:** with `uv venv <dir>`, the venv IS that directory;
there is no `.venv` subdir. The Python interpreter is at
`<dir>/bin/python`. Don't waste a round-trip trying `source .venv/bin/activate`.

If validation needs an optional backend (e.g. Schematron for DocLang
needs `saxonche`), install it explicitly and expect the default install
to break that path.

### 3. Map the surface — `dir()`, `OutputFormat`, `InputFormat`, public methods

Before writing a probe, list what the library actually exposes.
Concretely:

```python
from <lib> import <main_class>
import inspect
print([x for x in dir(<main_class>) if not x.startswith('_')])
```

For Docling-class libraries this means enumerating `InputFormat` /
`OutputFormat` enums and the `export_to_*` / `save_as_*` methods on the
in-memory document class. **If the surface claims to support X, find
the exact method name and where it lives** — it's often in a sub-module
(e.g. `docling.backend.xml.doclang_backend`, not `docling.backend.doclang_backend`).

### 4. Write a deterministic end-to-end probe (NOT a one-liner)

A one-liner hides bugs. Write a script that:

1. Builds a converter with explicit `format_options`
2. Runs the actual conversion
3. Inspects the in-memory doc (item counts, structure tree)
4. Calls **every** relevant `export_to_*` / `save_as_*` method and writes
   each to a file
5. Validates the output against the spec (XSD, schema, manifest)
6. Catches and reports failures per-output-format (so you can see which
   half works)

```python
for fmt_name, exporter in [
    ("markdown", doc.export_to_markdown),
    ("doctags",  doc.export_to_doctags),
    ("doclang",  doc.export_to_doclang),
]:
    try:
        out = exporter()
        (OUT / f"sample.{fmt_name}.txt").write_text(out)
        print(f"[+] {fmt_name}: {len(out)} chars")
    except Exception as e:
        print(f"[!] {fmt_name} FAILED: {type(e).__name__}: {e}")
```

**For things the docs claim but the code path is uncertain**, trace the
claim to source before reporting it. Search the package for the symbol
the docs reference (`search_files pattern="OutputFormat\.DOCLANG" path=...`).
If the symbol lives in a shim file (e.g. `latex_backend.py` is 3 lines,
just `from .latex import LatexDocumentBackend`), the impl is somewhere
else — find it.

### 5. Stress with a harder sample, then report a verdict table

The first sample proves the happy path. The second sample exposes
fragility:
- Cross-references and hyperrefs (often broken in source-level parsers)
- Multi-file / `\input` / `\bibliography` (often unsupported)
- Custom environments (theorem, lemma, proof)
- Embedded media (figures, audio)
- Edge cases: empty input, missing references, special chars

Then write a **verdict table**:

| Feature | Behavior | Severity |
|---|---|---|
| ... | ... | ✅ / ⚠️ / ❌ |

Follow with a **TL;DR** that's safe to quote in the user's own notes,
and a **"what you'd need to fix"** section if the answer is "works but
fragile."

## Gotchas / pitfalls

- **Marketing pages claim more than the code.** Read the docs as a hint,
  not a contract. Test the actual surface. (Verified 2026-07: Docling's
  docs list LaTeX and DocLang as supported; both work, but the LaTeX
  backend degrades cross-refs, hyperrefs, and figures.)
- **Spec drift.** Docstrings and even the spec repo can lag the actual
  shipped code. When you find a contradiction (e.g.
  `save_as_doclang_archive` docstring says `doclang.xml`, archive actually
  contains `document.xml`), **report it** — don't paper over it.
- **Optional validation backends.** Schematron / XSD / pyo3-style
  validation often needs a separate `pip install <pkg>[extra]`. The
  default install will raise `ModuleNotFoundError` mid-call. Install it
  up-front, not when you hit the error.
- **NaN confidence scores are normal for non-paginated inputs.** If the
  library's quality report assumes per-page pipeline (Docling does for
  PDFs), it returns NaN for source-level inputs. Don't treat that as a
  bug — note it and move on.
- **404'd blog posts are a signal.** If the project's launch blog is a
  404, the project is in active flux. Probe carefully and version-pin
  everything.
- **Don't trust `assertEqual` against the doc.** Assert against an
  independently-derived expectation (hand-rolled expected output, or a
  cross-tool oracle). Otherwise you're testing that the implementation
  matches itself.

## Report format the user wants

For research-curious users like Aditya:
1. **Headline verdict** in 1-2 sentences up top ("works / fragile / broken")
2. **End-to-end proof** — minimal working code, run it, show it succeeded
3. **Caveats table** with severity (✅ / ⚠️ / ❌)
4. **What you'd need to fix** — concrete blockers, not abstract concerns
5. **TL;DR block** safe to copy into Obsidian / a paper note

## Support files

- `references/docling-doclang-latex.md` — concrete probe results from
  the 2026-07 Docling + DocLang + LaTeX investigation. Use as a worked
  example of the methodology and as a known-quirks list for the
  Docling/DocLang stack.
