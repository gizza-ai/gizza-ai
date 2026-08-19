# video-scene-split — competitor scan + design decisions (2026-08-14)

Scan run BEFORE implementing. All notes are paraphrased from public documentation;
no competitor copy, branding or trademarks are reproduced anywhere in the tool.

## Duplicate check

`ls blocks/ | grep -iE 'scene|split|video|cut'` surfaced three neighbours, all
confirmed distinct by reading their `core/src/lib.rs`:

- **`video-scene-cut-diff`** — detects scene cuts in **two** edits of the same
  footage and reports added/removed/moved cuts. Detection-only; it never produces
  clips. Shares the detector, not the deliverable.
- **`video-cut-segments`** — cuts a video at **manually typed** `start-end`
  windows. No detection.
- **`video-silence-cut`** — cuts on **audio** silence, not visual shot changes.

Not a duplicate: this tool is the only one that detects visual shot boundaries in
one video and emits per-scene clips.

## Competitors reviewed

1. **PySceneDetect** (`scenedetect.com` — CLI reference + video-splitter API docs).
   The de-facto reference implementation: `detect-content` (default threshold 27 on
   a 0–255 content scale), `detect-adaptive`, `detect-threshold` (default 12, fade
   detection), a `min-scene-len` de-bounce, then `split-video` (default
   `-map 0 -c:v libx264 -preset veryfast -crf 22 -c:a aac`, output template
   `$VIDEO_NAME-Scene-$SCENE_NUMBER.mp4`, `-c/--copy` for a lossless remux),
   `list-scenes` (CSV export), `save-images` (thumbnails), and global
   `--downscale` / `--frame-skip` performance knobs plus a `time` sub-command for
   start/end/duration limits.
2. **ffmpeg scene-detection guide** (ffmpeglab.com). Contrasts
   `select='gt(scene,X)'` (0–1 score, 0.3–0.5 recommended) with the newer `scdet`
   filter (MAFD percentage, 8–14 recommended); shows extracting cut timestamps via
   `showinfo`/`metadata=print`, and splitting with either the `segment` muxer or
   per-clip `-ss`/`-to`. States plainly that keyframe-accurate splitting requires
   re-encoding.
3. **achesco detect-and-split gist** (github.com). A minimal shell pipeline:
   `select='gt(scene,$diff)',showinfo` with a 0.4 default, timestamps grepped out
   of the `pts_time:` fields, then one `-ss … -t …` libx264 re-encode per segment,
   with an optional `-an` to strip audio.

(SplitClips, the fourth hit, is a paid desktop app with no public parameter
documentation — nothing verifiable to compare against, so it was dropped.)

## Table stakes → where each one landed

| Table stake | Source | Decision |
| --- | --- | --- |
| Detection sensitivity | all three | **In-model** → `threshold` (0.0–1.0, default 0.3). ffmpeg's own scene score, so the number matches the sibling `video-scene-cut-diff` block on the same footage. |
| Minimum scene length | PySceneDetect `min-scene-len` | **In-model** → `min_scene` (seconds, default 0.6 — PySceneDetect parity). Also folds a too-short *final* scene back, which PySceneDetect does not. |
| Lossless copy vs re-encode | PySceneDetect `-c/--copy`, ffmpeg guide | **In-model** → `mode` enum `reencode` (default) / `copy`, with the keyframe-snapping caveat spelled out on the page and in the descriptor. |
| Encode quality | PySceneDetect `-crf` (22) | **In-model** → `crf` (0–51, default 22, same default). |
| Encode speed preset | PySceneDetect `-p/--preset` (veryfast) | **In-model** → `preset` enum, default `veryfast` (same default). |
| Strip audio | gist `-an` | **In-model** → `keep_audio` boolean, default true. |
| Scene-list CSV export | PySceneDetect `list-scenes` | **In-model, always on** → `scenes.csv` (`scene,start_seconds,end_seconds,duration_seconds,filename`) ships inside the ZIP and as its own download on the page. No extra parameter needed. |
| `$VIDEO_NAME-Scene-$NUMBER` naming | PySceneDetect | **In-model, automatic** → clips are `<source-stem>-Scene-001.<ext>` (stem sanitized to `[A-Za-z0-9._-]`, 40 chars). |

## Deliberately NOT built

- **Alternative detectors** (`detect-adaptive`, `detect-threshold` fade detection,
  `detect-hash`, `detect-hist`, and ffmpeg's `scdet`). Spiked `scdet` — it works in
  native ffmpeg but exposes a second, differently-scaled threshold (MAFD percent)
  that would need its own units, docs and defaults. One well-documented sensitivity
  scale beats two half-documented ones; revisit if users report missing soft cuts
  that a lower `threshold` can't catch.
- **`save-images` scene thumbnails.** Feasible (one extra `-frames:v 1` pass per
  scene) but it is a different deliverable, and `blocks/video-frame-extract`
  already covers "grab a still at time T" — adding it here would double the pass
  count for every run.
- **`--downscale` / `--frame-skip`.** Performance knobs for hour-long sources. With
  a 25 MB / ~200-clip working envelope the detection pass is already the cheap half
  of the run, and both knobs trade detection accuracy for speed in ways that are
  hard to explain on a one-page UI.
- **`time --start/--end/--duration`.** Limiting the analysed range is exactly what
  `blocks/video-trim` does; chaining beats duplicating.
- **Statistics file (`-s/--stats`).** A cache format for repeated runs of a local
  CLI; meaningless for a stateless per-call tool.

## Model / surface notes

- The page cannot use the generic single-pass ffmpeg driver: this is detect →
  N extract passes → N outputs. `page/custom.js` takes over fully, in the same
  shape as `video-autocrop-bars`' two-pass takeover, previewing scene 1 in the
  player and listing every clip plus `scenes.csv` as downloads.
- Chat/CLI return ONE `application/zip` envelope (clips STORED, CSV deflated) —
  the same multi-output answer `collage-splitter` and `csv-group-split` use.
- ffmpeg cannot run in the chat Service Worker, so the supported surfaces are the
  standalone page and the CLI.

## Verification

Fixture: a 3 s, 128×128 clip of three 1 s solid-colour shots (red → lime → blue,
distinct luma) with an AAC tone and forced keyframes at 0/1/2 s, so both cut modes
have exact expected boundaries. Detected scene scores on it are 0.64 at 1 s and
1.00 at 2 s — comfortably either side of the 0.3 default and below a 0.7 "no cuts"
probe. Covered end to end: default 3-way split, `mode=copy`, non-default
`keep_audio=false`, the no-cuts message, a bad-threshold error, a `?param=`
deep-link, and the CLI's exact scene table.
