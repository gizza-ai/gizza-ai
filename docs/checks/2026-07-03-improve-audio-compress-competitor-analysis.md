# audio-compress — competitor analysis (2026-07-03)

One WebSearch ("compress audio file online mp3 smaller bitrate tool"); skimmed the top real
tools: XConvert audio-compressor, FreeConvert mp3-compressor, MP3Smaller, Media.io online
audio compressor, Aconvert audio compress. (Initial scan 2026-07-02, reconfirmed 2026-07-03.)

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Compress by picking a lower bitrate | all | in-model | `bitrate` integer param, kbps |
| Bitrate presets around 64/96/128/192 | MP3Smaller, Media.io | in-model | free 32–320 range covers every preset; page copy recommends 64 speech / 96 default / 128–192 music |
| 32–320 kbps supported range | Media.io ("320Kbps to 32Kbps") | in-model | same range; out-of-range **rejected, not clamped** |
| Many input formats (mp3/wav/m4a/aac/flac/ogg) | all | in-model | anything ffmpeg decodes; `AssetKind::Audio` |
| Lossy output format choice | XConvert, Aconvert | in-model | `format` enum mp3\|ogg\|m4a, default mp3 |
| Target-size mode ("shrink to X MB") | XConvert, FreeConvert | out-of-model for v1 | needs input duration probing before argv build; listed, not built |
| Percentage mode ("cut 50%") | XConvert | out-of-model | same reason as target-size |
| Mono/sample-rate reduction combo | Media.io advanced | out-of-model | audio-to-mono is its own tool; keep params orthogonal |
| Speech vs music bitrate guidance | FreeConvert blog, podcast guides | in-model (copy) | worked example + "picking a bitrate" section on the page |

## Design decisions

- Formats are **lossy only** (mp3/ogg/m4a): re-encoding to wav/flac never shrinks a file, so
  those targets error with a pointer to audio-convert instead of producing a bigger "compressed"
  file. This split keeps audio-compress semantically distinct from audio-convert (which owns
  format conversion incl. lossless, default 192 kbps).
- Default bitrate **96 kbps** (audio-convert defaults 192): the tool's job is size reduction,
  so the default must clearly shrink typical 128–320 kbps sources while staying listenable.
- Out-of-range bitrates are **errors, not clamps** — a user asking for 8 or 1000 kbps learns
  the supported range instead of silently getting a different file.
- The LLM envelope reports input→output bytes and % saved, and calls out the
  already-below-target case (compressing a 64 kbps file "to" 96 kbps can't shrink it) — the
  same caveat the page copy and FAQ explain.
- Verification proves real size reduction: the page test fixture is a 192 kbps mp3 (~73 KB);
  default 96 kbps must come out under 0.65× input, the `?bitrate=32&format=ogg` deep link
  under 0.35×, both decoding to the full ~3 s duration. CLI check compresses a public 51 kbps
  ogg at 32 kbps (8124 → 5421 bytes, ffprobe 33 kbps) and proves the range error message.
