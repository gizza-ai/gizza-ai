## Tighten long pauses without making speech sound chopped

Upload a podcast clip, interview, lecture, or voice note and this tool shortens only the pauses that are longer than your chosen limit. Short natural pauses are left alone; long gaps are capped to the target pause length with ffmpeg's `silenceremove` filter.

### Worked example

A voice note has several awkward two-second gaps. Upload `voice.m4a`, leave **Silence threshold** at `-30`, set **Shorten pauses longer than** to `1.5`, and **Keep this much pause** to `0.5`. The exported `voice-tightened.mp3` keeps half-second breathing room but removes the dragging dead air.

### When to adjust the threshold

Use a more negative threshold such as `-40` if the room is quiet and breaths should not count as silence. Use a less negative threshold such as `-25` if the recording has background noise and long gaps are not being detected. The defaults are tuned for spoken-word recordings.

### Limits and edge cases

- Input files up to 10 MiB; any audio format ffmpeg can decode should work.
- `target_pause` must be less than `max_pause`, otherwise the edit would be a no-op.
- Leading silence is preserved intentionally. Use `audio-silence-remove` if you want to strip leading/trailing dead air aggressively.
- Output is re-encoded to mp3, wav, ogg, flac, or m4a. Embedded album art is dropped.

## FAQ

<details>
<summary>How is this different from removing silence?</summary>

Removing silence can make speech sound jumpy because every pause is cut down to almost nothing. This tool only caps pauses above your limit, so normal short pauses remain and long gaps become a consistent natural beat.

</details>

<details>
<summary>What values should I start with for podcasts?</summary>

Start with `threshold_db=-30`, `max_pause=1.5`, and `target_pause=0.5`. For a tighter edit, lower `max_pause` to `1.0` and `target_pause` to `0.25`. For a more relaxed edit, keep `target_pause` around `0.75`.

</details>

<details>
<summary>Why did some pauses stay untouched?</summary>

Pauses shorter than **Shorten pauses longer than** are intentionally left alone. If a long gap remains, the audio may not be quiet enough to cross the threshold; try a less negative threshold such as `-25`.

</details>

<details>
<summary>Is my audio uploaded anywhere?</summary>

No. The page runs ffmpeg WebAssembly in your browser tab, so your audio stays on your device. The CLI/chat version uses the same ffmpeg plan through the local tool runtime.

</details>
