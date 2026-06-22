# error-level-analysis — competitor analysis (2026-06-22)

Error Level Analysis (ELA) recompresses a JPEG at a known quality and amplifies the
per-pixel difference from the original. Regions with a different compression history
(splices, paste-ins, painted-over areas, added text) stand out brighter than the
uniformly-compressed background, exposing likely edits.

## Surfaces verified

- **Chat / LLM API**: descriptor single-sources the schema; drift-guard test green.
- **CLI**: `gizza tool error-level-analysis url=https://httpbin.org/image/jpeg quality=90 scale=20`
  → valid PNG map; `normalize=true grayscale=true` variant also writes a valid PNG.
- **Page**: none. A pure-Rust image-bytes output has no page render mode (same model as
  `lsb-embed` / `add-text-to-image` / `code-screenshot`). Chat + CLI are the supported surfaces.

## Competitors surveyed (paraphrased — no copy/branding reused)

1. **FotoForensics** — the reference public ELA service. Fixed-ish recompression, colour
   ELA output, pairs ELA with metadata/EXIF and JPEG-quality estimation. Server-side upload.
2. **Scanly ELA Scanner** — adjustable JPEG quality slider (~85–99%) and an amplification
   scale (~3–30×); browser-local processing claim.
3. **Fake-image-detector.org** — ELA plus a learned classifier verdict; upload-based.
4. **Infraredtechno ELA Analyzer** — ELA visualisation with brightness/scale control,
   guidance copy on reading the map.
5. **Infosec / FlatEarth write-ups** — establish the canonical technique: resave at high
   quality (90–95), even bright error = consistent history, localized bright = edited.

## Gap analysis vs our tool

| Capability | Competitors | gizza error-level-analysis | Status |
|---|---|---|---|
| Adjustable recompression quality | yes (85–99) | `quality` 1–100, default 90 | covered (wider range) |
| Amplification / scale control | yes (3–30×) | `scale` 1–100, default 15 | covered (wider range) |
| Auto-contrast / normalize | some | `normalize=true` (stretch peak→255) | covered |
| Grayscale / luminance view | some | `grayscale=true` | covered |
| Colour (per-channel) ELA map | yes | default output | covered |
| Lossless output (no re-JPEG of the map) | varies | always PNG | covered (exact map) |
| Accepts non-JPEG inputs | varies | PNG/WebP/GIF/BMP/JPEG all decoded, then JPEG-recompressed for ELA | covered |
| Browser-local / no upload | claimed by some | yes — pure-Rust wasm, runs in chat SW + CLI, image never leaves device | covered (real, not just claimed) |

## Out-of-model (considered, not built)

- **EXIF / JPEG-quality estimation panel** — a separate concern; gizza already ships
  `image-metadata-viewer` / `strip-exif` for metadata. Kept out to keep this tool single-purpose.
- **Learned forgery classifier / "fake or real" verdict** — needs an ML model; out of gizza's
  pure-Rust + ffmpeg model (the picker defers model tools). ELA is a *visualisation* aid, and
  conflating it with an automated verdict overstates certainty.
- **Side-by-side original vs ELA viewer / heatmap overlay** — a page-UI feature; this tool has
  no page (image-bytes output), so it is N/A here.

## Limitations (documented honestly)

- ELA is a heuristic, not proof: bright regions are *suggestive* of a different compression
  history, not a definitive edit. High-frequency/high-contrast areas naturally show more error.
- Most informative on JPEG inputs that were edited and re-saved; a never-JPEG'd or heavily
  re-compressed image yields a flat map.
