# audio-fade — competitor analysis (2026-07-03)

One WebSearch ("add fade in fade out to audio online tool mp3"); skimmed the top real tools:
Notevibes add-fade-to-audio, WuTools audio-fade, audiocut.io fade in/out, audioeditor.org
fade tool (also ffmpeg-in-browser), Premierely audio-fade, Aspose fade in/out.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Fade-in and fade-out lengths in seconds | all | in-model | `fade_in`/`fade_out` number params |
| 3 s / 3 s as the suggested preset | Notevibes | in-model | both default 3.0 |
| Fade only one side | all (checkbox or 0) | in-model | 0 skips that side; both 0 rejected as a no-op |
| Local/in-browser processing (ffmpeg.wasm) | audioeditor.org, Notevibes | in-model | how gizza pages work; stated on page |
| Output format choice | most | in-model | family-standard `format` enum, default mp3 |
| Fade curve shapes (log/exp/sine) | WuTools only | out-of-model for v1 | ffmpeg default tri curve; listed, not built (afade `curve=` would be a trivial later param) |
| Crossfade between two files | Notevibes | out-of-model | multi-input ffmpeg is un-buildable here (single-file page input) |
| Live waveform preview of the fade | Premierely | out-of-model | page framework is run-per-change, no waveform UI |

## Design decisions

- The fade-out start time (duration − length) is unknowable at argv-build time (the page
  builds the argv before ffmpeg ever sees the file), so fade-out uses the duration-free
  `areverse,afade=t=in,areverse` trick; a unit test pins the absence of any `t=out`/`st=`
  term that would need the duration. Two full-buffer reverses are fine under the 10 MiB cap.
- Fade lengths 0–30 s, rejected (not clamped) outside the range, like the rest of the family.
- Fades longer than the clip are allowed (they just span the whole file, ffmpeg semantics);
  the overlap behaviour is documented on the page rather than second-guessed in code.
- Verification proves the ENVELOPE, not just "output differs": on a constant 3 s tone with
  1 s fades, the first/last 0.2 s windows must fall below 0.3× the middle RMS (pre-measured
  0.115×, -40.3 dB vs -21.5 dB) while the middle stays within ±15% of the input's — a plain
  volume cut would fail that pair. CLI check: 0.4 s fades on a public beep drop its first
  0.15 s window from -29.8 to -42.8 dB; the both-zero guiding error is also exercised.
