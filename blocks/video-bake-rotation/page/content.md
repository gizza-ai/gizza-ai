## Fix sideways videos by baking the rotate flag into pixels

Some phones save video sideways and add a hidden **rotation metadata** flag that
tells players how to display it. Most modern players obey the flag; some editing
apps, upload pipelines, previews, and older players ignore it, so the clip looks
sideways. This tool re-encodes the video so ffmpeg applies the embedded
orientation while decoding, then writes normal upright pixels and clears the
rotate flag.

**Worked example:** load a portrait phone clip that looks correct in your phone's
player but sideways after upload. Run the tool once. The result is an H.264 video
whose pixels are actually upright, so players no longer need to understand the
original rotate/display-matrix metadata.

### What changes

- The video stream is re-encoded to H.264 (`CRF 23`, preset `medium`) because the
  orientation has to become real pixels, not just metadata.
- The rotate tag is cleared (`rotate=0`) so players do not double-rotate the
  baked result.
- Audio is copied when the output container supports it; otherwise it is encoded
  to AAC in an MP4 output.

### Notes and limits

- If your video has no rotate metadata, the output will look the same, just
  normalized through the same H.264 encode path.
- This differs from an explicit rotate tool: you do not choose 90/180/270 degrees
  here. The embedded rotation metadata in the file is the source of truth.
- Input and output are each capped at 25 MB because ffmpeg runs in the browser.

### FAQ

<details>
<summary>How is this different from rotating a video manually?</summary>

Manual rotation asks you to choose an angle. This tool reads the angle already
stored in the video container and bakes that orientation into the pixels, then
clears the metadata flag.

</details>

<details>
<summary>Will this be lossless?</summary>

No. Baking metadata into pixels requires decoding and re-encoding the video. The
default H.264 CRF 23 setting is a practical quality/size tradeoff, and audio is
copied when the container allows it.

</details>

<details>
<summary>Why clear the rotate flag?</summary>

After the pixels are upright, keeping the old flag could make players rotate the
clip a second time. Clearing it makes the baked output display consistently.

</details>

<details>
<summary>What if my clip is already upright?</summary>

The output should still display upright. If there was no rotate metadata, this
acts as a normalization/transcode pass rather than a visible rotation fix.

</details>
