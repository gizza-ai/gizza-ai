## Give a silent video the audio stream other tools insist on

Plenty of files have a picture but no sound at all: screen captures, exported
renders, timelapses, GIF conversions, drone clips. That is perfectly valid
video — but a lot of uploaders, editors, ad platforms and playback pipelines
assume every file has an audio stream, and reject or mangle the ones that don't.
The standard fix is to add a track of pure digital silence.

This tool does exactly that. Load a video, and it comes back with a silent audio
track attached. The **picture is stream-copied** (`-c:v copy`) — not re-encoded —
so it stays byte-for-byte identical, the file processes in seconds, and nothing
is uploaded: ffmpeg runs inside your browser tab.

### What it does under the hood

The silence isn't a file you have to supply — it is generated on the fly by
ffmpeg's `anullsrc` source at the channel layout and sample rate you pick, then
muxed alongside your untouched video and bounded to the video's exact length.

**Worked example:** `screencast.mp4` is a 12 MB, 3-minute H.264 recording with
no audio stream, and an upload check keeps failing it. Load it here with the
defaults (stereo, 48 kHz, 128 kbps) and you get `screencast-silent-audio.mp4`:
the same 3-minute H.264 picture, unchanged, now carrying one AAC audio track of
silence — and the upload goes through.

### The options

- **Channels** — `stereo` (default) is what most validators expect; `mono` is
  half the size and still a perfectly valid audio stream.
- **Sample rate** — `48000` Hz (default) is the video-world standard, `44100` Hz
  matches CD/editor conventions, `22050` Hz keeps the added bytes smallest.
- **Audio bitrate** — `128` kbps is the universally accepted default. Silence
  sounds the same at every setting, so `32`–`64` kbps is a good choice on a long
  clip; `192` kbps is there for checks that demand a higher rate.
- **If the video already has audio** — `replace` (default) leaves exactly one
  track in the output, the silence, so the clip ends up muted. `keep` preserves
  the original track and adds the silence as a second track. On a video with no
  audio the two behave identically.

### Notes and limits

- The video stream is copied losslessly — the picture, resolution, frame rate
  and duration are untouched.
- The output keeps the input container (mp4 → mp4, mov → mov, mkv → mkv,
  webm → webm). Any other container falls back to mp4.
- WebM output uses **Opus at 48 kHz** — that is the only rate Opus encodes, so a
  22.05/44.1 kHz choice is pinned to 48 kHz there. Every other container uses
  **AAC** at the rate you picked.
- `keep` re-encodes the existing audio track at the chosen bitrate (it can't be
  copied while a filter-generated track is being added in the same pass).
- Input and output are each capped at 25 MB — the file is processed in your
  browser's memory.
- This adds an audio stream; it does not add sound. If you want to attach real
  music or narration, that needs a second media file and a different tool.

## FAQ

<details>
<summary>My video already plays fine — why would I add silent audio?</summary>

Because something downstream requires an audio stream. Upload validators, ad
platforms, some NLE timelines, hardware players and automated encoding pipelines
routinely error out, drop the file, or produce a broken result when a video
carries no audio track at all. A silent track satisfies the requirement without
changing anything you can see or hear.

</details>

<details>
<summary>Does this re-encode or degrade my video?</summary>

No. The picture is stream-copied with `-c:v copy`, so the video stream in the
output is bit-identical to the input. Only the new audio track is encoded, and
it contains nothing but silence.

</details>

<details>
<summary>What's the difference between "no audio track" and "a muted audio track"?</summary>

A video with **no audio track** has zero audio streams — that's what trips up
strict tools. A **muted** video has an audio stream whose samples are all zero.
This tool turns the first into the second. If your file already has an audio
stream and you simply want it silenced, choose `replace`: the existing track is
dropped and a silent one takes its place.

</details>

<details>
<summary>How much bigger does the file get?</summary>

Very little. Silence compresses extremely well, and the added track is roughly
the bitrate you choose times the clip length as an upper bound — at 32 kbps
that's about 240 KB per minute at worst, usually far less. The video stream,
which is the bulk of the file, is copied unchanged.

</details>

<details>
<summary>Which formats work, and how large can the file be?</summary>

Anything ffmpeg can read; mp4, mov, mkv and webm are the common cases and each
keeps its own container. The output is named after the original with a
`-silent-audio` suffix (e.g. `clip.mp4` → `clip-silent-audio.mp4`). WebM gets an
Opus track at 48 kHz, everything else an AAC track. Input and output are each
capped at 25 MB.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. ffmpeg runs as WebAssembly inside your browser tab, so the file never leaves
your device.

</details>
