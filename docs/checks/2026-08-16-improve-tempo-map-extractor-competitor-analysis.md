# tempo-map-extractor — competitor analysis (2026-08-16)

Scan run BEFORE implementation, per the create-next-tool step 4 rule. All competitor
observations are **paraphrased** — no competitor copy, branding, or trademarks are reproduced
here or in the shipped page.

## What the backlog row asks for

> Tracks a variable tempo curve over time and exports BPM-versus-time rather than one global
> tempo.

The distinguishing deliverable is a **tempo map** — a BPM-vs-time series — not a single global
BPM number.

## Scope decision: what the input can be

gizza is pure-Rust + ffmpeg with no ML models. Audio-input tempo *detection* is already
skiplisted repo-wide (`bpm-detect`, `video-beat-detector`, `audio-to-midi`, `chord-recognizer`):
ffmpeg has no beat/onset/tempo-measuring filter (only `atempo`, which *changes* playback speed),
and there is no decoded-PCM→pure-wasm DSP bridge on any headlessly-verifiable surface.

So this tool takes the artefact those detectors *produce* and every DAW/analysis tool can already
export: **a list of beat times**. Sources users actually have — Audacity label tracks, Sonic
Visualiser / aubio beat output, DAW markers, tapped timestamps, a MIDI-derived beat list, a
CSV column of onsets. It converts them into the BPM-versus-time curve, statistics, and the
DAW-ready tempo-map exports. That is `pure` (matching the backlog `type_hint`), fully verifiable
on the CLI and page, and does not overlap the deferred audio-analysis class.

## Duplicate check

- `ls blocks/ | grep -iE 'tempo|bpm|beat|midi'` → `midi-tempo-change`, `midi-to-json`,
  `midi-note-extract`, `midi-track-splitter`, `guitar-tab-to-midi`. Confirmed by reading their
  cores:
  - `blocks/midi-tempo-change` — *rewrites* a MIDI file's tempo events and returns a retimed
    `.mid`. Input is a MIDI file, output is a MIDI file. No BPM-vs-time series.
  - `blocks/midi-to-json` — parses SMF bytes into JSON; its header carries a `tempos` array as
    one field among many. Input is a MIDI file; it does not take beat times, does not compute
    per-beat instantaneous BPM, has no smoothing/grid/statistics, and emits no CSV/label/tempo-map
    exports.
  Neither takes a beat-time list nor produces a tempo curve → **not a duplicate**.

## Competitors reviewed

Four tools were reachable and skimmed (one candidate, a tap-tempo site returning HTTP 403 to the
fetcher, was replaced by a fourth reachable tool per the "replace an unreachable one" rule).

### C1 — Browser BPM finder with a tempo curve (audio-input, no upload)
- Accepts common audio containers; analysis is in-browser.
- Reports a single headline BPM for steady material, **and a range plus a tempo curve plotted
  over the track duration** when the tempo varies.
- Hover read-out: time position + BPM at that point on the curve.
- Confidence colouring (high/uncertain regions) and explicit honesty labels when the material
  does not fit a steady pulse.
- **Half (½×) / double (2×) "octave" buttons** to correct a reading that locked onto the wrong
  metrical level.
- Manual tap fallback for material without a steady pulse.
- Genre BPM reference table; cross-link to a delay-time calculator.

### C2 — Tap-tempo / BPM session tool with statistics and exports
- **Tap pulse selection**: the tapped pulse can be a quarter note, eighth, dotted quarter, or
  half note; the reported tempo is converted back to quarter-note BPM.
- **Stabilisation window**: average over the last *N* tap intervals (roughly 2–16) or a rolling
  window of 1–8 seconds.
- **Ignore taps faster than X ms** (roughly 80–500 ms) to swallow accidental double taps;
  **reset after a pause of N seconds**; undo-last-tap; reset session.
- Read-outs: rolling BPM, instantaneous BPM from the newest interval, tapped-pulse BPM, beat
  duration, tap count.
- Statistics: **jitter as a standard deviation in milliseconds**, a **drift band** (largest
  recent deviation from the rolling average), a confidence/stability rating on a 4-step scale,
  and a conventional tempo-family name (Andante/Moderato/Allegro…).
- Charts: a live BPM curve and a drift-deviation plot.
- **Tap ledger table**: per tap — index, interval in ms, pulse classification, BPM at that tap,
  deviation from the average.
- Exports: CSV (chart data and session), JSON (full session), DOCX, and chart images.

### C3 — Timeline marker converter (30+ formats)
- Accepts markers from NLE/DAW exports (XML/EDL/TXT/MIDI), subtitle formats, spreadsheets, and
  plain text; exports to editor formats plus CSV/XLSX/JSON/PDF and shell scripts.
- **Multiple time representations parsed and emitted**: `m:ss`, `hh:mm:ss:ff` (frames), decimal
  seconds, raw frame counts; the user picks the output duration unit.
- **Shift all markers** by a positive/negative offset.
- Filtering by range, removing empty markers, merging files, de-duplicating identical timecodes.
- Field selection for the export (id, timecode, duration, name, comment).

### C4 — BPM/tempo calculator with a note-duration table
- Numeric BPM field, time-signature dropdown (2/2, 2/4, 3/4, 4/4, 3/8, 6/8), tap pad with tap
  counter, calculate + reset buttons; defaults are 120 BPM, 4/4.
- Outputs a full **note-duration table** — beat, bar, whole/half/quarter/eighth/sixteenth/
  thirty-second durations in seconds and milliseconds.
- Averages the most recent taps; copy-to-clipboard with confirmation.

## Table stakes → decision

| # | Table stake | Seen in | Decision |
|---|---|---|---|
| 1 | BPM-vs-time curve, not one global number | C1, C2 | **In model** — the core output: one row per beat interval, `time_seconds,bpm`. |
| 2 | Multiple time formats in (decimal seconds, `m:ss.mmm`, `h:mm:ss.mmm`, `hh:mm:ss:ff` frames, milliseconds) | C3, C2 | **In model** — auto-detecting parser + `time_unit` (`auto`/`seconds`/`milliseconds`) + `fps` for frame timecode. |
| 3 | Tolerant input parsing (label tracks, CSV columns, comments, blank lines) | C3 | **In model** — first time-like token per line; tab/comma/space separated; `#`/`//` comments and blank lines skipped; a single comma/space-separated line also works. |
| 4 | Tapped-pulse → quarter-note conversion (eighth, dotted quarter, half…) | C2 | **In model** — `beat_unit` enum (whole … sixteenth incl. dotted and triplet eighth). |
| 5 | Half / double "octave" correction | C1 | **In model** — covered exactly by `beat_unit` (`half` doubles, `eighth` halves); called out in the parameter description and FAQ so the intent is discoverable. |
| 6 | Smoothing / stabilisation window over N intervals | C2 | **In model** — `smoothing` (window in beats, 1–64) + `smooth_method` (`mean`/`median`). Centred window, documented. |
| 7 | Ignore taps closer than X ms (double-tap guard) | C2 | **In model** — `min_interval_ms`. |
| 8 | Shift all times by an offset | C3 | **In model** — `offset_seconds` (accepts negatives). |
| 9 | Rounding control | C2, C4 | **In model** — `decimals` (0–4). |
| 10 | Jitter (std-dev ms), drift band, min/max/mean/median, stability rating | C2 | **In model** — the `summary` output and the JSON `summary` object report beat count, span, mean/median/min/max BPM, drift range, BPM std-dev, interval jitter in ms, a stability rating, an overall average BPM across the whole span, and a least-squares tempo slope in BPM per minute (speeding up vs slowing down). |
| 11 | Per-beat ledger table (index, interval ms, BPM, deviation) | C2 | **In model** — the `table` output (aligned columns: beat, time, interval ms, BPM, deviation from mean) plus the same fields in CSV/TSV/JSON. |
| 12 | Fixed-grid sampling so the curve is plottable at a regular rate | C1 (curve over duration) | **In model** — `grid_seconds`: resample the curve onto an even time grid (step-hold), instead of one row per beat. |
| 13 | Multiple export formats (CSV/TSV/JSON) | C2, C3 | **In model** — `output` = `csv`, `tsv`, `json`, `table`, `summary`. |
| 14 | DAW-ready tempo-map export | C1/C2 workflow context, C3 (editor formats) | **In model** — `output = midi` emits Standard-MIDI-File tempo-map rows (`tick,microseconds_per_quarter,bpm`) at a configurable `ppq`, and `output = audacity` emits a tab-separated label track (`start`, `end`, `<bpm> BPM`) that imports straight back into a label-track editor. |
| 15 | Note-duration table (beat/bar/eighth… seconds) for a BPM | C4 | **Out of model for this tool (considered, rejected)** — it is a property of *one* tempo, not of a tempo curve; folding a note-duration table into a per-beat series would produce dozens of columns per row. It belongs to a single-BPM calculator, not a tempo-map extractor. |
| 16 | Time-signature selector / bar numbering | C4 | **Out of model (considered, rejected)** — beat-time input carries no downbeat information, so any bar numbering would be a guess presented as fact. Users who tap downbeats already get the right curve via `beat_unit`. |
| 17 | Interactive tap pad, live tapping | C1, C2, C4 | **Out of model** — requires stateful real-time UI in the shared page runtime; gizza pages are a declarative form → one deterministic run. The tool consumes the *timestamps* such a pad produces. |
| 18 | Curve/drift charts, hover read-out, chart image export | C1, C2 | **Out of model** — the generic page renderer emits text/media, not per-tool interactive charts; adding a plotting widget would be a per-tool custom renderer. The `csv`/`grid_seconds` output is designed to paste straight into any plotting tool instead. |
| 19 | Audio-file input with automatic beat detection | C1 | **Out of model** — the repo-wide deferred class (no ffmpeg beat/onset filter, no decoded-PCM→wasm DSP surface); see the `bpm-detect` / `video-beat-detector` skiplist entries. The page copy states the tool starts from beat times. |
| 20 | Confidence colouring per curve region | C1 | **Out of model as colouring**, but the honest substance is kept: the stability rating and per-beat deviation column tell the user which parts of the curve are shaky. |
| 21 | Accounts, cloud sessions, sharing, newsletter, DOCX export | C1, C2 | **Out of model** — no backend, no accounts; DOCX is a document-authoring format outside this tool's remit. |

Every table stake above lands in the descriptor or in the out-of-model list; none was dropped
silently.

## Preset chips shipped (competitors ship presets/quick modes)

`[[example]]` chips on the page cover: a steady click track, a slowing ritardando, tapping on
half notes, a smoothed noisy tap sequence, a one-second grid curve, `mm:ss.mmm` timecode input,
the statistics summary, and the MIDI tempo-map export.

## Limits stated on the page

Maximum 20 000 beat times per run; at least 2 are required; times must strictly increase after
the offset and double-tap filtering; `grid_seconds` cannot be combined with `output = midi`
(tempo ticks must land on real beats); `fps` only applies to `hh:mm:ss:ff` timecode.
