# mcp-atlassian v2.x — condensed (learned the hard way, real transcripts)

`mcp-atlassian` is an **npm** package, NOT a Python/`uvx` package. Current tested version: **2.1.0**.

## Install (avoid `npx -y` — it skips a dep)
```bash
npm install -g mcp-atlassian
cd "$(npm root -g)/mcp-atlassian" && npm install jsdom@^21
```
Why the extra `jsdom` step: `npx -y mcp-atlassian` (and even the global install) sometimes omits `jsdom`, which `dist/utils/html-sanitizer.js` imports →
`Error [ERR_MODULE_NOT_FOUND]: Cannot find package 'jsdom'`. Install it into the package dir to fix.

## Env vars — RENAMED in v2 (the big gotcha)
| v1 / intuition | v2 actual |
|---|---|
| `CONFLUENCE_URL` | `ATLASSIAN_BASE_URL` |
| `CONFLUENCE_USERNAME` | `ATLASSIAN_EMAIL` |
| `CONFLUENCE_API_TOKEN` | `ATLASSIAN_API_TOKEN` |

If unset, the server fails fast:
`Error: Missing required environment variable: ATLASSIAN_BASE_URL`
(order of complaints seen: BASE_URL → USER_EMAIL (wrong, it's `ATLASSIAN_EMAIL`) → API_TOKEN).

Map a single token from a `CONFLUENCE_*`-named `.env` in the harness config:
`"ATLASSIAN_API_TOKEN": "{env:CONFLUENCE_API_TOKEN}"`.

## Tool names — camelCase, NOT `confluence_*`
Wrong: `confluence_get_page`, `confluence_update_page`
Right: `read_confluence_page`, `update_confluence_page`, `create_confluence_page`,
`search_confluence_pages`, `list_confluence_spaces`, `get_confluence_space`,
`upload_confluence_attachment`, `list_confluence_page_children`, …

Calling a wrong name returns: `{"result":{"content":[{"type":"text","text":"Unknown tool: confluence_get_page"}],"isError":true}}`.

**Page-id arg is `pageId` (camelCase), not `page_id`.** Passing `page_id` →
`Validation failed: Either pageId or title must be provided`. Pass `version` to `update_confluence_page`.

## Known-good opencode.json mcp block
```json
"mcp": {
  "confluence": {
    "command": "/home/arctic/.npm-global/bin/mcp-atlassian",
    "env": {
      "ATLASSIAN_BASE_URL": "https://<site>.atlassian.net/wiki",
      "ATLASSIAN_EMAIL": "you@company.com",
      "ATLASSIAN_API_TOKEN": "{env:CONFLUENCE_API_TOKEN}"
    }
  }
}
```
Use the **absolute global binary path** (from `which mcp-atlassian`), not `npx -y` (re-triggers broken cache).

## Verify the server over stdio (do this before claiming done)
Load `.env`, export the `ATLASSIAN_*` vars, then pipe JSON-RPC frames:
```bash
set -a; . ./.env; set +a
export ATLASSIAN_BASE_URL="..." ATLASSIAN_EMAIL="..." ATLASSIAN_API_TOKEN="$CONFLUENCE_API_TOKEN"
printf '%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"smoke","version":"0"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"read_confluence_page","arguments":{"pageId":"<id>","format":"markdown"}}}' \
  | timeout 40 /home/arctic/.npm-global/bin/mcp-atlassian
```
Expected: `serverInfo":{"name":"mcp-atlassian","version":"2.1.0"}` + a `tools/call` result
with real Confluence content. `tools/list` returns ~40 tools. A live `read_confluence_page`
returning data = end-to-end verified (auth + API reachability).

## Notes
- `--help` crashes without env set ("Missing required environment variable") — don't rely on it.
- TOC / live-macro pages return empty `content` via `read_confluence_page` — expected, not an error.
