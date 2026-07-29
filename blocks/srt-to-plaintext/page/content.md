## About this tool

**SRT to Plain Text** turns subtitle files into readable transcript text. Paste a
SubRip (`.srt`) or WebVTT file and it removes the structural parts of each cue:
cue numbers, timestamp ranges such as `00:00:01,000 --> 00:00:04,000`, blank
separators, WebVTT headers and note/style blocks.

Use it to clean captions before editing a transcript, feeding text into a note
app, summarizing a video, translating dialogue, or removing timestamps from an
exported subtitle file.

### Options

- **Output layout**: one cleaned line per cue, preserve cue blocks, or one flowing
  paragraph.
- **Strip formatting tags**: removes `<i>`, `<b>`, `<font ...>` and ASS/SSA
  override blocks like `{\an8}`.
- **Remove sound effects**: drops bracketed non-speech cues like `[applause]`,
  `(door slams)`, and music-note markers.
- **Remove speaker labels**: strips leading labels such as `NARRATOR:` or
  `- JOHN:`. It is intentionally optional because some real dialogue starts with
  a colon.
- **Dedupe**: collapses consecutive duplicate captions from rolling auto-caption
  exports.

## Worked example

Input:

```srt
1
00:00:01,000 --> 00:00:04,000
<i>Hello there.</i>

2
00:00:05,500 --> 00:00:07,250
[applause] JOHN: Welcome back.
```

With tag stripping, sound-effect removal and speaker-label removal enabled:

```text
Hello there.
Welcome back.
```

## FAQ

<details>
<summary>Does this support WebVTT as well as SRT?</summary>

Yes. It recognizes WebVTT timing lines with dot milliseconds, skips a leading
`WEBVTT` signature, and drops `NOTE`/`STYLE` header blocks. It is not a full VTT
converter; it focuses on extracting transcript text.

</details>

<details>
<summary>Will it remove every timestamp in the file?</summary>

It removes subtitle timing lines shaped like `start --> end` where both sides are
SRT/WebVTT timestamps. Ordinary text that happens to contain a time is kept,
because only cue timing lines are considered structure.

</details>

<details>
<summary>Why are speaker labels optional?</summary>

A leading `NAME:` pattern is a heuristic. It is useful for captions such as
`NARRATOR: It begins`, but it can be wrong if the dialogue itself starts with a
colon-shaped phrase. Leave it off unless your subtitles consistently include
speaker labels.

</details>

<details>
<summary>Can it keep cue breaks instead of making one line per cue?</summary>

Yes. Choose **Preserve cue blocks** to keep the original line breaks inside each
caption and put a blank line between cues. Choose **One paragraph** to join the
whole transcript into a single flowing block.

</details>

## Limits

- This is a text cleaner, not OCR or speech recognition. It needs an existing
  subtitle file.
- It rejects input with no recognizable subtitle timing line instead of echoing a
  random text blob.
- It does not preserve style, positioning, cue IDs, chapter metadata, or VTT cue
  settings; only transcript text is returned.
