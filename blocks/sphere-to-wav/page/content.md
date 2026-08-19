## About this tool

NIST SPHERE is the container almost every classic speech corpus ships in — TIMIT, Switchboard,
Fisher, WSJ, CALLHOME, TEDLIUM. A `.sph` file is a fixed-size block of ASCII header text followed
by raw interleaved samples, and nothing on a modern desktop plays it: the extension is often
`.wav` even though the bytes are not RIFF, the samples are frequently big-endian, and telephone
corpora store 8-bit mu-law or A-law instead of linear PCM.

This converter reads the header field by field, applies what it says, and writes a standard
RIFF/WAVE file you can open anywhere. Paste the `.sph` bytes as base64 or hex (or a `data:` URI),
pick what you want back, and the conversion runs locally in your browser through WebAssembly —
the recording never leaves the machine.

What it does with the header:

- `sample_byte_format` — `10` means big-endian, so every 16-bit sample is byte-swapped into the
  little-endian order WAV requires. `01` passes through untouched. Force it with **Byte order**
  when a corpus ships a wrong value and the result sounds like noise.
- `sample_coding` — `pcm` is copied, `ulaw` / `mu-law` and `alaw` are expanded to 16-bit PCM by
  G.711 decoding so ordinary players handle them.
- `channel_count` — two-channel conversation recordings can be split (`Channel 1 only` /
  `Channel 2 only`) or averaged into a mono downmix.
- `sample_count`, `sample_n_bytes` — used to locate and bound the sample data; a file whose
  declared sample count exceeds the bytes actually present is reported as truncated rather than
  quietly half-converted.

### Worked example

The demo file behind the “Convert to WAV” chip has this header, padded to 256 bytes and followed
by 40 bytes of samples:

```text
NIST_1A
    256
sample_rate -i 8000
channel_count -i 1
sample_n_bytes -i 2
sample_byte_format -s2 10
sample_coding -s3 pcm
sample_count -i 20
end_head
```

Its first sample pair on disk is `00 00 0f e0` — big-endian. With the defaults (16-bit PCM, all
channels, WAV container) the output is an 84-byte file: a 44-byte RIFF header plus the 40
byte-swapped sample bytes, returned as

```text
data:audio/wav;base64,UklGRkwAAABXQVZFZm10IBAAAAABAAEAQB8AAIA+AAACABAAZGF0YSgAAAAA…
```

Switch **Container** to `Raw, headerless samples` and **Return** to `Hex audio bytes` and you get
the samples on their own, now little-endian:

```text
0000e00fe11d58280b2e4c2e1329ff1e4111780184f146e36dd841d27fd13cd6eadf63ed0ffd130d
```

Switch **Return** to `Header report` and you get the parsed field table instead of audio: every
header line with its type token, then the derived sample rate, channel count, coding, byte order,
frame count and duration, plus what the conversion would produce (container, encoding, size). For
`Raw, headerless samples` the report ends with a ready-to-run re-import command such as
`ffmpeg -f s16le -ar 8000 -ac 1 -i out.raw out.wav`, because headerless audio carries none of
those parameters with it.

### Limits and edge cases

- Decoded input is capped at 6 MiB and produced audio at 12 MiB; the hex rendering is capped at
  4 MiB of audio because it doubles again as text. Use **Start sample frame** / **Max sample
  frames** to excerpt a long recording — one frame is one sample per channel, so seconds ×
  `sample_rate` gives the frame index.
- Shorten-compressed payloads (`sample_coding: pcm,embedded-shorten-v1.09` and friends) are
  detected and reported, not decoded. Decompress those with a desktop converter that bundles a
  shorten decoder, then convert the uncompressed file here.
- The sample rate is never changed — this tool re-containers audio, it does not resample.
- 8-bit output is written unsigned, because that is what WAV requires of 8-bit PCM; 16-, 24- and
  32-bit output stays signed little-endian.
- A header with no `sample_byte_format` and samples wider than one byte is an error rather than a
  guess; set **Byte order** explicitly.
- Files with a header size other than the usual 1024 bytes are fine — the size on line 2 is what
  is honoured.

## FAQ

<details>
<summary>Why does my .sph file already end in .wav, and why won't it play?</summary>

Several corpora (TIMIT most famously) name their SPHERE files `.WAV`. The extension is a lie: the
bytes begin with the ASCII magic `NIST_1A`, not `RIFF`. Players read the first four bytes, fail to
find a RIFF chunk, and refuse the file. Converting it here rewrites the container so the extension
and the contents finally agree.

</details>

<details>
<summary>Do I have to upload the recording anywhere?</summary>

No. The converter is a WebAssembly module that runs in the page, so the bytes you paste stay in
the browser tab. That is also why the input is base64 or hex text rather than a file picker, and
why the size caps are lower than a desktop tool's — everything happens in one sandboxed process.

</details>

<details>
<summary>What is the difference between "16-bit PCM" and "Keep the file's own encoding"?</summary>

`16-bit PCM` always produces linear signed 16-bit samples: mu-law and A-law corpora are expanded
by G.711 decoding, and 8-bit PCM is scaled up. That is the widest-compatibility option and the
default. `Keep the file's own encoding` preserves the original bit depth and companding, fixing
only what WAV strictly requires — byte order, and the unsigned convention for 8-bit PCM. A mu-law
file converted that way stays mu-law inside the WAV (format tag 7, with the `fact` chunk WAVE
requires), which halves the size but is not understood by every editor.

</details>

<details>
<summary>The audio came out as loud static. What went wrong?</summary>

Almost always the byte order. If a header claims `sample_byte_format 01` but the samples are
actually big-endian (or the field is missing and you guessed), every 16-bit sample is read with
its halves swapped, which sounds like harsh noise at the right duration. Set **Byte order** to the
opposite value and convert again — the duration and the header report stay identical, only the
samples change.

</details>

<details>
<summary>How do I convert just one side of a telephone conversation?</summary>

Two-channel corpora such as Switchboard and Fisher interleave the two speakers. Set **Channels**
to `Channel 1 only` or `Channel 2 only` to keep one side, which also halves the output size. Pick
`Mono downmix (average)` instead if you want both speakers mixed into a single track. Combine
either with **Start sample frame** and **Max sample frames** to pull out one turn of the
conversation.

</details>

<details>
<summary>Can it read a headerless file, or write AU/AIFF/SPHERE output?</summary>

No on both counts. A detached header (a separate file describing raw samples) needs a second
input, and this page takes one payload. For output, WAV and raw samples are supported; AU, AIFF
and re-wrapping back into SPHERE are not — the raw option plus the re-import command in the header
report covers the same ground for pipelines that need something else.

</details>
