## About this tool

Paste an uncompressed **WAV** clip as base64 or hex and this tool decodes its PCM
samples and writes them to **CSV** — one column per audio channel, optionally
preceded by a time (seconds) or sample-index column. It is the online equivalent
of a "sample data export": turn the raw waveform into a numeric table you can open
in a spreadsheet, load into pandas/NumPy, or feed to a signal-processing or
machine-learning pipeline.

You choose how each sample value is written:

- **float** (default) — the normalized amplitude in `[-1, 1]`, independent of the
  source bit depth.
- **int** — the raw PCM integer at the source bit depth (a 16-bit source emits
  values in `-32768..32767`); float-WAV sources map to the 32-bit integer range.
- **db** — the dBFS magnitude, where `0` is full scale and silence reports a
  `-120` floor instead of negative infinity.

You also control the decimal precision, the delimiter (comma for CSV, semicolon,
or tab for TSV), whether to write a header row, and a `start_frame` / `max_frames`
window so a long recording exports only the slice you want. Everything runs
locally in your browser using WebAssembly — the audio bytes are never uploaded.

### Worked example

The base64 above is a 16 kHz, mono, 16-bit WAV whose first three sample frames are
the raw integers `16384, -8192, 0`. With the defaults (time index, float values,
comma delimiter, header on) it exports:

```
time_s,channel_1
0.000000,0.500000
0.000063,-0.250000
0.000125,0.000000
```

Switch **Sample value scale** to `int` and **Index column(s)** to `sample` and the
same clip becomes:

```
sample,channel_1
0,16384
1,-8192
2,0
```

### Which WAV variants are decoded

RIFF/WAVE **PCM** at 8, 16, 24, or 32-bit integer and **IEEE float** at 32 or
64-bit are decoded. Compressed or companded audio — MP3, AAC/M4A, Ogg (Vorbis/
Opus), FLAC, and A-law / mu-law WAV — is rejected with a message naming the format
it detected, rather than producing garbage. Convert those to uncompressed WAV
first (for example `ffmpeg -i clip.mp3 clip.wav`).

### Limits and edge cases

- Input is a base64 or hex **string**, not a file upload. To export a `.wav`
  file, base64-encode it first (for example `base64 clip.wav`) and paste the text.
  Base64 text is about 33% larger than the binary it encodes.
- Stereo and multichannel clips get **one column per channel** (`channel_1`,
  `channel_2`, …); the samples are de-interleaved for you.
- Export is capped at `max_frames` sample frames (default 100000, max 500000) so a
  long clip can't blow the in-browser memory budget — use `start_frame` to page
  through it in windows.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. -->

<details>
<summary>How do I turn my .wav file into base64 to paste here?</summary>

There is no file upload — the tool takes a base64 or hex **string**. On macOS or
Linux run `base64 clip.wav` (or `xxd -p clip.wav` for hex) and paste the output.
On Windows PowerShell, use
`[Convert]::ToBase64String([IO.File]::ReadAllBytes("clip.wav"))`. Then set **Input
encoding** to match what you pasted.

</details>

<details>
<summary>Which WAV formats are supported?</summary>

Uncompressed RIFF/WAVE only: PCM at **8, 16, 24, or 32-bit integer** and **IEEE
float at 32 or 64-bit**. Compressed containers and codecs (MP3, AAC/M4A, Ogg,
FLAC) and companded A-law / mu-law WAV are rejected with a clear message naming
the detected format — convert them to plain WAV first, e.g. `ffmpeg -i clip.mp3
clip.wav`.

</details>

<details>
<summary>What is the difference between the float, int, and dB value scales?</summary>

**float** writes the normalized amplitude in `[-1, 1]`, so the numbers are
comparable across clips regardless of bit depth. **int** writes the raw PCM
integer at the source bit depth (a 16-bit file gives `-32768..32767`), which is
handy when you need the exact stored samples. **db** writes the dBFS magnitude,
where `0` dB is full scale and true silence reports a `-120` floor instead of
negative infinity.

</details>

<details>
<summary>How do I export just part of a long recording?</summary>

Use **Start frame** and **Max frames**. One frame is one sample per channel, so at
44.1 kHz, frame 44100 is one second in. Set **Start frame** to where you want to
begin and **Max frames** to how many frames to write (up to 500000). The export
covers `start_frame … start_frame + max_frames`, clamped to the clip length.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The decoder is compiled to WebAssembly and runs entirely in your browser tab.
The WAV bytes you paste never leave your device, and there is no sign-up or server
request.

</details>
