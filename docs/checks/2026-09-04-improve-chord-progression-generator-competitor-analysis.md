# chord-progression-generator — competitor analysis (2026-09-04)

Scan run **before** implementation so the descriptor could ship the table-stakes controls from
day one. All findings are paraphrased from public tool pages; no competitor copy, branding,
trademarks or assets were reproduced.

Search: "chord progression generator online key style diatonic borrowed chords".

## Competitor profiles

### 1. AutoChords — https://autochords.com/
- **Features:** pick a key, generate a progression, see the chord names plus the notes inside each
  chord, browse alternative progressions derived from circle-of-fifths relationships, and see the
  full diatonic chord set for the selected key.
- **Params/options:** key (12 roots), major/minor mode, instrument, a randomize button, a naming
  field, and an edit/export affordance.
- **Input/output:** no file input; output is on-page chord names + spelled note names + suggested
  alternates. Playback is implied rather than a documented export.
- **UX patterns:** one-click randomize; alternates shown alongside the main result; beginner-key
  guidance in the copy (G major / E minor / C major / A minor suggested as easy keys).
- **Limits stated:** none.
- **Free vs paid:** free.

### 2. OneMotion Chord Player — https://www.onemotion.com/chord-player/
- **Features:** build a progression from a key/scale, audition it with a built-in player, control
  the voicing and rhythm, and export.
- **Params/options:** key with +/- stepper; scale/mode; chord quality set (triad, 7, maj7, 9,
  maj9); bass-note offset; style/genre preset; bass pedal patterns (none, 3:2, light, 2:1, shuffle,
  3:1, hard); tones-per-chord (2, 3, 4, all); voicing interval pairs (octave, 3rd, 4th, 5th, 6th,
  10th); duration stepper; sustain slider; tempo stepper; volume; per-chord random; key filter
  (all 12 / scale 7 / pentatonic 5 / blues 6).
- **Output:** live audio playback plus MIDI/WAV export.
- **UX patterns:** dense one-screen control panel, live preview, per-chord randomize, presets.
- **Limits stated:** none.

### 3. Chords.lol progression generator — https://www.chords.lol/progression-generator
- **Features:** curated named progressions per "vibe" plus a generative mode; explicit support for
  borrowed chords (it calls out where the ♭VI and ♭VII come from).
- **Params/options:** key (30 entries — 15 major, 15 minor spellings including enharmonics); vibe
  (pop, sad, jazz, blues, folk, rock) plus a "surprise" generative mode; tempo (default 96);
  instrument (guitar or piano).
- **Output:** Roman numerals alongside chord names, per-chord playback, play-all, re-roll within
  the vibe's set, copy-as-text.
- **UX patterns:** re-roll button, per-chord audition, copy to clipboard, FAQ explaining that the
  generative mode produces plausible in-key progressions rather than quoting real songs.
- **Limits stated:** 30 documented progressions across 30 keys; each vibe draws from a curated set
  (roughly 6–9 entries per vibe). No MIDI export, no extension controls.

### 4. Guitar Tool Hub — https://guitartoolhub.com/chord-progression-generator
- **Features:** generate by key + mode + genre, with Roman-numeral analysis and guitar-specific
  output.
- **Params/options:** key (12 chromatic roots); mode (major/minor); style (pop, rock, blues, jazz,
  folk, ballad, EDM/dance, worship, country, R&B, metal, reggae); tempo 60–180 (default 96);
  guitar sound (acoustic / electric / classical); re-roll.
- **Output:** Roman numerals, chord names per scale degree, sampled-guitar playback, clickable
  finger diagrams, right-click to audition one chord, hand-off to a strumming player.
- **UX patterns:** re-roll cycles ~9 variations per style/key pair; chord chips are interactive.
- **Limits stated:** 9 variations per style+key; no bar-count, seventh-extension, borrowed-chord or
  MIDI-export controls.

## Table-stakes list → where each landed

| Table stake | Seen at | Verdict |
| --- | --- | --- |
| Key / tonic incl. enharmonic spellings | all 4 | **in-model** → `key` enum, 17 roots (C…B with both sharp and flat spellings) |
| Major/minor mode | all 4 | **in-model** → `mode` enum, extended to 9 (7 church modes + harmonic/melodic minor) |
| Style / genre / vibe preset | 3, 4 (2 partially) | **in-model** → `style` enum, 15 curated sets + `random` generative mode |
| Re-roll / randomize / "surprise me" | all 4 | **in-model** → `variation` (1–99). Deterministic, so a run is reproducible and shareable via URL — competitors' re-roll is not |
| Roman-numeral analysis | 3, 4 | **in-model** → always emitted; `output=roman` prints only the numeral line |
| Spelled notes per chord | 1 | **in-model** → shown in the detailed output and the CSV output |
| Seventh / extended chord quality | 2 | **in-model** → `sevenths` = auto / triads / sevenths / extended |
| Borrowed / modal-interchange chords | 3 (named), 2 (implicit) | **in-model** → `borrowed` = none / light / rich, plus chromatic tokens baked into rock/blues/metal templates |
| Chord count / length control | none exposed it | **in-model** → `chords` (0 = the template's natural length, so a 12-bar blues stays 12) |
| Tempo | 2, 3, 4 | **in-model** → `tempo` 40–300, default 100 |
| Instrument choice | all 4 | **in-model** → `instrument`, 16 General MIDI programs written into the file |
| Rhythm / pattern (block, arpeggio, strum) | 2 | **in-model** → `pattern` = block / arpeggio up / down / up-down / strum |
| Voicing quality (voice leading) | 2 (voicing pairs) | **in-model** → `voice_leading` checkbox (nearest-inversion voice leading, on by default) |
| Loop the progression | 2 (player loops) | **in-model** → `repeats` 1–8, baked into the MIDI |
| Copy result as text | 3 | **in-model** → the shared page Copy button + `output=chords` for a bare one-liner |
| MIDI export | 2 | **in-model** → every run produces a downloadable Standard MIDI File |
| Register / octave | 2 (bass offset) | **in-model** → `octave` 1–7 |
| Live sampled audio playback | all 4 | **out-of-model** — needs a sampled instrument engine and audio graph; the MIDI download is the answer here |
| WAV/audio-file export | 2 | **out-of-model** — same reason (would need a bundled synth + renderer) |
| Guitar chord diagrams / fingerings | 1, 4 | **considered, rejected** — a fretboard renderer is a different tool's job, not a param on this one |
| Per-chord audition / clickable chips | 3, 4 | **out-of-model** — depends on the playback engine above |
| Hyperlinks to a chord-reference library | 3 | **out-of-model** — site-specific content, not a toolkit feature |
| Circle-of-fifths "alternative progressions" panel | 1 | **considered, rejected** — `variation` covers re-rolling; an alternates panel would need bespoke page UI |

## How this tool differs

- Deterministic by construction: `(key, mode, style, variation, …)` always produces the same
  progression, so a result is reproducible, diff-able and shareable as a URL. Every competitor's
  re-roll is random and unrepeatable.
- Real Standard MIDI File output on every run (only competitor 2 exports MIDI at all), with
  voicing, pattern, instrument and repeat count applied.
- Modes beyond major/minor (dorian, phrygian, lydian, mixolydian, locrian, harmonic minor, melodic
  minor) — none of the four offer these.
- Same behaviour on the page, the CLI and in chat, from one descriptor.
