## About this tool

Some places simply refuse a photo. Video-only feeds, a slideshow timeline that needs a real clip
between two cuts, an ad slot that accepts MP4 and nothing else — all of them want a video file even
when the content is a single, perfectly good picture. This tool makes that file: it holds your
image motionless for as long as you ask and encodes the result as a genuine, playable video.

The hold is deliberate. There is no drift, no slow push, no pan — every frame is identical, so the
clip cuts cleanly against anything either side of it and compresses down to a tiny file. If you
want the picture to move instead, use the Ken Burns tool, which animates a pan-and-zoom across the
same still.

You control the frame completely. **Duration** sets the length, from a tenth of a second up to a
minute. **Width** and **height** set the output size — 1920×1080 for widescreen, 1080×1080 for a
square post, 1080×1920 for a vertical story. **Fit** decides what happens when your picture's
shape doesn't match that frame:

- **Contain** fits the whole image inside the frame and fills the leftover space with the
  **padding color** — nothing is cropped, so a portrait photo in a widescreen frame gets bars down
  the sides. The color is yours to pick: black, white, or any hex value.
- **Cover** fills the frame edge to edge and center-crops whatever hangs over. Nothing is padded,
  but the edges of the picture are lost.
- **Stretch** forces the exact size and distorts the picture to reach it.
- **Original** ignores width and height entirely and keeps the image at its own size, snapped to
  even dimensions and capped at 3840 px so a huge scan can't turn into an 8K encode.

**Frames per second** is worth thinking about here. Nothing moves, so a high frame rate buys you
nothing — 30 fps is a safe default for editors and players that expect it, but dropping to 10 fps
makes a noticeably smaller file that looks exactly the same. **Quality** runs 1–100 and maps onto
the encoder's CRF; 80 is visually clean for a static hold, and there is rarely a reason to go
higher for a picture that never changes.

**Format** picks the container. MP4 gives you H.264, which plays in every browser, editor, phone
and upload form there is. WebM gives you VP9, which is smaller for the same quality and is the
better choice for embedding directly on a web page. MOV is H.264 in a QuickTime container, which
is what some editing suites prefer to ingest.

Everything runs in your browser. The image is decoded, scaled and encoded on your own machine —
it is never uploaded to a server, which means a private photo stays private and the tool works the
same whether the file is 40 KB or 12 MB.

## FAQ

<details>
<summary>Why does my picture have bars down the sides?</summary>

That's the **contain** fit doing its job: your image's aspect ratio doesn't match the output frame,
so it's scaled to fit whole and the leftover space is filled with the padding color. If you'd
rather fill the frame completely, switch **Fit** to **cover** — the image is scaled up until it
covers the frame and the overflow is center-cropped. To keep the picture's own proportions and
avoid both, set **Fit** to **original**, or set width and height to match your image's aspect
ratio.

</details>

<details>
<summary>Does the clip have any sound?</summary>

No — the output is video only, with no audio track at all. Most players and platforms handle a
silent video fine, but a few uploaders reject a file with no audio stream. If you hit one of those,
run the finished clip through the "add silent audio" tool, which muxes in a silent track without
re-encoding the picture.

</details>

<details>
<summary>How long can the clip be?</summary>

Up to 60 seconds, and down to 0.1 seconds if you need a single-frame flash. Longer holds are
possible in principle but not offered here: the encode runs in your browser tab, and a multi-minute
clip at 1080p is slow enough that it feels broken. If you need a long hold, make a short one and
loop it in your editor — a static clip loops seamlessly by definition.

</details>

<details>
<summary>Which image formats can I use?</summary>

PNG, JPEG, WebP, BMP and GIF all work (an animated GIF contributes only its first frame — the clip
is static by design). Camera RAW, PSD and HEIC are not supported: those need decoders that aren't
part of the in-browser video engine. Convert them to PNG or JPEG first with one of the image
conversion tools, then bring the result here.

</details>

<details>
<summary>Why is the output size different from what I typed?</summary>

H.264 and VP9 both encode in a colour format that requires even pixel dimensions, so an odd width
or height is snapped down by one — ask for 641×481 and you get 640×480. The same rule applies to
**original** fit, which also caps the longest side at 3840 px. Values are clamped to the 16–3840
range in both directions.

</details>

<details>
<summary>Can I combine several photos into one slideshow?</summary>

Not with this tool — it takes a single image and produces a single clip. Make one clip per photo
and join them in an editor, or use a dedicated slideshow tool. For a moving version of a single
photo, the Ken Burns tool animates a pan-and-zoom over the same still instead of holding it.

</details>
