#!/usr/bin/env python3
"""Benchmark streaming chat response speed on Hermes (Nous Portal) free/paid models.

Measures, per model:
  - ttft_s                  : seconds to first streamed token of ANY kind
  - time_to_first_content_s : seconds until the first actual answer ('content') token
  - total_s                 : wall-clock to stream completion
  - reasoning_chars/content_chars : split, so reasoning-heavy models don't look empty
  - out_tok_s               : content tokens / total (rough, ~4 chars/token)

Usage:
  python3 bench_speed.py                                 # benchmarks default model list
  python3 bench_speed.py "tencent/hy3:free" "stepfun/step-3.7-flash:free"
"""
import json, sys, time, urllib.request, os

AUTH = json.load(open(os.path.expanduser('/home/arctic/.hermes/auth.json')))
TOKEN = AUTH['providers']['nous']['access_token']
BASE = "https://inference-api.nousresearch.com/v1"

DEFAULT_MODELS = ["tencent/hy3:free", "stepfun/step-3.7-flash:free"]
PROMPT = ("Write a detailed 300-word explanation of how mixture-of-experts routing "
          "works in large language models, including the role of the router and active parameters.")
MAX_TOKENS = 4000  # MUST be large: reasoning models burn 1500-2800 tokens thinking first


def bench(model, prompt=PROMPT, max_tokens=MAX_TOKENS):
    payload = json.dumps({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": True,
        "max_tokens": max_tokens,
    }).encode()
    req = urllib.request.Request(BASE + "/chat/completions", data=payload, method="POST")
    req.add_header("Authorization", f"Bearer {TOKEN}")
    req.add_header("Content-Type", "application/json")
    t0 = time.time()
    ttft = tt_content = None
    provider = reason_chars = content_chars = n_reason = n_content = 0
    try:
        with urllib.request.urlopen(req, timeout=240) as r:
            for raw in r:
                line = raw.decode().strip()
                if line.startswith(": "):
                    continue  # SSE keep-alive comment (e.g. ": OPENROUTER PROCESSING")
                if not line.startswith("data:"):
                    continue
                data = line[5:].strip()
                if data == "[DONE]":
                    break
                obj = json.loads(data)
                if provider is None:
                    provider = obj.get("provider")
                delta = obj["choices"][0].get("delta", {})
                rt = delta.get("reasoning") or ""
                ct = delta.get("content") or ""
                if rt:
                    if ttft is None:
                        ttft = time.time() - t0
                    reason_chars += len(rt)
                    n_reason += 1
                if ct:
                    if ttft is None:
                        ttft = time.time() - t0
                    if tt_content is None:
                        tt_content = time.time() - t0
                    content_chars += len(ct)
                    n_content += 1
    except Exception as e:
        return {"model": model, "error": str(e)}
    total = time.time() - t0
    est_content_tok = max(1, content_chars // 4)
    return {
        "model": model, "provider": provider,
        "ttft_s": round(ttft, 2) if ttft else None,
        "time_to_first_content_s": round(tt_content, 2) if tt_content else None,
        "total_s": round(total, 2),
        "reasoning_chars": reason_chars, "content_chars": content_chars,
        "est_content_tokens": est_content_tok,
        "out_tok_s": round(est_content_tok / total, 1),
    }


if __name__ == "__main__":
    models = sys.argv[1:] or DEFAULT_MODELS
    results = []
    for m in models:
        print(f"Benchmarking {m} ...", flush=True)
        r = bench(m)
        print(json.dumps(r), flush=True)
        results.append(r)
        time.sleep(4)
    print("\n=== SUMMARY ===")
    print(json.dumps(results, indent=2))
