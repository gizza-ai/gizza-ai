## Put a title or lower third on your video

Type a caption, drop in a video, and this tool burns the text straight onto the
picture — a big centered **title card**, a news-style **lower third** in the
corner, a top banner, whatever you need. You control where the text sits (seven
anchor presets), how big it is, its color, an optional semi-transparent
**background bar** behind it, and exactly **when** it appears and disappears
(start/end in seconds). Everything runs in your browser with ffmpeg; the video
never leaves your device.

The caption is drawn **literally** — apostrophes, colons, ampersands, `%` and
quotes all render as-is, no escaping and nothing to break. A clean bold
sans-serif font is bundled in, so the result looks the same on every device.

### Worked example

Upload a **1280×720** clip, type `Jane Doe — CEO`, pick **Bottom left (lower
third)**, leave the font at **48 px** and the black bar at **0.5** opacity, and
set the window to **0–5 s**. The output is a same-size **1280×720** MP4 that
looks identical to the source for the first 5 seconds except for a white
`Jane Doe — CEO` caption on a half-transparent black bar near the bottom-left
corner — then the caption disappears and the rest of the clip plays untouched.
Prefer a big opening title instead? Set position **Center**, font size **80**
and untick the background bar for plain white text over the middle of the frame.

### Notes and limits

- **Position** offers seven anchors: top / center / bottom × left / center /
  right. Bottom-left and bottom-center are the classic lower-third spots.
- **Timing**: `start` and `end` are seconds from the beginning of the video.
  The caption is only drawn while `start ≤ t < end`, so `0`–`3` shows it for the
  first three seconds. `end` must be greater than `start`. If `end` is longer
  than the video, the caption simply stays until the clip ends.
- **Font size** is in pixels (8–400). The text does not auto-shrink, so a very
  long caption at a large size can run off the frame — shorten it or drop the
  size. Keep captions to one short line.
- **Text color** and **background bar color** take any CSS color name
  (`white`, `black`, `navy`, …) or hex like `#FFCC00` / `#fc0` — the same names
  ffmpeg itself understands. The swatch next to each field picks a hex visually;
  typing a name works too.
- The **background bar** is a box drawn behind the text with the padding built
  in; its opacity runs from `0` (invisible) to `1` (solid). Untick it for plain
  text with no bar.
- Videos up to **25 MB** are supported. Video re-encodes as H.264 (`medium`
  preset, `yuv420p`). **Audio is stream-copied** when the input keeps its
  container (mp4/mov/m4v/mkv); other inputs (webm, …) are converted to MP4 and
  the audio is re-encoded to AAC. MP4/MOV outputs are written with `+faststart`
  so players can start streaming before the download finishes.

### FAQ

<details>
<summary>What's the difference between a title card and a lower third?</summary>

They're the same tool, just different settings. A **lower third** is a small
caption anchored near the bottom of the frame (bottom-left or bottom-center)
with a background bar — the name/title strip you see under people in
interviews and news. A **title card** is usually larger text in the center or
top, often without a bar. Pick the position, size and background to taste.

</details>

<details>
<summary>Can I make the caption appear only during part of the video?</summary>

Yes — that's what **Show from** and **Show until** are for. Set them in
seconds and the text is drawn only inside that window; outside it the frame is
untouched. For example `0` to `3` shows the caption for the first three
seconds, then it disappears. `Show until` must be greater than `Show from`.

</details>

<details>
<summary>Do I need to escape quotes, colons or percent signs in my text?</summary>

No. The caption is passed to ffmpeg as a text file and drawn literally, so
`Don't Stop: 100% Live` renders exactly as typed. There are no special
characters to escape and nothing you can type will break the render.

</details>

<details>
<summary>Can I change the font, or add multiple captions?</summary>

Not yet — one clean bold sans-serif font is bundled in, and each run adds a
single caption. To stack captions, run the tool again on the output. Custom
fonts and animated in/out transitions aren't supported here (they'd need a
font upload or a full editor); this tool focuses on a fast, clean static
overlay.

</details>

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab via WebAssembly, so the file never
leaves your device. You can confirm it: open your browser's developer tools,
watch the network panel while the caption is added, and you'll see no upload.

</details>
