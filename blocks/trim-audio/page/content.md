## Trim audio in your browser

Pick an audio file, set a start and end time in seconds, and choose what to do
with that selection: **keep** extracts just the selection (the classic
ringtone cut), **remove** deletes it and joins what's left (drop an ad break or
a cough from a recording). Leave **End** empty to trim to the end of the track
— set only **Start** to `15` and you've dropped the first 15 seconds. The trim
runs entirely in your browser with ffmpeg compiled to WebAssembly, so your
audio is never uploaded to a server.

### Worked example

Cut a 30-second ringtone out of a song: upload the song, set **Start** to `15`,
**End** to `45`, leave **Mode** on `keep` and **Format** on `mp3`, and tick
**Fade edges** so the clip doesn't start or end with a click. The result is a
30-second `mp3` (192 kbps) named like the original with a `-trimmed.mp3`
suffix. To instead delete seconds 15–45 from the song and keep the rest,
switch **Mode** to `remove`. To cut a rambling intro off a recording, set
**Start** to where the good part begins and leave **End** empty.

### Modes

- **keep** — output is exactly the `[start, end]` selection, sample-accurate.
- **remove** — output is everything *except* the selection. The cut lands on
  the nearest audio frame (roughly 20–40 ms), which is inaudible for most
  edits.

### Formats

- **MP3** — lossy at 192 kbps; small and playable everywhere (the default).
- **WAV** — lossless 16-bit PCM; largest, ideal for further editing.
- **OGG** — lossy Vorbis at 192 kbps; open format, good quality per byte.
- **FLAC** — lossless and compressed; smaller than WAV, still a perfect copy.
- **M4A** — AAC in an mp4 container; good quality at small sizes.

### Limits and edge cases

- Input files up to 10 MiB; anything ffmpeg can decode works (mp3, wav, flac,
  m4a/aac, ogg, opus, and most video containers' audio tracks).
- The audio is always re-encoded (the trim filters require it), so `keep` with
  the same format still writes a fresh 192 kbps mp3 rather than a bit-exact
  copy.
- **Fade edges** applies up to a 0.5 s fade-in and fade-out inside the kept
  selection (shorter on clips under 2 s so the fades never overlap; a flat
  0.5 s each way when **End** is empty, since the length isn't known up
  front). It only works with `keep` mode — in `remove` mode the output length
  isn't known up front, so the option reports an error instead of silently
  doing nothing.
- **End** empty (or 0) means "to the end of the track"; otherwise `end` must
  be greater than `start`. Times beyond the file's real length are clamped by
  ffmpeg to the end of the audio.
- Start `0` with an empty **End** selects the whole file — that's nothing to
  trim, so the tool asks you to set a start and/or end instead of silently
  re-encoding.

## FAQ

<details>
<summary>How do I cut out the middle of a recording, not keep it?</summary>

Set **Mode** to `remove`. The selection between your start and end times is
deleted and the audio before and after it is joined together — handy for
removing an ad break, a long pause, or a cough.

</details>

<details>
<summary>How do I cut off just the intro (or outro)?</summary>

For an intro: set **Start** to where the good part begins and leave **End**
empty — everything from start to the end of the track is kept. For an outro:
switch **Mode** to `remove`, set **Start** to where the ending begins, and
leave **End** empty — everything from there to the end is deleted.

</details>

<details>
<summary>Which format should I pick?</summary>

`mp3` (the default) plays everywhere and is small — right for ringtones and
voice clips. Pick `wav` or `flac` if you plan to edit the clip further and
want a lossless copy; pick `m4a` for small files on Apple devices.

</details>

<details>
<summary>Why is my trimmed file re-encoded instead of a bit-exact copy?</summary>

Sample-accurate trimming and edge fades use ffmpeg audio filters, and filters
always decode and re-encode the stream. At 192 kbps mp3 (or lossless wav/flac)
the quality loss is negligible for a single edit.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
