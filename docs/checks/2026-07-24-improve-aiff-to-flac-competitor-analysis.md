# aiff-to-flac competitor analysis (2026-07-24)

## Competitors checked

Search: `AIFF to FLAC online converter options compression level metadata preserve tags`

1. Audio Transcoder AIFF to FLAC guide — desktop-oriented flow with drag/drop or file picker, format selection, and optional tag editing before conversion.
2. Audio-Convert online converter — broad online audio converter supporting AIFF and FLAC among many formats, with a simple upload → choose output → convert pattern.
3. Online Audio Convert / FreeConvert-style converters — browser upload forms with a target format dropdown, quality/settings controls for some formats, and a downloadable converted file.

## Table-stakes decisions

| Capability / UX pattern | In model? | Decision |
| --- | --- | --- |
| Upload an audio file and produce downloadable FLAC | Yes | Implemented as a file-input ffmpeg page with `format = "audio"` and FLAC output. |
| Accept AIFF/AIF input | Yes | File picker accepts `audio/*`; ffmpeg probes the uploaded bytes, and examples/docs call out AIFF/AIF. |
| Preserve lossless audio samples | Yes | Uses ffmpeg's native FLAC encoder; compression level only changes encode effort/size, not decoded samples. |
| Preserve textual metadata/tags | Yes | Adds `-map_metadata 0` so source container tags are copied into FLAC Vorbis comments where ffmpeg can map them. |
| Compression/quality control | Yes | Exposes FLAC `compression_level` 0–12 with default 5 and a slider/numeric control on the page. |
| Batch conversion / multi-file queue | Out of model | Current gizza page model is one uploaded file per run; documented as a limit. |
| Tag editor UI before conversion | Out of model | The tool preserves source tags but does not provide editable metadata fields. |
| Cloud import/export integrations | Out of model | This public toolkit runs local browser/CLI conversions; third-party storage integrations are not part of the block model. |
| Optional cover-art embedding | Out of model | Audio-only FLAC output intentionally drops attached-picture streams with `-vn` to avoid mux failures. |

## Descriptor/page requirements taken from scan

- Keep a single obvious file upload control for AIFF/AIF/audio inputs.
- Show `compression_level` as the only conversion setting; default 5, accepted 0–12, higher means slower/smaller.
- State that FLAC is lossless and compression level never changes audio fidelity.
- State tag-preservation behavior and cover-art limitation.
- Include examples for default compression and maximum compression.
