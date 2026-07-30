# mp4-to-mov — competitor analysis (2026-07-30)

Tool: **mp4-to-mov** — repackages an MP4 (ISO-BMFF) into a QuickTime `.mov`
container by lossless stream-copy (`ffmpeg -i in.mp4 -map 0 -c copy -movflags
+faststart out.mov`) for Final Cut Pro / iMovie / QuickTime Player workflows. No
re-encode, no params. Runs fully client-side (ffmpeg-wasm on the page; ffmpeg
runtime for chat/CLI).

## Competitor scan

Surveyed the top general-web results for "convert MP4 to MOV online" (paraphrased
observations only — no copy, branding, or wording reproduced):

1. **CloudConvert** — server-side conversion service. Broad format matrix, exposes
   codec/quality/resolution options, and (crucially) actually *re-encodes* by
   default. Requires upload; free tier is minute-capped. Strength: options depth.
   Gap vs us: files leave the device; slower; not a pure lossless remux.

2. **FreeConvert** — server-side, upload-based. Offers "advanced settings" (codec,
   CR-style quality, trim). Larger size ceiling than ours behind sign-in. Same
   privacy/upload trade-off; conversion is a re-encode, not a copy remux.

3. **Clideo** — browser-front UI but server-processed; oriented at casual users.
   Watermark/upsell on the free tier for some flows. No lossless-remux guarantee;
   uploads required.

4. **Cloudinary MP4-to-MOV tool** — marketing front-end for a media API. Server
   pipeline, developer-oriented. Re-encodes and re-hosts. No local processing.

5. **EchoWave / VEED / HappyScribe** — creator-focused SaaS. "No signup / no
   watermark" messaging; all upload-and-process server-side, and bundle editing
   upsells. None advertise a bit-for-bit stream-copy; output is a re-encode.

Also noted a couple of pure client-side ffmpeg-wasm demos (file-converter-online
style) — closest in mechanism to us, but generic single-purpose pages with no
worked example, no track-preservation explanation, and no chat/CLI surface.

## Disposition — gaps and decisions

| Observed competitor feature | In our model? | Decision |
|---|---|---|
| Client-side, no upload | Yes (already our core differentiator) | Keep + emphasize in copy. |
| Lossless stream-copy remux (`-c copy`) | Yes | Keep; competitors mostly re-encode — this is our edge. Documented. |
| `+faststart` for progressive playback | Yes | Keep; already in argv + explained on page. |
| Keeps *all* audio/data tracks (`-map 0`) | Yes | Keep; added an FCP/iMovie + "keeps all audio tracks" FAQ. |
| Re-encode to ProRes / codec+quality options | **No** (out of model) | Do NOT build. Point users to `video-transcode` / `video-compress` in copy + FAQ. |
| Trim / resolution / batch settings | **No** (out of model, separate tools) | Out of scope; separate gizza tools cover these. |
| Larger file ceilings (behind sign-in) | Partially — 10 MiB cap | Documented as a limit; matches other gizza ffmpeg tools. |
| Watermark / signup upsell | N/A | We have none — implicit advantage, not copied messaging. |

### Actions taken this pass

- Filled `page/meta.toml` (generic title/description/tags, `video` format,
  `video/mp4,...` accept, `MOV video` output label, `ffmpeg` runtime).
- Wrote `page/content.md`: how-it-works (container superset + `-map 0 -c copy
  -movflags +faststart`), "why MOV" (Apple/FCP workflows), a worked example, an
  explicit limits section, and 5 real `<details>` FAQs (quality, FCP/iMovie
  compatibility incl. the ProRes caveat, speed, audio-track preservation,
  privacy).
- Synced `manifest.json` / `wafer.toml` summary from the descriptor.

### Not done (deliberately, out of model)

- No re-encode / ProRes / codec-quality controls — that is `video-transcode`'s
  job; adding it would duplicate an existing tool and break the "lossless remux"
  contract.
- No server-side large-file path — client-side + 10 MiB cap is the repo-wide
  ffmpeg-tool shape and the privacy guarantee.

### Distinctness (vs existing gizza tools)

Confirmed distinct: `mp4-to-mkv` (Matroska target), `mov-to-mp4` (reverse
direction), `mkv-to-mp4`, and `video-transcode` (re-encode). mp4-to-mov is the
only MP4→QuickTime lossless remux. Not a duplicate — shipped.
