## Make a ringtone from any song, in your browser

Upload a song, drag the two handles on the waveform (or type **Start** and
**End** in seconds) onto the hook you want, and download it as a ready-to-use
ringtone: **m4r** — the AAC format iPhone requires — or **mp3** for Android
and everything else. Leave **End** empty for the classic 30-second cut
starting at **Start**. The tool also does the two things a raw cut misses:
**Boost loudness** levels the slice to −14 LUFS (EBU R128) so it actually
rings loud on a phone speaker, and the short **fade-in/fade-out** ramps
(0.5 s by default) stop the cut from starting or ending with a click.
Everything runs locally with ffmpeg compiled to WebAssembly — the song is
never uploaded to a server.

### Worked example

The chorus of your song starts at 45 seconds. Upload the file, set **Start**
to `45`, leave **End** empty (that's `45 + 30 = 75` — a standard 30-second
ringtone), keep **Boost loudness** ticked and both fades at `0.5`, and leave
**Format** on `m4r — iPhone`. The result is a 30-second, loudness-leveled
AAC ringtone named like the original with a `-ringtone.m4r` suffix — e.g.
`song.mp3` becomes `song-ringtone.m4r`. For an Android phone, switch
**Format** to `mp3 — Android & others` and you get `song-ringtone.mp3`
instead. Preview it with the play button before downloading.

### Formats

- **m4r — iPhone** (default) — AAC at 192 kbps in the iPod/MP4 container.
  This is the one format iOS accepts for custom ringtones.
- **mp3 — Android & others** — 192 kbps mp3; Android, Samsung and most other
  phones set it as a ringtone directly from the file manager.

### Limits and edge cases

- Ringtone length (**End** − **Start**) is capped at **40 seconds** — iPhone
  rejects anything longer — and must be at least 1 second. **End** empty or
  `0` means **Start** + 30 s.
- Input files up to 10 MiB; anything ffmpeg can decode works (mp3, wav, flac,
  m4a/aac, ogg, opus — even a video file's audio track).
- If the song ends before your selection does, the ringtone is simply shorter
  — times past the end of the file are clamped, and the fade-out still lands
  on the real ending.
- **Boost loudness** resamples the output to 44.1 kHz (the loudness filter
  works at a high internal rate); with it off, the source sample rate is
  kept.
- Fades are 0–5 s per side; set both sliders to `0` for a hard cut. Album
  art embedded in the source is dropped — the output is audio only.

## FAQ

<details>
<summary>How do I get the m4r file onto my iPhone as a ringtone?</summary>

Connect the phone and drop the `.m4r` file onto it in **Finder** (macOS) or
**iTunes** (Windows) — it appears under Settings → Sounds & Haptics →
Ringtone. Without a computer, save the file to the Files app and import it
with Apple's free **GarageBand** app (Share → Ringtone). iOS has no direct
"set as ringtone" for downloaded files, so one of those two steps is always
needed.

</details>

<details>
<summary>How do I set the mp3 as a ringtone on Android?</summary>

Copy or download the `mp3` to the phone, long-press it in your file manager
and choose **Set as ringtone** — or move it into the `Ringtones` folder and
pick it under Settings → Sound → Phone ringtone. Any length works on
Android, but 30 seconds is plenty: the phone loops or cuts it anyway.

</details>

<details>
<summary>Why is my ringtone capped at 40 seconds?</summary>

That's iPhone's hard limit — iOS refuses to list ringtones longer than
40 seconds. This tool enforces the same cap for mp3 too, because a ringtone
only plays for the ~25–30 seconds a call rings; the 30-second default is the
sweet spot on every platform.

</details>

<details>
<summary>What does "Boost loudness" actually do — and when should I turn it off?</summary>

It runs EBU R128 loudness normalization (`loudnorm`) on the cut slice,
targeting −14 LUFS integrated with a −1.5 dBTP true-peak ceiling — the level
streaming services master to. Songs are often mastered quieter, and a quiet
ringtone is a missed call. Turn it off if you're cutting something already
mastered loud, or want the original level and dynamics kept as-is.

</details>

<details>
<summary>Is my song uploaded anywhere?</summary>

No. The page downloads an ffmpeg WebAssembly build once and then processes
your file locally in the browser tab — the audio never leaves your device.

</details>
