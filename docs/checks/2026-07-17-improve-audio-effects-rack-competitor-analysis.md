# audio-effects-rack — competitor analysis (2026-07-17)

Tool: **audio-effects-rack** — "Applies classic effects — reverb, echo/delay, chorus,
tremolo, and compression — to a clip." Multi-effect audio processor (ffmpeg family,
`Input::Audio`). Scan done BEFORE implementing to set the table-stakes param set.

All copy below is **paraphrased** from public marketing pages — no competitor copy,
branding, or trademarks are reproduced. Out-of-model items are listed, not built.

## Competitors surveyed (top 5)

| # | Tool | URL | What it offers |
| - | ---- | --- | -------------- |
| 1 | SoundTools (Add Reverb) | soundtools.io/add-reverb-to-audio | Reverb-only: presets Room / Hall / Plate / Cathedral + one amount slider (0–200%, default ~35%), plus a "custom" mode. In: MP3/FLAC/WAV/AAC/OGG. Out: MP3. "No file limits." Preset buttons + single slider; no preview mentioned. |
| 2 | Tembrica (Reverb & Echo) | tembrica.com/en/reverb-echo | 12 reverb presets (Small Room, Cathedral, Plate, Vocal…) + 8 echo presets (Slapback → Ping-Pong → Dub). Params: room size, decay, brightness, pre-delay, mix, stereo width, delay time, feedback, damping. Sliders + preset buttons + real-time A/B preview + waveform region select. In: MP3/WAV/FLAC/OGG/AAC/M4A/WebM. Out: MP3/WAV/FLAC/OGG. Files ≤2 GB. Runs local (Web Audio + ffmpeg wasm). Free tier = daily allowance; premium for more. |
| 3 | RemoveVocals (Audio Effects) | removevocals.ai/audio-effects | Reverb (4–5 room types) + 3D/8D, pitch, chorus, flanger, echo, distortion, tempo. Reverb: room-size select + wet/dry mix %. **Single-effect-at-a-time.** Drag-drop, real-time preview, preset buttons, sliders. In: MP3/WAV/OGG/FLAC/M4A. ≤500 MB. Out: WAV. Local, no signup. |
| 4 | Soundation (Audio Effects) | soundation.com/audio-effects | Full DAW rack: Compressor + Limiter (dynamics), Reverb + syncable Delay, Tremolo, Phaser, Fakie, EQ/Filters, Distortion, Vocal Tuner, stereo/gain utility. Before/after previews + presets. Account-based studio. |
| 5 | djmixer.online (Radio FX) | djmixer.online/blog/audio-effects-online | 14 effects incl. reverb, delay, chorus, phaser, compressor. All in-browser, no server-side processing. (Page 403'd to the fetcher; profile from the search index + article summary.) |

## Table-stakes → decision

| Capability | ≥1 competitor | Our decision | In/Out model |
| ---------- | ------------- | ------------ | ------------ |
| Reverb presets (room/hall/plate/cathedral) | 1,2,3 | `reverb` enum `none/room/hall/plate` via multi-tap `aecho` | **in** |
| Echo / delay with time control | 2,3,4,5 | `echo` = delay in ms (0 = off) via `aecho` | **in** |
| Chorus | 3,4,5 | `chorus` enum `none/light/deep` via `chorus` filter | **in** |
| Tremolo (volume modulation) | 4 | `tremolo` = rate in Hz (0 = off) via `tremolo` filter | **in** |
| Compression (even out levels) | 4,5 | `compression` enum `none/light/medium/heavy` via `acompressor` | **in** |
| Multiple output formats | 1,2,3 | `format` enum mp3/wav/ogg/flac/m4a | **in** |
| Preset one-click buttons | 1,2,3 | `[[example]]` "Try:" chips (radio-vocal, telephone, ambient…) | **in** |
| Sliders for numeric knobs | 1,2,3 | `kind = "slider"` on `echo` + `tremolo` | **in** |
| **Combine several effects in one pass (a real rack/chain)** | 4 (Soundation) | Yes — our differentiator vs the single-effect tools (1,2,3): one pass chains dynamics → modulation → time → space in a fixed musical order | **in** |
| Per-effect wet/dry mix % slider | 2,3 | Approximated via preset out-gain/decay; a true independent dry-blend per stage is heavier (needs `asplit`/`amix` per effect). Considered, not built — keep the knob count sane. | out (approx) |
| Reverb decay/pre-delay/damping/brightness knobs | 2 | Folded into the 3 presets; exposing all 9 knobs is DAW-grade UI creep. | considered, rejected |
| Real-time / A/B preview, waveform region select | 2,3,4 | The generic page renders the finished clip in an `<audio>` player; no live scrubbing/region UI in this repo's page model. | out-of-model |
| Very large files (500 MB–2 GB) | 2,3 | Browser-local ffmpeg wasm caps at a 10 MiB input here. | out-of-model |
| Phaser / flanger / distortion / 3D-8D / pitch / auto-tune | 3,4 | Separate effects out of this tool's stated scope (chorus/tremolo/reverb/echo/compression). Pitch already exists as `audio-pitch-shift`. | out of scope |

## Notes
- Every listed table-stake lands in the descriptor OR is explicitly listed above — none dropped silently.
- Our edge over the reverb-only / single-effect competitors (1,2,3): a genuine **rack** that
  applies dynamics + modulation + time + space **in one pass**, fully browser-local, no account,
  no watermark, no daily cap.
- ffmpeg filter feasibility spiked before tagging anything in-model: full chain
  `acompressor,chorus,tremolo,aecho,aecho` and every preset variant encode cleanly.
