## About this tool

A `.wav` file is a RIFF container: a short header, a `fmt ` chunk describing the
sample layout, often a `LIST`/`fact`/`cue ` chunk or two, and a `data` chunk
holding the actual audio. **Raw PCM** is that `data` chunk on its own — no
header, no metadata, just interleaved sample bytes. This tool walks the chunk
chain of an uncompressed WAV you paste as base64 or hex and hands back only the
payload.

With the defaults (**Sample format** = `Source`, **Channels** = `All`) the
payload is sliced out **byte-for-byte**: nothing is decoded and nothing is
re-encoded, so what you get is exactly what the container stored. Pick a
different sample format and each sample is decoded and rewritten — `u8`, `s16`,
`s24`, `s32` or 32-bit float, in little- or big-endian — which is the same
conversion `ffmpeg -f s16le` or `sox -t raw` would perform. Pick a channel
selection and the clip is downmixed to mono or split to one side.

Four output shapes:

- **Base64** (default) — decode it back to a `.pcm`/`.raw` file with
  `base64 -d > out.pcm`.
- **Hex bytes** — grouped by the bytes-per-line slider; set it to `0` for one
  unbroken run, which is what `xxd -r -p` expects.
- **C array** — a compilable `const unsigned char pcm_data[]` plus its length,
  for embedding a sound in firmware.
- **Format report** — the sample rate, channel count, encoding, frame count and
  chunk map, plus ready-to-run ffmpeg, SoX and Audacity re-import settings. Raw
  PCM carries no header, so those parameters have to travel with the bytes.

Everything runs locally in your browser using WebAssembly — the audio bytes are
never uploaded.

### Worked example

The base64 in the placeholder is a 16 kHz, mono, 16-bit WAV of three sample
frames (`16384, -8192, 0`). The file is 50 bytes; the payload is the last 6. With
the defaults the whole 44-byte header disappears and the output is:

```
AEAA4AAA
```

Switch **Output as** to `Hex bytes` and the same six bytes read:

```
00 40 00 e0 00 00
```

Switch **Sample format** to `u8 — unsigned 8-bit` and each 16-bit sample is
requantised to one byte (`16384` → `0.5` → `192` = `0xc0`):

```
c0 60 80
```

Choose `Format report + re-import commands` and nothing is dumped at all —
you get the description a headerless file can't carry:

```
Source WAV
  file bytes      50
  codec           PCM integer (format tag 0x0001)
  sample rate     16000 Hz
  channels        1
  bit depth       16-bit
  block align     2 bytes per frame
  total frames    3
  duration        0.000188 s
  chunks          fmt (16 B), data (6 B)
  data chunk      offset 44, 6 bytes

Extracted PCM
  encoding        s16le (signed 16-bit little-endian), verbatim payload
  channels        1 interleaved (as stored)
  frames          3 of 3 (index 0 - 2)
  bytes           6

Re-import (raw PCM has no header — state the format yourself)
  ffmpeg          ffmpeg -f s16le -ar 16000 -ac 1 -i out.pcm out.wav
  sox             sox -t raw -e signed-integer -b 16 -L -r 16000 -c 1 out.pcm out.wav
  Audacity        Import > Raw Data: Signed 16-bit PCM, Little-endian, 1 channel, 16000 Hz
```

### Embedding a clip in firmware

Set **Output as** to `C array`, pick the sample format your playback code
expects, and use the bytes-per-line slider to control the wrapping. A stereo
clip downmixed to mono at `s16le`, eight bytes per line, comes out as:

```c
/* raw PCM: s16le, 1 channel, 16000 Hz */
const unsigned char pcm_data[] = {
  0x00, 0x00, 0x00, 0x40
};
const unsigned int pcm_data_len = 4;
```

### Which WAV variants work

RIFF/WAVE **PCM** at 8, 16, 24 or 32-bit integer and **IEEE float** at 32 or
64-bit can be extracted and converted, including `WAVE_FORMAT_EXTENSIBLE` files
whose real codec sits in the SubFormat GUID. **A-law and mu-law** WAVs can be
extracted verbatim (`Sample format` = `Source`, `Channels` = `All`) but not
converted — their samples are companded, not linear PCM, and the error says so.
Compressed containers (MP3, AAC/M4A, Ogg Vorbis/Opus, FLAC, AIFF, big-endian
RIFX) have no `data` chunk to strip; they are rejected with a message naming the
format that was detected. Convert those first, e.g. `ffmpeg -i clip.mp3 clip.wav`.

### Limits and edge cases

- Input is a base64 or hex **string**, not a file upload. Encode the file first
  (`base64 clip.wav`, or `xxd -p clip.wav` for hex) and paste the text; base64
  text is about 33% larger than the binary it encodes.
- Output size is capped per format because each one inflates the payload
  differently: **6 MiB** of PCM for base64, **3 MiB** for hex, **1 MiB** for the
  C array. Over the cap you get an error naming the cap and the frame size — use
  **Start frame** / **Max frames** to take a window, or switch to base64.
- **Start frame** is measured in frames, not bytes or seconds: one frame is one
  sample per channel, so at 44.1 kHz frame 44100 is one second in. A start frame
  at or past the end is an error that reports the real frame count.
- `Channels` = `Right only` needs at least two channels; on a mono clip it errors
  rather than silently returning the left side.
- Any sample format other than `Source`, or any channel selection other than
  `All`, leaves the byte-for-byte path — conversion to a smaller depth is lossy,
  and a mono downmix averages the channels.
- Only the **first** `data` chunk is extracted. Trailing chunks after it, odd
  chunk sizes with their RIFF pad byte, and a wrong `block_align` in the `fmt `
  chunk are all handled; a file with no `fmt ` or no `data` chunk is an error.
- The tool never writes a WAV back. To play the result, re-import it with the
  settings printed by the format report.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. -->

<details>
<summary>How do I turn my .wav file into base64 to paste here?</summary>

There is no file upload — the tool takes a base64 or hex **string**. On macOS or
Linux run `base64 clip.wav` (or `xxd -p clip.wav` for hex) and paste the output.
On Windows PowerShell use
`[Convert]::ToBase64String([IO.File]::ReadAllBytes("clip.wav"))`. Then set
**Input encoding** to match what you pasted.

</details>

<details>
<summary>How do I turn the base64 output back into a .pcm file?</summary>

Copy the result and run `base64 -d > out.pcm` (paste, then Ctrl-D), or on
Windows `certutil -decode out.b64 out.pcm`. If you chose the hex output with
**Bytes per line** set to `0`, `xxd -r -p out.hex > out.pcm` does the same job.
The file has no header, so anything that plays it needs the sample rate, channel
count and encoding — the `Format report` output prints them.

</details>

<details>
<summary>Why is my raw PCM file silent, noisy, or the wrong speed?</summary>

Almost always a mismatch between the format the bytes really are and the format
the player was told. Raw PCM has no header to correct a wrong guess: the wrong
sample rate plays at the wrong speed, the wrong channel count swaps stereo into
alternating garbage, and the wrong bit depth or byte order turns audio into
static. Run the `Format report` output and copy the ffmpeg, SoX or Audacity
settings it prints verbatim.

</details>

<details>
<summary>What is the difference between "Source" and picking a sample format?</summary>

`Source` copies the `data` chunk out byte-for-byte — no decode, no re-encode, so
the output is bit-identical to what the file stored and the operation is
lossless. Choosing an explicit format (`s16le`, `u8`, `f32le`, …) decodes every
sample to an amplitude and rewrites it in that encoding, which is how you convert
a 24-bit file down to 16-bit or flip byte order. Converting to a smaller depth
loses precision; converting to the file's own format round-trips exactly.

</details>

<details>
<summary>Can I extract just a few seconds out of a long recording?</summary>

Yes — **Start frame** and **Max frames** cut a window. One frame is one sample
per channel, so multiply seconds by the sample rate: at 44.1 kHz, second 10 to
second 11 is start frame `441000`, max frames `44100`. Leave **Max frames** at
`0` to run to the end of the clip. Windowing is also how you get a long file
under the output size cap.

</details>

<details>
<summary>Why does my MP3, FLAC, or A-law file not work?</summary>

Only RIFF/WAVE files have a `data` chunk to strip. MP3, AAC/M4A, Ogg
Vorbis/Opus, FLAC, AIFF and big-endian RIFX are detected by their signature and
rejected by name, so you know what you actually pasted — convert them with
`ffmpeg -i clip.mp3 clip.wav` first. A-law and mu-law WAVs are a special case:
their payload can be extracted verbatim, but it cannot be converted to a linear
PCM format here, because those samples are companded rather than linear.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The extractor is compiled to WebAssembly and runs entirely in your browser
tab. The WAV bytes you paste never leave your device, and there is no sign-up or
server request.

</details>
