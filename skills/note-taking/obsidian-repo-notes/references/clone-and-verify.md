# Cloning all repos locally + verifying git validity

## One-time auth wiring
```bash
gh auth setup-git        # registers gh as git credential helper (keyring)
git config --global --get-all credential.helper   # should show the helper
```
After this, `gh repo clone` authenticates BOTH public and your own private repos
automatically via the keyring — no token juggling, no per-repo URL edits.

## Clone loop (background it for many repos)
```bash
#!/usr/bin/env bash
set -u
OWNER=adityasasidhar
DEST=/home/arctic/projects
mapfile -t repos < repos.txt     # one repo name per line, or compute from vault notes
ok=0; fail=0
for r in "${repos[@]}"; do
  [ -d "$DEST/$r/.git" ] && { echo "SKIP $r"; continue; }
  if gh repo clone "$OWNER/$r" "$DEST/$r" >>/tmp/clone_log.txt 2>&1; then
    echo "OK $r"; ok=$((ok+1))
  else
    echo "FAIL $r"; fail=$((fail+1))
  fi
done
echo "DONE ok=$ok fail=$fail"
```
Run with `terminal(background=true, notify_on_complete=true)`; poll `process(action=wait)`.

## Verify git validity — do NOT just check for `.git`
A `.git` directory can exist without a valid repo. Use:
```bash
git -C /path/to/repo rev-parse --is-inside-work-tree   # prints "true", rc=0 if valid
```
Python check across a folder:
```python
import os, subprocess
for name in repos:
    p = os.path.join(DEST, name)
    r = subprocess.run(['git','rev-parse','--is-inside-work-tree'], cwd=p,
                       capture_output=True, text=True)
    if r.returncode != 0 or r.stdout.strip() != 'true':
        print("NOT A VALID GIT REPO:", name)
```

## Cross-check coverage
List vault note names (filenames minus `.md`); flag any not present in DEST or not a
valid git repo. This confirms 100% coverage after a bulk clone.
