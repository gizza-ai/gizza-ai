# gif-optimize — competitor analysis & differentiation

**Tool:** `gizza-ai/gif-optimize` — shrink an animated GIF by reducing colors,
frames, or dimensions.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `gifsicle` | CLI | The reference optimizer, but a native install and a wall of flags (`--scale`, `--lossy`, `--colors`, frame selection). |
| `ffmpeg` GIF pipelines | CLI | Heavyweight; palette/scale filter graphs are notoriously fiddly. |
| ezgif.com "optimize GIF" | Web | Popular and capable, but **uploads your GIF to a server**, ad-supported, with size caps on the free tier. |
| Other online GIF compressors | Web | Same upload/privacy problem; quality and options vary. |

## How gizza's tool is better / different

1. **Pure-Rust, no ffmpeg/gifsicle — runs everywhere.** Decode + re-encode via
   the `image` crate's GIF codec, so it works in the chat Service Worker and the
   CLI (all WASM). The picker tagged it "ffmpeg"; it needs none.
2. **Local — your GIF never uploaded.** The privacy win over every web optimizer.
3. **Three independent levers, one call.** `scale` (downsize every frame),
   `frame_step` (drop every Nth frame — with dropped delays rolled forward so the
   animation keeps its duration), and `color_bits` (lossy per-channel color
   reduction). Stack them for big savings.
4. **Reports the win.** Output states frames in→out, new dimensions, and
   before→after byte size.
5. **Sensible defaults.** All-1.0/keep-all/8-bit by default, so you opt into each
   reduction.

## Verification

Core tests cover each lever on real GIF bytes (downscale to exact dims, drop to
3 of 6 frames, posterize a gradient down to ≤5 colors, decode-back checks).
**End-to-end CLI** on a real 12-frame 256×256 animated GIF with
`scale=0.5, frame_step=2, color_bits=5` produced a valid `GIF89a`: **12→6
frames, 256×256→128×128, 7935→3511 bytes (~56% smaller)**.

## Surfaces & honest scope

- **Chat + CLI only — no web page.** GIF-bytes output doesn't fit the page model
  (same as `flip-image` / `gif-from-images`).
- Color reduction is **posterization** (bit-depth masking), not a perceptual
  NeuQuant palette — simple, fast, and predictable; for extreme color
  optimization gifsicle's `--colors` is still the specialist. The encoder always
  builds a ≤256-color palette regardless.

## Possible future enhancements

- True palette quantization (`color_quant`/NeuQuant) for a `max_colors` target.
- Max-width/height cap instead of a scale factor.
- Frame de-duplication (drop identical consecutive frames).
