# Messi / Soccer-Star Leaf Playbook

The general `references/leaf-beat-reporter-workflow.md` covers source
discovery, image verification, and JSON output for any beat. This
playbook adds the **soccer-star beat** layer: foreign-timezone match-day
math, preview-vs-result framing, the no-result-is-still-a-lead case,
club-vs-national-team switching, and the CDN/image-fallback patterns
specific to player-focused sports coverage (Messi in particular, but
also applicable to any individual-driven soccer beat: Ronaldo, Mbappé,
Haaland, Bellingham, Vinícius Jr, Yamal, etc.).

Load this in addition to the general playbook when the beat is a named
soccer player and the brief is a per-day edition with same-day fixtures.

## Foreign-timezone match-day math (verify BEFORE framing)

Soccer fixtures in major leagues run in non-IST timezones. The most
common kickoff slots in an IST-published paper (06:00 IST daily):

| League | Common kickoff (local) | IST equivalent | Frame in an IST paper |
| --- | --- | --- | --- |
| MLS (regular season) | 7:30 PM ET | 5:00 AM IST **next day** | preview, ~23h before kickoff |
| La Liga (typical) | 9:00 PM CET | 1:30 AM IST **next day** | preview, ~6h before kickoff |
| Premier League (typical) | 8:00 PM BST | 12:30 AM IST **next day** | preview, ~6h before kickoff |
| Champions League | 9:00 PM CET | 1:30 AM IST **next day** | preview, ~6h before kickoff |
| Argentina home WCQs | 9:00 PM ART (UTC-3) | 4:30 AM IST **next day** | preview, ~22h before kickoff |
| Argentina friendly Asia tour | varies | varies | preview if same calendar day IST, or recap if already played |

**Hard rule:** verify the actual current time and the fixture's local
kickoff time before writing the lead. The lead frame depends entirely
on whether the match has already been played:

```bash
TZ=Asia/Kolkata date +"%Y-%m-%d %H:%M IST"
TZ=America/New_York date +"%Y-%m-%d %H:%M ET"   # for MLS / USMNT / Argentina
TZ=Europe/Madrid date +"%Y-%m-%d %H:%M CET"     # for La Liga / UCL
```

If `paper_ist < fixture_ist`, the match is a **preview**. If
`paper_ist > fixture_ist + 2.5h` (regulation + stoppage), it is a
**recap**. Anything in between is "in progress" — file the preview and
flag the recap as a follow-up.

## Preview-only lead (no result yet) — what to file

When the leaf discovers the fixture is still in the future:

- **Do not invent a result.** No "2-1 Inter Miami win", no "Messi hat
  trick", no quote from a post-match press conference that hasn't
  happened. This is the most common fabrication mode for sports
  leaves — they assume the home team wins and write the recap as if
  they saw it.
- **Lead with the actual story:** projected XI, manager quotes from the
  pre-match presser, tactical preview, stakes (qualification
  scenarios, points needed, table position), notable absences
  (injuries, suspensions, bereavement leave), broadcast info.
- **Use "preview" language in the headline.** "Messi-less Inter Miami
  teeters on the brink as Club León arrives" — not "Inter Miami beats
  Club León to reach the quarterfinals". If the leaf wants to hedge,
  "Inter Miami faces Club León tonight in must-win Leagues Cup finale"
  is fine and honest.
- **Body word count discipline matters more for previews.** A preview
  body that lists the projected XI, the qualification math, and the
  broadcast details runs to ~150-200 words easily. A recap body that
  recounts goals, key moments, and post-match quotes runs to ~250-300
  words. Don't pad the preview with recap-style filler ("Messi
  celebrated with his trademark arms-raised salute..." when Messi
  isn't playing).
- **Flag in the JSON summary:** "preview-only lead; result pending.
  Orchestrator may want to refresh tomorrow with the recap."

## Source priority for soccer-star beats

Order of preference (matches the general playbook's primary-source
rule, with soccer-specific outlets):

1. **Club official site** for lineup, manager presser, injury report
   — `intermiamicf.com/news/...`, `realmadrid.com/en/news/...`,
   `fcbarcelona.com/en/news/...`. Always check `/news` and the home
   hero banner.
2. **League official site** for standings, results, fixtures —
   `mlssoccer.com`, `laliga.com`, `premierleague.com`, `afa.org.ar`.
3. **National team official site / AFA press** for Argentina-specific
   news — AFA's English press is sparse; Spanish-language `afa.org.ar`
   and `tycsports.com` are primary.
4. **Getty Images via editorial event pages** for hero photos — see
   the CDN cheat sheet below.
5. **Reputable third-party reporting** — ESPN FC, BBC Sport, Sky
   Sports, MARCA, AS, TyC Sports, Olé, Bolavip. Treat as primary
   for quotes, secondary for events.

**Avoid as primary sources:** roundup blogs ("Top 10 Messi moments"),
SEO listicles, fan Twitter accounts, Instagram reposts without
attribution, Wikipedia.

## Image fallback when no new match photo exists

The lead story is a preview, the match hasn't happened, and there's no
fresh photo to download. **Fallback chain (use in order):**

1. **Most recent prior match** with the same player — if the lead is
   "Inter Miami vs Club León Aug 12", the Aug 5 match vs San Luis is
   the natural source for a Messi hero photo. The image is from a
   different event but the same subject, jersey, and stadium — it
   reads correctly as "Messi at Nu Stadium this month" without
   misleading the reader.
2. **Most recent training photo** from the club's official X/IG —
   clubs post training photos 1-2× per week. Pull from the club's
   official handle, not fan accounts.
3. **Generated cover** via `mmx image generate` with a constrained
   prompt — "Lionel Messi in Inter Miami pink #10 jersey on the
   pitch, arms raised, stadium crowd background, no text, no logos".
   Vision-verify for text artifacts.
4. **Omit `image`** — set `image: null` (or omit the key per the
   orchestrator's contract). The renderer handles text-only cards.

**Always label the fallback in the JSON** — the orchestrator's
image-credit audit needs to know the photo isn't from today's match:

```json
"image": {
  "kind": "downloaded",
  "local_path": "/home/arctic/.hermes/data/hermes-times-v4/assets/2026-08-12/messi_leaguescup_goal_san_luis_aug5.jpg",
  "source_url": "https://media.zenfs.com/en/miami_herald_slideshows_759/42fea863d2670ad3c7622fce5b238a50",
  "credit": "Matias J. Ocner / Miami Herald"
}
```

The slug `messi_leaguescup_goal_san_luis_aug5` encodes the source
match — the orchestrator (and future readers) can tell at a glance
that this isn't an Aug 12 León-match photo.

## CDN cheat sheet (soccer-star additions to the general playbook)

| CDN | URL pattern | UA / headers needed |
| --- | --- | --- |
| Miami Herald / McClatchy Zenfs | `media.zenfs.com/en/<outlet-slug>_slideshows_<n>/<hash>` | Default curl works; no special headers. Stable JPEG URL with no expiry. Pattern confirmed working for Inter Miami vs San Luis (Aug 5 2026): 1280×720, 112 KB JPEG. Discoverable via `web_extract` on a Miami Herald / Yahoo Sports slideshow page |
| Inter Miami official CDN | `images.mlssoccer.com/image/upload/v<ts>/assets/mia/<slug>.<ext>` | Default curl works; club hero photos at this host. PNG usually |
| AFA (Argentina) | `afa.org.ar` rarely hosts photos; Argentine outlets (`tycsports.com`, `ole.com.ar`) use their own CDNs (see below) | — |
| TyC Sports / Olé CDN | `images.tycsports.com/<path>/<slug>.<ext>` and `www.ole.com.ar/img/<path>/<slug>.<ext>` | Default curl works; both serve JPEG. TyC CDN often hot-links the same photo as the AFA press |
| Bolavip | `media.bolavip.com/wp-content/uploads/sites/10/<YYYY>/<MM>/<id>/<slug>.<ext>` | Default curl works; usually JPEG. Bolavip credits the actual photographer in the caption (often Rich Storry / Getty), so reuse the original credit |
| Getty Images (editorial) | `media.gettyimages.com/...` or `www.gettyimages.com/detail/news-photo/...` | Default curl works for the asset page; the image itself is behind a paywall. Don't burn cycles trying to scrape the asset — link the editorial page as `source_url` and download a wire photo from Bolavip / Miami Herald / Yahoo Sports instead |
| ESPN player headshots | `a.espncdn.com/combiner/i?img=/i/teamlogos/soccer/500/<id>.png&h=80&w=80` | Default curl works; team logos only (player portraits live behind paywall) |
| MLS team logos | `images.mlssoccer.com/image/upload/v<ts>/assets/<slug>/logo.svg` or `.png` | Default curl works; SVG or PNG |

**Discovery recipe for Miami Herald / Yahoo Sports slideshows:**

```bash
curl -A "Mozilla/5.0" -sSL "https://sports.yahoo.com/<slug>/" \
  | grep -oE 'https://media\.zenfs\.com/en/[a-z_]+_[0-9]+/[a-f0-9]+' \
  | sort -u
```

Each URL is a stable asset path. The first match in the slideshow
HTML is usually the lead photo; subsequent ones are the gallery.
Verify with `file` (must be JPEG, > 20 KB) and `vision_analyze` (must
show the named subject).

## The "preview + secondary" stacking pattern

When the lead is a preview, the orchestrator benefits from a secondary
non-match-day story to fill the rest of the beat:

- **Club contract extension / transfer rumors** — only if reported by
  a primary outlet in the window. The Inter Miami / Messi extension
  through 2028 (announced earlier in 2026) is a recurring example
  for this beat.
- **National team windows** — Argentina friendlies in September /
  October, AFA press on Copa América / World Cup qualifying. These
  often surface in the window without a match having happened.
- **Off-pitch personal news** — bereavement leave (Jorge Messi's
  funeral Aug 9 2026), foundation work, charity appearances. Only
  with primary-source confirmation.
- **Career milestones / records** — e.g. Messi reaches career goal
  #921, becomes all-time Leagues Cup top scorer. Only if the
  milestone happened in the window (not "approaching" or "expected to
  pass").

**Stacking rule:** the preview is always rank 1 (primary). The
secondary is rank 2+ with `primary: false`. Don't demote the preview
just because the secondary is "bigger news globally" — the preview is
the day's actual event for the beat.

## Pitfalls specific to this beat

- **Treating "no news today" as a story.** If Messi is genuinely
  resting (between windows, no match for 5+ days), file a single-line
  "He is resting. He has earned it." and stop. Don't pad with
  week-old stories about contract extensions or training clips.
  Quality > quantity.
- **Quoting a player statement from a fan account.** Messi posts
  occasionally on his official IG; everything else is repost
  journalism. Treat the original IG post as primary, the repost as
  secondary.
- **National team switching leaks.** When a star player hints at
  retirement or a national-team farewell tour, the story goes viral
  across Spanish-language outlets within hours. Verify against the
  player's own statement (or the federation's) before publishing —
  even one fabricated "Messi confirms retirement" headline is a
  reputation-ending bug for the beat.
- **Match-report fabrication is the #1 failure mode.** A leaf with no
  tool budget for `web_extract` on the live scoreboard will sometimes
  emit a plausible-looking 2-1 recap from memory. The
  orchestrator's preview frame is the only defense — the JSON
  schema doesn't have a `fixture_status` field, so the leaf must
  self-report "preview vs recap" in its summary text and the
  orchestrator must catch mismatches.
- **Same player, two clubs in 24 hours.** Soccer stars occasionally
  surface on the national-team wire hours after a club match (e.g.
  Messi joins Argentina camp after a Wednesday MLS match, plays
  Saturday in Buenos Aires). When filing two stories on the same
  player for the same paper, separate the club and national-team
  stories clearly — different sections, different contexts, different
  sources.
- **Photo from the wrong era.** Getty's Messi archive contains
  20,000+ photos spanning Barcelona, PSG, and Inter Miami. If the
  leaf surfaces a 2015 Camp Nou photo for a 2026 Inter Miami story,
  the orchestrator's vision check must reject it. Verify the kit
  color (Inter Miami pink/black vs Barcelona blaugrana vs PSG navy)
  and the player's visible age (39 in 2026 vs 28 in 2015) before
  keeping the image.

## JSON output sanity check (preview vs recap)

Before returning the leaf JSON, the leaf should self-report:

```text
Lead fixture status: <preview / in-progress / recap>
Lead fixture local kickoff: <ISO 8601 with TZ offset>
Lead fixture IST equivalent: <ISO 8601 with +05:30>
Paper publication moment IST: <ISO 8601 with +05:30>
Delta (paper - fixture): <hours, negative if preview>
```

The orchestrator's manifest-builder parses this from the leaf's
summary text. A recap that should be a preview (or vice versa) is a
hard reject — the orchestrator re-files the beat as preview rather
than ship the wrong frame.
