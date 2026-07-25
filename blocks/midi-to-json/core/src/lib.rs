//! gizza-ai/midi-to-json core — parse a Standard MIDI File (SMF, `.mid`/`.midi`)
//! into structured JSON: a header (format, division/PPQ, tempo & time/key-signature
//! maps), paired notes (pitch name + number, start/duration in ticks AND seconds),
//! control changes, program changes, pitch bends and meta events. Pure Rust
//! (`midly`); no wafer/wasm-bindgen deps, so it runs on every backend.

use midly::{Format, MetaMessage, MidiMessage, Smf, Timing, TrackEventKind};
use serde_json::{json, Value as Json};

/// Parse MIDI `input` (a base64 or hex string, per `encoding`) into JSON `format`.
///
/// `encoding`: `"auto"` (default) detects hex (only hex digits, even length) else
/// base64; `"base64"` / `"hex"` force one.
///
/// `format` selects the output shape:
/// - `"notes"` (default) → `{ header, tracks:[{name, notes:[…]}] }`, a musical view
///   with note-on/note-off paired into notes carrying ticks AND seconds.
/// - `"events"` → `{ …header fields, tracks:[[event,…]] }`, the raw per-track event
///   stream (loss-less: every note, CC, program change, pitch bend, meta event).
/// - `"summary"` → a compact overview (format, PPQ, BPM, duration, counts, names).
pub fn convert(input: &str, encoding: &str, format: &str) -> Result<String, String> {
    let bytes = decode_bytes(input, encoding)?;
    convert_bytes(&bytes, format)
}

/// Convert already-resolved MIDI `bytes` into JSON (used by file-input surfaces).
pub fn convert_bytes(bytes: &[u8], format: &str) -> Result<String, String> {
    if bytes.is_empty() {
        return Err("no MIDI data: input is empty".into());
    }
    if bytes.len() < 4 || &bytes[..4] != b"MThd" {
        return Err(
            "not a Standard MIDI File (missing the 'MThd' header chunk). This tool reads .mid / .midi files. RMID (RIFF-wrapped) and karaoke .kar work only if they start with a standard MThd chunk."
                .into(),
        );
    }
    let smf = Smf::parse(bytes).map_err(|e| format!("failed to parse MIDI file: {e}"))?;

    let format_num = match smf.header.format {
        Format::SingleTrack => 0,
        Format::Parallel => 1,
        Format::Sequential => 2,
    };
    let format_type = match smf.header.format {
        Format::SingleTrack => "single-track",
        Format::Parallel => "parallel",
        Format::Sequential => "sequential",
    };
    let timing = TimeMap::new(smf.header.timing, &smf.tracks);

    let mode = if format.trim().is_empty() { "notes" } else { format.trim() };
    let value = match mode {
        "notes" => build_notes(format_num, format_type, &timing, &smf),
        "events" => build_events(format_num, format_type, &timing, &smf),
        "summary" => build_summary(format_num, format_type, &timing, &smf),
        other => {
            return Err(format!(
                "unknown format '{other}' (use notes, events, or summary)"
            ))
        }
    };
    serde_json::to_string_pretty(&value).map_err(|e| format!("serialize JSON: {e}"))
}

// ----------------------------------------------------------------------------
// Timing / tempo map: convert absolute ticks → seconds.
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
                Self { ppq: Some(tpq.as_int() as u32), smpte_spt: None, tempos }
            }
            Timing::Timecode(fps, subframe) => {
                let ticks_per_second = fps.as_f32() as f64 * subframe as f64;
                let spt = if ticks_per_second > 0.0 { 1.0 / ticks_per_second } else { 0.0 };
                Self { ppq: None, smpte_spt: Some(spt), tempos: Vec::new() }
            }
        }
    }

    /// Absolute tick → seconds, integrating over tempo segments (500000 µs/quarter
    /// = 120 BPM until the first tempo event, the MIDI default).
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

    fn division_json(&self) -> Json {
        if let Some(ppq) = self.ppq {
            json!({ "type": "ticksPerQuarter", "ticksPerQuarter": ppq })
        } else {
            let tps = self.smpte_spt.map(|s| if s > 0.0 { 1.0 / s } else { 0.0 }).unwrap_or(0.0);
            json!({ "type": "smpteTimecode", "ticksPerSecond": round3(tps) })
        }
    }

    fn tempos_json(&self) -> Json {
        let list: Vec<Json> = self
            .tempos
            .iter()
            .map(|&(tick, us)| {
                json!({
                    "ticks": tick,
                    "time": round3(self.seconds(tick)),
                    "microsecondsPerQuarter": us,
                    "bpm": round3(60_000_000.0 / us as f64),
                })
            })
            .collect();
        Json::Array(list)
    }
}

// ----------------------------------------------------------------------------
// Output builders.
// ----------------------------------------------------------------------------

fn build_header(
    format_num: u8,
    format_type: &str,
    tm: &TimeMap,
    smf: &Smf,
) -> serde_json::Map<String, Json> {
    let mut time_sigs: Vec<Json> = Vec::new();
    let mut key_sigs: Vec<Json> = Vec::new();
    let mut max_tick = 0u64;
    for track in &smf.tracks {
        let mut tick = 0u64;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            max_tick = max_tick.max(tick);
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::TimeSignature(num, denom_pow, clocks, notes32)) => {
                    time_sigs.push(json!({
                        "ticks": tick,
                        "time": round3(tm.seconds(tick)),
                        "numerator": num,
                        "denominator": 1u32 << denom_pow,
                        "clocksPerClick": clocks,
                        "thirtySecondNotesPerQuarter": notes32,
                    }));
                }
                TrackEventKind::Meta(MetaMessage::KeySignature(sharps, minor)) => {
                    key_sigs.push(json!({
                        "ticks": tick,
                        "time": round3(tm.seconds(tick)),
                        "key": sharps,
                        "scale": if minor { "minor" } else { "major" },
                        "name": key_signature_name(sharps, minor),
                    }));
                }
                _ => {}
            }
        }
    }
    let mut header = serde_json::Map::new();
    header.insert("format".into(), json!(format_num));
    header.insert("formatType".into(), json!(format_type));
    header.insert("trackCount".into(), json!(smf.tracks.len()));
    header.insert("division".into(), tm.division_json());
    if let Some(ppq) = tm.ppq {
        header.insert("ticksPerQuarter".into(), json!(ppq));
    }
    header.insert("durationTicks".into(), json!(max_tick));
    header.insert("durationSeconds".into(), json!(round3(tm.seconds(max_tick))));
    header.insert("tempos".into(), tm.tempos_json());
    header.insert("timeSignatures".into(), Json::Array(time_sigs));
    header.insert("keySignatures".into(), Json::Array(key_sigs));
    header
}

fn build_notes(format_num: u8, format_type: &str, tm: &TimeMap, smf: &Smf) -> Json {
    let header = build_header(format_num, format_type, tm, smf);
    let mut tracks: Vec<Json> = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0u64;
        let mut name: Option<String> = None;
        let mut channel: Option<u8> = None;
        // Open note-ons keyed by (channel, key) → (start_tick, velocity).
        let mut open: Vec<((u8, u8), (u64, u8))> = Vec::new();
        let mut notes: Vec<Json> = Vec::new();
        for ev in track {
            tick += ev.delta.as_int() as u64;
            match ev.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => {
                    if name.is_none() {
                        name = Some(text_of(bytes));
                    }
                }
                TrackEventKind::Midi { channel: ch, message } => {
                    let ch = ch.as_int();
                    if channel.is_none() {
                        channel = Some(ch);
                    }
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
                                notes.push(json!({
                                    "name": note_name(k),
                                    "midi": k,
                                    "channel": ch,
                                    "ticks": start,
                                    "time": round3(tm.seconds(start)),
                                    "durationTicks": tick.saturating_sub(start),
                                    "duration": round3(tm.seconds(tick) - tm.seconds(start)),
                                    "velocity": vel,
                                }));
                            }
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        tracks.push(json!({
            "name": name.unwrap_or_default(),
            "channel": channel,
            "noteCount": notes.len(),
            "notes": notes,
        }));
    }
    json!({ "header": header, "tracks": tracks })
}

fn build_events(format_num: u8, format_type: &str, tm: &TimeMap, smf: &Smf) -> Json {
    let mut tracks: Vec<Json> = Vec::new();
    for track in &smf.tracks {
        let mut tick = 0u64;
        let mut events: Vec<Json> = Vec::new();
        for ev in track {
            let delta = ev.delta.as_int() as u64;
            tick += delta;
            let mut obj = serde_json::Map::new();
            obj.insert("deltaTicks".into(), json!(delta));
            obj.insert("ticks".into(), json!(tick));
            obj.insert("time".into(), json!(round3(tm.seconds(tick))));
            fill_event(&mut obj, ev.kind);
            events.push(Json::Object(obj));
        }
        tracks.push(Json::Array(events));
    }
    json!({
        "format": format_num,
        "formatType": format_type,
        "trackCount": smf.tracks.len(),
        "division": tm.division_json(),
        "tracks": tracks,
    })
}

fn fill_event(obj: &mut serde_json::Map<String, Json>, kind: TrackEventKind) {
    match kind {
        TrackEventKind::Midi { channel, message } => {
            let ch = channel.as_int();
            obj.insert("channel".into(), json!(ch));
            match message {
                MidiMessage::NoteOff { key, vel } => {
                    obj.insert("type".into(), json!("noteOff"));
                    obj.insert("note".into(), json!(key.as_int()));
                    obj.insert("name".into(), json!(note_name(key.as_int())));
                    obj.insert("velocity".into(), json!(vel.as_int()));
                }
                MidiMessage::NoteOn { key, vel } => {
                    obj.insert(
                        "type".into(),
                        json!(if vel.as_int() > 0 { "noteOn" } else { "noteOff" }),
                    );
                    obj.insert("note".into(), json!(key.as_int()));
                    obj.insert("name".into(), json!(note_name(key.as_int())));
                    obj.insert("velocity".into(), json!(vel.as_int()));
                }
                MidiMessage::Aftertouch { key, vel } => {
                    obj.insert("type".into(), json!("noteAftertouch"));
                    obj.insert("note".into(), json!(key.as_int()));
                    obj.insert("name".into(), json!(note_name(key.as_int())));
                    obj.insert("pressure".into(), json!(vel.as_int()));
                }
                MidiMessage::Controller { controller, value } => {
                    obj.insert("type".into(), json!("controlChange"));
                    obj.insert("controller".into(), json!(controller.as_int()));
                    obj.insert("value".into(), json!(value.as_int()));
                }
                MidiMessage::ProgramChange { program } => {
                    obj.insert("type".into(), json!("programChange"));
                    obj.insert("program".into(), json!(program.as_int()));
                }
                MidiMessage::ChannelAftertouch { vel } => {
                    obj.insert("type".into(), json!("channelAftertouch"));
                    obj.insert("pressure".into(), json!(vel.as_int()));
                }
                MidiMessage::PitchBend { bend } => {
                    obj.insert("type".into(), json!("pitchBend"));
                    // Signed −8192..+8191 (0 = centre).
                    obj.insert("value".into(), json!(bend.as_int()));
                }
            }
        }
        TrackEventKind::SysEx(data) => {
            obj.insert("type".into(), json!("sysEx"));
            obj.insert("data".into(), json!(hex_of(data)));
        }
        TrackEventKind::Escape(data) => {
            obj.insert("type".into(), json!("escape"));
            obj.insert("data".into(), json!(hex_of(data)));
        }
        TrackEventKind::Meta(meta) => fill_meta(obj, meta),
    }
}

fn fill_meta(obj: &mut serde_json::Map<String, Json>, meta: MetaMessage) {
    match meta {
        MetaMessage::TrackNumber(n) => {
            obj.insert("type".into(), json!("trackNumber"));
            obj.insert("number".into(), json!(n));
        }
        MetaMessage::Text(b) => text_event(obj, "text", b),
        MetaMessage::Copyright(b) => text_event(obj, "copyright", b),
        MetaMessage::TrackName(b) => text_event(obj, "trackName", b),
        MetaMessage::InstrumentName(b) => text_event(obj, "instrumentName", b),
        MetaMessage::Lyric(b) => text_event(obj, "lyric", b),
        MetaMessage::Marker(b) => text_event(obj, "marker", b),
        MetaMessage::CuePoint(b) => text_event(obj, "cuePoint", b),
        MetaMessage::ProgramName(b) => text_event(obj, "programName", b),
        MetaMessage::DeviceName(b) => text_event(obj, "deviceName", b),
        MetaMessage::MidiChannel(ch) => {
            obj.insert("type".into(), json!("channelPrefix"));
            obj.insert("channel".into(), json!(ch.as_int()));
        }
        MetaMessage::MidiPort(p) => {
            obj.insert("type".into(), json!("midiPort"));
            obj.insert("port".into(), json!(p.as_int()));
        }
        MetaMessage::EndOfTrack => {
            obj.insert("type".into(), json!("endOfTrack"));
        }
        MetaMessage::Tempo(us) => {
            obj.insert("type".into(), json!("tempo"));
            obj.insert("microsecondsPerQuarter".into(), json!(us.as_int()));
            obj.insert("bpm".into(), json!(round3(60_000_000.0 / us.as_int() as f64)));
        }
        MetaMessage::SmpteOffset(o) => {
            obj.insert("type".into(), json!("smpteOffset"));
            obj.insert("hour".into(), json!(o.hour()));
            obj.insert("minute".into(), json!(o.minute()));
            obj.insert("second".into(), json!(o.second()));
            obj.insert("frame".into(), json!(o.frame()));
            obj.insert("subframe".into(), json!(o.subframe()));
        }
        MetaMessage::TimeSignature(num, denom_pow, clocks, notes32) => {
            obj.insert("type".into(), json!("timeSignature"));
            obj.insert("numerator".into(), json!(num));
            obj.insert("denominator".into(), json!(1u32 << denom_pow));
            obj.insert("clocksPerClick".into(), json!(clocks));
            obj.insert("thirtySecondNotesPerQuarter".into(), json!(notes32));
        }
        MetaMessage::KeySignature(sharps, minor) => {
            obj.insert("type".into(), json!("keySignature"));
            obj.insert("key".into(), json!(sharps));
            obj.insert("scale".into(), json!(if minor { "minor" } else { "major" }));
            obj.insert("name".into(), json!(key_signature_name(sharps, minor)));
        }
        MetaMessage::SequencerSpecific(b) => {
            obj.insert("type".into(), json!("sequencerSpecific"));
            obj.insert("data".into(), json!(hex_of(b)));
        }
        MetaMessage::Unknown(kind, b) => {
            obj.insert("type".into(), json!("unknownMeta"));
            obj.insert("metaType".into(), json!(kind));
            obj.insert("data".into(), json!(hex_of(b)));
        }
    }
}

fn text_event(obj: &mut serde_json::Map<String, Json>, ty: &str, bytes: &[u8]) {
    obj.insert("type".into(), json!(ty));
    obj.insert("text".into(), json!(text_of(bytes)));
}

fn build_summary(format_num: u8, format_type: &str, tm: &TimeMap, smf: &Smf) -> Json {
    let mut note_count = 0usize;
    let mut cc_count = 0usize;
    let mut track_names: Vec<String> = Vec::new();
    let mut channels: Vec<u8> = Vec::new();
    let mut max_tick = 0u64;
    let mut first_ts: Option<Json> = None;
    let mut first_ks: Option<String> = None;
    for track in &smf.tracks {
        let mut tick = 0u64;
        let mut this_name: Option<String> = None;
        for ev in track {
            tick += ev.delta.as_int() as u64;
            max_tick = max_tick.max(tick);
            match ev.kind {
                TrackEventKind::Midi { channel, message } => {
                    let ch = channel.as_int();
                    if !channels.contains(&ch) {
                        channels.push(ch);
                    }
                    match message {
                        MidiMessage::NoteOn { vel, .. } if vel.as_int() > 0 => note_count += 1,
                        MidiMessage::Controller { .. } => cc_count += 1,
                        _ => {}
                    }
                }
                TrackEventKind::Meta(MetaMessage::TrackName(b)) => {
                    if this_name.is_none() {
                        this_name = Some(text_of(b));
                    }
                }
                TrackEventKind::Meta(MetaMessage::TimeSignature(n, d, _, _)) => {
                    if first_ts.is_none() {
                        first_ts = Some(json!(format!("{}/{}", n, 1u32 << d)));
                    }
                }
                TrackEventKind::Meta(MetaMessage::KeySignature(s, m)) => {
                    if first_ks.is_none() {
                        first_ks = Some(key_signature_name(s, m));
                    }
                }
                _ => {}
            }
        }
        if let Some(n) = this_name {
            if !n.is_empty() {
                track_names.push(n);
            }
        }
    }
    channels.sort_unstable();
    let initial_us = tm.tempos.first().map(|&(_, us)| us).unwrap_or(500_000);
    json!({
        "format": format_num,
        "formatType": format_type,
        "trackCount": smf.tracks.len(),
        "division": tm.division_json(),
        "durationTicks": max_tick,
        "durationSeconds": round3(tm.seconds(max_tick)),
        "tempoBpm": round3(60_000_000.0 / initial_us as f64),
        "tempoChanges": tm.tempos.len(),
        "timeSignature": first_ts.unwrap_or(Json::Null),
        "keySignature": first_ks.map(Json::String).unwrap_or(Json::Null),
        "noteCount": note_count,
        "controlChangeCount": cc_count,
        "channels": channels,
        "trackNames": track_names,
    })
}

// ----------------------------------------------------------------------------
// Small helpers.
// ----------------------------------------------------------------------------

/// MIDI note number → scientific pitch name (middle C = 60 = `C4`).
fn note_name(midi: u8) -> String {
    const NAMES: [&str; 12] =
        ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"];
    let octave = midi as i32 / 12 - 1;
    format!("{}{}", NAMES[(midi % 12) as usize], octave)
}

/// Circle-of-fifths key signature → readable name (sharps>0, flats<0).
fn key_signature_name(sharps: i8, minor: bool) -> String {
    const MAJOR: [&str; 15] = [
        "Cb", "Gb", "Db", "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#",
    ];
    const MINOR: [&str; 15] = [
        "Ab", "Eb", "Bb", "F", "C", "G", "D", "A", "E", "B", "F#", "C#", "G#", "D#", "A#",
    ];
    let idx = (sharps as i32 + 7).clamp(0, 14) as usize;
    let root = if minor { MINOR[idx] } else { MAJOR[idx] };
    format!("{} {}", root, if minor { "minor" } else { "major" })
}

fn text_of(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn hex_of(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Round to 3 decimal places (keeps note times/BPM readable, avoids float noise).
fn round3(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

fn decode_bytes(input: &str, encoding: &str) -> Result<Vec<u8>, String> {
    let enc = if encoding.trim().is_empty() { "auto" } else { encoding.trim() };
    let cleaned: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("input is empty".into());
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
        other => Err(format!("unknown encoding '{other}' (use auto, base64, or hex)")),
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
    use midly::{Header, Track, TrackEvent};

    /// Build a tiny Format-1 SMF in memory: 480 PPQ, 120 BPM, one note C4 for one
    /// quarter, with a track name and 4/4 time signature.
    fn sample_midi() -> Vec<u8> {
        let meta_track: Track = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let note_track: Track = vec![
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Piano")),
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOn { key: u7::new(60), vel: u7::new(100) },
                },
            },
            TrackEvent {
                delta: u28::new(480),
                kind: TrackEventKind::Midi {
                    channel: u4::new(0),
                    message: MidiMessage::NoteOff { key: u7::new(60), vel: u7::new(0) },
                },
            },
            TrackEvent {
                delta: u28::new(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];

        let smf = Smf {
            header: Header::new(Format::Parallel, Timing::Metrical(u15::new(480))),
            tracks: vec![meta_track, note_track],
        };
        let mut buf = Vec::new();
        smf.write(&mut buf).unwrap();
        buf
    }

    #[test]
    fn notes_happy_path() {
        let bytes = sample_midi();
        let json = convert_bytes(&bytes, "notes").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["header"]["format"], 1);
        assert_eq!(v["header"]["ticksPerQuarter"], 480);
        assert_eq!(v["header"]["tempos"][0]["bpm"], 120.0);
        assert_eq!(v["header"]["timeSignatures"][0]["numerator"], 4);
        assert_eq!(v["header"]["timeSignatures"][0]["denominator"], 4);
        // Second track carries the note.
        let note = &v["tracks"][1]["notes"][0];
        assert_eq!(note["name"], "C4");
        assert_eq!(note["midi"], 60);
        assert_eq!(note["velocity"], 100);
        assert_eq!(note["durationTicks"], 480);
        // 480 ticks at 480 PPQ, 120 BPM = one quarter = 0.5 s.
        assert_eq!(note["duration"], 0.5);
        assert_eq!(v["tracks"][1]["name"], "Piano");
    }

    #[test]
    fn events_and_summary() {
        let bytes = sample_midi();
        let ev: serde_json::Value =
            serde_json::from_str(&convert_bytes(&bytes, "events").unwrap()).unwrap();
        assert_eq!(ev["format"], 1);
        // meta track first event = tempo.
        assert_eq!(ev["tracks"][0][0]["type"], "tempo");
        assert_eq!(ev["tracks"][0][0]["bpm"], 120.0);

        let sm: serde_json::Value =
            serde_json::from_str(&convert_bytes(&bytes, "summary").unwrap()).unwrap();
        assert_eq!(sm["noteCount"], 1);
        assert_eq!(sm["tempoBpm"], 120.0);
        assert_eq!(sm["timeSignature"], "4/4");
        assert_eq!(sm["trackNames"][0], "Piano");
    }

    #[test]
    fn hex_input_auto_detected() {
        let bytes = sample_midi();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let json = convert(&hex, "auto", "summary").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["noteCount"], 1);
    }

    #[test]
    fn rejects_non_midi() {
        let err = convert_bytes(b"not a midi file at all", "notes").unwrap_err();
        assert!(err.contains("MThd"), "got: {err}");
    }

    #[test]
    fn rejects_empty() {
        assert!(convert("", "auto", "notes").unwrap_err().contains("empty"));
    }

    #[test]
    fn rejects_unknown_format() {
        let bytes = sample_midi();
        let err = convert_bytes(&bytes, "xml").unwrap_err();
        assert!(err.contains("unknown format"), "got: {err}");
    }
}
