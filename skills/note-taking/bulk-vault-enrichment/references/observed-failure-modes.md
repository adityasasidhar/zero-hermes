# Observed Failure Modes — Bulk Vault Enrichment Burns

Concrete transcripts and recovery patterns from real burns. Each section
includes the diagnostic command, what the symptom looked like, the
root cause, and the fix applied.

---

## 1. `/tmp/` wipe mid-burn (snap/systemd recovery)

**Date:** 2026-07-28 23:26 IST

**Symptom:** 9 subagent outputs that had been written to
`/tmp/aiml-burn/wave{1..4}/` between 23:21 and 23:25 IST vanished when a
snap recovery event ran. Disk recovery also wiped `/home/arctic/Documents/fun/wiki/INDEX.json`
temporarily.

**Root cause:** Linux `/tmp` is tmpfs by default
(`mount | grep "on /tmp "` returns `tmpfs on /tmp type tmpfs`).
tmpfs lives in volatile system memory; snapd's auto-recovery on store
refresh can flush it. Not a hermes issue, not a subagent issue — just
tmpfs.

**Diagnostic:**
```
mount | grep "on /tmp "
# tmpfs on /tmp type tmpfs (rw,nosuid,nodev,...)
```

**Fix:** Burn-mode scratch must be on durable disk. Use
`<user-projects>/<task>-tmp/` (e.g.
`/home/arctic/projects/aiml-burn-tmp/`). Hard-code this path in the
brief template — never let the subagent choose.

---

## 2. Parent-side batch wrapper crash on multi-task dispatch

**Date:** 2026-07-28 ~23:21 IST, repeated 2026-07-29 13:56–18:30 IST.

**Symptom:** Every multi-task `delegate_task` batch that completes
reports:

```
[ASYNC DELEGATION BATCH COMPLETE — deleg_<id>]
--- ERROR ---
The batch did not complete successfully: Delegation owner exited
before recording a terminal result; outcome unknown.
```

…even when the individual subagent transcripts in
`/home/arctic/.hermes/cache/delegation/live/<id>/task-N.log` show clean
`status=completed` exits with `PASS:` lines written.

**Root cause:** The parent-side aggregation wrapper that summarizes a
batch into one chat message can crash mid-aggregate on long-running
subagent bursts (esp. when the third+ subagent in a batch takes 10+
minutes). It does not affect the subagents themselves.

**Diagnostic:**
```
tail -3 /home/arctic/.hermes/cache/delegation/live/<deleg_id>/task-*.log
# Look for "status=completed" in the final assistant message,
# vs the wrapper's "owner exited" notification.
```

**Fix:**
1. Don't trust the wrapper's batch completion text. Verify each
   subagent output via `ls <scratch-dir>/waveN/`.
2. Switch to **single-task `delegate_task` calls** (`tasks=undefined`
   in arg schema — just pass `goal:` and `context:`) when the batch
   wrapper starts failing. Slower, but predictable: each subagent
   result returns inline in its own wrapper message.
3. Append `Single-task dispatch: do NOT batch in this conversation.`
   to the brief as a hint to the subagent.

---

## 3. Pool saturation → "synchronous" inline return

**Date:** 2026-07-29 ~13:58 IST.

**Symptom:** A dispatched batch returns with text like:

```
[results from sub-agent ...] status=completed ...
note: The background delegation pool was at capacity
(delegation.max_concurrent_children), so the subagent(s) ran
SYNCHRONOUSLY and the result is included above.
```

**Root cause:** `delegation.max_concurrent_children` (default 10) is
full from prior in-flight delegations. The new batch ran serially and
returned inline.

**Implication:** **Concurrency is effectively 1 not 10 in this codebase**
during long burns. Throughput is bounded by wall-clock, not concurrency.
Plan accordingly (see Token budgeting in SKILL.md).

**Diagnostic:**
```
grep max_concurrent_children ~/.hermes/config.yaml
# Should print 10 (or whatever you set)
```

**Fix:** Plan wall-clock = `notes × avg_subagent_time ÷ effective_concurrent`.
For 50 notes at 10 min each on effective_concurrent=3: ~2.5–4 hours.

---

## 4. Subagent exit with HTTP 429 tail error after content written

**Date:** 2026-07-29 (multiple), across ~5 subagent runs.

**Symptom:** A subagent reports:

```
status=completed (or max_iterations)
exit_reason=max_iterations
Final response: "API call failed after 3 retries: HTTP 429: Token
Plan usage limit reached: ..."
```

…BUT the temp file at `<scratch-dir>/waveN/<basename>.md` is fully
written at the expected size with substantively correct content. The
HTTP 429 hit on a final API call (probably the call that would have
written the PASS line) — the file write happened in a prior call.

**Diagnostic:**
```
ls -la <scratch-dir>/waveN/<basename>.md
wc -c <scratch-dir>/waveN/<basename>.md
tail -5 <scratch-dir>/waveN/<basename>.md
# If file exists at the target size with real content, apply it.
```

**Fix:** Don't retry a fresh subagent (it'll just consume more tokens).
Apply the file as-is. The PASS line at the bottom may be a placeholder
(`PASS: <bytes> bytes, <outbound_links> outbound links`) — that's OK,
parent-side `wc -c` is the authoritative size.

---

## 5. Subagent over-shoots byte cap (10 KB → 12–18 KB)

**Date:** 2026-07-29 (multiple), ~1/3 of subagents.

**Symptom:** Several subagents produce notes 11–18 KB instead of the
6–10 KB target. They report this in their final summary:
"Output is X bytes, slightly above the stated 6,000–10,000 target."

**Root cause:** Subagents prioritize "complete mechanism + math + all
required cross-links" over artificial truncation. Trim passes can push
a 13 KB note to 9 KB but cost multiple iteration cycles.

**Fix:**
1. Accept over-shoots in bulk-mode burns. The wiki content is the
   goal; size is a soft target.
2. Add `## Trim-pass targets` to the morning report listing
   over-cap notes for a future, focused single-pass trim.
3. If you have plenty of budget later: do a "round 2" burn with
   `Maximum 7000 bytes` in the brief and let subagents fight harder
   for density.

---

## 6. Subagent introduces cross-vault-bridge wikilinks in body

**Date:** 2026-07-29, `entities/small-language-model.md`.

**Symptom:** Subagent wrote `[[Concepts/RoPE (Rotary Position
Embeddings)|RoPE]]` in the body. The resolver doesn't walk the
`Concepts/` namespace in the aiml wiki, so this is a broken link.

**Diagnostic:**
```
python3 /home/arctic/Documents/fun/wiki/build_index.py --check | grep -i 'BROKEN'
# Returns the broken line with note:path
```

**Fix:** Patch to bare basename: `[[rope-positional-encoding|RoPE]]`.

**Why it happened:** The original (pre-enrichment) note had
`[[rope-positional-encoding|RoPE]]` already, but during the
11-section rewrite the subagent saw `Concepts/RMSNorm.md` and
`Concepts/SwiGLU.md` in nearby notes (those DO resolve as cross-vault
bridges) and standardized on that namespace. It didn't realize that
`Concepts/RoPE (...)` doesn't exist as a file. The brief should forbid
this pattern:

```
DO NOT use [[Concepts/<X>]] wikilinks in body content. Only the
existing counterpart: fields may reference cross-vault bridges.
```

Encode this in the brief template's "Constraints" section.

---

## Diagnostic command summary

```bash
# Scratch dir state
ls -la /home/arctic/projects/aiml-burn-tmp/wave*/

# Wiki health
cd /home/arctic/Documents/fun
python3 wiki/build_index.py --check 2>&1 | head -5
echo "broken=$(python3 wiki/build_index.py --check 2>&1 | head -1)"

# Live subagent activity
ls /home/arctic/.hermes/cache/delegation/live/ | wc -l
tail -3 /home/arctic/.hermes/cache/delegation/live/<deleg_id>/task-*.log

# /tmp volatility check
mount | grep "on /tmp "

# Token spend estimate (rough)
grep -h '"input"\|"output"' /home/arctic/.hermes/cache/delegation/live/*/task-*.log \
  | python3 -c "import json,sys,re; tot_in=tot_out=0
for line in sys.stdin:
  m = re.search(r'\"input\":\s*(\d+).*?\"output\":\s*(\d+)', line)
  if m: tot_in+=int(m.group(1)); tot_out+=int(m.group(2))
print(f'tokens: input={tot_in:,} output={tot_out:,} total={tot_in+tot_out:,}')"
```
