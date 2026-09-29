## About this tool

This tool generates a complete chord progression from musical controls instead of making you type chord symbols by hand. Choose a tonic, mode and style preset, then use `variation` as a deterministic re-roll: the same settings always produce the same Roman numerals, chord names, note spellings and MIDI bytes.

The default call gives a C major pop loop. Change the mode for modal harmony, use `sevenths` for jazzier chord thickness, and set `borrowed` to add modal-interchange colours such as iv, bIII, bVI and bVII when they fit the selected style. `chords=0` keeps each preset's natural length, while a positive chord count cycles or trims it to a fixed number.

MIDI settings are included even though the page output is text. `tempo`, `instrument`, `pattern`, `octave`, `repeats` and `voice_leading` shape the Standard MIDI File returned by `output=midi-base64`. Decode that base64 to a `.mid` file and open it in a DAW, notation program or browser synth.

Example CLI runs:

```bash
gizza tool chord-progression-generator key=C mode=major style=pop variation=1 output=chords
# C G Am F

gizza tool chord-progression-generator key=F mode=major style=jazz sevenths=extended tempo=92 instrument=electric-piano pattern=arpeggio-updown output=midi-base64
```

Limits and edge cases:

- Keys are offered as common sharp and flat tonic spellings. Enharmonic keys sound the same but affect note spelling.
- Modes are generated from scale degrees, so exotic notation and full harmonic analysis beyond the advertised Roman numerals are out of scope.
- `variation` is 1-99, `chords` is 0-32, `tempo` is 40-300 BPM, `repeats` is 1-8 and `octave` is 1-7.
- `output=midi-base64` returns the MIDI file as base64 text so it works in the same CLI/page/chat text surfaces as the analysis modes.
- The MIDI file contains General MIDI program and note events. It is not an audio render; the final sound depends on the synth that opens it.

## FAQ

<details>
<summary>How do I re-roll without losing reproducibility?</summary>

Change `variation`. It behaves like a seed: `variation=7` with the same key, mode, style and other options always returns the same progression and MIDI file.

</details>

<details>
<summary>What is the difference between style and mode?</summary>

`mode` defines the scale and chord qualities available from the key. `style` chooses progression templates or a random walk over those degrees, so C major pop and C major jazz share a scale but choose different harmonic patterns.

</details>

<details>
<summary>Can it make a downloadable MIDI file?</summary>

Yes. Set `output=midi-base64`, copy the returned base64, decode it to a `.mid` file, and open it in a DAW or notation app. The text output also reports how many MIDI bytes were generated.

</details>

<details>
<summary>What does borrowed chords do?</summary>

`borrowed=light` keeps the chromatic chords baked into styles such as rock, blues and metal. `borrowed=rich` adds extra modal-interchange colour where it fits, for example iv, bIII, bVI or a secondary dominant.

</details>

<details>
<summary>Does voice leading change the chord names?</summary>

No. Voice leading only changes how notes are inverted in the MIDI file so adjacent chords move more smoothly. The printed chord symbols and Roman numerals stay the same.

</details>
