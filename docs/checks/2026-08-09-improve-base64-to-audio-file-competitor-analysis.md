# Base64 to audio file competitor analysis (2026-08-09)

Backlog tool: `base64-to-audio-file` — decode a Base64 string or `data:` URI back into a downloadable audio file without transcoding.

## Competitor scan

| Competitor | Observed surface | Table-stakes controls and UX | In-model decisions for this tool | Out-of-model / not built |
| --- | --- | --- | --- | --- |
| base64.guru — Base64 to Audio | Single paste field for Base64, decode action, browser preview/download, and metadata such as MIME, extension and size. Per-format pages exist for common audio containers. | Paste Base64, auto-detect audio type, produce playable/downloadable output, clear invalid-Base64 errors. | `data` is the required multiline input. The block strips optional `data:` URI prefixes, decodes bytes, sniffs common audio headers, returns a standard media envelope for CLI/chat downloads, and the page returns an audio `data:` URL. | Duration/sample-rate parsing and rich metadata tables are not included; this block names the container and size summary only. |
| onlinebase64.com — Base64 to Audio | Search snippets describe raw Base64 and `data:` URI input, client-side conversion, common output formats such as MP3/WAV/OGG/M4A/FLAC, preview/download, and browser-local privacy. | Accept full data URIs, tolerate pasted text, auto-detect the format, use predictable output names, and state that nothing is uploaded. | Whitespace, quotes, URL-safe alphabet, and missing padding are tolerated. `filename` defaults to `audio`; `format=auto` sniffs, while explicit formats force MIME/extension for headerless payloads. | Extracting a Base64 blob from arbitrary JSON documents is left to existing extraction tools; this tool decodes one pasted blob. |
| IPVoid — Base64 to MP3 | Minimal page for paste Base64 → preview → download as MP3. | Zero-configuration path and a simple audio result. | Default `format=auto` works with only the required `data` argument. Users can force `format=mp3` when bytes have no sniffable header. | Transcoding a WAV/OGG payload to MP3 is out of model; this tool only reverses Base64. Use audio conversion tools for re-encoding. |
| 8gwifi / generic Base64 converters | Common pattern: paste encoded text, decode locally/online, download or copy result. | Tolerant paste handling and actionable decode errors. | Invalid Base64 errors name the offending character and position; strict mode rejects decoded non-audio bytes by default. | Batch conversion and file-upload-of-text are out of scope for this single-field page. |

## Table-stakes matrix

| Capability | Decision | Notes |
| --- | --- | --- |
| Paste Base64 and get an audio file back | In model | `data` is required. Chat/CLI return the standard media envelope so `--out` writes the decoded file. |
| Accept a `data:` URI | In model | `data:audio/…;base64,` is stripped; declared MIME is reported if it disagrees with sniffed bytes. |
| Whitespace / quote / URL-safe / missing-padding tolerance | In model | Real copied payloads often contain line wraps or URL-safe alphabet variants. |
| Auto-detect common audio containers | In model | WAV, MP3, Ogg, FLAC, MP4/M4A, ADTS AAC, WebM, AIFF, AMR, WMA/ASF and MIDI are sniffed by magic bytes. |
| Explicit format override | In model | `format` enum: auto/mp3/wav/ogg/flac/m4a/aac/webm/aiff/amr/wma/midi/bin. |
| Strict rejection of non-audio bytes | In model | Default `strict=true` prevents saving a PNG/PDF/text blob as an audio file by accident; `strict=false` saves bytes as `.bin`. |
| Browser preview/download | In model, basic | The page outputs a `data:audio/...;base64,...` URL. The generic text page provides copy/download affordances; no custom player code was added. |
| Rich duration/sample-rate/bitrate metadata | Out of model for this pass | Requires per-container parsers; not needed for a safe decode/download block. |
| Re-encoding/transcoding | Out of model | Bytes are not changed. Existing ffmpeg audio tools handle conversion. |
| Batch decoding | Out of model | One payload per run. |

## Defaults and UX choices

- `format=auto` and `strict=true` make the default path safe: paste a real audio blob and get the matching file; paste non-audio and get a clear rejection.
- `filename=audio` keeps downloads predictable while the extension comes from sniffing or the forced format.
- The 32 MiB decoded-byte cap is stated in descriptors and page copy to avoid silent truncation.
- Example chips include a tiny WAV payload, a `data:` URI, and URL-safe unpadded Base64 so the page demonstrates the supported paste shapes.
- Competitor pages informed the controls and error handling only; copy and branding were not reused.
