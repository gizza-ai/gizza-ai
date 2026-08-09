## About this tool

FFT Analyzer turns a pasted time-domain sample list into a frequency-domain report. It accepts real or complex samples separated by commas, semicolons, spaces or newlines, applies optional windowing and zero padding, and returns the bin frequency, level, phase, real and imaginary components, dominant peaks, bin resolution and Nyquist frequency.

Use it when you have measured samples, generated values, sensor readings, or a short DSP test vector and need to check which frequencies are present. Set the sample rate in hertz to get a real frequency axis, or leave it at `1` for normalised cycles-per-sample output. The default `amplitude` scale makes a unit cosine report an amplitude near `1`; `magnitude`, `normalized`, `db`, `power`, `csv`, `json`, and `chart` are available for debugging and export.

Worked example: paste `0, 1, 0, -1, 0, 1, 0, -1`, set `sample_rate` to `8`, and keep the default rectangular window. The strongest peak appears at bin `2`, frequency `2`, because the samples contain a two-cycle cosine over one second.

Limits: the tool accepts up to 65,536 input samples. `pad=pow2` zero-pads to the next power of two and uses an FFT; `pad=none` preserves exact length, but non-power-of-two exact transforms use a direct DFT capped at 4,096 samples. Zero padding adds frequency bins for interpolation, not extra physical resolution.

## FAQ

<details>
<summary>What is the difference between magnitude, normalized, amplitude and dB?</summary>

`magnitude` is the raw DFT bin length, so it grows with the number of samples. `normalized` divides by the sample count. `amplitude` is the one most people want for real signals: a cosine with amplitude `1` reports about `1` in the matching one-sided bin. `db` is `20·log10(amplitude)`, so a unit-amplitude tone is `0 dB`.

</details>

<details>
<summary>Should I use a window function?</summary>

Use `rectangular` when the signal contains an exact whole number of cycles in the sample window. If the tone falls between bins, spectral leakage spreads energy into neighbouring bins; `hann` or `hamming` usually gives a cleaner peak list, while `blackman`, `blackman-harris`, and `flattop` trade wider peaks for lower side lobes or more accurate amplitude readings.

</details>

<details>
<summary>Why does zero padding change the transform length but not the resolution?</summary>

Zero padding asks the FFT to evaluate more bin positions between the frequencies implied by the original data. That can make a peak easier to locate visually, but the true resolution still comes from the sample rate divided by the original observation duration. The report states `resolution = sample_rate / transform_length` for the computed grid and reminds you not to treat padding as new measurements.

</details>

<details>
<summary>When should I choose one-sided or two-sided output?</summary>

For real-valued input, positive and negative frequency bins mirror each other, so the default `auto` view shows the compact one-sided spectrum from DC to Nyquist. Complex input can carry different positive and negative frequencies, so `auto` switches to a two-sided table. You can force either view with the `spectrum` control.

</details>
