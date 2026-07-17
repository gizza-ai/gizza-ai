# video-strip-metadata — competitor analysis (2026-07-17)

Scan done BEFORE implementing. Goal: remove embedded metadata (GPS/location,
device make/model, timestamps, editing-software traces, title/artist/comment/
copyright) from a video by remuxing it clean — stream-copy, no re-encode.

## Competitors skimmed (top 3 real tools)

1. **Fylite — Strip Video Metadata** (`fylite.com/en/tools/video-strip-metadata/`)
2. **MetaClean — Video Metadata Remover** (`metaclean.app/video-metadata`)
3. **Flonnect — Remove Video Metadata** (`flonnect.com/media-tools/remove-metadata`)
   (plus search-snippet corroboration from remove-metadata.org, metaremove.com —
   remove-metadata.org returned HTTP 403 to direct fetch, replaced by MetaClean.)

## Table-stakes findings (paraphrased — no copy reused)

| Capability | Fylite | MetaClean | Flonnect | In/out of model | Decision |
|---|---|---|---|---|---|
| Input MP4 | ✅ | ✅ | ✅ | in | supported |
| Input MOV | ✅ | ✅ | ✅ | in | supported |
| Input WebM | ✅ | — | ✅ | in | supported (keep container) |
| Input MKV | ✅ | — | ✅ | in | supported (keep container) |
| Input AVI | ✅ (via mp4 out) | — | — | in | best-effort (keep container) |
| Stream-copy, no re-encode / no quality loss | ✅ | ✅ | ✅ | in | **default** (`-c copy`) |
| Strip GPS/location | ✅ | ✅ | ✅ | in | done via `-map_metadata -1` |
| Strip device make/model/serial | ✅ | ✅ | ✅ | in | done via `-map_metadata -1` |
| Strip creation/modification timestamps | ✅ | ✅ | ✅ | in | done (global + per-stream) |
| Strip editing-software / encoder tags | — | ✅ | — | in | done (`-bitexact` drops muxer `encoder` tag) |
| Strip chapters | — | — | — | in | **added** — `chapters` enum (remove default / keep) |
| Force standard MP4 output | ✅ (always MP4) | — | — | in | **added** — `container` enum (keep default / mp4) |
| 100% in-browser, no upload | ✅ | ✅ | ✅ | in | matches (page runs ffmpeg.wasm locally) |
| "View detected metadata before processing" | — | ✅ | — | out | out-of-model — needs a metadata-reader UI; use the `image-metadata-viewer`/probe family instead |
| Batch (many files at once, up to 50) | ✅ | ✅ (50) | ✅ | out | out-of-model — page is a single-file upload; run repeatedly / CLI-script |
| No configurable settings ("just click Process") | ✅ | ✅ | ✅ | — | we go slightly beyond: two robust, optional enum controls |

## UX control patterns observed
- All three are single-action ("upload → process → download"); essentially no knobs.
- MetaClean shows the detected metadata pre-strip (out-of-model here — no probe UI).
- MetaClean/Fylite advertise stream-copy / "no quality loss" prominently → we mirror
  that message (original wording, not copied).

## Design decisions (in-model → shipped in the descriptor)
- **`container`** enum `keep` (default) | `mp4`. `keep` = same container, always lossless
  and universal; `mp4` remuxes into a standard `.mp4` (matches Fylite's always-MP4 output).
  Documented limitation: `mp4` stream-copy requires MP4-compatible codecs (H.264/H.265 +
  AAC); a WebM/VP9 input with `container=mp4` errors — surfaced clearly, use `keep`.
- **`chapters`** enum `remove` (default) | `keep`. Chapter markers are container metadata a
  privacy-minded user usually wants gone; `keep` preserves them.
- Always: `-map 0` (all streams: video+audio+subs kept), `-map_metadata -1`
  (global + per-stream via `:s`), `-bitexact` (drop the muxer's own `encoder`/version tag),
  `-c copy` (lossless). Output keeps the input filename stem.

## Out-of-model (considered, NOT built)
- Pre-strip metadata inspection/preview UI (probe display).
- Batch/multi-file processing (page is single-upload; ffmpeg can't run in chat SW).
- Selective per-field editing (rewrite artist/title) — this tool removes, does not edit.
