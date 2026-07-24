## About this tool

**Video Audio Dynamics Compressor** evens out a video's soundtrack so quiet and loud passages sit closer together. It uses ffmpeg's `acompressor` audio filter, copies the video stream unchanged, and re-encodes only the audio track.

Use it when a screen recording, lecture, interview, or phone clip jumps between hard-to-hear speech and loud bursts. Pick a preset:

- **Light** keeps most of the original dynamics and gently tames peaks.
- **Medium** is the default, a balanced broadcast-style levelling pass.
- **Heavy** is more aggressive and pulls quiet/loud passages much closer together.

**Apply make-up gain** is on by default. It restores some loudness after compression; turn it off when you only want to reduce peaks without lifting the whole track.

### Limits and edge cases

- Dynamic-range compression is different from file-size compression. To make a file smaller, use a video or audio compression tool instead.
- The video track is copied (`-c:v copy`), so pixels, resolution, frame rate, and visual quality are not changed.
- The audio track is re-encoded because the compressor rewrites samples.
- WebM outputs use Opus audio; MP4/MOV/M4V/MKV outputs use AAC.
- Very noisy clips may still need noise reduction before compression.

### Worked example

For a lecture where the speaker turns away from the microphone, choose **Medium levelling** with make-up gain on. The generated ffmpeg plan applies:

```text
-af acompressor=threshold=-24dB:ratio=4:attack=10:release=200:makeup=4
```

The resulting video keeps the original picture while the audio is easier to listen to.

## FAQ

<details>
<summary>Is this the same as making the video file smaller?</summary>

No. This compresses the **dynamic range** of the audio — the gap between quiet and loud moments. It is about listenability, not file size. For smaller files, use a video compression tool.

</details>

<details>
<summary>Will it change the picture quality?</summary>

No. The video stream is copied, so the image is not re-encoded. Only the audio stream is processed and re-encoded.

</details>

<details>
<summary>Which preset should I choose?</summary>

Start with **Medium**. Use **Light** when you want a natural result, and **Heavy** when speech levels vary a lot or the clip needs aggressive levelling.

</details>

<details>
<summary>What does make-up gain do?</summary>

Compression reduces peaks and can make the track feel quieter. Make-up gain raises the processed audio back up. Turn it off when you only want peak control without increasing the overall level.

</details>
