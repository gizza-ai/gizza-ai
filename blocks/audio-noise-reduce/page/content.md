## About this tool

**Reduce Audio Background Noise** strips steady background hiss and hum — tape hiss,
air-conditioner drone, mic self-noise, mains buzz — out of a recording so speech and
music sit forward. Pick an audio file, set how hard to push the reduction, choose a
denoiser, and download the cleaned track. Everything runs locally in your browser with
a WebAssembly build of ffmpeg: **your audio is never uploaded to a server.**

It works best on *steady, constant* noise (a consistent hiss or hum that runs through
the whole clip). It is not an AI speech re-synthesizer — it won't remove one-off
sounds like a door slam or reconstruct a badly clipped voice.

### Controls

- **Reduction strength (1–100)** — how aggressively to cut noise. Higher removes more
  but can make the audio sound hollow or "underwater." Start around **12** and raise
  gradually until the hiss is gone but the voice still sounds natural. **Default: 12.**
- **Denoiser** — **afftdn** (FFT-based, fast, the default) handles steady hiss/hum
  well. **anlmdn** (non-local means) is slower but can be gentler on transients like
  consonants and cymbals.
- **Also cut low hum/rumble** — adds an 80 Hz high-pass stage that removes mains hum,
  HVAC rumble, and handling thumps on top of the denoise. **Default: off.**
- **Output format** — mp3 (192 kbps, default), wav, ogg, flac, or m4a.

### Worked example

Upload a noisy voice memo `interview.wav` with a constant air-conditioner hiss, set
**strength 40**, **afftdn**, tick **Also cut low hum/rumble**, and choose **wav**. The
tool runs `afftdn=nr=38.8` behind an 80 Hz high-pass and returns
`interview-denoised.wav` — the same speech with the hiss and low drone knocked well
down. Prefer a smaller file? Switch the format to **mp3** and you get
`interview-denoised.mp3` at 192 kbps instead.

## FAQ

<details>
<summary>What kind of noise can this actually remove?</summary>

It targets **steady, constant** background noise — a hiss, hum, or drone that runs
through the whole clip (tape hiss, mic self-noise, air-conditioner or fan noise, mains
buzz). It is a spectral denoiser, not an AI restorer, so it won't cleanly remove
one-off events like a cough, a door slam, or street chatter, and it can't rebuild a
distorted or heavily clipped voice.

</details>

<details>
<summary>What strength should I use?</summary>

Start low. **12** is a safe default that takes the edge off most hiss without touching
the voice. Raise it in steps and listen: somewhere around **30–50** clears heavier
noise, but push too far and the audio starts to sound hollow, watery, or "underwater"
as the filter eats real signal along with the noise. If it sounds robotic, back off.

</details>

<details>
<summary>afftdn or anlmdn — which denoiser?</summary>

**afftdn** (the default) is an FFT/spectral denoiser: fast and good at steady
broadband hiss and hum. **anlmdn** is a non-local-means denoiser: slower, but it can
be gentler on transients (crisp consonants, cymbal hits), so try it if afftdn makes
speech sound lispy or smears music. For most voice recordings afftdn is the right
first choice.

</details>

<details>
<summary>What does "Also cut low hum/rumble" do?</summary>

It prepends an **80 Hz high-pass filter** before the denoise, which removes
low-frequency energy — 50/60 Hz mains hum, HVAC rumble, table thumps, and handling
noise. Leave it **off** for music with real bass you want to keep; turn it **on** for
spoken-word recordings where nothing useful lives below 80 Hz.

</details>

<details>
<summary>Is my audio uploaded anywhere? What are the limits?</summary>

No. Processing happens entirely in your browser via a WebAssembly ffmpeg build —
the file never leaves your device. Practical limits: input up to about **10 MB**,
output up to about **10 MB**. Input can be any common audio format (mp3, wav, m4a,
ogg, flac); output is mp3, wav, ogg, flac, or m4a. Very long or high-bitrate files
may hit the size cap — trim or compress first.

</details>
