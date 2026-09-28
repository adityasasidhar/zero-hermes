# mmx image-01 — Prompt Patterns That Work

Working prompts from real sessions. Each one took some rediscovery; preserve the structure rather than re-deriving.

## The "great prompt" template

A reliable prompt covers (in roughly this order):

1. **Subject + composition** — what is in the frame, where
2. **Setting / scene** — physical environment, surfaces
3. **Lighting** — direction, color temperature, intensity
4. **Style / medium** — photoreal, illustration, cinematic, etc.
5. **Camera / framing** — overhead, wide, shallow DOF, etc.
6. **Constraints** — what to avoid (no text, no logos, etc.)

## Examples by use case

### Tech / "what does X look like"

> "DIY electronics starter kit spread out on a glowing oak workbench: solderless breadboard, Arduino Uno, jumper wires in neat loops, RGB LED matrix showing a tiny rainbow, multimeter, hot glue gun, overhead cinematic shot, soft warm key light from the left, cool blue rim light, photoreal, shallow depth of field"

### Friendly / soft illustration

> "A friendly robot assistant sitting at a desk with floating holographic diagrams and chat bubbles around it, soft warm lighting, modern illustration style, clean and minimal"

### Sports / athletes (pitfall-laden)

> "Editorial silhouette photograph of a bareheaded male football player viewed from behind with both fists raised in celebration at night, soccer stadium floodlights creating dramatic god rays, pink-orange dusk Miami sky, palm trees at the stadium edges, blurred crowd, jersey has no visible logos or text, pure silhouette against the lit sky, photojournalism aesthetic, 16:9 aspect ratio"

(See `iterative-image-generation` for the full sports-kit failure-mode list.)

## Frequent failures & fixes

| Failure | Fix |
|---|---|
| Vague subject → mid image | Specify subject + scene + lighting + style |
| Fake text / fake logos in scene | Add "no visible logos or text" constraint |
| Wrong sport (American football vs soccer) | Specify "soccer / association football, bareheaded" |
| Chart direction wrong (curves render up) | Specify y-axis position explicitly |
| Multi-element composition collapses | Per-panel generation + composite (see `iterative-image-generation`) |
| Style loses cohesion with long prompt | Ruthlessly prioritize; put key style at end |

## Style vocabulary that lands

- **Photoreal**: "photoreal, shallow depth of field, 8k, cinematic shot, dramatic lighting"
- **Modern illustration**: "modern illustration style, clean and minimal, flat colors, soft shadows"
- **Editorial**: "editorial silhouette photograph, photojournalism aesthetic"
- **Hand-drawn**: "hand-drawn editorial collage style, three equal torn-paper panels, lavender/cream/terracotta palette"
- **Cinematic**: "cinematic shot, soft warm key light from the left, cool blue rim light, 16:9 aspect ratio"

## Aspect ratios

- 16:9 for wide editorial / hero / banner
- 1:1 for avatars / icons / square crops
- 4:3 for general purpose
- 9:16 for mobile / stories / vertical

Always set `--aspect-ratio` explicitly rather than relying on default — being explicit is the difference between a usable image and one that needs cropping.
