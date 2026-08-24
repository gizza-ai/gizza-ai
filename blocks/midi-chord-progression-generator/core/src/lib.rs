//! midi-chord-progression-generator core — turn a written chord-symbol
//! progression (`C G Am F`, `Cmaj7 | Dm7 | G7 | Cmaj7`, `C/E:2`) into a
//! Standard MIDI File. Pure Rust (`midly` for the SMF writer); no wafer or
//! wasm-bindgen deps, so the same logic runs in chat, the CLI and the browser
//! page and all three give identical results and identical errors.
//!
//! The pipeline is: parse each symbol into a root + a set of semitone
//! intervals (+ an optional slash bass and its own duration) → place those
//! intervals in an octave and re-voice them (inversion, drop/spread voicing,
//! optional doubled bass, transpose) → lay the voiced chords out in time as
//! block chords, an arpeggio or a strum → serialize as a format-0 SMF.

use midly::num::{u15, u24, u28, u4, u7};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// Ticks per quarter note in the produced file. 480 divides cleanly by
/// 2/3/4/5, so every supported note value and gate lands on an integer tick.
pub const PPQ: u16 = 480;
/// Reject pathologically large input before doing any work.
pub const MAX_INPUT_BYTES: usize = 64 * 1024;
/// Cap the number of chord slots so a runaway paste can't produce a huge file.
pub const MAX_CHORDS: usize = 512;
/// Cap the note count for the same reason (arpeggios multiply notes per chord).
pub const MAX_NOTES: usize = 20_000;
/// Onset spacing between the notes of a strummed chord, in ticks (a 64th note).
const STRUM_TICKS: u32 = PPQ as u32 / 16;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Every knob the conversion exposes. The enum-valued fields arrive as the raw
/// strings the descriptor/CLI/page pass, and are validated here so all three
/// surfaces produce the same message for the same mistake.
#[derive(Debug, Clone)]
pub struct Options {
    /// Tempo in quarter-note beats per minute.
    pub tempo: f64,
    /// How many beats one chord lasts, unless the symbol carries `:beats`.
    pub beats_per_chord: f64,
    /// Numerator of the time signature written into the file (denominator 4).
    pub beats_per_bar: u32,
    /// Octave the chord root is voiced in; 4 puts middle C at MIDI 60.
    pub octave: i32,
    /// `close` | `drop-2` | `drop-3` | `spread`.
    pub voicing: String,
    /// `root` | `first` | `second` | `third` | `smooth`.
    pub inversion: String,
    /// `block` | `arpeggio-up` | `arpeggio-down` | `arpeggio-updown` | `strum`.
    pub pattern: String,
    /// Note value of one arpeggio step (ignored by `block`/`strum`).
    pub arp_note: String,
    /// Percentage of its slot each note actually sounds for, 5-100.
    pub note_length: f64,
    /// Double the chord's bass note an octave below the voicing.
    pub add_bass: bool,
    /// Extra transposition in semitones, applied last.
    pub transpose: i32,
    /// MIDI note-on velocity, 1-127.
    pub velocity: u8,
    /// General MIDI instrument name (see [`instrument_program`]).
    pub instrument: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            tempo: 120.0,
            beats_per_chord: 4.0,
            beats_per_bar: 4,
            octave: 4,
            voicing: "close".into(),
            inversion: "root".into(),
            pattern: "block".into(),
            arp_note: "eighth".into(),
            note_length: 95.0,
            add_bass: false,
            transpose: 0,
            velocity: 96,
            instrument: "acoustic-grand-piano".into(),
        }
    }
}

/// What a successful conversion produced.
#[derive(Debug, Clone)]
pub struct Conversion {
    /// The Standard MIDI File bytes (format 0, single track).
    pub midi: Vec<u8>,
    /// Number of note events written.
    pub notes: usize,
    /// Number of chord slots, rests included.
    pub slots: usize,
    /// Number of sounding chords (slots that were not rests).
    pub chords: usize,
    /// Total length in beats.
    pub beats: f64,
    /// Playing time in seconds at the requested tempo.
    pub seconds: f64,
    /// Lowest and highest MIDI pitch written (only meaningful when `notes > 0`).
    pub lowest: u8,
    pub highest: u8,
    /// One line per slot: the symbol as written and the notes it voiced to.
    pub detail: Vec<String>,
}

impl Conversion {
    /// One-line human summary, shared by the chat envelope and the page.
    pub fn summary(&self) -> String {
        format!(
            "{} chord(s) in {} slot(s) → {} note(s), {:.2} beat(s) — {:.1}s of MIDI ({} bytes)",
            self.chords,
            self.slots,
            self.notes,
            self.beats,
            self.seconds,
            self.midi.len()
        )
    }

    /// The per-slot chord/notes breakdown as one printable block.
    pub fn detail_text(&self) -> String {
        self.detail.join("\n")
    }
}

// ---------------------------------------------------------------------------
// Vocabularies
// ---------------------------------------------------------------------------

/// Note names, sharp spelling, indexed by pitch class.
const SHARP_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
/// Note names, flat spelling, indexed by pitch class.
const FLAT_NAMES: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

/// Base chord qualities as semitone offsets from the root. Matched as the
/// LONGEST prefix of the normalized quality string, so the order here is
/// load-bearing: `maj7` must be tried before `maj`, and `maj*` before `m`.
const QUALITIES: &[(&str, &[i32])] = &[
    ("maj13", &[0, 4, 7, 11, 14, 21]),
    ("maj11", &[0, 4, 7, 11, 14, 17]),
    ("maj9", &[0, 4, 7, 11, 14]),
    ("maj7", &[0, 4, 7, 11]),
    ("maj6", &[0, 4, 7, 9]),
    ("maj", &[0, 4, 7]),
    ("m13", &[0, 3, 7, 10, 14, 21]),
    ("m11", &[0, 3, 7, 10, 14, 17]),
    ("m9", &[0, 3, 7, 10, 14]),
    ("m7", &[0, 3, 7, 10]),
    ("m6", &[0, 3, 7, 9]),
    ("m", &[0, 3, 7]),
    ("dim7", &[0, 3, 6, 9]),
    ("dim", &[0, 3, 6]),
    ("aug7", &[0, 4, 8, 10]),
    ("aug", &[0, 4, 8]),
    ("sus4", &[0, 5, 7]),
    ("sus2", &[0, 2, 7]),
    ("sus", &[0, 5, 7]),
    ("13", &[0, 4, 7, 10, 14, 21]),
    // A dominant 11 conventionally drops the 3rd, which would clash with the 11.
    ("11", &[0, 7, 10, 14, 17]),
    ("9", &[0, 4, 7, 10, 14]),
    ("7", &[0, 4, 7, 10]),
    ("6", &[0, 4, 7, 9]),
    ("5", &[0, 7]),
    ("", &[0, 4, 7]),
];

/// Note values usable as one arpeggio step, in quarter-note fractions.
const NOTE_VALUES: &[(&str, f64)] = &[
    ("whole", 4.0),
    ("half", 2.0),
    ("quarter", 1.0),
    ("eighth", 0.5),
    ("sixteenth", 0.25),
    ("thirty-second", 0.125),
];

/// General MIDI instruments offered, as (name, zero-based program number).
const INSTRUMENTS: &[(&str, u8)] = &[
    ("acoustic-grand-piano", 0),
    ("bright-acoustic-piano", 1),
    ("electric-piano", 4),
    ("harpsichord", 6),
    ("vibraphone", 11),
    ("drawbar-organ", 16),
    ("church-organ", 19),
    ("accordion", 21),
    ("acoustic-guitar-nylon", 24),
    ("acoustic-guitar-steel", 25),
    ("electric-guitar-clean", 27),
    ("acoustic-bass", 32),
    ("string-ensemble", 48),
    ("choir-aahs", 52),
    ("synth-pad-warm", 89),
    ("synth-lead-square", 80),
];

/// Zero-based General MIDI program for an instrument name.
pub fn instrument_program(name: &str) -> Result<u8, String> {
    INSTRUMENTS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, p)| *p)
        .ok_or_else(|| {
            format!(
                "unknown instrument '{name}': expected one of {}",
                INSTRUMENTS
                    .iter()
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })
}

/// Scientific pitch name for a MIDI note number, e.g. 60 → `C4`.
pub fn midi_to_name(m: i32) -> String {
    name_with_spelling(m, false)
}

/// Same, but able to spell black keys with flats (`Eb3`) instead of sharps.
fn name_with_spelling(m: i32, flats: bool) -> String {
    let pc = m.rem_euclid(12) as usize;
    let octave = m.div_euclid(12) - 1;
    let names = if flats { FLAT_NAMES } else { SHARP_NAMES };
    format!("{}{}", names[pc], octave)
}

// ---------------------------------------------------------------------------
// Chord-symbol parsing
// ---------------------------------------------------------------------------

/// One parsed slot of the progression.
#[derive(Debug, Clone, PartialEq)]
pub struct Chord {
    /// The symbol exactly as the user wrote it (minus any `:beats` suffix).
    pub symbol: String,
    /// Root pitch class, 0 = C.
    pub root: i32,
    /// Semitone offsets from the root, ascending, root first.
    pub intervals: Vec<i32>,
    /// Slash-bass pitch class, when the symbol carried one.
    pub bass: Option<i32>,
    /// Whether the root was written with a flat, so the notes read back flat.
    pub flats: bool,
    /// Length of this slot in beats.
    pub beats: f64,
    /// A rest slot: silent, but it still consumes its beats.
    pub rest: bool,
}

/// Pitch class of a note letter, 0 = C.
fn letter_pc(c: char) -> Option<i32> {
    match c.to_ascii_uppercase() {
        'C' => Some(0),
        'D' => Some(2),
        'E' => Some(4),
        'F' => Some(5),
        'G' => Some(7),
        'A' => Some(9),
        'B' => Some(11),
        _ => None,
    }
}

/// Read a root note (letter + any accidentals) off the front of `s`.
/// Returns the pitch class, whether it was spelled flat, and the rest.
fn take_root(s: &str, symbol: &str) -> Result<(i32, bool, usize), String> {
    let mut chars = s.char_indices();
    let (_, first) = chars.next().ok_or_else(|| {
        format!("chord '{symbol}' is empty: write a chord symbol such as C, Am or Fmaj7")
    })?;
    let mut pc = letter_pc(first).ok_or_else(|| {
        format!(
            "chord '{symbol}': '{first}' is not a note name — chord symbols start with A-G \
             (optionally followed by # or b), e.g. C, F#m or Bb7"
        )
    })?;
    let mut flats = false;
    let mut end = first.len_utf8();
    for (i, c) in s.char_indices().skip(1) {
        match c {
            '#' => {
                pc += 1;
                end = i + c.len_utf8();
            }
            'b' if end == i => {
                // Only an accidental when it is glued to the letter/previous
                // accidental — the `b` in `Cb5` after a digit is a flat-five.
                pc -= 1;
                flats = true;
                end = i + c.len_utf8();
            }
            _ => break,
        }
    }
    Ok((pc.rem_euclid(12), flats, end))
}

/// Fold the many written spellings of a quality into the canonical ASCII the
/// [`QUALITIES`] / modifier tables use.
fn normalize_quality(q: &str) -> String {
    let mut s = q.replace(['(', ')', ' '], "");
    s = s.replace('♯', "#").replace('♭', "b");
    s = s.replace("Δ7", "maj7").replace('Δ', "maj7");
    s = s.replace("∆7", "maj7").replace('∆', "maj7");
    s = s.replace('ø', "m7b5").replace('Ø', "m7b5");
    // Only the dedicated glyphs map to `dim` — a bare `o` would corrupt `no3`
    // and `omit5`, which are legitimate modifiers.
    s = s.replace('°', "dim").replace('º', "dim");
    s = s.replace("MAJ", "maj").replace("Maj", "maj");
    s = s.replace("MIN", "m").replace("Min", "m").replace("min", "m");
    s = s.replace("DIM", "dim").replace("Dim", "dim");
    s = s.replace("AUG", "aug").replace("Aug", "aug");
    s = s.replace("SUS", "sus").replace("Sus", "sus");
    s = s.replace("ADD", "add").replace("Add", "add");
    s = s.replace('M', "maj");
    if let Some(rest) = s.strip_prefix('-') {
        s = format!("m{rest}");
    }
    if let Some(rest) = s.strip_prefix('+') {
        s = format!("aug{rest}");
    }
    s
}

/// Insert `semitone` into an ascending interval list if it is not already there.
fn add_interval(v: &mut Vec<i32>, semitone: i32) {
    if !v.contains(&semitone) {
        v.push(semitone);
        v.sort_unstable();
    }
}

/// Replace whichever of `from` is present with `to` (or just add `to`).
fn alter(v: &mut Vec<i32>, from: i32, to: i32) {
    v.retain(|x| *x != from);
    add_interval(v, to);
}

/// Apply one trailing modifier token; returns how many bytes it consumed.
fn apply_modifier(v: &mut Vec<i32>, rest: &str) -> Option<usize> {
    // Longest first, so `add11` wins over `add1` and `sus4` over `sus`.
    const MODS: &[&str] = &[
        "omit3", "omit5", "add11", "add13", "maj7", "maj9", "sus2", "sus4", "add2", "add4", "add6",
        "add9", "no3", "no5", "b13", "#11", "alt", "sus", "b5", "#5", "b6", "b9", "#9", "#4",
    ];
    let m = MODS.iter().find(|m| rest.starts_with(**m))?;
    match *m {
        "sus" | "sus4" => {
            v.retain(|x| *x != 3 && *x != 4);
            add_interval(v, 5);
        }
        "sus2" => {
            v.retain(|x| *x != 3 && *x != 4);
            add_interval(v, 2);
        }
        "maj7" => add_interval(v, 11),
        "maj9" => {
            add_interval(v, 11);
            add_interval(v, 14);
        }
        "add2" => add_interval(v, 2),
        "add9" => add_interval(v, 14),
        "add4" => add_interval(v, 17),
        "add11" => add_interval(v, 17),
        "add6" => add_interval(v, 9),
        "add13" => add_interval(v, 21),
        "b5" => alter(v, 7, 6),
        "#5" => alter(v, 7, 8),
        "b6" => add_interval(v, 20),
        "b9" => add_interval(v, 13),
        "#9" => add_interval(v, 15),
        "#11" | "#4" => add_interval(v, 18),
        "b13" => add_interval(v, 20),
        "no3" | "omit3" => v.retain(|x| *x != 3 && *x != 4),
        "no5" | "omit5" => v.retain(|x| *x != 7),
        "alt" => {
            // The altered dominant: no natural 5th, both altered 9ths and 5ths.
            v.retain(|x| *x != 7);
            for s in [13, 15, 18, 20] {
                add_interval(v, s);
            }
        }
        _ => unreachable!("modifier table and match arms must agree"),
    }
    Some(m.len())
}

/// Parse one chord symbol (no `:beats` suffix) into root + intervals.
pub fn parse_chord_symbol(symbol: &str) -> Result<Chord, String> {
    let text = symbol.trim();
    // Split the slash bass off first so `C/E` doesn't confuse the root scan.
    let (chord_part, bass_part) = match text.rsplit_once('/') {
        Some((c, b)) if !c.is_empty() && !b.is_empty() => (c, Some(b)),
        _ => (text, None),
    };
    let (root, flats, used) = take_root(chord_part, symbol)?;
    let quality = normalize_quality(&chord_part[used..]);

    let (base, base_len) = QUALITIES
        .iter()
        .find(|(name, _)| quality.starts_with(name))
        .map(|(name, iv)| (*iv, name.len()))
        .expect("the empty quality always matches");
    let mut intervals: Vec<i32> = base.to_vec();

    let mut rest = &quality[base_len..];
    while !rest.is_empty() {
        let used = apply_modifier(&mut intervals, rest).ok_or_else(|| {
            format!(
                "chord '{symbol}': don't understand '{rest}' after the root — supported qualities \
                 are m, maj7, m7, 7, 6, m6, 9, 11, 13, dim, dim7, m7b5, aug, sus2, sus4, add9 and \
                 alterations b5 #5 b9 #9 #11 b13, plus a /bass note"
            )
        })?;
        rest = &rest[used..];
    }
    intervals.sort_unstable();

    let bass = match bass_part {
        Some(b) => {
            let (pc, _, used) = take_root(b, symbol)?;
            if used != b.len() {
                return Err(format!(
                    "chord '{symbol}': the bass note after '/' must be a plain note name such as \
                     /E or /Bb, got '{b}'"
                ));
            }
            Some(pc)
        }
        None => None,
    };

    Ok(Chord {
        symbol: text.to_string(),
        root,
        intervals,
        bass,
        flats,
        beats: 0.0,
        rest: false,
    })
}

/// Split the written progression into slots and parse each one.
///
/// Separators are whitespace and commas; `|` bar lines are accepted anywhere
/// and ignored. `-`, `_`, `r` and `rest` are silent slots. A `:beats` suffix
/// overrides `default_beats` for that slot only.
pub fn parse_progression(text: &str, default_beats: f64) -> Result<Vec<Chord>, String> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "progression is too long: {} bytes, the limit is {MAX_INPUT_BYTES}",
            text.len()
        ));
    }
    let mut out = Vec::new();
    for raw in text.split([' ', '\t', '\n', '\r', ',', ';']) {
        let token = raw.trim_matches('|').trim();
        if token.is_empty() {
            continue;
        }
        let (sym, beats) = match token.split_once(':') {
            Some((s, b)) => {
                let beats: f64 = b.trim().parse().map_err(|_| {
                    format!(
                        "chord '{token}': the length after ':' must be a number of beats, e.g. \
                         '{s}:2' for two beats, got '{b}'"
                    )
                })?;
                if !(beats.is_finite() && beats > 0.0 && beats <= 64.0) {
                    return Err(format!(
                        "chord '{token}': the length after ':' must be between 0.25 and 64 beats, \
                         got {beats}"
                    ));
                }
                (s.trim(), beats)
            }
            None => (token, default_beats),
        };
        if sym.is_empty() {
            return Err(format!(
                "'{token}' has a length but no chord — write it as 'C:{beats}'"
            ));
        }

        let mut chord = if matches!(sym, "-" | "_" | "r" | "R" | "rest" | "Rest" | "REST") {
            Chord {
                symbol: sym.to_string(),
                root: 0,
                intervals: Vec::new(),
                bass: None,
                flats: false,
                beats,
                rest: true,
            }
        } else {
            parse_chord_symbol(sym)?
        };
        chord.beats = beats;
        out.push(chord);

        if out.len() > MAX_CHORDS {
            return Err(format!(
                "too many chords: the limit is {MAX_CHORDS} slots per progression"
            ));
        }
    }
    if out.is_empty() {
        return Err(
            "no chords found: write a progression such as 'C G Am F' or 'Cmaj7 | Dm7 | G7 | Cmaj7'"
                .into(),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Voicing
// ---------------------------------------------------------------------------

/// Rotate the `n` lowest notes up an octave — the usual chord inversion.
fn invert(notes: &mut Vec<i32>, n: usize) {
    for _ in 0..n {
        notes.sort_unstable();
        if notes.is_empty() {
            return;
        }
        notes[0] += 12;
    }
    notes.sort_unstable();
}

/// Place a pitch class in the highest octave strictly below `ceiling`.
fn bass_below(pc: i32, ceiling: i32) -> i32 {
    let mut n = pc.rem_euclid(12);
    while n + 12 < ceiling {
        n += 12;
    }
    n
}

/// Voice one chord: intervals → actual MIDI note numbers.
///
/// `prev` is the previous chord's voicing, used only by `inversion = "smooth"`
/// to pick whichever inversion sits closest to what just sounded.
pub fn voice_chord(
    chord: &Chord,
    opts: &Options,
    prev: Option<&[i32]>,
) -> Result<Vec<i32>, String> {
    if chord.rest {
        return Ok(Vec::new());
    }
    let root_midi = 12 * (opts.octave + 1) + chord.root;
    let base: Vec<i32> = chord.intervals.iter().map(|i| root_midi + i).collect();

    let mut notes = match opts.inversion.as_str() {
        "root" => base.clone(),
        "first" | "second" | "third" => {
            let n = match opts.inversion.as_str() {
                "first" => 1,
                "second" => 2,
                _ => 3,
            };
            // A triad has no third inversion; clamp instead of inventing a note.
            let n = n.min(base.len().saturating_sub(1));
            let mut v = base.clone();
            invert(&mut v, n);
            v
        }
        "smooth" => smooth_voicing(&base, prev),
        other => {
            return Err(format!(
                "unknown inversion '{other}': expected root, first, second, third or smooth"
            ))
        }
    };
    notes.sort_unstable();

    match opts.voicing.as_str() {
        "close" => {}
        "drop-2" => {
            if notes.len() >= 3 {
                let i = notes.len() - 2;
                notes[i] -= 12;
            }
        }
        "drop-3" => {
            if notes.len() >= 4 {
                let i = notes.len() - 3;
                notes[i] -= 12;
            }
        }
        "spread" => {
            if notes.len() >= 2 {
                notes[0] -= 12;
            }
        }
        other => {
            return Err(format!(
                "unknown voicing '{other}': expected close, drop-2, drop-3 or spread"
            ))
        }
    }
    notes.sort_unstable();

    // A slash bass always becomes the lowest sounding note.
    if let Some(bass_pc) = chord.bass {
        let floor = *notes.first().unwrap_or(&root_midi);
        let n = bass_below(bass_pc, floor);
        if !notes.contains(&n) {
            notes.push(n);
        }
    }
    notes.sort_unstable();

    if opts.add_bass {
        let pc = chord.bass.unwrap_or(chord.root);
        let floor = *notes.first().unwrap_or(&root_midi);
        let n = bass_below(pc, floor);
        if !notes.contains(&n) {
            notes.push(n);
        }
    }

    for n in notes.iter_mut() {
        *n += opts.transpose;
    }
    notes.sort_unstable();
    notes.dedup();

    if let Some(bad) = notes.iter().find(|n| !(0..=127).contains(*n)) {
        return Err(format!(
            "chord '{}' voices to MIDI note {bad}, outside the valid range 0-127 — raise or lower \
             the octave or reduce the transposition",
            chord.symbol
        ));
    }
    Ok(notes)
}

/// Pick the inversion + octave placement of `base` nearest to `prev`.
///
/// Distance is measured between the mean pitches, which is the cheap standard
/// proxy for "how far the hand moved"; ties keep the lower rotation so the
/// result stays deterministic across surfaces.
fn smooth_voicing(base: &[i32], prev: Option<&[i32]>) -> Vec<i32> {
    let prev = match prev {
        Some(p) if !p.is_empty() => p,
        _ => return base.to_vec(),
    };
    let mean = |v: &[i32]| v.iter().map(|x| *x as f64).sum::<f64>() / v.len() as f64;
    let target = mean(prev);

    let mut best: Option<(f64, Vec<i32>)> = None;
    for rot in 0..base.len().max(1) {
        let mut cand = base.to_vec();
        invert(&mut cand, rot);
        for shift in [-12, 0, 12] {
            let shifted: Vec<i32> = cand.iter().map(|n| n + shift).collect();
            let d = (mean(&shifted) - target).abs();
            if best.as_ref().is_none_or(|(bd, _)| d < *bd - 1e-9) {
                best = Some((d, shifted));
            }
        }
    }
    best.map(|(_, v)| v).unwrap_or_else(|| base.to_vec())
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// Convert a written progression into a Standard MIDI File.
pub fn convert(progression: &str, opts: &Options) -> Result<Conversion, String> {
    if !(20.0..=400.0).contains(&opts.tempo) || !opts.tempo.is_finite() {
        return Err(format!(
            "tempo must be between 20 and 400 BPM, got {}",
            opts.tempo
        ));
    }
    if !(0.25..=64.0).contains(&opts.beats_per_chord) || !opts.beats_per_chord.is_finite() {
        return Err(format!(
            "beats_per_chord must be between 0.25 and 64, got {}",
            opts.beats_per_chord
        ));
    }
    if !(1..=16).contains(&opts.beats_per_bar) {
        return Err(format!(
            "beats_per_bar must be between 1 and 16, got {}",
            opts.beats_per_bar
        ));
    }
    if !(0..=8).contains(&opts.octave) {
        return Err(format!("octave must be between 0 and 8, got {}", opts.octave));
    }
    if !(-24..=24).contains(&opts.transpose) {
        return Err(format!(
            "transpose must be between -24 and 24 semitones, got {}",
            opts.transpose
        ));
    }
    if !(5.0..=100.0).contains(&opts.note_length) || !opts.note_length.is_finite() {
        return Err(format!(
            "note_length must be between 5 and 100 percent, got {}",
            opts.note_length
        ));
    }
    if opts.velocity == 0 || opts.velocity > 127 {
        return Err(format!(
            "velocity must be between 1 and 127, got {}",
            opts.velocity
        ));
    }
    let program = instrument_program(&opts.instrument)?;
    let arp_beats = NOTE_VALUES
        .iter()
        .find(|(n, _)| *n == opts.arp_note)
        .map(|(_, b)| *b)
        .ok_or_else(|| {
            format!(
                "unknown arp_note '{}': expected one of {}",
                opts.arp_note,
                NOTE_VALUES
                    .iter()
                    .map(|(n, _)| *n)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;
    if !matches!(
        opts.pattern.as_str(),
        "block" | "arpeggio-up" | "arpeggio-down" | "arpeggio-updown" | "strum"
    ) {
        return Err(format!(
            "unknown pattern '{}': expected block, arpeggio-up, arpeggio-down, arpeggio-updown or \
             strum",
            opts.pattern
        ));
    }

    let chords = parse_progression(progression, opts.beats_per_chord)?;

    // (tick_on, tick_off, pitch)
    let mut notes: Vec<(u32, u32, u8)> = Vec::new();
    let mut detail = Vec::new();
    let mut tick: u32 = 0;
    let mut total_beats = 0.0;
    let mut sounding = 0usize;
    let mut prev: Option<Vec<i32>> = None;

    for chord in &chords {
        let slot = (chord.beats * PPQ as f64).round().max(1.0) as u32;
        total_beats += chord.beats;

        if chord.rest {
            detail.push(format!("{} — rest ({} beats)", chord.symbol, fmt(chord.beats)));
            tick += slot;
            continue;
        }
        let voiced = voice_chord(chord, opts, prev.as_deref())?;
        sounding += 1;
        detail.push(format!(
            "{} — {} ({} beats)",
            chord.symbol,
            voiced
                .iter()
                .map(|n| name_with_spelling(*n, chord.flats))
                .collect::<Vec<_>>()
                .join(" "),
            fmt(chord.beats)
        ));
        prev = Some(voiced.clone());

        emit_slot(&mut notes, &voiced, tick, slot, arp_beats, opts);
        if notes.len() > MAX_NOTES {
            return Err(format!(
                "too many notes: the limit is {MAX_NOTES} — shorten the progression or use a \
                 slower arpeggio step"
            ));
        }
        tick += slot;
    }

    if notes.is_empty() {
        return Err("the progression contains only rests, so there is nothing to write".into());
    }
    let lowest = notes.iter().map(|(_, _, p)| *p).min().unwrap_or(0);
    let highest = notes.iter().map(|(_, _, p)| *p).max().unwrap_or(0);
    let midi = write_smf(&notes, opts, program, tick)?;
    let seconds = total_beats * 60.0 / opts.tempo;

    Ok(Conversion {
        midi,
        notes: notes.len(),
        slots: chords.len(),
        chords: sounding,
        beats: total_beats,
        seconds,
        lowest,
        highest,
        detail,
    })
}

/// Trim a beat count to a short human form (`4`, `1.5`).
fn fmt(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// Lay one voiced chord out in time according to `pattern`.
fn emit_slot(
    out: &mut Vec<(u32, u32, u8)>,
    voiced: &[i32],
    start: u32,
    slot: u32,
    arp_beats: f64,
    opts: &Options,
) {
    let gate = |span: u32| -> u32 { ((span as f64 * opts.note_length / 100.0).round() as u32).max(1) };

    match opts.pattern.as_str() {
        "block" => {
            let len = gate(slot);
            for n in voiced {
                out.push((start, start + len, *n as u8));
            }
        }
        "strum" => {
            // Every note still ends together; only the onsets are staggered, so
            // a long chord doesn't turn into an arpeggio.
            let spread = STRUM_TICKS.min(slot.saturating_sub(1) / voiced.len().max(1) as u32);
            let end = start + gate(slot);
            for (i, n) in voiced.iter().enumerate() {
                let on = start + spread * i as u32;
                out.push((on, end.max(on + 1), *n as u8));
            }
        }
        _ => {
            let step = ((arp_beats * PPQ as f64).round() as u32).max(1);
            let order = arp_order(voiced, &opts.pattern);
            let len = gate(step);
            let mut k = 0u32;
            while k * step < slot {
                let on = start + k * step;
                let off = (on + len).min(start + slot);
                out.push((on, off.max(on + 1), order[k as usize % order.len()] as u8));
                k += 1;
            }
        }
    }
}

/// The order the arpeggio walks the chord's notes in.
fn arp_order(voiced: &[i32], pattern: &str) -> Vec<i32> {
    let mut up = voiced.to_vec();
    up.sort_unstable();
    match pattern {
        "arpeggio-down" => {
            up.reverse();
            up
        }
        "arpeggio-updown" => {
            let mut v = up.clone();
            // Walk back down without repeating the top and bottom notes.
            for n in up.iter().rev().skip(1).take(up.len().saturating_sub(2)) {
                v.push(*n);
            }
            v
        }
        _ => up,
    }
}

/// Serialize the note list as a format-0 Standard MIDI File.
fn write_smf(
    notes: &[(u32, u32, u8)],
    opts: &Options,
    program: u8,
    end_tick: u32,
) -> Result<Vec<u8>, String> {
    // (tick, order, kind) — note-offs sort before note-ons at the same tick so a
    // repeated pitch retriggers instead of cancelling itself.
    let mut events: Vec<(u32, u8, TrackEventKind<'static>)> = Vec::new();
    let micros_per_beat = (60_000_000.0 / opts.tempo).round().clamp(1.0, 16_777_215.0) as u32;
    // The time-signature denominator is fixed at 4 (a quarter note), which is
    // the `2` below: SMF stores it as a negative power of two.
    let denom_pow = 2u8;

    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::TrackName(&b"Chord Progression"[..])),
    ));
    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::Tempo(u24::from(micros_per_beat))),
    ));
    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::TimeSignature(
            opts.beats_per_bar as u8,
            denom_pow,
            24,
            8,
        )),
    ));
    events.push((
        0,
        0,
        TrackEventKind::Midi {
            channel: u4::from(0),
            message: MidiMessage::ProgramChange {
                program: u7::from(program),
            },
        },
    ));

    for (on, off, pitch) in notes {
        events.push((
            *on,
            2,
            TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOn {
                    key: u7::from(*pitch),
                    vel: u7::from(opts.velocity),
                },
            },
        ));
        events.push((
            *off,
            1,
            TrackEventKind::Midi {
                channel: u4::from(0),
                message: MidiMessage::NoteOff {
                    key: u7::from(*pitch),
                    vel: u7::from(0),
                },
            },
        ));
    }
    events.sort_by_key(|(tick, order, _)| (*tick, *order));

    let last = events.last().map(|(t, _, _)| *t).unwrap_or(0).max(end_tick);
    let mut track: Vec<TrackEvent<'static>> = Vec::with_capacity(events.len() + 1);
    let mut prev = 0u32;
    for (tick, _, kind) in events {
        track.push(TrackEvent {
            delta: u28::from(tick - prev),
            kind,
        });
        prev = tick;
    }
    track.push(TrackEvent {
        delta: u28::from(last - prev),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });

    let smf = Smf {
        header: Header::new(Format::SingleTrack, Timing::Metrical(u15::from(PPQ))),
        tracks: vec![track],
    };
    let mut out = Vec::new();
    smf.write_std(&mut out)
        .map_err(|e| format!("failed to write the MIDI file: {e}"))?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// Surface helpers (shared by the page wrapper)
// ---------------------------------------------------------------------------

/// Parse a numeric page field, treating blank as the default.
pub fn parse_field<T: std::str::FromStr>(name: &str, raw: &str, default: T) -> Result<T, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<T>()
        .map_err(|_| format!("{name} must be a number, got '{t}'"))
}

/// Positive-truthy reading of a checkbox field value.
pub fn truthy(raw: &str, default: bool) -> bool {
    match raw.trim() {
        "" => default,
        v => matches!(v, "true" | "1" | "on" | "yes"),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode the produced file back into (tick, pitch, on/off) triples.
    fn read_notes(bytes: &[u8]) -> Vec<(u32, u8, bool)> {
        let smf = Smf::parse(bytes).expect("produced a parseable SMF");
        let mut out = Vec::new();
        let mut tick = 0u32;
        for ev in &smf.tracks[0] {
            tick += ev.delta.as_int();
            if let TrackEventKind::Midi { message, .. } = ev.kind {
                match message {
                    MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                        out.push((tick, key.as_int(), true))
                    }
                    MidiMessage::NoteOff { key, .. } => out.push((tick, key.as_int(), false)),
                    _ => {}
                }
            }
        }
        out
    }

    fn names(chord: &str, opts: &Options) -> String {
        let c = parse_chord_symbol(chord).unwrap();
        voice_chord(&c, opts, None)
            .unwrap()
            .iter()
            .map(|n| midi_to_name(*n))
            .collect::<Vec<_>>()
            .join(" ")
    }

    // --- happy path -------------------------------------------------------

    #[test]
    fn converts_a_basic_pop_progression() {
        let out = convert("C G Am F", &Options::default()).unwrap();
        assert_eq!(out.slots, 4);
        assert_eq!(out.chords, 4);
        assert_eq!(out.notes, 12, "four triads, block chords");
        assert_eq!(out.beats, 16.0);
        assert!((out.seconds - 8.0).abs() < 1e-9, "16 beats at 120 BPM");
        assert_eq!(midi_to_name(out.lowest as i32), "C4");

        let events = read_notes(&out.midi);
        let ons: Vec<_> = events.iter().filter(|(_, _, on)| *on).collect();
        assert_eq!(ons.len(), 12);
        // C major in root position at tick 0, then G at one bar (4 × 480).
        assert_eq!(ons[0], &(0, 60, true));
        assert_eq!(ons[1], &(0, 64, true));
        assert_eq!(ons[2], &(0, 67, true));
        assert_eq!(ons[3].0, 1920);
        assert_eq!(out.detail[0], "C — C4 E4 G4 (4 beats)");
        assert_eq!(out.detail[3], "F — F4 A4 C5 (4 beats)");
    }

    #[test]
    fn writes_a_parseable_single_track_file_with_tempo_and_program() {
        let opts = Options {
            tempo: 90.0,
            beats_per_bar: 3,
            instrument: "electric-piano".into(),
            ..Options::default()
        };
        let out = convert("Dm", &opts).unwrap();
        let smf = Smf::parse(&out.midi).unwrap();
        assert_eq!(smf.tracks.len(), 1);
        assert!(matches!(smf.header.timing, Timing::Metrical(t) if t.as_int() == PPQ));

        let mut tempo = None;
        let mut program = None;
        let mut numerator = None;
        for ev in &smf.tracks[0] {
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => tempo = Some(t.as_int()),
                TrackEventKind::Meta(MetaMessage::TimeSignature(n, _, _, _)) => numerator = Some(n),
                TrackEventKind::Midi {
                    message: MidiMessage::ProgramChange { program: p },
                    ..
                } => program = Some(p.as_int()),
                _ => {}
            }
        }
        assert_eq!(tempo, Some(666_667), "90 BPM in microseconds per beat");
        assert_eq!(numerator, Some(3));
        assert_eq!(program, Some(4), "electric piano");
    }

    // --- chord vocabulary -------------------------------------------------

    #[test]
    fn parses_the_common_chord_qualities() {
        let d = Options::default();
        assert_eq!(names("C", &d), "C4 E4 G4");
        assert_eq!(names("Cm", &d), "C4 D#4 G4");
        assert_eq!(names("C7", &d), "C4 E4 G4 A#4");
        assert_eq!(names("Cmaj7", &d), "C4 E4 G4 B4");
        assert_eq!(names("CM7", &d), "C4 E4 G4 B4", "M7 is a major seventh");
        assert_eq!(names("Cm7", &d), "C4 D#4 G4 A#4");
        assert_eq!(names("Cmin7", &d), "C4 D#4 G4 A#4");
        assert_eq!(names("C-7", &d), "C4 D#4 G4 A#4", "the jazz minus sign");
        assert_eq!(names("Cdim", &d), "C4 D#4 F#4");
        assert_eq!(names("Cdim7", &d), "C4 D#4 F#4 A4");
        assert_eq!(names("Caug", &d), "C4 E4 G#4");
        assert_eq!(names("C+", &d), "C4 E4 G#4");
        assert_eq!(names("Csus2", &d), "C4 D4 G4");
        assert_eq!(names("Csus4", &d), "C4 F4 G4");
        assert_eq!(names("C7sus4", &d), "C4 F4 G4 A#4");
        assert_eq!(names("C6", &d), "C4 E4 G4 A4");
        assert_eq!(names("C9", &d), "C4 E4 G4 A#4 D5");
        assert_eq!(names("Cadd9", &d), "C4 E4 G4 D5");
        assert_eq!(names("Cm7b5", &d), "C4 D#4 F#4 A#4");
        assert_eq!(names("Cø", &d), "C4 D#4 F#4 A#4", "half-diminished glyph");
        assert_eq!(names("C°7", &d), "C4 D#4 F#4 A4", "diminished glyph");
        assert_eq!(names("CΔ7", &d), "C4 E4 G4 B4", "major-seventh triangle");
        assert_eq!(names("C7#9", &d), "C4 E4 G4 A#4 D#5");
        assert_eq!(names("Cmaj7#11", &d), "C4 E4 G4 B4 F#5");
        assert_eq!(names("C13", &d), "C4 E4 G4 A#4 D5 A5");
        assert_eq!(names("Cm(maj7)", &d), "C4 D#4 G4 B4");
        assert_eq!(names("C5", &d), "C4 G4", "power chord");
    }

    #[test]
    fn handles_accidentals_slash_bass_and_flat_spelling() {
        let d = Options::default();
        assert_eq!(names("F#m", &d), "F#4 A4 C#5");
        assert_eq!(names("Bb7", &d), "A#4 D5 F5 G#5");
        // A slash bass drops below the voicing rather than joining it on top.
        assert_eq!(names("C/E", &d), "E3 C4 E4 G4");
        assert_eq!(names("C/G", &d), "G3 C4 E4 G4");

        // A flat-spelled root reads its notes back with flats.
        let out = convert("Bb Eb", &Options::default()).unwrap();
        assert_eq!(out.detail[0], "Bb — Bb4 D5 F5 (4 beats)");
        assert_eq!(out.detail[1], "Eb — Eb4 G4 Bb4 (4 beats)");
    }

    // --- timing -----------------------------------------------------------

    #[test]
    fn per_chord_durations_and_rests_control_the_timeline() {
        let out = convert("C:2 G:2 - Am", &Options::default()).unwrap();
        assert_eq!(out.slots, 4);
        assert_eq!(out.chords, 3, "the rest is a slot but not a chord");
        assert_eq!(out.beats, 2.0 + 2.0 + 4.0 + 4.0);
        let ons: Vec<_> = read_notes(&out.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(t, p, _)| (t, p))
            .collect();
        assert_eq!(ons[0], (0, 60));
        assert_eq!(ons[3], (960, 67), "G starts after two beats");
        // The rest occupies beats 4-8, so A minor starts at beat 8.
        assert_eq!(ons[6], (3840, 69));
        assert_eq!(out.detail[2], "- — rest (4 beats)");
    }

    #[test]
    fn bar_lines_commas_and_newlines_are_all_separators() {
        let a = convert("C G Am F", &Options::default()).unwrap();
        let b = convert("| C | G |\nAm, F |", &Options::default()).unwrap();
        assert_eq!(a.midi, b.midi);
    }

    #[test]
    fn note_length_shortens_each_note_within_its_slot() {
        let opts = Options {
            note_length: 50.0,
            ..Options::default()
        };
        let out = convert("C", &opts).unwrap();
        let offs: Vec<_> = read_notes(&out.midi)
            .into_iter()
            .filter(|(_, _, on)| !*on)
            .map(|(t, _, _)| t)
            .collect();
        assert_eq!(offs, vec![960, 960, 960], "half of a four-beat slot");
    }

    // --- voicing ----------------------------------------------------------

    #[test]
    fn inversions_and_voicings_move_the_notes_as_documented() {
        let inv = |name: &str| {
            names(
                "C",
                &Options {
                    inversion: name.into(),
                    ..Options::default()
                },
            )
        };
        assert_eq!(inv("root"), "C4 E4 G4");
        assert_eq!(inv("first"), "E4 G4 C5");
        assert_eq!(inv("second"), "G4 C5 E5");
        // A triad has no third inversion, so it clamps to the second.
        assert_eq!(inv("third"), "G4 C5 E5");

        let voi = |name: &str| {
            names(
                "Cmaj7",
                &Options {
                    voicing: name.into(),
                    ..Options::default()
                },
            )
        };
        assert_eq!(voi("close"), "C4 E4 G4 B4");
        assert_eq!(voi("drop-2"), "G3 C4 E4 B4");
        assert_eq!(voi("drop-3"), "E3 C4 G4 B4");
        assert_eq!(voi("spread"), "C3 E4 G4 B4");
    }

    #[test]
    fn smooth_inversion_keeps_the_voicing_near_the_previous_chord() {
        let opts = Options {
            inversion: "smooth".into(),
            ..Options::default()
        };
        let out = convert("C F G C", &opts).unwrap();
        // Root position for the first chord, then the nearest inversion of each
        // following chord instead of jumping back to the root every time.
        assert_eq!(out.detail[0], "C — C4 E4 G4 (4 beats)");
        assert_eq!(out.detail[1], "F — C4 F4 A4 (4 beats)");
        assert_eq!(out.detail[2], "G — B3 D4 G4 (4 beats)");
        assert_eq!(out.detail[3], "C — C4 E4 G4 (4 beats)");
    }

    #[test]
    fn octave_transpose_and_added_bass_shift_pitch_as_documented() {
        let d = Options::default();
        assert_eq!(
            names(
                "C",
                &Options {
                    octave: 3,
                    ..d.clone()
                }
            ),
            "C3 E3 G3"
        );
        assert_eq!(
            names(
                "C",
                &Options {
                    transpose: 2,
                    ..d.clone()
                }
            ),
            "D4 F#4 A4"
        );
        assert_eq!(
            names(
                "C",
                &Options {
                    add_bass: true,
                    ..d.clone()
                }
            ),
            "C3 C4 E4 G4"
        );
        // With a slash bass the doubled note is the written bass, not the root.
        assert_eq!(
            names(
                "C/E",
                &Options {
                    add_bass: true,
                    ..d
                }
            ),
            "E2 E3 C4 E4 G4"
        );
    }

    // --- patterns ---------------------------------------------------------

    #[test]
    fn arpeggio_patterns_spread_the_chord_over_its_slot() {
        let base = Options {
            pattern: "arpeggio-up".into(),
            arp_note: "quarter".into(),
            ..Options::default()
        };
        let up = convert("C", &base).unwrap();
        let ons: Vec<_> = read_notes(&up.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(t, p, _)| (t, p))
            .collect();
        // Four quarter-note steps in a four-beat slot, cycling C E G C.
        assert_eq!(ons, vec![(0, 60), (480, 64), (960, 67), (1440, 60)]);

        let down = convert(
            "C",
            &Options {
                pattern: "arpeggio-down".into(),
                ..base.clone()
            },
        )
        .unwrap();
        let pitches: Vec<_> = read_notes(&down.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(_, p, _)| p)
            .collect();
        assert_eq!(pitches, vec![67, 64, 60, 67]);

        let ud = convert(
            "Cmaj7",
            &Options {
                pattern: "arpeggio-updown".into(),
                ..base
            },
        )
        .unwrap();
        let pitches: Vec<_> = read_notes(&ud.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(_, p, _)| p)
            .collect();
        assert_eq!(pitches, vec![60, 64, 67, 71], "up then back down");
    }

    #[test]
    fn strum_staggers_the_onsets_but_ends_together() {
        let opts = Options {
            pattern: "strum".into(),
            note_length: 100.0,
            ..Options::default()
        };
        let out = convert("C", &opts).unwrap();
        let events = read_notes(&out.midi);
        let ons: Vec<_> = events
            .iter()
            .filter(|(_, _, on)| *on)
            .map(|(t, _, _)| *t)
            .collect();
        let offs: Vec<_> = events
            .iter()
            .filter(|(_, _, on)| !*on)
            .map(|(t, _, _)| *t)
            .collect();
        assert_eq!(ons, vec![0, 30, 60], "a 64th note apart");
        assert_eq!(offs, vec![1920, 1920, 1920]);
    }

    // --- errors -----------------------------------------------------------

    #[test]
    fn rejects_an_empty_progression() {
        let err = convert("   ", &Options::default()).unwrap_err();
        assert!(err.contains("no chords found"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_root_note() {
        let err = convert("C Hm G", &Options::default()).unwrap_err();
        assert!(err.contains("'H' is not a note name"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_chord_quality() {
        let err = convert("C Gwobble", &Options::default()).unwrap_err();
        assert!(err.contains("don't understand"), "{err}");
        assert!(err.contains("Gwobble"), "{err}");
    }

    #[test]
    fn rejects_a_bad_duration_suffix() {
        let err = convert("C:two", &Options::default()).unwrap_err();
        assert!(err.contains("must be a number of beats"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_settings() {
        let bad = |o: Options| convert("C", &o).unwrap_err();
        assert!(bad(Options {
            tempo: 1000.0,
            ..Options::default()
        })
        .contains("tempo must be between 20 and 400"));
        assert!(bad(Options {
            velocity: 0,
            ..Options::default()
        })
        .contains("velocity must be between 1 and 127"));
        assert!(bad(Options {
            instrument: "kazoo".into(),
            ..Options::default()
        })
        .contains("unknown instrument 'kazoo'"));
        assert!(bad(Options {
            pattern: "shuffle".into(),
            ..Options::default()
        })
        .contains("unknown pattern 'shuffle'"));
    }

    #[test]
    fn rejects_a_voicing_that_falls_outside_the_midi_range() {
        let err = convert(
            "C13",
            &Options {
                octave: 8,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("outside the valid range 0-127"), "{err}");
    }

    #[test]
    fn rejects_a_progression_of_only_rests() {
        let err = convert("- - -", &Options::default()).unwrap_err();
        assert!(err.contains("only rests"), "{err}");
    }

    #[test]
    fn rejects_input_beyond_the_size_and_slot_caps() {
        let huge = "C ".repeat(MAX_INPUT_BYTES);
        let err = convert(&huge, &Options::default()).unwrap_err();
        assert!(err.contains("too long"), "{err}");

        let many = "C ".repeat(MAX_CHORDS + 5);
        let err = convert(&many, &Options::default()).unwrap_err();
        assert!(err.contains("too many chords"), "{err}");
    }

    // --- helpers ----------------------------------------------------------

    #[test]
    fn page_field_helpers_treat_blank_as_the_default() {
        assert_eq!(parse_field("tempo", "", 120.0).unwrap(), 120.0);
        assert_eq!(parse_field("tempo", " 90 ", 120.0).unwrap(), 90.0);
        assert!(parse_field("tempo", "fast", 120.0).unwrap_err().contains("must be a number"));
        assert!(truthy("true", false));
        assert!(!truthy("false", true));
        assert!(truthy("", true));
    }
}
