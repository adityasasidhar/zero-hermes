# MiniMax API quirks (verified live, July 2026)

## Endpoint paths
- Chat (OpenAI-compat): `POST https://api.minimax.io/v1/chat/completions`
- Image gen: `POST https://api.minimax.io/v1/image_generation` (model `image-01`)
- Token-plan usage/quota: `GET https://api.minimax.io/v1/token_plan/remains`
  (alias `GET /v1/coding_plan/remains` returns identical payload)

## `/token_plan/remains` field semantics (MISLABELED by MiniMax)
Documented bug (MiniMax-M2 issue #99): field names lie.
- `current_interval_usage_count` => actually REMAINING requests in 5h window
- `current_weekly_usage_count` => actually REMAINING requests in weekly window
- On the NEW token plan these `*_count` fields are 0 (token-based, not request-based).
- Real signal: `remains_time` + `*_remaining_percent` per bucket (`model_name`).
  Buckets seen: `general` (text/LLM) and `video`. No separate `image` bucket;
  image gen draws from `general`.
- Estimate used tokens: `used ≈ remains_time / pct * (100 - pct) / 100`.

## `thinking` parameter
- Takes an OBJECT (`ThinkingConfig`), NOT a bare bool/string.
  `{"thinking": false}` => HTTP 400 "Mismatch type ... ThinkingConfig".
- Omit the field for default behavior; M3 default produced no separate reasoning
  stream in our tests (starts answering immediately).

## Caching (passive / automatic)
- Supported on MiniMax-M3 (and M2.x). No code change needed.
- Confirmed LIVE: identical ~12k-token prefix => 100% cache hit on 2nd+ call.
- Threshold: short prefixes (~200 tok) do NOT cache (cached_tokens=0). Keep the
  repeated prefix (system prompt + tools + doc context) long + identical.
- Usage field: `prompt_tokens_details.cached_tokens`.

## Image generation
- `model: "image-01"`, `aspect_ratio` (e.g. "1:1","16:9"), `n`, `response_format`
  ("url" or "base64"). I2I via `subject_reference`. Batch up to 90/session.

## Where the API key lives for this user
- NOT in Hermes `~/.hermes/.env` (MINIMAX_API_KEY is commented out there) and NOT in
  `~/.hermes/auth.json` (no minimax provider). It lives in a project .env:
  `/home/arctic/Internship/DataObserve/do-code/.env` as `MINIMAX_API_KEY=sk-cp-...`.
- Source it transiently per-command; never print or persist the value.
