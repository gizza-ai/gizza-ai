# woff2-convert — competitor analysis (2026-07-20)

Tool function: convert a font file between the four common desktop/web container
formats — **TTF, OTF, WOFF, WOFF2** — auto-detecting the input format. Browser-local /
wasm, no upload, no account. Surface: chat + CLI (binary file in, binary font file out —
no page, same shape as pdf-rotate / detect-file-type).

All findings below are **paraphrased** from public tool pages — no competitor copy,
branding, or trademarks reproduced.

## Competitors skimmed

1. **Fontsource converter** (closest analog — explicitly client-side / in-browser).
   - Input: TTF, OTF, WOFF, WOFF2. Output: TTF/OTF, WOFF, WOFF2.
   - Processing entirely in the browser, no server upload (privacy angle).
   - UI: drag-and-drop file area + output-format checkboxes + a convert action.
   - No subsetting / hinting / CSS extras surfaced.

2. **Transfonter** (full web-font *generator*, not just a converter).
   - Input: a very wide set — TTF, OTF, WOFF, WOFF2, plus legacy/niche (SVG, TTC,
     DFONT, CFF, PFA/PFB/PS Type1, SFD, …). Output: TTF, EOT, WOFF, WOFF2, SVG.
   - Options: family-name control, vertical-metrics tweak, base64 embedding, hinting
     mode (keep / autohint / strip), character subsetting by language/Unicode range,
     `font-display` selection, and a generated demo page + @font-face CSS.
   - Stated cap: max file size 5 MB.

3. **CloudConvert** (OTF↔WOFF2 etc.) — general server-side conversion service.
   - Input/output: the four common formats and many others; runs on their servers.
   - Account / quota model; conversion happens server-side (not browser-local).
   - Positions on "free & fast online conversion", broad format matrix.

(Other reachable tools — ttfconverter.com, AnyConv, font-converters.com, RouteNote —
offer the same four-format core; font-converters advertises Brotli WOFF2 compression +
auto @font-face + batch. Nothing new beyond the three profiled above.)

## Table-stakes → decision

| Table-stake | In/out-of-model | Where it lands |
| --- | --- | --- |
| Input TTF, OTF, WOFF, WOFF2 (auto-detect) | in-model | magic-byte detection; `wuff` decodes WOFF/WOFF2 (glyf **and** CFF) → SFNT |
| Output WOFF2 | in-model | `ttf2woff2` (glyf transform, best Brotli) for TrueType; hand-rolled null-transform WOFF2 (Brotli) for CFF/OTF |
| Output WOFF (v1) | in-model | hand-rolled per-table zlib (`flate2`) WOFF writer |
| Output TTF / OTF (decompress web font → desktop SFNT) | in-model | `wuff` decode → SFNT bytes, outline format preserved |
| Best WOFF2 Brotli compression | in-model | quality 11 (transform path) / quality 11 Brotli (null-transform path) |
| Client-side / privacy (no upload) | in-model | gizza runs browser-local wasm — positioning copy |
| Ready-to-use `@font-face` CSS snippet | in-model (bonus) | family name read from the `name` table → snippet in the result summary |

### Out-of-model / considered, not built (listed, never silently dropped)

- **Glyph re-outlining (true glyf↔CFF conversion).** Real "TTF→OTF" re-renders outlines
  (quadratic↔cubic) and rewrites CFF charstrings — needs a full CFF writer + curve
  converter. Not pure-wasm-feasible here. **TTF↔OTF in this tool is a container
  operation that PRESERVES the outline technology** (a TrueType font stays TrueType, a
  CFF font stays CFF); the `.ttf`/`.otf` choice sets the container/extension. Stated
  plainly on the tool.
- **Subsetting by Unicode range / language.** A separate concern (its own tool); not a
  format conversion. Out of scope.
- **Hinting control (keep / autohint / strip).** Needs a hinting engine; separate tool.
- **EOT and SVG font output.** EOT is IE-only legacy; SVG fonts are deprecated and
  dropped by browsers. Not worth shipping.
- **Legacy/niche INPUT (PS Type1 PFA/PFB, TTC collections, DFONT, SFD).** Each needs its
  own parser; TTC is multi-font. Single-font TTF/OTF/WOFF/WOFF2 covers the real demand.
- **Batch / multi-file conversion.** The chat/CLI model takes a single source per call.
- **Generated demo page / vertical-metrics editor / base64 CSS embedding.** UI-generator
  features of a full web-font suite, not a converter primitive. (A base64 `data:` URL is
  already how the result is returned.)

### UX-control patterns (competitor pages) vs. our surface

Competitor pages use drag-drop + an output-format dropdown/checkboxes. This tool has **no
page** (binary font input can't be pasted as text — same as every pdf-*/epub-* tool), so
those controls don't apply. The single `format` parameter (enum `woff2|woff|ttf|otf`) is
the equivalent choice, exposed via chat + CLI. Best Brotli quality is fixed (competitors
don't expose a quality knob either).

### WOFF2 decode limits noted

- `wuff` reconstructs the glyf-transform (default WOFF2) and CFF WOFF2. WOFF2 files using
  the rarer **hmtx transform** may not decode — handled as a clean error, stated on the
  tool.
- Font collections (TTC/WOFF2-collection) are not supported (single font only).
