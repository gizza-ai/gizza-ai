## About this tool

**High-Pass Filter Audio** removes low-frequency content from a recording. A
high-pass filter (also called a *low-cut*) lets everything **above** a chosen
cutoff frequency pass through while attenuating everything below it — so rumble,
mains hum, HVAC drone, table thumps, plosives, and mic handling noise fall away
while speech and most instruments stay intact. Pick an audio file, set the cutoff
and how steeply it rolls off, choose an output format, and download the result.
Everything runs locally in your browser with a WebAssembly build of ffmpeg:
**your audio is never uploaded to a server.**

It only reduces low-frequency energy. Broadband hiss, mid/high noise, or one-off
sounds (a cough, a door slam) need a different tool — try the denoiser for steady
hiss/hum across the whole spectrum.

### Controls

- **Cutoff frequency (Hz)** — the corner frequency. Everything below it is cut,
  everything above passes. **80 Hz** is the standard rumble cut that clears
  low-end junk without thinning a voice; bump it to **100–120 Hz** for stubborn
  hum, or higher to thin the low end on purpose. Range **10–2000 Hz**, default
  **80**.
- **Rolloff (steepness)** — how sharply the filter cuts below the cutoff, in
  dB/octave. **6** is the gentlest and most transparent, **12** (default) is the
  natural choice for voice, **24** tightens the low end more, and **48** is a
  steep brick-wall cut that can start to thin the sound. Gentler slopes sound more
  natural; steeper slopes clear more low-end but can make voices sound thin.
- **Output format** — mp3 (192 kbps, default), wav, ogg, flac, or m4a.

### Worked example

Upload a spoken-word recording `interview.wav` with a constant air-conditioner
rumble, set **cutoff 100**, **rolloff 24 dB/oct**, and choose **wav**. The tool
runs `highpass=f=100:p=2,highpass=f=100:p=2` and returns
`interview-highpass.wav` — the same speech with the low drone gone. Want a smaller
file for the web? Switch the format to **mp3** and you get
`interview-highpass.mp3` at 192 kbps instead.

## FAQ

<details>
<summary>What is a high-pass filter, and when should I use one?</summary>

A high-pass filter passes frequencies **above** a set cutoff and attenuates those
**below** it — the opposite of a low-pass. Reach for it whenever a recording has
low-frequency junk you want gone: room rumble, air-conditioner or traffic drone,
50/60 Hz mains hum, desk thumps, footsteps, or the boominess of a close-mic'd
voice. It's the single most common first move when cleaning up voice, podcast, and
field audio.

</details>

<details>
<summary>What cutoff frequency should I pick?</summary>

Start at **80 Hz** — that clears most rumble without touching a voice, which lives
well above it. If low hum or drone remains, raise the cutoff in 10 Hz steps toward
**100–120 Hz**. For overhead/cymbal or bright-instrument tracks, a cutoff around
**120 Hz** trims rumble without dulling the tone. Keep it low (below ~120 Hz) on
material with real bass — a kick drum, bass guitar, or full music mix — so you
don't hollow out the low end you want to keep.

</details>

<details>
<summary>What does the rolloff / steepness setting do?</summary>

Rolloff is how sharply the filter attenuates frequencies below the cutoff, in
dB per octave. **6 dB/oct** is a gentle 1-pole slope that's very transparent;
**12 dB/oct** (the default) is a natural-sounding 2-pole slope good for voice;
**24** and **48 dB/oct** cascade more stages for a tighter, steeper cut. Steeper
slopes remove more low-end right below the cutoff but can start to sound thin or
unnatural — gentle slopes are safer on music, steeper ones are handy for aggressive
rumble control on speech.

</details>

<details>
<summary>Will this remove hiss or background chatter?</summary>

No. A high-pass only touches **low** frequencies. Steady broadband hiss (tape
hiss, mic self-noise) and mid/high noise pass straight through, and it can't
remove one-off events like coughs, clicks, or speech in the background. For steady
hiss/hum across the whole spectrum, use a spectral denoiser instead; this tool is
specifically for low-frequency rumble, hum, and handling noise.

</details>

<details>
<summary>Is my audio uploaded anywhere? What are the limits?</summary>

No. Processing happens entirely in your browser via a WebAssembly ffmpeg build —
the file never leaves your device. Practical limits: input up to about **10 MB**,
output up to about **10 MB**. Input can be any common audio format (mp3, wav, m4a,
ogg, flac); output is mp3, wav, ogg, flac, or m4a. Very long or high-bitrate files
may hit the size cap — trim or compress first.

</details>
