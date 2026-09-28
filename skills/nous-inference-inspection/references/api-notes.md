# Nous Inference API — condensed findings (live probe, 2026-07-17)

Base URL (Nous provider): `https://inference-api.nousresearch.com/v1`
Active model in session: `tencent/hy3:free` (provider `nous`).

## Endpoints
- `GET /v1/models` — public, no auth. Returns OpenAI-wire catalog: `{ "data": [ {model records} ] }`.
- `POST /v1/chat/completions` — requires auth (Nous Portal JWT or x402 payment). Unauth → `402`.

## Unauthenticated chat request → HTTP 402
Headers include `x-402-accept: solana`. Body (x402 v1):
```json
{
  "x402Version": 1,
  "accepts": [{
    "scheme": "exact", "network": "solana", "maxAmountRequired": "0",
    "resource": "https://inference-api.nousresearch.com/v1/chat/completions",
    "description": "Nous Research API inference request for model tencent/hy3:free",
    "asset": "EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v"
  }],
  "error": "Payment required. Please provide either a valid Authorization header or x402 payment."
}
```
`maxAmountRequired: "0"` ⇒ free model is genuinely $0, but gated by OAuth.

## Example free model record (`/v1/models`, filtered)
```json
{
  "id": "tencent/hy3:free",
  "canonical_slug": "tencent/hy3-20260706",
  "context_length": 262144,
  "pricing": { "prompt": "0", "completion": "0", "input_cache_read": "0" },
  "top_provider": { "context_length": 262144, "max_completion_tokens": 131072, "is_moderated": false },
  "per_request_limits": null,
  "supported_parameters": ["frequency_penalty","include_reasoning","logit_bias","max_tokens",
    "min_p","presence_penalty","reasoning","reasoning_effort","repetition_penalty","response_format",
    "seed","stop","structured_outputs","temperature","tool_choice","tools","top_k","top_p"],
  "reasoning": { "mandatory": false, "default_enabled": false,
    "supported_efforts": ["high","low","none"], "default_effort": "high" },
  "synthesizedFreeVariant": true
}
```
Paid sibling `tencent/hy3` has the same shape but `pricing` > 0 and no `synthesizedFreeVariant`.

## Key takeaways
- Free = $0 cost, NOT unlimited throughput. `per_request_limits: null` everywhere → no published cap.
- No RPM/quota figure is documented anywhere (docs or metadata). Only authenticated response `x-ratelimit-*` headers reveal live limits.
- Free variants ride a shared throttled pool; heavy/parallel work (e.g. many delegate_task subagents) will 429 → use a paid model there.
- `hermes portal info` confirms auth without exposing secrets.
