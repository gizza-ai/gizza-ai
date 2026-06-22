# gif-reverse — competitor analysis & differentiation

**Tool:** `gizza-ai/gif-reverse` — reverse the playback order of an animated GIF
(plus an optional boomerang / ping-pong mode).
**Date:** 2026-06-22

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `gifsicle` | CLI | Can reverse via frame-index tricks (`#-1--0`), but it's an obscure incantation and a native install. |
| `ffmpeg` `reverse` filter | CLI | `ffmpeg -i in.gif -vf reverse out.gif` works but is heavyweight and the GIF palette/loop handling is fiddly. |
| ezgif.com "reverse GIF" | Web | Popular and easy, but **uploads your GIF to a server**, ad-supported, with free-tier size caps. |
| Other online GIF reversers / "boomerang makers" | Web | Same upload/privacy problem; some only offer reverse OR boomerang, not both. |
| Instagram Boomerang / phone apps | App | Boomerang from camera capture, not from an existing GIF; mobile-only. |

## How gizza's tool is better / different

1. **Pure-Rust, no ffmpeg/gifsicle — runs everywhere.** Decode + re-encode via the
   `image` crate's GIF codec, so it works in the chat Service Worker and the CLI
   (all WASM). The picker tagged it "ffmpeg"; it needs none.
2. **Local — your GIF is never uploaded.** The privacy win over every web reverser.
3. **Two modes in one call.** Plain reverse (last frame first) and `boomerang=true`
   (forward then backward, ping-pong) for a seamless loop — the turnaround frame is
   not duplicated, so there's no stutter.
4. **Timing-faithful.** Each frame keeps its own per-frame delay when re-ordered, so
   the reversed/boomerang GIF plays at the original speed; the output always loops
   infinitely.
5. **Reports the result.** Output states frames in→out, dimensions, and before→after
   byte size.

## Verification

Core unit tests assert the frame **order** is actually reversed (first output
pixel == last input pixel; last output == first input) on a 4-frame synthetic GIF,
that boomerang of an N=3 GIF yields exactly `2N-1=5` frames in `[2,1,0,1,2]` order,
the single-frame edge case, and error paths (empty / non-GIF bytes). The chat block
**instantiates and validates** under `wafer build`.
**End-to-end CLI** on a real 12-frame 256×256 animated GIF: plain reverse →
`12→12 frames` valid `GIF89a`; `boomerang=true` → `12→23 frames` (2N-1) valid
`GIF89a`. A separate 7-frame GIF reversed `7→7`.

## Surfaces & honest scope

- **Chat + CLI only — no web page.** GIF-bytes output doesn't fit the page model
  (same as `gif-optimize` / `gif-from-images`).
- Reverse operates on the **fully-composited** RGBA frames the decoder produces, so
  GIFs that use partial-frame disposal/transparency optimizations are re-encoded as
  full frames; the visual result is correct, file size may differ from the source.

## Possible future enhancements

- A `loop_count` option (finite loops instead of always-infinite).
- A `speed` multiplier applied to the per-frame delays while reversing.
- Boomerang variants (hold the end frames; backward-then-forward ordering).
