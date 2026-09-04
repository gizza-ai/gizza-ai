## About this tool

Scale Chord Finder answers two common music-theory questions from the same catalogue. In `find` mode, enter the notes in a chord, riff or melody and it ranks the scales and modes that contain those pitch classes. In `list` mode, choose a tonic and scale to print the spelled notes, degrees, semitone pattern, step pattern, diatonic chords and related modes.

The lookup is deterministic and notation-aware. A search for `Eb G Bb` keeps the flat spelling in the results, while `D# G A#` finds the same pitch classes with sharp-root spellings. `fit=exact` is useful for pentatonic or symmetric note sets, `fit=near` allows one missing note when a passing tone makes the strict search too narrow, and `root` narrows the answer to one tonic when you already know the tonal centre.

Example CLI runs:

```bash
gizza tool scale-chord-finder notes="C E G B" max_results=3 output=names
# C major
# C harmonic-major
# G major

gizza tool scale-chord-finder action=list key=G scale=lydian chord_type=both output=text
```

Limits and edge cases:

- Notes are letters A-G with optional `#`, `b`, unicode accidentals or `x` for double sharp. Octave numbers such as `C4` are accepted and ignored.
- `notes` accepts up to 24 tokens, then deduplicates by pitch class because a scale search only needs the twelve pitch classes.
- `max_results` is 1-50. The full search still runs; the printed list is truncated after ranking.
- Diatonic chords are emitted for seven-note scales only. Pentatonic, blues, symmetric and chromatic scales still list notes and steps.
- This is a theory lookup, not audio analysis. It does not detect notes from a recording or name a chord from voicing/inversion.

## FAQ

<details>
<summary>Should I use find or list?</summary>

Use `find` when you have notes and want possible scales. Use `list` when you already know the key and scale name and want its spelling, degrees, steps and diatonic chords. `auto` picks `find` whenever `notes` is non-empty and otherwise lists the selected scale.

</details>

<details>
<summary>What does exact fit mean?</summary>

`fit=exact` keeps only scales whose pitch-class set is exactly the notes you entered. For example `C D E G A` returns major-pentatonic matches instead of seven-note major scales that contain two extra notes.

</details>

<details>
<summary>Why do some scales have no chords?</summary>

The chord rows stack thirds through a seven-note scale. Five-note, six-note, eight-note and chromatic collections do not have one unambiguous seven-degree third stack, so the tool prints their notes and reports that diatonic chords are only listed for seven-note scales.

</details>

<details>
<summary>Can I force sharps or flats?</summary>

Yes. `spelling=auto` uses notation-friendly spellings from the selected or inferred root. Choose `spelling=sharps` or `spelling=flats` when you need a chromatic spelling for lead sheets, code examples or CSV output.

</details>
