# image-vignette — competitor analysis (2026-07-03)

One WebSearch ("online vignette photo effect tool add vignette to image adjustable
strength"); skimmed the top real tools: Tuxpi vignette editor, Imagy vignette effect,
IMGCentury apply-vignette, plus PineTools / ImageOnline.io / CapCut & VSCO for the
mobile-app end of the market.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Strength/intensity slider, subtle → dramatic | all of them (Imagy, IMGCentury, VSCO, ImageOnline) | in-model | `strength` number 0–100, default 40; core maps it linearly onto ffmpeg `vignette`'s angle [0, PI/2] — 40 lands exactly on the filter's default PI/5; raw radians never exposed |
| Darken vs lighten (white/faded) vignette | Tuxpi (tint), general photo editors | in-model | `mode` enum darken\|lighten, default darken → ffmpeg `mode=forward|backward` |
| Vignette position / focus point | Tuxpi (position control) | in-model | `center_x`/`center_y` as percent of the image size (0–100, default 50/50) → `x0=w*f:y0=h*f` expressions, resolution-independent |
| Size/spread slider separate from strength | Imagy, IMGCentury ("size" + "strength") | out-of-model | ffmpeg `vignette` couples reach and darkness in the one angle; a separate spread would need a custom radial mask — listed, not built; page copy says so under "Limits" |
| Feather/softness control | Tuxpi | out-of-model | same coupling; the filter's cos⁴ falloff is inherently soft |
| Mask shape (oval/rect/rounded) & tint color | Tuxpi (shapes, tint) | out-of-model | filter is elliptical following the aspect ratio only; colored tints would need an overlay pipeline |
| Live preview + free, no signup | all of them | in-model (site-wide) | page recomputes on field change, in-browser ffmpeg, nothing uploaded |
| Format preserved on download | typical | in-model | `out.<ext>` keeps the input extension; dimensions never change |

## Design decisions

- Single `-vf vignette=angle=<rad>:x0=w*<fx>:y0=h*<fy>:mode=<forward|backward>` token.
  Angle formatted `%.6f`, centers `%.4f` fractions — deterministic across chat/CLI/page.
- Strength 0 is allowed and documented as a no-op (matches competitor sliders that start
  at 0); 40 is the default so the page prefills a classic soft vignette. Out-of-range
  strength/center values get guiding errors naming the 0–100 range.
- Duplicate check: `photo-filter-presets` bundles 9 fixed filter chains — no vignette
  anywhere in its core, and nothing adjustable — so a dedicated parameterized vignette
  tool is not a dup.
- Pixel behavior pre-measured against local ffmpeg 6.1 (white/gray 64×64 fixtures):
  strength 40 → corners ~109–116 with center untouched at 255; 80 → ~2–4; 100 → exactly 0;
  lighten 80 on gray 128 → corners 255, center 128. These numbers are baked into the page's
  worked example and the Playwright bounds (wide margins for browser-ffmpeg drift).
- Alpha loss verified empirically (RGBA in → opaque RGB out via the YUV path) and stated
  under "Limits"; animated GIFs verified to stay animated (6 frames in → 6 out).

## Verification (all run, all green)

- Unit: 16 core tests (strength→angle mapping incl. endpoints/PI-5 default/monotonicity,
  mode parsing + aliases, filter exactness, single-token guard, center validation, plan
  argv/extension) + 2 block tests (drift-guard schema, enum coverage).
- `wafer build` OK (541 KiB block.wasm); `wasm-pack` web build OK; manifest synced from the
  live descriptor; generator rendered the page with number inputs (min/max/prefill) and a
  real `<select>` for mode.
- CLI: default → corner (109,109,109) exact; strength=100 → corner (0,0,0) exact, 300×300
  unchanged; lighten keeps a white corner white where darken-100 sends it to black;
  strength=150 and mode=blur rejected with guiding messages, exit 1.
- Playwright (3 tests, all passing): default darken (dims unchanged + corners darker than
  both center and the input's corners, but not black), `?strength=80` deep-link (field
  prefill asserted + near-black corners prove the param reached ffmpeg), lighten mode via
  the select (gray fixture corners brighten to white, center stays gray).
- Hygiene gate (strict per-slug): exit 0.

No competitor copy, branding, or trademarks were reused; capability lists above are
paraphrased observations.
