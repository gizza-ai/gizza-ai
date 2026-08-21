# Competitor analysis — mel-spectrogram-generator (2026-08-21)

## Sources skimmed

| Reference | What users expect | In model? | Decision |
| --- | --- | --- | --- |
| librosa `feature.melspectrogram` docs | `n_fft`, `hop_length`, mel-band count, `fmin`/`fmax`, Slaney/HTK-style mel behavior, power/log transforms | yes | Expose FFT, hop, bands, frequency range, mel scale, and dB/power/magnitude controls. |
| torchaudio `MelSpectrogram` docs | STFT settings, window choice, sample rate, mel scale, `center` padding, power spectrogram defaults | yes | Add window enum, `center` checkbox, and optional resample rate for repeatable ML preprocessing. |
| MATLAB `melSpectrogram` examples | Window/overlap, 64+ mel bands, frequency range, time-frequency image interpretation | yes | Provide preset chips and document that low frequencies render at the bottom of the PNG. |
| SpectrogramGenerator-style examples | Fixed output width, magma-like palettes, worked audio examples | yes | Add width/height controls, colormap enum, and PNG download rendering. |

## Table-stakes controls

- Input: local audio file or pasted bytes; decode common WAV/FLAC/MP3/OGG/M4A containers locally.
- Analysis: `n_fft` default 2048, `hop_length` default 0 (`n_fft/4`), `n_mels` default 128, `fmin`/`fmax`, optional resample rate, channel selection, and centered frames.
- Scaling: dB/log default with dynamic range, plus linear power and magnitude modes.
- Visual output: PNG, selectable colormaps (`magma`, `inferno`, `viridis`, `plasma`, `turbo`, grayscale variants), width and height controls, natural matrix-size mode.
- UX: sliders for bounded numeric values, select labels for enums, checkbox for centered frames, preset chips for librosa-style, speech 16 kHz, and natural-size outputs.

## Out of model / intentionally not built

- Returning NumPy/PyTorch tensors or `.npy` arrays: this public toolkit ships local browser/CLI tools, not Python runtime objects.
- Live microphone capture and streaming spectrograms: page file upload is the current local input model.
- GPU/model frontend integration: this tool prepares a diagnostic PNG and does not run a classifier.

## Gap closure

The implementation is pure Rust rather than ffmpeg: audio decode uses the same wasm-proven Symphonia stack as neighboring audio feature tools, STFT uses `rustfft`, the mel filterbank is implemented in core, and PNG rendering uses the Rust `image` encoder. The page exposes the controls above, renders the returned PNG in the media slot, and keeps the generated CLI schema as the source of truth.
