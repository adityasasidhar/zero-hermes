# gh api jq object-constructor pitfall

## Symptom
```
gh api repos/OWNER/NAME --jq '.{name,description,language}'
# -> failed to parse jq expression (line 1, column 2)
#    .{name,description,language
```

The bundled `jq` in the Hermes environment does NOT support the object-constructor
syntax `.{key: expr, ...}`. It throws on the `{` at column 2.

## Fix — fetch full JSON, parse in Python
```python
import json, subprocess

def gh_json(path, timeout=30):
    r = subprocess.run(['gh','api',path], capture_output=True, text=True, timeout=timeout)
    if r.returncode != 0:
        return None
    return json.loads(r.stdout)

# list of owner repos
repos = gh_json('users/adityasasidhar/repos?per_page=100&type=owner&sort=updated')
meta = {d['name']: d for d in repos}

# per-repo (use for private repos not in the listing)
for name in missing_private:
    d = gh_json(f'repos/adityasasidhar/{name}')
    if d:
        meta[name] = d  # has name, description, language, visibility,
                        # stargazers_count, forks_count, topics, created_at,
                        # updated_at, pushed_at, homepage, default_branch, size,
                        # archived, fork
```

## Notes
- `--jq '.'` is fine (identity). Avoid any jq that builds new objects/arrays.
- For a single string field you CAN use `--jq '.fieldname'` (that works). The failure
  is specifically the `{...}` constructor.
- Always set a `timeout` on `subprocess.run` — `gh api` can hang on network issues.
