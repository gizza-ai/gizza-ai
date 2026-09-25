# speech-bubble-adder — competitor analysis (2026-09-04)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
Everything below is paraphrased observation of publicly documented feature lists — no competitor
copy, branding, or trademarks are reproduced or reused anywhere in the block.

## Semantic duplicate check (done first)

The backlog row (`speech-bubble-adder` — "Adds comic-style speech or thought bubbles with caption
text onto an image at chosen coordinates.", type_hint `pure`) was checked against the four nearest
shipped blocks before any code was written. Verdict: **not a duplicate — build it.**

| Existing block | What it actually does (read from `core/src/lib.rs` + descriptor) | Why it is not this tool |
| --- | --- | --- |
| `add-text-to-image` | `render(img, text, x, y, font_size, color)` — rasterizes glyphs with fontdue and alpha-blends them straight onto the pixels. 168 lines, no shape drawing at all. | Draws **bare text only**. There is no balloon, no fill, no outline, no tail — the defining features of this tool. |
| `image-annotate` | JSON array of marks: `box` (hollow rect), `arrow`, `highlight` (translucent wash), `text`. | Its four primitives are review/markup marks. No ellipse, no cloud, no starburst, no tail, and no text-inside-a-shape layout. Drawing a bubble with it is not expressible. |
| `meme-caption` | Impact-style top/bottom captions, auto-sized to image width, uppercased, white fill + black stroke. | Fixed top/bottom edge placement, no arbitrary coordinates, no enclosing shape, no speaker pointer. |
| `text-banner-image` | Generates a **new** 1200×400 gradient banner PNG from a headline. | Takes no input image — it is an image *generator*, not an overlay. |

The distinct capability here is the **balloon geometry**: a closed filled+outlined shape (oval,
rounded rect, cloud, starburst), text laid out *inside* it with wrapping and auto-fit, and a
**tail that points at a speaker**. Nothing in the repo draws any of those. Not skiplisted.

## Competitors reviewed

1. **imageonline.io — Speech Bubble Generator** (`imageonline.io/speech-bubble-generator/`)
2. **addspeechbubble.com — Add a Speech Bubble to a Photo** (`addspeechbubble.com/add-speech-bubble-to-photo/`)
3. **wallpapers.com — Speech Bubble Generator** (`wallpapers.com/tools/text-graphic-design/speech-bubble`)

Cross-checked against the feature blurbs of Fotor, Canva, Pippit and Media.io from the same
search; none of those added a capability the three above do not already cover.

## Table-stakes matrix

| Capability | Seen at | Fit | Where it landed |
| --- | --- | --- | --- |
| Bubble style: speech (rounded + tail) | all 3 | in-model | `style=speech` (default) |
| Bubble style: oval / classic balloon | 1, 2 | in-model | `style=oval` |
| Bubble style: thought (cloud + trailing dots) | all 3 | in-model | `style=thought`, tail rendered as 3 shrinking puffs |
| Bubble style: shout / jagged starburst | all 3 | in-model | `style=shout` |
| Bubble style: whisper (dotted outline) | 3 | in-model | `style=whisper` (dashed stroke) |
| Bubble style: caption / narrator box | 3 | in-model | `style=caption` (sharp rect, no tail) |
| Text inside the bubble, auto-wrapped | all 3 | in-model | wrapping + `\n` hard breaks |
| Bubble auto-grows as you type | 2, 3 | in-model | `width=0`/`height=0` → auto-size from the text |
| Font size small → extra large | 1 | in-model | `font_size` (0 = auto-fit to the bubble) |
| Text colour (full RGB) | 1, 2 | in-model | `text_color` (#rgb/#rrggbb/#rrggbbaa) |
| Bubble fill colour | all 3 | in-model | `fill_color` |
| Outline/border colour | 1, 2 | in-model | `outline_color` |
| Outline width | 3 | in-model | `outline_width` (0 = no outline) |
| Position the bubble anywhere | all 3 | in-model | `x` / `y` (px, top-left origin) |
| Resize the bubble | all 3 | in-model | `width` / `height` |
| Tail direction presets (bottom-left/right, top-left/right) | 2 | in-model | `tail` enum, 8 directions + `none` |
| Aim the tail at the speaker (drag the dot) | 2, 3 | in-model | `tail_x` / `tail_y` — exact pixel aim point |
| Drop shadow toggle | 2 | in-model | `shadow` |
| Multiple bubbles on one image | all 3 | in-model | `bubbles` JSON array (each item inherits the top-level defaults) |
| ALL-CAPS comic lettering | (meme convention) | in-model | `uppercase` |
| PNG output, no watermark | all 3 | in-model | always PNG, no watermark, runs locally |
| Accepts JPG / PNG / WebP / GIF / BMP input | 2 | in-model | `image` crate decode features |

### Out of model — listed, deliberately not built

| Capability | Seen at | Why not |
| --- | --- | --- |
| Interactive drag/resize handles, click-to-place | all 3 | This block is a headless chat + CLI transform (no page — a pure-Rust image-bytes output has no page render mode in this repo). Coordinates are the deliberate interface; that is what the backlog row asks for. |
| Comic/handwritten/marker/pixel font families | 2, 3 | Would need extra licensed font binaries bundled into the wasm (each ≈340–420 KB). The block ships one bold sans face (Liberation Sans Bold, licence included), which is the standard comic-lettering weight. Revisit if a licence-clean comic face is vendored repo-wide. |
| Bubble rotation | 2 | Not a table stake on 2 of 3; adds a second geometry pass. Deferred. |
| JPG / WebP export | 1, 3 | The repo already ships `image-convert`; chaining it keeps one PNG contract here (same choice as `add-text-to-image` / `image-annotate`). |
| Bulk mode (up to 200 images per job) | 3 | Batch orchestration is a caller concern; the block is one image per call. |
| Neon-glow / Discord / anime / pixel skins | 2 | Styling skins, not distinct geometry. The six shapes above cover every geometry those skins are built on. |
| Copy-to-clipboard | 1 | Surface concern, not a block capability. |

## Decisions taken from the scan

- **Six styles, not one.** All three competitors ship at least three shapes; shipping only a
  rounded rect would have missed the row's own words ("speech **or thought** bubbles").
- **Both tail models.** Competitor 2 ships direction presets *and* a draggable aim dot, so the
  descriptor carries both: an 8-way `tail` enum for the common case and `tail_x`/`tail_y` for exact
  aiming at a speaker's mouth. `tail_x`/`tail_y` win when supplied.
- **Auto-everything defaults.** `width`/`height`/`font_size` all default to `0` = auto, mirroring
  the "bubble grows as you type" behaviour of competitors 2 and 3, so the minimal call is just
  `text=` plus the image.
- **Multi-bubble via a JSON array**, following the shipped `image-annotate` convention (top-level
  params act as the per-item defaults) rather than inventing a second idiom.
- **No page.** A pure-Rust image-bytes output has no page render mode in this repo (the page image
  mode is for ffmpeg argv tools) — same shape as `add-text-to-image`, `image-annotate`,
  `meme-caption`. Surfaces verified: chat schema (drift-guard test) + CLI.
