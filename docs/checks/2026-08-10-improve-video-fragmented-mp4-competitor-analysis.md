# video-fragmented-mp4 — competitor analysis (2026-08-10)

Scan run **before** implementation, per `create-next-tool` step 4. Everything below is a
paraphrase of publicly documented behaviour; no competitor copy, branding, or trademarked
wording is reproduced or reused in the tool's page.

**Tool function:** take a standard (progressive) MP4 and rewrite it as a *fragmented* MP4
(fMP4) — a small `moov` initialization header followed by a chain of `moof`+`mdat` fragments —
so that Media Source Extensions players, DASH/CMAF/HLS-fMP4 packagers, and byte-range
streaming can consume it.

## Competitors reviewed

### 1. FFmpeg `mov,mp4,ismv` muxer (official muxer documentation)

The reference implementation everybody else is compared against. Relevant surface:

| Option | Documented behaviour |
| --- | --- |
| `-movflags empty_moov` | Delay the initial `moov` until the first fragment is cut — i.e. emit an initialization-only `moov`. |
| `-movflags frag_keyframe` | Start a new fragment at each video keyframe. |
| `-movflags frag_every_frame` | Cut a fragment at every frame. |
| `-movflags default_base_moof` | Omit the absolute `base_data_offset` in `tfhd`, using the default-base-is-moof flag instead (easier/robuster parsing). |
| `-movflags dash` | Emit DASH-compatible fragmented output. |
| `-movflags cmaf` | Emit CMAF-compatible fragmented output. |
| `-movflags negative_cts_offsets` | `ctts` version 1 with negative CTS offsets (DASH-IF guidance). |
| `-movflags global_sidx` | Write a global segment index (`sidx`) at the start of the file. |
| `-movflags skip_sidx` | Skip writing `sidx` boxes. |
| `-movflags separate_moof` | One `moof` per track rather than one combined `moof`. |
| `-movflags faststart` | Second pass that moves the index (`moov`) to the front. |
| `-frag_duration <µs>` | Fragments of the given duration (microseconds). |
| `-min_frag_duration <µs>` | Never cut a fragment shorter than this (microseconds). |
| `-frag_size <bytes>` | Cut fragments at a payload byte budget. |
| `-movie_timescale <n>` | `mvhd` timescale; default 1000. |

Community/how-to consensus for an MSE-playable single file is the combination
`frag_keyframe + empty_moov + default_base_moof`, with `dash`/`cmaf` added for those
packaging targets.

### 2. Bento4 `mp4fragment` (Axiomatic Systems CLI)

The dedicated "make this MP4 fragmented" utility, not a general transcoder.

- `--fragment-duration <ms>` — target fragment duration in **milliseconds**; documented
  default **2000 ms** (2 s).
- `--index` — (re)create the segment index (`sidx`) after fragmenting.
- `--force-i-frame-sync <auto|all>` — treat I-frames as sync samples; `auto` only when an
  open-GOP source is detected, `all` unconditionally.
- Track selection / verbosity flags for picking which tracks are carried over.
- Model: **stream copy only** — it re-packages an existing MP4, it never re-encodes.

### 3. GPAC `MP4Box` fragmentation (`-frag`, `-rap`, `-frag-rap`)

- `-frag <ms>` — produce track fragments of roughly the given duration in milliseconds;
  documented default fragment duration **500 ms** when unspecified (outside DASH mode, where
  it inherits the DASH segment duration).
- `-rap` — cut so that segments start at a random-access point (IDR/I-frame).
- `-frag-rap` — force every fragment to begin at a random-access point; notes that the actual
  fragment duration then deviates from the requested one because the encoded video is not
  modified.
- Also stream-copy oriented; re-encoding is a separate step.

### 4 (context). HLS-fMP4 how-to guides

Segmenting workflows (`-hls_segment_type fmp4`) that emit an `init.mp4` plus `.m4s` segments,
typically pairing a 2 s forced-keyframe cadence (`-force_key_frames "expr:gte(t,n_forced*2)"`)
with 6 s segments. Useful mainly as evidence that **keyframe cadence control** is a table
stake next to fragment duration — but the multi-file output shape itself is out of model here
(see below).

## Table stakes → decisions

| Table stake (seen in ≥1 competitor) | In model? | Decision |
| --- | --- | --- |
| Fragmented output for MSE: `empty_moov` + `frag_keyframe` + `default_base_moof` | ✅ | **Always applied.** This is the tool. |
| Stream copy by default (no re-encode, no quality loss) | ✅ | **`mode = copy` is the default** (`-map 0 -c copy`). |
| Re-encode fallback for sources whose codecs a browser/MSE can't play | ✅ | `mode = h264` → `-c:v libx264 -pix_fmt yuv420p -crf 23 -c:a aac -b:a 128k`. |
| Target fragment duration (Bento4 2000 ms, MP4Box 500 ms) | ✅ | `fragment_duration` in **seconds**, default `0` = one fragment per keyframe. Implemented as `-min_frag_duration <µs>` **together with** `frag_keyframe`, which is exactly MP4Box's `-frag-rap` semantics: fragments stay keyframe-aligned and are never shorter than the requested target. Verified on real ffmpeg 7.1.4. |
| Keyframe-aligned fragments / force I-frame sync (`-rap`, `--force-i-frame-sync`) | ✅ | `frag_keyframe` is always on, so every fragment starts at a keyframe. In `h264` mode `keyframe_interval` additionally forces a regular cadence via `-force_key_frames expr:gte(t,n_forced*N)` — the fix for sources with sparse/irregular keyframes, where copy-mode fragments would otherwise be huge and unevenly spaced. |
| Segment index (`sidx`) — Bento4 `--index`, ffmpeg `global_sidx` | ✅ | `segment_index` boolean → `+global_sidx`. Off by default (a global `sidx` needs a seekable second pass and is only useful for byte-range/single-file DASH). Verified: adds two `sidx` boxes ahead of the first `moof`. |
| DASH-compatible output profile | ✅ | `profile = dash` → adds `+dash+negative_cts_offsets`. |
| CMAF-compatible output profile | ✅ | `profile = cmaf` → adds `+cmaf+negative_cts_offsets`. |
| `faststart` | ✅ but pointless | **Deliberately not exposed.** With `empty_moov` the `moov` is already an ~1 KB init header at the very front of the file, so the extra `faststart` pass has nothing to relocate. It muxes fine (verified: byte-identical size to the run without it) — it is simply a no-op cost, so exposing it would be a misleading knob. Documented on the page FAQ instead. |
| `frag_every_frame` | ✅ but harmful | **Not exposed.** Verified it produces a valid file, but on a 6 s test clip it inflated the output from 68 KB → 106 KB (+56 %) because every frame carries a `moof`. It exists for ultra-low-latency live pipelines, not for the "convert a file for a player" job this tool does. Listed here rather than shipped. |
| Multi-file segmenting: `init.mp4` + numbered `.m4s`, HLS/DASH manifests | ❌ out of model | The gizza ffmpeg dispatch returns exactly **one** output file (`block-utils` `ExecArgs.output` is a single read-back filename) and the page renders a single media result — there is no zip/multi-file output format. Same constraint that skiplisted `video-chapter-splitter`. A single fragmented MP4 is the single-file equivalent, and is what MSE `SourceBuffer.appendBuffer` wants anyway. |
| Track selection / per-track fragmenting (Bento4, MP4Box) | ❌ out of scope | The tool carries **all** tracks (`-map 0`). Choosing tracks is `video-audio-track-selector`'s job; adding a track picker here would duplicate it. |
| CENC/DRM encryption of fragments (Bento4 `mp4encrypt`, GPAC CENC) | ❌ out of model | Needs key management and a DRM system; no wasm-safe path and out of scope for a format-conversion tool. |
| Inspecting the result (`mp4ff-info`, `mp4dump`) | ❌ separate tool | `media-info` already reports container/codec/track structure. |
| `frag_size` (byte-budgeted fragments), `separate_moof`, `movie_timescale` | ❌ not shipped | Niche packager-internal knobs; no consumer-facing competitor exposes them as a primary control, and each adds a failure mode without a matching user story. Listed, not built. |

## UX patterns competitors ship (and what this page does)

- CLI tools express fragment duration in **milliseconds**; a browser tool reads better in
  **seconds** — the page uses seconds and says so in the field label, describe text, and FAQ.
- Both dedicated fragmenters ship a **default fragment duration** rather than "auto". Here the
  default is `0` = "one fragment per keyframe", which is the honest ffmpeg default and avoids
  silently re-cutting a file that is already well-GOP'd; the 2 s Bento4-style behaviour is one
  preset chip away.
- Packaging tools expose the target ecosystem as a **profile** rather than raw flags; the page
  mirrors that with an `mse | dash | cmaf` select with friendly labels, not a raw `movflags`
  text box.
- Preset buttons: competitors' docs are organised around a handful of recipes (MSE playback,
  DASH packaging, 2 s fragments). The page ships `[[example]]` chips for those recipes.
- `fragment_duration` gets a slider (bounded 0–30 s range), matching the slider-for-bounded-numeric
  convention already used across this repo's video pages.

## Out-of-model list (stated, not built)

Multi-file segment output + HLS/DASH manifest generation; per-track selection and per-track
fragmenting; CENC/DRM fragment encryption; byte-budgeted fragments (`frag_size`);
`separate_moof`; `movie_timescale`; `frag_every_frame` (harmful for this use case);
`faststart` (no-op with `empty_moov`); fMP4 → progressive-MP4 defragmentation (the inverse
direction — a separate tool, and `video-duration-fix-remux` already covers the general
"remux and rebuild the index" need).

## Sources

- [FFmpeg formats documentation — mov/mp4/ismv muxer](https://ffmpeg.org/ffmpeg-formats.html)
- [Bento4 mp4fragment documentation](https://www.bento4.com/documentation/mp4fragment/)
- [GPAC MP4Box fragmentation wiki](https://wiki.gpac.io/Howtos/dash/Fragmentation,-segmentation,-splitting-and-interleaving/)
- [GPAC MP4Box general options](https://wiki.gpac.io/MP4Box/mp4box-gen-opts/)
- [HLS and fragmented MP4 how-to](https://hlsbook.net/hls-fragmented-mp4/)
- [Use fragmented MP4 — web A/V best practices](https://danabrams.gitbook.io/av-best-practices/use-fragmented-mp4)
