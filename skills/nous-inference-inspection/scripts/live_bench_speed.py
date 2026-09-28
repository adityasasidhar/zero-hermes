#!/usr/bin/env python3
"""Live speed benchmark for FREE models served through Hermes' Nous Portal.

WHY THIS EXISTS
  Aggregate benchmark sites (Artificial Analysis, OpenRouter) measure *paid/first-party*
  serving. The free `:free` variants on Nous Portal route through throttled/shared pools
  (Novita, OpenRouter->StepFun, etc.) and DO NOT match those published numbers. Measure
  the real thing before trusting a "X is 8x faster" headline.

GOTCHA (the trap that wastes a run)
  Step 3.7 Flash (and other reasoning models) emit THINKING tokens in a `reasoning`
  field BEFORE the answer. A naive benchmark that only counts `delta.content` will show
  ~0 chars and conclude the model is broken. Count BOTH `reasoning` and `content`.
  Also: SSE lines starting with ": " are comments (e.g. ": OPENROUTER PROCESSING") -- skip them.

USAGE
  python3 live_bench_speed.py [--models m1,m2] [--prompt "..."] [--max-tokens 1500] [--repeats 1]

Outputs one JSON line per model: ttft, time_to_first_content, total_s, reasoning_chars,
content_chars, est_content_tokens, out_tok_s.
"""
import json, time, urllib.request, argparse, os

AUTH = os.path.expanduser("~/.hermes/auth.json")
BASE = "https://inference-api.nousresearch.com/v1"

def load_token():
    d = json.load(open(AUTH))
    return d["providers"]["nous"]["access_token"]

def bench(model, prompt, max_tokens):
    payload = json.dumps({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "stream": True,
        "max_tokens": max_tokens,
    }).encode()
    req = urllib.request.Request(BASE + "/chat/completions", data=payload, method="POST")
    req.add_header("Authorization", f"Bearer {load_token()}")
    req.add_header("Content-Type", "application/json")
    t0 = time.time()
    ttft = None            # first token of ANY kind
    tt_content = None      # first actual answer token
    provider = None
    reasoning_chars = content_chars = 0
    n_reason = n_content = 0
    with urllib.request.urlopen(req, timeout=240) as r:
        for line in r:
            line = line.decode().strip()
            if line.startswith(": "):
                continue
            if not line.startswith("data:"):
                continue
            data = line[5:].strip()
            if data == "[DONE]":
                break
            o = json.loads(data)
            if provider is None:
                provider = o.get("provider")
            delta = o["choices"][0].get("delta", {})
            rt = delta.get("reasoning") or ""
            ct = delta.get("content") or ""
            if rt:
                if ttft is None: ttft = time.time() - t0
                reasoning_chars += len(rt); n_reason += 1
            if ct:
                if ttft is None: ttft = time.time() - t0
                if tt_content is None: tt_content = time.time() - t0
                content_chars += len(ct); n_content += 1
    total = time.time() - t0
    est_content_tok = max(1, content_chars // 4)
    return {
        "model": model, "provider": provider,
        "ttft_s": round(ttft, 2) if ttft else None,
        "time_to_first_content_s": round(tt_content, 2) if tt_content else None,
        "total_s": round(total, 2),
        "reasoning_chars": reasoning_chars, "content_chars": content_chars,
        "est_content_tokens": est_content_tok,
        "out_tok_s_content_only": round(est_content_tok / total, 1),
    }

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--models", default="tencent/hy3:free,stepfun/step-3.7-flash:free")
    ap.add_argument("--prompt", default=("Write a detailed 300-word explanation of how "
        "mixture-of-experts routing works in large language models, including the role "
        "of the router and active parameters."))
    ap.add_argument("--max-tokens", type=int, default=1500)
    ap.add_argument("--repeats", type=int, default=1)
    a = ap.parse_args()
    models = [m.strip() for m in a.models.split(",")]
    for m in models:
        for _ in range(a.repeats):
            print(json.dumps(bench(m, a.prompt, a.max_tokens)), flush=True)
            time.sleep(4)

if __name__ == "__main__":
    main()
