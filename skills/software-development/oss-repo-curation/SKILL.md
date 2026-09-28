---
name: oss-repo-curation
description: Curate a verified-active list of third-party GitHub repos a specific contributor could dig into. Use when the user asks for "a list of repos to contribute to", "overnight OSS targets", "what should I work on this weekend", "active repos for X stack/profile", or wants a rank-ordered recommendation set with one concrete "thing to do" per repo. Covers candidate discovery, freshness verification via the GitHub API, license/star-band filtering, and the structural shape of the deliverable.
---

# OSS repo curation for a contributor

The user has a profile (skill level, stack, hardware, interests). The deliverable is a list of currently-maintained third-party repos with one concrete contribution vector each. Quality > quantity. Every repo must be verified live.

## When to use

Trigger phrases: "list of repos I can contribute to", "overnight OSS targets", "what should I work on this weekend", "active repos for X", "give me a list of <stack> repos to look at", "what to dig into for <topic>", "cluster N: <bucket name>", "find repos that match <person>'s interests", "second batch / third batch / next round of <topic>". When the user names clusters or numbered batches, each session is one cluster and the deliverable should match the shape of any prior cluster reports the user has.

Do **not** use this skill for: managing the user's own repos (→ `github-repo-management`), code review of an existing PR (→ `github-code-review`), paper-discovery for reading (→ `arxiv`), or skill-discovery across this agent (that's `skills_list`).

## Workflow

### 1. Lock the contributor profile

Before searching anything, anchor on four things:

- **Skill level** (intermediate/advanced/expert). Drives difficulty scoring and which repos are realistic.
- **Stack** (PyTorch/JAX/Go/Rust/…). Drives the candidate pool.
- **Hardware ceiling** (1-2 GPU box vs multi-node). Hard-kill any "requires 8x H100" candidates.
- **Interest vectors** (from-scratch, paper repro, agents, eval/internals, infra, etc.). Drives the bucket split.

If any of these are missing, ask. Guessing wastes 5 min of API calls.

### 1a. Match the user's existing report format when one exists

If the user already has a sibling report (e.g. `night-oss-list.md` with entries like `smol-course`, `stanford-cs336`), **read it first** and:

1. Mirror its structural shape exactly — same field order, same difficulty scheme, same attack-plan table at the end.
2. Treat its exclusions as ground truth — any repo it lists is off-limits for the new report (the user has presumably already done or ruled those out).
3. Look for the bucket/cluster language (e.g. "CLUSTER 3: paper reproductions") — when the user names clusters, each session is one cluster and they want the same shape per cluster.

This is the difference between a "list" and a "list that fits into the user's series."

### 2. Bucket-split before searching

Define 3-4 buckets *before* you query. For a typical "ML contributor overnight" task:

- Bucket 1: from-scratch / educational impl
- Bucket 2: agent / framework harnesses
- Bucket 3: paper-reproduction repos
- Bucket 4: eval / interpretability / model-internals

Aim for 2-3 picks per bucket. The bucket split is the difference between a list and a *curated* list.

### 3. Candidate discovery (gh CLI)

Use `gh search repos` with bucket-specific queries. Verify candidates with `gh api repos/{owner}/{name}`.

**The schema gotcha (see references/gh-search-vs-api.md):**

- `gh search repos` returns a different JSON shape than `gh api repos/...`. Search needs `--json fullName,stargazersCount,pushedAt,license,isArchived` (camelCase). Direct `gh api` returns snake_case fields usable directly with `jq`.
- Pipelining `jq` against raw `gh search repos` output (no `--json`) silently returns `null` for every field. Don't waste 10 min rediscovering this.

**Org-level enumeration when the user names candidate orgs:**

```bash
# Enumerate every public repo in an org, sorted by recency
gh search repos "" --owner EleutherAI --limit 50 \
  --json fullName,description,pushedAt,stargazersCount,isArchived --sort updated \
  | jq -r '.[] | "\(.pushedAt[:10]) | \(.stargazersCount)★ | \(.fullName) | \(.description[:80])"'
```

Use this for each org the user lists (e.g. "look in `allenai`, `EleutherAI`, `kyegomez`, `princeton-nlp`, `GAIR-NLP`, `HazyResearch`, `thunlp`, `state-spaces`, `allenai`, `open-thoughts`"). It is the fastest way to surface "alive but I didn't know about it" gems without naming each repo.

**Discovery commands that work:**

```bash
# Search (use --json or pipe to jq with --raw-output + manual parse)
gh search repos "<query>" --limit 10 --json fullName,stargazersCount,pushedAt,license,isArchived

# Verify (this is the truth source)
gh api repos/{owner}/{name} | jq '{full_name, stars: .stargazers_count, pushed_at, license: .license.spdx_id, open_issues: .open_issues_count, archived, desc: .description}'
```

### 3a. Rate-limit pacing (the invisible tax)

GitHub's secondary rate limit kicks in fast when batching `gh api` calls. A single session can hit `403 rate_limit_exceeded` after ~15-20 fast `gh api repos/X` calls in a row. Symptoms: `gh search repos` starts returning `[]` for valid queries, and `gh api` returns 404 for repos you know exist.

Fix:

- Insert `sleep 4` to `sleep 6` between batched API calls when you have > 10 to do.
- For `gh search repos` specifically, 5s of sleep per call is the safe rate.
- If `gh search repos` returns `[]` for a query you know should match, retry after `sleep 8` — it's the rate limit, not a real empty result.
- The 30-call-per-hour authed REST limit applies separately; if you must do bulk verification, group into 25-call chunks with a 60s pause.

Do NOT work around it with retries faster than ~3s — you'll just compound the limit.

### 4. Freshness filter (the non-negotiable rule)

**Every repo MUST be verified live before being included.** No exceptions, no "I think it's still active."

Hard rules:

- `archived: true` → **drop immediately**, no matter how good the repo is.
- `pushed_at` older than 6 months → **drop** (unless explicitly tagged "treat as reference reading" in the deliverable).
- `pushed_at` 6-12 months old → flag, do not include unless exceptional.
- `pushed_at` < 30 days → strong signal of active maintenance.

**Verification pattern (the one that works):**

```bash
# In a single batched loop, never one-at-a-time
for repo in "owner1/repo1" "owner2/repo2" ...; do
  gh api repos/$repo 2>&1 | jq -c '{full_name, stars: .stargazers_count, pushed_at, license: .license.spdx_id, archived}'
done
```

The 2>&1 catches 404s (e.g. typos in `owner/name`) and prints them inline.

### 5. The "thing to do" per repo (the part everyone skips)

A list with no concrete next action is a list, not a curation. For each pick, the deliverable MUST include one of:

- An open issue to tackle (link to it)
- A roadmap item / feature to add
- A paper to reproduce (with arxiv link)
- A specific fork experiment (e.g. "port this to SmolLM")

The "thing to do" must be **small enough to do in a weekend** for the easy picks, and **explicitly multi-week** for the deep picks. Never vague ("contribute to X") — that's not a task.

### 6. Star band + license filter

Reasonable defaults for a mid-level contributor who wants merges:

- **Sweet spot:** 1k-50k stars. Big enough to be alive, small enough that PRs aren't ignored.
- **Hidden gems:** < 1k stars, BUT only if pushed in last 60 days AND maintainers respond.
- **Mega-repos:** > 50k stars are fine to include as "reference reading" or if the issue queue is well-tended (e.g. litgpt, deepagents, smol-course). Avoid them as primary overnight targets.
- **License:** prefer MIT or Apache-2.0. "NOASSERTION" is OK if the code is openly used in practice (e.g. rasbt/LLMs-from-scratch). No-license repos are risky to PR against; flag them.

### 7. Deliverable shape (the format that ships)

A markdown report with one block per repo, ordered by recommended attack sequence. Each block has:

- **Name + URL + 1-line desc**
- **Why for this contributor** (tied explicitly to their profile)
- **What to do** (the concrete action)
- **Activity signal** (pushed_at, open_issues count)
- **Stars / License**
- **Difficulty** (1-5)
- **Order tag** (1-5 stars showing when to attack it)

End with a one-table "suggested attack plan" that maps hours to picks. The table is what the user actually reads at 2am when picking what to open first.

### 8. Pitfalls

- **Don't trust star count alone.** A 30k-star repo with last push 2 years ago is dead. A 300-star repo pushed yesterday is gold.
- **Don't include `archived: true` repos even as references** unless you flag it explicitly. The user reads the list fast.
- **Don't write the report before verifying all 12.** Partial verification → false confidence → user wastes 4 hours on a dead repo.
- **Don't fabricate issue numbers or PR descriptions.** If you can't find a concrete open issue, write "open an issue proposing X" — that's a real, valid action.
- **Don't exceed the word budget.** 1500 words for 12 picks is ~125 words/pick. Discipline. The user wants signal density.
- **Don't mix repos at radically different scale** (e.g. a 200-star hidden gem and a 100k-star juggernaut) without explaining the size difference. The contributor will calibrate their effort wrong.
- **Don't trust user-supplied repo names without verifying them.** The user will frequently name repos that have been renamed, deleted, never existed publicly, or live under a different org (e.g. user says `HuggingFaceTB/smollm-corpus` but it's `huggingface/smollm-corpus`; user says `EleutherAI/TransformerLens` but it was renamed; user says `HuggingFaceFW/nanoMoE` but the org doesn't exist on GitHub). When a user-named repo 404s, search by topic/keyword to find the real home, or drop it and document the drop honestly in the report. **Never fabricate a substitute repo to fill the slot.**
- **Don't search `--owner <user>` for an org that doesn't exist on GitHub.** Some orgs the user names are not GitHub orgs at all (e.g. `HuggingFaceFW`, `HuggingFaceTB`, `xai-org`, `InflectionAI`, `ai21`). `gh search repos "" --owner X` will return `[]` silently — distinguish that from a real "no recent activity" empty by trying one known repo from the org first; if that 404s, the org doesn't exist and you should note it explicitly. Don't waste 5 min wondering why an org returns empty.
- **Don't let `gh search repos` returning `[]` decide the answer.** When the rate limit hits (see §3a), valid queries return `[]` and you'll wrongly conclude "nothing matches" and skip the bucket. Sleep and retry before declaring a query dead.

### 9. Verification before delivery

Before writing the report, run this sanity check on the candidate list:

- [ ] All 10-12 repos confirmed via `gh api` (no stars/pushed_at/archived guesses)
- [ ] No archived repos in the list
- [ ] No repo with `pushed_at` > 6 months old (unless flagged reference)
- [ ] All picks have a concrete "thing to do"
- [ ] Star band spread (mix of sizes), not all 1k-range
- [ ] Buckets all hit (no bucket with 0 picks)
- [ ] License not "None" or unknown for the primary PR targets
- [ ] Every user-named repo in the prompt either verified-included OR explicitly documented in the "what I excluded" section with the reason (404, renamed, not-on-GitHub, stale). No silent drops.

## Linked files

- `references/gh-search-vs-api.md` — the JSON-schema gotcha, exact commands, the batch-verification loop, org-level enumeration, rate-limit pacing, full field-name cheat sheet
- `scripts/verify-repo-batch.sh` — drop-in batch verifier (TSV output with status/stars/push/license/archived). Handles 404s inline, paces itself at ~5s/call. Use this instead of re-typing the loop every session.
