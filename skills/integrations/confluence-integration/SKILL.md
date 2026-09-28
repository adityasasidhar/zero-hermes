---
name: confluence-integration
description: Connect an AI agent or code to Atlassian Confluence (Cloud or Server/DC) to read/update product documentation. Covers MCP (mcp-atlassian and composio-confluence) and a direct v2 REST client, auth mechanics, ADF write format, version-bump conflicts, and short-link→page-ID resolution. Use when the user wants an agent to read/write Confluence pages, or asks about Confluence API credentials/integration.
---

# Confluence Integration

Connect an AI agent or code to Atlassian Confluence to read/update pages (e.g. product docs).

## Two routes
1. **MCP (fastest, agent-native).** Two options:
   - **`mcp-atlassian`** — official. Exposes ready-made tools; the harness handles ADF conversion + version-bumping. Slot it into any MCP-capable harness (opencode `mcp` block, Claude Code `mcpServers`). ⚠️ **v2.x env-var + tool-name renames** — see `references/mcp-atlassian-v2.md`. Common mistakes: using `CONFLUENCE_*` vars (renamed to `ATLASSIAN_*` in v2), using `confluence_get_page` (renamed to `read_confluence_page`), and `npx -y` dropping the `jsdom` dep. Known-good opencode config is in that reference.
   - **`composio-confluence`** (Composio remote MCP at `https://connect.composio.dev/mcp`) — zero-config, OAuth handled by Composio. Often pre-installed on opencode setups as `composio-confluence` in `~/.config/opencode/opencode.jsonc`. To remove, delete the `mcp.composio-confluence` block from that config.
2. **Direct REST (from-scratch client).** Use when you want full control or the harness can't run MCP. See `references/confluence-cloud-api.md`.

## Required credentials (Cloud) — three pieces, a token ALONE is NOT enough
- `CONFLUENCE_URL` → `https://<site>.atlassian.net/wiki` (or a custom wiki domain)
- `CONFLUENCE_USERNAME` → the Atlassian account email (e.g. `you@company.com`)
- `CONFLUENCE_API_TOKEN` → from id.atlassian.com > Security > API tokens

The REST route uses these `CONFLUENCE_*` names. **mcp-atlassian v2 renamed them** to
`ATLASSIAN_BASE_URL` / `ATLASSIAN_EMAIL` / `ATLASSIAN_API_TOKEN` — map a single token
via the harness config (`"ATLASSIAN_API_TOKEN": "{env:CONFLUENCE_API_TOKEN}"`).

Auth = HTTP Basic with `email:token`. **Do NOT use the account password.**

## Verify before building
Run `scripts/verify_confluence.py` from the project root (reads `.env`, does a live `200` probe + optional short-link resolution). `200` = creds good; `401` = wrong email/token. Don't ship integration code against unverified creds. For MCP, additionally smoke-test the server over stdio (see references/mcp-atlassian-v2.md): `initialize` → `tools/list` should return ~40 tools and a valid `serverInfo`.

## Pitfalls
- **Version bump (REST):** every PUT must send `version.number = current+1`, read *immediately* before write. Concurrent edits → `409 Conflict`. (MCP's `update_confluence_page` handles this if you pass the current `version`.)
- **Write format is ADF on Cloud v2 REST.** You cannot POST raw HTML/storage on write — the body must be `atlas_doc_format`. See references for the shape.
- **Short-links are not page IDs.** `/wiki/x/<CODE>` redirects to `/pages/<id>`. GET with `allow_redirects=True`, extract `/pages/<id>` from final URL.
- **Server / Data Center differs:** v1 `/rest/api/content`, write `body.storage` (HTML) directly, no ADF, same Basic auth.
- **Permission:** the agent's Confluence user needs edit rights on the target space, or you'll get `403` even with valid creds.
- **Inspect `.env` without leaking secrets:** the file tool blocks reading `.env` (secret defense-in-depth). To confirm WHICH keys exist without printing values, use the terminal: `cut -d= -f1 .env | grep -v '^#' | grep -v '^$'`. Never print token values into chat.
- **`.env` with NO trailing newline:** `printf '...\n' >> .env` appends the next key onto the last value, corrupting the token. Insert a newline boundary or edit via Python. A `401` from the verifier signals a broken token.
- **TOC / macro pages** return empty `content` from `read_confluence_page` (live-generated) — not an error. Point the agent at a real content page for body text.

## References / support files
- `references/confluence-cloud-api.md` — endpoints, ADF shape, error codes, Server/DC diff.
- `references/mcp-atlassian-v2.md` — v2 env-var renames, real tool/arg names, install gotchas, known-good opencode config, stdio verification recipe.
- `templates/.env.example.confluence` — env template to drop into a project.
- `scripts/verify_confluence.py` — live credential + short-link verifier (reads `.env`, exit 0 on success).
