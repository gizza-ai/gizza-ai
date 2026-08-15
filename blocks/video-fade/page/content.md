## Fade a video in from black and out to black

Use this tool to soften how a clip starts and ends. The **picture** ramps up from
a solid colour — black by default — at the beginning, and ramps back down to that
colour at the end. The **sound** ramps out of and into silence over exactly the
same spans, so the two never drift apart. Set either side to `0` to skip it, pick
a different fade colour, or restrict the fade to just the picture or just the
sound. Everything runs in your browser and nothing is uploaded.

**Worked example:** load a 10-second `intro.mp4`, set **Fade in** to `1`, **Fade
out** to `1`, and **Exact clip length** to `10`. The first second lifts the
picture out of black while the audio rises from silence; the last second
(starting at 10 − 1 = 9 s) sinks both back down. The result is a 10-second
`intro-fade.mp4` — same length, same footage, softer ends.

### Why the clip length is asked for

A fade-out has to be placed at an absolute time: `clip length − fade out`. The
plan is built **before** the file is decoded, so the length can't be read off the
video first — type it in the **Exact clip length** box (your player's total-time
readout is the number you want). A fade-**in** always starts at `0`, so a
fade-in-only run needs no length at all; leave the box at `0`.

### What each control does

- **Fade in** / **Fade out** — length in seconds of each ramp, `0` to `30`. At
  least one of the two must be above `0`; both at `0` is rejected as a no-op.
  Their sum may not exceed the clip length.
- **Fade** — `Picture and sound` (default), `Picture only` (audio stays at full
  level), or `Sound only` (the picture is stream-copied untouched).
- **Fade colour** — any colour the ffmpeg filter accepts: a name (`black`,
  `white`), a hex value (`#101820` or `0x101820`), or `name@alpha` such as
  `black@0.8`. Ignored when you fade sound only.
- **Output quality** — the H.264 CRF used for the re-encoded picture: high
  (18), balanced (23, the default), or small (28). A lower number keeps more
  detail and produces a larger file.

### Notes and limits

- Fading the picture rewrites pixels, so that path **re-encodes** to H.264/AAC in
  an MP4 regardless of the input container. Quality loss is normal re-encode
  loss; pick high quality if the source is precious.
- **Sound only** is the lossless path: the video stream is copied bit-for-bit and
  the input container is kept (`mp4` → `mp4`, `webm` → `webm`). WebM audio is
  written as Opus, everything else as AAC.
- Each fade is capped at `30` seconds per side, and the clip length at 10 hours.
- Input and output are each capped at 25 MB, because ffmpeg runs inside the
  browser tab rather than on a server.
- The ramps are linear. The picture fade filter has no easing/curve option at
  all, so a curved audio ramp would no longer match the picture.
- This fades one clip against a flat colour. It does not cross-dissolve between
  two clips, and it does not fade an alpha channel — H.264 in MP4 has no
  transparency to fade.
- If the length you type is wrong, the fade-out lands in the wrong place: too
  large and it starts late (or gets cut off by the real end), too small and it
  finishes before the clip does.

### FAQ

<details>
<summary>Why do I have to type the clip length for a fade-out?</summary>

The ffmpeg `fade` and `afade` filters take an **absolute start time**, so a
closing fade is placed at `length − fade out`. The command is assembled before
the video is decoded, so the length has to come from you. A fade-in starts at `0`
and never needs it — leave the field at `0` for a fade-in-only run.

</details>

<details>
<summary>Can I fade to white, or to a custom colour?</summary>

Yes. Put `white`, a hex value like `#101820` or `0x101820`, or a `name@alpha`
value like `black@0.8` in **Fade colour**. The same colour is used for both the
opening and the closing ramp. Colours containing filtergraph punctuation
(`,` `:` `[` `]` `'` or spaces) are rejected rather than escaped.

</details>

<details>
<summary>Will the video be re-encoded, and will quality drop?</summary>

If the fade touches the picture, yes — changing pixels requires a re-encode, and
the output is H.264/AAC in an MP4 at the CRF you choose. If you set **Fade** to
`Sound only`, the picture is stream-copied with no quality change at all and only
the audio is re-encoded.

</details>

<details>
<summary>Can I fade only one end of the clip?</summary>

Yes. Set the side you don't want to `0`. Fade-in only leaves the ending
untouched (and needs no clip length); fade-out only leaves the opening at full
brightness and volume. Setting both to `0` is rejected, because the output would
be identical to the input.

</details>

<details>
<summary>How is this different from fading only the audio?</summary>

A picture fade changes what you see — the frames dip to and from a solid colour —
and therefore re-encodes the video stream. An audio-only fade changes only the
volume curve and copies the picture untouched. Choose `Sound only` here for the
audio-only behaviour, or `Picture only` if you want the visuals to dip while the
soundtrack stays at full level.

</details>

<details>
<summary>What is the longest fade I can apply?</summary>

30 seconds per side. The two fades together also cannot be longer than the clip
itself — a 3-second fade in plus a 3-second fade out on a 5-second clip is
rejected, while 2.5 + 2.5 on the same clip is allowed and leaves no full-level
frames in between.

</details>
