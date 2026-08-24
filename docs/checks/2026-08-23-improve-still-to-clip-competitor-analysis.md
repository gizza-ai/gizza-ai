# still-to-clip — competitor analysis (2026-08-23)

Tool under build: **still-to-clip** — "Turns a single still image into a fixed-duration static
video clip at a chosen resolution and frame rate." (ffmpeg, image → video).

All notes below are paraphrased observations of publicly documented feature sets. **No competitor
copy, branding, or trademark is reproduced anywhere in the tool.**

## Duplicate check (done first)

| Existing block | Overlap? | Verdict |
| --- | --- | --- |
| `video-ken-burns` | image → mp4, but ALWAYS animates (zoompan pan/zoom) and always `scale=…:force_original_aspect_ratio=increase,crop` (cover-crops the photo). Zoom is validated `1.0..=2.0`, so even at 1.0 the framing is a forced center-crop with no pad/letterbox path. | **Not a dup.** still-to-clip is deliberately motionless and adds fit/pad/background + container/quality choice. |
| `loop-video`, `seamless-loop-video` | input is a **video/GIF**, not a still. | Not a dup. |
| `gif-from-images`, `images-to-animated-webp`, `ken-burns-slideshow-video` (skiplisted) | multi-image inputs. | Not a dup (single still here). |
| `video-aspect-pad` | pads an existing **video**. | Not a dup. |
| `video-title-card` | overlays text on an existing video. | Not a dup. |

## Competitors reviewed

1. **XConvert — image to video** (xconvert.com/convert-image-to-video)
2. **online-convert — JPG to MP4** (video.online-convert.com/convert/jpg-to-mp4)
3. **Kapwing — image to video** (kapwing.com/tools/convert/image-to-video)
4. Secondary skim: CoolUtils PNG/JPG→MP4, onlineconverter.com image-to-video (both are
   upload-and-wait converters with a single "duration" control and no other knobs).

## Table-stakes matrix

| Capability | Seen in | Fit to our model | Where it landed |
| --- | --- | --- | --- |
| Hold duration in seconds | all | in-model | `duration` (0.1–60 s, default 5) |
| Frame rate | online-convert (explicit fps), XConvert (implied by per-frame hold) | in-model | `fps` (1–60, default 30) |
| Output resolution + presets | XConvert (1080×1920 / 1080×1080 / 1920×1080 / 3840×2160), online-convert (custom W/H) | in-model | `width`/`height` (16–3840, default 1920×1080) + preset example chips for widescreen / square / vertical / 4K |
| "Keep original size" | XConvert (default) | in-model | `fit = original` (snaps to even dims, caps at 3840) |
| Aspect handling: keep-ratio bars vs stretch vs crop | online-convert (black/white bars, crop, stretch) | in-model | `fit = contain \| cover \| stretch` |
| Background/bar colour | XConvert (white default, custom), online-convert (black/white bars) | in-model | `background` (CSS name or hex, colour-picker control, default black) |
| Output container choice | XConvert (10+), online-convert (10+) | partially in-model | `format = mp4 \| webm \| mov` — the three the browser ffmpeg build and every player handle; the long tail (wmv, flv, 3gp, avi, mpeg, ogv, ts) is legacy and omitted |
| Quality / compression control | XConvert (quality preset + CRF), online-convert (bitrate/filesize) | in-model | `quality` 1–100 → CRF (matches the repo-wide convention in `video-transcode`) |
| Codec picker (h264/h265/vp9/av1/…) | XConvert, online-convert | out-of-model | Not exposed: codec is implied by container (mp4/mov → H.264, webm → VP9). x265/AV1 aren't in the browser ffmpeg build. |
| Blurred-background bars | online-convert | out-of-model **for this tool** | Already shipped as an option on `blocks/video-aspect-pad`; not duplicated here. |
| Audio track / background music | Kapwing, onlineconverter.com | out-of-model | Needs a second media input (the page has one file input). `blocks/video-add-silent-audio` already adds a silent track to a finished clip — documented in the FAQ. |
| Multi-image slideshow / per-image durations | XConvert (merge mode), Kapwing | out-of-model | Multi-input ffmpeg is un-buildable in this model (single file input; see `ken-burns-slideshow-video` skiplist entry). |
| AI animation / templates | Kapwing | out-of-model | Needs an ML model; gizza is pure Rust + ffmpeg. |
| Rotate / mirror / crop-pixels / deinterlace | online-convert | out-of-model **here** | Covered by existing dedicated blocks (`rotate-image`, `flip-image`, `image-crop`). |
| RAW/HEIC/PSD input | XConvert | out-of-model | The runtime accepts the `image/*` MIME class; RAW/PSD decode isn't in the ffmpeg build. PNG/JPEG/WebP/BMP/GIF work. |
| Preset chips / one-click sizes | XConvert (resolution presets) | in-model | 4 `[[example]]` chips on the page. |
| Slider controls | — (competitors use plain boxes) | in-model | `duration`/`fps`/`quality` render as sliders; better than the plain boxes competitors ship. |

## Differentiators we ship

- Runs entirely in the browser — the image is never uploaded (every competitor above is a
  server-side upload/convert/download round trip).
- Same tool on three surfaces: page, `gizza` CLI, and chat.
- Explicit `fit` semantics (contain / cover / stretch / original) with a real colour picker for
  the pad colour, rather than a fixed black/white bar choice.

## Decisions

- Motion stays out: that is `video-ken-burns`' job, and keeping this one static is what makes it a
  distinct tool rather than a redundant one.
- No audio parameter: a separate single-purpose block already covers it, and synthesising silence
  would need a second (lavfi) input.
- Container list stopped at mp4/webm/mov deliberately — every additional legacy container adds a
  muxer/codec pair that the page's ffmpeg build may not carry.
