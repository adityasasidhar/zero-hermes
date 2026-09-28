#!/usr/bin/env python3
"""Live streaming speed/usage benchmark for any OpenAI-compatible chat endpoint.

Measures: time-to-first-token, time-to-first-content-token, total wall-clock,
reasoning chars, content chars, est content tok/s. Handles SSE comments and
reasoning streams. Run: python3 scripts/bench_speed.py
"""
import json
import time
import urllib.request


def bench(url, key, model, prompt, max_tokens=1500, extra_body=None,
          stream=True, headers=None):
    body = {"model": model, "messages": [{"role": "user", "content": prompt}],
            "stream": stream, "max_tokens": max_tokens}
    if extra_body:
        body.update(extra_body)
    payload = json.dumps(body).encode()
    req = urllib.request.Request(url, data=payload, method="POST")
    req.add_header("Authorization", f"Bearer {key}")
    req.add_header("Content-Type", "application/json")
    for h, v in (headers or {}).items():
        req.add_header(h, v)
    t0 = time.time()
    ttft = None
    tt_content = None
    rc = cc = 0
    try:
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
                d = o["choices"][0]["delta"]
                rt = d.get("reasoning") or ""
                ct = d.get("content") or ""
                if rt or ct:
                    if ttft is None:
                        ttft = time.time() - t0
                if rt:
                    rc += len(rt)
                if ct:
                    if tt_content is None:
                        tt_content = time.time() - t0
                    cc += len(ct)
    except Exception as e:
        return {"error": str(e)}
    total = time.time() - t0
    est = max(1, cc // 4)
    return {
        "ttft_s": round(ttft, 2) if ttft else None,
        "tt_content_s": round(tt_content, 2) if tt_content else None,
        "total_s": round(total, 2),
        "reason_chars": rc,
        "content_chars": cc,
        "est_content_tokens": est,
        "content_tok_s": round(est / total, 1),
    }


if __name__ == "__main__":
    AUTH = json.load(open("/home/arctic/.hermes/auth.json"))
    TOKEN = AUTH["providers"]["nous"]["access_token"]
    BASE = "https://inference-api.nousresearch.com/v1"
    PROMPT = ("Write a detailed 300-word explanation of how mixture-of-experts routing "
              "works in large language models, including the role of the router and "
              "active parameters.")
    for m in ["tencent/hy3:free", "stepfun/step-3.7-flash:free"]:
        print(m, bench(f"{BASE}/chat/completions", TOKEN, m, PROMPT))
