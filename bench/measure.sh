#!/usr/bin/env bash
# bench/measure.sh — measure binary size, cold start, and idle RSS for
# the locally-built `zero-hermes` binary. Output as a markdown table.
#
# Usage: ./bench/measure.sh [N_RUNS]
#   N_RUNS = number of cold-start samples (default 10)

set -euo pipefail

N="${1:-10}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BIN="$ROOT/target/release/zero-hermes"

if [[ ! -x "$BIN" ]]; then
    echo "binary not found at $BIN — run 'cargo build --release' first" >&2
    exit 1
fi

# Binary size.
SIZE_BYTES=$(stat -c %s "$BIN" 2>/dev/null || stat -f %z "$BIN")
SIZE_MB=$(awk "BEGIN { printf \"%.2f\", $SIZE_BYTES / 1048576 }")

# Cold start: measure `--version` (no I/O, exits immediately) N times and
# take the median. `--version` is the smallest possible startup path.
TIMES_FILE=$(mktemp)
for _ in $(seq 1 "$N"); do
    { time "$BIN" --version > /dev/null; } 2>&1 | awk '/real/ {print $2}' >> "$TIMES_FILE"
done

# Convert times like "0m0.003s" to milliseconds and sort.
MS_FILE=$(mktemp)
awk '
{
    # Parse "0m0.003s" or "0.003s"
    if (match($0, /[0-9]+m[0-9.]+s/)) {
        m = split($0, parts, /[ms]/)
        printf "%d\n", parts[1] * 60000 + parts[2] * 1000
    } else if (match($0, /[0-9.]+s/)) {
        split($0, parts, /s/)
        printf "%d\n", parts[1] * 1000
    }
}' "$TIMES_FILE" >> "$MS_FILE"

MEDIAN_MS=$(sort -n "$MS_FILE" | awk 'NR==int((NR+1)/2)+0 {print int($1)}' | head -1)
MIN_MS=$(sort -n "$MS_FILE" | head -1)
MAX_MS=$(sort -n "$MS_FILE" | tail -1)

rm -f "$TIMES_FILE" "$MS_FILE"

# Build a one-shot "open the world and exit" via `--mock tools`. That
# opens Memory + SkillRegistry + ToolRegistry — the same warm path the
# gateway takes before listening.
if /usr/bin/time -v true 2>/dev/null; then
    TIME_CMD=/usr/bin/time
else
    TIME_CMD=""
fi

# Build a one-shot "open the world and exit" via `--mock tools`. That
# opens Memory + SkillRegistry + ToolRegistry — the same warm path the
# gateway takes before listening.
TMP_CFG=$(mktemp)
TMP_DB=$(mktemp)
cat > "$TMP_CFG" <<EOF
[provider]
kind = "anthropic"
base_url = "https://example.invalid"
api_key = "bench"
model = "bench"
max_tokens = 1

[memory]
path = "$TMP_DB"

[agent]
max_iterations = 1
context_window = 1
enabled_tools = []
EOF

if [[ -n "$TIME_CMD" ]]; then
    # /usr/bin/time -v writes its output to stderr. Capture stderr only.
    TIME_OUT=$($TIME_CMD -v "$BIN" --mock --config "$TMP_CFG" tools 2>&1 > /dev/null || true)
    RSS_KB=$(echo "$TIME_OUT" | awk '/Maximum resident/ {print $6}')
    if [[ -z "$RSS_KB" ]]; then RSS_KB="n/a"; fi
else
    RSS_KB="n/a"
fi
rm -f "$TMP_CFG" "$TMP_DB"

cat <<EOF

## zero-hermes bench

| metric                    | value           |
| ------------------------- | --------------- |
| binary size               | ${SIZE_MB} MB   |
| cold start (median, n=$N) | ${MEDIAN_MS} ms |
| cold start (min)          | ${MIN_MS} ms    |
| cold start (max)          | ${MAX_MS} ms    |
| idle RSS (registry open)  | ${RSS_KB} KB    |

Run with \`./bench/measure.sh 30\` for more samples.

EOF
