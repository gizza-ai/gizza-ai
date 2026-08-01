# Competitor analysis — video-audio-hum-remover (2026-08-01)

Tool function: remove 50/60 Hz mains ("electrical") hum and its harmonics from a
video's audio track with a tuned chain of narrow notch (band-reject) filters,
leaving the picture untouched.

One WebSearch ("remove mains hum 50 60 Hz from video audio online tool notch
filter") → skimmed the three most relevant reachable competitors below. All
findings are paraphrased — no competitor copy, branding, or trademarks reused.

## Competitors skimmed

### 1. Audacity — Notch Filter (desktop, open source)
- **Params:** `Frequency` (default 60 Hz; guidance: 60 for North America, 50 for
  UK / most of the world; range up to Nyquist) and `Q factor` (default 1, min
  0.1; >1 = narrower notch, <1 = wider).
- **Mains-hum guidance:** apply at the fundamental, then inspect the spectrum and
  re-apply at each harmonic; "a Q between 2 and 10 works well for mains hum,"
  especially on higher harmonics to reduce artifacts.
- **UX:** text boxes for frequency + Q, a small preview of the notch shape.
- **Gap it exposes:** harmonics must be knocked out one-by-one manually. A tool
  that auto-notches the fundamental *and* N harmonics in one pass is strictly
  easier. → in-model (a filter chain).

### 2. StemSplit — Hum Remover (browser, AI)
- **Params:** none exposed — fully automated "targets the hum regardless of
  frequency"; no 50/60 choice, no Q, no strength.
- **Formats:** MP3/WAV/FLAC/M4A/OGG/WEBM/AAC/OPUS/AIFF/WMA, up to 500 MB, 30–60 s.
- **UX:** drag-drop, single "Remove Hum" button, before/after compare.
- **Tag:** AI black-box detection → **out-of-model** (needs an ML model; gizza is
  pure Rust + ffmpeg). We instead expose an explicit 50/60 choice — deterministic
  and transparent.

### 3. Notevibes — De-Hum (browser, Web Audio)
- **Feature:** "notch 50/60 Hz mains buzz + harmonics"; can apply to the whole
  clip or a selected region.
- **Params:** not documented publicly (no Q/harmonic-count/strength shown).
- **UX:** part of a multi-effect restoration editor.
- **Region-only application** → **out-of-model** here (single setting for the
  whole clip; trimming/region work is a separate gizza tool).

## Table-stakes → decisions

| Table-stake | Source | In / out of model | Decision |
| --- | --- | --- | --- |
| 50 vs 60 Hz fundamental | Audacity, Notevibes | in-model | `frequency` enum `{50,60}`, default 50 |
| Notch the harmonics too | Audacity, Notevibes | in-model | `harmonics` int 0–12 (fundamental + N), default 4 |
| Q / notch narrowness | Audacity | in-model | `q` slider 1–100, default 10 (Audacity 2–10 band, higher = narrower/safer) |
| Keep the picture untouched | (video tool) | in-model | `-c:v copy`, only audio re-encoded |
| Region-only application | Notevibes | out-of-model | whole-clip only; use the trim tool to select first |
| Automatic hum detection | StemSplit | out-of-model | needs an ML model; we expose an explicit, deterministic 50/60 choice |
| Drag-drop upload / before-after compare | StemSplit | in-model (platform) | page already has paste/drag upload + inline media preview |
| Preset buttons | (general UX) | in-model | `[[example]]` chips: Europe 50 Hz, North America 60 Hz, aggressive harmonics, narrow/music-safe |

## UX control patterns adopted
- `frequency` → `<select>` with friendly region labels via `[input.labels]`.
- `harmonics` and `q` → `kind = "slider"` (bounded numeric ranges).
- `[[example]]` preset chips for the two regions + an aggressive and a music-safe
  preset.

## Worked example designed into the page
Load a webcam clip with a steady 50 Hz buzz from a nearby power supply → leave
`frequency` on 50 Hz, `harmonics` 4, `q` 10 → notches at 50/100/150/200/250 Hz
pull the buzz down while the voice is untouched → `clip-dehummed.mp4`.

## Limits stated on the page
- Whole-clip only; one setting for the entire track.
- Classic notch chain, not an AI voice model — targets *steady* mains hum, not
  broadband noise (use the denoise tool for hiss).
- Input/output each capped at 25 MB (processed in browser memory).
- Video stream copied losslessly; container kept (mp4→mp4, webm→webm; Opus audio
  for webm, AAC otherwise).
