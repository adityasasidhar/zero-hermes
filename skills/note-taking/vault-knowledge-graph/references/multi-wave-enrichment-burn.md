# Multi-wave vault-enrichment burn — parent orchestration playbook

Companion to `aiml-wiki-deep-enrichment.md` (which is the leaf-agent's contract).
This file covers the **parent agent's job**: dispatching 10–100 leaf subagents
across multiple waves against a goal like *"deepen all 50 notes in the aiml
wiki from 1.5k → 6–10k chars over a 4-hour wall-clock budget"*.

Session-evidenced 2026-07-28 / 2026-07-29: an aiml-wiki burn took two attempts
to land. The first attempt lost 9 of 12 wave files to `/tmp` snap recovery and
the parent-side batch wrapper crashing. The second attempt used the recipes
here and shipped 41 subagents / 50 enriched notes / `broken=0` in ~4.5 hours.

## The minimum-viable recipe

```text
# Setup (once)
mkdir -p /home/arctic/projects/<burn-name>-tmp/wave{0,1,2,...,N}
write /home/arctic/projects/<burn-name>-tmp/BRIEF-TEMPLATE.md   # reusable per task
python3 wiki/build_index.py --check 2>&1 | head -3 \
    > /home/arctic/projects/<burn-name>-tmp/baseline-stdout.txt

# Loop: one subagent per dispatch (NOT batched)
for target in <list of NOTE_PATHs>; do
    delegate_task(goal="<full brief with NOTE_PATH substituted>")
    # Wait for completion signal (single-task mode is reliable).
    # cp /home/arctic/projects/<burn-name>-tmp/waveN/<basename>.md \
    #    /home/arctic/Documents/fun/wiki/aiml/<...>.md
done

# Closeout
python3 wiki/build_index.py --check                # confirm broken=0
cat >> /home/arctic/Documents/fun/wiki/aiml/log.md <<EOF
## [date] ... burn summary
EOF
```

Five components, in order:

1. **Durable scratch dir** (not `/tmp/`).
2. **Single-task dispatch** (not batched).
3. **Poll-and-apply** (not end-of-wave batch apply).
4. **HTTP 429 re-dispatch** (not abort).
5. **Log entry + final diagnostic** (closeout gate).

## Why the recipe can't shortcut any of the five

### 1. Durable scratch dir

`/tmp` on this user's setup is a tmpfs that was wiped by a snap recovery
event at 2026-07-28 23:26 IST. Wave 0 outputs of the first burn survived
only because parent had `cp`'d them to the real wiki before the wipe.
Waves 1–4 outputs (9 subagent deliverables, 100% complete per their
transcripts) were permanently lost. Use `/home/arctic/projects/<burn-name>-tmp/`
— durable disk that survives snap/systemd recovery events.

### 2. Single-task dispatch

Current Hermes's parent-side batch wrapper is unreliable. Symptom on every
multi-task `delegate_task(goals=[a, b, c])` call:

```
--- ERROR ---
The batch did not complete successfully: Delegation owner exited
before recording a terminal result; outcome unknown.
```

…even when every subagent in the batch completed cleanly (their files are
on disk, their live transcripts end with `status=completed`, and re-dispatching
any individual task returns a clean result inline).

**Workaround**: dispatch one task per call.

```python
for target in targets:
    delegate_task(goal="<brief with one target substituted>")
    # Each returns a full result inline; no batch signal needed.
    # Parent can apply between iterations.
```

This is slower than batching (~1 subagent returned per call vs. N per call),
but it's the only mode where the per-task result reliably lands in the
parent's context. The "lost 9 wave files" failure in round 1 was *both*
the snap recovery AND the batch wrapper — the recovery wiped the files
before parent could `cp` them, AND the wrapper failed to surface the
batch completion so parent never knew `cp` was needed.

### 3. Poll-and-apply between iterations

Don't `cp` everything at end-of-wave. Instead:

```bash
# After EACH delegate_task returns, immediately:
cp /home/arctic/projects/<burn-name>-tmp/waveN/<basename>.md \
   /home/arctic/Documents/fun/wiki/aiml/<...>.md
# Then run a quick `build_index.py --check` to confirm broken
# didn't spike. If broken > baseline+0, the subagent wrote a
# bad wikilink — patch it before dispatching the next task.
```

The `broken=0 → broken=1 → broken=0` cycle during round 2 was caught
this way: the `small-language-model` entity enrichment introduced
`[[Concepts/RoPE (Rotary Position Embeddings)|RoPE]]`, which doesn't
resolve in the aiml wiki. Parent caught it within ~30 seconds of the
subagent returning and patched back to `[[rope-positional-encoding|RoPE]]`
before dispatching the next 8 subagents. If we'd batched the verify at
end-of-wave, all 8 subsequent subagents could have inherited the same
bad-link habit (or worse, propagated it).

### 4. HTTP 429 re-dispatch, not abort

When a subagent's transcript ends with `exit_reason: max_iterations`
and a stale or missing PASS line, that's the rate-limit signature.
Practical moves:

- **The file on disk may still be usable** (≥6 KB, has the right schema).
  Apply it directly. Don't re-dispatch if the content is good.
- If the file is incomplete, **wait** for the rate-limit window to clear
  (usually 1–5 minutes), then re-dispatch with the SAME brief. The
  second pass usually converges cleanly — your first subagent did the
  research, second has a cleaner starting point.
- Don't change the brief to reduce scope. Rate-limit retry isn't a
  scope problem.

Round 2 hit 429 twice (`deepseek-v3-technical-report`,
`block-sparse-vs-fine-grained-attention`); both files surfaced with
~12–14 KB of content and were applied directly. The re-dispatches
succeeded with no brief changes.

### 5. Closeout: log entry + final diagnostic

The aiml wiki's own `SCHEMA.md` declares *"Every action must be appended to
`log.md`"*. A multi-wave burn qualifies. The log entry should record:

- Date / time
- Total notes enriched, total bytes added
- Per-wave pass/fail (or aggregated pass count)
- Notable issues encountered (rate limits, broken links patched, etc.)

Save it before declaring done. Then run one last `build_index.py --check`
to confirm `broken=0` end-to-end.

## Pitfalls the recipe doesn't fix by itself

- **The brief template must be tight on wikilink targets.** Subagents
  faithfully copy whatever the brief lists. If you list
  `[[Concepts/RMSNorm]]` (cross-vault bridge style), you'll get
  `[[Concepts/RMSNorm]]` in the output, and `build_index.py` will
  report it as broken because the aiml wiki resolver doesn't walk
  `Concepts/` namespace folders. List targets as bare basenames that
  resolve in the aiml wiki (`[[rmsnorm]]`, not `[[Concepts/RMSNorm]]`).

- **Frontmatter preservation discipline doesn't auto-transfer.**
  Each subagent must read its own brief's "preserve these fields verbatim"
  section. Round-2 brief was explicit about `counterpart:`, `Part of [[...]]`,
  and the `raw/articles` four-field schema vs the concept/entity schema
  — but a few subagents still missed nuance. If the brief has *n*
  preservation rules, expect ~10% of subagents to drop one. The
  verifier script catches most of these.

- **Trim-loop convergence is slow.** Concept notes with rich math
  content routinely come in at 14–18 KB on first draft when the brief
  caps at 10 KB. Each subagent spends 3–10 iterations trimming. Plan
  wall-clock accordingly — ~5–15 minutes per subagent for reasoning-
  heavy output, with rate-limit pauses on top.

- **PASS-line truthfulness is the leaf's job, not the parent's.**
  Catch it with `python3 scripts/verify_aiml_wiki_output.py
  /home/arctic/projects/<burn-name>-tmp/waveN/<basename>.md` — exits
  non-zero if the PASS line drifts from the actual byte count.
  Several subagents shipped with placeholder PASS lines
  (`PASS: <bytes> bytes, <outbound_links> outbound links`) that grep
  accepts but humans can't parse. Apply those files anyway, but
  patch the PASS line manually before moving on, or ship a note in
  the closeout log.

- **Sync-pool fallback ≠ parallelism.** A `max_concurrent_children: 10`
  config does not give 10-way parallelism when the pool is full —
  subagents fall into a synchronous queue. The 41-subagent aiml
  burn sustained ~6 min/subagent effective throughput regardless
  of advertised concurrency. Plan wall-clock off ~6 min/subagent
  for reasoning-heavy work, NOT off `concurrency × per-task-time`.

## The one-sentence summary

A multi-wave vault-enrichment burn is `delegate_task + cp + build_index
verify` looped 10–100 times. The leaf agent's contract is documented in
`references/aiml-wiki-deep-enrichment.md`; the *parent's* contract is:
durable scratch dir, single-task dispatch, poll-and-apply between
iterations, re-dispatch on rate limit, closeout with log entry.