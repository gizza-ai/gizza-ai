## About this tool

Audio Feature CSV Exporter turns a short audio clip into a tidy frame-by-frame table of classic signal features: RMS level, spectral centroid, zero-crossing rate, spectral rolloff, spectral flatness, plus optional bandwidth and flux. The defaults use 25 ms frames and 10 ms hops, which match common music-information-retrieval and speech-analysis workflows while keeping the CSV small enough to inspect in a spreadsheet.

Upload WAV, MP3, FLAC, OGG/Vorbis, M4A/AAC, AIFF, CAF, MKV/WebM, or another supported audio container. The tool decodes the first usable audio track, mixes or selects a stereo channel, optionally resamples to a fixed analysis rate, then exports CSV, TSV, or JSON. Use the preset chips for a standard feature table, all-feature export, librosa-style centered frames, onset-detection flux, or metadata-rich JSON.

Worked example:

1. Upload a 3-second WAV tone.
2. Keep `frame_ms = 25`, `hop_ms = 10`, `window = hann`, and `output = csv`.
3. Run the tool. The output starts with a header such as `time_s,rms_dbfs,centroid_hz,zcr,rolloff_hz,flatness` followed by roughly 298 rows for a 3-second clip.
4. Turn on `bandwidth` and `flux` when you need timbre-spread and onset columns, or switch `output` to `json` when you want the resolved sample rate and frame settings alongside the matrix.

Limits and edge cases: pasted/loaded audio is capped at 24 MiB, analysis is capped at 4,000,000 mono samples and 200,000 output rows, and Opus/AC-3/DTS-style codecs are rejected with a clear error. Very short clips need either a shorter `frame_ms` or centered frames. Digital silence stays finite (`-200 dBFS`) instead of producing `NaN`.

## FAQ

<details>
<summary>How close are these columns to common audio-analysis libraries?</summary>

The definitions follow the usual MIR formulas: RMS over the raw frame, magnitude-weighted spectral centroid, zero-crossing rate, spectral rolloff by cumulative magnitude, and spectral flatness as geometric mean over arithmetic mean. Centered frames, Hann windows, rolloff percentage, and resampling are exposed so you can line up the output with typical notebook pipelines.

</details>

<details>
<summary>Should I export CSV, TSV, or JSON?</summary>

Use CSV for spreadsheets, pandas, R, and most BI tools. Use TSV when commas in downstream tooling are inconvenient. Use JSON when you also need metadata such as the original sample rate, resolved analysis rate, frame length, hop length, FFT size, selected features, and truncation flags.

</details>

<details>
<summary>Why is there a resample option?</summary>

Features depend on sample rate and frame size. Setting `resample_hz` to a fixed value such as 16000 or 22050 makes files recorded at different rates produce comparable row counts and frequency bins. Leave it at `0` when you want to preserve the file's native rate.

</details>

<details>
<summary>What do the optional bandwidth and flux columns add?</summary>

Bandwidth measures how spread out the spectrum is around the centroid, which helps separate narrow tonal sounds from broadband ones. Flux measures positive frame-to-frame spectral change, so it is useful for onset detection and spotting sudden timbre changes.

</details>
