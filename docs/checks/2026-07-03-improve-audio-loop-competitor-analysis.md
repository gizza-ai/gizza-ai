# audio-loop — competitor analysis (2026-07-03)

One WebSearch ("loop audio online tool repeat sound to duration mp3"); skimmed the top real
tools: Kapwing audio looper, miniwebtool MP3 Looper, audiocutter.org audio-looper, MAZTR
audio file looper, Tembrica audio-looper, VEED audio looper, Audjust loop-mp3.

## Table stakes observed (paraphrased)

| Capability | Seen at | Fit | Decision |
|---|---|---|---|
| Loop a set number of times | audiocutter ("Loop Count"), MAZTR, Tembrica (2-999) | in-model | `count` = total plays 2-100 |
| Loop to a target duration | audiocutter ("Loop Time"), MAZTR, Tembrica ("e.g. 1 hour") | in-model | `duration` seconds, cut with `-t`; the headline mode (default 30 s) |
| Seamless joins | Tembrica ("repeat seamlessly") | in-model | re-encode (not `-c copy`) so joins are PCM-level; caveat that the CLIP's edges decide audibility |
| Common output formats | VEED, Kapwing | in-model | family-standard `format` enum, default mp3 |
| Loop only a selected section | miniwebtool (waveform selection) | out-of-model | that's trim-audio's job first; page copy points the pipeline trim → loop |
| Crossfaded loop joins | (music-focused loopers) | out-of-model | needs a self-overlap filter graph; FAQ explains the fade-the-edges workaround |
| Waveform editor UI | miniwebtool, Kapwing | out-of-model | page framework is run-per-change, no waveform UI |

## Design decisions

- Mirrors the shipped loop-video block's mode logic (duration > 0 wins; count = total plays →
  `-stream_loop count-1`), so the two tools behave identically across the site. `-stream_loop`
  is an INPUT option; a unit test pins that it precedes `-i`.
- Unlike loop-video's `-c copy`, audio RE-ENCODES: packet-copy loop joins click on mp3
  (encoder delay/padding at every seam); decoding and re-encoding makes joins sample-level.
- duration ≤ 3600 s and count ≤ 100 bound the argv, but the 10 MiB output envelope is the
  real cap (~7 min of 192 kbps mp3) — stated on the page rather than hidden.
- duration=0 with count<2 is rejected with a guiding error naming both modes.
- Verification proves REPETITION, not just length: a window inside a later play must carry
  the input's mid-tone RMS (±15%) — silence padding would fail. Page: 3 s tone → 10 s
  (duration mode, decoded 10.0 s) and ×3 deep link (decoded ~9.1 s, third play checked).
  CLI: public 1.26 s beep → exactly 5.000000 s wav; window at 2.55 s (third play) reads
  -29.8 dB, identical to the input's beep window; guiding zero-mode error exercised.
