#!/usr/bin/env python3
"""Generate schema-consistent Obsidian repo notes from GitHub metadata + READMEs.

Usage:
    python3 generate_repo_notes.py --owner adityasasidhar \
        --repos-dir /home/arctic/Documents/fun/Github/Repos

Requires the `gh` CLI authenticated (`gh auth status`). Private repos are fetched
per-name. Existing substantial notes (>12 lines / >600 chars) are preserved.
Re-runnable: reads cached /tmp/repos_full.json + /tmp/readmes.json when present,
or pass --refresh to force re-fetch.
"""
import argparse, os, re, json, base64, subprocess, datetime, sys

TECH_KW = {
    'Python': ['python'], 'JavaScript': ['javascript', 'node'], 'TypeScript': ['typescript', 'ts'],
    'Rust': ['rust', 'cargo'], 'HTML': ['html'], 'Astro': ['astro'],
    'Jupyter Notebook': ['jupyter', 'notebook'], 'React': ['react'], 'FastAPI': ['fastapi'],
    'Flask': ['flask'], 'Streamlit': ['streamlit'], 'PyTorch': ['pytorch', 'torch'],
    'TensorFlow': ['tensorflow'], 'ONNX': [' onnx'], 'Ollama': ['ollama'],
    'LLaMA': ['llama'], 'GPT-2': ['gpt-2'], 'Stable Diffusion': ['stable diffusion'],
    'BART': ['bart'], 'Transformers': ['transformer'], 'OpenAI': ['openai'],
    'Anthropic': ['anthropic', 'claude'], 'Hugging Face': ['huggingface', 'hugging face'],
    'MongoDB': ['mongodb'], 'SQLite': ['sqlite'], 'PostgreSQL': ['postgresql', 'postgres'],
    'Redis': ['redis'], 'Docker': ['docker'], 'LangChain': ['langchain'],
    'Chroma': ['chroma'], 'FAISS': ['faiss'], 'Qdrant': ['qdrant'],
    'MCP': ['model context protocol', 'mcp'], 'CLI': ['cli', 'argparse', 'typer'],
    'Chrome Extension': ['chrome extension', 'chrome'], 'Pygame': ['pygame'],
    'scikit-learn': ['scikit', 'sklearn'], 'NumPy': ['numpy'], 'Pandas': ['pandas'],
    'Node.js': ['node.js', 'nodejs'], 'Express': ['express'], 'Next.js': ['next.js', 'nextjs'],
    'OpenCV': ['opencv', 'cv2'], 'Whisper': ['whisper'], 'Gemini': ['gemini'],
    'Mistral': ['mistral'], 'Gradio': ['gradio'], 'uv': ['uv '], 'pip': ['pip'],
    'Poetry': ['poetry'], 'Pydantic': ['pydantic'],
}

def gh_json(path, timeout=30):
    r = subprocess.run(['gh', 'api', path], capture_output=True, text=True, timeout=timeout)
    return json.loads(r.stdout) if r.returncode == 0 and r.stdout.strip() else None

def clean(t):
    t = re.sub(r'<[^>]+>', '', t)
    t = re.sub(r'\[!\[[^\]]*\]\([^)]*\)\]\([^)]*\)', '', t)
    t = re.sub(r'!\[[^\]]*\]\([^)]*\)', '', t)
    t = re.sub(r'\[[\s]*\]\([^)]*\)', '', t)
    t = re.sub(r'\[([^\]]+)\]\([^)]*\)', r'\1', t)
    t = re.sub(r'<!--.*?-->', '', t, flags=re.S)
    return t.strip()

def md_title(name):
    return name.replace('-', ' ').replace('_', ' ').title()

def intro_and_sections(text):
    lines = text.splitlines()
    i = 0
    while i < len(lines) and lines[i].strip().startswith('#'):
        i += 1
    sections = {}; cur = None; buf = []; paras = []
    for ln in lines[i:]:
        m = re.match(r'^#{1,6}\s+(.*)', ln)
        if m:
            if cur: sections[cur] = '\n'.join(buf).strip()
            cur = m.group(1).strip(); buf = []
        else:
            buf.append(ln)
            if ln.strip(): paras.append(ln.strip())
    if cur: sections[cur] = '\n'.join(buf).strip()
    intro = ''
    for p in paras:
        if len(p) > 30 and not p.startswith(('-', '*', '>', '|', '`')):
            intro = p; break
    return intro, sections

def extract_features(sections, text):
    feats = []
    for key in sections:
        if re.search(r'feature|key|highlight|what it does|capabilit', key, re.I):
            for ln in sections[key].splitlines():
                ln = ln.strip()
                if re.match(r'^[-*]\s+', ln):
                    f = re.sub(r'\*\*(.*?)\*\*', r'\1', clean(re.sub(r'^[-*]\s+', '', ln)))
                    if f and len(f) < 200: feats.append(f)
    if not feats:
        for ln in text.splitlines()[:60]:
            ln = ln.strip()
            if re.match(r'^[-*]\s+', ln):
                f = re.sub(r'\*\*(.*?)\*\*', r'\1', clean(re.sub(r'^[-*]\s+', '', ln)))
                if f and len(f) < 160 and not re.search(r'pip|npm|cargo|git clone|cd ', f, re.I):
                    feats.append(f)
    return feats[:10]

def is_cmd(ln):
    return bool(re.search(r'pip|npm|cargo|git clone|python|uv |poetry|npm i|brew|conda|docker|cd |make|go run|node ', ln, re.I))

def extract_install(sections, text, url):
    blocks = []
    for b in re.findall(r'```[a-zA-Z]*\n(.*?)```', text, re.S):
        lines = [l for l in b.splitlines() if l.strip()]
        if not lines: continue
        if sum(1 for l in lines if l.strip().startswith('#')) * 2 >= len(lines): continue
        if not any(is_cmd(l) for l in lines): continue
        bl = b.strip().replace('<repository-url>', url).replace('<your-username>', OWNER)
        if len(bl) < 600: blocks.append(bl)
    return blocks[:3]

def categorize(desc, lang, topics, name):
    blob = (desc + ' ' + name + ' ' + ' '.join(topics)).lower()
    if any(k in blob for k in ['agent', 'mcp', 'llm', 'gpt', 'claude', 'ollama']): return 'LLM / Agent'
    if any(k in blob for k in ['from scratch', 'tutorial', 'learning', 'implement', 'understand', 'educational', 'course']): return 'Learning / Educational'
    if any(k in blob for k in ['classifier', 'mnist', 'neural', 'transformer', 'model', 'babylm', 'training', 'dataset', 'ml ', 'machine learning', 'embedding']): return 'ML Research / Model'
    if 'game' in blob or 'pygame' in blob: return 'Game'
    if 'cli' in blob: return 'CLI Tool'
    if any(k in blob for k in ['web', 'react', 'flask', 'fastapi', 'astro', 'website', 'chrome extension', 'extension', 'frontend']): return 'Web App'
    if 'config' in blob or 'profile' in name: return 'Config / Profile'
    return 'Application / Utility'

def status_of(m):
    if m.get('archived'): return 'archived'
    try:
        pushed = datetime.datetime.fromisoformat(m['pushed_at'].replace('Z', '+00:00'))
        if (datetime.datetime.now(datetime.timezone.utc) - pushed).days > 180:
            return 'experimental / inactive'
    except Exception: pass
    return 'active'

def fmt_date(s):
    try: return s[:10]
    except Exception: return ''

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--owner', required=True)
    ap.add_argument('--repos-dir', required=True)
    ap.add_argument('--refresh', action='store_true')
    args = ap.parse_args()
    global OWNER; OWNER = args.owner
    VAULT = args.repos_dir
    notes = sorted(os.path.splitext(f)[0] for f in os.listdir(VAULT) if f.endswith('.md'))

    meta_path, readme_path = '/tmp/repos_full.json', '/tmp/readmes.json'
    if args.refresh or not os.path.exists(meta_path):
        data = gh_json('users/%s/repos?per_page=100&type=owner&sort=updated') or []
        meta = {d['name']: d for d in data}
        for n in notes:
            if n not in meta:
                d = gh_json('repos/%s/%s' % (OWNER, n))
                if d: meta[n] = d
        json.dump(meta, open(meta_path, 'w'))
    else:
        meta = json.load(open(meta_path))

    if args.refresh or not os.path.exists(readme_path):
        readmes = {}
        for n in notes:
            out = subprocess.run(['gh', 'api', 'repos/%s/%s/readme' % (OWNER, n), '--jq', '.content'],
                                 capture_output=True, text=True, timeout=30)
            c = None
            if out.returncode == 0 and out.stdout.strip():
                try: c = base64.b64decode(out.stdout.strip()).decode('utf-8', 'replace')
                except Exception: c = None
            readmes[n] = c
        json.dump(readmes, open(readme_path, 'w'))
    else:
        readmes = json.load(open(readme_path))

    written = skipped = 0
    for name in notes:
        path = os.path.join(VAULT, name + '.md')
        if os.path.exists(path):
            cur = open(path, encoding='utf-8').read()
            if len(cur.splitlines()) > 12 or len(cur) > 600:
                skipped += 1; continue  # preserve curated note
        m = meta.get(name, {})
        rd = readmes.get(name) or ''
        desc = m.get('description') or (clean(rd)[:200] if rd else '')
        desc = clean(desc)
        vis = m.get('visibility', 'public')
        lang = m.get('language') or 'Unknown'
        topics = m.get('topics') or []
        intro, sections = intro_and_sections(rd) if rd else ('', {})
        overview = intro if intro else desc
        feats = extract_features(sections, rd)
        found = set(); low = rd.lower()
        for label, kws in TECH_KW.items():
            for kw in kws:
                if kw.strip() and kw.strip().lower() in low:
                    found.add(label); break
        if lang: found.add(lang)
        for t in topics: found.add(t.replace('-', ' ').title())
        tech = sorted(found)
        installs = extract_install(sections, rd, 'https://github.com/%s/%s' % (OWNER, name))
        cat = categorize(desc, lang, topics, name)
        st = status_of(m)
        created = fmt_date(m.get('created_at', ''))
        pushed = fmt_date(m.get('pushed_at', ''))
        stars = m.get('stargazers_count', 0) or 0
        forks = m.get('forks_count', 0) or 0
        group = 'Private Repos' if vis == 'private' else 'Public Repos'
        url = 'https://github.com/%s/%s' % (OWNER, name)

        L = ['---', f'repo: {name}', f'description: {desc[:300]}', f'github: {url}',
             f'visibility: {vis}', f'language: {lang}',
             f'topics: [{((", ".join(topics)))}]' if topics else 'topics: []',
             f'stars: {stars}', f'forks: {forks}', f'status: {st}',
             f'created: {created}', f'last_pushed: {pushed}', f'type: {cat}',
             f'vault_group: {group}', '---', '', f'# {md_title(name)}', '']
        ov = '\n'.join(ln for ln in (clean(x) for x in overview.splitlines()) if ln.strip())
        if ov: L += [ov, '']
        L += ['## Overview', '', desc if desc else 'No description provided.', '']
        if feats:
            L += ['## Key Features', '']
            L += [f'- {f}' for f in feats] + ['']
        if tech:
            L += ['## Tech Stack', '', '- **Primary language:** ' + (lang if lang != 'Unknown' else 'Not specified')]
            others = [t for t in tech if t != lang]
            if others: L.append('- **Frameworks / libraries / services:** ' + (', '.join(others)))
            if topics: L.append('- **GitHub topics:** ' + (', '.join(topics)))
            L.append('')
        if installs:
            L += ['## Getting Started', '']
            for b in installs: L += ['```', b, '```', '']
        L += ['## Stats', '', f'- ⭐ Stars: {stars}', f'- 🍴 Forks: {forks}',
              f'- 🔒 Visibility: {vis}', f'- 📅 Created: {created}',
              f'- 🔄 Last pushed: {pushed}', f'- 🏷️ Category: {cat}', f'- 📊 Status: {st}', '',
              '## Links', '', f'- GitHub: {url}', f'- Part of: [[{group}]]']
        open(path, 'w', encoding='utf-8').write('\n'.join(L) + '\n')
        written += 1
    print(f'wrote={written} skipped(curated)={skipped} total_notes={len(notes)}')

if __name__ == '__main__':
    main()
