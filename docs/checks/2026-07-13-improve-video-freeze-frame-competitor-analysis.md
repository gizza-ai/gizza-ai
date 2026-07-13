# video-freeze-frame — competitor scan (2026-07-13)

One WebSearch for "freeze frame video effect tool online"; skimmed the top real
competitors. All observations paraphrased — no competitor copy, branding, or
trademarks reproduced.

## Competitors skimmed

1. **EzGIF — Freeze video.** Upload, scrub a built-in player to the frame, click
   "use current position" to set the freeze point, then set a pause length (from
   fractions of a second up to ~2000 s). Accepts MP4/WebM/MOV/AVI/MKV up to 200 MB.
2. **Kapwing — Freeze Frame.** Timeline editor; move the playhead to the frame,
   press a Freeze button in the timing panel to generate a still segment; drag to
   set the hold length. Account-based editor.
3. **Clideo / CapCut / Flixier — Freeze Frame.** Same shape: scrub a timeline
   playhead to the moment, click "Freeze" to turn that frame into a still, then
   drag the still's edges to set how long it holds. Timeline-editor UX, mostly
   account-gated for export.

## Table-stakes → decisions

| Capability | In/out of model | Decision |
|---|---|---|
| Choose the freeze point | in-model | `time` param (seconds), default 1. |
| Choose the hold/pause length | in-model | `duration` param (seconds, 0–60), slider on the page, default 2. |
| Output = source + hold length | in-model | Split + `tpad` clone + concat → output is exactly `duration` s longer. |
| Multi-container input (MP4/MOV/WebM/MKV) | in-model | `Input::Video`; H.264 re-encode; container kept when it can hold H.264 else MP4. |
| Freeze the very first frame (intro still) | in-model | `time = 0` supported; example chip "Hold the opening frame". |
| Preset hold lengths | in-model | `[[example]]` chips (2 s, dramatic 4 s, quick 0.5 s beat, opening frame). |
| In-browser, no account, nothing uploaded | in-model | Native to gizza (wasm ffmpeg, local). Stated on the page. |
| Timeline playhead scrubbing to find the frame | out-of-model (UX) | We take a numeric timestamp instead of a scrub-timeline editor; page states this. Considered, not built (no timeline widget in the gizza page model). |
| Keep audio (continuing or silenced during the hold) | out-of-model (scope) | **Dropped.** Re-timing audio around an inserted still (silence for the hold, tail shifted by `duration`) needs probing + a multi-segment audio graph that also has to tolerate silent inputs — out of scope for a robust single-pass builder. Documented as a limitation + FAQ; suggest re-adding audio with a separate tool. |
| Export the freeze as a standalone still image | out-of-model (different tool) | gizza already has frame-extraction tools (`video-frame-extract`, `extract-frames`); not duplicated here. |
| Very long holds (up to ~2000 s like EzGIF) | rejected | Capped at 60 s — longer holds bloat the wasm re-encode and are almost always a mistake for the 25 MB in-browser budget. |

## Non-dup confirmation

`video-frame-extract` / `video-first-last-frame-extractor` / `extract-frames`
*extract* frames to images; this tool *inserts* a held still into the moving video
(freeze-frame effect) and returns a video. Different output kind → not a duplicate.
