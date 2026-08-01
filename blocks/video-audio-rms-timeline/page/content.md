## About this tool

Extract a **windowed audio-level time series** from a video (or plain audio)
file. The tool decodes the first audio track, downmixes it to mono, slices it
into fixed-length windows, and reports each window's **RMS** level (the average
energy — how loud the window sounds) and **peak** level (the single loudest
sample — what a limiter or clip indicator reacts to). The result is a plain CSV
or JSON time series you can chart, threshold, or feed into a spreadsheet.

Paste the file's bytes as **base64** or **hex**. In chat or the `gizza` CLI you
can pass a URL instead; on this page the bytes go straight into the field so
nothing leaves your browser — decoding and level maths run entirely in
WebAssembly.

### Worked example

With the default settings (window `100` ms, hop `0` = non-overlapping, unit
`dbfs`, output `csv`) a one-second full-scale test tone produces ten rows:

```
window,start_s,end_s,rms_dbfs,peak_dbfs
0,0,0.1,-3.01,0
1,0.1,0.2,-3.01,0
...
9,0.9,1,-3.01,0
```

Each row covers one 100 ms window. `start_s`/`end_s` are the window bounds in
seconds; `rms_dbfs` and `peak_dbfs` are the levels in decibels relative to full
scale (0 dBFS = the loudest a sample can be). A full-scale sine reads about
−3 dBFS RMS and 0 dBFS peak, exactly as expected.

### Controls

- **Window length (`window_ms`)** — how much audio each row measures (1–60000 ms,
  default 100). Short windows (10–50 ms) track fast transients; long windows
  (250–1000 ms) give a smooth loudness envelope.
- **Hop (`hop_ms`)** — the step between window starts (0–60000 ms, default 0).
  `0` means non-overlapping (hop = window). A hop **smaller** than the window
  overlaps frames for a smoother curve; a hop **larger** than the window samples
  the level periodically and skips the audio in between.
- **Level unit** — `dbfs` (0 dB = full scale; silence is floored to −120 dB so
  the CSV stays numeric) or `linear` (a 0–1 amplitude fraction, like
  `librosa.feature.rms`).
- **Output format** — `csv` (a header row plus one row per window) or `json`
  (the same rows plus `sample_rate`, `channels`, `duration_s`, and window
  metadata).

### Limits and edge cases

- **Mono downmix.** Levels are measured on the mono downmix of the track, not
  per channel — two channels of opposite polarity cancel to silence. Per-channel
  columns are out of scope.
- **Silence floor.** In dBFS a fully silent window reads −120 dB (not −∞) so the
  value stays parseable.
- **Length cap.** About five minutes of audio is analyzed; longer input is
  truncated (the JSON output flags this with `"truncated": true`).
- **Row cap.** A window/hop combination that would emit more than 500,000 rows is
  rejected — widen the window or hop.
- **Supported formats.** Containers MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF,
  FLAC, MP3, AAC-ADTS; codecs AAC-LC, ALAC, MP3, Vorbis, FLAC, PCM, ADPCM.
  Opus, AC-3 and DTS are not supported, and a silent/video-only file is rejected.

## FAQ

<details>
<summary>What is the difference between the RMS and peak columns?</summary>

**RMS** is the root-mean-square of the samples in a window — it approximates how
loud that stretch of audio *sounds*, because it averages energy over time.
**Peak** is the largest absolute sample value in the window — it captures the
single loudest instant, which is what matters for clipping and headroom. A busy
window can have a high peak but a modest RMS; a sustained tone has RMS and peak
close together.

</details>

<details>
<summary>Should I use dBFS or linear units?</summary>

Use **dBFS** for level work: it is the decibel scale meters and editors use, so
−6 dBFS, −18 dBFS and 0 dBFS (full scale) are directly comparable to what your
DAW shows. Use **linear** when you want the raw 0–1 amplitude fraction for
plotting or further maths (this matches `librosa.feature.rms`, which returns
linear RMS). In dBFS a silent window is floored to −120 dB so the number stays
finite.

</details>

<details>
<summary>How do window length and hop affect the output?</summary>

`window_ms` sets how much audio each row averages; `hop_ms` sets how far apart
the rows are. With the default hop of `0` the windows are back-to-back and each
sample is counted once. Set `hop_ms` **below** `window_ms` to overlap windows —
e.g. a 100 ms window with a 50 ms hop gives twice as many rows and a smoother
level curve, the standard frame/hop analysis librosa and ffmpeg's `astats` use.
A hop **larger** than the window samples the level periodically.

</details>

<details>
<summary>Why is the level slightly different from ffmpeg's astats or my DAW?</summary>

Small differences are expected. This tool measures RMS over the mono downmix in
the linear sample domain, then converts to dBFS with `20·log10(rms)`. Tools like
ffmpeg's `astats` window differently (per packet or per `asetnsamples` chunk)
and some report per-channel or apply a different reference, so absolute values
can differ by a fraction of a dB. The *shape* of the timeline — where it rises
and falls — is what matters and stays consistent.

</details>

<details>
<summary>Does my file leave the browser on this page?</summary>

No. On this page the audio is decoded and analyzed entirely in WebAssembly in
your browser — the pasted bytes are never uploaded. (The chat and `gizza` CLI
surfaces can additionally fetch a URL you provide, which does make a network
request to that URL.)

</details>
