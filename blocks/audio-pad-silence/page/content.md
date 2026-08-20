## Add silence to the start and end of an audio clip

Upload a clip, say how many seconds of silence go **before** it and how many go **after** it, and download the padded file. The clip itself is not touched — nothing is trimmed, sped up or re-timed — so the result is exactly `before + original + after` seconds long. Both ends are padded in a single pass, which is what you want for IVR and phone prompts (systems that clip the first syllable need a lead-in), radio and podcast beds, alignment against a video edit, or a ringtone that needs breathing room at the top.

Under the hood it is one ffmpeg pass: `adelay` shifts every channel later by the lead-in you ask for, and `apad=pad_dur=` appends a bounded run of digital silence at the tail. A side left at `0` is dropped from the chain entirely.

### Worked example

You have `greeting.mp3`, a 4.0-second recorded phone greeting, and the IVR system swallows the first word. Upload it, set **Silence before** to `0.5`, set **Silence after** to `1.5`, and choose `mp3`. The exported `greeting-padded.mp3` runs **6.0 seconds**: half a second of silence, then your 4-second greeting untouched, then a second and a half of silence. Set **Silence before** to `2` and **Silence after** to `0` instead and you get a 6.0-second file with a two-second run-up and no tail.

### Common padding lengths

| Use | Before | After |
| --- | --- | --- |
| IVR / phone prompt | `0.25`–`0.5` | `0.5`–`1` |
| Podcast or video intro gap | `1`–`2` | `0` |
| Ad spot breathing room | `0.5` | `0.5` |
| Ringtone lead-in | `0` | `2`–`5` |

The **Try:** chips above the form fill these in for you in one click.

### Limits and edge cases

- Input and output are capped at 10 MiB (the file stays in your browser).
- Each side accepts `0` to `3600` seconds (1 hour). Decimals are allowed down to `0.001` (1 ms) — the lead-in is applied in whole milliseconds, so anything smaller is rejected rather than silently ignored.
- At least one of the two fields must be greater than `0`; padding by nothing is reported as an error instead of returning a re-encoded copy.
- The audio is re-encoded (not stream-copied) so the silence joins the decoded audio cleanly with no click at the seam. Embedded album art is dropped.
- Output formats: mp3 (192 kbps, default), wav, ogg, flac, or m4a. Channel count and sample rate are inherited from the input — use `audio-resampler` or `audio-to-mono` if you need to change those.
- Silence can only go at the ends. To insert a gap in the *middle* of a clip, split it with `trim-audio` and pad the pieces.

## FAQ

<details>
<summary>Can I add silence to both the beginning and the end at the same time?</summary>

Yes — that is the point of having two fields. Set **Silence before** and **Silence after** together and one pass adds both, so a 10-second clip with `2` before and `3` after comes back as a 15-second file. Tools that expose a single "position" dropdown make you run the file twice; here it is one run.

</details>

<details>
<summary>How long can the silence be, and can I use fractions of a second?</summary>

Each side takes anything from `0` to `3600` seconds. Decimals work — `0.5` is half a second, `0.25` is a quarter — down to `0.001` (one millisecond), which is the resolution the lead-in filter works at. In practice the 10 MiB output cap bites first: an hour of silence in wav or flac will exceed it long before the 3600-second ceiling.

</details>

<details>
<summary>Does padding re-encode and degrade my audio?</summary>

The file is decoded and re-encoded, because appending silence to a compressed stream without decoding it produces a click at the seam. The clip's own samples are not otherwise altered. If you want to avoid a second lossy generation, pick `wav` (uncompressed PCM) or `flac` (lossless) as the output format instead of the `mp3` default.

</details>

<details>
<summary>Why do I get an error when both fields are 0?</summary>

Because the tool would have nothing to do — it would just hand back a re-encoded copy of your file, which is almost never what was meant. Set at least one side to a number greater than `0`. If you actually want a format conversion with no padding, use `audio-convert`.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page runs ffmpeg WebAssembly in your browser tab, so your audio never leaves your device. The CLI and chat versions run the same ffmpeg plan through the local tool runtime.

</details>
