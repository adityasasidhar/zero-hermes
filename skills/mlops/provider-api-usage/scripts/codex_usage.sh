#!/usr/bin/env bash
# codex_usage.sh — print Codex CLI usage parsed from local rollout JSONLs.
#
# Why this exists:
#   The Codex CLI does NOT expose a public usage API. Its `~/.codex/auth.json`
#   carries a ChatGPT OAuth bearer that:
#     - api.openai.com/v1/{me,usage,dashboard/billing/*}  -> 401 invalid_jwt
#     - chatgpt.com/backend-api/codex/{usage,rate_limits,credits} -> 403 (browser only)
#   The only authoritative number on disk is what Codex itself emits as
#   `event_msg.token_count` events in the rollout JSONLs:
#     - info.last_token_usage  (per-turn deltas: input/output/cache/reasoning)
#     - info.total_token_usage (session cumulative)
#     - rate_limits.primary    (used_percent, window_minutes, resets_at)
#     - rate_limits.credits    (unlimited, has_credits, limit_id)
#   Plan tier (Plus / Pro / Business) and renewal come from the JWT claims in
#   ~/.codex/auth.json → tokens.id_token (decode base64url payload, no sig check).
#
# Usage:
#   bash codex_usage.sh              # full history, plain text
#   bash codex_usage.sh --since 7d   # last 7 days (also: 14d, 30d, 24h, 1h)
#   bash codex_usage.sh --rate       # just the rate-limit snapshot
#   bash codex_usage.sh --json       # machine-readable JSON output
#
# Companion at user's home: ~/scripts/codex-usage/codex_usage.sh + README.md
# Tested: codex-cli 0.144-0.145; ChatGPT Plus (10080-min window); 2026-07-27.

set -euo pipefail

SINCE_DAYS="${SINCE_DAYS:-}"
JSON_OUT=false
RATE_ONLY=false
prev=""
for arg in "$@"; do
  if [[ -n "$prev" ]]; then
    case "$prev" in
      --since) SINCE_DAYS="$arg" ;;
    esac
    prev=""
    continue
  fi
  case "$arg" in
    --since) prev="--since" ;;
    --since=*) SINCE_DAYS="${arg#--since=}" ;;
    --json) JSON_OUT=true ;;
    --rate) RATE_ONLY=true ;;
    --help|-h)
      sed -n '2,20p' "$0"
      exit 0 ;;
    *) echo "unknown arg: $arg" >&2; exit 2 ;;
  esac
done

CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"
SESS_DIR="$CODEX_HOME/sessions"

if [[ ! -d "$SESS_DIR" ]]; then
  echo "no codex sessions dir at $SESS_DIR — is codex installed?" >&2
  exit 1
fi

# Cutoff for --since (epoch seconds)
NOW_EPOCH=$(date +%s)
if [[ -n "$SINCE_DAYS" && "$SINCE_DAYS" =~ ^[0-9]+[dh]?$ ]]; then
  unit="${SINCE_DAYS: -1}"
  val="${SINCE_DAYS%[dh]}"
  if [[ "$unit" == "h" ]]; then
    CUTOFF=$((NOW_EPOCH - val*3600))
  else
    CUTOFF=$((NOW_EPOCH - val*86400))
  fi
else
  CUTOFF=0
fi

python3 - "$SESS_DIR" "$CUTOFF" "$JSON_OUT" "$RATE_ONLY" <<'PY'
import glob, json, datetime as dt, os, sys
from collections import defaultdict

sess_dir, cutoff, json_out_str, rate_only_str = sys.argv[1:5]
cutoff = int(cutoff)
def as_bool(s): return str(s).strip().lower() in ('1', 'true', 'yes', 'y', 'on')
json_out = as_bool(json_out_str)
rate_only = as_bool(rate_only_str)

def parse_ts(ts):
    try: return int(dt.datetime.fromisoformat(ts.replace('Z','+00:00')).timestamp())
    except Exception: return 0

day_turns = defaultdict(lambda: {'turns':0,'in':0,'out':0,'cache':0,'reason':0})
day_peak = defaultdict(float)
latest_rate = {}
newest_session_rate = None

files = sorted(glob.glob(f'{sess_dir}/**/rollout-*.jsonl', recursive=True))
n_sessions = 0
n_with_usage = 0
for fp in files:
    n_sessions += 1
    last_session_epoch = 0
    with open(fp) as f:
        for line in f:
            try: o = json.loads(line)
            except Exception: continue
            ts_iso = o.get('timestamp') or ''
            ev_epoch = parse_ts(ts_iso)
            if ev_epoch > last_session_epoch:
                last_session_epoch = ev_epoch
            if o.get('type') != 'event_msg': continue
            p = o.get('payload') or {}
            if p.get('type') != 'token_count': continue
            info = p.get('info') or {}
            tu = info.get('last_token_usage') or {}
            rlp = p.get('rate_limits') or {}
            try:
                it = int(tu.get('input_tokens') or 0)
                ot = int(tu.get('output_tokens') or 0)
                ci = int(tu.get('cached_input_tokens') or 0)
                rr = int(tu.get('reasoning_output_tokens') or tu.get('reasoning_tokens') or 0)
            except Exception:
                continue
            if it + ot == 0:
                continue
            n_with_usage += 1
            day = ts_iso[:10] if ts_iso else 'unknown'
            d = day_turns[day]
            d['turns'] += 1
            d['in'] += it; d['out'] += ot; d['cache'] += ci; d['reason'] += rr
            prim = (rlp.get('primary') or {})
            used = prim.get('used_percent')
            rs = prim.get('resets_at')
            wm = prim.get('window_minutes')
            credits = (rlp.get('credits') or {})
            if used is not None:
                day_peak[day] = max(day_peak[day], float(used))
                latest_rate[day] = (float(used), wm, rs, credits.get('unlimited'), credits.get('has_credits'), rlp.get('limit_id'))
            if newest_session_rate is None or last_session_epoch > newest_session_rate[0]:
                newest_session_rate = (last_session_epoch, float(used) if used is not None else None, rs, wm,
                                       credits.get('unlimited'), credits.get('has_credits'), rlp.get('limit_id'))

def in_window(day):
    if not cutoff: return True
    try:
        e = int(dt.datetime.fromisoformat(day + 'T00:00:00+00:00').timestamp())
        return e >= cutoff
    except Exception:
        return True

total_in = total_out = total_cache = total_reason = total_turns = 0
out_lines = []
header = (f'{"date":<11} {"turns":>6} {"input":>11} {"output":>10} {"cache":>11} {"reason":>10}  peak%')
out_lines.append(header)
out_lines.append('-' * len(header))
for d in sorted(day_turns):
    if not in_window(d): continue
    r = day_turns[d]
    pk = day_peak.get(d)
    pk_s = f'{pk:>5.1f}' if pk else '   --'
    out_lines.append(
        f'{d:<11} {r["turns"]:>6} {r["in"]:>11} {r["out"]:>10} {r["cache"]:>11} {r["reason"]:>10}  {pk_s}'
    )
    total_in += r['in']; total_out += r['out']; total_cache += r['cache']
    total_reason += r['reason']; total_turns += r['turns']
out_lines.append('-' * len(header))
out_lines.append(
    f'{"TOTAL":<11} {total_turns:>6} {total_in:>11} {total_out:>10} {total_cache:>11} {total_reason:>10}')

def human(n):
    for u in ['','K','M','B']:
        if n < 1000:
            return f'{n:.0f}{u}'
        n /= 1000
    return f'{n:.1f}T'

def cache_pct(c, t):
    return f'{(100*c/t):.1f}%' if t else '0.0%'

def rate_snapshot_block():
    if newest_session_rate is None:
        return ['  no rate-limit data on disk — has codex been used yet?']
    epoch, used_pct, rs, wm, unlim, has_credits, lid = newest_session_rate
    as_of = dt.datetime.fromtimestamp(epoch, tz=dt.timezone.utc).strftime('%Y-%m-%d %H:%M UTC')
    resets = dt.datetime.fromtimestamp(rs, tz=dt.timezone.utc).strftime('%Y-%m-%d %H:%M UTC') if rs else '?'
    cstr = ''
    if unlim is not None:
        cstr = f'  unlimited={unlim}, has_credits={has_credits}'
    lines = [
        f'  source: most recent token_count event on disk ({as_of})',
        f'  primary used_percent: {used_pct}%   window_minutes: {wm}   limit_id: {lid}{cstr}',
        f'  resets_at (UTC):      {resets}   (window={wm/1440:.1f} day)',
        f'  ~remaining headroom:  {100 - (used_pct or 0):.1f}%',
    ]
    auth = os.path.expanduser('~/.codex/auth.json')
    if os.path.exists(auth):
        try:
            import base64
            a = json.load(open(auth))
            tok = (a.get('tokens') or {}).get('id_token')
            if tok:
                payload_b64 = tok.split('.')[1]
                payload_b64 += '=' * (4 - len(payload_b64)%4)
                pl = json.loads(base64.urlsafe_b64decode(payload_b64))
                auth_info = pl.get('https://api.openai.com/auth') or {}
                plan = auth_info.get('chatgpt_plan_type')
                renew = auth_info.get('chatgpt_subscription_active_until')
                if plan:
                    lines.append(f'  plan (from id_token claim): {plan}   renews: {renew}')
        except Exception:
            pass
    return lines

if json_out:
    print(json.dumps({
        'sessions_scanned': n_sessions,
        'sessions_with_usage': n_with_usage,
        'totals': {
            'turns': total_turns,
            'input_tokens': total_in,
            'output_tokens': total_out,
            'cached_input_tokens': total_cache,
            'reasoning_tokens': total_reason,
            'cache_hit_rate': (total_cache/total_in) if total_in else 0.0,
        },
        'by_day': [
            {'date': d,
             'turns': day_turns[d]['turns'],
             'input': day_turns[d]['in'],
             'output': day_turns[d]['out'],
             'cached_input': day_turns[d]['cache'],
             'reasoning': day_turns[d]['reason'],
             'peak_used_percent': day_peak.get(d)}
            for d in sorted(day_turns) if in_window(d)
        ],
        'latest_rate_limit': None if newest_session_rate is None else {
            'as_of_utc': dt.datetime.fromtimestamp(newest_session_rate[0], tz=dt.timezone.utc).isoformat(),
            'primary_used_percent': newest_session_rate[1],
            'window_minutes': newest_session_rate[3],
            'resets_at_utc': dt.datetime.fromtimestamp(newest_session_rate[2], tz=dt.timezone.utc).isoformat() if newest_session_rate[2] else None,
            'unlimited': newest_session_rate[4],
            'has_credits': newest_session_rate[5],
            'limit_id': newest_session_rate[6],
        },
    }, indent=2))
    sys.exit(0)

if rate_only:
    print()
    print('=== Latest rate-limit snapshot ===')
    for L in rate_snapshot_block(): print(L)
    print()
    sys.exit(0)

print()
print('Codex CLI usage — parsed from ~/.codex/sessions/*/rollout-*.jsonl')
print('(No public usage API exists for the ChatGPT-OAuth Codex auth flow.)')
print(f'sessions scanned: {n_sessions}    sessions with token_count events: {n_with_usage}')
print()
for L in out_lines: print(L)
if total_in:
    print(f'\nCache hit rate: {cache_pct(total_cache, total_in)} '
          f'({human(total_cache)} cached / {human(total_in)} input)')
print()
print('=== Latest rate-limit snapshot ===')
for L in rate_snapshot_block(): print(L)
print()
PY