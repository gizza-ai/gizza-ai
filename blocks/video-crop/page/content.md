## Crop a video

Pick a video, set the crop **width** and **height**, and (optionally) an **x/y**
offset for the top-left corner — leave the offset blank to crop from the center.
The video is re-encoded right in your browser with ffmpeg; nothing is uploaded.

### How it works

- **Width / height** — the size of the rectangle to keep, in pixels.
- **X / Y offset** — where the rectangle starts from the top-left. Leave both
  blank to center the crop.
- The output keeps the original container format (mp4, webm, …) and is
  re-encoded with H.264 video + AAC audio.

### Notes

- Cropping re-encodes the video, so very large files take longer (everything
  runs locally on your machine).
- Want to shrink the file instead of changing its frame? Use the video
  compressor. Want to cut its length? Use the video trimmer.

### FAQ

<details>
<summary>What happens if I fill in only the X offset (or only Y)?</summary>

The moment *either* offset is set, the crop stops being centered: the missing
offset defaults to **0** (the top or left edge). So `x=100` with an empty Y crops
100 px from the left but starts at the very top. Leave **both** offsets blank if
you want the crop centered on the frame.

</details>

<details>
<summary>Does cropping change the video's format or quality?</summary>

The output keeps your original container (an `.mp4` stays `.mp4`, a `.webm` stays
`.webm`), but the streams are re-encoded to **H.264 video + AAC audio** — cropping
can't be done without re-encoding. Re-encoding introduces a small generational
quality loss, so crop once from the original rather than repeatedly re-cropping
outputs.

</details>

<details>
<summary>Is my video uploaded?</summary>

No — the ffmpeg engine runs in your browser tab; the
file never leaves your device.

</details>

<details>
<summary>What if my crop is bigger than the video?</summary>

ffmpeg will reject an out-of-bounds
crop; pick a width/height/offset that fits within the source dimensions.

</details>
