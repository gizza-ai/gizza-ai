## About this tool

Phones, screen recorders, OBS and game capture cards usually write **variable frame rate** (VFR) video: instead of a frame every fixed fraction of a second, each frame carries its own timestamp and the gaps between them stretch and shrink with whatever the encoder was doing. Players handle that fine. Editors, NLEs and most conversion tools do not — they assume an even cadence, so the timing error accumulates and you get the classic symptom: *the audio starts in sync and drifts further out the longer the clip runs*.

This tool rebuilds the clip as **constant frame rate** (CFR). ffmpeg regenerates the presentation timestamps on an exact grid (`-fps_mode cfr`), duplicating a frame where the source had a long gap and dropping one where it had a burst, so every frame lands exactly where an editor expects it. It runs locally through the browser's ffmpeg build — the video never leaves your machine.

Leave **Constant frame rate** on `auto` unless you have a reason not to. Auto keeps the source's own nominal rate and only makes the timing even, which neither drops nor invents frames beyond what the irregular timestamps already force. Pick an explicit rate when your timeline demands one: 23.976 or 24 for film, 25 and 50 for PAL, 29.97/30 and 59.94/60 for NTSC and web.

**Re-lock audio** is on by default. It resamples the audio through `aresample=async=1:first_pts=0`, which pads or trims the track so it stays anchored to the new video timeline from the very first frame. Turn it off only when you want the original audio stream copied bit-for-bit and you are confident it was already in sync.

### Worked example

A 3-minute OBS capture named `raid-night.mp4` plays fine in VLC, but in the editor the commentary is a third of a second late by the end. Choose:

- Constant frame rate: `60 — gameplay and smooth screen capture`
- Quality: `High`
- Re-lock audio: checked

The output is `raid-night-cfr.mp4`: H.264 `yuv420p` at a true 60 fps, with AAC audio re-anchored to the first frame. Drop it on the timeline and the sync holds from start to finish.

### Limits and edge cases

- Re-timing frames means the picture must be re-encoded — this is not a lossless remux. The quality preset picks the x264 `-crf` (18 / 20 / 24), all at `-preset medium`.
- Re-locking the audio applies a filter, and a filtered stream cannot be stream-copied, so audio is re-encoded to AAC at 192 kbps whenever the switch is on.
- `mp4`, `mov`, `m4v` and `mkv` inputs keep their container. Anything else — `webm` in particular — comes out as MP4, because the container has to be able to hold H.264/AAC.
- Raising the rate above the source's (for example forcing 60 on a 30 fps capture) duplicates frames. It does not create new motion or make the clip look smoother.
- Browser runs are best for short clips. Long or high-resolution videos can exceed the browser ffmpeg runtime's memory limits — use the CLI for those.

## FAQ

<details>
<summary>How do I know my video is variable frame rate?</summary>

The usual tells are behavioural: the audio drifts further out of sync the longer the clip plays in your editor, the reported duration disagrees with the frame count, or cuts land a frame or two off. Anything recorded on a phone, or captured with OBS, ShadowPlay, a screen recorder or a browser's MediaRecorder is VFR by default. If `ffprobe` reports a different `r_frame_rate` and `avg_frame_rate`, that is the direct confirmation.

</details>

<details>
<summary>Should I leave the frame rate on auto or pick a number?</summary>

Leave it on `auto` unless your timeline requires a specific rate. Auto keeps the source's own nominal rate and only makes the spacing even, which is the smallest change that fixes the problem. Pick an explicit rate when you are conforming footage to a project — 25 for a PAL timeline, 23.976 for film, 60 for gameplay — or when you are mixing clips that were recorded at different rates.

</details>

<details>
<summary>Why does the video get re-encoded? Can't it just be remuxed?</summary>

No. Constant frame rate is a property of the frames' timestamps and their spacing, and making the spacing even requires duplicating frames across long gaps and dropping them across bursts. That changes which frames exist, so the video stream has to be rebuilt. The tool uses H.264 with `yuv420p` — the combination every editor and browser can decode — and defaults to CRF 20, which is visually close to transparent for typical capture footage.

</details>

<details>
<summary>What does "re-lock audio" actually change?</summary>

It runs the audio through `aresample=async=1:first_pts=0`. The `async=1` part stretches or squeezes the track to correct drift against the new video timeline, and `first_pts=0` anchors it at the very first frame, so a track that started late does not stay late. With the box unchecked the original audio is stream-copied untouched (unless the container has to change), which preserves the exact bits but keeps any existing offset.

</details>

<details>
<summary>Will converting to CFR make my video look choppy?</summary>

Only if the source genuinely dropped frames. Where the recorder stalled, CFR has to hold the last frame for the length of the gap — that stutter was already in the recording, it is just now expressed as duplicate frames instead of a long timestamp gap. What CFR fixes is the timing model, not missing footage. Keeping `auto` minimises the number of duplicated and dropped frames.

</details>

<details>
<summary>Does anything get uploaded?</summary>

No. The page loads an ffmpeg build compiled to WebAssembly and runs the conversion in your browser tab. The file you pick is read locally and the result is handed straight back to the page for download.

</details>
