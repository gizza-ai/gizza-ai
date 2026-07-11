# video-denoise — competitor analysis (2026-07-11)

Tool function: reduce visual noise/grain in a **video's picture** with ffmpeg's
`hqdn3d` (temporal + spatial 3D) or `nlmeans` (non-local means) denoiser, then
re-encode the picture to H.264 (`-crf 20`) while stream-copying the audio when
the container allows. Surfaces: standalone page + CLI (chat ffmpeg is
unavailable in the Service Worker). Picture-side sibling of the built
`video-audio-denoise` (which cleans the audio track instead).

> Note: this analysis is based on **known ffmpeg denoise capabilities and the
> common feature set of browser/desktop video-denoise tools**, not on verified
> live competitor pages (no web search was run for this pass). To avoid
> fabrication, no specific competitor product names are cited below — the
> "market" column describes the widely-seen table-stakes shape, not a named
> vendor.

## Table-stakes params, defaults, worked examples

| Capability | Common in market | Our decision | In/out of model |
|---|---|---|---|
| Denoise strength/intensity control | single strength/level slider | `strength` 1–100 slider, default 30 | **in** |
| Choice of denoise algorithm/quality | "fast" vs. "high quality" preset | `method` = `hqdn3d` (fast, default) or `nlmeans` (slower, detail-preserving) | **in** |
| Temporal + spatial denoise | implied by "video" denoisers | hqdn3d averages across pixels and frames | **in** |
| Keeps audio, picture-only change | typical | audio `-c:a copy` when container kept; AAC only on webm→mp4 | **in** |
| Fully in-browser / private | privacy-first web tools | ffmpeg runs in the page tab; nothing uploaded | **in** |
| Deterministic, deep-linkable settings | rare | `?method=…&strength=…` query params drive the page | **in** |
| AI upscale / restoration / detail synthesis | ML "enhance" products | out — gizza is pure-Rust + ffmpeg, no ML model | **out-of-model** (documented, not built) |
| Per-scene / masked denoise | pro NLE plugins | out — one setting applied to the whole clip | **out-of-model** |
| Before/after A/B preview | some web tools | out — page shows processed result + download only | **out-of-model** (UI feature, not a param) |
| Sharpen after denoise | some one-click tools | out — compose with a separate sharpen/filter tool | **out-of-model** |

Every table-stake is either in the descriptor (`method`, `strength`, implicit
audio-copy) or listed above as out-of-model. Nothing is dropped silently.

## Design notes

- **strength → filter mapping** (deterministic, unit-tested):
  - `hqdn3d`: `luma_spatial = strength / 10` (0.1–10), with the other three
    components in ffmpeg's default proportions (`cs = 0.75·ls`, `lt = 1.5·ls`,
    `ct = 1.125·ls`). Strength 40 reproduces ffmpeg's own `4:3:6:4.5` default.
  - `nlmeans`: `s = 1 + (strength-1)·14/99`, mapping into nlmeans' useful 1–15
    band (its native default of 1.0 barely denoises).
- Default **strength 30** (moderate) matches the "start moderate, raise until
  grain is gone" guidance; over-denoising smears fine detail.
- Default **method hqdn3d** for speed; the page tests the non-default nlmeans
  path via a deep link.
- Picture is re-encoded to H.264 `-crf 20` (near-transparent). Container kept
  for mp4/mov/m4v/mkv; other inputs (e.g. webm) convert to MP4.
- No copy/branding/trademarks reproduced from any product; wording is original.
