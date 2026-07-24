## Make an audio clip an exact number of seconds

Upload a clip and set a target length. If the clip is **shorter** than the target it is padded with silence to reach exactly that duration; if it is **longer** it is trimmed to exactly that duration. Both cases come from one ffmpeg pass: `apad=whole_dur=T` extends the stream to the target length, and `-t T` guarantees the output is exactly `T` seconds. It's the tool you want when a spot has to fill a fixed slot — a 15- or 30-second ad break, a looping bumper, a fixed-length intro — no matter how long the source happens to be.

### Worked example

You have `voiceover.mp3` that runs 24.6 seconds and it needs to fill a 30-second radio slot. Upload it, set **Target length** to `30`, leave **Pad silence at** on `End`, and choose `mp3`. The exported `voiceover-fit.mp3` is exactly 30.000 seconds: your voice plays from the start and 5.4 seconds of clean silence follows. Switch **Pad silence at** to `Start` instead and the silence lands before the voice, so the audio ends precisely on the 30-second mark — handy when the important beat has to hit the end of the slot.

### Padding vs. trimming

**Pad silence at** only decides where silence is *added* when the clip is shorter than the target. When the clip is *longer* than the target it is always trimmed from the end, whichever pad position you pick — so a 40-second clip fit to `30` returns its first 30 seconds either way. To cut a specific in/out selection rather than fit-to-length, use `trim-audio`; to repeat a short sound until it fills a length, use `audio-loop`.

### Limits and edge cases

- Input and output are capped at 10 MiB (the file stays in your browser).
- Target length must be greater than `0` and at most `3600` seconds (1 hour).
- The audio is re-encoded (not stream-copied) so appended silence joins the decoded audio cleanly with no click at the seam. Embedded album art is dropped.
- Output formats: mp3 (192 kbps, default), wav, ogg, flac, or m4a.

## FAQ

<details>
<summary>What happens if my clip is already longer than the target?</summary>

It is trimmed to exactly the target length, cutting from the end. For example, a 45-second clip fit to `30` returns its first 30 seconds. The **Pad silence at** setting has no effect in this case — it only controls where silence goes when the clip is *shorter* than the target.

</details>

<details>
<summary>Can I add the silence before the clip instead of after it?</summary>

Yes. Set **Pad silence at** to `Start` and the silence is prepended, so the clip ends exactly on the target mark. Leave it on `End` (the default) to append silence after the clip so it starts at zero. Padding at both ends or in the middle isn't supported here.

</details>

<details>
<summary>Is the output exactly the length I ask for?</summary>

Yes — the output is trimmed to precisely the target with `-t`, so a request for `30` produces a 30.000-second file. The one practical ceiling is the 10 MiB output cap, so very long targets in a lossless format (wav/flac) may exceed the limit before they reach an hour.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page runs ffmpeg WebAssembly in your browser tab, so your audio never leaves your device. The CLI and chat versions run the same ffmpeg plan through the local tool runtime.

</details>
