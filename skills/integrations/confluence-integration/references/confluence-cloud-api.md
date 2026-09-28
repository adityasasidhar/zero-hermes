# Confluence Cloud v2 REST API — condensed

Base: `https://<site>.atlassian.net/wiki/api/v2`
Auth: Basic `email:API_TOKEN` (API token from id.atlassian.com, NOT password).

## Read a page
`GET /pages/{id}`
Response: `id`, `title`, `spaceId`, `version.number`, plus body if requested.

## Update a page
`PUT /pages/{id}`
```json
{
  "id": "<id>",
  "status": "current",
  "title": "...",
  "spaceId": "<spaceId>",
  "version": {"number": <current+1>},
  "body": {"atlas_doc_format": {"value": "<ADF json string>", "representation": "atlas_doc_format"}}
}
```
- `version.number` MUST equal current+1 or you get `409 Conflict`. Read current version immediately before writing.
- `body.atlas_doc_format.value` is a **JSON string** of the ADF doc (serialize the dict, don't nest the object directly).

## ADF shape (minimal)
```json
{"version":1,"type":"doc","content":[
  {"type":"heading","attrs":{"level":1},"content":[{"type":"text","text":"Title"}]},
  {"type":"paragraph","content":[{"type":"text","text":"Body text."}]}
]}
```
Node types: `paragraph`, `heading` (attrs.level), `bulletList`/`listItem`, `codeBlock`, `table`, `text` (with optional `marks` for strong/em/link).

## Create a page
`POST /pages` — needs `spaceId`, optional `parentId`, `title`, `version {number:1}`, `body`.

## Short-link resolution
`GET https://<site>.atlassian.net/wiki/x/<CODE>` with `allow_redirects=True`, auth Basic.
Final URL contains `/pages/<id>`. Extract: `re.search(r"/pages/(\d+)", final_url)`.

## Error codes
- `401` wrong email/token (or token revoked)
- `403` valid creds, no permission on space
- `404` page/space not found
- `409` version conflict (stale `version.number`)

## Server / Data Center (v1)
- Base: `/rest/api/content`
- Auth: same Basic `email:token`
- Write `body.storage.value = <HTML>`, `representation: "storage"`
- No ADF. Versioning still required (`version.number = current+1`).

## Conversion
To turn existing storage-format/HTML → ADF, use `POST /wiki/api/v2/pages/{id}/body` conversion endpoint, or parse to ADF manually.