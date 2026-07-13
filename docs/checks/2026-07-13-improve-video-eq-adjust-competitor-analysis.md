# video-eq-adjust — competitor analysis (2026-07-13)

Tool function: adjust a video's brightness, contrast, saturation, and gamma in a
single ffmpeg pass (the `eq` filter). All observations below are paraphrased from
public product pages; no competitor copy, branding, or trademarks are reproduced.

## Competitors skimmed (top real results)

1. **Clideo — Adjust Video** (clideo.com/adjust-video). Upload, then drag sliders
   for brightness, saturation, contrast and more; live preview before export.
   Server-side processing.
2. **Kapwing — Adjust** (kapwing.com/tools/adjust). Sliders for brightness,
   saturation, contrast, opacity; also ships a set of one-click preset *filters*
   in addition to manual tweaking.
3. **Video Tools — Adjust** (videotools.app/tools/video-adjust). Browser-based
   ffmpeg filters, no upload; brightness, contrast, saturation, grayscale.
4. (cross-ref) **Flixier / VEED / Rendley** — same slider set (brightness,
   contrast, saturation, plus hue on Flixier); Rendley emphasises local/private
   processing.

## Table-stakes params & where each lands

| Capability             | Competitor norm                    | gizza decision |
|------------------------|------------------------------------|----------------|
| Brightness             | slider, signed                     | **in-model** — `eq=brightness` (-1..1, 0 = none) |
| Contrast               | slider                             | **in-model** — `eq=contrast` (0..4, 1 = none) |
| Saturation             | slider (0 = grayscale)             | **in-model** — `eq=saturation` (0..3, 1 = none) |
| Gamma                  | some tools / gamma-specific tools  | **in-model** — `eq=gamma` (0.1..10, 1 = none) |
| Live preview           | all                                | out-of-model — page is a one-shot ffmpeg encode, no realtime scrub |
| Preset filter looks    | Kapwing preset filters             | **in-model via preset chips** — `[[example]]` chips (brighten, vivid, faded, grayscale) |
| Local/private          | Rendley / videotools               | **in-model** — browser ffmpeg, nothing uploaded |
| Hue rotation           | Flixier                            | out-of-scope — `eq` has no hue; belongs in a dedicated video-hue tool (feasible via the separate `hue` filter, deferred, not silently dropped) |

## Defaults / worked example

Identity when brightness=0, contrast=1, saturation=1, gamma=1 (no visible change).
Worked example: brightness=0.1, contrast=1.2, saturation=1.4, gamma=0.9 → a
brighter, punchier, more vivid clip. ffmpeg filter:
`eq=brightness=0.1:contrast=1.2:saturation=1.4:gamma=0.9`.

## UX controls to match

- Four sliders (brightness / contrast / saturation / gamma) with sensible ranges.
- Preset chips for common looks (Kapwing-style presets, expressed declaratively).
- Re-encode H.264 + AAC; keep the input container when it can hold them.
