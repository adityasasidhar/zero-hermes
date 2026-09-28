#!/usr/bin/env python3
"""
Apply STRIP / CREATE_STUB / FUZZY_AUTO / AUTO_FIX policies to vault source
files and write results to /tmp/bridge-slice-N/. Does NOT touch the vault.

This is the reusable policy-application kernel extracted from the slide 3
bridge-slice run (July 2026). The brief lived at /tmp/bridge-slice-N/BRIEF.md
and the per-target slice at /tmp/bridge-slice-N/bridge-slice-N.json.

Usage as a library:
    from strip_wikilinks import process_file, SPECIAL_CASES
    manifest = process_file(entry, vault_path, out_dir)

The full recipe (brief format, manifest schema, parent-agent verification
steps) is in references/broken-link-repair.md.
"""
import json
import os
import re
import sys

VAULT = '/home/arctic/Documents/fun'

# Per-brief special-case overrides. The slice JSON policy is the default;
# these are checked BEFORE applying policy to a target. Each entry is
# (src_substring, target) -> (new_policy, new_arg).
SPECIAL_CASES = {
    # Hobbies/Cooking/recipies: folder redirect to existing index
    ('Hobbies/Cooking/recipies', 'Hobbies/Cooking/recipies/'): (
        'AUTO_FIX', 'Hobbies/Cooking/recipies/recipies',
    ),
    # Work/Internship/Dataobserve: redirect to existing sibling note
    ('Work/Internship/Dataobserve', 'Work/Internship/Dataobserve/'): (
        'FUZZY_AUTO', 'Work/Internship/Data Observe',
    ),
}


def line_in_codeblock(lines, idx):
    """Return True if line idx (0-based) is inside a ``` code block."""
    in_block = False
    for i in range(idx + 1):
        if lines[i].lstrip().startswith('```'):
            in_block = not in_block
    return in_block


def apply_policies(entry, src_path, out_path):
    """Apply per-target policies to one source file. Returns (manifest_delta, new_content)."""
    with open(src_path) as f:
        content = f.read()
    lines = content.splitlines(keepends=True)
    original_lines = len(lines)

    target_policy = {t['target']: t for t in entry['targets']}

    # Apply special-case overrides
    for (target, src_substr), (new_policy, new_arg) in SPECIAL_CASES.items():
        if src_substr in src_path and target in target_policy:
            target_policy[target] = {
                **target_policy[target],
                'policy': new_policy,
                'arg': new_arg,
                'original_policy': target_policy[target]['policy'],
            }

    strip_targets = [t for t, p in target_policy.items() if p['policy'] == 'STRIP']
    rewrite_targets = [
        (t, p) for t, p in target_policy.items()
        if p['policy'] in ('FUZZY_AUTO', 'AUTO_FIX')
    ]

    rewrite_map = {}
    rewrites = []
    for t, p in rewrite_targets:
        if p['policy'] == 'FUZZY_AUTO':
            arg = p['arg']
            # Strip .md extension to match Obsidian convention
            if arg.endswith('.md'):
                arg = arg[:-3]
            # Display = last path component of broken link
            display = t.split('/')[-1] if '/' in t else t
        else:  # AUTO_FIX
            arg = p['arg']
            # For recipes special case, display is just "recipies"
            display = 'recipies' if t == 'Hobbies/Cooking/recipies' else t
        new_link = f'[[{arg}|{display}]]'
        rewrite_map[t] = new_link
        rewrites.append({'old': f'[[{t}]]', 'new': new_link})

    new_lines = []
    for i, line in enumerate(lines):
        if line_in_codeblock(lines, i):
            new_lines.append(line)
            continue

        new_line = line
        # Rewrites first
        for t, new_link in rewrite_map.items():
            new_line = new_line.replace(f'[[{t}]]', new_link)
        # Strips: 4-step pattern (leading-comma, trailing-comma, whole-line, anchored)
        for t in strip_targets:
            tok = f'[[{t}]]'
            new_line = re.sub(r',\s*' + re.escape(tok), '', new_line)
            new_line = re.sub(re.escape(tok) + r'(,\s*)', '', new_line)
            new_line = re.sub(r'^\s*' + re.escape(tok) + r'\s*$', '', new_line)
            new_line = re.sub(r'^\s*' + re.escape(tok) + r'\s*', '', new_line)
            new_line = re.sub(r'\s*' + re.escape(tok) + r'\s*$', '', new_line)
        # Tidy
        new_line = re.sub(r',\s*,', ',', new_line)
        new_line = re.sub(r',\s*$', '', new_line)
        new_line = re.sub(r'^\s*,\s*', '', new_line)
        new_line = re.sub(r'\s+', ' ', new_line)

        stripped = new_line.strip()
        if not stripped:
            continue
        if not new_line.startswith('-') and stripped and not new_line.startswith('  '):
            if line.lstrip().startswith('- '):
                new_line = '- ' + new_line.lstrip()
        # Drop orphan `- Concepts:` / `- Themes:` bullets when all wikilinks gone
        if re.match(r'^-\s+\w[\w\s]*:\s*$', stripped):
            orig_had_link = bool(re.search(r'\[\[[^\]]+\]\]', line))
            new_has_link = bool(re.search(r'\[\[[^\]]+\]\]', new_line))
            if orig_had_link and not new_has_link:
                continue

        new_lines.append(new_line)

    new_content = ''.join(new_lines)
    with open(out_path, 'w') as f:
        f.write(new_content)

    delta = {
        'orig_lines': original_lines,
        'new_lines': len(new_lines),
        'delta_lines': len(new_lines) - original_lines,
        'rewrites': rewrites,
    }
    return delta, new_content


if __name__ == '__main__':
    # CLI: python3 strip_wikilinks.py <slice-json> <out-dir>
    if len(sys.argv) < 3:
        print('Usage: strip_wikilinks.py <slice.json> <out-dir>', file=sys.stderr)
        sys.exit(2)
    slice_path = sys.argv[1]
    out_dir = sys.argv[2]
    os.makedirs(out_dir, exist_ok=True)

    with open(slice_path) as f:
        slice_data = json.load(f)

    manifest = {
        'modified_files': [],
        'created_stubs': [],
        'rewrites': [],
        'skipped': [],
    }

    for entry in slice_data:
        src_rel = entry['src']
        src_path = os.path.join(VAULT, src_rel)
        if not os.path.exists(src_path):
            manifest['skipped'].append({'src': src_rel, 'reason': 'source not found'})
            continue
        out_basename = os.path.basename(src_rel)
        out_path = os.path.join(out_dir, out_basename)
        delta, _ = apply_policies(entry, src_path, out_path)
        manifest['modified_files'].append({
            'src': src_rel,
            'output_path': out_path,
            **delta,
        })
        for t, p in {t['target']: t for t in entry['targets']}.items():
            if p['policy'] == 'CREATE_STUB' and not any(
                target == t and new_policy != 'CREATE_STUB'
                for (target, _), (new_policy, _) in SPECIAL_CASES.items()
            ):
                manifest['created_stubs'].append({
                    'target': t,
                    'output_path': os.path.join(out_dir, 'stubs', 'Concepts', f'{t}.md'),
                })
                manifest['rewrites'].append({
                    'src': src_rel,
                    'old': f'[[{t}]]',
                    'new': '(stub created in vault by parent)',
                })

    # Dedupe created_stubs
    seen = set()
    unique_stubs = []
    for s in manifest['created_stubs']:
        if s['target'] not in seen:
            seen.add(s['target'])
            unique_stubs.append(s)
    manifest['created_stubs'] = unique_stubs

    with open(os.path.join(out_dir, 'MANIFEST.json'), 'w') as f:
        json.dump(manifest, f, indent=2)
    print(f'Processed {len(manifest["modified_files"])} files')
    print(f'Created {len(manifest["created_stubs"])} stubs (planned)')
    print(f'Rewrites: {len(manifest["rewrites"])}')
    print(f'Skipped: {len(manifest["skipped"])}')