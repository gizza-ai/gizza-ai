# guitar-tab-to-midi — competitor analysis (2026-08-08)

Scan run **before** implementation, per `/create-next-tool` step 5. Everything below is
**paraphrased** from public documentation/UI observation — no competitor copy, branding, or
trademarked text is reproduced or reused. Competitor names appear only to attribute a
capability observation.

## Search

One web search for the tool's function ("convert ASCII guitar tab to MIDI online converter").
The result set contained exactly one live browser-based converter (TAB2MIDI); the remaining
hits were desktop editors (Guitar Pro, TuxGuitar, Power Tab), the reverse direction
(MIDI → tab, e.g. tuttut), or forum threads. So the three profiles below are the three real,
reachable *ASCII-tab → MIDI* implementations available to study.

## Competitor profiles

### 1. TAB2MIDI (tab2midi.com) — browser tool, closest direct competitor
- **Input:** paste ASCII tab into a textarea; also accepts a PDF upload of a tab.
- **Parameters:** tempo in BPM; note duration as three presets (sixteenth = fast,
  eighth = standard, quarter = slow).
- **Tab syntax handled:** chords (simultaneous strings), double-digit frets, rests, and
  what it calls dynamic spacing detection.
- **Processing model:** entirely client-side (built on a JS MIDI-writing library); states
  nothing is uploaded.
- **Not documented / absent:** tuning selection, capo, transpose, instrument/program choice,
  velocity, string-count handling beyond 6-string guitar, technique characters
  (h/p/b/r/~ / \\ x), note-sustain behaviour, stated limits.

### 2. MIDI::Tab (Perl module, metacpan) — library, richest parameter surface
- **Tuning:** per-string root note supplied as absolute scientific pitch names (e.g. `A2`) —
  i.e. arbitrary custom tunings, but **no defaults**; the caller must always specify.
- **Note duration:** passed as a duration token (sixteenth `sn`, whole `wn`, …); no default.
- **Tempo / instrument:** not owned by the module — the caller sets them on the underlying
  MIDI object (patch change) before conversion.
- **Bar lines:** `|` characters are ignored.
- **Other modes:** drum tabs (letter codes → General MIDI percussion) and piano tabs, where
  digits 1–9 mean velocity rather than fret.
- **Extras:** an experimental control line for triplet timing.
- **Absent:** chords are not documented, no capo/transpose, no defaults anywhere.

### 3. Sarath18/guitar-tabs-to-MIDI (GitHub, open source) — reference implementation
- **Core mapping:** MIDI pitch = open-string pitch + fret number (the standard model our
  core uses too).
- **Pipeline:** strip unsupported characters → map digits to pitches → write a `.mid`.
- **Parameters:** tempo and instrument are set on the track, but not documented as
  user-configurable; standard tuning is hard-coded via open-string pitch constants.
- **Absent:** no documented syntax coverage (techniques, bar lines, multi-digit frets), no
  tuning/capo/transpose options, no stated limits.

## Table stakes → decisions

| Capability | Seen in | In/out of model | Decision |
| --- | --- | --- | --- |
| Paste ASCII tab, get a `.mid` download | 1, 2, 3 | in-model | **Built** — `tab` param; page renders a Download `.mid` button |
| Tempo (BPM) | 1, 3 | in-model | **Built** — `tempo`, default 120, 20–400, page `slider` |
| Note duration per tab column | 1, 2 | in-model | **Built** — `note_duration` enum, whole…thirty-second, default `eighth` (matches competitor 1's "standard") |
| Chords / simultaneous strings | 1 | in-model | **Built** — every string with an onset in the same column starts together |
| Double-digit frets (`12`, `10`) | 1 | in-model | **Built** — a run of digits is one fret; the onset is the first digit's column |
| Rests (`-` runs) | 1, 2 | in-model | **Built** — a dash column advances time without an onset |
| Bar lines `\|` ignored | 2 | in-model | **Built** — a column that is `\|` on every string consumes **no** time (this is the "dynamic spacing" behaviour competitor 1 advertises) |
| Client-side / nothing uploaded | 1 | in-model | **Already true** — wasm in the browser, no network |
| Custom per-string tuning | 2 | in-model | **Built** — `custom_tuning` (comma-separated scientific pitch names, low→high) |
| Instrument / GM program | 2, 3 | in-model | **Built** — `instrument` enum of the GM guitar/bass programs |
| Velocity | 2 (as digit semantics in piano mode) | in-model | **Built** — `velocity` 1–127, default 96 |
| PDF-of-tab upload | 1 | **out-of-model** | **Not built** — needs a PDF text-extraction stage in front of a pure text tool; the repo already has `pdf-extract-text` for that, so the composition is chain-two-tools, not one more input mode |
| In-page MIDI playback preview | 1 (partially) | **out-of-model** | **Not built** — browsers cannot play MIDI without a bundled soundfont synth; shipping one would dwarf the tool |
| Drum-tab and piano-tab modes | 2 | in-model but **rejected** | Declined: a drum tab's letter codes and a piano tab's velocity digits are a *different* input grammar; folding them in would make every parameter conditional. A separate tool is the honest shape |
| Triplet timing control line | 2 | in-model but **rejected** | Declined: an experimental, non-standard extra tab line; `note_duration` + `timing = "events"` covers the practical need without inventing syntax |

## Gaps we close that no competitor offers

These came out of the diff, not from any competitor's feature list:

- **`tuning` presets with auto-detection** — `auto` (default) picks the tuning from the number
  of string lines in the stave (4 → bass, 5 → 5-string bass, 6 → guitar, 7/8 → extended range),
  plus named presets (drop D, half/full step down, drop C, open G, open D, DADGAD, ukulele).
  Competitor 1 assumes 6-string standard; competitors 2 and 3 require hard-coding.
- **`capo`** (0–12 frets) and **`transpose`** (−24…+24 semitones) as separate, composable shifts.
- **`timing`** — `columns` (each character column is one step, like every competitor) **or**
  `events` (each onset column is one step, evenly spaced), which rescues tabs whose dash
  spacing is decorative rather than rhythmic.
- **`sustain`** — `until-next` (a string rings until its next note) vs `step` (every note is
  exactly one step). No competitor documents note-off placement at all.
- **`string_order`** — handles tabs written low-string-on-top instead of silently producing a
  pitch-inverted file.
- **Technique characters are tolerated, not stripped blindly** — `h p b r s t / \ ~ ^ * . ( )`
  are skipped as articulation marks while the frets around them still sound; `x`/`X` mutes are
  skipped by default or rendered as short muted notes via `muted_notes`.
- **Multiple staves/systems concatenate in reading order**, so a whole multi-line song
  converts in one pass.
- **Errors name the line and column** ("fret 99 on line 3 column 12 is above the 36-fret
  limit") instead of failing silently — competitor tooling is silent about malformed input.
- **Stated limits on the page**: input size cap, max frets, note cap, and the MIDI pitch range
  clamp are documented rather than discovered through a failure.

## Not attempted (and why)

- Copying any competitor's page copy, naming, layout, or presets — prohibited by the skill
  rules and unnecessary; all copy here is original.
- Server-side conversion, accounts, or a paid tier: outside the browser-local model.
