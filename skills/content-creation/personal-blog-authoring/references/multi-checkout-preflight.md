# Pre-flight verification for multi-checkout repos

The single biggest footgun when writing to a personal blog: the user
has two (or more) checkouts of what looks like the same project, and
you're about to write into the wrong one.

## The pattern (observed)

```
/home/arctic/projects/adityasasidhar.github.io/      <- mirror named after deploy URL
/home/arctic/projects/website/the-deep-field/         <- actually live working tree
```

Both have the same package.json, both run `npm run dev` successfully,
both have an AGENTS.md. The dev server is running from one of them —
the other is dormant. If you write to the dormant one, the user
refreshes the browser, sees nothing, and rightly gets frustrated.

## Pre-flight checklist (run BEFORE delegating to a subagent)

```bash
# 1. Which process is bound to the dev server's port?
ss -ltnp 2>/dev/null | grep -E ":4321|:3000|:5173"
# or
lsof -iTCP:<port> -sTCP:LISTEN

# 2. Get the working directory of that process
ls -l /proc/<pid>/cwd

# 3. Verify the AGENTS.md / schema in that working tree, not the
#    repo you assumed was live
cat /proc/<pid>/cwd/AGENTS.md 2>/dev/null
cat /proc/<pid>/cwd/src/content.config.ts 2>/dev/null
```

The `cwd` of the dev server process IS the live working tree. Trust
that, not the directory name.

## Mid-session correction

If the user catches you mid-session ("you wrote it in the wrong place,
write it in X instead"):

1. **Stop.** Don't try to also fix unrelated issues.
2. **Copy / move** the file to the correct path (not rewrite — keep the
   same content the subagent produced).
3. **Delete** the file from the wrong repo so it doesn't cause confusion
   later.
4. **Verify** with `ls` and `git status` in both repos that the right
   state exists.
5. If there was an existing user-written version of the same post in
   the live repo, **read it first** — the user's version is usually
   better and you should refine it rather than overwrite.

## When this is ambiguous

If `ss` / `lsof` doesn't find the port (e.g., dev server not running),
ask the user which checkout is canonical. Don't guess from directory
names — they can be misleading (a deploy-named folder that's actually
dormant, a workspace folder that's actually live).

## When this is unambiguous but easy to miss

The user's `projects/` directory often has obvious-lookalike folders:
- `<username>.github.io` (deploy mirror, often dormant)
- `website/<project>` (active working tree)
- `<project>` (clean clone, neither)

The dev server output (`Local: http://localhost:4321/`) doesn't tell you
which working tree it's serving — only `lsof` on the listening socket
does. Always verify the process cwd before writing.

## Why this matters

This isn't a theoretical concern. In the reference session, the
subagent wrote two files into the wrong repo before the user caught
it. The user was visibly frustrated ("yo bitch write it in the deep
field not somewhere else"). The cost was two deleted files, one
re-titled file, and a writeup of the mistake back to the user — all of
which would have been avoided by 30 seconds of `ss -ltnp` before the
first delegation.
