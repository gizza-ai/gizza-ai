# video-mute-section — competitor analysis (2026-07-25)

Function: silence the audio over one chosen `[start, end]` time range of a video
while leaving the rest of the soundtrack intact and the picture untouched.

## Competitors scanned

1. **WuTools — Mute Video** (wutools.com/video/mute-video) — has a dedicated
   "Mute a Time Range" mode: enter a start and end time (seconds like `12.5` or
   timecode `00:01:30`); video is copied losslessly, only audio re-encoded; runs
   locally in the browser; input mp4/webm/mov/avi/mkv/m4v; output "same as input"
   (bit-for-bit video copy) or convert to mp4/webm.
2. **Clideo — Mute Video / "How to mute part of a video"**
   (clideo.com/mute-video) — supports muting a whole video, and a manual
   workflow to mute a section: split the track at a playhead, split again at the
   end of the section, set that chunk's volume to 0. Timeline/playhead driven,
   account/watermark on free tier.
3. **VEED — Mute Video** (veed.io/tools/remove-audio-from-video) — split the
   clip and mute selected sub-clips to silence specific sections; full timeline
   editor; watermark/subscription.
4. **Kapwing — Mute** (kapwing.com/tools/mute) — keyframe-based muting of
   individual parts on a timeline; account required; watermark on free.
5. **EZGif — Mute video** (ezgif.com/mute-video) — free, no watermark, but mutes
   the ENTIRE track (no time-range option).

## Table-stakes params / behaviors

| Capability | In model? | Where |
| --- | --- | --- |
| Start + end time to silence one range | ✅ | `start`/`end` number params (seconds) |
| Lossless picture (video stream copy, audio-only re-encode) | ✅ | core `-c:v copy` + audio re-encode |
| Keep input container / format support (mp4/mov/mkv/m4v/webm) | ✅ | `copy_out_ext` + webm→Opus, else AAC |
| Private / local / no watermark | ✅ | page runs ffmpeg in-browser, no upload |
| Fractional-second precision (e.g. drop one word) | ✅ | `f64` seconds, `fmt_num` keeps decimals |
| Deep-link / one-click presets | ✅ | `?start=&end=` deep-link + `[[example]]` chips |

## Out-of-model (listed, not built)

- **Timecode `HH:MM:SS` entry.** Competitors accept both seconds and timecode;
  we accept seconds (plain numbers, decimals allowed) which fully covers the same
  range without a timecode parser + drift surface. Noted in a page FAQ.
- **Multiple ranges in one pass.** WuTools/VEED/Kapwing use a timeline to mute
  several sub-clips at once. Our model is one range per run (re-run on the output
  for more); documented in content + a FAQ. A visual scrubbing timeline is a UI
  affordance the static generator doesn't provide.
- **Isolating one voice / one instrument** (mute just the music) — needs an AI
  source-separation model; out of scope for pure ffmpeg. Documented in a FAQ.
- **Convert-to-different-container on output** (WuTools' mp4/webm re-mux choice)
  — we keep the input container to preserve the lossless copy; switching would
  force a re-encode in some cases. Kept simple.

## Decisions

- Two required params `start`/`end` (seconds), `end > start` validated in core.
- `volume=enable='between(t,start,end)':volume=0` timeline gate — only the window
  is zeroed, the rest passes through; `-c:v copy` keeps the picture lossless.
- Audio codec follows the kept container (webm→libopus, else aac) so the copy is
  always valid, matching the `video-audio-fade` precedent.
- Three example chips (mid-clip range, first-N-seconds, sub-second word bleep).

Never copied competitor copy, branding, or trademarks.
