## About this tool

Mel spectrograms compress an audio clip into the time-frequency view used by many speech, music, keyword-spotting, and audio-classification pipelines. This tool decodes the audio locally, applies an STFT, maps FFT bins through triangular mel filters, scales the energy, and renders the result as a PNG image.

Worked example: upload a WAV voice clip, choose the “Speech 16 kHz compact” preset, and download the PNG. The output uses 80 mel bands, a 1024-sample FFT, a 256-sample hop, an 8 kHz high-frequency edge, and a 16 kHz analysis sample rate — a compact diagnostic view for speech models.

Limits: input audio is capped at 24 MiB and 4,000,000 decoded samples. Very short clips may need `center=true` or a smaller FFT. `width=0` and `height=0` use the natural matrix size: one pixel per frame and one row per mel band.

## FAQ

<details>
<summary>Is this the same as an MFCC extractor?</summary>

No. A mel spectrogram keeps the mel-band energy image before the DCT step. MFCC tools convert the log-mel bands into cepstral coefficients for tabular features; this tool renders the mel bands directly as a PNG for inspection or image-based pipelines.

</details>

<details>
<summary>Which settings match common audio-ML examples?</summary>

A common starting point is `n_fft=2048`, `hop_length=512`, `n_mels=128`, `mel_scale=slaney`, `scale=db`, and `center=true`, which mirrors the defaults many librosa-style examples use. Speech pipelines often resample to 16 kHz and use 64 or 80 mel bands.

</details>

<details>
<summary>Why are low frequencies at the bottom of the PNG?</summary>

That orientation matches the way spectrograms are usually read: time runs left to right and frequency rises from bottom to top. The summary reports the loudest mel-band frequency so you can sanity-check tones and hums.

</details>

<details>
<summary>What do `peak` and `full_scale` dB references change?</summary>

`peak` maps the loudest cell in the clip to the brightest color, which is useful for visual contrast. `full_scale` keeps the reference tied to digital full scale, so quiet files stay visibly dimmer and are easier to compare across clips.

</details>
