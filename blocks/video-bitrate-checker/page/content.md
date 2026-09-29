## About this tool

This checker answers two questions about a media file: **what bitrate is it actually running at**, and **is that inside the range you need?**

The overall bitrate is the number every player and every upload guide means: file size × 8 ÷ duration. The per-stream numbers are *measured*, not copied out of a header — the container is demuxed and the payload bytes carried for each track are totalled, then divided by that track's own duration. That reproduces `ffprobe -show_entries stream=bit_rate` to the kilobit, and it still works on WebM and Matroska, which store no per-stream bitrate at all.

Leave both threshold boxes at `0` and the tool simply reports (`status: INFO`). Fill in a minimum, a maximum, or both and you get `PASS` or `FAIL`, the amount you are over or under, and the exact bitrate that was compared.

Nothing is decoded, no re-encoding happens, and the file is read inside the page — it is never uploaded.

### Worked example

A 2-second 128×128 H.264 + AAC MP4 of 23,593 bytes, checked with no range set:

```json
{
  "status": "INFO",
  "reason": "not_checked",
  "container": "MP4 / MOV / M4A (ISO BMFF)",
  "duration_seconds": 2.0,
  "file_bytes": 23593,
  "file_size_human": "23.0 KiB",
  "overall_bitrate_kbps": 94.4,
  "video_bitrate_kbps": 19.5,
  "audio_bitrate_kbps": 64.8,
  "container_overhead_kbps": 10.1,
  "streams": [
    { "index": 1, "kind": "video", "codec": "H.264 / AVC", "bitrate_kbps": 19.5,
      "width": 128, "height": 128, "frame_rate": 10.0, "packets": 20 },
    { "index": 2, "kind": "audio", "codec": "AAC", "bitrate_kbps": 64.8,
      "sample_rate": 48000, "channels": 1, "packets": 95 }
  ]
}
```

23,593 × 8 ÷ 2.0 = 94.4 kbit/s overall. The two streams account for 84.3 of that; the remaining 10.1 kbit/s is `moov`, the sample index and muxer padding — normal, and proportionally much larger on a tiny clip than on a real one.

Set **Maximum** to `50` with units `kbps` and the same file comes back `FAIL` / `too_high`, 44.4 kbit/s over the ceiling. Switch **Apply the range to** to `video` and it passes: the video track alone is only 19.5 kbit/s.

### Limits and edge cases

- **Duration is required.** Bitrate is bytes ÷ time, so a file whose container records no usable duration (a stopped-mid-recording WebM, a damaged `moov`) cannot be rated. Remux it first.
- **Truncated files.** A partial download whose header is missing or cut short is rejected with an error rather than silently measured against a wrong duration.
- **Measured, not nominal.** These are the bitrates the file actually achieves, not the `-b:v` target the encoder was given. A VBR encode routinely lands under its target; that is the encoder working, not a fault.
- **Container overhead counts.** `overall` includes headers and index tables, so it is always a little higher than the streams added together. Platform specs are usually written against the overall number.
- **Averages, not peaks.** This is the average over the whole file. A clip can average 5 Mbit/s and still spike far above it in a busy scene; a peak/VBV analysis is a different tool.
- **What can be read:** MP4/MOV/M4A, Matroska/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3 and AAC/ADTS. Stream kind, codec name and picture size are read from the MP4 and Matroska track tables; in other containers a stream is reported with what the demuxer knows.
- **Size cap.** The command-line and chat surfaces cap input at 32 MB because the whole file has to fit in the sandbox alongside the demuxer. The browser page is limited only by your own device's memory.

## FAQ

<details>
<summary>Why is the overall bitrate higher than the video and audio bitrates added together?</summary>

Because a container is more than its streams. The difference — reported as `container_overhead_kbps` — is the file's headers, the sample index that lets a player seek, and any padding the muxer wrote. On a short clip that can be 10% or more of the file; on a feature-length video it is usually well under 1%. Platform requirements are normally written against the overall figure, which is why it is the default target.

</details>

<details>
<summary>Does this match what ffprobe reports?</summary>

Yes, for both the overall figure and the per-stream ones. The overall bitrate uses the container's declared play time, the same duration ffprobe prints, so the two agree. The per-stream numbers are measured by adding up each track's packet payload and dividing by that track's duration, which reproduces ffprobe's `stream=bit_rate` to the kilobit.

The interesting case is WebM and Matroska: those containers never store a per-stream bitrate, so ffprobe answers `N/A`. Measuring the packets gives a real number there anyway.

</details>

<details>
<summary>What should I set the minimum and maximum to?</summary>

It depends on what you are checking against, which is why there are no built-in defaults — leave both at `0` and the tool just reports. The preset buttons cover the common cases: a 1080p30 upload is generally expected to carry roughly 6-9 Mbit/s of video and 4K30 around 35-45, a web delivery cap is often 5 Mbit/s overall, and ad specs are usually written in kbit/s. Always check the current published requirements for whatever platform you are delivering to; these presets are starting points, not rules.

</details>

<details>
<summary>Can I check just the audio track?</summary>

Yes — set **Apply the range to** to `audio` and the range is applied to the audio streams only (summed, if there is more than one). The same works for `video`. Every bitrate is still reported either way; the setting only decides which one gets the PASS/FAIL. Asking for `video` on a file that has no video stream is an error rather than a silent failure.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The page reads the file you choose directly in the browser and runs the whole measurement there in WebAssembly. Nothing is sent to a server, and the file is only demuxed — never decoded and never re-encoded — so even a large clip is read quickly.

</details>
