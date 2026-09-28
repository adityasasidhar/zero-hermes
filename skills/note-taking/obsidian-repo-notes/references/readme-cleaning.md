# README -> Obsidian note cleaning

Raw GitHub READMEs contain markup that renders as garbage inside an Obsidian note.
Strip it before writing. Order matters.

## clean() — full strip chain
```python
import re

def clean(t):
    t = re.sub(r'<[^>]+>', '', t)                              # HTML tags (<img>, <br>)
    t = re.sub(r'\[!\[[^\]]*\]\([^)]*\)\]\([^)]*\)', '', t)   # badge images [![x](img)](link)
    t = re.sub(r'!\[[^\]]*\]\([^)]*\)', '', t)                # standalone images ![alt](url)
    t = re.sub(r'\[[\s]*\]\([^)]*\)', '', t)                  # empty-link remnants [](url)
    t = re.sub(r'\[([^\]]+)\]\([^)]*\)', r'\1', t)            # real links -> text
    t = re.sub(r'<!--.*?-->', '', t, flags=re.S)              # HTML comments
    return t.strip()
```
Apply `clean()` to the overview paragraph and to each feature bullet
(also strip `**bold**` markers from bullets: `re.sub(r'\*\*(.*?)\*\*', r'\1', f)`).

## Feature extraction
- Prefer a section whose title matches `feature|key|highlight|what it does|capabilit`.
  Take its `- ` / `* ` bullets (cleaned, length < 200).
- Fallback: intro-area bullets, excluding any that look like install commands
  (`pip|npm|cargo|git clone|cd `).
- Cap at ~10 features.

## Tech-stack detection (keyword scan of README text)
Maintain a `TECH_KW` dict mapping a label to substrings; lowercase-scan the README.
Always add the repo's primary `language` (from GitHub API — authoritative) and any
GitHub `topics`. Examples: `Python`, `PyTorch` (`pytorch|torch`), `Ollama` (`ollama`),
`MCP` (`model context protocol|mcp`), `Hugging Face` (`huggingface|hugging face`),
`Streamlit`, `FastAPI`, `Flask`, `React`, `Chrome Extension` (`chrome extension`),
`uv`, `LangChain`, `Anthropic` (`anthropic|claude`), `OpenAI`, `SQLite`, `Docker`, etc.

## Getting-Started blocks (real commands only)
Pull fenced ``` code blocks from the README. Keep a block only if:
- not all-comment (≤ half the non-empty lines start with `#`), AND
- at least one line matches a command keyword
  (`pip|npm|cargo|git clone|python|uv |poetry|npm i|brew|conda|docker|cd |make|go run|node `).
Replace placeholders: `<repository-url>` -> the repo URL, `<your-username>` -> owner.
Cap at 3 blocks, each < 600 chars.
