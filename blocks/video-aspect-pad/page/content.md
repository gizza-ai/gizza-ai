## Letterbox or pillarbox a video to any aspect ratio

Social platforms all want a different frame: **9:16** for Reels, Shorts and
TikTok, **1:1** for square feed posts, **4:5** for Instagram portrait, **2:3**
for Pinterest, **16:9** for YouTube. This tool fits your whole video onto the
target canvas — nothing is cropped and nothing is stretched. The frame is
scaled to fit, centered, and the leftover space is filled either with solid
bars in the color you choose (letterbox = bars above/below, pillarbox = bars
at the sides) or with a **blurred, zoomed copy of the video itself** — the
backdrop look TV news uses for portrait phone clips. Everything runs in your
browser with ffmpeg; the file never leaves your device.

### Worked example

A landscape **1920×1080** clip padded to **9:16** comes out as exactly
**1080×1920**: the clip is scaled to fit the 1080-pixel canvas width
(≈1080×606) and centered, leaving ≈657-pixel bars above and below in your
chosen color — ready to post as a Reel or Short. The same clip padded to
**1:1** becomes 1080×1080 with bars above and below; a portrait phone video
padded to **16:9** gets bars on the left and right instead. Tick **blurred
background** and those bars become a soft, magnified blur of the footage
instead of a flat color.

### Notes and limits

- The output is always **exactly** the target canvas: the platform-standard
  size for the chosen ratio (9:16 → 1080×1920, 1:1 → 1080×1080, 16:9 →
  1920×1080, 4:5 → 1080×1350, 3:4 → 1080×1440, 4:3 → 1440×1080, 3:2 →
  1620×1080, 2:3 → 1080×1620, 21:9 → 2520×1080), or your **width** × the
  ratio-derived height.
- A custom width must be an **even** number from 16 to 4096 (H.264 requires
  even dimensions); odd values are rejected, not silently changed.
- Bar color takes any CSS color name (`black`, `white`, `navy`, …) or hex like
  `#1A2B3C` / `#f0f` — the same 140 names ffmpeg itself understands. The
  swatch next to the field picks a hex visually; typing a name works too.
- **Quality** maps straight to x264 CRF — high = CRF 18 (bigger file),
  medium = CRF 23 (the default), low = CRF 28 (smaller file). No hidden
  "premium" tiers; the numbers are the whole story.
- Videos up to **25 MB** are supported. Video re-encodes as H.264 with the
  `medium` preset. **Audio is stream-copied** when the input keeps its
  container (mp4/mov/m4v/mkv); other inputs (webm, …) are converted to MP4 and
  the audio is re-encoded to AAC. MP4/MOV outputs are written with
  `+faststart`, so players can start streaming before the file finishes
  downloading.
- A small clip is scaled **up** to the standard canvas — set a smaller width
  if you want to keep the original pixel size.

### FAQ

<details>
<summary>Does padding crop or distort my video?</summary>

No — that's the point of padding. The whole frame is scaled to *fit inside*
the target canvas (never beyond it), so every pixel of your original video
stays visible. Bars fill the leftover space. If you'd rather fill the frame
and lose the edges, use the crop tool instead.

</details>

<details>
<summary>Should I pad, crop, or stretch to change aspect ratio?</summary>

**Pad** (this tool) when nothing may be lost — interviews, screen recordings,
anything with content near the edges. **Crop** when the subject sits safely
in the center and you want the frame completely filled. **Stretch** distorts
faces and almost never looks right. The blurred-background option is the
middle path: the frame is filled edge-to-edge, but your whole video stays
visible on top.

</details>

<details>
<summary>How does the blurred background work?</summary>

The video is used twice: one copy is zoomed until it covers the whole target
canvas, center-cropped and heavily blurred to become the backdrop; your
untouched full frame is then centered on top. The blur strength scales with
the output size, and the bar color setting is ignored while the box is
ticked.

</details>

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs in your browser tab via WebAssembly; the file never leaves
your device. You can verify it yourself: open the browser's developer tools,
watch the network panel while converting, and you'll see no upload request.

</details>

<details>
<summary>Which aspect ratio should I pick for each platform?</summary>

Use **9:16** for Instagram Reels, YouTube Shorts and TikTok, **1:1** or
**4:5** for feed posts, **2:3** for Pinterest, **16:9** for YouTube and most
landscape players, **3:2** to match classic photo frames, and **21:9** for a
cinematic letterbox look. The dropdown lists the exact pixel canvas each
preset renders.

</details>

<details>
<summary>Why is the output 1080×1920 when my video was much smaller?</summary>

By default the tool renders the platform-standard canvas for the chosen
ratio, which is what the apps expect (e.g. 1080×1920 for 9:16). If you want
to keep your clip's own scale, set **width** to your video's width — the
height still follows the target ratio and only bars are added.

</details>
