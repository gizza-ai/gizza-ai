# waveform-image — competitor analysis (2026-07-03)

One WebSearch ("online audio waveform image generator PNG tool"); skimmed the top real
tools: Audioalter waveform-image, lotsofsounds waveform generator, Elysia Tools audio
waveform generator (itself showwavespic-based), plus WaveVisual / audiowaveform.org /
Verbatik for the out-of-model art end of the market.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Custom image size (width × height) | Audioalter (selectable size), lotsofsounds (fixed 1200×300) | in-model | `width`/`height` integer params, default 1200×300 (the social-banner shape lotsofsounds standardized on), 16–4096 / 16–2048 |
| Custom waveform color | Audioalter, WaveVisual | in-model | `color` hex param, default `#4f46e5` (site accent); strict `#RGB`/`#RRGGBB` validation (also guards the filtergraph against injection) |
| Background color / transparent PNG | Audioalter (bg choice), lotsofsounds (transparent for embeds) | in-model | `background` hex param, empty = transparent (showwavespic's native alpha); non-empty renders a `color` source + `overlay` |
| Broad input format support (mp3/wav/flac/ogg…) | Audioalter (mp3, wav, flac, ogg) | in-model | anything ffmpeg decodes (`audio/*` input, incl. m4a/aac/opus) |
| PNG download | all of them | in-model | `out.png`, page `format = "image"` renders `<img>` + download link |
| Stereo/split channel view | (pro tools; showwavespic native option) | in-model | `split_channels` boolean — off = clean mono downmix (one wave), on = one lane per channel |
| Quiet-audio visibility (log/compressed view) | audio editors; showwavespic native `scale` | in-model | `scale` enum lin/sqrt/cbrt/log, default lin |
| Bar / circular / "sound wave art" styles | WaveVisual, audiowaveform.org | out-of-model | showwavespic draws sample lines only; bar/radial rendering would need a custom rasterizer — listed, not built |
| SVG / PDF / MP4 export | WaveVisual, Verbatik | out-of-model | PNG only; animated waveform video is the separate `audiogram` backlog item |
| Spotify/track-URL import | WaveVisual | out-of-model | page is file-upload; CLI/chat accept a public audio URL which covers the linkable case |

## Design decisions

- Single ffmpeg invocation with `showwavespic` (`-frames:v 1 -update 1 out.png`), the exact
  engine the closest competitor (Elysia) uses. Transparent output is showwavespic's native
  RGBA; a background color wraps the wave chain in the ffmpeg-wiki
  `color=c=…:s=WxH[bg];…[wave];[bg][wave]overlay` recipe (size set directly, no scale2ref
  needed since W×H is known).
- Mono downmix (`aformat=channel_layouts=mono`) when `split_channels` is off, so stereo
  files render one clean wave instead of two channels blended on top of each other;
  `split_channels=1` skips the downmix and stacks one lane per channel.
- Colors are strictly validated hex (`#RGB`/`#RRGGBB`, case-insensitive); empty color falls
  back to the default, empty background means transparent. This doubles as filtergraph
  hardening — the color strings are interpolated into `-filter_complex`.
- Dimensions arrive as f64 from the page (empty = 0 = default); they are rounded, then
  range-checked 16–4096 × 16–2048 with guiding error messages.
- `scale` (lin|sqrt|cbrt|log) exposed because quiet recordings render as a near-flat line
  in linear scale — the FAQ points quiet-file users at sqrt/log.
- Family invariants kept: `Input::Audio` descriptor (url⊕ref), 10 MiB in/out caps,
  `-waveform.png` filename suffix, drift-guard schema test, chat surface documented as
  page+CLI only (SW ffmpeg constraint).

## Verification (all run, all green)

- Recipes pre-verified against local ffmpeg before any wasm build: plain/background/split
  graphs each produced RGBA PNGs at the exact requested size; PIL decode confirmed a
  transparent corner + #4f46e5 wave pixels (plain), an opaque black corner + red wave
  (background), and per-channel lanes (split).
- Unit: 12 core tests + 2 block tests (argv exactness for default/bg/split/scale paths,
  hex + dimension validation, injection rejection, inclusive bounds, drift guard,
  `-waveform.png` filename suffix). `wafer build` validates the chat block wasm (545.9 KiB).
- CLI vs the public beep_short.ogg: default run → 1200×300 PNG, transparent corner, wave
  pixels present; `width=640 height=200 color=#ff0000 background=#000000` → 640×200 PNG
  with opaque black corner and 269 red wave pixels; exact-output error cases: `width=4097`
  → "width must be between 16 and 4096 pixels (0 or empty = 1200), got 4097"; `color=red`
  → "color must be a hex color like #4f46e5 or #f00, got \"red\"".
- Playwright (4/4): 320×100 render decoded via canvas — asserts naturalWidth/Height match
  the REQUEST (not the default), corner alpha 0, >100 indigo wave pixels; background run
  asserts opaque black corners, full-canvas opacity and a red wave; named-color run asserts
  the guiding hex error; deep-link `?width=320&height=100&color=%23ff0000&scale=sqrt`
  asserts prefills, then renders red wave pixels at the requested size.
- `python3 scripts/check-tool-hygiene.py waveform-image` → exit 0 (strict per-slug mode).
