---
name: hermes-self-backup
description: Backup Hermes to a private repo with daily cron.
---

# Hermes Self-Backup

A complete recipe for snapshotting a Hermes Agent instance into a git-tracked local directory, pushing it to a private GitHub repo, and keeping it fresh with a daily cron. Built and verified 2026-08-06.

**Trigger:** user asks to "backup yourself" / "snapshot Hermes" / "set up remote backup" / "create a backup copy of yourself".

## What "portable identity" actually means

A Hermes backup must capture:

1. `~/.hermes/config.yaml` + history `.bak.*` files — provider/model/voice/toolset/personality
2. `~/.hermes/SOUL.md` — base operating context
3. `~/.hermes/skills/` — all 117 SKILL.md files (~53 MB; the dominant component)
4. `~/.hermes/memory_store.db` — holographic facts/entities (compact, ~86 KB)
5. `~/.hermes/memories/` — per-profile memory files
6. `~/.hermes/cron/jobs.json` — active scheduled jobs
7. `~/.hermes/cron/output/<run>/` — recent cron outputs (last few days)
8. `~/.hermes/.skills_prompt_snapshot.json` — provenance for last-loaded skills
9. `~/.claude/CLAUDE.md` if it exists (it doesn't on this box — text is served by gateway from the hermes-agent checkout; capture `~/.hermes/hermes-agent/AGENTS.md` as fallback)

**Always exclude:** `auth.json`, `.env` (credentials), `state.db` (session/runtime state, 437 MB+), `cache/`, `audio_cache/`, `image_cache/`, `sessions/`, `chrome-debug/`, `kanban.db`.

## Procedure

```bash
# 1. Create backup dir with manifest + README skeleton
BACKUP=/home/arctic/hermes-self-backup/hermes-self-$(date '+%Y-%m-%d-%H%M')
mkdir -p "$BACKUP"
# copy the relevant subset (see "What to capture" above)

# 2. CRITICAL: copy memory_store.db atomically with SQLite backup API
python3 -c "
import sqlite3
src = sqlite3.connect('/home/arctic/.hermes/memory_store.db')
dst = sqlite3.connect('$BACKUP/memory/holographic_store.db')
src.backup(dst)        # atomic, handles active WAL correctly
dst.close(); src.close()
"
# Plain `cp` misses the WAL — you'll get ~half the facts. Always use the backup API.

# 3. Write README.md with restore recipe + manifest.json with byte sizes
# 4. `git init -b main && git add . && git commit -m "..."`

# 5. Create private GitHub repo (gh CLI is authed for HTTPS on this box)
gh repo create adityasasidhar/hermes-self-backup --private \
    --description "Portable snapshot of this Hermes Agent instance..."

# 6. Push via HTTPS (SSH key forwarding is unreliable on this box — ssh-askpass
#    missing; fall back to HTTPS with gh CLI keyring)
git remote add origin https://github.com/adityasasidhar/hermes-self-backup.git
git push -u origin main

# 7. Write push script under ~/.hermes/scripts/<name>-push.sh mirroring the
#    daily-vault-push.sh pattern:
#      - empty working tree → silent exit 0
#      - local changes      → git add -A + commit + git push origin main
#      - failure            → exit non-zero → cron gateway alerts to chat
#      - GIT_TERMINAL_PROMPT=0 prevents credential hangs
#      - script hardcodes the BACKUP_DIR path; bails with exit 2 if missing

# 8. Register cron with script-mode + no_agent
hermes cron create "30 2 * * *" \
    --name hermes-self-backup-push \
    --script hermes-self-backup-push.sh \
    --no-agent \
    --deliver origin \
    "<hygiene-aware prompt>"
```

## Schedule choice

- `02:30 IST` is the verified off-hours slot on this machine. 1 AM is acceptable for data-collection crons per memory rule; 2:30 AM extends that window without hitting the 06:00 wake boundary.
- Avoid colliding with `hermes-times-v4` (06:00, heavy morning load) or `daily-vault-push` (01:15, vault repo) or `daily-memory-yesterday` (01:00).

## Cron prompt hygiene (MANDATORY)

The cron prompt MUST include:
- Explicit "DO NOT touch the daily-vault-push cron" (different repo, different scope)
- Explicit "DO NOT regenerate the backup dir from this cron" (interactive op)
- The hardcoded backup path + the hardcoded remote URL (so any future agent reading the prompt knows the literal scope)
- The silence semantics (empty stdout → silent)

After registering, audit every other active cron prompt in `~/.hermes/cron/jobs.json` for hardcoded paths that overlap with the new backup dir. None of the existing crons on this machine do; this check is fast.

## Pitfalls

- **`cp` of memory_store.db misses WAL.** Witnessed: copy landed with 9 facts when live DB had 15. Always use `sqlite3.Connection.backup()`.
- **SSH push fails silently.** `ssh-askpass` is not installed on this box. Use HTTPS + gh CLI keyring.
- **`hermes cron create` prompt argument quoting.** Multi-line prompts in shell variables get split on whitespace and rejected as "unrecognized arguments". Pass the prompt as the **last positional** after all flags, single-quoted, single line (or escape internal newlines).
- **`workdir` on script-mode crons is irrelevant** — the script `cd`s into the dir itself. Don't bother setting it (matches `daily-vault-push.sh` which sets it but it's cosmetic for script-mode).

## Verification

After registering, run `hermes cron list` and confirm:
- New job appears with `Mode: no-agent (script stdout delivered directly)`
- `Script:` field points at the file you wrote
- `Next run:` is in the future
- All 3 other active crons still appear (no accidental loss)
- Test the script directly: `bash ~/.hermes/scripts/hermes-self-backup-push.sh` → exit 0, no output (silent on clean tree)