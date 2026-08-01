## About this tool

The **audio noise gate** quiets background noise that appears in the gaps between wanted sounds. Upload a voice note, podcast take, instrument stem, or field recording, set the level that should open the gate, and the tool uses ffmpeg's `agate` dynamics filter to push quieter material down while louder speech or music passes through.

This is useful when room tone, hiss, low hum, mouth noise, breaths, headphone bleed, or mic preamp noise becomes noticeable during pauses. It is a level-based gate: it does not learn a noise print, remove broadband hiss under speech, or shorten the file. The timing stays the same; quiet passages are simply attenuated.

Key controls:

- **Threshold** is the dBFS level a signal must exceed to open the gate. Lower values gate only very quiet noise; higher values clamp more aggressively.
- **Reduction** is how far below-threshold audio is pushed down. `30 dB` is a practical default; `80 dB` approaches silence.
- **Ratio** controls how steeply the gate pulls audio down below threshold.
- **Attack** controls how quickly the gate opens; fast attack preserves consonants and transients.
- **Release** controls how smoothly the gate closes; longer release avoids chattering.
- **Detection** chooses RMS for smoother average loudness or peak for sharper transient response.

All processing runs in the browser with WebAssembly and ffmpeg. The audio is not uploaded.

## FAQ

<details>
<summary>Is this the same as noise reduction?</summary>

No. A noise gate is level-based: it turns quiet sections down when they fall below a threshold. It works well for pauses between words or notes. Spectral noise reduction tries to remove hiss under the wanted sound, which is a different tool and can create artifacts if overused.

</details>

<details>
<summary>What threshold should I start with?</summary>

Start around `-35 dB` for spoken-word recordings. If quiet background noise still leaks through, raise the threshold toward `-30` or `-25`. If syllables or note tails get clipped, lower it toward `-45` or lengthen the release.

</details>

<details>
<summary>What does reduction do?</summary>

Reduction sets the closed-gate floor. `10–20 dB` makes gaps subtly quieter, `30 dB` is a strong cleanup, and `80 dB` is close to muting quiet passages. A reduction of `0 dB` would do nothing, so the tool rejects it.

</details>

<details>
<summary>Should I use RMS or peak detection?</summary>

Use **RMS** for speech, podcasts, and steady music because it follows average loudness smoothly. Use **peak** when short transients must open the gate quickly, such as percussion or sharp consonants.

</details>

<details>
<summary>Will this remove silence or shorten the audio?</summary>

No. The output keeps the same timeline and duration. Quiet gaps are attenuated but still present. Use a silence-removal or pause-shortener tool if you need to cut time out of a recording.

</details>
