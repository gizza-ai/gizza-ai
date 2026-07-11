## First frame vs last frame, in one image

Load a video and this tool grabs its **very first frame** and its **very last
frame** and joins them into a single **before/after image** — placed side by side
or stacked top over bottom. Everything runs locally in your browser with ffmpeg
(WebAssembly); the video is never uploaded.

Both frames come from **one decode pass** and no timestamp is needed: the graph
splits the stream, keeps frame 0 on one branch, and reverses the other branch so
*its* frame 0 is the original last frame — then glues the two together. Because
the page renders one output file, you get the pair as this single stitched image
rather than two separate downloads.

Choose the **layout**:

- **Side by side (horizontal)** — first frame on the left, last frame on the
  right. Good for wide comparisons and thumbnails.
- **Stacked (vertical)** — first frame on top, last frame on the bottom. Good for
  portrait/vertical clips or a tall before/after strip.

Then pick **PNG** (lossless, crisp for UI recordings and text) or **JPG**
(smaller, better for photographic footage). The source dimensions are preserved,
so a 1280×720 clip stitches to **2560×720** side by side, or **1280×1440**
stacked.

### Worked example

Upload a 64×64 test clip whose first frame is red and last frame is blue, pick
**Side by side** and **PNG**. The tool returns a **128×64** image — solid red on
the left, solid blue on the right — showing the clip's start-vs-end change at a
glance. Right-click to save it, or use the Download link.

## Handy for

- Spotting how a clip changes from start to finish (a before/after at a glance).
- Making a two-panel thumbnail or cover image from a single video.
- Picking **start** and **end** keyframes to feed AI video / image-to-video tools.

## Limits and good to know

- Reaching the **last** frame requires reversing the decoded video, which buffers
  it in memory, so the input is capped (**25 MiB**) — trim or downscale huge files
  first. This is a "grab two frames" tool, not a transcoder.
- The **two frames share the source dimensions**, so side by side doubles the
  width and stacked doubles the height; both halves always align.
- Works with anything ffmpeg can decode in the browser — MP4/H.264, WebM, MOV and
  more. The output is always a single **image**, not a video.

## FAQ

<details>
<summary>Can I download the first and last frame as two separate images?</summary>

Not from this page — it produces **one** stitched image with both frames joined
(side by side or stacked), which the browser can render and download as a single
file. To crop the halves apart afterward, or to grab one exact frame at a chosen
timestamp on its own, use an image cropper or the **Extract a Video Frame** tool
instead.

</details>

<details>
<summary>What's the difference between the horizontal and vertical layout?</summary>

Only how the two frames are arranged. **Horizontal** places the first frame left
and the last frame right (the output is twice as wide as the source).
**Vertical** stacks the first frame on top and the last on the bottom (twice as
tall). The frames themselves are identical either way — pick whichever fits your
clip's shape.

</details>

<details>
<summary>Do I need to know the video's length or set a timestamp?</summary>

No. The tool always takes frame 0 (the first frame) and the final frame,
whatever the duration is. It finds the last frame by reversing the stream, so you
never enter a time — just upload and run.

</details>

<details>
<summary>Which format should I choose, PNG or JPG?</summary>

**PNG** is lossless and stays crisp on screen recordings, UI, and text — pick it
when you want an exact copy of the frames. **JPG** is smaller and fine for
photographic or camera footage where a little compression won't show. Both keep
the source dimensions.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The video is decoded and the frames are stitched entirely in your browser
with WebAssembly ffmpeg. Nothing leaves your device, and there's no account or
upload step.

</details>
