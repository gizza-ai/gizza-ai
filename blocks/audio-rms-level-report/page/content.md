## About this tool

Use this tool when you need a whole-file level check before publishing, normalizing, matching stems, or debugging clipped audio. Paste a WAV, FLAC, MP3, OGG/Vorbis, M4A/MP4, AIFF, CAF, MKV/WebM, or AAC-ADTS file as base64 or hex and it reports each decoded channel plus an overall row.

The JSON summary includes sample rate, channel labels, duration, frame counts, whole-file RMS, sample peak, mean absolute level, short-window RMS peak/trough, crest factor, headroom, DC offset, zero crossings, and clipped-sample counts. Choose CSV for spreadsheets or the text report for a quick terminal-style readout.

### Worked example

For the preset constant half-scale WAV, JSON output includes values like:

```json
{
  "sample_rate": 8000,
  "channels": 1,
  "duration_s": 0.01,
  "per_channel": [
    {"channel": 1, "label": "M", "rms_dbfs": -6.021, "peak_dbfs": -6.021}
  ]
}
```

The same input in CSV mode starts with:

```csv
channel,label,samples,rms_dbfs,peak_dbfs,average_dbfs
1,M,80,-6.021,-6.021,-6.021
```

## Limits and edge cases

- Input must be pasted as base64 or hex file bytes; browser file upload is not used for this pure text surface.
- The decoded input byte cap is 64 MiB and analysis stops after 30,000,000 frames per channel, reporting `truncated: true` if that cap is reached.
- Opus, AC-3, DTS, encrypted media, and video-only files are not decoded by the pure-Rust backend.
- RMS and peak are sample-based dBFS values, not LUFS or inter-sample true peak. Use a loudness-meter tool when broadcast LUFS or true-peak compliance is required.
- The short RMS window controls only the `rms_peak_dbfs` and `rms_trough_dbfs` fields; it does not change whole-file RMS, average, or sample peak.

## FAQ

<details>
<summary>Is RMS the same as LUFS?</summary>

No. RMS is an electrical average of sample energy over the file or a short window. LUFS uses a perceptual weighting and gating model. This report is useful for technical level checks, clipping, headroom, and channel balance; it is not a broadcast loudness compliance meter.

</details>

<details>
<summary>Why does a constant half-scale sample read about -6.021 dBFS?</summary>

dBFS is calculated with `20 * log10(linear_amplitude)`. A linear amplitude of `0.5` is therefore about `-6.021` dBFS. Digital full scale is `0` dBFS, and silence is floored to `-120` dBFS so JSON and CSV stay numeric.

</details>

<details>
<summary>Does the tool downmix stereo before measuring?</summary>

No. Each channel is measured separately and the `overall` row aggregates all samples across all channels. That makes left/right imbalances, channel-specific clipping, and DC offset visible instead of hiding them in a downmix.

</details>

<details>
<summary>Can it detect true peaks between samples?</summary>

No. The `peak_dbfs` field is a sample peak. It does not oversample to estimate inter-sample true peaks. Use a dedicated loudness or true-peak analyzer when delivery specs require dBTP values.

</details>
