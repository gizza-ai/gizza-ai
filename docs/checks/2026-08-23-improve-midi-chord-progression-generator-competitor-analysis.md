# midi-chord-progression-generator — competitor analysis (2026-08-23)

Scan run BEFORE implementation, per `/improve-tool` Phase 2. Everything below is **paraphrased**
from public marketing/UI copy; no competitor wording, branding or assets were copied into this
repo. Reachability was checked with `WebFetch`; two candidates returned effectively empty bodies
(client-rendered SPAs) and are recorded as such rather than invented.

## Search

`WebSearch: "chord progression to MIDI generator online"` plus a narrower
`"chord symbols text to MIDI file converter … export mid"` pass, because our tool CONVERTS a typed
chord-symbol progression rather than randomly generating one — the generic query is dominated by
random-progression generators, so the second query was needed to find the true functional peers.

## Profiles

### 1. CalvinSunday Studios — MIDI chord progression generator (reachable, richest UI)
- **Features:** browser-side generation, WebAudio preview with play/stop + volume, "new
  progression" without downloading, one-click `.mid` download aimed at DAW drag-and-drop.
- **Params/options:** BPM; key (all 12, spelled with enharmonics); scale (major, natural minor,
  dorian, mixolydian); complexity (simple / colourful / jazzy, where the richer settings introduce
  7ths, 9ths and borrowed chords); number of chords 2–8; octave centre (a low or a middle
  register); a toggle for adding 7ths to diatonic dominants; an optional filename tag.
- **Output:** `.mid`, voicings described as including inversions.
- **Limits stated:** chord count capped at 8. No FAQ section.
- **UX patterns:** two labelled groups ("basics" vs "voicing & extras"), audible preview,
  regenerate-without-download.

### 2. Chordoo — free chord progression generator with MIDI export (reachable, thin spec)
- **Features:** genre-flavoured random progressions (pop, EDM, lo-fi, hip hop, jazz, trap, house,
  K-pop), spacebar to regenerate, lock individual chords and re-roll the rest, MIDI download.
- **Params/options:** genre preset is the only exposed control on the marketing page; no BPM,
  voicing, octave or inversion controls are documented publicly.
- **Output:** standard MIDI, explicitly positioned for FL Studio / Ableton / Logic / GarageBand.
- **Limits stated:** none — "unlimited", no signup.
- **FAQ topics:** what a chord progression generator is; DAW compatibility; is it free;
  do-I-need-theory.

### 3. OneMotion Chord Player (reachable, deepest voicing controls)
- **Features:** interactive chord pad/player, record + edit a progression, explore substitutions.
- **Params/options:** instrument; volume; style; scale and mode; key up/down; tempo; a bass
  pedal-point setting; chord extension selector (triad / 7 / maj7 / 9 / maj9); voicing controls
  including how many tones sound (2, 3, 4 or all) and an interval-pairing choice (octave, 3rd,
  4th, 5th, 6th, 10th); bass-note offset; duration/sustain; play-bass-only, play-chord-only,
  silence and break options.
- **Output:** playback plus export; export parameters not documented on the page.
- **Limits stated:** none published.

### 4. Rhyd Lewis "text to MIDI" (reachable, the closest FUNCTIONAL peer)
- **Features:** the only scanned tool that takes TYPED chord symbols rather than generating them —
  a comma-separated list such as a basic triad sequence or a `maj7 / m7 / 7` jazz turnaround.
- **Params/options:** a key-signature selector (~30 major/minor choices). Tempo and metre are
  **fixed** at 120 BPM and 4/4; one chord occupies one bar.
- **Output:** a "download MIDI" button.
- **Limits stated:** none published; no slash-chord or per-chord-duration syntax documented.

### 5. Chordoo/ChordProgressions-class SPAs (chordprogressions.app, chordprogenerator.com)
- **Not usefully reachable:** both returned a title-only body to a plain fetch (client-rendered),
  so no parameter list could be confirmed. Recorded honestly rather than guessed. Their indexed
  descriptions advertise diatonic progressions in any key/mode, piano preview, genre presets
  (pop, trap, house, techno, trance) and piano-roll editing before MIDI export.

## Table stakes extracted

| Table stake | Seen at | Our decision |
| --- | --- | --- |
| Typed chord symbols incl. 7ths/extensions | Rhyd Lewis, OneMotion | **In model** — `progression`, with a broad symbol parser (triads, 6/7/9/11/13, sus, add, dim/aug, half-dim, alterations, slash bass) |
| Tempo (BPM) | CalvinSunday, OneMotion | **In model** — `tempo`, default 120 |
| One chord = one bar of 4/4 | Rhyd Lewis | **In model, and made adjustable** — `beats_per_chord` (default 4) + `beats_per_bar` (default 4) + a per-chord `Symbol:beats` override, which no scanned tool offers |
| Octave centre / register | CalvinSunday, OneMotion | **In model** — `octave`, default 4 (middle C) |
| Inversions / voice leading | CalvinSunday, OneMotion | **In model** — `inversion` (root/first/second/third/**smooth** nearest-inversion voice leading) |
| Voicing spread (how tones are spaced) | OneMotion | **In model** — `voicing` (close / drop-2 / drop-3 / spread) |
| Bass note / pedal point | OneMotion | **In model** — `add_bass`, plus slash chords (`C/E`) always placed in the bass |
| Instrument choice | OneMotion | **In model** — `instrument`, a 16-entry General MIDI keys/guitar/strings/pad list written as a program change |
| Key change / transposition | CalvinSunday, OneMotion, Rhyd Lewis | **In model** — `transpose` in semitones (a converter transposes; it does not need a key/scale picker because the symbols already state the harmony) |
| Note length / sustain | OneMotion | **In model** — `note_length` as a percentage of each slot |
| Arpeggio / strum / rhythm feel | chordprogenerator (indexed), OneMotion | **In model** — `pattern` (block / arpeggio up, down, up-down / strum) + `arp_note` step |
| Velocity | none exposed it | **In model** — `velocity`, for parity with our sibling MIDI tools |
| Rests / bar-line syntax | none | **In model** — `-` rests and `|` bar lines are accepted and ignored/silent, a genuine gap in the peer set |
| Presets / genre chips | Chordoo, indexed SPAs | **In model** — `[[example]]` chips on the page (pop turnaround, jazz ii–V–I, smooth-voiced ballad, arpeggiated) |
| Stated limits | mostly absent | **In model** — we state them on the page (512 chords, 64 KiB, 20 000 notes, MIDI 0–127 range) |

## In-model vs out-of-model

**In model (all built this pass):** every row in the table above.

**Out of model — considered, not built:**
- **Random / AI progression generation by key, scale, genre or "complexity"** (Chordoo,
  CalvinSunday, the SPAs). Our brief is a CONVERTER: chord symbols in, MIDI out. Random output
  also needs an RNG, which would make a pure, deterministic block non-reproducible across the
  chat/CLI/page surfaces. Not built; the user brings the harmony.
- **In-page audio preview / WebAudio synth playback** (CalvinSunday, OneMotion). A `.mid` file is
  note data, not sound — previewing it needs a soundfont synth in the page runtime, which the
  shared generator runtime doesn't provide. The page states this explicitly in its FAQ instead of
  pretending otherwise.
- **Piano-roll editing before export** (chordprogenerator). Needs a bespoke interactive canvas
  editor; far outside a declarative `meta.toml` form and the shared page runtime.
- **Lock-a-chord-and-re-roll** (Chordoo). Only meaningful with random generation, see above.
- **Accounts, cloud project saving, DAW plugin versions.** Server/account-bound; gizza tools are
  browser-local and no-account.
