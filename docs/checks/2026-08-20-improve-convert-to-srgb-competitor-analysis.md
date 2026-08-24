# convert-to-srgb competitor analysis (2026-08-20)

Backlog row: convert an image with an embedded ICC colour profile (Display P3, Adobe RGB, scanner/printer profile, etc.) into plain sRGB for consistent web display.

## Competitors scanned

| Tool / reference | Table-stakes surfaced | In-model decisions for this block |
|---|---|---|
| BrushCue “Convert Image to sRGB” | Upload an image locally, convert colours with ICC profiles, target web-safe sRGB/BT.709, return a downloadable image. Its adjacent Display P3 converter shows the same workflow in reverse and makes the target colour space explicit. | Build the real ICC transform, not just metadata stripping. Output a downloadable PNG with pixels converted through the embedded profile. Keep the tool local/offline. |
| NoFileUpload ICC profile viewer | Detects embedded ICC profile family and explains whether it is sRGB, Display P3, Adobe RGB, ProPhoto, CMYK, or custom. Emphasizes no-upload/local inspection. | Error clearly when no ICC profile is present so users do not mistake a no-op for a conversion. Include profile-byte count in the LLM-facing report. Full tag inspection is out of scope for this converter and belongs to a profile-viewer/checker block. |
| Tectalic “Preparing Images for the Web: Colour Profiles, sRGB and Adobe RGB” | Web-publishing guidance: wide-gamut/print profiles should be converted to sRGB before web upload; preserving the appearance requires colour conversion, not merely assigning or removing a profile. | Default to sRGB PNG output and document that the pixel values are converted, while the output is unprofiled/standard sRGB. |

## Table-stakes matrix

| Capability / UX pattern | In model? | Decision |
|---|---:|---|
| Accept common raster images carrying ICC profiles (JPEG/PNG/WebP/GIF/BMP where the decoder exposes the profile) | Yes | `Input::Image`; `image` decoders read ICC where supported. |
| Apply actual ICC-to-sRGB colour management | Yes | `moxcms` creates an 8-bit RGBA transform from the embedded profile to `ColorProfile::new_srgb()`. |
| Strip or avoid carrying the old source profile into output | Yes | The output is encoded as PNG without embedding the source ICC profile. |
| Local/offline operation | Yes | Pure Rust core; no network except standard gizza URL/ref source resolution. |
| Downloadable image output | Yes for chat/CLI | Returns `image/png` media envelope. Generic pure-WASM file-to-image pages are not available in this repo, so no standalone page is shipped for this binary-output source tool, matching other no-page pure image-source blocks. |
| Choose output format / quality | Deferred | PNG is the safest default for colour-critical conversion; JPEG/WebP quality controls can be added later but are not needed for the first verified surface. |
| Inspect/dump all ICC tags | Out of model for this slug | A profile viewer/checker is a separate diagnostic tool; this converter reports concise conversion stats only. |
| Assign a profile without converting | Out of model for this slug | Assigning profiles can make colours wrong; the row asks for conversion to sRGB. |

## Verification notes

The implementation uses `image` for decoding/PNG encoding and `moxcms` for ICC transforms. It rejects images without embedded ICC profiles, caps decoded pixels, and returns a concise report: dimensions, source ICC byte length, input bytes and output bytes.
