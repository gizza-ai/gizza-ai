## What this tool does

Converts subtitle files between the two most common plain-text formats —
**SubRip (`.srt`)** and **WebVTT (`.vtt`)** — in either direction. Paste your
subtitles, and the converter detects the format and produces the other one:
SubRip for media players and re-encoding, WebVTT for the HTML5 `<track>`
element and the web. Nothing is uploaded — it runs entirely in your browser,
works offline, and needs no sign-up.

## How to use it

1. Paste the full contents of your `.srt` or `.vtt` file into the box.
2. Leave **Direction** on **auto** to detect the input format and convert to
   the other, or force **SRT → VTT** / **VTT → SRT**.
3. Copy the converted subtitles from the output and save them with the matching
   extension (`.vtt` or `.srt`).

## SRT vs WebVTT — what actually changes

The two formats are almost identical line-oriented cue lists. The converter
rewrites only what differs:

| | SubRip (`.srt`) | WebVTT (`.vtt`) |
| --- | --- | --- |
| Timestamp separator | comma — `00:00:01,000` | period — `00:00:01.000` |
| File header | none | a leading `WEBVTT` line |
| Hours field | always `HH:MM:SS` | `HH:MM:SS` or short `MM:SS` |
| Cue settings | not supported | optional, e.g. `line:90% align:center` |

Going **SRT → VTT** adds the `WEBVTT` signature and switches commas to periods.
Going **VTT → SRT** strips the `WEBVTT` header block, switches periods to
commas, expands any short `MM:SS.mmm` timestamps to full `HH:MM:SS,mmm`, and
drops WebVTT cue settings (SubRip has no equivalent). Cue numbers and the
dialogue text are always kept verbatim, and Windows (`\r\n`) or Unix (`\n`)
line endings are preserved.

## Examples

`SRT → VTT`

```
1
00:00:01,000 --> 00:00:04,000
First line.
```
becomes
```
WEBVTT

1
00:00:01.000 --> 00:00:04.000
First line.
```

## FAQ

<details>
<summary>Which format do I need?</summary>

Use **WebVTT** (`.vtt`) for subtitles shown in a web
browser via the HTML5 `<track>` tag. Use **SubRip** (`.srt`) for desktop media
players (VLC, MPV), uploading to most video platforms, and muxing into a video
file.

</details>

<details>
<summary>Does it change the timing?</summary>

No. Only the timestamp *format* changes (comma vs
period); the actual times stay the same, so the subtitles stay in sync.

</details>

<details>
<summary>What about WebVTT styling and positioning?</summary>

Cue settings after the timestamp
(like `line:90%` or `align:center`) are dropped when converting to SRT, because
SubRip doesn't support them. WebVTT styling blocks (`STYLE`, `NOTE`) in the
header are removed too. Going the other way, SRT has no cue settings to add.

</details>

<details>
<summary>Is my file uploaded anywhere?</summary>

No. The conversion happens locally in your
browser, so your subtitles never leave your device, and it keeps working offline
once the page has loaded.

</details>

<details>
<summary>What format does it expect?</summary>

Standard SubRip or WebVTT: a cue number
(optional in VTT), a `-->` timing line, then one or more lines of text, with a
blank line between cues.

</details>
