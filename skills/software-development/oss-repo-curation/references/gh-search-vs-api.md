# gh search vs gh api: the JSON schema gotcha

The two `gh` commands you'd reach for when curating repo lists return **different JSON shapes** with different field names. Mixing them up wastes ~10 minutes per session.

## TL;DR

| Command | Field style | Works with raw `jq`? | How to call |
|---|---|---|---|
| `gh api repos/owner/name` | snake_case | yes | pipe directly to `jq` |
| `gh search repos "<q>"` (no flag) | tabular text | no | useless for `jq` — every `.[].stars` is `null` |
| `gh search repos "<q>" --json <fields>` | camelCase | yes | use this when searching |

## The working patterns

### Verifying a specific repo (the truth source)

```bash
gh api repos/{owner}/{name} \
  | jq -c '{full_name, stars: .stargazers_count, pushed_at, license: .license.spdx_id, open_issues: .open_issues_count, archived, desc: .description}'
```

snake_case fields you actually need:
- `stargazers_count` (NOT `stargazersCount` here)
- `pushed_at`
- `archived`
- `open_issues_count`
- `license.spdx_id` (may be `null` → use `// "NOASSERTION"` in jq)
- `description`

### Searching for candidates

```bash
gh search repos "<query>" --limit 10 \
  --json fullName,stargazersCount,pushedAt,license,isArchived \
  | jq -c '.[] | {full_name: .fullName, stars: .stargazersCount, pushed: .pushedAt, license: (.license // {}).spdxId, archived: .isArchived}'
```

camelCase fields you need when using `--json`:
- `fullName` (NOT `full_name`)
- `stargazersCount`
- `pushedAt`
- `isArchived`
- `license.spdxId`

### The batch-verification loop (the right tool for curation)

```bash
for repo in "owner1/repo1" "owner2/repo2" "owner3/repo3"; do
  echo "=== $repo ==="
  gh api repos/$repo 2>&1 \
    | jq -c '{full_name, stars: .stargazers_count, pushed_at, license: .license.spdx_id, archived}' \
    2>/dev/null || echo "FAILED (likely 404)"
done
```

The `2>&1` is critical: 404s (typos, renamed repos) print inline rather than killing the loop. The `2>/dev/null` on jq suppresses the parse error for 404 responses so the next iteration still runs.

## The anti-patterns

### ❌ Raw search piped to jq with snake_case fields

```bash
gh search repos "OpenThoughts" --limit 5 \
  | jq -c '.[] | {full_name, stars: .stargazers_count}'   # every stars is null
```

Tabular search output has no JSON object structure for fields — the columns aren't keys. You'll get `null` for every field name you try. Use `--json` to get JSON.

### ❌ Mixing snake_case into a `--json` invocation

```bash
gh search repos "x" --json full_name  # error: unknown field "full_name"
```

The search `--json` flag is a whitelist; field names are camelCase by GitHub's REST-search convention.

### ❌ Mixing camelCase into `gh api` output

```bash
gh api repos/owner/repo | jq '.stargazersCount'   # null
```

`gh api` returns the raw REST API payload, which is snake_case.

## A 404 trap to watch for

When the candidate repo's `owner/name` is wrong (typo, rename, or just stale recall), `gh api` returns a JSON error body that jq will try to parse as a repo object — every field becomes `null` and the loop looks "successful." The fix:

```bash
gh api repos/owner/repo 2>&1 | head -1   # if this says "Not Found", skip it
```

Or in a loop, check `.message`:

```bash
gh api repos/$repo 2>&1 | jq -e '.message | not' >/dev/null && echo "valid" || echo "404"
```

## Quick decision tree

1. "I know the exact repo, just need its state" → `gh api repos/owner/repo` + snake_case jq.
2. "I'm searching for candidates" → `gh search repos "..."` with `--json` + camelCase jq.
3. "I'm curating a list of 10+" → batch loop, both patterns, verify every entry.
4. "Filter by license" → both APIs return `license.spdx_id`; handle `null` for unlicensed.

## Org-level enumeration

When the user lists candidate orgs by name (very common in curation requests), enumerate each org's repos:

```bash
gh search repos "" --owner <ORG> --limit 50 \
  --json fullName,description,pushedAt,stargazersCount,isArchived --sort updated \
  | jq -r '.[] | "\(.pushedAt[:10]) | \(.stargazersCount)★ | \(.fullName) | \(.description[:80])"'
```

This surfaces "alive but I didn't know about it" gems without you having to recall each repo. Use it for every org the user names.

**Some orgs the user names may not exist on GitHub at all.** Before spending time enumerating, sanity-check with one likely repo from that org. If `gh api repos/<ORG>/<likely-name>` returns 404 and the user is confident the org exists, it might be under a different casing or naming (e.g. `HuggingFaceTB/` → `huggingface/`, `allenai/` is correct, but `state-spaces/` and `open-r1/` both work). If the org genuinely doesn't have a GitHub presence, note it explicitly in the deliverable and move on — do NOT keep retrying.

## Rate-limit pacing

Batched `gh api` and `gh search` calls hit GitHub's secondary rate limit fast. Symptoms:

- `gh search repos "valid-query"` suddenly returns `[]`
- `gh api repos/X` starts returning 404 for repos you know exist
- Authed REST endpoint caps around 30 calls/hour

Pacing rules:

- `sleep 4` to `sleep 6` between batched `gh api` calls when the batch is > 10
- `sleep 5` between `gh search repos` calls
- If you see `[]` from a valid query, `sleep 8` and retry — it's the rate limit, not an empty result
- Don't retry faster than ~3s; you'll compound the cooldown

A useful mental model: budget 1 verification call per 5 seconds for steady-state work.

## Reference: full field-name cheat sheet

| Concept | `gh api` (snake) | `gh search --json` (camel) |
|---|---|---|
| repo full name | `full_name` | `fullName` |
| star count | `stargazers_count` | `stargazersCount` |
| last push | `pushed_at` | `pushedAt` |
| license SPDX | `license.spdx_id` (or null) | `license.spdxId` (or null) |
| archived flag | `archived` | `isArchived` |
| open issues | `open_issues_count` | `openIssuesCount` |
| description | `description` | `description` |
| fork flag | `fork` | `isFork` |
| language | `language` | `primaryLanguage.name` |
| topics | `topics` | `repositoryTopics[].name` |
| last update (metadata) | `updated_at` | `updatedAt` |
