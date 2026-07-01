## Extract audio from a video in your browser

Pick a video, choose MP3 or WAV, and get just the audio track — no video. The
extraction runs entirely in your browser with ffmpeg compiled to WebAssembly,
so your video is never uploaded to a server.

### Formats

- **MP3** — lossy, small, and playable everywhere (libmp3lame). Pick a bitrate
  from 32 to 320 kbps; 192 kbps (the default) is a good balance of size and
  quality.
- **WAV** — lossless 16-bit PCM. Larger files, but a perfect copy of the
  decoded audio — ideal for editing or archiving. (The bitrate field is ignored
  for WAV.)

### Tips

- Leave the format blank to default to MP3 at 192 kbps.
- The output keeps the original filename with the new audio extension.
- Works offline once the page has loaded — nothing leaves your device.

## FAQ

<details>
<summary>Which video files can I extract audio from?</summary>

Anything the bundled ffmpeg build can decode — MP4, MOV, WebM, MKV, AVI, and most
other common containers. The video stream is simply dropped (`-vn`) and the audio
track is re-encoded to your chosen output format.

</details>

<details>
<summary>Should I choose MP3 or WAV?</summary>

MP3 (the default) is lossy but small and plays everywhere; pick a bitrate between
32 and 320 kbps — 192 kbps is a good balance. WAV is lossless 16-bit PCM: much
larger, but a perfect copy of the decoded audio, which is what you want before
editing in a DAW. The bitrate setting is ignored for WAV.

</details>

<details>
<summary>What if I enter a bitrate like 500 kbps?</summary>

Bitrates outside the libmp3lame-supported 32–320 kbps range are rejected with a
clear error rather than silently changed, so the file you get always matches the
settings you asked for.

</details>

<details>
<summary>Is my video uploaded to a server?</summary>

No. The extraction runs in your browser with ffmpeg compiled to WebAssembly. The
video never leaves your device, and once the page has loaded the tool even works
offline.

</details>
