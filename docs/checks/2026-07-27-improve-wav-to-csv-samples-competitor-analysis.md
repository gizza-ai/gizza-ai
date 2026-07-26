# wav-to-csv-samples — competitor analysis (2026-07-27)

Paraphrased research only — no competitor copy, branding, or trademarks reproduced.
Goal: export decoded PCM samples from an uncompressed WAV to CSV, one column per
channel plus a time/index column.

## Competitors surveyed

1. **Audacity "Sample Data Export"** (desktop, the de-facto reference) — the most
   feature-complete take on this exact task.
2. **ConvertFiles WAV→CSV** (online converter) — positions the output as numeric CSV
   for ML/data workflows; advertises PCM 16/24/32-bit + IEEE float, mono/stereo/
   multichannel support.
3. **ConvertHelper WAV→CSV** (online converter) — minimal "upload → download CSV", no
   documented options.
4. **Lukious/wav-to-csv & kardantel/wav-to-csv** (GitHub, Python/librosa) — read WAV,
   dump samples (and sometimes rows-per-file) to CSV; no options beyond file in/out.
5. **Transcriptly WAV→CSV** — out of scope: it exports *transcripts* to CSV (an ASR
   product), not raw PCM samples. Named only to disambiguate the search space.

So there are really ~2 substantive references (Audacity + the ConvertFiles feature
list); the rest are file-in/file-out with no knobs.

## Table-stakes options (from Audacity Sample Data Export) + our decision

| Option (Audacity)            | Choices / default                                   | In our tool? |
|------------------------------|-----------------------------------------------------|--------------|
| Limit output to first N      | default 100, cap 1,000,000 samples                  | `max_frames` (default 100000, cap 500000) + `start_frame` window |
| Measurement scale            | linear (±1) **default** / dB                        | `value_scale` = float **default** / db / int |
| Index column (text only)     | none **default** / sample count / time indexed      | `index_column` = time **default** / sample / both / none |
| Output file format           | TXT **default** / CSV / HTML                         | CSV/TSV via `delimiter` (comma **default** / semicolon / tab); HTML out-of-model |
| Header information            | none / minimal / standard / all                     | `header` bool (column-name row). Metadata-block header **rejected** — breaks CSV import |
| Stereo channel layout        | L-R same line **default** / alternate lines / L-first| Fixed "same line" = one column per channel (matches the tool's contract) |
| Raw integer PCM (ConvertFiles)| 16/24/32-bit integer values                        | `value_scale=int` emits the raw integer at the source bit depth |

## Params shipped (in-model)

- `input` (required) — WAV bytes as base64 or hex (paste; generic page has no file picker).
- `input_format` — `base64` (default) / `hex`.
- `value_scale` — `float` (normalized ±1, default) / `int` (raw PCM integer at source bit
  depth; float-WAV sources scale to 32-bit) / `db` (dBFS magnitude).
- `precision` — decimals for float/db values (default 6, 0–15).
- `index_column` — `time` (seconds, default) / `sample` (absolute frame index) / `both` / `none`.
- `delimiter` — `comma` (default) / `semicolon` / `tab`.
- `header` — include the column-name header row (default true).
- `start_frame` — first sample frame to export (default 0) — windowing for large clips.
- `max_frames` — cap on exported frames (default 100000, max 500000) — Audacity's "limit
  output to first N samples", made an explicit documented contract (no silent truncation).

## Worked example

A 16 kHz mono 16-bit WAV whose first three sample frames decode to normalized
`0.5, -0.25, 0.0` exports (defaults: time index, float, comma, header) as:

```
time_s,channel_1
0.000000,0.500000
0.000063,-0.250000
0.000125,0.000000
```

Switching `value_scale=int` (16-bit source) yields `16384, -8192, 0`; `index_column=both`
prepends a `sample` column (`0,1,2`).

## Out-of-model / rejected (documented, not built)

- **File upload / drag-drop** — the generic page uses pasted base64/hex text; a browser
  file picker for pure-Rust page tools isn't in the platform. `base64 clip.wav` → paste.
- **HTML/TXT output formats** — CSV/TSV is the tool's contract; HTML tables are a
  separate concern. (Rejected as scope creep.)
- **Metadata header block** (Audacity "standard/all") — peak/RMS/DC-offset/filename lines
  before the data break spreadsheet CSV import; rejected. A dedicated audio-stats tool
  (`speech-audio-quality-checker`) already reports those.
- **Alternate stereo layouts** (alternate lines / L-first) — the tool's defining contract
  is one column per channel; alternate row layouts contradict it. Not built.
- **Compressed inputs (MP3/AAC/FLAC/Ogg/Opus, A-law/mu-law)** — decoding needs codecs
  outside the pure-Rust model; rejected clearly at parse time with a "convert to WAV first"
  message that names the detected container.

## UX patterns adopted

- Enum controls render as `<select>` (value_scale, input_format, index_column, delimiter);
  header is a checkbox; numeric fields get placeholders.
- Preset "Try:" example chips (defaults, raw-int, dB) double as worked examples.
- Limits (uncompressed WAV only; max_frames cap; base64 ~33% larger than binary) stated on
  the page, and errors name what was expected.

Sources (paraphrased, not copied): Audacity Manual "Sample Data Export"; ConvertFiles
WAV→CSV; ConvertHelper WAV→CSV; GitHub Lukious/wav-to-csv, kardantel/wav-to-csv.
