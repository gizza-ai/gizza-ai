## About this tool

**video-cut-segments** trims a single video down to the exact parts you want. Type a
list of `start-end` time windows and pick a mode:

- **Keep** extracts only the windows you listed and joins them into one clip, in the
  order they appear in the source — a fast way to build a highlight reel or drop the
  boring bits between the good takes.
- **Remove** cuts the listed windows out and keeps everything else — the classic
  "delete the middle" edit, an intro trim, or pulling a few mistakes out of a screen
  recording.

Each time can be written as `SS`, `MM:SS`, or `HH:MM:SS`, and fractions of a second
work too (`0:03.5`). Separate windows with a new line, a comma, or a semicolon.
Overlapping or touching windows are merged automatically, so a frame is never
duplicated. Audio stays in sync — the tool trims and re-joins the video and audio
tracks together (`trim`/`atrim` + `concat`), which is more reliable across multiple
sections than the bare `select` filter. The result is a single re-encoded H.264/AAC
`.mp4`.

Everything runs locally in your browser with ffmpeg (WebAssembly). Your video is
never uploaded to a server. For a single continuous clip with no re-encode, use the
**video-trim** tool instead; to auto-drop silent gaps, see **video-silence-cut**.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: site/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown renders. -->

<details>
<summary>How do I write the time windows?</summary>

Each window is `start-end`, and you list one or more of them. Times can be `SS`
(seconds), `MM:SS`, or `HH:MM:SS`, with optional fractions — for example
`5-10`, `0:05-0:10`, or `1:02:03-1:02:30`. Put each window on its own line, or
separate them with commas or semicolons: `0:05-0:10, 1:30-1:45`.

</details>

<details>
<summary>What's the difference between keep and remove?</summary>

**Keep** produces a clip containing only the windows you listed, joined together —
everything else is discarded. **Remove** does the opposite: it deletes the listed
windows and keeps all the footage around them. Both run in a single pass and join
into one output file.

</details>

<details>
<summary>Will the audio stay in sync?</summary>

Yes. The video and audio tracks are trimmed and re-joined together for every
window (`trim`/`atrim` + `setpts`/`asetpts` + `concat`), which keeps them aligned
across multiple cuts. This is why the output is re-encoded rather than a
stream-copy — a frame-accurate multi-segment join can't be done losslessly.

</details>

<details>
<summary>Is my video uploaded anywhere?</summary>

No. The cut runs entirely in your browser with ffmpeg compiled to WebAssembly.
The file never leaves your device, so it works offline and keeps private footage
private.

</details>

<details>
<summary>What if my windows overlap?</summary>

Overlapping or back-to-back windows are merged before cutting, so the same frame
is never selected twice (in keep mode) or double-counted (in remove mode). The
windows are also sorted by start time, so you don't have to list them in order.

</details>
