# wav-samples-to-json — competitor analysis (2026-08-07)

Scan run BEFORE implementation, per `.claude/skills/create-next-tool/SKILL.md` step 3.
All findings are paraphrased observations of publicly documented behaviour — no competitor
copy, branding, or trademarks are reproduced or reused.

Backlog row: `wav-samples-to-json` — "Exports PCM samples as a JSON array alongside format
metadata for use in code." Type hint: `pure`.

## Duplicate check

`blocks/wav-to-csv-samples` already decodes WAV → **CSV** (one column per channel, optional
time/sample index, float/int/dB scales, delimiter, header, frame window). It has **no JSON
output and emits no format metadata at all** — its only output is a delimited numeric table.

This tool is the JSON sibling: a machine-readable object carrying the `fmt`-chunk metadata
(sample rate, channels, bit depth, encoding, duration, frame count, byte rate, block align)
*next to* the sample array, in a shape you can paste straight into JS/Python. The repo already
ships format-variant siblings for the same source data (`csv-to-ndjson`, `csv-to-yaml`,
`csv-to-xml`, `csv-to-sql`), so this is a sibling, not a duplicate. Not skiplisted.

## Competitors reviewed

| # | Tool / library | Reachable | What it is |
|---|---|---|---|
| 1 | Zamzar `wav-to-json` (zamzar.com) | yes | Server-side generic file converter, WAV → JSON |
| 2 | PDFMall `wav-to-json` (pdfmall.com) | yes | Server-side generic file converter, WAV → JSON |
| 3 | `wavefile` (npm, rochars/wavefile) | yes | Developer library: read/write WAV, expose fmt + samples |
| 4 | `audiowaveform` (bbc/audiowaveform) | yes | CLI producing waveform **data** files incl. a JSON format |

(Xonvert's `wav-to-json` page returned HTTP 404 and was replaced by `audiowaveform`, per the
"replace an unreachable competitor, don't run with fewer" rule.)

### 1–2. Generic online converters (Zamzar, PDFMall)

Both are upload → pick format → download flows with essentially **no conversion options**:
no sample scale, no windowing, no metadata toggles. Neither page documents the JSON structure
it produces. Zamzar states a **50 MB free-tier file cap**; PDFMall states no numeric limit and
asserts files are discarded after conversion. UX is drag-and-drop plus cloud-storage pickers
(Dropbox / Google Drive), a three-step wizard, and no per-conversion controls. FAQ content is
generic format description (what WAV is, what JSON is) plus privacy/registration assurances.

**Read:** the whole category is under-specified. The differentiator available to us is being
explicit about the output shape and giving real knobs.

### 3. `wavefile` (npm library) — the substantive spec

The most informative competitor, because it is what a developer actually reaches for.

- Exposes the full `fmt` chunk as an object: `audioFormat`, `numChannels`, `sampleRate`,
  `byteRate`, `blockAlign`, `bitsPerSample` (plus extensible fields `cbSize`,
  `validBitsPerSample`, `dwChannelMask`, `subformat`).
- `getSamples(interleaved, OutputType)` — **de-interleaved by default** (one typed array per
  channel), interleaved on request; output container selectable (Float64Array default, also
  Int32Array / Int16Array).
- Supported depths: 8, 16, 24, 32 integer, 32f, 64 float (plus IMA-ADPCM, A-law, mu-law).
- Container/chunk metadata surfaced as plain object properties.

**Read:** the two table stakes here are (a) metadata as named fields, and (b) an
**interleaved vs per-channel layout switch** with a **value-type switch**. Both are in-model.

### 4. `audiowaveform` (BBC CLI)

Produces waveform data files including JSON. Relevant options: `--bits` (8 or 16 output
resolution), `--zoom` (samples per output point, default 256), `--pixels-per-second`
(default 100), `--start` / `--end` windowing, `--output-format`.

**Read:** the load-bearing idea is **decimation** — nobody wants 44 100 raw values per second
in a JSON blob for a waveform preview; they want every Nth frame. That maps cleanly to a
`frame_step` parameter. Its peak-per-bucket reduction is a different tool's job (we already
ship `waveform-image`), but plain striding is one line and covers the common case.

## Table stakes → in-model / out-of-model

| Table stake | Source | Decision |
|---|---|---|
| Format metadata as named fields (sample rate, channels, bit depth, encoding) | wavefile | **in** — `metadata` object in the output |
| Derived metadata: duration, total frames, byte rate, block align | wavefile | **in** — computed from the fmt chunk |
| Interleaved vs per-channel sample layout | wavefile | **in** — `layout` enum |
| Sample value type: normalized float vs raw integer | wavefile | **in** — `value_scale` enum (plus `db`, matching our CSV sibling) |
| Decimal precision for float values | general | **in** — `precision` 0–15 |
| Windowing (start / count) | audiowaveform `--start/--end` | **in** — `start_frame` + `max_frames` |
| Decimation for waveform previews | audiowaveform `--zoom` | **in** — `frame_step` (keep every Nth frame) |
| Pretty vs compact JSON | general JSON-tool convention | **in** — `indent` 0–8 (0 = single line) |
| Metadata-only / samples-only output | wavefile's split API | **in** — `output` enum (`full` / `samples` / `metadata`) |
| Hex as well as base64 input | our CSV sibling | **in** — `input_format` enum |
| Clear rejection of compressed input | wavefile supports more codecs | **in (partial)** — MP3/AAC/Ogg/FLAC/A-law/mu-law are *sniffed and named* in the error, not decoded |
| Peak-per-bucket (min/max) waveform reduction | audiowaveform | **out** — that is a distinct waveform-rendering tool; we already ship `waveform-image`. Striding covers the preview case |
| A-law / mu-law / IMA-ADPCM decoding | wavefile | **out** — companded/ADPCM decoders are out of scope for this block; the error names the format so the user can convert first |
| File upload (drag-and-drop, 50 MB) | Zamzar, PDFMall | **out** — pure blocks take a text param; input is base64/hex text (documented on the page with the exact `base64 clip.wav` command) |
| Cloud-storage pickers (Dropbox / Drive) | Zamzar, PDFMall | **out** — no network/auth surface in a pure block |
| Speech transcription → JSON | speechflow (search result) | **out** — needs an ML model; gizza is pure-Rust + ffmpeg |

Every table stake above lands in the descriptor or in the out-of-model list; none was dropped
silently.

## UX patterns adopted

- **Preset chips** (`[[example]]`) — the converters have no presets, but `audiowaveform`'s
  zoom/bits defaults and wavefile's typed-array choices are effectively presets. Shipping
  chips for: defaults, raw 16-bit integers, per-channel layout, metadata-only, and a
  decimated waveform preview.
- **Slider** for `precision` and `indent` (small bounded ranges).
- `[input.labels]` for friendly `<select>` labels on `output`, `layout`, and `value_scale`.
- `multiline` textarea for the pasted bytes (base64 wraps).
- Placeholders on every text/number field.

## Privacy / positioning

Both online converters upload the file to a server. Ours decodes in-browser via WebAssembly —
stated plainly on the page as a limit-and-capability note, not as a marketing claim.
