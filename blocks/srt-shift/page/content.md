## What this tool does

Subtitles out of sync with the video? This tool shifts **every** timestamp in a
SubRip (`.srt`) subtitle file forward or backward by the same fixed offset, so
the whole track lines up again. Paste your subtitles, type how far to move them,
and copy the corrected file back out. Nothing is uploaded — it runs entirely in
your browser, works offline, and needs no sign-up.

## How to use it

1. Paste the full contents of your `.srt` file into the box.
2. Enter the **Offset**:
   - a **positive** number to *delay* the subtitles (they appear later), or
   - a **negative** number to *advance* them (they appear earlier).
3. Pick the **Unit** — **seconds** (fractions like `2.5` are fine) or
   **milliseconds**.
4. Copy the shifted subtitles from the output.

## What gets changed

Only the timing lines are rewritten — for example
`00:00:01,000 --> 00:00:04,000`. The cue numbers and the dialogue text are kept
exactly as they were, and the file's line endings (Windows `\r\n` or Unix `\n`)
are preserved.

Any timestamp that would land before the start of the video is clamped to
`00:00:00,000`, so a large negative shift never produces a negative time.

## Examples

| Offset | Unit | Effect on `00:00:01,000 --> 00:00:04,000` |
| --- | --- | --- |
| `2.5` | seconds | `00:00:03,500 --> 00:00:06,500` (subtitles delayed) |
| `-0.5` | seconds | `00:00:00,500 --> 00:00:03,500` (subtitles advanced) |
| `750` | milliseconds | `00:00:01,750 --> 00:00:04,750` |

## FAQ

**Which way should I shift?** If the subtitles appear *too early* (before the
words are spoken), use a **positive** offset to delay them. If they appear *too
late*, use a **negative** offset to advance them.

**Does it handle fractional seconds?** Yes — enter values like `1.25` seconds,
or switch to milliseconds for exact frame-level nudges.

**Is my file uploaded anywhere?** No. The shift happens locally in your browser,
so your subtitles never leave your device, and it keeps working offline once the
page has loaded.

**My subtitles drift — they're fine at the start but off by the end.** This tool
applies one constant offset to the whole file, which fixes a uniform lead/lag. A
*growing* drift means a frame-rate mismatch (e.g. 23.976 vs 25 fps), which needs
a linear time-stretch rather than a flat shift.

**What format does it expect?** Standard SubRip (`.srt`): a cue number, a
`HH:MM:SS,mmm --> HH:MM:SS,mmm` timing line, then one or more lines of text,
with a blank line between cues.
