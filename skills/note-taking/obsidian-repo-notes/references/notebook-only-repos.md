# Notebook-only repos — inventory, run verification, note structure

Companion to `deep-enrichment-recipes.md` and `tiny-repo-enrichment.md` for the
specific edge case where the repository's only meaningful source file is a
Jupyter notebook (or a folder of them). The tiny-repo playbook assumes a
runnable language source (`*.py`, `*.rs`, `*.ts`); notebook-only repos have a
different shape and a different set of "what to look at" steps.

## When to use this file

- `git ls-files '*.py' '*.rs' '*.ts' '*.js' '*.go' | wc -l` returns 0 (or near-zero).
- `git ls-files '*.ipynb' | wc -l` returns ≥ 1.
- The GitHub API reports `"language": "Jupyter Notebook"`.
- `gh api repos/OWNER/SLUG/readme` returns 404 (common — many learning repos skip the README entirely).
- The "main entry point" is a `.ipynb` cell, not a function call.

Examples observed in the wild: `adityasasidhar/reinforcing-myself-with-reinforcement-learning`
(this session), `*-bonus-unit*` Colab exports, ML-Agents tutorial captures,
anything where the author "ran it in Colab, exported the .ipynb, committed, never went back."

## What the repo actually contains

Almost nothing:

- `notebooks/<unit>/<unit>.ipynb` — the only meaningful tracked file.
- `.gitignore` — usually excludes `.venv`, `.idea`, `__pycache__`.
- Everything else the user sees (the framework checkout, the environment binary, the YAML config, the trained weights, the ONNX export) is created or downloaded *during execution* of the notebook cells, not tracked.

This is a real finding worth surfacing in `### File structure` and `### Notable details`: the tracked tree is 2 files (one notebook, one ignore), but the *runtime* tree during a session is dozens of files and several hundred MB.

## Inventory batch (notebook-first variant)

```bash
# Notebook count + cell-type histogram. The numbers you want:
#   cells=    total cell count
#   code=     code cells
#   md=       markdown cells
#   executed= cells with non-null execution_count (proves the notebook ran)
git ls-files '*.ipynb'
python3 - <<'PY'
import json
from pathlib import Path
for nb_path in sorted(Path('.').rglob('*.ipynb')):
    if not nb_path.is_file(): continue
    try: nb = json.loads(nb_path.read_text())
    except Exception as e: print(f'{nb_path} UNREADABLE: {e}'); continue
    cells = nb.get('cells', [])
    n_code = sum(c.get('cell_type') == 'code' for c in cells)
    n_md   = sum(c.get('cell_type') == 'markdown' for c in cells)
    n_exec = sum(c.get('execution_count') is not None for c in cells)
    print(f'{nb_path} cells={len(cells)} code={n_code} md={n_md} executed={n_exec}')
PY

# Whether the notebook has been run before — execution_count and outputs
# are the cheapest signal of "the author actually executed this" vs
# "they exported straight from Colab without running."
python3 - <<'PY'
import json
from pathlib import Path
for nb_path in sorted(Path('.').rglob('*.ipynb')):
    if not nb_path.is_file(): continue
    nb = json.loads(nb_path.read_text())
    has_outputs = any(c.get('cell_type') == 'code' and c.get('outputs') for c in nb.get('cells', []))
    metadata = nb.get('metadata', {})
    colab = metadata.get('colab', {})
    print(f'{nb_path} outputs={has_outputs} '
          f'private_outputs={colab.get("private_outputs")} '
          f'gpu={metadata.get("accelerator")}')
PY

# Notebook-language size on GitHub — surfaces that the entire repo's
# "Jupyter Notebook" line is one big file.
gh api repos/OWNER/SLUG/languages
git ls-files '*.ipynb' | xargs wc -c

# Commit history shape — notebook repos often look like:
#   1. "wddwd" / "initial commit" (just .gitignore)
#   2. "Created using Colab" (the .ipynb)
#   3. merge commit
# Two "Created using Colab" commits with identical trees are a real
# finding — the author exported twice from Colab and merged the duplicate.
git log --oneline --decorate --all
git log --format='%H %T %s' --reverse
git log --since="6 months ago" --oneline | wc -l
```

The `git log --format='%H %T %s'` line is the cheapest way to detect identical-tree commits: compare the middle `%T` column for consecutive "Created using Colab" entries. When two are identical, that's a duplication finding worth surfacing in `### Notable details`.

## Reading the notebook directly

`read_file` on a 100k+ char notebook shows a truncated preview that drops outputs. Use the bundled `scripts/extract_notebook_outputs.py`:

```bash
python3 ~/.hermes/skills/note-taking/obsidian-repo-notes/scripts/extract_notebook_outputs.py \
    /home/arctic/projects/<slug>
```

That script prints cell-by-cell source text + code-cell outputs (stream, execute_result, error). Use it to:

- Capture recorded run results (loss traces, accuracy, predictions) into `### Notable details` — these are the *actual evidence* of training.
- Detect notebooks that have never been executed (`execution_count` is null on every code cell). This is normal for course-material exports but worth stating in `## Status`.
- Spot the "one cell with the YAML config" pattern: a code cell that contains a multi-line string starting with `behaviors:` is the entire behavior configuration. Pull it verbatim into `### Key files`.

## Reproducing the run — DON'T

A notebook that uses `git clone --depth 1 https://github.com/<framework>` and then runs a 2-million-step training job cannot be re-executed in this environment. It needs:

- GPU runtime (Colab or equivalent)
- A compatible Python/framework stack matching the framework's `python_requires`
- The compiled environment executable (binary, OS-specific)
- Tens of minutes to hours of wall time
- Network access to Hugging Face Hub for the publish step

Do **not** invent a "verified run" claim. The note should say something like:

> "Training was not rerun for this note: it requires external downloads, the environment binary, a compatible framework stack, and a GPU-scale N-step job, while the commit contains no saved outputs."

That sentence (or a variant) belongs in `### How to run it` and/or `## Status`. The `### How to run it` section's actual command should be the cell-level invocation the notebook intends to run, not a wrapper the agent invents.

## Note structure for notebook-only repos

When the repo is one notebook, the `### Architecture / How it works` section should describe the *flow* the notebook orchestrates, not the algorithm. The 6-section "Inside the Codebase" pattern still applies:

1. **Architecture** — number the runtime steps the notebook walks through (1: clone framework, 2: install Python+deps, 3: download env executable, 4: write YAML config, 5: launch trainer, 6: publish to Hub). Treat the notebook as a *linear orchestration script*; the cell-by-cell division is implicit, not the architecture.
2. **File structure** — the tracked tree (`notebooks/...`, `.gitignore`) plus a bullet called "Runtime-only — <list of files the notebook creates during a session>" and an "Absent" bullet naming what the repo deliberately doesn't track (weights, logs, configs, binaries, README, manifests, tests, CI).
3. **Key files** — the notebook itself plus any embedded YAML cell. The embedded YAML cell *is* a key file even though it's a string literal inside cell source — that's what the trainer reads, not a separate `Huggy.yaml` on disk.
4. **Notable details** — at minimum:
   - Pinned vs unpinned deps (Python version pinned; framework commit unpinned)
   - Recorded results vs absence thereof (`execution_count` nulls)
   - Reproducibility gap (shallow clone + mutable Miniconda URL)
   - Run-naming drift (train `--run-id=X` vs publish `--run-id=Y`)
   - History shape (two identical-tree "Created using Colab" commits)
   - Course snapshot attribution when the notebook credits another author
5. **Tech stack** — list the interpreter, framework, environment, distribution tool, and explicitly note "no library versions declared in a manifest." The framework's version is whatever HEAD of its default branch resolves to at clone time; do not invent a version number.
6. **How to run it** — the cell-level command the notebook actually runs (`mlagents-learn ...`, `!wget ...`, `!pip3 install -e ...`). Skip the long heredoc-style setup block — name the steps in prose and quote the single load-bearing training command in one code fence.

## Filling the byte budget honestly

Notebook-only repos fight the 5–10k byte floor because the source is one file. The depth has to come from these places, in priority order:

1. **The full cell-by-cell walkthrough** — describe what each of the 47 cells does (group related cells: cells 1–9 = intro/course branding, cells 10–18 = prerequisites/Python env, cells 19–27 = framework+env setup, cells 28–35 = config/training, cells 36–47 = publish/play).
2. **The embedded config** — quote the YAML verbatim. It's the only concrete technical artifact in the file.
3. **The history shape** — four commits, two identical trees, all on one day. That's a 4-sentence finding worth its own bullet.
4. **The "what this is not" list** — naming what isn't in the repo (no Stable Baselines3, no Gym/Gymnasium, no DQN/Q-learning, no tests, no CI, no license, no manifest, no recorded training evidence).
5. **The reproducibility caveats** — `Miniconda3-latest`, shallow clone, mutable framework default branch, binary executable per-OS.

A first draft at ~11–12k bytes for a notebook-only repo is normal and not a floor violation. Trim `### Notable details` if it runs long, not the cell walkthrough.

## Pitfalls specific to notebook-only repos

- **Don't quote Colab cell IDs as code.** Colab notebooks embed an `id: "..."` field in every cell metadata block. Those are runtime identifiers, not source-code references. Strip them before quoting cell content.
- **`private_outputs: true` is a Colab-side scrub, not a sign the notebook failed.** Many Colab exports scrub outputs to keep the file lean. The lack of outputs doesn't mean training didn't run — it means Colab stripped them. Worth one sentence in `### Notable details` so the reader doesn't conclude "broken notebook."
- **`!command` lines aren't Python.** A notebook with `!mlagents-learn ...` is invoking a shell command, not importing a Python module. The `### Tech stack` section should list `mlagents-learn` as the trainer binary and the ML-Agents package as the library that supplies it, with a one-line note that no Python import statements appear in the notebook itself.
- **`wget` of a binary release + `chmod -R 755` is a real install path.** Don't dismiss it as "hacky." A precompiled Unity environment executable for Linux x86_64 is exactly what the framework expects to find at the `--env` path. Name this in `### How to run it` verbatim.
- **The embedded `Huggy.yaml` isn't a file in the repo.** It's a string inside one cell that the notebook asks the user to paste into `/content/ml-agents/config/ppo/Huggy.yaml`. Quote it as a "config cell" in `### Key files`, not as a tracked artifact. Same pattern applies to any notebook that embeds a YAML/JSON config string in a cell.
- **Course attribution is a real provenance signal.** When the notebook credits Thomas Simonini / Hugging Face Deep RL Course / Unity ML-Agents upstream, that's a "course snapshot, not original framework code" finding — one sentence in `### Notable details` saves a future reader from wondering who wrote the algorithm.
- **`null` on language is fine.** The GitHub API returns `language: "Jupyter Notebook"` for a notebook-only repo; that string is valid for the frontmatter `language:` field. Don't leave it blank or substitute the notebook's runtime language — the brief wants the *language field* GitHub reports, which is the notebook format itself.
- **The notebook's `metadata.accelerator` may differ from what the user actually has.** Colab exports embed `"accelerator": "GPU"` and `"gpuClass": "standard"` even when the notebook was authored on CPU. Surface the request, not the runtime — "notebook requests a standard GPU" is accurate; "ran on a standard GPU" is not.
