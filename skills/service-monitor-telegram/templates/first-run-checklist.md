# First-run checklist for a new service monitor

Run these in order. Skip one and the first tick will misbehave.

1. **Install the script** under `~/.local/bin/<service>_monitor`
   (or any PATH directory). Make it executable: `chmod +x`.

2. **Baseline the seen-set.** Run `<service>_monitor --init` once
   *before* scheduling the cron job. This marks current state as
   already-seen so the first scheduled tick doesn't alert on every
   existing item. If you skip this, expect a flood on the first tick
   and a 30-min cooldown before the system recovers.

3. **Add the SELF filter** for any signal the user can produce
   themselves. See the SELF-filter section in SKILL.md. Without it,
   the next test message from the user triggers a real Telegram alert.

4. **Drop the wrapper** in `~/.hermes/scripts/`. Cron rejects absolute
   paths; use the `templates/cron-wrapper.sh` template, edit the two
   paths, and `chmod +x`.

5. **Schedule.** Use `hermes cron create` or the in-session `cronjob`
   tool:
   ```bash
   hermes cron create every 30m \
     --name <service>-monitor \
     --script <service>.sh \
     --no-agent
   ```
   `--no-agent` is critical: the script IS the job. With it set, the
   scheduler runs the script and delivers stdout via `--deliver`. Without
   it, the scheduler also spins up an LLM agent loop on every tick —
   burning tokens for no reason.

6. **Force a tick** to verify: `hermes cron run <job-id>`. Check
   `hermes cron list` — the `last_status` field should be `ok` and
   `execution_success: true`. (The CLI emits a TUI table, not JSON —
   `hermes cron list | jq` will fail.)

7. **Send a real-looking test signal** (e.g. a self-sent email with
   an important-looking subject) and verify the SELF filter drops it
   silently. If a Telegram alert fires, the SELF filter is missing or
   misordered.

8. **Send a real important signal** (or wait for one) and verify the
   alert format matches the samples in SKILL.md. If the alert is too
   verbose, too sparse, or fires on digest items, revisit the
   classifier rules.

9. **Re-baseline if you tune the classifier mid-run.** Existing
   already-seen UIDs in `~/.local/state/<service>_monitor/last_seen.json`
   will still be suppressed. Run `<service>_monitor --init` again to
   re-baseline if you change the importance rules substantially.
