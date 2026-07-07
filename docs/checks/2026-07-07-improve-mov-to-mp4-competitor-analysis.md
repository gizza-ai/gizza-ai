# mov-to-mp4 — competitor analysis (2026-07-07)

Scan done BEFORE finalizing the descriptor. All findings paraphrased — no
competitor copy, branding, or trademarks reproduced. Out-of-model items are
listed, not built.

## Competitors reviewed (top real MOV→MP4 tools)

1. **CloudConvert** — server-side converter with granular controls: target
   video codec, resolution, quality, and output file size. General-purpose (many
   formats), not remux-specialized; re-encodes.
2. **FreeConvert** — browser-front / server-back converter; free, files
   auto-deleted, SSL. Basic quality/codec options.
3. **XConvert** — explicitly markets the "H.264/AAC MOV = container remux, no
   re-encode, zero quality loss" story; handles iPhone HEVC/H.264, 4K60,
   Cinematic, ProRes (transcoded).
4. **Clipy** — local/in-browser remux: `-c copy` stream-copy of H.264 video +
   AAC audio into MP4 with `+faststart`; near-instant, lossless.
5. **Flonnect** — in-browser WebAssembly container conversion (MP4/MKV/WebM/MOV)
   with no re-encode and no upload.

## Table-stakes → in-model / out-of-model

| Capability | Decision | Where |
|---|---|---|
| Lossless stream-copy remux when codecs are MP4-legal (H.264/HEVC + AAC) | **in-model** | `mode=copy` (default) → `-c copy` |
| Transcode fallback to H.264/AAC for non-MP4-legal codecs (ProRes etc.) | **in-model** | `mode=transcode` → `-c:v libx264 -c:a aac` |
| `+faststart` (moov atom up front) for progressive web playback | **in-model** | both argv paths set `-movflags +faststart` |
| Runs locally / nothing uploaded | **in-model** | ffmpeg-wasm on the page; no upload |
| HEVC/H.265 input | **in-model** | copy handles HEVC (it's just a packet copy) |
| Quality control for the re-encode path | **in-model** | `quality` slider 1–100 → CRF 18–40 |
| Free, no watermark | **in-model** | always free, no watermark |
| One-click presets | **in-model** | 3 `[[example]]` chips (remux / re-encode hi-q / re-encode small) |
| Friendly labels on the mode chooser | **in-model** | `[input.labels]` on the mode `<select>` |
| Resolution / scaling on output | **out-of-model** | use the existing `video-resize` tool (only affects the transcode path; keeps this tool remux-focused) |
| Target output file size | **out-of-model** | use `video-target-filesize-encoder` |
| Trim/cut before converting | **out-of-model** | use `video-trim` |
| Batch / multi-file conversion | **out-of-model** | the page is single-file; multi-input ffmpeg is unsupported here (see create-next-tool references/page-patterns.md) |
| WebM/MKV output targets | **out-of-model** | this tool is MOV→MP4 only; `video-transcode` covers MP4↔WebM |

## Design decisions

- **Distinct from `video-transcode` / `video-compress`**, which ALWAYS re-encode
  with libx264 (lossy, slow). The headline here is the lossless `-c copy` remux,
  which none of the existing gizza video blocks offer. Not a duplicate.
- **Default = copy** so the common iPhone/camera case (H.264/HEVC + AAC) is an
  instant, lossless container swap. `transcode` is the explicit fallback the FAQ
  points users to when a ProRes MOV won't remux.
- **Quality maps to CRF 18–40, never CRF 0.** A spike showed CRF 0 (true
  lossless) produces ~11 MiB from a 13 s 640×360 clip, overflowing the 10 MiB
  output cap and crashing the wasm guest during the read-back. CRF 18 ("visually
  lossless") for the same clip is 0.8 MiB, so `quality=100` now succeeds. This is
  what real converters expose — a lossless-re-encode slider extreme is a footgun,
  not a feature.

## Spike results (ffmpeg, real files)

- H.264+AAC MOV `-c copy` → MP4: lossless, codecs + duration preserved, instant.
- ProRes MOV `-c copy` → MP4: **fails** ("Invalid argument") — needs transcode.
- H.264+PCM MOV `-c copy` → MP4: succeeds on modern ffmpeg (PCM copy is tolerated).
- Verified across surfaces: CLI (copy + transcode, quality 1/75/90/100, plus 101
  rejected and bad-mode rejected) and the page (Playwright decodes the output and
  asserts container=mp4, 128×128 dims, and preserved duration; copy remux,
  transcode deep-link, and an MP4 secondary-format input).
