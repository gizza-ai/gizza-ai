# waveform-image — competitor analysis (2026-07-03)

Two passes. **Build-time pass** (kept below for the record): one WebSearch + skim of the
top tools, which set the v1 param set (size, colors, transparent background, split
channels, scale). **Improve pass** (this update): 5 parallel read-only researcher
subagents, one per competitor, each driving/scraping the live tool and returning the
competitor-profile schema. Everything paraphrased; no copy, branding or assets taken.

## The 5 competitors profiled (improve pass)

### 1. Audioalter Waveform Image — audioalter.com/waveform
- Server-side (uploads to their API, 50 MB cap, files deleted after hours) — our
  browser-local angle is the direct counter.
- Params: 6 size presets (all wide 8:1 strips, default 1920×240) + custom width/height
  (100–4000); wave + background colors via a Chrome-style **rgba picker with alpha
  sliders and checkerboard swatches** (default: opaque blue on fully transparent).
- Input mp3/wav/flac/ogg; output PNG (8-bit RGBA) only. No styles, no channel split, no
  live preview, filename is a server UUID.
- UX: progressive disclosure (preset select + "custom size" checkbox gate), side-by-side
  color swatches, processing-status page, result page with inline preview + share.
- SEO: "your size and colors" pitch, amplitude=loudness mini-education, format list in
  the meta description, privacy/deletion FAQ.

### 2. lotsofsounds Waveform Generator — lotsofsounds.com/tools/waveform-generator
- Browser-local (Web Audio + canvas), strong privacy copy. Fixed 1200×300 PNG out.
- Params: 31 five-stop **gradient presets** (marketing says 32 — off-by-one in their
  bundle), 3 styles (square/rounded bars, outline), 3 layouts (mirror/top/bottom),
  4 densities (80/150/250/400 bars), 4 backgrounds (transparent/white/dark/warm).
- No custom colors/hex, no custom size, no stereo (first channel only, average-of-abs
  buckets — soft transients), no URL state.
- UX: dropzone → controls appear after load; swatch-grid color choice only; live
  re-render on every change; checkerboard for transparency; file chip + remove.
- SEO: use-case-led sections (podcast promos, video thumbnails, blog embeds), FAQ on
  formats/limits/commercial use, footer of per-format-pair tool pages; tool is lead-gen
  for a paid sound-library API.

### 3. Serverless Tools Audio Waveform — serverless.tools/audio-waveform/
- Browser-local (Web Audio + canvas), privacy banner, claims offline-capable.
- Params: 3 styles (bars/mirror/line), rounded-vs-sharp toggle, **gradient mode**
  (fixed hue-rotation of the base color, not a chosen end color), wave + background
  native color pickers (no hex text), transparent-background checkbox, width 400–2400 /
  height 100–800 **sliders**, bar width 1–12, bar gap 0–6.
- RMS per bucket normalized to loudest; channel 0 only; PNG-only export ("Export SVG"
  strings exist in their i18n table but no button — planned/dropped).
- UX: two-pane workspace (280px control column + live canvas), controls grouped into
  Style/Colors/Dimensions cards, sliders with live readouts, metadata chips (duration,
  sample rate, channels), toast on download, 5-language UI.
- SEO: long-form article below the tool (features/use-cases/privacy/FAQ), overlay-for-
  video-editors angle, names the browser APIs as credibility copy.

### 4. WaveVisual — wavevisual.com/audio-waveform-generator
- A full design studio around the waveform: 7 styles (mirrored/one-sided bars, radial,
  circular outline, bezier, dotted), **multi-color segment waves + gradient direction**,
  ~60 background gradient presets, text overlays with fonts/shadows, QR-code-to-audio,
  layers, ~17 canvas ratios incl. print sizes, trimming, mic/Spotify import.
- Exports PNG/JPG/SVG/PDF/MP4/GIF/WebM/ProRes-alpha up to 12000px — but free tier is
  small watermarked PNG/JPG only; paid per-design (~$9) or subscription; video renders
  server-side.
- UX: icon rail of task tabs, first-run "what are you making" size wizard, thumbnail
  style grid, sliders with per-control reset, autosave/undo/share.
- SEO: sound-wave-art-as-gift (weddings, baby's first words), print/DPI guides,
  scannable-QR art, several keyword landing pages onto the same editor.

### 5. AudioWave — audiowaveform.org
- Really an **animated audiogram editor** (800×600 canvas, layers, text/image overlays,
  11 waveform styles incl. circular/neon/orbital): the only working export is
  login-gated watermarked WebM video on a credit system, despite copy promising instant
  image downloads — a credibility gap our instant local PNG attacks directly.
- Accepts video files as audio sources; template gallery; style thumbnails rendered in
  the user's chosen color; native color swatches only; no size/ratio control; no
  privacy messaging at all.
- SEO: heavy "waveform generator free" keyword stacking, style-led examples gallery,
  4-step how-to, product-update blog.

## Gap list → decisions (improve pass)

| Gap (≥1 competitor) | Seen at | Fit | Decision |
|---|---|---|---|
| Gradient-colored wave | lotsofsounds (31 presets), serverless (hue-shift), WaveVisual (multi-color+direction) | in-model | `color2` param: horizontal left→right `gradients`+`alphaextract`+`alphamerge` fill; verified in system ffmpeg (CLI) and @ffmpeg/core (page) |
| Color picker UX (not bare text) | Audioalter (rgba picker), serverless + audiowaveform (native pickers), WaveVisual (swatch rows) | in-model | NEW declarative `kind = "color"` in the shared generator: native swatch two-way mirrored onto the canonical hex TEXT input — keeps empty=transparent/default, alpha hex and comma lists expressible (a bare `<input type=color>` can't say "transparent") |
| Alpha in colors (translucent wave/scrim) | Audioalter (alpha sliders on both colors) | in-model | accept `#RGBA`/`#RRGGBBAA` everywhere (e.g. `background=#00000080`); corner alpha 128 verified end-to-end |
| Per-channel lane colors | WaveVisual (multi-color), pro tools | in-model | `color` accepts a comma list (≤8), joined to showwavespic's pipe syntax; stereo fixture renders red top lane / blue bottom lane |
| Fuller/peakier wave (transients) | their bar styles read "full"; both local competitors average-only (soft transients — their stated weakness) | in-model | `sampling` enum average\|peak (showwavespic `filter=peak`); peak > average wave-pixel count asserted on page + CLI |
| Size/shape presets | Audioalter (6 presets + custom gate), WaveVisual (17 ratios) | in-model | 6 `[[example]]` chips (banner, square post, stereo lanes, sunset gradient, overlay, quiet-voice boost) — presets without a new control |
| Runnable examples everywhere | all (their pages show real values) | in-model | CLI/deep-link samples: `kind=color` sample rule uses the hex placeholder/schema default and OMITS "empty means transparent" fields (fixes the previous `background=transparent — or a hex like #0b1220` non-runnable example) |
| Bar / dotted / circular / neon styles | lotsofsounds, serverless, WaveVisual, audiowaveform | out-of-model | needs a custom rasterizer, not showwavespic; honestly FAQ'd ("Can I make bar-style or circular waveforms?") |
| SVG/PDF vector, video/audiogram export | WaveVisual, audiowaveform | out-of-model | PNG only; animated waveform video is the separate `audiogram` backlog item |
| Text/logo/QR overlay compositing, background images, templates | WaveVisual, audiowaveform | out-of-model | design-studio scope; background *image* would also need a second file input (page framework supports one) |
| Spotify/mic import | WaveVisual | out-of-model | page is file-upload; CLI/chat accept a public URL which covers the linkable case |
| Demo/sample audio to try without a file | WaveVisual | out-of-model (for now) | would mean bundling an audio asset into the page; noted as a possible platform follow-up |

**Bug found by re-verifying deeper (not a competitor gap):** the advertised `#RGB`
short hex (e.g. `#f00`) passed our validation but ffmpeg's color parser only knows
`#RRGGBB[AA]` — it warned and silently drew a **white** wave on every surface. Fix at
root cause: `parse_hex_color` now expands 3/4-digit forms before interpolation
(regression-tested in core, on the page, and via the CLI).

## Design decisions (v1, build-time — still true)

- Single ffmpeg invocation with `showwavespic` (`-frames:v 1 -update 1 out.png`).
  Transparent output is showwavespic's native RGBA; a background color wraps the wave
  chain in the ffmpeg-wiki `color=c=…:s=WxH[bg];…[wave];[bg][wave]overlay` recipe.
- Mono downmix (`aformat=channel_layouts=mono`) when `split_channels` is off;
  `split_channels=1` skips the downmix and stacks one lane per channel.
- Colors strictly validated hex (filtergraph hardening — they are interpolated into
  `-filter_complex`); empty color → default, empty background → transparent.
- Dimensions arrive as f64 (empty = 0 = default), rounded, range-checked 16–4096 ×
  16–2048 with guiding error messages.
- `scale` (lin|sqrt|cbrt|log) exposed for quiet recordings.
- Family invariants: `Input::Audio` descriptor (url⊕ref), 10 MiB caps,
  `-waveform.png` suffix, drift-guard schema test, chat surface = page + CLI only
  (Service-Worker ffmpeg constraint).

## Verification (improve pass — all run, all green)

- Recipes pre-verified against local ffmpeg before any wasm build: gradient
  (`gradients`+`alphaextract`+`alphamerge`), gradient+background overlay,
  gradient+split+peak, per-channel pipe colors on a stereo file, `#RRGGBBAA` scrim
  (corner alpha 128) — and the `#f00` failure mode reproduced (white wave + warning).
- Unit: 22 core tests + 2 block tests (drift-guard REGENERATED for the 2 new params +
  reworded color descriptions; argv exactness for gradient/peak/list paths; expansion,
  alpha, cap and injection cases). `wafer build` validates the chat block (554.3 KiB).
- Generator: 46 tests incl. new `kind=color` control, swatch-default seeding,
  `expand_hex` normalization, and the runnable-CLI-example omit rule.
- Playwright 10/10: the 4 pre-existing cases plus 3-digit-hex regression, gradient
  left-red/right-blue pixel assertion (proves @ffmpeg/core ships the gradient filters),
  stereo per-channel lanes, peak>average pixel count, "Sunset gradient" example chip
  (prefills + re-render + opaque background), swatch→text mirror pick that re-runs.
- CLI vs the public beep_short.ogg: `color=#f00` → 60 red px (was white);
  gradient → left-red, transparent corners; `sampling=peak` → 205 px vs 60 default;
  `background=#00000080` → corner (0,0,0,128); guiding errors for `color2` with a
  color list and for `sampling=rms`.
- `python3 scripts/check-tool-hygiene.py waveform-image` → exit 0 (strict per-slug).
- `npm test` (js unit) 41/41; `solobase build` intentionally skipped (throughput rule);
  no wafer `tests/*.json` fixtures exist for the ffmpeg family (noted, not invented).
