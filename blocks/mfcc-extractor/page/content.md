## About this tool

MFCC Extractor turns an audio file into Mel-frequency cepstral coefficients: the compact frame-by-frame feature matrix used by speech recognizers, speaker-ID systems, keyword spotting models, and many audio classifiers. Upload WAV, FLAC, MP3, M4A, OGG, WebM, or another supported audio container and choose whether the result should be CSV, TSV, or JSON metadata plus matrix rows.

The default settings follow the classic speech-feature pipeline: 25 ms frames, 10 ms hop, 13 coefficients, 26 mel filters, 0.97 pre-emphasis, HTK mel spacing, a lifter of 22, and log frame energy in C0. Preset chips switch to a librosa-style setup, add delta and delta-delta features, or widen the analysis for music-like audio.

### Worked example

For a short speech clip, keep the Speech / ASR defaults and upload the audio. The CSV output starts with a header such as:

```text
time_s,c0,c1,c2,c3,c4,c5,c6,c7,c8,c9,c10,c11,c12
0.000,-4.812345,2.103456,...
```

Each row is one complete analysis frame. `time_s` is the frame start time; turn it off when you need only the numeric coefficient matrix. Set `output=json` when you also want the resolved sample rate, frame length, hop length, FFT size, mel scale, and truncation flags.

### Limits and edge cases

- Input audio bytes are capped at 24 MiB before decoding.
- The decoder analyzes at most 4,000,000 mono samples and reports truncation in JSON output.
- Output is capped at 200,000 frames so tiny hops do not create oversized CSV files.
- Frames are taken from sample 0 with no centering, reflect padding, or zero-padded tail frame.
- DCT-II with orthonormal scaling is used for the cepstral transform; DCT-I/DCT-III and CMVN are separate post-processing steps.
- The log stage uses the natural log of mel energies, not librosa's dB-scaled `power_to_db` reference/max floor.
- Opus, AC-3, and DTS are not decoded by the pure-Rust audio stack; use WAV, FLAC, MP3, AAC/M4A, ALAC, Vorbis/OGG, or PCM-style formats for portable results.

## FAQ

<details>
<summary>What MFCC settings should I use for speech recognition?</summary>

Start with the defaults: 13 coefficients, 26 mel filters, 25 ms frames, 10 ms hop, HTK mel scale, pre-emphasis 0.97, lifter 22, and log energy in C0. If your files have mixed sample rates, set `resample_hz` to 16000 so matrices are comparable.

</details>

<details>
<summary>Why does this not match librosa exactly?</summary>

The defaults are speech-toolkit defaults, while librosa centers frames, uses a Hann window, Slaney mel normalization, larger sample-window settings, and dB-scaled mel power by default. Use the Librosa-style preset to get closer, but this tool still frames from sample 0 and uses natural-log mel energies, so exact equality is not expected.

</details>

<details>
<summary>What do delta and delta-delta columns mean?</summary>

Delta columns estimate how each coefficient changes over nearby frames. Delta-delta columns estimate the change of those deltas. They are common features for acoustic models because they add short-term motion information without changing the original audio.

</details>

<details>
<summary>Can I feed stereo or music files?</summary>

Yes. Stereo and multichannel files are downmixed to mono before analysis. For music-like audio, try more coefficients and filters, a longer frame, Slaney mel scale, and a wider `fmax`; the Wide-band music preset fills those fields.

</details>
