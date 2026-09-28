#!/bin/bash
# verify-repo-batch.sh — verify a list of "owner/name" GitHub repos against the
# real API. Prints a TSV with full_name, stars, last_push, license, archived.
#
# Usage:
#   ./verify-repo-batch.sh owner1/repo1 owner2/repo2 owner3/repo3 ...
#   ./verify-repo-batch.sh -f repos-to-check.txt     # one repo per line
#
# Exit codes: 0 always (404s are reported inline, not fatal). The script
# intentionally does NOT kill the loop on a 404 so a single bad name doesn't
# waste your run.
#
# Rate-limit pacing: ~5s/call. For batches > 30, consider splitting across
# sessions to stay under the 30/hour authed REST cap.

set -u

if [ "$1" = "-f" ]; then
  shift
  REPOS=$(grep -v '^\s*#' "$1" | grep -v '^\s*$')
else
  REPOS="$@"
fi

# Header
printf "STATUS\tFULL_NAME\tSTARS\tLAST_PUSH\tLICENSE\tARCHIVED\tDESCRIPTION\n"

for repo in $REPOS; do
  # Pad throttle — set to 0 if you're confident you're not rate-limited.
  sleep 5

  out=$(gh api "repos/$repo" 2>&1)

  # 404 / 403 / other error path
  if echo "$out" | grep -q '"message"'; then
    msg=$(echo "$out" | python3 -c "import sys,json; print(json.load(sys.stdin).get('message','?'))" 2>/dev/null || echo "?")
    printf "MISSING\t%s\t-\t-\t-\t-\t%s\n" "$repo" "$msg"
    continue
  fi

  # Happy path — pipe to python so we don't trip on the jq-on-error body issue.
  echo "$out" | python3 -c "
import sys, json
try:
    r = json.load(sys.stdin)
    lic = r.get('license') or {}
    spdx = lic.get('spdx_id') or 'NOASSERTION'
    desc = (r.get('description') or '').replace('\t',' ').replace('\n',' ')[:80]
    print(f\"OK\t{r['full_name']}\t{r['stargazers_count']}\t{r['pushed_at'][:10]}\t{spdx}\t{r['archived']}\t{desc}\")
except Exception as e:
    print(f\"PARSE_ERR\t-\t-\t-\t-\t-\t{e}\")
"
done