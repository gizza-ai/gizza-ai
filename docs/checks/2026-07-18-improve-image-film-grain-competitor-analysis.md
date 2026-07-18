# image-film-grain — competitor analysis (2026-07-18)

Function: overlay analog film grain (per-pixel photographic noise) on a photo, in the
browser via ffmpeg's `noise` filter — a friendly amount 0–100, neutral (monochrome) or
colored grain, optional format convert. Everything runs locally; nothing is uploaded.

## Competitors skimmed (top 5 + others)

1. **Evoto Grain Filter** (evoto.ai) — adjustable intensity, grain *size*, roughness,
   contrast, and color. Frames monochrome (fine, clean film / darkroom) vs coarse colored
   (retro) as the key choice. AI-desktop-editor positioning, sign-in.
2. **AddGrains** (addgrains.com) — precision sliders for grain *intensity*, noise *size*,
   color *tint*, plus vignette, blur, brightness. 100% free, no sign-up, no watermark, no
   limits. The closest pure-web analog to ours; bundles extra adjustments.
3. **Fotor Grain Filter** (fotor.com) — grain as one filter in a broader editor; intensity
   slider, presets, part of a paid suite.
4. **Vayce Film Grain Effect** (vayce.app) — amount, grain *size*, *softness*, and a
   shadows/midtones/highlights *placement* control; monochrome or subtle color.
5. **ElitePX** (elitepx.com) — single 5%→100% strength slider; explicit monochrome toggle
   ("silver-halide") vs off ("multi-colored digital noise"). Almost exactly our model.
6. Others: imageonline.io/add-noise, imagy.app/grain-filter, media.io, insMind, shadcn.io
   jpg-grain-generator — all offer a grayscale-vs-color noise choice + an intensity slider.

## Table-stakes params / defaults / UX (paraphrased)

- **Amount / intensity slider** is the universal primary control (5%–100% at ElitePX,
  0–100-ish elsewhere). → in-model: `amount` 0–100, default 20, rendered as a slider.
- **Monochrome vs colored grain** — ubiquitous (ElitePX, Evoto, Vayce, imageonline, media.io).
  Neutral = silver-halide/B&W film; colored = digital/high-ISO RGB static. → in-model:
  `monochrome` boolean, default true (luma-only `noise=c0s`), false = `noise=alls` all channels.
- **Local / no upload / no watermark** — table stakes for the privacy framing (AddGrains,
  ElitePX). → in-model: ffmpeg-wasm on the page, nothing uploaded, no watermark.
- **Output format / download** — most just re-download; a convert option is a plus. → in-model:
  `format` enum keep|png|jpg|webp (keep default; jpg pinned to `-q:v 2` so grain isn't smeared).
- **Grain *size* / coarseness** (Evoto, AddGrains, Vayce) — clumpy vs fine grain. → OUT of model:
  ffmpeg's `noise` filter is single-pixel; no clump-size knob. Documented as a known limit in copy.
- **Tonal *placement*** (grain only in shadows/midtones/highlights — Vayce) → OUT of model:
  no per-tone masking in the `noise` filter. Documented.
- **Bundled adjustments** (vignette, blur, brightness, contrast — AddGrains, Evoto) → OUT of model
  here by design: single-purpose tool; those are separate gizza tools (image-vignette, etc.).

## Decisions

- Params: `amount` (number 0–100, default 20, slider + presets) + `monochrome` (bool, default
  true) + `format` (enum keep|png|jpg|webp, default keep). Matches the ElitePX/Evoto core model.
- Monochrome path rounds through `yuv444p` and noises luma only (`c0s=<n>:c0f=u`) — neutral
  gray speckle that stays natural over color photos; colored path noises all channels
  (`alls=<n>:allf=u`). Uniform `u` noise + fixed default seed → deterministic across surfaces.
- Presets: Subtle (20), 35mm (40 mono), Heavy (70), Colored ISO (40, monochrome off).
- Copy documents grain-size/coarseness and tonal-placement as out of scope, and the amount
  ladder (5–15 anti-banding, 20–35 natural, 40–70 strong, 80–100 lo-fi). No competitor copy or
  branding reused; paraphrase only.

Sources: evoto.ai, addgrains.com, fotor.com, vayce.app, elitepx.com, imageonline.io/add-noise,
imagy.app/grain-filter, media.io, insmind.com, shadcn.io (paraphrased; no copy reused).
