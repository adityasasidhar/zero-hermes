#!/usr/bin/env python3
"""Generate schema-consistent Obsidian notes for a GitHub user's repos.

Pulls REAL metadata + READMEs via `gh`, then writes one detailed note per
repo into a vault folder. Deterministic and idempotent — re-running
overwrites, so it's safe after README edits.

Usage:
  python3 generate_repo_notes.py --owner adityasasidhar --vault ~/Documents/fun/Github/Repos
  python3 generate_repo_notes.py --owner <owner> --vault <dir> --dry-run --limit 5

No pip deps (stdlib only). Requires `gh` authenticated.
"""
import argparse, base64, datetime, json, os, re, subprocess, sys

# ---- index fallback descriptions (edit for your account) ----
INDEX_DESC = {}  # name -> description; filled from API description first.

TECH_KW = {
    'Python': ['python'], 'JavaScript': ['javascript', 'node'], 'TypeScript': ['typescript', 'ts'],
    'Rust': ['rust', 'cargo'], 'HTML': ['html'], 'Astro': ['astro'], 'Jupyter Notebook': ['jupyter', 'notebook'],
    'React': ['react'], 'FastAPI': ['fastapi'], 'Flask': ['flask'], 'Streamlit': ['streamlit'],
    'PyTorch': ['pytorch', 'torch'], 'TensorFlow': ['tensorflow'], 'ONNX': [' onnx'],
    'Ollama': ['ollama'], 'LLaMA': ['llama'], 'GPT-2': ['gpt-2'], 'Stable Diffusion': ['stable diffusion'],
    'BART': ['bart'], 'Transformers': ['transformer'], 'OpenAI': ['openai'], 'Anthropic': ['anthropic', 'claude'],
    'Hugging Face': ['huggingface', 'hugging face'], 'MongoDB': ['mongodb'], 'SQLite': ['sqlite'],
    'PostgreSQL': ['postgresql', 'postgres'], 'Redis': ['redis'], 'Docker': ['docker'],
    'LangChain': ['langchain'], 'Chroma': ['chroma'], 'FAISS': ['faiss'], 'Qdrant': ['qdrant'],
    'MCP': ['model context protocol', 'mcp'], 'CLI': ['cli', 'argparse', 'typer'], 'Chrome Extension': ['chrome extension', 'chrome'],
    'Pygame': ['pygame'], 'scikit-learn': ['scikit', 'sklearn'], 'NumPy': ['numpy'], 'Pandas': ['pandas'],
    'Node.js': ['node.js', 'nodejs'], 'Express': ['express'], 'Next.js': ['next.js', 'nextjs'],
    'OpenCV': ['opencv', 'cv2'], 'Whisper': ['whisper'], 'Gemini': ['gemini'], 'Mistral': ['mistral'],
    'Gradio': ['gradio'], 'uv': ['uv '], 'pip': ['pip'], 'Poetry': ['poetry'], 'Pydantic': ['pydantic'],
}


def gh(args):
    r = subprocess.run(['gh', 'api'] + args, capture_output=True, text=True, timeout=60)
    if r.returncode != 0:
        raise RuntimeError(r.stderr.strip()[:200])
    return r.stdout


def clean(t):
    t = re.sub(r'<[^>]+>', '', t)
    t = re.sub(r'\[!\[[^\]]*\]\([^)]*\)\]\([^)]*\)', '', t)  # badge
    t = re.sub(r'!\[[^\]]*\]\([^)]*\)', '', t)               # image
    t = re.sub(r'\[[\s]*\]\([^)]*\)', '', t)                 # empty link
    t = re.sub(r'\[([^\]]+)\]\([^)]*\)', r'\1', t)           # link -> text
    t = re.sub(r'<!--.*?-->', '', t, flags=re.S)
    return t.strip()


def md_title(name):
    return name.replace('-', ' ').replace('_', ' ').title()


def intro_and_sections(text):
    lines = text.splitlines()
    i = 0
    while i < len(lines) and lines[i].strip().startswith('#'):
        i += 1
    sections, cur, buf, paras = {}, None, [], []
    for ln in lines[i:]:
        m = re.match(r'^#{1,6}\s+(.*)', ln)
        if m:
            if cur:
                sections[cur] = '\n'.join(buf).strip()
            cur, buf = m.group(1).strip(), []
        else:
            buf.append(ln)
            if ln.strip():
                paras.append(ln.strip())
    if cur:
        sections[cur] = '\n'.join(buf).strip()
    intro = ''
    for p in paras:
        if len(p) > 30 and not p.startswith(('-', '*', '>', '|', '`')):
            intro = p
            break
    return intro, sections


def extract_features(sections, text):
    feats = []
    for key in sections:
        if re.search(r'feature|key|highlight|what it does|capabilit', key, re.I):
            for ln in sections[key].splitlines():
                ln = ln.strip()
                if re.match(r'^[-*]\s+', ln):
                    f = clean(re.sub(r'^[-*]\s+', '', ln))
                    f = re.sub(r'\*\*(.*?)\*\*', r'\1', f)
                    if f and len(f) < 200:
                        feats.append(f)
    if not feats:
        for ln in text.splitlines()[:60]:
            ln = ln.strip()
            if re.match(r'^[-*]\s+', ln):
                f = clean(re.sub(r'^[-*]\s+', '', ln))
                f = re.sub(r'\*\*(.*?)\*\*', r'\1', f)
                if f and len(f) < 160 and not re.search(r'pip|npm|cargo|git clone|cd ', f, re.I):
                    feats.append(f)
    return feats[:10]


def is_cmd_line(ln):
    return bool(re.search(r'pip|npm|cargo|git clone|python|uv |poetry|npm i|brew|conda|docker|cd |make|go run|node ', ln, re.I))


def extract_install(sections, text, url):
    blocks = []
    for b in re.findall(r'```[a-zA-Z]*\n(.*?)```', text, re.S):
        lines = [l for l in b.splitlines() if l.strip()]
        if not lines:
            continue
        if sum(1 for l in lines if l.strip().startswith('#')) * 2 >= len(lines):
            continue  # comment-only
        if not any(is_cmd_line(l) for l in lines):
            continue
        bl = b.strip().replace('<repository-url>', url).replace('<your-username>', OWNER_PLACEHOLDER)
        if len(bl) < 600:
            blocks.append(bl)
    if not blocks:
        for key in sections:
            if re.search(r'install|setup|getting|quick.?start|usage', key, re.I):
                for b in re.findall(r'```[a-zA-Z]*\n(.*?)```', sections[key], re.S):
                    lines = [l for l in b.splitlines() if l.strip()]
                    if not lines:
                        continue
                    if sum(1 for l in lines if l.strip().startswith('#')) * 2 >= len(lines):
                        continue
                    if any(is_cmd_line(l) for l in lines):
                        bl = b.strip().replace('<repository-url>', url).replace('<your-username>', OWNER_PLACEHOLDER)
                        if len(bl) < 600:
                            blocks.append(bl)
    return blocks[:3]


def categorize(desc, lang, topics, name):
    blob = (desc + ' ' + name + ' ' + ' '.join(topics)).lower()
    if any(k in blob for k in ['agent', 'mcp', 'llm', 'gpt', 'claude', 'ollama']):
        return 'LLM / Agent'
    if any(k in blob for k in ['from scratch', 'tutorial', 'learning', 'implement', 'understand', 'educational', 'course']):
        return 'Learning / Educational'
    if any(k in blob for k in ['classifier', 'mnist', 'neural', 'transformer', 'model', 'babylm', 'training', 'dataset', 'ml ', 'machine learning', 'embedding']):
        return 'ML Research / Model'
    if 'game' in blob or 'pygame' in blob:
        return 'Game'
    if 'cli' in blob:
        return 'CLI Tool'
    if any(k in blob for k in ['web', 'react', 'flask', 'fastapi', 'astro', 'website', 'chrome extension', 'extension', 'frontend']):
        return 'Web App'
    if 'config' in blob or 'profile' in name:
        return 'Config / Profile'
    return 'Application / Utility'


def status_of(m):
    if m.get('archived'):
        return 'archived'
    try:
        pushed = datetime.datetime.fromisoformat(m['pushed_at'].replace('Z', '+00:00'))
        if (datetime.datetime.now(datetime.timezone.utc) - pushed).days > 180:
            return 'experimental / inactive'
    except Exception:
        pass
    return 'active'


def fmt_date(s):
    try:
        return s[:10]
    except Exception:
        return ''


def strip_badge_line(line):
    line = re.sub(r'\[!\[[^\]]*\]\([^)]*\)\]\([^)]*\)', '', line)
    line = re.sub(r'!\[[^\]]*\]\([^)]*\)', '', line)
    line = re.sub(r'\[[\s]*\]\([^)]*\)', '', line)
    return line


REQUIRED = ['repo:', 'description:', 'github:', 'visibility:', 'language:', 'topics:', 'stars:',
            'forks:', 'status:', 'created:', 'last_pushed:', 'type:', 'vault_group:',
            '## Overview', '## Links', 'Part of: [[']


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--owner', default=None, help='GitHub owner (default: authenticated user)')
    ap.add_argument('--vault', required=True, help='Vault Repos directory')
    ap.add_argument('--dry-run', action='store_true')
    ap.add_argument('--limit', type=int, default=0)
    args = ap.parse_args()

    global OWNER_PLACEHOLDER
    OWNER_PLACEHOLDER = args.owner or json.loads(gh(['user', '--jq', '.login']))
    owner = OWNER_PLACEHOLDER

    if not os.path.isdir(args.vault):
        sys.exit(f"Vault dir not found: {args.vault}")

    # 1) metadata (full JSON — NEVER --jq '.{...}' object constructors, see skill pitfalls)
    repos = [json.loads(l) for l in gh([f"users/{owner}/repos?per_page=100&type=owner&sort=updated"]).splitlines() if l.strip()]
    if args.limit:
        repos = repos[:args.limit]

    written = 0
    for d in repos:
        name = d['name']
        url = f"https://github.com/{owner}/{name}"
        # 2) README
        rd = ''
        try:
            c = gh([f"repos/{owner}/{name}/readme", '--jq', '.content'])
            if c.strip():
                rd = base64.b64decode(c.strip()).decode('utf-8', 'replace')
        except RuntimeError:
            pass

        desc = d.get('description') or INDEX_DESC.get(name) or (clean(rd)[:200] if rd else '')
        desc = clean(desc)
        vis = d.get('visibility', 'public')
        lang = d.get('language') or 'Unknown'
        topics = d.get('topics') or []
        intro, sections = intro_and_sections(rd) if rd else ('', {})
        overview = intro if intro else desc
        feats = extract_features(sections, rd)
        found = set()
        low = rd.lower()
        for label, kws in TECH_KW.items():
            for kw in kws:
                if kw.strip() and kw.strip().lower() in low:
                    found.add(label)
                    break
        if lang:
            found.add(lang)
        for t in topics:
            found.add(t.replace('-', ' ').title())
        tech = sorted(found)
        installs = extract_install(sections, rd, url)
        cat = categorize(desc, lang, topics, name)
        st = status_of(d)
        created = fmt_date(d.get('created_at', ''))
        pushed = fmt_date(d.get('pushed_at', ''))
        stars = d.get('stargazers_count', 0) or 0
        forks = d.get('forks_count', 0) or 0
        group = 'Private Repos' if vis == 'private' else 'Public Repos'

        L = ['---', f'repo: {name}', f'description: {desc[:300]}', f'github: {url}',
             f'visibility: {vis}', f'language: {lang}',
             'topics: [' + (', '.join(topics)) + ']' if topics else 'topics: []',
             f'stars: {stars}', f'forks: {forks}', f'status: {st}', f'created: {created}',
             f'last_pushed: {pushed}', f'type: {cat}', f'vault_group: {group}', '---', '',
             f'# {md_title(name)', '']
        ov = '\n'.join(strip_badge_line(l) for l in overview.splitlines())
        ov = '\n'.join(ln for ln in ov.splitlines() if ln.strip())
        if ov:
            L += [ov, '']
        L += ['## Overview', '', desc if desc else 'No description provided.', '']
        if feats:
            L += ['## Key Features', ''] + [f'- {f}' for f in feats] + ['']
        if tech:
            L += ['## Tech Stack', '', '- **Primary language:** ' + (lang if lang != 'Unknown' else 'Not specified')]
            others = [t for t in tech if t != lang]
            if others:
                L.append('- **Frameworks / libraries / services:** ' + (', '.join(others)))
            if topics:
                L.append('- **GitHub topics:** ' + (', '.join(topics)))
            L.append('')
        if installs:
            L += ['## Getting Started', '']
            for b in installs:
                L += ['```', b, '```', '']
        L += ['## Stats', '', f'- ⭐ Stars: {stars}', f'- 🍴 Forks: {forks}', f'- 🔒 Visibility: {vis}',
              f'- 📅 Created: {created}', f'- 🔄 Last pushed: {pushed}', f'- 🏷️ Category: {cat}',
              f'- 📊 Status: {st}', '', '## Links', '', f'- GitHub: {url}', f'- Part of: [[{group}]]']
        content = '\n'.join(L) + '\n'

        if args.dry_run:
            print(f"[dry-run] would write {name}.md ({len(content)} chars)")
        else:
            with open(os.path.join(args.vault, name + '.md'), 'w', encoding='utf-8') as f:
                f.write(content)
        written += 1

    print(f"Processed {written} repos" + (" (dry-run)" if args.dry_run else ""))


if __name__ == '__main__':
    main()
