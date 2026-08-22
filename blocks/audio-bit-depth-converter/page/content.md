## Change audio bit depth in your browser

Bit depth is how many bits each PCM sample uses — the *word length* of your
audio. It sets the dynamic range: roughly 6 dB per bit, so 8-bit gives about
48 dB, 16-bit about 96 dB (the CD and streaming standard) and 24-bit about
144 dB (the studio standard). This tool requantizes an audio file to the depth
you pick and writes lossless **WAV** or **FLAC**. Everything runs locally in
your browser tab via ffmpeg compiled to WebAssembly, so your audio is never
uploaded to a server. Anything ffmpeg can decode works as input: wav, flac,
mp3, m4a/aac, ogg, opus and more.

Going **down** in depth throws away low-order bits. Doing that by plain
truncation turns the discarded remainder into quantization *distortion* —
correlated with the signal, and most audible in fades, reverb tails and quiet
passages. **Dither** fixes it: a tiny amount of noise is added before the
truncation so the error becomes uncorrelated hiss instead. That is why the
dither control defaults to **triangular (TPDF)** rather than off.

### Worked example

You have a 24-bit master, `mixdown.wav`, and the distributor wants 16-bit.
Upload the file, leave **Target bit depth** at `16`, leave **Dither** at
`triangular`, keep **Output format** at `wav`, and you get
`mixdown-16bit.wav` — the same length and sample rate, 16-bit words, dithered.
The file is about a third smaller (24 bits per sample → 16). Want the noise
tucked further out of the ear's most sensitive band? Switch **Dither** to
`shibata` and re-run.

### Target bit depths

- **8-bit** — unsigned PCM, ~48 dB of range. Tiny files, obvious hiss; useful
  for lo-fi, retro game audio and toy hardware. WAV only.
- **16-bit** — the CD, streaming and delivery standard, ~96 dB. The default.
- **24-bit** — studio and mastering standard, ~144 dB. Keeps headroom for
  further processing.
- **32-bit float** — DAW interchange. Values are stored as floats, so levels
  above 0 dBFS survive round-trips instead of clipping. Nothing is truncated,
  so dither does not apply. WAV only.

### Dither algorithms

- **None** — plain truncation. Correct only when you are going *up* in depth,
  or when another stage will dither later.
- **Rectangular (RPDF)** — the minimal flat dither; lower noise than TPDF but
  leaves some noise modulation.
- **Triangular (TPDF)** — the textbook default: fully decorrelates the error at
  the cost of ~3 dB more noise. Pick this if you are unsure.
- **Triangular high-pass** — TPDF with the noise pushed upward in frequency.
- **Lipshitz, F-weighted, Modified E-weighted, Improved E-weighted, Shibata,
  Low Shibata, High Shibata** — noise-shaped dithers. Same total noise energy,
  but moved to where the ear is least sensitive, so the *perceived* noise floor
  drops. `shibata` is the usual choice for a 16-bit master; `low_shibata` is
  the gentler variant.

### Limits and edge cases

- Input files up to 10 MiB; the output is capped at 10 MiB too.
- **FLAC stores 16-bit and 24-bit only.** Asking for 8-bit or 32-bit float FLAC
  fails with an explicit message — use WAV for those depths.
- Dither is applied only when the target is an integer depth. With
  `bit_depth = 32f` the dither choice is ignored, because floats truncate
  nothing.
- Increasing depth (16 → 24) does not add detail the recording never captured;
  it just stores the same values in wider words.
- The **sample rate is left untouched** — that's a separate axis. Use the
  audio-resampler tool to change Hz, and audio-convert to change container or
  bitrate.
- Embedded album art is dropped: cover images ride along as a video stream,
  which audio-only muxers like wav can't carry. Text tags are copied unless you
  clear **Keep title/artist tags**.
- The output keeps the original filename with a depth suffix and the new
  extension (`mixdown.wav` → `mixdown-16bit.wav`, `take.wav` →
  `take-24bit.flac`).

## FAQ

<details>
<summary>Do I need dither when converting 24-bit to 16-bit?</summary>

Yes, in almost every case. Dropping from 24 to 16 bits discards eight bits per
sample; without dither the rounding error tracks the signal and shows up as
gritty distortion on fades and quiet passages rather than as neutral noise.
TPDF (`triangular`, the default here) is the standard, safe answer. The one
time to choose `none` is when the material is already 16-bit or will be
dithered by a later stage — dithering twice just stacks noise.

</details>

<details>
<summary>Which dither should I pick — triangular or shibata?</summary>

`triangular` is the neutral default and is never wrong. Noise-shaped options
like `shibata` keep the same total noise energy but move it into frequency
bands where hearing is less sensitive, so the master sounds quieter even though
a meter reads the same. Use `shibata` for a final 16-bit music master,
`low_shibata` if the shaped hiss feels too bright, and stay on `triangular` for
speech, archival transfers or anything that will be processed again.

</details>

<details>
<summary>Does converting to 24-bit or 32-bit float improve the sound?</summary>

No. Going up in depth stores the exact same sample values in wider words — it
cannot recover detail the original recording never had. It is genuinely useful
before further processing, where the extra headroom and precision stop
successive edits from accumulating rounding error, and 32-bit float in
particular can hold levels above 0 dBFS without clipping.

</details>

<details>
<summary>Why can't I export 8-bit or 32-bit float FLAC?</summary>

The FLAC format itself only defines integer sample sizes up to 24 bits in
practice, and the encoder here accepts 16-bit and 24-bit. Requesting 8-bit or
32-bit float FLAC returns an error naming the supported combinations instead of
silently writing something else. Choose WAV, which carries all four depths.

</details>

<details>
<summary>Does this change the sample rate or the file's length?</summary>

No. Only the word length of each sample changes; the sample rate (44.1 kHz,
48 kHz, …), the duration and the channel count all pass through untouched. To
change the rate, use the separate audio-resampler tool; to change container or
bitrate, use audio-convert.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file entirely inside the browser tab — the audio never leaves your device, and
there is no account, queue or server-side storage.

</details>
