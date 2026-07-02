# audio-silence-remove — competitor analysis (2026-07-02)

One WebSearch ("remove silence from audio online tool podcast dead air silence remover");
skimmed the top real tools: Cleanvoice dead-air remover, Submind audio silence remover,
Kapwing silence remover, Descript silence remover, AudioCleaner, Timbrica, ImageToolHub.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Auto-detect + strip dead air (leading/middle/trailing) | all | in-model | single-pass `silenceremove` with `start_periods=1` + `stop_periods=-1` |
| Adjustable sensitivity/threshold | Descript, Timbrica | in-model | `threshold_db` (≤ 0, default -30) — names/wording shared with video-silence-cut |
| Minimum pause length control | Descript ("word gap"), Cleanvoice | in-model | `min_silence` seconds (> 0, default 0.5) |
| Natural-sounding cuts (don't fully close gaps) | Cleanvoice ("shortens" rather than deletes), AudioCleaner | in-model | fixed `*_silence=0.25` keeps 0.25 s of each removed gap |
| Local/in-browser processing | Submind | in-model | how gizza pages work; stated on page |
| Waveform preview / transcript-based editing | Timbrica, Descript | out-of-model | page framework has no waveform/transcript UI |
| 1.5 GB uploads, AI filler-word removal | Cleanvoice | out-of-model | 10 MiB cap; no ML in gizza blocks |

## Design decisions

- Param names `threshold_db` / `min_silence` copied from video-silence-cut so the silence
  family reads identically across tools; that tool stays two-pass (A/V sync), this one is
  single-pass because it's audio-only — the difference is stated in both tools' copy/FAQ.
- Page zeros (empty fields) map to the defaults: threshold 0 dB would classify everything as
  silence and a 0 s gap is invalid, so 0 is unambiguous as "use default".
- BUG CAUGHT BY THE TESTS: the first filter draft passed `min_silence` as `start_duration`
  too. That option actually means "length of NON-silence required before audio counts as
  started" — a 0.11 s beep with `start_duration=0.5` produced an empty 78-byte wav, and the
  min_silence=2 page test swallowed 0.7 s tones entirely. Fixed by mapping `min_silence` to
  `stop_duration` only; leading silence is always trimmed (stated in the page copy).
- Verified end-to-end after the fix: the 2.95 s fixture with a 1.5 s mid-gap collapses to
  ~1.65 s in the page test (WebAudio decode), the min_silence=2 deep-link case returns the
  full ~2.95 s, and the CLI shortens beep_short.ogg from 1.26 s to 0.87 s (ffprobe — the
  beep's decay tail sits above -30 dB longer than the naive estimate assumed).
