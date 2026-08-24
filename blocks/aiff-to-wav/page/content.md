## Convert AIFF to WAV in your browser

AIFF (`.aiff`, `.aif`, `.aifc`) and WAV (`.wav`) are both containers for uncompressed linear PCM
audio. The intrinsic difference is byte order: AIFF stores samples big-endian, WAV little-endian.
So converting one to the other at a matching bit depth is a re-container, not a re-encode — the
decoded sample values come out bit-for-bit identical. This page does that with ffmpeg compiled to
WebAssembly, so the file never leaves your device.

One detail matters more than any option on this page: ffmpeg's WAV muxer falls back to 16-bit PCM
when no codec is given, which would silently truncate a 24-bit master. This tool therefore always
sends an explicit `-c:a`, and defaults to **24-bit**, a depth that preserves every bit of both
16-bit and 24-bit sources.

### Worked example

You have `session-vocal.aiff` bounced from a 24-bit / 48 kHz session and a video editor that wants
a WAV.

1. Leave **Output PCM encoding** on `24-bit integer PCM`.
2. Leave **Sample rate** on `Keep source rate` and **Channel layout** on `Keep source layout` —
   nothing is resampled or downmixed, so the conversion stays lossless.
3. Leave **Keep textual tags** checked so the title/artist/album strings ride along in the WAV's
   LIST/INFO chunk.
4. Upload the file and download `session-vocal.wav`.

The resulting argv is `-i in.aiff -vn -map_metadata 0 -c:a pcm_s24le out.wav`. If the target
instead demanded CD audio, picking `16-bit integer PCM`, `44100 Hz` and `Stereo` appends
`-ar 44100 -ac 2` and drops the depth to `pcm_s16le` — both of which *do* change the audio, which
is why neither is the default.

### Limits and edge cases

- **Only a matching depth is lossless.** Choosing a depth below the source's (a 24-bit master to
  `16-bit`) discards bits, and `A-law` / `mu-law` are 8-bit companded telephony encodings — heavy,
  irreversible quality loss. `32-bit` and `32-bit float` never lose source detail but roughly
  double the file size against a 16-bit original.
- **Resampling and downmixing are one-way.** `Keep source rate` and `Keep source layout` are the
  defaults for that reason. Set them only when a target system demands a specific rate or channel
  count; `Mono` sums the sides and cannot be undone, `Stereo` on a mono source just duplicates it.
- **WAV files can be bigger than the AIFF they came from.** At the default 24-bit depth a 16-bit
  source is widened 1.5×. The chat and CLI surfaces accept inputs up to 25 MiB and emit up to
  50 MiB; in the browser the practical ceiling is your device's memory.
- **Cover art is always dropped.** An embedded picture rides as a video stream and WAV has no
  standard picture chunk, so `-vn` removes it. Textual tags survive when **Keep textual tags** is
  on; unchecking it maps `-map_metadata -1` for a clean delivery file.
- **Classic WAV is capped near 4 GiB** by its 32-bit chunk sizes. Very long or very high-rate
  material can exceed that — trim or split it first.
- **One file per run, and no trimming.** There is no batch mode and no start/end range here; run
  the files one at a time, or trim beforehand.
- AIFF is the intended input, but the picker accepts any audio ffmpeg can decode, and ffmpeg
  probes the actual bytes rather than trusting the extension. `.aifc` (compressed AIFF) decodes
  fine and is written out as plain PCM.

## FAQ

<details>
<summary>Does converting AIFF to WAV lose any quality?</summary>

Not at a matching bit depth. Both formats carry uncompressed linear PCM and differ only in byte
order, so decoding the WAV gives the same sample values as the AIFF. Quality is only lost if you
choose a smaller depth than the source, resample, or downmix to mono.

</details>

<details>
<summary>Which bit depth should I choose?</summary>

Leave it on `24` unless something downstream demands otherwise — it preserves both 16-bit and
24-bit sources exactly. Choose `16` for CD-style delivery, `32-bit float` if the next tool in the
chain mixes in float, and `A-law` / `mu-law` only for telephony pipelines that require G.711.

</details>

<details>
<summary>Why is my WAV larger than the AIFF I uploaded?</summary>

Because the default output depth is 24-bit. A 16-bit source widened to 24-bit uses 1.5× the bytes
per sample even though it sounds identical. Pick `16-bit integer PCM` to match a 16-bit source
byte-for-byte.

</details>

<details>
<summary>Are the title, artist and album tags kept?</summary>

Yes, while **Keep textual tags** is checked — ffmpeg maps the source's textual metadata into the
WAV's LIST/INFO chunk. Uncheck it to strip everything for a clean delivery file. Embedded cover
art is dropped either way, since WAV has no standard picture chunk.

</details>

<details>
<summary>Can I convert several AIFF files at once?</summary>

No. This page converts one uploaded file per run. For a batch, drive the command-line tool from a
shell loop, or convert the files one at a time here.

</details>

<details>
<summary>Does it work with .aif and .aifc files?</summary>

Yes. `.aif` is the same format under the 3-character name older systems used, and `.aifc` is the
compressed AIFF variant — ffmpeg detects the real format from the file's bytes rather than its
extension, and both are written out as uncompressed PCM WAV.

</details>
