# video-cut-segments — competitor analysis (2026-07-11)

**Tool function:** Cut a single video down to multiple keep/remove windows given a
manual timestamp list, then join the kept parts into one clip (multi-segment
trim / "jump-cut by hand"). Single video input; single-pass ffmpeg.

## Competitors scanned

1. **Canva Video Splitter/Trimmer** — split the timeline at chosen timestamps,
   delete the sections you don't want, the rest re-joins. Timestamp guide while
   dragging; split "by the minute". Visual timeline UI (out-of-model — gizza is a
   headless params tool).
2. **Clideo Cut/Split Video** — mark a part with start/end markers, then choose
   *extract* (keep) or *delete* (remove) the selection. Supports several segments;
   duration controls for uniform clips.
3. **Clipchamp** — split media into sections at specific timestamps and delete the
   clip in between (the classic "remove the middle" case).
4. **Adobe Express / ImageToolHub** — trim with precise start/end timestamps typed
   in manually; multiple output formats.
5. **ffmpeg cookbooks (markheath.net, ffmpeg-user list, superuser)** — the
   engineering reference: use `filter_complex` with `trim`/`atrim` +
   `setpts=PTS-STARTPTS`/`asetpts` + `concat` to keep several windows and re-join;
   the bare `select` filter accumulates A/V desync across multiple sections and
   does not update timestamps, so trim+concat is the recommended path.

## Table-stakes → decision

| Capability | In model? | Where it lands |
|---|---|---|
| Multiple time windows from a typed list | ✅ | `segments` param (comma/newline list of `start-end`) |
| Keep the listed windows (extract + join) | ✅ | `mode = keep` (default) |
| Remove the listed windows, keep the rest | ✅ | `mode = remove` (complement, open-ended tail → no duration probe) |
| Timestamp formats `SS`, `MM:SS`, `HH:MM:SS`, fractional | ✅ | `parse_timestamp` in core |
| Audio kept in sync | ✅ | trim+atrim+concat (a=1), the correct path over bare `select` |
| Re-join into one file | ✅ | `concat=n=N:v=1:a=1` |
| Preset chips for common cases | ✅ | `[[example]]` chips (keep two clips; remove a middle section) |
| Visual timeline / drag-to-mark UI | ❌ out-of-model | headless params tool — user types timestamps |
| Frame-accurate scrubbing / preview | ❌ out-of-model | no interactive player surface |
| Multiple separate output files (one per clip) | ❌ out-of-model | gizza ffmpeg dispatch is single-output; we join into one |
| Lossless stream-copy multi-cut | ❌ out-of-model | multi-segment concat needs re-encode for frame-accurate joins (single-window lossless is `video-trim`) |

## Design decisions

- **trim+concat, not bare `select`.** Competitor/ffmpeg research flags `select`'s
  A/V desync accumulation across multiple windows (`video-silence-cut` uses
  `select` because it re-times from a *detected* complement; here the windows are
  user-typed and must stay frame/audio aligned, so `filter_complex` trim+concat is
  the correct engine).
- **Both modes are single-pass, no duration probe.** `keep` trims each window with
  explicit `start:end`; `remove` builds the complement where the final tail segment
  uses an open-ended `trim=start=X` (runs to EOF), so we never need the clip
  duration → the generic single-pass ffmpeg **page** driver works (unlike
  `video-silence-cut`, which is chat+CLI only for its detect pass).
- **Output is mp4 (H.264/AAC).** Joining re-encodes; container normalized to mp4.
- **Not a duplicate:** `video-trim` = one window, stream-copy; `video-silence-cut`
  = auto silence detection; this = manual multi-window keep/remove + join.

_No competitor copy, branding, or trademarks were used; paraphrase only._
