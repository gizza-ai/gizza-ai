# cmyk-jpeg-to-rgb — competitor analysis (2026-08-23)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased observations of publicly visible feature sets — no competitor
copy, branding, or trademarks are reproduced or reused anywhere in the tool.

## Scope note — two different tools share the name "CMYK to RGB"

Most search hits are **colour-value** converters (type four CMYK numbers, get an RGB triple).
That is a different tool from this backlog row, which converts an **image file** whose pixels are
stored in a CMYK/YCCK colour space. Only image-file converters were treated as competitors.
(The colour-value shape is already covered in this repo by `blocks/color-format-convert` and
`blocks/css-color-converter`, so it is not a gap.)

## Competitors reviewed

| # | Tool | Reachable | What it offers |
|---|------|-----------|----------------|
| 1 | imageonline.io — CMYK to RGB | yes | Output format buttons PNG / JPG / WebP, JPEG quality slider 10–100, client-side conversion ("no server uploads"), copy-to-clipboard, explanatory sections (what it is, how to convert, CMYK vs RGB, when to convert, output formats) |
| 2 | cmyk2rgb.com | yes | Accepts jpg/jpeg/png/tif/tiff/gif/pdf; outputs JPG / PNG / TIF; **eight target RGB working-space profiles** (sRGB, Adobe RGB 1998, ColorMatch RGB, ECI RGB v2, ECI RGB v2 ICCv4, ProPhoto RGB, PAL/SECAM, a gamut-warning profile); stated 20 MB max file size; no quality setting |
| 3 | sharkfoto.com — CMYK to RGB | yes | Drag-and-drop plus paste-up-to-100-URLs batch mode; output "set to RGB automatically" (no format choice); no quality control; mixed processing (common formats in-browser, "advanced formats" on their server); 5 FAQs (free?, privacy, quality loss, how-to, batch) |

`imgxcolor.com` and `imageconvert.org` both returned HTTP 403 to the fetcher and were replaced by
sharkfoto so that three reachable competitors were actually reviewed, not two.

## Table stakes → in-model / out-of-model

| Table stake | Seen at | Verdict | Where it landed |
|---|---|---|---|
| Choose output format PNG / JPEG / WebP | 1, 2 | **in-model** | `format` param, `Param::enumv("format", ["png","jpeg","webp"])`, default `png` |
| JPEG/WebP quality slider | 1 | **in-model** | `quality` param 1–100, default 90, `kind = "slider"` on the page |
| One-click presets | 1 (format buttons) | **in-model** | three `[[example]]` chips (web PNG, smaller JPEG, print-quality 4:4:4 JPEG) |
| Runs locally / files not uploaded | 1, 3 (3 only partly) | **in-model, already true** | the page runs ffmpeg.wasm in the browser; stated in the hero + FAQ |
| Stated max file size | 2 (20 MB) | **in-model** | 8 MiB input / 8 MiB output cap, stated on the page and in the error text |
| Accept more than JPEG on input | 2 (png/tif/gif/pdf) | **partly in-model** | any format the shared image pipeline accepts (`image/*` → png/jpeg/webp) is accepted; TIFF/GIF/PDF input are **out-of-model** here (the shared `mime_to_ext` image set is png/jpeg/webp, and PDF is not an image input at all) |
| Explanatory CMYK-vs-RGB copy + FAQ | 1, 3 | **in-model** | `content.md`: worked example, limits, 5 `<details>` FAQs |
| Batch / multiple files per run | 3 | **out-of-model** | the page file input is a single upload and the chat/CLI schema is one source per call (see `references/page-patterns.md`) |
| Pick the target RGB working space (Adobe RGB, ProPhoto, ECI, …) | 2 | **out-of-model** | requires an ICC engine. Spiked: ffmpeg's colour-management filters need `--enable-lcms2` and in any case only handle RGB working spaces, not a CMYK source profile; the browser `@ffmpeg/core` build has no lcms2 at all. Output is plain untagged sRGB, which is what browsers assume — stated explicitly on the page rather than silently implied |
| ICC-accurate (SWOP/FOGRA-profiled) CMYK→RGB | 2 (implied by its profile list) | **out-of-model** | same reason; documented on the page as "arithmetic conversion, not a profiled proof". Measured against a Photoshop-profiled reference render of the same image, ffmpeg's conversion is hue-correct with a worst-case per-channel delta of ~33/255 in saturated reds |
| TIFF output | 2 | **out-of-model** | the browser page cannot preview `image/tiff` (the runtime `EXT_MIME` table would render `application/octet-stream`), and the shared image format map is png/jpeg/webp |

## Decisions this scan drove

1. **PNG is the default output**, not JPEG. Every reviewed competitor that offers a choice lists
   PNG first as the lossless option, and print-origin CMYK artwork is usually flat colour and
   text where a lossy re-encode is the wrong default.
2. **Ship a `chroma` param (4:2:0 vs 4:4:4) that no competitor offers.** CMYK sources are
   overwhelmingly print artwork — logos, type, flat colour — where 4:2:0 chroma subsampling
   visibly smears coloured edges. Verified with a real ffmpeg run: `-pix_fmt yuvj444p` produced a
   distinctly larger, full-chroma JPEG (4787 B vs 1983 B on the test fixture) and `ffprobe`
   confirms the pixel format. It only applies to JPEG output; PNG is always full-resolution RGB
   and libwebp always writes 4:2:0, and the page/describe copy says so.
3. **Force `-pix_fmt rgb24` on PNG output.** This is the concrete defect the tool exists to fix
   and was confirmed by a real run: ffmpeg decodes an Adobe YCCK JPEG as `yuva444p` (and a plain
   Adobe CMYK JPEG as `gbrap`), mapping the black/K channel into an alpha slot, so a naive
   transcode writes an **RGBA** PNG carrying a pointless fully-opaque alpha channel. Pinning
   `rgb24` yields a true 24-bit RGB PNG (2606 B vs 2722 B on the Pillow CMYK sample).
4. **Do not ship a `strip_metadata` toggle.** It was on the shortlist, but a real run showed
   `-map_metadata -1` changes nothing for image output: converting a CMYK JPEG carrying Exif, a
   Photoshop 3.0 resource block and XMP produced byte-identical output (1983 B) with and without
   the flag, because ffmpeg's mjpeg muxer already writes no Exif/ICC. Shipping the toggle would
   have been a parameter that lies about doing something.
5. **Report what the input actually was.** No reviewed competitor tells you whether your file was
   really CMYK. The block parses the JPEG's SOF component count and APP14 Adobe transform and
   says so in its result summary (`4-component Adobe YCCK`, `4-component Adobe CMYK`, plain
   3-component RGB/YCbCr, …), so a file that was already RGB is reported as a re-encode instead
   of being passed off as a conversion.

## Not copied

No competitor wording, page structure, tag line, or asset was reused. The observations above are
feature-level notes taken from publicly visible controls and section headings.
