## Clean up a noisy video's audio

Pick a video, choose how hard to **reduce the background noise** in its audio,
and get the same clip back — clearer. The picture is **stream-copied** (not
re-encoded), so it stays byte-for-byte identical and processing is fast; only
the audio track is re-encoded. Everything runs in your browser; nothing is
uploaded.

### Choosing a strength

The **Strength** slider (1–100) controls how aggressively noise is removed:

- **Low (10–20)** — a gentle cleanup that shaves off steady hiss while keeping
  voices natural. This is the safe starting point.
- **Medium (30–50)** — a stronger reduction for noticeable air-conditioner hum,
  fan noise, or tape hiss.
- **High (60–100)** — maximum reduction. Effective on heavy noise, but push too
  far and quiet passages can start to sound hollow or "underwater".

Start low and raise it only until the noise is gone — over-processing is the
usual mistake.

**Worked example:** a talking-head clip recorded near a noisy laptop fan. Load
`interview.mp4`, set **Strength** to `20`, leave **Denoiser** on **FFT
(afftdn)**, and you get `interview-denoised.mp4` — the same video with the fan
hiss pulled down and the voice intact.

### Denoiser and hum removal

- **Denoiser** — **FFT (afftdn)** is the fast, general-purpose default and works
  well for steady hiss/hum. **Non-local means (anlmdn)** is slower but can hold
  onto fine detail on broadband noise; try it if afftdn sounds too smeared.
- **Also remove low hum/rumble** — adds a high-pass filter at 80 Hz to cut
  low-frequency mains hum, HVAC rumble and handling thumps. Leave it off if your
  audio has wanted bass (music, deep voices).

### Notes and limits

- The video stream is copied losslessly; the output keeps the same container
  (mp4 → mp4, webm → webm). WebM audio is re-encoded to Opus, everything else to
  AAC.
- This is a classic ffmpeg denoiser, not an AI "studio voice" model — it reduces
  steady background noise rather than isolating a single speaker.
- One setting is applied to the whole clip. To also raise the level afterward,
  run the result through the **Change a Video's Audio Volume** tool.
- Input and output are each capped at 25 MB (the file is processed in your
  browser's memory).

### FAQ

<details>
<summary>Is my video uploaded to a server?</summary>

No — ffmpeg runs inside your browser tab, so the file never leaves your device.

</details>

<details>
<summary>Will denoising hurt the video quality?</summary>

No. Only the audio is changed; the **picture is stream-copied without
re-encoding**, so the video quality is identical to the original.

</details>

<details>
<summary>What strength should I use?</summary>

Start around **12–20** and raise it only until the noise is gone. High values
(60+) remove more noise but can make quiet parts sound hollow or robotic, so
increase gradually and stop when it sounds clean.

</details>

<details>
<summary>What's the difference between the FFT and non-local means denoisers?</summary>

**FFT (afftdn)** works in the frequency domain and is fast — a good default for
steady hiss and hum. **Non-local means (anlmdn)** compares similar chunks of the
waveform and can preserve fine detail better on broadband noise, at the cost of
speed. If one sounds too smeared, try the other.

</details>

<details>
<summary>Can it remove a hum or buzz?</summary>

Turn on **Also remove low hum/rumble** to add an 80 Hz high-pass that cuts
mains hum, HVAC rumble and handling thumps. Combine it with a moderate strength
for the rest of the noise.

</details>

<details>
<summary>Which video formats can I use, and how big can the file be?</summary>

Anything ffmpeg can read — mp4, mov, mkv and webm are the common cases. The
output keeps the input's container and is named after the original with a
`-denoised` suffix (e.g. `clip.mp4` → `clip-denoised.mp4`). The input and output
are each capped at 25 MB.

</details>
