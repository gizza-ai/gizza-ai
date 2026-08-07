## About this tool

Paste an uncompressed **WAV** clip as base64 or hex and this tool decodes it into
**JSON**: the format metadata read from the file's `fmt` chunk — sample rate,
channel count, bit depth, encoding, format tag, byte rate, block align, total
frames and duration — alongside the decoded PCM samples as a JSON array you can
paste straight into JavaScript, Python, or a test fixture.

It is the JSON counterpart to a sample-data export: instead of a spreadsheet
table you get a machine-readable document, so a `fetch()` in the browser, a
`json.load()` in Python, or a unit test can consume the waveform directly.

You choose what the document contains:

- **Metadata + samples** (default) — an object with a `metadata` block, an
  `export` block describing exactly which frames were written, and `samples`.
- **Samples array only** — the bare array, ready to drop into code.
- **Format metadata only** — just the header fields, when you only need to know
  what the file *is*.

…how the samples are shaped (**interleaved** `L,R,L,R…` in one flat array, or
**per channel** as an array per channel), and how each value is written:

- **float** (default) — the normalized amplitude in `[-1, 1]`, independent of the
  source bit depth.
- **int** — the raw PCM integer at the source bit depth (a 16-bit source emits
  values in `-32768..32767`); float-WAV sources map to the 32-bit integer range.
- **db** — the dBFS magnitude, where `0` is full scale and silence reports a
  `-120` floor instead of `-Infinity` (which JSON cannot express).

Everything runs locally in your browser using WebAssembly — the audio bytes are
never uploaded.

### Worked example

The base64 in the placeholder is a 16 kHz, mono, 16-bit WAV whose three sample
frames are the raw integers `16384, -8192, 0`. With the defaults (metadata +
samples, interleaved, normalized float, 6 decimals, indent 2) it exports:

```json
{
  "metadata": {
    "sampleRate": 16000,
    "channels": 1,
    "bitDepth": 16,
    "encoding": "pcm-int",
    "formatTag": 1,
    "byteRate": 32000,
    "blockAlign": 2,
    "totalFrames": 3,
    "durationSeconds": 0.000188
  },
  "export": {
    "startFrame": 0,
    "frameStep": 1,
    "frameCount": 3,
    "valueScale": "float",
    "layout": "interleaved"
  },
  "samples": [0.500000, -0.250000, 0.000000]
}
```

Switch **JSON document** to `Samples array only` and **Sample value scale** to
`Raw PCM integer` and the same clip becomes just:

```json
[16384, -8192, 0]
```

For a stereo clip, **Sample layout** = `Per channel` de-interleaves it — the
left channel is `samples[0]`, the right is `samples[1]`:

```json
[
  [16384, 0],
  [-16384, 32767]
]
```

### Decimating a long clip

A full second of 44.1 kHz audio is 44 100 numbers per channel, which makes an
unusably large JSON blob for a waveform preview. **Keep every Nth frame**
(`frame_step`) strides through the clip instead: `441` gives about 100 points per
second. Combine it with **Decimal places** = `3` and **JSON indent** = `0` for a
compact preview array like `[0.000,0.250,0.500,0.750]`.

### Which WAV variants are decoded

RIFF/WAVE **PCM** at 8, 16, 24, or 32-bit integer and **IEEE float** at 32 or
64-bit are decoded, including `WAVE_FORMAT_EXTENSIBLE` files whose real codec is
in the SubFormat GUID. Compressed or companded audio — MP3, AAC/M4A, Ogg
(Vorbis/Opus), FLAC, and A-law / mu-law WAV — is rejected with a message naming
the format it detected, rather than producing garbage. Convert those to
uncompressed WAV first (for example `ffmpeg -i clip.mp3 clip.wav`).

### Limits and edge cases

- Input is a base64 or hex **string**, not a file upload. To export a `.wav`
  file, base64-encode it first (for example `base64 clip.wav`) and paste the
  text. Base64 text is about 33% larger than the binary it encodes.
- Export is capped at **200 000 frames** (`max_frames`, default 50 000) *after*
  step-decimation, so a long clip can't blow the in-browser memory budget — use
  **Start frame** to page through it in windows. The `export.frameCount` field
  tells you how many frames actually made it into the document.
- Sample arrays are always written on **one line**, even with indenting on. One
  value per line would turn a 50 000-frame export into a 50 000-line file.
- `durationSeconds` and `totalFrames` describe the whole clip, not the exported
  window — the `export` block covers the window.
- Peak-per-bucket (min/max) waveform reduction is not offered here; this tool
  strides, it does not summarize.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. -->

<details>
<summary>How do I turn my .wav file into base64 to paste here?</summary>

There is no file upload — the tool takes a base64 or hex **string**. On macOS or
Linux run `base64 clip.wav` (or `xxd -p clip.wav` for hex) and paste the output.
On Windows PowerShell, use
`[Convert]::ToBase64String([IO.File]::ReadAllBytes("clip.wav"))`. Then set
**Input encoding** to match what you pasted.

</details>

<details>
<summary>What exactly is in the metadata block?</summary>

The fields read straight from the WAV `fmt ` chunk — `sampleRate`, `channels`,
`bitDepth`, `formatTag` (`1` for integer PCM, `3` for IEEE float), `byteRate` and
`blockAlign` — plus `encoding` as a readable string (`pcm-int` or `ieee-float`)
and two derived values, `totalFrames` and `durationSeconds`, for the whole clip.
Choose **Format metadata only** to get that object on its own.

</details>

<details>
<summary>What is the difference between the float, int, and dB value scales?</summary>

**float** writes the normalized amplitude in `[-1, 1]`, so the numbers are
comparable across clips regardless of bit depth. **int** writes the raw PCM
integer at the source bit depth (a 16-bit file gives `-32768..32767`), which is
what you want to round-trip the exact stored samples. **db** writes the dBFS
magnitude, where `0` dB is full scale; true silence reports a `-120` floor
because JSON has no way to represent negative infinity.

</details>

<details>
<summary>How do I get one array per channel instead of interleaved samples?</summary>

Set **Sample layout** to `Per channel`. A flat interleaved array stores a stereo
clip as `L, R, L, R, …`; the per-channel layout gives you `[[…left…],
[…right…]]`, so `samples[0]` is the left channel and `samples[1]` the right. A
mono clip still gets a single nested array, so code that indexes by channel keeps
working.

</details>

<details>
<summary>How do I export just part of a long recording?</summary>

Use **Start frame**, **Keep every Nth frame**, and **Max frames**. One frame is
one sample per channel, so at 44.1 kHz frame 44100 is one second in. The export
starts at `start_frame` and steps forward by `frame_step` until it has written
`max_frames` frames or run off the end of the clip, whichever comes first. The
`export` block in the output echoes all three back plus the real `frameCount`.

</details>

<details>
<summary>Why does my MP3 or FLAC file not work?</summary>

Only uncompressed RIFF/WAVE is decoded. Compressed containers and codecs (MP3,
AAC/M4A, Ogg Vorbis/Opus, FLAC) and companded A-law / mu-law WAV are rejected
with a message naming the format that was detected, so you know what you pasted.
Convert to plain PCM WAV first, e.g. `ffmpeg -i clip.mp3 clip.wav`.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The decoder is compiled to WebAssembly and runs entirely in your browser tab.
The WAV bytes you paste never leave your device, and there is no sign-up or
server request.

</details>
