# audio-declick — competitor analysis (2026-08-09)

Scan run **before** implementing `blocks/audio-declick`. All findings are paraphrased
observations of publicly documented behaviour — no competitor copy, branding, or
trademarked wording is reused anywhere in the tool.

## Scope check (not a duplicate)

| Existing block | What it does | Why declick is distinct |
| --- | --- | --- |
| `audio-noise-reduce` | `afftdn` / `anlmdn` — **steady broadband** hiss/hum | Spectral denoisers smear or ignore single-sample impulses; they do not interpolate over them |
| `audio-noise-gate` | gates below a level threshold | A click is louder than the signal floor, so a gate never touches it |
| `video-audio-hum-remover` | narrow-band mains hum notch | Periodic tone removal, not impulsive repair |
| `clipping-detector` | **reports** clipped samples | Detection only, no repair |

`adeclick` is a different engine class: autoregressive interpolation over detected
impulsive samples. Verified locally against a synthetic fixture (see "Engine spike").

## Competitors reviewed

### 1. Elysia Tools — online audio declicker / click removal (browser tool, closest analogue)

- Inputs: WAV, MP3, FLAC, AAC, OGG, MP4, M4A; single file, 50 MB cap; uploads deleted after 6 h.
- Controls, all dropdowns:
  - **Click detection threshold** — Low (conservative) / Medium (balanced) / High (aggressive) / Very aggressive.
  - **Burst threshold** — Short (2 samples) / Medium (5) / Long (10) / Disabled.
  - **Detection method** — Adaptive (recommended) / Overlap-save.
- No presets, no in-page numeric fields; server-side processing (files are uploaded).

### 2. iZotope RX — De-click module (professional restoration standard)

- **Sensitivity** (strength, threshold-like), **Frequency skew** (bias toward high ticks vs low
  thumps), **Click widening** (extend the repaired region around a detection),
  **Output clicks only** (monitoring), algorithm modes incl. multi-band for wide vinyl clicks
  and thumps with periodic-content protection.
- Ships **presets by artifact type**: random discontinuity, random thumps, short digital clicks.
- Documented workflow: pick a preset → raise sensitivity until instruments degrade → back off →
  fine-tune widening/skew → re-run passes for stubborn artifacts.

### 3. Audacity — Click Removal / De-clicker (free desktop standard)

- Two parameters only: **Threshold** and **Max spike width**; documentation says the defaults
  work in most cases.
- Documented limit: needs a selection of at least 4096 samples, so it cannot repair one isolated
  click in a tiny selection; for dense soft vinyl crackle their docs point at noise reduction instead.

*(Adobe Audition's Click/Pop Eliminator page timed out repeatedly and was replaced by
Audacity + iZotope above; VinylStudio's public declick page documents no numeric settings —
only that it repairs bursts up to ~400 samples and has percussion/brass protection heuristics.)*

## Engine spike (run before deciding fit-to-model)

ffmpeg 
`adeclick` options: `w` window 10–100 ms (55), `o` overlap 50–95 % (75), `a` AR order 0–25 % (2),
`t` threshold 1–100 (2, lower = more detections), `b` burst 0–10 % (2), `m` add|save (add).
`adeclip` repairs clipped samples with the same AR interpolation.

Measured on a 3 s 440 Hz tone (peak −24.08 dB) with 12 full-scale two-sample spikes injected
(peak −0.21 dB):

| chain | output peak | reads as |
| --- | --- | --- |
| `adeclick=t=2` / `t=10` / `t=10.5` | **−24.08 dB** | clicks fully interpolated away |
| `adeclick=t=13` | −1.23 dB | partial |
| `adeclick=t=15` / `t=19.81` / `t=50` | −0.21 dB | clicks survive |
| `adeclick=t=10:w=25` | −5.23 dB | window too short for the burst |
| `adeclick=t=10:b=0` / `m=s` / `,adeclip` | −24.08 dB | all repair correctly |

On a *soft* crackle fixture (±4000 spikes) every threshold behaved the same, but
`m=add` altered every sample (mean abs deviation ≈ 9 LSB) while `m=save` left non-repaired
samples byte-identical (max deviation 1 LSB) — a real, user-visible difference worth exposing.

Conclusion: the useful threshold band is roughly **1–15**, not 1–100, so a friendly 1–100
strength slider must map into that band rather than pass through.

## Table-stakes matrix

| Capability | Elysia | iZotope | Audacity | Decision |
| --- | --- | --- | --- | --- |
| Detection strength / sensitivity | ✅ 4-step dropdown | ✅ slider | ✅ threshold | **IN** — `strength` 1–100 slider → `t = clamp(20 − 0.19·strength, 1, 20)`; default 50 → `t = 10.5`, verified to repair |
| Burst / max spike width | ✅ 2/5/10/off | ✅ click widening | ✅ max spike width | **IN** — `burst` 0–10 (0 = off), default 2 |
| Detection / overlap method | ✅ adaptive vs overlap-save | — | — | **IN** — `method` add\|save, default add |
| Repair window (wide thumps vs narrow ticks) | — | ✅ multi-band for wide clicks | — | **IN** — `window` 10–100 ms, default 55 |
| Artifact-type presets | — | ✅ 3 presets | — | **IN** — four one-click page chips (vinyl crackle, digital glitches, gentle, declick+declip) |
| Declip / clipped-peak repair | — | ✅ separate De-clip module | — | **IN** — `declip` checkbox chains `adeclip` after the declick stage |
| Output format choice | ✅ (mirrors input set) | ✅ | ✅ | **IN** — mp3 / wav / ogg / flac / m4a, default mp3 |
| Local, nothing uploaded | ❌ (server upload, 6 h retention) | n/a desktop | n/a desktop | **IN, and a genuine advantage** — page runs ffmpeg in WebAssembly |
| Frequency skew (bias to ticks vs thumps) | — | ✅ | — | **OUT of model** — `adeclick` has no frequency-weighted detector; approximated only by `window` |
| "Output clicks only" monitoring | — | ✅ | — | **OUT of model** — needs a live A/B monitor; gizza surfaces render one static output |
| Interactive spectral repair / per-click editing | — | ✅ | ✅ manual | **OUT of model** — no interactive canvas on any gizza surface (same class as the skiplisted `pixel-art-editor`) |
| Percussion / brass protection heuristics | — | ✅ (multi-band) | — | **OUT of model** — not implemented by `adeclick`; mitigated by documenting `method=save` + lower strength |
| Multiple repair passes in one run | — | ✅ | — | **OUT of scope** — run the tool again on its own output |

## Copy / UX gaps closed

- Competitors' plain-language framings ("clicks, pops, crackle, ticks, vinyl scratches, digital
  glitches") are matched with our own wording so search intent lands.
- The 4-step dropdowns of the closest online competitor are covered more finely by a 1–100 slider
  plus preset chips — the chips give the one-click path, the slider gives the fine control.
- Their documented limits (50 MB, server upload, 6 h retention) are answered on our page with the
  in-browser / never-uploaded guarantee and the honest ~10 MB in/out cap.
- Audacity's documented failure mode (dense soft crackle is better served by noise reduction) is
  reproduced as an FAQ pointing at `audio-noise-reduce`, plus the spike's finding that soft
  crackle only partially responds.
