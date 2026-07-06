# png-optimizer — competitor analysis (2026-07-06)

Tool: **png-optimizer** — losslessly shrinks PNG files by re-encoding with optimal
filters and palette, without changing a single pixel.

Type: pure-Rust image-bytes tool (PNG in → PNG out) via the `png` crate. Surfaces:
**chat + CLI** only (image-bytes output has no page render mode — same shape as
`gif-optimize` / `image-color-quantize` / `normalize-image`).

## Competitors scanned (top 3, all reachable)

1. **oxipng** (Rust CLI, the reference lossless PNG optimizer; rewrite of OptiPNG).
2. **ezgif "Compress PNG"** (browser tool; offers a lossless OxiPNG mode + a separate
   lossy 8-bit palette mode).
3. **Squoosh** (browser; OxiPNG encoder panel with an "effort" control + a lossy
   "reduce palette" toggle).

(Paraphrased only — no competitor copy, branding, or trademarks reproduced.)

## Table-stakes matrix

| Capability | oxipng | ezgif | Squoosh | Decision | Fit |
|---|---|---|---|---|---|
| Lossless guarantee — never change a pixel | yes (default) | lossless mode | lossless base | **CORE PROMISE**: decode→re-encode is bit-exact; unit test asserts pixels unchanged | in-model |
| Optimization level / effort | `-o 0..6`, default `-o 2` | auto | effort slider | `effort` enum `fast\|default\|max` (3 genuinely distinct behaviours) | in-model → descriptor |
| Strip metadata (EXIF/text/gamma) | `--strip safe\|all` | strips | auto | **Inherent**: decode→re-encode drops all ancillary chunks; documented on the tool | in-model (inherent) |
| Palette / bit-depth reduction (RGB→indexed) | yes (auto reductions) | (lossy mode) | reduce-palette (lossy) | `reduce` boolean (default on): **lossless** — RGB→indexed when ≤256 distinct colours, drop fully-opaque alpha, RGB→grayscale when R==G==B | in-model → descriptor |
| Interlace removal (`--interlace 0`) | yes | — | — | **Inherent**: output is always non-interlaced; documented | in-model (inherent) |
| Never enlarge the file | yes | yes | yes | Returns the ORIGINAL bytes if it can't beat them (reports 0% saved) | in-model (inherent) |
| Alpha bleed (`--alpha`) | opt-in, **LOSSY** | — | — | **OUT** — alters fully-transparent pixel RGB; breaks the pixel-perfect promise | out-of-model (lossy) |
| Lossy palette quantization (fewer colours) | no (pngquant) | lossy mode | reduce-palette | **OUT of scope** — that is the existing `image-color-quantize` tool (lossy); point users there | out-of-scope (separate tool) |
| Dithering | — | yes | yes | **OUT** — only relevant to lossy quantization | out-of-model (lossy) |
| APNG frame dropping | — | yes | — | **OUT** — animation; out of scope for a single-image lossless optimizer | out-of-model |
| Multithreading | yes | — | — | N/A — single-threaded wasm runtime; irrelevant to output quality | not applicable |

## Design decisions

Descriptor params (both in-model, both real):
- `effort` — enum `fast` (Fast compression, non-adaptive filter, single pass),
  `default` (Best compression + adaptive per-row filter, single pass),
  `max` (Best compression + brute-force filter sweep: try adaptive and each of the
  5 PNG filter types, keep the smallest). Default `default`.
- `reduce` — boolean, default `true`. Lossless colour-type/palette reduction
  (RGB→indexed ≤256 colours; drop opaque alpha; RGB→grayscale).

Every table-stake lands in the descriptor OR the out-of-model list — nothing dropped
silently. Out-of-model items are LISTED here, not built.

Documented inherent behaviours (stated in the tool summary/skill description, so users
and the LLM know): strips all metadata, de-interlaces, never enlarges, never changes a
pixel; non-PNG input is rejected with a clear error (not silently transcoded).

## Engine feasibility (spiked before tagging)

- `oxipng` itself is NOT wasm-safe for this runtime: it depends on `libdeflater`
  (C, via `libdeflate-sys` build.rs) which won't reliably instantiate under
  `wafer build` (wasm32-wasip1). Rejected.
- `png = "0.17"` (pure Rust, miniz_oxide/fdeflate backend — already used by
  `svg-to-png` and `code-screenshot`) provides `Compression::{Fast,Default,Best}`,
  `FilterType`, `AdaptiveFilterType::Adaptive`, and indexed encode with
  `set_palette`/`set_trns` — everything the in-model params need. Chosen.
</content>
</invoke>
