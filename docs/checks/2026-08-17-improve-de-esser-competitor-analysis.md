# de-esser — competitor analysis (2026-08-17)

Scan performed before finalizing the implementation. Query: "online de esser vocal sibilance tool amount frequency audition removed s". Findings are paraphrased; no competitor copy, branding or trademarks are reused.

## Competitors skimmed

### 1. Acon Digital DeVerberate / De-esser style plugin pages
- Typical controls: threshold/amount, frequency or band focus, maximum reduction, listen/audition mode.
- UX pattern: vocal-oriented presets, meters that show gain reduction, warning that too much processing creates a lisp.
- Model fit: threshold/meter visualization is out-of-model for the current ffmpeg page, but amount/band/max reduction/listen mode fit.

### 2. Waves-style de-esser plugin docs/tutorials
- Typical controls: split-band vs wideband processing, sidechain frequency, threshold and range, monitor sidechain.
- Defaults vary by voice but tutorials commonly start around the sibilance band and adjust while listening to removed esses.
- Model fit: ffmpeg's `deesser` exposes a dynamic split-band filter, output/ess/input modes and unitless controls. Exact Hz, dB threshold and visual gain-reduction meters are out-of-model because the filter does not expose them directly.

### 3. Online vocal cleanup / podcast enhancement tools
- Typical UX: upload audio, choose a gentle/strong preset, export MP3/WAV, sometimes combined with noise reduction and leveling.
- Table-stakes copy: explain that de-essing is for harsh S/T consonants, not background noise; warn about overprocessing.
- Model fit: presets are represented by example chips. Noise reduction/leveling are separate existing gizza tools and intentionally out-of-scope.

## Table stakes → implementation decisions

| Table stake | Decision |
|---|---|
| Dynamic sibilance reduction rather than static EQ | **in-model** via ffmpeg `deesser` filter |
| Amount/strength control | **in-model** as `amount` 1-100, default 60 |
| Band/frequency focus | **in-model with honest naming** as `band` 1-100, default 70; not labeled Hz because ffmpeg exposes a coefficient |
| Maximum range/reduction | **in-model** as `max_reduction` 1-100, default 50, inverted onto ffmpeg's `m` coefficient |
| Listen/audition sidechain | **in-model** as `mode=ess`; `mode=input` also gives A/B reference |
| Output format choices | **in-model** as `format=mp3|wav|ogg|flac|m4a`, matching neighbouring audio tools |
| Presets | **in-model** as page example chips: default vocal, gentle podcast, audition removed esses |
| Gain reduction meter / waveform display | **out-of-model**; current page driver executes ffmpeg and returns output, but does not expose per-frame gain telemetry |
| Exact Hz/dB labels | **out-of-model / misleading** because ffmpeg `deesser` options are unitless coefficients dependent on sample rate |
| Noise removal, clicks, breath control | **out-of-scope**; existing tools cover noise gate/noise reduce, and this tool stays focused on sibilance |

## Worked example carried into docs

Start with `amount=60 band=70 max_reduction=50 mode=output format=mp3`. If the result still has sharp esses, increase amount toward 75. If the voice starts to lisp or vowels dull, raise band toward 80 or lower max reduction toward 35. Switch to `mode=ess` temporarily; the output should mostly be S/T bursts, not the full voice.
