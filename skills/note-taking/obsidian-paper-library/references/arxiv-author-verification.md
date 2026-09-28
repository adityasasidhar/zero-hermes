# Author verification across major AI labs

Patterns observed from past sessions for identifying whether an arXiv paper belongs to a target lab or author group. **PDF header is ground truth** — abstract pages alone often omit affiliations, and author-name collisions are common.

## Moonshot AI / Kimi Team

- **Affiliation string:** `Moonshot AI` (one word, capital M).
- **Author label:** Sometimes `Kimi Team` (capital K, single-name block at the top of v2 papers).
- **Subtitle:** `TECHNICAL REPORT OF <MODEL>` all-caps header on the second page after the title.
- **Decoys to reject:**
  - `K2-Think: A Parameter-Efficient Reasoning System` (arXiv:2509.07604) — author list is independent academic group; the `K2` is branding. Reject.
  - `Optimizing Mixture of Block Attention` (arXiv:2511.11571) — MIT + NVIDIA (Song Han lab). Builds on Moonshot's MoBA, but isn't Moonshot's. Belongs under a separate "external follow-ups" list, not the lab library.
  - Papers co-authored with Kimi Team sometimes have Numina, THU, PKU, etc. as joint authors (e.g. `Kimina-Prover`). Include with note.

## DeepSeek

- **Affiliation string:** `DeepSeek-AI`, `DeepSeek`, occasionally just `High-Flyer` (the parent hedge fund).
- **Authors:** Often labeled `DeepSeek-AI` as a single author block.
- **Decoys to watch:**
  - `DeepSeek-R1` papers are real (DeepSeek-AI). Other "DeepSeek-X" titles from non-DeepSeek institutions should be inspected.

## Anthropic

- **Affiliation string:** `Anthropic`.
- **Decoys:** None obvious. Anthropic is consistent about affiliation.

## OpenAI

- **Affiliation:** `OpenAI`.
- **Decoys:** Several independent "OpenAI-X" survey papers cite OpenAI as inspiration but aren't authored there. Always verify the affiliation in the PDF header.

## MIT / MIT-HAN-Lab (Song Han)

- **Affiliation:** `MIT`, `MIT-IBM Watson AI Lab`, `NVIDIA` (often co-affiliated).
- **Decoys:** Look for the explicit MIT-HAN-Lab GitHub pointer in the comments field if it's an ML-systems paper.

## Tsinghua / THU

- **Affiliation:** `Tsinghua University`, `Department of Computer Science and Technology, Tsinghua University`, sometimes `THU` in the GitHub org (`thunlp`, `thucst`).
- **Joint affiliations common:** THU + PKU + Microsoft Research Asia (typical NLP paper clusters).

## HKUST / HKU / CUHK (Hong Kong)

- **Affiliation:** `The Hong Kong University of Science and Technology`, `The University of Hong Kong`, etc.
- **Watch for:** Joint authorship with `Microsoft Research Asia` or `Tencent AI Lab`.

## Princeton / Allen AI / NYU / Stanford

- Less pattern noise — affiliations are university-only and rarely confused with one another.

## Verification recipe (re-use this in `executive_code` or terminal scripts)

```python
# Pseudo-code: check the first ~500 chars of the PDF for an affiliation string.
from hermes_tools import web_extract

async def is_from_lab(arxiv_id: str, affiliation: str) -> bool:
    r = await web_extract([f"https://arxiv.org/pdf/{arxiv_id}"], char_limit=800)
    content = r["results"][0]["content"]
    return affiliation.lower() in content.lower()
```

For ambiguous cases, also check the GitHub org listed in the comments field (arXiv abstract page). A real lab paper usually links to a repo **inside** that lab's GitHub org.

## What to do when verification fails

1. **Don't add the paper** to the target lab's collection. Stop here.
2. If the user asked "all papers by <lab>", **report the skipped candidates** with a one-line reason each, so they can decide whether to relax the filter.
3. If the paper is a follow-up or analysis from a different lab, propose a sibling subfolder like `Research/Papers/<Lab>/external-followups/` and ask the user if they want it added there.
