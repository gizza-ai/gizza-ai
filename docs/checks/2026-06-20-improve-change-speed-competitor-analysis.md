# change-speed — competitor analysis (2026-06-20)

Seventh `/create-next-tool` backlog pick (base64-file-converter was skiplisted
before it as a two-IO-shapes mismatch). ffmpeg media tool — surfaces: standalone
**page** + **CLI**; chat ffmpeg non-functional. Scoped to video (audio stays in
sync); standalone-audio files are a deferred variant (no Input::Audio kind, and
AssetKind::Video rejects audio mime). Research via `WebSearch`, paraphrased.

## Competitors surveyed
| tool | does well (paraphrased) | dimension |
| ---- | ----------------------- | --------- |
| miniwebtool / Flixier / Clideo | 0.25x-4x range; speed presets; ffmpeg.wasm, in-browser | capabilities |
| Kapwing / Adobe Express | pitch correction (natural-sounding audio at speed) | capabilities |
| videotools / zippyedit | custom factor + presets; slow-mo / timelapse framing | UX |

## Gap diff vs our tool
Our tool: a free `factor` from 0.25x to 4x; video PTS scaled (`setpts`) and audio
tempo matched (`atempo`, chained for factors outside 0.5-2.0) so picture + sound
stay in sync, keeping the container format.

**Already competitive / ahead:**
- **Pitch preservation** — `atempo` changes tempo WITHOUT shifting pitch, so audio
  sounds natural at any speed. Several competitors gate this as a premium toggle;
  it's our default behavior. (No work needed.)
- **Range** — 0.25x-4x matches the common competitor range.

**In-model gaps considered, deferred:**
- **Standalone audio files** (mp3/wav speed change) — needs an Input::Audio kind
  + AssetKind::Audio (the model currently has Image/Video/Document/File); a small
  framework addition, tracked as a follow-up.
- **Wider range (0.1x-10x)** — easy to widen later; 0.25-4x is the safe default.
- A "chipmunk" mode (drop pitch preservation) — a toggle, minor.

**Out-of-model:** per-segment speed ramps, frame-interpolated slow-mo (needs an ML
model), preset-button UI (the page is a simple form).

## Tested
unit (5: double/half setpts+atempo, large+small factor atempo chaining, plan
validation + extension) + drift-guard · `wafer build` validates the block ·
wasm-pack web · generator · Playwright page (uploads tiny-128x128.mp4 — no audio,
so the per-stream -filter:a is correctly skipped; in-browser ffmpeg → data:video)
· CLI on a real public video (Big_Buck_Bunny 10.0s → 4.98s at factor 2, ffprobe-
verified). Chat ffmpeg: non-functional — page + CLI are the surfaces.

> Original work only — no competitor copy, branding, or trademarks copied.
