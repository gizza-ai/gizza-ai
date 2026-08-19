## About this tool

MIDI note extractor flattens a Standard MIDI File into a single note table you can drop straight into a spreadsheet, a pandas `read_csv`, or a plotting script. Paste the `.mid`/`.midi` bytes as base64 or hex, pick the columns and the time unit, and every note-on/note-off pair in the file becomes one row: pitch, start, duration and velocity, plus the track and channel it came from.

Everything happens in your browser — the MIDI data is parsed locally by WebAssembly, never uploaded. Note-on and note-off messages are paired per channel and pitch (a note-on with velocity 0 counts as a note-off, as the MIDI spec allows), all tracks are flattened into one table, and rows are sorted by start time unless you ask for track or pitch order.

Choose **Minimal** columns for the classic four-field note list (`start,duration,pitch,velocity`), **Standard** to keep the track, channel and scientific pitch name, or **Full** to also get the track name, the note end, and the tempo in force when the note starts. Times can be seconds (computed from the file's tempo map, including mid-file tempo changes), raw MIDI ticks, or beats (quarter notes). Velocity can stay the raw 0–127 integer or be normalized to 0.0–1.0 for analysis.

### Worked example

The example chips below load a tiny one-note MIDI file: 96 ticks per quarter note, 120 BPM, one track called `Piano` playing middle C at velocity 64 for one quarter note.

With **Standard** columns and time in **seconds**, the result is:

```
track,channel,start,duration,pitch,note_name,velocity
0,0,0.000,0.500,60,C4,64
```

Switch the time unit to **ticks** and the columns to **minimal**, and the same note becomes:

```
start,duration,pitch,velocity
0,96,60,64
```

At 96 ticks per quarter and 120 BPM, a quarter note is 96 ticks and 0.5 seconds — the two views agree.

### Limits and edge cases

- The input must be a Standard MIDI File whose first chunk is `MThd` (Format 0, 1 or 2). RIFF-wrapped RMID files, sequencer project files, and audio recordings are not decoded.
- Up to 50,000 notes are returned. Larger scores can be narrowed with the track or channel filter, or split before extraction.
- Track and channel numbers are 0-based, so channel 9 is the General MIDI drum channel and `0,2` selects the first and third tracks.
- A note-on that the file never closes with a note-off is held to the end of its track rather than being silently dropped.
- Beats require a metrical (PPQ) file; files using SMPTE timecode division have no musical beat grid, so use seconds or ticks for those.
- Track names containing the delimiter, a double quote, or a newline are quoted RFC 4180 style, so the output stays valid CSV.
- Tick values are always whole numbers; the decimal-places setting applies to seconds, beats, normalized velocity, and tempo.

## FAQ

<details>
<summary>How do I get the base64 or hex text for my .mid file?</summary>

Encode the file locally first — `base64 song.mid` on macOS or Linux, `[Convert]::ToBase64String([IO.File]::ReadAllBytes("song.mid"))` in PowerShell, or `xxd -p song.mid` for hex — then paste the text into the input box. Leave the encoding on **Auto** and the tool works out which one it is. The command-line version accepts the same string.

</details>

<details>
<summary>What exactly do the start and duration columns measure?</summary>

`start` is the note-on position measured from the beginning of the file, and `duration` is the distance from that note-on to its matching note-off. In **seconds** both are computed from the file's tempo map, so a tempo change part-way through a note is accounted for. In **ticks** they are the raw MIDI clock values, and in **beats** they are quarter notes (ticks divided by the file's ticks-per-quarter).

</details>

<details>
<summary>How are overlapping notes and multiple tracks handled?</summary>

Each track is scanned separately and open note-ons are matched to the next note-off with the same channel and pitch, so overlapping or repeated notes pair correctly. All tracks are then flattened into one table with a `track` column (Standard and Full column sets), and rows are ordered by start time by default. Pick **By track, then start time** if you would rather keep each track's notes together.

</details>

<details>
<summary>Can I extract just the drums, or just one instrument?</summary>

Yes. Set **Channels** to `9` for the General MIDI drum channel, or **Tracks** to the 0-based track number that holds the part you want; both fields also accept comma-separated lists such as `0,2`. Run once with **Full** columns to see the track names and channels in the file, then filter.

</details>

<details>
<summary>Does this convert the CSV back into a MIDI file, or play the notes?</summary>

No. This tool reads MIDI and writes a note table; it does not write MIDI files and does not synthesize audio. MIDI is symbolic performance data, so what you get is timing and pitch information, not sound.

</details>
