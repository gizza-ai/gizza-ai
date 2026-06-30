## What this tool does

Got subtitles split across several files — a movie in two parts, or an English
and a translated track you want shown together? This tool **merges two or more
SubRip (`.srt`) or WebVTT (`.vtt`) subtitle files into one**. Every cue from
every file is collected, sorted by its start time, renumbered from 1, and
written back out as a single clean subtitle file. Nothing is uploaded — it runs
entirely in your browser, works offline, and needs no sign-up.

## How to use it

1. Paste your subtitle files into the box, **one after another, with a line
   containing only `===` between each file** (this is how the tool knows where
   one file ends and the next begins).
2. Set the **Per-file shift** if you're joining sequential parts:
   - leave it at `0` to *overlay* tracks that already share a timeline (e.g. two
     languages), or
   - enter a positive number of milliseconds to push each file after the first
     further down the timeline (file 2 by the shift, file 3 by twice the shift,
     and so on) — handy for back-to-back parts like CD1/CD2.
3. Choose the **Output format** — `auto` keeps the first file's format, or force
   `srt` or `vtt`.
4. Copy the merged subtitles from the output.

## How the merge works

- **Mix formats freely.** Each file's format is detected on its own, so you can
  merge a `.srt` and a `.vtt` together; the chosen output format is applied to
  the whole result.
- **Sorted by time.** All cues are ordered by their start time. When two cues
  start at exactly the same moment, the file you listed first stays first — so an
  overlaid second-language track lands in a predictable order.
- **Renumbered.** The output is numbered `1, 2, 3 …` regardless of the original
  cue numbers.
- **Clamped.** A shift that would push a timestamp before the start of the video
  is clamped to `00:00:00`.

## Examples

Two files, overlaid (shift `0`), produce a single track ordered by time:

```
1
00:00:01,000 --> 00:00:02,000
Hello.

===

1
00:00:01,000 --> 00:00:02,000
Bonjour.
```

→

```
1
00:00:01,000 --> 00:00:02,000
Hello.

2
00:00:01,000 --> 00:00:02,000
Bonjour.
```

With a per-file shift of `3600000` ms (one hour), the second file's cues move an
hour later — perfect for appending part 2 after a one-hour part 1.

## FAQ

**How do I separate the files?** Put a line that contains only `===` (three or
more equals signs) between each file. Everything between separators is treated as
one subtitle file.

**Can I merge an SRT and a VTT together?** Yes. Each file's format is detected
independently, and the combined result is written in whichever output format you
pick (or the first file's format on `auto`).

**My two parts both start at 00:00 — how do I join them end to end?** Use the
per-file shift: enter the length of the first part in milliseconds. File 2 is
moved by that amount, file 3 by twice it, and so on.

**Will it keep multi-line dialogue and styling?** Multi-line cue text is kept as
written. WebVTT cue settings (positioning/alignment after the timestamp) are
dropped when they have no SubRip equivalent.

**Is my file uploaded anywhere?** No. The merge happens locally in your browser,
so your subtitles never leave your device, and it keeps working offline once the
page has loaded.
