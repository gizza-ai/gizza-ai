# Competitor analysis — video-scene-cut-diff (2026-08-01)

Tool function: detect the scene-cut (shot-boundary) timestamps in TWO edits of the
same footage and report which cuts were ADDED, REMOVED, or MOVED between them — an
editorial diff of a rough vs fine cut, a before/after re-edit, or two renders. Edit 1
is the reference timeline; every verdict is relative to it. Detection and diff only —
the videos are never re-cut or rewritten.

## Landscape

### 1. FFmpeg `select='gt(scene,…)'` / `scdet`
- FFmpeg's `scene` expression and the newer `scdet` filter flag frames whose visual
  difference from the previous frame exceeds a threshold; piping through `showinfo`
  prints each flagged frame's `pts_time`.
- This gives raw per-file cut timestamps but nothing more: there is no built-in way to
  DIFF two edits — an operator runs the command twice and reconciles the two timestamp
  lists by hand.
- This tool builds directly on that detector (same `select`/`showinfo` mechanism) and
  adds the pairwise diff/classification that FFmpeg does not provide.

### 2. Shot-detection libraries (PySceneDetect and similar)
- Content/threshold shot detectors segment a single video into a scene list and can
  export CSV/EDL/timestamps, with tunable detector sensitivity and a minimum-scene-
  length guard so one cut is not double-counted.
- They are single-video segmenters focused on splitting or listing scenes; comparing
  two edits and labelling added/removed/moved cuts is left to the user.
- Table-stakes borrowed as ideas (not copy): a sensitivity threshold and a
  minimum-scene-length de-bounce — both exposed here as `threshold` and `min_scene`.

### 3. NLE / editorial diff workflows (EDL / timeline comparison)
- Editors compare an offline and online cut by exporting EDLs or change lists and
  diffing edit points; some finishing tools surface a per-event added/removed/trimmed
  report between two timelines.
- These are project/EDL-based (they read the edit decision list), not pixel-based, and
  require the source project files. This tool infers the cuts from the rendered pixels,
  so it works on two exported files without any project/EDL.

### 4. Video-comparison / QC web tools
- Online tools generally take an upload and either play two clips side by side or report
  simple frame differences; a scene-boundary-level added/removed/moved diff between two
  edits is uncommon.
- Uploading full videos to a service is out-of-model for this repo; local analysis of
  user-provided files/refs via the ffmpeg-runtime bridge is the fit.

## Table-stakes → decisions

| Capability | Seen in | In/out of model | Decision |
| --- | --- | --- | --- |
| Per-frame scene detection | FFmpeg scene/scdet, shot libs | in-model | Run `select='gt(scene,threshold)',showinfo` per video and parse `pts_time` |
| Tunable detector sensitivity | FFmpeg, shot libs | in-model | `threshold` 0.0–1.0, default 0.3 |
| Minimum scene length / de-bounce | shot libs | in-model | `min_scene` seconds, default 0.4 (0 disables) |
| List of cut timestamps per file | FFmpeg + showinfo, shot libs | in-model | `cuts_edit1[]` / `cuts_edit2[]` in the JSON |
| Diff two edits (added/removed) | NLE change lists | in-model | Two-pointer match within `tolerance`; unmatched → added (edit 2) / removed (edit 1) |
| Moved / shifted cut detection | NLE timeline compare | in-model | Matched pair beyond `SAME_EPSILON` (~1 frame) → moved, with `from`/`to`/`shift` |
| Global-shift tolerance | editorial reconciliation | in-model | `tolerance` seconds, default 0.5, so a small offset reads as moves not add+remove |
| One-line human summary + counts | QC reports | in-model | `counts` object + `summary` string |
| Re-cut / render the diffed video | NLE tools | existing sibling tools | Not built here; this MEASURES only. Use video-trim / video-silence-cut to actually cut |
| Side-by-side visual playback, thumbnails | web QC tools, NLEs | out-of-model for this two-video shape | No standalone page; flat JSON via chat + CLI only |
| Project/EDL import | NLE editorial diff | out-of-model | Pixel-based detection instead; no project files required |

## In-vs-out-of-model surface decision (no standalone page)

This tool is intentionally **chat + CLI only — no standalone tool page.** The generic
page driver (`site/tool.js`) handles a SINGLE `source="file"` upload and one ffmpeg
pass; the descriptor here takes `Input::None` plus a required two-item `videos`
`source_list` and runs the ffmpeg scene detector once per video. That two-video /
two-pass shape cannot be exercised by the single-upload page driver, so it cannot be
page-verified and no page is generated (same architecture decision as the sibling
two-file tools video-audio-loudness-compare and video-audio-sync-offset-finder). Both
locally-verifiable surfaces that DO apply are exercised: the descriptor/drift-guard
schema tests (what chat consumes) via `cargo test --workspace`, and a real runtime run
via `gizza tool video-scene-cut-diff --json …` over the native ffmpeg-runtime bridge.
The no-page decision is also recorded in the block's `src/lib.rs` module docs.

Each ffmpeg exec is SINGLE-input and independent (one video per pass); only the pure
`core::diff_cuts` step combines the two timestamp lists. That is why this sidesteps the
un-buildable multi-input ffmpeg case (e.g. video-concat, which needs several inputs in
ONE ffmpeg invocation): here nothing is muxed, so no single ffmpeg call ever needs two
media inputs.

## Existing-block duplicate check

- `video-silence-cut` detects silent spans in ONE video and re-cuts it; it is a
  single-video editor, not a two-edit cut-list diff.
- `video-audio-sync-offset-finder` compares two files' AUDIO timing (a single offset),
  not their visual scene structure.
- `video-audio-loudness-compare` compares two files' loudness, not their cuts.
- No existing block detects or diffs scene cuts, so this is not a semantic duplicate: it
  is the measurement-only, two-edit scene-cut comparison surface.

> Original work only — competitor behaviour is paraphrased; no competitor copy,
> branding, or trademarks are reused.
