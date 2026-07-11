# dicom-to-image — competitor analysis (2026-07-11)

Tool function: decode a DICOM file's pixel data and render it to a viewable PNG/JPEG,
with adjustable window/level (contrast) — the classic "convert my .dcm scan to a normal
image" job for CT/MR/X-ray/ultrasound.

## Competitors scanned (top 3 + extras)

1. **CoolUtils DICOM→JPG/PNG** (coolutils.com/online/DICOM-to-JPG). Browser upload,
   pick JPG or PNG, "pixel data and contrast are preserved accurately". Batch. No
   explicit window/level UI — uses the file's embedded values.
2. **CT Read DICOM→JPG** (ctread.com/tools/dicom-to-jpg). Local (no upload). Keeps the
   "optimal brightness and contrast settings from the DICOM file" — i.e. reads the
   embedded WindowCenter/WindowWidth and applies them automatically.
3. **X-ray Interpreter DICOM→JPG** (xrayinterpreter.com/dicom-to-jpg). Fully local, for
   CT/MRI/X-ray/ultrasound. Upload → JPG download.
4. **Convertio DCM→PNG/JPG** (convertio.co/dcm-png). Generic converter, 105+ formats
   (also TIFF/PDF/BMP). Cloud upload; no contrast controls.
5. **DICOM Converter (dicomapps.com)**. JPEG/PNG/BMP/TIFF, batch, offline app.

## Table-stakes params / defaults / behaviour

| Capability | Competitors | Our decision |
|---|---|---|
| Output PNG **and** JPG/JPEG | all | **in-model** — `format` enum (png/jpeg), default png |
| Use file's embedded window center/width by default | CT Read, CoolUtils | **in-model** — auto: read (0028,1050)/(0028,1051), else auto-window from pixel min/max |
| Manual window/level (contrast) override | pro DICOM viewers (RadiAnt, Horos); the tool's own description | **in-model** — `window_center` + `window_width` optional numbers |
| Rescale slope/intercept → real HU (CT) | all correct viewers apply it silently | **in-model** — apply (0028,1052)/(0028,1053) before windowing |
| MONOCHROME1 vs MONOCHROME2 (photometric inversion) | all correct viewers | **in-model** — honoured automatically; plus an `invert` toggle |
| Multi-frame: pick a frame | RadiAnt/Horos; some converters export all | **in-model** — `frame` (1-based) selects one frame |
| JPEG quality | Convertio/converters | **in-model** — `quality` 1–100, default 90 |
| Batch / many files at once | CoolUtils, converters | **out-of-model** — one file per call (chat/CLI single-source shape) |
| TIFF / BMP / PDF output | Convertio, dicomapps | **out-of-model** — PNG/JPEG cover the "viewable image" job; TIFF/PDF are format bloat |
| **Compressed** transfer syntaxes (JPEG, JPEG-2000, JPEG-LS, RLE) | pro viewers | **out-of-model (honest limit)** — needs native/JPEG2000 codecs that aren't wasm-safe; we decode **uncompressed** DICOM (Implicit/Explicit VR Little Endian) and return a clear error otherwise |
| Colour (RGB/YBR) DICOM | some viewers | **out-of-model (baseline)** — grayscale (MONOCHROME1/2) is the medical core (CT/MR/X-ray); colour returns a clear error |
| Anonymisation / metadata strip | pro tools | **out-of-model** — separate concern |

## Scoped baseline (honest, matches the description)

Pure-Rust / wasm-safe: a hand-written minimal DICOM parser (no native codecs) + the
`image` crate for PNG/JPEG. Supports the common **uncompressed** transfer syntaxes
(Implicit VR LE `1.2.840.10008.1.2`, Explicit VR LE `1.2.840.10008.1.2.1`), single-sample
**grayscale** MONOCHROME1/MONOCHROME2, 8- or 16-bit, signed or unsigned pixels, rescale
slope/intercept, embedded or manual window/level, and multi-frame selection. Compressed
pixel data (encapsulated JPEG/JPEG2000/JPEG-LS/RLE) and colour images return an explicit,
honest error rather than a wrong picture — table-stakes competitors that "preserve pixel
data accurately" describe exactly this uncompressed windowed-grayscale path.

## UX patterns noted

Sliders for window/level in pro viewers (Horos/RadiAnt); simple converters are upload→pick
format→download. Our surfaces are chat + CLI (image-bytes output → no page, like svg-to-png
/ blur-image), so the controls are the descriptor params; presets (window_center/width) are
documented per modality (brain, lung, bone) in the tool description.

Sources: coolutils.com/online/DICOM-to-JPG, ctread.com/tools/dicom-to-jpg,
convertio.co/dcm-png, dicomapps.com/dicom-converter, xrayinterpreter.com/dicom-to-jpg
