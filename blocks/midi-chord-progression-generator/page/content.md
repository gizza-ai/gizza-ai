## About this tool

Chord progressions are easy to type and annoying to turn into a file. This tool converts symbols such as `C G Am F`, `Cmaj7 | Dm7 G7 | Cmaj7`, `C/E`, `Gsus4` and `Bbmaj7` into a real Standard MIDI File you can download and drop into a DAW, notation app or sequencer.

The default makes one four-beat block chord per symbol at 120 BPM. Add `:beats` to a chord when a single slot should last a different length, for example `C/E:2 F:2 G:4`. Use `R` or `rest` for silence. The parser supports common triads, sevenths, sixths, ninths, elevenths, thirteenths, sus chords, diminished and augmented chords, sharps/flats and slash-bass chords.

Voicing controls shape the harmony before the file is written. **Close** keeps notes compact; **Drop 2**, **Drop 3** and **Spread** open the chord out. Inversions can be fixed, or **Smooth nearest voice-leading** can choose the inversion closest to the previous chord. **Double the bass note** adds a lower octave for piano-pad sketches, and **Transpose** moves the whole output up or down.

Timing controls decide how the notes are laid out: block chords, arpeggio up/down/up-down, or a light strum. The arpeggio step controls how quickly notes are repeated inside the chord slot, and note length controls the gate. Tempo, beats per bar, velocity and General MIDI instrument are written into the MIDI file so most players open it with the same feel you preview here.

The output box shows a short summary, the voiced notes for each slot and a **Download .mid** button. The file is generated locally in WebAssembly; nothing is uploaded.

Limits and edge cases:

- This is a chord-symbol generator, not an audio renderer. It creates MIDI note events; the sound depends on the synth or DAW that opens the file.
- Chord spelling is practical rather than exhaustive. Use common symbols such as `m`, `7`, `maj7`, `m7`, `dim`, `aug`, `sus2`, `sus4`, `6`, `9`, `11` and `13`.
- Maximum input is 64 KiB and 512 chord slots, with a 20,000-note output cap.
- The file is format-0 Standard MIDI with one track, one General MIDI program and quarter-note-based timing.
- Smooth voice-leading is heuristic: it minimizes note movement between neighboring chords, but it is not a full arranging engine.

## FAQ

<details>
<summary>Can I use bars or line breaks in the progression?</summary>

Yes. Spaces, commas, bars and line breaks all separate chord slots, so `C G Am F`, `C | G | Am | F` and one chord per line are equivalent. A `:beats` suffix such as `C:2` overrides the default length for that one slot.

</details>

<details>
<summary>Does it make audio?</summary>

No. The download is a `.mid` file containing tempo, program-change and note events. Open it in a DAW, notation app, browser synth or media player to hear it, then choose any sound you want there.

</details>

<details>
<summary>What does Smooth inversion do?</summary>

It tries each inversion of the current chord and picks the one nearest to the previous voiced chord. That keeps repeated progressions from jumping around as much as fixed root-position voicings, while still staying deterministic and fast enough for WebAssembly.

</details>

<details>
<summary>How are slash chords handled?</summary>

A slash chord such as `C/E` uses the chord before the slash for the upper notes and places the slash note in the bass when possible. If you also enable bass doubling, that bass is doubled an octave lower.

</details>

<details>
<summary>Why does my chord symbol fail?</summary>

The parser accepts common pop, jazz and theory shorthand, but not every editorial notation system. Try a simpler spelling (`Bbmaj7` instead of a fully annotated symbol), remove parenthesized alterations, or split complex voicings into the closest supported chord quality.

</details>
