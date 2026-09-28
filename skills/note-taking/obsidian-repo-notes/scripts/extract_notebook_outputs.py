#!/usr/bin/env python3
"""Extract cell-by-cell markdown text and code-cell outputs from every
``*.ipynb`` in a repo. Used by the deep-enrichment workflow for
notebook-first repos (train/experiment notebooks with no Python entry point).

Usage::

    python3 extract_notebook_outputs.py <repo-root>     # prints to stdout
    python3 extract_notebook_outputs.py <repo-root> > notebook_outputs.txt

The script is stdlib-only (json + pathlib) so it runs in any Python 3.10+
without `pip install`. It handles the three output types you'll see:
``stream``, ``execute_result`` / ``display_data``, and ``error``.

Why this exists: ``read_file`` on a 100k+ char notebook shows a truncated
preview that drops outputs. Running the notebooks is slow and may require
GPU / large downloads. Reading the JSON directly is fast, deterministic, and
shows you the *actual* recorded run results — loss traces, accuracy, sample
predictions — which are exactly the kind of evidence the deep-enrichment
brief asks for in ``### Notable details``.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path


def _format_outputs(outputs: list[dict]) -> str:
    pieces: list[str] = []
    for out in outputs:
        kind = out.get("output_type")
        if kind == "stream":
            pieces.append("".join(out.get("text", [])))
        elif kind in ("execute_result", "display_data"):
            data = out.get("data", {})
            text = data.get("text/plain", "")
            if isinstance(text, list):
                text = "".join(text)
            pieces.append(str(text))
        elif kind == "error":
            pieces.append(f"ERROR {out.get('ename')}: {out.get('evalue')}")
    cleaned = [b.strip().replace("\n", " | ") for b in pieces if b and b.strip()]
    return " || ".join(cleaned)


def extract(root: Path) -> None:
    for nb_path in sorted(root.glob("*.ipynb")):
        try:
            nb = json.loads(nb_path.read_text())
        except json.JSONDecodeError as exc:
            print(f"=== {nb_path.name} (UNREADABLE: {exc}) ===\n")
            continue

        print(f"=== {nb_path.name} ===")
        meta = nb.get("metadata", {})
        kernelspec = meta.get("kernelspec", {})
        language_info = meta.get("language_info", {})
        print(
            "metadata:",
            json.dumps(
                {
                    "kernelspec": {
                        k: kernelspec.get(k)
                        for k in ("display_name", "name", "language")
                    },
                    "language_info.version": language_info.get("version"),
                }
            ),
        )

        cells = nb.get("cells", [])
        n_code = sum(c.get("cell_type") == "code" for c in cells)
        n_md = sum(c.get("cell_type") == "markdown" for c in cells)
        print(f"cells: {len(cells)}  code: {n_code}  markdown: {n_md}")

        for idx, cell in enumerate(cells, 1):
            ctype = cell.get("cell_type")
            source = "".join(cell.get("source", [])).strip()
            if ctype != "code":
                print(f"MD {idx}: {source[:300]}")
                continue
            outputs = cell.get("outputs", [])
            if outputs:
                formatted = _format_outputs(outputs)
                print(
                    f"OUT {idx} exec={cell.get('execution_count')}: "
                    f"{formatted[:1200]}"
                )
        print()


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    root = Path(sys.argv[1]).expanduser().resolve()
    if not root.is_dir():
        print(f"not a directory: {root}", file=sys.stderr)
        return 2
    extract(root)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
