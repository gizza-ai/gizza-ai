## Hide a license plate, face or name tag in a video

Pick a video, draw a rectangle over the thing you want to hide — a **license
plate**, a **name tag**, a **house number**, a **logo** or a **face** that stays
in one place — and get the same clip back with that rectangle **blurred** or
**pixelated** on every frame. Everything runs with ffmpeg inside your browser
tab, so **nothing is uploaded**, and the tool is free.

The region is a fixed rectangle set by four numbers — **X** and **Y** are the
top-left corner in pixels (measured from the top-left of the frame), and
**Width** and **Height** are the size of the box. That rectangle is cropped out,
processed, and laid back over the original frame, so only the box changes and
the rest of the picture is untouched.

### Blur vs pixelate

- **Blur** softens the region with a Gaussian blur. It looks natural but, at low
  strength, a determined viewer can sometimes still make out shapes.
- **Pixelate** replaces the region with a coarse mosaic of flat blocks. It looks
  more obviously censored and is **harder to reverse**, so it is the safer choice
  for genuine redaction (plates, IDs, faces).

**Strength (1–100)** controls how strong the effect is:

- For **blur** it is the Gaussian sigma — higher means softer and more hidden.
- For **pixelate** it is the mosaic **block size** in pixels — higher means
  bigger blocks and a coarser, more thoroughly hidden region. If the block size
  is larger than the region, the whole box collapses to a single flat colour.

### Worked example

You filmed a walk-through of a car and its plate sits in a `200×60` box whose
top-left corner is at `x = 40`, `y = 30`. Load `car.mp4`, set **X** `40`, **Y**
`30`, **Width** `200`, **Height** `60`, choose **Pixelate**, and set **Strength**
to `16`. You get `car-blur-region.mp4` — the same clip with the plate reduced to
a mosaic on every frame and the rest of the picture unchanged.

On the command line the same job is `gizza video-blur-region --x 40 --y 30
--width 200 --height 60 --mode pixelate --strength 16 car.mp4`, and the page URL
`/tools/video-blur-region/?x=40&y=30&width=200&height=60&mode=pixelate&strength=16`
deep-links to the same pre-filled settings.

### Notes and limits

- The rectangle is **fixed** — it stays in the same place for the whole clip.
  This tool does **not** track a moving subject or auto-detect faces/plates
  (that needs an AI model); it blurs the box you specify.
- One rectangle per run. To hide several areas, run the tool more than once, or
  pick a box that covers them all.
- The video is re-encoded to H.264 (audio to AAC). mp4, mov, m4v and mkv keep
  their container; other inputs (e.g. webm) come out as MP4.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.
Nothing is uploaded and nothing is stored.

</details>

<details>
<summary>Should I use blur or pixelate to hide a license plate?</summary>

**Pixelate** is the safer choice for real redaction. A coarse mosaic throws away
the detail permanently and can't be un-blurred, whereas a light Gaussian blur can
sometimes be partly reversed. Use a block size (strength) large enough that
individual characters are no longer distinguishable — 16–24 works well for a
plate-sized box.

</details>

<details>
<summary>How do I find the x, y, width and height of the region?</summary>

The numbers are pixels measured from the **top-left** of the frame: **X** is how
far right the box starts, **Y** how far down, and **Width**/**Height** are its
size. If you know the video's resolution, estimate the box position from that —
e.g. on a 1280×720 clip a plate near the middle-bottom might be around
`x = 500, y = 520, width = 260, height = 70`. Run it, check the result, and nudge
the numbers if the box is off.

</details>

<details>
<summary>Can it follow a moving car, face or subject?</summary>

No. This tool blurs a **fixed** rectangle that stays put for the whole clip, so
it suits things that don't move much (a parked plate, an on-screen name tag, a
watermark/logo). Tracking a moving subject or auto-detecting faces/plates needs
an AI model and is out of scope here.

</details>

<details>
<summary>Will the rest of the video change?</summary>

Only the rectangle you specify is blurred or pixelated; the rest of each frame is
copied through untouched. The clip is re-encoded to H.264 so the file isn't
byte-for-byte identical to the source, but the visible change is confined to your
box. Audio is kept.

</details>

<details>
<summary>Which formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, m4v, mkv and webm are the common cases.
mp4/mov/m4v/mkv keep their container; other inputs (e.g. webm) come out as MP4.
The input and output are each capped at 25 MB.

</details>
