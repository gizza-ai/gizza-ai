# svg-recolor — competitor analysis (2026-06-21)

## Surfaces verified
- **Chat block** — `wafer build` validated (`OK gizza-ai/svg-recolor v0.1.0`, instantiates clean).
- **CLI** — `gizza tool svg-recolor` map mode, monochrome mode, and error path all correct.
- **Page** — Playwright `tool-page-svg-recolor.spec.ts`: 3/3 passing (map swap, format-insensitive
  match, monochrome + `none` preserved).

## Competitors surveyed (top 5)
1. **SVG Genie — SVG Color Changer** (svggenie.com) — detects fills/strokes/inline styles, groups
   identical colors so one change updates every match; in-browser, private.
2. **IcoSix — SVG Color Editor** (icosix.com) — recolor icons/logos with live preview, download.
3. **SVGMaker — SVG Color Editor** (svgmaker.io) — per-element fill/stroke/opacity edits + "select
   all elements sharing a color" for batch recolor.
4. **GetIllustrations — SVG Color Editor** (getillustrations.com) — auto-finds all colors including
   gradients and inherited ones.
5. **1000 Free Tools — SVG Color Changer** (1000freetools.com) — scans fill/stroke/stop-color
   (incl. gradient stops), find-and-replace by hex or picker.

## Gap diff (fit-to-model)

| Capability | Competitors | svg-recolor | Status |
|---|---|---|---|
| Recolor `fill` / `stroke` | yes | yes | covered |
| `stop-color` (gradient stops) | some | yes | covered |
| `color` / `flood-color` / `lighting-color` / `solid-color` | rare | yes | **exceeds** |
| Inline `style="fill:…"` | yes | yes | covered |
| Colors inside `<style>` blocks | rare | yes | **exceeds** |
| Format-insensitive matching (#fff = #ffffff = rgb()) | "group identical colors" | yes | covered |
| Named-color matching (`red`, `white`, …) | some | yes (24 names) | covered |
| Batch "recolor everything to one color" (monochrome) | rare | yes | **exceeds** |
| Preserve `none` / `transparent` / `currentColor` | implicit | yes (explicit) | covered |
| Privacy / runs locally (no upload) | yes | yes (wasm, in-browser) | covered |
| Multiple from→to pairs in one pass | yes | yes (comma/newline list) | covered |

## In-model gaps closed this build
- Multi-pair mapping with `=>`/`->`/`:`/`=` separators and comma/newline pair separators.
- Format-insensitive source matching (canonical RGBA keying) — the "group identical colors"
  equivalent: a source `#ffffff` also matches `#fff`, `#ffffffff`, `rgb(255,255,255)`, `white`.
- Full color-property coverage beyond fill/stroke (stop-color, color, flood/lighting/solid-color),
  inline `style=`, and `<style>`-block CSS.
- Monochrome (single-tint) mode — exceeds most competitors.

## Out-of-model (not built — documented, not copied)
- **Live visual SVG preview / color swatches** — the page driver renders `format = "text"`; a
  rendered SVG preview + clickable per-color swatches needs a custom preview surface that the page
  generator doesn't provide. The text output is the supported surface (the recolored SVG can be
  pasted/rendered downstream). Not a capability gap, a UI affordance gap.
- **Auto-detect & list every color in the SVG** — a distinct read-only operation that would change
  the tool's output contract (it returns the recolored SVG, not a color report). Better as a
  separate tool than bolted on here.
- **Gradient `<linearGradient>`/`<radialGradient>` re-authoring beyond `stop-color`** — gradients are
  already handled via their `stop-color` stops; restructuring gradient definitions is out of scope.

No competitor copy, branding, or trademarks were reproduced.
