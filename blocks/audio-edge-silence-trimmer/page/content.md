## Trim the dead air at the ends without touching the middle

Upload a voice memo, podcast segment, interview, or narration take and this tool removes only the silence at the very start and end of the file. Every pause inside the recording — breaths, dramatic beats, gaps between sentences — is left exactly as it was. It uses ffmpeg's `silenceremove` filter applied to each edge (`silenceremove … areverse … silenceremove … areverse`), so no `stop_periods` ever run and internal quiet passages are never cut.

### Worked example

A phone recording starts with 1.2 seconds of room tone before you begin talking and trails off into silence at the end. Upload `memo.mp3`, leave **Silence threshold** at `-50`, keep **Pad kept at each edge** at `0.1`, and choose `mp3`. The exported `memo-trimmed.mp3` drops the leading and trailing dead air but keeps a tenth of a second of silence on each side so the cut never sounds abrupt — a ~3.6s clip comes back around 1.4s with the spoken part intact.

### When to adjust the threshold and pad

Use a more negative threshold such as `-55` if the room is quiet and even faint tone should count as silence. Use a less negative threshold such as `-40` if there is background hiss and the edges are not being detected. Set **Pad** to `0` for a hard cut right up to the first and last sound, or raise it to `0.25` to leave more breathing room.

### Limits and edge cases

- Input files up to 10 MiB; any audio format ffmpeg can decode should work.
- Threshold must be `0` dB or below (silence is measured below full scale); pad must be `0` or greater.
- Only the leading and trailing silence is trimmed. To collapse long gaps *inside* the recording, use `audio-silence-remove`; to shorten (not remove) long internal pauses, use `audio-pause-shortener`.
- Output is re-encoded to mp3 (192 kbps), wav, ogg, flac, or m4a. Embedded album art is dropped.

## FAQ

<details>
<summary>How is this different from removing silence?</summary>

`audio-silence-remove` strips every silent gap, including the pauses in the middle of your recording, which can make speech sound jumpy. This tool touches only the two edges: it trims the dead air before the first sound and after the last sound, and leaves everything in between untouched.

</details>

<details>
<summary>Why is there still a little silence at the start and end?</summary>

That is the **Pad** setting. By default `0.1` seconds of silence is kept at each edge so the recording does not start or stop mid-breath. Set **Pad** to `0` if you want the cut right up against the first and last sound.

</details>

<details>
<summary>The edges were not trimmed — what happened?</summary>

The audio at the edges was probably louder than the threshold, so it did not count as silence. If the start has faint room tone or hiss, try a less negative threshold such as `-40`. For a very quiet recording where even breaths should be cut, use a more negative threshold such as `-55`.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page runs ffmpeg WebAssembly in your browser tab, so your audio stays on your device. The CLI and chat versions use the same ffmpeg plan through the local tool runtime.

</details>
