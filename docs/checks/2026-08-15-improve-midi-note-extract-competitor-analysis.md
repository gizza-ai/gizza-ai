# midi-note-extract — competitor analysis (2026-08-15)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md`. All findings are
paraphrased; no competitor copy, branding, or trademarks were reused.

## Dup check vs the existing MIDI block

`blocks/midi-to-json` parses SMF bytes with `midly` and emits **JSON** in three shapes
(`notes` = per-track paired notes, `events` = loss-less event stream, `summary` = counts/BPM).
It has no delimited-text output, no cross-track flat table, no unit switch (its notes always carry
ticks *and* seconds), no velocity normalization, and no track/channel filter or row ordering.

Verdict: **not a duplicate — build it.** midi-note-extract is the tabular sibling: one flat
CSV/TSV note table across all tracks, aimed at spreadsheets / pandas / plotting, with the
unit, precision, ordering and filtering controls that a data table needs. midi-to-json stays the
structured-JSON view. Overlap is limited to the shared "parse SMF, pair note-on/off" step, which
each block implements against `midly` in its own dependency-light core (the midi-to-json core does
not export its pairing/tempo helpers, so there is nothing public to reuse).

## Competitors reviewed (top 3)

### 1. MIDICSV (fourmilab.ch) — the reference CLI implementation
- Bidirectional: `midicsv` MIDI→CSV, `csvmidi` CSV→MIDI; explicitly loss-less round-trip.
- Row shape is **event-level**, not note-level: `track, time, type, …params`, e.g. `Note_on_c`
  and `Note_off_c` as separate records. Times are absolute MIDI clock ticks only.
- Records sorted by track, then time. Track numbering is 1-based, header records use track 0.
- Options are minimal (`-v` verbose, `-x`/`-z` on the reverse direction); pipeline/stdin oriented.
- Documented weakness: assumes well-formed input, no semantic validation of unmatched note-on/off.

### 2. FMP notebooks "Symbolic Format: CSV" (audiolabs-erlangen.de) — the academic convention
- Defines the canonical note-list row: `start, duration, pitch, velocity, label`.
- `pitch` is the MIDI note number; **velocity normalized 0.0–1.0**; `label` carries instrument /
  voice / staff text.
- Delimiter is a **semicolon** in their examples; times in seconds (or measures).
- Consumed directly by piano-roll plotting code — i.e. the table is the analysis input format.

### 3. `algopenne/midi-csv` (GitHub) — the pandas-oriented script
- Columns: `note_name`, `start_time`, `duration`, `velocity`, `tempo` (BPM as its own column).
- Emits human-readable note names (e.g. `E-4`) rather than only numbers; decimal times.
- Batch mode over a directory of files; no per-file options; multi-track behavior undocumented.
- (A fourth hit, `bonnetn/midiparser`, is the same shape with frequency instead of pitch —
  noted, not counted as one of the three.)

## Table stakes → where each landed

| Capability | Seen in | Our decision |
|---|---|---|
| Row per note with start / duration / pitch / velocity | all 3 | **built** — every column set carries these four |
| MIDI note number as `pitch` | FMP, MIDICSV | **built** — `pitch` column |
| Readable note name (`C4`) | midi-csv | **built** — `note_name` in standard/full |
| Times in seconds | FMP, midi-csv | **built** — `time_unit = seconds` (default), from the tempo map |
| Times in raw ticks | MIDICSV | **built** — `time_unit = ticks`, whole numbers |
| Times in beats/measures | FMP (measures) | **built** as beats (quarter notes); bar/beat notation not built (needs a time-signature grid — listed below) |
| Velocity normalized 0–1 | FMP | **built** — `velocity_scale = normalized` |
| Tempo (BPM) column | midi-csv | **built** — `tempo_bpm` in the full column set, tempo in force at note start |
| Track / instrument label column | MIDICSV, FMP (`label`) | **built** — `track`, `track_name` (RFC 4180 quoted) |
| Channel column | MIDICSV | **built** — `channel` |
| Semicolon delimiter | FMP | **built** — `delimiter = comma\|semicolon\|tab` |
| Header row | midi-csv (pandas) | **built** — `header` toggle, on by default |
| Sort by track then time | MIDICSV | **built** — `sort = time\|track\|pitch` |
| Channel/track extraction | MIDICSV (documented as an example script) | **built as a first-class filter** — `track` / `channel` accept `all` or a list |
| Decimal precision control | none (implicit) | **built** — `decimals` 0–6 |
| Unmatched note-on handling | MIDICSV explicitly does not resolve it | **improved** — held to the end of its track instead of dropped |

## Considered, not built (out of model or out of scope)

- **CSV → MIDI (the reverse direction).** MIDICSV's `csvmidi` half. A distinct tool, not a mode of
  this one; would need a MIDI writer and its own page. Not in this block's scope.
- **Loss-less event-level CSV** (control changes, pitch bends, meta events as rows). That view
  already exists in this repo as `midi-to-json` with `format = events`; duplicating it as CSV
  would blur the two tools. Listed, not built.
- **Bar : beat : tick position notation.** Needs a time-signature grid layered on the tempo map;
  the beats unit covers the common analysis case. Deferred as a future improvement.
- **Batch/directory processing** (midi-csv's mode). Out of model: gizza tools are single-input,
  browser-local, no server; the CLI can be looped by the caller instead.
- **File upload widget for `.mid`.** The page's pure-wasm path takes text fields; users paste
  base64/hex, matching the sibling `midi-to-json` page. A binary file input for pure (non-ffmpeg)
  blocks is a platform feature, not a per-tool one — noted for the generator, not built here.
- **Piano-roll visualization** (FMP's plotting step). Out of scope for a text-output tool.
- **Frequency (Hz) column** (`midiparser`). Trivially derivable from `pitch`
  (`440 · 2^((pitch−69)/12)`), and no reviewed mainstream tool ships it as a default column;
  rejected to keep the row shape tight.

## Verification notes

Unit tests cover each advertised value form (all three column sets, all three time units, both
velocity scales, all three delimiters, all three sort orders, header on/off, track and channel
filters, the 50,000-row cap at and one over the boundary, SMPTE-timecode rejection for beats,
base64 auto-detection, and non-MIDI/empty input errors). The page spec exercises the same forms
through the browser, including a `?param=` deep link and a non-default checkbox state.
