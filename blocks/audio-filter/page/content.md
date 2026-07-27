## About this tool

**Audio Filter** applies one of the four classic frequency filters to a recording:

- **Low-pass** — keeps everything **below** the cutoff frequency and attenuates the
  highs above it. Use it to soften harsh, sibilant, or hissy top end, or to make a
  clip sound muffled/underwater on purpose.
- **High-pass** (low-cut) — keeps everything **above** the cutoff and attenuates the
  lows below it. Use it to strip rumble, mains hum, HVAC drone, plosives, and mic
  handling noise.
- **Band-pass** — keeps only a **band** of frequencies centred on your frequency,
  as wide as the band width, and cuts everything outside it. A ~300–3400 Hz band is
  the classic telephone / walkie-talkie voice sound.
- **Notch** (band-reject) — the opposite of band-pass: it **removes** a narrow band
  centred on your frequency while everything else passes. Perfect for surgically
  killing a single tone like 50/60 Hz mains hum or a whistle.

Pick an audio file, choose the filter type, set the frequency (and, for band-pass or
notch, the band width), choose an output format, and download the result. Everything
runs locally in your browser with a WebAssembly build of ffmpeg: **your audio is
never uploaded to a server.**

### Controls

- **Filter type** — low-pass, high-pass, band-pass, or notch (see above).
- **Frequency (Hz)** — for **low-pass / high-pass** this is the corner (cutoff)
  frequency; for **band-pass / notch** it is the **centre** of the band. Range
  **20–20000 Hz**, default **1000**.
- **Band width (Hz)** — how wide the band is, for **band-pass and notch only**
  (ignored by low-pass and high-pass). A narrow width like **20** makes a surgical
  notch; a wide one like **3000** passes a broad band. Range **1–10000 Hz**, default
  **200**.
- **Output format** — mp3 (192 kbps, default), wav, ogg, flac, or m4a.

### Worked example

Upload a spoken-word recording `interview.wav` with a steady **60 Hz** mains hum.
Choose **Notch**, set **frequency 60**, **band width 20**, and pick **wav**. The tool
runs `bandreject=f=60:width_type=h:w=20` and returns `interview-filtered.wav` — the
same voice with the hum gone. Want the classic telephone sound instead? Switch to
**Band-pass**, **frequency 1000**, **band width 2800**, and you get
`interview-filtered.mp3` with only the mid-band voice passing through.

## FAQ

<details>
<summary>What's the difference between low-pass, high-pass, band-pass, and notch?</summary>

They differ in which frequencies they let through. A **low-pass** passes lows and
cuts highs above the cutoff. A **high-pass** passes highs and cuts lows below the
cutoff. A **band-pass** passes only a band of frequencies centred on your frequency
and cuts everything else. A **notch** (band-reject) does the reverse of a band-pass:
it removes just that band and passes everything else. Low-pass and high-pass are set
by a single cutoff; band-pass and notch also need a band width.

</details>

<details>
<summary>What frequency should I choose?</summary>

It depends on the filter and what you're targeting. To **cut rumble** with a
high-pass, start around **80–120 Hz**. To **tame harsh highs** with a low-pass, try
**3000–8000 Hz**. For a **telephone-style band-pass**, centre near **1000 Hz** with a
wide band (~2800 Hz) so roughly 300–3400 Hz passes. To **kill mains hum** with a
notch, set the frequency to your local mains frequency — **60 Hz** (Americas) or
**50 Hz** (most of Europe/Asia) — with a small band width. Frequencies must fall in
the **20–20000 Hz** audible range.

</details>

<details>
<summary>What does the band width do, and why is it greyed-out for low/high-pass?</summary>

Band width sets how wide the affected band is, in Hz, and it only applies to
**band-pass** and **notch** — those two filters act on a band centred on your
frequency. Low-pass and high-pass are defined by a single cutoff with no band, so
the width value is ignored for them. A **narrow** width (e.g. 20 Hz) makes a
surgical notch that removes just one tone; a **wide** width (e.g. 3000 Hz) passes or
rejects a broad range. Range **1–10000 Hz**, default **200**.

</details>

<details>
<summary>Can this remove hiss or background chatter?</summary>

Only if the unwanted sound sits in a specific frequency range. A steady tone like
mains hum or a whistle can be removed with a **notch**; broadband **hiss** and mid/
high **noise** overlap the frequencies you want to keep, so a simple filter will
dull the whole recording rather than cleanly remove them — a spectral denoiser is
the right tool there. One-off sounds (a cough, a click, a door slam) can't be
targeted by frequency at all.

</details>

<details>
<summary>Is my audio uploaded anywhere? What are the limits?</summary>

No. Processing happens entirely in your browser via a WebAssembly ffmpeg build —
the file never leaves your device. Practical limits: input up to about **10 MB**,
output up to about **10 MB**. Input can be any common audio format (mp3, wav, m4a,
ogg, flac); output is mp3, wav, ogg, flac, or m4a. Filtering re-encodes the audio,
so a lossless stream copy isn't possible. Very long or high-bitrate files may hit
the size cap — trim or compress first.

</details>
