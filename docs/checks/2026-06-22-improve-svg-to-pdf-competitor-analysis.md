# svg-to-pdf — competitor analysis (2026-06-22)

## Tool

`gizza tool svg-to-pdf` — convert an SVG drawing into a single-page PDF document.
Pure-Rust (resvg/usvg rasterizer + tiny-skia + lopdf serializer), so it runs on
every backend including the chat Service Worker. Inputs: `svg` (the full
`<svg>…</svg>` markup), `dpi` (24–1200, default 150), `background` (`white` or a
hex colour). Output: an `application/pdf` data-URL envelope.

Surfaces: **chat + CLI**. No page — document-bytes output has no page render mode
(same as `svg-to-png`, `images-to-pdf`, the QR/chart tools). Verified:
`cargo test --workspace` (9 tests: 8 core + 1 chat-schema drift guard), `wafer
build` (block validates + instantiates, 1424.8 KiB), CLI emits a valid
`%PDF-1.5…%%EOF` document for default / `dpi=300` / `background=#ff0000`, and
rejects non-SVG input with a clear error.

## Competitors surveyed

- **CloudConvert** (cloudconvert.com/svg-to-pdf) — resolution / quality / file-size
  controls; rasterizes vector files; batch.
- **FreeConvert** (freeconvert.com/svg-to-pdf) — batch conversion, files up to large
  sizes, server-side with SSL + auto-delete.
- **Convertio** (convertio.co/svg-pdf) — multiple SVGs → individual PDF pages.
- **SVG Genie** (svggenie.com/tools/svg-to-pdf) — keeps vector quality; lets the user
  pick **page size and orientation**.
- **SVGtoPDF.com** — convert separately or **merge multiple SVGs into one PDF**.
- **LightPDF** (lightpdf.com/svg-to-pdf) — **local in-browser** processing, no upload.
- **PDF24** (tools.pdf24.org/en/svg-to-pdf) — free, no install/config.
- **rsvg-convert** (librsvg CLI) — the closest reference: renders SVG → PNG **or**
  PDF/PS; default **96 DPI**; background via CSS colour (default none/transparent).

## Capability diff & gap ranking (fit-to-model)

| Capability | Competitors | gizza svg-to-pdf | Status |
|---|---|---|---|
| SVG → PDF, page auto-sized to artwork | yes (rsvg "auto-fit") | **yes** — page sized in points to the SVG's intrinsic size (1 px = 72/96 pt), so it prints at real-world size | ✅ at parity |
| Resolution / DPI control | CloudConvert, rsvg (96 default) | **yes** — `dpi` 24–1200, default 150 (we default higher than rsvg's 96 for crisper output) | ✅ ahead of default |
| Background colour | rsvg (CSS colour) | **yes** — `white` (default) or hex `#rgb`/`#rrggbb`; PDF pages are opaque so transparent SVG areas show as this colour | ✅ at parity |
| 100% local, no upload | LightPDF only | **yes** — runs on-device in the chat SW / CLI; nothing leaves the machine | ✅ ahead (most competitors are server-side) |
| Embedded `data:` raster images in the SVG | yes | **yes** — `data:` PNG/JPEG/GIF/WebP hrefs resolve (external file/URL hrefs intentionally not fetched — no SSRF, no fs) | ✅ at parity |
| Vector-preserving output (native PDF paths) | CloudConvert/SVG Genie claim it | **rasterized** — we embed a high-DPI raster, not native vector paths | ⚠️ documented limitation |
| Page-size presets (A4/Letter) + orientation | SVG Genie | auto-size only | ⏭️ out of current model |
| Batch / multi-SVG / merge to one PDF | FreeConvert, Convertio, SVGtoPDF | single input | ⏭️ out of model |
| `<text>` with system fonts | server tools have fonts | shapes/paths exact; text needs an embedded/converted font | ⚠️ documented limitation |

### In-model gaps closed
All in-model core capabilities a single-input local converter should have are
present: DPI control, background colour, faithful real-world page sizing,
embedded-raster support, and clear input validation. The chat skill description and
manifest spell out the rasterize-at-DPI behaviour and the text/font caveat so the
LLM sets expectations correctly. No copy/branding was taken from any competitor.

### Out-of-model / deliberately not built (with reasons)
- **Native-vector PDF output.** A faithful SVG→PDF *vector* translator (gradients,
  clips, filters, blend modes, text shaping + font embedding) is a large
  undertaking; like `svg-to-png` we rasterize with resvg, which renders the SVG
  exactly as a browser would and embeds it losslessly at a chosen DPI. Bumping
  `dpi` (e.g. 300) gives print-crisp output. This is the same trade-off rsvg makes
  when it falls back to raster, and it keeps the tool pure-Rust + SW-safe.
- **Page-size presets (A4/Letter) + orientation.** The current model sizes the page
  1:1 to the artwork (the "auto-fit" behaviour competitors also offer), which is the
  most faithful default. Fixed-paper layout (centre/scale-to-fit on an A4 sheet)
  would be a reasonable future enhancement but is not required for parity and adds
  layout-policy surface area; deferred.
- **Batch / multi-SVG / merge.** Single-input by design. Multi-image→one-PDF is
  already covered by the existing `images-to-pdf` block; a multi-SVG merge would be
  a near-dup of that pipeline.
- **`<text>` with system fonts.** No system font DB ships in wasm; shapes/paths
  render exactly, and the description tells users to convert text to paths for
  pixel-perfect labels (same caveat as `svg-to-png`).

## Conclusion
svg-to-pdf reaches feature parity with mainstream online SVG→PDF converters on the
capabilities that fit a pure-Rust, single-input, on-device tool — and beats most on
privacy (fully local) and default resolution (150 vs rsvg's 96 DPI). The only
material divergence is raster-vs-vector output, an intentional, documented trade-off
shared with the sibling `svg-to-png` tool.
