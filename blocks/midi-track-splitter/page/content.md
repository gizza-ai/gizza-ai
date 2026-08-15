## About this tool

A multi-track MIDI file is a container: one header, then a stack of **track
chunks**, one per part, plus a conductor track at the front holding the tempo,
time signature and key signature for the whole piece. This tool takes that
container apart and writes each part back out as a complete `.mid` file of its
own — so you can import just the bass line into a project, hand one player their
part, or feed a single melody to a tool that only accepts one track at a time.

Paste the file as **base64 or hex** and choose how it should be cut:

- **One file per track** — the usual cut for a Format 1 file exported by a DAW
  or notation program, where each instrument already has its own track chunk.
- **One file per MIDI channel** — every event on channel 3 is gathered from
  *every* track into one part. This is the right cut for files that put several
  instruments in one track, and it is what a Format 0 file (which has only a
  single track) is always split by.

The thing naive splitters get wrong is the conductor data. Tempo and time
signature normally live in track 0 only, so a part pulled out on its own plays
back at the MIDI default — 120 BPM in 4/4 — no matter what the original said.
**Copy tempo, time and key signature into every part** is on by default and
fixes exactly that; events a part already carried are never duplicated. Turn it
off if you are re-importing the parts into a project that supplies its own tempo
map.

Each part is written as **Format 0** (one merged track — a genuinely
single-track file) or **Format 1** (the conductor data kept as its own first
track, with the part second). Either way, every note-on, note-off, velocity,
controller, pitch bend and program change is carried through at its original
tick, and the source division — ticks per quarter note, or SMPTE timecode — is
preserved, so nothing is re-gridded or quantised.

Filenames are built from what is actually in the file: the track name meta event
when there is one, otherwise the General MIDI instrument implied by the part's
program change (channel 10 becomes the drum kit), so an import shows
`part-02-bass.mid` rather than `track2.mid`.

### Worked example

Splitting a small three-track file — a conductor track at 120 BPM in 4/4, a
**Piano** part on channel 1 with two notes, and a **Bass** part on channel 2
with one — with every default left alone gives two files:

| # | Part | From | Instrument | Notes | Length | File |
|---|------|------|------------|-------|--------|------|
| 2 | Piano | track 2 · ch 1 | Acoustic Grand Piano | 2 | 1.00 s | `part-02-piano.mid` |
| 3 | Bass | track 3 · ch 2 | Electric Bass (finger) | 1 | 1.00 s | `part-03-bass.mid` |

The conductor track itself is not exported, because it holds no notes — its
tempo and 4/4 time signature were copied into both parts instead, so each one
opens at 120 BPM on its own. Every "Try:" button above runs a real example;
switch **Return** to *Just the list of parts* to see the same table without
producing any files, which is the quick way to look inside a file first.

### Limits and edge cases

- The decoded file may be up to **4 MiB**, and one split may produce at most
  **64 files**. Use the parts selector (`1,3-5`) to work through a bigger file in
  batches.
- **Format 0** input has a single track, so a track split would just hand back
  the input; it is cut by channel automatically and the summary says so.
- **Format 2** files hold independent sequences rather than parallel parts, so
  each track is exported as-is and no conductor data is shared between them.
- **SMPTE timecode** files are supported: the division is copied unchanged and
  part lengths are computed from the frame rate rather than a tempo map.
- Parts with no notes — the conductor track, muted placeholders — are skipped
  unless you untick **Skip parts with no notes**.
- Nothing is uploaded. The whole split runs as WebAssembly inside this tab, so
  unreleased material never leaves your machine.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     `code`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question. -->

<details>
<summary>Will the split parts still play at the right tempo?</summary>

Yes, as long as **Copy tempo, time and key signature into every part** is left
on (it is by default). Tempo, time signature, key signature and SMPTE offset are
collected from the conductor track — and from anywhere else in the file that
carries them — and written into each part, so a part opened on its own plays at
the original tempo instead of the MIDI default of 120 BPM. If a part already
contained one of those events, it is kept and not duplicated.

</details>

<details>
<summary>Should I split by track or by channel?</summary>

Split by **track** when your file came out of a DAW or notation program with one
instrument per track — that is the normal Format 1 layout, and it preserves the
names you already gave each part. Split by **channel** when several instruments
share a track (common in older files, game music and General MIDI karaoke
files), or whenever you specifically want one file per MIDI channel. A Format 0
file only has one track, so it is always cut by channel.

</details>

<details>
<summary>What is the difference between Format 0 and Format 1 output?</summary>

**Format 0** merges the conductor data and the part into a single track — one
genuinely single-track file, which is what most importers and simple players
expect. **Format 1** keeps the conductor data as its own first track with the
part second, which is closer to how DAWs organise a project and makes the tempo
map easy to see and edit separately. The notes are identical either way.

</details>

<details>
<summary>Why paste base64 or hex instead of uploading the .mid file?</summary>

This page runs the whole split as WebAssembly in your browser tab and takes its
input as text, which keeps the tool a pure, link-shareable function — every
field can be pre-filled from the URL. To get the text, run
`base64 -w0 song.mid` (or `xxd -p song.mid | tr -d '\n'` for hex) and paste the
result. Encoding is auto-detected, so you rarely need to touch the encoding
selector.

</details>

<details>
<summary>Does splitting change my notes, timing or velocities?</summary>

No. Note positions are stored in **ticks** — musical units relative to the
file's division — and every event is copied to its original tick in the output.
The division itself is preserved too, so nothing is quantised, re-gridded or
rounded. Swing, humanised timing, velocity dynamics, controller curves and pitch
bends all come through exactly as they went in.

</details>

<details>
<summary>How are the output files named?</summary>

Each name is `<prefix>-<number>-<part name>.mid`, for example
`part-02-bass.mid`. The number is the source track number (or the MIDI channel,
when splitting by channel), and the name comes from the track name meta event,
falling back to the General MIDI instrument of the part's program change, or to
the channel number. Channel 10 is named as the drum kit, per the General MIDI
convention. Change the prefix to anything you like; duplicates get a numeric
suffix so no two files collide.

</details>

<details>
<summary>Can I export only some of the parts?</summary>

Yes — put the ones you want in **Only these parts** as numbers and ranges, such
as `1,3-5`. The numbers are track numbers when splitting by track (track 1 is
the first chunk in the file) and MIDI channel numbers 1-16 when splitting by
channel. Leave the field empty to export everything. Setting **Return** to
*Just the list of parts* first is the easiest way to see which numbers you want.

</details>
