# extract-pdf-images — competitor analysis & improvements (2026-06-20)

**Tool:** `gizza-ai/extract-pdf-images` — extract the embedded raster images from
a PDF and return them as a ZIP. Chat + CLI (no page: a ZIP-of-images output fits
neither the pure-text nor the ffmpeg media page shape — the F3 no-page
file-input pattern, like encrypt-file).

## What competitors do

- **Online "extract images from PDF" sites** (smallpdf, ilovepdf, pdfcandy,
  extractpdf.com, adobe online) — upload a PDF, download a ZIP of images.
  Strengths: handle many colour spaces, often also rasterize pages. Weaknesses:
  the PDF is **uploaded to a server** (privacy + size caps), most cap free use
  (pages/day, watermarks) and queue large files.
- **Poppler `pdfimages`** — the reference CLI. Dumps every image XObject; very
  thorough (handles CMYK, indexed, CCITT, etc.). Requires installing poppler and
  knowing the flags; not browser/agent-friendly.
- **Python (PyMuPDF / pikepdf)** — `page.get_images()` + save. Powerful but needs
  a Python environment and code.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (lopdf + image + zip) compiled
   to wasm, so it runs in the chat Service Worker and headless via the CLI. The
   PDF never leaves the device.
2. **Lossless where it counts.** JPEG (DCTDecode) and JPEG-2000 (JPXDecode)
   streams are written out **byte-for-byte** as `.jpg` / `.jp2` — no re-encode,
   no quality loss. Reconstructed raw images become `.png` (also lossless).
3. **One tidy ZIP** — every recovered image is bundled into a single archive
   with stable `image-NNN.ext` names, so the output is one downloadable file
   (and one `ref` chainable into other tools).
4. **Honest about scope.** It extracts *stored image objects* (it does not
   rasterize/screenshot pages — a different operation), and it reports how many
   images were *skipped* because their filter/colour space isn't reconstructable
   here, instead of emitting corrupt files.
5. **Zero-config & free** — no page/day caps, no watermarks, no account.

## Scope / honest limitations (out-of-model or future work)

- Reconstructs **8-bit DeviceGray / DeviceRGB** Flate/LZW/ASCII85 images to PNG.
  Indexed, CMYK, ICCBased, and 1/2/4/16-bit images are currently **skipped**
  (counted in the summary) rather than mis-coloured — a future improvement could
  apply the palette / convert CMYK→RGB.
- **CCITTFax / JBIG2** bilevel fax images are skipped (would need a fax decoder).
- Soft-mask (`/SMask`) alpha channels are not merged into the PNG.
- Does not rasterize pages — that would need a full PDF renderer (out of model).

These are listed, not built (per the loop's in-model rule). The JPEG/JPEG-2000
passthrough + 8-bit gray/RGB PNG path already covers the most common PDFs.

## Tests

5 core unit tests built on **synthetic PDFs assembled in-test with lopdf**: a
FlateDecode 8-bit gray image → round-trips to a valid 8×4 PNG; a DCTDecode stream
→ extracted bytes are byte-identical to the source JPEG; empty input errors; a
PDF with no images errors; `samples_to_png` rejects non-8-bit / unknown colour
spaces / short buffers. Plus the block drift-guard schema test. CLI verified over
the wire on `somatosensory.pdf` (a real PDF with a DCTDecode image) → a ZIP
containing a valid `image-001.jpg` (27 KB, `FF D8 FF` magic).
