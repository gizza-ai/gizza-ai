## Convert audio to mono in your browser

Pick an audio file and it's downmixed to a single channel with ffmpeg,
entirely in your browser. **mix** (the default) blends every channel using
ffmpeg's standard downmix — the classic (L+R)/2 for stereo, and the proper
weighted fold-down for 5.1/7.1 surround. **left** or **right** instead keeps
just that side of the recording.

### Worked example

A two-person interview was recorded with each voice on its own side, and the
right channel picked up an air conditioner. Upload `interview.wav`, set
**Channel** to `left` and **Format** to `mp3` — the result
`interview-mono.mp3` contains only the clean left-side voice track. For normal
music or voice memos, leave **Channel** on `mix`.

### Why go mono?

- Voice content (podcasts, dictation, calls) doesn't benefit from stereo, and
  mono mp3 files are noticeably smaller at the same quality.
- Many transcription and telephony systems expect a single channel.
- One-sided recordings (mic plugged into one input) become properly audible on
  both earbuds.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode works.
- `left`/`right` reference the first two channels; on a surround source they
  pick the front-left/front-right channels.
- A file that is already mono passes through unchanged (aside from
  re-encoding).
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC).
  Embedded album art is dropped.

## FAQ

<details>
<summary>Does mixing to mono make the audio quieter?</summary>

Slightly, when the two sides differ: ffmpeg's downmix averages the channels,
so sound present on only one side ends up at half its original level. Sound
present on both sides (like most voice recordings) keeps its level. If the
result is too quiet, run it through the audio-normalize tool afterwards.

</details>

<details>
<summary>When should I use left or right instead of mix?</summary>

When one channel is the only good one: interviews recorded with a single mic
panned to one side, damaged tracks with noise on one channel, or hardware that
recorded silence on one side. Mixing in a bad channel would add noise —
keeping just the good side avoids that.

</details>

<details>
<summary>Can I convert mono back to stereo?</summary>

A mono file plays through both speakers already — players duplicate the
channel automatically. True stereo (different left/right content) can't be
recovered once it's mixed down; keep your original file if you may need the
stereo image again.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
