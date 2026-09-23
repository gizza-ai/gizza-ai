//! gizza-ai/video-bitrate-checker core — measure a media file's overall and
//! per-stream bitrate and flag it against a configured min/max range.
//!
//! The overall bitrate is the honest one every player agrees on: file size × 8
//! ÷ duration. The per-stream numbers are *measured*, not guessed: the
//! pure-Rust `symphonia` demuxers walk the container's packets and we total the
//! payload bytes carried for each track, then divide by that track's own
//! duration. That reproduces `ffprobe -show_entries stream=bit_rate` to the
//! kilobit on files ffprobe can answer for, and still works on WebM/Matroska,
//! where the container stores no per-stream bitrate at all.
//!
//! Nothing is decoded, no ffmpeg is involved and there is no I/O, so the exact
//! same measurement runs in the chat block, the CLI and the browser page.
//!
//! symphonia reports a container's video track with the null codec (it has no
//! video decoder), so stream kind, codec name and picture size come from a
//! small best-effort read of the container's own track table: the MP4 `hdlr` /
//! `stsd` atoms and the Matroska `TrackEntry` elements. When that read fails we
//! degrade to what symphonia knows rather than guessing.

use serde::Serialize;
use std::collections::HashMap;
use std::io::Cursor;
use symphonia::core::codecs::CodecType;
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Largest accepted threshold, in kbit/s (10 Gbit/s). Anything above this is a
/// unit mix-up (bit/s typed into a kbit/s box), not a real limit.
pub const MAX_BITRATE_KBPS: f64 = 10_000_000.0;

/// Which bitrate the min/max range is applied to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Target {
    /// Whole file: size × 8 ÷ duration (default).
    #[default]
    Overall,
    /// Video streams only, summed.
    Video,
    /// Audio streams only, summed.
    Audio,
}

impl Target {
    /// Canonical wire name, as it appears in the schema and the JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            Target::Overall => "overall",
            Target::Video => "video",
            Target::Audio => "audio",
        }
    }

    /// Parse the enum value, with a message naming the accepted values.
    pub fn parse(s: &str) -> Result<Target, String> {
        match s.trim() {
            "overall" => Ok(Target::Overall),
            "video" => Ok(Target::Video),
            "audio" => Ok(Target::Audio),
            other => Err(format!(
                "unknown target {other:?}; use one of overall, video, audio"
            )),
        }
    }
}

/// The unit the `min_bitrate` / `max_bitrate` thresholds are written in. The
/// report always states both kbit/s and Mbit/s regardless.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Units {
    /// kbit/s — how ad specs and audio tracks are usually quoted (default).
    #[default]
    Kbps,
    /// Mbit/s — how upload guidance for 1080p/4K is usually quoted.
    Mbps,
}

impl Units {
    /// Canonical wire name, as it appears in the schema and the JSON output.
    pub fn as_str(self) -> &'static str {
        match self {
            Units::Kbps => "kbps",
            Units::Mbps => "Mbps",
        }
    }

    /// Parse the enum value, with a message naming the accepted values.
    pub fn parse(s: &str) -> Result<Units, String> {
        match s.trim() {
            "kbps" => Ok(Units::Kbps),
            "Mbps" => Ok(Units::Mbps),
            other => Err(format!("unknown units {other:?}; use one of kbps, Mbps")),
        }
    }

    /// Convert a threshold written in these units to kbit/s.
    pub fn to_kbps(self, v: f64) -> f64 {
        match self {
            Units::Kbps => v,
            Units::Mbps => v * 1000.0,
        }
    }
}

/// The range to flag against. Both bounds default to 0, which means "no bound"
/// — with neither set the tool simply reports the measured bitrates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Options {
    /// Lower bound in `units`; 0 disables the floor.
    pub min_bitrate: f64,
    /// Upper bound in `units`; 0 disables the ceiling.
    pub max_bitrate: f64,
    pub units: Units,
    pub target: Target,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            min_bitrate: 0.0,
            max_bitrate: 0.0,
            units: Units::Kbps,
            target: Target::Overall,
        }
    }
}

impl Options {
    /// Reject an impossible range BEFORE any file is fetched or read.
    pub fn validate(&self) -> Result<(), String> {
        for (label, v) in [
            ("min_bitrate", self.min_bitrate),
            ("max_bitrate", self.max_bitrate),
        ] {
            if !v.is_finite() {
                return Err(format!("{label} must be a finite number"));
            }
            if v < 0.0 {
                return Err(format!("{label} must not be negative"));
            }
            if self.units.to_kbps(v) > MAX_BITRATE_KBPS {
                return Err(format!(
                    "{label} is above the {MAX_BITRATE_KBPS} kbit/s ceiling — check the units"
                ));
            }
        }
        if self.min_kbps() > 0.0 && self.max_kbps() > 0.0 && self.min_kbps() > self.max_kbps() {
            return Err(format!(
                "min_bitrate ({}) must not be above max_bitrate ({})",
                self.min_bitrate, self.max_bitrate
            ));
        }
        Ok(())
    }

    /// Lower bound in kbit/s (0 = none).
    pub fn min_kbps(&self) -> f64 {
        self.units.to_kbps(self.min_bitrate)
    }

    /// Upper bound in kbit/s (0 = none).
    pub fn max_kbps(&self) -> f64 {
        self.units.to_kbps(self.max_bitrate)
    }
}

/// One measured stream.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StreamInfo {
    /// 1-based position in the container's track table.
    pub index: usize,
    /// "video", "audio", "subtitle", "data" or "other".
    pub kind: String,
    /// Friendly codec name, e.g. "H.264 / AVC", "AAC", "VP9".
    pub codec: String,
    /// Measured bitrate of this stream in kbit/s (payload bytes ÷ duration).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_mbps: Option<f64>,
    /// Share of the file's bytes this stream's payload accounts for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub share_percent: Option<f64>,
    /// Payload bytes carried for this stream (container overhead excluded).
    pub bytes: u64,
    /// Packets (video frames, audio frames) demuxed for this stream.
    pub packets: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
    /// Measured frame rate (packets ÷ duration) for video streams.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frame_rate: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
}

/// The full bitrate report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// "PASS", "FAIL", or "INFO" when no range was configured.
    pub status: String,
    /// `null` when no range was configured (nothing to pass or fail).
    pub pass: Option<bool>,
    /// "ok", "too_low", "too_high", "not_checked" or "unmeasurable".
    pub reason: String,
    /// Which bitrate the range was applied to.
    pub target: String,
    /// The unit the thresholds were written in.
    pub units: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_bitrate_kbps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked_bitrate_mbps: Option<f64>,
    /// Configured floor in kbit/s (`null` when none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min_bitrate_kbps: Option<f64>,
    /// Configured ceiling in kbit/s (`null` when none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_bitrate_kbps: Option<f64>,
    pub container: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_human: Option<String>,
    pub file_bytes: u64,
    pub file_size_human: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall_bitrate_kbps: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall_bitrate_mbps: Option<f64>,
    /// Sum of the measured video streams (`null` when the file has none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub video_bitrate_kbps: Option<f64>,
    /// Sum of the measured audio streams (`null` when the file has none).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_bitrate_kbps: Option<f64>,
    /// Overall minus the streams: index tables, headers and muxer padding.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_overhead_kbps: Option<f64>,
    pub stream_count: usize,
    pub streams: Vec<StreamInfo>,
    pub summary: String,
}

/// Round to one decimal — the resolution a bitrate is worth quoting at.
fn r1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

/// Round to three decimals, for Mbit/s.
fn r3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}

fn kbps(bytes: u64, seconds: f64) -> Option<f64> {
    if seconds > 0.0 {
        Some(r1(bytes as f64 * 8.0 / seconds / 1000.0))
    } else {
        None
    }
}

fn human_bytes(n: u64) -> String {
    const K: f64 = 1024.0;
    let f = n as f64;
    if f < K {
        format!("{n} B")
    } else if f < K * K {
        format!("{:.1} KiB", f / K)
    } else if f < K * K * K {
        format!("{:.1} MiB", f / (K * K))
    } else {
        format!("{:.2} GiB", f / (K * K * K))
    }
}

fn human_duration(secs: f64) -> String {
    let total = secs.max(0.0);
    let h = (total / 3600.0).floor() as u64;
    let m = ((total % 3600.0) / 60.0).floor() as u64;
    let s = total % 60.0;
    if h > 0 {
        format!("{h}:{m:02}:{s:06.3}")
    } else {
        format!("{m}:{s:06.3}")
    }
}

/// Apply the configured range to one measured bitrate. Split out so the rule
/// table is testable without any media bytes.
pub fn verdict(
    bitrate_kbps: Option<f64>,
    min_kbps: f64,
    max_kbps: f64,
) -> (&'static str, Option<bool>, &'static str) {
    if min_kbps <= 0.0 && max_kbps <= 0.0 {
        return ("INFO", None, "not_checked");
    }
    let Some(b) = bitrate_kbps else {
        return ("INFO", None, "unmeasurable");
    };
    if min_kbps > 0.0 && b < min_kbps {
        return ("FAIL", Some(false), "too_low");
    }
    if max_kbps > 0.0 && b > max_kbps {
        return ("FAIL", Some(false), "too_high");
    }
    ("PASS", Some(true), "ok")
}

// ---------------------------------------------------------------------------
// Container track table (kind / codec / picture size), best effort.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, PartialEq)]
struct ContainerTrack {
    kind: Option<&'static str>,
    codec: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    sample_rate: Option<u32>,
    channels: Option<u32>,
}

/// Direct children of an ISO-BMFF box payload, as `(type, payload)`.
fn mp4_boxes(buf: &[u8]) -> Vec<(&[u8], &[u8])> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i + 8 <= buf.len() {
        let size = u32::from_be_bytes([buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]) as usize;
        let typ = &buf[i + 4..i + 8];
        let (header, total) = if size == 1 {
            if i + 16 > buf.len() {
                break;
            }
            let large = u64::from_be_bytes(buf[i + 8..i + 16].try_into().unwrap()) as usize;
            (16, large)
        } else if size == 0 {
            (8, buf.len() - i)
        } else {
            (8, size)
        };
        if total < header || i + total > buf.len() {
            break;
        }
        out.push((typ, &buf[i + header..i + total]));
        i += total;
        // A pathological zero-length box would spin forever.
        if total == 0 {
            break;
        }
    }
    out
}

fn mp4_find<'a>(buf: &'a [u8], typ: &[u8; 4]) -> Option<&'a [u8]> {
    mp4_boxes(buf)
        .into_iter()
        .find(|(t, _)| *t == typ)
        .map(|(_, p)| p)
}

fn mp4_path<'a>(buf: &'a [u8], parts: &[&[u8; 4]]) -> Option<&'a [u8]> {
    let mut cur = buf;
    for p in parts {
        cur = mp4_find(cur, p)?;
    }
    Some(cur)
}

/// Friendly name for an MP4 sample-entry format (the `stsd` four-character code).
fn mp4_codec_name(fourcc: &[u8]) -> Option<String> {
    let name = match fourcc {
        b"avc1" | b"avc3" => "H.264 / AVC",
        b"hev1" | b"hvc1" => "H.265 / HEVC",
        b"av01" => "AV1",
        b"vp09" => "VP9",
        b"vp08" => "VP8",
        b"mp4v" => "MPEG-4 Visual",
        b"jpeg" | b"mjpa" | b"mjpb" => "Motion JPEG",
        b"apch" | b"apcn" | b"apcs" | b"apco" | b"ap4h" => "Apple ProRes",
        b"dvh1" | b"dvhe" => "Dolby Vision (HEVC)",
        b"mp4a" => "AAC",
        b"alac" => "ALAC",
        b"Opus" => "Opus",
        b"fLaC" => "FLAC",
        b"ac-3" => "AC-3",
        b"ec-3" => "E-AC-3",
        b".mp3" | b"mp3 " => "MP3",
        b"sowt" | b"twos" | b"lpcm" | b"in24" | b"in32" => "PCM",
        b"tx3g" | b"wvtt" | b"c608" | b"c708" => "Timed text",
        _ => return None,
    };
    Some(name.to_string())
}

fn mp4_kind(handler: &[u8]) -> Option<&'static str> {
    Some(match handler {
        b"vide" => "video",
        b"soun" => "audio",
        b"sbtl" | b"subt" | b"text" | b"clcp" => "subtitle",
        b"meta" | b"data" => "data",
        b"hint" => "other",
        _ => return None,
    })
}

/// The container's own `mvhd` play time, which is what ffprobe and players
/// report as the file duration (it can differ slightly from the longest
/// track's, e.g. when an AAC track runs past the video by a frame or two).
fn mp4_duration(bytes: &[u8]) -> Option<f64> {
    let mvhd = mp4_path(bytes, &[b"moov", b"mvhd"])?;
    let be32 = |o: usize| -> Option<u64> {
        Some(u32::from_be_bytes(mvhd.get(o..o + 4)?.try_into().ok()?) as u64)
    };
    let (timescale, duration) = match *mvhd.first()? {
        0 => (be32(12)?, be32(16)?),
        1 => (
            be32(20)?,
            u64::from_be_bytes(mvhd.get(24..32)?.try_into().ok()?),
        ),
        _ => return None,
    };
    // 0xFFFFFFFF is the "unknown duration" sentinel.
    if timescale == 0 || duration == 0 || duration == u32::MAX as u64 {
        return None;
    }
    Some(duration as f64 / timescale as f64)
}

/// Read the MP4 track table. Keyed by the id symphonia reports for ISO-BMFF,
/// which is the track's position in `moov` (see `symphonia-format-isomp4`).
fn mp4_tracks(bytes: &[u8]) -> HashMap<u32, ContainerTrack> {
    let mut out = HashMap::new();
    let Some(moov) = mp4_find(bytes, b"moov") else {
        return out;
    };
    for (idx, (_, trak)) in mp4_boxes(moov)
        .into_iter()
        .filter(|(t, _)| *t == b"trak")
        .enumerate()
    {
        let mut ct = ContainerTrack::default();
        if let Some(hdlr) = mp4_path(trak, &[b"mdia", b"hdlr"]) {
            if hdlr.len() >= 12 {
                ct.kind = mp4_kind(&hdlr[8..12]);
            }
        }
        if let Some(stsd) = mp4_path(trak, &[b"mdia", b"minf", b"stbl", b"stsd"]) {
            if stsd.len() >= 16 {
                ct.codec = mp4_codec_name(&stsd[12..16]);
                let be16 = |o: usize| u16::from_be_bytes([stsd[o], stsd[o + 1]]) as u32;
                match ct.kind {
                    // VisualSampleEntry: width/height sit 24 bytes into the entry.
                    Some("video") if stsd.len() >= 44 => {
                        ct.width = Some(be16(40));
                        ct.height = Some(be16(42));
                    }
                    // AudioSampleEntry: channel count then a 16.16 sample rate.
                    Some("audio") if stsd.len() >= 44 => {
                        ct.channels = Some(be16(32)).filter(|c| *c > 0);
                        ct.sample_rate = Some(be16(40)).filter(|r| *r > 0);
                    }
                    _ => {}
                }
            }
        }
        out.insert(idx as u32, ct);
    }
    out
}

/// Read one EBML variable-length integer. `keep_marker` is true for element
/// ids (whose marker bits are part of the id) and false for sizes.
/// Returns the value and, for sizes, whether it was the all-ones "unknown".
fn ebml_vint(buf: &[u8], i: &mut usize, keep_marker: bool) -> Option<(u64, bool)> {
    let first = *buf.get(*i)?;
    if first == 0 {
        return None;
    }
    let len = first.leading_zeros() as usize + 1;
    if len > 8 || *i + len > buf.len() {
        return None;
    }
    // An 8-byte vint's first byte is all marker (`0x01`); `0xFF >> 8` would
    // overflow the shift, so mask explicitly.
    let mask: u8 = if len >= 8 { 0 } else { 0xFFu8 >> len };
    let mut value = if keep_marker {
        first as u64
    } else {
        (first & mask) as u64
    };
    let mut data = (first & mask) as u64;
    for k in 1..len {
        value = (value << 8) | buf[*i + k] as u64;
        data = (data << 8) | buf[*i + k] as u64;
    }
    *i += len;
    let unknown = data == (1u64 << (7 * len)) - 1;
    Some((value, unknown))
}

/// Direct children of an EBML master element, as `(id, payload)`. Stops at the
/// first element whose size is "unknown", after yielding it with the rest of
/// the buffer as its payload (this is how a streamed Segment is written).
fn ebml_children(buf: &[u8]) -> Vec<(u64, &[u8])> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < buf.len() {
        let Some((id, _)) = ebml_vint(buf, &mut i, true) else {
            break;
        };
        let Some((size, unknown)) = ebml_vint(buf, &mut i, false) else {
            break;
        };
        if unknown {
            out.push((id, &buf[i..]));
            break;
        }
        let end = i.saturating_add(size as usize);
        if end > buf.len() {
            break;
        }
        out.push((id, &buf[i..end]));
        i = end;
    }
    out
}

fn ebml_uint(payload: &[u8]) -> Option<u64> {
    if payload.is_empty() || payload.len() > 8 {
        return None;
    }
    Some(payload.iter().fold(0u64, |a, b| (a << 8) | *b as u64))
}

fn ebml_float(payload: &[u8]) -> Option<f64> {
    match payload.len() {
        4 => Some(f32::from_be_bytes(payload.try_into().ok()?) as f64),
        8 => Some(f64::from_be_bytes(payload.try_into().ok()?)),
        _ => None,
    }
}

/// Friendly name for a Matroska `CodecID`.
fn mkv_codec_name(id: &str) -> String {
    match id {
        "V_MPEG4/ISO/AVC" => "H.264 / AVC",
        "V_MPEGH/ISO/HEVC" => "H.265 / HEVC",
        "V_AV1" => "AV1",
        "V_VP9" => "VP9",
        "V_VP8" => "VP8",
        "V_MPEG4/ISO/ASP" | "V_MPEG4/ISO/SP" => "MPEG-4 Visual",
        "V_MPEG2" => "MPEG-2 Video",
        "V_THEORA" => "Theora",
        "A_OPUS" => "Opus",
        "A_VORBIS" => "Vorbis",
        "A_AAC" => "AAC",
        "A_FLAC" => "FLAC",
        "A_MPEG/L3" => "MP3",
        "A_MPEG/L2" => "MP2",
        "A_AC3" => "AC-3",
        "A_EAC3" => "E-AC-3",
        "A_TRUEHD" => "Dolby TrueHD",
        "A_DTS" => "DTS",
        "A_ALAC" => "ALAC",
        other if other.starts_with("A_PCM") => "PCM",
        other if other.starts_with("A_") => return other.trim_start_matches("A_").to_string(),
        other if other.starts_with("V_") => return other.trim_start_matches("V_").to_string(),
        other => return other.to_string(),
    }
    .to_string()
}

fn mkv_kind(track_type: u64) -> Option<&'static str> {
    Some(match track_type {
        1 => "video",
        2 => "audio",
        0x11 => "subtitle",
        0x12 => "other",
        3 => "other",
        _ => return None,
    })
}

/// Read the Matroska/WebM track table. Keyed by `TrackNumber`, which is the id
/// symphonia reports for this container.
fn mkv_tracks(bytes: &[u8]) -> HashMap<u32, ContainerTrack> {
    const SEGMENT: u64 = 0x1853_8067;
    const TRACKS: u64 = 0x1654_AE6B;
    const TRACK_ENTRY: u64 = 0xAE;

    let mut out = HashMap::new();
    for (id, payload) in ebml_children(bytes) {
        if id != SEGMENT {
            continue;
        }
        for (sid, spayload) in ebml_children(payload) {
            if sid != TRACKS {
                continue;
            }
            for (tid, entry) in ebml_children(spayload) {
                if tid != TRACK_ENTRY {
                    continue;
                }
                let mut number: Option<u64> = None;
                let mut ct = ContainerTrack::default();
                for (fid, value) in ebml_children(entry) {
                    match fid {
                        0xD7 => number = ebml_uint(value),
                        0x83 => ct.kind = ebml_uint(value).and_then(mkv_kind),
                        0x86 => {
                            ct.codec = std::str::from_utf8(value)
                                .ok()
                                .map(|s| mkv_codec_name(s.trim_end_matches('\0')))
                        }
                        // Video master element.
                        0xE0 => {
                            for (vid, v) in ebml_children(value) {
                                match vid {
                                    0xB0 => ct.width = ebml_uint(v).map(|n| n as u32),
                                    0xBA => ct.height = ebml_uint(v).map(|n| n as u32),
                                    _ => {}
                                }
                            }
                        }
                        // Audio master element.
                        0xE1 => {
                            for (aid, v) in ebml_children(value) {
                                match aid {
                                    0xB5 => {
                                        ct.sample_rate =
                                            ebml_float(v).map(|f| f.round() as u32).filter(|r| *r > 0)
                                    }
                                    0x9F => ct.channels = ebml_uint(v).map(|n| n as u32),
                                    _ => {}
                                }
                            }
                        }
                        _ => {}
                    }
                }
                if let Some(n) = number {
                    out.insert(n as u32, ct);
                }
            }
        }
    }
    out
}

/// The Matroska segment duration: `Info/Duration` counted in `TimecodeScale`
/// nanosecond ticks (default 1 ms per tick).
fn mkv_duration(bytes: &[u8]) -> Option<f64> {
    const SEGMENT: u64 = 0x1853_8067;
    const INFO: u64 = 0x1549_A966;
    for (id, payload) in ebml_children(bytes) {
        if id != SEGMENT {
            continue;
        }
        for (sid, info) in ebml_children(payload) {
            if sid != INFO {
                continue;
            }
            let mut scale_ns = 1_000_000f64;
            let mut ticks = None;
            for (fid, v) in ebml_children(info) {
                match fid {
                    0x2AD7B1 => {
                        if let Some(n) = ebml_uint(v) {
                            if n > 0 {
                                scale_ns = n as f64;
                            }
                        }
                    }
                    0x4489 => ticks = ebml_float(v),
                    _ => {}
                }
            }
            if let Some(t) = ticks.filter(|t| *t > 0.0) {
                return Some(t * scale_ns / 1e9);
            }
        }
    }
    None
}

/// Lightweight container sniff for the friendly label (symphonia keeps the
/// format's short name private).
fn sniff_container(b: &[u8]) -> &'static str {
    if b.len() < 12 {
        return "media";
    }
    if &b[4..8] == b"ftyp" {
        return "isomp4";
    }
    if b.starts_with(&[0x1A, 0x45, 0xDF, 0xA3]) {
        return "matroska";
    }
    if b.starts_with(b"OggS") {
        return "ogg";
    }
    if b.starts_with(b"RIFF") && &b[8..12] == b"WAVE" {
        return "wave";
    }
    if b.starts_with(b"FORM") && (&b[8..12] == b"AIFF" || &b[8..12] == b"AIFC") {
        return "aiff";
    }
    if b.starts_with(b"caff") {
        return "caf";
    }
    if b.starts_with(b"fLaC") {
        return "flac";
    }
    if b.starts_with(b"ID3") || (b[0] == 0xFF && (b[1] & 0xE0) == 0xE0) {
        return "mp3";
    }
    "media"
}

fn container_label(short: &str) -> String {
    match short {
        "isomp4" => "MP4 / MOV / M4A (ISO BMFF)",
        "matroska" => "Matroska / WebM",
        "ogg" => "OGG",
        "wave" => "WAVE (RIFF)",
        "aiff" => "AIFF",
        "caf" => "Core Audio (CAF)",
        "flac" => "FLAC (native)",
        "mp3" => "MP3 (MPEG audio)",
        other => return other.to_string(),
    }
    .to_string()
}

/// Friendly name for the codecs symphonia itself identifies (audio only — it
/// has no video decoders, so a container's video track is the null codec).
fn symphonia_codec_name(c: CodecType) -> Option<String> {
    use symphonia::core::codecs::*;
    let name = match c {
        CODEC_TYPE_NULL => return None,
        CODEC_TYPE_VORBIS => "Vorbis",
        CODEC_TYPE_MP1 => "MP1",
        CODEC_TYPE_MP2 => "MP2",
        CODEC_TYPE_MP3 => "MP3",
        CODEC_TYPE_AAC => "AAC",
        CODEC_TYPE_FLAC => "FLAC",
        CODEC_TYPE_ALAC => "ALAC",
        CODEC_TYPE_OPUS => "Opus",
        CODEC_TYPE_ADPCM_MS | CODEC_TYPE_ADPCM_IMA_WAV => "ADPCM",
        other => {
            // Every PCM flavour reports as PCM; anything else is unknown.
            let pcm = [
                CODEC_TYPE_PCM_S16LE,
                CODEC_TYPE_PCM_S16BE,
                CODEC_TYPE_PCM_S24LE,
                CODEC_TYPE_PCM_S24BE,
                CODEC_TYPE_PCM_S32LE,
                CODEC_TYPE_PCM_S32BE,
                CODEC_TYPE_PCM_S8,
                CODEC_TYPE_PCM_U8,
                CODEC_TYPE_PCM_U16LE,
                CODEC_TYPE_PCM_U16BE,
                CODEC_TYPE_PCM_U24LE,
                CODEC_TYPE_PCM_U24BE,
                CODEC_TYPE_PCM_U32LE,
                CODEC_TYPE_PCM_U32BE,
                CODEC_TYPE_PCM_F32LE,
                CODEC_TYPE_PCM_F32BE,
                CODEC_TYPE_PCM_F64LE,
                CODEC_TYPE_PCM_F64BE,
            ];
            if pcm.contains(&other) {
                "PCM"
            } else {
                return None;
            }
        }
    };
    Some(name.to_string())
}

// ---------------------------------------------------------------------------
// Measurement
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Measured {
    bytes: u64,
    packets: u64,
    ts_lo: Option<u64>,
    ts_hi: u64,
}

/// Measure `bytes` and apply `opts`. Takes the buffer by value so the demuxer
/// can own it without a second multi-megabyte copy in the wasm sandbox.
pub fn analyze(bytes: Vec<u8>, opts: &Options) -> Result<Report, String> {
    opts.validate()?;
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    let file_bytes = bytes.len() as u64;
    let container = container_label(sniff_container(&bytes));
    let (by_container, declared_container_duration) = match sniff_container(&bytes) {
        "isomp4" => (mp4_tracks(&bytes), mp4_duration(&bytes)),
        "matroska" => (mkv_tracks(&bytes), mkv_duration(&bytes)),
        _ => (HashMap::new(), None),
    };

    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("unrecognised or unsupported media container: {e}"))?;
    let mut reader = probed.format;

    // Track ids as symphonia reports them, in container order, plus everything
    // the header already knows.
    struct Head {
        id: u32,
        codec: CodecType,
        sample_rate: Option<u32>,
        channels: Option<u32>,
        time_base: Option<f64>,
        declared_duration: Option<f64>,
    }
    let heads: Vec<Head> = reader
        .tracks()
        .iter()
        .map(|t| {
            let p = &t.codec_params;
            let time_base = p.time_base.map(|tb| tb.numer as f64 / tb.denom as f64);
            let declared_duration = match (p.n_frames, time_base) {
                (Some(n), Some(tb)) => Some(n as f64 * tb),
                _ => match (p.n_frames, p.sample_rate) {
                    (Some(n), Some(sr)) if sr > 0 => Some(n as f64 / sr as f64),
                    _ => None,
                },
            };
            Head {
                id: t.id,
                codec: p.codec,
                sample_rate: p.sample_rate,
                channels: p.channels.map(|c| c.count() as u32),
                time_base,
                declared_duration,
            }
        })
        .collect();
    if heads.is_empty() {
        return Err("no media tracks found in the container".into());
    }

    let mut index_of: HashMap<u32, usize> = HashMap::new();
    for (i, h) in heads.iter().enumerate() {
        index_of.insert(h.id, i);
    }
    let mut measured: Vec<Measured> = heads.iter().map(|_| Measured::default()).collect();

    // Demux (no decode) and total the payload bytes carried per track.
    loop {
        match reader.next_packet() {
            Ok(pkt) => {
                let Some(&i) = index_of.get(&pkt.track_id()) else {
                    continue;
                };
                let m = &mut measured[i];
                m.bytes += pkt.data.len() as u64;
                m.packets += 1;
                let ts = pkt.ts();
                m.ts_lo = Some(m.ts_lo.map_or(ts, |lo| lo.min(ts)));
                m.ts_hi = m.ts_hi.max(ts.saturating_add(pkt.dur()));
            }
            // A truncated tail is normal for a streamed/partial file — report
            // what was measured rather than refusing the whole file.
            Err(SymError::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(_) => break,
        }
    }

    let mut streams = Vec::with_capacity(heads.len());
    let mut duration_seconds: Option<f64> = None;
    let (mut video_kbps, mut audio_kbps) = (None::<f64>, None::<f64>);

    for (i, h) in heads.iter().enumerate() {
        let m = &measured[i];
        let meta = by_container.get(&h.id).cloned().unwrap_or_default();
        let measured_duration = match (m.ts_lo, h.time_base) {
            (Some(lo), Some(tb)) if m.ts_hi > lo => Some((m.ts_hi - lo) as f64 * tb),
            _ => None,
        };
        let stream_duration = h.declared_duration.or(measured_duration).filter(|d| *d > 0.0);
        if let Some(d) = stream_duration {
            duration_seconds = Some(duration_seconds.map_or(d, |x: f64| x.max(d)));
        }
        let bitrate = stream_duration.and_then(|d| kbps(m.bytes, d));

        let kind = meta
            .kind
            .map(|k| k.to_string())
            .unwrap_or_else(|| match symphonia_codec_name(h.codec) {
                Some(_) => "audio".to_string(),
                None => "other".to_string(),
            });
        let codec = meta
            .codec
            .or_else(|| symphonia_codec_name(h.codec))
            .unwrap_or_else(|| "unknown".to_string());

        if let Some(b) = bitrate {
            match kind.as_str() {
                "video" => video_kbps = Some(video_kbps.unwrap_or(0.0) + b),
                "audio" => audio_kbps = Some(audio_kbps.unwrap_or(0.0) + b),
                _ => {}
            }
        }

        let frame_rate = match (kind.as_str(), stream_duration) {
            ("video", Some(d)) if m.packets > 1 && d > 0.0 => {
                Some((m.packets as f64 / d * 100.0).round() / 100.0)
            }
            _ => None,
        };

        streams.push(StreamInfo {
            index: i + 1,
            kind,
            codec,
            bitrate_kbps: bitrate,
            bitrate_mbps: bitrate.map(|b| r3(b / 1000.0)),
            share_percent: (file_bytes > 0)
                .then(|| r1(m.bytes as f64 * 100.0 / file_bytes as f64)),
            bytes: m.bytes,
            packets: m.packets,
            duration_seconds: stream_duration.map(|d| (d * 1000.0).round() / 1000.0),
            width: meta.width,
            height: meta.height,
            frame_rate,
            sample_rate: meta.sample_rate.or(h.sample_rate),
            channels: meta.channels.or(h.channels),
        });
    }

    // The file's play time, as ffprobe and players report it: the container's
    // own declared duration, falling back to the longest track's.
    let duration_seconds = declared_container_duration
        .filter(|d| *d > 0.0)
        .or(duration_seconds);
    let overall = duration_seconds.and_then(|d| kbps(file_bytes, d));
    let video_kbps = video_kbps.map(r1);
    let audio_kbps = audio_kbps.map(r1);
    // What the muxer spends on headers, index tables and padding: the overall
    // rate minus everything the streams account for.
    let overhead = overall.map(|o| {
        let streamed: f64 = streams.iter().filter_map(|s| s.bitrate_kbps).sum();
        r1((o - streamed).max(0.0))
    });

    let checked = match opts.target {
        Target::Overall => overall,
        Target::Video => {
            if !streams.iter().any(|s| s.kind == "video") {
                return Err(
                    "this file has no video stream — check the overall or audio bitrate instead"
                        .into(),
                );
            }
            video_kbps
        }
        Target::Audio => {
            if !streams.iter().any(|s| s.kind == "audio") {
                return Err(
                    "this file has no audio stream — check the overall or video bitrate instead"
                        .into(),
                );
            }
            audio_kbps
        }
    };

    let (status, pass, reason) = verdict(checked, opts.min_kbps(), opts.max_kbps());
    let summary = summarize(
        status, reason, opts, checked, overall, video_kbps, audio_kbps,
    );

    Ok(Report {
        status: status.to_string(),
        pass,
        reason: reason.to_string(),
        target: opts.target.as_str().to_string(),
        units: opts.units.as_str().to_string(),
        checked_bitrate_kbps: checked,
        checked_bitrate_mbps: checked.map(|b| r3(b / 1000.0)),
        min_bitrate_kbps: (opts.min_kbps() > 0.0).then(|| r1(opts.min_kbps())),
        max_bitrate_kbps: (opts.max_kbps() > 0.0).then(|| r1(opts.max_kbps())),
        container,
        duration_seconds: duration_seconds.map(|d| (d * 1000.0).round() / 1000.0),
        duration_human: duration_seconds.map(human_duration),
        file_bytes,
        file_size_human: human_bytes(file_bytes),
        overall_bitrate_kbps: overall,
        overall_bitrate_mbps: overall.map(|b| r3(b / 1000.0)),
        video_bitrate_kbps: video_kbps,
        audio_bitrate_kbps: audio_kbps,
        container_overhead_kbps: overhead,
        stream_count: streams.len(),
        streams,
        summary,
    })
}

fn fmt_kbps(v: f64) -> String {
    if v >= 1000.0 {
        format!("{v} kbit/s ({} Mbit/s)", r3(v / 1000.0))
    } else {
        format!("{v} kbit/s")
    }
}

fn summarize(
    status: &str,
    reason: &str,
    opts: &Options,
    checked: Option<f64>,
    overall: Option<f64>,
    video: Option<f64>,
    audio: Option<f64>,
) -> String {
    let label = match opts.target {
        Target::Overall => "Overall bitrate",
        Target::Video => "Video bitrate",
        Target::Audio => "Audio bitrate",
    };
    let mut parts = Vec::new();
    if let Some(o) = overall {
        parts.push(format!("overall {}", fmt_kbps(o)));
    }
    if let Some(v) = video {
        parts.push(format!("video {}", fmt_kbps(v)));
    }
    if let Some(a) = audio {
        parts.push(format!("audio {}", fmt_kbps(a)));
    }
    let breakdown = if parts.is_empty() {
        "no measurable stream".to_string()
    } else {
        parts.join(", ")
    };

    match (status, reason, checked) {
        ("INFO", "not_checked", _) => {
            format!("{breakdown}. No min/max range was set, so nothing was flagged.")
        }
        ("INFO", _, _) => format!(
            "{breakdown}. The {} bitrate could not be measured — the container records no usable duration.",
            opts.target.as_str()
        ),
        ("FAIL", "too_low", Some(b)) => format!(
            "FAIL — {label} {} is below the {} floor by {} kbit/s. ({breakdown}.)",
            fmt_kbps(b),
            fmt_kbps(r1(opts.min_kbps())),
            r1(opts.min_kbps() - b)
        ),
        ("FAIL", _, Some(b)) => format!(
            "FAIL — {label} {} is above the {} ceiling by {} kbit/s. ({breakdown}.)",
            fmt_kbps(b),
            fmt_kbps(r1(opts.max_kbps())),
            r1(b - opts.max_kbps())
        ),
        (_, _, Some(b)) => format!(
            "PASS — {label} {} is inside the allowed range. ({breakdown}.)",
            fmt_kbps(b)
        ),
        _ => breakdown,
    }
}

/// Convenience wrapper returning the report as pretty JSON (used by the page).
pub fn analyze_json(bytes: Vec<u8>, opts: &Options) -> Result<String, String> {
    let report = analyze(bytes, opts)?;
    serde_json::to_string_pretty(&report).map_err(|e| format!("serialize report: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn units_convert_and_parse() {
        assert_eq!(Units::parse("kbps").unwrap(), Units::Kbps);
        assert_eq!(Units::parse("Mbps").unwrap(), Units::Mbps);
        assert_eq!(Units::Mbps.to_kbps(8.0), 8000.0);
        assert_eq!(Units::Kbps.to_kbps(800.0), 800.0);
        assert!(Units::parse("mbit").unwrap_err().contains("kbps, Mbps"));
    }

    #[test]
    fn targets_parse() {
        for t in ["overall", "video", "audio"] {
            assert_eq!(Target::parse(t).unwrap().as_str(), t);
        }
        assert!(Target::parse("subtitle")
            .unwrap_err()
            .contains("overall, video, audio"));
    }

    #[test]
    fn verdict_rules() {
        // No bounds configured: report only.
        assert_eq!(verdict(Some(900.0), 0.0, 0.0), ("INFO", None, "not_checked"));
        // Ceiling only.
        assert_eq!(verdict(Some(900.0), 0.0, 800.0), ("FAIL", Some(false), "too_high"));
        assert_eq!(verdict(Some(700.0), 0.0, 800.0), ("PASS", Some(true), "ok"));
        // Floor only.
        assert_eq!(verdict(Some(100.0), 128.0, 0.0), ("FAIL", Some(false), "too_low"));
        // Both — boundaries are inclusive.
        assert_eq!(verdict(Some(800.0), 500.0, 800.0), ("PASS", Some(true), "ok"));
        assert_eq!(verdict(Some(500.0), 500.0, 800.0), ("PASS", Some(true), "ok"));
        // Nothing measurable but a range was asked for.
        assert_eq!(verdict(None, 500.0, 800.0), ("INFO", None, "unmeasurable"));
    }

    #[test]
    fn options_reject_impossible_ranges() {
        let bad = Options {
            min_bitrate: 900.0,
            max_bitrate: 800.0,
            ..Default::default()
        };
        assert!(bad.validate().unwrap_err().contains("must not be above"));

        let neg = Options {
            min_bitrate: -1.0,
            ..Default::default()
        };
        assert!(neg.validate().unwrap_err().contains("must not be negative"));

        let huge = Options {
            max_bitrate: 20_000_000.0,
            ..Default::default()
        };
        assert!(huge.validate().unwrap_err().contains("check the units"));

        let nan = Options {
            min_bitrate: f64::NAN,
            ..Default::default()
        };
        assert!(nan.validate().unwrap_err().contains("finite"));

        // A Mbps ceiling is converted before the range check.
        let ok = Options {
            min_bitrate: 6.0,
            max_bitrate: 9.0,
            units: Units::Mbps,
            ..Default::default()
        };
        assert!(ok.validate().is_ok());
        assert_eq!(ok.min_kbps(), 6000.0);
    }

    #[test]
    fn errors_on_empty_and_garbage() {
        let o = Options::default();
        assert_eq!(analyze(Vec::new(), &o).unwrap_err(), "input is empty");
        let e = analyze(b"not a media file at all, just some text".to_vec(), &o).unwrap_err();
        assert!(e.contains("unrecognised or unsupported media container"), "{e}");
    }

    /// Minimal valid 8-bit mono PCM WAV: 44-byte header + N sample bytes.
    fn wav(sample_rate: u32, samples: usize) -> Vec<u8> {
        let data = vec![128u8; samples];
        let mut w = Vec::new();
        w.extend_from_slice(b"RIFF");
        w.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
        w.extend_from_slice(b"WAVE");
        w.extend_from_slice(b"fmt ");
        w.extend_from_slice(&16u32.to_le_bytes());
        w.extend_from_slice(&1u16.to_le_bytes()); // PCM
        w.extend_from_slice(&1u16.to_le_bytes()); // mono
        w.extend_from_slice(&sample_rate.to_le_bytes());
        w.extend_from_slice(&sample_rate.to_le_bytes()); // byte rate
        w.extend_from_slice(&1u16.to_le_bytes()); // block align
        w.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
        w.extend_from_slice(b"data");
        w.extend_from_slice(&(data.len() as u32).to_le_bytes());
        w.extend_from_slice(&data);
        w
    }

    #[test]
    fn measures_a_real_wav_and_reports_only_without_a_range() {
        // 8000 Hz, 8-bit mono, 8000 samples = exactly 1 second of audio.
        let r = analyze(wav(8000, 8000), &Options::default()).unwrap();
        assert_eq!(r.container, "WAVE (RIFF)");
        assert_eq!(r.status, "INFO");
        assert_eq!(r.pass, None);
        assert_eq!(r.reason, "not_checked");
        assert_eq!(r.duration_seconds, Some(1.0));
        assert_eq!(r.stream_count, 1);
        // 8000 bytes of payload at 8-bit mono 8 kHz = 64 kbit/s.
        assert_eq!(r.audio_bitrate_kbps, Some(64.0));
        assert_eq!(r.streams[0].kind, "audio");
        assert_eq!(r.streams[0].codec, "PCM");
        assert_eq!(r.streams[0].sample_rate, Some(8000));
        // The file is the payload plus a 44-byte header.
        assert_eq!(r.file_bytes, 8044);
        assert_eq!(r.overall_bitrate_kbps, Some(64.4));
        assert!(r.summary.contains("No min/max range"), "{}", r.summary);
    }

    #[test]
    fn flags_a_bitrate_above_the_ceiling() {
        let opts = Options {
            max_bitrate: 32.0,
            ..Default::default()
        };
        let r = analyze(wav(8000, 8000), &opts).unwrap();
        assert_eq!(r.status, "FAIL");
        assert_eq!(r.pass, Some(false));
        assert_eq!(r.reason, "too_high");
        assert_eq!(r.checked_bitrate_kbps, Some(64.4));
        assert_eq!(r.max_bitrate_kbps, Some(32.0));
        assert_eq!(r.min_bitrate_kbps, None);
        assert!(r.summary.starts_with("FAIL —"), "{}", r.summary);
    }

    #[test]
    fn a_video_target_on_an_audio_only_file_is_an_error() {
        let opts = Options {
            target: Target::Video,
            max_bitrate: 5.0,
            units: Units::Mbps,
            ..Default::default()
        };
        let e = analyze(wav(8000, 8000), &opts).unwrap_err();
        assert!(e.contains("no video stream"), "{e}");
    }

    #[test]
    fn human_helpers() {
        assert_eq!(human_bytes(512), "512 B");
        assert_eq!(human_bytes(23593), "23.0 KiB");
        assert_eq!(human_bytes(5 * 1024 * 1024), "5.0 MiB");
        assert_eq!(human_duration(83.45), "1:23.450");
        assert!(human_duration(3661.0).starts_with("1:01:01"));
    }

    #[test]
    fn ebml_vint_reads_ids_sizes_and_unknown_sizes() {
        // Segment id 0x18538067 keeps its marker bits.
        let mut i = 0;
        assert_eq!(
            ebml_vint(&[0x18, 0x53, 0x80, 0x67], &mut i, true).unwrap().0,
            0x1853_8067
        );
        assert_eq!(i, 4);
        // A one-byte size of 5.
        let mut i = 0;
        assert_eq!(ebml_vint(&[0x85], &mut i, false).unwrap(), (5, false));
        // All-ones = the "unknown size" a streamed Segment uses.
        let mut i = 0;
        assert!(ebml_vint(&[0xFF], &mut i, false).unwrap().1);
        let mut i = 0;
        assert!(ebml_vint(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF], &mut i, false)
            .unwrap()
            .1);
        // Invalid leading byte.
        let mut i = 0;
        assert!(ebml_vint(&[0x00, 0x01], &mut i, false).is_none());
    }

    #[test]
    fn mp4_box_walk_is_bounds_safe() {
        // A box claiming more bytes than the buffer holds must not panic.
        let mut b = Vec::new();
        b.extend_from_slice(&999u32.to_be_bytes());
        b.extend_from_slice(b"moov");
        assert!(mp4_boxes(&b).is_empty());
        assert!(mp4_tracks(&b).is_empty());
        assert!(mkv_tracks(&b).is_empty());
    }

    #[test]
    fn codec_names_map_both_containers() {
        assert_eq!(mp4_codec_name(b"avc1").as_deref(), Some("H.264 / AVC"));
        assert_eq!(mp4_codec_name(b"av01").as_deref(), Some("AV1"));
        assert_eq!(mp4_codec_name(b"mp4a").as_deref(), Some("AAC"));
        assert_eq!(mp4_codec_name(b"zzzz"), None);
        assert_eq!(mkv_codec_name("V_VP9"), "VP9");
        assert_eq!(mkv_codec_name("A_OPUS"), "Opus");
        assert_eq!(mkv_codec_name("V_SOMETHING"), "SOMETHING");
    }

    #[test]
    fn report_serializes_the_documented_fields() {
        let opts = Options {
            min_bitrate: 32.0,
            max_bitrate: 128.0,
            ..Default::default()
        };
        let r = analyze(wav(8000, 8000), &opts).unwrap();
        let v = serde_json::to_value(&r).unwrap();
        assert_eq!(v["status"], "PASS");
        assert_eq!(v["pass"], true);
        assert_eq!(v["reason"], "ok");
        assert_eq!(v["target"], "overall");
        assert_eq!(v["units"], "kbps");
        assert_eq!(v["checked_bitrate_kbps"], 64.4);
        assert_eq!(v["min_bitrate_kbps"], 32.0);
        assert_eq!(v["max_bitrate_kbps"], 128.0);
        assert_eq!(v["file_bytes"], 8044);
        assert_eq!(v["file_size_human"], "7.9 KiB");
        assert_eq!(v["stream_count"], 1);
        assert_eq!(v["streams"][0]["kind"], "audio");
        assert_eq!(v["streams"][0]["bitrate_kbps"], 64.0);
        assert!(v["summary"].as_str().unwrap().starts_with("PASS —"));
    }
}
