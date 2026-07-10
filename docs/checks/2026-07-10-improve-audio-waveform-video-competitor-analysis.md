# audio-waveform-video — competitor analysis (2026-07-10)

Goal: turn an audio track into an animated waveform MP4 (audiogram) with the
original audio muxed back in and in sync. Audio in → video out.

## Competitors skimmed

- **EchoWave** (echowave.io) — browser audiogram maker: upload MP3/WAV, pick a
  bar / line / radial / circular style, set colors and aspect ratio, render MP4.
  No signup. Runs client-side.
- **Exemplary AI** (exemplary.ai) — template-based audiogram export to MP4;
  bar/line styles, brand colors, captions (paid).
- **Recast** (recast.studio) — import MP3/WAV/M4A, pick colors, bar count,
  animation behavior, subtitle style, branding; platform-sized outputs.
- **Wave.video audiogram**, **Melobytes**, **audiowaveform.org** — same shape:
  style presets, color, aspect ratio, animated MP4 for social.

## Table-stakes → decision

| Capability (table-stake)                     | In/out of model | Where it lands |
|----------------------------------------------|-----------------|----------------|
| Multiple wave styles (bar/line/wave/point)   | in-model        | `mode` = mirror\|bars\|wave\|points (ffmpeg `showwaves`) |
| Wave color                                   | in-model        | `color` (hex, incl. #RRGGBBAA alpha) |
| Gradient fill                                | in-model        | `color2` → horizontal color→color2 gradient (gradients+alphamerge) |
| Background color                             | in-model        | `background` (opaque; MP4 has no alpha) |
| Aspect ratio / frame size (16:9, 9:16, 1:1)  | in-model        | `width` × `height` + preset chips (720p, 9:16, square) |
| Frame rate                                   | in-model        | `fps` 5–60 |
| Boost quiet audio so the wave keeps moving   | in-model        | `scale` = lin\|sqrt\|cbrt\|log (`showwaves scale`) |
| Original audio kept in sync                  | in-model        | `-map [v] -map 0:a -shortest`, source audio muxed |
| Style/color presets                          | in-model        | three `[[example]]` chips (audiogram, vertical story, square gradient) |
| Radial / circular visualizer                 | **out-of-model** | `showwaves` is linear only; a circular visualizer needs `showcqt`/custom compositing — a separate tool, not this one |
| Captions / subtitles / auto-transcription    | **out-of-model** | belongs to a caption-burner tool (we ship `video-caption-burner`); not an audiogram concern |
| Brand logo / image background overlay        | **out-of-model** | image-over-video compositing is a different tool |
| Progress bar / playhead                      | **out-of-model** | not a `showwaves` feature |

Every table-stake is either implemented in the descriptor or explicitly listed
out-of-model above — none dropped silently. Copy is paraphrased; no competitor
branding or wording is reused.

## UX controls matched

- `mode` and `scale` render as labelled `<select>`s (`[input.labels]`).
- `color` / `color2` / `background` use the native color-swatch control (`kind = "color"`).
- `width` / `height` / `fps` use sliders (`kind = "slider"`).
- Three preset chips (`[[example]]`) mirror competitors' style presets:
  default 720p audiogram, 9:16 vertical story, square gradient.
