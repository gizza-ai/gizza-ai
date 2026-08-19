## About this tool

A **keyframe** (I-frame) is a fully self-contained picture; every other frame in a video is
described relative to its neighbours. The distance between keyframes — the **GOP**, or group of
pictures — decides how precisely a player can seek, and where a streaming packager is allowed to
cut a segment. Most cameras, screen recorders and encoders leave that distance to the encoder,
which places keyframes wherever it likes and inserts extra ones at scene changes.

This tool re-encodes a clip with a **fixed** cadence instead: one keyframe every N seconds, or an
exact GOP size in frames. That is what HLS/DASH packaging, smooth scrubbing in an editor, and
frame-accurate remote seeking all depend on.

### Worked example

Upload a 10-second clip, leave **Keyframe interval** at `2` with unit **seconds**, and run it.
The tool builds and executes:

```
ffmpeg -i in.mp4 -force_key_frames expr:gte(t,n_forced*2) -sc_threshold 0 -flags +cgop \
  -c:v libx264 -preset medium -crf 23 -pix_fmt yuv420p -c:a aac -b:a 128k \
  -movflags +faststart out.mp4
```

The result is an MP4 with keyframes at 0 s, 2 s, 4 s, 6 s and 8 s — five evenly spaced entry
points. Switch the unit to **frames** and enter `60` and the flags become `-g 60 -keyint_min 60`
instead, which is exactly 2 seconds at 30 fps.

### Choosing an interval

- **1–2 seconds** — live and low-latency streaming; smallest startup delay, largest file.
- **2 seconds** — the streaming default most platforms converge on.
- **4–10 seconds** — on-demand video where bandwidth matters more than seek precision.
- **Frames** — when you need an exact GOP size to match a packager's segment length.

### Limits

- Input up to 50 MB; output up to 60 MB.
- Interval: 0.1–60 seconds, or 1–3000 frames.
- Quality: CRF 1–51 (lower is better). True-lossless CRF 0 is not offered — it produces files far
  beyond the size cap.
- The output is always re-encoded H.264/AAC in a progressive MP4 with `+faststart`. A keyframe
  cadence cannot be applied by stream copy, so some generation loss is unavoidable.

## FAQ

<details>
<summary>What keyframe interval should I use for streaming?</summary>

Two seconds is the safe default — it is what most live platforms recommend and it divides evenly
into the 2, 4 and 6 second segment lengths packagers use. Go down to 1 second only if you are
chasing low latency, and up to 4–10 seconds for on-demand content where a smaller file matters
more than seek precision.

</details>

<details>
<summary>Should I set the interval in seconds or frames?</summary>

Use **seconds** unless you have a reason not to. Seconds mode places keyframes by timestamp
(`-force_key_frames`), so it produces the right cadence no matter the frame rate — including
variable-frame-rate screen and phone recordings, where a frame count drifts. Use **frames** when a
tool downstream expects a specific GOP size; multiply your frame rate by the seconds you want
(30 fps × 2 s = 60 frames).

</details>

<details>
<summary>Why is there an option to allow extra keyframes at scene changes?</summary>

By default the tool passes `-sc_threshold 0`, which turns off scene-cut detection. Without that,
the encoder adds an unscheduled keyframe whenever the picture changes sharply — good for quality
on hard cuts, but it destroys the even spacing that segment alignment relies on. Tick the box if
you care about quality at cuts more than about a strictly fixed cadence.

</details>

<details>
<summary>What does "closed GOP" mean and should I leave it on?</summary>

In a closed GOP no frame refers to anything before its own keyframe, so a player can start
decoding at any keyframe and a stream can switch bitrates there cleanly. Open GOPs compress
marginally better but make those cut points less reliable. Leave it on unless you are optimising
purely for file size.

</details>

<details>
<summary>Does this change the file without re-encoding?</summary>

No. Keyframe positions are baked into the encoded video stream, so changing the cadence requires a
full re-encode — a stream copy can only preserve the keyframes that are already there. The quality
slider (CRF) controls how much is lost in that re-encode; 18–20 is close to visually lossless.

</details>

<details>
<summary>How is this different from making a fragmented MP4?</summary>

This tool outputs a normal progressive MP4 whose keyframes are evenly spaced. A fragmented MP4 is
a different container layout (a small init header followed by self-describing fragments) used by
Media Source Extensions and DASH/CMAF players. If that is what you need, use the fragmented-MP4
tool instead; if you need a regular file that simply seeks and segments cleanly, this is the one.

</details>
