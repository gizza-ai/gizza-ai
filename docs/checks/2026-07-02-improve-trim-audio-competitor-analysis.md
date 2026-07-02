# trim-audio — competitor analysis (2026-07-02)

One WebSearch ("trim audio online tool cut mp3 start end time"); skimmed the top real tools:
Clideo cut-audio, mp3cut.net, audiotrimmer.com, ezgif cut-audio, Flixier audio-cutter,
Pi7 audio-cutter, onlineconverter cut-mp3, soundtools.io audio-trimmer.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Start/end time entry in seconds | all (ezgif, onlineconverter enter times; others drag handles) | in-model | `start` + `end` number params (seconds) |
| Keep selection vs delete selection | Clideo, mp3cut (extract vs remove modes) | in-model | `mode` enum `keep\|remove`, default `keep` |
| Fade in/out at cut edges | mp3cut, audiotrimmer | in-model (keep mode) | `fade` boolean, default off; keep mode only (remove-mode output duration is unknown at argv-build time, so fade-out start can't be computed) |
| Output format choice | ezgif (mp3/wav/flac/m4a/ogg), Pi7 | in-model | `format` enum `mp3\|wav\|flac\|m4a`, default `mp3`. OGG dropped at design time on libvorbis uncertainty — later DISPROVEN by audio-convert's in-browser ogg test (2026-07-02), so adding ogg here is a cheap follow-up. |
| Wide input-format support (mp3/wav/flac/aac/ogg/m4a/opus + video containers) | Pi7, mp3cut (300+ formats) | in-model | ffmpeg decodes; `Input::Audio` accepts the `audio/*` MIME class |
| Waveform visualizer with drag handles + live preview | Clideo, mp3cut, audiotrimmer, soundtools | out-of-model | page framework renders plain fields; listed, not built |
| "Use current position" from a player | ezgif | out-of-model | no player in the page framework |
| Ringtone/quality presets, 200 MB uploads | mp3cut, ezgif | out-of-model / deferred | 10 MiB input cap; fixed 192 kbps for mp3 (a `bitrate` param like extract-audio-from-video's is a sensible later addition) |

## Design decisions

- `start`/`end` (not `start`/`duration` like video-trim): every audio competitor
  expresses the selection as start→end; end must be > start.
- Keep mode uses `atrim=start=S:end=E,asetpts=N/SR/TB` (sample-accurate, timeline reset so
  optional `afade` offsets are computable from `end-start`); remove mode uses
  `aselect='not(between(t,S,E))',asetpts=N/SR/TB` (frame-granular ~20–40 ms — documented).
- Re-encode always (filters require it); codecs: libmp3lame 192k / pcm_s16le / flac / aac.
- First tool of the audio-input family unlocked by block-utils `Input::Audio` (ff9a63f).
