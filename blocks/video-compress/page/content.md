## About this tool

Make a video file smaller without leaving your browser. Pick a video, choose a
quality level, and it's re-encoded locally — the file never leaves your device.

## How it works

The tool runs a single-pass [CRF](https://trac.ffmpeg.org/wiki/Encode/H.264#crf)
("constant rate factor") re-encode with ffmpeg, compiled to WebAssembly. The
video is encoded to **H.264** with **AAC** audio, keeping the original container
format (mp4 stays mp4, webm stays webm, and so on).

CRF is a quality knob: **lower CRF means higher quality and a larger file;
higher CRF means a smaller file with more visible compression**. The default of
**28** is a good "noticeably smaller, still watchable" starting point. The value
is clamped to the **18–34** range.

## Notes

- **Quality, not a target size.** This is a single-pass encode, so you choose a
  quality level rather than an exact output size. Hitting a precise byte target
  (for example "under 10 MB") reliably needs a two-pass encode — a planned
  follow-up.
- **Private by design.** Everything runs in your browser. No upload, no server.
- Re-encoding an already-heavily-compressed clip may not shrink it much; raise
  the CRF for a smaller file.

## FAQ

<details>
<summary>Can I target an exact output size, like "under 10 MB"?</summary>

Not with this tool — it's a single-pass CRF encode, so you pick a *quality*
and the size follows from the content. If the result is too big, raise the
CRF and re-run. Hitting a precise byte budget reliably requires a two-pass
encode, which is a planned follow-up.

</details>

<details>
<summary>What CRF value should I use?</summary>

Start at the default **28**. As a rule of thumb each +6 CRF roughly halves
the bitrate: 23–24 looks close to the source, 28 is noticeably smaller but
still watchable, and 32–34 is for when size matters most. Values outside
18–34 are clamped into that range.

</details>

<details>
<summary>Do the codecs or file format change?</summary>

The container is preserved — an `.mp4` stays `.mp4`, a `.webm` stays `.webm` —
but the streams inside are always re-encoded to H.264 video (libx264, medium
preset) with AAC audio, whatever the original codecs were.

</details>

<details>
<summary>Why does compressing take a while?</summary>

The whole encode runs on your own CPU via ffmpeg compiled to WebAssembly —
that's what keeps the video on your device. Encoding time grows with
duration and resolution, so a long 4K clip can take several minutes where a
short phone clip finishes in seconds.

</details>
