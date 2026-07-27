## Shrink a video's audio without touching the picture

Pick a video, choose a target **audio bitrate**, and get the same clip back
with a smaller, re-encoded soundtrack. The picture is **stream-copied** (not
re-encoded), so it stays byte-for-byte identical and processing is fast — only
the audio track is re-encoded to the bitrate you pick. Everything runs in your
browser; nothing is uploaded.

### Why change the audio bitrate?

A lot of clips carry a soundtrack recorded at 256–320 kbps (or higher) when the
audio is only speech. Dropping it to 96–128 kbps can shave megabytes off the
file with no visible change to the video and little audible change to talking.
Because the video is copied untouched, the whole file is only as big as its
picture plus the smaller audio you chose.

### Picking a bitrate

- **64–96 kbps** — voice, podcasts, screen recordings, narration. Smallest
  files; fine for speech.
- **128 kbps** — the default. A good balance for stereo music or mixed speech
  and music.
- **160–192 kbps** — keeps music sounding clean.
- **256–320 kbps** — near-transparent quality; use when the soundtrack matters
  and file size doesn't.

**Worked example:** a 40 MB screen recording whose audio is a 320 kbps stereo
track of someone talking. Load `recording.mp4`, pick **96 kbps**, and you get
`recording-audio.mp4` — the same video, an audio track a fraction of the size,
and a noticeably smaller overall file.

### Notes and limits

- The video stream is copied losslessly; the output keeps the same container
  (mp4 → mp4, webm → webm). WebM audio is re-encoded to Opus, everything else
  to AAC.
- Bitrate is **constant (CBR)** — the same target across the whole clip.
- This lowers (or raises) the audio bitrate; it does not re-encode or shrink the
  picture. To make the *video* smaller, use a video-compression tool instead.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

## FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.

</details>

<details>
<summary>Will changing the audio bitrate hurt the video quality?</summary>

No. Only the audio is re-encoded; the **picture is stream-copied without
re-encoding**, so the video quality is identical to the original.

</details>

<details>
<summary>Does a lower bitrate make the whole file smaller?</summary>

Yes, for the audio portion. The video stream is copied unchanged, so the total
saving equals the difference between the old and new audio bitrates times the
clip length. On a talky clip with a high-bitrate soundtrack that can be several
megabytes.

</details>

<details>
<summary>Can I raise the bitrate to improve quality?</summary>

You can pick a higher value, but re-encoding lossy audio to a higher bitrate
cannot recover detail that was already discarded — it only makes the file
bigger. Raising the bitrate makes sense only when the source audio is
high-quality to begin with.

</details>

<details>
<summary>Which video formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, mkv and webm are the common cases. The
output keeps the input's container and is named after the original with an
`-audio` suffix (e.g. `clip.mp4` → `clip-audio.mp4`). WebM audio is re-encoded
to Opus and everything else to AAC. The input and output are each capped at
25 MB.

</details>
