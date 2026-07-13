# audio-metadata-stripper — competitor analysis (2026-07-13)

Scan of the top browser/offline "remove audio metadata / strip ID3 tags" tools.
Paraphrased only; no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **removemd.com — remove audio metadata** — strips ID3 tags, Vorbis comments
   and embedded cover art from MP3/FLAC/OGG/WAV/AAC; processes in-memory,
   nothing stored server-side. Framed around privacy (nothing uploaded).
2. **metadataview.com — remove audio metadata** — strips ID3, Vorbis comments,
   ASF/RIFF tags, cover art and custom fields across MP3/FLAC/OGG/OPUS/M4A/
   AAC/WAV/WMA **without re-encoding** (bit-identical audio). Also shows the
   tags before removal.
3. **mp3conv.com — metadata stripper** — drops every ID3 tag, cover image and
   embedded metadata frame; the audio bits are copied through unchanged so the
   sound is bit-identical with zero metadata. Free, in-browser.
   (nofileupload.com is a near-identical fourth data point: strip all ID3 tags,
   encoder identity, cover art, comments and hidden frames; audio preserved
   bit-perfectly, no upload.)

## Table-stakes params / behaviours

| Feature | In model? | How |
| --- | --- | --- |
| Remove ALL tags (ID3v1/v2, Vorbis comments, ASF/RIFF, chapters) | ✅ in-model | `-map_metadata -1 -map_chapters -1` |
| Remove embedded cover art / album art | ✅ in-model | drop the attached-picture stream (`-map 0:a`) |
| **Option to keep cover art** while stripping text tags | ✅ in-model | `keep_cover_art` param → `-map 0` keeps the picture stream |
| Bit-perfect audio (no re-encode) | ✅ in-model | `-c copy` (stream copy) |
| Preserve original format/container | ✅ in-model | output keeps the input extension + codec |
| Multi-format input (mp3, flac, ogg, m4a, wav, …) | ✅ in-model | any container ffmpeg can demux/stream-copy |
| Runs locally / nothing uploaded (page) | ✅ in-model | ffmpeg.wasm in the browser tab |
| Suppress ffmpeg's own muxer `encoder` tag | ✅ in-model | `-bitexact` output flag |

## Out-of-model (listed, NOT built)

- **View/preview the existing tags before stripping** — a read-only metadata
  *reader* is a separate tool surface; this tool is a one-shot stripper. (Image
  metadata already has `metadata-privacy-linter` / `image-metadata-viewer`.)
- **Selective field editing / re-tagging** (change artist, keep album, etc.) —
  a tag *editor*, not a stripper; different UX and scope.

## Defaults chosen

- `keep_cover_art` defaults to **false** — the privacy-first default strips the
  cover image too (matches the description "strips all tags, cover art, and
  metadata"). Users who only want text tags gone flip it on.
- Stream copy always (bit-identical audio); no bitrate/quality knobs because
  nothing is re-encoded.

## UX control patterns adopted

- Single boolean toggle (`keep_cover_art`) — mirrors the "keep album art"
  checkbox competitors expose, rendered as a page checkbox.
- Output filename keeps the original stem + extension (`song.mp3` → `song.mp3`,
  clean of metadata).
