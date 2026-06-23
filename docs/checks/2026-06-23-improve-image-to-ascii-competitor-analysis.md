# image-to-ascii — competitor analysis (2026-06-23)

## Tool
`gizza-ai/image-to-ascii` — converts an image (url/ref) into ASCII / ANSI character
art. Pure-Rust (`image` crate decode + resize + luminance ramp). Surfaces: **chat +
CLI**. No standalone page (image-input + text-report = the F3 no-page file-input
pattern, like `image-color-picker`).

## Surface verification (Phase 1)
- **Chat block:** `wafer build` validates `target/block.wasm` instantiates (1211 KiB). Pure
  Rust, so it runs in the chat Service Worker too.
- **CLI:** `gizza tool image-to-ascii url=… …` returns flat JSON `{art, cols, rows, width,
  height, ramp, color}`. Verified standard/detailed/blocks ramps, `color=true` (24-bit ANSI
  escapes), `invert=true`, custom `charset`, and `brightness`. The Tux logo renders
  recognizably at width 40-60.
- **Drift guard:** `schema_json_matches_authored_chat_schema` unit test pins the LLM-facing
  schema; manifest.json kept in sync.
- No page surface to Playwright (stated, not skipped).

## Competitors surveyed
1. inventivehq.com/tools/developer/ascii-art-generator — resolution, color vs monochrome,
   ANSI/HTML/PNG/TXT export.
2. folge.me/tools/image-to-ascii — multiple character sets, color mode, adjustable width,
   TXT/HTML export.
3. asciiart.eu/image-to-ascii — character count, brightness, contrast, saturation, full ANSI
   color.
4. convertico.com/image-to-ascii — width, monochrome/grayscale/full-color modes, PNG/TXT/HTML.
5. jasperbernaers.com/ASCII-generator — detailed/blocks character sets, TXT/PNG/HTML export.

## Capability diff (in-model = pure compute, fits chat+CLI text output)

| Capability | Competitors | image-to-ascii | Status |
|---|---|---|---|
| Adjustable width / resolution | yes | `width` 1-400 | ✅ have |
| Aspect-ratio correction for tall cells | implicit | `CELL_ASPECT` vertical fix | ✅ have |
| Multiple built-in character ramps | yes | standard / detailed / blocks | ✅ have |
| Custom character set | some | `charset` (dark→light, overrides ramp) | ✅ **added this pass** |
| ANSI truecolor output | yes | `color=true` → 24-bit escapes | ✅ have |
| Invert / light-on-dark | some | `invert=true` | ✅ have |
| Brightness adjustment | asciiart.eu | `brightness` -1.0…1.0 | ✅ **added this pass** |
| Reports output/source dimensions | rare | cols/rows/width/height | ✅ have (extra) |

## Gaps closed this pass
- **Custom `charset`** parameter: users supply their own dark→light ramp (e.g. `" .:oO@"`),
  overriding the built-in ramp. Matches folge.me / asciiart.eu custom-character-set feature.
- **`brightness`** parameter (-1.0…1.0): added to normalized luminance before ramp mapping,
  clamped. Matches asciiart.eu brightness control. Verified darkest/lightest edge cases in
  unit tests + CLI.

## Out-of-model (NOT built — documented, not attempted)
- **PNG / HTML export of the art** (competitors offer download as image / colored HTML span
  grid). gizza tools render text or a single media envelope; a colored-HTML or rasterized-PNG
  output is a different surface and out of scope for this text tool. The ANSI `color` output
  already preserves color for terminals.
- **Contrast / saturation sliders** (asciiart.eu): brightness covers the most common
  adjustment; contrast/saturation are lower-value and were left out to keep the param surface
  small. Could be added later as pure-compute params if demand appears.
- **Interactive in-browser preview / file upload page**: this is a no-page tool (file-input +
  text report); the chat + CLI surfaces are the supported ones.

## Result
All in-model capability and copy gaps vs the top-5 competitors are closed. Tool builds, all
12 unit/drift tests pass, chat block instantiates, CLI verified across every parameter path.
