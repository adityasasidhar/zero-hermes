# Nous Inference API — observed headers & model facts
Captured 2026-07-17 against `https://inference-api.nousresearch.com/v1`.
Re-run the probe (see SKILL.md) to refresh; numbers can change server-side.

## Free-tier rate-limit headers (tencent/hy3:free)
Rate limits are ACCOUNT-scoped and shared across ALL free models.
```
x-ratelimit-limit-requests: 50            # short window
x-ratelimit-limit-requests-1h: 2100
x-ratelimit-limit-tokens: 500000
x-ratelimit-limit-tokens-1h: 6000000
x-ratelimit-remaining-requests: ~43       # ~75 req/min burst, ~35/min sustained
x-ratelimit-remaining-requests-1h: ~2045
x-ratelimit-remaining-tokens: ~499434
x-ratelimit-remaining-tokens-1h: ~5998267
x-ratelimit-reset-requests: ~40s
x-ratelimit-reset-requests-1h: ~2919s
x-ratelimit-reset-tokens: ~38s
x-ratelimit-reset-tokens-1h: ~2917s
```

## Paid credit envelope (x-nous-credits-*)
```
x-nous-credits-paid-access: false         # true once a paid plan is active
x-nous-credits-remaining-usd: 0.10        # starter envelope on free tier
x-nous-credits-subscription-limit-usd: 0.10
x-nous-credits-disabled-reason: out_of_credits   # on the free tier
```
A paid plan flips `paid-access: true` and raises `remaining-usd`.

## 404 message (paid model, empty credits)
```
{"status":404,"message":"Model 'minimax/minimax-m3' requires available credits.
Your account balance is too low to use paid models — add credits at
https://portal.nousresearch.com or pick a free model."}
```

## Model comparison (Design Arena ELO where available)
| Model | ctx | max_out | pricing in/out per tok | reasoning | modality | notes |
|---|---|---|---|---|---|---|
| tencent/hy3:free | 262144 | 131072 | $0 / $0 | optional (high/low/none) | text->text | free; codecategories ELO 1232 |
| tencent/hy3 | 262144 | 131072 | $0.0000002 / $0.0000008 | optional | text->text | paid; same benchmarks as free |
| minimax/minimax-m3 | 1048576 | 512000 | $0.0000003 / $0.0000012 | optional (off default) | text->text | AI coding idx 58.6; 1M ctx |
| stepfun/step-3.7-flash:free | 256000 | 256000 | $0 / $0 | mandatory (high/med/low) | text+image+video->text | multimodal free |

### hy3:free vs minimax-m3 (shared categories, +ELO to M3)
- 3d: 1251 -> 1293 (+42)
- codecategories: 1232 -> 1293 (+61)
- dataviz: 1210 -> 1277 (+67)
- gamedev: 1190 -> 1280 (+90)
- uicomponent: 1242 -> 1284 (+42)
- website: 1220 -> 1291 (+71)

M3 wins every shared category (avg ~+62 ELO); flips win-rate from ~44% to
~54-56%. Bigger upside: 4x context (1M vs 262K) and 4x max output (512K vs 131K).
Not a frontier leap (AI coding idx 58.6 vs GPT-5.6/Kimi-K3 class 70+), but
clearly above the free model - worth it for long-context/repo-level ML work.
