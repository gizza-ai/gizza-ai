## Downmix a video's audio to mono

Pick a video and its audio track is collapsed to a **single mono channel**. The
picture is **stream-copied** (not re-encoded), so it stays byte-for-byte
identical and processing is fast; only the audio is re-encoded. Everything runs
in your browser — nothing is uploaded.

Two reasons people reach for this:

- **One-sided audio.** A lapel mic plugged into the left input, a camera where
  only one channel was armed, a screen recording that captured the microphone
  on one side and nothing on the other. Play it back and the voice comes out of
  one earbud. Downmixing puts it in both.
- **Size.** A mono track needs roughly half the bits of the same stereo track,
  and speech doesn't need CD bandwidth. Dropping to 64 or 32 kbps mono can take
  a large chunk off a talking-head clip without touching the picture.

### Source channel

The **Source channel** control decides what actually feeds the mono track:

- **Mix all channels** — ffmpeg's standard downmix (`-ac 1`). Correct for
  ordinary stereo and for 5.1/7.1 surround. Use this unless a specific channel
  is broken.
- **Left channel only** / **Right channel only** — throw the other side away and
  copy the good one everywhere. This is the one-sided-audio fix: mixing a silent
  right channel into a loud left one would halve your volume, whereas picking
  **Left** keeps it at full level.
- **Difference (L − R)** — the side signal. Anything identical in both channels
  (usually the centred voice) cancels out, which is a quick way to hear what is
  out of phase, or to check whether a track is genuinely stereo at all.

**Worked example:** an interview recorded with the mic on the left input only.
Load `interview.mp4`, set **Source channel** to `Left channel only`, leave
**Audio bitrate** at `128` and **Sample rate** on `Keep the source rate`. You
get `interview-mono.mp4` — same picture, same length, and the voice now plays at
full volume in both ears instead of just one.

### Bitrate and sample rate

**Audio bitrate** is the size knob, in kbps (16–320, default 128). Because the
track is mono, 128 kbps here is about as detailed as 256 kbps stereo was. For a
voice recording 64 kbps is comfortable and 32 kbps is still clearly
intelligible.

**Sample rate** optionally resamples. `Keep the source rate` leaves it alone;
48000 is the video standard, 44100 the CD standard, and 16000 is the usual
speech rate. Lowering it shrinks the file beyond what the bitrate alone does,
at the cost of high-frequency detail.

### Notes and limits

- The video stream is copied losslessly and the output keeps the same container
  (mp4 → mp4, webm → webm). WebM audio is re-encoded to Opus, everything else to
  AAC.
- libopus only accepts 8000/12000/16000/24000/48000 Hz, so for **webm** output a
  requested rate is snapped to the nearest one it supports (44100 → 48000,
  22050 → 24000).
- The whole clip gets one treatment — this isn't a per-track or per-region
  editor. To keep one audio track out of several, use the audio-track selector
  tool; to remove audio entirely, use the mute tool.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

### FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.

</details>

<details>
<summary>Will this reduce the video quality?</summary>

No. Only the audio is changed; the **picture is stream-copied without
re-encoding**, so it is identical to the original, frame for frame.

</details>

<details>
<summary>The sound only comes out of one side. Which setting fixes it?</summary>

Set **Source channel** to the side that actually has sound — `Left channel only`
or `Right channel only`. That copies the good channel to the mono track at full
level. Picking **Mix all channels** would also make it play in both ears, but
about 6 dB quieter, because it averages the good channel with the silent one.

</details>

<details>
<summary>How much smaller will the file be?</summary>

Only the audio shrinks, so it depends on how much of the file was audio. Going
from a 256 kbps stereo track to 128 kbps mono halves the audio; going to 32 kbps
at 16000 Hz cuts it to about an eighth. On a short talking-head clip that can be
a noticeable share of the total; on a long high-bitrate video the picture still
dominates.

</details>

<details>
<summary>What does "Difference (L − R)" do?</summary>

It subtracts the right channel from the left instead of adding them. Content
that is identical in both channels — typically a voice mixed to the centre —
cancels out, leaving only what differs. It's useful for checking phase problems
or confirming a "stereo" track isn't just two copies of the same mono signal. It
is a diagnostic, not the setting you want for normal playback.

</details>

<details>
<summary>Which video formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, mkv and webm are the common cases. The
output keeps the input's container and is named after the original with a
`-mono` suffix (e.g. `clip.mp4` → `clip-mono.mp4`). The input and output are
each capped at 25 MB.

</details>

<details>
<summary>Can I go back to stereo afterwards?</summary>

Not meaningfully. Once the channels are mixed into one, the separation is gone —
duplicating a mono track into two identical channels gives you a stereo
container but no stereo image. Keep the original if you might need it.

</details>
