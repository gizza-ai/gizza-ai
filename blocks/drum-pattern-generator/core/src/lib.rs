//! drum-pattern-generator core — rule-based drum-pattern generation, shared by
//! the chat skill block and the web page.
//!
//! Two artifacts come out of one deterministic generation pass:
//!
//! * a format-0 Standard MIDI File on General-MIDI channel 10, and
//! * a rendered preview: 22.05 kHz mono 16-bit PCM wrapped in a RIFF/WAVE
//!   header, synthesised here in pure Rust (no ffmpeg, no samples).
//!
//! Plus an ASCII step grid, which is the only "preview" a text surface (chat,
//! CLI) can actually show.
//!
//! Patterns are declared as onsets measured in QUARTER NOTES with a repeating
//! cycle length, so one declaration maps onto every supported time signature by
//! tiling the cycle across the bar and dropping what runs past the bar line.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use midly::{
    num::{u15, u24, u28, u4, u7},
    Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind,
};

/// Ticks per quarter note in the generated MIDI file.
pub const PPQ: u16 = 480;
/// Sample rate of the rendered preview.
pub const SAMPLE_RATE: u32 = 22_050;
/// Longest audio preview we will render, in seconds.
pub const MAX_PREVIEW_SECONDS: f64 = 30.0;
/// Longest pattern we will generate, in bars.
pub const MAX_BARS: u32 = 64;
/// Safety cap on generated note events.
pub const MAX_HITS: usize = 20_000;
/// Bars shown in the ASCII step grid before it is truncated.
pub const MAX_GRID_BARS: u32 = 8;
/// Text/file shape returned by `run`.
pub const OUTPUTS: [&str; 5] = ["report", "grid", "midi-base64", "wav-base64", "json"];

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub genre: String,
    pub time_signature: String,
    pub bars: u32,
    /// Beats per minute. 0 means "use the genre's typical tempo".
    pub tempo: f64,
    pub complexity: String,
    pub hat_subdivision: String,
    pub swing: f64,
    pub humanize: f64,
    pub fill_every: u32,
    pub velocity: u8,
    pub kit: String,
    pub seed: u32,
    pub preview: String,
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            genre: "rock".into(),
            time_signature: "4/4".into(),
            bars: 2,
            tempo: 0.0,
            complexity: "standard".into(),
            hat_subdivision: "auto".into(),
            swing: 0.0,
            humanize: 0.0,
            fill_every: 0,
            velocity: 100,
            kit: "standard".into(),
            seed: 1,
            preview: "drums".into(),
            output: "report".into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Voices
// ---------------------------------------------------------------------------

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Voice {
    Crash,
    Ride,
    OpenHat,
    ClosedHat,
    PedalHat,
    Tambourine,
    Shaker,
    Cowbell,
    Claves,
    HiConga,
    LoConga,
    HighTom,
    MidTom,
    LowTom,
    Clap,
    Snare,
    SideStick,
    Kick,
}

/// Display order, high voices first — the conventional drum-grid layout.
pub const VOICE_ORDER: [Voice; 18] = [
    Voice::Crash,
    Voice::Ride,
    Voice::OpenHat,
    Voice::ClosedHat,
    Voice::PedalHat,
    Voice::Tambourine,
    Voice::Shaker,
    Voice::Cowbell,
    Voice::Claves,
    Voice::HiConga,
    Voice::LoConga,
    Voice::HighTom,
    Voice::MidTom,
    Voice::LowTom,
    Voice::Clap,
    Voice::Snare,
    Voice::SideStick,
    Voice::Kick,
];

impl Voice {
    /// General MIDI percussion key number (channel 10).
    pub fn note(self) -> u8 {
        match self {
            Voice::Kick => 36,
            Voice::SideStick => 37,
            Voice::Snare => 38,
            Voice::Clap => 39,
            Voice::ClosedHat => 42,
            Voice::PedalHat => 44,
            Voice::LowTom => 45,
            Voice::OpenHat => 46,
            Voice::MidTom => 47,
            Voice::Crash => 49,
            Voice::HighTom => 50,
            Voice::Ride => 51,
            Voice::Tambourine => 54,
            Voice::Cowbell => 56,
            Voice::HiConga => 63,
            Voice::LoConga => 64,
            Voice::Shaker => 70,
            Voice::Claves => 75,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Voice::Crash => "Crash",
            Voice::Ride => "Ride",
            Voice::OpenHat => "Open hat",
            Voice::ClosedHat => "Closed hat",
            Voice::PedalHat => "Pedal hat",
            Voice::Tambourine => "Tambourine",
            Voice::Shaker => "Shaker",
            Voice::Cowbell => "Cowbell",
            Voice::Claves => "Claves",
            Voice::HiConga => "Hi conga",
            Voice::LoConga => "Lo conga",
            Voice::HighTom => "High tom",
            Voice::MidTom => "Mid tom",
            Voice::LowTom => "Low tom",
            Voice::Clap => "Clap",
            Voice::Snare => "Snare",
            Voice::SideStick => "Side stick",
            Voice::Kick => "Kick",
        }
    }

    fn index(self) -> u32 {
        VOICE_ORDER.iter().position(|v| *v == self).unwrap_or(0) as u32
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Level {
    Accent,
    Normal,
    Ghost,
}

impl Level {
    fn velocity(self, base: u8) -> u8 {
        let v = match self {
            Level::Accent => base as i32 + 18,
            Level::Normal => base as i32,
            Level::Ghost => base as i32 - 45,
        };
        v.clamp(1, 127) as u8
    }

    fn symbol(self) -> char {
        match self {
            Level::Accent => 'X',
            Level::Normal => 'x',
            Level::Ghost => 'o',
        }
    }
}

// ---------------------------------------------------------------------------
// Genre table
// ---------------------------------------------------------------------------

/// One voice line of a genre: `onsets` are quarter-note offsets inside a
/// repeating `cycle` (also in quarter notes), tiled across every bar.
struct Vp {
    voice: Voice,
    cycle: f64,
    onsets: &'static [f64],
    level: Level,
    /// 0 = always, 1 = standard and busy, 2 = busy only.
    min_cx: u8,
}

struct Genre {
    key: &'static str,
    label: &'static str,
    tempo: f64,
    /// Voice played on every grid step, and the subdivision `auto` resolves to.
    hat: Option<Voice>,
    subdiv: &'static str,
    crash_start: bool,
    voices: &'static [Vp],
}

macro_rules! vp {
    ($v:ident, $c:expr, $o:expr, $l:ident, $m:expr) => {
        Vp {
            voice: Voice::$v,
            cycle: $c,
            onsets: &$o,
            level: Level::$l,
            min_cx: $m,
        }
    };
}

const GENRES: &[Genre] = &[
    Genre {
        key: "rock",
        label: "Rock",
        tempo: 100.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: true,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.0], Normal, 0),
            vp!(Kick, 4.0, [2.5, 3.5], Normal, 2),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
        ],
    },
    Genre {
        key: "pop",
        label: "Pop",
        tempo: 105.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: true,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.0], Normal, 0),
            vp!(Kick, 4.0, [1.5], Normal, 2),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
            vp!(Clap, 4.0, [1.0, 3.0], Normal, 2),
        ],
    },
    Genre {
        key: "funk",
        label: "Funk",
        tempo: 100.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "sixteenth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 0.75, 2.5], Normal, 0),
            vp!(Kick, 4.0, [3.25], Normal, 2),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
            vp!(Snare, 4.0, [0.5, 1.75, 2.25, 3.5], Ghost, 1),
            vp!(OpenHat, 4.0, [3.5], Normal, 2),
        ],
    },
    Genre {
        key: "disco",
        label: "Disco",
        tempo: 120.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: true,
        voices: &[
            vp!(Kick, 1.0, [0.0], Normal, 0),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
            vp!(OpenHat, 1.0, [0.5], Normal, 0),
            vp!(Clap, 4.0, [1.0, 3.0], Normal, 2),
        ],
    },
    Genre {
        key: "house",
        label: "House",
        tempo: 124.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(Kick, 1.0, [0.0], Accent, 0),
            vp!(Clap, 4.0, [1.0, 3.0], Accent, 0),
            vp!(OpenHat, 1.0, [0.5], Normal, 1),
            vp!(Shaker, 4.0, [0.75, 1.75, 2.75, 3.75], Ghost, 2),
        ],
    },
    Genre {
        key: "techno",
        label: "Techno",
        tempo: 132.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "sixteenth",
        crash_start: false,
        voices: &[
            vp!(Kick, 1.0, [0.0], Accent, 0),
            vp!(OpenHat, 1.0, [0.5], Normal, 0),
            vp!(Clap, 4.0, [1.0, 3.0], Normal, 1),
            vp!(SideStick, 4.0, [2.5], Normal, 2),
        ],
    },
    Genre {
        key: "dnb",
        label: "Drum and bass",
        tempo: 174.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "sixteenth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.5], Normal, 0),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
            vp!(Snare, 4.0, [1.75, 3.5], Ghost, 1),
            vp!(Kick, 4.0, [3.75], Normal, 2),
        ],
    },
    Genre {
        key: "breakbeat",
        label: "Breakbeat",
        tempo: 135.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: true,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.5], Normal, 0),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
            vp!(Snare, 4.0, [2.25, 3.75], Ghost, 1),
            vp!(Kick, 4.0, [1.5], Normal, 2),
        ],
    },
    Genre {
        key: "trap",
        label: "Trap",
        tempo: 140.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "sixteenth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 1.75, 2.5], Normal, 0),
            vp!(Snare, 4.0, [2.0], Accent, 0),
            vp!(Clap, 4.0, [2.0], Normal, 1),
            vp!(Kick, 4.0, [3.5], Normal, 2),
        ],
    },
    Genre {
        key: "boom-bap",
        label: "Boom bap",
        tempo: 88.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.5], Normal, 0),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
            vp!(Snare, 4.0, [3.5], Ghost, 1),
            vp!(Kick, 4.0, [0.75], Normal, 2),
        ],
    },
    Genre {
        key: "lofi",
        label: "Lo-fi hip hop",
        tempo: 78.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.5], Normal, 0),
            vp!(Snare, 4.0, [1.0, 3.0], Normal, 0),
            vp!(Shaker, 4.0, [0.5, 1.5, 2.5, 3.5], Ghost, 1),
            vp!(SideStick, 4.0, [3.75], Ghost, 2),
        ],
    },
    Genre {
        key: "reggae",
        label: "Reggae one drop",
        tempo: 78.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [2.0], Accent, 0),
            vp!(SideStick, 4.0, [2.0], Accent, 0),
            vp!(Snare, 4.0, [0.5, 1.5, 3.5], Ghost, 1),
            vp!(OpenHat, 4.0, [1.5, 3.5], Normal, 2),
        ],
    },
    Genre {
        key: "reggaeton",
        label: "Reggaeton dembow",
        tempo: 95.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.0], Normal, 0),
            vp!(Snare, 4.0, [0.75, 1.5, 2.75, 3.5], Accent, 0),
            vp!(Kick, 4.0, [1.5, 3.5], Normal, 2),
        ],
    },
    Genre {
        key: "afrobeat",
        label: "Afrobeat",
        tempo: 108.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "sixteenth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 1.5, 2.5], Normal, 0),
            vp!(SideStick, 4.0, [1.0, 3.0], Normal, 0),
            vp!(Shaker, 0.5, [0.0], Ghost, 1),
            vp!(HiConga, 4.0, [0.75, 2.75], Normal, 1),
            vp!(LoConga, 4.0, [1.75, 3.75], Normal, 2),
        ],
    },
    Genre {
        key: "bossa-nova",
        label: "Bossa nova",
        tempo: 130.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(SideStick, 4.0, [0.0, 0.75, 1.5, 2.5, 3.0], Accent, 0),
            vp!(Kick, 2.0, [0.0, 1.5], Normal, 0),
            vp!(Shaker, 4.0, [0.5, 1.5, 2.5, 3.5], Ghost, 2),
        ],
    },
    Genre {
        key: "jazz-swing",
        label: "Jazz swing",
        tempo: 140.0,
        hat: None,
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(Ride, 2.0, [0.0], Accent, 0),
            vp!(Ride, 2.0, [1.0, 1.5], Normal, 0),
            vp!(PedalHat, 2.0, [1.0], Normal, 0),
            vp!(Kick, 4.0, [0.0, 1.0, 2.0, 3.0], Ghost, 1),
            vp!(Snare, 4.0, [1.5, 3.5], Ghost, 2),
        ],
    },
    Genre {
        key: "blues-shuffle",
        label: "Blues shuffle",
        tempo: 90.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "triplet-eighth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.0], Normal, 0),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
            vp!(Kick, 4.0, [2.6667], Normal, 2),
        ],
    },
    Genre {
        key: "metal",
        label: "Metal",
        tempo: 160.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: true,
        voices: &[
            vp!(Kick, 1.0, [0.0, 0.5], Normal, 0),
            vp!(Snare, 4.0, [1.0, 3.0], Accent, 0),
            vp!(Kick, 0.25, [0.0], Normal, 2),
        ],
    },
    Genre {
        key: "country",
        label: "Country",
        tempo: 120.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(Kick, 4.0, [0.0, 2.0], Normal, 0),
            vp!(SideStick, 4.0, [1.0, 3.0], Accent, 0),
            vp!(Snare, 0.5, [0.0], Ghost, 2),
        ],
    },
    Genre {
        key: "waltz",
        label: "Waltz",
        tempo: 110.0,
        hat: Some(Voice::ClosedHat),
        subdiv: "eighth",
        crash_start: false,
        voices: &[
            vp!(Kick, 3.0, [0.0], Accent, 0),
            vp!(Snare, 3.0, [1.0, 2.0], Normal, 0),
            vp!(SideStick, 3.0, [2.5], Ghost, 2),
        ],
    },
];

/// Every accepted `genre` value, in descriptor order.
pub fn genre_keys() -> Vec<&'static str> {
    GENRES.iter().map(|g| g.key).collect()
}

/// Human label for a genre key, or the key itself if unknown.
pub fn genre_label(key: &str) -> &str {
    GENRES
        .iter()
        .find(|g| g.key == key)
        .map(|g| g.label)
        .unwrap_or(key)
}

/// The genre's typical tempo in BPM (what `tempo = 0` resolves to).
pub fn genre_tempo(key: &str) -> Option<f64> {
    GENRES.iter().find(|g| g.key == key).map(|g| g.tempo)
}

fn find_genre(key: &str) -> Result<&'static Genre, String> {
    GENRES.iter().find(|g| g.key == key).ok_or_else(|| {
        format!(
            "unknown genre '{}'; expected one of: {}",
            key,
            genre_keys().join(", ")
        )
    })
}

// ---------------------------------------------------------------------------
// Kits + time signatures
// ---------------------------------------------------------------------------

const KITS: &[(&str, &str, u8)] = &[
    ("standard", "Standard", 0),
    ("room", "Room", 8),
    ("power", "Power", 16),
    ("electronic", "Electronic", 24),
    ("tr808", "TR-808", 25),
    ("jazz", "Jazz", 32),
    ("brush", "Brush", 40),
    ("orchestra", "Orchestra", 48),
];

pub fn kit_keys() -> Vec<&'static str> {
    KITS.iter().map(|k| k.0).collect()
}

pub fn kit_program(key: &str) -> Result<u8, String> {
    KITS.iter()
        .find(|k| k.0 == key)
        .map(|k| k.2)
        .ok_or_else(|| {
            format!(
                "unknown kit '{}'; expected one of: {}",
                key,
                kit_keys().join(", ")
            )
        })
}

pub fn kit_label(key: &str) -> &str {
    KITS.iter().find(|k| k.0 == key).map(|k| k.1).unwrap_or(key)
}

const TIME_SIGNATURES: &[(&str, u32, u32)] = &[
    ("4/4", 4, 4),
    ("3/4", 3, 4),
    ("2/4", 2, 4),
    ("5/4", 5, 4),
    ("6/8", 6, 8),
    ("7/8", 7, 8),
    ("12/8", 12, 8),
];

pub fn time_signature_keys() -> Vec<&'static str> {
    TIME_SIGNATURES.iter().map(|t| t.0).collect()
}

fn parse_time_signature(s: &str) -> Result<(u32, u32), String> {
    TIME_SIGNATURES
        .iter()
        .find(|t| t.0 == s)
        .map(|t| (t.1, t.2))
        .ok_or_else(|| {
            format!(
                "unknown time_signature '{}'; expected one of: {}",
                s,
                time_signature_keys().join(", ")
            )
        })
}

pub const COMPLEXITIES: [&str; 3] = ["basic", "standard", "busy"];
pub const SUBDIVISIONS: [&str; 5] = ["auto", "quarter", "eighth", "sixteenth", "triplet-eighth"];
pub const PREVIEWS: [&str; 4] = ["drums", "drums-and-click", "click", "off"];

fn complexity_level(s: &str) -> Result<u8, String> {
    match s {
        "basic" => Ok(0),
        "standard" => Ok(1),
        "busy" => Ok(2),
        other => Err(format!(
            "unknown complexity '{}'; expected one of: {}",
            other,
            COMPLEXITIES.join(", ")
        )),
    }
}

/// Ticks per step for a named subdivision.
fn subdiv_ticks(s: &str) -> Result<u32, String> {
    match s {
        "quarter" => Ok(PPQ as u32),
        "eighth" => Ok(PPQ as u32 / 2),
        "sixteenth" => Ok(PPQ as u32 / 4),
        "triplet-eighth" => Ok(PPQ as u32 / 3),
        other => Err(format!(
            "unknown hat_subdivision '{}'; expected one of: {}",
            other,
            SUBDIVISIONS.join(", ")
        )),
    }
}

/// Does a bar of `bar_ticks` divide evenly by this subdivision (and by the note
/// grid it implies)?
fn fits(bar_ticks: u32, subdiv: &str) -> bool {
    let hat = match subdiv_ticks(subdiv) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let grid = if subdiv == "triplet-eighth" {
        PPQ as u32 / 3
    } else {
        PPQ as u32 / 4
    };
    bar_ticks % hat == 0 && bar_ticks % grid == 0
}

/// Shift a subdivision one step coarser (basic) or finer (busy).
fn shift_subdiv(s: &str, level: u8) -> &'static str {
    match (s, level) {
        ("sixteenth", 0) => "eighth",
        ("eighth", 0) => "quarter",
        ("triplet-eighth", 0) => "quarter",
        ("quarter", 0) => "quarter",
        ("quarter", 2) => "eighth",
        ("eighth", 2) => "sixteenth",
        ("sixteenth", 2) => "sixteenth",
        ("triplet-eighth", _) => "triplet-eighth",
        ("quarter", _) => "quarter",
        ("eighth", _) => "eighth",
        _ => "sixteenth",
    }
}

// ---------------------------------------------------------------------------
// Generation
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hit {
    pub tick: u32,
    pub voice: Voice,
    pub velocity: u8,
}

#[derive(Debug)]
pub struct Generated {
    pub midi: Vec<u8>,
    /// Empty when `preview = "off"`.
    pub wav: Vec<u8>,
    pub grid: String,
    pub genre_label: String,
    pub kit_label: String,
    pub tempo: f64,
    pub bars: u32,
    pub numerator: u32,
    pub denominator: u32,
    pub subdivision: String,
    pub steps_per_bar: usize,
    pub hits: usize,
    pub voice_labels: Vec<String>,
    pub seconds: f64,
    pub preview_seconds: f64,
    pub preview_bars: u32,
    pub preview_truncated: bool,
    pub grid_truncated: bool,
}

impl Generated {
    pub fn summary(&self) -> String {
        let mut s = format!(
            "{} pattern in {}/{} — {} bar{} at {} BPM ({:.1} s), {} hit{} across {} voice{}.",
            self.genre_label,
            self.numerator,
            self.denominator,
            self.bars,
            if self.bars == 1 { "" } else { "s" },
            fmt_num(self.tempo),
            self.seconds,
            self.hits,
            if self.hits == 1 { "" } else { "s" },
            self.voice_labels.len(),
            if self.voice_labels.len() == 1 {
                ""
            } else {
                "s"
            }
        );
        s.push_str(&format!(
            " Kit: {}. Grid: {} ({} steps per bar).",
            self.kit_label, self.subdivision, self.steps_per_bar
        ));
        if !self.wav.is_empty() {
            s.push_str(&format!(
                " Preview: {:.1} s of rendered audio{}.",
                self.preview_seconds,
                if self.preview_truncated {
                    format!(" (first {} bars)", self.preview_bars)
                } else {
                    String::new()
                }
            ));
        }
        s
    }

    pub fn detail_text(&self) -> String {
        let mut out = self.summary();
        out.push_str("\n\n");
        out.push_str(&self.grid);
        out
    }
}

fn fmt_num(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v:.2}")
    }
}

fn hash32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// Generate the pattern, the MIDI file and the rendered preview.
pub fn generate(opts: &Options) -> Result<Generated, String> {
    let genre = find_genre(opts.genre.trim())?;
    let (num, den) = parse_time_signature(opts.time_signature.trim())?;
    let cx = complexity_level(opts.complexity.trim())?;
    let program = kit_program(opts.kit.trim())?;

    if opts.bars < 1 || opts.bars > MAX_BARS {
        return Err(format!(
            "bars must be between 1 and {MAX_BARS}, got {}",
            opts.bars
        ));
    }
    if opts.tempo != 0.0 && !(20.0..=300.0).contains(&opts.tempo) {
        return Err(format!(
            "tempo must be 0 (use the genre tempo) or between 20 and 300 BPM, got {}",
            fmt_num(opts.tempo)
        ));
    }
    if !(0.0..=75.0).contains(&opts.swing) {
        return Err(format!(
            "swing must be between 0 and 75 percent, got {}",
            fmt_num(opts.swing)
        ));
    }
    if !(0.0..=100.0).contains(&opts.humanize) {
        return Err(format!(
            "humanize must be between 0 and 100 percent, got {}",
            fmt_num(opts.humanize)
        ));
    }
    if opts.fill_every > MAX_BARS {
        return Err(format!(
            "fill_every must be between 0 (no fills) and {MAX_BARS}, got {}",
            opts.fill_every
        ));
    }
    if opts.velocity < 1 {
        return Err("velocity must be between 1 and 127, got 0".to_string());
    }
    let preview = opts.preview.trim();
    if !PREVIEWS.contains(&preview) {
        return Err(format!(
            "unknown preview '{}'; expected one of: {}",
            preview,
            PREVIEWS.join(", ")
        ));
    }

    let tempo = if opts.tempo == 0.0 {
        genre.tempo
    } else {
        opts.tempo
    };

    // Resolve the hat subdivision: auto -> the genre's, then complexity shifts it.
    let requested = opts.hat_subdivision.trim();
    if !SUBDIVISIONS.contains(&requested) {
        return Err(format!(
            "unknown hat_subdivision '{}'; expected one of: {}",
            requested,
            SUBDIVISIONS.join(", ")
        ));
    }
    let auto = requested == "auto";
    let mut subdiv: &str = if auto {
        shift_subdiv(genre.subdiv, cx)
    } else {
        requested
    };

    let bar_quarters = num as f64 * 4.0 / den as f64;
    let bar_ticks_f = bar_quarters * PPQ as f64;
    let bar_ticks = bar_ticks_f.round() as u32;
    if (bar_ticks_f - bar_ticks as f64).abs() > 1e-6 {
        return Err(format!(
            "time signature {}/{} does not map to whole MIDI ticks",
            num, den
        ));
    }
    // `auto` must always work: fall back to a grid this bar can be divided by.
    if auto {
        for candidate in [subdiv, "eighth", "sixteenth"] {
            if fits(bar_ticks, candidate) {
                subdiv = candidate;
                break;
            }
        }
    }
    if !fits(bar_ticks, subdiv) {
        return Err(format!(
            "a {}/{} bar cannot be divided evenly by the {} grid; try hat_subdivision = eighth or sixteenth",
            num, den, subdiv
        ));
    }
    let hat_ticks = subdiv_ticks(subdiv)?;
    // Everything not on the hat lands on a 16th grid (or the triplet grid).
    let grid_ticks: u32 = if subdiv == "triplet-eighth" {
        PPQ as u32 / 3
    } else {
        PPQ as u32 / 4
    };
    let steps_per_bar = (bar_ticks / grid_ticks) as usize;

    // --- nominal hits (pre-swing, pre-humanize) ----------------------------
    let mut nominal: Vec<Hit> = Vec::new();
    let base_vel = opts.velocity;

    for bar in 0..opts.bars {
        let bar_start = bar * bar_ticks;
        let mut bar_hits: Vec<Hit> = Vec::new();

        if let Some(hv) = genre.hat {
            let mut t = 0u32;
            while t < bar_ticks {
                let level = if t % PPQ as u32 == 0 {
                    Level::Accent
                } else {
                    Level::Normal
                };
                bar_hits.push(Hit {
                    tick: bar_start + t,
                    voice: hv,
                    velocity: level.velocity(base_vel),
                });
                t += hat_ticks;
            }
        }

        for vp in genre.voices {
            if vp.min_cx > cx {
                continue;
            }
            let mut c = 0.0f64;
            while c < bar_quarters - 1e-9 {
                for o in vp.onsets {
                    let pos = c + o;
                    if pos >= bar_quarters - 1e-9 {
                        continue;
                    }
                    let raw = pos * PPQ as f64;
                    let snapped = ((raw / grid_ticks as f64).round() as u32)
                        .min(steps_per_bar as u32 - 1)
                        * grid_ticks;
                    bar_hits.push(Hit {
                        tick: bar_start + snapped,
                        voice: vp.voice,
                        velocity: vp.level.velocity(base_vel),
                    });
                }
                c += vp.cycle.max(0.0625);
            }
        }

        if genre.crash_start && cx >= 1 && bar == 0 {
            bar_hits.push(Hit {
                tick: bar_start,
                voice: Voice::Crash,
                velocity: Level::Accent.velocity(base_vel),
            });
        }

        // Fill: replace the last quarter note of every `fill_every`-th bar.
        let is_fill = opts.fill_every > 0 && (bar + 1) % opts.fill_every == 0;
        if is_fill {
            let window_start = bar_start + bar_ticks - PPQ as u32;
            bar_hits.retain(|h| h.tick < window_start);
            let steps = PPQ as u32 / grid_ticks;
            const FILL: [Voice; 8] = [
                Voice::Snare,
                Voice::Snare,
                Voice::HighTom,
                Voice::HighTom,
                Voice::MidTom,
                Voice::MidTom,
                Voice::LowTom,
                Voice::LowTom,
            ];
            let offset = (bar / opts.fill_every.max(1)) as usize;
            for s in 0..steps {
                let voice = FILL[(offset + s as usize) % FILL.len()];
                let level = if s == 0 { Level::Accent } else { Level::Normal };
                bar_hits.push(Hit {
                    tick: window_start + s * grid_ticks,
                    voice,
                    velocity: level.velocity(base_vel),
                });
            }
            if bar + 1 < opts.bars {
                nominal.push(Hit {
                    tick: bar_start + bar_ticks,
                    voice: Voice::Crash,
                    velocity: Level::Accent.velocity(base_vel),
                });
            }
        }

        nominal.append(&mut bar_hits);
    }

    // An open hat silences a closed hat on the same tick.
    let open_ticks: Vec<u32> = nominal
        .iter()
        .filter(|h| h.voice == Voice::OpenHat)
        .map(|h| h.tick)
        .collect();
    nominal.retain(|h| !(h.voice == Voice::ClosedHat && open_ticks.contains(&h.tick)));

    // Deduplicate (voice, tick), keeping the loudest.
    nominal.sort_by_key(|h| (h.tick, h.voice.index(), 127 - h.velocity));
    nominal.dedup_by_key(|h| (h.tick, h.voice.index()));

    if nominal.is_empty() {
        return Err("this combination of genre, complexity and time signature produced no hits; try complexity = standard".to_string());
    }
    if nominal.len() > MAX_HITS {
        return Err(format!(
            "pattern would contain {} notes, over the {MAX_HITS} note cap; reduce bars or complexity",
            nominal.len()
        ));
    }

    let total_ticks = opts.bars * bar_ticks;
    let grid = render_grid(&nominal, opts.bars, bar_ticks, grid_ticks, steps_per_bar);

    // --- swing + humanize --------------------------------------------------
    let mut hits = nominal.clone();
    if opts.swing > 0.0 && subdiv != "triplet-eighth" {
        let unit = if hat_ticks >= PPQ as u32 {
            PPQ as u32 / 2
        } else {
            hat_ticks
        };
        let shift = (opts.swing / 100.0 * unit as f64 / 2.0).round() as u32;
        for h in hits.iter_mut() {
            if unit > 0 && (h.tick / unit) % 2 == 1 && h.tick % unit == 0 {
                h.tick += shift;
            }
        }
    }
    if opts.humanize > 0.0 {
        let t_amt = opts.humanize / 100.0 * (PPQ as f64 / 16.0);
        let v_amt = opts.humanize / 100.0 * 20.0;
        for h in hits.iter_mut() {
            let r = hash32(
                opts.seed
                    .wrapping_mul(2_654_435_761)
                    .wrapping_add(h.tick.wrapping_mul(2_246_822_519))
                    .wrapping_add(h.voice.index().wrapping_mul(40_503)),
            );
            let a = (r & 0xffff) as f64 / 65_535.0 * 2.0 - 1.0;
            let b = ((r >> 16) & 0xffff) as f64 / 65_535.0 * 2.0 - 1.0;
            let dt = (a * t_amt).round() as i64;
            h.tick = (h.tick as i64 + dt).clamp(0, total_ticks as i64 - 1) as u32;
            let dv = (b * v_amt).round() as i32;
            h.velocity = (h.velocity as i32 + dv).clamp(1, 127) as u8;
        }
    }
    hits.sort_by_key(|h| (h.tick, h.voice.index()));

    let mut voice_labels: Vec<String> = Vec::new();
    for v in VOICE_ORDER {
        if hits.iter().any(|h| h.voice == v) {
            voice_labels.push(v.label().to_string());
        }
    }

    let midi = write_smf(&hits, tempo, num, den, program, total_ticks)?;

    let quarter_seconds = 60.0 / tempo;
    let bar_seconds = bar_quarters * quarter_seconds;
    let seconds = bar_seconds * opts.bars as f64;

    let (wav, preview_bars, preview_seconds, preview_truncated) = if preview == "off" {
        (Vec::new(), 0, 0.0, false)
    } else {
        let max_bars = (MAX_PREVIEW_SECONDS / bar_seconds).floor().max(1.0) as u32;
        let pb = opts.bars.min(max_bars);
        let samples = render_preview(&hits, preview, tempo, num, den, bar_ticks, pb, opts.seed);
        let secs = samples.len() as f64 / SAMPLE_RATE as f64;
        (wav_bytes(&samples), pb, secs, pb < opts.bars)
    };

    Ok(Generated {
        midi,
        wav,
        grid,
        genre_label: genre.label.to_string(),
        kit_label: kit_label(opts.kit.trim()).to_string(),
        tempo,
        bars: opts.bars,
        numerator: num,
        denominator: den,
        subdivision: subdiv.to_string(),
        steps_per_bar,
        hits: hits.len(),
        voice_labels,
        seconds,
        preview_seconds,
        preview_bars,
        preview_truncated,
        grid_truncated: opts.bars > MAX_GRID_BARS,
    })
}

// ---------------------------------------------------------------------------
// ASCII step grid
// ---------------------------------------------------------------------------

fn render_grid(
    hits: &[Hit],
    bars: u32,
    bar_ticks: u32,
    grid_ticks: u32,
    steps_per_bar: usize,
) -> String {
    let shown = bars.min(MAX_GRID_BARS);
    let mut used: Vec<Voice> = Vec::new();
    for v in VOICE_ORDER {
        if hits.iter().any(|h| h.voice == v) {
            used.push(v);
        }
    }
    let name_w = used.iter().map(|v| v.label().len()).max().unwrap_or(4);

    // Header: beat numbers on quarter-note boundaries.
    let mut header = format!("{:name_w$} ", "", name_w = name_w);
    for bar in 0..shown {
        header.push('|');
        for s in 0..steps_per_bar {
            let tick = s as u32 * grid_ticks;
            if tick % PPQ as u32 == 0 {
                let beat = tick / PPQ as u32 + 1;
                header.push_str(&format!("{}", beat % 10));
            } else if tick % (PPQ as u32 / 2) == 0 {
                header.push('+');
            } else {
                header.push('.');
            }
        }
        let _ = bar;
    }
    header.push('|');

    let mut lines = vec![header];
    for v in &used {
        let mut line = format!("{:name_w$} ", v.label(), name_w = name_w);
        for bar in 0..shown {
            line.push('|');
            for s in 0..steps_per_bar {
                let tick = bar * bar_ticks + s as u32 * grid_ticks;
                let hit = hits.iter().find(|h| h.voice == *v && h.tick == tick);
                line.push(match hit {
                    Some(h) => level_of(h.velocity).symbol(),
                    None => '-',
                });
            }
        }
        line.push('|');
        lines.push(line);
    }
    if bars > shown {
        lines.push(format!(
            "{:name_w$} (first {} of {} bars shown)",
            "",
            shown,
            bars,
            name_w = name_w
        ));
    }
    lines.join("\n")
}

/// Classify a rendered velocity back into a grid symbol band.
fn level_of(vel: u8) -> Level {
    if vel <= 60 {
        Level::Ghost
    } else if vel >= 110 {
        Level::Accent
    } else {
        Level::Normal
    }
}

// ---------------------------------------------------------------------------
// MIDI
// ---------------------------------------------------------------------------

/// GM percussion lives on MIDI channel 10, which is channel index 9.
const DRUM_CHANNEL: u8 = 9;

fn write_smf(
    hits: &[Hit],
    tempo: f64,
    num: u32,
    den: u32,
    program: u8,
    end_tick: u32,
) -> Result<Vec<u8>, String> {
    let micros_per_beat = (60_000_000.0 / tempo).round().clamp(1.0, 16_777_215.0) as u32;
    let denom_pow = match den {
        2 => 1u8,
        4 => 2,
        8 => 3,
        16 => 4,
        _ => 2,
    };
    // (tick, order, event) — note-offs sort before note-ons at the same tick.
    let mut events: Vec<(u32, u8, TrackEventKind<'static>)> = Vec::new();
    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::TrackName(&b"Drum pattern"[..])),
    ));
    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::Tempo(u24::from(micros_per_beat))),
    ));
    events.push((
        0,
        0,
        TrackEventKind::Meta(MetaMessage::TimeSignature(num as u8, denom_pow, 24, 8)),
    ));
    events.push((
        0,
        0,
        TrackEventKind::Midi {
            channel: u4::from(DRUM_CHANNEL),
            message: MidiMessage::ProgramChange {
                program: u7::from(program),
            },
        },
    ));

    // Drum notes are one-shots: a short fixed gate is enough for every player.
    let gate = PPQ as u32 / 8;
    for h in hits {
        events.push((
            h.tick,
            2,
            TrackEventKind::Midi {
                channel: u4::from(DRUM_CHANNEL),
                message: MidiMessage::NoteOn {
                    key: u7::from(h.voice.note()),
                    vel: u7::from(h.velocity),
                },
            },
        ));
        events.push((
            h.tick + gate,
            1,
            TrackEventKind::Midi {
                channel: u4::from(DRUM_CHANNEL),
                message: MidiMessage::NoteOff {
                    key: u7::from(h.voice.note()),
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
// Preview rendering (pure-Rust drum synthesis -> RIFF/WAVE)
// ---------------------------------------------------------------------------

struct Noise(u32);

impl Noise {
    fn new(seed: u32) -> Self {
        Noise(seed | 1)
    }
    fn next(&mut self) -> f32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        (x as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// Voice-specific synthesis parameters: (decay seconds, tone Hz, tone sweep to
/// Hz, noise mix 0..1, brightness passes).
fn voice_synth(v: Voice) -> (f32, f32, f32, f32, u8) {
    match v {
        Voice::Kick => (0.28, 125.0, 45.0, 0.0, 0),
        Voice::LowTom => (0.30, 140.0, 100.0, 0.06, 0),
        Voice::MidTom => (0.26, 190.0, 140.0, 0.06, 0),
        Voice::HighTom => (0.22, 260.0, 190.0, 0.06, 0),
        Voice::Snare => (0.16, 190.0, 170.0, 0.62, 1),
        Voice::Clap => (0.18, 0.0, 0.0, 1.0, 2),
        Voice::SideStick => (0.05, 900.0, 780.0, 0.35, 1),
        Voice::ClosedHat => (0.045, 0.0, 0.0, 1.0, 3),
        Voice::PedalHat => (0.07, 0.0, 0.0, 1.0, 3),
        Voice::OpenHat => (0.30, 0.0, 0.0, 1.0, 3),
        Voice::Crash => (1.20, 0.0, 0.0, 1.0, 2),
        Voice::Ride => (0.55, 620.0, 620.0, 0.80, 2),
        Voice::Tambourine => (0.16, 0.0, 0.0, 1.0, 3),
        Voice::Shaker => (0.06, 0.0, 0.0, 1.0, 3),
        Voice::Cowbell => (0.28, 540.0, 540.0, 0.05, 0),
        Voice::Claves => (0.05, 2400.0, 2400.0, 0.02, 0),
        Voice::HiConga => (0.20, 300.0, 260.0, 0.10, 0),
        Voice::LoConga => (0.26, 200.0, 175.0, 0.10, 0),
    }
}

fn voice_gain(v: Voice) -> f32 {
    match v {
        Voice::Kick => 1.00,
        Voice::Snare => 0.72,
        Voice::Clap => 0.62,
        Voice::Crash => 0.42,
        Voice::Ride => 0.34,
        Voice::OpenHat => 0.32,
        Voice::ClosedHat => 0.28,
        Voice::PedalHat => 0.26,
        Voice::Shaker | Voice::Tambourine => 0.26,
        Voice::SideStick | Voice::Claves => 0.55,
        _ => 0.60,
    }
}

/// Mix one drum voice into `buf` starting at sample `at`.
fn render_voice(buf: &mut [f32], at: usize, v: Voice, velocity: u8, seed: u32) {
    let (decay, f0, f1, noise_mix, bright) = voice_synth(v);
    let sr = SAMPLE_RATE as f32;
    let n = ((decay * 4.0) * sr) as usize;
    let amp = voice_gain(v) * (velocity as f32 / 127.0).powf(1.4);
    let mut rng = Noise::new(hash32(
        seed ^ v.index().wrapping_mul(2_654_435_761) ^ (at as u32),
    ));
    // Cowbell gets a second detuned partial; claps get their multi-burst tail.
    let extra_f = if v == Voice::Cowbell { 800.0 } else { 0.0 };
    let mut prev = [0.0f32; 3];
    let mut phase = 0.0f32;
    let mut phase2 = 0.0f32;
    for i in 0..n {
        let idx = at + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / sr;
        let env = (-t / decay.max(1e-4) * 3.0).exp();
        if env < 1e-4 {
            break;
        }
        // Multi-burst envelope for a hand-clap.
        let env = if v == Voice::Clap {
            let burst = ((t / 0.010).floor() as i32).min(3);
            if burst < 3 {
                env * (1.0 - 0.35 * ((t / 0.010) % 1.0))
            } else {
                env
            }
        } else {
            env
        };

        let mut s = 0.0f32;
        if noise_mix < 1.0 {
            let f = f1 + (f0 - f1) * (-t / (decay * 0.35).max(1e-4)).exp();
            phase += f / sr;
            s += (1.0 - noise_mix) * (phase * std::f32::consts::TAU).sin();
            if extra_f > 0.0 {
                phase2 += extra_f / sr;
                s += 0.5 * (1.0 - noise_mix) * (phase2 * std::f32::consts::TAU).sin();
            }
        }
        if noise_mix > 0.0 {
            let mut nz = rng.next();
            for p in 0..bright.min(3) as usize {
                let hp = nz - prev[p];
                prev[p] = nz;
                nz = hp;
            }
            s += noise_mix * nz * 0.8;
        }
        buf[idx] += s * env * amp;
    }
}

/// Render a metronome blip.
fn render_click(buf: &mut [f32], at: usize, downbeat: bool) {
    let sr = SAMPLE_RATE as f32;
    let f = if downbeat { 1500.0 } else { 1000.0 };
    let decay = 0.030f32;
    let amp = if downbeat { 0.55 } else { 0.38 };
    let n = (decay * 4.0 * sr) as usize;
    for i in 0..n {
        let idx = at + i;
        if idx >= buf.len() {
            break;
        }
        let t = i as f32 / sr;
        let env = (-t / decay * 3.0).exp();
        buf[idx] += (t * f * std::f32::consts::TAU).sin() * env * amp;
    }
}

#[allow(clippy::too_many_arguments)]
fn render_preview(
    hits: &[Hit],
    preview: &str,
    tempo: f64,
    num: u32,
    den: u32,
    bar_ticks: u32,
    bars: u32,
    seed: u32,
) -> Vec<f32> {
    let seconds_per_tick = 60.0 / tempo / PPQ as f64;
    let end_ticks = bars * bar_ticks;
    let tail = 1.0f64;
    let total = ((end_ticks as f64 * seconds_per_tick + tail) * SAMPLE_RATE as f64) as usize + 1;
    let mut buf = vec![0.0f32; total];

    if preview != "click" {
        for h in hits {
            if h.tick >= end_ticks {
                continue;
            }
            let at = (h.tick as f64 * seconds_per_tick * SAMPLE_RATE as f64) as usize;
            render_voice(&mut buf, at, h.voice, h.velocity, seed);
        }
    }
    if preview == "click" || preview == "drums-and-click" {
        // Pulse: a quarter note in x/4, a dotted quarter in 6/8 and 12/8,
        // an eighth otherwise.
        let pulse_ticks: u32 = if den == 4 {
            PPQ as u32
        } else if num % 3 == 0 {
            PPQ as u32 * 3 / 2
        } else {
            PPQ as u32 / 2
        };
        let mut t = 0u32;
        while t < end_ticks {
            let at = (t as f64 * seconds_per_tick * SAMPLE_RATE as f64) as usize;
            render_click(&mut buf, at, t % bar_ticks == 0);
            t += pulse_ticks;
        }
    }
    buf
}

/// Wrap mono f32 samples in a 16-bit PCM RIFF/WAVE container.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let mut pcm: Vec<u8> = Vec::with_capacity(samples.len() * 2);
    for s in samples {
        let v = (s * 0.9).clamp(-1.0, 1.0);
        let i = (v * 32_767.0).round() as i16;
        pcm.extend_from_slice(&i.to_le_bytes());
    }
    let data_len = pcm.len() as u32;
    let mut out = Vec::with_capacity(44 + pcm.len());
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&1u16.to_le_bytes()); // mono
    out.extend_from_slice(&SAMPLE_RATE.to_le_bytes());
    out.extend_from_slice(&(SAMPLE_RATE * 2).to_le_bytes()); // byte rate
    out.extend_from_slice(&2u16.to_le_bytes()); // block align
    out.extend_from_slice(&16u16.to_le_bytes()); // bits
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out.extend_from_slice(&pcm);
    out
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

/// Blank page field falls back to the default value.
pub fn or_default(raw: &str, default: &str) -> String {
    if raw.trim().is_empty() {
        default.to_string()
    } else {
        raw.trim().to_string()
    }
}

fn check_enum(name: &str, value: &str, allowed: &[&str]) -> Result<String, String> {
    let v = value.trim();
    if allowed.contains(&v) {
        Ok(v.to_string())
    } else {
        Err(format!(
            "unknown {name} '{}'; expected one of: {}",
            v,
            allowed.join(", ")
        ))
    }
}

fn jesc(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn jarr(items: &[String]) -> String {
    let inner = items
        .iter()
        .map(|s| format!("\"{}\"", jesc(s)))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{inner}]")
}

/// Validate options, generate the pattern, and return the requested text/file shape.
pub fn run(opts: &Options) -> Result<String, String> {
    let output = check_enum("output", &opts.output, &OUTPUTS)?;
    let g = generate(opts)?;
    match output.as_str() {
        "grid" => Ok(g.grid),
        "midi-base64" => Ok(B64.encode(&g.midi)),
        "wav-base64" => {
            if g.wav.is_empty() {
                Err("preview=off did not render WAV audio; choose preview=drums, drums-and-click or click".into())
            } else {
                Ok(B64.encode(&g.wav))
            }
        }
        "json" => Ok(format!(
            "{{\"genre\":\"{}\",\"kit\":\"{}\",\"time_signature\":\"{}/{}\",\"tempo\":{},\"bars\":{},\"subdivision\":\"{}\",\"hits\":{},\"voices\":{},\"seconds\":{:.3},\"preview_seconds\":{:.3},\"preview_truncated\":{},\"grid\":\"{}\",\"midi_base64\":\"{}\",\"wav_base64\":\"{}\"}}",
            jesc(&g.genre_label),
            jesc(&g.kit_label),
            g.numerator,
            g.denominator,
            fmt_num(g.tempo),
            g.bars,
            jesc(&g.subdivision),
            g.hits,
            jarr(&g.voice_labels),
            g.seconds,
            g.preview_seconds,
            g.preview_truncated,
            jesc(&g.grid),
            B64.encode(&g.midi),
            if g.wav.is_empty() { String::new() } else { B64.encode(&g.wav) }
        )),
        _ => Ok(g.detail_text()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Decode the produced file back into (tick, key, velocity) note-on triples.
    fn read_notes(bytes: &[u8]) -> Vec<(u32, u8, u8)> {
        let smf = Smf::parse(bytes).expect("produced a parseable SMF");
        let mut out = Vec::new();
        let mut tick = 0u32;
        for ev in &smf.tracks[0] {
            tick += ev.delta.as_int();
            if let TrackEventKind::Midi { channel, message } = ev.kind {
                if let MidiMessage::NoteOn { key, vel } = message {
                    if vel.as_int() > 0 {
                        assert_eq!(
                            channel.as_int(),
                            DRUM_CHANNEL,
                            "drums must be on channel 10"
                        );
                        out.push((tick, key.as_int(), vel.as_int()));
                    }
                }
            }
        }
        out
    }

    fn ticks_of(bytes: &[u8], note: u8) -> Vec<u32> {
        read_notes(bytes)
            .into_iter()
            .filter(|(_, k, _)| *k == note)
            .map(|(t, _, _)| t)
            .collect()
    }

    // --- happy path -------------------------------------------------------

    #[test]
    fn generates_a_default_rock_pattern_with_backbeat_and_hats() {
        let out = generate(&Options::default()).unwrap();
        assert_eq!(out.genre_label, "Rock");
        assert_eq!(out.tempo, 100.0);
        assert_eq!(out.bars, 2);
        assert_eq!(out.numerator, 4);
        assert_eq!(out.denominator, 4);
        assert_eq!(out.steps_per_bar, 16);
        assert_eq!(out.subdivision, "eighth");

        // Kick on 1 and 3, snare on 2 and 4, in both bars.
        assert_eq!(
            ticks_of(&out.midi, Voice::Kick.note()),
            vec![0, 960, 1920, 2880]
        );
        assert_eq!(
            ticks_of(&out.midi, Voice::Snare.note()),
            vec![480, 1440, 2400, 3360]
        );
        // Eighth-note hats: 8 per bar.
        assert_eq!(ticks_of(&out.midi, Voice::ClosedHat.note()).len(), 16);
        // A crash opens the pattern at standard complexity.
        assert_eq!(ticks_of(&out.midi, Voice::Crash.note()), vec![0]);
        // The rendered preview is a real, non-empty RIFF/WAVE file.
        assert_eq!(&out.wav[0..4], b"RIFF");
        assert_eq!(&out.wav[8..12], b"WAVE");
        assert!(out.wav.len() > 44 * 100);
        assert!(out.grid.contains("Kick"));
        assert!(out.grid.contains("Snare"));
    }

    #[test]
    fn the_generated_file_carries_tempo_time_signature_and_the_kit_program() {
        let opts = Options {
            genre: "waltz".into(),
            time_signature: "3/4".into(),
            tempo: 90.0,
            kit: "jazz".into(),
            ..Options::default()
        };
        let out = generate(&opts).unwrap();
        let smf = Smf::parse(&out.midi).unwrap();
        let mut tempo = None;
        let mut sig = None;
        let mut program = None;
        for ev in &smf.tracks[0] {
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(t)) => tempo = Some(t.as_int()),
                TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => sig = Some((n, d)),
                TrackEventKind::Midi {
                    message: MidiMessage::ProgramChange { program: p },
                    ..
                } => program = Some(p.as_int()),
                _ => {}
            }
        }
        assert_eq!(tempo, Some(666_667)); // 60e6 / 90
        assert_eq!(sig, Some((3, 2)));
        assert_eq!(program, Some(32)); // GM jazz kit
        assert_eq!(out.steps_per_bar, 12);
    }

    #[test]
    fn tempo_zero_uses_the_genre_tempo_and_an_explicit_tempo_overrides_it() {
        let auto = generate(&Options {
            genre: "dnb".into(),
            ..Options::default()
        })
        .unwrap();
        assert_eq!(auto.tempo, 174.0);
        let explicit = generate(&Options {
            genre: "dnb".into(),
            tempo: 90.0,
            ..Options::default()
        })
        .unwrap();
        assert_eq!(explicit.tempo, 90.0);
        assert!(explicit.seconds > auto.seconds);
    }

    #[test]
    fn complexity_adds_and_removes_voices() {
        let basic = generate(&Options {
            complexity: "basic".into(),
            ..Options::default()
        })
        .unwrap();
        let standard = generate(&Options::default()).unwrap();
        let busy = generate(&Options {
            complexity: "busy".into(),
            ..Options::default()
        })
        .unwrap();
        // basic coarsens the hat to quarters and drops the crash.
        assert_eq!(basic.subdivision, "quarter");
        assert_eq!(ticks_of(&basic.midi, Voice::ClosedHat.note()).len(), 8);
        assert!(ticks_of(&basic.midi, Voice::Crash.note()).is_empty());
        // busy refines it to sixteenths and adds kicks.
        assert_eq!(busy.subdivision, "sixteenth");
        assert_eq!(ticks_of(&busy.midi, Voice::ClosedHat.note()).len(), 32);
        assert!(
            ticks_of(&busy.midi, Voice::Kick.note()).len()
                > ticks_of(&standard.midi, Voice::Kick.note()).len()
        );
    }

    #[test]
    fn an_explicit_hat_subdivision_overrides_both_genre_and_complexity() {
        let out = generate(&Options {
            hat_subdivision: "sixteenth".into(),
            complexity: "basic".into(),
            ..Options::default()
        })
        .unwrap();
        assert_eq!(out.subdivision, "sixteenth");
        assert_eq!(out.steps_per_bar, 16);
        assert_eq!(ticks_of(&out.midi, Voice::ClosedHat.note()).len(), 32);
    }

    #[test]
    fn every_genre_time_signature_pair_generates_something_playable() {
        for genre in genre_keys() {
            for ts in time_signature_keys() {
                let out = generate(&Options {
                    genre: genre.into(),
                    time_signature: ts.into(),
                    bars: 1,
                    preview: "off".into(),
                    ..Options::default()
                })
                .unwrap_or_else(|e| panic!("{genre} in {ts} failed: {e}"));
                assert!(out.hits > 0, "{genre} in {ts} produced no hits");
                assert!(
                    Smf::parse(&out.midi).is_ok(),
                    "{genre} in {ts} wrote bad MIDI"
                );
            }
        }
    }

    #[test]
    fn swing_delays_the_offbeats_only() {
        let straight = generate(&Options {
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        let swung = generate(&Options {
            swing: 60.0,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        let a = ticks_of(&straight.midi, Voice::ClosedHat.note());
        let b = ticks_of(&swung.midi, Voice::ClosedHat.note());
        assert_eq!(a.len(), b.len());
        // Downbeat eighths stay put; offbeat eighths move later by 60% of half a step.
        assert_eq!(a[0], b[0]);
        assert_eq!(b[1] - a[1], 72);
        // Backbeat snares are on the beat, so swing leaves them alone.
        assert_eq!(
            ticks_of(&straight.midi, Voice::Snare.note()),
            ticks_of(&swung.midi, Voice::Snare.note())
        );
    }

    #[test]
    fn humanize_is_deterministic_for_a_seed_and_changes_with_it() {
        let a = generate(&Options {
            humanize: 60.0,
            seed: 7,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        let b = generate(&Options {
            humanize: 60.0,
            seed: 7,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        let c = generate(&Options {
            humanize: 60.0,
            seed: 8,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        assert_eq!(a.midi, b.midi, "same seed must be byte-identical");
        assert_ne!(a.midi, c.midi, "a different seed must vary the timing");
        // Straight output is untouched at humanize = 0.
        let plain = generate(&Options {
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        assert_ne!(plain.midi, a.midi);
    }

    #[test]
    fn fills_replace_the_last_beat_and_land_a_crash_on_the_next_bar() {
        let out = generate(&Options {
            bars: 4,
            fill_every: 2,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        // Bar 2 (ticks 1920..3840): its last quarter (3360..3840) is the fill.
        let toms: Vec<u32> = read_notes(&out.midi)
            .into_iter()
            .filter(|(_, k, _)| [45u8, 47, 50].contains(k))
            .map(|(t, _, _)| t)
            .collect();
        assert!(
            toms.iter().any(|t| (3360..3840).contains(t)),
            "expected tom hits in the fill window, got {toms:?}"
        );
        // No hats survive inside the fill window.
        assert!(!ticks_of(&out.midi, Voice::ClosedHat.note())
            .iter()
            .any(|t| (3360..3840).contains(t)));
        // A crash marks the downbeat of the bar after each fill.
        let crashes = ticks_of(&out.midi, Voice::Crash.note());
        assert!(
            crashes.contains(&3840),
            "expected a crash at 3840, got {crashes:?}"
        );
    }

    #[test]
    fn preview_modes_control_what_gets_rendered() {
        let off = generate(&Options {
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        assert!(off.wav.is_empty());
        let click = generate(&Options {
            preview: "click".into(),
            ..Options::default()
        })
        .unwrap();
        let drums = generate(&Options::default()).unwrap();
        let both = generate(&Options {
            preview: "drums-and-click".into(),
            ..Options::default()
        })
        .unwrap();
        for w in [&click.wav, &drums.wav, &both.wav] {
            assert_eq!(&w[0..4], b"RIFF");
            assert_eq!(&w[8..12], b"WAVE");
        }
        // Same length, different content: a bare click is quieter than the kit.
        assert_eq!(click.wav.len(), drums.wav.len());
        assert_ne!(click.wav, drums.wav);
        assert_ne!(both.wav, drums.wav);
        assert!(peak(&drums.wav) > peak(&click.wav));
    }

    fn peak(wav: &[u8]) -> i32 {
        wav[44..]
            .chunks_exact(2)
            .map(|c| (i16::from_le_bytes([c[0], c[1]]) as i32).abs())
            .max()
            .unwrap_or(0)
    }

    #[test]
    fn a_long_slow_pattern_truncates_only_the_preview_not_the_midi() {
        let out = generate(&Options {
            bars: 32,
            tempo: 60.0,
            ..Options::default()
        })
        .unwrap();
        assert_eq!(out.bars, 32);
        assert!(out.seconds > 100.0);
        assert!(out.preview_truncated);
        assert!(out.preview_bars < 32);
        assert!(out.preview_seconds <= MAX_PREVIEW_SECONDS + 1.5);
        // The MIDI still covers all 32 bars.
        let last = ticks_of(&out.midi, Voice::Snare.note()).pop().unwrap();
        assert!(last > 31 * 1920);
        assert!(out.grid_truncated);
        assert!(out.grid.contains("first 8 of 32 bars"));
    }

    #[test]
    fn the_ascii_grid_marks_accents_ghosts_and_rests() {
        let out = generate(&Options {
            genre: "funk".into(),
            bars: 1,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        let kick = out
            .grid
            .lines()
            .find(|l| l.starts_with("Kick"))
            .expect("a kick row");
        assert!(kick.contains('x'));
        assert!(kick.contains('-'));
        let snare = out
            .grid
            .lines()
            .find(|l| l.starts_with("Snare"))
            .expect("a snare row");
        assert!(snare.contains('X'), "backbeats are accents: {snare}");
        assert!(snare.contains('o'), "funk ghost notes: {snare}");
    }

    #[test]
    fn velocity_scales_accents_and_ghosts_together() {
        let quiet = generate(&Options {
            genre: "funk".into(),
            velocity: 60,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        let loud = generate(&Options {
            genre: "funk".into(),
            velocity: 110,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        let q: u8 = read_notes(&quiet.midi).iter().map(|n| n.2).max().unwrap();
        let l: u8 = read_notes(&loud.midi).iter().map(|n| n.2).max().unwrap();
        assert!(l > q, "expected louder output at velocity 110: {l} vs {q}");
        assert!(l <= 127);
    }

    #[test]
    fn open_hats_silence_a_closed_hat_on_the_same_step() {
        let out = generate(&Options {
            genre: "disco".into(),
            bars: 1,
            preview: "off".into(),
            ..Options::default()
        })
        .unwrap();
        let open = ticks_of(&out.midi, Voice::OpenHat.note());
        let closed = ticks_of(&out.midi, Voice::ClosedHat.note());
        assert!(!open.is_empty());
        for t in &open {
            assert!(
                !closed.contains(t),
                "closed hat left on top of an open hat at {t}"
            );
        }
    }

    #[test]
    fn the_wav_header_describes_mono_16_bit_pcm_at_the_documented_rate() {
        let out = generate(&Options {
            bars: 1,
            ..Options::default()
        })
        .unwrap();
        let w = &out.wav;
        assert_eq!(u16::from_le_bytes([w[20], w[21]]), 1); // PCM
        assert_eq!(u16::from_le_bytes([w[22], w[23]]), 1); // mono
        assert_eq!(
            u32::from_le_bytes([w[24], w[25], w[26], w[27]]),
            SAMPLE_RATE
        );
        assert_eq!(u16::from_le_bytes([w[34], w[35]]), 16); // bit depth
        let data_len = u32::from_le_bytes([w[40], w[41], w[42], w[43]]) as usize;
        assert_eq!(data_len, w.len() - 44);
    }

    #[test]
    fn the_same_options_always_produce_byte_identical_output() {
        let opts = Options {
            genre: "trap".into(),
            swing: 25.0,
            humanize: 40.0,
            seed: 99,
            ..Options::default()
        };
        let a = generate(&opts).unwrap();
        let b = generate(&opts).unwrap();
        assert_eq!(a.midi, b.midi);
        assert_eq!(a.wav, b.wav);
    }

    // --- errors -----------------------------------------------------------

    #[test]
    fn every_kit_writes_its_general_midi_program_change() {
        for key in kit_keys() {
            let out = generate(&Options {
                kit: key.into(),
                bars: 1,
                preview: "off".into(),
                ..Options::default()
            })
            .unwrap_or_else(|e| panic!("kit {key} failed: {e}"));
            let smf = Smf::parse(&out.midi).unwrap();
            let program = smf.tracks[0].iter().find_map(|ev| match ev.kind {
                TrackEventKind::Midi {
                    message: MidiMessage::ProgramChange { program },
                    ..
                } => Some(program.as_int()),
                _ => None,
            });
            assert_eq!(program, Some(kit_program(key).unwrap()), "kit {key}");
            assert_ne!(kit_label(key), key, "kit {key} needs a display label");
        }
    }

    #[test]
    fn rejects_an_unknown_genre() {
        let err = generate(&Options {
            genre: "polka".into(),
            ..Options::default()
        })
        .unwrap_err();
        assert!(err.contains("unknown genre 'polka'"), "{err}");
        assert!(
            err.contains("boom-bap"),
            "the error lists the choices: {err}"
        );
    }

    #[test]
    fn rejects_an_unknown_time_signature() {
        let err = generate(&Options {
            time_signature: "9/8".into(),
            ..Options::default()
        })
        .unwrap_err();
        assert!(err.contains("unknown time_signature '9/8'"), "{err}");
    }

    #[test]
    fn rejects_a_grid_that_cannot_divide_the_bar() {
        let err = generate(&Options {
            time_signature: "7/8".into(),
            hat_subdivision: "quarter".into(),
            ..Options::default()
        })
        .unwrap_err();
        assert!(err.contains("cannot be divided evenly"), "{err}");
        assert!(err.contains("sixteenth"), "the error suggests a fix: {err}");
    }

    #[test]
    fn rejects_out_of_range_settings() {
        for (opts, needle) in [
            (
                Options {
                    bars: 0,
                    ..Options::default()
                },
                "bars must be between 1 and 64",
            ),
            (
                Options {
                    bars: 200,
                    ..Options::default()
                },
                "bars must be between 1 and 64",
            ),
            (
                Options {
                    tempo: 5.0,
                    ..Options::default()
                },
                "tempo must be 0",
            ),
            (
                Options {
                    swing: 90.0,
                    ..Options::default()
                },
                "swing must be between 0 and 75",
            ),
            (
                Options {
                    humanize: 150.0,
                    ..Options::default()
                },
                "humanize must be between 0 and 100",
            ),
            (
                Options {
                    kit: "gabber".into(),
                    ..Options::default()
                },
                "unknown kit 'gabber'",
            ),
            (
                Options {
                    complexity: "insane".into(),
                    ..Options::default()
                },
                "unknown complexity 'insane'",
            ),
            (
                Options {
                    preview: "video".into(),
                    ..Options::default()
                },
                "unknown preview 'video'",
            ),
            (
                Options {
                    hat_subdivision: "octuplet".into(),
                    ..Options::default()
                },
                "unknown hat_subdivision 'octuplet'",
            ),
        ] {
            let err = generate(&opts).unwrap_err();
            assert!(err.contains(needle), "expected '{needle}' in '{err}'");
        }
    }

    #[test]
    fn page_field_helpers_treat_blank_as_the_default() {
        assert_eq!(parse_field("bars", "", 2u32).unwrap(), 2);
        assert_eq!(parse_field("bars", " 8 ", 2u32).unwrap(), 8);
        assert!(parse_field("bars", "many", 2u32).is_err());
        assert_eq!(or_default("", "rock"), "rock");
        assert_eq!(or_default(" funk ", "rock"), "funk");
    }
}
