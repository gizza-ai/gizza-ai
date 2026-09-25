# video-audio-to-mono — competitor analysis (2026-09-25)

Scope: browser/online tools that downmix a **video's** audio to mono. Scanned the
top results for "convert video audio to mono online" / "stereo to mono video".
No competitor copy, naming or branding was reused — this is a capability diff
only.

## Competitors reviewed

### 1. Fylite — "Resample Video Audio" (fylite.com/en/tools/video-audio-resample/)
The closest match: video in, video out, in-browser, no upload.

- Channels: `Mono (1 channel)` / `Stereo (2 channels)`.
- Sample rate: `44100 Hz`, `48000 Hz` — two choices only.
- Input: MP4, WebM, MOV, AVI, MKV. **Output: MP4 only.**
- 500 MB cap; states all processing is local and Chrome/Edge 94+.
- **No bitrate control at all.**

### 2. WUTools — "Stereo to Mono Downmix Converter" (wutools.com/audio/stereo-mono-converter)
Audio-first but accepts video containers (MP4/MKV/MOV); in-browser ffmpeg.wasm.

- Downmix law: `Average (0.5 L + 0.5 R)`, `-3 dB Sum (0.707 L + 0.707 R)`,
  `Mid (L + R)`, `Left channel only`, `Right channel only`, `Side (L − R)`.
- Output format: `Same as Input`, MP3, WAV, OGG, AAC.
- Quality presets: 320 / 192 / 128 kbps.
- Extras: "Mono Compatibility Analysis" — phase correlation, predicted mono
  peak, verdict.
- Also offers the reverse direction (mono → stereo).

### 3. Wondershare — stereo-to-mono page (videoconverter.wondershare.com)
Mostly a funnel to the desktop UniConverter; the online alternatives it lists
(Coolutils, Online-Convert, FConvert) are server-side uploaders.

- Channel dropdown plus bitrate, sample rate and encoder controls (desktop app).
- Explicitly concedes the online options "have limited memory upload size".
- No privacy/no-upload claim for the web path — files are uploaded.

## Gap diff → what shipped

| Capability | Fylite | WUTools | Wondershare | gizza |
|---|---|---|---|---|
| Video in → video out | yes | partial | yes (upload) | **yes** |
| Picture untouched (stream copy) | claimed | n/a | re-encode | **yes, `-c:v copy`** |
| Keeps input container | no (mp4 only) | "same as input" | no | **yes (mp4/mov/m4v/mkv/webm)** |
| Standard downmix | yes | yes | yes | **yes (`-ac 1`, 5.1-aware)** |
| Left / right only | no | yes | no | **yes** |
| Difference (L − R) | no | yes | no | **yes** |
| Audio bitrate control | **no** | 3 presets | yes | **yes, free 16–320 kbps** |
| Sample rate control | 2 values | no | yes | **yes, keep + 5 values** |
| Runs locally, no upload | yes | yes | no | **yes** |

Closed against the scan:

- **Bitrate** — Fylite's biggest hole, and it is the knob the tool's own
  "shrink size" promise depends on. Shipped as a free 16–320 kbps slider rather
  than WUTools' three presets, since 32/64 kbps mono speech is the interesting
  range and no competitor exposes it.
- **Sample rate** — Fylite offers 44100/48000. Shipped `keep` plus
  48000/44100/32000/22050/16000, with `keep` as the default so the common case
  changes nothing it doesn't have to.
- **Left / right / difference** — WUTools has these, Fylite doesn't; they are
  what makes this a one-sided-audio *fix* and not just a downmixer. All three
  ship.
- **Container preservation** — Fylite forces MP4 out even for a WebM input. We
  keep the container and pick the codec that container can hold (AAC, or Opus
  for WebM), with the libopus sample-rate snap so a 44100 request on WebM
  doesn't hard-fail the encode.

## Deliberately not built (out of model / out of scope)

- **Mono → stereo** (WUTools, Fylite). The inverse direction, and a separate
  tool's job; `audio-channel` already covers up-mixing on the audio side.
- **Phase-correlation / mono-compatibility analysis** (WUTools). This is a
  measurement surface, not a transform — it belongs with the reporting blocks
  (`audio-rms-level-report`, `clipping-detector`), not on a converter page.
- **Extra downmix laws** (`-3 dB Sum`, `Mid (L + R)` at unity). `-ac 1` already
  applies ffmpeg's normalised downmix; an un-normalised `L + R` sum mostly
  produces clipping, so exposing it would be a footgun without a limiter stage.
- **Audio-only output formats** (MP3/WAV/OGG from a video). That is
  `extract-audio-from-video` / `audio-to-mono`, both already in the model.
- **500 MB inputs** (Fylite). Everything here is processed in browser memory;
  the cap stays 25 MB in and 25 MB out, and the page says so.
