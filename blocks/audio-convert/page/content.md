## Convert audio in your browser

Pick an audio file and a target format — mp3, wav, ogg, flac or m4a. The
conversion runs entirely in your browser with ffmpeg compiled to WebAssembly,
so your audio is never uploaded to a server. Anything ffmpeg can decode works
as input: mp3, wav, flac, m4a/aac, ogg, opus and more.

### Worked example

Turn a voice memo into a small shareable file: upload `memo.wav`, set **Target
format** to `mp3` and leave **Bitrate** at `192` — the result is `memo.mp3` at
192 kbps, a fraction of the WAV's size. Going the other way (mp3 → `wav`)
gives you an uncompressed file that any editor can open, though it can't
restore quality the mp3 encoding already discarded.

### Formats

- **MP3** — lossy; small and playable everywhere. Bitrate 32–320 kbps.
- **WAV** — lossless 16-bit PCM; largest, ideal for editing. No bitrate.
- **OGG** — lossy Vorbis; open format, good quality per byte. Bitrate applies.
- **FLAC** — lossless and compressed; smaller than WAV, still a perfect copy.
- **M4A** — lossy AAC in an mp4 container; good quality at small sizes.

### Limits and edge cases

- Input files up to 10 MiB.
- **Bitrate** only applies to the lossy targets (mp3, ogg, m4a); wav and flac
  are lossless and ignore it. Values outside 32–320 kbps are clamped.
- Converting lossy → lossless (e.g. mp3 → flac) produces a bigger file but
  cannot restore quality the lossy encoding already removed.
- Embedded album art is dropped: cover images ride along as a video stream,
  which audio-only formats like wav can't carry.
- The output keeps the original filename with the new extension
  (`song.mp3` → `song.wav`).

## FAQ

<details>
<summary>Which format should I pick?</summary>

`mp3` for maximum compatibility, `m4a` for small high-quality files on Apple
devices, `ogg` for open-source pipelines, and `wav` or `flac` when you need a
lossless copy for editing or archiving.

</details>

<details>
<summary>Does converting mp3 to flac or wav improve the quality?</summary>

No. Lossless targets preserve exactly what's in the source — quality the mp3
encoder already discarded is gone for good. Convert to flac/wav when a tool
needs that container, not to "upgrade" audio.

</details>

<details>
<summary>What bitrate should I use for mp3?</summary>

192 kbps (the default) is transparent for most listeners and music. Use 128
kbps for voice recordings where size matters, and 256–320 kbps for archiving
music you care about.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
