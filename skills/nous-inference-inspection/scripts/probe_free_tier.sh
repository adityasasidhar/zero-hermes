#!/usr/bin/env bash
# probe_free_tier.sh — empirically inspect the Nous inference backend Hermes uses.
# SAFE: only reads public model metadata + portal auth status. Does NOT generate tokens.
set -euo pipefail

echo "=== Hermes portal auth / provider status (no secrets printed) ==="
hermes portal info 2>/dev/null || echo "(hermes not found or portal not configured)"
echo

echo "=== Zero-cost / free-tier models on the Nous inference API ==="
curl -s -m 30 https://inference-api.nousresearch.com/v1/models \
  | python3 -c '
import sys, json
try:
    data = json.load(sys.stdin).get("data", [])
except Exception as e:
    print("failed to parse models endpoint:", e); sys.exit(0)
free = [m for m in data
        if str(m.get("pricing", {}).get("prompt", "x")) in ("0", "0.0")
        or str(m.get("pricing", {}).get("completion", "x")) in ("0", "0.0")]
if not free:
    print("(no zero-cost models returned)")
for m in free:
    p = m.get("pricing", {})
    print(f"{m[\"id\"]:34s} prompt={p.get(\"prompt\")} completion={p.get(\"completion\")} "
          f"free_variant={m.get(\"synthesizedFreeVariant\", False)} ctx={m.get(\"context_length\")}")
'
echo

echo "=== Rate limits ==="
echo "No RPM/quota is published for the free tier. Limits are dynamic on a shared pool."
echo "To read your LIVE per-account limits, inspect x-ratelimit-* headers on an AUTHENTICATED request:"
echo '  curl -s -D - -o /dev/null https://inference-api.nousresearch.com/v1/chat/completions \'
echo '    -H "Authorization: Bearer $NOUS_JWT" -H "Content-Type: application/json" \'
echo '    -d "{\"model\":\"tencent/hy3:free\",\"messages\":[{\"role\":\"user\",\"content\":\"hi\"}]}"'
echo "(The JWT is obtained via the Nous Portal OAuth flow; Hermes stores it in ~/.hermes/auth.json.)"
echo "Unauthenticated requests return HTTP 402 with maxAmountRequired:\"0\" for free models (OAuth-gated, $0)."
