## About this tool

MIDI to JSON turns a Standard MIDI File into readable structured data without uploading the file anywhere. Paste the `.mid`/`.midi` bytes as base64 or hex, choose the output view, and the tool parses the SMF header, tracks, timing division, tempo map, notes, controller messages, program changes, pitch bends, and common meta events.

Use **Notes** when you want a musical view for visualizers, sequencers, games, or data analysis: note-on/note-off pairs are combined into notes with a pitch name such as `C4`, MIDI note number, channel, start time, duration in ticks, duration in seconds, and velocity. Use **Events** when you need the ordered raw event stream for debugging or conversion work. Use **Summary** for a quick overview of track count, duration, BPM, time signature, channels, note count, and track names.

### Worked examples

- Paste the hex example from **Summary — compact overview** to get a small JSON object showing one track named `Piano`, a 120 BPM tempo, 4/4 time, and one note.
- Switch the same example to **Notes** to see that note paired as `C4`, starting at tick `0` with a `480` tick / `0.5` second duration.
- Choose **Events** to inspect the underlying MIDI messages: track name, tempo, time signature, note-on, note-off, and end-of-track.

### Limits and edge cases

- The input must be a Standard MIDI File that starts with the `MThd` chunk. RIFF-wrapped RMID, karaoke variants, and sequencer project files are not decoded unless their bytes begin with a normal SMF header.
- Timing in seconds is calculated from tempo events across the file. SMPTE timecode division is also reported, but musical PPQ files are the common case.
- Output is JSON text, not a playable audio render. This tool does not synthesize MIDI to sound and does not convert JSON back into a MIDI file.
- Program numbers are emitted as numbers; General MIDI instrument names are intentionally not guessed.

## FAQ

<details>
<summary>Can I upload a .mid file directly?</summary>

This page accepts the MIDI bytes as base64 or hex text. If you have a `.mid` file locally, base64-encode it first, paste the text into the input field, and leave encoding on **Auto** or set it to **Base64**. The CLI can also receive the same base64 or hex string.

</details>

<details>
<summary>What is the difference between Notes, Events, and Summary?</summary>

**Notes** pairs note-on and note-off messages into musical notes with names, start times, durations, channels, and velocities. **Events** preserves the ordered MIDI event stream for each track, including raw note messages, controller changes, program changes, pitch bends, SysEx, and meta events. **Summary** returns counts and headline timing fields for a quick inspection.

</details>

<details>
<summary>Does this render MIDI audio or identify instruments by name?</summary>

No. MIDI is symbolic performance data, not audio. This tool parses the data into JSON and reports program-change numbers, but it does not synthesize audio and does not bundle a General MIDI instrument-name table.

</details>

<details>
<summary>Why did my file fail with a missing MThd header?</summary>

The parser expects a Standard MIDI File whose first chunk is `MThd`. Some containers wrap MIDI data in another format, and some music apps save project/session files that are not SMF files. Export the sequence as `.mid` or `.midi`, then encode those bytes as base64 or hex.

</details>
