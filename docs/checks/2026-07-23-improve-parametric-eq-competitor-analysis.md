# parametric-eq — competitor analysis (2026-07-23)

Tool function: apply a multi-band parametric equalizer to a single audio file, with user-controlled centre frequency, gain, and Q/bandwidth for each band, then export a new audio file.

## Competitors scanned

1. Browser EQ tools and simple online equalizers: typically expose low/mid/high or graphic sliders and export a processed file. Table stakes are private browser processing, dB gain controls, audio preview/download, and common formats.
2. DAW/plugin-style parametric EQs: parametric bands have frequency in Hz, gain in dB, and Q/bandwidth; common presets include vocal presence, cut mud, and add air. Spectrum display and live playback are common but not required for a file-transform tool.
3. ffmpeg equalizer/loudness recipes: use `equalizer=f=<hz>:t=q:w=<q>:g=<db>` for peaking bands, chained for multiple bands. This is directly in-model for gizza's single-audio ffmpeg surface.

## Table stakes → decisions

| Capability | In-model? | Where it lands |
| --- | --- | --- |
| Single audio input, local processing | yes | `Input::Audio`, page file input, ffmpeg runtime |
| Multiple parametric bands | yes | three bands, each freq/gain/Q |
| Frequency range 20 Hz–20 kHz | yes | numeric params + validation |
| Gain in dB with positive boost / negative cut | yes | slider/numeric params, ±20 dB cap |
| Q / bandwidth control | yes | slider/numeric params, 0.1–10 |
| Output formats mp3/wav/ogg/flac/m4a | yes | `format` enum |
| Preset chips for common fixes | yes | vocal presence, cut mud, add air examples |
| Live spectrum/analyzer graph | no | out-of-model for current generic ffmpeg page; not needed for verified transform |
| Unlimited bands / visual node editor | no | out-of-model; three bands keeps schema and page understandable |
| Realtime audition while dragging | no | generic page re-runs file transforms, not a realtime audio workstation |

## UX controls

Use number fields for centre frequencies (20–20000 Hz), slider controls for gain and Q, and preset chips for common EQ moves. A zero-gain band is intentionally omitted; an all-zero request errors instead of re-encoding unchanged audio.

## Relationship to audio-eq

`audio-eq` is the fixed tone-control tool (bass at ~100 Hz, mid at 1 kHz, treble at ~3 kHz). This tool is distinct because the user sets the frequency and Q for each band; it covers surgical cuts and exact resonances that fixed bass/mid/treble cannot.
