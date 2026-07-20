# video-speech-segment-detector — competitor analysis (2026-07-20)

Scan done BEFORE implementation (create-next-tool step: competitor scan). Paraphrased notes only —
no competitor copy, branding, or trademarks reproduced in tool copy.

## Competitors examined

1. **auditok** (github.com/amsehili/auditok) — Python audio-activity-detection CLI/library.
   Energy-based (no ML). Params: energy threshold (dB, default 50 on its own scale), `min_dur`
   (min event length, default 0.2 s), `max_dur` (max event length, default 5 s — splits longer
   events), `max_silence` (silence tolerated *inside* one event, default 0.3 s), leading/trailing
   silence kept around events. Output: list of regions with float second start/end. Extras:
   plotting, saving each region to its own file, live mic input, adaptive threshold estimators
   (otsu/percentile), optional WebRTC-based validator.
2. **Audacity "Label Sounds" analyzer** (manual.audacityteam.org/man/label_sounds.html) —
   threshold level (default -30 dB), threshold measurement (peak/average/RMS), min silence
   duration between sounds (default 1 s), min label interval ≈ min sound duration (default 1 s),
   max leading/trailing silence kept as padding (default 0), label type (regions over sounds OR
   regions between sounds, i.e. label the silences instead), label-track output (tab-separated
   start/end/label, exportable).
3. **Silero VAD** (github.com/snakers4/silero-vad) — ML neural VAD; returns speech timestamps
   (samples or seconds). Well-known knobs (wiki/code): probability threshold (0.5),
   `min_speech_duration_ms` (250), `min_silence_duration_ms` (100), `speech_pad_ms` (30).
   Others in the same family: WebRTC VAD (aggressiveness 0–3, 10–30 ms frames),
   pyannote-audio (ML speech timeline / diarization).

## Table-stakes → decision

| Capability (competitor) | Tag | Where it landed |
|---|---|---|
| Loudness threshold in dB (Audacity -30 default; auditok) | in-model | `threshold_db` number, default -30, range -90..0 |
| Min silence gap that splits segments (Audacity 1 s; Silero 0.1 s) | in-model | `min_silence` number, default 0.5 s (family default shared with video-silence-cut) |
| Min speech/sound duration — drop blips (auditok 0.2 s; Silero 0.25 s; Audacity 1 s) | in-model | `min_speech` number, default 0.25 s |
| Padding kept around speech (Audacity leading/trailing; Silero speech_pad 30 ms; auditok) | in-model | `pad` number, default 0 s, symmetric, merges overlaps |
| Label the sounds OR the silences (Audacity label type) | in-model | `segments` enum: both / speech / non-speech |
| Timestamp list output (auditok regions; Silero timestamps) | in-model | `output=report` human-readable list + summary |
| Label-track export (Audacity tab-separated) | in-model | `output=audacity` (start TAB end TAB label) |
| Machine-readable export | in-model | `output=csv` (start,end,duration,label) and `output=srt` (subtitle scaffolding for later transcription) |
| Voice-band focus (WebRTC/ML VADs analyze speech-band energy) | in-model (approximation) | `voice_band` boolean default true — 200–3000 Hz band-pass before detection via ffmpeg highpass/lowpass |
| ML/probability VAD (Silero, pyannote, WebRTC model) | OUT-OF-MODEL | Needs a neural model; gizza is pure Rust + ffmpeg. Listed in page copy as a stated limit (energy-based, not ML). |
| Threshold measurement mode peak/average/RMS (Audacity) | OUT-OF-MODEL | ffmpeg silencedetect has a single measurement mode; no filter exposes the choice. |
| Adaptive/auto threshold (auditok otsu/percentile) | OUT-OF-MODEL | No ffmpeg filter computes adaptive thresholds and feeds silencedetect in one pass. |
| Max event duration / split long events (auditok max_dur) | OUT-OF-MODEL (deliberate omit) | Corpus-chunking niche; post-processing split is feasible but out of scope for a report tool — documented as a limit. |
| Save each segment as its own media file (auditok split) | OUT-OF-MODEL here | This is the *detector*; cutting lives in video-cut-segments / video-silence-cut (page copy cross-references the workflow). |
| Live microphone / streaming input (auditok) | OUT-OF-MODEL | Page/CLI operate on files. |
| Interactive plot of detections (auditok) | OUT-OF-MODEL | Page shows the textual report; no waveform for video input in the shared page driver. |
| Speaker diarization (pyannote) | OUT-OF-MODEL | ML. Not built. |

## UX control patterns adopted

- Threshold as a **slider** (-60..-10 dB, step 1) mirroring video-autocrop-bars' threshold slider.
- Numeric fields with placeholders for min_silence / min_speech / pad.
- `<select>` with friendly labels for `segments` and `output`; checkbox for `voice_band`.
- **Preset chips** (`[[example]]`): defaults report; SRT speech-only with padding (subtitle prep);
  CSV silence report (non-speech list).
- Report is downloadable (Download link, extension follows the output format: .txt/.csv/.srt).

## Design notes

- Detection = ffmpeg `silencedetect` (energy VAD) on the audio track, optionally band-passed to
  200–3000 Hz; speech = complement of detected silence, then min_speech filter + pad + merge.
  Same detect-pass architecture as video-silence-cut pass 1 (core reused via path dep) and the
  video-autocrop-bars page pattern (custom.js two-step: run detect, parse the ffmpeg log in the
  shared wasm core, render text).
- Chat surface: ffmpeg cannot run in the chat Service Worker (platform-wide) — page + CLI are the
  verified surfaces.
- 10 MiB input cap (video tool family standard).
