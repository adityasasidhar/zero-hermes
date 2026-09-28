# Docling + DocLang + LaTeX: empirical probe results

Captured 2026-07-23 from a hands-on probe of:

- `docling` 2.114.0
- `doclang` 0.7.3
- `saxonche` 13.0.0 (for Schematron validation, installed after first failure)

This is a **known-quirks** doc, not a tutorial. Use it when evaluating
this stack for a real pipeline (paper extraction, document RAG, etc.) —
or as a worked example of the methodology in
`library-behavior-probing/SKILL.md`.

## TL;DR

Docling's LaTeX backend + DocLang serializer **works end-to-end for
simple, single-file LaTeX** (sections, paragraphs, lists, tables,
equations, footnotes, code). It is **source-level parsing, not a TeX
engine** — so macros, cross-references, hyperrefs, figures, and
multi-file documents degrade or break. Spec is moving (blog post 404,
archive layout contradicts docstring). Treat as "good first attempt,"
not "production-ready for academic papers."

## The actual pipeline

```python
from docling.document_converter import DocumentConverter
converter = DocumentConverter()
result = converter.convert("paper.tex")
doclang_xml = result.document.export_to_doclang()        # → <doclang> XML
result.document.save_as_doclang_archive("paper.dclx")    # → OPC package
```

The wire is:

1. `docling.backend.latex_backend.LatexDocumentBackend` (3-line shim
   re-exporting `docling.backend.latex.LatexDocumentBackend`) parses
   the `.tex` source via AST/regex — **not** a `pdflatex` invocation.
2. Result is a `DoclingDocument` (the unified IR).
3. `DocLangDocSerializer` walks that IR and emits DocLang XML.
4. `save_as_doclang_archive` packs the XML + image assets into a `.dclx`
   (OPC / Open Packaging Conventions) archive.

## Verdict table — LaTeX → DocLang feature matrix

Verified by running two probes: a simple `sample.tex` (sections, lists,
table, math, footnote, bibliography) and a hard `sample_hard.tex`
(theorems, align, hyperref, verbatim, figure, missing `\ref`).

| LaTeX feature | What happens | Severity |
|---|---|---|
| `\section{...}` | Becomes `<heading level="N">` | ✅ good |
| Body text | `<text>` blocks | ✅ good |
| `\textbf`, `\emph` | Preserved as `**...**`, `*...*` markdown inside `<text>` (not DocLang `<b>`/`<i>`) | ⚠️ not ideal |
| Inline math `$...$` | Stays as raw LaTeX inside text blocks. DocLang has `<formula>`, but inline math doesn't use it | ⚠️ mixed |
| Display math `equation`, `\[ \]` | Becomes `<formula>` | ✅ |
| `align` environment | Becomes `<formula>` but raw LaTeX source is kept, not converted | ⚠️ |
| `\footnote` | Promoted to real `<footnote>` element | ✅ |
| `itemize` | Becomes `<list>` with `<ldiv/>` separators | ✅ |
| `tabular` | Becomes `<table>` with `<fcel/>` / `<nl/>` markers. **LaTeX backend doesn't run TableFormer** — no column-span or merged-cell detection | ⚠️ partial |
| `booktabs` (`\toprule` etc.) | Stripped silently, table still works | minor |
| `\cite{}` | Becomes plain text `[key]` — **not** a `<reference>` element | ⚠️ |
| `\ref{}` | **Damaged:** `\ref{lem:aux}` → `Lemma[lem:aux]` (braces eaten) | ❌ bug |
| `\href{}{}` | **Damaged:** `\href{url}{label}` → `urllabel` (space + brace eaten) | ❌ bug |
| `\includegraphics` | Renders caption as `<picture><caption>Image: name</caption></picture>` but **no image data flows through** | ❌ broken |
| `\input{other.tex}`, `\bibliography{}` | Not handled — only a single `.tex` file is parsed | ❌ unsupported |
| `\begin{verbatim}` | Becomes `<code>` with CDATA | ✅ |
| Custom envs (`theorem`, `lemma`, `proof`) | Treated as plain paragraphs, prefixed with `**Theorem.**` text | ⚠️ lossy |
| Title / author / `\maketitle` | Title becomes `<heading>`, author becomes `<text>` | ✅ |

## Validation pitfalls (real, verified)

`doclang.validate()` for Schematron requires an extra backend:

```bash
uv pip install saxonche     # ~700MB, Java-based Saxon-CHE Python binding
```

Even after installing it, **the Python API surface in 0.7.3 is rough**:

- `doclang.xsd_validation` is a **module**, not a callable. Calling it
  raises `TypeError: 'module' object is not callable`. The actual API is
  in a submodule and isn't obvious from the package `__init__`.
- `doclang.schematron.SchematronValidator.__init__` is `(*args, **kwargs)`
  — empty signature in 0.7.3. Calling it raises `TypeError: Protocols
  cannot be instantiated`. The Protocol is for downstream implementers,
  not for default use.

**Workaround:** write your own well-formedness check (lxml / `xml.etree`
parse + structural assertions) until the validate API stabilizes. The
generated XML is at minimum well-formed XML — you can verify that
without the broken Schematron path.

## `.dclx` archive — spec drift, real finding

The `save_as_doclang_archive` docstring claims:

> Picture and page images are always stored outside the markup, under
> `assets/` and `pages/` in the archive respectively.

The actual archive layout from the probe:

```
533 bytes  [Content_Types].xml
263 bytes  _rels/.rels
1441 bytes document.xml
```

**No `doclang.xml`, no `assets/`, no `pages/` directories.** The docstring
is stale — the shipped code uses `document.xml` as the part name, and
the Content_Types XML registers it as
`application/vnd.doclang.document+xml`. If you're writing a `.dclx`
parser, use the OPC relationships in `_rels/.rels`, not the docstring.

## Confidence scores are NaN for LaTeX input

```python
result.confidence.mean_score  # nan
result.confidence.layout_score # nan
result.confidence.table_score  # nan
```

This is **expected**, not a bug — Docling's quality report assumes a
per-page pipeline (PDF, image). LaTeX is single-shot source parsing, so
there are no per-page confidences. Don't report NaN as a quality issue.

## Versions and install

```bash
cd /tmp && uv venv doclang-probe --python 3.11
cd doclang-probe
uv pip install docling doclang
uv pip install saxonche   # only if you need Schematron validation
```

Sizes: `docling` installs ~3GB (torch, transformers, layout models) even
though the LaTeX backend doesn't use most of them. If you only need the
LaTeX → DocLang path, consider `docling-slim` (slimmer deps, check the
docs first).

`uv venv` quirk: the venv **IS** the dir you named. There's no `.venv`
subdir. Interpreter is `<dir>/bin/python`. Don't waste a round-trip on
`source .venv/bin/activate`.

## What to fix before shipping this for paper extraction

1. **Wrap LaTeX in pdflatex first, then parse the PDF** — if you have
   control of the build. The LaTeX backend will never match a real TeX
   engine on macro expansion, refs, hyperref, and figures. PDF path
   uses DocLayNet + TableFormer and is mature.
2. **Or normalize LaTeX pre-Docling**: pre-process to flatten
   `\ref`/`\href`/`\input` into a single self-contained file, expand
   common macros with `latexcodec`/`pylatexenc`. Then run Docling.
3. **Skip `.dclx` packaging for now** — use the raw DocLang XML string
   (`export_to_doclang()`). The archive format is in flux.
4. **Skip doclang.validate()** — roll your own XML well-formedness check
   until the validation API stabilizes past 0.7.x.

## Other findings worth remembering

- The official launch blog post (dated March 15, 2026) at
  `docling.ai/blog/20260315_00_the_latex_story` returned **404** when
  fetched 2026-07-23. The LaTeX backend story is a moving target.
- The repo `docling-project/docling` has issue #2885 titled
  "Support for parsing LaTeX (.tex) documents as structured input" —
  i.e. someone filed it as a **request**, but the LaTeX backend was
  actually shipped. Either the issue is stale or the backend is
  separate from what the issue author wanted.
- DocLang's own `export_to_doclang` lives in
  `docling_core.types.doc.document.DoclingDocument` — it's an
  in-memory transform, not bound to any specific input format. So in
  principle any Docling-supported input → DocLang works the same way.
