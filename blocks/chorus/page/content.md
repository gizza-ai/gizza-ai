## Add chorus to audio in your browser

A chorus effect thickens a sound by mixing the dry signal with short delayed copies whose delay time
moves slightly. That tiny modulation makes a vocal, synth, guitar, or pad feel wider and richer than a
single track. This tool wraps ffmpeg's chorus filter with friendly controls instead of raw filter text.

Use **Voices** for how many delayed copies are added, **Delay** for the base delay time, **Depth** for
how far the modulation moves, **Rate** for how fast it moves, and **Voice level** for the wet delayed
copies. The page processes locally in your browser and exports mp3, wav, ogg, flac, or m4a.

## Worked example

For a plain mono guitar part, start with the **Classic Chorus** preset: 2 voices, 50 ms delay, 2 ms
depth, 0.4 Hz rate, and 0.4 voice level. For a wider synth pad, try **Lush Ensemble** with 4 voices,
55 ms delay, 5 ms depth, and a 0.55 voice level. For a moving shimmer, increase the rate above 2 Hz
and keep the level moderate.

## Control guide

- **Voices (2-4)**: more voices sound wider and denser.
- **Delay (20-80 ms)**: shorter values stay tighter; longer values become spacious.
- **Depth (1-8 ms)**: higher values make the pitch movement more obvious.
- **Rate (0.1-5 Hz)**: low rates drift slowly; high rates shimmer or wobble.
- **Voice level (0.1-0.9)**: higher values make the effect wetter and louder.

## Limits and edge cases

- Input files are capped at 10 MiB.
- Embedded album art or video streams are dropped from the output.
- The dry gain and output gain are fixed to avoid clipping; use the voice-level control for effect amount.
- ffmpeg's chorus filter has no feedback or separate dry/wet mix knob, so those are listed as out of scope.
- Output is re-encoded; choose wav or flac when you need a lossless output container.

## FAQ

<details>
<summary>What is the difference between voices and decay?</summary>

Voices controls how many delayed copies are mixed in. Decay controls the level of each copy. More
voices make the sound wider; more decay makes those voices louder and more obvious.

</details>

<details>
<summary>Why do the voices not all use the same delay and rate?</summary>

Each added voice is staggered by 8 ms and modulates at a slightly different rate. That deterministic
spread prevents copies from lining up perfectly and collapsing back into one thin-sounding delay.

</details>

<details>
<summary>Can I use this on vocals and guitars?</summary>

Yes. Subtle two-voice settings work well for vocals and clean guitars. Pads, synths, and ambient
textures can tolerate more voices, depth, and decay.

</details>

<details>
<summary>Does this upload my audio?</summary>

No. The page runs the ffmpeg WebAssembly runtime in your browser tab. Your audio stays local to the
browser while the effect is rendered.

</details>
