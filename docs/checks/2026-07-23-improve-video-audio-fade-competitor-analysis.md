# video-audio-fade — competitor analysis (2026-07-23)

Tool function: add an audio-only fade-in at the start and/or fade-out at the end of a
video, without touching the picture (video stream copied losslessly).

## Competitor scan (paraphrased — no copy/branding reproduced)

Searched: "add audio fade in fade out to video online without re-encoding video".
Top real tools skimmed:

1. **ImageOnline — fade audio in a video.** Upload a video, set fade-in and fade-out
   lengths, get the video back with only the audio faded. Advertises keeping the video
   without re-encoding it. Supports MP4/MOV/MKV. Two independent length controls.
2. **Clipchamp (blog + editor).** Full timeline editor; audio *and* video fades are a
   drag handle on the clip. Fades are applied per-clip on a timeline; export re-encodes.
3. **Flixier — video fade effect.** Online editor; fades audio in/out along with the
   video. Preset-style fade handles, cloud render.

(Audio-only competitors — AudioEditor.org, Notevibes, Adobe Express audio fade — fade an
audio *file*, not a video, so they map to the existing `audio-fade` tool, not this one.)

## Table-stakes params (each tagged in-model / out-of-model)

| Capability | Tag | Decision |
|---|---|---|
| Fade-in length (seconds) | in-model | `fade_in` number param, slider 0–30 s |
| Fade-out length (seconds) | in-model | `fade_out` number param, slider 0–30 s |
| Either side independently (fade only in, only out, or both) | in-model | 0 skips a side; both-0 rejected as no-op |
| Keep the picture without re-encoding | in-model | `-c:v copy` (lossless, fast); only audio re-encoded |
| Keep input container / format | in-model | output keeps input container (mp4→mp4, webm→webm); audio codec matched (webm→libopus, else aac) |
| Preset fade lengths (quick chips) | in-model | `[[example]]` chips: gentle 3 s both, 1 s quick, fade-out only, cinematic 5 s |
| Slider UX for durations | in-model | `kind = "slider"`, step 0.5 |

## Out-of-model (listed, NOT built)

- **Timeline / per-clip fade handles** (Clipchamp, Flixier) — needs a multi-clip NLE
  timeline; gizza is a single-shot ffmpeg transform. Out of model.
- **Video (picture) crossfade / fade-to-black** — that is a *picture* fade, a different
  tool; this tool is deliberately audio-only (picture is copied untouched). A separate
  video-fade tool would cover it.
- **Fade curve shape (log/exp/quarter-sine presets)** — ffmpeg `afade:curve=` *does*
  support this in-model, but no scanned competitor exposes it and it adds a rarely-used
  control; left out to keep the UI to the two table-stake length controls. Recorded here
  as a possible future in-model addition, not a competitor gap.
- **Cloud rendering / accounts / longer files** — competitors upload to a server; gizza
  runs ffmpeg in the browser/CLI, capped at 25 MB. Privacy trade-off, intentional.

## Technical note (spike)

Fade-out start time = duration − fade, which is unknown at argv-build time (argv is built
before decode). Reuse the proven `areverse,afade=t=in,areverse` trick from `audio-fade`:
fading-in a reversed stream needs no duration. Video is `-c:v copy`, so ffmpeg only
buffers the (small) audio stream for the reverse — cheap within the 25 MB input cap.
