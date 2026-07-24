## About this tool

This thumbnail generator turns a single frame from a video into a ready-to-share PNG.
Choose the timestamp, crop the frame to a target canvas (1280×720 by default), add a
large outlined headline, and optionally draw a bright accent bar on one edge.

The tool is designed for demo videos, explainers, launch clips, course lessons, and
README previews where you need a clear still image without opening a full design app.
It runs the video processing locally through ffmpeg in your browser and downloads a PNG.

### Example

Upload a short video, set **Frame time** to `1`, enter this headline:

```
BIG IDEA IN 10 MINUTES
```

Keep the default `1280 × 720` canvas, headline position **Bottom**, white text, black
outline, and a yellow bottom accent bar. The output is a PNG thumbnail whose source
frame is center-cropped to 16:9, with the headline drawn from a bundled bold font.

### Limits and edge cases

- Output is a single **PNG image**. It does not upload to any platform or create a video.
- The frame is scaled and center-cropped to the requested canvas; use a 16:9 size such as
  `1280 × 720` for typical video thumbnails.
- The headline is capped at 120 characters. Short punchy text works best.
- Colors accept common CSS color names or hex values like `#ffcc00` and `#fc0`.
- Text wrapping is not automatic. Add a shorter headline if it overflows the canvas.

## FAQ

<details>
<summary>Does this make an actual YouTube upload thumbnail?</summary>

It creates a **PNG image** with the usual thumbnail shape and styling controls. You can
then upload that PNG wherever you need it. The tool does not connect to any account,
platform, API, or publishing workflow.

</details>

<details>
<summary>How do I choose the best frame?</summary>

Use **Frame time (seconds)** to seek into the video and grab a still frame. Try a few
seconds where the subject is clear, motion blur is low, and there is enough empty space
for the headline. The output updates when you run the tool again.

</details>

<details>
<summary>Why is my headline cut off?</summary>

The headline is drawn as one line. Reduce **Font size**, shorten the text, move it to a
different position, or make the canvas wider. This keeps the tool deterministic and
avoids surprising automatic line breaks.

</details>

<details>
<summary>Can I remove the accent bar?</summary>

Yes. Set **Accent bar position** to `none` or set the bar thickness to `0`. The headline
and outline still render normally.

</details>
