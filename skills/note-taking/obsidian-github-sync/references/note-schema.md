# Note Schema — Obsidian GitHub Sync

Every generated note (`Github/Repos/<repo>.md`) follows this shape.

## Frontmatter (YAML)
```yaml
---
repo: space-cli
description: Space is a powerful, fully local CLI coding assistant powered by Ollama.
github: https://github.com/<owner>/<repo>
visibility: public            # or private
language: Python
topics: [ai-agents, cli]      # [] if none
stars: 0
forks: 0
status: active                # active | experimental/inactive | archived
created: 2025-11-14
last_pushed: 2026-01-29
type: LLM / Agent             # see categories
vault_group: Public Repos     # Public Repos | Private Repos
---
```

### Field rules
- `status`: `archived` if `archived` true; else `experimental/inactive` if `pushed_at` > 180 days ago; else `active`.
- `type` (auto-category, checked in order):
  1. **LLM / Agent** — blob contains `agent`, `mcp`, `llm`, `gpt`, `claude`, `ollama`.
  2. **Learning / Educational** — contains `from scratch`, `tutorial`, `learning`, `implement`, `understand`, `educational`, `course`.
  3. **ML Research / Model** — contains `classifier`, `mnist`, `neural`, `transformer`, `model`, `babylm`, `training`, `dataset`, `machine learning`, `embedding`.
  4. **Game** — `game` or `pygame`.
  5. **CLI Tool** — `cli`.
  6. **Web App** — `web`, `react`, `flask`, `fastapi`, `astro`, `website`, `chrome extension`, `frontend`.
  7. **Config / Profile** — `config` or `profile` in name.
  8. **Application / Utility** — default.
- `vault_group`: `Private Repos` if `visibility == private` else `Public Repos`.

## Body
```markdown
# <Title Case Of Repo>

<overview: README intro paragraph, cleaned of badges; fallback = description>

## Overview

<description or "No description provided.">

## Key Features          # only if features were extracted
- feature 1
- feature 2

## Tech Stack            # always present
- **Primary language:** Python
- **Frameworks / libraries / services:** Ollama, LLaMA, MCP, CLI
- **GitHub topics:** ai-agents, cli        # only if topics exist

## Getting Started       # only if real command blocks found
```
git clone https://github.com/<owner>/<repo>.git
cd <repo>
uv sync
```

## Stats
- ⭐ Stars: 0
- 🍴 Forks: 0
- 🔒 Visibility: public
- 📅 Created: 2025-11-14
- 🔄 Last pushed: 2026-01-29
- 🏷️ Category: LLM / Agent
- 📊 Status: active

## Links
- GitHub: https://github.com/<owner>/<repo>
- Part of: [[Public Repos]]
```

## Cleanup rules applied during extraction
- Strip `[![...](...)](...)`, `![...](...)`, `[ ](...)`, HTML comments.
- Links in prose: `[text](url)` → `text`.
- Feature bullets: strip leading `- ` / `* ` and `**bold**`.
- Code blocks: drop comment-only blocks; require a real command token before inclusion in Getting Started.
