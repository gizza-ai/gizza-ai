# audio-fit-to-length — competitor scan (2026-07-23)

Tool function: pad an audio file with silence (when shorter) or trim it (when longer) so
its total duration matches an exact target length. Distinct from trim-audio (cuts a
selection), audio-loop (repeats to fill), audio-silence-remove/edge-silence-trimmer
(strip silence), and audio-pause-shortener. This is "make the file exactly T seconds."

All competitor observations below are PARAPHRASED — no copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **Descript — Add Silence to Audio** (descript.com/tools/add-silence-to-audio)
   - Select a section, choose "add silence", type the exact length wanted. Framed around
     precise pacing/spacing. Browser tool, common formats.
   - Params: silence length (seconds), insert point. Out-of-model extras: waveform click-to-place,
     account/editor integration.

2. **Tembrica — Add Silence to Audio** (tembrica.com/en/add-silence)
   - Add silence at the beginning, end, or an arbitrary waveform position. Duration presets
     (0.5 / 1 / 2 / 5 s) plus custom. Files up to ~1 GB. Formats: MP3, WAV, FLAC, OGG, M4A.
   - Table-stakes: **position = start / end** (in-model), duration presets → chips (in-model),
     multi-format output (in-model). Arbitrary mid-waveform insert point → out-of-model
     (needs interactive waveform + a known playhead offset; not a "fit to length" op).

3. **Elysia Tools — Audio Add Silence** (elysiatools.com/en/tools/audio-add-silence)
   - Set silence duration in seconds; add to **start or end**. Accepts 0.1 s up to 3600 s (1 h).
   - Table-stakes: start/end position (in-model); 0.1–3600 s range → informs our cap (max 3600).

4. **XConvert / KlipTools Audio Trimmer** (xconvert.com/audio-trimmer, kliptools.com/audio-trimmer)
   - Precise trim-to-length with start + duration, waveform preview, fade in/out, MP3/WAV/M4A/OGG/AAC,
     "no quality loss". Represents the *trim* half of fit-to-length.
   - Table-stakes: trim to an exact length (in-model, our long-input branch), multi-format (in-model).
     Interactive waveform-drag selection, fade → out-of-model here (fades live in audio-fade;
     waveform UI needs a full editor).

## Table-stakes decisions (each tagged in/out of model)

| capability | decision | how |
| ---------- | -------- | --- |
| target total duration (seconds) | **in-model** | `apad=whole_dur=T` (pad) + `-t T` (trim), one pass |
| pad at END (default) | **in-model** | `-af apad=whole_dur=T -t T` |
| pad at START | **in-model** | `-af areverse,apad=whole_dur=T,areverse -t T` (finite via whole_dur) |
| trim when longer than target | **in-model** | `-t T` (trims from the end) |
| output MP3/WAV/OGG/FLAC/M4A | **in-model** | family codec map (192 kbps lossy) |
| duration presets (ad slots 15/30/60 s) | **in-model** | `[[example]]` chips |
| range up to 3600 s | **in-model** | cap MAX_DURATION_S = 3600 |
| pad at BOTH ends / center | **out-of-model** | splitting `T − d` silence needs input duration `d` at argv-build time → two-pass; the ffmpeg bridge is single-pass (same reason the video-audio-peak-normalize two-pass path was skiplisted) |
| arbitrary mid-track silence insert | **out-of-model** | needs an interactive waveform + a chosen playhead offset; not a fit-to-length operation |
| waveform preview / drag selection, fade in/out | **out-of-model** | full editor UI; fades already live in `audio-fade` |
| batch / >10 MiB uploads | **out-of-model** | single-file, 10 MiB in/out cap (browser-local wasm) |

## Notes on the ffmpeg spike

- `apad=whole_dur=T` yields `max(inputDur, T)`; `-t T` then guarantees exactly T. When the input
  is already longer than T, `apad` adds nothing and `-t` trims → both branches, one filter chain.
- START padding uses `areverse,apad=whole_dur=T,areverse`: reversing makes the appended silence
  land at the head; `whole_dur` keeps the stream finite so the second `areverse` (which must buffer
  the whole stream) terminates. When the input is longer than T, start-pad still trims from the end
  (pad position only governs *where silence is added*) — stated on the page.
- Re-encode (not `-c copy`) so silence joins the decoded PCM cleanly; `-vn` drops album-art streams.
