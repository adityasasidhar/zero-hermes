# Deep enrichment — concrete recipes and gap categories

Companion to the deep-enrichment variant in SKILL.md. This file is the
"how exactly" companion: shell commands that pay off, and the categories of
defects worth surfacing in `### Notable details`.

## Inventory batch (run first, before reading any source file)

```bash
# Tracked file count + extension histogram
git ls-files | wc -l
git ls-files | python3 -c "
import sys
from collections import Counter
files = [x.strip() for x in sys.stdin if x.strip()]
print('tracked_files', len(files))
print(Counter((x.rsplit('.', 1)[-1].lower() if '.' in x else '[none]') for x in files))
"
```

```bash
# Source counts by language
git ls-files '*.py' '*.html' '*.js' '*.css' | wc -l
git ls-files 'tests/test_*.py'

# JS/Vite monorepo variant — for repos where `.jsx` / `.tsx` dominate.
# Run BOTH the Python and JS variants on mixed-stack repos; an extension
# histogram (Counter of suffixes) is the fastest way to see the shape.
git ls-files '*.jsx' '*.tsx' '*.js' '*.mjs' '*.cjs' | wc -l
git ls-files '*.json' '*.html' '*.css' '*.svg' | wc -l
```

```bash
# Recent commit shape (4-day sprint vs 3-year slow burn)
git log -5 --date=short --pretty=format:'%h%x09%ad%x09%s'
git log --oneline | wc -l
git log --since="6 months ago" --oneline | wc -l
```

```bash
# Branches + cleanliness
git branch -a
git rev-parse --show-toplevel

# Tracked deletions — reveal legacy code paths the README hides.
# Files deleted from the index but still referenced in `git log` are
# real "dead code" signals worth surfacing in Notable details.
git log --all --diff-filter=D --name-only --pretty=format: | \
  python3 -c "import sys; print('\n'.join(sorted(set(x.strip() for x in sys.stdin if x.strip()))))"
```

These commands answer most "what is this project" questions before you
read a single Python file. The extension histogram is the single highest-value
output — it tells you whether to expect a Flask app, a JS SPA, a notebook
pile, or a textbook-PDF-backed school app.

## Verification batch (run last, before final reply)

```bash
# Char + byte count
wc -c -m /tmp/enrich-wave<n>/<slug>.md

# All required sections present AND in the correct order. `grep -Fqx` alone
# confirms presence but not ordering — do BOTH checks.
python3 - <<'PY'
from pathlib import Path
import sys
p = Path('/tmp/enrich-wave<n>/<slug>.md')
s = p.read_text()
markers = [
  '## Overview', '## Status', '## Inside the Codebase',
  '### Architecture / How it works', '### File structure',
  '### Key files', '### Notable details', '### Tech stack',
  '### How to run it', '## What this project is about',
  "## Use cases / When you'd reach for this",
  '## Key Features', '## Getting Started', '## Stats', '## Links',
]
pos = [s.index(m) for m in markers]
ordered = pos == sorted(pos)
present = all(m in s for m in markers)
print('present', present, 'ordered', ordered)
sys.exit(0 if present and ordered else 1)
PY

# Wikilink footer present
grep -Fqx -- '- Part of: [[Public Repos]]' /tmp/enrich-wave<n>/<slug>.md

# Vault untouched — `git status --short` is the correct check, not `ls -la`.
test "$(git -C <vault_root> status --short -- Github/Repos/<slug>.md)" = ""
```

If any of those fail, the parent will bounce the work back. Catch it first.

### Python smoke test (no test suite but want a fast dependency-free pass)
```bash
# Package-level syntax check — proves the source tree compiles.
# Faster than `pytest -q` and works without `pytest` installed.
# Output is silent on success; any SyntaxError becomes the verifiable evidence.
python3 -m compileall -q <package_dir>
echo "compileall_exit=$?"
```
Use this when the repo has no tracked `tests/` and `compileall` is your
only fast cheap signal that the source tree is in a runnable state.
For pure-stdlib CLI tools this is usually enough; for GUI/desktop apps
(per the gui-smoke recipe) you need a forced headless driver instead.

## Categories of "reality gaps" worth surfacing in `### Notable details`

These are the kinds of cross-source contradictions that make a note
genuinely useful rather than a re-skin of the README. Look for them every
time.

### 1. Manifest vs imports
- Package imported but missing from `requirements.txt` / `pyproject.toml`.
  Example: `from flask_wtf.csrf import CSRFProtect` with no `Flask-WTF` line.
- Package pinned but never imported (e.g. security middleware wired into
  `requirements.txt` but `init_app` never called).
- Two packages that share an import namespace (e.g. `fpdf` and `fpdf2` both
  pinned — only one can actually be loaded).
- JS variant: an npm dependency declared in `package.json` but not imported
  anywhere in tracked source. The reverse (imported but undeclared) also
  surfaces here.

### 2. Code vs README
- README claims N tools/agents/handlers; source registration has N-k.
- README lists libraries or features that aren't actually wired up.
- Description (GitHub) says one thing; README says another; code says a third.
- README mentions a config file (e.g. `apikey.txt`) that the README's own
  code path no longer reads.
- JS variant: a deployed homepage URL advertised in the README/GitHub
  description but the `vite.config.js` / `package.json` proxy shows the
  client hardcodes a different backend host (typical: README says
  localhost, code defaults to Render/Vercel).

### 3. Dead or duplicate code
- Two files claiming to do the same job. One is the routed path; the other
  is a legacy copy that will mislead readers.
- `__pycache__/` checked in.
- `*.pyc` matching a Python version that doesn't match the README's stated
  version requirement.
- JS variant: tracked `*.svg` asset files from the Vite/React starter that
  are never referenced (`vite.svg`, `react.svg`) — leftover scaffolding,
  not "design assets".
- JS variant: leftover root-level files from an earlier monorepo layout
  (e.g. an empty `index.js` at repo root that imports nothing). The
  `git log --all --diff-filter=D` recipe above is the cheapest way to spot
  these.

### 4. Tracked secrets / hygiene
- `apikey.txt`, `.env`, `*.db` present in `git ls-files`. Even if the
  contents are placeholder, the *fact* that they're tracked is a real
  finding. Don't print the secret; print the *name* and the lesson.
- `.gitignore` excludes the file but git still tracks it (the `.gitignore`
  was added after the file was committed).

### 5. Configuration mismatches
- Frontend template expects shape `A → B → C`; the helper that feeds it
  produces `X → Y → Z`. The UI silently fails to cascade. Surface it.
- Severity / impact column says a feature is on; the route requires a
  flag that's never set.
- JS variant: the backend reads `process.env.PORT || 3000`, the frontend
  reads `import.meta.env.VITE_API_URL || <hardcoded-deploy-url>` —
  this is a common asymmetry where local dev needs the env override.

### 6. Test / CI gaps
- Tests exist but aren't pinned in the manifest (`import pytest` with no
  `pytest` in `requirements.txt`).
- Tests exist but `python3 -m pytest -q` fails because the env is missing
  even the top-level framework (e.g. ModuleNotFoundError at collection time).
  Capture the error verbatim in the `## Status` section.
- No `.github/workflows/` despite pinned lint/test tooling.
- JS variant: `package.json` `scripts.test` is the placeholder
  `"echo \"Error: no test specified\" && exit 1"` — that's the Vite/React
  starter default and should be called out in Status as "intentional
  placeholder, not a real test suite".

### 7. Manifest vs README prerequisites
- `pyproject.toml` declares `requires-python = ">=3.12"` but the README's
  "Prerequisites" section says "Python 3.10+". The manifest is the truth;
  the README is stale. Surface this in `### Notable details` and refuse to
  copy the README's lower number into `## Getting Started`.
- `pyproject.toml` lists eight direct dependencies but the README only
  mentions three ("Qwen, Llama, Mistral") by name; the unmentioned ones are
  still pinned and load at runtime. Capture the full list, not the README's
  abridged one.
- Common drift patterns: (a) README cites a major version ("Python 3.10+")
  while the manifest's floor moves up; (b) README mentions a model by name
  ("uses Qwen") but the code defaults to a different one (`qwen3:4b-thinking`
  vs the README's `qwen2.5-coder:7b` recommendation); (c) README shows a
  single install line that doesn't match the manifest's editable install
  (`pip install -e .`).

### 8. Tool-count reconciliation (CODE vs README)
- The README claims "20+ tools" or "40+ function tool registry"; the source
  fact is the size of the dispatch dict in `Agent._init_tools()` (or
  equivalent). Mechanical count, one-liner:
  ```bash
  grep -cE '^[[:space:]]*"[a-z_]+":' space/agent.py
  ```
  (the `ast.Dict` walk variant is more accurate but heavier):
  ```python
  python3 -c "import ast; t=ast.parse(open('space/agent.py').read());
  print(sum(isinstance(n,ast.Dict) for n in ast.walk(t)))"
  ```
  If the count diverges from the README, surface it in `### Notable details`
  ("README claims 20+; `agent.py` registers 17") and let the code's number
  win. The brief says trust the CODE; the count is the code's number.

### 9. License / governance
- README claims MIT/GPL/Apache; no `LICENSE` file is tracked.
- GitHub's API `/license` field is `null`.
- Private repo where the parent agent expects public.
- JS variant: backend `package.json` declares `"license": "ISC"` but the
  GitHub API reports no repository license. Same drift, different file.

## Reading source files in the right order

For a typical Python web app, this order beats "read the README, then read
`app.py"`:

1. `requirements.txt` — real dep tree with versions.
2. `app.py` (or `main.py`, `wsgi.py`) — entry point and factory.
3. One file per blueprint / module — top-level route signature only.
4. `models/` — entities and their relationships (also reveals the table names).
5. `src/` — domain logic, prompts, schemas.
6. `templates/base.html` — look for inline JS, MathJax, CDN URLs.
7. `tests/conftest.py` + one test file — shows what the author thought was
   worth covering.
8. `package.json` / `pyproject.toml` — only if the repo has both Python and JS.

For a JavaScript monorepo (Hono/Express backend + React/Vite frontend):

1. Root `package.json` — usually a placeholder; ignore the name field.
2. `backend/package.json` AND `backend/package-lock.json` — direct
   dependencies AND resolved versions from the lockfile.
3. `backend/src/index.{js,ts}` — entry point, routes, server bootstrap.
4. `backend/src/<service>.{js,ts}` — domain logic, prompts, Zod schemas.
5. `frontend/package.json` AND `frontend/package-lock.json` — frontend
   deps with locked versions.
6. `frontend/src/App.{jsx,tsx}` — state machine and submission flow.
7. `frontend/src/components/` — read the largest 2–3 components
   (controls, results, staging) and skip the trivial ones (Header,
   LoadingSpinner).
8. `frontend/src/App.css` (or `.module.css`) — responsive grid, themes,
   layout breakpoints.
9. `vite.config.js` / `eslint.config.js` — only if the frontend has
   non-default configuration.

Don't read all the templates — base + one feature page is enough. Don't
read every test file — one is enough. Don't read every React component —
the stateful root plus the 3 largest leaf components is enough.

## Research-baseline-as-sibling-package pattern

A common shape for ML / agent research repos is two sibling top-level
packages in one wheel: one is the *proposed* implementation, the other
is a *conventional baseline* the author used for comparison. Observed
in `LEAP` (`leap_agent/` proposed sequential-orchestration protocol +
`langchain_agent/` LangChain ReAct baseline). The pattern has three
recognizable markers:

1. **`pyproject.toml` declares ≥2 wheel packages:**
   ```toml
   [tool.hatch.build.targets.wheel]
   packages = ["langchain_agent", "leap_agent"]
   ```
   (or equivalent `setuptools.packages.find` / `[tool.setuptools]`).

2. **Two console scripts side-by-side**, often with a comment naming one
   as the baseline:
   ```toml
   [project.scripts]
   # Baseline ReAct Agent (for comparison)
   react-agent = "langchain_agent.main:main"
   # LEAP Agent (the research project)
   leap = "leap_agent.main:main"
   ```

3. **Mirror tool modules** at `<pkg>/tools/{file,shell,web,code,utility,...}_tools.py`
   — each side reimplements the same tool surface so the comparison is
   apples-to-apples. Often the baseline uses LangChain `@tool` decorators
   while the proposed package uses plain Python dict-returning functions.

When you see this pattern, the note's **Architecture** section must
explicitly call out the two-package comparison framing: which is
proposed, which is baseline, what they share, what they differ on
(filtering, state, prompt budget, model count). Don't write
"two-agent repo" — write "proposed sequential-orchestration protocol +
LangChain ReAct baseline; both share a tool surface; differ on
filtering, state, and model count". This framing IS the intellectual
contribution of the repo and is what makes the note useful vs. a
re-skin of the README.

For `### Tech stack`, list the deps that *only the baseline* uses
(`langchain`, `langchain-ollama`, `langgraph`) separately from the
proposed package's deps. The proposed package is intentionally lean
*because* it implements its own loop; the baseline's bloat is the
exact thing the proposed package is comparing against. A reader
should be able to count the line-item dep difference and immediately
see the comparison being made.

This pattern also feeds `### File structure`: the two packages get
their own bullets, and the *shared* tool surface is its own bullet
("`<pkg>/tools/` — mirrored across both packages so the comparison
is apples-to-apples"). Note that even when tool names match, the
*signatures* often differ — the baseline returns `str`, the proposed
returns `dict` (for the filter stage). Surface that asymmetry in
`### Notable details` ("baseline tools return `str`; LEAP tools
return `dict` so the sub-agent can filter by field").

## Inline JSON-schema introspection (Anthropic / OpenAI tool-use)

When a repo embeds a `tools=[{...}]` schema inline (Anthropic `tool_use`,
OpenAI `function_calling`, Pydantic `model_json_schema()`), the schema's
leaf-field count is the single most useful number for `### Key files` —
it tells the reader how much structured surface the model has to fill.
Counting by hand is unreliable; nested `properties` dicts with optional
sub-properties and arrays break naïve counting. Use a recursive leaf
walker over the parsed dict:

```bash
python3 - <<'PY'
import ast, pathlib
t = ast.parse(pathlib.Path('secondary.py').read_text())
# Pick the right module-level assignment by name
schema = next(
    n for n in t.body
    if isinstance(n, ast.Assign)
    and any(isinstance(tg, ast.Name) and tg.id == 'BALANCE_SHEET_TOOL' for tg in n.targets)
)
d = ast.literal_eval(schema.value)
def leaves(x):
    if not isinstance(x, dict): return 0
    if 'properties' in x:
        return sum(leaves(v) for v in x['properties'].values())
    if x.get('type') == 'array':
        return leaves(x['items']) if isinstance(x.get('items'), dict) else 1
    return 1
top = list(d['input_schema']['properties'])
print('top_groups', top)
print('leaf_fields', sum(leaves(v) for v in d['input_schema']['properties'].values()))
PY
```

Output reads, e.g., `top_groups ['metadata', 'stato_patrimoniale_attivo',
'stato_patrimoniale_passivo', 'conto_economico', 'nota_integrativa']
leaf_fields 149`. Quote both the group list and the leaf count in
`### Notable details` — the leaf count is what tells a reader whether
the schema is comprehensive (`~150 fields`) or skeletal (`~10`).

**Trap:** when iterating `ast.Module.body`, `ast.Assign` has no `name`
or `end_lineno` — they belong to `ast.FunctionDef` only. A naive
`(n.name, n.lineno, n.end_lineno)` tuple comprehension on a mixed
body crashes with `AttributeError` on the first `ast.Assign`. Filter
by `isinstance` first or use `getattr(n, 'name', '')`.

## Single-function smoke check (no venv, no imports)

When the project's deps aren't installed in your agent env but you
still want to verify a single pure-Python helper (validator, formatter,
calculator) without `pip install`-ing the full tree:

```python
import ast, pathlib
src = pathlib.Path('secondary.py').read_text()
tree = ast.parse(src)
node = next(n for n in tree.body
            if isinstance(n, ast.FunctionDef) and n.name == '_validate_and_calculate_python')
mod = ast.Module(body=[node], type_ignores=[])
ast.fix_missing_locations(mod)
ns = {'Dict': dict, 'Any': object, 'List': list}  # supply typing names it references
exec(compile(mod, 'secondary.py', 'exec'), ns)
result, errors = ns['_validate_and_calculate_python']({})
print(errors)
```

This runs *only* that one function (no third-party imports), so it
works without `anthropic` / `reportlab` / `docx` installed. Useful
for the validator-smoke pattern: feed empty data, confirm "balanced"
returns no errors — and then **flag in `### Notable details` that
empty-input balance is vacuous**, because both sides and the profit
figure reduce to zero and the equality check passes trivially. A
project that "balances on empty data" is not a project that extracts
correctly; the note must surface the distinction so a reader doesn't
mistake "the smoke check passed" for "the validator works."

## Lockfile parsing (cross-ecosystem)

For each ecosystem, the lockfile is the source of resolved versions.
Always paste both the declared range and the locked version in
`### Tech stack`.

### Python (uv)
```bash
# Quick visual dump of the lockfile (format and structure)
python3 -c "import tomllib; print(tomllib.load(open('uv.lock','rb')))"
```

For pyproject.toml-style projects (no plain `requirements.txt`), the canonical
two-step recipe splits declared ranges from locked versions:
```python
# Step 1: declared ranges from pyproject.toml
import tomllib, pathlib
d = tomllib.loads(pathlib.Path('pyproject.toml').read_text())
for dep in d['project']['dependencies']:
    print(dep)

# Step 2: locked versions from uv.lock — pull only the direct deps,
# not the transitive closure. Adjust the WANTED set to the manifest's
# `project.dependencies` list.
import tomllib
d = tomllib.load(open('uv.lock','rb'))
WANTED = {'crawl4ai','ddgs','langchain-ollama','mcp','ollama',
          'prompt-toolkit','rich','typer'}   # adjust to the repo
for p in d['package']:
    if p.get('name') in WANTED:
        print(p['name'], p['version'])
```
Use both lines in `### Tech stack` (`"typer>=0.21.0" (lock 0.21.0)`).

### Python (pip / requirements)
No lockfile by convention; pin the manifest line directly. If a `uv.lock`
coexists with a `requirements.txt`, prefer the lockfile — the manifest
is the source of declared ranges, the lock is the resolved graph.

### Rust
```bash
grep -A1 '^name = "<crate>"' Cargo.lock
```

### Node / npm
```python
import json
d = json.load(open('package-lock.json'))
root = d['packages']['']
deps = {**root.get('dependencies', {}), **root.get('devDependencies', {})}
for name in deps:
    declared = deps[name]
    locked = d['packages']['node_modules/'+name]['version']
    resolved = d['packages']['node_modules/'+name]['resolved']
    print(f'{name}\tdeclared={declared}\tlocked={locked}\tregistry={resolved}')
```

### Go
`go.sum` is the lockfile; `go.mod` declares the range.

## When to paste a code snippet vs paraphrase

- Paste a short snippet (3–10 lines) when the snippet *is* the insight: a
  Pydantic schema, a retry decorator, a factory pattern.
- Paraphrase when the snippet is long boilerplate; cite the file path
  instead.
- For pinned versions, always paste the real line from the manifest. Don't
  paraphrase "Flask 3.x" — say `Flask==3.1.2`.
- For the `### Tech stack` table, every version MUST come from
  `requirements.txt` / `package.json` / `pyproject.toml`. Never from the
  README's "Built with" prose.
