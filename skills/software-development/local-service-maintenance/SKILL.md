---
name: local-service-maintenance
description: "Maintain self-hosted systemd user services."
version: 1.0.0
author: Hermes Agent
license: MIT
platforms: [linux, macos]
metadata:
  hermes:
    tags: [systemd, services, npm, paperclipai, self-hosted, updates, verification, daemon]
    related_skills: [library-behavior-probing, live-model-benchmarking, vault-knowledge-graph]
---

# Local Service Maintenance

Self-hosted services on the user's machine (paperclipai, future embedded-DB
CLIs, npm/pip-managed daemons) usually run as **systemd user units**. The
maintenance loop is small but has subtle traps:

1. The unit's `ExecStart` may be pinned to a version or to `@latest` —
   these have different update semantics.
2. `npm view <pkg> version` shows the **registry latest**, not what the
   running process actually loaded.
3. `npx --yes <pkg>` resolves through `~/.npm/_npx/<hash>/node_modules/<pkg>/`
   — the loaded version lives there, NOT in any global install.
4. A service restart that takes a fresh npx cache pulls a new `<hash>`,
   so the actually-loaded package version can change **between restarts**
   even when the unit file is byte-identical.
5. The service must be health-verified end-to-end after restart, not just
   "is the unit active". Active ≠ serving. Hit the actual HTTP endpoint.
6. Every service has a slot in the user's knowledge graph (`Infrastructure`
   facet). Update that slot, then verify `wiki/build_index.py --check` shows
   zero new broken links.

This skill captures the loop. The 2026-08-04 paperclipai@2026.722.0 update
on the user's `victus` is the worked example.

## When to use

- "Update <service> to the latest version"
- "Restart <service>"
- "Is <service> up to date?" / "what version is <service> on?"
- A scheduled cron should fire to check version drift (not yet wired, but
  this skill enables that flow)
- A service crashes / unit goes inactive / health endpoint 404s

## The loop (6 steps)

### 1. Locate the unit and inspect current state

```bash
systemctl --user list-unit-files --type=service '<name>*'
systemctl --user list-units --all --type=service '<name>*'
systemctl --user show <name>.service \
  -p MainPID -p ExecStart -p ActiveEnterTimestamp -p FragmentPath
```

Capture: `FragmentPath`, `ExecStart`, `MainPID`. The unit file is the
authoritative source of *intent* (what should run), not of *actual*
state (what's loaded).

### 2. Determine the registry latest

```bash
npm view <pkg> name version dist-tags --json
```

Note `dist-tags.latest` AND `dist-tags.canary` (paperclipai uses both —
canary `2026.803.0-canary.8` is unstable and should never be the default
service target). **Stable `latest` is the right target for production
services** unless the user explicitly asks for canary.

### 3. Determine the actually-loaded version (NOT registry latest)

The running process loaded a specific package tarball. Two ways to find it:

**A. From the running process's cmdline:**
```bash
pid=$(systemctl --user show <name>.service -p MainPID --value)
tr '\0' ' ' < "/proc/$pid/cmdline"
```

This shows the `npx --yes <pkg>@X run ...` command. Parse the `@X` if
present.

**B. From the npx cache (more reliable — survives restarts):**
```bash
find ~/.npm/_npx -maxdepth 5 -name '<pkg>' -type d 2>/dev/null
```

For each matching dir, read `<dir>/../../package.json` (the resolved
package's manifest). Paperclipai example:
```bash
node -e 'console.log(require("/home/arctic/.npm/_npx/0aa74679bec75e15/node_modules/paperclipai/package.json").version)'
```

**The actually-loaded version may NOT match `npm view` latest.** Reasons:
- The unit pins a fixed version (`paperclipai@2026.722.0`)
- The unit pins `@latest` but the npx cache hasn't been invalidated
  (npx reuses a hash-dir for ~24h unless explicitly refreshed)
- The user wants canary but the unit says latest

### 4. Decide: pin or `@latest`?

| User intent | What to do in the unit |
|---|---|
| "Update to latest" (one-shot) | Either (a) restart with `npx --yes <pkg>@<ver>` for the new version, or (b) change the unit to `@latest` for permanent tracking |
| "Always track latest" | Set `ExecStart=/.../npx --yes <pkg>@latest run --bind ...` |
| "Stay on a specific version" | Pin `@<ver>` |
| "Try canary" | Pin `@canary` |

Paperclipai update 2026-08-04: the user's prior service file had
`paperclipai run` (no version pin) which resolves to latest at every
restart, but the loaded tarball was the prior latest because npx cache
hadn't been invalidated. Patched the unit to `paperclipai@latest`
explicitly so future restarts always re-resolve. Restarted.

**Why explicit `@latest` even though `<pkg>` (no tag) also defaults to
latest:** clarity. A future agent reading the unit can see intent
without having to reason about npx's default-tag semantics.

### 5. Restart and verify (with timeout, not blind sleep)

```bash
systemctl --user daemon-reload          # if you changed the unit
systemctl --user restart <name>.service

# Poll for ready, with a hard cap
for i in $(seq 1 300); do
  code=$(curl -sS -o /tmp/<name>-health.json -w '%{http_code}' \
    --max-time 2 http://127.0.0.1:<port>/<health-path> 2>/dev/null || true)
  if [ "$code" = 200 ]; then break; fi
  if ! systemctl --user is-active --quiet <name>.service; then
    echo 'service died during restart'; break
  fi
  sleep 1
done
```

Required checks before declaring done:

- [ ] `systemctl --user is-active <name>.service` = `active`
- [ ] `systemctl --user is-enabled <name>.service` = `enabled`
- [ ] `curl http://127.0.0.1:<port>/<health>` returns 200
- [ ] The health body reports the **expected** `version` / `serverVersion`
      (parse the JSON, don't trust the npm registry)
- [ ] `journalctl --user -u <name>.service --since '<start-ts>'` shows the
      new process claiming the port and the embedded DB (if any) starting
- [ ] For services with embedded DBs: `ls ~/.local/share/<svc>/instances/default/db`
      exists and is writable (data preserved across restart)
- [ ] For services that bind to a unix socket: `ss -lntp | grep <svc>`
      or `lsof -nP -p <pid>` shows the listen socket

**Pitfall: silent shutdown drain failure.** Some services log
`WARN: graceful X drain failed {"signal":"SIGTERM"}` during shutdown —
this is normal during a quick restart. Don't mistake it for a restart
failure. Look at the post-startup logs instead.

### 6. Update the knowledge graph

Find the relevant facet (typically `wiki/personal/facets/infrastructure.md`
for the user's setup). Add a one-paragraph entry:

```markdown
- **<Service>** — <one-liner purpose>. Managed as `<name>.service`
  (enabled user unit). <endpoint>. Current verified version: `<ver>`
  (<date>); registry `latest` = `<ver>`. ExecStart:
  `/home/.../npx --yes <pkg>@latest run --bind loopback`.
```

Bump frontmatter `updated:` to today's date. Then:

```bash
cd ~/Documents/fun && python3 wiki/build_index.py --check
```

Must show `broken=0`. If it rises, you introduced a broken wikilink —
fix before declaring done.

## Common service recipes (verified 2026-08-04)

### Paperclip AI

- Unit: `/home/arctic/.config/systemd/user/paperclipai.service`
- ExecStart: `/home/arctic/.npm-global/bin/npx --yes paperclipai@latest run --bind loopback`
- WorkingDirectory: `/home/arctic`
- Environment: `HOME=/home/arctic`, `PATH=/home/arctic/.npm-global/bin:...`
- Bind: `127.0.0.1:3100` (loopback only)
- Data: `~/.paperclip/instances/default/` (embedded PostgreSQL on port 54329)
- Health: `GET http://127.0.0.1:3100/api/health` → JSON with `version`,
  `serverVersion`, `deploymentMode`, `databaseBackup.{enabled,latestBackup}`
- Useful CLI flags: `--bind loopback` / `--bind lan`, `--instance <name>`
  (separate config + DB), `--no-repair`
- Known warning (ignore unless debugging): `database_backup_stale` if no
  fresh backup in 26h — paperclip has its own backup scheduler (60m/keep 30d)

### Pattern that generalizes

Any `npx --yes <pkg>@<tag> run --bind ...` service:

| Service slot | Source |
|---|---|
| Unit file | `~/.config/systemd/user/<name>.service` |
| Data dir | `~/.<name>/instances/default/` |
| Embedded DB port | usually 54329–54340 (paperclipai: 54329) |
| HTTP health | usually `/api/health` on the bind port |
| Version source | `npm view <pkg> version` + the loaded tarball in `~/.npm/_npx/<hash>/` |

## Pitfalls

- **Don't `daemon-reload` blindly.** Only needed when the unit FILE
  changed. After a pure service restart, skip.
- **`process(state)` ≠ `systemctl is-active`.** A systemd-managed
  process can be in `S` (sleeping) state and still be healthy. Don't
  diagnose from process state; diagnose from the health endpoint.
- **`npm view` doesn't see canary tags as `latest`.** Some packages
  ship a `canary` dist-tag ahead of `latest`. Always check both.
- **Restart doesn't always refresh npx cache.** A unit pinned to
  `@latest` may serve the prior latest if npx reused a hash-dir. To
  force a fresh pull: `npx --yes <pkg>@latest --version` (runs the
  binary's version probe through a new hash), or `npm cache clean
  --force` (heavy-handed, only when truly stale).
- **Graceful drain logs are noise.** Services that print `WARN:
  graceful heartbeat drain failed` during SIGTERM are telling you
  their shutdown was forced. This is fine for a 5-second restart and
  not fine if the service is being decommissioned. Read the message
  context, don't pattern-match.
- **Embedded DB port collisions.** Two services using `embedded-postgres`
  will collide on the default port if they share a hostname. Pin
  explicit ports per-service if you run multiple.
- **Health endpoints may report `status: ok` but fail at request time.**
  `/api/health` is a liveness probe; for actual functional checks, hit
  a route that exercises the data path (paperclipai: `GET /companies/<id>/agents`).
- **Don't pin to canary for production services.** Canary exists to
  burn off bugs before promoting to latest. If you do pin to canary,
  add a cron to alert when it falls behind or ahead of `latest`.
- **KG link resolution: use bare basenames.** `[[infrastructure]]`,
  not `[[wiki/personal/facets/infrastructure|wiki: infrastructure]]`
  unless disambiguating. The vault's `wiki/build_index.py` resolver
  does NOT normalize `..` segments — always bare basename for new
  internal links.

## Verification checklist (per maintenance run)

- [ ] Unit file inspected, current `ExecStart` recorded
- [ ] Registry latest AND canary captured
- [ ] Actually-loaded version determined (process cmdline OR npx cache)
- [ ] Decision made: pin / `@latest` / fixed version — match user intent
- [ ] Unit updated if needed, `daemon-reload` if file changed
- [ ] Restart issued, polled for ready with hard timeout
- [ ] Health endpoint returns 200 + expected version in JSON body
- [ ] Service logs show clean start (no fatal errors, embedded DB ready
      if applicable)
- [ ] Knowledge graph infrastructure facet updated; `updated:` bumped
- [ ] `python3 wiki/build_index.py --check` shows `broken=0`
- [ ] User told the verified version (not "latest" — the actual number)

## See also

- `library-behavior-probing` — same philosophy (measure the real binary,
  don't trust docs) for pip/python libraries.
- `live-model-benchmarking` — same philosophy for LLM provider features.
- `vault-knowledge-graph` — the KG update discipline for captured facts.