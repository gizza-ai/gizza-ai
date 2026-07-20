## About this tool

This tool answers one question about any video: **when is someone speaking, and when
not?** It runs voice-activity detection (VAD) on the audio track and returns the
timestamps of every speech and non-speech segment — it does **not** transcribe words
and it does **not** modify the video. The timestamps are what editors, subtitle
workflows, and scripts consume: jump-cut lists, subtitle scaffolds, ad-break or
dead-air audits, "how much of this lecture is actually talking" statistics.

Detection is energy-based, the same technique audio engineers use with ffmpeg by hand:

1. **Band-pass** — with *Voice band only* on (the default), the audio is first
   filtered to 200–3000 Hz, where speech lives. Low rumble (traffic, handling noise)
   and high hiss stop counting as "sound", so the result tracks *voices*, not noise.
2. **Detect** — ffmpeg's `silencedetect` filter finds every stretch quieter than the
   **silence threshold** that lasts at least the **min pause**. Everything else is
   speech.
3. **Clean up** — bursts shorter than **min burst** (coughs, clicks, door slams) are
   folded back into the surrounding non-speech, and optional **padding** widens each
   speech segment so soft word edges aren't clipped when the timestamps drive a cutter.

Everything runs in your browser via WebAssembly ffmpeg — the file is never uploaded.

### Worked example

A 6-second clip where someone talks for 2 seconds, pauses for 2, and talks 2 more.
With the defaults (threshold **-30 dB**, min pause **0.5 s**, min burst **0.25 s**),
the report reads:

```
speech: 2 segments, 4.000s of 6.000s (66.7%)
non-speech: 1 segment, 2.000s (33.3%)

#1  0:00.000 → 0:02.000  speech      2.000s
#2  0:02.000 → 0:04.000  non-speech  2.000s
#3  0:04.000 → 0:06.000  speech      2.000s
```

Switch **Output format** to CSV for `start,end,duration,label` rows, to SRT for
subtitle-timed segments (a ready-made scaffold if you transcribe afterwards), or to an
Audacity label track you can import over the original audio. **List** narrows the rows
to speech-only or non-speech-only — the summary always covers both.

Tuning: raise the threshold (e.g. **-20 dB**) for noisy recordings where hiss keeps
counting as speech; lower it (e.g. **-45 dB**) for quiet or distant speakers. A larger
min pause (1–2 s) keeps whole sentences in one segment; a smaller one (0.2 s) splits on
every breath.

## Limits and edge cases

- Detection is **loudness-based, not a neural VAD** — loud non-voice sound inside the
  voice band (music, applause) counts as "speech", and whispering below the threshold
  counts as silence. For broadcast-grade VAD or speaker diarization you need an ML
  model, which this tool deliberately doesn't ship.
- **No transcription** — the output is timestamps and labels only.
- The file needs an **audio track**: a silent screen recording errors out with a clear
  message rather than reporting "all silence".
- Via chat/CLI the input is capped at **10 MiB** — compress or trim longer footage
  first (in the browser the practical limit is your device's memory).
- A clip that is entirely below the threshold reports **zero speech segments** plus a
  hint to lower the threshold — that's the tool refusing to guess, not a failure.
- Segments are reported as detected; very long speech runs are **not** auto-split into
  chunks (no max-duration option).

## FAQ

<details>
<summary>Does this transcribe what is said?</summary>

No. It only detects *when* speech happens, not *what* is said. That's the point: the
timestamps are useful on their own (cut lists, subtitle timing, silence audits) and the
detection runs entirely in your browser with no ML model download. If you need words,
run a transcriber afterwards — the SRT output gives it perfectly timed empty slots.

</details>

<details>
<summary>How is this different from the silence-removal tools?</summary>

Same detection, different deliverable. The silence-cut tools re-encode your video with
the quiet parts removed; this one **only reports the timestamps** and leaves the video
untouched. Use it when you want to inspect first, feed an editor or script, or keep the
original file — the CSV rows paste straight into a cutter's keep/remove list.

</details>

<details>
<summary>What exactly do "speech" and "non-speech" mean here?</summary>

A segment is *speech* when its audio stays louder than the silence threshold (measured
in the 200–3000 Hz voice band by default) for at least the min-burst duration.
*Non-speech* is everything else: silence, room tone, and anything quieter than the
threshold. It's an energy detector — a barking dog at full volume in the voice band
will be labeled speech, and a whisper under the threshold won't.

</details>

<details>
<summary>Which output format should I pick?</summary>

**Report** for reading: a summary (segment counts, seconds, speech percentage) plus a
numbered list. **CSV** for spreadsheets and scripts. **SRT** when the segments should
line up with subtitle tooling — each speech segment becomes one correctly-timed entry.
**Audacity label track** to eyeball the detection against the waveform: import it in
Audacity via the label-track import and the segments appear under the audio.

</details>

<details>
<summary>Why does my video report 100% speech (or 100% silence)?</summary>

The threshold doesn't match the recording's level. Background music or constant noise
above **-30 dB** makes everything "speech" — raise the threshold toward **-20 dB**, or
leave *Voice band only* on so out-of-band noise is ignored. A quiet speaker far from
the microphone can sit entirely below **-30 dB** — lower the threshold to **-45 dB** or
beyond. The dB scale is relative to full scale: 0 dB is the loudest possible sample.

</details>
