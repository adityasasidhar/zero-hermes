# Messi beat — discovery cheatsheet

Companion to `../SKILL.md` for the news-correspondent umbrella. Load this when the
beat assignment is `messi`; skip otherwise.

The umbrella's source-hierarchy list (MLS official, Inter Miami CF, Argentina AFA,
Getty, ESPN FC, TyC, Olé, MARCA, AS) is authoritative. This file is about the
discovery mechanics — which URLs to hit first, where the un-gated art lives, and the
recurring false-positive traps.

## Primary discovery surfaces (hit in this order)

1. **Inter Miami CF newsroom** — `https://www.intermiamicf.com/news`. The canonical
   source. Every training-session announcement, injury update, signing, and team
   statement lands here first. The article's first `<img>` is almost always a free,
   usable press photo (see Image source fallbacks below). The homepage
   `https://www.intermiamicf.com` carries the same articles with their hero images;
   `web_extract` on the homepage returns ~30+ headlines plus image URLs in one
   call — use it as a quick scan before drilling in.

2. **MLS Soccer editorial** — `https://www.mlssoccer.com/competitions/...` and
   `https://www.mlssoccer.com/clubs/inter-miami-cf`. The league's own editorial
   desk covers Messi as a league-level story (MVP tracker, league-record goal
   contributions, World Cup-to-MLS pipeline stories). Hero images on MLS Soccer
   articles are CDN-served and un-gated (see below).

3. **ESPN FC** — `https://www.espn.com/soccer/story/_/id/<id>/...`. Long-form with
   sourced reporting (named beat reporters — Lizzy Becherano for Miami, Tim
   Howard/Gab Marcotti for global football). Source-verified claims ("source
   confirmed to ESPN") carry more weight than Reddit/Instagram rumours. ESPN's
   story pages sometimes carry usable AP/Getty photos via their CDN; the
   `media.gettyimages.com` URLs they embed are usually gated but the article's
   `og:image` is often an un-gated ESPN-hosted version.

4. **Argentina AFA** — `https://www.afa.com.ar`. Spanish-language, the only official
   source for national-team statements. Date extraction: the URL slug usually
   carries `/YYYY/MM/DD/...`. The site does not expose JSON-LD; web_extract gives
   the body text but no publish timestamp.

5. **Wire services as primary** — Reuters (often via `aljazeera.com/sports`,
   `theguardian.com`, `france24.com`), AP (via `espn.com`, `mlssoccer.com`,
   `usatoday.com`). Wire copy is acceptable when it names the actual reporting
   desk and the article is dated within the window.

## Beat-specific trusted outlets (not in the umbrella list)

- **beIN Sports USA** (`beinsports.com/en-us/soccer/mls/...`) — strong on MLS
  coverage, often embeds Inter Miami's Instagram photo of Messi in the
  article body. Cross-checks the wire.
- **GOAL USA** (`goal.com/us`) — same coverage as ESPN but sometimes faster on
  training-return scoops; treat the same weight as a major outlet.
- **MARCA, AS, Mundo Deportivo** — Spanish press for the emotional framing on
  Argentina-related stories (final aftermath, retirement rumours).
- **TyC Sports** (`tycsports.com`) — Argentine outlet; the canonical primary for
  Messi-on-Argentina news. Quote-heavy; named reporters (César Luis Merlo, etc.)
  are reliable.
- **Olé** — tabloid-grade; treat quotes as primary but treat speculation as
  speculation, not fact.

## Image source fallbacks (the critical recipe)

The umbrella's image-verification recipe assumes Getty Images works as a direct
source. **For the Messi beat the asset detail page is gated** — `curl` of
`www.gettyimages.com/detail/news-photo/.../<id>` returns a login wall — but the
*editorial event page* route is the working workaround (see source 4 below).
For the un-gated alternatives, the `file` check is reliable: real JPEG/PNG
bytes return `JPEG image data` / `PNG image data`; gated pages return
`HTML document`.

**Working sources, in priority order:**

1. **AP News direct assets** — the underlying asset URL on
   `assets.apnews.com/<hash>/<hash>` returns the actual JPEG/PNG bytes —
   verified 4344×2896, ~2.5 MB, with full EXIF (photographer byline in
   `ImageDescription`, model/software fields). **The thumbnail resizer is**
   `dims.apnews.com/dims4/default/<config>?url=<assets.apnews.com URL>` — when
   syndicated outlets (Miami Herald, ESPN, USA Today) embed AP images, the
   page HTML exposes the `dims4` URL and the underlying `assets.apnews.com`
   URL is the un-gated asset. To extract: read the `dims` URL's `?url=`
   query parameter and curl that directly with the dim transformations
   stripped. Wire-service photo IDs surface from `apnews.com/hub/...` index
   pages and from syndicated use on ESPN / USA Today. **This is the cleanest
   un-gated source for wire-service photography** — preferable to MLS
   Cloudinary when the lead story is league-/national-team-level (World Cup
   aftermath, league award, etc.) rather than club-specific. **Downside:**
   AP's site does not always expose the raw asset URL on the article page
   itself; the `dims` indirection is the only reliable discovery path.

1a. **Al Jazeera / wire syndication CDN** — when Reuters, AP, or AFP wire copy
   is published by `aljazeera.com/sports`, `france24.com`, `theguardian.com`,
   or other reputable syndicators, the wire photo often lives at
   `https://www.<syndicator>/wp-content/uploads/<YYYY>/<MM>/<slug>.jpg?<query>`
   — verified ~58-200KB JPEGs served with default `curl -L -A "Mozilla/5.0"`,
   no `Referer` required, no auth wall. The URL is discoverable from the
   article's first `<img>` element via `web_extract`. **Use this when AP
   isn't the wire source** (e.g., Reuters-syndicated stories that didn't go
   through AP). Credit as `"<Photographer> / <Syndicator>"` (e.g. `"Lee Smith
   / Reuters"`) — the photographer string is in the article caption, not in
   the JPEG EXIF. Confirmed working example (2026-08-09 Messi beat):
   `https://www.aljazeera.com/wp-content/uploads/2026/08/2026-07-19T224207Z_799602868_UP1EM7J1R25CK_RTRMADP_3_SOCCER-WORLDCUP-ESP-ARG-1786176870.jpg?resize=770%2C513&quality=80`
   returned a 770×513 / 58KB JPEG of the World Cup trophy presentation with
   Tapia and Messi — the lead image for the AFA-president retirement-timing
   story. **Verify the same way** (file → `JPEG image data`, stat → > 20480),
   then `vision_analyze` to confirm the image matches the story body
   (Trophy presentation photo for a Tapia-on-Messi-retirement story is
   on-topic; an unrelated celebration photo for the same story is not).

2. **MLS Soccer / Inter Miami CDN** — `images.mlssoccer.com/image/private/...`.
   Cloudinary-backed. **Un-gated.** Returns the actual image bytes. Two URL shapes:

   - Hero-image (editorial graphics + composite photos): pattern
     `https://images.mlssoccer.com/image/private/t_editorial_landscape_8_desktop_mobile/f_png/mls/<hash>.png`
     The `f_png` transform forces PNG; `f_jpg` forces JPEG. These are 824×464,
     ~150-400 KB. Use as a fallback only if no club photo is available — they're
     often composite graphics with text overlays, not pure photos.
   - Club press photo (real photography): pattern
     `https://images.mlssoccer.com/image/private/t_photogallery/f_auto/mls-mia/<hash>.jpg`
     The `mls-mia/` namespace = Inter Miami CF. These are 1500×1000+ JPEGs and
     are real press photography from club events. **This is the preferred
     fallback when Inter Miami's official article hero is unavailable.**
   - The transformation query string controls output: `f_auto` returns the
     source's native format; `f_png`/`f_jpg` forces; `t_<transform>` controls
     crop. `t_editorial_landscape_8_desktop_mobile` is a 8:5 landscape crop
     suited to a newspaper column.

3. **Inter Miami's official CDN** — same domain as above; the club's own assets
   live at `images.mlssoccer.com/image/upload/.../assets/mia/...`. These are
   press-kit logos, third-kit reveals, badge artwork — useful for header art but
   rarely the right photo for a story.

4. **Getty Images via editorial event page** — the working path for sports
   photos when no MLS Cloudinary asset is available. The editorial event page
   (`https://www.gettyimages.com/editorial-images/sport/event/<event_id>/...`)
   exposes a gallery of thumbnails whose `<img>` elements already carry
   signed CDN URLs of the form
   `https://media.gettyimages.com/id/<id>/photo/<slug>.jpg?s=612x612&w=gi&k=20&c=<hash>=`.
   These URLs serve real JPEGs (~30–40KB at 612px) when the request carries
   a `Referer: https://www.gettyimages.com/` header. Without the referer, the
   CDN returns `HTML document` (a login wall) and the verification recipe lies.
   Recipe:

   ```bash
   # 1. Load the event page in browser, then extract signed URLs from the DOM:
   #    browser_console expression:
   #    Array.from(document.querySelectorAll('img')).map(i => i.src)
   #      .filter(s => s && s.includes('media.gettyimages.com/id/'))
   #      .slice(0, 15)
   # 2. Click any thumbnail to open the detail page; the EXIF caption
   #    confirms player + match + date.
   # 3. Download with referer:
   mkdir -p ~/.hermes/data/hermes-times-v4/assets/<DATE>/
   curl -sSL -A "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36" \
     -H "Referer: https://www.gettyimages.com/" \
     -o <target>.jpg "<signed-cdn-url>"
   file <target>.jpg    # must say "JPEG image data"
   stat -c '%s' <target>.jpg  # must be > 20480
   ```

   **Verify visually after download** — the same player may appear in multiple
   photos from the same match; `vision_analyze` confirms you downloaded the
   right one (especially when the gallery mixes training, warmup, and in-match
   shots). The **EXIF `Description` field in the downloaded JPEG carries the
   caption verbatim** ("MIAMI, FLORIDA - AUGUST 01: Lionel Messi #10 of Inter
   Miami CF looks on during the MLS match between Inter Miami CF and
   Columbus...") — that string is your in-JPEG proof of player/match/date,
   plus the photographer credit is visible in the detail page's DETAILS table.

   The 612px (`?s=612x612`) variant is the right size for verification;
   bumped variants (`?s=2048x2048`) currently return 0 bytes without a
   different auth context, so don't try to upscale.

   **Trap:** the older blanket rule for the Messi beat — "Getty URLs are
   gated, treat as references only" — is wrong for the editorial event page
   path. That rule was written when the working surface was the
   `www.gettyimages.com/detail/` page (which IS gated). The event gallery
   route is the workaround.

**Verification recipe for Messi beat (replaces the umbrella recipe):**

```bash
mkdir -p ~/.hermes/data/hermes-times-v4/assets/<DATE>/
curl -L --fail -sS -o <target> "<mls_or_intermiami_url>"
file <target>           # must say JPEG or PNG; "HTML document" = gate hit
stat -c '%s' <target>   # must be > 20480 (~20KB)
```

If `file` says `HTML document`, the URL is gated — fall back to the MLS
Cloudinary URL by re-extracting the article page and finding the
`images.mlssoccer.com` URL pattern.

**Important note on file extension:** the convention is `messi_<slug>.jpg`, but
the MLS CDN URLs ending in `.png` return real PNGs that you should rename to
`.jpg` anyway — the brief accepts both JPEG and PNG. Do not try to `convert`
PNG → JPG to match the convention; the orchestrator's downstream renderer reads
the file's actual MIME from `file`, not the extension.

## Live/in-progress match result: ESPN HTML is the scoreboard

When a match is in progress or just finished and no recap has been filed yet, the
official recaps at `intermiamicf.com/news` and `leaguescup.com/news` are empty.
**The working surface is the ESPN match page HTML itself** — a ~1.3 MB
React-server-rendered page at
`https://www.espn.com/soccer/match/_/gameId/<id>` whose Next.js payload carries
the live commentary feed in plaintext plus the score progression in JSON keys.

Recipe:

```bash
curl -sL -A "Mozilla/5.0" --compressed --max-time 30 \
  -o /tmp/espn.html "https://www.espn.com/soccer/match/_/gameId/<id>"
grep -B 1 -A 1 -oiE "Goal!.{1,200}" /tmp/espn.html | head -40   # goals + assists
grep -oiE "statusPrimary[^,}]{0,30}" /tmp/espn.html | head       # HT/FT/LIVE
grep -B 1 -A 1 -oiE "homeScore[^a-z][^,}]*|awayScore[^a-z][^,}]*" \
  /tmp/espn.html | tail -20                                     # score progression
```

Each `Goal!` string is `Goal! Team A <score>, Team B <score>. <scorer> (<club>)
<shot type> from <location>. Assisted by <player>.` — enough to write the
rank-1 lead body before the official recap lands. ESPN gameIds are stable (the
same URL works for the live page and the post-match archive); find the gameId
on the ESPN schedule page or in the `scoreboard` widget on the league
standings page.

ESPN returns a 0-byte body without `--compressed` (the payload is gzip) and
returns `Access Denied` without a real-browser `User-Agent`. Default
`Mozilla/5.0` plus `--compressed` works.

**Confirmed pattern from the 2026-08-05 Leagues Cup opener (kickoff ~5 AM IST,
no recap at halftime):** Miami 4–1 San Luis at halftime (Rodríguez 4'; Messi 11'
from Allen; Segovia 26'; Messi 45'+2' from Allen; Micael 45'+7' from Messi
corner) — all scraped directly from the live ESPN feed before any primary
outlet had filed a story.

**Confirmed pattern from the 2026-08-09 Leagues Cup Matchday 2 (kickoff 5 AM
IST, no recap at ~30 min in):** FOX Sports live boxscore
`https://www.foxsports.com/soccer/leagues-cup-inter-miami-cf-vs-monterrey-aug-09-2026-game-boxscore-723311`
showed `MIA 0 - 0 MON` at 1st half with empty per-half cells. ESPN's `Goal!`
regex returned zero hits because no goals had been scored yet — same data
through the underlying feed, not a broken scrape. 365Scores
(`365scores.com/en-us/football/match/leagues-cup-7242/inter-miami-monterrey-1257-54729-7242`)
and Bolavip carried the same 0-0 first-half state in scrapeable form. **A
0-0 first-half reading means there is no match story yet** — the lead must
come from a non-match in-window story instead (see the SKILL.md pitfall on
"0-0 in-progress is NOT a lead"). Don't iterate through scoreboard sources
hoping one has a goal; they all read the same feed. Move on and find the lead
elsewhere in the window.

The `gettyimages.com/detail/` page route is gated and useless for art. The ESPN
match page's embedded `og:image` is usually the pre-match promo graphic, not a
goal photo — for the lead image, fall back to MLS Cloudinary or the Getty
editorial event page route (see above).

## Recurring story archetypes (and the year's cycle)

The Messi beat has a **predictable weekly/monthly cadence** that surfaces as
"the same story, different vendor" — same pitfall the umbrella flags for
AI-industry. Recognizing the archetype lets you skip the SEO noise:

- **Post-tournament training return** — every World Cup / Copa América cycle:
  Messi lands back at the Florida Blue Training Center ~10 days after the
  final (or elimination). The Inter Miami CF Instagram post of Messi arriving
  with his mate is the canonical first frame; beIN Sports and ESPN usually
  pick it up within hours. **Don't manufacture a "return" story when he's
  been training all along** — only run this when he's been absent for ≥5 days
  and the club confirms he's back at 100%.
- **21-day rest window** — FIFA/FIFPRO grant players 21 days off after a
  World Cup run. Anyone going deep at the tournament can sit out ~3 matches
  without club consequence. ESPN sources reported Messi + De Paul both
  inside this window after the 2026 WC; the club skips the All-Star Game
  too. Use the window as the *reason* for absences, not as a story on its
  own.
- **Career milestone framing** — every World Cup, Copa, or major tournament
  drops a "first player to..." or "Messi sets record for..." story. The
  canonical pattern: most goal contributions (33), most assists (12),
  most finals as captain (3), most caps (207), most international assists
  (68). The list is mostly stable year-to-year — only add a new one if the
  tournament actually changed the count.
- **Family news sidebar** — Jorge Messi's health has been a recurring item
  since June 2026 (illness disclosed during the WC). Treat any "father"
  news as primary when the family issues a statement; otherwise it's
  speculation. Don't lead the beat with it; relegate to rank 3.
- **All-Star Game / Leagues Cup opener** — both events are predictable
  recurring fixtures the umbrella already flags as year-bleed risks
  (Charlotte 2026 vs Austin 2025). Cross-check venue and opponent against
  the current year's fixture before treating as in-window.

## Hard recency traps specific to this beat

- **"Messi wins record 9th Ballon d'Or" or similar award stories** — search
  results keep returning forward-dated award announcement articles for
  months. Always cross-check the article body for the actual award date
  before publishing; the URL slug and headline can both lie about recency.
- **Transfer rumours to PSG, Barcelona, Saudi, etc.** — the
  umbrella already gates these on credible-outlet sourcing. For Messi
  specifically: rumours from `mundodeportivo.com`, `sport.es`,
  `leparisien.fr`, or RMC Sport are tier 2 (named Spanish/French press
  with named reporters); rumours from `caughtoffside.com`,
  `teamtalk.com`, or any "Messi to..." Facebook post are tier 4 (SEO
  farms). The tier-2 ones can run if the user explicitly asks for transfer
  coverage; otherwise drop them silently.
- **Year bleed on the World Cup final recap** — search results for
  "Messi World Cup final" return both 2022 (Qatar, 3-3 ET, pens, Argentina
  win) and 2026 (U.S., 1-0 ET, Ferran Torres, Spain win) frequently. The
  2026 final was at MetLife Stadium, NY/NJ on 2026-07-19 — if the article
  says Lusail or Qatar, it's the 2022 tournament; if the article says
  Argentina won, it's also the 2022 tournament (or the article is wrong).
  Surface-level indicators that distinguish the two: 2026 had the Leandro
  Paredes–Eric García brawl at the final whistle, the AFA retired No. 10
  jersey debate, and Lionel Scaloni defending his "warriors" in the post-
  match presser; 2026 had Messi in tears at full-time and his "wound will
  take time to heal" Instagram post the next day.

## Body length and shape

The umbrella's 130-200 word band applies. The Messi beat over-writes by
~30% on average because the story is rich with context (records, family,
rest window, upcoming matches). **Pattern:**

- Lead body: 150-180 words. State the news, name the source, give the
  immediate context (return date, status of upcoming match).
- Rank 2 (match preview / standings): 120-150 words. Just the fixture,
  the form, the table position. Resist the urge to recap the World Cup
  here.
- Rank 3 (legacy / records / off-pitch): 130-170 words. Hit the records,
  the family sidebar, the career milestone — but don't repeat facts from
  rank 1's body.

If your first draft of any body comes in over 200 words, cut the second
paragraph first — that's where the "and also..." padding hides.

## When there's no match or obvious news

The brief is explicit: **never return empty**. For Messi beat specifically,
acceptable "no obvious news" stories that count as SOMETHING real:

- **Training photo / fitness update** — if Inter Miami CF posted a new
  press photo or a video clip in the last 48 hours, that's a story.
- **Award / MVP tracker** — back-to-back Landon Donovan MLS MVP mentions
  surface weekly during the season; "Messi in line for record third MVP"
  is acceptable when there's reporting behind it.
- **Argentina friendly / Scaloni squad announcement** — even out-of-window
  Scaloni calls become news for this beat; tag the source carefully (AFA
  press > TyC > tabloid).
- **Off-pitch business** — Adidas contract news, Apple MLS partnership,
  his Inter Miami equity stake. **Skip if unconfirmed by a primary
  outlet**; this category is where rumours metastasise.

If truly nothing has happened in 48 hours (rare), the brief's "He is
resting" line is the right answer. Don't manufacture filler.

**Sub-pattern: in-window non-match lead when the live match has nothing to
report.** When the match is in progress and the live scoreboard is still
0-0 (no goals yet), the live match cannot be the lead — but a non-match
in-window story CAN be. Confirmed archetypes that beat the "resting" line:

- **AFA / Scaloni / Tapia statements** — any official AFA communication
  about Messi (retirement timing, Copa América 2028 plans, September
  friendlies squad). Quote-heavy, primary source (AFA > TyC > Olé).
  Confirmed working lead (2026-08-09 beat): AFA president Claudio Tapia
  told Argentine TV on Aug 8 that Messi alone will decide his retirement
  timing — Reuters via Al Jazeera, in-window by one day, sourced to a
  credible outlet, with a usable Reuters/Lee Smith hero photo via the
  Al Jazeera CDN (source 1a above). This became rank-1; the still-in-
  progress Inter Miami vs Monterrey match moved to rank-2 as a "live
  bulletin" with no fabricated score.
- **Argentina friendly opponent / venue announcement** — AFA press > TyC.
- **Messi business / sponsorship disclosures** — only when a primary
  outlet (Reuters, Bloomberg, the brand itself) breaks it.

The non-match lead needs a usable hero photo and a 150-180 word body
that doesn't pad with the live match recap (which the user will see in
rank-2 anyway). The "and the live match is 0-0 at halftime" framing belongs
in rank-2, not rank-1.