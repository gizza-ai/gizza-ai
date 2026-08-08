//! guitar-tab-to-midi core — turn ASCII guitar/bass tablature into a Standard
//! MIDI File. Pure Rust (`midly` for the SMF writer); no wafer/wasm-bindgen
//! deps, so the same logic runs in chat, the CLI and the browser page.
//!
//! The model is the classic one: a tab stave is a grid of character columns,
//! one line per string. A run of digits on a string is a fret number, and the
//! pitch it sounds is `open_string + capo + fret + transpose`. Columns advance
//! time by one step (`note_duration`); a column that is a bar line `|` on every
//! string consumes no time, so bar lines never shift the beat.

use midly::num::{u4, u7, u15, u24, u28};
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// Ticks per quarter note in the produced file. 480 divides cleanly by 2/3/4,
/// so every supported step length lands on an exact integer tick.
pub const PPQ: u16 = 480;
/// Highest fret number accepted. Real instruments top out around 24; the extra
/// headroom covers tabs that notate harmonics high on the neck.
pub const MAX_FRET: u32 = 36;
/// Reject pathologically large input before doing any work.
pub const MAX_INPUT_BYTES: usize = 1024 * 1024;
/// Cap the note count so a runaway tab can't produce an unusable file.
pub const MAX_NOTES: usize = 20_000;

/// Every knob the conversion exposes. Strings are the raw parameter values from
/// the descriptor/CLI/page; they are validated here so all three surfaces give
/// identical errors.
#[derive(Debug, Clone)]
pub struct Options {
    /// Tuning preset name, or `"auto"` to pick one from the string count.
    pub tuning: String,
    /// Explicit tuning (comma/space separated pitch names or MIDI numbers,
    /// bottom tab line first). Overrides `tuning` when non-empty.
    pub custom_tuning: String,
    /// `"high-on-top"` (default) or `"low-on-top"`.
    pub string_order: String,
    /// Capo position in frets, added to every string.
    pub capo: i32,
    /// Extra transposition in semitones.
    pub transpose: i32,
    /// Tempo in beats (quarter notes) per minute.
    pub tempo: f64,
    /// Musical length of one tab column.
    pub note_duration: String,
    /// `"columns"` (every column is a step) or `"events"` (only columns that
    /// contain a note are steps).
    pub timing: String,
    /// `"until-next"` (a string rings until its next note) or `"step"`.
    pub sustain: String,
    /// Note-on velocity, 1–127.
    pub velocity: u8,
    /// General MIDI instrument name (see [`instrument_program`]).
    pub instrument: String,
    /// Render `x`/`X` mutes as short muted notes instead of skipping them.
    pub muted_notes: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            tuning: "auto".into(),
            custom_tuning: String::new(),
            string_order: "high-on-top".into(),
            capo: 0,
            transpose: 0,
            tempo: 120.0,
            note_duration: "eighth".into(),
            timing: "columns".into(),
            sustain: "until-next".into(),
            velocity: 96,
            instrument: "acoustic-guitar-steel".into(),
            muted_notes: false,
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
    /// Number of tab staves (systems) found and concatenated.
    pub staves: usize,
    /// String lines per stave.
    pub strings: usize,
    /// Total time steps across all staves.
    pub steps: usize,
    /// Playing time in seconds at the requested tempo.
    pub seconds: f64,
    /// Resolved tuning as MIDI note numbers, bottom tab line first.
    pub tuning: Vec<i32>,
    /// Human label for the resolved tuning (e.g. `standard`, `custom`).
    pub tuning_name: String,
    /// Lowest and highest MIDI pitch written (only meaningful when `notes > 0`).
    pub lowest: u8,
    pub highest: u8,
}

impl Conversion {
    /// Resolved tuning rendered as scientific pitch names, bottom line first.
    pub fn tuning_names(&self) -> String {
        self.tuning
            .iter()
            .map(|m| midi_to_name(*m))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// One-line human summary, shared by the chat envelope and the page.
    pub fn summary(&self) -> String {
        format!(
            "{} note(s) from {} stave(s) × {} string(s), {} tuning ({}) — {:.1}s of MIDI ({} bytes)",
            self.notes,
            self.staves,
            self.strings,
            self.tuning_name,
            self.tuning_names(),
            self.seconds,
            self.midi.len()
        )
    }
}

// ---------------------------------------------------------------------------
// Vocabularies
// ---------------------------------------------------------------------------

/// Tuning presets, listed bottom tab line first (i.e. thickest string first for
/// guitar/bass; for ukulele that is the re-entrant low-G string).
const TUNINGS: &[(&str, &[i32])] = &[
    ("standard", &[40, 45, 50, 55, 59, 64]),           // E2 A2 D3 G3 B3 E4
    ("drop-d", &[38, 45, 50, 55, 59, 64]),             // D2 A2 D3 G3 B3 E4
    ("half-step-down", &[39, 44, 49, 54, 58, 63]),     // Eb2 Ab2 Db3 Gb3 Bb3 Eb4
    ("full-step-down", &[38, 43, 48, 53, 57, 62]),     // D2 G2 C3 F3 A3 D4
    ("drop-c", &[36, 43, 48, 53, 57, 62]),             // C2 G2 C3 F3 A3 D4
    ("open-g", &[38, 43, 50, 55, 59, 62]),             // D2 G2 D3 G3 B3 D4
    ("open-d", &[38, 45, 50, 54, 57, 62]),             // D2 A2 D3 F#3 A3 D4
    ("dadgad", &[38, 45, 50, 55, 57, 62]),             // D2 A2 D3 G3 A3 D4
    ("seven-string", &[35, 40, 45, 50, 55, 59, 64]),   // B1 + standard
    ("eight-string", &[30, 35, 40, 45, 50, 55, 59, 64]), // F#1 B1 + standard
    ("bass-standard", &[28, 33, 38, 43]),              // E1 A1 D2 G2
    ("bass-5-string", &[23, 28, 33, 38, 43]),          // B0 E1 A1 D2 G2
    ("ukulele", &[67, 60, 64, 69]),                    // gCEA (re-entrant)
];

/// General MIDI programs offered, by friendly name (0-based program numbers).
const INSTRUMENTS: &[(&str, u8)] = &[
    ("acoustic-guitar-nylon", 24),
    ("acoustic-guitar-steel", 25),
    ("electric-guitar-jazz", 26),
    ("electric-guitar-clean", 27),
    ("electric-guitar-muted", 28),
    ("overdriven-guitar", 29),
    ("distortion-guitar", 30),
    ("guitar-harmonics", 31),
    ("acoustic-bass", 32),
    ("electric-bass-finger", 33),
    ("electric-bass-pick", 34),
    ("fretless-bass", 35),
    ("acoustic-grand-piano", 0),
];

/// Step length names → ticks, at [`PPQ`].
const DURATIONS: &[(&str, u32)] = &[
    ("whole", 1920),
    ("half", 960),
    ("quarter", 480),
    ("eighth", 240),
    ("sixteenth", 120),
    ("thirty-second", 60),
];

/// The tuning `auto` picks for a given string count.
fn auto_tuning(strings: usize) -> Option<&'static str> {
    match strings {
        4 => Some("bass-standard"),
        5 => Some("bass-5-string"),
        6 => Some("standard"),
        7 => Some("seven-string"),
        8 => Some("eight-string"),
        _ => None,
    }
}

/// General MIDI program number for an instrument name.
pub fn instrument_program(name: &str) -> Result<u8, String> {
    let key = name.trim().to_ascii_lowercase();
    INSTRUMENTS
        .iter()
        .find(|(n, _)| *n == key)
        .map(|(_, p)| *p)
        .ok_or_else(|| {
            format!(
                "unknown instrument '{}' (expected one of: {})",
                name.trim(),
                INSTRUMENTS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
            )
        })
}

/// Ticks per tab column for a step-length name.
pub fn duration_ticks(name: &str) -> Result<u32, String> {
    let key = name.trim().to_ascii_lowercase();
    DURATIONS
        .iter()
        .find(|(n, _)| *n == key)
        .map(|(_, t)| *t)
        .ok_or_else(|| {
            format!(
                "unknown note_duration '{}' (expected one of: {})",
                name.trim(),
                DURATIONS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
            )
        })
}

/// Parse a numeric form field. An empty value means "use the default" — the
/// page sends empty strings for untouched number boxes, and every surface must
/// agree on what that means.
pub fn parse_field<T: std::str::FromStr>(name: &str, raw: &str, default: T) -> Result<T, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(default);
    }
    t.parse::<T>()
        .map_err(|_| format!("{name} must be a number, got '{t}'"))
}

/// Parse a checkbox value positive-truthy; an empty value keeps `default`.
pub fn truthy(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

/// MIDI number → scientific pitch name (middle C = C4 = 60).
pub fn midi_to_name(m: i32) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let idx = m.rem_euclid(12) as usize;
    let octave = m.div_euclid(12) - 1;
    format!("{}{}", NAMES[idx], octave)
}

/// Scientific pitch name (`E2`, `F#3`, `Bb2`) or a bare MIDI number → MIDI number.
pub fn parse_pitch(s: &str) -> Result<i32, String> {
    let t = s.trim();
    if t.is_empty() {
        return Err("empty note name".into());
    }
    if t.chars().all(|c| c.is_ascii_digit()) {
        let n: i32 = t
            .parse()
            .map_err(|_| format!("'{t}' is not a MIDI note number"))?;
        if !(0..=127).contains(&n) {
            return Err(format!("MIDI note number {n} is outside 0–127"));
        }
        return Ok(n);
    }
    let mut chars = t.chars();
    let letter = chars.next().unwrap().to_ascii_uppercase();
    let base = match letter {
        'C' => 0,
        'D' => 2,
        'E' => 4,
        'F' => 5,
        'G' => 7,
        'A' => 9,
        'B' => 11,
        _ => {
            return Err(format!(
                "'{t}' is not a note name — expected a letter A–G, e.g. E2, F#3, Bb2"
            ))
        }
    };
    let rest: String = chars.collect();
    let mut rest = rest.as_str();
    let mut accidental = 0i32;
    loop {
        if let Some(r) = rest.strip_prefix('#').or_else(|| rest.strip_prefix('♯')) {
            accidental += 1;
            rest = r;
        } else if let Some(r) = rest.strip_prefix('b').or_else(|| rest.strip_prefix('♭')) {
            accidental -= 1;
            rest = r;
        } else {
            break;
        }
    }
    let octave: i32 = rest
        .trim()
        .parse()
        .map_err(|_| format!("'{t}' is missing an octave number — write e.g. E2, F#3, Bb2"))?;
    let midi = (octave + 1) * 12 + base + accidental;
    if !(0..=127).contains(&midi) {
        return Err(format!("note '{t}' is outside the MIDI range (0–127)"));
    }
    Ok(midi)
}

/// Resolve the tuning to use, bottom tab line first.
///
/// `custom` wins whenever it is non-empty; otherwise the preset `name` is used,
/// with `auto` picking from `strings`.
pub fn resolve_tuning(
    name: &str,
    custom: &str,
    strings: usize,
) -> Result<(Vec<i32>, String), String> {
    if !custom.trim().is_empty() {
        let parts: Vec<&str> = custom
            .split(|c: char| c == ',' || c.is_whitespace())
            .filter(|p| !p.is_empty())
            .collect();
        let pitches = parts
            .iter()
            .map(|p| parse_pitch(p))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("custom_tuning: {e}"))?;
        if pitches.len() != strings {
            return Err(format!(
                "custom_tuning lists {} string(s) but the tab has {} string line(s) — list one note per line, lowest tab line first",
                pitches.len(),
                strings
            ));
        }
        return Ok((pitches, "custom".into()));
    }

    let key = name.trim().to_ascii_lowercase();
    let key = if key.is_empty() { "auto".to_string() } else { key };
    let resolved = if key == "auto" {
        auto_tuning(strings).ok_or_else(|| {
            format!(
                "no automatic tuning for a {strings}-line stave (auto covers 4–8 lines) — set `tuning` or `custom_tuning`"
            )
        })?
    } else {
        TUNINGS
            .iter()
            .find(|(n, _)| *n == key)
            .map(|(n, _)| *n)
            .ok_or_else(|| {
                format!(
                    "unknown tuning '{}' (expected auto or one of: {})",
                    name.trim(),
                    TUNINGS.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
                )
            })?
    };
    let pitches = TUNINGS
        .iter()
        .find(|(n, _)| *n == resolved)
        .map(|(_, p)| p.to_vec())
        .expect("resolved tuning exists");
    if pitches.len() != strings {
        return Err(format!(
            "tuning '{resolved}' has {} string(s) but the tab has {strings} string line(s) — pick a matching tuning or use `custom_tuning`",
            pitches.len()
        ));
    }
    Ok((pitches, resolved.to_string()))
}

// ---------------------------------------------------------------------------
// Tab parsing
// ---------------------------------------------------------------------------

/// One tab line: its content with any `e|` style string label stripped, plus the
/// 1-based line number in the original input (used in error messages).
#[derive(Debug)]
struct TabLine {
    content: Vec<char>,
    lineno: usize,
}

/// Strip a leading string label such as `e|`, `Bb|`, `G |` or a bare leading `|`.
fn strip_label(line: &str) -> &str {
    let bytes = line.as_bytes();
    // Look for a '|' or ':' separator within the first few characters.
    for (i, b) in bytes.iter().enumerate().take(5) {
        if *b == b'|' || *b == b':' {
            let prefix_ok = bytes[..i]
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || *c == b'#' || *c == b' ' || *c == b'\t');
            if prefix_ok {
                return &line[i + 1..];
            }
            break;
        }
    }
    line
}

/// Characters a tab line is allowed to be made of: rests (`-`, `=`), bar lines,
/// fret digits, and the usual articulation marks (hammer-on, pull-off, bend,
/// release, slide, tap, mute, slides, vibrato, ghost notes…).
fn is_tab_char(c: char) -> bool {
    c.is_ascii_digit()
        || matches!(
            c,
            '-' | '=' | '|' | ':' | '/' | '\\' | '~' | '^' | '*' | '.' | '(' | ')' | '<' | '>'
                | '+' | '_' | '\''
        )
        || matches!(
            c.to_ascii_lowercase(),
            'h' | 'p' | 'b' | 'r' | 's' | 't' | 'x' | 'v'
        )
}

/// Is this a tab line? A tab line needs a few rest dashes and is built almost
/// entirely from tab characters; prose has letters that never appear in a tab.
/// (A dash-ratio test alone is wrong in both directions — it rejects dense tabs
/// like `|5-7-9-12|` and accepts hyphen-heavy prose.)
fn is_tab_line(content: &str) -> bool {
    let non_space: Vec<char> = content.chars().filter(|c| !c.is_whitespace()).collect();
    let dashes = non_space.iter().filter(|c| **c == '-' || **c == '=').count();
    let tabbish = non_space.iter().filter(|c| is_tab_char(**c)).count();
    non_space.len() >= 4 && dashes >= 3 && tabbish * 10 >= non_space.len() * 9
}

/// Split the input into staves (groups of consecutive tab lines). Staves with no
/// playable characters at all (a plain rule of dashes) are dropped.
fn split_staves(tab: &str) -> Vec<Vec<TabLine>> {
    let mut staves: Vec<Vec<TabLine>> = Vec::new();
    let mut current: Vec<TabLine> = Vec::new();
    for (idx, raw) in tab.lines().enumerate() {
        let content = strip_label(raw);
        if is_tab_line(content) {
            current.push(TabLine {
                content: content.chars().collect(),
                lineno: idx + 1,
            });
        } else if !current.is_empty() {
            staves.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        staves.push(current);
    }
    staves.retain(|s| {
        s.iter()
            .any(|l| l.content.iter().any(|c| c.is_ascii_digit() || *c == 'x' || *c == 'X'))
    });
    staves
}

/// A note onset found in the grid, before pitches are resolved.
#[derive(Debug)]
struct Onset {
    step: usize,
    string: usize,
    fret: u32,
    muted: bool,
    lineno: usize,
    col: usize,
}

// ---------------------------------------------------------------------------
// Conversion
// ---------------------------------------------------------------------------

/// Convert ASCII tablature into a Standard MIDI File.
pub fn convert(tab: &str, opts: &Options) -> Result<Conversion, String> {
    if tab.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "tab is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            tab.len()
        ));
    }
    if tab.trim().is_empty() {
        return Err("tab is empty — paste some ASCII tablature (lines like `e|---3---5---|`)".into());
    }

    // --- validate the simple options up front so errors are precise ---
    let step_ticks = duration_ticks(&opts.note_duration)?;
    let program = instrument_program(&opts.instrument)?;
    if !(20.0..=400.0).contains(&opts.tempo) || !opts.tempo.is_finite() {
        return Err(format!(
            "tempo {} is outside 20–400 BPM",
            opts.tempo
        ));
    }
    if !(0..=12).contains(&opts.capo) {
        return Err(format!("capo {} is outside 0–12 frets", opts.capo));
    }
    if !(-24..=24).contains(&opts.transpose) {
        return Err(format!(
            "transpose {} is outside -24 to +24 semitones",
            opts.transpose
        ));
    }
    if !(1..=127).contains(&opts.velocity) {
        return Err(format!("velocity {} is outside 1–127", opts.velocity));
    }
    let high_on_top = match opts.string_order.trim().to_ascii_lowercase().as_str() {
        "" | "high-on-top" => true,
        "low-on-top" => false,
        other => {
            return Err(format!(
                "unknown string_order '{other}' (expected high-on-top or low-on-top)"
            ))
        }
    };
    let events_timing = match opts.timing.trim().to_ascii_lowercase().as_str() {
        "" | "columns" => false,
        "events" => true,
        other => {
            return Err(format!(
                "unknown timing '{other}' (expected columns or events)"
            ))
        }
    };
    let sustain_until_next = match opts.sustain.trim().to_ascii_lowercase().as_str() {
        "" | "until-next" => true,
        "step" => false,
        other => {
            return Err(format!(
                "unknown sustain '{other}' (expected until-next or step)"
            ))
        }
    };

    // --- parse the grid ---
    let staves = split_staves(tab);
    if staves.is_empty() {
        return Err("no tablature found — a tab line looks like `e|---3---5---|`: mostly dashes, with fret numbers on it".into());
    }
    let strings = staves[0].len();
    for (i, stave) in staves.iter().enumerate().skip(1) {
        if stave.len() != strings {
            return Err(format!(
                "stave {} (line {}) has {} string line(s) but the first stave has {strings} — every stave must use the same number of strings",
                i + 1,
                stave[0].lineno,
                stave.len()
            ));
        }
    }

    let (tuning, tuning_name) = resolve_tuning(&opts.tuning, &opts.custom_tuning, strings)?;

    let mut onsets: Vec<Onset> = Vec::new();
    let mut step = 0usize;
    for stave in &staves {
        let width = stave.iter().map(|l| l.content.len()).max().unwrap_or(0);
        for col in 0..width {
            let present: Vec<char> = stave
                .iter()
                .filter_map(|l| l.content.get(col).copied())
                .collect();
            if present.is_empty() || present.iter().all(|c| c.is_whitespace()) {
                continue; // padding — no time passes
            }
            if present.iter().all(|c| *c == '|') {
                continue; // bar line — no time passes
            }

            let mut col_onsets: Vec<Onset> = Vec::new();
            for (li, line) in stave.iter().enumerate() {
                let ch = match line.content.get(col) {
                    Some(c) => *c,
                    None => continue,
                };
                // Map the visual line index to a string index into `tuning`
                // (which is stored bottom line first).
                let string = if high_on_top { strings - 1 - li } else { li };
                if ch.is_ascii_digit() {
                    // Only the FIRST digit of a run starts a note; `12` is fret 12.
                    if col > 0 && line.content[col - 1].is_ascii_digit() {
                        continue;
                    }
                    let mut digits = String::new();
                    let mut k = col;
                    while let Some(d) = line.content.get(k) {
                        if d.is_ascii_digit() {
                            digits.push(*d);
                            k += 1;
                        } else {
                            break;
                        }
                    }
                    let fret: u32 = digits.parse().map_err(|_| {
                        format!("fret '{digits}' on line {} column {} is not a number", line.lineno, col + 1)
                    })?;
                    if fret > MAX_FRET {
                        return Err(format!(
                            "fret {fret} on line {} column {} is above the {MAX_FRET}-fret limit",
                            line.lineno,
                            col + 1
                        ));
                    }
                    col_onsets.push(Onset {
                        step: 0,
                        string,
                        fret,
                        muted: false,
                        lineno: line.lineno,
                        col: col + 1,
                    });
                } else if (ch == 'x' || ch == 'X') && opts.muted_notes {
                    col_onsets.push(Onset {
                        step: 0,
                        string,
                        fret: 0,
                        muted: true,
                        lineno: line.lineno,
                        col: col + 1,
                    });
                }
            }

            if events_timing && col_onsets.is_empty() {
                continue; // only note columns advance time in `events` mode
            }
            for mut o in col_onsets {
                o.step = step;
                onsets.push(o);
            }
            step += 1;
            if onsets.len() > MAX_NOTES {
                return Err(format!(
                    "tab produces more than {MAX_NOTES} notes — split it into smaller sections"
                ));
            }
        }
    }
    let total_steps = step.max(1);

    if onsets.is_empty() {
        return Err(
            "no fret numbers found in the tablature — every column was a rest, bar line or mute"
                .into(),
        );
    }

    // --- resolve pitches + note lengths ---
    // Per string, the next onset determines where a ringing note stops.
    let mut per_string: Vec<Vec<usize>> = vec![Vec::new(); strings];
    for (i, o) in onsets.iter().enumerate() {
        per_string[o.string].push(i);
    }
    for list in per_string.iter_mut() {
        list.sort_by_key(|i| onsets[*i].step);
    }

    let mut notes: Vec<(u32, u32, u8)> = Vec::new(); // (on_tick, off_tick, pitch)
    let mut lowest = 127i32;
    let mut highest = 0i32;
    for list in &per_string {
        for (pos, &i) in list.iter().enumerate() {
            let o = &onsets[i];
            let pitch = tuning[o.string] + opts.capo + o.fret as i32 + opts.transpose;
            if !(0..=127).contains(&pitch) {
                return Err(format!(
                    "fret {} on line {} column {} sounds MIDI note {pitch}, outside the playable 0–127 range (check capo/transpose/tuning)",
                    o.fret, o.lineno, o.col
                ));
            }
            let end_step = if o.muted || !sustain_until_next {
                o.step + 1
            } else {
                list.get(pos + 1)
                    .map(|&n| onsets[n].step)
                    .unwrap_or(total_steps)
                    .max(o.step + 1)
            };
            let on = o.step as u32 * step_ticks;
            let off = end_step as u32 * step_ticks;
            notes.push((on, off, pitch as u8));
            lowest = lowest.min(pitch);
            highest = highest.max(pitch);
        }
    }

    // --- write the SMF ---
    let midi = write_smf(&notes, opts.tempo, program, opts.velocity)?;
    let total_ticks = notes.iter().map(|(_, off, _)| *off).max().unwrap_or(0);
    let seconds = total_ticks as f64 / PPQ as f64 * (60.0 / opts.tempo);

    Ok(Conversion {
        midi,
        notes: notes.len(),
        staves: staves.len(),
        strings,
        steps: total_steps,
        seconds,
        tuning,
        tuning_name,
        lowest: lowest as u8,
        highest: highest as u8,
    })
}

/// Serialize the note list as a format-0 Standard MIDI File.
fn write_smf(
    notes: &[(u32, u32, u8)],
    tempo_bpm: f64,
    program: u8,
    velocity: u8,
) -> Result<Vec<u8>, String> {
    // (tick, order, kind) — note-offs sort before note-ons at the same tick so a
    // repeated pitch retriggers instead of cancelling itself.
    let mut events: Vec<(u32, u8, TrackEventKind<'static>)> = Vec::new();
    let micros_per_beat = (60_000_000.0 / tempo_bpm).round().clamp(1.0, 16_777_215.0) as u32;

    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::TrackName(&b"Guitar Tab"[..])),
    ));
    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::Tempo(u24::from(micros_per_beat))),
    ));
    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
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
                    vel: u7::from(velocity),
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

    let end_tick = events.last().map(|(t, _, _)| *t).unwrap_or(0);
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
        delta: u28::from(end_tick - prev),
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

    const SIMPLE: &str = "\
e|-----------------|
B|-----------------|
G|-----------------|
D|-----------------|
A|--2--------------|
E|-----3-----------|";

    #[test]
    fn happy_path_maps_frets_to_pitches() {
        let c = convert(SIMPLE, &Options::default()).expect("converts");
        assert_eq!(c.strings, 6);
        assert_eq!(c.staves, 1);
        assert_eq!(c.tuning_name, "standard");
        assert_eq!(c.notes, 2);
        // A2 (45) + fret 2 = B2 (47); E2 (40) + fret 3 = G2 (43).
        let ons: Vec<(u32, u8)> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(t, p, _)| (t, p))
            .collect();
        assert_eq!(ons, vec![(2 * 240, 47), (5 * 240, 43)]);
        assert_eq!(c.lowest, 43);
        assert_eq!(c.highest, 47);
        assert!(c.midi.starts_with(b"MThd"));
    }

    #[test]
    fn header_encodes_ppq_and_format_zero() {
        let c = convert(SIMPLE, &Options::default()).unwrap();
        let smf = Smf::parse(&c.midi).unwrap();
        assert!(matches!(smf.header.format, Format::SingleTrack));
        assert!(matches!(smf.header.timing, Timing::Metrical(t) if t.as_int() == PPQ));
        assert_eq!(smf.tracks.len(), 1);
    }

    #[test]
    fn double_digit_frets_are_one_note() {
        let tab = "e|--12--|\nB|------|\nG|------|\nD|------|\nA|------|\nE|------|";
        let c = convert(tab, &Options::default()).unwrap();
        assert_eq!(c.notes, 1);
        // High E4 (64) + 12 = E5 (76), starting on the FIRST digit's column.
        let ons: Vec<(u32, u8)> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(t, p, _)| (t, p))
            .collect();
        assert_eq!(ons, vec![(2 * 240, 76)]);
    }

    #[test]
    fn bar_lines_do_not_consume_time() {
        // Two identical staves; the second has extra bar lines. The note onset
        // step must be the same in both.
        let plain = "e|---3----|\nB|--------|\nG|--------|\nD|--------|\nA|--------|\nE|--------|";
        let barred = "e|--|-3----|\nB|--|------|\nG|--|------|\nD|--|------|\nA|--|------|\nE|--|------|";
        let a = convert(plain, &Options::default()).unwrap();
        let b = convert(barred, &Options::default()).unwrap();
        let first = |c: &Conversion| read_notes(&c.midi).into_iter().find(|(_, _, on)| *on).unwrap();
        assert_eq!(first(&a).0, first(&b).0);
    }

    #[test]
    fn chord_columns_start_together() {
        let tab = "e|--0--|\nB|--1--|\nG|--0--|\nD|--2--|\nA|--3--|\nE|-----|";
        let c = convert(tab, &Options::default()).unwrap();
        assert_eq!(c.notes, 5);
        let ons: Vec<(u32, u8)> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(t, p, _)| (t, p))
            .collect();
        // A C-major shape: every note starts on the same tick.
        assert!(ons.iter().all(|(t, _)| *t == 2 * 240));
        let mut pitches: Vec<u8> = ons.iter().map(|(_, p)| *p).collect();
        pitches.sort_unstable();
        assert_eq!(pitches, vec![48, 52, 55, 60, 64]); // C3 E3 G3 C4 E4
    }

    #[test]
    fn capo_and_transpose_shift_pitch() {
        let mut opts = Options::default();
        opts.capo = 2;
        opts.transpose = -12;
        let c = convert(SIMPLE, &opts).unwrap();
        let pitches: Vec<u8> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(_, p, _)| p)
            .collect();
        assert_eq!(pitches, vec![47 + 2 - 12, 43 + 2 - 12]);
    }

    #[test]
    fn auto_tuning_picks_bass_for_four_lines() {
        let tab = "G|-------|\nD|-------|\nA|--5----|\nE|-------|";
        let c = convert(tab, &Options::default()).unwrap();
        assert_eq!(c.tuning_name, "bass-standard");
        // A1 (33) + 5 = D2 (38).
        let pitches: Vec<u8> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(_, p, _)| p)
            .collect();
        assert_eq!(pitches, vec![38]);
    }

    #[test]
    fn custom_tuning_overrides_the_preset() {
        let mut opts = Options::default();
        opts.tuning = "standard".into();
        opts.custom_tuning = "D2,A2,D3,G3,B3,E4".into(); // drop D by hand
        let tab = "e|-------|\nB|-------|\nG|-------|\nD|-------|\nA|-------|\nE|--0----|";
        let c = convert(tab, &opts).unwrap();
        assert_eq!(c.tuning_name, "custom");
        let pitches: Vec<u8> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(_, p, _)| p)
            .collect();
        assert_eq!(pitches, vec![38]); // D2, not E2
    }

    #[test]
    fn low_on_top_reverses_the_string_mapping() {
        let mut opts = Options::default();
        opts.string_order = "low-on-top".into();
        let tab = "E|--0----|\nA|-------|\nD|-------|\nG|-------|\nB|-------|\ne|-------|";
        let c = convert(tab, &opts).unwrap();
        let pitches: Vec<u8> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(_, p, _)| p)
            .collect();
        assert_eq!(pitches, vec![40]); // low E2, because the low string is on top
    }

    #[test]
    fn events_timing_evenly_spaces_onsets() {
        let mut opts = Options::default();
        opts.timing = "events".into();
        let tab = "e|--3-------7--|\nB|-------------|\nG|-------------|\nD|-------------|\nA|-------------|\nE|-------------|";
        let c = convert(tab, &opts).unwrap();
        let ticks: Vec<u32> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(t, _, _)| t)
            .collect();
        assert_eq!(ticks, vec![0, 240]); // back to back, spacing ignored
    }

    #[test]
    fn sustain_step_shortens_notes_to_one_step() {
        let mut opts = Options::default();
        opts.sustain = "step".into();
        let tab = "e|--3----------|\nB|-------------|\nG|-------------|\nD|-------------|\nA|-------------|\nE|-------------|";
        let c = convert(tab, &opts).unwrap();
        let evs = read_notes(&c.midi);
        let on = evs.iter().find(|(_, _, o)| *o).unwrap();
        let off = evs.iter().find(|(_, _, o)| !*o).unwrap();
        assert_eq!(off.0 - on.0, 240); // exactly one eighth-note step
    }

    #[test]
    fn sustain_until_next_rings_to_the_following_note() {
        let tab = "e|--3-----5----|\nB|-------------|\nG|-------------|\nD|-------------|\nA|-------------|\nE|-------------|";
        let c = convert(tab, &Options::default()).unwrap();
        let evs = read_notes(&c.midi);
        let on = evs.iter().find(|(_, _, o)| *o).unwrap();
        let off = evs.iter().find(|(_, _, o)| !*o).unwrap();
        assert_eq!(off.0 - on.0, 6 * 240); // held from column 2 to column 8
    }

    #[test]
    fn mutes_are_skipped_unless_enabled() {
        let tab = "e|--x--3--|\nB|--x-----|\nG|--------|\nD|--------|\nA|--------|\nE|--------|";
        let off = convert(tab, &Options::default()).unwrap();
        assert_eq!(off.notes, 1);
        let mut opts = Options::default();
        opts.muted_notes = true;
        let on = convert(tab, &opts).unwrap();
        assert_eq!(on.notes, 3);
    }

    #[test]
    fn technique_letters_do_not_break_the_frets_around_them() {
        let tab = "e|--5h7p5--|\nB|---------|\nG|---------|\nD|---------|\nA|---------|\nE|---------|";
        let c = convert(tab, &Options::default()).unwrap();
        assert_eq!(c.notes, 3);
        let pitches: Vec<u8> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(_, p, _)| p)
            .collect();
        assert_eq!(pitches, vec![69, 71, 69]); // A4, B4, A4 on the high E string
    }

    #[test]
    fn multiple_staves_play_in_sequence() {
        let tab = "\
e|--3--|
B|-----|
G|-----|
D|-----|
A|-----|
E|-----|

e|--5--|
B|-----|
G|-----|
D|-----|
A|-----|
E|-----|";
        let c = convert(tab, &Options::default()).unwrap();
        assert_eq!(c.staves, 2);
        let ons: Vec<(u32, u8)> = read_notes(&c.midi)
            .into_iter()
            .filter(|(_, _, on)| *on)
            .map(|(t, p, _)| (t, p))
            .collect();
        assert_eq!(ons, vec![(2 * 240, 67), (7 * 240, 69)]);
    }

    #[test]
    fn prose_lines_between_staves_are_ignored() {
        let tab = "Verse 1 — play it slowly\ne|--3--|\nB|-----|\nG|-----|\nD|-----|\nA|-----|\nE|-----|\nrepeat x4";
        let c = convert(tab, &Options::default()).unwrap();
        assert_eq!(c.strings, 6);
        assert_eq!(c.notes, 1);
    }

    #[test]
    fn tempo_and_instrument_reach_the_file() {
        let mut opts = Options::default();
        opts.tempo = 90.0;
        opts.instrument = "distortion-guitar".into();
        let c = convert(SIMPLE, &opts).unwrap();
        let smf = Smf::parse(&c.midi).unwrap();
        let mut tempo = None;
        let mut program = None;
        for ev in &smf.tracks[0] {
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => tempo = Some(t.as_int()),
                TrackEventKind::Midi {
                    message: MidiMessage::ProgramChange { program: p },
                    ..
                } => program = Some(p.as_int()),
                _ => {}
            }
        }
        assert_eq!(tempo, Some(666_667)); // 60e6 / 90
        assert_eq!(program, Some(30));
        // SIMPLE is 17 time columns; both notes ring to the end, so the file is
        // 17 eighth notes = 8.5 beats long, which at 90 BPM is 5⅔ seconds.
        assert_eq!(c.steps, 17);
        assert!((c.seconds - (17.0 * 240.0 / PPQ as f64) * (60.0 / 90.0)).abs() < 1e-6);
    }

    #[test]
    fn note_duration_scales_the_step() {
        let mut opts = Options::default();
        opts.note_duration = "sixteenth".into();
        let c = convert(SIMPLE, &opts).unwrap();
        let first = read_notes(&c.midi).into_iter().find(|(_, _, on)| *on).unwrap();
        assert_eq!(first.0, 2 * 120);
    }

    // --- error paths ---

    #[test]
    fn empty_input_is_an_error() {
        let err = convert("   \n\n", &Options::default()).unwrap_err();
        assert!(err.contains("tab is empty"), "{err}");
    }

    #[test]
    fn prose_only_input_reports_no_tablature() {
        let err = convert("just some words\nand more words", &Options::default()).unwrap_err();
        assert!(err.contains("no tablature found"), "{err}");
    }

    #[test]
    fn fret_above_the_limit_names_line_and_column() {
        let tab = "e|--99--|\nB|------|\nG|------|\nD|------|\nA|------|\nE|------|";
        let err = convert(tab, &Options::default()).unwrap_err();
        assert!(err.contains("fret 99"), "{err}");
        assert!(err.contains("line 1"), "{err}");
        assert!(err.contains("column 3"), "{err}");
    }

    #[test]
    fn tuning_string_count_mismatch_is_explained() {
        let mut opts = Options::default();
        opts.tuning = "bass-standard".into();
        let err = convert(SIMPLE, &opts).unwrap_err();
        assert!(err.contains("bass-standard"), "{err}");
        assert!(err.contains("4 string(s)"), "{err}");
    }

    #[test]
    fn custom_tuning_length_mismatch_is_explained() {
        let mut opts = Options::default();
        opts.custom_tuning = "E2,A2,D3".into();
        let err = convert(SIMPLE, &opts).unwrap_err();
        assert!(err.contains("custom_tuning lists 3"), "{err}");
    }

    #[test]
    fn unknown_enum_values_list_the_choices() {
        let mut opts = Options::default();
        opts.note_duration = "crotchet".into();
        let err = convert(SIMPLE, &opts).unwrap_err();
        assert!(err.contains("unknown note_duration"), "{err}");
        assert!(err.contains("sixteenth"), "{err}");

        let mut opts = Options::default();
        opts.instrument = "banjo".into();
        let err = convert(SIMPLE, &opts).unwrap_err();
        assert!(err.contains("unknown instrument"), "{err}");
    }

    #[test]
    fn out_of_range_pitch_is_rejected_with_context() {
        let mut opts = Options::default();
        opts.transpose = -24;
        opts.capo = 0;
        // 5 lines → bass-5-string, whose lowest string is B0 = MIDI 23; two
        // octaves down is MIDI -1, below the playable range.
        let tab = "G|-------|\nD|-------|\nA|-------|\nE|-------|\nB|--0----|";
        let err = convert(tab, &opts).unwrap_err();
        assert!(err.contains("outside the playable 0–127 range"), "{err}");
    }

    #[test]
    fn inconsistent_stave_width_is_reported() {
        let tab = "e|--3--|\nB|-----|\n\ne|--3--|\nB|-----|\nG|-----|";
        let err = convert(tab, &Options::default()).unwrap_err();
        assert!(err.contains("string line(s)"), "{err}");
    }

    #[test]
    fn pitch_names_round_trip() {
        assert_eq!(parse_pitch("E2").unwrap(), 40);
        assert_eq!(parse_pitch("C4").unwrap(), 60);
        assert_eq!(parse_pitch("F#3").unwrap(), 54);
        assert_eq!(parse_pitch("Bb2").unwrap(), 46);
        assert_eq!(parse_pitch("64").unwrap(), 64);
        assert_eq!(midi_to_name(40), "E2");
        assert_eq!(midi_to_name(60), "C4");
        assert!(parse_pitch("H2").is_err());
        assert!(parse_pitch("E").is_err());
    }

    #[test]
    fn dense_tab_lines_without_many_dashes_are_still_tablature() {
        // Few rest dashes, lots of frets — the line is still a tab line.
        let tab = "e|5-7-9-12-14|\nB|-----------|\nG|-----------|\nD|-----------|\nA|-----------|\nE|-----------|";
        let c = convert(tab, &Options::default()).unwrap();
        assert_eq!(c.strings, 6);
        assert_eq!(c.notes, 5);
    }

    #[test]
    fn hyphenated_prose_is_not_mistaken_for_tablature() {
        let err = convert(
            "well---this---is---it\nand---here---we---go",
            &Options::default(),
        )
        .unwrap_err();
        assert!(err.contains("no tablature found"), "{err}");
    }

    #[test]
    fn form_field_helpers_fall_back_to_defaults() {
        assert_eq!(parse_field("capo", "", 0i32).unwrap(), 0);
        assert_eq!(parse_field("capo", " 3 ", 0i32).unwrap(), 3);
        assert_eq!(parse_field("tempo", "90.5", 120.0f64).unwrap(), 90.5);
        let err = parse_field("tempo", "fast", 120.0f64).unwrap_err();
        assert!(err.contains("tempo must be a number"), "{err}");
        assert!(truthy("", true));
        assert!(!truthy("", false));
        assert!(truthy("true", false));
        assert!(truthy("on", false));
        assert!(!truthy("false", true));
    }

    #[test]
    fn summary_mentions_the_tuning() {
        let c = convert(SIMPLE, &Options::default()).unwrap();
        let s = c.summary();
        assert!(s.contains("standard"), "{s}");
        assert!(s.contains("E2 A2 D3 G3 B3 E4"), "{s}");
    }
}
