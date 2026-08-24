# flac-picture-extractor — competitor analysis (2026-08-20)

Scope: pull the embedded artwork out of a **native FLAC** file's `PICTURE` metadata block
(block type 6) and hand back the image bytes plus everything the block declares about them —
picture type, MIME, description, width/height, colour depth, indexed-colour count.

Scan done BEFORE implementation. All findings are paraphrased from the format spec and from
how the tools behave; **no competitor copy, branding or trademark text is reused** anywhere in
this tool.

## Competitors reviewed (5 real tools)

| # | Tool | What it is | Relevant behaviour |
|---|------|-----------|--------------------|
| 1 | **metaflac** (the reference FLAC CLI, Xiph) | Canonical FLAC metadata editor | `--export-picture-to-file=FILE` writes the artwork bytes out; `--list --block-type=PICTURE` prints the parsed block — type number *and* its spec name, MIME, description, `width x height x depth`, `colors`, and the payload byte length. With several PICTURE blocks it exports the **first** one unless `--block-number` narrows it. Takeaway: the parsed field dump and the raw bytes are both expected output, and multi-picture files need explicit selection. |
| 2 | **ffmpeg** | General media tool | Exposes FLAC artwork as an attached-picture *video stream* (`-map 0:v -c copy cover.jpg`). Reports the stream's real pixel dimensions (it decodes the image header) rather than the numbers the FLAC block declares. Fails unhelpfully when the file has no attached picture. Takeaway: real decoded dimensions are more trustworthy than declared ones — and a "no artwork here" answer should be a clean report, not a crash. |
| 3 | **Mutagen** (Python tagging library) | Library used by most scripted extractors | `FLAC(f).pictures` returns every `Picture` with `.type/.mime/.desc/.width/.height/.depth/.colors/.data`, and it *also* reads pictures that live inside the Vorbis comment as a base64 `METADATA_BLOCK_PICTURE` field, plus the deprecated `COVERART`/`COVERARTMIME` pair. Takeaway: artwork is not always in a native PICTURE block — an extractor that only walks block type 6 misses real files. |
| 4 | **Mp3tag / Kid3** (desktop tag editors) | GUI taggers with cover-art panes | Show artwork as a gallery: one entry per picture with its APIC-style type label ("Front cover", "Back cover", "Artist", …), format, pixel size, and byte size; export the selected one. Both let the user pick by *role* (front cover) rather than by position. Takeaway: selecting by picture **type** is the interaction users actually want; the numeric type must be rendered as its human name. |
| 5 | **Browser "FLAC cover art extractor" utilities** (several near-identical single-page sites) | Upload-and-download art grabbers | Take a file, show a thumbnail, offer a download. Typically grab only the first picture, give no field detail, and silently return nothing for a file whose art is stored in the Vorbis comment or whose MIME is the `-->` URL form. Takeaway: the low bar is "first picture, no explanation" — the gap is honest reporting. |

Field layout taken from the FLAC format specification's `METADATA_BLOCK_PICTURE` (all
integers big-endian, in this order): picture type (32), MIME length + MIME (ASCII), description
length + description (UTF-8), width, height, colour depth in bits-per-pixel, number of colours
for indexed images (0 otherwise), payload length + payload. The type numbers are the ID3v2
APIC table, 0–20. The MIME string `-->` is special: the payload is then a URL to the image,
not the image itself.

## Table stakes → where each one landed

| Capability | Verdict | Where it lands |
|---|---|---|
| Walk native FLAC metadata blocks and find type 6 (metaflac) | in-model | `core::parse` — header is `last-block` bit + 7-bit type + 24-bit length |
| Report picture type number **and** its spec name (metaflac, Mp3tag) | in-model | `picture_type` (number), `picture_type_name` ("Cover (front)"), `picture_type_slug` (`front-cover`) |
| Report MIME, description, declared width/height/depth/colors, payload size (metaflac) | in-model | every field surfaced verbatim in the report |
| Return the artwork bytes themselves (metaflac `--export-picture-to-file`) | in-model | media envelope with the picture's own MIME + a derived filename |
| Select which picture when a file holds several (metaflac `--block-number`) | in-model | `picture_index` (1-based) |
| Select by role — front cover, back cover, artist… (Mp3tag, Kid3) | in-model | `picture_type` enum, all 21 spec values plus `any` |
| Real decoded pixel dimensions, not just the declared ones (ffmpeg) | in-model | hand-rolled PNG/JPEG/GIF/WebP/BMP header sniff → `actual_width/height/format`, with a note when they disagree with the declared values |
| Pictures stored as base64 `METADATA_BLOCK_PICTURE` in the Vorbis comment (Mutagen) | in-model | parsed as a second source, reported with `source` naming where it came from |
| Deprecated `COVERART`/`COVERARTMIME` Vorbis fields (Mutagen) | in-model | parsed as a legacy source, flagged as such in the notes |
| Inventory of *every* picture in the file, not only the returned one (Mp3tag gallery) | in-model | the LLM/UI summary lists all pictures with index, type, MIME and size |
| Clean, explanatory answer for a FLAC with no artwork (ffmpeg's weak point) | in-model | an error naming how many metadata blocks were seen and which kinds |
| Tolerate an ID3v2 tag glued in front of the `fLaC` marker (real files from some taggers) | in-model | the ID3v2 header is skipped before the marker check |
| Handle the `-->` URL MIME form (the browser utilities silently fail here) | in-model | reported explicitly, with the URL, instead of returning bogus bytes |

## Out of model (listed, deliberately not built)

- **Writing / replacing / removing artwork** (metaflac `--import-picture-from`, Mp3tag, Kid3).
  This is a read-only extractor. `audio-metadata-stripper` covers the removal side.
- **Ogg-FLAC and other containers** (ffmpeg). Only the native FLAC stream layout is parsed;
  an Ogg-encapsulated stream is detected and named in the error rather than half-handled.
- **MP3 `APIC` / MP4 `covr` / WMA artwork** (Mp3tag, Kid3, Mutagen). Different containers
  entirely — a separate tool, not this one. Named on the tool's own error text so the gap is
  honest.
- **Re-encoding or resizing the artwork on the way out** (some browser utilities offer this).
  The payload is returned byte-for-byte; `image-convert` / `image-resize` do format work.
- **Batch folder scans and gallery UIs** (Mp3tag, Kid3). The block takes a single `url`⊕`ref`
  source; multi-file batch has no surface in this model.

## Surface decision

File-in → **image-bytes-out**. That is the established no-page shape in this repo
(`dicom-to-image`, `psd-to-png`, `blur-image`): the tool-page generator has no render mode
for a binary-file upload that yields an image, so this ships as **chat + CLI only**, with no
`page/` or `web/` directory and no Playwright spec. The metadata travels in the envelope's
`for_llm` text so chat and the CLI both get the full parsed field dump alongside the bytes.

Input class is `Input::File` + `AssetKind::Any` rather than `Input::Audio`: FLAC is very
commonly served as `application/octet-stream`, which the `audio/*` MIME-class check rejects
(recorded in the toolchain notes), and that would make the tool unusable against ordinary
hosting.
