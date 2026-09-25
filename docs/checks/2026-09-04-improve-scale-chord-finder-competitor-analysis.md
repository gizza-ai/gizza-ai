# scale-chord-finder competitor analysis — 2026-09-04

## Scope

Build a generic, deterministic music-theory utility that can (1) find scales/modes from a supplied note set and (2) list the notes and diatonic chords for a selected scale. The gizza model is pure Rust/WASM text output; audio note detection, ML transcription and branded site features are out of model.

## Competitor scan

Searched for scale finder / scale chord finder tools and compared common public music-theory utilities such as Scale Finder style note-set lookups, scales/chords dictionary pages, and guitar/piano-oriented scale charts.

| Table-stakes feature | Observed pattern | Fit decision |
| --- | --- | --- |
| Enter notes to find matching scales | Competitors accept pitch names or clickable note buttons and return candidate scales/modes. | In model: `notes`, `fit`, `root`, ranked results. |
| List a named scale by key | Scale dictionary pages show notes and intervals for a chosen tonic/scale. | In model: `action=list`, `key`, `scale`, notes/degrees/semitones/steps. |
| Major modes and common minor-derived modes | Major modes, pentatonics, blues, harmonic/melodic minor families are expected. | In model: 42 authored scale definitions. |
| Chord suggestions for a scale | Theory pages often show diatonic triads/sevenths or harmonized scale degrees. | In model: `include_chords`, `chord_type`, Roman numerals for seven-note scales. |
| Enharmonic spelling controls | Tools commonly distinguish sharp/flat keys or offer both spellings. | In model: `key`, `root`, `spelling`. |
| Strict vs broad matching | Note-set finders vary between exact matches and supersets. | In model: `fit=contains`, `fit=exact`, `fit=near`. |
| Preset/example starts | Useful tools provide examples such as C major, pentatonic sets or modal lists. | In model/page: example chips for Cmaj7 search, G lydian list and exact pentatonic CSV. |
| Instrument fretboard or piano diagrams | Many music sites visualize fingerings and highlighted keys. | Out of model for this text-first block; result includes chromatic map and note spelling instead. |
| Audio/chord recognition | Some products detect notes/chords from recordings. | Out of model: requires audio analysis/ML; explicitly excluded in page copy. |

## Descriptor decisions

- All fixed choices are enums so the CLI, chat schema and page dropdowns cannot drift.
- `action=auto` keeps the bare/default call useful while allowing explicit `find` or `list`.
- `root=any` is separate from `key` because searches can span all tonics while listing one scale needs a specific tonic.
- `max_results` is capped at 50 to keep output readable while still searching the full catalogue.
- `output` supports full text, names-only, CSV and JSON to match both human and programmatic workflows.

## Verification notes

The local verification matrix should cover: exact text output for a known scale, enum/value forms across find/list surfaces, non-default booleans, the `max_results=50` boundary, CLI output modes and a page deep link.
