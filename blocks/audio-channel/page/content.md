## Swap, downmix, or re-route audio channels in your browser

Pick an audio file and one operation, and ffmpeg re-routes its channels
entirely in your browser — nothing is uploaded. **Swap** exchanges the left and
right sides of a stereo file. **Stereo → mono** folds every channel down to one.
**Mono → stereo** duplicates a single channel into two. **Left → both** and
**Right → both** copy one side onto both channels so a one-sided recording is
audible in both ears. Output is written as mp3, wav, ogg, flac, or m4a.

### Worked example

An interview was recorded with the microphone plugged into the right input, so
the whole conversation only plays out of the right earbud. Upload `talk.wav`,
set **Operation** to `Right channel → both sides` and **Output format** to
`mp3`, and the result `out.mp3` carries that voice on both channels — usable on
any speaker or phone. If the two sides were simply wired backwards instead, pick
`Swap left / right` to put them right.

### Which operation do I want?

- **Swap** — channels are reversed (backwards stereo mix, mis-wired cable).
- **Stereo → mono** — shrink a voice recording, or feed a transcription/telephony
  system that expects a single channel.
- **Mono → stereo** — force a two-channel file for a player or editor that
  refuses mono input.
- **Left → both** / **Right → both** — one side is silent, noisy, or the only
  good take; keep just that side and hear it everywhere.

### Limits and edge cases

- Input files up to 10 MiB; any format ffmpeg can decode is accepted.
- `swap`, `left`, and `right` operate on the first two channels; on a surround
  source they reference the front-left/front-right channels.
- `Stereo → mono` uses ffmpeg's standard fold-down, so 5.1/7.1 sources are mixed
  with the correct channel weights, not a naive L+R average.
- Copying one side onto both (`left`/`right`) discards the other channel — the
  stereo image cannot be recovered afterwards, so keep your original.
- Output is re-encoded (mp3/ogg at 192 kbps; wav/flac lossless; m4a AAC).
  Attached album-art streams are dropped.

## FAQ

<details>
<summary>What's the difference between "Stereo → mono" and "Left → both"?</summary>

**Stereo → mono** blends both channels into one, so anything on either side is
kept (at half level if it was only on one side). **Left → both** throws the
right channel away and copies the left onto both outputs — use it when one side
is unusable, not when you want to preserve everything.

</details>

<details>
<summary>Can I turn a mono file into real stereo?</summary>

Not truly. `Mono → stereo` duplicates the single channel so the file reports two
identical channels, which satisfies players and editors that require stereo, but
it doesn't invent a left/right image. Genuine stereo separation can't be
recovered once a recording is mono.

</details>

<details>
<summary>My recording only plays in one earbud — which operation fixes it?</summary>

If the audio is on the left only, choose **Left channel → both sides**; if it's
on the right, choose **Right channel → both sides**. Either copies the good side
onto both channels so it plays evenly on headphones and speakers.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file locally in the browser tab — the audio never leaves your device.

</details>
