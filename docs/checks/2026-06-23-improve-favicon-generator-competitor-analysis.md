# favicon-generator — competitor analysis (2026-06-23)

## Tool
`blocks/favicon-generator` — turn one source image into a complete favicon
bundle, returned as a single ZIP: a multi-resolution `favicon.ico` (16/32/48 px),
square PNG icons at each requested size (default 16,32,48,64,96,128,192,256,512),
an `apple-touch-icon.png` (180 px), and a `site.webmanifest` referencing the PNGs.
Pure Rust (`image` for decode/resize/ICO + `zip` for the bundle). Input is an
image url/ref (PNG/JPEG/WebP/GIF/BMP).

Surfaces: **chat** (block.wasm validates and instantiates in the wafer runtime —
fully pure Rust, no ffmpeg) + **CLI**. **No standalone page**: the output is a
binary ZIP bundle, which has no page render mode (same F3 pattern as
`image-collage` / `android-asset-generator` / `create-zip`).

## Competitors surveyed
- **RealFaviconGenerator** (realfavicongenerator.net) — the de-facto standard;
  multi-platform package, generates SVG favicon, HTML `<link>` markup + a favicon
  checker.
- **favicon.io** — favicon.ico + PNGs + apple-touch + Android icons + site.webmanifest;
  also generate-from-text/emoji modes.
- **Favic-o-Matic** — .ico + .png, transparent icons, "all sizes" toggles.
- **EasyWebTools** — all essential sizes (16/32/180/192/512), site.webmanifest,
  ready-to-paste HTML snippet.
- **Canva favicon generator** — design-first, image upload + templates.

## Capability diff

| Capability | Competitors | favicon-generator | Status |
|---|---|---|---|
| `favicon.ico` (multi-res 16/32/48) | yes | yes | ✅ parity |
| Square PNGs at standard sizes | yes | yes (configurable list) | ✅ parity |
| `apple-touch-icon.png` (180px) | yes | yes | ✅ parity |
| `site.webmanifest` (PWA) | yes | yes (references PNGs, app name) | ✅ parity |
| Transparent / custom pad background | Favic-o-Matic | yes (`background`, `fit=contain`) | ✅ parity |
| Non-square handling (pad vs crop) | implicit | yes (`fit=contain|cover`) | ✅ parity+ |
| Custom size list | some | yes (`sizes`) | ✅ parity |
| Configurable app/manifest name | yes | yes (`name`) | ✅ parity |
| Single-download bundle (ZIP) | yes | yes | ✅ parity |
| Accepts PNG/JPEG/WebP/GIF/BMP input | yes | yes | ✅ parity |

## Gaps NOT closed (with rationale)
- **SVG favicon output.** RealFaviconGenerator emits an `<svg>` favicon by
  embedding/optimizing a supplied vector. Out of scope here: rasterizing a raster
  input to SVG is image tracing (that is a separate tool, `blocks/vectorize`), and
  passing an SVG through unchanged is a trivial copy, not a favicon-generation step.
  Bitmap PNG/ICO is the universally-supported baseline and is fully covered.
- **HTML `<link>` snippet.** Competitors print the `<link rel>` / `<meta>` tags to
  paste into `<head>`. This is documentation text, not a generated asset; could be
  added as a `README.txt` entry inside the ZIP in a future pass, but it is copy, not
  a capability gap. Deliberately not copying any competitor's exact snippet wording.
- **Generate-from-text / emoji / templates.** favicon.io / Canva let you *design* an
  icon from letters or templates. That is a separate "design an icon" tool, not
  "generate a favicon bundle from an existing image" — out of this tool's scope.
- **Favicon checker / live preview.** A network-scanning validator of a deployed
  site is a different tool class (network probe), out of model for a local bundler.

## Improvements made over a naive first cut
- Added `fit` (contain/cover) + `background` so non-square and transparent sources
  produce clean square icons rather than a stretched aspect ratio.
- Made `sizes` fully configurable (validated, deduped, sorted, 1..=1024) rather than
  a fixed hardcoded set, while keeping a sensible default covering browser + PWA +
  Android Chrome.
- Manifest `name`/`short_name` are user-supplied and JSON-escaped.

## Verification
- `cargo test --workspace`: 8 tests pass (incl. drift-guard schema test + a ZIP
  round-trip asserting the ICO magic `00 00 01 00` and every file present).
- `wafer build`: chat block.wasm validates/instantiates OK (1616.9 KiB).
- CLI: `gizza tool favicon-generator url=… sizes=16,32,48,64,128 name=Gizza`
  produced an 8-file, ~44 KB ZIP; verified PNG sizes are exact squares, the
  `favicon.ico` has the correct ICO magic, and `site.webmanifest` lists the
  requested sizes with the custom name.
- No page surface (binary ZIP output — no page render mode).

Sources: [RealFaviconGenerator](https://realfavicongenerator.net/), [favicon.io](https://favicon.io/), [Favic-o-Matic](https://favicomatic.com/), [EasyWebTools](https://easywebtools.io/favicon-generator/), [Canva](https://www.canva.com/create/favicon-generator/)
