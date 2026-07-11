# video-audio-gain — competitor analysis (2026-07-10)

Function: raise or lower the audio volume of a video by a chosen amount, keeping
the picture untouched.

## Competitor scan (paraphrased, no copy reused)

WebSearch: "increase decrease audio volume of video online tool free". Skimmed
three real competitors:

1. **VideoLouder** — pick a video, choose how many **decibels** to increase or
   decrease the sound; browser-based, no install. Simple single dB control.
2. **OnlineConverter (Increase Video Volume)** — change volume by **percentage**
   (50%, 100%, 200%…) **or by decibels** (1 dB, 10 dB…). Two unit modes.
3. **EZGIF (boost volume)** — increase, decrease, or mute; **percentage** input,
   boosts up to 1000% of the original (i.e. a ×10 factor). Keeps the video.

(Also seen: Online Video Cutter's drag slider up to 4 GB, MP3Cut "no quality
loss", Kapwing.)

## Table-stakes params / defaults / UX

| Capability | Competitor norm | In/out-of-model | Decision |
|---|---|---|---|
| Gain in **decibels** (±) | VideoLouder, OnlineConverter | in-model | `amount` + `unit=db` (default) |
| Gain as **factor/percentage** | EZGIF (%), OnlineConverter (%) | in-model | `unit=factor` (2 = 200% = double) |
| **Lower** volume (negative dB / <1×) | all | in-model | negative dB / factor in (0,1) accepted |
| **Keep video quality** (no re-encode of picture) | MP3Cut "no quality loss" | in-model | `-c:v copy`, only audio re-encoded |
| **Clip/peak protection** on boosts | (implicit) | in-model | `limiter` (alimiter), on by default |
| **Presets** ("2× louder", "half") | slider UIs | in-model | page `[[example]]` chips + slider |
| Large files (up to 4 GB) | Online Video Cutter | out-of-model | 25 MB cap (browser wasm memory) — documented |
| Batch / timeline volume automation | Kapwing | out-of-model | single-clip, constant gain — documented |
| Mute entirely | EZGIF | already a tool | see `blocks/video-mute` |

## Feasibility spike

ffmpeg chain: `-i in -c:v copy -af "volume=6dB,alimiter=limit=1:level=disabled"
-c:a <aac|libopus> out.<same-container>`. Video stream-copied (lossless, fast),
audio re-encoded (required — the `volume` filter changes samples). Audio codec
chosen by container (webm→libopus, otherwise aac). Verified as the same proven
approach as `blocks/audio-volume-adjust`.

## Decisions

- In-model, built into the descriptor: `amount`, `unit` (db|factor), `limiter`.
- Out-of-model, **not built**: 4 GB uploads, batch, timeline/keyframe automation.
- Distinct from `audio-volume-adjust` (outputs an audio file, drops video) and
  `video-mute` (removes audio entirely). This keeps the video and re-gains its
  audio.
