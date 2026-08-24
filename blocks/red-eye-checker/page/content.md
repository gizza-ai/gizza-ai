## About this tool

Red-eye is what happens when a flash fires straight down the lens axis: the light
bounces off the blood-rich retina and comes back as a glowing red pupil. This tool
**finds** those pupils and tells you exactly where they are — it is a checker, not
a retoucher, so your photo is never modified.

Every red-dominant, saturated, bright pixel is grouped into a connected region;
regions that are the wrong size or the wrong shape for a pupil (a red jumper, a
brake light, a stripe of lipstick) are dropped. What survives is scored 0–1 on how
red, how round and how square it is, and listed highest confidence first.

The whole check runs in WebAssembly inside your browser. The photo never leaves
your device, there is no upload and no account.

### A worked example

Upload a 640×480 flash portrait with two red pupils at roughly (210, 180) and
(330, 182), leave every control at its default, and the report looks like this:

```json
{
  "width": 640,
  "height": 480,
  "candidate_count": 2,
  "sensitivity": "medium",
  "regions": [
    {
      "center_x": 210,
      "center_y": 180,
      "radius_px": 6.51,
      "area_px": 133,
      "average_red": 223.4,
      "confidence": 0.812
    },
    {
      "center_x": 330,
      "center_y": 182,
      "radius_px": 6.18,
      "area_px": 120,
      "average_red": 214.9,
      "confidence": 0.774
    }
  ],
  "warnings": []
}
```

`center_x`/`center_y` are pixels from the top-left corner, `radius_px` is the
radius of the disc with the same area as the red blob, and `average_red` is the
mean red channel (0–255) inside it. Paste those coordinates into any editor's
red-eye brush and you are done.

### Reading the controls

- **Sensitivity** — `low` only accepts bright, strongly saturated red; `medium`
  (the default) catches typical phone and compact-camera red-eye; `high` also
  flags dim, partly-corrected or orange "amber eye", at the cost of more false
  positives.
- **Smallest / largest radius** — the pupil size window, in pixels. On a big photo
  raise the minimum to skip JPEG noise; lower the maximum to reject large red
  objects.
- **Maximum regions listed** — caps the `regions` array. `candidate_count` always
  reports how many were found *before* the cap, and a warning tells you when the
  list was clipped.

### Limits worth knowing

- PNG and JPEG only, up to roughly 48 MB of decoded pixels — re-export a huge
  photo at a lower resolution if you hit that.
- `min_radius` accepts 1–80 px, `max_radius` 1–300 px, `max_regions` 1–100, and
  `min_radius` may not exceed `max_radius`.
- Fully transparent pixels are ignored, so a cut-out PNG will not trip the check.
- This is a colour-and-shape heuristic, not face detection: it does not know where
  the eyes are, only where pupil-shaped red is.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>Does this tool remove the red-eye as well?</summary>

No — it only reports it. You get the centre, radius, area, average redness and a
confidence score for each region, and your original file is left untouched. Feed
those coordinates to your editor's red-eye tool, or use them to decide whether a
photo is worth retouching at all.

</details>

<details>
<summary>It found something that isn't an eye. How do I get rid of it?</summary>

Red-eye detection is a colour-and-shape heuristic, so a round red button, a
tail-light or a bright lip highlight can score well. Three fixes, in order of
usefulness: drop **Sensitivity** to `low`, narrow the radius window with
**Smallest/largest radius** so only pupil-sized blobs qualify, or ignore the
low-`confidence` entries — the list is already sorted with the most eye-like
region first.

</details>

<details>
<summary>Why did it miss a pupil I can clearly see?</summary>

Usually the red is duller than the default thresholds expect — a partly corrected
photo, a dark-eyed subject, or the orange "amber eye" you get from an off-axis
flash. Switch **Sensitivity** to `high`. If the pupil is tiny (a face far from the
camera), also lower **Smallest radius** to 1 or 2. Check the `warnings` field: it
says explicitly when regions were skipped for being too small, too large, or not
pupil-shaped.

</details>

<details>
<summary>Is my photo uploaded anywhere?</summary>

No. The detector is compiled to WebAssembly and runs inside this page, so the
image bytes are read locally and never sent to a server. You can confirm it by
loading the page, going offline, and running the check.

</details>
