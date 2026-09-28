"""Tally provider usage from the Hermes local DB (no network calls).

Run with:
  uv run --with aiosqlite python3 hermes_usage_tally.py [--model minimax] [--provider minimax-oauth] [--days 14]

Prints:
  - Per-model totals (calls, in/out/reasoning/cache_read/cache_write, providers, first/last seen)
  - Per-day buckets (UTC)
  - Grand total
  - USD estimate using public MiniMax-M3 list pricing (clearly labeled as estimate)

Public pricing baked in here (mid-2026):
  input $3 / cache_read $0.30 / output $15 / cache_write $3.75 per 1M.
Override with --price-input / --price-output / --price-cache-read / --price-cache-write.
Reasoning tokens are billed as output.

This script reads from ~/.hermes/state.db (table session_model_usage) and
does NOT make any network calls.
"""
import argparse, asyncio, aiosqlite
from datetime import datetime, timezone
from collections import defaultdict


async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--db", default="/home/arctic/.hermes/state.db")
    ap.add_argument("--model", default="%minimax%", help="SQL LIKE pattern on model name")
    ap.add_argument("--provider", default=None, help="optional exact billing_provider filter")
    ap.add_argument("--days", type=int, default=0, help="if >0, only show last N days (UTC)")
    ap.add_argument("--price-input", type=float, default=3.0, help="USD per 1M non-cached input tokens")
    ap.add_argument("--price-cache-read", type=float, default=0.30)
    ap.add_argument("--price-cache-write", type=float, default=3.75)
    ap.add_argument("--price-output", type=float, default=15.0)
    args = ap.parse_args()

    where = "LOWER(model) LIKE LOWER(?)"
    params = [args.model]
    if args.provider:
        where += " AND billing_provider = ?"
        params.append(args.provider)
    sql = f"SELECT * FROM session_model_usage WHERE {where} ORDER BY last_seen DESC"

    db = await aiosqlite.connect(args.db)
    db.row_factory = aiosqlite.Row
    cur = await db.execute(sql, params)
    rows = await cur.fetchall()
    print(f"matching_rows={len(rows)}\n")

    def ts(t):
        try:
            return datetime.fromtimestamp(float(t), tz=timezone.utc).strftime("%Y-%m-%d %H:%M")
        except Exception:
            return "?"

    by_model = defaultdict(lambda: {
        "calls":0,"input":0,"output":0,"reasoning":0,"cache_read":0,"cache_write":0,
        "est_cost":0.0,"act_cost":0.0,"first":None,"last":None,"sessions":0,"providers":set()
    })
    by_day = defaultdict(lambda: {"calls":0,"in":0,"out":0,"cr":0,"cw":0,"r":0})
    sessions = set()

    for r in rows:
        d = dict(r)
        a = by_model[d["model"]]
        a["sessions"] += 1
        a["calls"]          += d.get("api_call_count") or 0
        a["input"]          += d.get("input_tokens") or 0
        a["output"]         += d.get("output_tokens") or 0
        a["reasoning"]      += d.get("reasoning_tokens") or 0
        a["cache_read"]     += d.get("cache_read_tokens") or 0
        a["cache_write"]    += d.get("cache_write_tokens") or 0
        a["est_cost"]       += d.get("estimated_cost_usd") or 0
        a["act_cost"]       += d.get("actual_cost_usd") or 0
        a["providers"].add(d.get("billing_provider") or "?")
        fs, ls = d.get("first_seen"), d.get("last_seen")
        if fs and (a["first"] is None or fs < a["first"]): a["first"] = fs
        if ls and (a["last"]  is None or ls > a["last"]):  a["last"]  = ls
        sessions.add(d.get("session_id"))
        if ls:
            day = datetime.fromtimestamp(float(ls), tz=timezone.utc).strftime("%Y-%m-%d")
            bd = by_day[day]
            bd["calls"] += d.get("api_call_count") or 0
            bd["in"]    += d.get("input_tokens") or 0
            bd["out"]   += d.get("output_tokens") or 0
            bd["cr"]    += d.get("cache_read_tokens") or 0
            bd["cw"]    += d.get("cache_write_tokens") or 0
            bd["r"]     += d.get("reasoning_tokens") or 0

    print("== per model ==")
    g = {"calls":0,"input":0,"output":0,"reasoning":0,"cache_read":0,"cache_write":0,"est":0.0,"act":0.0}
    for m, a in sorted(by_model.items()):
        tot = a["input"] + a["output"]
        for k in ("calls","input","output","reasoning","cache_read","cache_write"):
            g[k] += a[k]
        g["est"] += a["est_cost"]; g["act"] += a["act_cost"]
        print(f"{m[:34]:34s}  sessions={a['sessions']:3d}  calls={a['calls']:5d}  "
              f"in={a['input']:>11,}  out={a['output']:>10,}  reasoning={a['reasoning']:>10,}  "
              f"cache_read={a['cache_read']:>11,}  cache_write={a['cache_write']:>10,}  "
              f"total={tot:>11,}  providers={','.join(sorted(a['providers']))}  "
              f"first={ts(a['first'])}  last={ts(a['last'])}")

    print(f"\nGRAND TOTAL  unique_sessions={len(sessions)}  api_calls={g['calls']:,}  "
          f"input={g['input']:,}  output={g['output']:,}  reasoning={g['reasoning']:,}  "
          f"cache_read={g['cache_read']:,}  cache_write={g['cache_write']:,}  "
          f"total_inout={g['input']+g['output']:,}  "
          f"hermes_est_cost=${g['est']:.4f}  hermes_actual_cost=${g['act']:.4f}")

    days_to_show = sorted(by_day)
    if args.days > 0:
        cutoff = datetime.now(timezone.utc).timestamp() - args.days * 86400
        days_to_show = [d for d in days_to_show
                        if datetime.strptime(d, "%Y-%m-%d").replace(tzinfo=timezone.utc).timestamp() >= cutoff]
    print("\n== by day (UTC) ==")
    for day in days_to_show:
        a = by_day[day]
        tot = a["in"] + a["out"]
        print(f"{day}  calls={a['calls']:5d}  in={a['in']:>10,}  out={a['out']:>9,}  "
              f"reasoning={a['r']:>9,}  cache_read={a['cr']:>10,}  cache_write={a['cw']:>9,}  "
              f"total={tot:>11,}")

    billable_input = max(0, g["input"] - g["cache_read"])
    est_usd = (
        billable_input * args.price_input
        + g["cache_read"] * args.price_cache_read
        + g["cache_write"] * args.price_cache_write
        + (g["output"] + g["reasoning"]) * args.price_output
    ) / 1_000_000
    print(f"\nESTIMATE (public list pricing; NOT authoritative - provider console is)  ~${est_usd:.4f}")
    print(f"  billable_input = max(0, input - cache_read) = {billable_input:,}")

    await db.close()


if __name__ == "__main__":
    asyncio.run(main())