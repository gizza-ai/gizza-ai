# svg-optimize — competitor analysis (2026-06-21)

Tool: `blocks/svg-optimize` — minify and clean up SVG markup (SVGO-style), pure-Rust,
runs on all surfaces (chat block, CLI, browser page). Conservative by design: path data
and numeric values are **never rewritten**, so the rendered image is identical.

## Surfaces verified

- **Chat block** — `wafer build` validates + instantiates the wasm (306 KiB); schema
  drift-guard unit test passes (no LLM-facing schema drift).
- **CLI** — `gizza tool svg-optimize svg=… [remove_comments=…] [remove_metadata=…]
  [remove_ids=…] [remove_dimensions=…]`; verified strip of XML decl / DOCTYPE / comments
  / `<metadata>` / editor attrs, option toggles, and the non-SVG error path (exit 1).
- **Page** — `/tools/svg-optimize/`, 3 Playwright specs pass (default minify, keep-comments
  off-path, remove-ids/dimensions on-path). Boolean defaults render correctly
  (remove_comments + remove_metadata checked; remove_ids + remove_dimensions unchecked).

## Top competitors surveyed

| Tool | Notes |
|------|-------|
| SVGOMG (jakearchibald / svgomg.net) | The reference SVGO GUI — full per-plugin toggles, live preview, precision slider, file-size readout. |
| svg/svgo (CLI/Node lib) | The upstream engine all the web tools wrap. |
| allsvgicons.com / chromacreator / codeshack / devtoollab SVG optimizers | Browser-based SVGO wrappers; paste-or-upload, minify + clean, copy/download. |

Sources:
- https://jakearchibald.github.io/svgomg/
- https://github.com/svg/svgo
- https://allsvgicons.com/svg-optimizer/
- https://chromacreator.com/svg-optimizer
- https://codeshack.io/svg-optimizer/
- https://devtoollab.com/tools/svg-optimizer

## Capability diff (fit-to-model)

Covered (the safe, lossless core every competitor advertises):

- Collapse whitespace / indentation between and inside tags.
- Remove `<!-- … -->` comments (toggle).
- Strip the `<?xml … ?>` declaration and `<!DOCTYPE …>`.
- Remove editor metadata: `<metadata>`, Inkscape/Sodipodi elements + attributes
  (`inkscape:*`, `sodipodi:*`) and their unused `xmlns` declarations.
- Optional remove `id`/`class` (off by default — they may be referenced).
- Optional remove root `width`/`height` **only when a `viewBox` is present** (responsive),
  guarded so a viewBox-less SVG keeps its dimensions.
- Preserves `<script>`, `<style>` and `<text>`/`<tspan>` contents verbatim.
- 100% local / nothing uploaded (browser wasm) — matches the privacy pitch.

Deliberately **out of scope** (lossy or rendering-risk — these change pixels or rely on a
full SVG DOM + geometry engine, which is intentionally not part of this conservative tool):

- Numeric precision rounding of path/coordinate data (`cleanupNumericValues`,
  precision slider). Rewrites numbers → can shift rendering; explicitly avoided.
- Path data optimization / merging (`convertPathData`, `mergePaths`).
- Collapsing/merging groups, moving group attrs to children (`collapseGroups`,
  `moveGroupAttrsToElems`).
- Converting shapes to paths, converting colors, minifying inline styles.
- Removing "hidden"/off-canvas or default-valued elements (needs geometry/render model).

These are listed, not built — they require a geometry/DOM engine and would make the tool
lossy, which contradicts the "image is unchanged" guarantee. No competitor copy, branding,
or trademarks were reused.

## Outcome

No in-model capability/copy/UX gap left open. The tool ships the full lossless-clean
feature set with sensible safe defaults and three working surfaces.
