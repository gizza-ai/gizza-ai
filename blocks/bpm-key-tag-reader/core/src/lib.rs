//! gizza-ai/bpm-key-tag-reader core — read the **stored** tempo and musical-key
//! metadata out of a track's tags and normalise it into the notations DJs use.
//!
//! This is a tag READER, not an analyser: it reports what a tagger, DJ app or
//! library manager already wrote into the file. Nothing here estimates tempo or
//! key from the waveform (that needs DSP/ML, which is out of model for this
//! repo — see `docs/tool-skiplist.txt`).
//!
//! Tag systems covered, via `symphonia`'s always-enabled metadata readers:
//!   * **ID3v2** (MP3, AIFF, WAV) — `TBPM`, `TKEY`, and the user-defined
//!     `TXXX:` frames DJ software writes (`TXXX:INITIALKEY`,
//!     `TXXX:EnergyLevel`, …).
//!   * **Vorbis comments** (FLAC, Ogg) — `BPM`, `INITIALKEY`, `KEY`, …
//!   * **MP4 / iTunes `ilst`** (M4A, MP4, ALAC) — the `tmpo` atom plus freeform
//!     `com.apple.iTunes:<name>` atoms.
//!   * **RIFF INFO** (WAV) and **ID3v1**, for whatever they carry.
//!
//! Not covered: WMA/ASF (`WM/BeatsPerMinute`, `WM/InitialKey`), APEv2, and the
//! QuickTime `mdta`/`keys` metadata variant (what `ffmpeg -movflags
//! use_metadata_tags` writes) — symphonia ships no demuxer/reader for any of
//! them. The field names are still matched, so if another tag system ever
//! surfaces them they are picked up.
//!
//! Pure Rust, no I/O and no std clock, so the one implementation serves the
//! chat block and the CLI alike.

use std::io::Cursor;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::{MetadataOptions, MetadataRevision, StandardTagKey};
use symphonia::core::probe::Hint;

/// Tags past this many are not scanned — a pathological file can carry
/// thousands, and the report has to stay readable (and fit the chat context).
const MAX_TAGS: usize = 2000;
/// Long tag values (embedded art descriptions, lyric dumps) are elided.
const MAX_VALUE_CHARS: usize = 200;

/// Which notation(s) to report a parsed key in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyNotation {
    /// Traditional key name, e.g. `A minor`.
    Standard,
    /// Camelot / Lancelot wheel code, e.g. `8A`.
    Camelot,
    /// Open Key notation, e.g. `1m`.
    OpenKey,
    /// All three.
    All,
}

impl KeyNotation {
    /// Parse the descriptor's enum value. Unknown values are an error so a typo
    /// never silently changes the output.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "standard" => Ok(KeyNotation::Standard),
            "camelot" => Ok(KeyNotation::Camelot),
            "open-key" | "open_key" | "openkey" => Ok(KeyNotation::OpenKey),
            "all" => Ok(KeyNotation::All),
            other => Err(format!(
                "unknown key_notation '{other}' (use standard, camelot, open-key or all)"
            )),
        }
    }
}

/// Reader options.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    pub key_notation: KeyNotation,
    /// Also list every other tag found, not just the tempo/key ones.
    pub include_all_tags: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            key_notation: KeyNotation::All,
            include_all_tags: false,
        }
    }
}

/// One tag as stored in the file.
#[derive(Debug, Clone, PartialEq)]
pub struct TagHit {
    /// The literal field name, e.g. `TBPM`, `TXXX:INITIALKEY`, `BPM`, `tmpo`.
    pub field: String,
    /// The value exactly as stored (elided if very long).
    pub value: String,
}

/// The tempo reading.
#[derive(Debug, Clone, PartialEq)]
pub struct BpmReading {
    /// Parsed tempo in beats per minute.
    pub bpm: f64,
    /// `bpm` rounded to the nearest whole number — ID3v2.3 `TBPM` is defined as
    /// an integer, so this is what most software displays.
    pub bpm_rounded: u32,
    /// Field the value was read from.
    pub source_field: String,
    /// The stored text, before parsing.
    pub raw: String,
}

/// The musical-key reading.
#[derive(Debug, Clone, PartialEq)]
pub struct KeyReading {
    /// The stored text, before parsing.
    pub raw: String,
    /// Field the value was read from.
    pub source_field: String,
    /// The key rendered in the requested notation (the traditional name when
    /// `key_notation = all`).
    pub display: String,
    /// Traditional name, e.g. `A minor`. Present unless a single other notation
    /// was requested.
    pub standard: Option<String>,
    /// Camelot / Lancelot code, e.g. `8A`.
    pub camelot: Option<String>,
    /// Open Key code, e.g. `1m`.
    pub open_key: Option<String>,
    /// Tonic note as spelled on the Camelot wheel, e.g. `A`, `F#`, `Ab`.
    pub tonic: String,
    /// `major` or `minor`.
    pub mode: String,
}

/// The Mixed In Key style energy level, when the file carries one.
#[derive(Debug, Clone, PartialEq)]
pub struct EnergyReading {
    pub energy: String,
    pub source_field: String,
}

/// The full report.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Friendly container name sniffed from the magic bytes.
    pub container: String,
    /// True when a tempo and/or key tag was found.
    pub found: bool,
    pub bpm: Option<BpmReading>,
    pub key: Option<KeyReading>,
    pub energy: Option<EnergyReading>,
    /// Every tempo/key/energy tag found, including the ones not chosen.
    pub matched_tags: Vec<TagHit>,
    /// Every tag in the file — only when `include_all_tags` is set.
    pub all_tags: Option<Vec<TagHit>>,
    /// How many tags were scanned in total.
    pub tag_count: usize,
    /// Anything worth telling the caller (unparseable value, off-key marker,
    /// no tags at all, …).
    pub notes: Vec<String>,
}

// ---------------------------------------------------------------------------
// Notation tables
//
// Verified against Mixxx's src/track/keyutils.cpp rather than memory:
// C major = Camelot 8B = Open Key 1d; A minor = Camelot 8A = Open Key 1m.
// The Camelot number follows the circle of fifths, so +7 semitones = +1 step.
// ---------------------------------------------------------------------------

/// Camelot number for each MAJOR key, indexed by pitch class (C = 0).
const MAJOR_CAMELOT: [u8; 12] = [8, 3, 10, 5, 12, 7, 2, 9, 4, 11, 6, 1];
/// Camelot-wheel spelling of each major tonic, indexed by pitch class.
const MAJOR_NAME: [&str; 12] = [
    "C", "Db", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];
/// Camelot-wheel spelling of each minor tonic, indexed by pitch class.
const MINOR_NAME: [&str; 12] = [
    "C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B",
];

/// A key as `(pitch class, is_minor)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Key {
    pc: usize,
    minor: bool,
}

impl Key {
    fn camelot_number(self) -> u8 {
        // A minor key shares its Camelot number with its relative major, which
        // sits three semitones up (A minor <-> C major).
        let pc = if self.minor {
            (self.pc + 3) % 12
        } else {
            self.pc
        };
        MAJOR_CAMELOT[pc]
    }
    fn camelot(self) -> String {
        format!(
            "{}{}",
            self.camelot_number(),
            if self.minor { "A" } else { "B" }
        )
    }
    fn open_key(self) -> String {
        // Open Key number = Camelot number + 5 (mod 12), so Camelot 8 -> 1.
        let n = ((self.camelot_number() as u16 + 4) % 12) + 1;
        format!("{}{}", n, if self.minor { "m" } else { "d" })
    }
    fn tonic(self) -> &'static str {
        if self.minor {
            MINOR_NAME[self.pc]
        } else {
            MAJOR_NAME[self.pc]
        }
    }
    fn standard(self) -> String {
        format!(
            "{} {}",
            self.tonic(),
            if self.minor { "minor" } else { "major" }
        )
    }
    /// Rebuild a key from a Camelot number (1-12) + minor flag.
    fn from_camelot(number: u8, minor: bool) -> Option<Key> {
        let major_pc = MAJOR_CAMELOT.iter().position(|&n| n == number)?;
        let pc = if minor { (major_pc + 9) % 12 } else { major_pc };
        Some(Key { pc, minor })
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Normalise a tag field name so `TXXX:INITIALKEY`, `com.apple.iTunes:initialkey`,
/// `WM/InitialKey`, `Initial Key` and `initialkey` all compare equal.
fn normalize_field(field: &str) -> String {
    let tail = field
        .rsplit(|c| c == ':' || c == '/')
        .next()
        .unwrap_or(field);
    tail.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// The literal field name to report. symphonia's MP4 `ilst` reader recognises
/// the standard atoms (`tmpo`, `©ART`, …), maps them onto a `StandardTagKey` and
/// leaves the literal key EMPTY — so an MP4 tempo would otherwise be reported as
/// coming from field `""`. Name those from the standard key instead.
fn field_label(field: &str, std_key: Option<StandardTagKey>) -> String {
    if !field.is_empty() {
        return field.to_string();
    }
    match std_key {
        Some(StandardTagKey::Bpm) => "tmpo".to_string(),
        Some(other) => format!("{other:?}"),
        None => "(unnamed)".to_string(),
    }
}

/// Tempo field names, most authoritative first.
const BPM_FIELDS: [&str; 6] = ["TBPM", "BPM", "TMPO", "TEMPO", "BEATSPERMINUTE", "FMPSBPM"];
/// Key field names, most authoritative first.
const KEY_FIELDS: [&str; 5] = ["TKEY", "INITIALKEY", "KEY", "FMPSINITIALKEY", "MUSICALKEY"];
/// Energy-level field names, most authoritative first.
const ENERGY_FIELDS: [&str; 2] = ["ENERGYLEVEL", "ENERGY"];

/// Rank a normalised field name within a priority list (lower = better).
fn rank(list: &[&str], normalized: &str) -> Option<usize> {
    list.iter().position(|f| *f == normalized)
}

/// Parse a stored tempo string. Accepts `128`, `128.50`, `128,5` (comma decimal
/// separator, common in European taggers) and stray whitespace/NULs.
pub fn parse_bpm(raw: &str) -> Option<f64> {
    let cleaned: String = raw
        .trim_matches(|c: char| c.is_whitespace() || c == '\0')
        .chars()
        .map(|c| if c == ',' { '.' } else { c })
        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-' || *c == '+')
        .collect();
    let v: f64 = cleaned.parse().ok()?;
    // A tempo outside this range is a mis-tagged field, not a tempo.
    if v.is_finite() && v > 0.0 && v <= 999.0 {
        Some(v)
    } else {
        None
    }
}

/// Semitone offset of a note letter above C.
fn letter_pc(c: char) -> Option<usize> {
    Some(match c.to_ascii_uppercase() {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => return None,
    })
}

/// Parse a stored musical-key string in any of the forms real taggers write:
/// traditional (`Am`, `A min`, `A minor`, `F#m`, `Gb`, `C`, `Cmaj`), Camelot
/// (`8A`, `08A`) or Open Key (`1m`, `12d`). Returns `None` for anything else,
/// including the ID3v2 `TKEY` "off key" marker `o`.
fn parse_key(raw: &str) -> Option<Key> {
    let s: String = raw
        .trim_matches(|c: char| c.is_whitespace() || c == '\0')
        .chars()
        .map(|c| match c {
            '♯' => '#',
            '♭' => 'b',
            '-' | '_' => ' ',
            other => other,
        })
        .collect();
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let lower = s.to_ascii_lowercase();
    let compact: String = lower.chars().filter(|c| !c.is_whitespace()).collect();

    // Camelot (8A / 08B) and Open Key (1m / 12d) both start with a digit.
    if compact.starts_with(|c: char| c.is_ascii_digit()) {
        let digits: String = compact.chars().take_while(|c| c.is_ascii_digit()).collect();
        let suffix: String = compact.chars().skip(digits.len()).collect();
        let number: u16 = digits.parse().ok()?;
        if !(1..=12).contains(&number) {
            return None;
        }
        return match suffix.as_str() {
            "a" => Key::from_camelot(number as u8, true),
            "b" => Key::from_camelot(number as u8, false),
            "m" => {
                // Open Key number -> Camelot number (subtract 5, mod 12).
                let camelot = ((number + 6) % 12) + 1;
                Key::from_camelot(camelot as u8, true)
            }
            "d" => {
                let camelot = ((number + 6) % 12) + 1;
                Key::from_camelot(camelot as u8, false)
            }
            _ => None,
        };
    }

    // Traditional: note letter, optional accidental, optional mode word.
    let mut chars = compact.chars();
    let pc0 = letter_pc(chars.next()?)?;
    let rest: String = chars.collect();
    let (accidental, rest) = if let Some(r) = rest.strip_prefix('#') {
        (1i32, r.to_string())
    } else if let Some(r) = rest.strip_prefix('b') {
        // A leading `b` is an accidental only when something follows it or the
        // letter itself is not `B` — `Bb` is B-flat, plain `b` after a letter
        // with no mode word would be ambiguous, so treat it as flat.
        (-1i32, r.to_string())
    } else {
        (0i32, rest)
    };
    let minor = match rest.as_str() {
        "" | "maj" | "major" | "dur" | "d" => false,
        "m" | "min" | "minor" | "moll" => true,
        _ => return None,
    };
    let pc = ((pc0 as i32 + accidental).rem_euclid(12)) as usize;
    Some(Key { pc, minor })
}

// ---------------------------------------------------------------------------
// Container sniffing
// ---------------------------------------------------------------------------

fn container_name(bytes: &[u8]) -> String {
    let starts = |p: &[u8]| bytes.len() >= p.len() && &bytes[..p.len()] == p;
    // ID3v2 headers sit in front of MP3 (and sometimes AIFF/WAV) payloads.
    let after_id3 = if starts(b"ID3") {
        "MP3 (MPEG audio)"
    } else {
        ""
    };
    if starts(b"fLaC") {
        return "FLAC (native)".into();
    }
    if starts(b"OggS") {
        return "OGG".into();
    }
    if starts(b"RIFF") {
        return "WAVE (RIFF)".into();
    }
    if starts(b"FORM") {
        return "AIFF".into();
    }
    if starts(b"caff") {
        return "Core Audio (CAF)".into();
    }
    if starts(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return "Matroska / WebM".into();
    }
    if bytes.len() >= 12 && &bytes[4..8] == b"ftyp" {
        return "MP4 / M4A (ISO BMFF)".into();
    }
    if !after_id3.is_empty() {
        return after_id3.into();
    }
    if bytes.len() >= 2 && bytes[0] == 0xFF && (bytes[1] & 0xE0) == 0xE0 {
        return "MP3 (MPEG audio)".into();
    }
    "media".into()
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn elide(v: &str) -> String {
    let v = v.trim_matches(|c: char| c.is_whitespace() || c == '\0');
    if v.chars().count() > MAX_VALUE_CHARS {
        let head: String = v.chars().take(MAX_VALUE_CHARS).collect();
        format!("{head}…")
    } else {
        v.to_string()
    }
}

fn collect(rev: &MetadataRevision, out: &mut Vec<(String, Option<StandardTagKey>, String)>) {
    for tag in rev.tags() {
        if out.len() >= MAX_TAGS {
            return;
        }
        out.push((tag.key.clone(), tag.std_key, tag.value.to_string()));
    }
}

/// Read the tempo/key tags from `bytes`.
///
/// Errors only when the container cannot be opened at all; a file that simply
/// carries no tempo/key tags comes back as a clean `found = false` report.
pub fn read(bytes: &[u8], opts: Options) -> Result<Report, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let container = container_name(bytes);

    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("unrecognised or unsupported media container: {e}"))?;

    let mut raw: Vec<(String, Option<StandardTagKey>, String)> = Vec::new();
    // Tags found ahead of the container (an ID3v2 block in front of an MP3)
    // live on the probe result; tags inside the container live on the reader.
    // Each borrow is scoped so the two field borrows never overlap.
    {
        let mut probe_meta = probed.metadata.get();
        if let Some(rev) = probe_meta.as_mut().and_then(|m| m.current()) {
            collect(rev, &mut raw);
        }
    }
    {
        let format_meta = probed.format.metadata();
        if let Some(rev) = format_meta.current() {
            collect(rev, &mut raw);
        }
    }

    let mut notes: Vec<String> = Vec::new();
    if raw.len() >= MAX_TAGS {
        notes.push(format!(
            "only the first {MAX_TAGS} tags were scanned — the file carries more"
        ));
    }

    let mut matched_tags: Vec<TagHit> = Vec::new();
    let mut best_bpm: Option<(usize, BpmReading)> = None;
    let mut best_key: Option<(usize, KeyReading, Key)> = None;
    let mut best_energy: Option<(usize, EnergyReading)> = None;
    let mut unparsed_bpm: Option<String> = None;
    let mut unparsed_key: Option<String> = None;

    for (field, std_key, value) in &raw {
        // MP4's `ilst` reader leaves the literal key empty for the atoms it
        // recognises, so name those from their standard key instead.
        let field = &field_label(field, *std_key);
        let norm = normalize_field(field);
        let value = elide(value);
        if value.is_empty() {
            continue;
        }

        // symphonia maps ID3v2 TBPM, Vorbis `bpm` and the MP4 `tmpo` atom onto
        // the standard BPM key, so honour that as well as the literal names.
        let bpm_rank = if matches!(std_key, Some(StandardTagKey::Bpm)) {
            Some(0)
        } else {
            rank(&BPM_FIELDS, &norm)
        };
        if let Some(r) = bpm_rank {
            matched_tags.push(TagHit {
                field: field.clone(),
                value: value.clone(),
            });
            match parse_bpm(&value) {
                Some(bpm) => {
                    let reading = BpmReading {
                        bpm: (bpm * 1000.0).round() / 1000.0,
                        bpm_rounded: bpm.round() as u32,
                        source_field: field.clone(),
                        raw: value.clone(),
                    };
                    if best_bpm.as_ref().is_none_or(|(br, _)| r < *br) {
                        best_bpm = Some((r, reading));
                    }
                }
                None => {
                    if unparsed_bpm.is_none() {
                        unparsed_bpm = Some(value.clone());
                    }
                }
            }
            continue;
        }

        if let Some(r) = rank(&KEY_FIELDS, &norm) {
            matched_tags.push(TagHit {
                field: field.clone(),
                value: value.clone(),
            });
            match parse_key(&value) {
                Some(k) => {
                    if best_key.as_ref().is_none_or(|(br, _, _)| r < *br) {
                        best_key = Some((r, render_key(&value, field, k, opts.key_notation), k));
                    }
                }
                None => {
                    if unparsed_key.is_none() {
                        unparsed_key = Some(value.clone());
                    }
                }
            }
            continue;
        }

        if let Some(r) = rank(&ENERGY_FIELDS, &norm) {
            matched_tags.push(TagHit {
                field: field.clone(),
                value: value.clone(),
            });
            if best_energy.as_ref().is_none_or(|(br, _)| r < *br) {
                best_energy = Some((
                    r,
                    EnergyReading {
                        energy: value.clone(),
                        source_field: field.clone(),
                    },
                ));
            }
        }
    }

    let bpm = best_bpm.map(|(_, b)| b);
    let key = best_key.map(|(_, k, _)| k);
    let energy = best_energy.map(|(_, e)| e);

    if bpm.is_none() {
        if let Some(v) = unparsed_bpm {
            notes.push(format!(
                "a tempo tag was present but its value '{v}' is not a number"
            ));
        }
    }
    if key.is_none() {
        if let Some(v) = unparsed_key {
            if v.trim().eq_ignore_ascii_case("o") {
                notes.push(
                    "the key tag holds the ID3v2 'o' marker, which means the track was \
                     explicitly tagged as off-key"
                        .into(),
                );
            } else {
                notes.push(format!(
                    "a key tag was present but its value '{v}' is not a recognised key \
                     (expected a traditional name like Am, a Camelot code like 8A, or an \
                     Open Key code like 1m)"
                ));
            }
        }
    }

    let found = bpm.is_some() || key.is_some();
    if !found && notes.is_empty() {
        notes.push(
            "no tempo or key tags are stored in this file — tag it with a DJ library \
             manager or analyser first, then read it back here"
                .into(),
        );
    }

    let all_tags = opts.include_all_tags.then(|| {
        raw.iter()
            .map(|(field, std_key, value)| TagHit {
                field: field_label(field, *std_key),
                value: elide(value),
            })
            .collect()
    });

    Ok(Report {
        container,
        found,
        bpm,
        key,
        energy,
        matched_tags,
        all_tags,
        tag_count: raw.len(),
        notes,
    })
}

fn render_key(raw: &str, field: &str, k: Key, notation: KeyNotation) -> KeyReading {
    let standard = k.standard();
    let camelot = k.camelot();
    let open_key = k.open_key();
    let (display, standard, camelot, open_key) = match notation {
        KeyNotation::Standard => (standard.clone(), Some(standard), None, None),
        KeyNotation::Camelot => (camelot.clone(), None, Some(camelot), None),
        KeyNotation::OpenKey => (open_key.clone(), None, None, Some(open_key)),
        KeyNotation::All => (
            standard.clone(),
            Some(standard),
            Some(camelot),
            Some(open_key),
        ),
    };
    KeyReading {
        raw: raw.to_string(),
        source_field: field.to_string(),
        display,
        standard,
        camelot,
        open_key,
        tonic: k.tonic().to_string(),
        mode: if k.minor { "minor" } else { "major" }.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_of(s: &str) -> String {
        let k = parse_key(s).unwrap_or_else(|| panic!("failed to parse key {s:?}"));
        format!("{} / {} / {}", k.standard(), k.camelot(), k.open_key())
    }

    #[test]
    fn camelot_anchor_points_match_mixxx() {
        // The two anchors the whole wheel is derived from.
        assert_eq!(key_of("C"), "C major / 8B / 1d");
        assert_eq!(key_of("Am"), "A minor / 8A / 1m");
    }

    #[test]
    fn every_camelot_code_round_trips() {
        for number in 1..=12u8 {
            for minor in [false, true] {
                let k = Key::from_camelot(number, minor).expect("known camelot number");
                let code = k.camelot();
                assert_eq!(
                    code,
                    format!("{number}{}", if minor { "A" } else { "B" }),
                    "camelot code round-trip"
                );
                // Re-parsing the code, the traditional name and the Open Key
                // code must all land back on the same key.
                assert_eq!(parse_key(&code), Some(k));
                assert_eq!(parse_key(&k.open_key()), Some(k));
                assert_eq!(parse_key(&k.standard()), Some(k));
            }
        }
    }

    #[test]
    fn known_wheel_positions() {
        assert_eq!(key_of("F#m"), "F# minor / 11A / 4m");
        assert_eq!(key_of("Gbm"), "F# minor / 11A / 4m"); // enharmonic
        assert_eq!(key_of("B"), "B major / 1B / 6d");
        assert_eq!(key_of("Ebm"), "Eb minor / 2A / 7m");
        assert_eq!(key_of("Dm"), "D minor / 7A / 12m");
    }

    #[test]
    fn accepts_the_forms_real_taggers_write() {
        for (input, expect) in [
            ("A minor", "8A"),
            ("Amin", "8A"),
            ("A min", "8A"),
            ("a-minor", "8A"),
            ("AMoll", "8A"),
            ("8A", "8A"),
            ("08A", "8A"),
            ("1m", "8A"),
            ("C major", "8B"),
            ("Cmaj", "8B"),
            ("8B", "8B"),
            ("1d", "8B"),
            ("A\u{266D}m", "1A"), // Ab minor written with a flat sign
            ("F\u{266F}", "2B"),  // F# major written with a sharp sign
        ] {
            let k = parse_key(input).unwrap_or_else(|| panic!("failed on {input:?}"));
            assert_eq!(k.camelot(), expect, "input {input:?}");
        }
    }

    #[test]
    fn rejects_values_that_are_not_keys() {
        for bad in ["", "o", "13A", "0A", "H", "Z minor", "hello", "8Q", "1x"] {
            assert_eq!(parse_key(bad), None, "should reject {bad:?}");
        }
    }

    #[test]
    fn bpm_parsing_handles_real_world_values() {
        assert_eq!(parse_bpm("128"), Some(128.0));
        assert_eq!(parse_bpm(" 128.50 "), Some(128.5));
        assert_eq!(parse_bpm("128,5"), Some(128.5)); // comma decimal separator
        assert_eq!(parse_bpm("174.00\0"), Some(174.0));
        assert_eq!(parse_bpm(""), None);
        assert_eq!(parse_bpm("fast"), None);
        assert_eq!(parse_bpm("0"), None);
        assert_eq!(parse_bpm("1200"), None);
    }

    #[test]
    fn field_names_normalise_across_tag_systems() {
        assert_eq!(normalize_field("TXXX:INITIALKEY"), "INITIALKEY");
        assert_eq!(normalize_field("com.apple.iTunes:initialkey"), "INITIALKEY");
        assert_eq!(normalize_field("WM/InitialKey"), "INITIALKEY");
        assert_eq!(normalize_field("Initial Key"), "INITIALKEY");
        assert_eq!(normalize_field("TBPM"), "TBPM");
        assert_eq!(normalize_field("tmpo"), "TMPO");
        assert_eq!(normalize_field("TXXX:EnergyLevel"), "ENERGYLEVEL");
    }

    #[test]
    fn notation_option_selects_what_is_reported() {
        let k = parse_key("Am").unwrap();
        let all = render_key("Am", "TKEY", k, KeyNotation::All);
        assert_eq!(all.display, "A minor");
        assert_eq!(all.camelot.as_deref(), Some("8A"));
        assert_eq!(all.open_key.as_deref(), Some("1m"));

        let cam = render_key("Am", "TKEY", k, KeyNotation::Camelot);
        assert_eq!(cam.display, "8A");
        assert_eq!(cam.standard, None);
        assert_eq!(cam.open_key, None);

        let ok = render_key("Am", "TKEY", k, KeyNotation::OpenKey);
        assert_eq!(ok.display, "1m");
        assert_eq!(ok.camelot, None);
    }

    #[test]
    fn key_notation_values_parse_and_typos_error() {
        assert_eq!(KeyNotation::parse("all").unwrap(), KeyNotation::All);
        assert_eq!(KeyNotation::parse("camelot").unwrap(), KeyNotation::Camelot);
        assert_eq!(
            KeyNotation::parse("open-key").unwrap(),
            KeyNotation::OpenKey
        );
        assert_eq!(
            KeyNotation::parse("standard").unwrap(),
            KeyNotation::Standard
        );
        assert!(KeyNotation::parse("camalot").is_err());
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = read(&[], Options::default()).unwrap_err();
        assert!(err.contains("empty"), "got {err}");
    }

    #[test]
    fn unrecognised_container_is_an_error() {
        let err = read(b"not a media file at all, just text", Options::default()).unwrap_err();
        assert!(err.contains("unrecognised"), "got {err}");
    }

    #[test]
    fn container_sniffing() {
        assert_eq!(container_name(b"fLaC\0\0\0\x22"), "FLAC (native)");
        assert_eq!(container_name(b"OggS\0\x02"), "OGG");
        assert_eq!(container_name(b"ID3\x04\0\0\0\0\0\0"), "MP3 (MPEG audio)");
        assert_eq!(container_name(b"RIFF....WAVE"), "WAVE (RIFF)");
        assert_eq!(
            container_name(b"\0\0\0\x20ftypM4A "),
            "MP4 / M4A (ISO BMFF)"
        );
    }
}
