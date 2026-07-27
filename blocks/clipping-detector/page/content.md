## About this tool

Digital clipping happens when audio samples hit full scale and flatten against the ceiling. A waveform can look mostly fine while a few short runs of full-scale samples create harsh clicks, crunch, or distortion. This tool decodes a short uncompressed WAV clip and reports exactly where those clipped samples occur.

Paste WAV bytes as **base64** (default) or **hex**. The detector supports uncompressed RIFF/WAVE PCM (8/16/24/32-bit integer) and IEEE float WAV (32/64-bit). It counts individual clipped samples, sample-frames with any clipped channel, the longest consecutive run, the overall peak in dBFS, and the worst regions with start/end timestamps.

### Worked example

The sample preset contains an 8 Hz mono WAV with three consecutive full-scale samples. Run it with the default `0.99` threshold and you will see one clipped region from `0:00.250` to `0:00.625`, `3 of 8` clipped samples, and a longest run of `3` frames. Set **Output** to `json` when you want structured counts and region objects for another script.

### Controls

- **Threshold** is the absolute sample magnitude that counts as clipped. `0.99` catches true 0 dBFS peaks with a small guard band; `1.0` only catches exact full-scale samples.
- **Minimum run length** filters out single-sample spikes. Audacity's classic clipping check uses `3` consecutive clipped samples.
- **Bridge gap** merges runs separated by a few clean frames so one distorted burst is not split into many tiny regions.
- **Worst regions** limits how many regions are listed, ranked by length and then by peak.

### Limits and edge cases

This is a sample scanner, not a decoder for every audio format. Convert MP3, AAC/M4A, FLAC, Ogg/Opus, A-law, or mu-law audio to uncompressed WAV first, then paste those bytes here. Very large audio should be clipped to the section you want to inspect before base64/hex encoding; the page is designed for short diagnostic clips. Stereo and multichannel files are scanned frame-by-frame: one clipped channel makes the frame count as clipped, while the sample count still counts every clipped channel sample.

## FAQ

<details>
<summary>What does "clipping" mean?</summary>

Clipping means a sample reached the maximum representable amplitude, so the waveform is flattened at the top or bottom instead of following the original shape. A few isolated full-scale samples may be harmless, but consecutive runs often mean audible distortion.

</details>

<details>
<summary>Why do I need WAV bytes instead of uploading an MP3?</summary>

This pure WebAssembly tool inspects decoded PCM samples directly. MP3/AAC/FLAC/Ogg need a codec decode step first, so they are rejected with a clear message rather than guessed at. Convert the file or the suspect section to uncompressed WAV, then paste its base64 or hex bytes.

</details>

<details>
<summary>Which threshold should I use?</summary>

Use `0.99` for a practical scan: it catches samples that are effectively full-scale even if a codec or export step nudged them slightly below 1.0. Use `1.0` when you only want mathematically exact full-scale samples. Lower values such as `0.95` are useful for finding near-clipping headroom problems.

</details>

<details>
<summary>What is a clipped region?</summary>

A region is a consecutive run of sample-frames where at least one channel is clipped. `min_run` decides how long the run must be before it is listed, and `gap` can bridge a few clean frames between clipped frames when one distorted burst is broken into fragments.

</details>
