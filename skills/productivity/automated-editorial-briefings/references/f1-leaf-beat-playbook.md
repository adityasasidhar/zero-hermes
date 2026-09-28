# F1 Leaf Beat Playbook

Class-level reference for any F1 / motorsport leaf correspondent. Built from
real Hungarian GP 2026 sourcing. Cross-reference from `leaf-beat-reporter-workflow.md`
for the general flow; this file covers F1-specific extraction.

## Primary sources (tier order)

1. **formula1.com** — official race reports, qualifying reports, "what the
   teams said", grid announcements. Most articles publish within minutes
   of session end. Hero images and metadata are reliable.
2. **fia.com/news** — race reports, steward decisions, calendar changes,
   technical directives. Slow on race coverage (hours after the flag) but
   authoritative on regulations and officiating.
3. **motorsport.com / the-race.com / autosport.com / bbc.com/sport/formula1** —
   wire-quality secondary. Good for driver-market colour and driver quotes.
   Avoid citing as primary for race results.
4. **Team official sites / FIA team press kits** — for Aston Martin,
   McLaren, Mercedes, Ferrari, Red Bull upgrades and post-race statements.

## Formula1.com — extraction patterns

The site is a Next.js app. The body text ships in `__next_f.push(...)`
JSON blobs, not in the static HTML. This means:

- `web_extract` works fine for the rendered article body. You do not need
  the browser. The plain text after extraction reads cleanly.
- Hero image URL and credits live in separate places. You must extract
  three fields separately.

### Finding article URLs on the latest page

The `/en/latest.html` index lists every ranked article. Every link has
the form `/en/latest/article/<slug>.<hash>` — the bare slug returns
**404**: the hash suffix is mandatory. Pattern that works:

```python
import re, urllib.request
req = urllib.request.Request(
    "https://www.formula1.com/en/latest.html",
    headers={"User-Agent": "Mozilla/5.0 (X11; Linux) Hermes/1.0"})
html = urllib.request.urlopen(req, timeout=20).read().decode("utf-8", "ignore")

# This regex captures the full path including the hash.
article_urls = re.findall(r'href="(/en/latest/article/[^"]+)"', html)

# Dedupe preserving order, then build full URLs.
seen, unique = set(), []
for u in article_urls:
    if u not in seen:
        seen.add(u); unique.append(u)
for u in unique:
    print(f"https://www.formula1.com{u}")
```

**Traps:**

- The shorter-looking regex `r'href="(/en/latest/article/[a-z0-9]+)"'`
  silently returns zero hits — the URL class needs to accept `.` and
  `-` (the hash separator and slug word boundaries). Always use
  `[^"]+` inside the URL class for Next.js article-link extraction.
- The visible `<a>` text is the article's actual title (HTML-entity
  decoded; `&#x27;` is a single quote). Use it for fast `grep`
  filtering before drilling into each article.
- For each candidate, fetch `datePublished` from the article page
  (`grep '"datePublished"'`) and filter against the 72-hour cutoff
  *before* running `web_extract` on the body. The latest page mixes
  yesterday's late posts with today's, so the listing alone is not a
  reliable window filter. Verified 2026-07-29: the latest page listed
  16 articles, with `datePublished` ranging from 2026-07-27T11:19Z
  (yesterday) to 2026-07-28T14:44Z (today). A naive "top 4" pick
  duplicates the leader, the race, and respectful-racing stories from
  yesterday's edition.

### Pulling the publish date

Look in the Next.js streamed JSON, not the visible `<time>` tag:

```python
import re
date = re.search(r'"datePublished"\s*:\s*"([^"]+)"', html).group(1)
# e.g. "2026-07-26T14:54:41.131Z"
```

This is UTC with `Z`. Use it directly as `published_at` — no conversion.

The visible `<time>` text ("Jul 26, 2026 2:54pm UTC") and the JSON-LD
`datePublished` agree, but the JSON value is what the page treats as
canonical. Trust the JSON value.

### Pulling the hero image ID

The article's hero `<img>` has a numeric `public_id` like `2287727846`. The
id appears in three places; pick any:

```python
# Option A: og:image meta tag (easiest)
og = re.search(r'og:image" content="(https://media\.formula1\.com/[^"]+2287727846[^"]+)"', html).group(1)

# Option B: preload link
pre = re.search(r'<link rel="preload" as="image" href="(https://media\.formula1\.com/[^"]+2287727846[^"]+)"', html).group(1)
```

The `og:image` URL uses Cloudinary auto-formatting (`/q_auto/` + `/f_auto/`)
and returns WebP. To get a guaranteed JPEG, swap the transformation:

```python
# template: insert f_jpg after w_X/ segment, replace v1740000001 → keep
jpg_url = re.sub(r'/q_auto/', '/f_jpg/q_auto/', og_url)
```

Or build from scratch using the `public_id`:

```text
https://media.formula1.com/image/upload/t_16by9North/c_lfill,w_3392/f_jpg/q_auto/v1740000001/trackside-images/2026/F1_Grand_Prix_of_Hungary/<ID>.jpg
```

The `t_16by9North`, `t_16by9South`, `t_16by9Centre` transforms are the
official thumbnail crops. `c_lfill,w_3392` clamps to a 3392px-wide fill.
For a lead image, `w_3392` is the right size.

### Pulling the photographer credit

This is the hard part. The credit text never appears in the rendered
article body — it lives in Next.js's `__next_f.push()` JSON for
related-article thumbnails. Search like this:

```python
# Credit patterns that appear in the related-article JSON
for m in re.finditer(r'Photo by ([^()]+)/([^()]+Getty Images)', html):
    print(m.group(0))
# Common shapes:
#   "Photo by Mark Sutton - Formula 1/Formula 1 via Getty Images"
#   "Photo by Rudy Carezzevoli/Getty Images"
#   "Photo by Sona Maleterova/Getty Images"
#   "Photo by Dom Gibbons - Formula 1/Formula 1 via Getty Images"
```

For the hero image specifically, the article's own hero may not carry
an in-page credit string. In that case, use the same photographer name
that appears on other race-weekend photos from the same outlet (Formula 1
/ Formula 1 via Getty Images is the default for `trackside-images/`).

**When you cannot pin a credit with confidence**, fall back to:

```json
"credit": "Formula 1 / Formula 1 via Getty Images"
```

This is correct for the vast majority of `trackside-images/` Cloudinary
IDs published in 2026 race-weekend articles. Only override if you found
a specific named photographer string.

**Nuance for `fom-website/` upload paths.** The Cloudinary URL segment
distinguishes two upload classes:

- `trackside-images/<year>/<gp_slug>/<id>.jpg` — race-weekend photos
  taken by the Formula 1 / Getty pool photographers. Default credit is
  `Formula 1 / Formula 1 via Getty Images`.
- `fom-website/<year>/<red-bull|ferrari|...>/<slug>.<ext>` — art
  uploaded by the FOM editorial team for announcement-style pieces
  (driver hires, sponsor reveals, livery launches). Credits vary:
  `Mark Thompson/Getty Images`, `Team handout`, or the team's own media
  channel. The Lagrue/Red Bull Junior Programme article (2026-07-27)
  uses `fom-website/2026/Red Bull/Lagrue.jpg` with `Photo by Mark
  Thompson/Getty Images`. **Do not blanket-default these to
  "Formula 1 / Formula 1 via Getty Images" — they are editorial
  uploads, not trackside photography.** Scan the `__next_f.push()`
  JSON for `Photo by <name>...` patterns; if none surface, the photo
  is most likely a team handout and the credit should be `Formula 1`
  (editorial label) or `<Team> handout`.

### Verifying the downloaded image

```bash
curl -A "Mozilla/5.0" -sSL "<jpg_url>" -o /path/to/f1_<slug>.jpg
file /path/to/f1_<slug>.jpg   # MUST say "JPEG image data"
ls -l /path/to/f1_<slug>.jpg  # MUST be > 20 KB; expect ~300-500 KB at w_3392
```

If `file` says "HTML document" or the bytes are < 5 KB, the Cloudinary
URL was malformed. Re-check the `public_id` and the path segments.

## FIA.com — extraction patterns

FIA.com is a Drupal site with classic server-rendered HTML. Easier than
formula1.com.

### Pulling the publish date

```python
# FIA uses this exact meta tag
date = re.search(r'article:published_time" content="([^"]+)"', html).group(1)
# e.g. "2026-07-26T09:09:00+02:00"
```

**Trap:** FIA dates carry the host-city timezone. For race-weekend
releases from Budapest, the offset is `+02:00` (CEST), **not** UTC.
Convert to UTC before emitting `published_at`:

```python
from datetime import datetime, timezone
dt = datetime.fromisoformat("2026-07-26T09:09:00+02:00")
utc = dt.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
# → "2026-07-26T07:09:00Z"
```

### Pulling FIA hero images

```python
# og:image on FIA is usually a clean jpg path under /sites/default/files/
og = re.search(r'og:image" content="(https://www\.fia\.com/sites/default/files/[^"]+)"', html).group(1)
# The og:image is already a JPEG. Download directly.
```

## Citing driver penalties (steward documents)

When the beat asks "what penalty did X get", the steward decision
document is published on FIA.com or surfaced via formula1.com's
penalty-explainer articles. The exact regulatory clause is in the
decision text. Common shapes:

- "failing to slow sufficiently under yellow flags" → Article 2.5.5 b)
  of the International Sporting Code
- "impeding another driver" → standard 3-place grid drop
- "unsafe release" → 5-second time penalty (and team fined)

For 2026, the standard grid-penalty guidelines are:

- Impeding in qualifying: 3 grid places
- Yellow-flag breach: 5 grid places (reduced to 3 if "limited time and
  distance to react" — see Antonelli's Hungary 2026 ruling)
- Pit-lane speeding: 5 seconds added at the finish

## F1-specific race-window disciplines

For a typical F1 race weekend (Fri-Sun), the beat window is:

- **Friday**: FP1 + FP2 reports, plus the FIA stewards' Thursday press
  conference transcript (rare but useful).
- **Saturday**: FP3 + Qualifying report + "what the teams said" + grid
  confirmation. Penalties reshuffle the grid in the late afternoon.
- **Sunday**: Race report + driver-of-the-day + post-race press
  conference transcript + stewards' documents.

All three days fit inside a 72-hour window comfortably if the briefing
runs Monday-morning IST. If the briefing runs during the weekend, focus
on what's published so far and **do not pre-write the race result** from
social posts.

## Calendar / breaking-news flag

When the FIA confirms a calendar change (e.g. the 2026 Bahrain→Malaysia
swap), it lands as a top-tier story ahead of any on-track action even
on a race weekend. The release time is usually mid-morning local on
race day or the day before. Do not let a calendar story displace the
race result — keep the calendar item at rank 3-4 unless it materially
affects the current race.

## Pitfalls specific to F1

- **The "race winner" hero photo and the on-track photo are different
  IDs.** When you want a celebration photo for the lead, look for the
  podium image (`Race winner Lando Norris of Great Britain and McLaren
  celebrates on`). The on-track collision/incident photo is a separate
  ID. Both work for `art_source_url`; pick by narrative.
- **Getty ID ≠ Cloudinary ID.** Formula1.com serves Cloudinary-hosted
  assets whose Cloudinary `public_id` ends in the same number as the
  underlying Getty asset (e.g. `2287727846`). The Cloudinary URL is
  the one to download; the Getty page is the credit attribution chain.
- **Race results can shift after the flag.** Steward penalties applied
  30-60 minutes after the finish can demote/promote drivers. Always
  cite the official classification as published by formula1.com or
  FIA.com, not the live-timing provisional classification.
- **Steward document URLs are not stable.** FIA's `/decision/` paths
  change. Link to the formula1.com explainer article instead when
  available — it is a stable URL and quotes the relevant document text.

## The shutdown week is NOT a dead week

The FIA-mandated 14-day factory shutdown (Article 21.8 — typically late
July through mid-August for a Hungarian-GP-ending calendar) does **not**
silence the beat. Four story archetypes reliably surface in the
shutdown window and have been confirmed in-window for the 2026 cycle
(Aug 1-2 lead-time statements):

1. **Horner-return speculation / Wolff on Horner.** Sacked team
   principals and re-entry rumours keep moving during the silence —
   Wolff, Horner, Briatore, Marko all give press during this window.
   Lead candidate: `Auto Hebdo`, `ESPN`, `RACER`, `Reuters` pickup of
   Press Association quotes. **Trap (real 2026-08-04 lesson):** the
   canonical Press Association / ESPN / RACER / Sky Sports "Wolff casts
   doubt on Horner return" pieces are all dated **March 30, 2026** —
   they are NOT fresh Aug 1-3 statements. A leaf that searches "Horner
   Alpine August 2026" and grabs the Wolff "broken glass" quote without
   checking `datePublished` will publish a 5-month-stale statement as if
   it were shutdown-week news. **Always parse `datePublished` (or URL
   slug date) and confirm the article lands inside the rolling window
   before using it as a rank-1 lead.** If the only Horner-Wolff articles
   in search are from January-March 2026, either drop Horner-Wolff as a
   story entirely OR move it down to a rank-3 "context: how this Alpine
   stake saga got to here" frame with the actual August source as rank 1.
2. **Power-unit / chassis budget decisions.** Racing directors (McNish
   at Audi, Watanabe at Honda, Mekies at Red Bull) use the quiet week
   to confirm 2027-vs-2026 resource splits. The tell is a press
   event or interview tied to a Hungary-weekend release. Sources:
   `motorsport.com` Italian-language desk (`it.motorsport.com`),
   `GPFans`, `Motorsport Week`, `RacingNews365`. The McNish
   2027-Audi freeze (`audi-wont-upgrade-f1-power-unit-again-until-2027`)
   is the canonical 2026 example.
3. **Driver contract framing from team principals.** Newey on Alonso,
   Vowles on Sainz, Wolff on Russell/Antonelli — these set up the
   post-summer driver-market cycle. Source: official team sites
   (`astonmartinf1.com`, `williamsf1.com`) plus `formula1.com` deep
   coverage. The Newey-Alonso 2027 framing piece
   (`newey-shares-update-on-aston-martins-chances-of-keeping-alonso-for-2027`)
   is the canonical 2026 example.
4. **Circuit / promoter ownership shuffles in the final-event window.**
   When the F1 calendar confirms a circuit's last race (Zandvoort 2026
   being the canonical example), the weeks before the farewell surface
   real ownership statements: principals selling stakes, transition
   announcements, "the mission is complete" quotes. **Confirmed 2026-08-02
   in-window example:** Prince Bernhard of Orange-Nassau selling his
   stake in Circuit Zandvoort to partners De Jong and Coen Groeneveld,
   with Verstappen quoted on a special Dutch-tribute helmet. Source:
   `f1-fansite.com/f1-news/prince-sells-zandvoort-stake-before-final-dutch-gp`,
   cross-reference `visitzandvoort.com/circuit/formula-1`,
   `racingnews365.com/max-verstappen-cherishes-a-special-f1-chapter`,
   `verstappen.com/tickets/dutch-gp`. This is a genuine Aug 2-dated
   in-window item, not a recycled March statement — the kind of
   beat-specific ownership-color story that's easy to miss if you only
   search "Horner Alpine" and "factory shutdown" as your headlines.

**Rank count during shutdown is editorial-discretion, not fixed.** The
v4 task brief (2026-08-08 edition) explicitly allowed "2-3 stories
instead of 4" because the masthead already carried a separate
calendar-reminder slot. When the brief permits a 3-rank edition, ship 3
real in-window statements and drop the calendar reminder; do not pad
to hit 4. Confirmed v4 2026-08-08: Vowles defiant Williams message
(rank 1), Bottas gravel cycling during shutdown (rank 2), Coulthard
FIA "road-roller" critique (rank 3) — all three real in-window, no
calendar slot needed. When the brief mandates 4 ranks, the pattern is:
lead with the highest-stakes in-window statement, then two more
in-window statements, then a one-paragraph rank-4 that names the next
race's date, sprint schedule, and what to watch when the cars return.
Confirmed in v4 Issue 8 (2026-08-03): lead Wolff-Horner, then McNish,
then Newey, then Zandvoort calendar reminder. Four ranks is the
ceiling; three is the floor; respect the brief.

## `fom-website/<year>/<Team>/<File>.jpg` — editorial-upload JPEG route

The existing trackside-images JPEG recipes above cover race-weekend
photos (`trackside-images/2026/F1_Grand_Prix_of_<X>/<ID>.jpg`). The
**editorial-upload path** `fom-website/<year>/<Team>/<File>.jpg` also
serves real JPEG bytes when the file extension in the URL is `.jpg`.
Confirmed in the F1 v4 2026-08-08 leaf:

```text
# Williams / Vowles exclusive-interview hero — works
https://media.formula1.com/image/upload/c_lfill,w_3392/q_auto/v1740000001/fom-website/2026/Williams/EXCLUSIVEINTERVIEWADHOC%20FEATURE%20V3.jpg
# → 1920x1080 JPEG, 216 KB, file reports "JPEG image data, JFIF standard 1.01"

# Cadillac / Bottas seat-fit photo — works
https://media.formula1.com/image/upload/c_lfill,w_3392/q_auto/v1740000001/fom-website/2025/Cadillac%20(GM)/Untitled-10.jpg
# → 3392x2261 JPEG, 298 KB

# Williams Hungary video page hero — works
https://media.formula1.com/image/upload/c_lfill,w_3392/q_auto/v1740000001/fom-website/2026/Hungary/GettyImages-2287565472.jpg
```

The `.jpg` extension is the JPEG transform (same as the existing
trackside-images rule); `.webp` returns WebP and fails `file`'s check.
URL-encoded detail:

- `Cadillac (GM)` (parentheses around GM) is mandatory in the URL —
  `Cadillac GM` (no parens) returns a 404.
- `%20` (URL-encoded space) is mandatory between multi-word filename
  tokens. `EXCLUSIVEINTERVIEWADHOC FEATURE V3.jpg` → the spaces
  become `%20` in the URL.

**Credit convention for `fom-website/<Team>/` uploads:** these are
editorial hero images, not trackside photography. The credits differ
from the trackside-images pool:

- `fom-website/2026/Williams/...` — typically team-specific label
  (`Formula 1 / Atlassian Williams F1 Team`) or `Mark Thompson /
  Getty Images`. Scan the article's `__next_f.push()` JSON for
  `Photo by <name>` patterns; if none surface, the photo is a team
  handout and credit as `<Team> handout` or `Formula 1` editorial.
- `fom-website/2025/Cadillac (GM)/...` — typically a team press-kit
  image (`Formula 1 / Cadillac F1 Team`) when from the announcement
  gallery, or `Kym Illman / Getty Images` for in-paddock shots.

Do not blanket-default `fom-website/` uploads to
`Formula 1 / Formula 1 via Getty Images` (the trackside-images
default) — the credits are team-specific.

## Motorsport.com article IDs are not date-encoded

`motorsport.com/f1/news/<slug>/<numeric-id>/` article IDs
(e.g. `/10843241/`, `/10844506/`, `/10843458/`) are sequential
internal counters, NOT date-encoded. Confirmed v4 2026-08-08: the
Williams Vowles article ID is `/10843241/`, the Bottas cycling article
ID is `/10844506/`, and the Coulthard FIA-critique article ID is
`/10843458/`. Three articles published within 48 hours (Aug 3-5) have
IDs that are non-monotonic in either direction — `/10844506` is larger
than `/10843458` but was published later.

**Implication for `published_at`:** the umbrella's general pitfall
("parse the URL slug for `/YYYY/MM/DD/`") does not apply here. For
Motorsport.com URLs, `published_at` has to be inferred from (a) the
article body's social-media timestamp or visible `<time>` tag, or
(b) the page's `og:` / `article:published_time` meta via `web_extract`.
Use the article body as ground truth and treat any URL-derived date as
unreliable.

## Zandvoort preview source pack (2026 cycle)

Confirmed working sources for the pre-Dutch-GP rank-4 calendar reminder
*and* the post-Hungary "next race" framing. Each item is a primary source
the leaf can cite verbatim:

- **Tyre compounds** — `https://press.pirelli.com/tyre-compounds-selected-for-zandvoort-monza-and-madrid`.
  Pirelli publishes the per-circuit C1-C5 selection in a single press
  release two-to-three weeks before the round. The release carries the
  rationale paragraph ("medium range, comprising the C2, C3 and C4, will
  be used for the Dutch and Spanish Grands Prix"). Reliable for `published_at`
  (timestamped Europe/Amsterdam on the release page) and for the
  surface-temperature note circuit-by-circuit. The 2026 Zandvoort
  selection was C2/C3/C4 — same as 2025 — driven by the
  low-to-medium-speed corners and the sandy North Sea surface.
- **Weekend schedule & sprint structure** — `https://www.formula1.com/en/racing/2026/netherlands`
  on `formula1.com` lists FP1, Sprint Qualifying, Sprint, Qualifying,
  and GP local times. Zandvoort 2026 is the **first-ever F1 Sprint on
  Dutch soil** (Friday 21 Aug FP1 + Sprint Qualifying; Saturday 22 Aug
  Sprint + GP Qualifying; Sunday 23 Aug GP at 15:00 local). The
  hero-image on the race page is reliable.
- **Weather forecast** — two reliable sources:
  - `weatherapi.com` returns a clean JSON for `Zandvoort,North Holland,Netherlands`
    with current temp, wind, humidity, chance_of_rain. Use for the
    *current* Zandvoort observation (a "right now" reading is
    fine for context, not for race-day forecast).
  - `gpdestinations.com/netherlands-f1-travel-guide` carries the
    historical climate section: "Expect temperatures of 12C at night,
    rising to 18C during the day. Strong, cold winds are also common."
    Cite as the long-range climatology baseline.
  - `dutchtickets.gp/en/weather-forecast` is the canonical
    "race-weekend weather" page — it explicitly says "It's too early
    to look at the weather forecast" until 5-7 days pre-race, then
    re-checks daily. Bookmark the URL but only quote it in the final
    48-hour window.
- **The farewell framing** — when the round is the last F1 race at a
  venue (Zandvoort 2026 being the canonical example), the on-record
  ownership / "final chapter" quotes from the local promoter are the
  under-cited lead. Primary: `visitzandvoort.com/circuit/formula-1`
  (event times), `dutchgp.com/en` (ticketing / closing concert),
  `f1-fansite.com/f1-news/prince-sells-zandvoort-stake-before-final-dutch-gp`
  (owner stake-sale story from 2026-08-02).

Recipe: in the rank-4 calendar-anchor body, layer these as three
short paragraphs — (1) tyre compounds + the Pirelli rationale quote,
(2) weekend schedule with the Sprint standing out, (3) the climatology
range + whether forecasts show showers. Do not attempt a race-day
weather forecast for Zandvoort this far from the race; cite the
climatology and the "showers possible" framing instead.

## Renault articles-of-association lock-in clause (Alpine stake)

The Horner-vs-Wolff Alpine stake story has a contractual speed limit
that is easy to miss. The 2026-cycle source is the **13 September 2023**
Alpine articles-of-association clause (not the shareholder agreement,
the **articles**): Otro Capital cannot sell its 24% stake for three
years from the date of adoption, i.e. **not until 13 September 2026**.
Even after that date, any sale requires formal approval from Renault
Group (the 76% majority owner). The pin: Planet F1 obtained the
document and reported the clause in August 2026; the Briatore
comments at the January 2026 Barcelona launch established the
broader appetite.

When the lead candidate is "Horner bid for Alpine," check whether the
article you're citing carries the Renault veto / September 2026
expiry. Without it, the story reads as "any day now" when the
realistic window is "after Baku." Sources to triangulate the lock-in:

- `reuters.com/sports/formula1/horner-interested-otros-alpine-stake-says-briatore-2026-01-23`
  — Briatore's January 2026 acknowledgement at the Barcelona launch.
- `racingnews365.com/christian-horner-and-toto-wolff-raising-the-stakes-in-latest-f1-scrap`
  — the "Otro is in no rush" framing, plus the valuation band
  (£1.5bn-£1.86bn for the team → ~£448m for the 24% stake).
- `blackbookmotorsport.com/news/f1-alpine-toto-wolff-christian-horner-alpine-stake-march-2026`
  — Wolff's January 2025 sale of 15% of his personal Mercedes stake
  ($300m liquid) gives him the cash to bid corporate-side; reporting
  names him as the consortium leader.

Trap: editorial stories that lead with "Horner POISED to buy Alpine"
without the clause context are not wrong, but they mislead the reader
on timing. A precise version: "Horner has external backing for the
consortium, but the Renault veto and the 13 September
articles-of-association lock-in mean the closing window is the
Azerbaijan GP week, not the August break."

## The "FIA's 2026 rules aren't fixable" framing (Tombazis governance)

The most-credible shutdown-week regulatory story of the 2026 cycle is
the **FIA single-seater director's confession** that the 2026
energy-management rules were never going to be revised mid-cycle. The
canonical source: `motorsport.com/f1/news/fia-opens-up-on-f1s-2026-rule-issues-heres-why-it-couldnt-intervene-sooner/10843811`,
Nikolas Tombazis interviewed by Motorsport.com in early August 2026.

The shape of the disclosure is what matters:

- Tombazis calls it "a bit of a shame" the 2027/2028 re-balance
  (toward 60-40 ICE/electric) couldn't have been agreed two years
  earlier. That is on the record.
- The mechanism: a 2022 governance agreement signed by the FIA, FOM,
  and PU manufacturers (Mercedes, Ferrari, Renault/Alpine, Honda,
  Audi, Red Bull Powertrains) requires a supermajority of four
  manufacturers to approve any mid-cycle PU regulation change. Four
  did not agree during the years Verstappen and Piastri were
  publicly warning about the new rules.
- The decision-space: Tombazis confirms the FIA knew about the
  energy-management issues from simulator work as early as 2022, but
  "we were a bit too far down the line there to change things and
  some people had committed too much resource."

Lead angles for this story (in descending strength):

1. "The rules weren't fixable — and the people who knew include the
   people who wrote them." Governance as the blocker, not technical
   inability.
2. "Verstappen called it in 2023. The fix lands in 2027." The
   long time-to-revise path.
3. "The 2022 governance agreement is the smoking gun." The mechanism
   that locked the rules.

Confirm Tombazis attribution by reading the article body (not the
snippet) — the Motorsport.com mid-roll banners obscure the
attribution. The article is paywall-lite; the free preview usually
carries the Tombazis quotes.

## Don't re-lead yesterday's shutdown story

When the F1 leaf runs on consecutive shutdown days (Aug 3, Aug 4,
Aug 5, Aug 6...), the headline-grab stories from Tuesday
(Wolff-Horner "broken glass") and Wednesday (McNish-Audi 2027
freeze) are likely already covered. The temptation trap: re-rank
the same Wolff-Horner quote at rank 1 because it's the search
result with the most words.

The discipline: before writing rank 1, **read the previous day's
beat JSON** (`f1_beat_<prev-date>.json` in the working dir). If the
top-line statement is a Wolff / Horner / McNish / Newey / Vowles
quote, you have to find a new lead by hunting for in-window
statements that are NOT in the previous file. Confirmed working
hunt for shutdown-day-after-shutdown-day:

- **Filming-day engine / aero confirmations** (the post-Hungary
  filming day is the canonical slot — Honda, Aston Martin, Williams
  all run new parts on the Wednesday after the GP).
- **Contractual-mechanism deep dives** (the Renault articles-of-association
  clause, the Wolff-MERCEDES-as-corporate-bidder framing, the Audi
  2026-vs-2027 budget split).
- **Federation-level confessions** (the Tombazis governance piece).
- **Calendar-anchor rank-4** with Pirelli + weather + sprint structure
  even when the first three ranks are strong.

The hard pattern: on a four-day shutdown stretch, the lead must rotate
daily. A leaf that lands "Wolff / Horner" three shutdown days in a row
fails the orchestrator's continuity check even though the JSON
parses cleanly.

## Image-download pattern when an existing asset is an under-extension WebP

If the previous day's beat already has `f1_<slug>.webp` in the asset
dir, but downstream verification insists on real JPEG bytes (the
"file: JPEG image data" check), the leaf should NOT rename the file
client-side. The clean pattern: download a fresh JPEG variant of the
same Cloudinary `public_id` by using the `.jpg` extension in the URL.

Confirmed in the F1 v4 leaf (2026-08-05): the Aug 4 asset
`f1_hungary_norris_winner.webp` was the WebP-retitled version of
the Norris trophy photo. Re-downloading from
`https://media.formula1.com/image/upload/c_fit,w_3200,h_1800/q_auto/v1740000001/trackside-images/2026/F1_Grand_Prix_of_Hungary/2287733754.jpg`
(the `.jpg` extension in the URL is the JPEG transform) returned a
199 KB 2700x1800 JPEG that `file` confirmed as `JPEG image data,
JFIF standard 1.01`. vision-analyze confirmed it is Norris in
papaya McLaren suit (defending champion, "1" on the cap, pointing
up). The two files coexist in the asset dir; the new file is the
canonical `image.local_path` for today's beat.

Same recipe works for any Sutton Images / LAT Images Cloudinary
ID — the URL extension controls the format. Don't waste time
on `q_auto` rewrites; the `.jpg` extension is the strong signal.

## `content/dam/fom-website/sutton/<year>/<GP>/` — the historical/archival path

Distinct from the two documented FOM Cloudinary paths
(`trackside-images/<year>/<GP>/<id>.<ext>` for race-weekend trackside,
`fom-website/<year>/<Team>/<file>.<ext>` for editorial uploads), there
is a **third** FOM Cloudinary path used for historical / archival /
non-race-weekend photos:

```text
https://media.formula1.com/image/upload/<transforms>/v1740000001/content/dam/fom-website/sutton/<year>/<Country>/<Day-or-Session>/<hash>.<ext>
```

The path segment is the Sutton Images archive bucket (Mark Sutton
is the dominant F1 Getty pool photographer; the bucket is named
after him). Photos here are typically 2021-2024 era, used by
formula1.com for lifestyle/feature articles, anniversary pieces, and
historic-circuit framing (Zandvoort, Monza, Spa).

**Working example (F1 v4 2026-08-12 IST lead image):**

```text
https://media.formula1.com/image/upload/ar_16:9,c_fill/c_lfill,w_3392/q_auto/v1740000001/content/dam/fom-website/sutton/2021/Netherlands/Sunday/1338467279.webp
```

This URL has a literal `.webp` extension at the end — Cloudinary
returns WebP bytes regardless of the `q_auto` transform. `file`
reports `RIFF (little-endian) data, Web/P image` which fails the
umbrella's "JPEG image data" check. **Unlike the `trackside-images/`
bucket, the legacy `content/dam/` path does NOT honour the
extension-swap trick** — appending `.jpg` to the URL still returns
WebP bytes.

**Confirmed-working conversion recipe for content/dam/ URLs:**

```bash
# Download the literal .webp
curl -sSL -A "Mozilla/5.0" -o /tmp/img.webp \
  "https://media.formula1.com/image/upload/.../sutton/2021/Netherlands/Sunday/1338467279.webp"

# Convert with ffmpeg (single-frame decode, ~0.1s wall clock)
ffmpeg -y -i /tmp/img.webp /tmp/img.jpg 2>&1 | tail -2
#   → frame=    1 fps=0.0 q=11.8 Lsize=N/A (single-frame decode, success)

# Verify, then move
file --brief --mime-type /tmp/img.jpg
#   → "image/jpeg"  ✓
mv /tmp/img.jpg <asset_dir>/f1_<slug>.jpg
```

Confirmed F1 v4 2026-08-12: 264 KB JPEG, 3000×2000 px, vision-verified
as Verstappen in Red Bull race suit holding a Dutch tricolor at
Zandvoort (the iconic 2021 Dutch GP win). Credit verbatim from the
formula1.com article body where the URL was sourced:
`Mark Sutton / Sutton Images / Formula 1 via Getty`.

**Discovery recipe** for content/dam/ URLs: scrape the article HTML
of any `formula1.com/en/latest/article/<slug>.<hash>` page that
references a historical / anniversary / lifestyle topic (e.g. the
"Verstappen and the Orange Army" feature at
`https://www.formula1.com/en/latest/article/how-max-verstappen-captivated-and-transformed-f1-fan-culture-in-the.AFysFZC9m6zQY0T0mqUbo`).
Then
`grep -oE 'https://media\.formula1\.com/image/upload/[^"]+content/dam/[^"]+\.(webp|jpg|png)'`
extracts the archival URLs. The article's `__next_f.push()` JSON
usually carries the `Photo by <name>/<agency>` credit inline.

## Vision-verify the lead image BEFORE composing JSON

The umbrella's general pitfall (vision-verify every downloaded image
at the orchestrator, even when the leaf says it's verified) applies
on the leaf side too, but the F1 shutdown-week case has an extra
twist: **the most on-message historic image is the most likely to
fail the topic-fit check.** Confirmed F1 v4 2026-08-12: a
Zandvoort-2021 Verstappen image is on-message for "Verstappen
Zandvoort farewell + exit clause" only if the lead story actually
centres Verstappen; if the lead is the Sainz-Audi shutdown or the
Pirelli Zandvoort preview, the same image becomes off-topic art
and should be moved to a Verstappen-led rank (or replaced).

**Hard rule for the lead image:** after `curl` + `file` + `stat`
pass, run `vision_analyze` on the local JPEG with the question
"What is shown in this image? Is it a photo of <lead subject>?"
The acceptable answer shape is "<person/team/thing>, doing <action>,
in <setting>." If the answer names a different subject (e.g. the
image shows a Ferrari driver when the lead is about Red Bull), drop
the image and pick a different hero before writing the JSON.

The 30-second vision check costs less than the orchestrator
flagging a topic-mismatched lead and re-issuing the beat. Treat
`vision_analyze` on the lead as a required verification step, not
an optional one.

## Body length trap on shutdown beats

Shutdown beats tempt the writer to pad. Each of the four real
in-window statements in Issue 8 was a short press comment — 1-3
quotes max — and the editorial temptation is to paraphrase and
contextualise until the body balloons past 230 words. **Auto-count
bodies before emit** (see umbrella rule: 130–200 word band, 150
target). For shutdown ranks, aim for the lower end of the band
(140-170 words) — the news is tight, the lead phrase carries the
weight, and the second paragraph can name the next-race trigger
instead of restating context the dek already implies.

## F1.com Cloudinary URL — confirmed JPEG transform

Two reliable JPEG transforms for `trackside-images/2026/F1_Grand_Prix_of_<X>/<ID>`:

```text
# Variant A: simple fit
https://media.formula1.com/image/upload/c_fit,w_3200,h_1800/q_auto/v1740000001/trackside-images/2026/F1_Grand_Prix_of_Hungary/<ID>.jpg

# Variant B: lfill (preserves aspect, letterboxes)
https://media.formula1.com/image/upload/c_lfill,w_3392/f_jpg/q_auto/v1740000001/trackside-images/2026/F1_Grand_Prix_of_Hungary/<ID>.jpg
```

**Key finding:** writing the file extension as `.jpg` (not `.webp`)
in the Cloudinary URL is itself the JPEG transform — Cloudinary
honours the requested format and returns real JPEG bytes, not a
WebP-with-`.jpg`-name. `file <path>` then reports
`JPEG image data, JFIF standard 1.01, ..., 2700x1800`, which is
the spec the image-verification recipe expects. **Don't rely on
URL swapping `q_auto`→`f_jpg,q_auto` — the extension is simpler
and equally reliable.** Confirmed 2026-08-03: variant A returned a
199 KB 2700x1800 JPEG for Hungary Sunday `2287733754`, vision-checked
as Lando Norris in papaya McLaren suit (defending champion race #1).

**The .webp URLs return WebP** even though the byte-stream is
visually identical. `file` reports `RIFF (little-endian) data, Web/P image`
which **does not satisfy the umbrella's "JPEG image data" check**.
Always use the `.jpg` extension in the URL — never `.webp` — when
the downstream verifier is strict about format.

## Credits for trackside-images — confirmed 2026 photographer pool

The umbrella default of `Formula 1 / Formula 1 via Getty Images` is
correct but imprecise. The actual 2026 Hungary GP photographer pool
on Formula1.com's `trackside-images/2026/F1_Grand_Prix_of_Hungary/`
Cloudinary directory resolves to:

- `Sam Bagnall / Sutton Images` — podium and trophy-shot IDs (e.g.
  `2287733754` Norris trophy). **This is the most common credit for
  Sunday-race podium photos.**
- `Alastair Staley / LAT Images via Getty` — on-track action shots
  (e.g. `2287727846` Norris on-track).
- `Mark Thompson / Getty Images` — paddock and portrait shots.

When you download a `trackside-images/` hero and can't pin a specific
photographer string from `__next_f.push()`, **default to
`Sam Bagnall / Sutton Images via Formula 1`** for podium/trophy
photos and `Alastair Staley / LAT Images via Getty` for on-track
action. Do not emit `Formula 1 / Formula 1 via Getty Images` as a
generic — it's accurate but the renderer prefers the named
photographer when one is known.

For `fom-website/` uploads the existing rules still apply (Mark
Thompson / team handout / editorial label — scan `__next_f.push()`
for `Photo by ...` patterns).

## BBC iChef CDN — for F1 / motorsport images on BBC Sport articles

When Formula1.com Cloudinary is unavailable or doesn't carry the exact
shot you need (e.g. team principals together at a press conference,
portraits from a non-race-weekend event), BBC Sport's iChef CDN serves
real editorial JPEGs at production quality. Confirmed working pattern
in the F1 v4 leaf (2026-08-04):

```text
https://ichef.bbci.co.uk/ace/standard/3840/cpsprodpb/<slug>/live/<hash>.jpg
```

Download with:

```bash
curl -sSL -A "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36" \
  -H "Referer: https://www.bbc.com/" \
  -o f1_<slug>.jpg \
  "https://ichef.bbci.co.uk/ace/standard/3840/cpsprodpb/<slug>/live/<hash>.jpg"
```

The `Referer: https://www.bbc.com/` header is mandatory — without it
iChef returns 403 and the file lands as HTML/text. With the header, it
serves a real progressive JPEG at 3840x2159, typically 300-700 KB.
The `<hash>` is the unique image identifier in BBC's asset pipeline;
discover it by extracting `og:image` from the article's HTML (or any
`<img>` with a `cpsprodpb` URL). Example: the Horner + Briatore
shot from the BBC article on Horner's Alpine interest was at
`https://ichef.bbci.co.uk/ace/standard/3840/cpsprodpb/ebd9/live/a63bcfc0-fa0a-11f0-b8ce-0fa566ac6f07.jpg`
(621 KB JPEG, `file` confirmed JPEG, vision-confirmed as Horner and
Briatore at FIA-branded microphones).

Credit format for BBC-hosted F1 images: the photographer string is
usually embedded in the BBC caption ("Christian Horner (left) gets on
well with Alpine executive adviser Flavio Briatore (right)" / "Photo
by Mark Thompson / Getty Images" or similar). Use the credited
photographer string verbatim when available; fall back to
"Getty Images / Formula 1" when the caption only describes the
subjects.

## Verification gates before emit (F1-specific)

```bash
# After downloading the JPEG to the asset path:
file <path>                              # MUST: "JPEG image data"
ls -l <path>                             # MUST: > 20 KB, expect 100-500 KB at w_3200
python3 -c "from PIL import Image; im = Image.open('<path>'); print(im.size, im.format)"  # MUST: format='JPEG', size >= (1920, 1080)
```

The third check (PIL format string) is stricter than `file` and
catches the WebP-with-`.jpg`-name trap directly. If `im.format`
is `'WEBP'` not `'JPEG'`, re-download with the `.jpg` extension in
the URL — do not rename the file client-side.

## Image-discovery surface for shutdown-week beats

During the FIA-mandated factory shutdown (Aug 1-20 of every year),
the F1 leaf needs track-layout / announcement / driver-portrait
imagery that isn't race-weekend trackside. The reliable sources,
beyond the existing `trackside-images/<year>/<GP>/<ID>.jpg` route:

1. **Track layout diagrams** for the rank-4 calendar reminder.
   The canonical URL pattern on `media.formula1.com` is:

   ```text
   https://media.formula1.com/image/upload/c_lfill,w_3392/q_auto/v1740000001/common/f1/2026/track/2026track<gpname>detailed.webp
   ```

   Replace `<gpname>` with the lowercase gp slug (e.g.
   `zandvoort`, `monza`, `madrid`, `imola`, `suzuka`). The download
   returns a WebP (the `.webp` extension is literal). `file` reports
   `RIFF (little-endian) data, Web/P image` — fails the umbrella's
   "JPEG image data" check, so convert with ffmpeg:

   ```bash
   curl -sL -A "Mozilla/5.0" -o /tmp/track.webp \
     "https://media.formula1.com/image/upload/c_lfill,w_3392/q_auto/v1740000001/common/f1/2026/track/2026trackzandvoortdetailed.webp"
   ffmpeg -y -i /tmp/track.webp /tmp/track.jpeg
   mv /tmp/track.jpeg <asset_dir>/f1_zandvoort_track.jpg
   file <asset_dir>/f1_zandvoort_track.jpg   # MUST: "JPEG image data"
   ```

   Confirmed in F1 v4 (2026-08-09) for Zandvoort: 122 KB JPEG,
   vision-confirmed as the official 2026 track layout (14 numbered
   corners, blue/yellow/red sector markers, "SPEED TRAP" label).
   Credit as `Formula 1 / Dromo Circuit Design` — the layout is
   Dromo's work, commissioned by F1.

2. **Editorial-upload `.png` URLs in `fom-website/<year>/<Team>/`**
   follow a different convention than the existing `.jpg` rule. The
   canonical team-announcement pattern is:

   ```text
   https://media.formula1.com/image/upload/f_auto/q_auto/v<version>/fom-website/<year>/<Team>/16x9%20single%20image%20(<N>).png
   ```

   These URLs return a **real JPEG** at high resolution (~300-500 KB
   at full size) when the URL ends in `.png` — Cloudinary's `f_auto`
   transform auto-detects format and returns the best match. The
   `.png` extension in the URL is just a name; the actual bytes are
   JPEG when the transform is `f_auto,q_auto`. Confirmed in F1 v4
   (2026-08-09) for the Cadillac F1 reveal:
   `https://media.formula1.com/image/upload/f_auto/q_auto/v1768315260/fom-website/2026/Cadillac/16x9%20single%20image%20(16).png`
   → 126 KB JPEG, vision-confirmed as a black Cadillac F1 car
   render on a dark gradient background. `file` reports
   `JPEG image data, JFIF standard 1.01`. URL-encoded space (`%20`)
   between filename tokens is mandatory.

3. **The empty-file CDN response trap.** Two distinct failure modes
   on `media.formula1.com`:

   - **HTML 404** (the documented case): `file` reports
     `HTML document`, body is the standard 404 page. The fix in
     the existing playbook is correct.
   - **Empty 0-byte body** (`inode/x-empty`): `file` reports
     `inode/x-empty` or `empty`, `stat -c '%s'` returns `0`. The
     cause: a Cloudinary URL with a path that exists but the asset
     has been removed or moved. The trap: `file --brief --mime-type`
     does NOT flag this as a problem (it returns `inode/x-empty` or
     `application/x-empty` cleanly), but the umbrella's
     "20 KB minimum" check catches it. **Always run `stat -c '%s'`
     in addition to `file` and `file --brief --mime-type`.** If
     `stat` returns 0, the URL is dead — pick a different URL or
     skip the image.

   Confirmed in F1 v4 (2026-08-09): two of four initial Cloudinary
   URLs returned `inode/x-empty` 0-byte files. The fix path was
   to (a) `curl -sL` the formula1.com article HTML directly,
   (b) `grep -oE 'https://media.formula1.com/[^"]+\.(webp|jpg|png)'`
     to extract every media URL mentioned on the page,
   (c) try the freshly-discovered URLs.

4. **The "image-name ≠ image-content" trap.** A URL with the
   path segment `Netherlands` (or any country/circuit name) in
   the filename does NOT guarantee the image is the circuit
   aerial or layout. The 2018-redesign-assets header images
   (`fom-website/manual/Misc/2021manual/2021DutchGPManuals/...`)
   can be track aerials, but the asset-bucket header image at
   `https://media.formula1.com/image/upload/c_lfill,w_3392/q_auto/v1740000001/content/dam/fom-website/2018-redesign-assets/Racehub%20header%20images%2016x9/Netherlands.webp`
   is a generic Red Bull podium shot — not a Zandvoort aerial.
   **Always vision-verify the downloaded image BEFORE citing it
   in the JSON.** A URL whose path matches the topic is necessary
   but not sufficient. The vision check (existing playbook pitfall)
   is the only reliable guard against this trap. Confirmed
   2026-08-09: the "Netherlands" header image downloaded as a
   Red Bull podium champagne celebration, not a Zandvoort circuit
   shot — caught by vision-analyze and replaced with the
   `/common/f1/2026/track/2026trackzandvoortdetailed.webp` layout
   diagram.

## Motorsport.com CDN — WebP trap + ffmpeg conversion recipe

When Formula1.com Cloudinary or BBC iChef isn't carrying the exact
shot you need (e.g. mid-week shutdown portraits, principal-interview
headshots, candid paddock), motorsport.com's own CDN is the
fallback. The URL pattern is:

```text
https://cdn-<N>.motorsport.com/images/amp/<hash>/s1000/<filename>.webp
```

where `<N>` is 1-9 (load-balanced CDN nodes — `cdn-9` verified
2026-08-06) and `<hash>` is the article's image asset ID (visible
in the article HTML's `<img>` tags and the `og:image` meta).

**Trap:** the `.webp` extension is *literal* — this CDN returns
WebP bytes, not a WebP-renamed-as-JPEG. `file` reports
`RIFF (little-endian) data, Web/P image, VP8 encoding, 1000x666, YUV color`
which **fails the umbrella's "JPEG image data" check**. Confirmed
in F1 v4 (2026-08-06) for the Sergio Perez Cadillac shot at
`https://cdn-9.motorsport.com/images/amp/27NoaLv0/s1000/sergio-perez-cadillac-racing.webp`
(46 KB WebP, 1000x666).

**Fix:** convert to JPEG with `ffmpeg` after download:

```bash
curl -sL -A "Mozilla/5.0" -o /tmp/img.webp "https://cdn-9.motorsport.com/images/amp/27NoaLv0/s1000/sergio-perez-cadillac-racing.webp"
ffmpeg -y -i /tmp/img.webp /tmp/img.jpeg        # 56 KB JPEG, file reports JPEG
mv /tmp/img.jpeg <asset_dir>/f1_<slug>.jpg
file <asset_dir>/f1_<slug>.jpg                  # MUST: "JPEG image data"
```

`ffmpeg -y -i` reads WebP natively and writes a baseline JPEG.
The conversion is lossless enough for editorial use (1000x666
source → ~57 KB JPEG). If `ffmpeg` is unavailable, `convert` from
ImageMagick works equivalently: `convert /tmp/img.webp /tmp/img.jpg`.

**Credit string for motorsport.com CDN photos:** the photographer
is in the article body, never in the image URL. The standard caption
format is `"Photo by: <Name> / <Agency> via Getty"` — common shapes
in the 2026 cycle:

- `"Photo by: Sam Bloxham / LAT Images via Getty"` — paddock, garage
- `"Photo by: Alastair Staley / LAT Images via Getty"` — on-track
- `"Photo by: Andy Hone / LAT Images via Getty"` — portraits
- `"Photo by: Guido De Bortoli / LAT Images via Getty"` — garage close-ups
- `"Photo by: Mark Sutton / Sutton Images"` — trophy / podium
- `"Photo by: Rudy Carezzevoli / Getty Images"` — Racing Bulls

The article body usually has the caption visible near the top of
the page (in the `<figcaption>` of the lead image, or in the
"Photo by: ..." line directly under the article title). `web_extract`
returns this in the page metadata. Credit as `"<Name> / <Agency> via Getty"`
verbatim. **Do not credit these as "Motorsport.com"** — Motorsport is
the distributor, the photographer is the rights-holder.

## GrandPrix247 archive — parallel shutdown-beat discovery surface

For shutdown weeks (Aug 1-20 of every year), **GrandPrix247**
(`https://www.grandprix247.com/formula-1-news`) is a parallel
discovery surface that often surfaces MORE in-window stories than
the Motorsport.com archive. Confirmed 2026-08-10: the GP247 front
page surfaced **four** in-window shutdown-day stories dated 6-9
August 2026:

- **Silly Season Sunday: Sainz to Audi, Pérez to Williams** (9 Aug
  2026, 13:07 UTC, citing Auto Sport Web Japan / Motorsport.com) —
  the lead in-window story, escalating a 7 Aug Motorsport.com report
  that Pérez's management has opened 2027 talks with Williams.
- **Liberty Media Q2: $127M team payment drop, 38% revenue fall** (6
  Aug 2026, 21:51 UTC, citing Liberty's 8-K) — the canonical
  shutdown-week "war and money" story, tying Bahrain→Sepang to the
  Q2 results call.
- **Domenicali confirms 2027 sprint expansion; Briatore wants 24**
  (7 Aug 2026, 11:22 UTC, citing The Race interview) — the
  calendar-anchor piece for the second week of shutdown.
- **Andrea Stella: Norris-Piastri harmony intact** (9 Aug 2026,
  07:30 UTC, citing McLaren media) — McLaren's mid-season statement
  on the "disruptive" 2026 regs.

GP247's archive page lists every article with a **visible publish
date** in the byline (e.g. `Sunday, 09 August 2026 at 13:07`) — no
need to parse JSON-LD. The article URLs follow the pattern
`https://www.grandprix247.com/formula-1-news/<slug>`. Each article
image's lower-right corner carries a `Photo by: <credit>` watermark
that you can read off directly without scraping `__next_f.push()`
JSON.

**Why this matters:** the F1 leaf historically defaulted to the
Motorsport.com archive and `web_search` aggregations, which on
shutdown days return a flood of stale July Hungary GP results. The
GP247 archive cuts through that noise with fresh, dated stories
that are explicitly in-window. Pair the GP247 archive with the
Motorsport.com archive for shutdown-beat discovery — they
complement, not duplicate.

**Auto Sport Web (as-web.jp) is a credible secondary source for
Japanese paddock reporting** — the Pérez-Williams story originated
at `https://www.as-web.jp/f1/1340396` (verified 2026-08-10). The
article body carries the 2026-08-06/07 publish date and the
original Japanese text. Title pattern: `<English slug> | ニュース |
autosport web`. When a 2026 F1 driver-market rumour arrives via
Auto Sport Web first, treat the Japanese URL as the primary source
and the English-desk pickup (Motorsport.com / GP247 / RacingNews365)
as the secondary.

## GrandPrix247 image CDN — direct download, real JPEG

GP247 article images are served via a **two-layer CDN** — the
visible URL is a testifier resize wrapper, but the origin is a
DigitalOcean Spaces bucket. Both layers serve real JPEG bytes at
the size you request. Confirmed working pattern in F1 v4
(2026-08-10):

```text
# Visible (article HTML) URL — testifier resize wrapper:
https://r.testifier.nl/Acbs8526SDKI/resizing_type:fit/width:1920/height:1280/plain/https://s3-newsifier.ams3.digitaloceanspaces.com/grandprix247.com/images/2026-08/perez-sainz-silly-season-6a785cce4b6d6.jpg

# Origin (direct S3) URL — bypasses the wrapper:
https://s3-newsifier.ams3.digitaloceanspaces.com/grandprix247.com/images/2026-08/perez-sainz-silly-season-6a785cce4b6d6.jpg
```

**Both URLs return real JPEG bytes** with `Content-Type: image/jpeg`
on a plain `curl -sL -A "Mozilla/5.0"` request. No auth, no cookies,
no referer. The S3 origin returns slightly smaller files (no
wrapper overhead) but the byte-stream is identical.

**Filename pattern:** `images/<YYYY-MM>/<descriptive-slug>-<hash>.jpg`
where the hash is a random alphanumeric suffix. The hash is unique
per upload and is **not** derivable from the article slug — you must
read it off the article's `<img>` tag or `og:image` meta. Don't try
to construct GP247 image URLs from the article slug alone.

**Verified 2026-08-10 batch (all real JPEG, 75-130 KB, vision-verified
for topic):**

- `f1_silly_season_perez_sainz.jpg` — Pérez in Cadillac Tommy
  Hilfiger kit + a Williams driver, crowd walk — perfect for the
  2027 driver-market story.
- `f1_financial_results_middle_east.jpg` — Liberty Media / Reuters
  newsroom composite — fine for the Q2 financial-results story.
- `f1_domenicali_sprint_2027.jpg` — Verstappen #3 leads Russell
  Mercedes on track (Sutton Images for Red Bull Content Pool) —
  good for the sprint-expansion piece.
- `f1_stella_piagpi_hungary.jpg` — McLaren team celebrating Norris
  "P1" pole with a yellow placard — perfect for the Stella piece.

**Credit format:** the article body's `Photo by: <name>` line is
the canonical credit. The `__next_f`-style hidden credit doesn't
exist on GP247 — what you see on the page is what you ship. When
the credit is missing, default to `Photo: F1 Media (via GrandPrix247)`
or `Liberty Media (via GrandPrix247)` depending on the subject.

## Motorsport.com article-page User-Agent trap

**The default `Mozilla/5.0` User-Agent on `curl` returns a 919-byte
HTML block from `motorsport.com` article URLs — NOT the article
body.** Confirmed 2026-08-10: a clean `curl -L -A "Mozilla/5.0" "https://www.motorsport.com/f1/news/<slug>/<id>/"`
returns a Cloudflare-style 919-byte HTML body with `<TITLE>ERROR: The
request could not be satisfied</TITLE>` and no article content. The
file size signature (under 1 KB, HTML body, no JSON-LD) is the
diagnostic.

**Fix:** use the desktop-Chrome User-Agent string instead:

```bash
curl -sL -A "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36" \
  "https://www.motorsport.com/f1/news/<slug>/<id>/" -o /tmp/article.html
```

Verified 2026-08-10: the Mac-UA request returns a ~1 MB HTML body
with the full article text, the `datePublished` JSON-LD, the
`og:image` hero, and the `<link rel="alternate">` siblings (which
expose the per-edition URL — `motorsport.com/es/`, `motorsport.com/jp/`,
etc., for cross-language verification).

**Alternative path (preferred for the beat):** skip the curl
chicken-and-egg entirely and use `web_extract(char_limit=10000)` on
the article URL. `web_extract` is the safe path; it returned a
clean 22 KB article body for the 2026-08-10 Pérez-Williams piece.
The Mac-UA curl is a fallback when you specifically need the
JSON-LD `datePublished` field for window-evaluation, because
`web_extract` strips the JSON-LD from the response.

**Reading the JSON-LD `datePublished`:** once the Mac-UA curl
returns the full body, parse it like this:

```bash
grep -oE '"datePublished":"[^"]+"' /tmp/article.html | head -1
# → "datePublished":"2026-08-07T21:16:12Z"
```

The `datePublished` value is UTC with `Z` — use it directly as
`published_at` without conversion.

## Liberty Media 8-K — the canonical F1 financial source

For shutdown-week stories about F1's financial performance, the
**Liberty Media 8-K SEC filing** is the primary source, not a
journalist's rewrite. Confirmed 2026-08-06: Liberty filed the Q2
2026 results via 8-K on 6 August, and every shutdown-day
republication (GP247, Motorcycle Sports Facebook, finance.yahoo.com)
cites it.

Direct URL pattern:

```text
https://www.libertymedia.com/investors/news-events/press-releases/detail/587/liberty-media-corporation-reports-second-quarter-2026
```

The 8-K's "Formula 1" bullet carries the headline numbers verbatim
(verified 2026-08-10):

- Primary F1 revenue Q2 2026: $622M (down 40% YoY)
- Total motorsport revenue Q2 2026: $764M (down 38% YoY)
- Team payments H1 2026: $500M (down 20% from $627M in H1 2025)
- Operating income Q2 2026: $73M (down from $293M)
- Adjusted OIBDA Q2 2026: $139M (down 61% from $361M)
- F1 revenue H1 2026: $1.381B (down 15% from $1.629B)
- Five races held Q2 2026 vs. nine in Q2 2025

The 38% Q2 revenue drop is the **only** number any F1 beat should
use for a "F1 revenue fell" story. The $127M team-payment drop is
the only number to use for a "F1 teams lost" story. The H1 2026
15% revenue decline is the headline for any mid-year summary.
Don't invent round numbers — cite the 8-K.

**Recipe:**

```bash
# 1. Pull the 8-K press release page
web_extract(urls=["https://www.libertymedia.com/investors/news-events/press-releases/detail/587/liberty-media-corporation-reports-second-quarter-2026"], char_limit=12000)

# 2. Cite the 8-K as the source URL in the beat JSON, even when the
#    body came from a GP247 / Motorsport.com rewrite. Liberty is
#    primary; the republisher is the path to a clean article body.
# 3. Source name: "Liberty Media 8-K (via GrandPrix247)" or similar.
```

The `stocktitan.net/sec-filings/FWONA/8-k-...` page route is a
syndicator that reproduces the 8-K text verbatim. It is a
fallback only when libertymedia.com returns a 5xx.

## Rank-4 calendar reminder is a fallback, not a forced slot

The existing playbook suggests a rank-4 calendar reminder (Pirelli
compounds + Zandvoort weekend structure + climatology) as the
fourth rank on shutdown beats. **This is a fallback, not a
mandatory slot.** Confirmed 2026-08-10: a fresher in-window
statement (Andrea Stella's McLaren mid-season interview, 9 Aug
2026 07:30 UTC) took the rank-4 slot over a stale Pirelli / sprint
reminder.

**Decision rule for rank 4 on shutdown beats:**

- If three strong in-window stories have shipped, **rank 4 should
  still be a fresh in-window statement** if one exists. Don't pad
  to the calendar reminder just because the playbook lists it.
- The calendar reminder is appropriate ONLY when fewer than three
  in-window statements are available (i.e. rank 3 is the last real
  story and rank 4 needs filler).
- For Aug 10 specifically: the Stella piece (McLaren's "harmony
  intact" statement, citing the 50-50 power split and the Hungary
  win) is a stronger rank-4 than the Pirelli / Zandvoort weather
  reminder, because the reminder was already covered in the Aug 3
  beat JSON.

The existing 4-rank pattern (Wolff-Horner / McNish / Newey /
Zandvoort calendar) was correct for Aug 3 because all four slots
had fresh in-window material. The 4-rank pattern (Silly Season /
Liberty Q2 / Domenicali / Stella) was correct for Aug 10 because
all four slots had fresh in-window material. **Match the slot
density to the news density; don't pad to 4.**

## Motorsport.com August archive page — discovery surface for shutdown beats

For shutdown weeks (Aug 1-20 of every year), the canonical
discovery surface for in-window F1 news is:

```text
https://www.motorsport.com/f1/news/archive/2026/08
```

This lists every F1 article published in August 2026, sorted by date
descending — typically 30-50 entries per month. Each entry shows the
headline, section ("Hungarian GP", "Hungaroring Pirelli test", etc.),
and a short teaser. `web_extract` on this page returns the full
listing.

**Why this matters:** during the FIA-mandated factory shutdown,
motorsport.com's editorial team keeps running on-record interviews
with team principals and racing directors that surface here but
NOT in normal `web_search` for queries like "F1 news August 2026"
(which is dominated by stale July Hungary GP results).

**Confirmed in-window shutdown archetypes surfaced via this archive
page in 2026-08-04 to 2026-08-06:**

- Sergio Perez "rethink processes" Cadillac critique (Motorsport.com
  Ed Hardy / Stuart Codling, 2026-08-04)
- Alex Albon "Baku upgrade won't cure it" Williams piece (Ed Hardy /
  Stuart Codling, 2026-08-04)
- Toto Wolff "closely monitoring" Mercedes upgrade timing (Ed Hardy
  / Oleg Karpov, 2026-08-05)
- Honda Orihara "realised scale of problems in January" interview
  (Ronald Vording, 2026-08-05)
- Andrea Stella "Macarena wing" McLaren coyness explanation
  (Stuart Codling / Ronald Vording, 2026-08-05)
- Arvid Lindblad exclusive "no plan B" interview (Roberto Chinchero,
  early August)
- Guenther Steiner praises Red Bull Lagrue hire (Lydia Mee, ~2026-08-04)
- Rachel Brookes predicts 2027 driver changes (early August)
- Allan McNish "Audi performing better than expected" interview
  (Ed Hardy / Oleg Karpov, ~2026-08-04)

Eight to ten fresh, in-window stories surface from this archive
page on any shutdown day. **The archive is the discovery surface;
individual article URLs are the citation targets.**

Recipe:

```bash
# 1. Pull the archive listing
web_extract(urls=["https://www.motorsport.com/f1/news/archive/2026/08"])

# 2. For each promising headline (3-6 candidates), pull the full body
web_extract(urls=["<article_url_1>", "<article_url_2>", ...], char_limit=10000)

# 3. Rank by in-window + new-statement quality. Filter out anything
#    that quotes Wolff / Horner / Briatore / Newey / McNish at the lead
#    if those were already covered in yesterday's beat JSON.

# 4. Pick the lead, find the image in the article body (motorsport.com
#    CDN `cdn-N.motorsport.com/images/amp/...`), download + ffmpeg-convert.
```

## Shutdown-day rotation discipline — "previous day continuity check"

When the F1 leaf runs on consecutive shutdown days (Aug 3, 4, 5, 6,
7...), the lead candidate MUST rotate. Yesterday's lead is already
in the orchestrator's continuity log and a duplicate lead fails
the orchestrator's continuity check even if the JSON parses cleanly.

**Hard rule for any shutdown-day-after-shutdown-day beat:**

```bash
# Before writing rank 1, read yesterday's beat JSON:
cat /home/arctic/.hermes/scripts/hermes-times-v4/f1_beat_<prev-date>.json | python3 -m json.tool
```

Confirm the top-line statement from yesterday's rank 1. If it's a
Wolff / Horner / McNish / Newey / Vowles / Tombazis quote, you MUST
find a fresh lead from a different category:

- **Already-shipped Aug-5 archetypes (do NOT re-lead):**
  - Tombazis "a shame" governance confession (Motorsport.com, 2026-08-01)
  - Honda 30bhp Hungaroring filming day (formula1.com, 2026-07-30)
  - Renault lock-in clause blocking Horner (GrandPrix247, 2026-08-02)
  - Pirelli Zandvoort C2/C3/C4 selection (Pirelli press, 2026-07-28)

- **Fresh Aug-6 archetypes from Motorsport.com archive page:**
  - Driver-on-team-development critique (Perez on Cadillac, Albon on Williams)
  - Team-principal-on-strategy (Wolff on Mercedes upgrade timing)
  - PU-supplier-on-problems (Orihara on Honda, McNish on Audi)
  - Tech-director's reasoning (Stella on McLaren wing timing)
  - Driver-exclusive interview (Lindblad on Red Bull junior path)

The pattern across a 4-day shutdown stretch should be:

| Day | Rank 1 archetype | Rank 2 | Rank 3 |
|-----|------------------|--------|--------|
| Aug 3 | Wolff / Horner return speculation | McNish Audi 2027 freeze | Newey / Alonso 2027 framing |
| Aug 4 | Tombazis governance confession | Honda 30bhp filming day | Renault lock-in clause |
| Aug 5 | Honda 30bhp / Alonso birthday | Tombazis follow-on | Renault lock-in follow-on |
| Aug 6 | Perez Cadillac "rethink processes" | Wolff Mercedes upgrade timing | Orihara Honda interview |
| Aug 10 | Silly Season: Sainz→Audi, Pérez→Williams | Liberty Q2 ($127M, 38%) | Domenicali 2027 sprint expansion |

**Aug 10 refresh:** the first four shutdown-day ranks were driven by
team-principal / racing-director interviews (Wolff, McNish, Newey,
Vowles, Stella). When the principal-interview well runs dry mid-week,
**pivot to driver-market / financial / governance stories sourced
from the GP247 archive** — Aug 10 was the first day in the 2026
cycle where Silly Season and the Liberty 8-K led. The Auto Sport Web
Japanese pickup pattern is the most reliable signal that a driver-
market story is real and has paddock sourcing behind it.

**Trap:** on Aug 5 the leaf shipped "Honda 30bhp at Hungaroring" as
rank 1 (the 29-Jul filming-day piece). On Aug 6 the leaf must NOT
ship another Honda story as rank 1, even though the Honda Orihara
interview is technically "newer" — it overlaps the same news cycle.
Move the Honda interview to rank 2 or 3 and lead with a different
category entirely. The orchestrator's continuity check looks at
headline-level semantic similarity, not just `published_at`.