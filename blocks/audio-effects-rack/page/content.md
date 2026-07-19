## An effects rack in your browser

Pick an audio file and chain up to five classic effects in a single ffmpeg
pass — no upload, no account, everything runs locally in the browser tab. The
stages are applied in the usual musical signal order:

1. **Compression** — evens out loud/quiet swings (`none`, `light`, `medium`,
   `heavy`). This is loudness dynamics, not file-size compression.
2. **Chorus** — detuned doubling for width and shimmer (`none`, `light`,
   `deep`).
3. **Tremolo** — a rhythmic amplitude wobble at 0.1–20 Hz (`0` = off).
4. **Echo** — a single discrete repeat, 0–1000 ms delay (`0` = off). 250 ms is
   a quarter-second slapback.
5. **Reverb** — a sense of space (`none`, `room`, `hall`, `plate`).

Any stage left at `none` or `0` is skipped, so you only pay for what you use.
At least one stage has to be active — an all-off request is rejected instead of
wasting a re-encode on an unchanged file.

### Worked example

You have a dry vocal take, `vocal.wav`, that sounds flat and lifeless. Choose
**Compression → Medium** to bring it forward, **Chorus → Light** for a touch of
width, **Echo → 250 ms** for a slapback, and **Reverb → Hall** for space; leave
**Tremolo** at `0`. Keep the format on **mp3** and you get back `vocal-fx.mp3`
with the whole chain baked in. Too washed-out? Drop reverb to `room` and echo to
`0`. Want a lo-fi pulse instead? Set **Tremolo → 6 Hz** and **Reverb → Room**.

### What each stage does

- **Compression** — `light` for a gentle level-up, `heavy` to squash hard for a
  loud, upfront, "radio" sound. Heavier presets use a lower threshold, higher
  ratio and more make-up gain.
- **Chorus** — `light` is one gentle voice; `deep` is two detuned voices for a
  lush, wide shimmer. Great on thin synths, pads and backing vocals.
- **Tremolo** — a fixed 70% depth wobble; slow rates (2–4 Hz) pulse, faster
  rates (8–12 Hz) shimmer. Above ~20 Hz it stops sounding like tremolo.
- **Echo** — one clean repeat with a fixed decay. Short delays (80–150 ms)
  thicken; longer ones (300–500 ms) read as a distinct doubling.
- **Reverb** — `room` is tight and small, `hall` is big with a long tail,
  `plate` is bright and dense. It's a lightweight multi-tap approximation, so it
  runs the same everywhere without an impulse-response file.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works.
- Echo delay is capped at 1000 ms and tremolo at 20 Hz — beyond those the
  effects stop being musical and start adding artefacts.
- Tremolo can't run between 0 and 0.1 Hz (ffmpeg's `tremolo` rejects it); use
  `0` for off or `0.1` and up.
- Effects are additive and can push loud audio into clipping. If the result
  crackles, ease off compression make-up or lower the level first with
  audio-volume-adjust, then add effects.
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC), and
  any embedded album-art image stream is dropped.

## FAQ

<details>
<summary>In what order are the effects applied?</summary>

Always compression → chorus → tremolo → echo → reverb, the standard musical
signal chain: dynamics first, then modulation, then time and space. Compressing
before reverb keeps the tail smooth; reverberating last means the space wraps
the whole processed sound. The order is fixed so the result is predictable — you
switch stages on and off, not reorder them.

</details>

<details>
<summary>Can I use just one effect?</summary>

Yes. Leave every other stage at `none` or `0` and only the one you set is added
to the chain — e.g. reverb `hall` alone, or echo `250` alone. The only rule is
that at least one stage must be active; an all-off request is rejected so you
don't re-encode a file for no change.

</details>

<details>
<summary>Is this "compression" the same as making the file smaller?</summary>

No. Here **compression** means dynamic-range compression — it narrows the gap
between the loudest and quietest parts so the audio sounds more even and
upfront. It does not shrink the file; for smaller files pick a lossy format
(mp3/ogg/m4a) or use the separate audio-compress tool.

</details>

<details>
<summary>What's the difference between echo and reverb here?</summary>

Echo is one discrete repeat you can hear as a separate copy (set by the delay in
milliseconds). Reverb is many overlapping reflections blurred into a continuous
sense of space (room/hall/plate). Use echo for a slapback or doubling effect,
reverb to place the sound in a room.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file locally in the browser tab — the audio never leaves your device.

</details>
