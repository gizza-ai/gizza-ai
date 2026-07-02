# audio-convert — competitor analysis (2026-07-02)

One WebSearch ("convert audio online mp3 wav flac m4a ogg converter bitrate"); skimmed the top
real tools: online-audio-converter.com, aconvert.com, convertio.co, convertr.org,
soundandgo.com, xconvert.com, playback.fm.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Target format selection (mp3/wav/ogg/flac/m4a at minimum) | all | in-model | `format` enum `mp3\|wav\|ogg\|flac\|m4a`, required (converting is the whole point — no silent default) |
| Bitrate control for lossy output (128–320 kbps typical) | online-audio-converter, aconvert, convertr, soundandgo | in-model | `bitrate` integer 32–320, default 192; ignored for lossless wav/flac |
| Very wide input support (300+ formats incl. video containers) | online-audio-converter, convertio | in-model | ffmpeg decodes; `Input::Audio` accepts the `audio/*` MIME class |
| Sample-rate / channel controls | aconvert, onlineaudioconverter | out-of-model for v1 | listed as a sensible later addition, not built |
| Batch / multi-file conversion | convertr, soundandgo | out-of-model | page framework is single-upload; chat/CLI convert one file per call |
| Ringtone (m4r) preset | online-audio-converter | out-of-model | niche Apple container; skipped |

## Design decisions

- `-vn` in the argv: audio files with embedded album art carry an attached-picture (video)
  stream that breaks audio-only muxers (e.g. wav); dropping video is correct for every
  conversion. (Same fix applied to trim-audio.)
- OGG output uses libvorbis. The native ffmpeg vorbis encoder is experimental, so if the
  browser @ffmpeg/core build turns out to lack libvorbis the page Playwright test will fail
  and ogg gets dropped from the enum — CLI/chat use native ffmpeg where libvorbis is present.
  VERIFIED in this run: the Playwright deep-link test (`?format=ogg&bitrate=96`) converted
  successfully in-browser — the @ffmpeg/core build DOES ship libvorbis, so ogg stays in the
  enum. (Follow-up: trim-audio could add ogg too on the same evidence.)
- Output filename keeps the original stem with the new extension (song.mp3 → song.wav),
  matching converter conventions.
