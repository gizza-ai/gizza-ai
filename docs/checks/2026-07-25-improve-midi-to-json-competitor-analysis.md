# midi-to-json — competitor analysis (2026-07-25)

Scan performed before implementing. One WebSearch ("MIDI to JSON converter tool online notes
tracks events"); skimmed the top real competitors below. All notes are paraphrased — no
competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **Tone.js/Midi (open-source library, github.com/Tonejs/Midi)** — the de-facto "musical" JSON
   shape. Produces a high-level object: a `header` (name, tempo/BPM list, time-signature list,
   key-signature list, ticks-per-quarter) and a `tracks` array, each track carrying paired
   `notes` (each note has a MIDI number, a pitch NAME like `C4`, start in ticks AND seconds,
   duration in ticks AND seconds, and a normalized 0–1 velocity), plus grouped control changes,
   pitch bends, and an instrument guess. Round-trips back to MIDI. Note-centric view.

2. **midi-json-parser (open-source npm/github, chrisguttandin)** — the "raw structural" JSON
   shape. Emits one JSON object mirroring the MIDI byte layout: `division`, `format`, and a
   `tracks` array where each track is an ordered list of raw events, each with its `delta` time
   and a typed payload (`noteOn`/`noteOff` with note number + velocity, `controlChange`,
   `setTempo` with microseconds-per-quarter, `timeSignature`, `keySignature`, `trackName`,
   `endOfTrack`, etc.). Event-centric, loss-less view.

3. **Online converters (MidiEasy, DevUtl, AudioDevTools)** — browser-only, no upload to a server,
   privacy-first framing. Common table stakes: accept Format 0 and Format 1 SMF; preserve every
   note event, control change, tempo, and time signature; surface BPM, note list, and time
   signature; some also expose a compact "summary/overview" (BPM, note count, track names,
   duration) aimed at game/visualization developers.

## Table-stakes → decision (each lands in the descriptor OR is listed out-of-model)

| Capability | In model? | Where |
|---|---|---|
| Parse SMF Format 0 / 1 / 2 | in | core (`midly`) — always |
| Header: format, track count, ticks-per-quarter (PPQ) / timecode division | in | header in every output |
| Note pairing (note-on↔note-off) with pitch number + name (e.g. `C4`) | in | `format=notes` (default) |
| Start & duration in BOTH ticks and seconds | in | `format=notes` (tempo-mapped seconds) |
| Velocity (raw 0–127) | in | notes + events |
| Tempo / setTempo (BPM + microseconds-per-quarter) | in | header tempos + events |
| Time signature, key signature | in | header + events |
| Control changes (CC number + value + channel) | in | events; header count |
| Program changes (instrument number) | in | events |
| Pitch bend | in | events |
| Meta events (track name, text, marker, lyric, end-of-track…) | in | events + track names in header |
| Raw loss-less event stream (midi-json-parser shape) | in | `format=events` |
| Compact overview (BPM, counts, duration, track names) | in | `format=summary` |
| Privacy / in-browser, no upload | in | pure Rust, runs on-device on every surface |
| Round-trip JSON→MIDI (re-encode) | OUT | reverse direction; separate tool, not this slug |
| Instrument NAME guess / General-MIDI patch names | OUT | GM name table is a large lookup; program NUMBER is emitted, name is out of scope |
| Normalized 0–1 velocity (Tone.js convention) | OUT | we emit raw 0–127 (standard MIDI); 0–1 is a trivial client divide, not worth a param |

## Our design

- `input` — MIDI (.mid/.midi) bytes as base64 or hex (mirrors avro-to-json's binary-in pattern).
- `encoding` — `auto` (default) / `base64` / `hex`.
- `format` — `notes` (default, Tone.js-style musical view: header + paired notes with ticks &
  seconds), `events` (midi-json-parser-style raw per-track event stream), `summary` (compact
  overview: format, ppq, BPM, duration, counts, track names).

Preset chips on the page for each `format`. Seconds are computed from a tempo map built across all
tracks (SMF tempo events can live in any track), so `notes`/`summary` seconds are correct through
tempo changes.
