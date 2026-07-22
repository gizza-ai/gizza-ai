## Bleep, mute, or duck words in audio — in your browser

Pick an audio file, list the time regions you want to censor, and each one is
replaced right in your browser with ffmpeg — no upload. Use **bleep** for the
classic tone over a swear word, **mute** to drop a region to silence, or
**duck** to lower it just enough that something was clearly there. Everything
outside the regions you list is left completely untouched.

Regions are a comma-separated list of `start-end` ranges. Each time can be
plain seconds (`1.5`) or a timestamp (`0:07`, `1:02:03.5`), so you can copy
times straight off a player's clock.

### Worked example

You have a `podcast.mp3` and someone swears at 12.4–12.9 seconds and again at
1 minute 5 seconds. Set **Regions** to `12.4-12.9, 1:05-1:06.5`, leave **Mode**
on **Bleep** and the **tone** at `1000` Hz, and download `podcast-censored.mp3`
— both spots now carry a classic 1 kHz bleep and the rest of the episode is
identical. Prefer silence? Switch **Mode** to **Mute**. Want the timing to
still read as speech without the words being intelligible? Use **Duck**.

### Picking a mode

- **Bleep** — the expected TV/radio censor: a steady tone covers the word.
  Raise the **tone** for a sharper beep, lower it for a softer one.
- **Mute** — total silence over the region; cleanest when you don't want any
  sound to draw attention.
- **Duck** — lowers the region to about a tenth of its volume, leaving a faint
  trace so the edit doesn't feel like a dropout.

### Limits and edge cases

- Input files up to 16 MiB; any format ffmpeg can decode works.
- List up to 50 regions; each must be written `start-end` with the end after
  the start.
- The bleep tone frequency is 100–8000 Hz (default 1000).
- Overlapping or out-of-order regions are fine — each range is applied on its
  own; a region past the end of the track simply does nothing.
- Bleep output is normalized to 44.1 kHz stereo before mixing the tone;
  mute/duck keep the source layout. Output is always re-encoded (mp3/ogg at
  192 kbps, wav/flac lossless, m4a AAC) and embedded album art is dropped.

## FAQ

<details>
<summary>How do I write the regions?</summary>

As a comma-separated list of `start-end` pairs, e.g. `1.5-2.0, 0:07-0:08.5`.
Each time is either seconds (`12.4`) or a `mm:ss` / `hh:mm:ss` timestamp
(`1:05`, `1:02:03.5`). The end of each pair must be later than its start.

</details>

<details>
<summary>What's the difference between bleep, mute, and duck?</summary>

**Bleep** mixes a tone over the region so a word is covered by a beep.
**Mute** silences the region completely. **Duck** lowers the region to roughly
a tenth of its volume, which keeps a faint trace of the audio so the edit is
less abrupt. Bleep is the default.

</details>

<details>
<summary>Can I change the bleep tone?</summary>

Yes — the **Bleep tone** control sets the frequency in Hz. `1000` is the
classic TV/radio bleep; go higher (say `1500`) for a sharper beep or lower for
a softer one. The tone control only affects **Bleep** mode.

</details>

<details>
<summary>Does it detect the swear words for me?</summary>

No. This tool censors the exact time regions you give it — it does not
transcribe the audio or find profanity automatically. Note the times of the
parts you want to hide (a media player's clock works well) and list them.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes your
file locally in the browser tab — the audio never leaves your device.

</details>
