# Extracting dates from the Hugging Face blog

The HF blog doesn't expose a clean JSON list endpoint. The reliable way to get every post's date is to scrape `/blog` for links, then fetch each post and pull `datePublished` from the JSON-LD block in the HTML.

## One-liner — pull all recent post URLs

```bash
curl -sL https://huggingface.co/blog | \
  grep -oE 'href="/blog/[^"]+"' | sort -u
```

## Loop to print (url, date) for each post

```bash
for url in $(curl -sL https://huggingface.co/blog | grep -oE 'href="/blog/[^"]+"' | sed 's/href="//;s/"//' | sort -u); do
  full="https://huggingface.co$url"
  date=$(curl -sL "$full" | grep -oE '"datePublished"\s*:\s*"[^"]+"' | head -1)
  echo "$full  $date"
done
```

## What the JSON-LD looks like

The blog post HTML embeds:

```html
<script type="application/ld+json">
{
  "@context": "https://schema.org",
  "@type": "BlogPosting",
  "headline": "...",
  "datePublished": "2026-07-23T21:56:43.299Z",
  ...
}
</script>
```

The regex `'"datePublished"\s*:\s*"[^"]+"'` matches it reliably across every post format HF has used.

## Pitfalls

- **URL slugs are NOT predictable from the title.** `cosmos3edge` works for NVIDIA's Cosmos 3 Edge post; `introducing-cosmos-3-edge` 404s. Always test with `curl -sIL` first if you're guessing.
- **Authors get a subdirectory:** `/blog/{author}/{slug}`. `{author}` is usually a username (`/blog/badaoui/...`, `/blog/jeffboudier/...`) but can be an org or nickname.
- **Don't forget the auth flag on /api/blogPosts.** That endpoint is real (`GET /api/blogPosts/{author}/{slug}`) but inconsistent; the HTML scrape + JSON-LD regex is more reliable.
- **Filtering to "in window":** once you have the (url, date) list, sort by `datePublished` descending. The first 10-15 entries are usually the most recent.
- **Speed:** each post fetch is ~50-100 KB of HTML. For 30 posts that's ~3 MB. Tolerable. Don't parallelize aggressively or you'll 429.

## Full extractor (drop into a script)

```python
import subprocess, re, datetime

def fetch_blog_index():
    r = subprocess.run(
        ["curl", "-sL", "https://huggingface.co/blog"],
        capture_output=True, text=True, timeout=30,
    )
    return sorted(set(re.findall(r'href="(/blog/[^"]+)"', r.stdout)))

def fetch_date(slug):
    url = f"https://huggingface.co{slug}"
    r = subprocess.run(["curl", "-sL", url], capture_output=True, text=True, timeout=15)
    m = re.search(r'"datePublished"\s*:\s*"([^"]+)"', r.stdout)
    return url, m.group(1) if m else None

posts = fetch_blog_index()
for slug in posts:
    url, date = fetch_date(slug)
    if date:
        print(f"{date}  {url}")
```
