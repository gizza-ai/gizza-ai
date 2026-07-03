# image-vignette — competitor analysis (2026-07-03)

Two passes. **Build pass:** one WebSearch ("online vignette photo effect tool add
vignette to image adjustable strength"); skimmed Tuxpi, Imagy, IMGCentury, PineTools,
ImageOnline.io. **Improve pass (same day):** 5 parallel read-only research subagents, one
per competitor, each returning a deep profile (params/defaults/ranges, input/output
formats, limits, free-vs-paid, UX patterns, SEO angles). Everything below is paraphrased —
no competitor copy, branding, or trademarks reused.

## Competitor profiles (paraphrased, improve pass)

### 1. Tuxpi — vignette photo effect (tuxpi.com)
- Vignette is a stackable layer inside a general canvas editor, not a standalone page.
- Params: strength slider 0–1 (default 0.75); feather slider 0–1 (default 0.3); on-canvas
  drag/resize/rotate of the mask; ~20 mask shapes (oval default, rects, stars, hearts, …);
  corner-radius slider; **tint color picker (any hex, default #000000)**; export
  width/height with aspect lock; crop-to-border checkbox.
- Output: JPEG/PNG/WebP, incl. transparent-background PNG/WebP variants; user-set export
  resolution; no watermark; free, ad-monetized.
- UX: before/after carousel on the landing page, "try an example image" modal,
  non-destructive effect layers, export as a separate modal.
- SEO: eye-drawing/vintage-lens framing; edge-treatment content cluster (fades, blurred
  borders, focus frames); lomo/retro angle.

### 2. Imagy — vignette effect (imagy.app)
- Params: strength + size sliders (implied 0–100; FAQ recommends 30–50 strength,
  20–40 size for portraits); live preview.
- Batch mode with uniform settings + per-image or ZIP download; paste-to-upload.
- Formats: very broad input (RAW, PSD, SVG, HEIC, JXL, animated GIF/WebP/APNG, even MP4);
  output AVIF/BMP/GIF/JXL/JPG/PNG/SVG/TIFF/WebP. Animated inputs keep animation per-frame.
- WASM browser-local processing pitched as privacy; free tier capped at 4 uses/day across
  their effects family (1 bulk/day); Pro $4.99/mo lifts caps; no watermark on any tier.
- SEO: portrait/cinematic use-cases, recommended-values FAQ (featured-snippet bait),
  related-tools topic cluster.

### 3. IMGCentury — apply vignette (imgcentury.com)
- Params: vignette size + strength sliders (both 0–100, default 50); fixed dark edges only
  (no color/shape/position).
- Explicit Apply button (no live drag preview despite marketing copy); download disabled
  until applied; separate Reset (defaults) vs Clear All (new image) actions.
- Client-side canvas; no stated limits; free, ad-supported, no watermark.
- Copy-quality flaw observed: instructions reference a tab that doesn't exist (templated
  copy) — a reminder to keep our own copy synced to the real UI.

### 4. PineTools — vignette effect (pinetools.com)
- Params: size % (default 50) + strength (default 35) sliders (jQuery UI + numeric box).
- Apply button, drag-drop/paste/URL input; separate bulk variant page with the same params.
- CamanJS canvas processing; no format choice, no color/shape/position; free, ad-monetized,
  dark-mode chrome.

### 5. ImageOnline.io — vignette effect (imageonline.io)
- Params: mode toggle darken|lighten (default darken); intensity slider (default 50%);
  size/spread slider (default 60%); **download-format select PNG|JPG|WebP (default PNG)**.
- Copy-to-clipboard for the result in addition to download; live canvas preview;
  drag-drop + paste + picker input; no reset, no presets, no color/position.
- Client-side canvas at original resolution; free, no watermark, no limits.
- SEO: darken=moody/noir vs lighten=airy/wedding framing; numeric guidance in body copy
  ("40–60% natural"); recipe combos (low intensity + large size = subtle, etc.).

## Gap list and decisions

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Strength slider, subtle → dramatic | all 5 | in-model | `strength` 0–100 (default 40 = ffmpeg's PI/5); **now a range slider** paired with the number box (`kind="slider"`) |
| Darken vs lighten | ImageOnline, Tuxpi | in-model | `mode` enum, **now with platform-labeled options** ("classic dark edges" / "faded, hazy border") |
| **Vignette tint color** | Tuxpi (hex picker, default #000000) | in-model | **BUILT (improve pass):** `color` param (names + #RGB/#RRGGBB/bare/0x hex) → masked-merge chain `color·(1−m) + image·m` where m = the vignette filter applied to a white frame (exact per-pixel attenuation). Black keeps the plain single-filter path and is numerically identical (corner 109 at strength 40 on white, measured both paths). Rejected with guidance when combined with `mode=lighten`. Page control is the shared swatch+hex `kind="color"` |
| **Output format choice** | ImageOnline (PNG/JPG/WebP), Imagy (9 formats), Tuxpi | in-model | **BUILT (improve pass):** `format` enum keep\|png\|jpg\|webp (default keep). Conversions are single-frame (`-frames:v 1`) so animated GIFs convert to their first frame; `keep` preserves animation. Any jpg output pins `-q:v 2` (mjpeg default is visibly lossy). Envelope mime/filename follow the OUTPUT format. WebP verified working in browser @ffmpeg/core |
| Vignette position | Tuxpi (on-canvas drag) | in-model (numeric) | `center_x`/`center_y` percent params, **now sliders**; on-canvas dragging itself would need per-tool JS — not built |
| Preset one-click styles | none of the 5 (!) | in-model | **BUILT:** 6 `[[example]]` chips (Subtle 20 / Classic 40 / Dramatic 80 / Spotlight 100 / Hazy light edges / Sepia fade) — exceeds the field |
| Size/spread slider separate from strength | 4 of 5 (PineTools, Imagy, ImageOnline, IMGCentury) | out-of-model (perf) | ffmpeg `vignette` couples reach+darkness in the one angle; a custom radial mask needs per-pixel expression filters (geq) that are too slow in browser wasm for large photos. Stated honestly under "Limits" on the page |
| Feather/softness control | Tuxpi | out-of-model (same coupling) | the filter's cos⁴ falloff is inherently soft; documented |
| Mask shapes / corner radius / rotation | Tuxpi | out-of-model | needs a canvas editor, not a declarative form page |
| Batch/bulk + ZIP | PineTools, Imagy | out-of-model here | platform-level feature (single-file page infra); listed in PR |
| Copy result to clipboard | ImageOnline | in-model, platform-scoped | belongs in the shared media-output chrome for ALL ffmpeg tools; listed as platform follow-up, not a per-tool bolt-on |
| Transparent-background export | Tuxpi | out-of-model | the vignette path composites in opaque planar RGB/YUV; alpha loss already documented |
| Live preview, free, no signup, paste-to-upload | all 5 | in-model (site-wide) | already platform behavior |

## Design decisions

- Plain path unchanged: `vignette=angle=<rad>:x0=w*<fx>:y0=h*<fy>:mode=<forward|backward>`,
  angle `%.6f`, centers `%.4f` — deterministic across chat/CLI/page.
- Tint chain (single argv token, no spaces):
  `format=gbrp,split=3[img][a][b];[a]lutrgb=r=R:g=G:b=B[cf];[b]lutrgb=r=255:g=255:b=255,format=yuvj444p,vignette=…,format=gbrp[mask];[cf][img][mask]maskedmerge`.
  `lutrgb` constant fills are exact in planar RGB (drawbox was tried and rejected: it blends
  through YUV and shifted 0xB08050 to (164,140,116)); the mask leg round-trips through
  full-range yuvj444p (the vignette filter's format family) so white→255 restores exactly
  and strength 0 stays a no-op. Verified against local ffmpeg 6.1: black tint ≡ plain path
  (corner 109 at 40), red tint 100 → corners exactly (255,0,0), 0xB08050 → exactly
  (176,128,80).
- Color parsing: curated CSS-name table (+ `sepia` (112,66,20), the classic photo tint) and
  #RGB/#RRGGBB/bare/0x hex; digits-only bare hex like `112233` is protected from numeric
  coercion on the page by a shared `tool.js` guard on the color control class (fixes the
  same latent issue for video-aspect-pad's bar color).
- Strength 0 remains a documented no-op; all out-of-range/unknown values get errors naming
  the expected form ("expected X, got Y").
- Pixel behavior pre-measured against local ffmpeg 6.1 and baked into the page's worked
  example and the Playwright bounds (wide margins for browser-ffmpeg drift).

## Verification (improve pass — all run, all green)

- Unit: 28 core tests (strength mapping, mode/color/format parsing incl. aliases + hex
  forms + guidance errors, plain + tint filter exactness, single-token guards, plan argv
  for keep/convert incl. `-q:v 2` and `-frames:v 1` placement) + 3 block tests
  (drift-guard regenerated for the color+format schema, mode + format enum coverage).
- `wafer build` OK (553.4 KiB block.wasm); `wasm-pack` web build OK; manifest re-synced
  from the live descriptor; generator re-rendered (sliders, labeled selects, color swatch,
  6 preset chips); hygiene gate exit 0. No wafer JSON fixtures exist for the ffmpeg family
  (noted, not invented). `solobase build` skipped per throughput rule.
- CLI (native ffmpeg): default → corner (109,109,109) exact; `color=#A52 strength=100` →
  corners exactly (170,85,34), center white; `format=jpg strength=80` → real JPEG,
  corner (3,3,3), envelope renamed `ffffff.jpg`; `format=webp` → real WEBP; lighten+color
  and unknown color → guiding errors, exit 1; `center_x=100 strength=100` → right-middle
  white, left-middle black. Page's generated CLI example copy-paste-runs (args parse;
  graceful HTTP 404 for the placeholder URL).
- Playwright (10 tests): the 3 baseline tests (default darken, ?strength=80 deep-link,
  lighten) + color deep-link `?color=%23ff0000` (corners red, center white), digits-only
  bare hex `112233` via the page (≈(17,34,51) corners — proves the tool.js guard),
  format=jpg (data:image/jpeg + `out.jpg` download + vignette survived conversion),
  format=webp (browser encoder verified), JPEG input end-to-end (jpg in → jpg out,
  near-black corners), preset chips (fields + slider mirror + sepia warm-corner pixel
  asserts), lighten+color page error (`.error` class + names the fix). Plus
  video-aspect-pad + waveform-image suites re-run green (16 tests — the shared tool.js
  change regression-checked against the other color-control tools).

No competitor copy, branding, or trademarks were reused; capability lists above are
paraphrased observations.
