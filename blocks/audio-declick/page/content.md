## Repair clicks, pops and crackle locally

Use this audio declicker when a recording has short impulsive defects: vinyl pops,
tape ticks, digital buffer glitches, small crackle bursts, or clipped peaks. The
file stays in your browser. The tool runs ffmpeg's `adeclick` filter to detect
short damaged regions and interpolate replacement samples from the surrounding
audio, then optionally runs `adeclip` for flat-topped clipped peaks.

### Worked example

For a vinyl rip with scattered pops, choose the **Vinyl crackle** preset or start
with these settings:

- Detection strength: `70`
- Repair window: `80` ms
- Burst fusion: `4`
- Method: `add`
- Output format: `mp3`

For short digital ticks where untouched samples should stay as close as possible,
use **Digital glitches**: strength `60`, window `20`, burst `0`, method `save`,
and `wav` output.

### What the controls mean

**Detection strength** is a friendly 1–100 sensitivity scale. Higher values flag
more samples as damage; lower values protect percussion and sharp consonants.
**Repair window** controls how much neighbouring audio is used to rebuild a click.
Use longer windows for broad thumps and shorter windows for tiny ticks.
**Burst fusion** merges nearby detections so a crackle burst is repaired together;
`0` disables fusion. **Method** controls overlap handling: `add` is smoothest,
while `save` changes only samples that were flagged. **Declip** adds a second pass
for clipped, flat-topped peaks.

### Limits and edge cases

This is not broadband noise reduction. Hiss, hum and steady crackle beds are often
better handled by an audio denoise/noise-reduction tool. Aggressive settings can
soften drums, consonants and other intentional transients, so preview a short
section and back off if the music starts to dull. Very large files may exceed the
browser memory budget; trim long recordings first or use a lossless intermediate
such as WAV/FLAC for iterative restoration.

## FAQ

<details>
<summary>Will this remove background hiss?</summary>

No. Declicking targets short impulses: clicks, pops, ticks and small digital
spikes. Continuous hiss, hum and room noise need a spectral denoise/noise-reduce
workflow instead.

</details>

<details>
<summary>Should I use add or save mode?</summary>

Use `add` when you want the smoothest repair and do not mind tiny changes around
the repaired areas. Use `save` when you want untouched samples to remain as close
as possible to the original; it is often safer for drums and sharp attacks.

</details>

<details>
<summary>What does the declip option do?</summary>

`declip` runs an additional interpolation pass over clipped, flat-topped peaks.
It can help with digital overload, but it is not a full mastering repair tool and
cannot reconstruct detail that was completely lost.

</details>

<details>
<summary>Which output format should I choose?</summary>

Use WAV or FLAC when you plan more editing. Use MP3, OGG or M4A for smaller
shareable files. The audio is re-encoded because repairing changes samples; a
lossless stream copy is not possible.

</details>
