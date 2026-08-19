//! gizza-ai/midi-tempo-change core — rewrite the tempo of a Standard MIDI File
//! (SMF, `.mid`/`.midi`) without touching a single note. Every note-on,
//! note-off, controller, program change, track name and time signature is
//! carried through byte-for-byte; only the tempo meta events (microseconds per
//! quarter note) are recomputed, so the musical relationships between notes —
//! their order, their relative lengths, their velocities — survive exactly.
//!
//! Two ways to ask for the change:
//!   * `set-bpm` — give a target BPM and the file's FIRST tempo becomes it.
//!   * `scale`   — give a speed multiplier (1.5 = 50% faster, 0.5 = half speed).
//!
//! and two ways to treat a file that already changes tempo as it plays:
//!   * `tempo_map = "scale"`   — every tempo event is scaled by the same ratio,
//!     so ritardandos/accelerandos keep their shape (default);
//!   * `tempo_map = "flatten"` — the whole map is replaced by ONE constant tempo.
//!
//! `keep_duration` is the renotation case: the tempo number changes and every
//! event tick is rescaled by the same ratio, so the file still plays for exactly
//! as long as it did — used to line a part up with a project's BPM without
//! altering how it sounds.
//!
//! Pure Rust (`midly`); no wafer/wasm-bindgen deps, so the same code runs in
//! chat, the CLI and the browser page.

use midly::num::{u24, u28};
use midly::{Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// Largest decoded MIDI file accepted (4 MiB). A dense orchestral score is a
/// few hundred KB; the cap keeps a pathological upload out of browser memory.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// Lowest target tempo accepted by `set-bpm`.
pub const MIN_BPM: f64 = 20.0;
/// Highest target tempo accepted by `set-bpm`.
pub const MAX_BPM: f64 = 400.0;
/// Slowest speed multiplier accepted by `scale` (10× slower).
pub const MIN_FACTOR: f64 = 0.1;
/// Fastest speed multiplier accepted by `scale` (10× faster).
pub const MAX_FACTOR: f64 = 10.0;

/// A MIDI tempo meta event stores microseconds per quarter note in 24 bits.
const MIN_TEMPO_US: f64 = 1.0;
const MAX_TEMPO_US: f64 = 16_777_215.0;
/// MIDI delta times are variable-length 28-bit integers.
const MAX_DELTA: u64 = 0x0FFF_FFFF;
/// The MIDI default when a file carries no tempo event at all: 120 BPM.
const DEFAULT_TEMPO_US: u32 = 500_000;

// ----------------------------------------------------------------------------
// Options
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Target an absolute BPM; the file's first tempo becomes it.
    SetBpm,
    /// Multiply the existing tempo by a factor.
    Scale,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TempoMap {
    /// Scale every tempo event by the same ratio (keeps tempo changes).
    Scale,
    /// Replace the whole tempo map with one constant tempo.
    Flatten,
}

#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub mode: Mode,
    /// Target tempo for `Mode::SetBpm`, in quarter-note beats per minute.
    pub bpm: f64,
    /// Speed multiplier for `Mode::Scale`.
    pub factor: f64,
    pub tempo_map: TempoMap,
    /// Rescale every event tick by the same ratio so the playing time is unchanged.
    pub keep_duration: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::SetBpm,
            bpm: 120.0,
            factor: 1.0,
            tempo_map: TempoMap::Scale,
            keep_duration: false,
        }
    }
}

impl Options {
    /// Build options from the surface-level (string/number/bool) parameters the
    /// chat schema, the CLI and the page all pass, so every surface validates
    /// identically and gives the same error text.
    pub fn parse(
        mode: &str,
        bpm: f64,
        factor: f64,
        tempo_map: &str,
        keep_duration: bool,
    ) -> Result<Self, String> {
        let mode = match trimmed_or(mode, "set-bpm") {
            "set-bpm" => Mode::SetBpm,
            "scale" => Mode::Scale,
            other => {
                return Err(format!(
                    "unknown mode '{other}' (use set-bpm or scale)"
                ))
            }
        };
        let tempo_map = match trimmed_or(tempo_map, "scale") {
            "scale" => TempoMap::Scale,
            "flatten" => TempoMap::Flatten,
            other => {
                return Err(format!(
                    "unknown tempo_map '{other}' (use scale or flatten)"
                ))
            }
        };
        if mode == Mode::SetBpm && !(MIN_BPM..=MAX_BPM).contains(&bpm) {
            return Err(format!(
                "bpm must be between {MIN_BPM} and {MAX_BPM} (got {bpm})"
            ));
        }
        if mode == Mode::Scale && !(MIN_FACTOR..=MAX_FACTOR).contains(&factor) {
            return Err(format!(
                "factor must be between {MIN_FACTOR} and {MAX_FACTOR} (got {factor})"
            ));
        }
        Ok(Self {
            mode,
            bpm,
            factor,
            tempo_map,
            keep_duration,
        })
    }
}

fn trimmed_or<'a>(s: &'a str, fallback: &'a str) -> &'a str {
    let t = s.trim();
    if t.is_empty() {
        fallback
    } else {
        t
    }
}

/// Parse a numeric field from a page/CLI string; an empty value keeps `default`.
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

// ----------------------------------------------------------------------------
// Output
// ----------------------------------------------------------------------------

/// The retimed file plus the numbers every surface reports.
#[derive(Clone, Debug)]
pub struct Output {
    /// The complete Standard MIDI File bytes.
    pub midi: Vec<u8>,
    /// The first tempo of the INPUT file, in BPM (120 when it carried none).
    pub original_bpm: f64,
    /// The first tempo of the OUTPUT file, in BPM (as actually stored).
    pub new_bpm: f64,
    /// Speed ratio applied — 2.0 is twice as fast.
    pub ratio: f64,
    /// Tempo events found in the input.
    pub tempo_events_in: usize,
    /// Tempo events present in the output.
    pub tempo_events_out: usize,
    pub tracks: usize,
    /// Ticks per quarter note (unchanged; the note grid is never re-gridded).
    pub ppq: u16,
    /// Note-on events with velocity > 0 (identical in and out).
    pub notes: usize,
    pub seconds_before: f64,
    pub seconds_after: f64,
    pub keep_duration: bool,
    pub flattened: bool,
}

impl Output {
    /// One-line human/LLM summary — also what the CLI prints.
    pub fn summary(&self) -> String {
        let mut s = format!(
            "Tempo {:.2} → {:.2} BPM ({:.3}x). {} tempo event(s) in, {} out; {} track(s), {} PPQ, {} note(s) unchanged. Playing time {:.3} s → {:.3} s. MIDI file: {} bytes.",
            self.original_bpm,
            self.new_bpm,
            self.ratio,
            self.tempo_events_in,
            self.tempo_events_out,
            self.tracks,
            self.ppq,
            self.notes,
            self.seconds_before,
            self.seconds_after,
            self.midi.len()
        );
        if self.flattened {
            s.push_str(" Tempo map flattened to one constant tempo.");
        }
        if self.keep_duration {
            s.push_str(" Note positions rescaled so the playing time is unchanged.");
        }
        s
    }
}

// ----------------------------------------------------------------------------
// Public entry points
// ----------------------------------------------------------------------------

/// Retime MIDI `input` given as base64 or hex text (per `encoding`).
///
/// `encoding`: `"auto"` (default) reads hex when the input is all hex digits
/// with an even length, otherwise base64; `"base64"` / `"hex"` force one.
pub fn change_tempo(input: &str, encoding: &str, opts: &Options) -> Result<Output, String> {
    let bytes = decode_bytes(input, encoding)?;
    change_tempo_bytes(&bytes, opts)
}

/// Retime already-decoded MIDI `bytes`.
pub fn change_tempo_bytes(bytes: &[u8], opts: &Options) -> Result<Output, String> {
    if bytes.is_empty() {
        return Err("no MIDI data: input is empty".into());
    }
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "MIDI file is too large: {} bytes (limit {MAX_INPUT_BYTES} bytes)",
            bytes.len()
        ));
    }
    if bytes.len() < 4 || &bytes[..4] != b"MThd" {
        return Err(
            "not a Standard MIDI File (missing the 'MThd' header chunk). This tool reads .mid / .midi files — export or convert your sequence to a Standard MIDI File first."
                .into(),
        );
    }
    let smf = Smf::parse(bytes).map_err(|e| format!("failed to parse MIDI file: {e}"))?;
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => t.as_int(),
        Timing::Timecode(..) => {
            return Err(
                "this file uses SMPTE timecode division, where playback speed comes from the frame rate rather than tempo events, so there is no tempo to change — export it as a metrical (ticks-per-quarter-note) MIDI file first."
                    .into(),
            )
        }
    };
    if ppq == 0 {
        return Err("this file declares 0 ticks per quarter note, so it has no usable time base".into());
    }
    if smf.tracks.is_empty() {
        return Err("this MIDI file has no tracks".into());
    }

    // Absolute-tick working copy: every event keeps its kind, so nothing but the
    // tempo (and, with keep_duration, the tick positions) can change.
    let mut abs = to_abs(&smf.tracks);
    let before = scan(&abs);
    let original_us = before
        .tempos
        .first()
        .map(|&(_, us)| us)
        .unwrap_or(DEFAULT_TEMPO_US)
        .max(1);
    let original_bpm = 60_000_000.0 / original_us as f64;

    let ratio = match opts.mode {
        Mode::SetBpm => {
            if !(MIN_BPM..=MAX_BPM).contains(&opts.bpm) {
                return Err(format!(
                    "bpm must be between {MIN_BPM} and {MAX_BPM} (got {})",
                    opts.bpm
                ));
            }
            opts.bpm / original_bpm
        }
        Mode::Scale => {
            if !(MIN_FACTOR..=MAX_FACTOR).contains(&opts.factor) {
                return Err(format!(
                    "factor must be between {MIN_FACTOR} and {MAX_FACTOR} (got {})",
                    opts.factor
                ));
            }
            opts.factor
        }
    };
    if !ratio.is_finite() || ratio <= 0.0 {
        return Err(format!("the requested tempo change is not a usable ratio ({ratio})"));
    }

    // --- rewrite the tempo map -------------------------------------------------
    let new_first_us = tempo_us(original_us as f64 / ratio)?;
    let flattened = opts.tempo_map == TempoMap::Flatten;
    if flattened {
        for track in abs.iter_mut() {
            track.retain(|(_, kind)| !matches!(kind, TrackEventKind::Meta(MetaMessage::Tempo(_))));
        }
        abs[0].insert(
            0,
            (0, TrackEventKind::Meta(MetaMessage::Tempo(u24::from(new_first_us)))),
        );
    } else if before.tempos.is_empty() {
        // No tempo event at all: the file plays at the implied 120 BPM, so the
        // only way to retime it is to write the tempo it never had.
        abs[0].insert(
            0,
            (0, TrackEventKind::Meta(MetaMessage::Tempo(u24::from(new_first_us)))),
        );
    } else {
        for track in abs.iter_mut() {
            for (_, kind) in track.iter_mut() {
                let current = match kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(us)) => Some(us.as_int()),
                    _ => None,
                };
                if let Some(us) = current {
                    let scaled = tempo_us(us as f64 / ratio)?;
                    *kind = TrackEventKind::Meta(MetaMessage::Tempo(u24::from(scaled)));
                }
            }
        }
    }

    // --- optionally renotate so the playing time is unchanged -------------------
    if opts.keep_duration {
        for track in abs.iter_mut() {
            for (tick, _) in track.iter_mut() {
                let scaled = (*tick as f64 * ratio).round();
                if !scaled.is_finite() || scaled < 0.0 || scaled > u32::MAX as f64 {
                    return Err(
                        "the rescaled note positions do not fit in a MIDI tick range — use a smaller change, or turn keep_duration off"
                            .into(),
                    );
                }
                *tick = scaled as u64;
            }
        }
    }

    // --- serialize -------------------------------------------------------------
    let mut out_tracks: Vec<Vec<TrackEvent>> = Vec::with_capacity(abs.len());
    for track in abs {
        let mut prev = 0u64;
        let mut events = Vec::with_capacity(track.len());
        for (tick, kind) in track {
            // Rounding is monotonic, but clamp anyway so a delta can never wrap.
            let tick = tick.max(prev);
            let delta = tick - prev;
            if delta > MAX_DELTA {
                return Err(
                    "the rescaled note positions overflow a 28-bit MIDI delta time — use a smaller change, or turn keep_duration off"
                        .into(),
                );
            }
            events.push(TrackEvent {
                delta: u28::from(delta as u32),
                kind,
            });
            prev = tick;
        }
        out_tracks.push(events);
    }
    let new_smf = Smf {
        header: Header::new(smf.header.format, smf.header.timing),
        tracks: out_tracks,
    };
    let mut midi = Vec::new();
    new_smf
        .write_std(&mut midi)
        .map_err(|e| format!("failed to write the MIDI file: {e}"))?;

    // Re-read what we wrote: the reported numbers then describe the actual file,
    // not our intent, and a malformed result would be caught right here.
    let written = Smf::parse(&midi).map_err(|e| format!("the retimed file failed to re-parse: {e}"))?;
    let after_abs = to_abs(&written.tracks);
    let after = scan(&after_abs);
    let new_us = after
        .tempos
        .first()
        .map(|&(_, us)| us)
        .unwrap_or(DEFAULT_TEMPO_US)
        .max(1);

    Ok(Output {
        original_bpm,
        new_bpm: 60_000_000.0 / new_us as f64,
        ratio,
        tempo_events_in: before.tempos.len(),
        tempo_events_out: after.tempos.len(),
        tracks: written.tracks.len(),
        ppq,
        notes: before.notes,
        seconds_before: seconds(&before.tempos, ppq, before.end_tick),
        seconds_after: seconds(&after.tempos, ppq, after.end_tick),
        keep_duration: opts.keep_duration,
        flattened,
        midi,
    })
}

// ----------------------------------------------------------------------------
// MIDI helpers
// ----------------------------------------------------------------------------

/// Per-track events keyed by ABSOLUTE tick (deltas are re-derived on write).
fn to_abs<'a>(tracks: &[Vec<TrackEvent<'a>>]) -> Vec<Vec<(u64, TrackEventKind<'a>)>> {
    tracks
        .iter()
        .map(|track| {
            let mut tick = 0u64;
            track
                .iter()
                .map(|ev| {
                    tick += ev.delta.as_int() as u64;
                    (tick, ev.kind)
                })
                .collect()
        })
        .collect()
}

struct Scan {
    /// `(tick, microseconds per quarter note)`, chronological.
    tempos: Vec<(u64, u32)>,
    /// Last tick used by any track — the end of the file.
    end_tick: u64,
    /// Note-on events with velocity > 0.
    notes: usize,
}

fn scan(abs: &[Vec<(u64, TrackEventKind)>]) -> Scan {
    let mut tempos = Vec::new();
    let mut end_tick = 0u64;
    let mut notes = 0usize;
    for track in abs {
        for (tick, kind) in track {
            match kind {
                TrackEventKind::Meta(MetaMessage::Tempo(us)) => tempos.push((*tick, us.as_int())),
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { vel, .. },
                    ..
                } if vel.as_int() > 0 => notes += 1,
                _ => {}
            }
            end_tick = end_tick.max(*tick);
        }
    }
    tempos.sort_by_key(|(t, _)| *t);
    Scan {
        tempos,
        end_tick,
        notes,
    }
}

/// Round a microseconds-per-quarter-note value into the 24-bit field, or say why
/// the requested tempo cannot be stored in a MIDI file at all.
fn tempo_us(us: f64) -> Result<u32, String> {
    let r = us.round();
    if !r.is_finite() || !(MIN_TEMPO_US..=MAX_TEMPO_US).contains(&r) {
        let bpm = if r > 0.0 { 60_000_000.0 / r } else { f64::INFINITY };
        return Err(format!(
            "the resulting tempo ({bpm:.2} BPM) is outside the range a MIDI tempo event can store (3.58 to 60000000 BPM)"
        ));
    }
    Ok(r as u32)
}

/// Absolute tick → seconds, integrating over the tempo map (the MIDI default of
/// 500000 µs per quarter = 120 BPM applies until the first tempo event).
fn seconds(tempos: &[(u64, u32)], ppq: u16, tick: u64) -> f64 {
    let ppq = ppq.max(1) as f64;
    let mut secs = 0.0f64;
    let mut prev_tick = 0u64;
    let mut cur_us = DEFAULT_TEMPO_US as f64;
    for &(t, us) in tempos {
        if t >= tick {
            break;
        }
        secs += (t - prev_tick) as f64 * (cur_us / 1_000_000.0) / ppq;
        prev_tick = t;
        cur_us = us as f64;
    }
    secs += (tick - prev_tick) as f64 * (cur_us / 1_000_000.0) / ppq;
    secs
}

// ----------------------------------------------------------------------------
// Input decoding (base64 / hex, no crate deps so core stays dependency-light)
// ----------------------------------------------------------------------------

/// Decode `input` into raw bytes. Whitespace is ignored everywhere, and `:`/`-`
/// separators are ignored in hex.
pub fn decode_bytes(input: &str, encoding: &str) -> Result<Vec<u8>, String> {
    let enc = trimmed_or(encoding, "auto");
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("no MIDI data: input is empty".into());
    }
    let resolved = match enc {
        "auto" => {
            let no_sep: String = cleaned.chars().filter(|c| *c != ':' && *c != '-').collect();
            if !no_sep.is_empty()
                && no_sep.len() % 2 == 0
                && no_sep.chars().all(|c| c.is_ascii_hexdigit())
            {
                "hex"
            } else {
                "base64"
            }
        }
        e => e,
    };
    match resolved {
        "hex" => {
            let h: String = cleaned.chars().filter(|c| *c != ':' && *c != '-').collect();
            if h.len() % 2 != 0 {
                return Err("hex input has an odd number of digits".into());
            }
            (0..h.len())
                .step_by(2)
                .map(|i| {
                    u8::from_str_radix(&h[i..i + 2], 16)
                        .map_err(|_| format!("invalid hex byte '{}'", &h[i..i + 2]))
                })
                .collect()
        }
        "base64" => base64_decode(&cleaned),
        other => Err(format!(
            "unknown encoding '{other}' (use auto, base64, or hex)"
        )),
    }
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    fn val(c: u8) -> Option<u8> {
        match c {
            b'A'..=b'Z' => Some(c - b'A'),
            b'a'..=b'z' => Some(c - b'a' + 26),
            b'0'..=b'9' => Some(c - b'0' + 52),
            b'+' | b'-' => Some(62),
            b'/' | b'_' => Some(63),
            _ => None,
        }
    }
    let mut out = Vec::new();
    let mut buf = 0u32;
    let mut bits = 0u8;
    for &c in s.as_bytes() {
        if c == b'=' {
            break;
        }
        let v = val(c).ok_or_else(|| {
            format!("invalid base64 character '{}' — paste the file as base64 or hex", c as char)
        })?;
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    if out.is_empty() {
        return Err("no MIDI data: input decoded to zero bytes".into());
    }
    Ok(out)
}

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Format 0, 96 PPQ, one track: track name "Piano", tempo 500000 µs
    /// (120 BPM), a 4/4 time signature, then one middle-C quarter note.
    const FIXTURE: &str = "4d546864000000060000000100604d54726b0000002400ff03055069616e6f00ff510307a12000ff58040402180800903c4060803c0000ff2f00";

    fn opts() -> Options {
        Options::default()
    }

    fn tempos_of(bytes: &[u8]) -> Vec<u32> {
        let smf = Smf::parse(bytes).unwrap();
        let abs = to_abs(&smf.tracks);
        scan(&abs).tempos.into_iter().map(|(_, us)| us).collect()
    }

    #[test]
    fn set_bpm_rewrites_the_tempo_and_keeps_every_note() {
        let out = change_tempo(
            FIXTURE,
            "hex",
            &Options {
                bpm: 140.0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(&out.midi[..4], b"MThd");
        assert_eq!(out.notes, 1);
        assert_eq!(out.tracks, 1);
        assert_eq!(out.ppq, 96);
        assert_eq!(out.tempo_events_in, 1);
        assert_eq!(out.tempo_events_out, 1);
        assert!((out.original_bpm - 120.0).abs() < 1e-9);
        assert!((out.new_bpm - 140.0).abs() < 0.01);
        // 500000 µs / (140/120) = 428571 µs per quarter note.
        assert_eq!(tempos_of(&out.midi), vec![428_571]);
        // A quarter note at 96 PPQ: 0.5 s at 120 BPM, 0.4286 s at 140.
        assert!((out.seconds_before - 0.5).abs() < 1e-6);
        assert!((out.seconds_after - 0.428_571).abs() < 1e-4);
    }

    #[test]
    fn note_bytes_survive_the_round_trip() {
        let out = change_tempo(FIXTURE, "hex", &Options { bpm: 200.0, ..opts() }).unwrap();
        let before = Smf::parse(&decode_bytes(FIXTURE, "hex").unwrap()).unwrap().tracks[0]
            .iter()
            .filter(|e| matches!(e.kind, TrackEventKind::Midi { .. }))
            .map(|e| (e.delta.as_int(), format!("{:?}", e.kind)))
            .collect::<Vec<_>>();
        let after = Smf::parse(&out.midi).unwrap().tracks[0]
            .iter()
            .filter(|e| matches!(e.kind, TrackEventKind::Midi { .. }))
            .map(|e| (e.delta.as_int(), format!("{:?}", e.kind)))
            .collect::<Vec<_>>();
        assert_eq!(before, after, "notes must be untouched by a tempo change");
    }

    #[test]
    fn every_non_tempo_event_survives_unchanged() {
        let out = change_tempo(FIXTURE, "hex", &Options { bpm: 90.0, ..opts() }).unwrap();
        let strip = |bytes: &[u8]| -> Vec<(u32, String)> {
            Smf::parse(bytes).unwrap().tracks[0]
                .iter()
                .filter(|e| !matches!(e.kind, TrackEventKind::Meta(MetaMessage::Tempo(_))))
                .map(|e| (e.delta.as_int(), format!("{:?}", e.kind)))
                .collect()
        };
        let before = strip(&decode_bytes(FIXTURE, "hex").unwrap());
        let after = strip(&out.midi);
        assert_eq!(before, after, "only tempo events may change");
        assert!(
            out.midi.windows(5).any(|w| w == b"Piano"),
            "the track name must survive in the written bytes"
        );
        assert!(
            after.iter().any(|(_, k)| k.contains("TimeSignature")),
            "the time signature must survive: {after:?}"
        );
    }

    #[test]
    fn scale_mode_multiplies_the_existing_tempo() {
        let out = change_tempo(
            FIXTURE,
            "hex",
            &Options {
                mode: Mode::Scale,
                factor: 2.0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(tempos_of(&out.midi), vec![250_000]);
        assert!((out.new_bpm - 240.0).abs() < 1e-6);
        assert!((out.seconds_after - 0.25).abs() < 1e-6);
    }

    #[test]
    fn half_speed_doubles_the_microseconds_per_beat() {
        let out = change_tempo(
            FIXTURE,
            "hex",
            &Options {
                mode: Mode::Scale,
                factor: 0.5,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(tempos_of(&out.midi), vec![1_000_000]);
        assert!((out.seconds_after - 1.0).abs() < 1e-6);
    }

    #[test]
    fn keep_duration_rescales_ticks_so_the_playing_time_is_unchanged() {
        let out = change_tempo(
            FIXTURE,
            "hex",
            &Options {
                bpm: 240.0,
                keep_duration: true,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(tempos_of(&out.midi), vec![250_000]);
        assert!(
            (out.seconds_after - out.seconds_before).abs() < 1e-6,
            "{} vs {}",
            out.seconds_after,
            out.seconds_before
        );
        // The quarter note became a half note: 96 ticks → 192 ticks.
        let smf = Smf::parse(&out.midi).unwrap();
        let note_off_delta = smf.tracks[0]
            .iter()
            .find(|e| {
                matches!(
                    e.kind,
                    TrackEventKind::Midi {
                        message: MidiMessage::NoteOff { .. },
                        ..
                    }
                )
            })
            .unwrap()
            .delta
            .as_int();
        assert_eq!(note_off_delta, 192);
    }

    /// Two tempo events (120 then 60 BPM) in one track, one note under each.
    fn two_tempo_file() -> Vec<u8> {
        let mut smf = Smf::new(Header::new(
            midly::Format::SingleTrack,
            Timing::Metrical(midly::num::u15::from(96)),
        ));
        let mut track = Vec::new();
        track.push(TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(500_000))),
        });
        track.push(TrackEvent {
            delta: u28::from(96),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(1_000_000))),
        });
        track.push(TrackEvent {
            delta: u28::from(96),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        });
        smf.tracks.push(track);
        let mut out = Vec::new();
        smf.write_std(&mut out).unwrap();
        out
    }

    #[test]
    fn tempo_map_scale_keeps_every_tempo_change() {
        let out = change_tempo_bytes(
            &two_tempo_file(),
            &Options {
                mode: Mode::Scale,
                factor: 2.0,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out.tempo_events_in, 2);
        assert_eq!(tempos_of(&out.midi), vec![250_000, 500_000]);
    }

    #[test]
    fn tempo_map_flatten_replaces_the_map_with_one_tempo() {
        let out = change_tempo_bytes(
            &two_tempo_file(),
            &Options {
                bpm: 100.0,
                tempo_map: TempoMap::Flatten,
                ..opts()
            },
        )
        .unwrap();
        assert_eq!(out.tempo_events_in, 2);
        assert_eq!(out.tempo_events_out, 1);
        assert_eq!(tempos_of(&out.midi), vec![600_000]);
        assert!((out.new_bpm - 100.0).abs() < 1e-6);
        assert!(out.summary().contains("flattened"));
    }

    #[test]
    fn a_file_without_a_tempo_event_gets_one() {
        let mut smf = Smf::new(Header::new(
            midly::Format::SingleTrack,
            Timing::Metrical(midly::num::u15::from(96)),
        ));
        smf.tracks.push(vec![TrackEvent {
            delta: u28::from(96),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        }]);
        let mut bytes = Vec::new();
        smf.write_std(&mut bytes).unwrap();

        let out = change_tempo_bytes(&bytes, &Options { bpm: 60.0, ..opts() }).unwrap();
        assert_eq!(out.tempo_events_in, 0);
        assert_eq!(out.tempo_events_out, 1);
        // The implied default is 120 BPM, so 60 BPM is 1000000 µs per quarter.
        assert_eq!(tempos_of(&out.midi), vec![1_000_000]);
        assert!((out.original_bpm - 120.0).abs() < 1e-9);
    }

    #[test]
    fn base64_input_is_auto_detected() {
        let b64 = "TVRoZAAAAAYAAAABAGBNVHJrAAAAJAD/AwVQaWFubwD/UQMHoSAA/1gEBAIYCACQPEBggDwAAP8vAA==";
        let out = change_tempo(b64, "auto", &Options { bpm: 240.0, ..opts() }).unwrap();
        assert_eq!(tempos_of(&out.midi), vec![250_000]);
        assert_eq!(out.notes, 1);
    }

    #[test]
    fn summary_reports_the_change() {
        let out = change_tempo(FIXTURE, "hex", &Options { bpm: 140.0, ..opts() }).unwrap();
        assert_eq!(
            out.summary(),
            "Tempo 120.00 → 140.00 BPM (1.167x). 1 tempo event(s) in, 1 out; 1 track(s), 96 PPQ, 1 note(s) unchanged. Playing time 0.500 s → 0.429 s. MIDI file: 58 bytes."
        );
    }

    // --- errors -------------------------------------------------------------

    #[test]
    fn empty_input_is_rejected() {
        let err = change_tempo("   ", "auto", &opts()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn non_midi_input_is_rejected() {
        let err = change_tempo("68656c6c6f20776f726c64", "hex", &opts()).unwrap_err();
        assert!(err.contains("MThd"), "{err}");
    }

    #[test]
    fn odd_hex_input_is_rejected() {
        let err = change_tempo("4d5468640", "hex", &opts()).unwrap_err();
        assert!(err.contains("odd number of digits"), "{err}");
    }

    #[test]
    fn smpte_timecode_files_are_rejected_with_an_explanation() {
        let mut smf = Smf::new(Header::new(
            midly::Format::SingleTrack,
            Timing::Timecode(midly::Fps::Fps25, 40),
        ));
        smf.tracks.push(vec![TrackEvent {
            delta: u28::from(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        }]);
        let mut bytes = Vec::new();
        smf.write_std(&mut bytes).unwrap();
        let err = change_tempo_bytes(&bytes, &opts()).unwrap_err();
        assert!(err.contains("SMPTE"), "{err}");
    }

    #[test]
    fn out_of_range_bpm_is_rejected() {
        let err = Options::parse("set-bpm", 1000.0, 1.0, "scale", false).unwrap_err();
        assert!(err.contains("bpm must be between"), "{err}");
        let err = change_tempo(FIXTURE, "hex", &Options { bpm: 0.5, ..opts() }).unwrap_err();
        assert!(err.contains("bpm must be between"), "{err}");
    }

    #[test]
    fn out_of_range_factor_is_rejected() {
        let err = Options::parse("scale", 120.0, 50.0, "scale", false).unwrap_err();
        assert!(err.contains("factor must be between"), "{err}");
    }

    #[test]
    fn unknown_mode_and_tempo_map_are_rejected() {
        assert!(Options::parse("warp", 120.0, 1.0, "scale", false)
            .unwrap_err()
            .contains("unknown mode"));
        assert!(Options::parse("set-bpm", 120.0, 1.0, "melt", false)
            .unwrap_err()
            .contains("unknown tempo_map"));
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = vec![0u8; MAX_INPUT_BYTES + 1];
        let err = change_tempo_bytes(&big, &opts()).unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }

    #[test]
    fn parse_accepts_the_documented_vocabulary() {
        let o = Options::parse("scale", 90.0, 1.25, "flatten", true).unwrap();
        assert_eq!(o.mode, Mode::Scale);
        assert_eq!(o.tempo_map, TempoMap::Flatten);
        assert!(o.keep_duration);
        assert!((o.factor - 1.25).abs() < 1e-9);
    }

    #[test]
    fn truthy_and_parse_field_follow_the_page_conventions() {
        assert!(truthy("true", false));
        assert!(truthy("", true));
        assert!(!truthy("false", true));
        assert_eq!(parse_field("bpm", "", 120.0).unwrap(), 120.0);
        assert_eq!(parse_field("bpm", " 90 ", 120.0).unwrap(), 90.0);
        assert!(parse_field("bpm", "fast", 120.0).is_err());
    }
}
