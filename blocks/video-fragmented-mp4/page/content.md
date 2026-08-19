## About this tool

A progressive MP4 usually has one `moov` index for the whole file. A fragmented MP4 (fMP4) instead starts with a small initialization header and then stores media in `moof` + `mdat` fragments. That layout is what Media Source Extensions players, DASH byte-range workflows, and CMAF-style packaging expect.

This tool rewrites a video into a **single-file fragmented MP4**. In the default `copy` mode it stream-copies every track with ffmpeg, so there is no quality loss and no re-encode step. If the source codecs are not browser-friendly, switch to `h264` mode to re-encode video as H.264/yuv420p and audio as AAC.

### Worked example

For an MP4 that already uses browser-compatible codecs, use:

- **Conversion mode:** `copy`
- **Streaming profile:** `MSE / generic fMP4`
- **Minimum fragment duration:** `0`
- **sidx segment index:** off

The ffmpeg plan applies the key flags for a single-file fMP4: keyframe-aligned fragments, an empty initialization `moov`, and default-base-is-moof offsets. The output remains an `.mp4`, but it is organized as fragments instead of one progressive media data run.

### Limits and edge cases

- The output is one `.mp4` file. It does not create an HLS/DASH manifest, `init.mp4`, or multiple `.m4s` segment files.
- `copy` mode cannot create new keyframes. Fragment boundaries follow the keyframes already in the source. Use `h264` mode when you need regular 2-second keyframes.
- `fragment_duration` is a minimum target in seconds. Fragments stay keyframe-aligned, so exact lengths depend on the encoded keyframes.
- `faststart` is not exposed: with an empty initialization `moov`, the useful header is already at the front.
- A global `sidx` is optional because it adds overhead and is mainly useful for byte-range single-file DASH.

## FAQ

<details>
<summary>Is fragmented MP4 the same as moving moov to the front?</summary>

No. `faststart` moves one progressive MP4 index to the beginning. Fragmented MP4 creates an initialization header plus repeated `moof`/`mdat` fragments, which is a different container layout for streaming append and byte-range workflows.

</details>

<details>
<summary>Does copy mode reduce quality?</summary>

No. Copy mode uses ffmpeg stream copy, so encoded audio and video packets are not decoded or re-encoded. The container is rewritten, but the media quality is unchanged.

</details>

<details>
<summary>When should I use H.264 mode?</summary>

Use H.264 mode when the source codec is not accepted by your target browser/player, or when the source has sparse keyframes and you want more regular fragments. It is slower and lossy because it re-encodes the video.

</details>

<details>
<summary>Why are my fragments not exactly the duration I requested?</summary>

Fragments are cut on keyframes so each fragment starts at a random-access point. If the source has keyframes every five seconds, copy mode cannot safely produce exact two-second fragments. Re-encode with a two-second keyframe interval if you need that cadence.

</details>

<details>
<summary>Can this create an HLS or DASH manifest?</summary>

No. This block returns a single media file, which fits the gizza page and CLI surfaces. Manifest generation and multi-file segment output need a different artifact model.

</details>
