## What this tool does

Paste an uncompressed RIFF/WAVE file as base64 or hex and this tool writes the decoded PCM samples as a real NumPy `.npy` v1.0 array. The output starts with the `\x93NUMPY` magic header and can be loaded directly with `np.load()` after you decode the returned base64 or hex string.

Use it when you need a small browser-safe bridge from WAV audio into Python, DSP notebooks, ML preprocessing, fixture generation, or regression tests. The decoder supports PCM integer WAVs (8, 16, 24, and 32 bit) plus IEEE-float WAVs (32 and 64 bit). Compressed containers such as MP3, AAC/M4A, FLAC, and Ogg are rejected with named errors instead of guessed.

Key controls:

- **Output dtype** — write normalized `float32`/`float64`, scaled `int16`/`int32`/`uint8`, or `auto` to keep the source dtype in the style of `scipy.io.wavfile.read`.
- **Array shape** — keep the usual `(frames,)` or `(frames, channels)` layout, force always-2D `(frames, channels)`, transpose to `(channels, frames)`, or flatten interleaved samples.
- **Frame windowing** — export a `start_frame` and `max_frames` slice when you only need a short fixture.
- **Info report** — inspect sample rate, shape, dtype, order, and byte counts before emitting the actual `.npy` text.

## Worked example

The placeholder WAV is a 16 kHz mono, 16-bit PCM clip with three samples. Leave the defaults and run it as **Base64 .npy** to get this exact output:

```text
k05VTVBZAQB2AHsnZGVzY3InOiAnPGY0JywgJ2ZvcnRyYW5fb3JkZXInOiBGYWxzZSwgJ3NoYXBlJzogKDMsKSwgfSAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAoAAAA/AACAvgAAAAA=
```

Decode it with:

```bash
printf '%s' 'k05VTVBZAQB2AHsnZGVzY3InOiAnPGY0JywgJ2ZvcnRyYW5fb3JkZXInOiBGYWxzZSwgJ3NoYXBlJzogKDMsKSwgfSAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgICAgIAoAAAA/AACAvgAAAAA=' | base64 -d > audio.npy
python3 - <<'PY'
import numpy as np
x = np.load('audio.npy')
print(x.dtype, x.shape, x.tolist())
PY
```

The array loads as `float32`, shape `(3,)`, with normalized values `[0.5, -0.25, 0.0]`.

## Limits and edge cases

- `.npy` stores the array only; it does **not** store WAV metadata such as sample rate. Use **Output as: Info report** and save the displayed sample rate beside your array.
- The decoded WAV input is capped at 32 MiB. Emitted `.npy` data is capped separately for base64 and hex output so the page stays responsive.
- `max_frames=0` means “to the end of the clip”; any positive value is capped at 1,000,000 frames.
- A mono downmix averages channels and is lossy. It is off by default so multichannel data is preserved.
- This is a pure WASM parser. Convert compressed audio to uncompressed WAV first with an audio conversion tool, then paste the resulting bytes here.

## FAQ

<details>
<summary>Can I load the result with NumPy directly?</summary>

Yes. Decode the base64 output to a file and run `np.load("audio.npy")`. The tool writes a standard NumPy `.npy` v1.0 header with dtype, shape, order, and raw array bytes.

</details>

<details>
<summary>Where is the sample rate stored?</summary>

It is not stored in `.npy`. SciPy returns `(sample_rate, data)` as two separate values for the same reason. Choose **Output as: Info report** to see the source sample rate and copy it into your notebook or sidecar metadata.

</details>

<details>
<summary>What does dtype auto mean?</summary>

`auto` keeps the source storage dtype: 8-bit PCM becomes `uint8`, 16-bit becomes `int16`, 24-bit and 32-bit integer become `int32`, and float WAVs stay `float32` or `float64`. This mirrors the common SciPy WAV reading convention.

</details>

<details>
<summary>Why does the tool reject MP3, FLAC, or Ogg input?</summary>

Those formats need codec decoding. This block is intentionally a deterministic WAV-to-array converter with no ffmpeg or external codec runtime. Convert compressed audio to WAV first, then use this tool to write the `.npy` array.

</details>
