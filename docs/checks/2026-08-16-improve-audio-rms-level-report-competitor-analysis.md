# audio-rms-level-report competitor analysis (2026-08-16)

Tool: `audio-rms-level-report` — reports per-channel RMS, sample-peak, and average levels in dBFS as a JSON summary.

## Sources scanned

- MAZTR Free Online Audio File Analyzer (`maztr.com/audiofileanalyzer`) — file analyzer with channel detail including DC offset, min/max level, peak level dB, RMS level dB, RMS peak/trough, crest factor, and sample counts.
- Peak Mastering online loudness meter (`peakmastering.com/loudness-meter`) — browser loudness meter focused on LUFS, sample peak, true peak, and downloadable/instant results.
- FFmpeg `astats` filter documentation — command-line reference for per-channel and overall audio statistics including peak level, RMS level, DC offset, RMS peak/trough, peak counts, and configurable short-window length.

## Table-stakes capabilities

| Capability / UX pattern | Seen in competitors | In current gizza model? | Decision |
| --- | --- | --- | --- |
| Upload or paste an audio file and analyze locally | Online analyzers and loudness meters | Partly | This repo's pure page surface accepts base64/hex bytes; chat/CLI use the same descriptor. Direct browser file upload would require a file-input page shape, so it is documented as out-of-scope for this text page. |
| Per-channel plus overall rows | MAZTR and FFmpeg astats | Yes | Implemented. Each channel is measured separately and an `overall` row aggregates all samples. |
| RMS and sample peak in dBFS | All scanned tools | Yes | Implemented as `rms_dbfs` and `peak_dbfs`, with linear values beside them. |
| Average/mean level and DC offset | MAZTR / astats-style analyzers | Yes | Implemented as mean absolute average and signed DC offset. |
| Clipping facts | Online quality meters and astats-style checks | Yes | Implemented sample count, percentage, longest run, and configurable linear threshold. |
| RMS peak/trough over a short window | FFmpeg astats and MAZTR-style channel detail | Yes | Implemented with `rms_window_ms`; default 50 ms follows astats' common short-window convention. |
| LUFS / true peak / dBTP | Loudness meters | Out-of-model for this tool | Not implemented because this backlog item is an RMS/sample-peak report. Dedicated LUFS/true-peak tools should use a loudness algorithm/oversampling path and a different descriptor. |
| Spectrum/frequency plots | Audio analyzers | Out-of-model for this tool | Not implemented; this is a whole-file numeric report, not a spectral analyzer or visualization tool. |
| Multiple export formats | Online analyzers often show tables; CLI tools emit text/metadata | Yes | Implemented JSON (default), CSV, and aligned text report. |
| Presets / quick examples | Online tools often provide default controls | Yes | Page examples include JSON and CSV presets with a tiny half-scale WAV. |
| Slider controls for numeric settings | Common web control pattern | Yes | Page metadata uses sliders for RMS window and clipping threshold while keeping text fields canonical. |

## Defaults and examples chosen

- `input_format=base64` because pasted base64 is shorter and safer than raw binary in chat/page forms; `hex` remains supported for exact byte fixtures and deep links.
- `output=json` because it is machine-readable and preserves the full nested summary.
- `rms_window_ms=50` to match the common astats short-window default.
- `clip_threshold=0.99` to count near-full-scale PCM samples without requiring exact `1.0` values.
- Worked examples use a generated tiny PCM WAV at half-scale. Expected RMS, average, and sample peak are all about `-6.021` dBFS, making the exact-output checks easy to audit.

## Out-of-model / deliberately deferred

- Direct browser file upload on this text-only pure page. The current generated pure text surface is base64/hex; file upload would need a different page integration.
- LUFS, momentary/short-term loudness, true peak, and delivery-spec pass/fail. Those are loudness-meter features, not this RMS/sample-peak summary.
- Spectrogram/spectrum display and frequency-band analysis.
- Multi-file batch comparison.

## Verification focus

The final checks should prove:

1. A known half-scale WAV reports `-6.021` dBFS RMS/peak/average.
2. Hex input works as a secondary accepted input format.
3. CSV/report modes exercise the enum choices.
4. The page deep link pre-fills non-default controls and produces real output.
5. Hygiene rejects stale TODOs and stale manifest/schema drift.
