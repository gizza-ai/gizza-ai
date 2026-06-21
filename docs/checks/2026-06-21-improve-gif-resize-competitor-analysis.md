# gif-resize — competitor analysis & differentiation

**Tool:** `gizza-ai/gif-resize` — resize an animated GIF to new dimensions while
preserving the loop.
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `gifsicle --resize WxH` / `--resize-width` | CLI | The reference, but a native install and flag-juggling. |
| `ffmpeg -vf scale` | CLI | Heavyweight; preserving the loop + palette is fiddly. |
| ezgif.com "resize" | Web | Popular but **uploads your GIF**, ad-supported, free-tier caps. |
| Image editors | App | Most flatten the animation or are overkill. |

## How gizza's tool is better / different

1. **Pure-Rust, no ffmpeg/gifsicle — runs everywhere** (chat SW + CLI, all WASM).
   The picker tagged it "ffmpeg"; it needs none.
2. **Local — your GIF never leaves the device.** Privacy win over web resizers.
3. **Exact dimensions, with aspect-preserve.** Give `width` and `height` for an
   exact size, or just one and the other is computed to keep the aspect ratio —
   no manual math.
4. **Loop & timing preserved.** Every frame and its delay are kept; the output
   loops forever, just like the source.
5. **Reports the change.** Output states original→new dimensions, frame count,
   and byte size.

## Relationship to gif-optimize

`gif-resize` is the **exact-dimensions** tool ("make it 320 wide"); `gif-optimize`
is the **proportional-scale + frame-drop + color-reduce** shrinker ("make it
smaller, however"). Different mental models and interfaces; both are in the
backlog intentionally.

## Verification

Core tests cover exact dimensions (decode-back confirms WxH and frame count) and
aspect-preserving width-only / height-only resizes. **End-to-end CLI** on a real
256×256, 12-frame GIF with `width=64` produced a valid `GIF89a` with a **64×64
logical screen, 12 frames** retained.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (GIF-bytes output; same as gif-optimize /
  flip-image).
- Exact-WxH (both given) does not letterbox — it stretches to fit, matching
  gifsicle's `--resize` behavior; use a single dimension to preserve aspect.

## Possible future enhancements

- `fit` mode (letterbox to exact WxH preserving aspect).
- Percentage resize alias (overlaps gif-optimize's `scale`).
- Max-dimension cap ("no larger than NxN").
