## About this tool

Raw PCM is audio with the label torn off. A `.pcm`, `.raw` or `.bin` dump — what you get out of a
microphone driver, a DSP trace, an embedded recorder, a speech-synthesis buffer or an
`ffmpeg -f s16le` pipe — is nothing but sample values back to back. There is no magic number, no
sample rate, no channel count, no statement of how wide a sample is or which way round its bytes
go. Every player that refuses to open the file is refusing for the same reason: those five facts
are missing.

This tool supplies them. Paste the bytes as base64 or hex (or a `data:` URI), say what they are,
and get a standard RIFF/WAVE file back — the 44-byte canonical header for integer PCM, or the
18-byte `fmt ` chunk plus the `fact` chunk WAVE requires for float and G.711 data. Nothing is
decoded and nothing is resampled: the samples you paste are the samples in the file. The only
edits are the two rules WAVE will not bend on — multi-byte samples are stored little-endian, and
8-bit PCM is stored unsigned while every wider integer depth is stored signed.

What to set:

- **Sample rate** — the rate the dump was recorded at. Guess it wrong and the file plays, just at
  the wrong speed and pitch. Nothing in the bytes can tell you; 8000 and 16000 are typical for
  speech and telephony, 44100 for CD-derived material, 48000 for anything from video.
- **Channels** — 1 for mono, 2 for interleaved stereo (samples alternate L, R, L, R). Higher
  counts up to 16 are accepted for multichannel captures.
- **Bit depth** and **Sample encoding** — the pair that ffmpeg fuses into one token: `s16le` is
  16-bit signed, `u8` is 8-bit unsigned, `f32le` is 32-bit float. Invalid pairings are rejected
  rather than guessed: 64-bit is float-only, and float exists only at 32 and 64 bits.
- **Byte order** — little-endian for anything produced on a PC or phone. Big-endian data
  byte-swapped into WAV order is the usual reason a conversion that "worked" sounds like static.
- **Skip leading bytes** / **Max sample frames** — drop a proprietary header in front of the
  samples, or wrap a window instead of the whole dump.

### Worked example

The demo behind the “CD-style stereo dump” chip is a 32-byte capture — 8 stereo frames of 16-bit
signed little-endian PCM:

```text
00000000 001000f0 002000e0 003000d0 004000c0 005000b0 006000a0 00700090
```

At 44100 Hz, 2 channels, 16-bit signed little-endian, the result is a 76-byte WAV: a 44-byte
header followed by those 32 bytes copied verbatim.

```text
data:audio/wav;base64,UklGRkQAAABXQVZFZm10IBAAAAABAAIARKwAABCxAgAEABAAZGF0YSAAAAAAAAAAABAA8AAgAOAAMADQAEAAwABQALAAYACgAHAAkA==
```

Switch **Return** to `Hex WAV bytes` to read the header field by field — `52494646` is `RIFF`,
`0100` the PCM format tag, `0200` the channel count, `44ac0000` the 44100 Hz rate, `10b10200` the
176400 bytes/s byte rate, `0400` the block align, `1000` the 16 bits per sample, and `64617461`
the `data` chunk your samples begin after.

Switch it to `Header report` instead and you get the arithmetic rather than the audio:

```text
  frame size       4 bytes (2 ch x 2 bytes)
  usable           32 bytes = 8 sample frames
  leftover         0 bytes (the data divides evenly into frames)
  format tag       1 (WAVE_FORMAT_PCM)
  byte rate        176400 bytes/s
  file size        76 bytes (76 B)
  ffmpeg           ffmpeg -f s16le -ar 44100 -ac 2 -i in.pcm out.wav
  sox              sox -t raw -r 44100 -b 16 -e signed-integer -L -c 2 in.pcm out.wav
```

The **leftover** line is the one to read when something is wrong. A dump whose byte count does not
divide evenly into frames has a bit depth or channel count that does not match the data, and that
is nearly always the real bug — the header report says so before you listen to noise.

### Limits and edge cases

- Decoded input is capped at 12 MiB and the produced WAV at 12 MiB; the hex rendering is capped at
  4 MiB of audio because it doubles again as text. Use **Max sample frames** to excerpt a longer
  dump — one frame is one sample per channel, so seconds × sample rate gives the frame count.
- Sample rate accepts 1–768000 Hz and channels 1–16.
- 8-bit signed input is converted to the unsigned form WAVE mandates (−128 becomes 0, 0 becomes
  128); unsigned input wider than 8 bits is converted the other way, to signed. That is a
  re-labelling of the same waveform, not a change to it.
- `encoding=float` accepts bit depth 32 or 64 only, and bit depth 64 is float-only — WAVE has no
  64-bit integer PCM. Both mistakes are errors naming the allowed values, not silent guesses.
- mu-law and A-law are always one byte per sample, so the bit depth control is ignored for them
  and the WAV keeps the companded bytes (format tag 7 and 6) rather than expanding them.
- The sample rate is never changed — this tool writes a header, it does not resample. There is no
  format auto-detection either: a detector would have to score twenty candidate layouts
  statistically, and a confident wrong answer is worse than asking.
- Compressed sources are out of scope. GSM, IMA/MS-ADPCM, MP3 frames and Opus packets are not
  linear PCM; wrapping them in a PCM header produces noise. Decode them first.

## FAQ

<details>
<summary>How do I find the sample rate and bit depth of a raw file?</summary>

They are not in the file — that is what "headerless" means. Get them from whatever produced the
dump: the `ffmpeg -f s16le -ar 44100` command in the pipeline, the recorder's configuration, the
device datasheet, or the API call that returned the buffer. Failing that, the file size is a
useful check: bytes ÷ (channels × bytes per sample) is the frame count, and dividing that by a
candidate rate gives a duration you can sanity-check against how long the recording should be.
The **Header report** does that arithmetic for you for whatever you type in.

</details>

<details>
<summary>The audio came out as loud static. What did I get wrong?</summary>

Byte order first: 16-bit samples read with their halves swapped sound like harsh noise at exactly
the right duration, so flip **Byte order** and convert again. If the duration is also wrong, the
bit depth or channel count is wrong instead — check the **leftover** line in the header report,
because a non-zero leftover means the data does not divide evenly into the frame size you
described. Signedness gets its own symptom: an 8-bit dump interpreted with the wrong signedness
plays the right waveform with a loud DC offset and clipping, not noise.

</details>

<details>
<summary>What is the difference between a sample, a frame and a byte here?</summary>

A **sample** is one value for one channel. A **frame** is one sample for every channel, so stereo
16-bit audio has 4 bytes per frame (2 channels × 2 bytes). Duration depends on frames, not
samples: 44100 frames is one second at 44.1 kHz regardless of the channel count. **Max sample
frames** counts frames for that reason, while **Skip leading bytes** counts raw bytes, because a
junk header rarely aligns to a frame boundary.

</details>

<details>
<summary>Does this change the audio quality, or re-encode anything?</summary>

No. The sample values are copied through untouched; only their container changes. The two
exceptions are pure re-labellings that WAVE requires: multi-byte samples are byte-swapped into
little-endian order, and 8-bit integer samples are stored unsigned. Nothing is resampled, no bit
depth is converted, no dithering is applied, and mu-law/A-law data stays companded instead of
being expanded to 16-bit.

</details>

<details>
<summary>My file has a header in front of the samples. Can I keep it out?</summary>

Yes — set **Skip leading bytes** to the size of that header and the wrapper starts after it.
That covers proprietary recorder headers, a stray text preamble, or a partially-stripped
container. If you do not know the size, wrap the file without skipping and look at the start of
the hex output: a run of ASCII text or a repeating structure before the samples begin tells you
how many bytes to drop.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The converter is a WebAssembly module that runs inside the page, so the bytes you paste stay
in the browser tab. That is also why the input is pasted base64 or hex rather than a file picker,
and why the size caps are lower than a desktop tool's — everything happens in one sandboxed
process. For dumps larger than the caps, the header report prints the exact `ffmpeg` and `sox`
commands that do the same job locally.

</details>
