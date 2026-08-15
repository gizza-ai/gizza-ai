# video-fade — competitor scan + design decisions (2026-08-15)

Scan run alongside the build. Every note below is **paraphrased** from public
product/documentation pages; no competitor copy, branding, trademarks or assets
are reproduced in the tool, its page, or this file.

## Duplicate check

`ls blocks/ | grep -iE 'fade|video'` surfaced two neighbours; both were confirmed
distinct by reading their `core/src/lib.rs`:

- **`video-audio-fade`** — fades a video's **audio only**. It stream-copies the
  picture (`-c:v copy`) and builds its end fade with a reverse/afade/reverse
  chain, so it never needs the clip length. It cannot dip the picture.
- **`audio-fade`** — the same ramp on a standalone audio file; no video stream at
  all.

Not a duplicate: `video-fade` is the only block that ramps the **picture** to and
from a solid colour (and can drive the sound over the same spans, or the picture
alone). Its picture path necessarily re-encodes, which is a different
cost/quality contract from either neighbour.

## Competitors reviewed (5)

1. **WuTools — video fade in/out** (`wutools.com/video/video-fade-in-out`). The
   closest match in shape: browser-local ffmpeg.wasm, fade-in and fade-out each
   0.1–10 s (2 s default), a fade-target selector (both / video only / audio
   only), a CRF quality selector exposed as four named steps (≈18 / 20 / 23 / 28),
   an encode-speed preset (very fast → slow), and an output-container choice
   (same as input / MP4 / WebM / MOV). States a 100 MB cap and a wide
   ffmpeg-compatible input list. No fade-colour control is documented — the copy
   assumes black.
2. **Vidocu — free video fade** (`vidocu.ai/free-video-fade`). Fade-in and
   fade-out 0.5–5 s, a **black-or-white** colour choice, audio fades together
   with the picture, up to 500 MB, upload-or-URL input with a preview step.
   Server-side: free but account-gated, with the first watermark-free run
   complimentary.
3. **EchoWave — fade video** (`echowave.io/tools/fade-video/`). A timeline editor
   rather than a single-purpose tool: entrance/exit fades, opacity keyframes with
   ~30 easing curves, and eight dissolve transitions including dip-to-black,
   dip-to-white, a custom dip colour, luma fade and a soft-blur fade, plus
   per-clip audio fade sliders. Free tier exports 720p with a watermark; higher
   resolutions are paid.
4. **Flixier — fade to black transition** (`flixier.com/tools/…/fade-to-black-transition`).
   Timeline transitions whose length is set by dragging handles; audio can be
   faded alongside. Accepts most common formats and normalises output to MP4 for
   browser playback. No account needed to try; limits not published on the page.
5. **ConvertFleet — video transitions** (`convertfleet.com/video-transitions`).
   Browser-local ffmpeg.wasm again: fade-in, fade-out and cross-dissolve, preset
   durations (0.5 / 1 / 2 s), black-or-white fade colour, an automatically
   matched audio fade, and a 2 GB cap. Free with a paid tier alongside.

Technical baseline: ffmpeg's own `fade` and `afade` filters, checked against the
local binary's `-h filter=…` output. Confirmed there that **`fade` has no
easing/curve option** (only `type`, `start_frame`, `nb_frames`, `alpha`,
`start_time`, `duration`, `color`), while `afade` exposes ~24 curve shapes — the
asymmetry that decides the "curve" row below.

## Table stakes → where each one landed

| Table stake | Source | Decision |
| --- | --- | --- |
| Independent fade-in / fade-out lengths | all 5 | **In-model** → `fade_in` / `fade_out`, 0–30 s, `0` skips a side. Range is wider than every competitor scanned (best was 0.1–10 s). |
| Both-zero is meaningless | implied everywhere | **In-model** → rejected with an actionable message instead of silently returning the input. |
| Fade target: picture / sound / both | WuTools, EchoWave | **In-model** → `streams` enum `both` (default) / `video` / `audio`. `audio` stream-copies the picture and keeps the container — the lossless path. |
| Fade colour beyond black | Vidocu, EchoWave, ConvertFleet | **In-model** → `color`, and wider than the black/white pair the others offer: any ffmpeg colour name, `#rrggbb`, `0xrrggbb` or `name@alpha`. Validated against a strict character set so filtergraph punctuation can never escape the argument. |
| Output quality control | WuTools | **In-model** → `quality` enum high / balanced / small → CRF 18 / 23 / 28. Three named steps rather than WuTools' four; 23 is the ffmpeg default and matches this repo's other re-encoding video blocks. |
| Preview before download | Vidocu, EchoWave, Flixier | **In-model, already on the platform** → the generated page previews the result in a `<video>` element with a download button; no per-tool work needed. |
| One-click starting points | WuTools, ConvertFleet presets | **In-model** → five `[[example]]` chips (1 s both ends, fade-in only, cinematic 3 s tail, fade to white, sound only) that prefill and run. |
| Browser-local processing | WuTools, ConvertFleet | **Already true** → same ffmpeg-in-the-tab model; stated plainly on the page. |
| Stated limits | WuTools 100 MB, Vidocu 500 MB, ConvertFleet 2 GB | **In-model, documented** → 25 MB in and out, 30 s per fade, 10 h clip length, all spelled out in the page's limits section rather than discovered on failure. |

## Deliberately NOT built

- **Fade curves / easings** (EchoWave's ~30 opacity easings, `afade`'s curve
  list). Verified against the local ffmpeg build: the video `fade` filter is
  linear-only. Exposing a curve would bend the sound ramp while the picture ramp
  stayed linear, so the two would visibly desynchronise on any non-linear
  setting. One honest linear ramp beats a knob that silently breaks the pairing.
- **Cross-dissolve between two clips** (EchoWave, Flixier, ConvertFleet). A
  genuinely different operation: two inputs, an overlap window and an `xfade`
  filtergraph. Out of this tool's single-input shape.
- **Encode-speed preset** (WuTools). The preset is pinned to `veryfast` because
  ffmpeg-in-a-browser-tab is single-threaded; the slower presets multiply an
  already slow run for a few percent of file size, and `quality` already gives
  users the size/detail lever.
- **Output-container choice** (WuTools' same-as-input / MP4 / WebM / MOV). The
  picture path always writes H.264 in MP4 — the one container every input we
  accept can be normalised into and every browser plays. A WebM option would mean
  VP9/AV1 encoding, which is drastically slower in wasm. `streams=audio` already
  preserves the input container for the lossless case.
- **Alpha fading** (`fade`'s `alpha=1`). H.264 in MP4 has no alpha channel to
  fade, so the option would be inert on this tool's output path.
- **URL input on the page** (Vidocu). Server-side fetch-and-process; the page is
  local-only by design. The chat/CLI surface already takes a `url` (or a `ref`
  from a prior tool call) where a fetch is appropriate.
- **Accounts, watermarks, resolution tiers** (Vidocu, EchoWave). Nothing to
  match — there is no gating here.

## Follow-up noted, not done here (platform-level)

Every competitor discovers the clip length itself, so none of them asks for it;
this tool needs `duration` typed in for a fade-out because the argv is built
before the file is decoded. Auto-filling it would mean a **generator-level**
feature (read `loadedmetadata` from the selected file into a declaratively marked
field, e.g. `default = "media-duration"`), shared by `video-trim`,
`video-freeze-frame`, `loop-video` and friends — not a per-slug `custom.js` hack,
which the usability standards forbid. Left for a platform change; meanwhile the
page explains the field and where to read the number, and a fade-in-only run
never needs it.

## Model / surface notes

- ffmpeg cannot run in the chat Service Worker (`import()`/`Worker` are
  forbidden there), so the supported surfaces are the standalone **page** and the
  **CLI**. The descriptor/drift-guard tests still validate the exact schema chat
  would consume.
- The page uses the generic single-pass ffmpeg driver — one input, one argv, one
  output — so no `custom.js` is needed.

## Verification

Fixture: `tests/fixtures/tiny-av-128x128.mp4` — a 1 s, 128×128 H.264 clip with an
AAC track, so both the picture and sound paths have something real to ramp and
the exact duration is known. Covered end to end: the page's default both-ends
run, a `?fade_in=…&fade_out=…&duration=…` deep-link that pre-fills and runs, a
`streams=audio` page run that must keep the input container (`data:video/mp4`
from a stream copy), the CLI on a public MP4 URL, unit tests for every validation
branch and both argv shapes, and the repo hygiene gate.
