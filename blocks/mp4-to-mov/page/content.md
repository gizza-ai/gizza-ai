## Convert MP4 to MOV in your browser

An MP4 and a QuickTime `.mov` are both just *containers* — boxes that hold
already-compressed video, audio and data streams. In fact they're close
relatives: MP4 (ISO-BMFF) grew directly out of Apple's QuickTime file format, so
moving from one to the other doesn't require touching the media inside. This tool
does a **lossless remux**: it runs `ffmpeg -i in.mp4 -map 0 -c copy -movflags
+faststart out.mov`, which stream-copies **every** track from the MP4 straight
into a `.mov` wrapper. Nothing is re-encoded, so the picture and sound are
bit-for-bit identical, and a short clip converts in a fraction of a second.

`-map 0` selects every stream in the input — the video, *all* audio tracks and
data — rather than just the default video plus first audio. `-c copy` copies
those packets across without invoking an encoder. `-movflags +faststart` moves
the `moov` index atom to the front of the file so the `.mov` can start playing
before it has fully downloaded. Because `.mov` is a superset container that
accepts every codec MP4 can carry (H.264, HEVC, MPEG-4 video; AAC, AC-3, PCM
audio), the remux always succeeds — there is no codec that "won't fit," so this
tool has no re-encode fallback and no settings to tune.

Everything runs locally with ffmpeg compiled to WebAssembly — your file is never
uploaded to a server, and the page keeps working offline once it has loaded.

### Why move to MOV?

`.mov` is the container Apple's creative apps expect. Final Cut Pro, iMovie,
QuickTime Player and other macOS tools read and write `.mov` natively, and some
import or capture workflows only offer it. Remuxing to `.mov` first —
losslessly, keeping your original streams untouched — hands those apps the
wrapper they want without paying the time and quality cost of a re-encode. If you
instead need to *re-encode* (change codec, resolution or quality — for example to
ProRes), that's a different job: use video-transcode or video-compress.

### Worked example

Take a 6-second clip `holiday.mp4` holding an H.264 video stream and an AAC
audio stream. Drop it in. Out comes `holiday.mov` with the exact same H.264 and
AAC streams copied across — identical dimensions, identical duration, no visible
or audible change — now in a QuickTime container that Final Cut Pro or iMovie
will import directly. The conversion finishes almost instantly because nothing is
re-compressed.

### Limits

- Input and output are each capped at **10 MiB** — trim or compress longer clips
  first.
- This is a container remux only. It never changes codec, resolution or
  quality; to do that, use video-transcode or video-compress.
- Output is always a `.mov` (`video/quicktime`). The remux keeps whatever
  codecs the MP4 already used — it does not convert them to ProRes or anything
  else.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: site/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown renders. -->

<details>
<summary>Does converting MP4 to MOV lose quality?</summary>

No. This is a lossless container remux, not a re-encode. The exact video and
audio streams are copied into the QuickTime wrapper untouched with `-c copy`, so
the result is bit-for-bit the same media, just in a different box. Only tools
that genuinely re-compress the picture (video-transcode, video-compress) change
quality.

</details>

<details>
<summary>Will the .mov open in Final Cut Pro and iMovie?</summary>

That's the point of the conversion. `.mov` is QuickTime's native container, so
Final Cut Pro, iMovie and QuickTime Player import it directly. Note that the
remux keeps the MP4's original codecs (usually H.264/AAC), which those apps
support — it does not transcode to ProRes. If a workflow specifically requires
ProRes, re-encode with video-transcode instead.

</details>

<details>
<summary>Why is the conversion so fast?</summary>

Because nothing is re-encoded. Re-encoding video is the slow, expensive part; a
remux only rewrites the container metadata and copies the already-compressed
packets across with `-map 0 -c copy`, so a short clip converts in well under a
second.

</details>

<details>
<summary>Does it keep all my audio tracks?</summary>

Yes. `-map 0` selects every stream in the MP4 — the video, all audio tracks and
data — and copies them all into the `.mov`. Nothing is dropped. Because `.mov`
is a superset container that holds every codec MP4 can, whatever the MP4 held
fits, which is exactly why the remux always succeeds.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The conversion runs entirely in your browser using ffmpeg compiled to
WebAssembly — the file never leaves your device, and the page works offline once
it has loaded.

</details>
