# hough-line-detection — competitor analysis (2026-09-25)

Scan run BEFORE implementation, per the create-next-tool recipe. One WebSearch
(`online Hough transform line detection tool image detect straight lines`), then the top
real tools/APIs that people actually use for this job were skimmed. **All notes below are
paraphrased** — no competitor copy, branding or trademarks are reproduced, and nothing from
them is shipped in our page/CLI text.

## Competitors skimmed

| # | What it is | Why it is the benchmark |
|---|---|---|
| 1 | Pixlane "Hough Transform" browser tool (pixlane.media) | The only genuinely comparable *online, no-signup, in-browser* line/circle detector the search surfaced. Runs OpenCV compiled to WebAssembly on-device. |
| 2 | OpenCV `HoughLines` / `HoughLinesP` (docs.opencv.org + the OpenCV-Python tutorial mirror) | The de-facto reference API. Every blog post, Stack Overflow answer and online tool copies its parameter names and defaults, so these ARE the table stakes. |
| 3 | scikit-image `transform.probabilistic_hough_line` + `hough_line_peaks` | The second reference implementation; contributes the peak-separation knobs OpenCV hides (`min_distance`, `min_angle`, `num_peaks`). |

(The GeeksforGeeks / LearnOpenCV / datahacker walkthroughs and the Edinburgh HIPR2 teaching page
returned by the same search are tutorials over competitor #2's API, not separate tools, so they were
read only as evidence of which defaults are conventional. `learnopencv.com` and `docs.opencv.org`
both returned HTTP 403 to the fetcher; the OpenCV parameter set was taken from the
`opencv24-python-tutorials` mirror instead, which is the same API surface.)

## Table-stakes parameters observed

| Capability | Where seen | Conventional default | Our decision |
|---|---|---|---|
| Canny edge pre-pass before the transform | #1, #2 (tutorials always `Canny` first) | low 50 / high 150 on the 0–255 gradient scale, Sobel aperture 3 | **IN** — `canny_low` / `canny_high`, expressed as 0–1 fractions of the maximum possible Sobel gradient (same convention as our shipped `edge-detection` block), with `0` = automatic (Otsu on the gradient histogram, low = 0.4 × high). Hand-rolled Canny (blur → Sobel → non-maximum suppression → hysteresis). |
| Pre-smoothing / noise suppression | #1 ("works on an edge map, not the raw photo"), #2 aperture size | Gaussian, small radius | **IN** — `blur` (Gaussian sigma, 0–5, default 1.0). |
| `rho` — distance resolution of the accumulator | #1, #2 | 1 pixel | **IN** — `rho_resolution`, 0.5–20 px, default 1. |
| `theta` — angle resolution of the accumulator | #1, #2 | 1 degree (`np.pi/180`) | **IN** — `angle_resolution`, 0.1–5 degrees, default 1. |
| `threshold` — minimum accumulator votes | #1 ("adjustable to filter weak detections"), #2 (100 for the probabilistic example, 200 for the standard one), #3 (`threshold`, defaults to 0.5 × the accumulator maximum) | image-dependent | **IN** — `threshold`, integer, `0` = automatic. Our automatic rule is derived from the segment length actually being asked for (0.6 × the minimum line length in analysis pixels, floor 16), which is scale-invariant in a way a fixed 100 is not. |
| `minLineLength` | #1 ("available in the probabilistic variant"), #2 (100 px), #3 (`line_length`) | 100 px on a ~600 px image | **IN** — `min_line_length` in ORIGINAL image pixels, `0` = automatic (8 % of the image diagonal, floor 20 px). |
| `maxLineGap` | #2 (10 px), #3 (`line_gap`) | 10 px | **IN** — `max_line_gap` in original pixels, `0` = automatic (1.5 % of the diagonal, floor 3 px). |
| Standard (infinite lines) vs probabilistic (segments) modes | #1 ("standard, probabilistic, and circle detection"), #2 (`HoughLines` vs `HoughLinesP`) | both offered | **IN** — `mode` = `segments` (default, endpoint output) or `lines` (accumulator peaks, reported as the infinite line clipped to the image rectangle). |
| Peak separation / de-duplication | #3 (`min_distance` 9 px, `min_angle` 10°) | 9 px / 10° | **IN (fixed, documented)** — non-maximum suppression in the accumulator over a 9-px rho window and a 5° theta window, plus consumed-edge removal in segments mode so two near-identical peaks cannot report the same ink twice. Not exposed as a param: the two reference implementations disagree on the units and users tune `threshold` instead. |
| Cap on how many lines come back | #3 (`num_peaks`, default unbounded) | unbounded | **IN** — `max_lines`, 1–500, default 50. Unbounded output is a bad fit for a chat/CLI tool that has to print the result. |
| Overlay visualization of the detections | #1 (the whole point of the browser tool), #2 tutorials (`cv2.line` onto the source) | red lines over the source image | **IN** — `output` = `report` (default, JSON) or `overlay` (a rendered image), plus `color`, `line_width` (`0` = automatic from image size) and `overlay_background` = `original` \| `edges` \| `black` \| `white`. |
| Reviewable numeric result / re-tuning | #1 ("reviewable results you can re-tune in real time"), #2 (returns (ρ, θ) or (x1,y1,x2,y2)) | — | **IN** — the report returns per-line `x1,y1,x2,y2,length,angle_degrees,rho,theta_degrees,votes,orientation` plus the effective (post-`auto`) value of every threshold, so the next run can be tuned from the output. |
| JPG / PNG / WebP input | #1 (explicitly listed) | — | **IN** — PNG, JPEG, WebP, GIF and BMP decode. |
| Output image format choice | #2 tutorials write PNG/JPG | — | **IN** — `format` = `png` \| `jpg` for the overlay. |

## Deliberately NOT built (out of model / out of scope)

* **Circle detection (Hough circles).** #1 bundles it in the same page. It is a different
  transform with a different parameter set and a different answer shape; it belongs in its own
  backlog row, not bolted onto a line detector. Listed, not built.
* **Generalized Hough transform (arbitrary shapes, ellipses, polygons).** #1 explicitly calls this
  out as beyond its own scope too. Needs a template/R-table input the tool has no way to accept.
* **Live interactive re-tuning with a slider preview.** #1's headline UX. Our surfaces here are
  chat + CLI (this is the no-page file-input pattern: the generator's pure-tool page cannot hand
  uploaded bytes to a wasm decoder, same as `image-blank-detector`, `background-color-detector`,
  `document-skew-detector`). The equivalent for us is that every automatic value is echoed back in
  the report so the next invocation can be tuned in one step — that is the in-model answer to
  "re-tune", and the reason those fields exist.
* **A visible accumulator/parameter-space image.** Teaching material (HIPR2) shows the Hough space
  itself. Interesting, but it is a diagnostic picture, not an answer; `overlay_background = "edges"`
  covers the "show me what the transform actually saw" need at a fraction of the payload.
* **Vanishing-point estimation from the detected lines.** A natural follow-on, but a separate tool
  and a separate answer.

## Fit against what we already ship (dup check)

Not a duplicate of an existing block:

* `blocks/edge-detection` — ffmpeg Canny/Sobel, returns an **edge map image**. It has no
  transform, no accumulator and returns no line geometry.
* `blocks/document-skew-detector` — projection-profile sweep returning **one** skew angle for a
  page of text; it deliberately keys on text lines, not on straight edges, and returns no segments.
* `blocks/image-horizon-tilt-checker` — returns **one** tilt angle for levelling a photo.
* `blocks/document-scan` — finds a page **quadrilateral** and warps it.

The delta this tool owns is the enumerated geometry: N straight line segments with endpoints,
lengths, angles, orientation class and vote counts — plus an overlay that draws them. Nothing we
ship produces that.

## Implementation note recorded during the scan

The backlog row suggests the `imageproc` crate. It was **rejected**: three shipped image blocks
(`document-scan`, `collage-splitter`, `multi-photo-scan-splitter`) already record that
`imageproc`/`fast_image_resize` bake `v128` SIMD that wafer's wasmi runtime rejects, and
`imageproc::hough` only returns infinite polar lines anyway (no segment extraction, which is the
table stake from #2/#3). The Canny pass, the accumulator, peak suppression, segment extraction and
the line rasterizer are therefore all hand-rolled on top of the already-proven pure-Rust `image`
crate.
