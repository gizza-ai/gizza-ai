## Letterbox or pillarbox a video to any aspect ratio

Social platforms all want a different frame: **9:16** for Reels, Shorts and
TikTok, **1:1** for square feed posts, **4:5** for Instagram portrait, **2:3**
for Pinterest, **16:9** for YouTube. This tool fits your whole video onto the
target canvas — nothing is cropped and nothing is stretched. The frame is
scaled to fit, centered, and the leftover space is filled with bars in the
color you choose (letterbox = bars above/below, pillarbox = bars at the
sides). Everything runs in your browser with ffmpeg; the file never leaves
your device.

### Worked example

A landscape **1920×1080** clip padded to **9:16** comes out as exactly
**1080×1920**: the clip is scaled to fit the 1080-pixel canvas width
(≈1080×606) and centered, leaving ≈657-pixel bars above and below in your
chosen color — ready to post as a Reel or Short. The same clip padded to
**1:1** becomes 1080×1080 with bars above and below; a portrait phone video
padded to **16:9** gets bars on the left and right instead.

### Notes and limits

- The output is always **exactly** the target canvas: the platform-standard
  size for the chosen ratio (9:16 → 1080×1920, 1:1 → 1080×1080, 16:9 →
  1920×1080, 4:5 → 1080×1350, 3:4 → 1080×1440, 4:3 → 1440×1080, 2:3 →
  1080×1620, 21:9 → 2520×1080), or your **width** × the ratio-derived height.
- A custom width must be an **even** number from 16 to 4096 (H.264 requires
  even dimensions); odd values are rejected, not silently changed.
- Bar color takes any CSS color name (`black`, `white`, `navy`, …) or hex like
  `#1A2B3C` / `#f0f` — the same 140 names ffmpeg itself understands.
- Videos up to **25 MB** are supported. Video re-encodes as H.264 (CRF 23,
  `medium` preset); **audio is copied untouched**.
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
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs in your browser tab via WebAssembly; the file never leaves
your device.

</details>

<details>
<summary>Which aspect ratio should I pick for each platform?</summary>

Use **9:16** for Instagram Reels, YouTube Shorts and TikTok, **1:1** or
**4:5** for feed posts, **2:3** for Pinterest, **16:9** for YouTube and most
landscape players, and **21:9** for a cinematic letterbox look.

</details>

<details>
<summary>Why is the output 1080×1920 when my video was much smaller?</summary>

By default the tool renders the platform-standard canvas for the chosen
ratio, which is what the apps expect (e.g. 1080×1920 for 9:16). If you want
to keep your clip's own scale, set **width** to your video's width — the
height still follows the target ratio and only bars are added.

</details>

<details>
<summary>Can I use a blurred version of the video as the background instead of a solid color?</summary>

Not yet — this tool fills the bars with a solid color (name or hex). A
blurred-background pad is a planned variant; for now pick a color that
matches your brand or footage.

</details>
