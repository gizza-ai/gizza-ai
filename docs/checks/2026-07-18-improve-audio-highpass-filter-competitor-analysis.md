# Competitor analysis — audio-highpass-filter (2026-07-18)

Pre-build scan for the new `audio-highpass-filter` tool (ffmpeg `highpass` biquad,
browser-local via wasm ffmpeg + gizza CLI). All findings paraphrased — no competitor
copy, branding, or trademarks reproduced.

## Competitors scanned (top real tools)

1. **audioeditor.org — Free Online High-Pass Filter**
   - Controls: a single user-chosen cutoff frequency; drag-and-drop upload; result
     preview before download. Browser-based ffmpeg.
   - Input formats: MP3, WAV, M4A, FLAC, OGG, AAC ("and more"). Output: MP3 or WAV.
   - Guidance: speech responds well to cutoff moves ~60–120 Hz. No slope/rolloff control
     exposed. States it only reduces low-frequency content (mid/high noise needs other
     tools).

2. **elysiatools.com — Audio High-Pass Filter**
   - Controls: **Cutoff Frequency (Hz)** + **Order (slope)** numeric (higher = steeper)
     + output-format dropdown.
   - Input: any audio/* up to 100 MB. Output: MP3, AAC, M4A, OGG, Opus, FLAC, WAV.
   - Worked examples: `{cutoffHz:80, order:4, wav}` (studio 60 Hz hum), `{cutoffHz:100,
     order:2, mp3}` (podcast bass clean-up). This confirms cutoff + an order/slope knob
     are table-stakes, and 80/100 Hz are the canonical defaults.

3. **safeaudiokit.com — High Pass Audio Filter**
   - Controls: Volume / Frequency / Peak sliders; real-time play/pause preview; drag-drop.
     Beginner-focused, thin technical docs; no documented slope/order.

4. **toololis.com — Noise Remover** (adjacent; uses a high-pass under the hood)
   - Default cutoff **80 Hz** described as removing most rumble without touching speech —
     confirms 80 Hz as the industry default.

5. **General reference (Audacity HPF guides, iZotope, Audio University)**
   - Slope framed in **dB/octave**: gentle 12–18 dB/oct sounds natural on voice; steep
     48 dB/oct thins voices. Common cutoffs: 60–120 Hz voice, ~120 Hz overhead/cymbals.

## Table-stakes → our decisions

| Capability | Competitor(s) | Fit | Decision |
| --- | --- | --- | --- |
| Cutoff frequency (Hz) | all | in-model | `cutoff` number, default **80**, range 10–2000 Hz, slider. |
| Slope / order / rolloff | elysiatools (order), refs (dB/oct) | in-model | `rolloff` enum in **dB/octave** — 6/12/24/48 — mapped to cascaded ffmpeg `highpass` biquads (1×p1, 1×p2, 2×p2, 4×p2). Default **12** (natural on voice). More intuitive than a raw "order" integer. |
| Output format choice | all | in-model | `format` enum mp3/wav/ogg/flac/m4a (audio-family standard), default mp3 @192 kbps. |
| Broad input format support | audioeditor, elysiatools | in-model | `Input::Audio` accepts any audio/* (mp3/wav/m4a/ogg/flac …); re-encoded to chosen output. |
| Preset one-click configs | (implicit via examples) | in-model | `[[example]]` chips: voice rumble (80/12), studio hum (100/24, wav), podcast (120/12). |
| Result preview before download | audioeditor, safeaudiokit | in-model | Page renders a playable `<audio>` element before the download button (generator default). |

## Out-of-model / considered, not built

- **Live real-time preview scrubbing** (safeaudiokit) — the page already plays the
  rendered result inline; a pre-render live-DSP preview would need a persistent WebAudio
  graph, out of the one-shot ffmpeg render model. Not built.
- **Opus output** (elysiatools) — kept to the audio family's standard 5 containers
  (mp3/wav/ogg/flac/m4a) for cross-tool consistency; Opus considered, not added.
- **100 MB uploads** (elysiatools) — gizza caps at ~10 MB in/out (browser-local memory);
  stated on the page. Larger files: trim/compress first.
- **Combined denoise + high-pass** — that overlap is already covered by the sibling
  `audio-noise-reduce` tool (its `remove_hum` fixed 80 Hz stage); this tool stays a
  focused, tunable high-pass so the two don't duplicate.
