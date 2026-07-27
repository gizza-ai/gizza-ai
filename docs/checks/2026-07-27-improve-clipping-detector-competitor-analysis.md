# clipping-detector competitor analysis (2026-07-27)

Goal: scan decoded PCM for digital clipping and report counts, timestamps, and worst consecutive clipped regions.

## Scan

| Reference | Table-stakes observed | In model? | Decision |
| --- | --- | --- | --- |
| Audacity clipping detection / "show clipping" workflows | Full-scale sample detection, common 3-consecutive-sample rule, visual emphasis on where clipping occurs. | Yes | `threshold`, `min_run`, region grouping, start/end timestamps, and longest-run reporting are implemented. |
| Audio repair/mastering tools | Peak level in dBFS, clipped sample counts, worst areas, adjustable sensitivity. | Yes | Report includes peak and peak dBFS, clipped sample/frame percentages, total regions, and worst regions ranked by run length then peak. |
| Online loudness/quality analyzers | Upload audio files directly, decode many formats, show waveforms and batch reports. | Partial | Direct compressed-audio decoding and waveform UI are out-of-model for this pure text tool. It accepts base64/hex uncompressed WAV bytes and rejects compressed formats clearly. |

## Parameter decisions

- `input`: in-model. Pasted WAV bytes keep the tool pure and deterministic across chat, CLI, and page surfaces.
- `input_format`: in-model enum (`base64`, `hex`) because diagnostic clips are commonly copied from scripts in either form.
- `output`: in-model enum (`report`, `json`) for human review or automation.
- `threshold`: in-model number, default `0.99`, range `0.5..1.0` for exact clipping and near-clipping scans.
- `min_run`: in-model integer, default `1`; users can set `3` for Audacity-style consecutive clipping.
- `gap`: in-model integer, default `0`; bridges fragmented bursts without adding waveform UI.
- `top_regions`: in-model integer, default `5`; keeps reports short while still surfacing the worst spots.

## Out-of-model / deferred

- MP3/AAC/FLAC/Ogg decoding: would require either ffmpeg media plumbing or codec crates and a file upload surface; this v1 stays pure and supports uncompressed WAV.
- Waveform visualization and repair: separate UI/tool scope; this tool reports exact timestamps and counts.
- Full-file batch analysis: outside the single-call tool model.

## Verification plan

- Core unit tests build minimal WAV files and assert clipped counts, timestamp ranges, JSON ordering, stereo counting, base64/hex decoding, and compressed-format errors.
- CLI exact-output test uses the sample base64 WAV and asserts the report contains `Clipped:       3 of 8 samples` and `0:00.250 - 0:00.625`.
- Page test uses the same sample literal, asserts exact report text, JSON deep-link behavior, and generated form prefill.
