# seamless-loop-video — competitor analysis (2026-07-24)

Paraphrased scan only — no competitor copy, branding, or trademarks reproduced.

## Function

Make a video clip loop perfectly: crossfade (overlap) the clip's tail back into its
head so the loop join is invisible. Output is one slightly-shorter clip whose end frame
matches its start frame, so repeating it reads as continuous motion.

## Competitors skimmed (top real tools for "seamless loop video / crossfade")

1. **LiveLink AI — Infinite Loop Video.** Upload a file or paste a URL; explicitly offers
   a *crossfade* to smooth/hide the loop transition; notes crossfade works best when the
   start and end frames are already somewhat similar. Also an AI "infinite extension" path.
2. **HeyGen — Loop Video.** Timeline loop selector to pick the exact portion to repeat;
   markets "clean, professional loop"; browser preview + download, no complex software.
3. **EchoWave — Loop Video.** Stitches each repeat with "no gap, freeze or jump" when the
   clip already starts/ends on the same shot; free, no watermark.
4. **Flixier — Loop Video Online.** Precise timeline to cut a perfect loop; import by
   YouTube link; repeat count.
5. **Clideo / tech-lagoon (DataChef) — Seamless Loop.** Simple upload → seamless loop;
   basic controls.

## Table-stakes → in-model / out-of-model

| Capability | Decision |
|---|---|
| Upload by file **and** URL | **in-model** — descriptor `Input::Video` = `url` ⊕ `ref`. |
| Crossfade the tail into the head to hide the join (core feature) | **in-model** — implemented; the whole tool. |
| Adjustable crossfade / overlap duration | **in-model** — `crossfade` seconds param (slider). |
| No watermark | **in-model** — gizza never watermarks. |
| MP4 output + in-page preview & download | **in-model** — H.264/yuv420p `.mp4`, `format="video"`. |
| Preset crossfade lengths | **in-model** — `[[example]]` preset chips (subtle / standard / long). |
| Interactive timeline loop-point selection (HeyGen, Flixier) | **out-of-model** — needs an in-page video editor/timeline; our declarative page has no scrubber. Users pre-trim the clip to the loop region with the existing `video-trim` tool, then run this. |
| Repeat N times / infinite output length (most tools) | **out-of-scope here** — that is the existing `loop-video` tool (`-stream_loop`); this tool makes ONE seamlessly-loopable clip. Chain the two to get a long seamless loop. |
| YouTube-link import (Flixier) | **out-of-model** — we fetch direct public http(s) media URLs, not scraped YouTube pages. |
| AI infinite-extension / generative loop (LiveLink AI) | **out-of-model** — needs an ML model; gizza is pure Rust + ffmpeg. |
| Crossfade the AUDIO at the loop point | **considered, not built (limitation)** — in a single-pass ffmpeg argv with no probe step we cannot detect whether the input even has an audio track, and referencing a missing `[0:a]` hard-fails the graph. Output is therefore **silent** (ideal for muted background loops). Stated on the page. |

## Implementation notes (feasibility spike done before deciding)

- ffmpeg 7.1 `xfade` gives many transitions but **requires constant frame rate**; the
  `trim`+`setpts` branches it needs are VFR, so `xfade` failed graph-init here
  ("inputs needs to be a constant frame rate"). Rejected.
- Chosen graph is a straight **alpha crossfade via `overlay`** — robust, no CFR
  requirement, and the standard "crossfade" every competitor advertises for seamless loops:
  ```
  [0:v]split[s1][s2];
  [s1]reverse,trim=start=X,setpts=PTS-STARTPTS,reverse[base];      # source[0, D-X]
  [s2]reverse,trim=end=X,setpts=PTS-STARTPTS,reverse,              # tail source[D-X, D]
      format=yuva420p,fade=t=out:st=0:d=X:alpha=1[tail];           # tail fades out over first X
  [base][tail]overlay=eof_action=pass[v]                           # first X = crossfade tail→head
  ```
  Output length = `D - X`; its first frame equals the source frame at `D-X` (verified: PSNR
  ~45 dB between output frame 0 and the source at the loop point), so the loop join is
  invisible.
- **Probe-free** by design: the block never learns the clip duration `D`. `reverse` +
  front/back `trim` locate the clip end (drop the last / first `X` seconds) without needing
  `D`, so it works identically on the standalone page (@ffmpeg/core) and the CLI. Cost: the
  clip is buffered to reverse it, so this suits short clips (a few seconds); very long or
  high-resolution inputs may exhaust browser memory — stated on the page.
