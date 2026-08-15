## About this tool

A MIDI file does not store audio — it stores instructions, and the speed those
instructions play back at comes from a handful of **tempo meta events** that say
how many microseconds one quarter note lasts. This tool rewrites those events
and nothing else. Every note-on, note-off, velocity, controller, pitch bend,
program change, track name and time signature is carried through byte-for-byte,
so the music that comes out is the music that went in — just faster or slower.

Paste the file as **base64 or hex** and pick how you want the new tempo chosen:

- **Set an exact BPM** — give a number and the file's first tempo becomes it.
  This is what you want when a collaborator sends a part at 128 and your project
  runs at 120.
- **Scale by a multiplier** — `0.75` for a slow practice pass, `2.0` for double
  speed, `1.1` for "10% faster". Use this when the target is relative and you
  don't care what the absolute number lands on.

If the file already changes tempo as it plays — a ritardando at the end of a
phrase, an accelerando into a chorus — you choose what happens to that shape.
**Scale every tempo event** multiplies all of them by the same ratio, so the
rubato survives in proportion. **Flatten** deletes the whole map and writes one
steady tempo at the start, which is the usual fix for a performance-captured
file that drifts when you want it locked to a grid.

**Keep the playing time** is the re-notation case, and it is the opposite of
what most people want, so it is off by default. Normally changing the tempo
changes how long the file lasts. With this on, every note's tick position is
rescaled by the same ratio too, so the file still plays for exactly as long as
it did — a quarter note at 120 BPM becomes a half note at 240 BPM. Reach for it
when you need a part to *read* at your project's BPM without altering how it
sounds.

The tick resolution (PPQ) is never touched, so the note grid stays exactly as
your DAW wrote it. Everything runs as WebAssembly in this tab — the file is
never uploaded, stored or sent anywhere.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question. -->

<details>
<summary>Does changing the tempo damage the notes or the timing feel?</summary>

No. The note data is never rewritten — only the tempo meta events are. Note
positions are stored as **ticks**, which are musical units (so many ticks per
quarter note), not units of time. Changing the tempo changes how long a tick
lasts in seconds, so every note keeps its exact relative position, length and
velocity. Swing, humanised timing and velocity dynamics all survive untouched.

</details>

<details>
<summary>Why do I have to paste base64 or hex instead of uploading the .mid?</summary>

The tool runs entirely inside your browser tab, and this page takes its input as
text so the same code can serve the chat and command-line versions of the tool
without a file-upload service behind it. Convert your file first — on macOS or
Linux, `base64 song.mid | tr -d '\n'` prints exactly what to paste; on Windows,
`certutil -encode song.mid out.txt` does the same. Hex works too, and the
**Auto-detect** setting figures out which one you pasted. Whitespace is ignored,
as are `:` and `-` separators in hex.

</details>

<details>
<summary>My file speeds up and slows down. Which tempo-map option do I want?</summary>

If those changes are musical and you want to keep them, use **Scale every tempo
event** — the default. A file that ritards from 120 to 60 at the end, scaled to
1.5×, ritards from 180 to 90: the same gesture, faster. If the tempo drift is an
artefact of a live performance capture and you want the part locked to one
number, use **Flatten**, which removes every tempo event and writes a single
constant tempo at the very start.

</details>

<details>
<summary>What does the file's original BPM come from if it has no tempo event?</summary>

The MIDI specification says a file with no tempo event plays at **120 BPM**, so
that is the value used as the starting point, and a tempo event is written into
the output. This is why the summary can report an original tempo of 120.00 for a
file that technically contained no tempo information at all.

</details>

<details>
<summary>Why was my file rejected for using SMPTE timecode?</summary>

There are two ways a MIDI file can express time. Almost all files are
**metrical**: timing is ticks-per-quarter-note, and playback speed comes from
tempo events — those are the files this tool works on. A few use **SMPTE
timecode** division, where position is tied to real video frames and the speed
comes from the frame rate. Such a file has no tempo to change, so re-exporting it
from your DAW with a metrical (PPQ) time base is the fix.

</details>

<details>
<summary>What are the limits on tempo and file size?</summary>

Target BPM must be between **20 and 400**, and the speed multiplier between
**0.1 and 10** (ten times slower to ten times faster). Files up to **4 MiB** are
accepted, which comfortably covers even a dense multi-track orchestral score.
Beyond those, a MIDI tempo event physically cannot store a value outside roughly
3.58 BPM to 60,000,000 BPM, and the tool reports that rather than writing a file
your DAW would refuse to open.

</details>
