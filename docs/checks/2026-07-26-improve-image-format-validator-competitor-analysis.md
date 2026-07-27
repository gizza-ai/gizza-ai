# image-format-validator — competitor analysis (2026-07-26)

Tool function: verify that an image file's actual bytes match its *claimed* format
(extension / declared type), report dimensions + color depth, and flag corruption —
i.e. a **validator/verifier**, not a passive info reader. Detects "spoofed" files
(a JPEG renamed `.png`) and truncated/corrupt uploads without throwing.

## Competitors skimmed (paraphrased — no copy/branding reproduced)

1. **Convertico — Image Format Detector.** Upload any image; reports the *true* format
   even when the extension is wrong, plus dimensions, file size, bit depth, alpha
   channel, animation status, and a warning next to any extension mismatch.
2. **Snappy-Fix — Image Authenticity Checker.** Checks whether a picture is corrupt or
   renamed by verifying the file signature (magic bytes) against the stated extension,
   plus MIME type, format, and dimensions; reports broken internal data. (Also markets a
   "real vs fake / AI" authenticity angle.)
3. **AnyOnlineTool — Image Validation Tool.** Reports format, dimensions, file size,
   aspect ratio, and corruption detection across JPG/PNG/GIF/WebP/BMP/TIFF/SVG.
4. **InventiveHQ — File Magic Number Checker.** (adjacent) Detects a file's real type by
   its magic bytes to catch spoofed extensions across many file types, not just images.

## Table-stakes → in-model / out-of-model decisions

| Capability | Decision | Where it lands |
|---|---|---|
| Detect true format from magic bytes | in-model | `detected_format` + `detected_mime` (via `image::guess_format`) |
| Compare actual vs claimed format (spoof/mismatch) | **in-model (core differentiator)** | `claimed_format` enum param + filename-extension fallback → `matches_claim` + `claim_source` |
| Corruption / truncation detection | in-model | full `image::load_from_memory` decode → `valid` + `corruption` diagnostic (never throws) |
| Width × height dimensions | in-model | `width`, `height` |
| Color/bit depth + alpha | in-model | `color_type`, `bits_per_pixel`, `channels`, `has_alpha` |
| File size (bytes) | in-model | `bytes` |
| Aspect ratio | in-model | `aspect_ratio` |
| Formats PNG/JPEG/GIF/WebP/BMP/TIFF/ICO | in-model | `image` crate features (matches image-info) |
| SVG validation | **out-of-model** | SVG is XML, not a raster the `image` decoder validates; would need an XML/SVG parser — listed, not built |
| Animation status (animated GIF/WebP) | **out-of-model (deferred)** | reliable frame-count needs per-format `AnimationDecoder` wiring; deferred to keep the verdict robust — noted here, not silently dropped |
| "Real vs fake / AI-generated / EXIF-tamper" authenticity | **out-of-model** | needs an ML model; gizza is pure-Rust + ffmpeg. (EXIF *reading* already exists as `image-metadata-viewer`.) |

## UX controls
All competitors are browser file-upload widgets. This tool follows gizza's **F3
no-page file-input pattern** (pure-Rust image→JSON report, chat + CLI only — the
standalone-page file-input path is ffmpeg-only; same as `image-info`,
`detect-file-type`, `image-metadata-viewer`). So slider/color/preset-chip controls are
N/A. The one meaningful control — choosing the claimed format to check against — is a
fixed-choice `Param::enumv` (`auto`/png/jpeg/gif/webp/bmp/tiff/ico), which the chat UI
renders as a select.

## Distinct from existing gizza blocks
- `image-info` — *passive* reporter (format/dims/color); **errors** on undecodable input
  and never compares against a claimed format. This tool is a **verifier**: it returns a
  structured `valid`/`matches_claim` verdict + corruption diagnostic and never throws.
- `image-metadata-viewer` — EXIF/GPS metadata only. `detect-file-type` /
  `identify-archive-format` — general blob magic-byte typing, not image-specific
  validation with claim-matching, decode-integrity, and depth reporting.
