//! gizza-ai/midi-note-extract core — flatten a Standard MIDI File (SMF,
//! `.mid`/`.midi`) into ONE delimited note table: every note-on/note-off pair
//! becomes a row of track, channel, start, duration, pitch, note name and
//! velocity. Times can be emitted in seconds (from the file's tempo map),
//! raw ticks, or beats (quarter notes); velocity raw 0–127 or normalized
//! 0.0–1.0. Pure Rust (`midly`); no wafer/wasm-bindgen deps, so the same code
//! runs in chat, the CLI and the browser page.

use midly::{MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};

/// Hard cap on emitted rows — a dense orchestral score is well under this, and
/// the limit keeps a pathological file from exhausting browser memory.
pub const MAX_NOTES: usize = 50_000;

// ----------------------------------------------------------------------------
// Options
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Columns {
    /// `start,duration,pitch,velocity` — the plain four-column note list.
    Minimal,
    /// `track,channel,start,duration,pitch,note_name,velocity`.
    Standard,
    /// Everything, incl. track name, note end and the tempo in force.
    Full,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeUnit {
    Seconds,
    Ticks,
    Beats,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum VelocityScale {
    /// Raw MIDI velocity, 0–127.
    Raw,
    /// Velocity ÷ 127, 0.0–1.0.
    Normalized,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    /// Chronological across the whole file (ties broken by track, channel, pitch).
    Time,
    /// Grouped by track, chronological inside each track.
    Track,
    /// Lowest pitch first, then chronological.
    Pitch,
}

/// `all`, or an explicit set of track / channel numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Filter {
    All,
    Only(Vec<u32>),
}

impl Filter {
    fn parse(raw: &str, field: &str) -> Result<Self, String> {
        let t = raw.trim();
        if t.is_empty() || t.eq_ignore_ascii_case("all") || t == "*" {
            return Ok(Filter::All);
        }
        let mut out = Vec::new();
        for part in t.split(|c| c == ',' || c == ' ' || c == ';') {
            let p = part.trim();
            if p.is_empty() {
                continue;
            }
            let n: u32 = p.parse().map_err(|_| {
                format!("{field} must be 'all' or a comma-separated list of numbers (got '{p}')")
            })?;
            if !out.contains(&n) {
                out.push(n);
            }
        }
        if out.is_empty() {
            return Ok(Filter::All);
        }
        Ok(Filter::Only(out))
    }

    fn allows(&self, n: u32) -> bool {
        match self {
            Filter::All => true,
            Filter::Only(list) => list.contains(&n),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Options {
    pub columns: Columns,
    pub time_unit: TimeUnit,
    pub velocity_scale: VelocityScale,
    pub delimiter: char,
    pub header: bool,
    pub track: Filter,
    pub channel: Filter,
    pub decimals: usize,
    pub sort: Sort,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            columns: Columns::Standard,
            time_unit: TimeUnit::Seconds,
            velocity_scale: VelocityScale::Raw,
            delimiter: ',',
            header: true,
            track: Filter::All,
            channel: Filter::All,
            decimals: 3,
            sort: Sort::Time,
        }
    }
}

impl Options {
    /// Build options from the surface-level (string/bool/number) parameters that
    /// the chat schema, the CLI and the page all pass.
    #[allow(clippy::too_many_arguments)]
    pub fn parse(
        columns: &str,
        time_unit: &str,
        velocity_scale: &str,
        delimiter: &str,
        header: bool,
        track: &str,
        channel: &str,
        decimals: i64,
        sort: &str,
    ) -> Result<Self, String> {
        let columns = match trimmed_or(columns, "standard") {
            "minimal" => Columns::Minimal,
            "standard" => Columns::Standard,
            "full" => Columns::Full,
            other => {
                return Err(format!(
                    "unknown columns '{other}' (use minimal, standard, or full)"
                ))
            }
        };
        let time_unit = match trimmed_or(time_unit, "seconds") {
            "seconds" => TimeUnit::Seconds,
            "ticks" => TimeUnit::Ticks,
            "beats" => TimeUnit::Beats,
            other => {
                return Err(format!(
                    "unknown time_unit '{other}' (use seconds, ticks, or beats)"
                ))
            }
        };
        let velocity_scale = match trimmed_or(velocity_scale, "raw") {
            "raw" => VelocityScale::Raw,
            "normalized" => VelocityScale::Normalized,
            other => {
                return Err(format!(
                    "unknown velocity_scale '{other}' (use raw or normalized)"
                ))
            }
        };
        let delimiter = match trimmed_or(delimiter, "comma") {
            "comma" => ',',
            "semicolon" => ';',
            "tab" => '\t',
            other => {
                return Err(format!(
                    "unknown delimiter '{other}' (use comma, semicolon, or tab)"
                ))
            }
        };
        let sort = match trimmed_or(sort, "time") {
            "time" => Sort::Time,
            "track" => Sort::Track,
            "pitch" => Sort::Pitch,
            other => {
                return Err(format!("unknown sort '{other}' (use time, track, or pitch)"))
            }
        };
        if !(0..=6).contains(&decimals) {
            return Err(format!(
                "decimals must be between 0 and 6 (got {decimals})"
            ));
        }
        Ok(Self {
            columns,
            time_unit,
            velocity_scale,
            delimiter,
            header,
            track: Filter::parse(track, "track")?,
            channel: Filter::parse(channel, "channel")?,
            decimals: decimals as usize,
            sort,
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

// ----------------------------------------------------------------------------
// Public entry points
// ----------------------------------------------------------------------------

/// Extract every note from MIDI `input` (base64 or hex text, per `encoding`).
///
/// `encoding`: `"auto"` (default) reads hex when the input is all hex digits
/// with an even length, otherwise base64; `"base64"` / `"hex"` force one.
pub fn extract(input: &str, encoding: &str, opts: &Options) -> Result<String, String> {
    let bytes = decode_bytes(input, encoding)?;
    extract_bytes(&bytes, opts)
}

/// Extract every note from already-decoded MIDI `bytes`.
pub fn extract_bytes(bytes: &[u8], opts: &Options) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("no MIDI data: input is empty".into());
    }
    if bytes.len() < 4 || &bytes[..4] != b"MThd" {
        return Err(
            "not a Standard MIDI File (missing the 'MThd' header chunk). This tool reads .mid / .midi files — export or convert your sequence to a Standard MIDI File first."
                .into(),
        );
    }
    let smf = Smf::parse(bytes).map_err(|e| format!("failed to parse MIDI file: {e}"))?;
    let timing = TimeMap::new(smf.header.timing, &smf.tracks);
    if opts.time_unit == TimeUnit::Beats && timing.ppq.is_none() {
        return Err(
            "this file uses SMPTE timecode division, which has no musical beat grid — use time_unit 'seconds' or 'ticks'"
                .into(),
        );
    }

    let (notes, names) = collect_notes(&smf, opts)?;
    Ok(render(&notes, &names, &timing, opts))
}

// ----------------------------------------------------------------------------
// Note collection
// ----------------------------------------------------------------------------

struct Note {
    track: u32,
    channel: u8,
    start_tick: u64,
    end_tick: u64,
    pitch: u8,
    velocity: u8,
}

/// Pair note-on/note-off per (channel, key) and flatten every track into one list.
fn collect_notes(smf: &Smf, opts: &Options) -> Result<(Vec<Note>, Vec<String>), String> {
    let mut notes: Vec<Note> = Vec::new();
    let mut names: Vec<String> = Vec::new();
    for (index, track) in smf.tracks.iter().enumerate() {
        let track_no = index as u32;
        let mut name = String::new();
        let mut tick = 0u64;
        // Open note-ons keyed by (channel, key) → (start_tick, velocity).
        let mut open: Vec<((u8, u8), (u64, u8))> = Vec::new();
        let keep_track = opts.track.allows(track_no);
        for ev in track {
            tick += ev.delta.as_int() as u64;
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => {
                    if name.is_empty() {
                        name = String::from_utf8_lossy(bytes).into_owned();
                    }
                }
                TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int();
                    match message {
                        MidiMessage::NoteOn { key, vel } if vel.as_int() > 0 => {
                            open.push(((ch, key.as_int()), (tick, vel.as_int())));
                        }
                        MidiMessage::NoteOff { key, .. } | MidiMessage::NoteOn { key, .. } => {
                            let k = key.as_int();
                            if let Some(pos) =
                                open.iter().position(|&((c, kk), _)| c == ch && kk == k)
                            {
                                let (_, (start, vel)) = open.remove(pos);
                                if keep_track && opts.channel.allows(ch as u32) {
                                    push_note(
                                        &mut notes,
                                        Note {
                                            track: track_no,
                                            channel: ch,
                                            start_tick: start,
                                            end_tick: tick,
                                            pitch: k,
                                            velocity: vel,
                                        },
                                    )?;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        // A note-on never closed by a note-off (a truncated or sloppily exported
        // file) still counts — it is held to the end of its track.
        for ((ch, k), (start, vel)) in open {
            if keep_track && opts.channel.allows(ch as u32) {
                push_note(
                    &mut notes,
                    Note {
                        track: track_no,
                        channel: ch,
                        start_tick: start,
                        end_tick: tick.max(start),
                        pitch: k,
                        velocity: vel,
                    },
                )?;
            }
        }
        names.push(name);
    }

    match opts.sort {
        Sort::Time => notes.sort_by_key(|n| (n.start_tick, n.track, n.channel, n.pitch)),
        Sort::Track => notes.sort_by_key(|n| (n.track, n.start_tick, n.channel, n.pitch)),
        Sort::Pitch => notes.sort_by_key(|n| (n.pitch, n.start_tick, n.track, n.channel)),
    }
    Ok((notes, names))
}

fn push_note(notes: &mut Vec<Note>, note: Note) -> Result<(), String> {
    if notes.len() >= MAX_NOTES {
        return Err(format!(
            "this file has more than {MAX_NOTES} notes, which is past this tool's limit — filter to one track or channel, or split the file"
        ));
    }
    notes.push(note);
    Ok(())
}

// ----------------------------------------------------------------------------
// Rendering
// ----------------------------------------------------------------------------

fn headers(columns: Columns) -> &'static [&'static str] {
    match columns {
        Columns::Minimal => &["start", "duration", "pitch", "velocity"],
        Columns::Standard => &[
            "track",
            "channel",
            "start",
            "duration",
            "pitch",
            "note_name",
            "velocity",
        ],
        Columns::Full => &[
            "track",
            "track_name",
            "channel",
            "start",
            "end",
            "duration",
            "pitch",
            "note_name",
            "velocity",
            "tempo_bpm",
        ],
    }
}

fn render(notes: &[Note], names: &[String], tm: &TimeMap, opts: &Options) -> String {
    let d = opts.delimiter;
    let mut out = String::new();
    if opts.header {
        out.push_str(&headers(opts.columns).join(&d.to_string()));
    }
    for n in notes {
        if !out.is_empty() {
            out.push('\n');
        }
        let start = time_value(tm, n.start_tick, opts);
        let end = time_value(tm, n.end_tick, opts);
        let duration = duration_value(tm, n.start_tick, n.end_tick, opts);
        let velocity = match opts.velocity_scale {
            VelocityScale::Raw => n.velocity.to_string(),
            VelocityScale::Normalized => fixed(n.velocity as f64 / 127.0, opts.decimals),
        };
        let row: Vec<String> = match opts.columns {
            Columns::Minimal => vec![start, duration, n.pitch.to_string(), velocity],
            Columns::Standard => vec![
                n.track.to_string(),
                n.channel.to_string(),
                start,
                duration,
                n.pitch.to_string(),
                note_name(n.pitch),
                velocity,
            ],
            Columns::Full => vec![
                n.track.to_string(),
                escape(names.get(n.track as usize).map(String::as_str).unwrap_or(""), d),
                n.channel.to_string(),
                start,
                end,
                duration,
                n.pitch.to_string(),
                note_name(n.pitch),
                velocity,
                fixed(tm.bpm_at(n.start_tick), opts.decimals),
            ],
        };
        out.push_str(&row.join(&d.to_string()));
    }
    out
}

/// A time position in the requested unit (ticks stay whole numbers).
fn time_value(tm: &TimeMap, tick: u64, opts: &Options) -> String {
    match opts.time_unit {
        TimeUnit::Ticks => tick.to_string(),
        TimeUnit::Seconds => fixed(tm.seconds(tick), opts.decimals),
        TimeUnit::Beats => fixed(tm.beats(tick), opts.decimals),
    }
}

/// A duration in the requested unit — computed from both endpoints so a tempo
/// change inside the note is accounted for.
fn duration_value(tm: &TimeMap, start: u64, end: u64, opts: &Options) -> String {
    match opts.time_unit {
        TimeUnit::Ticks => end.saturating_sub(start).to_string(),
        TimeUnit::Seconds => fixed(tm.seconds(end) - tm.seconds(start), opts.decimals),
        TimeUnit::Beats => fixed(tm.beats(end) - tm.beats(start), opts.decimals),
    }
}

fn fixed(x: f64, decimals: usize) -> String {
    let s = format!("{x:.decimals$}");
    // Avoid "-0.000" for tiny negative rounding artifacts.
    if s.trim_start_matches('-').chars().all(|c| c == '0' || c == '.') {
        s.trim_start_matches('-').to_string()
    } else {
        s
    }
}

/// RFC 4180 quoting — track names can contain the delimiter, quotes or newlines.
fn escape(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// MIDI note number → scientific pitch name (middle C = 60 = `C4`).
fn note_name(midi: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = midi as i32 / 12 - 1;
    format!("{}{}", NAMES[(midi % 12) as usize], octave)
}

// ----------------------------------------------------------------------------
// Timing / tempo map: absolute ticks → seconds and beats.
// ----------------------------------------------------------------------------

struct TimeMap {
    /// `ticks_per_quarter` for metrical timing; `None` for SMPTE timecode.
    ppq: Option<u32>,
    /// Seconds per tick for SMPTE timecode (tempo-independent); `None` otherwise.
    smpte_spt: Option<f64>,
    /// Sorted `(tick, microseconds_per_quarter)` tempo changes (metrical only).
    tempos: Vec<(u64, u32)>,
}

impl TimeMap {
    fn new(timing: Timing, tracks: &[midly::Track]) -> Self {
        match timing {
            Timing::Metrical(tpq) => {
                let mut tempos: Vec<(u64, u32)> = Vec::new();
                for track in tracks {
                    let mut tick = 0u64;
                    for ev in track {
                        tick += ev.delta.as_int() as u64;
                        if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = ev.kind {
                            tempos.push((tick, us.as_int()));
                        }
                    }
                }
                tempos.sort_by_key(|(t, _)| *t);
                Self {
                    ppq: Some(tpq.as_int() as u32),
                    smpte_spt: None,
                    tempos,
                }
            }
            Timing::Timecode(fps, subframe) => {
                let ticks_per_second = fps.as_f32() as f64 * subframe as f64;
                let spt = if ticks_per_second > 0.0 {
                    1.0 / ticks_per_second
                } else {
                    0.0
                };
                Self {
                    ppq: None,
                    smpte_spt: Some(spt),
                    tempos: Vec::new(),
                }
            }
        }
    }

    /// Absolute tick → seconds, integrating over tempo segments (500000 µs per
    /// quarter = 120 BPM until the first tempo event, the MIDI default).
    fn seconds(&self, tick: u64) -> f64 {
        if let Some(spt) = self.smpte_spt {
            return tick as f64 * spt;
        }
        let ppq = self.ppq.unwrap_or(1).max(1) as f64;
        let mut seconds = 0.0f64;
        let mut prev_tick = 0u64;
        let mut cur_us = 500_000f64;
        for &(t, us) in &self.tempos {
            if t >= tick {
                break;
            }
            seconds += (t - prev_tick) as f64 * (cur_us / 1_000_000.0) / ppq;
            prev_tick = t;
            cur_us = us as f64;
        }
        seconds += (tick - prev_tick) as f64 * (cur_us / 1_000_000.0) / ppq;
        seconds
    }

    /// Absolute tick → beats (quarter notes). Metrical files only.
    fn beats(&self, tick: u64) -> f64 {
        let ppq = self.ppq.unwrap_or(1).max(1) as f64;
        tick as f64 / ppq
    }

    /// Tempo in beats per minute in force at `tick`.
    fn bpm_at(&self, tick: u64) -> f64 {
        let mut us = 500_000f64;
        for &(t, v) in &self.tempos {
            if t > tick {
                break;
            }
            us = v as f64;
        }
        60_000_000.0 / us
    }
}

// ----------------------------------------------------------------------------
// Input decoding
// ----------------------------------------------------------------------------

fn decode_bytes(input: &str, encoding: &str) -> Result<Vec<u8>, String> {
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
        let v = val(c).ok_or_else(|| format!("invalid base64 character '{}'", c as char))?;
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use midly::num::{u15, u24, u28, u4, u7};
    use midly::{Format, Header, Track, TrackEvent};

    /// The same tiny Format-0 file the page/CLI examples use: 96 PPQ, 120 BPM,
    /// track named "Piano", one C4 (MIDI 60) at velocity 64 held for a quarter.
    const SAMPLE_HEX: &str = "4d546864000000060000000100604d54726b0000002400ff03055069616e6f00ff510307a12000ff58040402180800903c4060803c0000ff2f00";

    fn opts() -> Options {
        Options::default()
    }

    /// Two tracks: a tempo/meta track plus a two-note piano track (C4 then E4 on
    /// channel 0, and one overlapping G4 on channel 1), at 480 PPQ.
    fn two_track_midi() -> Vec<u8> {
        let meta: Track = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let piano: Track = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Lead, Piano")),
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOn {
                        key: u7::new(60),
                        vel: u7::new(100),
                    },
                },
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel: u4::new(1),
                    message: MidiMessage::NoteOn {
                        key: u7::new(67),
                        vel: u7::new(80),
                    },
                },
            },
            TrackEvent {
                delta: u28::new(480),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOff {
                        key: u7::new(60),
                        vel: u7::new(0),
                    },
                },
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOn {
                        key: u7::new(64),
                        vel: u7::new(90),
                    },
                },
            },
            TrackEvent {
                delta: u28::new(240),
                kind: TrackEventKind::Midi {
                    channel: u4::new(1),
                    message: MidiMessage::NoteOff {
                        key: u7::new(67),
                        vel: u7::new(0),
                    },
                },
            },
            TrackEvent {
                delta: u28::new(240),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOff {
                        key: u7::new(64),
                        vel: u7::new(0),
                    },
                },
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let smf = Smf {
            header: Header::new(Format::Parallel, Timing::Metrical(u15::new(480))),
            tracks: vec![meta, piano],
        };
        let mut buf = Vec::new();
        smf.write(&mut buf).unwrap();
        buf
    }

    #[test]
    fn standard_csv_happy_path() {
        let csv = extract(SAMPLE_HEX, "hex", &opts()).unwrap();
        assert_eq!(
            csv,
            "track,channel,start,duration,pitch,note_name,velocity\n0,0,0.000,0.500,60,C4,64"
        );
    }

    #[test]
    fn minimal_columns_ticks_and_no_header() {
        let o = Options::parse(
            "minimal", "ticks", "raw", "comma", false, "all", "all", 3, "time",
        )
        .unwrap();
        let csv = extract(SAMPLE_HEX, "auto", &o).unwrap();
        assert_eq!(csv, "0,96,60,64");
    }

    #[test]
    fn full_columns_quote_track_names_and_report_tempo() {
        let o = Options::parse(
            "full",
            "beats",
            "normalized",
            "comma",
            true,
            "all",
            "all",
            2,
            "track",
        )
        .unwrap();
        let csv = extract_bytes(&two_track_midi(), &o).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(
            lines[0],
            "track,track_name,channel,start,end,duration,pitch,note_name,velocity,tempo_bpm"
        );
        // The track name contains a comma, so it must be RFC 4180 quoted.
        assert_eq!(
            lines[1],
            "1,\"Lead, Piano\",0,0.00,1.00,1.00,60,C4,0.79,120.00"
        );
        // Beats: the G4 on channel 1 runs 0 → 1.5 quarter notes.
        assert_eq!(
            lines[2],
            "1,\"Lead, Piano\",1,0.00,1.50,1.50,67,G4,0.63,120.00"
        );
    }

    #[test]
    fn channel_filter_and_tab_delimiter() {
        let o = Options::parse(
            "standard", "ticks", "raw", "tab", true, "all", "1", 3, "time",
        )
        .unwrap();
        let csv = extract_bytes(&two_track_midi(), &o).unwrap();
        assert_eq!(
            csv,
            "track\tchannel\tstart\tduration\tpitch\tnote_name\tvelocity\n1\t1\t0\t720\t67\tG4\t80"
        );
    }

    #[test]
    fn track_filter_can_select_nothing_and_still_emits_the_header() {
        let o = Options::parse(
            "minimal", "seconds", "raw", "comma", true, "9", "all", 3, "time",
        )
        .unwrap();
        let csv = extract_bytes(&two_track_midi(), &o).unwrap();
        assert_eq!(csv, "start,duration,pitch,velocity");
    }

    #[test]
    fn sort_by_pitch_orders_rows_low_to_high() {
        let o = Options::parse(
            "minimal", "ticks", "raw", "comma", false, "all", "all", 3, "pitch",
        )
        .unwrap();
        let csv = extract_bytes(&two_track_midi(), &o).unwrap();
        let pitches: Vec<&str> = csv.lines().map(|l| l.split(',').nth(2).unwrap()).collect();
        assert_eq!(pitches, vec!["60", "64", "67"]);
    }

    #[test]
    fn unclosed_note_is_held_to_the_end_of_its_track() {
        // A note-on with no matching note-off (truncated export).
        let track: Track = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOn {
                        key: u7::new(60),
                        vel: u7::new(100),
                    },
                },
            },
            TrackEvent {
                delta: u28::new(960),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let smf = Smf {
            header: Header::new(Format::SingleTrack, Timing::Metrical(u15::new(480))),
            tracks: vec![track],
        };
        let mut buf = Vec::new();
        smf.write(&mut buf).unwrap();
        let o = Options::parse(
            "minimal", "ticks", "raw", "comma", false, "all", "all", 3, "time",
        )
        .unwrap();
        assert_eq!(extract_bytes(&buf, &o).unwrap(), "0,960,60,100");
    }

    #[test]
    fn base64_input_is_auto_detected() {
        let b64 = "TVRoZAAAAAYAAAABAGBNVHJrAAAAJAD/AwVQaWFubwD/UQMHoSAA/1gEBAIYCACQPEBggDwAAP8vAA==";
        let csv = extract(b64, "auto", &opts()).unwrap();
        assert!(csv.ends_with("0,0,0.000,0.500,60,C4,64"), "got: {csv}");
    }

    #[test]
    fn rejects_non_midi_bytes() {
        let err = extract_bytes(b"this is not a midi file", &opts()).unwrap_err();
        assert!(err.contains("MThd"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        assert!(extract("", "auto", &opts()).unwrap_err().contains("empty"));
    }

    #[test]
    fn rejects_unknown_option_values() {
        let err = Options::parse(
            "standard", "minutes", "raw", "comma", true, "all", "all", 3, "time",
        )
        .unwrap_err();
        assert!(err.contains("unknown time_unit"), "got: {err}");

        let err = Options::parse(
            "standard", "seconds", "raw", "comma", true, "all", "all", 9, "time",
        )
        .unwrap_err();
        assert!(err.contains("decimals must be"), "got: {err}");

        let err = Options::parse(
            "standard", "seconds", "raw", "comma", true, "x1", "all", 3, "time",
        )
        .unwrap_err();
        assert!(err.contains("track must be 'all'"), "got: {err}");
    }

    #[test]
    fn beats_need_a_metrical_file() {
        // SMPTE timecode division: 25 fps × 40 subframes = 1000 ticks/second.
        let smf = Smf {
            header: Header::new(
                Format::SingleTrack,
                Timing::Timecode(midly::Fps::Fps25, 40),
            ),
            tracks: vec![vec![TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            }]],
        };
        let mut buf = Vec::new();
        smf.write(&mut buf).unwrap();
        let beats = Options::parse(
            "minimal", "beats", "raw", "comma", true, "all", "all", 3, "time",
        )
        .unwrap();
        let err = extract_bytes(&buf, &beats).unwrap_err();
        assert!(err.contains("SMPTE timecode"), "got: {err}");
        // Seconds still work for the same file.
        assert!(extract_bytes(&buf, &opts()).is_ok());
    }

    /// The row cap is enforced at the boundary: MAX_NOTES rows are fine, one more errors.
    #[test]
    fn note_cap_boundary() {
        fn midi_with(n: usize) -> Vec<u8> {
            let mut track: Track = Vec::with_capacity(n * 2 + 1);
            for _ in 0..n {
                track.push(TrackEvent {
                    delta: u28::new(0),
                    kind: TrackEventKind::Midi {
                        channel: u4::new(0),
                        message: MidiMessage::NoteOn {
                            key: u7::new(60),
                            vel: u7::new(64),
                        },
                    },
                });
                track.push(TrackEvent {
                    delta: u28::new(1),
                    kind: TrackEventKind::Midi {
                        channel: u4::new(0),
                        message: MidiMessage::NoteOff {
                            key: u7::new(60),
                            vel: u7::new(0),
                        },
                    },
                });
            }
            track.push(TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            });
            let smf = Smf {
                header: Header::new(Format::SingleTrack, Timing::Metrical(u15::new(480))),
                tracks: vec![track],
            };
            let mut buf = Vec::new();
            smf.write(&mut buf).unwrap();
            buf
        }
        let o = Options::parse(
            "minimal", "ticks", "raw", "comma", false, "all", "all", 3, "time",
        )
        .unwrap();
        let at_cap = extract_bytes(&midi_with(MAX_NOTES), &o).unwrap();
        assert_eq!(at_cap.lines().count(), MAX_NOTES);
        let err = extract_bytes(&midi_with(MAX_NOTES + 1), &o).unwrap_err();
        assert!(err.contains("more than 50000 notes"), "got: {err}");
    }
}
