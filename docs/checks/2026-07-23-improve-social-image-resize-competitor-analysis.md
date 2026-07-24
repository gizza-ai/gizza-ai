# social-image-resize — competitor analysis (2026-07-23)

Tool: crop + resize one image to a chosen social-platform standard size, in-browser
(ffmpeg-wasm), no upload. Single-image in → single-image out (keeps input format).

## Competitor scan (top 3, paraphrased — NO copy/branding reproduced)

Searched "social media image resizer all platform sizes". Skimmed:

1. **FreeImgKit — Social Media Resizer.** 8 presets: Instagram Square 1080×1080,
   Instagram Portrait 1080×1350, Instagram Story/Reel 1080×1920, Twitter/X Post
   1600×900, LinkedIn Post 1200×627, Facebook Cover 820×312, YouTube Thumbnail
   1280×720, Open Graph 1200×630. Cover-fit (fill + centre-crop) only, no anchor,
   no padding. Output: JPEG 92%. Per-preset resize + download; multi-size from one
   upload.
2. **ImageOnline.io — Social Media Image Resizer.** Instagram square/portrait/
   landscape (1080×566), Facebook cover 820×312 + post 1200×630, YouTube 1280×720,
   plus X/LinkedIn/Pinterest/TikTok. Fit modes: Crop / Fit / Blur / Stretch.
   Adjustable background colour. Quality slider (~90%). Output PNG/JPG/WebP.
   Batch export → ZIP of checked sizes; crop anchor buttons (Fit/Fill/Center) +
   manual drag. Lanczos scaling.
3. **Mixpost — Social Media Image Resizer.** 9 platforms (Instagram, Facebook, X,
   LinkedIn, Pinterest, YouTube, Threads, Bluesky, Mastodon), many sizes each incl.
   Instagram Story 1080×1920, X Header 1500×500, LinkedIn Post 1200×627, Pinterest
   Optimal Pin 1000×1500, YouTube Thumbnail 1280×720. Manual per-size crop. Output
   JPG/PNG/GIF/WebP. No account; client-side.

## Table-stakes → decision

| Capability | Competitors | Ours | Decision |
|---|---|---|---|
| Platform size presets (IG/FB/X/LinkedIn/YouTube/Pinterest/TikTok) | all | `target` enum, 12 values / 7 platforms | **in-model — in descriptor** |
| Cover-fit (fill + crop), default | all | `fit=cover` (default) | **in-model — in descriptor** |
| Contain / pad | ImageOnline | `fit=contain` | **in-model — in descriptor** |
| Stretch | ImageOnline | `fit=stretch` | **in-model — in descriptor** |
| Background/pad colour | ImageOnline | `background` (color) | **in-model — in descriptor** |
| Crop anchor (center/top/bottom) | ImageOnline (buttons) | `gravity` enum | **in-model — in descriptor** |
| Multiple input formats (PNG/JPG/WebP/…) | all | ffmpeg decodes them; output keeps input format | **in-model** |
| Lanczos scaling | ImageOnline | ffmpeg scale defaults to a high-quality filter | **in-model (implicit)** |
| **Batch export / download-all every size as ZIP ("at once")** | FreeImgKit, ImageOnline | — | **OUT OF MODEL:** a gizza tool emits ONE output file; multi-file/ZIP batch needs a server/multi-output. Users re-run per target (example chips make this one click). Listed, not built. |
| Interactive drag-to-crop canvas | ImageOnline, Mixpost | — | **OUT OF MODEL:** the generated page has no bespoke canvas editor; `gravity` covers the common crop-position need. |
| Per-format output choice (PNG/JPG/WebP) + quality slider | ImageOnline | output keeps input format | **considered, rejected:** cross-format quality control in ffmpeg is format-dependent (PNG lossless vs JPEG/WebP qscale) and adds schema noise; the family (pin-image-resizer, image-cover-fit) keeps input format. Change the input format to change the output format. |

## Design (descriptor) — in-model table-stakes baked in from the start

`target` (enumv, default `instagram-square`): instagram-square 1080×1080,
instagram-portrait 1080×1350, instagram-story 1080×1920, facebook-post 1200×630,
facebook-cover 820×312, twitter-post 1600×900, twitter-header 1500×500,
linkedin-post 1200×627, linkedin-cover 1584×396, youtube-thumbnail 1280×720,
pinterest-pin 1000×1500, tiktok-video 1080×1920.
`fit` (cover/contain/stretch, default cover) · `gravity` (center/top/bottom, default
center) · `background` (color, default #ffffff, contain only).

## Distinct from existing blocks

- `pin-image-resizer` — Pinterest-only (4 pin formats). Ours is multi-platform (7
  platforms, 12 targets). Not a dup.
- `image-cover-fit` / `image-contain-fit` — generic cover/contain crop to arbitrary
  WxH, no platform presets. Ours ships named platform targets. Not a dup.
