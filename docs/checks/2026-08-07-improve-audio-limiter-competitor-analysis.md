# Audio Brick-Wall Limiter — Competitor Analysis (2026-08-07)

Tool: `audio-limiter` — applies a lookahead peak limiter to an audio file with ffmpeg `alimiter`, exposing a ceiling in dBFS, optional input gain, attack/release timing, smoothing, auto-level, and output format. Audio in, audio out (mp3/wav/ogg/flac/m4a); album art is dropped and samples are re-encoded.

## Competitor comparison

| Tool | How it's framed | Ceiling / threshold | Drive / gain | Timing controls | Output | Local vs cloud |
|------|-----------------|---------------------|--------------|-----------------|--------|----------------|
| Browser waveform editors with limiter effects | Manual peak limiting/mastering step | Ceiling or limit level in dB | Input/output gain | Attack/release or simplified release | Audio export | Mixed; some local browser processing |
| Desktop editors / DAWs (Audacity, DAW limiter plugins) | Final safety limiter or loudness maximizer | Ceiling/limit, often -1 or -0.3 dB presets | Input gain or make-up gain | Attack/lookahead/release; sometimes true-peak mode | Audio render | Local install |
| Online mastering / loudness services | One-click loudness and clipping prevention | Hidden target ceiling, often described as true peak | Automatic | Hidden/adaptive | Audio | Cloud upload typical |
| ffmpeg `alimiter` references | Technical filter primitive | `limit` linear amplitude ceiling | `level_in`, `level_out` | attack/release, ASC smoothing | Any ffmpeg encode | Local engine |

## Table-stakes features

- **Ceiling control in dBFS** so users can set the brick wall directly (`-1` dBFS is the common safety margin, `-0.3` is hotter, `-3` leaves more headroom).
- **Input gain / drive** before the limiter. A limiter only makes audio louder when signal is pushed into it; competitors either expose this or hide it behind a maximize/loudness button.
- **Fast reaction controls** — attack/lookahead and release are standard in editor and plugin limiters, while consumer services hide them.
- **Safe defaults** that are useful on first run: `-1` dBFS ceiling, neutral gain, fast attack, short release.
- **Optional smoother release / auto-level** for dense material and loudness-maximizer behaviour.
- **Format choice and privacy framing**: browser-local audio processing with common output formats is a clear in-model differentiator versus cloud mastering pages.

## Params / defaults / UX decisions

- **ceiling** (`-24…0`, default `-1`) maps to ffmpeg `alimiter limit` after dB→linear conversion. This gives a familiar dBFS control instead of exposing ffmpeg's linear amplitude value.
- **gain** (`-20…20`, default `0`) maps to `level_in`. Positive values drive the limiter for louder perceived output; negative values can tame hot files before limiting.
- **attack** (`0.1…80 ms`, default `5`) and **release** (`1…8000 ms`, default `50`) expose the filter's timing range while keeping documented starting points simple.
- **smooth_release** maps to ffmpeg `asc`, a boolean for averaged release that reduces pumping.
- **auto_level** maps to ffmpeg `level`. It intentionally re-normalizes toward full scale after limiting, so copy warns that it overrides the ceiling.
- **format** (`mp3|wav|ogg|flac|m4a`, default `mp3`) matches the existing audio-tool family.
- **Preset chips / examples** cover safety limiting, louder podcast drive, and transparent mastering catch rather than hiding the knobs behind a single strength slider.

## In-model vs out-of-model

**IN-MODEL (shipped):**

- Brick-wall sample-peak limiting using ffmpeg `alimiter`.
- dBFS ceiling and input gain controls with range validation.
- Attack, release, smooth release and optional auto-level controls.
- Common output formats, local browser ffmpeg execution, `-limited` output naming, and explicit no-op rejection for `0 dB` ceiling with neutral gain.
- Documentation that distinguishes limiting from loudness normalization and ratio compression.

**OUT-OF-MODEL (not built):**

- Strict true-peak / oversampled dBTP measurement. ffmpeg `alimiter` is a sample-peak limiter; the page recommends extra headroom when strict dBTP compliance is required.
- Adaptive mastering, ML enhancement, genre-aware loudness decisions, or streaming-platform target matching; those require analysis/model logic outside this pure ffmpeg tool.
- Multi-band limiting, sidechain workflows, waveform-region editing, batch processing, and project timelines; those belong in DAWs or larger editors.
- LUFS normalization; this remains the separate `audio-normalize` tool.

**Decision:** ship the table-stakes manual limiter surface — ceiling, drive, attack, release, smoothing, auto-level, and format — with local processing and clear limits. Keep true-peak mastering and adaptive loudness services out of model rather than implying guarantees the ffmpeg filter does not provide.
