## Brick-wall limit audio in your browser

Pick an audio file and set a **ceiling** in dBFS — no sample in the result is
allowed above it. Add **input gain** to drive the signal into the limiter when
you also want it louder, and shape how the limiter reacts with **attack** and
**release**. The tool runs ffmpeg's `alimiter` lookahead limiter entirely in
your browser, so the audio never leaves your device — safe for unreleased
mixes, client work and private recordings.

A limiter is the standard last step before export: it guarantees a hard peak
ceiling, so the file can't clip in a player, on upload, or after conversion to a
lossy format.

### Worked example

You have a finished mix `master.wav` that peaks right at 0 dBFS and distorts on
some players. Upload it, leave **Ceiling** at `-1`, leave **Input gain** at `0`,
**Attack** at `5` and **Release** at `50`, and choose `wav` as the output. The
result `master-limited.wav` sounds the same but its loudest peak now sits at
−1 dBFS, leaving the 1 dB of headroom that lossy encoders and streaming
platforms expect.

Want it louder too? Set **Input gain** to `6`. The whole signal is pushed 6 dB
up *before* the ceiling, so quiet parts get 6 dB louder while peaks stay pinned
at −1 dBFS.

### What each control does

- **Ceiling (dBFS, −24…0)** — the brick wall. `-1` is the usual safety margin,
  `-0.3` squeezes out the last bit of level, `-3` leaves generous headroom for
  further processing.
- **Input gain (dB, −20…20)** — drive applied *before* the ceiling. `0` only
  catches peaks that already exceed the ceiling; `+3`…`+9` makes the result
  audibly louder by pushing more of the signal into limiting.
- **Attack (ms, 0.1…80)** — how fast a peak is clamped. Short (`1`–`5`) is
  transparent on speech and percussive material; longer (`20`+) is smoother but
  lets more of the transient through.
- **Release (ms, 1…8000)** — how fast gain recovers after a peak. Short
  (`20`–`50`) sounds loud and tight; long (`200`+) is smoother and less pumpy.
- **Smooth release** — averages the release over recent gain reduction, which
  sounds calmer on dense, peaky material.
- **Maximize back to full scale** — after limiting, re-normalizes the signal up
  to 0 dBFS. That deliberately overrides your ceiling, so leave it off whenever
  the ceiling has to be honoured.

### Starting points

- **Safety limit before export** — ceiling `-1`, gain `0`, attack `5`, release
  `50`. Nothing changes except that peaks can no longer cross −1 dBFS.
- **Louder podcast / voice-over** — ceiling `-1`, gain `6`, attack `5`, release
  `80`. Fuller and more consistent without audible clamping on speech.
- **Transparent mastering catch** — ceiling `-0.3`, gain `2`, attack `20`,
  release `250`, smooth release on. Only the loudest moments are touched.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works (mp3, wav, m4a,
  ogg, flac, and more).
- This limits **sample peaks**, not inter-sample (true) peaks. A `-1` dBFS
  ceiling is the conventional margin that covers inter-sample overshoot after
  lossy encoding; pick `-2` if you need to be extra safe.
- Choose **wav** or **flac** if the ceiling must hold exactly. mp3, ogg and m4a
  are lossy: decoding them back can overshoot the ceiling by a few tenths of a
  dB.
- A **0 dB ceiling with 0 dB gain and no maximizing does nothing** and is
  rejected — lower the ceiling or add input gain.
- Output is re-encoded because the limiter rewrites the samples (mp3/ogg at
  192 kbps; wav/flac lossless; m4a AAC). Embedded album art is dropped.
- Limiting is one-way. Heavy drive (large input gain into a low ceiling) flattens
  the dynamics and can pump audibly, so keep the original if it matters.

## FAQ

<details>
<summary>What's the difference between a limiter, a compressor and normalizing?</summary>

A **limiter** sets a hard ceiling — nothing gets past it, and everything below it
is left alone. A **compressor** works by ratio: it turns down everything above a
threshold by a proportion, changing the overall dynamics. **Normalizing** doesn't
change dynamics at all — it applies one constant gain so the file hits a target
peak or loudness. Use the limiter last, as a safety net; use the compressor to
even out a performance; use normalization to hit a target level.

</details>

<details>
<summary>What ceiling should I use?</summary>

`-1` dBFS is the standard safety margin and a good default: it stops clipping in
players and leaves room for the overshoot that mp3/AAC encoding introduces. Go to
`-0.3` when you want every last bit of level and the output stays lossless. Go to
`-2` or `-3` when the file will be re-encoded more than once, or when it'll be
processed further downstream.

</details>

<details>
<summary>Why isn't my audio any louder?</summary>

With **Input gain** at `0`, the limiter only touches peaks that already exceed
the ceiling — quiet material passes straight through unchanged. Loudness comes
from the gain control: raise it to `+3`…`+9` dB to push the signal into the
limiter. The peaks stay pinned at the ceiling while everything underneath gets
louder.

</details>

<details>
<summary>What do attack and release actually change?</summary>

**Attack** is how quickly the limiter clamps an incoming peak. A short attack
(`1`–`5` ms) catches sharp transients — plosives, drum hits — almost invisibly;
a longer one (`20`+ ms) lets the initial snap through, which sounds more open but
allows brief overshoot before the wall takes hold. **Release** is how fast the
gain comes back afterwards: short (`20`–`50` ms) is loud and tight, long (`200`+
ms) is smoother. If a fast release sounds like it's breathing, lengthen it or
turn on **smooth release**.

</details>

<details>
<summary>Does this handle true-peak (dBTP) limiting?</summary>

Not exactly. The limiter works on sample peaks, so the ceiling is enforced on the
decoded samples rather than on the reconstructed analogue waveform. In practice a
`-1` dBFS ceiling is the conventional margin that keeps true peaks in check
through lossy encoding; if a platform requires a strict −1 dBTP, set the ceiling
to `-1.5` or `-2` for extra room.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file locally in the browser tab — the audio never leaves your device.

</details>
