# audio-bit-depth-converter — competitor analysis (2026-08-23)

Pre-build competitor scan for the new `audio-bit-depth-converter` tool (changes PCM bit
depth, e.g. 24-bit → 16-bit, with proper dithering on down-conversion). Everything below is
**paraphrased** from public tool pages — no competitor copy, branding, or trademarks are
reproduced or reused. Out-of-model items are recorded, not built.

Search: "online audio bit depth converter 24-bit to 16-bit dither WAV" (WebSearch, 2026-08-23).
Top 3 reachable, real tool pages were profiled.

## Competitor profiles

### 1. SoniqTools — Bit Depth Converter (`soniqtools.com/bit-depth`)

| field | observed (paraphrased) |
| --- | --- |
| features | Browser-local bit-depth conversion between 16-bit, 24-bit and 32-bit float; a file queue |
| params/options | Target depth as radio presets (16 / 24 / 32-float); dither: None, TPDF (flagged as the recommended one), noise-shaped/psychoacoustic |
| input formats | WAV, FLAC, AIFF, ALAC (m4a) |
| output formats | Lossless only — WAV, FLAC, AIFF |
| output quality | Lossless targets only; displays the source sample rate but exposes no rate control |
| ux patterns | Drag-and-drop plus click-to-browse, queue table, radio-button depth presets, toggles for the dither method |
| seo/copy angles | CD mastering, shrinking file size, matching a DAW's project depth; explainers on bit depth ↔ dynamic range and why dither is needed; local-processing privacy |
| limits | None stated |
| free vs paid | Free, no account |

### 2. ezyZip — Convert WAV to 16-bit (`ezyzip.com/convert-wav-to-16bit-online.html`)

| field | observed (paraphrased) |
| --- | --- |
| features | WAV bit-depth conversion in the browser via WebAssembly, Web Worker driven |
| params/options | 8-bit, 16-bit, 24-bit, 32-bit float targets; TPDF dithering applied automatically when the depth is reduced (no dither picker) |
| input formats | WAV (the wider site claims 200+ A/V formats) |
| output formats | WAV |
| output quality | Fixed TPDF dither on down-conversion |
| ux patterns | Drag-and-drop, cloud-storage pickers, per-file progress readout, responsive layout |
| seo/copy angles | Cross-platform/browser support, client-side privacy, comparisons against other online converters, "why WebAssembly is faster than uploading" |
| limits | 1 GB per file on the free tier; unlimited conversions; the paid tier lifts the size cap |
| free vs paid | Free with ads; paid Pro tier (no ads, larger files, desktop apps) |

### 3. Elysia Tools — Audio Dither (`elysiatools.com/en/tools/audio-dither`)

| field | observed (paraphrased) |
| --- | --- |
| features | Requantize audio to a chosen sample format with a selectable dither algorithm |
| params/options | Target format s16 / 24-bit / s32; dither: Triangular (default), Rectangular, Shibata, Low Shibata, plus None; a "keep metadata" checkbox |
| input formats | WAV, FLAC, MP3, OGG, OPUS |
| output formats | WAV (PCM) or FLAC |
| output quality | Lossless containers only; no sample-rate control |
| ux patterns | Plain form: upload + two dropdowns + a checkbox. No presets, no preview |
| seo/copy angles | What dithering is and why it protects dynamic range, which method to pick per use case, supported inputs, FLAC output |
| limits | 100 MB per file; uploads deleted after 6 hours (server-side, not browser-local) |
| free vs paid | Free |

## Table-stakes matrix → our design

| table stake | who ships it | verdict | where it lands |
| --- | --- | --- | --- |
| 16-bit target (CD delivery) | all 3 | **in-model** | `bit_depth = "16"` (our default) |
| 24-bit target | all 3 | **in-model** | `bit_depth = "24"` → `pcm_s24le` / FLAC `bits_per_raw_sample 24` |
| 32-bit float target | SoniqTools, ezyZip | **in-model** | `bit_depth = "32f"` → `pcm_f32le` (WAV only) |
| 8-bit target | ezyZip | **in-model** | `bit_depth = "8"` → `pcm_u8` (WAV only) |
| Selectable dither method | SoniqTools (3), Elysia (5) | **in-model, exceeded** | `dither` enum with all 11 swresample methods (none, rectangular, triangular, triangular_hp, lipshitz, f_weighted, modified_e_weighted, improved_e_weighted, shibata, low_shibata, high_shibata) |
| TPDF as the recommended default | SoniqTools, Elysia | **in-model** | `dither` defaults to `triangular` (plain TPDF) |
| Noise-shaped dither option | SoniqTools ("psychoacoustic"), Elysia (shibata) | **in-model** | the shibata / e-weighted / lipshitz entries above |
| No-dither / truncate option | SoniqTools, Elysia | **in-model** | `dither = "none"` |
| WAV output | all 3 | **in-model** | `format = "wav"` (default) |
| FLAC output | SoniqTools, Elysia | **in-model** | `format = "flac"` (16/24-bit only — the FLAC codec has no 8-bit or float mode; we error with an explicit message) |
| Keep/strip metadata | Elysia | **in-model** | `keep_metadata` boolean, default true → `-map_metadata -1` when unchecked |
| Wide decodable input set (mp3/ogg/opus/m4a…) | Elysia, ezyZip | **in-model** | anything ffmpeg decodes; `Input::Audio` + `AssetKind::Audio` |
| Depth presets / one-click targets | SoniqTools (radios) | **in-model** | `[[example]]` preset chips on the page (CD master, noise-shaped, 24-bit FLAC, 32-bit float) |
| Dither only matters when reducing depth | SoniqTools, Elysia copy | **in-model (copy)** | stated on the page + in every `.describe()`; core skips dither for float targets |
| Stated size limit | ezyZip (1 GB), Elysia (100 MB) | **in-model** | 10 MiB in/out, stated on the page (our envelope cap) |
| Browser-local, no upload | SoniqTools, ezyZip | **in-model** | already true here (ffmpeg.wasm on the page) — Elysia actually uploads |

### Considered, rejected (in-model but declined)

- **AIFF output** (SoniqTools) — the muxer exists, but Chrome/Firefox `<audio>` can't preview
  AIFF and the page runtime's extension→MIME table would fall back to
  `application/octet-stream`, so the result would download as an opaque blob. WAV covers the
  same lossless-uncompressed need with a working preview.
- **Sample-rate control alongside depth** (SoniqTools displays the rate) — deliberately kept out
  of this schema; rate conversion is the existing `audio-resampler` tool's job, and mixing the
  two axes into one form makes both harder to reason about. Cross-referenced in the page copy.
- **`dither_scale`** (swresample exposes it) — no competitor surfaces it and it only scales the
  dither noise amplitude; schema bloat for a control almost nobody should touch.

### Out-of-model (not built)

- **Multi-file queue / batch conversion** (SoniqTools, ezyZip) — the page takes a single file
  upload and the chat/CLI surface resolves one source per call.
- **Cloud-storage pickers (Dropbox etc.)** (ezyZip) — needs accounts and third-party APIs.
- **Paid tiers / larger-than-10-MiB files** (ezyZip Pro) — no server, no accounts; the 10 MiB
  envelope cap is a hard property of the block runtime.
- **Reporting the SOURCE file's current bit depth before converting** (SoniqTools shows it) —
  requires probing the decoded stream, which the pure argv builder shared with the page cannot
  do. The backlog already tracks dedicated inspector tools for this.
- **Server-side storage of results** (Elysia keeps uploads for 6 hours) — anti-goal here;
  everything stays in the browser tab.

> Original work only — no competitor copy, branding, or trademarks were copied.
