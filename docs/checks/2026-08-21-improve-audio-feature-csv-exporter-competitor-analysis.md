# Competitor analysis — audio-feature-csv-exporter (2026-08-21)

Backlog tool: exports frame-level audio features (RMS, centroid, ZCR, rolloff, flatness) to CSV for analysis.

## Surveyed references

| Reference | Table-stakes observed | Fit decision |
| --- | --- | --- |
| librosa feature extraction examples / API | Frame and hop sizing, centered frames, windowing, RMS, spectral centroid, zero-crossing rate, rolloff percentage, flatness, bandwidth, time-aligned output. | In model. Implemented matching controls for frame_ms, hop_ms, center, window, rolloff_percent, RMS/centroid/ZCR/rolloff/flatness/bandwidth toggles and time/frame columns. |
| Meyda Web Audio feature extractor | Browser-side extraction, common low-level features, feature selection, spectral flatness/centroid/rolloff/ZCR/RMS style measures. | In model. Browser page runs locally via wasm, includes feature toggles and preset chips. Real-time microphone streaming is out of model for this repository's generated page pattern. |
| pyAudioAnalysis / open-source audio analysis scripts | CSV-oriented feature tables, frame-level and clip-level features, spectral flux, bandwidth, short-term analysis, many additional ML features. | Partly in model. Added optional flux and bandwidth because they are pure DSP and useful table stakes. MFCC/chroma/classifier pipelines are out of model for this tool because they require a larger feature stack and/or trained models. |
| QxLabIreland audio-feature-extraction script | Exports typical audio research features and aggregates frame-level features to clip-level CSV values. | Partly in model. Frame-level export is implemented. Clip-level summary statistics are omitted to keep the tool focused on raw frame tables; users can aggregate CSV downstream. |

## In-model capabilities shipped

- Audio upload/pasted bytes decoded locally with Symphonia-proven codecs.
- CSV, TSV, and JSON output modes.
- Frame/hop controls with common defaults: 25 ms frame and 10 ms hop.
- Window choice: Hann, Hamming, Blackman, rectangular.
- Optional librosa-style centered frames.
- Channel control: downmix, left, right.
- Optional resampling to a fixed analysis rate.
- Feature toggles for RMS, spectral centroid, zero-crossing rate, spectral rolloff, spectral flatness, spectral bandwidth, and spectral flux.
- Rolloff percentage control, RMS dBFS/linear units, flatness ratio/dB units, decimal precision, time column, and frame index column.
- Preset chips for default feature set, all features, librosa-style framing, onset-detection flux, and JSON metadata.

## Out-of-model or deliberately deferred

- Microphone live streaming and real-time plots: the generic tool page is file/input driven, not an audio capture UI.
- MFCC, chroma, contrast, tonnetz, classifier training, clustering, and clip-level ML summaries: useful but significantly larger than the requested low-level feature table and may warrant separate tools.
- Multi-file batch processing: current page pattern is one uploaded audio file; batch aggregation belongs in a future archive/multi-input workflow.

## Verification focus

The implementation should prove: default CSV has real frame rows; output modes and feature toggles change headers; non-default checkbox state works; rolloff percent and resample boundaries are exercised; CLI can parse an uploaded-file-equivalent base64 fixture; and the page deep-link populates controls before running on an uploaded audio fixture.
