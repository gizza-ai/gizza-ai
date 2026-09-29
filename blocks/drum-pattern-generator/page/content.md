## About this tool

Drum Pattern Generator creates a complete drum groove from musical controls instead of asking you to draw every step by hand. Pick a genre, meter, bar count and feel, then export the result as General MIDI base64, rendered WAV base64, JSON, or a text report with an ASCII step grid.

The default call generates a two-bar rock groove with kick, snare, hats and crash. `tempo=0` uses the genre's typical BPM; set an explicit tempo when you need a fixed project tempo. `complexity`, `swing`, `humanize`, `fill_every`, `velocity`, `kit` and `seed` let you shape the feel while keeping the output reproducible.

Example CLI runs:

```bash
gizza tool drum-pattern-generator genre=rock bars=1 preview=off output=grid

gizza tool drum-pattern-generator genre=trap complexity=busy humanize=15 seed=42 preview=off output=midi-base64
```

Limits and edge cases:

- `bars` is 1-64. The MIDI file always covers the full length.
- `tempo` is 0 for the genre default, or 20-300 BPM for an explicit tempo.
- WAV previews are rendered in pure Rust as 22.05 kHz mono 16-bit PCM and capped near 30 seconds; long patterns still keep full-length MIDI.
- `preview=off` skips WAV rendering. `output=wav-base64` requires `preview=drums`, `drums-and-click` or `click`.
- Some subdivision/time-signature combinations cannot divide the bar evenly; choose `auto`, `sixteenth` or `triplet-eighth` for odd and compound meters.

## FAQ

<details>
<summary>Is this an audio drum machine or a MIDI generator?</summary>

Both artifacts are generated. The MIDI output uses General MIDI percussion on channel 10, and the preview output is a small synthesized WAV render for listening or a click-track check. The text report also includes a step grid for CLI and chat surfaces.

</details>

<details>
<summary>How do I make the same groove again?</summary>

Keep the same parameters and `seed`. Humanize timing, velocity variation and synthesized noise are deterministic, so the same URL or CLI command reproduces byte-identical MIDI and WAV data.

</details>

<details>
<summary>Why does tempo default to zero?</summary>

`tempo=0` means "use the genre's typical tempo". For example drum and bass resolves much faster than lo-fi. Set a non-zero BPM when the groove must match a DAW session or video edit.

</details>

<details>
<summary>Can it export a normal file?</summary>

The tool returns base64 so the same result works in chat, CLI and browser pages. Set `output=midi-base64` or `output=wav-base64`, copy the text, decode it, and save the bytes as `.mid` or `.wav`.

</details>
