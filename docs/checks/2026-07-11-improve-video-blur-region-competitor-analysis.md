# video-blur-region — competitor analysis (2026-07-11)

Tool function: blur or pixelate a fixed rectangular region (license plate, name tag,
logo) across every frame of a video, in the browser, no upload.

## Competitors scanned (top 3+ real tools; paraphrased only, no copy/branding reused)

1. **Blur The Video** (blurthevideo.com) — free, browser-local, "nothing is uploaded".
   Offers both Gaussian **blur** (soft) and **pixelate** (coarse mosaic). Notes pixelate
   is preferred for redaction (intentional, harder to reverse). Manual region selection.
2. **moviemakeronline.com — Blur part of video** — censor tool to blur/pixelate a
   specific area/text/object; manual rectangle, adjustable intensity.
3. **AVCLabs Video Blur AI** — select area with **rectangular or elliptical** shapes,
   adjust blur **size and intensity**; also custom image/sticker cover.
4. **BGBlur / Vidio.ai / FlexClip** — AI auto-detection + **motion tracking** of faces /
   plates / vehicles across moving frames (no manual masking).

## Table-stakes params → decision

| Capability | In/out of model | Decision |
|---|---|---|
| Region rectangle (x, y, width, height) | in-model | **descriptor** (required-ish) |
| Blur vs pixelate mode | in-model | **descriptor** `mode` enum |
| Blur/pixelate intensity | in-model | **descriptor** `strength` (slider 1–100) |
| Keeps audio, re-encode locally | in-model | implemented (aac copy of stream) |
| Elliptical / freeform mask shape | in-model but out-of-scope | listed — tool scope is a *fixed rectangle*; ffmpeg alpha-mask ellipse is feasible but a different UX; not built |
| Multiple simultaneous regions | in-model but out-of-scope | listed — single-region scope keeps the form clean |
| AI face/plate auto-detection | **out-of-model** | listed — needs an ML detector; gizza is pure-Rust + ffmpeg |
| Motion tracking (region follows a moving subject) | **out-of-model** | listed — needs per-frame ML tracking; this tool blurs a *fixed* rect |
| Custom image/sticker cover instead of blur | out-of-scope | listed — separate overlay tool |

## Spike notes (feasibility ≠ model fit)
- Blur: `crop=W:H:X:Y,gblur=sigma=S` → `overlay=X:Y` — verified filtergraph shape, sigma
  unbounded so no radius-vs-dimension crash (unlike boxblur). **Built.**
- Pixelate: crop the region, `scale` down to `W/S:H/S` then back up `flags=neighbor` →
  coarse mosaic; overlay back. **Built.**
- Ellipse: doable via a `geq` alpha mask + full-frame blur overlay, but materially more
  complex and off-scope for a "rectangular region" tool — **not built, listed above.**

## Out-of-model / considered-not-built (never forced in)
- AI auto-detection of faces/plates and motion tracking (ML model required).
- Elliptical / freeform / multiple region masks (scope: one fixed rectangle).
- Sticker/image cover overlay (different tool).
