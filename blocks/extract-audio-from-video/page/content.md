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
