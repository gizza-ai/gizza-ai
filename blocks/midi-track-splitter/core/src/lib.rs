//! gizza-ai/midi-track-splitter core — split one multi-track Standard MIDI File
//! (SMF, `.mid`/`.midi`) into several single-part MIDI files.
//!
//! Two ways to cut the file up:
//!   * `split_by = "track"`   — one output file per track chunk (the usual case
//!     for a Format 1 file exported by a DAW or notation program);
//!   * `split_by = "channel"` — one output file per MIDI channel, gathering that
//!     channel's events from every track (the only sensible cut for a Format 0
//!     file, which has a single track carrying all 16 channels).
//!
//! The part that naive splitters get wrong is the CONDUCTOR data: tempo, time
//! signature and key signature normally live in track 0 only, so a part exported
//! on its own plays back at the MIDI default 120 BPM in 4/4. With
//! `include_conductor` (on by default) every output file gets a copy of that
//! data — collected from a true conductor track when the file has one, plus any
//! tempo/time/key-signature events found anywhere else — de-duplicated so a part
//! that already carried a tempo event never ends up with two.
//!
//! Output files are written as Format 0 (one merged track — a genuinely
//! single-track file) or Format 1 (conductor track + part track); the source
//! division (ticks per quarter note, or SMPTE timecode) is always preserved, so
//! nothing is re-gridded and every note keeps its exact position, length,
//! velocity, controller and program change.
//!
//! Pure Rust (`midly` + `serde_json`); no wafer/wasm-bindgen deps, so the same
//! code runs in chat, the CLI and the browser page.

use midly::num::u28;
use midly::{Format, Header, MetaMessage, MidiMessage, Smf, Timing, TrackEvent, TrackEventKind};

/// Largest decoded MIDI file accepted (4 MiB). A dense orchestral score is a
/// few hundred KB; the cap keeps a pathological upload out of browser memory.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
/// Most output files a single split may produce.
pub const MAX_FILES: usize = 64;
/// Longest sanitised part name used inside a filename.
const MAX_NAME_CHARS: usize = 32;
/// MIDI delta times are variable-length 28-bit integers.
const MAX_DELTA: u64 = 0x0FFF_FFFF;
/// The MIDI default when a file carries no tempo event at all: 120 BPM.
const DEFAULT_TEMPO_US: u32 = 500_000;
/// Standard MIDI File MIME type (used for the per-file `data:` URLs).
pub const MIDI_MIME: &str = "audio/midi";

// ----------------------------------------------------------------------------
// Options
// ----------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitBy {
    /// One output file per track chunk.
    Track,
    /// One output file per MIDI channel, gathered across every track.
    Channel,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutFormat {
    /// Format 0 — everything merged into ONE track.
    Format0,
    /// Format 1 — conductor track first, then the part track.
    Format1,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputMode {
    /// Every part's bytes are returned as a `data:` URL.
    Files,
    /// Only the table of parts — no file bytes (a preview pass).
    List,
}

#[derive(Clone, Debug)]
pub struct Options {
    pub split_by: SplitBy,
    /// Copy tempo / time signature / key signature into every output file.
    pub include_conductor: bool,
    pub output_format: OutFormat,
    /// Drop parts that contain no notes (an empty or conductor-only track).
    pub skip_empty: bool,
    /// Which parts to keep, as 1-based numbers and ranges (`"1,3-5"`).
    /// Empty keeps every part. Numbers are track numbers when splitting by
    /// track and MIDI channel numbers (1-16) when splitting by channel.
    pub select: String,
    /// Leading word of every generated filename.
    pub filename_prefix: String,
    pub output: OutputMode,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            split_by: SplitBy::Track,
            include_conductor: true,
            output_format: OutFormat::Format0,
            skip_empty: true,
            select: String::new(),
            filename_prefix: "part".to_string(),
            output: OutputMode::Files,
        }
    }
}

impl Options {
    /// Build options from the surface-level (string/bool) parameters the chat
    /// schema, the CLI and the page all pass, so every surface validates
    /// identically and produces the same error text.
    pub fn parse(
        split_by: &str,
        include_conductor: bool,
        output_format: &str,
        skip_empty: bool,
        select: &str,
        filename_prefix: &str,
        output: &str,
    ) -> Result<Self, String> {
        let split_by = match trimmed_or(split_by, "track") {
            "track" => SplitBy::Track,
            "channel" => SplitBy::Channel,
            other => return Err(format!("unknown split_by '{other}' (use track or channel)")),
        };
        let output_format = match trimmed_or(output_format, "format-0") {
            "format-0" => OutFormat::Format0,
            "format-1" => OutFormat::Format1,
            other => {
                return Err(format!(
                    "unknown output_format '{other}' (use format-0 or format-1)"
                ))
            }
        };
        let output = match trimmed_or(output, "files") {
            "files" => OutputMode::Files,
            "list" => OutputMode::List,
            other => return Err(format!("unknown output '{other}' (use files or list)")),
        };
        // Parsed here so a bad selection is reported before any MIDI work.
        parse_select(select)?;
        let prefix = sanitize_name(filename_prefix);
        Ok(Self {
            split_by,
            include_conductor,
            output_format,
            skip_empty,
            select: select.trim().to_string(),
            filename_prefix: if prefix.is_empty() {
                "part".to_string()
            } else {
                prefix
            },
            output,
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

/// Parse a checkbox value positive-truthy; an empty value keeps `default`.
pub fn truthy(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

/// `"1,3-5"` → `[1, 3, 4, 5]`. An empty string means "everything".
fn parse_select(raw: &str) -> Result<Option<Vec<usize>>, String> {
    let t = raw.trim();
    if t.is_empty() {
        return Ok(None);
    }
    let mut out = Vec::new();
    for token in t.split(',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        let (lo, hi) = match token.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (token, token),
        };
        let lo: usize = lo.parse().map_err(|_| select_err(token))?;
        let hi: usize = hi.parse().map_err(|_| select_err(token))?;
        if lo == 0 || hi == 0 {
            return Err("select numbers start at 1, not 0".to_string());
        }
        if hi < lo {
            return Err(format!(
                "select range '{token}' runs backwards — write it as {hi}-{lo}"
            ));
        }
        for n in lo..=hi {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    if out.is_empty() {
        return Ok(None);
    }
    Ok(Some(out))
}

fn select_err(token: &str) -> String {
    format!(
        "select must be a comma-separated list of numbers or ranges like '1,3-5' — could not read '{token}'"
    )
}

// ----------------------------------------------------------------------------
// Output
// ----------------------------------------------------------------------------

/// One produced single-part MIDI file.
#[derive(Clone, Debug)]
pub struct SplitFile {
    /// Suggested download name, e.g. `part-02-bass.mid`.
    pub filename: String,
    /// Human name of the part (track name, instrument, or channel).
    pub name: String,
    /// Where it came from: `"track 2"` or `"channel 10"`.
    pub source: String,
    /// 1-based source number — the track number, or the MIDI channel (1-16).
    pub index: usize,
    /// MIDI channels used by the part, 1-16.
    pub channels: Vec<u8>,
    /// General MIDI instrument implied by the part's first program change.
    pub instrument: Option<String>,
    /// Note-on events with velocity > 0.
    pub notes: usize,
    /// Events written into the file (excluding the end-of-track marker).
    pub events: usize,
    /// Playing time of the part in seconds, using the file's tempo map.
    pub seconds: f64,
    /// The complete Standard MIDI File bytes.
    pub midi: Vec<u8>,
}

/// The split result plus the numbers every surface reports.
#[derive(Clone, Debug)]
pub struct Output {
    pub files: Vec<SplitFile>,
    /// `"format-0"`, `"format-1"` or `"format-2"`.
    pub source_format: String,
    /// Human description of the source division.
    pub division: String,
    /// Ticks per quarter note, absent for SMPTE-timecode files.
    pub ppq: Option<u16>,
    /// First tempo of the source file in BPM (120 when it carried none).
    pub tempo_bpm: f64,
    pub tracks_in: usize,
    pub notes_in: usize,
    /// `"track"` or `"channel"`.
    pub split_by: String,
    /// A Format 0 source forced a channel split even though `track` was asked for.
    pub auto_channel: bool,
    /// Conductor events copied into each part (0 when `include_conductor` is off).
    pub conductor_events: usize,
    /// Parts dropped because they had no notes.
    pub skipped_empty: usize,
    /// Parts dropped because `select` did not list them.
    pub skipped_unselected: usize,
    /// True when the file bytes were left out (`output = "list"`).
    pub list_only: bool,
}

impl Output {
    /// One-line human/LLM summary — also the first line the CLI prints.
    pub fn summary(&self) -> String {
        let axis = if self.split_by == "channel" {
            "channel"
        } else {
            "track"
        };
        let mut s = format!(
            "Split a {} file ({} track(s), {}, {} note(s)) into {} single-part file(s), one per {axis}.",
            self.source_format,
            self.tracks_in,
            self.division,
            self.notes_in,
            self.files.len()
        );
        if self.auto_channel {
            s.push_str(" The source is a Format 0 file with a single track, so it was split by channel.");
        }
        if self.conductor_events > 0 {
            s.push_str(&format!(
                " {} conductor event(s) (tempo, time and key signature) were copied into every part.",
                self.conductor_events
            ));
        } else {
            s.push_str(" No conductor data was copied, so each part plays at the MIDI default 120 BPM unless it carries its own tempo.");
        }
        if self.skipped_empty > 0 {
            s.push_str(&format!(
                " {} part(s) with no notes were skipped.",
                self.skipped_empty
            ));
        }
        if self.skipped_unselected > 0 {
            s.push_str(&format!(
                " {} part(s) were left out by the selection.",
                self.skipped_unselected
            ));
        }
        if self.list_only {
            s.push_str(" Listing only — no file bytes were produced.");
        }
        s
    }

    /// The JSON document every surface renders (chat, CLI and the page).
    pub fn to_json(&self) -> String {
        let files: Vec<serde_json::Value> = self
            .files
            .iter()
            .map(|f| {
                let mut v = serde_json::json!({
                    "filename": f.filename,
                    "name": f.name,
                    "source": f.source,
                    "index": f.index,
                    "channels": f.channels,
                    "instrument": f.instrument,
                    "notes": f.notes,
                    "events": f.events,
                    "seconds": round3(f.seconds),
                });
                if !self.list_only {
                    v["bytes"] = serde_json::json!(f.midi.len());
                    v["data_url"] =
                        serde_json::json!(format!("data:{MIDI_MIME};base64,{}", b64_encode(&f.midi)));
                }
                v
            })
            .collect();
        let doc = serde_json::json!({
            "summary": self.summary(),
            "split_by": self.split_by,
            "files_produced": self.files.len(),
            "source": {
                "format": self.source_format,
                "division": self.division,
                "ticks_per_quarter_note": self.ppq,
                "tempo_bpm": round3(self.tempo_bpm),
                "tracks": self.tracks_in,
                "notes": self.notes_in,
            },
            "conductor_events_copied": self.conductor_events,
            "skipped_empty": self.skipped_empty,
            "skipped_unselected": self.skipped_unselected,
            "files": files,
        });
        serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

// ----------------------------------------------------------------------------
// Public entry points
// ----------------------------------------------------------------------------

/// Split MIDI `input` given as base64 or hex text (per `encoding`).
///
/// `encoding`: `"auto"` (default) reads hex when the input is all hex digits
/// with an even length, otherwise base64; `"base64"` / `"hex"` force one.
pub fn split(input: &str, encoding: &str, opts: &Options) -> Result<Output, String> {
    let bytes = decode_bytes(input, encoding)?;
    split_bytes(&bytes, opts)
}

/// Split, and return the JSON document every surface renders.
pub fn split_to_json(input: &str, encoding: &str, opts: &Options) -> Result<String, String> {
    Ok(split(input, encoding, opts)?.to_json())
}

/// Split already-decoded MIDI `bytes`.
pub fn split_bytes(bytes: &[u8], opts: &Options) -> Result<Output, String> {
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
    if smf.tracks.is_empty() {
        return Err("this MIDI file has no tracks, so there is nothing to split".into());
    }
    let ppq = match smf.header.timing {
        Timing::Metrical(t) => {
            let v = t.as_int();
            if v == 0 {
                return Err(
                    "this file declares 0 ticks per quarter note, so it has no usable time base"
                        .into(),
                );
            }
            Some(v)
        }
        Timing::Timecode(..) => None,
    };
    let division = match smf.header.timing {
        Timing::Metrical(t) => format!("{} ticks per quarter note", t.as_int()),
        Timing::Timecode(fps, sub) => format!("SMPTE timecode, {} fps x {sub} subframes", fps.as_f32()),
    };
    let source_format = match smf.header.format {
        Format::SingleTrack => "format-0",
        Format::Parallel => "format-1",
        Format::Sequential => "format-2",
    }
    .to_string();

    let abs = to_abs(&smf.tracks);
    let tempos = collect_tempos(&abs);
    let notes_in: usize = abs.iter().map(|t| count_notes(t)).sum();
    let tempo_bpm = 60_000_000.0
        / tempos
            .first()
            .map(|&(_, us)| us)
            .unwrap_or(DEFAULT_TEMPO_US)
            .max(1) as f64;

    // A Format 0 file has exactly one track, so a track split would just hand
    // back the input. Cut it by channel instead and say so in the summary.
    let mut auto_channel = false;
    let mut split_by = opts.split_by;
    if split_by == SplitBy::Track && (smf.header.format == Format::SingleTrack || abs.len() == 1) {
        split_by = SplitBy::Channel;
        auto_channel = true;
    }

    // Format 2 tracks are independent sequences, so there is no shared conductor
    // data to copy — each track already carries its own.
    let sequential = smf.header.format == Format::Sequential;
    let conductor = if opts.include_conductor && !sequential {
        conductor_events(&abs)
    } else {
        Vec::new()
    };

    let selection = parse_select(&opts.select)?;
    let parts = match split_by {
        SplitBy::Track => track_parts(&abs, &smf.tracks),
        SplitBy::Channel => channel_parts(&abs, &smf.tracks),
    };

    let mut skipped_empty = 0usize;
    let mut skipped_unselected = 0usize;
    let mut kept: Vec<Part> = Vec::new();
    for part in parts {
        if opts.skip_empty && part.notes == 0 {
            skipped_empty += 1;
            continue;
        }
        if let Some(sel) = &selection {
            if !sel.contains(&part.index) {
                skipped_unselected += 1;
                continue;
            }
        }
        kept.push(part);
    }
    if kept.is_empty() {
        return Err(describe_empty_result(
            split_by,
            skipped_empty,
            skipped_unselected,
            opts,
        ));
    }
    if kept.len() > MAX_FILES {
        return Err(format!(
            "this file would produce {} parts, over the limit of {MAX_FILES} — use `select` (e.g. 1-{MAX_FILES}) to split it in batches",
            kept.len()
        ));
    }

    let list_only = opts.output == OutputMode::List;
    let mut used_names: Vec<String> = Vec::new();
    let mut files = Vec::with_capacity(kept.len());
    for part in &kept {
        let filename = unique_filename(&opts.filename_prefix, part.index, &part.name, &mut used_names);
        let midi = if list_only {
            Vec::new()
        } else {
            build_file(part, &conductor, smf.header.timing, opts.output_format)?
        };
        files.push(SplitFile {
            filename,
            name: part.name.clone(),
            source: part.source.clone(),
            index: part.index,
            channels: part.channels.iter().map(|c| c + 1).collect(),
            instrument: part.instrument.clone(),
            notes: part.notes,
            events: part.events.len(),
            seconds: seconds(&tempos, ppq, smf.header.timing, part.end_tick),
            midi,
        });
    }

    Ok(Output {
        files,
        source_format,
        division,
        ppq,
        tempo_bpm,
        tracks_in: smf.tracks.len(),
        notes_in,
        split_by: match split_by {
            SplitBy::Track => "track".to_string(),
            SplitBy::Channel => "channel".to_string(),
        },
        auto_channel,
        conductor_events: conductor.len(),
        skipped_empty,
        skipped_unselected,
        list_only,
    })
}

fn describe_empty_result(
    split_by: SplitBy,
    skipped_empty: usize,
    skipped_unselected: usize,
    opts: &Options,
) -> String {
    if skipped_unselected > 0 {
        return format!(
            "the selection '{}' matched none of the {} part(s) in this file — the numbers are {} numbers, starting at 1",
            opts.select,
            skipped_unselected,
            if split_by == SplitBy::Channel {
                "MIDI channel"
            } else {
                "track"
            }
        );
    }
    if skipped_empty > 0 {
        return format!(
            "every one of the {skipped_empty} part(s) in this file is empty (no note events) — untick “skip parts with no notes” to export them anyway"
        );
    }
    "this MIDI file contains no events to split".to_string()
}

// ----------------------------------------------------------------------------
// Parts
// ----------------------------------------------------------------------------

/// One prospective output file, before it is written.
struct Part<'a> {
    /// 1-based track number or MIDI channel number.
    index: usize,
    name: String,
    source: String,
    /// Zero-based channels used.
    channels: Vec<u8>,
    instrument: Option<String>,
    notes: usize,
    end_tick: u64,
    events: Vec<(u64, TrackEventKind<'a>)>,
}

/// One part per track chunk, in file order.
fn track_parts<'a>(
    abs: &[Vec<(u64, TrackEventKind<'a>)>],
    raw: &[Vec<TrackEvent<'a>>],
) -> Vec<Part<'a>> {
    abs.iter()
        .enumerate()
        .map(|(i, events)| {
            let kept: Vec<(u64, TrackEventKind<'a>)> = events
                .iter()
                .filter(|(_, k)| !matches!(k, TrackEventKind::Meta(MetaMessage::EndOfTrack)))
                .cloned()
                .collect();
            let channels = channels_used(&kept);
            let instrument = instrument_of(&kept, &channels);
            let name = track_name(&raw[i])
                .or_else(|| instrument.clone())
                .unwrap_or_else(|| format!("track {}", i + 1));
            Part {
                index: i + 1,
                name,
                source: format!("track {}", i + 1),
                channels,
                instrument,
                notes: count_notes(&kept),
                end_tick: kept.iter().map(|(t, _)| *t).max().unwrap_or(0),
                events: kept,
            }
        })
        .collect()
}

/// One part per MIDI channel, gathering that channel's events from every track.
fn channel_parts<'a>(
    abs: &[Vec<(u64, TrackEventKind<'a>)>],
    raw: &[Vec<TrackEvent<'a>>],
) -> Vec<Part<'a>> {
    let mut out = Vec::new();
    for ch in 0u8..16 {
        let mut events: Vec<(u64, TrackEventKind<'a>)> = Vec::new();
        // Which source tracks fed this channel — used to name the part after the
        // source track when the channel lives in exactly one of them.
        let mut source_tracks: Vec<usize> = Vec::new();
        for (ti, track) in abs.iter().enumerate() {
            for (tick, kind) in track {
                if let TrackEventKind::Midi { channel, .. } = kind {
                    if channel.as_int() == ch {
                        events.push((*tick, *kind));
                        if !source_tracks.contains(&ti) {
                            source_tracks.push(ti);
                        }
                    }
                }
            }
        }
        if events.is_empty() {
            continue;
        }
        events.sort_by_key(|(t, _)| *t);
        let channels = vec![ch];
        let instrument = instrument_of(&events, &channels);
        // The source track's name only describes this part when that track is
        // devoted to this one channel — a Format 0 track carrying all 16
        // channels would otherwise name every part after the whole song.
        let name = source_tracks
            .first()
            .filter(|ti| source_tracks.len() == 1 && channels_used(&abs[**ti]) == [ch])
            .and_then(|ti| track_name(&raw[*ti]))
            .or_else(|| instrument.clone())
            .unwrap_or_else(|| format!("channel {}", ch + 1));
        out.push(Part {
            index: ch as usize + 1,
            name,
            source: format!("channel {}", ch + 1),
            channels,
            instrument,
            notes: count_notes(&events),
            end_tick: events.iter().map(|(t, _)| *t).max().unwrap_or(0),
            events,
        });
    }
    out
}

/// Conductor data: the whole of track 0 when it is a true conductor track (no
/// channel-voice events), plus every tempo / time-signature / key-signature /
/// SMPTE-offset event found anywhere in the file, de-duplicated.
fn conductor_events<'a>(abs: &[Vec<(u64, TrackEventKind<'a>)>]) -> Vec<(u64, TrackEventKind<'a>)> {
    let mut out: Vec<(u64, TrackEventKind<'a>)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    let push = |out: &mut Vec<(u64, TrackEventKind<'a>)>,
                    seen: &mut Vec<String>,
                    tick: u64,
                    kind: TrackEventKind<'a>| {
        let key = format!("{tick}:{kind:?}");
        if !seen.contains(&key) {
            seen.push(key);
            out.push((tick, kind));
        }
    };

    let first_is_conductor = abs
        .first()
        .map(|t| {
            !t.iter()
                .any(|(_, k)| matches!(k, TrackEventKind::Midi { .. }))
        })
        .unwrap_or(false);
    if first_is_conductor {
        for (tick, kind) in &abs[0] {
            if matches!(kind, TrackEventKind::Meta(MetaMessage::EndOfTrack)) {
                continue;
            }
            push(&mut out, &mut seen, *tick, *kind);
        }
    }
    for (ti, track) in abs.iter().enumerate() {
        if ti == 0 && first_is_conductor {
            continue;
        }
        for (tick, kind) in track {
            if is_global_meta(kind) {
                push(&mut out, &mut seen, *tick, *kind);
            }
        }
    }
    out.sort_by_key(|(t, _)| *t);
    out
}

fn is_global_meta(kind: &TrackEventKind) -> bool {
    matches!(
        kind,
        TrackEventKind::Meta(
            MetaMessage::Tempo(_)
                | MetaMessage::TimeSignature(..)
                | MetaMessage::KeySignature(..)
                | MetaMessage::SmpteOffset(_)
        )
    )
}

// ----------------------------------------------------------------------------
// Writing one output file
// ----------------------------------------------------------------------------

fn build_file(
    part: &Part<'_>,
    conductor: &[(u64, TrackEventKind<'_>)],
    timing: Timing,
    out_format: OutFormat,
) -> Result<Vec<u8>, String> {
    // The part's own events already win, so a track that carried its own tempo
    // never gets a duplicate copied in beside it.
    let own: Vec<String> = part
        .events
        .iter()
        .map(|(t, k)| format!("{t}:{k:?}"))
        .collect();
    let extra: Vec<(u64, TrackEventKind<'_>)> = conductor
        .iter()
        .filter(|(t, k)| !own.contains(&format!("{t}:{k:?}")))
        .cloned()
        .collect();

    // The part's display name, written as a track name so a DAW shows it on
    // import. Kept alive for as long as the borrowed events below.
    let name_bytes = part.name.as_bytes().to_vec();
    let named = TrackEventKind::Meta(MetaMessage::TrackName(&name_bytes));
    let has_name = part
        .events
        .iter()
        .any(|(_, k)| matches!(k, TrackEventKind::Meta(MetaMessage::TrackName(_))));

    let tracks: Vec<Vec<TrackEvent<'_>>> = match out_format {
        OutFormat::Format0 => {
            // One merged track: conductor data first at equal ticks, then the
            // part's events in their original order.
            let mut merged: Vec<(u64, u8, TrackEventKind<'_>)> = Vec::new();
            for (t, k) in &extra {
                // A conductor track's own name would shadow the part's.
                if matches!(k, TrackEventKind::Meta(MetaMessage::TrackName(_))) {
                    continue;
                }
                merged.push((*t, 0, *k));
            }
            for (t, k) in &part.events {
                merged.push((*t, 1, *k));
            }
            merged.sort_by_key(|(t, order, _)| (*t, *order));
            let mut events: Vec<(u64, TrackEventKind<'_>)> =
                merged.into_iter().map(|(t, _, k)| (t, k)).collect();
            if !has_name {
                events.insert(0, (0, named));
            }
            vec![to_track(events)?]
        }
        OutFormat::Format1 => {
            let mut part_events = part.events.clone();
            if !has_name {
                part_events.insert(0, (0, named));
            }
            vec![to_track(extra)?, to_track(part_events)?]
        }
    };

    let format = match out_format {
        OutFormat::Format0 => Format::SingleTrack,
        OutFormat::Format1 => Format::Parallel,
    };
    let smf = Smf {
        header: Header::new(format, timing),
        tracks,
    };
    let mut bytes = Vec::new();
    smf.write_std(&mut bytes)
        .map_err(|e| format!("failed to write the MIDI file: {e}"))?;
    Ok(bytes)
}

/// Absolute-tick events → delta-time `TrackEvent`s with exactly one
/// end-of-track marker.
fn to_track(events: Vec<(u64, TrackEventKind<'_>)>) -> Result<Vec<TrackEvent<'_>>, String> {
    let mut sorted = events;
    sorted.sort_by_key(|(t, _)| *t);
    let end = sorted.last().map(|(t, _)| *t).unwrap_or(0);
    let mut out = Vec::with_capacity(sorted.len() + 1);
    let mut prev = 0u64;
    for (tick, kind) in sorted {
        if matches!(kind, TrackEventKind::Meta(MetaMessage::EndOfTrack)) {
            continue;
        }
        let delta = tick - prev;
        if delta > MAX_DELTA {
            return Err(format!(
                "a gap of {delta} ticks is larger than the MIDI delta-time limit of {MAX_DELTA}"
            ));
        }
        prev = tick;
        out.push(TrackEvent {
            delta: u28::from(delta as u32),
            kind,
        });
    }
    out.push(TrackEvent {
        delta: u28::from((end - prev) as u32),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    Ok(out)
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

/// `(tick, microseconds per quarter note)` for every tempo event, chronological.
fn collect_tempos(abs: &[Vec<(u64, TrackEventKind)>]) -> Vec<(u64, u32)> {
    let mut out: Vec<(u64, u32)> = Vec::new();
    for track in abs {
        for (tick, kind) in track {
            if let TrackEventKind::Meta(MetaMessage::Tempo(us)) = kind {
                out.push((*tick, us.as_int()));
            }
        }
    }
    out.sort_by_key(|(t, _)| *t);
    out
}

fn count_notes(events: &[(u64, TrackEventKind)]) -> usize {
    events
        .iter()
        .filter(|(_, k)| {
            matches!(
                k,
                TrackEventKind::Midi {
                    message: MidiMessage::NoteOn { vel, .. },
                    ..
                } if vel.as_int() > 0
            )
        })
        .count()
}

fn channels_used(events: &[(u64, TrackEventKind)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    for (_, kind) in events {
        if let TrackEventKind::Midi { channel, .. } = kind {
            let c = channel.as_int();
            if !out.contains(&c) {
                out.push(c);
            }
        }
    }
    out.sort_unstable();
    out
}

/// The General MIDI instrument implied by the part's FIRST program change —
/// or the drum kit, since channel 10 is percussion by convention.
fn instrument_of(events: &[(u64, TrackEventKind)], channels: &[u8]) -> Option<String> {
    let program = events.iter().find_map(|(_, k)| match k {
        TrackEventKind::Midi {
            message: MidiMessage::ProgramChange { program },
            ..
        } => Some(program.as_int()),
        _ => None,
    });
    if channels == [9] {
        // Channel 10 (index 9) is the percussion channel in General MIDI.
        return Some("Drum kit".to_string());
    }
    program.map(|p| GM_PROGRAMS[p as usize % 128].to_string())
}

fn track_name(track: &[TrackEvent]) -> Option<String> {
    track.iter().find_map(|ev| match ev.kind {
        TrackEventKind::Meta(MetaMessage::TrackName(bytes)) => {
            let s = String::from_utf8_lossy(bytes).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        }
        _ => None,
    })
}

/// Seconds at `tick`, walking the tempo map (or the frame rate for SMPTE).
fn seconds(tempos: &[(u64, u32)], ppq: Option<u16>, timing: Timing, tick: u64) -> f64 {
    match (ppq, timing) {
        (Some(ppq), _) => {
            let ppq = ppq as f64;
            let mut secs = 0.0;
            let mut cursor = 0u64;
            let mut us = DEFAULT_TEMPO_US as f64;
            for &(t, next_us) in tempos {
                if t >= tick {
                    break;
                }
                secs += (t - cursor) as f64 / ppq * (us / 1_000_000.0);
                cursor = t;
                us = next_us.max(1) as f64;
            }
            secs + (tick.saturating_sub(cursor)) as f64 / ppq * (us / 1_000_000.0)
        }
        (None, Timing::Timecode(fps, sub)) => {
            let per_second = fps.as_f32() as f64 * sub as f64;
            if per_second <= 0.0 {
                0.0
            } else {
                tick as f64 / per_second
            }
        }
        _ => 0.0,
    }
}

// ----------------------------------------------------------------------------
// Naming
// ----------------------------------------------------------------------------

fn sanitize_name(raw: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for c in raw.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
        if out.chars().count() >= MAX_NAME_CHARS {
            break;
        }
    }
    out.trim_matches('-').to_string()
}

fn unique_filename(prefix: &str, index: usize, name: &str, used: &mut Vec<String>) -> String {
    let slug = sanitize_name(name);
    let base = if slug.is_empty() {
        format!("{prefix}-{index:02}")
    } else {
        format!("{prefix}-{index:02}-{slug}")
    };
    let mut candidate = format!("{base}.mid");
    let mut n = 2;
    while used.contains(&candidate) {
        candidate = format!("{base}-{n}.mid");
        n += 1;
    }
    used.push(candidate.clone());
    candidate
}

// ----------------------------------------------------------------------------
// Input decoding (base64 / hex, self-contained so the page needs no JS glue)
// ----------------------------------------------------------------------------

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
    let raw: Vec<u8> = s.bytes().filter(|b| *b != b'=').collect();
    let mut out = Vec::with_capacity(raw.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0u32;
    for b in raw {
        let v = val(b).ok_or_else(|| {
            format!(
                "invalid base64 character '{}' — paste the .mid file as base64 or hex",
                b as char
            )
        })? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

/// Standard base64 (no line breaks) for the per-file `data:` URLs.
pub fn b64_encode(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

/// The 128 General MIDI Level 1 program names, in program-number order.
const GM_PROGRAMS: [&str; 128] = [
    "Acoustic Grand Piano",
    "Bright Acoustic Piano",
    "Electric Grand Piano",
    "Honky-tonk Piano",
    "Electric Piano 1",
    "Electric Piano 2",
    "Harpsichord",
    "Clavi",
    "Celesta",
    "Glockenspiel",
    "Music Box",
    "Vibraphone",
    "Marimba",
    "Xylophone",
    "Tubular Bells",
    "Dulcimer",
    "Drawbar Organ",
    "Percussive Organ",
    "Rock Organ",
    "Church Organ",
    "Reed Organ",
    "Accordion",
    "Harmonica",
    "Tango Accordion",
    "Acoustic Guitar (nylon)",
    "Acoustic Guitar (steel)",
    "Electric Guitar (jazz)",
    "Electric Guitar (clean)",
    "Electric Guitar (muted)",
    "Overdriven Guitar",
    "Distortion Guitar",
    "Guitar Harmonics",
    "Acoustic Bass",
    "Electric Bass (finger)",
    "Electric Bass (pick)",
    "Fretless Bass",
    "Slap Bass 1",
    "Slap Bass 2",
    "Synth Bass 1",
    "Synth Bass 2",
    "Violin",
    "Viola",
    "Cello",
    "Contrabass",
    "Tremolo Strings",
    "Pizzicato Strings",
    "Orchestral Harp",
    "Timpani",
    "String Ensemble 1",
    "String Ensemble 2",
    "Synth Strings 1",
    "Synth Strings 2",
    "Choir Aahs",
    "Voice Oohs",
    "Synth Voice",
    "Orchestra Hit",
    "Trumpet",
    "Trombone",
    "Tuba",
    "Muted Trumpet",
    "French Horn",
    "Brass Section",
    "Synth Brass 1",
    "Synth Brass 2",
    "Soprano Sax",
    "Alto Sax",
    "Tenor Sax",
    "Baritone Sax",
    "Oboe",
    "English Horn",
    "Bassoon",
    "Clarinet",
    "Piccolo",
    "Flute",
    "Recorder",
    "Pan Flute",
    "Blown Bottle",
    "Shakuhachi",
    "Whistle",
    "Ocarina",
    "Lead 1 (square)",
    "Lead 2 (sawtooth)",
    "Lead 3 (calliope)",
    "Lead 4 (chiff)",
    "Lead 5 (charang)",
    "Lead 6 (voice)",
    "Lead 7 (fifths)",
    "Lead 8 (bass + lead)",
    "Pad 1 (new age)",
    "Pad 2 (warm)",
    "Pad 3 (polysynth)",
    "Pad 4 (choir)",
    "Pad 5 (bowed)",
    "Pad 6 (metallic)",
    "Pad 7 (halo)",
    "Pad 8 (sweep)",
    "FX 1 (rain)",
    "FX 2 (soundtrack)",
    "FX 3 (crystal)",
    "FX 4 (atmosphere)",
    "FX 5 (brightness)",
    "FX 6 (goblins)",
    "FX 7 (echoes)",
    "FX 8 (sci-fi)",
    "Sitar",
    "Banjo",
    "Shamisen",
    "Koto",
    "Kalimba",
    "Bag pipe",
    "Fiddle",
    "Shanai",
    "Tinkle Bell",
    "Agogo",
    "Steel Drums",
    "Woodblock",
    "Taiko Drum",
    "Melodic Tom",
    "Synth Drum",
    "Reverse Cymbal",
    "Guitar Fret Noise",
    "Breath Noise",
    "Seashore",
    "Bird Tweet",
    "Telephone Ring",
    "Helicopter",
    "Applause",
    "Gunshot",
];

// ----------------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// A Format 1 file: conductor track (tempo 120 + 4/4), a named piano part on
    /// channel 1 with two notes, and a named bass part on channel 2 with one.
    fn sample() -> Vec<u8> {
        use midly::num::{u15, u24, u4, u7};
        let conductor = vec![
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Conductor")),
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::from(500_000))),
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::TimeSignature(4, 2, 24, 8)),
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let piano = vec![
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Piano")),
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: u4::from(0),
                    message: MidiMessage::ProgramChange {
                        program: u7::from(0),
                    },
                },
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: u4::from(0),
                    message: MidiMessage::NoteOn {
                        key: u7::from(60),
                        vel: u7::from(64),
                    },
                },
            },
            TrackEvent {
                delta: u28::from(96),
                kind: TrackEventKind::Midi {
                    channel: u4::from(0),
                    message: MidiMessage::NoteOn {
                        key: u7::from(60),
                        vel: u7::from(0),
                    },
                },
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: u4::from(0),
                    message: MidiMessage::NoteOn {
                        key: u7::from(64),
                        vel: u7::from(70),
                    },
                },
            },
            TrackEvent {
                delta: u28::from(96),
                kind: TrackEventKind::Midi {
                    channel: u4::from(0),
                    message: MidiMessage::NoteOn {
                        key: u7::from(64),
                        vel: u7::from(0),
                    },
                },
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let bass = vec![
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::TrackName(b"Bass")),
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: u4::from(1),
                    message: MidiMessage::ProgramChange {
                        program: u7::from(33),
                    },
                },
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Midi {
                    channel: u4::from(1),
                    message: MidiMessage::NoteOn {
                        key: u7::from(36),
                        vel: u7::from(100),
                    },
                },
            },
            TrackEvent {
                delta: u28::from(192),
                kind: TrackEventKind::Midi {
                    channel: u4::from(1),
                    message: MidiMessage::NoteOn {
                        key: u7::from(36),
                        vel: u7::from(0),
                    },
                },
            },
            TrackEvent {
                delta: u28::from(0),
                kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
            },
        ];
        let smf = Smf {
            header: Header::new(Format::Parallel, Timing::Metrical(u15::from(96))),
            tracks: vec![conductor, piano, bass],
        };
        let mut out = Vec::new();
        smf.write_std(&mut out).unwrap();
        out
    }

    #[test]
    fn splits_a_format_1_file_into_one_named_file_per_track() {
        let out = split_bytes(&sample(), &Options::default()).unwrap();
        assert_eq!(out.source_format, "format-1");
        assert_eq!(out.split_by, "track");
        // The conductor track has no notes, so skip_empty drops it.
        assert_eq!(out.skipped_empty, 1);
        assert_eq!(out.files.len(), 2);
        assert_eq!(out.files[0].filename, "part-02-piano.mid");
        assert_eq!(out.files[0].name, "Piano");
        assert_eq!(out.files[0].notes, 2);
        assert_eq!(out.files[0].channels, vec![1]);
        assert_eq!(
            out.files[0].instrument.as_deref(),
            Some("Acoustic Grand Piano")
        );
        assert_eq!(out.files[1].filename, "part-03-bass.mid");
        assert_eq!(out.files[1].notes, 1);
        assert_eq!(out.files[1].channels, vec![2]);

        // Each part is a real single-track file that still carries the tempo and
        // time signature, so it plays back at 120 BPM in 4/4 on its own.
        for f in &out.files {
            let smf = Smf::parse(&f.midi).expect("part re-parses");
            assert_eq!(smf.header.format, Format::SingleTrack);
            assert_eq!(smf.tracks.len(), 1);
            assert_eq!(smf.header.timing, Timing::Metrical(midly::num::u15::from(96)));
            let kinds: Vec<TrackEventKind> = smf.tracks[0].iter().map(|e| e.kind).collect();
            assert!(kinds
                .iter()
                .any(|k| matches!(k, TrackEventKind::Meta(MetaMessage::Tempo(_)))));
            assert!(kinds
                .iter()
                .any(|k| matches!(k, TrackEventKind::Meta(MetaMessage::TimeSignature(..)))));
            assert_eq!(
                kinds
                    .iter()
                    .filter(|k| matches!(k, TrackEventKind::Meta(MetaMessage::EndOfTrack)))
                    .count(),
                1
            );
        }
        // Note positions survive: the piano part still ends at tick 192 (2 beats
        // at 96 PPQ = 1 second at 120 BPM).
        assert!((out.files[0].seconds - 1.0).abs() < 1e-9);
    }

    #[test]
    fn without_the_conductor_a_part_carries_no_tempo() {
        let opts = Options {
            include_conductor: false,
            ..Options::default()
        };
        let out = split_bytes(&sample(), &opts).unwrap();
        assert_eq!(out.conductor_events, 0);
        let smf = Smf::parse(&out.files[0].midi).unwrap();
        assert!(!smf.tracks[0]
            .iter()
            .any(|e| matches!(e.kind, TrackEventKind::Meta(MetaMessage::Tempo(_)))));
    }

    #[test]
    fn format_1_output_keeps_the_conductor_as_its_own_track() {
        let opts = Options {
            output_format: OutFormat::Format1,
            ..Options::default()
        };
        let out = split_bytes(&sample(), &opts).unwrap();
        let smf = Smf::parse(&out.files[0].midi).unwrap();
        assert_eq!(smf.header.format, Format::Parallel);
        assert_eq!(smf.tracks.len(), 2);
        assert!(smf.tracks[0]
            .iter()
            .any(|e| matches!(e.kind, TrackEventKind::Meta(MetaMessage::Tempo(_)))));
        assert!(smf.tracks[1]
            .iter()
            .any(|e| matches!(e.kind, TrackEventKind::Midi { .. })));
    }

    #[test]
    fn splitting_by_channel_gathers_events_across_tracks() {
        let opts = Options {
            split_by: SplitBy::Channel,
            ..Options::default()
        };
        let out = split_bytes(&sample(), &opts).unwrap();
        assert_eq!(out.split_by, "channel");
        assert_eq!(out.files.len(), 2);
        assert_eq!(out.files[0].source, "channel 1");
        assert_eq!(out.files[0].index, 1);
        assert_eq!(out.files[1].source, "channel 2");
        assert_eq!(out.files[1].notes, 1);
    }

    #[test]
    fn a_format_0_file_is_split_by_channel_automatically() {
        // Re-write the sample as Format 0 by merging every track into one.
        let src = sample();
        let smf = Smf::parse(&src).unwrap();
        let mut merged: Vec<(u64, TrackEventKind)> = Vec::new();
        for track in to_abs(&smf.tracks) {
            for (t, k) in track {
                if !matches!(k, TrackEventKind::Meta(MetaMessage::EndOfTrack)) {
                    merged.push((t, k));
                }
            }
        }
        let one = Smf {
            header: Header::new(Format::SingleTrack, smf.header.timing),
            tracks: vec![to_track(merged).unwrap()],
        };
        let mut bytes = Vec::new();
        one.write_std(&mut bytes).unwrap();

        let out = split_bytes(&bytes, &Options::default()).unwrap();
        assert!(out.auto_channel, "a Format 0 file falls back to a channel split");
        assert_eq!(out.split_by, "channel");
        assert_eq!(out.files.len(), 2);
    }

    #[test]
    fn select_keeps_only_the_listed_parts() {
        let opts = Options {
            select: "3".to_string(),
            ..Options::default()
        };
        let out = split_bytes(&sample(), &opts).unwrap();
        assert_eq!(out.files.len(), 1);
        assert_eq!(out.files[0].source, "track 3");
        assert_eq!(out.skipped_unselected, 1);
    }

    #[test]
    fn keeping_empty_parts_exports_the_conductor_track_too() {
        let opts = Options {
            skip_empty: false,
            ..Options::default()
        };
        let out = split_bytes(&sample(), &opts).unwrap();
        assert_eq!(out.files.len(), 3);
        assert_eq!(out.files[0].name, "Conductor");
        assert_eq!(out.files[0].notes, 0);
    }

    #[test]
    fn list_mode_returns_the_table_without_file_bytes() {
        let opts = Options {
            output: OutputMode::List,
            ..Options::default()
        };
        let out = split_bytes(&sample(), &opts).unwrap();
        assert!(out.list_only);
        assert!(out.files.iter().all(|f| f.midi.is_empty()));
        let json = out.to_json();
        assert!(!json.contains("data_url"));
        assert!(json.contains("part-02-piano.mid"));
    }

    #[test]
    fn json_carries_a_playable_data_url_per_part() {
        let json = split_to_json(&b64_encode(&sample()), "base64", &Options::default()).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["files_produced"], 2);
        let url = v["files"][0]["data_url"].as_str().unwrap();
        assert!(url.starts_with("data:audio/midi;base64,"));
        let bytes = decode_bytes(url.trim_start_matches("data:audio/midi;base64,"), "base64").unwrap();
        assert_eq!(&bytes[..4], b"MThd");
    }

    #[test]
    fn duplicate_track_names_get_distinct_filenames() {
        let mut used = Vec::new();
        assert_eq!(
            unique_filename("part", 2, "Piano", &mut used),
            "part-02-piano.mid"
        );
        assert_eq!(
            unique_filename("part", 2, "Piano", &mut used),
            "part-02-piano-2.mid"
        );
    }

    #[test]
    fn hex_and_base64_input_agree() {
        let bytes = sample();
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let a = split(&hex, "auto", &Options::default()).unwrap();
        let b = split(&b64_encode(&bytes), "auto", &Options::default()).unwrap();
        assert_eq!(a.files[0].midi, b.files[0].midi);
    }

    // --- error paths ---------------------------------------------------------

    #[test]
    fn rejects_input_that_is_not_a_midi_file() {
        let err = split("aGVsbG8gd29ybGQ=", "base64", &Options::default()).unwrap_err();
        assert!(err.contains("MThd"), "got: {err}");
    }

    #[test]
    fn rejects_an_unknown_split_axis() {
        let err = Options::parse("voices", true, "format-0", true, "", "part", "files").unwrap_err();
        assert_eq!(err, "unknown split_by 'voices' (use track or channel)");
    }

    #[test]
    fn rejects_a_malformed_selection() {
        let err = Options::parse("track", true, "format-0", true, "1,x", "part", "files").unwrap_err();
        assert!(err.contains("could not read 'x'"), "got: {err}");
        let err = Options::parse("track", true, "format-0", true, "5-2", "part", "files").unwrap_err();
        assert!(err.contains("runs backwards"), "got: {err}");
    }

    #[test]
    fn a_selection_that_matches_nothing_explains_itself() {
        let opts = Options {
            select: "9".to_string(),
            ..Options::default()
        };
        let err = split_bytes(&sample(), &opts).unwrap_err();
        assert!(err.contains("matched none"), "got: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = split("   ", "auto", &Options::default()).unwrap_err();
        assert!(err.contains("input is empty"), "got: {err}");
    }
}
