# png-chunk-stripper — competitor analysis (2026-07-22)

Scan of the real tools that losslessly remove PNG ancillary chunks. Paraphrased only —
no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **pngcrush** (`-rem` / `-keep` options) — the canonical CLI. Removes ancillary chunks by
   4-character name (`-rem gAMA`, `-rem tEXt`) or by category keyword: `alla` = all ancillary
   chunks except transparency (`tRNS`); `allb` = all ancillary including `tRNS`; `text` = the
   textual chunks (`tEXt`/`zTXt`/`iTXt`). A companion `-keep <name>` force-preserves a chunk even
   when a category rule would drop it. Never touches critical chunks (IHDR/PLTE/IDAT/IEND).
2. **ImageMagick `-strip`** — blunt one-shot: drops profiles/comments but (per a long-standing
   issue) leaves `gAMA`, `tIME`, and `tEXt` behind, so it is not a complete ancillary strip. No
   selectivity, no report of what was removed.
3. **OptimizePNG "PNG Metadata Remover"** (web) — checkbox-style privacy remover advertising
   "strip EXIF / XMP / comments", keeps the raster untouched, shows before/after size. Positions
   the color profile (`iCCP`) as an optional keep so on-screen colors don't shift.
4. **exifscrubber "PNG chunk manipulation"** (article + local app) — privacy angle: enumerates
   `eXIf`/`tEXt`/`iTXt`/`zTXt` as the personal-data carriers and re-serializes the chunk stream
   with those removed, leaving IHDR/IDAT/IEND byte-identical.

## Table-stakes params / behaviour (each tagged in-model / out-of-model)

| Capability | Competitor precedent | In-model? | Decision |
|---|---|---|---|
| Byte-level chunk surgery — IDAT/pixels never re-encoded | pngcrush, exifscrubber | in-model | core walks the chunk stream directly (no pixel decode); IDAT copied verbatim |
| Preset scope of removal (all vs metadata vs text-only) | pngcrush `alla`/`text`, ImageMagick | in-model | `mode` enum: `all` \| `metadata` \| `text` |
| Force-keep specific chunk types | pngcrush `-keep` | in-model | `keep` comma-list of 4-char types, overrides `mode` |
| Never drop critical chunks (IHDR/PLTE/IDAT/IEND) | all | in-model | always preserved |
| Preserve transparency (`tRNS`) so displayed pixels are unchanged | pngcrush `alla` keeps `tRNS` | in-model | `tRNS` always kept (removing it would alter visible pixels) |
| Keep color-management (`gAMA`/`cHRM`/`sRGB`/`iCCP`) so colors don't shift | OptimizePNG keep-profile | in-model | `metadata` mode keeps them; `all` drops them (documented appearance caveat) |
| Report what was removed + bytes saved | pngcrush verbose, OptimizePNG size delta | in-model | `for_llm` summary lists removed chunk types, counts, and byte savings |
| Reject non-PNG / corrupt input with a clear message | all | in-model | PNG-signature + chunk-walk validation with actionable errors |
| Add/rewrite chunks (insert gAMA, bit-depth change) | pngcrush | out-of-model | out of scope — this tool only removes, never edits pixels or adds chunks |
| Full re-compression / palette reduction | pngcrush, png-optimizer | out-of-model | covered by the existing `png-optimizer` block; this tool keeps IDAT byte-identical |

## Design descriptor (all in-model table-stakes included)

Pure-Rust, byte-level (no `png`/`image` decode — the chunk stream is parsed directly), so pixels
are provably untouched. Surfaces: **chat + CLI, no page** (pure image-bytes output has no page
render mode — same shape as `png-optimizer`, `gif-from-images`, `image-color-quantize`).

Params:
- `mode` — enum `all` (default, strip every ancillary chunk incl. color hints; smallest file) /
  `metadata` (strip text + timestamps + EXIF/XMP, keep color-management + physical-dimension chunks
  so appearance and DPI are preserved) / `text` (strip only text + EXIF/XMP privacy carriers).
- `keep` — comma-separated 4-character chunk types to always preserve, overriding `mode`
  (e.g. `iCCP,pHYs`). Case-sensitive PNG chunk names.

Always preserved regardless: the critical chunks IHDR/PLTE/IDAT/IEND and the transparency chunk
`tRNS` (dropping `tRNS` would change visible pixels, breaking the lossless guarantee).

Out-of-model items above are listed, not built.
