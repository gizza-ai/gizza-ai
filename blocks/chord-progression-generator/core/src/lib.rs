//! chord-progression-generator core — deterministic chord-progression generation
//! in any key/mode/style, with Roman-numeral analysis, spelled chord tones and a
//! Standard MIDI File rendered from the result.
//!
//! Nothing here is random: the same `Options` always produce the same
//! progression and the same MIDI bytes, so a result can be shared as a URL and
//! reproduced exactly. The MIDI writer is reused from the sibling
//! `midi-chord-progression-generator` core, so both tools voice and time chords
//! the same way.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_midi_chord_progression_generator_core as smf;

// ---------------------------------------------------------------------------
// Vocabularies (also used by the descriptor so the enums can't drift)
// ---------------------------------------------------------------------------

/// Tonic spellings offered, sharp and flat versions of every black key.
pub const KEYS: [&str; 17] = [
    "C", "C#", "Db", "D", "D#", "Eb", "E", "F", "F#", "Gb", "G", "G#", "Ab", "A", "A#", "Bb", "B",
];

/// Scale/mode names offered.
pub const MODES: [&str; 9] = [
    "major",
    "minor",
    "dorian",
    "phrygian",
    "lydian",
    "mixolydian",
    "locrian",
    "harmonic-minor",
    "melodic-minor",
];

/// Style presets; `random` generates a fresh in-key progression instead.
pub const STYLES: [&str; 16] = [
    "pop",
    "rock",
    "folk",
    "country",
    "ballad",
    "worship",
    "edm",
    "hip-hop",
    "lofi",
    "rnb",
    "jazz",
    "blues",
    "reggae",
    "metal",
    "cinematic",
    "random",
];

/// Chord-thickness choices.
pub const SEVENTHS: [&str; 4] = ["auto", "triads", "sevenths", "extended"];

/// Modal-interchange levels.
pub const BORROWED: [&str; 3] = ["none", "light", "rich"];

/// Playback patterns written into the MIDI file.
pub const PATTERNS: [&str; 5] = [
    "block",
    "arpeggio-up",
    "arpeggio-down",
    "arpeggio-updown",
    "strum",
];

/// General MIDI instruments offered (same set the MIDI writer knows).
pub const INSTRUMENTS: [&str; 16] = [
    "acoustic-grand-piano",
    "bright-acoustic-piano",
    "electric-piano",
    "harpsichord",
    "vibraphone",
    "drawbar-organ",
    "church-organ",
    "accordion",
    "acoustic-guitar-nylon",
    "acoustic-guitar-steel",
    "electric-guitar-clean",
    "acoustic-bass",
    "string-ensemble",
    "choir-aahs",
    "synth-pad-warm",
    "synth-lead-square",
];

/// Text shapes the result can be rendered as.
pub const OUTPUTS: [&str; 5] = ["text", "chords", "roman", "csv", "midi-base64"];

/// Largest `chords` value (0 means "the style's own length").
pub const MAX_CHORDS: i32 = 32;
/// Largest `repeats` value.
pub const MAX_REPEATS: i32 = 8;
/// Largest `variation` value.
pub const MAX_VARIATION: i32 = 99;
/// Beats each chord lasts; one bar of 4/4.
pub const BEATS_PER_CHORD: f64 = 4.0;
/// File name used for the generated Standard MIDI File.
pub const MIDI_FILENAME: &str = "generated-chord-progression.mid";
/// MIME type of the generated Standard MIDI File.
pub const MIDI_MIME: &str = "audio/midi";

const LETTERS: [char; 7] = ['C', 'D', 'E', 'F', 'G', 'A', 'B'];
const LETTER_PC: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const MAJOR: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const UPPER: [&str; 7] = ["I", "II", "III", "IV", "V", "VI", "VII"];
const LOWER: [&str; 7] = ["i", "ii", "iii", "iv", "v", "vi", "vii"];
const SHARP_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
const FLAT_NAMES: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B",
];

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Everything the generator needs. Field names match the tool's parameters.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Tonic note, one of [`KEYS`].
    pub key: String,
    /// Scale/mode, one of [`MODES`].
    pub mode: String,
    /// Style preset, one of [`STYLES`].
    pub style: String,
    /// Which progression of the style to use, 1..=[`MAX_VARIATION`].
    pub variation: i32,
    /// Chord thickness, one of [`SEVENTHS`].
    pub sevenths: String,
    /// Modal-interchange level, one of [`BORROWED`].
    pub borrowed: String,
    /// Number of chords, or 0 for the style's natural length.
    pub chords: i32,
    /// Tempo in BPM written into the MIDI file.
    pub tempo: f64,
    /// General MIDI instrument, one of [`INSTRUMENTS`].
    pub instrument: String,
    /// Playback pattern, one of [`PATTERNS`].
    pub pattern: String,
    /// Voice-lead by picking the nearest inversion to the previous chord.
    pub voice_leading: bool,
    /// How many times the progression is repeated in the MIDI file.
    pub repeats: i32,
    /// Octave of the tonic; 4 puts middle C at MIDI note 60.
    pub octave: i32,
    /// Text shape of the result, one of [`OUTPUTS`].
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            key: "C".into(),
            mode: "major".into(),
            style: "pop".into(),
            variation: 1,
            sevenths: "auto".into(),
            borrowed: "none".into(),
            chords: 0,
            tempo: 100.0,
            instrument: "acoustic-grand-piano".into(),
            pattern: "block".into(),
            voice_leading: true,
            repeats: 1,
            octave: 4,
            output: "text".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Style templates
// ---------------------------------------------------------------------------

/// One style preset: a name, whether `sevenths=auto` thickens its chords, and
/// its progressions written as scale-degree tokens.
struct Style {
    name: &'static str,
    auto_sevenths: bool,
    templates: &'static [&'static [&'static str]],
}

/// Token grammar: an optional `b`/`#` (relative to the MAJOR scale), a degree
/// digit 1-7, and an optional quality override — `D` dominant 7th, `m` minor,
/// `M` major, `s` sus4, `o` diminished.
const STYLES_TABLE: &[Style] = &[
    Style {
        name: "pop",
        auto_sevenths: false,
        templates: &[
            &["1", "5", "6", "4"],
            &["6", "4", "1", "5"],
            &["1", "6", "4", "5"],
            &["4", "1", "5", "6"],
            &["1", "4", "6", "5"],
            &["1", "5", "6", "3"],
        ],
    },
    Style {
        name: "rock",
        auto_sevenths: false,
        templates: &[
            &["1", "4", "5", "4"],
            &["1", "b7", "4", "1"],
            &["1", "5", "4", "5"],
            &["6", "5", "4", "5"],
            &["1", "4", "1", "5"],
            &["1", "b7", "b6", "5"],
        ],
    },
    Style {
        name: "folk",
        auto_sevenths: false,
        templates: &[
            &["1", "4", "5", "1"],
            &["1", "5", "4", "1"],
            &["1", "4", "1", "5"],
            &["1", "2", "4", "5"],
            &["1", "5", "1", "4"],
            &["1", "3", "4", "5"],
        ],
    },
    Style {
        name: "country",
        auto_sevenths: false,
        templates: &[
            &["1", "4", "1", "5"],
            &["1", "1", "4", "5"],
            &["1", "4", "5", "5"],
            &["5", "4", "1", "1"],
            &["1", "6", "2", "5"],
            &["1", "4", "5", "1"],
        ],
    },
    Style {
        name: "ballad",
        auto_sevenths: true,
        templates: &[
            &["1", "6", "4", "5"],
            &["1", "3", "4", "4"],
            &["4", "5", "1", "6"],
            &["1", "5", "2", "4"],
            &["6", "4", "1", "5"],
            &["1", "4", "2", "5"],
        ],
    },
    Style {
        name: "worship",
        auto_sevenths: false,
        templates: &[
            &["1", "5", "6", "4"],
            &["1", "4", "6", "5"],
            &["6", "4", "1", "5"],
            &["4", "1", "5", "6"],
            &["1", "4", "5", "6"],
            &["1", "6", "5", "4"],
        ],
    },
    Style {
        name: "edm",
        auto_sevenths: false,
        templates: &[
            &["6", "4", "1", "5"],
            &["6", "5", "4", "5"],
            &["1", "5", "6", "4"],
            &["6", "4", "5", "5"],
            &["4", "6", "1", "5"],
            &["6", "4", "5", "1"],
        ],
    },
    Style {
        name: "hip-hop",
        auto_sevenths: true,
        templates: &[
            &["1", "4"],
            &["6", "2"],
            &["1", "6", "4", "5"],
            &["2", "5", "1", "1"],
            &["6", "5", "4", "4"],
            &["1", "b3", "4", "5"],
        ],
    },
    Style {
        name: "lofi",
        auto_sevenths: true,
        templates: &[
            &["2", "5", "1", "6"],
            &["1", "6", "2", "5"],
            &["4", "5", "3", "6"],
            &["2", "5", "3", "6"],
            &["1", "4", "3", "6"],
            &["6", "2", "5", "1"],
        ],
    },
    Style {
        name: "rnb",
        auto_sevenths: true,
        templates: &[
            &["1", "4", "2", "5"],
            &["2", "5", "1", "6"],
            &["1", "6", "2", "5"],
            &["1", "3", "6", "4"],
            &["6", "2", "5", "1"],
            &["4", "5", "6", "1"],
        ],
    },
    Style {
        name: "jazz",
        auto_sevenths: true,
        templates: &[
            &["2", "5", "1", "1"],
            &["1", "6", "2", "5"],
            &["2", "5", "1", "4"],
            &["3", "6", "2", "5"],
            &["1", "4", "2", "5"],
            &["6", "2", "5", "1"],
        ],
    },
    Style {
        name: "blues",
        auto_sevenths: false,
        templates: &[
            &[
                "1D", "1D", "1D", "1D", "4D", "4D", "1D", "1D", "5D", "4D", "1D", "5D",
            ],
            &[
                "1D", "4D", "1D", "1D", "4D", "4D", "1D", "1D", "5D", "4D", "1D", "5D",
            ],
            &["1D", "4D", "5D", "4D"],
            &["1D", "1D", "4D", "4D", "1D", "1D", "5D", "5D"],
            &["1m", "4m", "1m", "5D"],
            &["1D", "4D", "1D", "5D", "4D", "1D"],
        ],
    },
    Style {
        name: "reggae",
        auto_sevenths: false,
        templates: &[
            &["1", "4", "5", "4"],
            &["1", "5", "6", "4"],
            &["1", "4", "1", "5"],
            &["1", "4", "4", "1"],
            &["6", "5", "4", "5"],
            &["1", "5", "4", "1"],
        ],
    },
    Style {
        name: "metal",
        auto_sevenths: false,
        templates: &[
            &["1", "b6", "b7", "1"],
            &["1", "b7", "b6", "5"],
            &["1", "4", "b6", "5"],
            &["1", "5", "b6", "b7"],
            &["1", "b7", "1", "b6"],
            &["1", "4", "5", "b6"],
        ],
    },
    Style {
        name: "cinematic",
        auto_sevenths: false,
        templates: &[
            &["1", "b6", "b7", "4"],
            &["6", "4", "5", "1"],
            &["1", "5", "b7", "4"],
            &["1", "4", "b6", "b7"],
            &["1", "3", "4", "6"],
            &["1", "b7", "4", "1"],
        ],
    },
];

/// Where the `random` walk can go from each scale degree (0-based).
const WALK: [&[usize]; 7] = [
    &[3, 4, 5, 1],
    &[4, 0, 3],
    &[5, 3, 1],
    &[4, 0, 1, 5],
    &[0, 5, 3],
    &[3, 1, 4, 0],
    &[0, 2],
];

// ---------------------------------------------------------------------------
// Deterministic pseudo-randomness
// ---------------------------------------------------------------------------

/// splitmix64 — a tiny, fully deterministic bit mixer.
fn mix(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Deterministic stream of small numbers seeded by the run's options.
struct Rng(u64);

impl Rng {
    fn seeded(parts: &[&str], variation: i32) -> Rng {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for p in parts {
            for b in p.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
            h ^= 0x5f;
        }
        Rng(mix(h ^ (variation as u64).wrapping_mul(0x9E37_79B9)))
    }

    fn below(&mut self, n: usize) -> usize {
        self.0 = mix(self.0);
        if n == 0 {
            0
        } else {
            (self.0 >> 11) as usize % n
        }
    }
}

// ---------------------------------------------------------------------------
// Degree tokens
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Force {
    Diatonic,
    Dominant,
    Minor,
    Major,
    Sus4,
    Dim,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Token {
    /// -1, 0 or +1 semitone against the MAJOR scale degree.
    accidental: i32,
    /// 0-based scale degree (0 = tonic).
    degree: usize,
    force: Force,
}

fn parse_token(t: &str) -> Result<Token, String> {
    let bad = || format!("internal: malformed style token '{t}'");
    let mut chars = t.chars();
    let mut c = chars.next().ok_or_else(bad)?;
    let mut accidental = 0;
    if c == 'b' {
        accidental = -1;
        c = chars.next().ok_or_else(bad)?;
    } else if c == '#' {
        accidental = 1;
        c = chars.next().ok_or_else(bad)?;
    }
    let degree = c.to_digit(10).ok_or_else(bad)? as usize;
    if !(1..=7).contains(&degree) {
        return Err(bad());
    }
    let force = match chars.next() {
        None => Force::Diatonic,
        Some('D') => Force::Dominant,
        Some('m') => Force::Minor,
        Some('M') => Force::Major,
        Some('s') => Force::Sus4,
        Some('o') => Force::Dim,
        Some(_) => return Err(bad()),
    };
    if chars.next().is_some() {
        return Err(bad());
    }
    Ok(Token {
        accidental,
        degree: degree - 1,
        force,
    })
}

// ---------------------------------------------------------------------------
// Scales, spelling and chord building
// ---------------------------------------------------------------------------

fn mode_scale(mode: &str) -> Option<[i32; 7]> {
    Some(match mode {
        "major" => [0, 2, 4, 5, 7, 9, 11],
        "minor" => [0, 2, 3, 5, 7, 8, 10],
        "dorian" => [0, 2, 3, 5, 7, 9, 10],
        "phrygian" => [0, 1, 3, 5, 7, 8, 10],
        "lydian" => [0, 2, 4, 6, 7, 9, 11],
        "mixolydian" => [0, 2, 4, 5, 7, 9, 10],
        "locrian" => [0, 1, 3, 5, 6, 8, 10],
        "harmonic-minor" => [0, 2, 3, 5, 7, 8, 11],
        "melodic-minor" => [0, 2, 3, 5, 7, 9, 11],
        _ => return None,
    })
}

/// Letter index (0 = C) and accidental offset of a written key.
fn key_spec(key: &str) -> Option<(i32, i32)> {
    let mut chars = key.chars();
    let letter = chars.next()?;
    let idx = LETTERS.iter().position(|l| *l == letter)? as i32;
    let acc = match chars.next() {
        None => 0,
        Some('#') => 1,
        Some('b') => -1,
        Some(_) => return None,
    };
    if chars.next().is_some() {
        return None;
    }
    Some((idx, acc))
}

/// Spell a note as a letter plus up to two accidentals. Falls back to a plain
/// pitch-class name when the theoretically correct spelling would need three.
fn spell(letter_index: i32, semitone: i32, prefer_flats: bool) -> String {
    let li = letter_index.rem_euclid(7) as usize;
    let natural = LETTER_PC[li];
    let mut acc = (semitone - natural).rem_euclid(12);
    if acc > 6 {
        acc -= 12;
    }
    let marks = match acc {
        -2 => "bb",
        -1 => "b",
        0 => "",
        1 => "#",
        2 => "##",
        _ => {
            let pc = semitone.rem_euclid(12) as usize;
            return if prefer_flats {
                FLAT_NAMES[pc].to_string()
            } else {
                SHARP_NAMES[pc].to_string()
            };
        }
    };
    format!("{}{}", LETTERS[li], marks)
}

/// One generated chord: its Roman numeral, its chord symbol and its notes.
#[derive(Debug, Clone, PartialEq)]
pub struct ChordOut {
    /// 1-based position in the progression (one bar of 4/4 each).
    pub bar: usize,
    /// Roman-numeral analysis in the chosen key and mode, e.g. `vi7` or `bVII`.
    pub roman: String,
    /// Chord symbol, e.g. `Am7` or `Bb`.
    pub symbol: String,
    /// Spelled chord tones, low to high, e.g. `["A", "C", "E", "G"]`.
    pub notes: Vec<String>,
}

impl ChordOut {
    /// The chord tones as one space-separated string.
    pub fn note_text(&self) -> String {
        self.notes.join(" ")
    }
}

/// Interval recipe of a chord: (letter steps above the root, semitones).
struct Recipe {
    third: (i32, i32),
    fifth: (i32, i32),
    seventh: Option<(i32, i32)>,
    ninth: Option<(i32, i32)>,
}

fn build_chord(
    tok: Token,
    scale: &[i32; 7],
    key_letter: i32,
    key_pc: i32,
    prefer_flats: bool,
    sevenths: &str,
    auto_sevenths: bool,
    bar: usize,
) -> ChordOut {
    let chromatic = tok.accidental != 0;
    let root_semi = if chromatic {
        MAJOR[tok.degree] + tok.accidental
    } else {
        scale[tok.degree]
    };

    // Diatonic stack, used both for the default quality and for the 9th test.
    let step = |n: usize| -> i32 {
        let raw = scale[(tok.degree + n) % 7];
        let wraps = ((tok.degree + n) / 7) as i32;
        raw + 12 * wraps - scale[tok.degree]
    };
    let (d3, d5, d7, d9) = (step(2), step(4), step(6), step(1) + 12);

    let thick = match sevenths {
        "triads" => false,
        "sevenths" | "extended" => true,
        _ => auto_sevenths,
    };
    let extended = sevenths == "extended";

    let recipe = match tok.force {
        Force::Dominant => Recipe {
            third: (2, 4),
            fifth: (4, 7),
            seventh: if sevenths == "triads" {
                None
            } else {
                Some((6, 10))
            },
            ninth: None,
        },
        Force::Minor => Recipe {
            third: (2, 3),
            fifth: (4, 7),
            seventh: if thick { Some((6, 10)) } else { None },
            ninth: None,
        },
        Force::Major => Recipe {
            third: (2, 4),
            fifth: (4, 7),
            seventh: if thick { Some((6, 11)) } else { None },
            ninth: None,
        },
        Force::Sus4 => Recipe {
            third: (3, 5),
            fifth: (4, 7),
            seventh: if thick { Some((6, 10)) } else { None },
            ninth: None,
        },
        Force::Dim => Recipe {
            third: (2, 3),
            fifth: (4, 6),
            seventh: if thick { Some((6, 9)) } else { None },
            ninth: None,
        },
        Force::Diatonic if chromatic => {
            // Borrowed majors: bVII behaves as a dominant, the rest as maj7.
            let seventh = if thick {
                Some((6, if tok.degree == 6 { 10 } else { 11 }))
            } else {
                None
            };
            Recipe {
                third: (2, 4),
                fifth: (4, 7),
                seventh,
                ninth: None,
            }
        }
        Force::Diatonic => Recipe {
            third: (2, d3),
            fifth: (4, d5),
            seventh: if thick { Some((6, d7)) } else { None },
            // Only a natural 9th is added, so an extended chord never turns a
            // flat 9 from the mode into a clash.
            ninth: if extended && thick && d9 == 14 && (d5 == 7) {
                Some((1, 14))
            } else {
                None
            },
        },
    };

    let root_letter = key_letter + tok.degree as i32;
    let root_pc = key_pc + root_semi;
    let mut notes = vec![spell(root_letter, root_pc, prefer_flats)];
    for (ls, semi) in [
        Some(recipe.third),
        Some(recipe.fifth),
        recipe.seventh,
        recipe.ninth,
    ]
    .into_iter()
    .flatten()
    {
        notes.push(spell(root_letter + ls, root_pc + semi, prefer_flats));
    }

    let seventh = recipe.seventh.map(|(_, s)| s);
    let ninth = recipe.ninth.is_some();
    let suffix = symbol_suffix(recipe.third.1, recipe.fifth.1, seventh, ninth);
    let numeral = roman(
        tok,
        recipe.third.1,
        recipe.fifth.1,
        seventh,
        ninth,
        tok.force == Force::Sus4,
    );

    ChordOut {
        bar,
        roman: numeral,
        symbol: format!("{}{}", notes[0], suffix),
        notes,
    }
}

/// Chord-symbol suffix for an interval shape. Every value returned here parses
/// with the shared MIDI chord-symbol parser (see the round-trip test).
fn symbol_suffix(third: i32, fifth: i32, seventh: Option<i32>, ninth: bool) -> &'static str {
    match (third, fifth, seventh) {
        (5, _, None) => "sus4",
        (5, _, Some(_)) => "7sus4",
        (4, 7, None) => "",
        (4, 7, Some(11)) => {
            if ninth {
                "maj9"
            } else {
                "maj7"
            }
        }
        (4, 7, Some(_)) => {
            if ninth {
                "9"
            } else {
                "7"
            }
        }
        (3, 7, None) => "m",
        (3, 7, Some(11)) => "m(maj7)",
        (3, 7, Some(_)) => {
            if ninth {
                "m9"
            } else {
                "m7"
            }
        }
        (3, 6, None) => "dim",
        (3, 6, Some(9)) => "dim7",
        (3, 6, Some(_)) => "m7b5",
        (4, 8, None) => "aug",
        (4, 8, Some(11)) => "aug(maj7)",
        (4, 8, Some(_)) => "aug7",
        _ => "",
    }
}

/// Roman-numeral analysis for a chord shape.
fn roman(
    tok: Token,
    third: i32,
    fifth: i32,
    seventh: Option<i32>,
    ninth: bool,
    sus: bool,
) -> String {
    let major_ish = third >= 4 || sus;
    let base = if major_ish {
        UPPER[tok.degree]
    } else {
        LOWER[tok.degree]
    };
    let acc = match tok.accidental {
        -1 => "b",
        1 => "#",
        _ => "",
    };
    let tail = if sus {
        match seventh {
            Some(_) => "7sus4".to_string(),
            None => "sus4".to_string(),
        }
    } else if fifth == 6 {
        match seventh {
            None => "\u{b0}".to_string(),
            Some(9) => "\u{b0}7".to_string(),
            Some(_) => "\u{f8}7".to_string(),
        }
    } else if fifth == 8 {
        match seventh {
            None => "+".to_string(),
            Some(11) => "+(maj7)".to_string(),
            Some(_) => "+7".to_string(),
        }
    } else {
        match seventh {
            None => String::new(),
            Some(11) if third == 3 => "(maj7)".to_string(),
            Some(11) => if ninth { "maj9" } else { "maj7" }.to_string(),
            Some(_) => if ninth { "9" } else { "7" }.to_string(),
        }
    };
    format!("{acc}{base}{tail}")
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

/// A finished progression plus the MIDI file rendered from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Generation {
    /// Tonic as written, e.g. `Eb`.
    pub key: String,
    /// Mode as written, e.g. `dorian`.
    pub mode: String,
    /// Style preset used, e.g. `jazz`.
    pub style: String,
    /// Variation index used.
    pub variation: i32,
    /// The chords of one pass through the progression.
    pub chords: Vec<ChordOut>,
    /// Tempo written into the MIDI file.
    pub tempo: f64,
    /// Pattern written into the MIDI file.
    pub pattern: String,
    /// Instrument written into the MIDI file.
    pub instrument: String,
    /// How many passes the MIDI file contains.
    pub repeats: i32,
    /// The Standard MIDI File bytes.
    pub midi: Vec<u8>,
    /// How many note events the MIDI file holds.
    pub note_count: usize,
    /// Playing time of the MIDI file in seconds.
    pub seconds: f64,
    /// Lowest sounding note, e.g. `C3`.
    pub lowest: String,
    /// Highest sounding note, e.g. `E5`.
    pub highest: String,
}

impl Generation {
    /// The Roman numerals as one line.
    pub fn roman_line(&self) -> String {
        self.chords
            .iter()
            .map(|c| c.roman.clone())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The chord symbols as one line.
    pub fn chord_line(&self) -> String {
        self.chords
            .iter()
            .map(|c| c.symbol.clone())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// `bar,roman,chord,notes` rows with a header.
    pub fn csv(&self) -> String {
        let mut s = String::from("bar,roman,chord,notes\n");
        for c in &self.chords {
            s.push_str(&format!(
                "{},{},{},{}\n",
                c.bar,
                c.roman,
                c.symbol,
                c.note_text()
            ));
        }
        s
    }

    /// The MIDI file as standard base64 (no line breaks).
    pub fn midi_base64(&self) -> String {
        B64.encode(&self.midi)
    }

    /// A `data:` URL for the MIDI file, used by the page and chat surfaces.
    pub fn data_url(&self) -> String {
        format!("data:{MIDI_MIME};base64,{}", self.midi_base64())
    }

    /// One-line description of the generated file.
    pub fn summary(&self) -> String {
        format!(
            "{} bar{} of {} {} ({} style, variation {}) — {} x {} in the MIDI file, {} notes, {} s, {} bytes.",
            self.chords.len(),
            if self.chords.len() == 1 { "" } else { "s" },
            self.key,
            self.mode,
            self.style,
            self.variation,
            self.repeats,
            if self.repeats == 1 { "pass" } else { "passes" },
            self.note_count,
            fmt_num(self.seconds),
            self.midi.len(),
        )
    }

    /// The full human-readable report.
    pub fn text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "Key: {} {} | Style: {} (variation {}) | Tempo: {} BPM | Pattern: {} | Instrument: {}\n\n",
            self.key,
            self.mode,
            self.style,
            self.variation,
            fmt_num(self.tempo),
            self.pattern,
            self.instrument,
        ));
        out.push_str(&format!("Roman:  {}\n", self.roman_line()));
        out.push_str(&format!("Chords: {}\n\n", self.chord_line()));

        let rw = self
            .chords
            .iter()
            .map(|c| c.roman.chars().count())
            .chain(std::iter::once(5))
            .max()
            .unwrap_or(5);
        let cw = self
            .chords
            .iter()
            .map(|c| c.symbol.chars().count())
            .chain(std::iter::once(5))
            .max()
            .unwrap_or(5);
        out.push_str(&format!(
            "{:<4}{:<w1$}  {:<w2$}  {}\n",
            "Bar",
            "Roman",
            "Chord",
            "Notes",
            w1 = rw,
            w2 = cw
        ));
        for c in &self.chords {
            out.push_str(&format!(
                "{:<4}{:<w1$}  {:<w2$}  {}\n",
                c.bar,
                c.roman,
                c.symbol,
                c.note_text(),
                w1 = rw,
                w2 = cw
            ));
        }
        out.push_str(&format!("\n{}\n", self.summary()));
        out.push_str(&format!(
            "Range: {} to {}. Download the .mid file, or use output=midi-base64 for the raw file.\n",
            self.lowest, self.highest
        ));
        out
    }

    /// Render the result in one of [`OUTPUTS`].
    pub fn render(&self, output: &str) -> Result<String, String> {
        match output {
            "text" => Ok(self.text()),
            "chords" => Ok(self.chord_line()),
            "roman" => Ok(self.roman_line()),
            "csv" => Ok(self.csv()),
            "midi-base64" => Ok(self.midi_base64()),
            other => Err(one_of("output", other, &OUTPUTS)),
        }
    }
}

fn one_of(name: &str, got: &str, allowed: &[&str]) -> String {
    format!(
        "unknown {name} '{got}': expected one of {}",
        allowed.join(", ")
    )
}

fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.1}")
    }
}

/// Modal-interchange substitutions used by `borrowed=rich`, keyed by the
/// 0-based diatonic degree they replace.
fn rich_substitute(degree: usize) -> Option<Token> {
    let t = |accidental: i32, degree: usize, force: Force| {
        Some(Token {
            accidental,
            degree,
            force,
        })
    };
    match degree {
        1 => t(0, 1, Force::Dominant), // ii -> II7, the secondary dominant of V
        2 => t(-1, 2, Force::Diatonic), // iii -> bIII
        3 => t(0, 3, Force::Minor),    // IV -> iv, the borrowed subdominant
        4 => t(-1, 6, Force::Diatonic), // V -> bVII
        5 => t(-1, 5, Force::Diatonic), // vi -> bVI
        _ => None,
    }
}

/// Generate a progression and render it to a Standard MIDI File.
pub fn generate(o: &Options) -> Result<Generation, String> {
    if !KEYS.contains(&o.key.as_str()) {
        return Err(one_of("key", &o.key, &KEYS));
    }
    if !MODES.contains(&o.mode.as_str()) {
        return Err(one_of("mode", &o.mode, &MODES));
    }
    if !STYLES.contains(&o.style.as_str()) {
        return Err(one_of("style", &o.style, &STYLES));
    }
    if !SEVENTHS.contains(&o.sevenths.as_str()) {
        return Err(one_of("sevenths", &o.sevenths, &SEVENTHS));
    }
    if !BORROWED.contains(&o.borrowed.as_str()) {
        return Err(one_of("borrowed", &o.borrowed, &BORROWED));
    }
    if !PATTERNS.contains(&o.pattern.as_str()) {
        return Err(one_of("pattern", &o.pattern, &PATTERNS));
    }
    if !INSTRUMENTS.contains(&o.instrument.as_str()) {
        return Err(one_of("instrument", &o.instrument, &INSTRUMENTS));
    }
    if !OUTPUTS.contains(&o.output.as_str()) {
        return Err(one_of("output", &o.output, &OUTPUTS));
    }
    if !(1..=MAX_VARIATION).contains(&o.variation) {
        return Err(format!(
            "variation must be between 1 and {MAX_VARIATION}, got {}",
            o.variation
        ));
    }
    if !(0..=MAX_CHORDS).contains(&o.chords) {
        return Err(format!(
            "chords must be between 0 (the style's own length) and {MAX_CHORDS}, got {}",
            o.chords
        ));
    }
    if !(1..=MAX_REPEATS).contains(&o.repeats) {
        return Err(format!(
            "repeats must be between 1 and {MAX_REPEATS}, got {}",
            o.repeats
        ));
    }
    if !(1..=7).contains(&o.octave) {
        return Err(format!("octave must be between 1 and 7, got {}", o.octave));
    }
    if !o.tempo.is_finite() || !(40.0..=300.0).contains(&o.tempo) {
        return Err(format!(
            "tempo must be between 40 and 300 BPM, got {}",
            fmt_num(o.tempo)
        ));
    }

    let scale = mode_scale(&o.mode).ok_or_else(|| one_of("mode", &o.mode, &MODES))?;
    let (key_letter, key_acc) = key_spec(&o.key).ok_or_else(|| one_of("key", &o.key, &KEYS))?;
    let key_pc = LETTER_PC[key_letter as usize] + key_acc;
    let prefer_flats = o.key.contains('b') || o.key == "F";

    // 1. Pick the raw token list.
    let (mut tokens, auto_sevenths) = if o.style == "random" {
        (random_tokens(o), false)
    } else {
        let style = STYLES_TABLE
            .iter()
            .find(|s| s.name == o.style)
            .ok_or_else(|| one_of("style", &o.style, &STYLES))?;
        let template = style.templates[(o.variation as usize - 1) % style.templates.len()];
        let mut toks = Vec::with_capacity(template.len());
        for t in template {
            toks.push(parse_token(t)?);
        }
        (toks, style.auto_sevenths)
    };

    // 2. Apply the modal-interchange level.
    match o.borrowed.as_str() {
        "none" => {
            for t in tokens.iter_mut() {
                t.accidental = 0;
            }
        }
        "rich" => {
            let mut rng = Rng::seeded(&[&o.key, &o.mode, &o.style, "rich"], o.variation);
            let eligible: Vec<usize> = tokens
                .iter()
                .enumerate()
                .filter(|(_, t)| {
                    t.accidental == 0
                        && t.force == Force::Diatonic
                        && rich_substitute(t.degree).is_some()
                })
                .map(|(i, _)| i)
                .collect();
            if !eligible.is_empty() {
                // Always recolour one chord — `rich` that sometimes changed
                // nothing would look like a broken control — then thin the rest
                // out so the progression stays recognisable.
                let forced = eligible[rng.below(eligible.len())];
                for i in eligible {
                    if i == forced || rng.below(3) == 0 {
                        if let Some(sub) = rich_substitute(tokens[i].degree) {
                            tokens[i] = sub;
                        }
                    }
                }
            }
        }
        _ => {}
    }

    // 3. Fit the requested chord count by cycling or truncating.
    if o.chords > 0 {
        let natural = tokens.clone();
        let want = o.chords as usize;
        tokens = (0..want).map(|i| natural[i % natural.len()]).collect();
    }

    // 4. Build the chords.
    let chords: Vec<ChordOut> = tokens
        .iter()
        .enumerate()
        .map(|(i, t)| {
            build_chord(
                *t,
                &scale,
                key_letter,
                key_pc,
                prefer_flats,
                &o.sevenths,
                auto_sevenths,
                i + 1,
            )
        })
        .collect();

    // 5. Render the MIDI file through the shared chord-symbol writer.
    let mut symbols: Vec<String> = Vec::with_capacity(chords.len() * o.repeats as usize);
    for _ in 0..o.repeats {
        symbols.extend(chords.iter().map(|c| c.symbol.clone()));
    }
    let midi_opts = smf::Options {
        tempo: o.tempo,
        beats_per_chord: BEATS_PER_CHORD,
        beats_per_bar: 4,
        octave: o.octave,
        voicing: "close".into(),
        inversion: if o.voice_leading { "smooth" } else { "root" }.into(),
        pattern: o.pattern.clone(),
        arp_note: "eighth".into(),
        note_length: 95.0,
        add_bass: false,
        transpose: 0,
        velocity: 96,
        instrument: o.instrument.clone(),
    };
    let conv = smf::convert(&symbols.join(" "), &midi_opts)
        .map_err(|e| format!("could not render the MIDI file: {e}"))?;

    Ok(Generation {
        key: o.key.clone(),
        mode: o.mode.clone(),
        style: o.style.clone(),
        variation: o.variation,
        chords,
        tempo: o.tempo,
        pattern: o.pattern.clone(),
        instrument: o.instrument.clone(),
        repeats: o.repeats,
        note_count: conv.notes,
        seconds: conv.seconds,
        lowest: smf::midi_to_name(conv.lowest as i32),
        highest: smf::midi_to_name(conv.highest as i32),
        midi: conv.midi,
    })
}

/// Deterministic in-key walk used by `style=random`.
fn random_tokens(o: &Options) -> Vec<Token> {
    let mut rng = Rng::seeded(&[&o.key, &o.mode, "random"], o.variation);
    let len = if o.chords > 0 {
        o.chords as usize
    } else if rng.below(2) == 0 {
        4
    } else {
        8
    };
    let mut degree = 0usize;
    let mut out = Vec::with_capacity(len);
    for _ in 0..len {
        out.push(Token {
            accidental: 0,
            degree,
            force: Force::Diatonic,
        });
        let next = WALK[degree];
        degree = next[rng.below(next.len())];
    }
    out
}

/// Generate and render in one call — the entry point every surface uses.
pub fn run(o: &Options) -> Result<String, String> {
    generate(o)?.render(&o.output)
}

/// Parse one page/CLI field, falling back to `default` when it is blank.
pub fn parse_field<T: std::str::FromStr>(name: &str, raw: &str, default: T) -> Result<T, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<T>()
        .map_err(|_| format!("{name} must be a number, got '{t}'"))
}

/// Read a checkbox/flag string; blank keeps `default`.
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

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn c_major_pop_is_the_classic_four_chord_loop() {
        let g = generate(&opts()).unwrap();
        assert_eq!(g.chord_line(), "C G Am F");
        assert_eq!(g.roman_line(), "I V vi IV");
        assert_eq!(g.chords[2].notes, vec!["A", "C", "E"]);
        assert_eq!(&g.midi[0..4], b"MThd");
    }

    #[test]
    fn output_shapes_all_render() {
        let mut o = opts();
        for out in OUTPUTS {
            o.output = out.into();
            let s = run(&o).unwrap();
            assert!(!s.is_empty(), "{out} produced nothing");
        }
        o.output = "chords".into();
        assert_eq!(run(&o).unwrap(), "C G Am F");
        o.output = "roman".into();
        assert_eq!(run(&o).unwrap(), "I V vi IV");
        o.output = "csv".into();
        assert_eq!(
            run(&o).unwrap(),
            "bar,roman,chord,notes\n1,I,C,C E G\n2,V,G,G B D\n3,vi,Am,A C E\n4,IV,F,F A C\n"
        );
        o.output = "midi-base64".into();
        let b64 = run(&o).unwrap();
        assert!(b64.starts_with("TVRoZ"), "base64 SMF header, got {b64:.12}");
        assert_eq!(B64.decode(b64).unwrap(), generate(&opts()).unwrap().midi);
    }

    #[test]
    fn same_options_always_give_the_same_bytes() {
        let mut o = opts();
        o.style = "random".into();
        o.variation = 37;
        o.key = "Eb".into();
        o.mode = "dorian".into();
        let a = generate(&o).unwrap();
        let b = generate(&o).unwrap();
        assert_eq!(a.chord_line(), b.chord_line());
        assert_eq!(a.midi, b.midi);
        // A different variation is a different progression.
        o.variation = 38;
        let c = generate(&o).unwrap();
        assert_ne!(a.chord_line(), c.chord_line());
    }

    #[test]
    fn keys_and_modes_spell_correctly() {
        let mut o = opts();
        o.key = "Eb".into();
        assert_eq!(generate(&o).unwrap().chord_line(), "Eb Bb Cm Ab");
        o.key = "F#".into();
        assert_eq!(generate(&o).unwrap().chord_line(), "F# C# D#m B");
        o.key = "A".into();
        o.mode = "minor".into();
        let g = generate(&o).unwrap();
        assert_eq!(g.chord_line(), "Am Em F Dm");
        assert_eq!(g.roman_line(), "i v VI iv");
        o.mode = "dorian".into();
        assert_eq!(generate(&o).unwrap().chord_line(), "Am Em F#dim D");
        o.mode = "harmonic-minor".into();
        let g = generate(&o).unwrap();
        assert_eq!(g.chord_line(), "Am E F Dm");
        assert_eq!(g.roman_line(), "i V VI iv");
        o.key = "C".into();
        o.mode = "locrian".into();
        let g = generate(&o).unwrap();
        assert_eq!(g.chord_line(), "Cdim Gb Ab Fm");
        assert_eq!(g.roman_line(), "i\u{b0} V VI iv");
    }

    #[test]
    fn sevenths_thicken_the_chords() {
        let mut o = opts();
        o.sevenths = "sevenths".into();
        assert_eq!(generate(&o).unwrap().chord_line(), "Cmaj7 G7 Am7 Fmaj7");
        o.sevenths = "extended".into();
        assert_eq!(generate(&o).unwrap().chord_line(), "Cmaj9 G9 Am9 Fmaj9");
        o.sevenths = "triads".into();
        assert_eq!(generate(&o).unwrap().chord_line(), "C G Am F");
        // auto follows the style: jazz is a seventh-chord idiom, pop is not.
        o.sevenths = "auto".into();
        o.style = "jazz".into();
        assert_eq!(generate(&o).unwrap().chord_line(), "Dm7 G7 Cmaj7 Cmaj7");
        // triads override the style's dominant sevenths too.
        o.style = "blues".into();
        assert_eq!(generate(&o).unwrap().chords[0].symbol, "C7");
        o.sevenths = "triads".into();
        assert_eq!(generate(&o).unwrap().chords[0].symbol, "C");
    }

    #[test]
    fn borrowed_levels_control_chromatic_chords() {
        let mut o = opts();
        o.style = "metal".into();
        o.borrowed = "light".into();
        assert_eq!(generate(&o).unwrap().roman_line(), "I bVI bVII I");
        o.borrowed = "none".into();
        assert_eq!(generate(&o).unwrap().roman_line(), "I vi vii\u{b0} I");
        // rich adds modal interchange to an otherwise diatonic style.
        o.style = "pop".into();
        o.borrowed = "none".into();
        let plain = generate(&o).unwrap().roman_line();
        o.borrowed = "rich".into();
        let rich = generate(&o).unwrap();
        assert_ne!(
            plain,
            rich.roman_line(),
            "rich should recolour the progression"
        );
        assert_eq!(rich.roman_line(), "I V bVI IV");
        assert_eq!(rich.chord_line(), "C G Ab F");
    }

    #[test]
    fn chord_count_and_repeats_shape_the_output() {
        let mut o = opts();
        o.style = "blues".into();
        assert_eq!(generate(&o).unwrap().chords.len(), 12);
        o.chords = 6;
        let g = generate(&o).unwrap();
        assert_eq!(g.chords.len(), 6);
        assert_eq!(g.chords[5].bar, 6);
        o.chords = 0;
        o.style = "pop".into();
        let one = generate(&o).unwrap();
        o.repeats = 3;
        let three = generate(&o).unwrap();
        assert_eq!(three.chords.len(), 4, "the report shows one pass");
        assert_eq!(three.note_count, one.note_count * 3);
        assert!(three.seconds > one.seconds);
    }

    #[test]
    fn patterns_voice_leading_and_instruments_change_the_file() {
        let base = generate(&opts()).unwrap();
        let mut o = opts();
        for p in PATTERNS {
            o.pattern = p.into();
            let g = generate(&o).unwrap();
            assert!(g.note_count >= 12, "{p} produced {} notes", g.note_count);
        }
        o = opts();
        o.voice_leading = false;
        assert_ne!(generate(&o).unwrap().midi, base.midi);
        o = opts();
        for i in INSTRUMENTS {
            o.instrument = i.into();
            assert_eq!(&generate(&o).unwrap().midi[0..4], b"MThd");
        }
    }

    #[test]
    fn every_generated_symbol_parses_as_a_chord() {
        // The MIDI writer re-parses our chord symbols, so a symbol it cannot
        // read would silently break the download for some key/mode/style.
        let mut o = opts();
        o.sevenths = "extended".into();
        o.borrowed = "rich".into();
        for key in KEYS {
            for mode in MODES {
                for style in STYLES {
                    o.key = key.into();
                    o.mode = mode.into();
                    o.style = style.into();
                    for variation in [1, 2, 3, 4, 5, 6, 7] {
                        o.variation = variation;
                        let g = generate(&o).unwrap();
                        for c in &g.chords {
                            smf::parse_chord_symbol(&c.symbol).unwrap_or_else(|e| {
                                panic!("{key} {mode} {style} v{variation}: {} -> {e}", c.symbol)
                            });
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_enum_value_is_accepted() {
        let mut o = opts();
        for key in KEYS {
            o.key = key.into();
            generate(&o).unwrap();
        }
        o = opts();
        for mode in MODES {
            o.mode = mode.into();
            generate(&o).unwrap();
        }
        o = opts();
        for style in STYLES {
            o.style = style.into();
            generate(&o).unwrap();
        }
        o = opts();
        for s in SEVENTHS {
            o.sevenths = s.into();
            generate(&o).unwrap();
        }
        o = opts();
        for b in BORROWED {
            o.borrowed = b.into();
            generate(&o).unwrap();
        }
        o = opts();
        for oct in 1..=7 {
            o.octave = oct;
            generate(&o).unwrap();
        }
        o = opts();
        for v in [1, 2, 3, 50, MAX_VARIATION] {
            o.variation = v;
            generate(&o).unwrap();
        }
    }

    #[test]
    fn bad_values_report_what_was_expected() {
        let mut o = opts();
        o.key = "H".into();
        assert!(generate(&o).unwrap_err().contains("unknown key 'H'"));
        o = opts();
        o.mode = "ionian".into();
        assert!(generate(&o).unwrap_err().contains("expected one of major"));
        o = opts();
        o.style = "polka".into();
        assert!(generate(&o).unwrap_err().contains("unknown style 'polka'"));
        o = opts();
        o.tempo = 12.0;
        assert_eq!(
            generate(&o).unwrap_err(),
            "tempo must be between 40 and 300 BPM, got 12"
        );
        o = opts();
        o.variation = 0;
        assert!(generate(&o)
            .unwrap_err()
            .contains("variation must be between 1 and 99"));
        o = opts();
        o.chords = 40;
        assert!(generate(&o).unwrap_err().contains("chords must be between"));
        o = opts();
        o.repeats = 9;
        assert!(generate(&o)
            .unwrap_err()
            .contains("repeats must be between"));
        o = opts();
        o.octave = 0;
        assert!(generate(&o).unwrap_err().contains("octave must be between"));
        o = opts();
        o.output = "midi".into();
        assert!(generate(&o)
            .unwrap_err()
            .contains("unknown output 'midi': expected one of text, chords"));
        assert!(generate(&opts())
            .unwrap()
            .render("wav")
            .unwrap_err()
            .contains("unknown output 'wav'"));
    }

    #[test]
    fn text_report_shows_the_analysis() {
        let g = generate(&opts()).unwrap();
        let t = g.text();
        assert!(t.starts_with("Key: C major | Style: pop (variation 1) | Tempo: 100 BPM"));
        assert!(t.contains("Roman:  I V vi IV"));
        assert!(t.contains("Chords: C G Am F"));
        assert!(t.contains("3   vi     Am     A C E"));
        assert!(t.contains("4 bars of C major"));
        assert!(t.contains("Range: "));
    }

    #[test]
    fn field_helpers_handle_blanks_and_junk() {
        assert_eq!(parse_field("tempo", "", 100.0).unwrap(), 100.0);
        assert_eq!(parse_field("tempo", " 120 ", 100.0).unwrap(), 120.0);
        assert_eq!(
            parse_field("tempo", "fast", 100.0).unwrap_err(),
            "tempo must be a number, got 'fast'"
        );
        assert!(truthy("", true));
        assert!(!truthy("false", true));
        assert!(truthy("on", false));
    }
}
