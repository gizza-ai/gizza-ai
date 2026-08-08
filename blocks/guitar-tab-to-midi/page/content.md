## Turn ASCII tablature into a playable MIDI file

Paste the kind of text tab that lives in forums, song archives and text files — one line per string,
fret numbers on the strings, dashes for rests — and download a real Standard MIDI File you can drop
into a DAW, notation editor or any MIDI player.

The model is the familiar one: a stave is a grid of character columns, and the pitch of a fret number
is `open string + capo + fret + transpose`. Every string carrying a fret in the same column starts
together, so chords stay chords. A run of digits is one fret (`12` is the twelfth fret, not a 1 and a
2), a column that is `|` on every string is a bar line and consumes no time, and articulation marks
(`h p b r s t / \ ~ ^ * . ( )`) are skipped while the frets around them still sound. Several staves
separated by blank or prose lines are concatenated in reading order, so a whole song file converts in
one pass.

Open strings come from a **tuning** preset — `auto` reads the number of string lines (4 → bass,
5 → 5-string bass, 6 → guitar, 7/8 → extended range) or you can pick drop D, half/full step down,
drop C, open G, open D, DADGAD or ukulele, or list exact pitches in **custom tuning**. Set a **capo**,
**transpose**, **tempo**, **velocity** and a General MIDI **instrument**, choose how long one column
lasts, and decide whether notes ring until the string's next note or stop after a single column.

### Worked example

This eighth-note power-chord riff:

```text
e|-------------|
B|-------------|
G|-------------|
D|--2--2--5--5-|
A|--2--2--5--5-|
E|--0--0--3--3-|
```

converts to 12 notes (four three-note chords: E5 twice, then G5 twice) on a 6-string stave in standard
tuning, spanning E2 to D4. At the default 120 BPM with eighth-note columns each column lasts 0.25 s.

Headlessly, the same conversion runs through the CLI — the base64 `.mid` comes back in the response
envelope:

```bash
gizza tool guitar-tab-to-midi "e|-------------|
B|-------------|
G|-------------|
D|--2--2--5--5-|
A|--2--2--5--5-|
E|--0--0--3--3-|" instrument=distortion-guitar tempo=132
```

Switch **How time advances** to *only note columns* when a tab's dash spacing is decorative rather
than rhythmic: the onsets are then spaced evenly instead of following the layout.

## Limits and edge cases

Text tablature carries pitch and order, not real rhythm — nothing in `--3---5--` says how long a note
is held. This converter therefore treats the grid as evenly spaced columns, which is faithful for
most tabs and approximate for anything with a swing feel, triplets or mixed note values. Techniques
are recognised as characters and skipped, not performed: a hammer-on, pull-off, slide, bend or
vibrato mark does not bend pitch or add a note, so a bent note sounds at its written fret.
`x`/`X` mutes are silent by default and become short muted notes on the open string when you enable
that option.

Every stave in one conversion must use the same number of string lines, and the tuning must have that
many strings — a 6-string preset against a 4-line bass tab is rejected with a message instead of a
wrong file. Frets are capped at 36, resulting pitches must land inside MIDI 0–127 (a large negative
transpose is what usually pushes them out), input is limited to 1 MiB and 20,000 notes, and tempo,
capo, transpose and velocity are range-checked. Errors name the offending line and column. The
output is a single-track format-0 file at 480 ticks per quarter note with one tempo event, one
program change and one channel — drum tabs, per-note velocities, lyrics and multi-instrument
arrangements are out of scope.

## FAQ

<details>
<summary>Which tab notation does it understand?</summary>

Standard ASCII tab: one line per string, an optional string label such as `e|` or `G |` at the start
of each line, fret numbers (single or double digit), `-` or `=` for rests, `|` bar lines, and the
common articulation characters `h p b r s t / \ ~ ^ * . ( ) < > + _ '`. Lines that are prose rather
than tab are ignored, so you can paste a whole song file with titles and comments in it.

</details>

<details>
<summary>How does it know the tuning?</summary>

By default it infers one from how many string lines a stave has — 4 lines is a bass, 5 a five-string
bass, 6 a guitar in standard tuning, 7 and 8 the extended-range tunings. Choose a named preset when
the tab is in drop D, drop C, DADGAD, an open tuning or a step down, or type exact open-string
pitches into **Custom tuning** (`D2,A2,D3,G3,B3,E4` or MIDI numbers like `38 45 50 55 59 64`),
lowest tab line first. A custom tuning always overrides the preset.

</details>

<details>
<summary>The file plays back at the wrong speed or rhythm. What should I change?</summary>

Two settings control that. **Length of one tab column** sets the musical value of a single character
column: pick sixteenth notes for dense tabs and quarter notes for sparse ones. **Tempo** sets the
BPM written into the file. If the dash spacing in your tab is decorative, switch **How time
advances** to *only note columns* so the onsets are evenly spaced regardless of layout.

</details>

<details>
<summary>My tab came out an octave or several semitones off.</summary>

Check three things. If the tab is written with the low string on the top line, set **Stave layout**
to the inverted option — otherwise the file comes out pitch-mirrored. If the song is played with a
capo, set the capo fret rather than adding it to every fret number. **Transpose** then applies a
further shift in semitones after the capo, so `-12` drops an octave and `+12` raises one.

</details>

<details>
<summary>Can I hear it on this page?</summary>

No — the page produces the `.mid` file for download rather than audio. A MIDI file contains notes,
not sound, so playing it needs an instrument: open the download in a DAW, a notation program, or any
MIDI player with a soundfont, and choose whatever instrument you like there. The General MIDI
instrument setting here is written into the file as a program change, which those players honour by
default but can always override.

</details>

<details>
<summary>Is my tab uploaded anywhere?</summary>

No. The converter is compiled to WebAssembly and runs inside your browser tab; the resulting file is
handed to you as a local `data:` URL. Nothing is sent to a server and nothing is stored.

</details>
