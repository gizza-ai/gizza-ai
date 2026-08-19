# fft-analyzer — competitor analysis (2026-08-09)

Scan run BEFORE implementation, per `/create-next-tool` step 4. Everything below is
**paraphrased** from public tool pages — no competitor copy, branding, or trademarks are
reproduced, and nothing from their pages is used as our page text.

Backlog row: `fft-analyzer` — "Computes the discrete Fourier transform of a signal and returns
the frequency-magnitude spectrum." (type hint: pure)

## Duplicate check

`ls blocks/ | grep -iE 'fft|fourier|freq|spectr|signal|wave|dft'` → no Fourier/DFT block exists.
Nearest neighbours are all distinct:

- `blocks/frequency-distribution` — tallies **symbols** (characters/bytes/n-grams) in text, not
  numeric spectral analysis.
- `blocks/spectral-eq-match`, `blocks/audio-eq`, `blocks/audio-filter` — ffmpeg filtergraphs that
  emit **audio media**, never a numeric spectrum table.
- `blocks/waveform-image` — ffmpeg `showwavespic`, a time-domain PNG.
- `blocks/descriptive-stats`, `blocks/csv-stats` — summary statistics, no transform.

Skiplist entries mentioning spectra (`spectrogram`, `wavelet-denoise`, `video-beat-detector`,
`chord-recognizer`) are all **audio-input → analysis** rows blocked because gizza has no decoded-PCM
→ pure-DSP bridge. This row is different: the input is a **pasted numeric sample list**, which is the
ordinary pure typed-in → typed-out shape (`Input::None`, text params). Not a duplicate; buildable.

## Competitors reviewed (5)

Two initially-picked pages were unreachable (one TLS-chain failure, one DNS failure) and were
replaced with the next real tools rather than running with fewer, per the skill.

### 1. MiniWebtool — FFT calculator
- **Inputs:** sample list (real **or complex**, e.g. `3+4i`, `-2i`); separators comma / space /
  semicolon / newline. Sample rate (Hz). Window: rectangular (default), Hann, Hamming, Blackman.
  FFT length: next power of two (default), input length, double power of two. Spectrum view:
  auto / one-sided / full.
- **Output:** per-bin frequency, real, imaginary, magnitude, magnitude normalised by N, phase in
  degrees; a dominant-peak list; magnitude-spectrum plot; waveform plot; copyable CSV export.
- **UX:** four preset example buttons (short sine-like sequence, windowed pulse, complex rotating
  phasor, clean sinusoid); states Δf = fs / N.
- **Limits:** none stated.

### 2. Sooeet — online FFT calculator
- **Inputs:** samples separated by spaces/commas; integer, decimal, and scientific notation all
  accepted. Sampling rate in Hz. Optional plot title / axis labels.
- **Behaviour:** automatically zero-pads to the next power of two; recommends power-of-two lengths.
- **Output:** amplitude-units-peak (√(Re²+Im²)/N), dB (20·log₁₀ of that), phase in radians,
  real and imaginary lists, sample count.
- **Inverse:** yes — a forward/inverse toggle.
- **Limits:** max 2¹⁸ = 262,144 samples; warns about memory/CPU beyond that.
- **UX:** radio buttons for y-only vs x/y-paired output, negative-vs-positive frequency ordering,
  zoom, window-function dropdown.

### 3. AI Math Calculator — online FFT calculator
- **Inputs:** comma/space-separated samples (a 32-point signal preloaded), sample rate in Hz,
  window dropdown: none (rectangular, default) / Hann / Hamming.
- **Output:** per-bin frequency (Hz), magnitude **normalised 2/N** (1/N for DC and Nyquist), phase
  in degrees; headline "dominant frequency" + its peak amplitude; explicitly reports Δf = fs/N and
  the Nyquist frequency; one-sided spectrum chart plus an expandable bin table.
- **UX:** preset signal loaders (two-tone 2 Hz + 5 Hz, single 8 Hz tone, three-tone chord, DC +
  ripple); live re-render on parameter change; zero-pads to the next power of two automatically.
- **Copy:** FAQ covers reading the output, choosing a sample rate, DFT vs FFT, and spectral leakage.

### 4. Clac360 — Fourier transform calculator
- **Inputs:** comma-separated samples or an expression; sampling frequency with Hz/kHz/MHz unit
  selector; sample count N; optional zero-padding; transform type DFT/FFT/IDFT/IFFT; window
  rectangular / Hamming / Hann / Blackman / flat-top.
- **Output:** frequency, real, imaginary, magnitude spectrum, phase spectrum, **power spectrum**;
  CSV export; reconstruction-error readout for inverse transforms.
- **Limits:** warns that N > ~10⁶ is slow in-browser; Nyquist/aliasing warning when the signal
  frequency exceeds fs/2.
- **Copy:** five worked step-by-step examples (including the classic 50 Hz + 120 Hz two-tone case)
  and an FAQ on time vs frequency domain, reversibility, and CFT/DFT/FFT selection.

### 5. Academo — spectrum analyzer demo
- **Inputs:** audio (preset clips or microphone/file), not a numeric list — the closest widely-used
  "see a spectrum" tool.
- **Output:** live spectrogram; linear/logarithmic **frequency-axis toggle**; colour-mapped
  intensity.
- **Limits:** browser-support caveats stated.
- Included for UX/positioning reference only; its audio-capture input is a different tool shape.

## Table stakes → decision

| Table stake | Seen in | Decision |
| --- | --- | --- |
| Flexible separators (comma/space/semicolon/newline), scientific notation | 1,2,3,4 | **Built** — `data` parser accepts all four separators + `1e3` style |
| Complex sample input (`3+4i`, `-2i`) | 1,4 | **Built** — full complex parser, two-sided spectrum auto-selected |
| Sample rate → real frequency axis | all | **Built** — `sample_rate` (Hz), default 1.0 (normalised) |
| Window functions | 1,2,3,4 | **Built** — rectangular / hann / hamming / blackman / blackman-harris / flattop, with coherent-gain amplitude correction |
| Zero-pad to next power of two vs exact length | 1,2,3,4 | **Built** — `pad` = `pow2` (default) / `none` (exact-length DFT) |
| One-sided vs two-sided spectrum | 1,2 | **Built** — `spectrum` = auto / one-sided / two-sided |
| Magnitude / normalised magnitude / amplitude / dB / power scaling | 1,2,3,4 | **Built** — `scale` = amplitude / magnitude / normalized / db / power |
| Phase, degrees or radians | 1,2,3,4 | **Built** — `phase_unit` = degrees / radians, with a noise-floor guard |
| Real + imaginary columns | 1,4 | **Built** — both columns in csv/json output |
| Dominant-peak list | 1,3 | **Built** — `peaks` (0–20, default 5), local-maximum ranked |
| Δf resolution + Nyquist reported | 3,4 | **Built** — both in every output format's header |
| Aliasing / Nyquist warning | 4 | **Built** — a note fires when a top peak sits in the last bin |
| CSV export | 1,4 | **Built** — `format = csv` (the page also gets the shared Download link for text output) |
| Spectrum chart | 1,3,4,5 | **Built as `format = chart`** — a Unicode bar chart of the one-sided spectrum, the in-model equivalent of their canvas plots |
| Preset example buttons | 1,3 | **Built** — five `[[example]]` chips (two-tone, single tone, DC + ripple, complex phasor, unwindowed leakage) |
| DC removal / detrend | common practice | **Built** — `remove_dc` boolean (default off) |
| Decimal-place control | — | **Built** — `decimals` 0–12, default 4 |

## Considered and rejected (in-model, declined on judgment)

- **Inverse FFT / IFFT** (competitors 2 and 4). Fully in-model mathematically, but it inverts this
  tool's output contract: the result would be a time-domain sample list, not a frequency-magnitude
  spectrum, which is what this backlog row specifies. Bolting a second output shape onto the same
  descriptor would make every parameter conditionally meaningful (windowing, scaling, peaks and the
  one-sided view are all forward-only). The right home is a separate `inverse-fft` tool.
- **Hz/kHz/MHz unit selector** (competitor 4). A second unit param buys nothing over typing the rate
  in Hz — `sample_rate = 48000` and "48 kHz" are the same keystroke count — and it doubles the ways
  a frequency column can be misread.
- **Expression input** (`sin(2π·50·t)`, competitor 4). That is a signal *generator*, a distinct tool;
  this one analyses samples the user already has.
- **Logarithmic frequency axis** (competitor 5). Meaningful for a rendered plot, not for a numeric
  bin table; the `chart` format's bins are already linear by construction.

## Out of model (not built)

- **Interactive canvas/SVG plots with zoom and hover readouts** (1,2,3,4). The generic page renders
  a text/number result; there is no per-tool plotting surface here, and adding one would be a
  site-repo concern. `format = chart` covers the "see the shape of the spectrum" need in text.
- **Live microphone / audio-file capture and streaming spectrograms** (5). Requires an audio decode
  path into a pure block; gizza's only decode path is the ffmpeg page runtime, which cannot emit a
  text/number result (`tools/generator/assets/runtime/tool.js` rejects `runtime=ffmpeg` +
  `format=text`). Same blocker documented in the skiplist for `chord-recognizer`.
- **Accounts, saved sessions, or server-side batch processing.** Out of the browser-local model.

## Stated limits we adopt

Competitor 2 caps at 2¹⁸ samples; competitor 4 warns beyond ~10⁶. We cap at **65,536 input samples**
(zero-padded FFT length up to 131,072), and additionally cap the exact-length path (`pad = none`,
non-power-of-two N) at **4,096 samples** because that path is an O(N²) direct DFT. Both caps are
stated on the page and produce actionable errors, not truncation.
