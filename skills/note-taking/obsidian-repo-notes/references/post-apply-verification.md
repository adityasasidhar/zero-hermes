# Post-apply verification for research-subject waves

After a subagent writes a research-subject wave to `/tmp/<wave>N/...` and the
parent agent copies the files into the vault, **two more checks are required
before declaring the wave done**. These catch the failure mode that no other
verification step catches: a subagent that *cited* a real-looking URL in the
Source log without ever fetching it.

## The two checks

### 1. HTTP-200 sweep over every URL in the Source log

Every URL listed in a Source log row must return `200 OK` (or the documented
success status for that site — `gatsby` build pages sometimes 301, etc.).

Run from the parent agent:

```python
import urllib.request, urllib.error

# Read the Source log to extract URLs
import re
src = open("/home/arctic/Documents/fun/Personal Growth/<Subject>/Source log.md").read()
urls = re.findall(r'https?://[^\s\)\]\|]+', src)
# dedupe, drop the table-pipe-trailing characters
urls = sorted(set(u.rstrip('.,;:') for u in urls))

failed = []
for url in urls:
    try:
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        r = urllib.request.urlopen(req, timeout=15)
        print(f"  ✓ {r.status}  {url}")
    except urllib.error.HTTPError as e:
        failed.append((url, f"HTTP {e.code}"))
        print(f"  ✗ HTTP {e.code}  {url}")
    except Exception as e:
        failed.append((url, f"{type(e).__name__}: {e}"))
        print(f"  ✗ {type(e).__name__}: {e}  {url}")

if failed:
    print(f"\n  {len(failed)}/{len(urls)} URLs failed. Wave is NOT shippable.")
    # Roll back the wave by restoring from /tmp/<wave>N/_pre_snapshot.json
else:
    print(f"\n  {len(urls)}/{len(urls)} URLs OK. Wave passes HTTP check.")
```

If any URL returns 404, 403, or times out, that's a hallucinated URL —
**do not ship the wave**. Roll back by restoring the file from the
`/tmp/<wave>N/_pre_snapshot.json` baseline (or by `git checkout` if the
vault has commits) and re-dispatch the subagent with stricter instructions
to actually fetch the page before citing it.

### 2. Numeric spot-check against the live primary source

A URL returning 200 doesn't prove the cited numbers actually appear on the
page. The subagent may have written `2,250 kcal/day` because it sounded
plausible; the live page might say `2,000 kcal/day`. The check: curl the
primary source and grep for every headline number cited in the wave.

```bash
# Fetch the canonical primary source
curl -sL --max-time 20 -A "Mozilla/5.0" \
  "https://blueprint.bryanjohnson.com/blogs/news/bryan-johnsons-protocol" \
  -o /tmp/<wave>N/_primary.html

# Extract every numeric claim mentioned in the concept notes
# (manually, or with a careful grep — false positives are fine, false
# negatives mean the subagent fabricated)

# For each headline number cited in Concepts/*.md, grep the live page:
for n in "8:30" "5:00" "2,250" "150 min" "Zone 2" "200°F" "HBOT" \
         "Metformin" "NMN" "Tadalafil" "71°F"; do
  hits=$(grep -c "$n" /tmp/<wave>N/_primary.html)
  printf "  %-15s → %3d hits on live page\n" "$n" "$hits"
done
```

A wave where every cited number appears on the live page is shippable.
A wave where one or more numbers return zero hits means that number is
fabricated — find the concept note that cited it, fix it (either to the
real number or to "Unknown — not publicly disclosed"), and re-run.

### Combined python one-liner

Both checks in a single `execute_code` call (use this pattern):

```python
import urllib.request, urllib.error, re, subprocess, os

VAULT = "/home/arctic/Documents/fun"
SUBJECT = "bryan_routine"  # the vault folder
SRC_LOG = f"{VAULT}/Personal Growth/{SUBJECT}/Source log.md"
PRIMARY = "https://blueprint.bryanjohnson.com/blogs/news/bryan-johnsons-protocol"
NUMBERS_TO_CHECK = ["8:30", "5:00", "2,250", "150 min", "Zone 2", "200°F",
                    "HBOT", "Metformin", "NMN", "Tadalafil", "71°F"]

# 1. URL sweep
src = open(SRC_LOG).read()
urls = sorted(set(re.findall(r'https?://[^\s\)\]\|]+', src)))
print("=" * 60)
print(f"URL sweep over {len(urls)} URLs from Source log")
print("=" * 60)
url_ok = 0
for url in urls:
    try:
        urllib.request.urlopen(urllib.request.Request(
            url, headers={"User-Agent": "Mozilla/5.0"}), timeout=15)
        url_ok += 1
    except Exception as e:
        print(f"  FAIL  {url}  → {type(e).__name__}: {e}")
print(f"\n{url_ok}/{len(urls)} URLs return 200")

# 2. Numeric spot-check
print("\n" + "=" * 60)
print(f"Numeric spot-check against primary source")
print("=" * 60)
subprocess.run(["curl", "-sL", "--max-time", "20", "-A", "Mozilla/5.0",
                PRIMARY, "-o", "/tmp/_primary.html"], check=True)
with open("/tmp/_primary.html") as f:
    page = f.read()
num_ok = 0
for n in NUMBERS_TO_CHECK:
    hits = page.count(n)
    marker = "  ✓" if hits > 0 else "  ✗"
    print(f"{marker}  {n:15} → {hits} hits")
    if hits > 0: num_ok += 1
print(f"\n{num_ok}/{len(NUMBERS_TO_CHECK)} headline numbers verified")

# Final verdict
if url_ok == len(urls) and num_ok == len(NUMBERS_TO_CHECK):
    print("\n  WAVE SHIPPABLE: all URL + numeric checks pass")
else:
    print("\n  WAVE NOT SHIPPABLE: investigate failures above")
```

## When the checks fail

For a 404 URL: the URL is fabricated. Find the Source log row that cited
it, replace it with the correct URL (or drop the row if no replacement
exists), and patch the concept notes that referenced that row.

For a missing number: the cited number is fabricated. Find the concept
note that cited it, replace with the correct value (verified by reading
the live page) or with "Unknown — not publicly disclosed." One missing
number per wave is usually enough to roll back; multiple missing numbers
means the subagent fabricated a meaningful fraction of the wave — re-dispatch.

## When the checks pass

State explicitly in the wave summary: "All N URLs returned HTTP 200 and
all M headline numbers appeared on the live primary source." This is the
audit trail a future reader needs to trust the wave.

## Why this matters

The Source log pattern only works if URLs are *real*. A Source log full of
404 URLs is decorative — it provides the *appearance* of provenance without
the substance. The two checks above are what make the Source log an audit
trail rather than a citation costume. They cost ~30 seconds and prevent
the single most embarrassing failure mode for a research-subject wave.

## When to skip these checks

For research-subject waves where the source URLs are obviously canonical
and stable (e.g. arxiv.org/abs/1234.5678 — the abstract page is permanent,
the ID is the URL), the URL sweep is still worth doing for the
*consistency* check, but the numeric spot-check is optional. The default
should be: run both checks on every wave. Skipping them is an explicit
decision the parent makes, not an oversight.