# MiniMax-Powered Premium Newspaper Pipeline

A proven implementation pattern for an image-rich daily AI newspaper where factual integrity and visual quality both matter.

## Architecture

```text
concurrent factual collectors
  ├─ official lab RSS
  ├─ arXiv API
  └─ GitHub releases/commits via authenticated `gh api`
       ↓
source pack with stable source IDs
       ↓
MiniMax-M3 editorial JSON
       ↓
identity repair from source-ID map
       ↓
MiniMax image-01 conceptual art (2–4 images, serial + retry)
       ↓
HTML/CSS editorial layout → Chrome PDF → per-page visual QA
       ↓
PNG cover + PDF via `hermes send`; cron `no_agent=true`, `deliver=local`
```

## Editorial JSON boundary

The model may write issue theme, headline, dek, why-it-matters, story angles, research takeaways, and text-free art prompts. It must choose only source IDs from the supplied pack.

After parsing, overwrite all factual identity fields from the source record selected by ID:

- title, URL, publisher/source, and publication date;
- arXiv authors and paper URL;
- GitHub owner/repo, release tag, and release URL.

Reject output that selects the wrong class or produces too few items per required section. Use a clearly labeled deterministic fallback only for an individual failed editorial call.

## Source collector reliability

- Use `gh api repos/OWNER/REPO/releases?per_page=N`, not unauthenticated `urllib` GitHub calls. A daily multi-repo paper can otherwise hit the public API's 60-request/hour limit.
- Bound each source request and collect independently so one provider failure does not erase unrelated sections.
- Respect arXiv pacing. Avoid repeated test/rerender loops against arXiv; cache the last successful source payload for graceful temporary fallback.
- Remove raw release Markdown before it reaches the model/layout: URLs, heading markers, link syntax, commit-hash tails, and changelog boilerplate are not editorial copy.

## MiniMax media operational notes

- `mmx text chat --model MiniMax-M3 ... --quiet` is suitable for the editorial pass. Strict JSON is still a prompt requirement; strip Markdown fences before parsing.
- `mmx image generate --out-dir ... --out-prefix ... --quiet` should always use an explicit output directory.
- Image generation can transiently fail under parallel load. Generate 2–4 illustrations serially with a bounded retry on `Network request failed`; source collection remains safely parallel.
- Prompts should say **no text, no logos, no screens, no UI, no watermark**. Image models are poor at exact diagrams and text; use them for conceptual editorial art, never factual evidence.
- A rights-first default is generated conceptual art. Do not automatically embed OpenGraph, RSS-enclosure, or social thumbnails merely because they are fetchable; they are not necessarily reusable. External imagery needs an explicit license/press-kit/public-domain allowlist and provenance/credit metadata.

## Visual QA gates

1. Verify exact intended PDF page count with `pdfinfo`.
2. Rasterise every page, not just page 1.
3. Inspect the actual scheduler-produced issue after a force-run—image generation makes the final render non-identical to a dry run.
4. Specifically check for:
   - folio/footer overlap from variable editorial copy;
   - fake image text, logos, or interface artifacts;
   - sparse fixed-height sections that look like unrendered template regions;
   - card overflow or clipped final story;
   - one dominant visual lead plus intentionally paced supporting imagery.
5. If the edition looks like a source dump, change editorial selection and composition, not merely font/color tokens.
