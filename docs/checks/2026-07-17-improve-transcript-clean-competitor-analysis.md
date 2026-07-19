# transcript-clean — competitor analysis (2026-07-17)

Scan performed BEFORE implementing. One WebSearch ("transcript cleaner remove filler
words fix punctuation capitalization online") plus fetches of the reachable competitors.
Paraphrased only — no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed (top of results)

1. **SocialKit — Transcript Cleaner** (socialkit.dev/transcript-cleaner). Fetched.
   Five independent switches: *Remove Timestamps* (`[00:01:23]`, `0:01`, SRT/VTT arrow
   timecodes), *Remove Filler Words* ("um", "uh", "like", "you know", "basically", "…and
   other filler words"), *Remove Speaker Labels*, *Remove Empty/Duplicate Lines*, *Fix
   Capitalization*. 100% client-side, real-time output. Defaults not stated on the page.

2. **TranscriptCleaner.org** (transcriptcleaner.org). Fetched. GPT-4o-backed. Options
   exposed as three checkboxes: *Remove filler words*, *Keep timestamps*, *Keep speaker
   labels*. Also "fixes capitalization and punctuation to create a natural reading flow"
   and removes "repeated lines and unnatural sentence breaks". Filler examples named:
   "um", "you know", "like".

3. **FormatPhantom — Transcript Cleaner** (formatphantom.com/transcript-tools/…). From
   the search index (page returned 403 to the fetcher). Removes filler words
   (um / uh / like / you know), *stutters*, bracketed timestamps, and bracket markers
   like `[laughter]`, with optional sentence-start capitalisation. No upload / private.

   (Cross-checked against the search-result summaries for Transcribr, WhisperUI,
   MyTextify, SpeakTidy — same feature cluster: timestamps, fillers, speaker labels,
   duplicate lines, capitalization; several also strip non-verbal cue markers and
   normalize punctuation. Transcribr/FormatPhantom/whisperui returned 403 to the fetcher
   so they were read from the result index, not fetched directly.)

## Table-stakes → our decision (fit-to-model)

Every table-stake lands in the descriptor OR the out-of-model list below — none dropped.

| Table-stake (competitors) | In our tool? | Descriptor surface |
|---|---|---|
| Remove filler words (um/uh/erm/hmm/you know/…) | ✅ in-model | `filler_level` enum (off/standard/aggressive) + `extra_fillers` |
| Configurable / discourse-marker fillers (like, basically, actually) | ✅ in-model | `filler_level = "aggressive"` (opt-in — deterministic, can over-strip real words) |
| Custom filler list | ✅ in-model (differentiator) | `extra_fillers` (comma-separated) |
| Remove timestamps ( `[00:01:23]`, `0:01`, SRT/VTT `-->`, seq numbers, `WEBVTT` ) | ✅ in-model | `remove_timestamps` bool (default true) |
| Remove non-verbal cue markers ( `[laughter]`, `(applause)`, `[inaudible]` ) | ✅ in-model | `remove_brackets` bool (default true) |
| Merge / tidy speaker turns | ✅ in-model | `merge_speakers` bool (default true) — merges consecutive same-speaker turns |
| Fix capitalization (sentence starts, standalone "i") | ✅ in-model | `fix_capitalization` bool (default true) |
| Fix / normalize punctuation & spacing | ✅ in-model (heuristic) | `fix_punctuation` bool (default true) |
| Remove empty / duplicate lines | ✅ in-model | done unconditionally during turn assembly |
| Stutter removal ("I-I-I", "w-w-what") | ✅ in-model | folded into filler removal (hyphenated stutter collapse) |
| 100% client-side / private | ✅ | pure wasm, runs in browser/CLI, no network |
| Preset examples / one-click chips | ✅ | `[[example]]` chips on the page |

## Out-of-model (listed, NOT built — needs an ML model / LLM)

- **True AI punctuation *restoration*** — inferring where commas/periods belong from
  prosody/semantics in un-punctuated speech-to-text. We do deterministic punctuation
  *normalization* (spacing, duplicate collapse, ensure terminal mark), not restoration.
- **Semantic filler disambiguation** — deciding whether a given "like"/"right" is a
  filler or meaningful. We use deterministic word lists + a level toggle; the page states
  the trade-off. (GPT-4o competitors do this; a pure tool cannot.)
- **Speaker diarization** — assigning speakers when the raw text has no labels.
- **Grammar correction / paraphrase / summarization / translation** — out of scope.

## UX patterns adopted

- Boolean switches rendered as page checkboxes (like the competitor "five switches" UX).
- `filler_level` rendered as a `<select>` with friendly labels.
- `multiline` textarea for the transcript paste box; Reset + Copy buttons (generator
  default); `[[example]]` preset chips as the worked examples.
