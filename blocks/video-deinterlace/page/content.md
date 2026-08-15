## About this tool

Old camcorder tapes, DV captures, DVD sources and 1080i broadcasts often store each video frame as two time-separated fields. When those fields are displayed as one progressive frame, moving edges show horizontal "comb" teeth. This tool runs ffmpeg's deinterlacers in your browser and writes a progressive video you can download.

The default preset uses `bwdif`, keeps the source frame rate, trusts the file's field-order flags, and deinterlaces every frame. That is the safest starting point for unlabelled captures. If motion looks less fluid than the original broadcast, switch frame rate to "Double" (`field` mode). If motion jitters backwards and forwards, force top-field-first (`tff`) or bottom-field-first (`bff`).

### Worked example

A PAL DV transfer named `family-tape.mp4` has combing on fast motion and is bottom-field-first. Choose:

- Deinterlacer: `bwdif`
- Frame rate: `Keep frame rate`
- Field order: `Bottom field first`
- Apply to: `All frames`

The output is `family-tape-deinterlaced.mp4`: H.264 video flagged progressive, with the audio stream kept when the container supports it.

### Limits and edge cases

- This is deinterlacing, not inverse telecine. Film-sourced 29.97i footage that should become 23.976p needs a separate field-match/decimate workflow.
- Deinterlacing rewrites the picture, so video is re-encoded to H.264 at CRF 20. Audio is copied unless the output container must switch to MP4.
- Browser runs are best for short clips. Very large videos can exceed the browser ffmpeg runtime's memory limits.
- `field` mode doubles the frame count and can roughly double encoding time and file size.

## FAQ

<details>
<summary>Should I use bwdif or yadif?</summary>

Use `bwdif` first. It is motion-adaptive like yadif but uses a sharper interpolator, so it usually keeps more vertical detail. Use `yadif` when you need the classic, widely documented ffmpeg filter for comparison or compatibility.

</details>

<details>
<summary>What does double frame rate do?</summary>

Interlaced video has two fields captured at different times. `frame` mode combines them into one progressive frame per input frame, keeping the nominal frame rate. `field` mode outputs one progressive frame per field, so 50i becomes 50p and motion looks smoother, but encoding takes longer and the file has more frames.

</details>

<details>
<summary>How do I choose top-field-first or bottom-field-first?</summary>

Start with `auto`. If the output has a forward/backward judder on motion, the file is probably mis-flagged. Try `tff` for HDV and 1080i broadcast footage, or `bff` for many SD DVD, DV and analogue-capture sources.

</details>

<details>
<summary>Why can the output still look soft?</summary>

A deinterlacer has to synthesize missing scan lines, so some softness is normal. `bwdif` usually preserves the most detail among the fast ffmpeg filters exposed here. Severe tape noise, blended fields, or telecined film may need cleanup or inverse-telecine tools before or instead of deinterlacing.

</details>
