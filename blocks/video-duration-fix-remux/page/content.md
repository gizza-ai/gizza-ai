## About this tool

Use this tool when a video plays fine but its **duration is missing, shows as
`Infinity`, jumps to `0:00`, or is simply wrong**. This is common with clips
recorded by the browser `MediaRecorder` API, screen recorders, and interrupted
downloads: the packets are intact but the container header never got a correct
duration written to it.

The fix is a **remux** — ffmpeg reads every packet with stream copy (`-c copy`)
and writes a brand-new container header with the real duration and index. Because
nothing is decoded or re-encoded, the audio and video are copied bit-for-bit and
there is no quality loss. Everything runs locally in your browser; the file is
never uploaded.

## Worked example

Upload a screen-recording `capture.webm` whose duration reads as `Infinity`,
leave **Output container** on `keep`, tick **Regenerate timestamps**, and
download `capture-duration-fixed.webm`. The rebuilt file reports its true length
(for a two-second clip, `ffprobe` now shows `duration=2.028000` instead of
`N/A`), while the VP9/Opus streams are unchanged.

To make a web-ready file at the same time, choose **Output container = mp4** and
keep **Web fast-start** on: the `moov` atom (the index that holds the duration) is
moved to the front of the file so players and websites read the correct length
immediately and the clip streams progressively.

## Options

- **Output container** — `keep` rebuilds the same container as your input and is
  the safest lossless choice. `mp4`, `mkv`, `mov`, and `webm` remux into that
  container instead. This is stream copy, so the codecs must fit the container
  (H.264/H.265/AAC → mp4/mov/mkv; VP8/VP9/Opus → webm/mkv/mp4). `mkv` is the most
  tolerant target.
- **Web fast-start** — MP4/MOV only. Moves the index to the front of the file
  (`-movflags +faststart`) for progressive playback and immediate duration
  reads. It is ignored for mkv/webm.
- **Regenerate timestamps** — adds `-fflags +genpts` to rebuild missing or broken
  presentation timestamps before remuxing. Turn this on when the duration is
  `Infinity`, `0:00`, or `N/A`.

## Limits and edge cases

- Maximum input size is 64 MB (browser-local processing).
- This is a container repair, not a transcode: it cannot change resolution,
  bitrate, or codec, and it cannot recover a truly corrupt/incomplete stream.
- Remuxing into a container the codec doesn't support (e.g. H.264 into `webm`)
  fails by design — use `keep` or `mkv` for the safest path.
- If a file has genuinely no timestamps at all, the recomputed duration is
  derived from the packet count and frame rate and may differ from the original
  intended length by a frame or two.

## FAQ

<details>
<summary>Why does my recorded WebM say the duration is Infinity?</summary>

Recorders that use the browser `MediaRecorder` API write the file header before
they know how long the recording will be, so the duration field is left empty and
players report `Infinity`. Remuxing reads the actual packets and writes the real
duration into a fresh header, which is exactly what this tool does — enable
**Regenerate timestamps** for these files.

</details>

<details>
<summary>Does fixing the duration reduce quality?</summary>

No. The remux uses stream copy (`-c copy`), so the encoded audio and video packets
are copied unchanged into a new container. Nothing is decoded or re-encoded, so the
result is bit-for-bit identical apart from the corrected container metadata.

</details>

<details>
<summary>What is "web fast-start" and when should I use it?</summary>

MP4 and MOV files store an index called the `moov` atom. If it sits at the end of
the file, a browser must download the whole file before it knows the duration or
can seek. Fast-start (`-movflags +faststart`) rewrites the file with that index at
the front, so websites can play and scrub it progressively. It only applies to
MP4/MOV output.

</details>

<details>
<summary>Can I change the exact duration to a specific value?</summary>

No. This tool repairs the duration to match the real content by rebuilding the
container; it does not let you set an arbitrary duration. Setting a made-up length
without re-encoding would either truncate the stream or desynchronize audio and
video, so it is intentionally out of scope.

</details>
