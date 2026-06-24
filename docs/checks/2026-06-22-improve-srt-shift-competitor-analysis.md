# srt-shift — competitor analysis (2026-06-22)

Tool: **SRT Subtitle Shifter** (`blocks/srt-shift`). Shifts every timestamp in a
SubRip (.srt) file forward/backward by a fixed offset to resync subtitles that
run uniformly early or late. Pure-compute (no model, no ffmpeg) — runs on all
three surfaces: chat (LLM API), CLI, and the standalone page.

## Surfaces verified

- **Chat block** — `wafer build` validated `target/block.wasm` (300.5 KiB, OK).
- **CLI** — `gizza tool srt-shift srt=… offset=2.5` (forward), `offset=-500
  unit=milliseconds` (backward), `offset=-10` (clamp to 00:00:00,000), and a
  non-SRT input correctly errors with exit 1.
- **Page** — 4 Playwright tests pass: forward (default seconds), backward
  (milliseconds via `<select>`), zero-clamp, and a `?srt=…&offset=…` deep link.

## Top competitors surveyed

1. **SubShifter** (subshifter.bitsnbites.eu) — paste/upload, shift all timestamps
   by an offset, download. Client-side. SRT focused.
2. **HappyScribe SRT Time Shift** — `+1.20` style offset (s + ms) applied to every
   timecode; upload/download.
3. **SubtitleTools — Subtitle Sync Shifter** — shifts all timings by an entered
   amount of milliseconds, forward or backward; file upload.
4. **Subtitles Edit — Subtitle Time Shifter** — paste **or upload SRT/VTT**,
   positive/negative value, download corrected file.
5. **Maestra / Matesub** — "free subtitle shifter"; Matesub additionally does
   **AI auto-sync** (analyzes speech to detect the mismatch automatically).

## Capability diff vs. gizza srt-shift

| Capability | Competitors | gizza srt-shift | Verdict |
| --- | --- | --- | --- |
| Shift all timestamps by a fixed offset | yes | **yes** | parity |
| Forward and backward (negative) offset | yes | **yes** | parity |
| Seconds **and** milliseconds units | mixed (some ms-only, some s) | **yes (both)** | at/above parity |
| Fractional seconds (e.g. 2.5) | yes | **yes** | parity |
| Clamp negative results to 00:00:00,000 | implicit | **yes (explicit + tested)** | parity |
| Preserve cue numbers + dialogue verbatim | yes | **yes** | parity |
| Preserve CRLF/LF line endings | varies | **yes (tested)** | above parity |
| Preserve trailing positioning coords | rarely | **yes (tested)** | above parity |
| Accept `.`-separated (VTT-style) timestamps on input | varies | **yes** (normalizes to `,`) | above parity |
| Runs locally / private | yes (client-side) | **yes** (wasm, no upload) | parity |
| Available via chat + CLI as well as a page | no | **yes** | above parity (unique) |

## Gaps considered

- **File upload / download of an .srt file.** Competitors offer a file picker +
  download button. gizza's page is a paste-in / copy-out text tool (the page
  framework's text format), consistent with the other text tools in this repo
  (url-encode, html-formatter, etc.). Not a model gap — the core handles the
  exact same content; only the I/O affordance differs, and adding a file-input
  asset kind for text is out of scope for the page framework here. **Not built.**
- **VTT (WebVTT) output / full VTT support.** Some competitors also retime
  `.vtt`. Our tool *accepts* VTT-style dot-separated timestamps on input and
  normalizes to SubRip output, but it does not emit the `WEBVTT` header / cue
  settings format. A dedicated srt↔vtt or vtt-shift tool would be a separate
  backlog item. **Not built** (would be a distinct tool, not a copy gap).
- **AI auto-sync** (Matesub) — detects the offset automatically from the audio.
  Out of model: needs speech recognition / an ML model + the video's audio,
  neither of which fits gizza's pure-Rust + ffmpeg envelope. **Not built —
  out of model.**

## Improvements applied during this pass

The tool was authored directly to competitive parity, so no follow-up fixes were
needed:
- Both **seconds and milliseconds** units (matches the union of competitor
  feature sets — some are ms-only, some seconds-only).
- **Fractional second** offsets and **negative** offsets.
- Robustness beyond the typical competitor: **CRLF/LF preservation**, **trailing
  positioning coordinate preservation**, **dot-separator tolerance**, explicit
  **zero-clamp**, and rejection of non-SRT input (rather than silently echoing
  it back). Each is covered by a unit test.

No competitor copy, branding, or trademarks were used.
