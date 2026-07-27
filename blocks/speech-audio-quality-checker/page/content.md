## About this tool

Paste a short uncompressed WAV recording and this tool checks whether it is ready
for speech-to-text / ASR. It decodes the actual audio bytes in WebAssembly, then
reports the sample rate, channel count, duration, peak and RMS dBFS, an estimated
signal-to-noise ratio, clipped-sample percentage, longest clipped run, and an
overall readiness verdict.

Use it before sending a representative clip to a transcription system. The
default thresholds match common ASR guidance: 16 kHz or better, mono preferred,
about 20 dB or better SNR, and no more than 1% clipped samples. You can tighten or
relax those thresholds when your pipeline has different requirements.

### Worked example

The built-in example is a tiny 16 kHz mono PCM WAV with loud speech-like frames
and quiet background frames. With default settings, the report includes:

```text
[PASS] Sample rate
[PASS] Channels
Verdict: READY for ASR / transcription
```

Switch **Output format** to `json` when you need machine-readable metrics such as
`sample_rate`, `snr_db`, `clipping_pct`, and `verdict`.

### Limits and edge cases

- This pure browser tool decodes uncompressed RIFF/WAVE only: PCM 8/16/24/32-bit
  integer and IEEE-float WAV. Convert MP3, AAC/M4A, FLAC, Ogg/Opus, A-law, or
  mu-law to PCM WAV first.
- Paste short representative clips rather than multi-hour recordings; base64 text
  grows about 33% larger than the binary file.
- SNR is a percentile estimate from 20 ms frame levels, not a voice-activity or
  perceptual speech-quality model.
- Everything runs locally in the browser; the audio text you paste is not
  uploaded.

## FAQ

<details>
<summary>What audio formats does this checker support?</summary>

It supports uncompressed RIFF/WAVE files: PCM 8-bit, 16-bit, 24-bit, and 32-bit
integer WAV, plus IEEE 32-bit and 64-bit float WAV. Compressed containers such as
MP3, AAC/M4A, FLAC, Ogg/Opus, A-law, and mu-law are rejected with a clear message
so the result is not guessed.

</details>

<details>
<summary>How is SNR estimated?</summary>

The tool downmixes the clip to mono, splits it into 20 ms frames, measures each
frame's RMS level, and subtracts the 10th-percentile level from the 90th-percentile
level. That gives a useful noise-floor estimate for quick preflight checks, but it
is not a true speech/non-speech SNR because no voice-activity model runs here.

</details>

<details>
<summary>Why does stereo audio warn instead of fail?</summary>

Most ASR systems downmix to mono internally. Stereo usually increases file size
without improving transcription accuracy, so the checker marks it as a warning:
still usable, but worth downmixing before batch transcription.

</details>

<details>
<summary>Can I upload a .wav file directly?</summary>

This generic page uses pasted base64 or hex text rather than a file picker. Run a
command such as `base64 clip.wav`, paste the output into the input box, and leave
**Input encoding** set to `base64`.

</details>
