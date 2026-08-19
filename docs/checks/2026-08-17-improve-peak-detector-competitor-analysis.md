# peak-detector — competitor scan + design decisions (2026-08-17)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4. All
competitor notes below are **paraphrased observations of published behaviour and documented
parameter lists** — no copy, branding, wording, or assets were reused. Out-of-model items are
listed, never built.

Backlog row: `peak-detector` / *"Finds local maxima/minima in a 1-D signal with configurable
prominence, height, and minimum-distance thresholds."* / `type_hint = pure`.

## Duplicate check

`ls blocks/ | grep -iE 'peak|maxima|minima|signal|extrem'` returns only `normalize-peak`, which is
an **audio** sample-peak normalizer (find the loudest sample, apply one constant gain to hit a
target dBFS) — a different domain and a different output (an audio file, not a peak table). The
skiplist entries mentioning "peak" (`true-peak-checker`, `video-audio-peak-normalize`, `limiter`)
are all audio-loudness rows, not 1-D signal peak finding. No duplicate; build proceeds.

## Competitors reviewed (top 3 real implementations)

### 1. SciPy — `scipy.signal.find_peaks` (the de-facto reference)
- Parameters: `height`, `threshold`, `distance`, `prominence`, `width`, `wlen`, `rel_height`
  (default `0.5`), `plateau_size`. Each of `height`/`threshold`/`prominence`/`width`/`plateau_size`
  accepts a scalar **or a `(min, max)` pair**, so both a floor and a ceiling are expressible.
- Documented filter evaluation order: `plateau_size` → `height` → `threshold` → `distance` →
  `prominence` → `width` (cheapest filters first, so later ones see fewer candidates).
- Flat-topped ("plateau") peaks: the **middle** sample of the flat run is returned, rounded down
  for even-length plateaus.
- Returns a `properties` dict whose keys appear only when the matching argument was supplied:
  `peak_heights`; `left_thresholds`/`right_thresholds`; `prominences`/`left_bases`/`right_bases`;
  `widths`/`width_heights`/`left_ips`/`right_ips`; `plateau_sizes`/`left_edges`/`right_edges`.
- Minima are not a parameter — the documented idiom is to negate the signal and re-run.
- Source: <https://docs.scipy.org/doc/scipy/reference/generated/scipy.signal.find_peaks.html>

### 2. MATLAB — `findpeaks` (Signal Processing Toolbox)
- Name-value options and defaults: `MinPeakHeight` (`-Inf`), `MinPeakProminence` (`0`),
  `Threshold` (`0`), `MinPeakDistance` (`0`), `MinPeakWidth` (`0`), `MaxPeakWidth` (`Inf`),
  `NPeaks` (all), `SortStr` (`none`, else `ascend`/`descend`), `WidthReference`
  (`halfprom`, else `halfheight`), `Annotate` (plot-only).
- Outputs: peak values `pks`, locations `locs`, widths `w`, prominences `p`. Locations can be
  reported against a supplied x-vector/sample rate rather than raw indices.
- Prominence definition: the minimum vertical distance the signal must descend on either side of
  the peak before climbing back above it or reaching an endpoint. Width: the distance between the
  two points where the signal crosses a reference line at half prominence (or half height).
- Source: <https://www.mathworks.com/help/signal/ref/findpeaks.html>

### 3. O'Haver — "Peak Finding and Measurement" (`findpeaksG`/`autofindpeaks`/`iPeak`, UMD)
- Derivative-based detection: smooth the first derivative, then look for downward-going
  zero-crossings, which suppresses noise-triggered false peaks.
- Parameters: `SlopeThreshold` (rejects broad features), `AmpThreshold` (rejects short peaks),
  `SmoothWidth` (noise reduction in the derivative — guidance: about half the peak half-width in
  points), `FitWidth`/`PeakGroup` (how many top-of-peak points feed the height/width estimate).
- Output is a **peak table**: one row per peak with peak number, position, height, width (FWHM)
  and area — i.e. tabular, exportable output is the expected deliverable, not just indices.
- Practical guidance emphasised: smoothing plus threshold tuning is what makes detection
  selective on noisy data.
- Source: <https://terpconnect.umd.edu/~toh/spectrum/PeakFindingandMeasurement.htm>

Browser-native competition is thin: a general web search for a paste-your-data peak finder turned
up CSV viewers and desktop/MATLAB scripts (e.g. the GitHub `peak-finder` desktop app that parses a
text file and exports peaks to Excel), not an in-browser tool. That is the gap this tool fills — a
no-install, no-upload peak table you can deep-link.

## Table stakes → decision

| Capability (seen in ≥1 competitor) | Verdict | How it lands here |
| --- | --- | --- |
| Minimum peak height / amplitude floor | **in-model — built** | `min_value` (blank = no floor) |
| Height *ceiling* (SciPy's `(min, max)` height tuple, MATLAB `MaxPeakWidth` analogue) | **in-model — built** | `max_value` (blank = no ceiling); band filter applies to maxima and minima alike |
| Minimum prominence | **in-model — built** | `min_prominence` (default `0` = off), computed with SciPy/MATLAB's base-walk definition |
| Minimum distance between peaks | **in-model — built** | `min_distance` in samples (default `0` = off), tallest-first suppression like SciPy |
| `threshold` (vertical drop to *both* immediate neighbours) | **in-model — built** | `threshold` (default `0` = off) |
| Minimum width | **in-model — built** | `min_width` in samples, measured at `rel_height` of prominence |
| `rel_height` reference for width (SciPy `0.5`; MATLAB `WidthReference=halfprom`) | **in-model — built** | `rel_height`, default `0.5` (= half-prominence, matching both) |
| Local **minima** (valleys) | **in-model — built** | `mode = maxima \| minima \| both` — negate-and-rerun done internally so the user never has to |
| Plateau / flat-top handling | **in-model — built** | flat runs collapse to their middle sample (SciPy's rule, rounded down); plateau length reported per peak |
| Smoothing before detection (O'Haver `SmoothWidth`) | **in-model — built** | `smooth` = odd moving-average window, default `0` = off |
| Cap the number of returned peaks (MATLAB `NPeaks`) | **in-model — built** | `max_peaks`, default `0` = all |
| Sort order (MATLAB `SortStr`) | **in-model — built** | `sort_by = position \| prominence \| value` |
| Peak **table** output with position/value/prominence/width (O'Haver, MATLAB) | **in-model — built** | text table, plus `json` and `csv` output formats for export |
| Peak bases / interpolated width endpoints (`left_bases`, `left_ips`, …) | **in-model — built** | present in the `json` output |
| Flexible pasted-data separators (CSV/TSV/newline) | **in-model — built** | `separator = auto \| comma \| newline \| space \| semicolon \| tab \| pipe` |
| Documented filter evaluation order | **in-model — built** | SciPy's order followed and stated on the page |
| Peak **area** (O'Haver peak table) | **considered, rejected** | area needs a baseline model + curve fit to be meaningful; a naive trapezoid between bases would be a confidently wrong number. Prominence + width already answer "how big is this peak". |
| Gaussian/Lorentzian curve fitting for sub-sample peak position (O'Haver `FitWidth`, iPeak) | **out-of-model** | a non-linear least-squares fitter is a much larger dependency and a different tool ("peak fitting"), not peak *detection* |
| Plot with annotated peaks / interactive threshold dragging (MATLAB `Annotate`, iPeak) | **out-of-model** | the page surface is a form + text/JSON result; there is no interactive chart canvas |
| x-vector / sample-rate location mapping (MATLAB `findpeaks(y, Fs)`) | **considered, rejected** | keeps the schema honest: indices are 0-based and unambiguous, and a caller who has an x-axis can map `index` themselves. Adding `x_start`/`x_step` would double the "which units is width in?" confusion. |
| `wlen` (bounded prominence search window) | **considered, rejected** | it is a speed knob for very long signals; with a 20 000-sample cap the full base walk is fast, and a half-understood `wlen` silently changes prominences |
| Excel/`.xlsx` export (GitHub desktop peak-finder) | **out-of-model** (partly covered) | no spreadsheet writer in-model; `format=csv` gives an import-ready file via the page's download link |
| Multi-column / whole-CSV-file upload | **out-of-model** | pure text-in/text-out tool; the user pastes one column |

## UX patterns adopted

- **Preset chips** (`[[example]]`): competitors ship worked examples and parameter recipes, so the
  page gets one-click presets — a clean synthetic signal, a noisy signal cleaned up with smoothing
  + prominence, a valley (minima) run, and a CSV-export run.
- **Right control for the data**: `mode`/`separator`/`sort_by`/`format` are `Param::enumv` →
  `<select>` with friendly `[input.labels]`; `rel_height` is a bounded `0–1` value; `data` is
  `multiline = true` so a pasted column keeps its newlines.
- **Every text/number field carries a real placeholder** that doubles as a worked example.
- **Stated limits on the page**: 20 000 values per run, 0-based indices, what `0`/blank means for
  each filter, and the documented filter order.

## Not copied

No competitor text, marketing copy, table wording, parameter help strings, branding, or assets
were reproduced. Parameter *names* that are effectively domain vocabulary (prominence, threshold,
distance, width) are used descriptively; all page and descriptor copy is original.
