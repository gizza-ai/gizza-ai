# audio-filter — competitor analysis (2026-07-26)

New general-purpose audio filter tool: apply a **low-pass, high-pass, band-pass, or
notch** filter to an audio clip in the browser (wasm ffmpeg), no upload. Distinct from
the existing single-purpose `audio-highpass-filter` (high-pass only, "rather than a
general band filter" per its own docs) and `audio-eq` (3-band shelf/peak equalizer).

## Competitor scan (top real tools, paraphrased — no copy/branding reproduced)

1. **SafeAudioKit — Low Pass Audio Filter** (safeaudiokit.com/effects/low-pass-audio-filter)
   - Single-mode effect: cutoff frequency slider + an "intensity" control; upload → process →
     download. Focused UX, one filter type per page (separate pages for each mode).
   - Table-stakes: **cutoff frequency**, **intensity/strength**, browser processing, download.

2. **ToolsBox — Audio Low-Pass Filter** (toolsbox.io/audio/audio-lowpass-filter)
   - "Remove frequencies above a cutoff you set." Single cutoff input; simple upload/download.
   - Table-stakes: **cutoff frequency**, common audio input/output formats.

3. **Schemalyzer — RC/LC Filter Calculator** (schemalyzer.com/en/tools/filter-calculator)
   - Lets you pick **filter type: low-pass, high-pass, band-pass, or notch**, and enter a
     cutoff frequency. (A circuit-design calculator, not an audio processor, but confirms the
     4-type taxonomy + cutoff as the core surface an audience expects.)
   - Table-stakes: **filter type selector (the 4 modes)**, **cutoff/center frequency**.

4. **Filter Design Tool — thecoatlessprofessor / GetZenQuery**
   - Designs low-pass / high-pass / band-pass / band-stop responses with adjustable cutoff and
     a choice of **Butterworth / Chebyshev / Bessel** response families + filter **order**;
     plots the frequency response. Design/visualisation tool, not a file processor.
   - Table-stakes surfaced: **response family**, **filter order/steepness**, **band-stop = notch**.

## Table-stakes → decision (in-model vs out-of-model)

| Capability | Decision |
| --- | --- |
| Filter type: low-pass / high-pass / band-pass / notch | **in-model** → `filter_type` enum (the core of this tool) |
| Cutoff (LP/HP) / center (BP/notch) frequency in Hz | **in-model** → `frequency` (slider, 20–20000) |
| Bandwidth for band-pass / notch | **in-model** → `width` (Hz; used only by band types) |
| Output format (mp3/wav/ogg/flac/m4a) | **in-model** → `format` enum |
| Browser-local, no upload, instant download | **in-model** (already the gizza model) |
| Intensity / cascaded steepness slider | **in-model but rejected** — ffmpeg's `lowpass`/`highpass` biquad is a fixed 2-pole slope and `bandpass`/`bandreject` take no poles; a general 4-type tool with one steepness knob that only affects 2 of 4 modes adds schema noise. The dedicated `audio-highpass-filter` owns the cascaded-rolloff use case; this tool trades steepness control for type breadth. Noted for a future pass. |
| Response family (Butterworth/Chebyshev/Bessel) | **out-of-model** — ffmpeg's audio filters expose one biquad response only; selecting an analog family would need a custom DSP engine, not ffmpeg. |
| Filter order beyond 2-pole | **out-of-model** for band types (ffmpeg `bandpass`/`bandreject` are fixed-order). |
| Live frequency-response plot | **out-of-model** — would need a spectrum/plot renderer; the tool ships an `<audio>` A/B preview instead. |
| Real-time preview while dragging | out-of-model (offline ffmpeg pass per run). |

## Worked-example UX patterns adopted

- **Preset chips** (`[[example]]`): "Telephone band-pass", "Remove low rumble (high-pass)",
  "Tame harsh highs (low-pass)", "Kill 60 Hz hum (notch)" — one-click prefill+run, matching the
  preset patterns competitors ship.
- **Slider** control for `frequency` (bounded 20–20000) and `width`; **friendly `<select>`
  labels** for `filter_type` and `format`.
- FAQ accordions covering: what each type does, how to pick a frequency, what the width does,
  band-pass vs notch, and privacy/limits.
