# video-set-keyframe-interval — competitor analysis (2026-08-12)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased; no competitor copy, branding, or trademark is reproduced.

## What the tool does

Re-encodes a video with a **fixed keyframe (GOP) cadence** — e.g. one keyframe every 2 seconds —
so players can seek cleanly and packagers can cut evenly-sized HLS/DASH segments on keyframe
boundaries. Output is a normal progressive MP4.

## Duplicate check (done first)

| Existing block | Overlap | Verdict |
|---|---|---|
| `video-fragmented-mp4` | Has a `keyframe_interval` knob, but **always** writes a *fragmented* MP4 (`+frag_keyframe+empty_moov+default_base_moof` are unconditional in `BASE_MOVFLAGS`), and the knob only applies in its `h264` mode as a side effect of even fragment spacing. It cannot produce a plain progressive MP4 with a fixed GOP. | Not a dup — different output artifact |
| `video-compress`, `video-transcode`, `video-to-h264`, `video-target-filesize-encoder`, `video-fps` | Grepped `core/src/lib.rs` of each: **zero** occurrences of `-g`, `keyint`, `sc_threshold`, `force_key_frames`. None exposes any GOP control. | Not a dup |
| `video-cut-segments`, `video-trim` | Consume existing keyframes (seek accuracy); never set the cadence. | Not a dup |

Skiplist grep for `keyframe|gop|segment|hls` surfaced only unrelated entries
(`video-trim-segment`, `video-split-into-segments` — a multi-output splitter, a different shape).

## Competitors surveyed

1. **HandBrake** (desktop encoder) — has no first-class GOP field; users type x264 parameters into an
   "extra options" box (`keyint=…:min-keyint=…`). Its x264 defaults are closed-GOP. Takeaway: a
   dedicated, discoverable control is the gap; frames-based `keyint` + a matching minimum is the
   canonical shape.
2. **FFmpeg-based streaming recipes (mpegflow HLS keyframe-tuning guide, plus general ffmpeg docs)** —
   the reference recipe is `-g N -keyint_min N -sc_threshold 0` with `N = fps × segment_seconds`,
   and it stresses that scene-cut detection must be disabled or it inserts extra keyframes and breaks
   segment alignment. It also notes that **variable-frame-rate sources need time-based keyframes**
   instead of a frame count.
3. **Cloud/CDN guidance (Cloudinary glossary, iOriver keyframe-interval reference)** — recommends
   ~2 s for streaming, 1–2 s for low latency, longer (5–10 s) for on-demand bandwidth savings, and
   closed GOPs where seeking / bitrate switching matters. Provides the seconds-oriented mental model
   (users think in seconds, encoders think in frames).

## Table stakes → decisions

| Capability | Competitors | In model? | Decision |
|---|---|---|---|
| Interval in **seconds** (2 s default) | Cloudinary/iOriver, OBS-style UIs | yes | `interval` + `unit="seconds"` default 2 → `-force_key_frames expr:gte(t,n_forced*N)` (fps-independent, correct for VFR too) |
| Interval in **frames** (GOP size) | HandBrake x264 `keyint`, ffmpeg `-g` | yes | `unit="frames"` → `-g N -keyint_min N` |
| Disable **scene-cut** keyframes | mpegflow recipe (`no-scenecut` / `-sc_threshold 0`) | yes | `scene_cut` boolean, default **off** (= strict cadence); on drops `-sc_threshold 0`/`-keyint_min` |
| **Closed GOP** | HandBrake note, Cloudinary guidance | yes | `closed_gop` boolean, default on → `-flags +cgop` |
| Quality / CRF | every encoder | yes | `quality` 1–100 → CRF 40…18 |
| Encoder speed preset | HandBrake, ffmpeg | yes | `preset` enum ultrafast/veryfast/medium/slow |
| Streaming-ready container | HLS guides | yes | always `-movflags +faststart`, `yuv420p`, AAC audio (baked in, documented) |
| Presets for common cadences | OBS/streaming presets | yes | `[[example]]` chips: 2 s streaming, 1 s low-latency, 30-frame editing, 10 s archive |
| Seconds↔frames helper (needs source fps) | encoder GUIs show it | **no** | Out of model: the argv is built without probing the input, so we can't display "2 s = 60 frames" for a given clip. Mitigated by making seconds mode fps-independent (`-force_key_frames`), which is the correct answer anyway. |
| HLS/DASH **segment** output (`.m3u8` + N chunks) | packagers | **no** | Out of model: gizza ffmpeg dispatch is single-output (`ExecResult.output` is one `Vec<u8>`). `video-fragmented-mp4` covers the single-file streaming container. |
| HEVC / VP9 / AV1 encoders with GOP control | cloud encoders | **no** (scope) | Listed, not built: this block pins libx264 so the cadence flags have one well-tested meaning; `video-to-hevc`/`video-transcode` own codec switching. |
| Per-scene / manual keyframe list | pro NLEs | **no** | Listed, not built — no UI surface for a timestamp list on a single-field page form. |
| Hardware encoders (NVENC/QSV syntax) | HandBrake QSV | **no** | Not available in the wasm/browser ffmpeg build. |

## Notes / limits stated on the page

- Always re-encodes (a fixed cadence cannot be applied by stream copy).
- Output is a progressive MP4 with `+faststart`; for a fragmented MP4 use `video-fragmented-mp4`.
- Input cap 50 MiB, output cap 60 MiB.
- Seconds mode places keyframes by timestamp (safe for variable-frame-rate sources); frames mode is
  exact for constant-frame-rate sources.
