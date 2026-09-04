//! scale-chord-finder core — two directions of the same lookup, sharing one
//! catalogue of scales:
//!
//! * `find` — given a set of notes, report every scale/mode whose pitch-class
//!   set contains them (or matches them exactly, or misses at most one), ranked
//!   by how tightly it fits.
//! * `list` — given a root and a scale/mode, report its spelled notes, scale
//!   degrees, step pattern and diatonic chords.
//!
//! Every scale is authored ONCE, as a list of `(degree, alteration)` pairs
//! against the major scale. The semitone pattern is derived from that spec, so
//! the interval content and the spelling can never disagree: `lydian`'s fourth
//! degree is `(4, +1)`, which is both "one semitone above a perfect fourth" and
//! "the fourth letter, sharpened" — hence `F#` in G lydian, never `Gb`.
//!
//! Nothing here is random and nothing does I/O: the same `Options` always give
//! byte-identical output, so a result can be shared as a URL and reproduced.

// ---------------------------------------------------------------------------
// Vocabularies (also used by the descriptor so the enums can't drift)
// ---------------------------------------------------------------------------

/// Which direction of the lookup to run. `auto` picks `find` when `notes` is
/// non-empty and `list` otherwise.
pub const ACTIONS: [&str; 3] = ["auto", "find", "list"];

/// Root spellings offered for `list`, sharp and flat versions of every black key.
pub const KEYS: [&str; 17] = [
    "C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb", "G", "G#", "Ab", "A", "A#", "Bb", "B",
];

/// Tonics offered as a `find` filter — every key spelling plus `any`, which
/// searches all twelve roots.
pub const ROOTS: [&str; 18] = [
    "any", "C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb", "G", "G#", "Ab", "A", "A#",
    "Bb", "B",
];

/// How closely a scale has to fit the searched notes.
pub const FITS: [&str; 3] = ["contains", "exact", "near"];

/// Accidental preference for the printed note names.
pub const SPELLINGS: [&str; 3] = ["auto", "sharps", "flats"];

/// Which diatonic chords to build on each degree.
pub const CHORD_TYPES: [&str; 3] = ["triads", "sevenths", "both"];

/// Shape of the returned text.
pub const OUTPUTS: [&str; 4] = ["text", "names", "csv", "json"];

/// Upper bound on `max_results`.
pub const MAX_RESULTS: i32 = 50;

/// Most note tokens `notes` may carry before the input is rejected.
pub const MAX_NOTES: usize = 24;

// ---------------------------------------------------------------------------
// Note letters and the major-scale reference
// ---------------------------------------------------------------------------

const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
const LETTER_PC: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
/// Semitones above the tonic of the unaltered scale degrees 1-7.
const MAJOR_DEG: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const SHARP_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const FLAT_NAMES: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];
/// The spelling used for a root nothing else pins down — the common key names.
const DEFAULT_ROOT_NAMES: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];
const ROMAN: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];

// ---------------------------------------------------------------------------
// Scale catalogue
// ---------------------------------------------------------------------------

/// One scale or mode, authored as scale degrees against the major scale.
pub struct ScaleDef {
    /// Machine name (the enum value).
    pub name: &'static str,
    /// Human label for the page dropdown and the report header.
    pub label: &'static str,
    /// `(degree 1-7, alteration in semitones)` per scale tone, ascending.
    pub degrees: &'static [(u8, i8)],
}

pub const SCALE_COUNT: usize = 42;

/// Every scale searched by `find` and offered to `list`. Ordered by family so a
/// tie in the ranking resolves to the more familiar scale first.
pub const CATALOG: [ScaleDef; SCALE_COUNT] = [
    // --- the seven modes of the major scale ---
    ScaleDef {
        name: "major",
        label: "Major (Ionian)",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, 0)],
    },
    ScaleDef {
        name: "minor",
        label: "Natural minor (Aeolian)",
        degrees: &[(1, 0), (2, 0), (3, -1), (4, 0), (5, 0), (6, -1), (7, -1)],
    },
    ScaleDef {
        name: "dorian",
        label: "Dorian",
        degrees: &[(1, 0), (2, 0), (3, -1), (4, 0), (5, 0), (6, 0), (7, -1)],
    },
    ScaleDef {
        name: "phrygian",
        label: "Phrygian",
        degrees: &[(1, 0), (2, -1), (3, -1), (4, 0), (5, 0), (6, -1), (7, -1)],
    },
    ScaleDef {
        name: "lydian",
        label: "Lydian",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 1), (5, 0), (6, 0), (7, 0)],
    },
    ScaleDef {
        name: "mixolydian",
        label: "Mixolydian",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, 0), (7, -1)],
    },
    ScaleDef {
        name: "locrian",
        label: "Locrian",
        degrees: &[(1, 0), (2, -1), (3, -1), (4, 0), (5, -1), (6, -1), (7, -1)],
    },
    // --- pentatonic and blues ---
    ScaleDef {
        name: "major-pentatonic",
        label: "Major pentatonic",
        degrees: &[(1, 0), (2, 0), (3, 0), (5, 0), (6, 0)],
    },
    ScaleDef {
        name: "minor-pentatonic",
        label: "Minor pentatonic",
        degrees: &[(1, 0), (3, -1), (4, 0), (5, 0), (7, -1)],
    },
    ScaleDef {
        name: "egyptian-pentatonic",
        label: "Egyptian (suspended) pentatonic",
        degrees: &[(1, 0), (2, 0), (4, 0), (5, 0), (7, -1)],
    },
    ScaleDef {
        name: "blues",
        label: "Blues (minor)",
        degrees: &[(1, 0), (3, -1), (4, 0), (5, -1), (5, 0), (7, -1)],
    },
    ScaleDef {
        name: "major-blues",
        label: "Major blues",
        degrees: &[(1, 0), (2, 0), (3, -1), (3, 0), (5, 0), (6, 0)],
    },
    // --- harmonic minor and its modes ---
    ScaleDef {
        name: "harmonic-minor",
        label: "Harmonic minor",
        degrees: &[(1, 0), (2, 0), (3, -1), (4, 0), (5, 0), (6, -1), (7, 0)],
    },
    ScaleDef {
        name: "locrian-natural6",
        label: "Locrian natural 6",
        degrees: &[(1, 0), (2, -1), (3, -1), (4, 0), (5, -1), (6, 0), (7, -1)],
    },
    ScaleDef {
        name: "ionian-augmented",
        label: "Ionian augmented",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 1), (6, 0), (7, 0)],
    },
    ScaleDef {
        name: "ukrainian-dorian",
        label: "Ukrainian Dorian (Dorian #4)",
        degrees: &[(1, 0), (2, 0), (3, -1), (4, 1), (5, 0), (6, 0), (7, -1)],
    },
    ScaleDef {
        name: "phrygian-dominant",
        label: "Phrygian dominant",
        degrees: &[(1, 0), (2, -1), (3, 0), (4, 0), (5, 0), (6, -1), (7, -1)],
    },
    ScaleDef {
        name: "lydian-sharp2",
        label: "Lydian #2",
        degrees: &[(1, 0), (2, 1), (3, 0), (4, 1), (5, 0), (6, 0), (7, 0)],
    },
    ScaleDef {
        name: "altered-diminished",
        label: "Altered diminished",
        degrees: &[(1, 0), (2, -1), (3, -1), (4, -1), (5, -1), (6, -1), (7, -2)],
    },
    // --- melodic minor and its modes ---
    ScaleDef {
        name: "melodic-minor",
        label: "Melodic minor (ascending)",
        degrees: &[(1, 0), (2, 0), (3, -1), (4, 0), (5, 0), (6, 0), (7, 0)],
    },
    ScaleDef {
        name: "dorian-flat2",
        label: "Dorian b2",
        degrees: &[(1, 0), (2, -1), (3, -1), (4, 0), (5, 0), (6, 0), (7, -1)],
    },
    ScaleDef {
        name: "lydian-augmented",
        label: "Lydian augmented",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 1), (5, 1), (6, 0), (7, 0)],
    },
    ScaleDef {
        name: "lydian-dominant",
        label: "Lydian dominant",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 1), (5, 0), (6, 0), (7, -1)],
    },
    ScaleDef {
        name: "mixolydian-flat6",
        label: "Mixolydian b6",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, -1), (7, -1)],
    },
    ScaleDef {
        name: "locrian-natural2",
        label: "Locrian natural 2 (half-diminished)",
        degrees: &[(1, 0), (2, 0), (3, -1), (4, 0), (5, -1), (6, -1), (7, -1)],
    },
    ScaleDef {
        name: "altered",
        label: "Altered (super Locrian)",
        degrees: &[(1, 0), (2, -1), (3, -1), (4, -1), (5, -1), (6, -1), (7, -1)],
    },
    // --- other seven-note scales ---
    ScaleDef {
        name: "harmonic-major",
        label: "Harmonic major",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 0), (5, 0), (6, -1), (7, 0)],
    },
    ScaleDef {
        name: "double-harmonic",
        label: "Double harmonic (Byzantine)",
        degrees: &[(1, 0), (2, -1), (3, 0), (4, 0), (5, 0), (6, -1), (7, 0)],
    },
    ScaleDef {
        name: "hungarian-minor",
        label: "Hungarian minor",
        degrees: &[(1, 0), (2, 0), (3, -1), (4, 1), (5, 0), (6, -1), (7, 0)],
    },
    ScaleDef {
        name: "neapolitan-minor",
        label: "Neapolitan minor",
        degrees: &[(1, 0), (2, -1), (3, -1), (4, 0), (5, 0), (6, -1), (7, 0)],
    },
    ScaleDef {
        name: "neapolitan-major",
        label: "Neapolitan major",
        degrees: &[(1, 0), (2, -1), (3, -1), (4, 0), (5, 0), (6, 0), (7, 0)],
    },
    // --- symmetric ---
    ScaleDef {
        name: "whole-tone",
        label: "Whole tone",
        degrees: &[(1, 0), (2, 0), (3, 0), (4, 1), (5, 1), (6, 1)],
    },
    ScaleDef {
        name: "augmented",
        label: "Augmented (hexatonic)",
        degrees: &[(1, 0), (3, -1), (3, 0), (5, 0), (6, -1), (7, 0)],
    },
    ScaleDef {
        name: "diminished-whole-half",
        label: "Diminished (whole-half)",
        degrees: &[
            (1, 0),
            (2, 0),
            (3, -1),
            (4, 0),
            (5, -1),
            (6, -1),
            (6, 0),
            (7, 0),
        ],
    },
    ScaleDef {
        name: "diminished-half-whole",
        label: "Diminished (half-whole)",
        degrees: &[
            (1, 0),
            (2, -1),
            (3, -1),
            (3, 0),
            (4, 1),
            (5, 0),
            (6, 0),
            (7, -1),
        ],
    },
    // --- bebop ---
    ScaleDef {
        name: "bebop-dominant",
        label: "Bebop dominant",
        degrees: &[
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 0),
            (5, 0),
            (6, 0),
            (7, -1),
            (7, 0),
        ],
    },
    ScaleDef {
        name: "bebop-major",
        label: "Bebop major",
        degrees: &[
            (1, 0),
            (2, 0),
            (3, 0),
            (4, 0),
            (5, 0),
            (6, -1),
            (6, 0),
            (7, 0),
        ],
    },
    // --- Japanese pentatonics ---
    ScaleDef {
        name: "hirajoshi",
        label: "Hirajoshi",
        degrees: &[(1, 0), (2, 0), (3, -1), (5, 0), (6, -1)],
    },
    ScaleDef {
        name: "in-sen",
        label: "In sen",
        degrees: &[(1, 0), (2, -1), (4, 0), (5, 0), (7, -1)],
    },
    ScaleDef {
        name: "iwato",
        label: "Iwato",
        degrees: &[(1, 0), (2, -1), (4, 0), (5, -1), (7, -1)],
    },
    ScaleDef {
        name: "kumoi",
        label: "Kumoi",
        degrees: &[(1, 0), (2, 0), (3, -1), (5, 0), (6, 0)],
    },
    // --- everything ---
    ScaleDef {
        name: "chromatic",
        label: "Chromatic",
        degrees: &[
            (1, 0),
            (2, -1),
            (2, 0),
            (3, -1),
            (3, 0),
            (4, 0),
            (5, -1),
            (5, 0),
            (6, -1),
            (6, 0),
            (7, -1),
            (7, 0),
        ],
    },
];

/// The catalogue's names, for the descriptor's `scale` enum.
pub const SCALES: [&str; SCALE_COUNT] = scale_names();

const fn scale_names() -> [&'static str; SCALE_COUNT] {
    let mut out = [""; SCALE_COUNT];
    let mut i = 0;
    while i < SCALE_COUNT {
        out[i] = CATALOG[i].name;
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub action: String,
    pub notes: String,
    pub root: String,
    pub key: String,
    pub scale: String,
    pub fit: String,
    pub spelling: String,
    pub include_chords: bool,
    pub include_modes: bool,
    pub chord_type: String,
    pub max_results: i32,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            action: "auto".into(),
            notes: String::new(),
            root: "any".into(),
            key: "C".into(),
            scale: "major".into(),
            fit: "contains".into(),
            spelling: "auto".into(),
            include_chords: true,
            include_modes: true,
            chord_type: "triads".into(),
            max_results: 12,
            output: "text".into(),
        }
    }
}

/// Parse one page field, falling back to `default` when it is blank. The page
/// hands every control through as a string, so the parse lives here and all
/// three surfaces share the same validation.
pub fn parse_field<T: std::str::FromStr>(name: &str, raw: &str, default: T) -> Result<T, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<T>()
        .map_err(|_| format!("{name} must be a number, got '{t}'"))
}

/// Positive-truthy checkbox parsing: the page sends `"true"`/`"false"`, a blank
/// value means "keep the default".
pub fn truthy(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Notes
// ---------------------------------------------------------------------------

/// A spelled note: a letter plus an accidental. Two notes with the same pitch
/// class can still be different notes (`C#` and `Db`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Note {
    /// Index into `LETTERS` (0 = C .. 6 = B).
    pub letter: usize,
    /// Accidental in semitones: -2 = double flat .. +2 = double sharp.
    pub alter: i32,
}

impl Note {
    pub fn pc(&self) -> i32 {
        (LETTER_PC[self.letter] + self.alter).rem_euclid(12)
    }
    pub fn name(&self) -> String {
        format!("{}{}", LETTERS[self.letter], accidental(self.alter))
    }
}

fn accidental(alter: i32) -> String {
    match alter {
        0 => String::new(),
        n if n > 0 => "#".repeat(n as usize),
        n => "b".repeat((-n) as usize),
    }
}

/// Parse one note token: a letter A-G, any number of `#`/`b` (or the unicode
/// accidentals, or `x` for a double sharp), and an optional octave number that
/// is ignored because only the pitch class matters here.
pub fn parse_note(token: &str) -> Result<Note, String> {
    let t = token.trim();
    if t.is_empty() {
        return Err("empty note".into());
    }
    let chars: Vec<char> = t.chars().collect();
    let letter = match chars[0].to_ascii_uppercase() {
        'C' => 0,
        'D' => 1,
        'E' => 2,
        'F' => 3,
        'G' => 4,
        'A' => 5,
        'B' => 6,
        _ => {
            return Err(format!(
                "unknown note '{t}' — expected a letter A-G with an optional # or b, for example C, F# or Bb"
            ))
        }
    };
    let mut alter = 0i32;
    for (i, c) in chars.iter().enumerate().skip(1) {
        match c {
            '#' | '\u{266F}' => alter += 1,
            'b' | '\u{266D}' => alter -= 1,
            'x' | 'X' => alter += 2,
            '\u{266E}' => {}
            // A trailing octave number (C4, Eb3) is accepted and ignored.
            '0'..='9' | '-' => {
                if chars[i..].iter().all(|c| c.is_ascii_digit() || *c == '-') {
                    break;
                }
                return Err(format!("unknown note '{t}' — the octave number must come last, as in C4"));
            }
            _ => {
                return Err(format!(
                    "unknown note '{t}' — expected a letter A-G with an optional # or b, for example C, F# or Bb"
                ))
            }
        }
    }
    if !(-2..=2).contains(&alter) {
        return Err(format!(
            "note '{t}' has too many accidentals — at most a double sharp (##) or double flat (bb)"
        ));
    }
    Ok(Note { letter, alter })
}

// ---------------------------------------------------------------------------
// Chord symbols
// ---------------------------------------------------------------------------

/// One chord quality, authored as `(degree, alteration)` pairs against the
/// major scale — the same spec the scales use, so `Cmaj7` spells itself
/// letterwise (`C E G B`) exactly the way a scale does, and `C7b9` gets a `Db`
/// rather than a `C#`.
pub struct ChordDef {
    /// The normalised suffix that selects this quality (see `normalize_quality`).
    pub suffix: &'static str,
    pub degrees: &'static [(u8, i8)],
}

const MAJ: &[(u8, i8)] = &[(1, 0), (3, 0), (5, 0)];
const MIN: &[(u8, i8)] = &[(1, 0), (3, -1), (5, 0)];
const DIM: &[(u8, i8)] = &[(1, 0), (3, -1), (5, -1)];
const AUG: &[(u8, i8)] = &[(1, 0), (3, 0), (5, 1)];
const FIVE: &[(u8, i8)] = &[(1, 0), (5, 0)];
const SUS2: &[(u8, i8)] = &[(1, 0), (2, 0), (5, 0)];
const SUS4: &[(u8, i8)] = &[(1, 0), (4, 0), (5, 0)];
const SIX: &[(u8, i8)] = &[(1, 0), (3, 0), (5, 0), (6, 0)];
const MIN6: &[(u8, i8)] = &[(1, 0), (3, -1), (5, 0), (6, 0)];
const SIX9: &[(u8, i8)] = &[(1, 0), (2, 0), (3, 0), (5, 0), (6, 0)];
const DOM7: &[(u8, i8)] = &[(1, 0), (3, 0), (5, 0), (7, -1)];
const MAJ7: &[(u8, i8)] = &[(1, 0), (3, 0), (5, 0), (7, 0)];
const MIN7: &[(u8, i8)] = &[(1, 0), (3, -1), (5, 0), (7, -1)];
const MINMAJ7: &[(u8, i8)] = &[(1, 0), (3, -1), (5, 0), (7, 0)];
const DIM7: &[(u8, i8)] = &[(1, 0), (3, -1), (5, -1), (7, -2)];
const HALFDIM: &[(u8, i8)] = &[(1, 0), (3, -1), (5, -1), (7, -1)];
const AUG7: &[(u8, i8)] = &[(1, 0), (3, 0), (5, 1), (7, -1)];
const MAJ7S5: &[(u8, i8)] = &[(1, 0), (3, 0), (5, 1), (7, 0)];
const DOM7B5: &[(u8, i8)] = &[(1, 0), (3, 0), (5, -1), (7, -1)];
const MAJ7B5: &[(u8, i8)] = &[(1, 0), (3, 0), (5, -1), (7, 0)];
const DOM7SUS4: &[(u8, i8)] = &[(1, 0), (4, 0), (5, 0), (7, -1)];
const DOM7SUS2: &[(u8, i8)] = &[(1, 0), (2, 0), (5, 0), (7, -1)];
const ADD9: &[(u8, i8)] = &[(1, 0), (2, 0), (3, 0), (5, 0)];
const MINADD9: &[(u8, i8)] = &[(1, 0), (2, 0), (3, -1), (5, 0)];
const DOM9: &[(u8, i8)] = &[(1, 0), (2, 0), (3, 0), (5, 0), (7, -1)];
const MAJ9: &[(u8, i8)] = &[(1, 0), (2, 0), (3, 0), (5, 0), (7, 0)];
const MIN9: &[(u8, i8)] = &[(1, 0), (2, 0), (3, -1), (5, 0), (7, -1)];
const DOM11: &[(u8, i8)] = &[(1, 0), (2, 0), (4, 0), (5, 0), (7, -1)];
const MIN11: &[(u8, i8)] = &[(1, 0), (2, 0), (4, 0), (5, 0), (7, -1)];
const MAJ11: &[(u8, i8)] = &[(1, 0), (2, 0), (4, 0), (5, 0), (7, 0)];
const DOM13: &[(u8, i8)] = &[(1, 0), (2, 0), (3, 0), (5, 0), (6, 0), (7, -1)];
const MIN13: &[(u8, i8)] = &[(1, 0), (2, 0), (3, -1), (5, 0), (6, 0), (7, -1)];
const MAJ13: &[(u8, i8)] = &[(1, 0), (2, 0), (3, 0), (5, 0), (6, 0), (7, 0)];
const DOM7B9: &[(u8, i8)] = &[(1, 0), (2, -1), (3, 0), (5, 0), (7, -1)];
const DOM7S9: &[(u8, i8)] = &[(1, 0), (2, 1), (3, 0), (5, 0), (7, -1)];
const DOM7S11: &[(u8, i8)] = &[(1, 0), (3, 0), (4, 1), (5, 0), (7, -1)];
const DOM7B13: &[(u8, i8)] = &[(1, 0), (3, 0), (5, 0), (6, -1), (7, -1)];
const DOM7S5B9: &[(u8, i8)] = &[(1, 0), (2, -1), (3, 0), (5, 1), (7, -1)];

/// Every chord suffix `notes` accepts, keyed by its normalised form. Several
/// spellings of the same quality (`m` / `min` / `-`, `M7` / `maj7` / `Δ7`)
/// normalise onto one entry rather than being repeated here.
pub const CHORD_QUALITIES: &[ChordDef] = &[
    ChordDef {
        suffix: "maj",
        degrees: MAJ,
    },
    ChordDef {
        suffix: "m",
        degrees: MIN,
    },
    ChordDef {
        suffix: "dim",
        degrees: DIM,
    },
    ChordDef {
        suffix: "aug",
        degrees: AUG,
    },
    ChordDef {
        suffix: "5",
        degrees: FIVE,
    },
    ChordDef {
        suffix: "sus2",
        degrees: SUS2,
    },
    ChordDef {
        suffix: "sus4",
        degrees: SUS4,
    },
    ChordDef {
        suffix: "sus",
        degrees: SUS4,
    },
    ChordDef {
        suffix: "6",
        degrees: SIX,
    },
    ChordDef {
        suffix: "m6",
        degrees: MIN6,
    },
    ChordDef {
        suffix: "69",
        degrees: SIX9,
    },
    ChordDef {
        suffix: "7",
        degrees: DOM7,
    },
    ChordDef {
        suffix: "dom7",
        degrees: DOM7,
    },
    ChordDef {
        suffix: "maj7",
        degrees: MAJ7,
    },
    ChordDef {
        suffix: "m7",
        degrees: MIN7,
    },
    ChordDef {
        suffix: "mmaj7",
        degrees: MINMAJ7,
    },
    ChordDef {
        suffix: "dim7",
        degrees: DIM7,
    },
    ChordDef {
        suffix: "m7b5",
        degrees: HALFDIM,
    },
    ChordDef {
        suffix: "aug7",
        degrees: AUG7,
    },
    ChordDef {
        suffix: "7#5",
        degrees: AUG7,
    },
    ChordDef {
        suffix: "maj7#5",
        degrees: MAJ7S5,
    },
    ChordDef {
        suffix: "7b5",
        degrees: DOM7B5,
    },
    ChordDef {
        suffix: "maj7b5",
        degrees: MAJ7B5,
    },
    ChordDef {
        suffix: "7sus4",
        degrees: DOM7SUS4,
    },
    ChordDef {
        suffix: "7sus",
        degrees: DOM7SUS4,
    },
    ChordDef {
        suffix: "7sus2",
        degrees: DOM7SUS2,
    },
    ChordDef {
        suffix: "add9",
        degrees: ADD9,
    },
    ChordDef {
        suffix: "madd9",
        degrees: MINADD9,
    },
    ChordDef {
        suffix: "9",
        degrees: DOM9,
    },
    ChordDef {
        suffix: "maj9",
        degrees: MAJ9,
    },
    ChordDef {
        suffix: "m9",
        degrees: MIN9,
    },
    ChordDef {
        suffix: "11",
        degrees: DOM11,
    },
    ChordDef {
        suffix: "m11",
        degrees: MIN11,
    },
    ChordDef {
        suffix: "maj11",
        degrees: MAJ11,
    },
    ChordDef {
        suffix: "13",
        degrees: DOM13,
    },
    ChordDef {
        suffix: "m13",
        degrees: MIN13,
    },
    ChordDef {
        suffix: "maj13",
        degrees: MAJ13,
    },
    ChordDef {
        suffix: "7b9",
        degrees: DOM7B9,
    },
    ChordDef {
        suffix: "7#9",
        degrees: DOM7S9,
    },
    ChordDef {
        suffix: "7#11",
        degrees: DOM7S11,
    },
    ChordDef {
        suffix: "7b13",
        degrees: DOM7B13,
    },
    ChordDef {
        suffix: "7#5b9",
        degrees: DOM7S5B9,
    },
];

/// Fold the many written forms of a chord quality onto the one spelling the
/// table uses: unicode symbols expand, `Maj`/`MAJ`/`M` all become `maj`,
/// `min`/`-` become `m`, and everything else lowercases.
fn normalize_quality(raw: &str) -> String {
    // Longest first, so `Δ7` never becomes `maj77` and `ø7` never `m7b57`.
    let pre = raw
        .replace('\u{266F}', "#")
        .replace('\u{266D}', "b")
        .replace("\u{0394}7", "maj7")
        .replace("\u{2206}7", "maj7")
        .replace('\u{0394}', "maj7")
        .replace('\u{2206}', "maj7")
        .replace("\u{00F8}7", "m7b5")
        .replace('\u{00F8}', "m7b5")
        .replace("\u{00B0}", "dim")
        .replace("\u{00BA}", "dim")
        .replace('+', "aug")
        .replace("6/9", "69");

    let chars: Vec<char> = pre.chars().collect();
    let mut out = String::with_capacity(chars.len());
    let mut i = 0;
    while i < chars.len() {
        let rest: String = chars[i..].iter().collect();
        let lower = rest.to_lowercase();
        let word = [
            ("major", "maj"),
            ("minor", "m"),
            ("maj", "maj"),
            ("min", "m"),
            ("dim", "dim"),
            ("aug", "aug"),
            ("sus", "sus"),
            ("add", "add"),
            ("dom", "dom"),
        ]
        .into_iter()
        .find(|(w, _)| lower.starts_with(w));
        match word {
            Some((w, repl)) => {
                out.push_str(repl);
                i += w.chars().count();
            }
            // A bare capital M is "major"; a bare lowercase m is "minor".
            None if chars[i] == 'M' => {
                out.push_str("maj");
                i += 1;
            }
            None => {
                out.extend(chars[i].to_lowercase());
                i += 1;
            }
        }
    }
    // Jazz lead sheets write minor as a leading dash: `C-7` is `Cm7`.
    match out.strip_prefix('-') {
        Some(rest) => format!("m{rest}"),
        None => out,
    }
}

/// Split a chord token into its root note and the rest. Accidentals belong to
/// the root, so `Bb7` roots on Bb and `C7b5` roots on C.
fn split_chord_root(token: &str) -> Option<(Note, String)> {
    let chars: Vec<char> = token.chars().collect();
    let first = *chars.first()?;
    if !first.is_ascii_alphabetic() || !"abcdefg".contains(first.to_ascii_lowercase()) {
        return None;
    }
    let mut end = 1;
    while end < chars.len() && matches!(chars[end], '#' | 'b' | '\u{266F}' | '\u{266D}' | 'x') {
        end += 1;
    }
    let root: String = chars[..end].iter().collect();
    let rest: String = chars[end..].iter().collect();
    parse_note(&root).ok().map(|n| (n, rest))
}

/// Parse a chord symbol such as `Cmaj7`, `F#m7b5`, `Bb13` or `Am7/G` into its
/// spelled chord tones. A bare root (`C`) is deliberately NOT a chord — it is
/// the note C, which is what a note-set search means by it.
///
/// Returns `Err(None)` when the token is not chord-shaped at all (so the caller
/// can fall back to reading it as a note) and `Err(Some(msg))` when it clearly
/// meant to be a chord but named an unsupported quality.
pub fn parse_chord(token: &str) -> Result<Vec<Note>, Option<String>> {
    let t = token.trim();
    let (root, rest) = split_chord_root(t).ok_or(None)?;
    // `Am7/G` — a slash bass. `6/9` was already folded to `69` by the
    // normaliser, so the only remaining slash is the bass separator.
    let (quality_raw, bass) = match rest.rsplit_once('/') {
        Some((q, b)) => match parse_note(b) {
            Ok(n) => (q.to_string(), Some(n)),
            Err(_) => (rest.clone(), None),
        },
        None => (rest.clone(), None),
    };
    let quality = normalize_quality(&quality_raw);
    if quality.is_empty() && bass.is_none() {
        return Err(None); // a bare note, not a chord
    }
    let def = CHORD_QUALITIES
        .iter()
        .find(|c| c.suffix == quality)
        .ok_or_else(|| {
            Some(format!(
                "unknown chord quality '{quality_raw}' in '{t}' — try a triad (C, Cm, Cdim, Caug, \
                 Csus4), a seventh (C7, Cmaj7, Cm7, Cm7b5, Cdim7) or an extension (C9, C11, C13, \
                 Cadd9, C7b9)"
            ))
        })?;
    let mut notes = spell_degrees(&root, def.degrees).ok_or_else(|| {
        Some(format!(
            "chord '{t}' cannot be spelled from that root — try its enharmonic root, for example \
             Db instead of C#"
        ))
    })?;
    if let Some(b) = bass {
        if !notes.iter().any(|n| n.pc() == b.pc()) {
            notes.push(b);
        }
    }
    Ok(notes)
}

/// One token of the `notes` field: a note (`C`, `F#`, `Bb`, `Eb3`) or a chord
/// symbol (`Cmaj7`, `Am7/G`). The chord reading wins when the suffix names a
/// real quality, so `C7` is a dominant seventh rather than "C in octave 7" —
/// the note C is written plainly as `C`.
fn parse_token(token: &str) -> Result<Vec<Note>, String> {
    match parse_chord(token) {
        Ok(notes) => Ok(notes),
        Err(chord_err) => parse_note(token)
            .map(|n| vec![n])
            .map_err(|note_err| chord_err.unwrap_or(note_err)),
    }
}

/// Split the `notes` field on commas, whitespace and the usual list separators,
/// then read each token as a note or a chord. `-` and `/` are NOT separators
/// (they belong to `C-7` and `Am7/G`), but a token that parses as neither is
/// retried split on them so `C-E-G` still works.
pub fn parse_notes(raw: &str) -> Result<Vec<Note>, String> {
    let tokens: Vec<&str> = raw
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | '|'))
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .collect();
    if tokens.len() > MAX_NOTES {
        return Err(format!(
            "too many notes: {} (limit {MAX_NOTES}); only 12 pitch classes exist",
            tokens.len()
        ));
    }
    let mut out = Vec::new();
    for token in tokens {
        match parse_token(token) {
            Ok(notes) => out.extend(notes),
            Err(e) => {
                // `C-E-G` / `C/E/G`: not one chord, but a dash- or slash-joined
                // list. Only accept the retry if every piece parses.
                let pieces: Vec<&str> = token
                    .split(['-', '/'])
                    .map(|p| p.trim())
                    .filter(|p| !p.is_empty())
                    .collect();
                let retry: Result<Vec<Vec<Note>>, String> = if pieces.len() > 1 {
                    pieces.iter().map(|p| parse_token(p)).collect()
                } else {
                    Err(e.clone())
                };
                match retry {
                    Ok(groups) => out.extend(groups.into_iter().flatten()),
                    Err(_) => return Err(e),
                }
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Spelling a scale from a root
// ---------------------------------------------------------------------------

fn semitone_of(deg: u8, alt: i8) -> i32 {
    (MAJOR_DEG[(deg - 1) as usize] + alt as i32).rem_euclid(12)
}

/// The pitch classes of `def` rooted on `root_pc`, ascending from the root.
fn pcs_of(def: &ScaleDef, root_pc: i32) -> Vec<i32> {
    def.degrees
        .iter()
        .map(|(d, a)| (root_pc + semitone_of(*d, *a)).rem_euclid(12))
        .collect()
}

fn pc_mask(pcs: &[i32]) -> u16 {
    pcs.iter().fold(0u16, |m, pc| m | 1 << pc)
}

/// Letter-aware spelling: degree `d` always takes the `d`-th letter above the
/// root, so a scale never spells two different degrees with the same letter
/// unless the degree spec itself repeats one (the blues b5/5 pair). Shared by
/// the scale catalogue and the chord-symbol parser, which use the same
/// `(degree, alteration)` spec — hence `Cmaj7` = `C E G B`, never `C E G Cb`.
fn spell_degrees(root: &Note, degrees: &[(u8, i8)]) -> Option<Vec<Note>> {
    let root_pc = root.pc();
    let mut out = Vec::with_capacity(degrees.len());
    for (d, a) in degrees {
        let letter = (root.letter + (*d as usize - 1)) % 7;
        let target = (root_pc + semitone_of(*d, *a)).rem_euclid(12);
        // Nearest representative of (target - natural) in -6..=5, so an
        // accidental is small rather than an 11-semitone detour.
        let alter = (target - LETTER_PC[letter] + 6).rem_euclid(12) - 6;
        if !(-2..=2).contains(&alter) {
            return None;
        }
        out.push(Note { letter, alter });
    }
    Some(out)
}

fn chromatic_names(pcs: &[i32], flats: bool) -> Vec<String> {
    let table = if flats { FLAT_NAMES } else { SHARP_NAMES };
    pcs.iter()
        .map(|pc| table[*pc as usize].to_string())
        .collect()
}

fn prefers_flats(root: &Note) -> bool {
    root.alter < 0 || (root.alter == 0 && root.letter == 3)
}

/// Spell the whole scale. `auto` uses the letter-aware spelling and only falls
/// back to plain sharps/flats when a root would need triple accidentals.
fn spell_scale(root: &Note, def: &ScaleDef, spelling: &str) -> Vec<String> {
    let pcs = pcs_of(def, root.pc());
    match spelling {
        "sharps" => chromatic_names(&pcs, false),
        "flats" => chromatic_names(&pcs, true),
        _ => match spell_degrees(root, def.degrees) {
            Some(notes) => notes.iter().map(|n| n.name()).collect(),
            None => chromatic_names(&pcs, prefers_flats(root)),
        },
    }
}

/// How the root itself is printed, honouring the accidental preference.
fn root_name(root: &Note, spelling: &str) -> String {
    match spelling {
        "sharps" => SHARP_NAMES[root.pc() as usize].to_string(),
        "flats" => FLAT_NAMES[root.pc() as usize].to_string(),
        _ => root.name(),
    }
}

fn note_from_name(name: &str) -> Note {
    parse_note(name).expect("built-in note table is valid")
}

// ---------------------------------------------------------------------------
// Chords
// ---------------------------------------------------------------------------

/// A diatonic chord built by stacking scale steps on one degree.
#[derive(Clone, Debug)]
pub struct Chord {
    pub degree: String,
    pub roman: String,
    pub symbol: String,
    pub notes: Vec<String>,
}

fn triad_quality(t3: i32, t5: i32) -> (String, bool, String) {
    match (t3, t5) {
        (4, 7) => (String::new(), true, String::new()),
        (3, 7) => ("m".into(), false, String::new()),
        (3, 6) => ("dim".into(), false, "°".into()),
        (4, 8) => ("aug".into(), true, "+".into()),
        (2, 7) => ("sus2".into(), true, "sus2".into()),
        (5, 7) => ("sus4".into(), true, "sus4".into()),
        (3, 8) => ("m#5".into(), false, "(#5)".into()),
        (4, 6) => ("b5".into(), true, "(b5)".into()),
        (2, 6) => ("sus2b5".into(), true, "sus2(b5)".into()),
        (5, 8) => ("sus4#5".into(), true, "sus4(#5)".into()),
        _ => {
            let s = format!("({t3}·{t5})");
            (s.clone(), t3 >= 4, s)
        }
    }
}

fn seventh_quality(t3: i32, t5: i32, t7: i32) -> String {
    match (t3, t5, t7) {
        (4, 7, 11) => "maj7".into(),
        (4, 7, 10) => "7".into(),
        (4, 7, 9) => "6".into(),
        (3, 7, 10) => "m7".into(),
        (3, 7, 11) => "mMaj7".into(),
        (3, 7, 9) => "m6".into(),
        (3, 6, 9) => "dim7".into(),
        (3, 6, 10) => "m7b5".into(),
        (4, 8, 11) => "maj7#5".into(),
        (4, 8, 10) => "7#5".into(),
        (2, 7, 10) => "7sus2".into(),
        (2, 7, 11) => "maj7sus2".into(),
        (5, 7, 10) => "7sus4".into(),
        (5, 7, 11) => "maj7sus4".into(),
        (4, 6, 10) => "7b5".into(),
        (4, 6, 11) => "maj7b5".into(),
        (3, 8, 10) => "m7#5".into(),
        (3, 8, 11) => "mMaj7#5".into(),
        _ => format!("({t3}·{t5}·{t7})"),
    }
}

fn accidental_prefix(alt: i8) -> String {
    accidental(alt as i32)
}

/// Stack thirds on every degree of a seven-note scale. Non-heptatonic scales
/// have no unambiguous "stack of thirds", so they return no chords at all.
pub fn diatonic_chords(root: &Note, def: &ScaleDef, spelling: &str, sevenths: bool) -> Vec<Chord> {
    if def.degrees.len() != 7 {
        return Vec::new();
    }
    let names = spell_scale(root, def, spelling);
    let pcs = pcs_of(def, root.pc());
    let mut out = Vec::with_capacity(7);
    for i in 0..7 {
        let r = pcs[i];
        let t3 = (pcs[(i + 2) % 7] - r).rem_euclid(12);
        let t5 = (pcs[(i + 4) % 7] - r).rem_euclid(12);
        let (suffix, upper, roman_suffix) = triad_quality(t3, t5);
        let (deg, alt) = def.degrees[i];
        let numeral = ROMAN[(deg - 1) as usize];
        let numeral = if upper {
            numeral.to_string()
        } else {
            numeral.to_lowercase()
        };
        let mut notes = vec![
            names[i].clone(),
            names[(i + 2) % 7].clone(),
            names[(i + 4) % 7].clone(),
        ];
        let symbol = if sevenths {
            let t7 = (pcs[(i + 6) % 7] - r).rem_euclid(12);
            notes.push(names[(i + 6) % 7].clone());
            format!("{}{}", names[i], seventh_quality(t3, t5, t7))
        } else {
            format!("{}{}", names[i], suffix)
        };
        out.push(Chord {
            degree: format!("{}{}", accidental_prefix(alt), deg),
            roman: format!("{}{}{}", accidental_prefix(alt), numeral, roman_suffix),
            symbol,
            notes,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Validation helpers
// ---------------------------------------------------------------------------

fn check_enum(name: &str, value: &str, allowed: &[&str]) -> Result<String, String> {
    let v = value.trim();
    if allowed.contains(&v) {
        return Ok(v.to_string());
    }
    let shown: Vec<&str> = allowed.iter().take(12).copied().collect();
    let more = if allowed.len() > shown.len() {
        format!(" (and {} more)", allowed.len() - shown.len())
    } else {
        String::new()
    };
    Err(format!(
        "unknown {name} '{v}' — expected one of: {}{more}",
        shown.join(", ")
    ))
}

fn scale_index(name: &str) -> Result<usize, String> {
    CATALOG
        .iter()
        .position(|d| d.name == name)
        .ok_or_else(|| check_enum("scale", name, &SCALES).unwrap_err())
}

// ---------------------------------------------------------------------------
// Text layout helper
// ---------------------------------------------------------------------------

/// Render label + cell rows as aligned columns so notes, degrees and chords
/// line up under each other.
fn aligned(rows: &[(&str, Vec<String>)]) -> String {
    let label_w = rows
        .iter()
        .map(|(l, _)| l.chars().count())
        .max()
        .unwrap_or(0);
    let cols = rows.iter().map(|(_, c)| c.len()).max().unwrap_or(0);
    let mut widths = vec![0usize; cols];
    for (_, cells) in rows {
        for (i, c) in cells.iter().enumerate() {
            widths[i] = widths[i].max(c.chars().count());
        }
    }
    let mut out = String::new();
    for (label, cells) in rows {
        let mut line = format!("{label:<label_w$}  ");
        for (i, c) in cells.iter().enumerate() {
            let w = widths[i];
            line.push_str(&format!("{c:<w$}"));
            if i + 1 < cells.len() {
                line.push(' ');
            }
        }
        out.push_str(line.trim_end());
        out.push('\n');
    }
    out
}

/// A twelve-slot chromatic map starting on the root: every scale tone under its
/// own name, every other semitone a dot. This is the text equivalent of the
/// piano-keyboard diagram the graphical scale finders draw — it shows at a
/// glance which of the twelve semitones the scale leaves out.
fn chromatic_map(root_pc: i32, pcs: &[i32], names: &[String]) -> Vec<String> {
    (0..12)
        .map(|i| {
            let pc = (root_pc + i).rem_euclid(12);
            match pcs.iter().position(|p| *p == pc) {
                Some(j) => names[j].clone(),
                None => "\u{00B7}".to_string(),
            }
        })
        .collect()
}

fn step_name(semitones: i32) -> String {
    match semitones {
        1 => "H".into(),
        2 => "W".into(),
        3 => "A2".into(),
        n => n.to_string(),
    }
}

fn jesc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn jarr(items: &[String]) -> String {
    let inner: Vec<String> = items.iter().map(|s| format!("\"{}\"", jesc(s))).collect();
    format!("[{}]", inner.join(","))
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// list — notes and chords of one scale
// ---------------------------------------------------------------------------

/// Every other (root, scale) pair in the catalogue whose pitch-class set is the
/// same — the modes of the same parent scale, plus any coincidental twins.
fn same_note_modes(mask: u16, root_pc: i32, idx: usize, spelling: &str) -> Vec<String> {
    let mut out = Vec::new();
    for pc in 0..12 {
        for (i, def) in CATALOG.iter().enumerate() {
            if i == idx && pc == root_pc {
                continue;
            }
            if def.degrees.len() != CATALOG[idx].degrees.len() {
                continue;
            }
            if pc_mask(&pcs_of(def, pc)) == mask {
                let r = note_from_name(DEFAULT_ROOT_NAMES[pc as usize]);
                out.push(format!("{} {}", root_name(&r, spelling), def.name));
            }
        }
    }
    out.truncate(12);
    out
}

fn run_list(o: &Options) -> Result<String, String> {
    let key = check_enum("key", &o.key, &KEYS)?;
    let scale = check_enum("scale", &o.scale, &SCALES)?;
    let spelling = check_enum("spelling", &o.spelling, &SPELLINGS)?;
    let chord_type = check_enum("chord_type", &o.chord_type, &CHORD_TYPES)?;
    let output = check_enum("output", &o.output, &OUTPUTS)?;

    let idx = scale_index(&scale)?;
    let def = &CATALOG[idx];
    let root = note_from_name(&key);
    let names = spell_scale(&root, def, &spelling);
    let pcs = pcs_of(def, root.pc());
    let semis: Vec<i32> = def
        .degrees
        .iter()
        .map(|(d, a)| semitone_of(*d, *a))
        .collect();
    let degrees: Vec<String> = def
        .degrees
        .iter()
        .map(|(d, a)| format!("{}{}", accidental_prefix(*a), d))
        .collect();
    let mut steps: Vec<String> = Vec::with_capacity(semis.len());
    for i in 0..semis.len() {
        let next = if i + 1 < semis.len() {
            semis[i + 1]
        } else {
            12
        };
        steps.push(step_name(next - semis[i]));
    }

    let want_triads = matches!(chord_type.as_str(), "triads" | "both");
    let want_sevenths = matches!(chord_type.as_str(), "sevenths" | "both");
    let triads = if o.include_chords && want_triads {
        diatonic_chords(&root, def, &spelling, false)
    } else {
        Vec::new()
    };
    let sevenths = if o.include_chords && want_sevenths {
        diatonic_chords(&root, def, &spelling, true)
    } else {
        Vec::new()
    };
    let modes = if o.include_modes {
        same_note_modes(pc_mask(&pcs), root.pc(), idx, &spelling)
    } else {
        Vec::new()
    };
    let printed_root = root_name(&root, &spelling);

    match output.as_str() {
        "names" => Ok(names.join(" ")),
        "csv" => {
            let mut head = vec!["degree", "note", "semitones"];
            if !triads.is_empty() {
                head.push("triad");
            }
            if !sevenths.is_empty() {
                head.push("seventh");
            }
            let mut out = head.join(",");
            out.push('\n');
            for i in 0..names.len() {
                let mut row = vec![
                    csv_cell(&degrees[i]),
                    csv_cell(&names[i]),
                    semis[i].to_string(),
                ];
                if !triads.is_empty() {
                    row.push(csv_cell(&triads[i].symbol));
                }
                if !sevenths.is_empty() {
                    row.push(csv_cell(&sevenths[i].symbol));
                }
                out.push_str(&row.join(","));
                out.push('\n');
            }
            Ok(out)
        }
        "json" => {
            let mut s = String::from("{");
            s.push_str(&format!(
                "\"action\":\"list\",\"root\":\"{}\",",
                jesc(&printed_root)
            ));
            s.push_str(&format!(
                "\"scale\":\"{}\",\"label\":\"{}\",",
                jesc(def.name),
                jesc(def.label)
            ));
            s.push_str(&format!("\"notes\":{},", jarr(&names)));
            s.push_str(&format!("\"degrees\":{},", jarr(&degrees)));
            s.push_str(&format!(
                "\"semitones\":[{}],",
                semis
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(",")
            ));
            s.push_str(&format!("\"steps\":{},", jarr(&steps)));
            s.push_str(&format!(
                "\"chromatic\":{},",
                jarr(&chromatic_map(root.pc(), &pcs, &names))
            ));
            let chord_json = |cs: &[Chord]| -> String {
                let items: Vec<String> = cs
                    .iter()
                    .map(|c| {
                        format!(
                            "{{\"degree\":\"{}\",\"roman\":\"{}\",\"symbol\":\"{}\",\"notes\":{}}}",
                            jesc(&c.degree),
                            jesc(&c.roman),
                            jesc(&c.symbol),
                            jarr(&c.notes)
                        )
                    })
                    .collect();
                format!("[{}]", items.join(","))
            };
            s.push_str(&format!("\"triads\":{},", chord_json(&triads)));
            s.push_str(&format!("\"sevenths\":{},", chord_json(&sevenths)));
            s.push_str(&format!("\"modes\":{}", jarr(&modes)));
            s.push('}');
            Ok(s)
        }
        _ => {
            let mut out = format!("Scale: {printed_root} {} ({})\n", def.name, def.label);
            let mut rows: Vec<(&str, Vec<String>)> = vec![
                ("Notes:", names.clone()),
                ("Degrees:", degrees.clone()),
                (
                    "Semitones:",
                    semis.iter().map(|n| n.to_string()).collect::<Vec<_>>(),
                ),
                ("Steps:", steps.clone()),
            ];
            if !triads.is_empty() {
                rows.push(("Triads:", triads.iter().map(|c| c.symbol.clone()).collect()));
                rows.push(("Roman:", triads.iter().map(|c| c.roman.clone()).collect()));
            }
            if !sevenths.is_empty() {
                rows.push((
                    "Sevenths:",
                    sevenths.iter().map(|c| c.symbol.clone()).collect(),
                ));
                if triads.is_empty() {
                    rows.push(("Roman:", sevenths.iter().map(|c| c.roman.clone()).collect()));
                }
            }
            out.push_str(&aligned(&rows));
            let map: Vec<String> = chromatic_map(root.pc(), &pcs, &names)
                .iter()
                .map(|c| format!("{c:<2}"))
                .collect();
            out.push_str(&format!(
                "\nChromatic from {printed_root}: {}\n",
                map.join(" ").trim_end()
            ));
            if o.include_chords && def.degrees.len() != 7 {
                out.push_str(
                    "\nDiatonic chords are listed for seven-note scales only; this scale has ",
                );
                out.push_str(&format!("{} notes.\n", def.degrees.len()));
            }
            if o.include_modes {
                out.push('\n');
                if modes.is_empty() {
                    out.push_str("Same notes as: no other scale in the catalogue.\n");
                } else {
                    out.push_str(&format!("Same notes as: {}\n", modes.join(", ")));
                }
                if let Some(line) = related_keys(&root, def, &spelling) {
                    out.push_str(&line);
                }
            }
            Ok(out)
        }
    }
}

/// Relative and parallel keys, for the two scales where those terms are used.
fn related_keys(root: &Note, def: &ScaleDef, spelling: &str) -> Option<String> {
    let printed = root_name(root, spelling);
    match def.name {
        "major" => {
            let rel = spell_scale(root, def, spelling)[5].clone();
            Some(format!(
                "Relative minor: {rel} minor · Parallel key: {printed} minor\n"
            ))
        }
        "minor" => {
            let rel = spell_scale(root, def, spelling)[2].clone();
            Some(format!(
                "Relative major: {rel} major · Parallel key: {printed} major\n"
            ))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// find — scales containing a note set
// ---------------------------------------------------------------------------

struct Match {
    root: Note,
    idx: usize,
    names: Vec<String>,
    mask: u16,
    missing: Vec<String>,
    extra: Vec<String>,
    root_bonus: u8,
}

/// Root spellings to try for a pitch class: the user's own spelling of that
/// note first (so a search for `Bb` reports Bb scales, not A# ones), then the
/// common key name, then its enharmonic twin.
fn root_candidates(pc: i32, input: &[Note]) -> Vec<Note> {
    let mut out: Vec<Note> = Vec::new();
    let mut push = |n: Note| {
        if !out.contains(&n) {
            out.push(n);
        }
    };
    for n in input {
        if n.pc() == pc {
            push(*n);
        }
    }
    push(note_from_name(DEFAULT_ROOT_NAMES[pc as usize]));
    push(note_from_name(SHARP_NAMES[pc as usize]));
    push(note_from_name(FLAT_NAMES[pc as usize]));
    out
}

fn run_find(o: &Options) -> Result<String, String> {
    let fit = check_enum("fit", &o.fit, &FITS)?;
    let spelling = check_enum("spelling", &o.spelling, &SPELLINGS)?;
    let chord_type = check_enum("chord_type", &o.chord_type, &CHORD_TYPES)?;
    let output = check_enum("output", &o.output, &OUTPUTS)?;
    let root_filter = check_enum("root", &o.root, &ROOTS)?;
    // `any` searches all twelve tonics; a named root keeps only the scales whose
    // tonic is the note the player already hears as home.
    let want_root: Option<Note> = match root_filter.as_str() {
        "any" => None,
        name => Some(note_from_name(name)),
    };
    if !(1..=MAX_RESULTS).contains(&o.max_results) {
        return Err(format!(
            "max_results must be between 1 and {MAX_RESULTS}, got {}",
            o.max_results
        ));
    }
    let input = parse_notes(&o.notes)?;
    if input.is_empty() {
        return Err(
            "notes is empty — list the notes to search for, for example notes=\"C E G B\"".into(),
        );
    }
    // Deduplicate by pitch class, keeping the first spelling the user gave.
    let mut wanted: Vec<Note> = Vec::new();
    for n in &input {
        if !wanted.iter().any(|w| w.pc() == n.pc()) {
            wanted.push(*n);
        }
    }
    let want_mask = pc_mask(&wanted.iter().map(|n| n.pc()).collect::<Vec<_>>());
    let first_pc = wanted[0].pc();

    let mut matches: Vec<Match> = Vec::new();
    for pc in 0..12 {
        if want_root.is_some_and(|r| r.pc() != pc) {
            continue;
        }
        for (idx, def) in CATALOG.iter().enumerate() {
            // The chromatic scale contains every note, so it is only a useful
            // answer when the search asks for an exact set match.
            if def.name == "chromatic" && fit != "exact" {
                continue;
            }
            let pcs = pcs_of(def, pc);
            let mask = pc_mask(&pcs);
            let missing_mask = want_mask & !mask;
            let missing_count = missing_mask.count_ones();
            let keep = match fit.as_str() {
                "exact" => mask == want_mask,
                "near" => missing_count <= 1,
                _ => missing_count == 0,
            };
            if !keep {
                continue;
            }
            // An explicitly requested root is also an explicit SPELLING of it:
            // asking for Eb should report Eb scales, not their D# twins.
            let mut candidates = root_candidates(pc, &wanted);
            if let Some(r) = want_root {
                candidates.retain(|c| *c != r);
                candidates.insert(0, r);
            }
            let root = candidates
                .into_iter()
                .find(|c| spell_degrees(c, def.degrees).is_some())
                .unwrap_or_else(|| note_from_name(DEFAULT_ROOT_NAMES[pc as usize]));
            let names = spell_scale(&root, def, &spelling);
            let missing: Vec<String> = wanted
                .iter()
                .filter(|n| missing_mask & (1 << n.pc()) != 0)
                .map(|n| n.name())
                .collect();
            let extra: Vec<String> = names
                .iter()
                .zip(pcs.iter())
                .filter(|(_, pc)| want_mask & (1 << **pc) == 0)
                .map(|(n, _)| n.clone())
                .collect();
            matches.push(Match {
                root,
                idx,
                names,
                mask,
                missing,
                extra,
                root_bonus: u8::from(pc != first_pc),
            });
        }
    }
    // Tightest fit first: fewest missing notes, then fewest extra notes, then
    // the scale rooted on the first note given, then catalogue order.
    matches.sort_by_key(|m| {
        (
            m.missing.len(),
            m.extra.len(),
            m.root_bonus,
            m.idx,
            m.root.pc(),
        )
    });
    if !o.include_modes {
        let mut seen: Vec<u16> = Vec::new();
        matches.retain(|m| {
            if seen.contains(&m.mask) {
                false
            } else {
                seen.push(m.mask);
                true
            }
        });
    }
    let total = matches.len();
    matches.truncate(o.max_results as usize);

    let want_triads = matches!(chord_type.as_str(), "triads" | "both");
    let want_sevenths = matches!(chord_type.as_str(), "sevenths" | "both");
    let chords_for = |m: &Match, sevenths: bool| -> Vec<String> {
        if !o.include_chords {
            return Vec::new();
        }
        diatonic_chords(&m.root, &CATALOG[m.idx], &spelling, sevenths)
            .iter()
            .map(|c| c.symbol.clone())
            .collect()
    };
    let title = |m: &Match| format!("{} {}", root_name(&m.root, &spelling), CATALOG[m.idx].name);
    let searched = if fit == "exact" {
        SCALE_COUNT
    } else {
        SCALE_COUNT - 1
    };
    let roots_searched = if want_root.is_some() { 1 } else { 12 };
    // How tightly each result fits, in the three words the graphical finders
    // use: the same note set, a superset of it, or one note short.
    let strength = |m: &Match| -> &'static str {
        if !m.missing.is_empty() {
            "near"
        } else if m.extra.is_empty() {
            "exact"
        } else {
            "contains"
        }
    };

    match output.as_str() {
        "names" => Ok(matches.iter().map(title).collect::<Vec<_>>().join("\n")),
        "csv" => {
            let mut head = vec![
                "rank", "root", "scale", "match", "notes", "missing", "extra",
            ];
            if o.include_chords && want_triads {
                head.push("triads");
            }
            if o.include_chords && want_sevenths {
                head.push("sevenths");
            }
            let mut out = head.join(",");
            out.push('\n');
            for (i, m) in matches.iter().enumerate() {
                let mut row = vec![
                    (i + 1).to_string(),
                    csv_cell(&root_name(&m.root, &spelling)),
                    csv_cell(CATALOG[m.idx].name),
                    strength(m).to_string(),
                    csv_cell(&m.names.join(" ")),
                    csv_cell(&m.missing.join(" ")),
                    csv_cell(&m.extra.join(" ")),
                ];
                if o.include_chords && want_triads {
                    row.push(csv_cell(&chords_for(m, false).join(" ")));
                }
                if o.include_chords && want_sevenths {
                    row.push(csv_cell(&chords_for(m, true).join(" ")));
                }
                out.push_str(&row.join(","));
                out.push('\n');
            }
            Ok(out)
        }
        "json" => {
            let mut items: Vec<String> = Vec::new();
            for m in &matches {
                let mut s = format!(
                    "{{\"root\":\"{}\",\"scale\":\"{}\",\"label\":\"{}\",\"match\":\"{}\",\"notes\":{}",
                    jesc(&root_name(&m.root, &spelling)),
                    jesc(CATALOG[m.idx].name),
                    jesc(CATALOG[m.idx].label),
                    strength(m),
                    jarr(&m.names)
                );
                s.push_str(&format!(",\"missing\":{}", jarr(&m.missing)));
                s.push_str(&format!(",\"extra\":{}", jarr(&m.extra)));
                if o.include_chords && want_triads {
                    s.push_str(&format!(",\"triads\":{}", jarr(&chords_for(m, false))));
                }
                if o.include_chords && want_sevenths {
                    s.push_str(&format!(",\"sevenths\":{}", jarr(&chords_for(m, true))));
                }
                s.push('}');
                items.push(s);
            }
            Ok(format!(
                "{{\"action\":\"find\",\"notes\":{},\"pitch_classes\":{},\"fit\":\"{}\",\"searched\":{},\"total_matches\":{},\"results\":[{}]}}",
                jarr(&wanted.iter().map(|n| n.name()).collect::<Vec<_>>()),
                wanted.len(),
                jesc(&fit),
                searched * roots_searched,
                total,
                items.join(",")
            ))
        }
        _ => {
            let mut out = format!(
                "Notes: {} ({} pitch {})\n",
                wanted
                    .iter()
                    .map(|n| n.name())
                    .collect::<Vec<_>>()
                    .join(" "),
                wanted.len(),
                if wanted.len() == 1 {
                    "class"
                } else {
                    "classes"
                }
            );
            out.push_str(&format!(
                "Searched {searched} scale types across 12 roots · fit={fit} · {total} {} · showing {}\n",
                if total == 1 { "match" } else { "matches" },
                matches.len()
            ));
            if matches.is_empty() {
                out.push_str("\nNo scale in the catalogue fits those notes. Try fit=near to allow one missing note, or remove a chromatic passing tone.\n");
                return Ok(out);
            }
            for (i, m) in matches.iter().enumerate() {
                out.push_str(&format!(
                    "\n{:>2}. {} — {}\n",
                    i + 1,
                    title(m),
                    m.names.join(" ")
                ));
                out.push_str(&format!("    {}\n", CATALOG[m.idx].label));
                if !m.missing.is_empty() {
                    out.push_str(&format!("    missing: {}\n", m.missing.join(" ")));
                }
                out.push_str(&format!(
                    "    extra: {}\n",
                    if m.extra.is_empty() {
                        "none — an exact match".to_string()
                    } else {
                        m.extra.join(" ")
                    }
                ));
                if o.include_chords && want_triads {
                    let c = chords_for(m, false);
                    if !c.is_empty() {
                        out.push_str(&format!("    triads: {}\n", c.join(" ")));
                    }
                }
                if o.include_chords && want_sevenths {
                    let c = chords_for(m, true);
                    if !c.is_empty() {
                        out.push_str(&format!("    sevenths: {}\n", c.join(" ")));
                    }
                }
            }
            Ok(out)
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(o: &Options) -> Result<String, String> {
    let action = check_enum("action", &o.action, &ACTIONS)?;
    let resolved = if action == "auto" {
        if o.notes.trim().is_empty() {
            "list"
        } else {
            "find"
        }
    } else {
        action.as_str()
    };
    match resolved {
        "find" => run_find(o),
        _ => run_list(o),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(f: impl FnOnce(&mut Options)) -> Options {
        let mut o = Options::default();
        f(&mut o);
        o
    }

    #[test]
    fn happy_find_lists_scales_containing_every_note() {
        let o = opts(|o| {
            o.notes = "C E G B".into();
            o.max_results = 5;
        });
        let out = run(&o).unwrap();
        assert!(
            out.starts_with("Notes: C E G B (4 pitch classes)\n"),
            "{out}"
        );
        assert!(
            out.contains("Searched 41 scale types across 12 roots"),
            "{out}"
        );
        assert!(
            !out.contains("missing:"),
            "contains fit should not miss input notes: {out}"
        );
        assert!(out.contains("E hirajoshi"), "{out}");
    }

    #[test]
    fn happy_list_spells_the_scale_and_its_chords() {
        let o = opts(|o| {
            o.action = "list".into();
            o.key = "G".into();
            o.scale = "lydian".into();
        });
        let out = run(&o).unwrap();
        assert!(out.starts_with("Scale: G lydian (Lydian)\n"), "{out}");
        assert!(out.contains("Notes:      G A  B   C#    D E  F#"), "{out}");
        assert!(out.contains("Degrees:    1 2  3   #4    5 6  7"), "{out}");
        assert!(out.contains("Triads:     G A  Bm  C#dim D Em F#m"), "{out}");
        assert!(out.contains("Roman:      I II iii #iv°  V vi vii"), "{out}");
        assert!(out.contains("D major"), "{out}");
    }

    #[test]
    fn list_names_output_is_just_the_notes() {
        let o = opts(|o| {
            o.action = "list".into();
            o.key = "Eb".into();
            o.scale = "minor".into();
            o.output = "names".into();
        });
        assert_eq!(run(&o).unwrap(), "Eb F Gb Ab Bb Cb Db");
    }

    #[test]
    fn sevenths_and_both_change_the_chord_column() {
        let _o = opts(|o| {
            o.action = "list".into();
            o.chord_type = "sevenths".into();
            o.output = "names".into();
        });
        // names output ignores chords; check the csv header instead.
        let o = opts(|o| {
            o.action = "list".into();
            o.chord_type = "both".into();
            o.output = "csv".into();
        });
        let out = run(&o).unwrap();
        assert!(
            out.starts_with("degree,note,semitones,triad,seventh\n"),
            "{out}"
        );
        assert!(out.contains("\n1,C,0,C,Cmaj7\n"), "{out}");
        assert!(out.contains("\n7,B,11,Bdim,Bm7b5\n"), "{out}");
    }

    #[test]
    fn include_chords_false_drops_the_chord_rows() {
        let o = opts(|o| {
            o.action = "list".into();
            o.include_chords = false;
        });
        let out = run(&o).unwrap();
        assert!(!out.contains("Triads:"), "{out}");
        assert!(out.contains("Notes:"), "{out}");
    }

    #[test]
    fn pentatonic_scales_report_no_diatonic_chords() {
        let o = opts(|o| {
            o.action = "list".into();
            o.scale = "minor-pentatonic".into();
        });
        let out = run(&o).unwrap();
        assert!(out.contains("Notes:      C  Eb F G  Bb"), "{out}");
        assert!(!out.contains("Triads:"), "{out}");
        assert!(
            out.contains("seven-note scales only; this scale has 5 notes"),
            "{out}"
        );
    }

    #[test]
    fn invalid_note_is_rejected_with_an_actionable_message() {
        let o = opts(|o| o.notes = "C E H".into());
        let err = run(&o).unwrap_err();
        assert!(err.contains("unknown note 'H'"), "{err}");
        assert!(err.contains("letter A-G"), "{err}");
    }

    #[test]
    fn other_invalid_input_is_rejected() {
        let o = opts(|o| o.key = "Q".into());
        assert!(run(&o).unwrap_err().contains("unknown key 'Q'"));
        let o = opts(|o| o.scale = "bagpipe".into());
        assert!(run(&o).unwrap_err().contains("unknown scale 'bagpipe'"));
        let o = opts(|o| {
            o.action = "find".into();
            o.notes = "  ".into();
        });
        assert!(run(&o).unwrap_err().contains("notes is empty"));
        let o = opts(|o| {
            o.notes = "C E G".into();
            o.max_results = 0;
        });
        assert!(run(&o)
            .unwrap_err()
            .contains("max_results must be between 1 and 50"));
        let o = opts(|o| o.notes = "C ".repeat(25));
        assert!(run(&o).unwrap_err().contains("too many notes: 25"));
        let o = opts(|o| o.notes = "Cbbb".into());
        assert!(run(&o).unwrap_err().contains("too many accidentals"));
    }

    #[test]
    fn auto_action_switches_on_whether_notes_were_given() {
        let empty = run(&Options::default()).unwrap();
        assert!(empty.starts_with("Scale: C major"), "{empty}");
        let filled = run(&opts(|o| o.notes = "C D E".into())).unwrap();
        assert!(filled.starts_with("Notes: C D E"), "{filled}");
        // An explicit action wins over the heuristic.
        let forced = run(&opts(|o| {
            o.action = "list".into();
            o.notes = "C D E".into();
        }))
        .unwrap();
        assert!(forced.starts_with("Scale: C major"), "{forced}");
    }

    #[test]
    fn input_is_case_and_separator_insensitive_and_deduplicates() {
        let a = run(&opts(|o| {
            o.notes = "c,e,g".into();
            o.max_results = 3;
        }))
        .unwrap();
        let b = run(&opts(|o| {
            o.notes = "C  E\nG".into();
            o.max_results = 3;
        }))
        .unwrap();
        let c = run(&opts(|o| {
            o.notes = "C4 E4 G4 C5".into();
            o.max_results = 3;
        }))
        .unwrap();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert!(a.starts_with("Notes: C E G (3 pitch classes)"), "{a}");
    }

    #[test]
    fn results_are_deterministic_across_runs() {
        let o = opts(|o| {
            o.notes = "F A C E".into();
            o.max_results = 8;
        });
        assert_eq!(run(&o).unwrap(), run(&o).unwrap());
    }

    #[test]
    fn enharmonic_input_keeps_the_users_spelling_but_matches_the_same_scales() {
        let flat = run(&opts(|o| {
            o.notes = "Eb G Bb".into();
            o.max_results = 4;
            o.output = "names".into();
        }))
        .unwrap();
        let sharp = run(&opts(|o| {
            o.notes = "D# G A#".into();
            o.max_results = 4;
            o.output = "names".into();
        }))
        .unwrap();
        assert!(flat.starts_with("Eb "), "{flat}");
        assert!(sharp.starts_with("D# "), "{sharp}");
        // Same pitch classes, so the same scale types come back in the same order.
        let names = |s: &str| {
            s.lines()
                .map(|l| l.split_once(' ').unwrap().1.to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&flat), names(&sharp));
    }

    #[test]
    fn spelling_override_forces_sharps_or_flats() {
        let sharps = run(&opts(|o| {
            o.action = "list".into();
            o.key = "Db".into();
            o.spelling = "sharps".into();
            o.output = "names".into();
        }))
        .unwrap();
        assert_eq!(sharps, "C# D# F F# G# A# C");
        let flats = run(&opts(|o| {
            o.action = "list".into();
            o.key = "Db".into();
            o.spelling = "flats".into();
            o.output = "names".into();
        }))
        .unwrap();
        assert_eq!(flats, "Db Eb F Gb Ab Bb C");
    }

    #[test]
    fn fit_exact_and_near_change_the_result_set() {
        let exact = run(&opts(|o| {
            o.notes = "C D E G A".into();
            o.fit = "exact".into();
            o.output = "names".into();
            o.max_results = 20;
        }))
        .unwrap();
        assert!(exact.lines().any(|l| l == "C major-pentatonic"), "{exact}");
        assert!(exact.lines().all(|l| l.ends_with("pentatonic")), "{exact}");

        // A note set no five- or seven-note scale holds still finds near matches.
        let strict = run(&opts(|o| {
            o.notes = "C Db D Eb E".into();
            o.output = "names".into();
        }))
        .unwrap();
        assert_eq!(strict.lines().count(), 0, "{strict}");
        let near = run(&opts(|o| {
            o.notes = "C Db D Eb E".into();
            o.fit = "near".into();
            o.output = "names".into();
        }))
        .unwrap();
        assert!(near.lines().count() > 0, "{near}");
    }

    #[test]
    fn include_modes_false_collapses_rotations_of_the_same_note_set() {
        let all = run(&opts(|o| {
            o.notes = "C E G".into();
            o.output = "names".into();
            o.max_results = 7;
        }))
        .unwrap();
        let collapsed = run(&opts(|o| {
            o.notes = "C E G".into();
            o.include_modes = false;
            o.output = "names".into();
            o.max_results = 7;
        }))
        .unwrap();
        assert!(all.lines().count() >= collapsed.lines().count());
        // At least one same-note-set rotation survives the collapse.
        assert!(all.lines().count() == 7);
        let has = |s: &str, want: &str| s.lines().any(|l| l == want);
        assert!(has(&all, "C major-pentatonic"), "{all}");
        assert!(collapsed.lines().count() <= all.lines().count());
    }

    #[test]
    fn find_json_output_is_parseable_and_reports_the_totals() {
        let out = run(&opts(|o| {
            o.notes = "C E G B".into();
            o.output = "json".into();
            o.max_results = 2;
        }))
        .unwrap();
        assert!(out.starts_with("{\"action\":\"find\",\"notes\":[\"C\",\"E\",\"G\",\"B\"],\"pitch_classes\":4,\"fit\":\"contains\",\"searched\":492,"), "{out}");
        assert_eq!(out.matches("\"root\":").count(), 2, "{out}");
        assert!(out.contains("\"triads\":["), "{out}");
    }

    #[test]
    fn find_csv_output_has_a_header_and_one_row_per_match() {
        let out = run(&opts(|o| {
            o.notes = "C E G".into();
            o.output = "csv".into();
            o.max_results = 3;
        }))
        .unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "rank,root,scale,match,notes,missing,extra,triads");
        assert_eq!(lines.len(), 4, "{out}");
        assert!(lines[1].starts_with("1,C,"), "{out}");
    }

    #[test]
    fn every_catalogue_scale_spells_and_runs_from_every_offered_key() {
        for key in KEYS {
            for scale in SCALES {
                let o = opts(|o| {
                    o.action = "list".into();
                    o.key = key.into();
                    o.scale = scale.into();
                    o.chord_type = "both".into();
                });
                let out = run(&o).unwrap_or_else(|e| panic!("{key} {scale}: {e}"));
                assert!(out.contains("Notes:"), "{key} {scale}");
            }
        }
    }

    #[test]
    fn degree_specs_produce_the_expected_semitone_patterns() {
        let pat = |name: &str| {
            let d = &CATALOG[scale_index(name).unwrap()];
            d.degrees
                .iter()
                .map(|(deg, a)| semitone_of(*deg, *a))
                .collect::<Vec<_>>()
        };
        assert_eq!(pat("major"), vec![0, 2, 4, 5, 7, 9, 11]);
        assert_eq!(pat("minor"), vec![0, 2, 3, 5, 7, 8, 10]);
        assert_eq!(pat("harmonic-minor"), vec![0, 2, 3, 5, 7, 8, 11]);
        assert_eq!(pat("melodic-minor"), vec![0, 2, 3, 5, 7, 9, 11]);
        assert_eq!(pat("blues"), vec![0, 3, 5, 6, 7, 10]);
        assert_eq!(pat("altered"), vec![0, 1, 3, 4, 6, 8, 10]);
        assert_eq!(pat("whole-tone"), vec![0, 2, 4, 6, 8, 10]);
        assert_eq!(pat("diminished-half-whole"), vec![0, 1, 3, 4, 6, 7, 9, 10]);
        assert_eq!(pat("chromatic"), (0..12).collect::<Vec<_>>());
        // Every scale is strictly ascending and has no duplicate pitch class.
        for def in CATALOG.iter() {
            let p: Vec<i32> = def
                .degrees
                .iter()
                .map(|(d, a)| semitone_of(*d, *a))
                .collect();
            assert!(
                p.windows(2).all(|w| w[0] < w[1]),
                "{} is not ascending: {p:?}",
                def.name
            );
        }
    }

    #[test]
    fn scale_names_matches_the_catalogue() {
        assert_eq!(SCALES.len(), CATALOG.len());
        for (i, def) in CATALOG.iter().enumerate() {
            assert_eq!(SCALES[i], def.name);
        }
    }

    #[test]
    fn page_field_helpers_fall_back_to_defaults() {
        assert_eq!(parse_field("max_results", "", 12i32).unwrap(), 12);
        assert_eq!(parse_field("max_results", " 7 ", 12i32).unwrap(), 7);
        assert!(parse_field("max_results", "x", 12i32)
            .unwrap_err()
            .contains("max_results must be a number"));
        assert!(truthy("", true));
        assert!(!truthy("", false));
        assert!(truthy("true", false));
        assert!(!truthy("false", true));
    }
}
