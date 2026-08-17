## About this tool

FLV (Flash Video) is the container that RTMP still speaks. Even now that Flash Player
itself is gone, plenty of infrastructure expects an FLV stream: Flash Media Server,
Wowza, Red5 and nginx-rtmp ingest endpoints, older IP cameras and encoders, digital-signage
players, and archives of Flash-era course material. This tool takes an MP4 — or any video
ffmpeg can decode, including MOV, MKV, WebM and AVI — and encodes it into a `.flv` file
with the codec, bitrate and keyframe settings those systems expect.

Everything runs inside your browser tab with a WebAssembly build of ffmpeg. The video is
never uploaded, there is no account, and the FLV comes back as a download link.

**This is always a re-encode, not a remux.** FLV accepts only a narrow codec set — H.264 or
one of the legacy Flash codecs for video, and AAC, MP3, Nellymoser, ADPCM or PCM for audio.
An MP4 whose video is HEVC, AV1 or VP9, or whose audio is Opus or FLAC, cannot be
stream-copied into FLV at all, so nothing here pretends `-c copy` will work.

### A worked example

Take a 1920×1080, 60 fps, H.264 + AAC MP4 and pick **Resolution cap = 720p**, **Frame rate =
30 fps**, **Video bitrate = 2500**, **Keyframe interval = 2**, **Audio codec = AAC**, **Audio
bitrate = 128**. The result is `out.flv`: 1280×720 at 30 fps, video capped at 2500 kbps with a
matching `-maxrate` and a 5000 kbps buffer, a keyframe every two seconds, and a 128 kbps AAC
track — which is exactly the shape most RTMP ingest endpoints document as their recommended
720p profile. A one-minute clip lands at roughly 20 MB.

Feed the same file the **Legacy Flash player** preset instead and you get 640×360 at 15 fps,
Sorenson Spark video at 800 kbps and 96 kbps MP3 audio at 44.1 kHz — decodable by Flash
players that predate version 9.0.115, which is when H.264-in-FLV arrived.

### What each control does

- **Video codec** — `h264` (libx264) is the default and what every modern RTMP endpoint
  wants. `flv1` is Sorenson Spark, an H.263 variant, for players and fixed-function decoders
  that predate H.264 support. Its compression is far weaker, so raise the bitrate by roughly
  50% when you pick it.
- **Resolution cap** — caps the output height. Smaller sources are never upscaled. Width
  follows the source aspect ratio, and both axes are rounded down to an even number because
  H.264 refuses odd dimensions outright.
- **Frame rate** — leave it at the source rate, or pin a constant rate. Some older ingest
  endpoints require a constant frame rate, and lowering it is the cheapest way to fit a tight
  bitrate.
- **Video bitrate** — 100–20000 kbps, applied as `-b:v` with a matching `-maxrate` and a
  two-second `-bufsize`, so the stream stays inside a bandwidth cap. Rough guide: 800–1200 for
  480p, 2000–3000 for 720p, 4000–6000 for 1080p.
- **Keyframe interval** — 1–10 seconds between forced keyframes. Two seconds is the near
  universal RTMP recommendation; it bounds how long a joining viewer waits for the first
  picture. The interval is expressed in time, so it holds whatever frame rate you end up at.
- **Audio codec** — `aac` keeps the source sample rate. `mp3` is resampled to 44100 Hz because
  FLV only permits MP3 at 44100, 22050 or 11025 Hz, and 48 kHz is the usual MP4 rate. `none`
  drops audio for a video-only stream.
- **Audio bitrate** — 32–320 kbps, ignored when audio is off. 96–128 is ample for speech,
  160–192 for music.

### Limits worth knowing

The browser holds the whole file in memory while encoding, so keep uploads to a few hundred
MB and prefer short clips; the chat and command-line paths cap the input at 32 MB and the
output at 64 MB. Encoding is real work — a minute of 1080p takes a minute or two on a typical
laptop — and the H.264 preset is fixed at `veryfast` because a slower preset costs minutes of
wall clock for a difference a bitrate-targeted legacy stream cannot show. FLV holds exactly
one video and one audio stream, so multi-track sources keep only the first of each.

## FAQ

<details>
<summary>Why is this a re-encode instead of a fast copy?</summary>

Because FLV's codec list is short. Video must be H.264 or a legacy Flash codec (Sorenson
Spark, VP6, Screen Video), and audio must be AAC, MP3, Nellymoser, ADPCM or PCM. Modern MP4s
increasingly carry HEVC, AV1 or Opus, none of which FLV can hold, so a `-c copy` remux would
fail on exactly the files people most often want to convert. Encoding every time is slower but
always produces a playable FLV. The reverse direction — FLV to MP4 — usually *is* a lossless
copy, because H.264 and AAC move into MP4 untouched.

</details>

<details>
<summary>Which settings should I use for RTMP ingest?</summary>

Start with the **RTMP live ingest** preset: H.264 video, AAC audio, a 2-second keyframe
interval, and a bitrate that fits the endpoint's published ceiling. The keyframe interval
matters more than people expect — most ingest services segment on keyframes downstream, and an
interval longer than a few seconds makes players take that long to show a first frame. If your
endpoint publishes a required frame rate, pin it rather than leaving it at the source rate.

</details>

<details>
<summary>When would I pick Sorenson Spark (FLV1) over H.264?</summary>

Only when something on the other end cannot decode H.264 inside FLV — Flash Player before
9.0.115, some Red5 and older Wowza demo pipelines, and a handful of fixed-function set-top
decoders. Sorenson Spark is an H.263 variant from 2002 and needs substantially more bitrate for
the same picture, so raise the video bitrate by about half when you switch. If you have any
choice, use H.264.

</details>

<details>
<summary>Why did my MP3 audio get resampled to 44.1 kHz?</summary>

FLV only permits MP3 at 44100, 22050 or 11025 Hz. Most MP4s carry 48 kHz audio, which is not on
that list, so an MP3-in-FLV mux at the source rate would be rejected outright. Picking MP3 here
therefore pins `-ar 44100`. AAC has no such restriction and keeps whatever sample rate the
source used.

</details>

<details>
<summary>My source has odd dimensions or multiple audio tracks — what happens?</summary>

Odd width or height is rounded down to the nearest even number, because H.264 rejects odd
dimensions at encoder-open time and you would otherwise get no output at all. That rounding is
applied even when you keep the source resolution. For streams, FLV holds one video and one
audio track, so the first video stream and the first audio stream are used and the rest are
dropped. A silent source is fine — it simply produces an FLV with no audio track.

</details>

<details>
<summary>What input formats can I feed it?</summary>

Anything the bundled ffmpeg build can decode: MP4, MOV, MKV, WebM, AVI, MPEG-TS, and more,
regardless of the codecs inside them, since the output is re-encoded anyway. The file picker
suggests `video/*`, but the container extension does not decide the result — the output is
always `out.flv` with the FLV muxer named explicitly rather than inferred from a filename.

</details>
