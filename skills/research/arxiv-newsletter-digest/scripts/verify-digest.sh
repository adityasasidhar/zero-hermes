#!/usr/bin/env bash
# verify-digest.sh — run the 5 verification checks against a digest JSON file.
# Usage: ./verify-digest.sh path/to/digest.json
# Exits 0 if all checks pass, non-zero otherwise.

set -u

FILE="${1:-}"
if [ -z "$FILE" ] || [ ! -f "$FILE" ]; then
  echo "Usage: $0 <digest.json>"
  exit 2
fi

if ! command -v jq >/dev/null 2>&1; then
  echo "FAIL: jq is required"
  exit 2
fi

FAILS=0

echo "=== 1. JSON parses ==="
if ! python3 -c "import json; json.load(open('$FILE'))" 2>/dev/null; then
  echo "FAIL: $FILE is not valid JSON"
  FAILS=$((FAILS+1))
else
  echo "OK"
fi

echo ""
echo "=== 2. Every image is a real PNG ==="
IMG_COUNT=$(jq -r '.stories[].image.path // empty' "$FILE" | wc -l)
if [ "$IMG_COUNT" -eq 0 ]; then
  echo "WARN: no images found in stories"
else
  while IFS= read -r img; do
    [ -z "$img" ] && continue
    if [ ! -f "$img" ]; then
      echo "FAIL: missing $img"
      FAILS=$((FAILS+1))
    elif ! file "$img" | grep -q "PNG image data"; then
      echo "FAIL: not a PNG: $img"
      FAILS=$((FAILS+1))
    fi
  done < <(jq -r '.stories[].image.path // empty' "$FILE")
  echo "OK ($IMG_COUNT images checked)"
fi

echo ""
echo "=== 3. Every image is > 20 KB ==="
while IFS= read -r img; do
  [ -z "$img" ] && continue
  [ -f "$img" ] || continue
  SIZE=$(stat -c%s "$img" 2>/dev/null || echo 0)
  if [ "$SIZE" -le 20480 ]; then
    echo "FAIL: too small ($SIZE bytes): $img"
    FAILS=$((FAILS+1))
  fi
done < <(jq -r '.stories[].image.path // empty' "$FILE")
echo "OK"

echo ""
echo "=== 4. Every arxiv_id is well-formed ==="
BAD_ARXIV=$(jq -r '.stories[].arxiv_id // empty' "$FILE" | grep -vE '^[0-9]{4}\.[0-9]{4,5}(v[0-9]+)?$' || true)
if [ -n "$BAD_ARXIV" ]; then
  echo "FAIL: malformed arxiv_id(s):"
  echo "$BAD_ARXIV"
  FAILS=$((FAILS+1))
else
  echo "OK"
fi

echo ""
echo "=== 5. Every published_at is ISO 8601 UTC ==="
BAD_DATES=$(jq -r '.stories[].published_at' "$FILE" | grep -vE '^[0-9]{4}-[0-9]{2}-[0-9]{2}T[0-9]{2}:[0-9]{2}:[0-9]{2}Z$' || true)
if [ -n "$BAD_DATES" ]; then
  echo "FAIL: malformed published_at:"
  echo "$BAD_DATES"
  FAILS=$((FAILS+1))
else
  echo "OK"
fi

echo ""
echo "=== 6. Every rank is unique and 1-indexed ==="
RANKS=$(jq -r '.stories[].rank' "$FILE" | sort -n | uniq -c | awk '$1 > 1 {print $2}' || true)
if [ -n "$RANKS" ]; then
  echo "FAIL: duplicate ranks: $RANKS"
  FAILS=$((FAILS+1))
fi
MIN_RANK=$(jq -r '.stories[].rank' "$FILE" | sort -n | head -1)
if [ "$MIN_RANK" != "1" ]; then
  echo "FAIL: smallest rank is $MIN_RANK, expected 1"
  FAILS=$((FAILS+1))
else
  echo "OK"
fi

echo ""
echo "=== 7. Story count is 4-6 ==="
COUNT=$(jq '.stories | length' "$FILE")
if [ "$COUNT" -lt 4 ] || [ "$COUNT" -gt 6 ]; then
  echo "FAIL: story count is $COUNT, expected 4-6"
  FAILS=$((FAILS+1))
else
  echo "OK ($COUNT stories)"
fi

echo ""
if [ "$FAILS" -eq 0 ]; then
  echo "ALL CHECKS PASSED"
  exit 0
else
  echo "$FAILS CHECK(S) FAILED"
  exit 1
fi