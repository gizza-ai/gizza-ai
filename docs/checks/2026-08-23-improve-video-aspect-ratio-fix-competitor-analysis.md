# video-aspect-ratio-fix — competitor scan (2026-08-23)

Scan run BEFORE implementation, per `/create-next-tool` step 4. All notes are **paraphrased
observations of capability and UX**; no competitor copy, branding, or trademarks are reproduced or
reused. Out-of-model items are listed for the record, not built.

## What the tool does

Sets the **display aspect ratio (DAR) tag** on a video whose stored aspect metadata is wrong —
the classic "anamorphic / squeezed / stretched" file where the pixels are fine but the container
tells the player the wrong shape. Implemented as `ffmpeg -i in -map 0 -c copy -aspect W:H out`,
i.e. **stream copy only**: no decode, no re-encode, bit-for-bit identical audio/video packets.

Verified by spike before descriptor design (`ffprobe` before → after, 640×480 test clip):

| command | reported SAR | reported DAR | bytes |
|---|---|---|---|
| source | 1:1 | 4:3 | 15349 |
| `-c copy -aspect 16:9` (mp4) | 4:3 | 16:9 | 15349 |
| `-c copy -aspect 2.39` (mkv) | 717:400 | 239:100 | — |
| `-c copy -aspect 21:9` (mov) | 7:4 | 7:3 (= 21:9 reduced) | — |
| `-c copy -aspect 16:9` (webm) | 427:320 | 427:240 (≈16:9, Matroska integer display dims) | — |

Two spike findings that shaped the design:

* `-aspect 0` is rejected (`Invalid aspect ratio: 0`) — there is **no** dimension-independent
  "reset to square pixels" with stream copy. The reachable recipe is to pass the video's stored
  pixel size as the ratio (`-aspect 640:480` → SAR 1:1), so the custom field accepts a `WxH` form
  and the FAQ documents that recipe.
* `-bsf:v h264_metadata=sample_aspect_ratio=1/1` writes the H.264 VUI but the container tag still
  wins on read, and it only works for H.264/HEVC — dropped as out-of-model (see below).

## Competitors reviewed (top 3 + a desktop reference)

### 1. Browser-side "change video aspect ratio" tool (ffmpeg-wasm, no upload)
Ratio presets 16:9, 9:16, 1:1, 4:3, 3:2, 21:9 plus a free-form custom width×height. Three
processing methods — stretch, crop-to-ratio, fit-with-bars. Output container choice across
mp4/mkv/mov/webm. Four re-encode quality tiers. States a 100 MB input cap and that processing is
local. FAQ covers landscape→vertical, legacy 4:3→16:9, and quality retention.

### 2. Long-running online GIF/video utility site — video resize page
Width/height/percentage sizing plus a preset resolution list (1080p, 720p, 4K, 1080×1920,
1080×1080, 480p, 360p). Notable for an explicit **"copy original" encoding option that remuxes
without re-encoding** — the same lossless philosophy as this tool. Wide input-format list
(mp4/webm/avi/mpeg/mkv/flv/ogg/mov/m4v/wmv/asf/3gp), 200 MB cap, files deleted server-side after
an hour.

### 3. General online converter — resize video page
Six sizing modes (keep-ratio, fixed/stretch, crop, max-width, max-height, auto bar removal),
ratio choices 16:9, 9:16, 4:3, 1:1, 5:4, 4:5, landscape/portrait resolution presets, MP4-only
output, 10–2998 px even-number range, 200 MB cap.

### 4. Desktop reference: Matroska muxer CLI (closest true DAR-tagging analogue)
Three mutually exclusive per-track options: set display dimensions as `WxH`; set an aspect ratio
as **either a fraction (`16/9`) or a decimal (`1.78`)**; or multiply the source ratio by a factor.
Display width/height are derived automatically. This is the only competitor that does what this
tool does — tag, don't re-encode — and it is where the accepted-value-forms table stake comes from.

## Table stakes → decision

| # | Table stake (from the scan) | In/out of model | Where it lands |
|---|---|---|---|
| 1 | Ratio presets 16:9, 9:16, 4:3, 1:1, 3:2, 21:9 (+ 3:4, 2:3, 4:5, 5:4 portrait siblings) | **in** | `aspect` `Param::enumv`, 13 values, default `16:9`, friendly `[input.labels]` |
| 2 | Cinema ratios 2.39:1 / 1.85:1 | **in** | preset enum values (spike: normalized to exact integer ratios 239:100 / 37:20) |
| 3 | Custom / free-form ratio | **in** | `custom_aspect` string, active when `aspect = custom` |
| 4 | Accept a **fraction** form (`16/9`) | **in** | custom parser accepts `:` and `/` |
| 5 | Accept a **decimal** form (`1.78`) | **in** | custom parser accepts a bare decimal (den = 1) |
| 6 | Accept **display dimensions** (`1920x1080`) | **in** | custom parser accepts `x`/`X`/`×`, reduced by gcd |
| 7 | Output container choice mp4/mkv/mov/webm | **in** | `container` `Param::enumv` `keep\|mp4\|mkv\|mov\|webm`, default `keep` |
| 8 | Web-streamable output | **in** | `faststart` boolean (default true, MP4/MOV only) |
| 9 | Preset one-click chips | **in** | four `[[example]]` chips (16:9 desqueeze, 9:16, 2.39:1, custom 1.85 → mp4) |
| 10 | Stated input cap + local processing | **in** | page copy + 64 MB `MAX_INPUT_BYTES` |
| 11 | Stated supported input formats | **in** | page "Limits and edge cases" |
| 12 | Lossless / "copy original" mode | **in** | this tool is *only* that — stream copy is the whole design |
| 13 | Stretch / crop / letterbox pixel methods | **out (covered elsewhere)** | those re-encode pixels; `blocks/video-aspect-pad` (letterbox/blur-pad), `blocks/video-crop`, `blocks/video-resize` already own them. Cross-linked from the page rather than duplicated here. |
| 14 | Re-encode quality tiers (CRF/preset) | **out (N/A)** | nothing is re-encoded; the page says so explicitly instead of offering a dead control |
| 15 | Aspect-ratio **factor** (multiply the source ratio) | **out** | needs to probe the source ratio first; this tool builds an argv without a probe pass. Reachable manually: read the current ratio, multiply, pass the result as a custom ratio (documented in the FAQ). |
| 16 | Dimension-independent "reset to square pixels" | **out (partial)** | no stream-copy ffmpeg form exists (`-aspect 0` errors). The equivalent — pass the stored pixel size as the ratio — is exposed via the `WxH` custom form and documented. |
| 17 | Bitstream-level SAR rewrite (`h264_metadata` BSF) | **out** | spiked: the container tag still wins on read, and it is H.264/HEVC-only. Not worth a codec-specific control. |
| 18 | Server-side batch / accounts / hour-long retention | **out (model)** | browser-local, no account, no server — nothing to retain |

## UX control patterns adopted

* `<select>` with friendly labels for the ratio (competitors all use preset buttons/dropdowns, never a raw text box).
* One-click preset chips (`[[example]]`) mirroring competitor preset rows.
* A separate custom field with a worked-example placeholder, so the common path is one click and the
  power path is still exact.
* Limits, supported containers, and the "this does not change pixels" contract stated on the page —
  competitors 1 and 2 both surface caps up front, competitor 3 hides the even-number rule until it errors.
* Errors name the expected forms (`16:9`, `16/9`, `1.85`, `1920x1080`) rather than a bare "invalid".
