# video-first-last-frame-extractor — competitor analysis (2026-07-10)

Tool function: grab a video's **first** frame and **last** frame in one decode
pass and stitch them into a single before/after image — side by side
(`horizontal`) or stacked (`vertical`), output PNG or JPG. Distinct from the
sibling `extract-frames` (samples *many* frames by interval/fps/scene into a
contact-sheet grid) and from a single-timestamp frame grab: this always takes
exactly frame 0 and the final frame, with no timestamp input.

Competitor scan done before finishing. Paraphrased only — no competitor copy,
branding, or trademarks reproduced.

## Competitors skimmed (top real, reachable tools)

Search: "extract first and last frame of video online free" and "save video
first frame last frame image" (July 2026).

1. **A browser-ffmpeg frame grabber.** Lets you scrub to any timestamp and export
   that single frame as PNG/JPG. First/last are reachable only by manually seeking
   to 0:00 and the end; there is no one-click "first + last" or any joining — you
   export two files and combine them yourself. Runs client-side, no upload.
2. **A "get video thumbnail / cover" tool.** Pulls one representative frame
   (usually the opener) as a poster image; format PNG/JPG. No last-frame option and
   no side-by-side output; it is a single-poster extractor.
3. **A general online video-to-image / frame-export utility.** Exports every Nth
   frame or a chosen range as a ZIP of stills. Can technically yield the first and
   last as part of a batch, but not as one comparison image, and it uploads the
   file to a server with a size tier.

## Table-stakes → decision (each lands in the descriptor or the out-of-model list)

| Table-stake | In gizza model? | Where |
| --- | --- | --- |
| First frame (frame 0) | yes | always grabbed — `select=eq(n,0)` branch |
| Last frame (final frame) | yes | always grabbed — `reverse` branch, no timestamp needed |
| PNG / JPG output choice | yes | `format` enum (png default, jpg smaller) |
| Combine both frames into one image | yes | `layout` enum — `hstack` (side by side) / `vstack` (stacked) |
| Keep source dimensions | yes | frames are unscaled; both halves align |
| Runs locally, no upload | yes | in-browser wasm ffmpeg (page) + local CLI |
| Arbitrary-timestamp single-frame grab | **out-of-model** | that is a *different* tool (single frame at a chosen time); this tool's whole point is the automatic first+last pair with no seeking |
| Export each frame as its own file / ZIP | **out-of-model** | the page renders one output file, so the pair is delivered as one stitched image; splitting is a downstream crop |
| Thumbnail/scale controls, padding, background color | **out-of-model** | keep the two frames at native size for a faithful before/after; sizing/gap knobs belong to `extract-frames` (the contact-sheet tool) |
| Large uploads (100 MB+ tiers) | **out-of-model** | 25 MiB cap — `reverse` buffers the decoded video in RAM to reach the last frame; everything runs locally, larger files are a server/paid feature |
| GIF / animated output of start→end | **out-of-model** | this produces a static comparison image; an animated start-vs-end loop is a separate video tool |

## UX control patterns to match (competitors ship these)

- Output format as a labelled `<select>` (PNG / JPG) → `Param::enumv` +
  `[input.labels]`.
- Layout as a labelled `<select>` (side by side / stacked) → `Param::enumv` +
  `[input.labels]`; this is the differentiator competitors *don't* offer.
- Preset "try" chips (side-by-side PNG, stacked PNG, side-by-side JPG) →
  `[[example]]`.

## Worked example (used on the page)

A 64×64 clip whose first frame is red and last frame is blue, run **side by
side** + **PNG**, returns a **128×64** image: solid red left half, solid blue
right half — the same fixture the Playwright spec asserts pixel-exactly.
