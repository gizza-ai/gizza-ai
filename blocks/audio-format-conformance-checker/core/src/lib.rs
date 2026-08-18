//! gizza-ai/audio-format-conformance-checker core — decide whether an audio
//! file's REAL container/codec matches the format it claims to be.
//!
//! Two independent signals are combined:
//!   1. a hand-rolled magic-byte sniff (`sniff`) that names the container from
//!      the leading bytes — including the codec carried inside an Ogg stream and
//!      the ISO-BMFF `ftyp` brand, and including common NON-audio formats so a
//!      `.mp3` that is really a PNG gets a clear verdict;
//!   2. a stream parse via the shared `media-info` core (pure-Rust symphonia
//!      demuxers) that reports the actual codec, sample rate, channels, bit
//!      depth, duration and bitrate.
//!
//! The claim comes from the `claimed_format` parameter, else from the filename
//! extension. The sniff is authoritative for the container; the stream parse is
//! enrichment — if it fails, the container verdict still stands and the parse
//! error is reported. `check` never panics and only errors on empty input.
//!
//! No I/O, no std clock → instantiates in the wafer chat runtime and the CLI.

use gizza_ai_media_info_core::info as media_info;
use serde::Serialize;

/// Container family — what "the same kind of file" means when deciding whether a
/// claimed extension is merely a relative (`.ogg` for Opus-in-Ogg) or plain wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// MPEG elementary audio (MP1/MP2/MP3, with or without an ID3 tag).
    Mpeg,
    /// RIFF/WAVE.
    Riff,
    /// AIFF / AIFF-C.
    Aiff,
    /// Native FLAC (`fLaC`), not FLAC-in-Ogg.
    Flac,
    /// Ogg — Vorbis, Opus, FLAC-in-Ogg, Speex, Theora.
    Ogg,
    /// ISO base media file format — MP4, M4A, M4B, MOV, 3GP.
    IsoBmff,
    /// Raw AAC in ADTS frames.
    Adts,
    /// Apple Core Audio Format.
    Caf,
    /// Matroska / WebM / MKA.
    Matroska,
    /// Microsoft ASF (WMA/WMV).
    Asf,
    /// 3GPP AMR narrow/wide band.
    Amr,
    /// Standard MIDI file (a score, not sampled audio).
    Midi,
    /// A recognised format with no sibling extensions (or a non-audio format).
    Standalone,
    /// Nothing recognised.
    Unknown,
}

impl Family {
    fn label(self) -> &'static str {
        match self {
            Family::Mpeg => "MPEG audio",
            Family::Riff => "RIFF",
            Family::Aiff => "AIFF",
            Family::Flac => "native FLAC",
            Family::Ogg => "Ogg",
            Family::IsoBmff => "ISO base media (MP4)",
            Family::Adts => "ADTS",
            Family::Caf => "Core Audio",
            Family::Matroska => "Matroska",
            Family::Asf => "ASF",
            Family::Amr => "AMR",
            Family::Midi => "MIDI",
            Family::Standalone => "standalone",
            Family::Unknown => "unknown",
        }
    }
}

/// What the magic bytes say the file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sniff {
    /// Canonical extension WITHOUT the dot, e.g. `wav`.
    pub ext: &'static str,
    /// Canonical media type, e.g. `audio/wav`.
    pub mime: &'static str,
    /// Human-readable format name, e.g. `WAV audio (RIFF)`.
    pub kind: &'static str,
    /// Coarse bucket: audio / video / image / document / archive / data / text / unknown.
    pub category: &'static str,
    /// Container family used for relative-extension checks.
    pub family: Family,
    /// Codec named by the signature itself, when the container reveals it
    /// (Ogg bitstream headers). `None` when only the stream parse can tell.
    pub codec_hint: Option<&'static str>,
}

const fn s(
    ext: &'static str,
    mime: &'static str,
    kind: &'static str,
    category: &'static str,
    family: Family,
) -> Sniff {
    Sniff { ext, mime, kind, category, family, codec_hint: None }
}

const fn audio(ext: &'static str, mime: &'static str, kind: &'static str, family: Family) -> Sniff {
    s(ext, mime, kind, "audio", family)
}

fn starts(b: &[u8], sig: &[u8]) -> bool {
    b.len() >= sig.len() && &b[..sig.len()] == sig
}

fn at(b: &[u8], off: usize, sig: &[u8]) -> bool {
    b.len() >= off + sig.len() && &b[off..off + sig.len()] == sig
}

fn contains(b: &[u8], needle: &[u8]) -> bool {
    b.len() >= needle.len() && b.windows(needle.len()).any(|w| w == needle)
}

/// Identify the codec inside an Ogg stream from its first bitstream page.
fn ogg_variant(b: &[u8]) -> Sniff {
    let head = &b[..b.len().min(256)];
    if contains(head, b"OpusHead") {
        return Sniff {
            codec_hint: Some("Opus"),
            ..audio("opus", "audio/opus", "Opus audio (Ogg)", Family::Ogg)
        };
    }
    if contains(head, b"\x01vorbis") {
        return Sniff {
            codec_hint: Some("Vorbis"),
            ..audio("ogg", "audio/ogg", "Vorbis audio (Ogg)", Family::Ogg)
        };
    }
    if contains(head, b"\x7fFLAC") {
        return Sniff {
            codec_hint: Some("FLAC"),
            ..audio("oga", "audio/ogg", "FLAC audio (Ogg)", Family::Ogg)
        };
    }
    if contains(head, b"Speex   ") {
        return Sniff {
            codec_hint: Some("Speex"),
            ..audio("spx", "audio/ogg", "Speex audio (Ogg)", Family::Ogg)
        };
    }
    if contains(head, b"\x80theora") {
        return Sniff {
            codec_hint: Some("Theora"),
            ..s("ogv", "video/ogg", "Theora video (Ogg)", "video", Family::Ogg)
        };
    }
    audio("ogg", "audio/ogg", "Ogg media (unknown bitstream)", Family::Ogg)
}

/// Map an ISO-BMFF `ftyp` major brand (the 4 bytes at offset 8) to a format.
fn ftyp_brand(brand: &[u8]) -> Sniff {
    match brand {
        b"M4A " => audio("m4a", "audio/mp4", "M4A audio (ISO BMFF)", Family::IsoBmff),
        b"M4B " => audio("m4b", "audio/mp4", "M4B audiobook (ISO BMFF)", Family::IsoBmff),
        b"M4P " => audio("m4p", "audio/mp4", "M4P protected audio (ISO BMFF)", Family::IsoBmff),
        b"F4A " | b"F4B " => audio("f4a", "audio/mp4", "F4A audio (ISO BMFF)", Family::IsoBmff),
        b"qt  " => s("mov", "video/quicktime", "QuickTime video", "video", Family::IsoBmff),
        b"M4V " | b"M4VP" => s("m4v", "video/x-m4v", "M4V video", "video", Family::IsoBmff),
        b"3gp4" | b"3gp5" | b"3gp6" | b"3g2a" => {
            s("3gp", "video/3gpp", "3GP media", "video", Family::IsoBmff)
        }
        b"avif" | b"avis" => s("avif", "image/avif", "AVIF image", "image", Family::IsoBmff),
        b"heic" | b"heix" | b"hevc" | b"mif1" => {
            s("heic", "image/heic", "HEIC/HEIF image", "image", Family::IsoBmff)
        }
        // isom / mp41 / mp42 / iso2 / dash / … → generic MP4. Whether it holds
        // audio only is decided by the stream parse, not the brand.
        _ => s("mp4", "video/mp4", "MP4 container (ISO BMFF)", "video", Family::IsoBmff),
    }
}

/// Identify the file behind `bytes` from its leading bytes alone. Returns the
/// unknown sniff for an unrecognised blob (never `None` — callers want a verdict).
pub fn sniff(b: &[u8]) -> Sniff {
    // ---- Audio containers ------------------------------------------------
    if starts(b, b"ID3") {
        return audio("mp3", "audio/mpeg", "MP3 audio (MPEG layer III, ID3-tagged)", Family::Mpeg);
    }
    if starts(b, &[0xFF, 0xF1]) || starts(b, &[0xFF, 0xF9]) {
        return Sniff {
            codec_hint: Some("AAC"),
            ..audio("aac", "audio/aac", "AAC audio (raw ADTS frames)", Family::Adts)
        };
    }
    // MPEG audio frame sync: 11 set bits, then a non-reserved version/layer.
    if b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0 && (b[1] & 0x06) != 0x00 {
        return audio("mp3", "audio/mpeg", "MP3 audio (MPEG layer II/III frames)", Family::Mpeg);
    }
    if starts(b, b"fLaC") {
        return Sniff {
            codec_hint: Some("FLAC"),
            ..audio("flac", "audio/flac", "FLAC audio (native container)", Family::Flac)
        };
    }
    if starts(b, b"OggS") {
        return ogg_variant(b);
    }
    if starts(b, b"RIFF") && b.len() >= 12 {
        return match &b[8..12] {
            b"WAVE" => audio("wav", "audio/wav", "WAV audio (RIFF)", Family::Riff),
            b"WEBP" => s("webp", "image/webp", "WebP image", "image", Family::Standalone),
            b"AVI " => s("avi", "video/x-msvideo", "AVI video (RIFF)", "video", Family::Riff),
            _ => s("riff", "application/octet-stream", "RIFF container (not WAVE)", "data", Family::Riff),
        };
    }
    if (starts(b, b"RF64") || starts(b, b"BW64")) && at(b, 12, b"WAVE") {
        return audio("wav", "audio/wav", "RF64/BW64 WAV audio (>4 GB RIFF)", Family::Riff);
    }
    if starts(b, b"FORM") && b.len() >= 12 {
        return match &b[8..12] {
            b"AIFF" => audio("aiff", "audio/aiff", "AIFF audio (uncompressed)", Family::Aiff),
            b"AIFC" => audio("aifc", "audio/aiff", "AIFF-C audio (compressed)", Family::Aiff),
            b"DSD " => audio("dff", "audio/x-dff", "DSDIFF audio", Family::Standalone),
            _ => s("iff", "application/octet-stream", "IFF container (not AIFF)", "data", Family::Aiff),
        };
    }
    if starts(b, b"caff") {
        return audio("caf", "audio/x-caf", "Core Audio Format (CAF)", Family::Caf);
    }
    if at(b, 4, b"ftyp") && b.len() >= 12 {
        return ftyp_brand(&b[8..12]);
    }
    if starts(b, &[0x1A, 0x45, 0xDF, 0xA3]) {
        let head = &b[..b.len().min(96)];
        return if contains(head, b"webm") {
            s("webm", "video/webm", "WebM media (Matroska)", "video", Family::Matroska)
        } else {
            s("mkv", "video/x-matroska", "Matroska media", "video", Family::Matroska)
        };
    }
    if starts(b, &[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11]) {
        return audio("wma", "audio/x-ms-wma", "ASF container (WMA/WMV)", Family::Asf);
    }
    if starts(b, b"#!AMR") {
        return audio("amr", "audio/amr", "AMR narrow/wide band audio", Family::Amr);
    }
    if starts(b, b"MThd") {
        return audio("mid", "audio/midi", "Standard MIDI file (score, not sampled audio)", Family::Midi);
    }
    if starts(b, b"MAC ") {
        return audio("ape", "audio/x-ape", "Monkey's Audio (APE)", Family::Standalone);
    }
    if starts(b, b"wvpk") {
        return audio("wv", "audio/x-wavpack", "WavPack audio", Family::Standalone);
    }
    if starts(b, b"TTA1") {
        return audio("tta", "audio/x-tta", "True Audio (TTA)", Family::Standalone);
    }
    if starts(b, b"MPCK") || starts(b, b"MP+") {
        return audio("mpc", "audio/x-musepack", "Musepack audio", Family::Standalone);
    }
    if starts(b, b".snd") {
        return audio("au", "audio/basic", "Sun/NeXT AU audio", Family::Standalone);
    }
    if starts(b, b".ra\xfd") {
        return audio("ra", "audio/x-pn-realaudio", "RealAudio", Family::Standalone);
    }
    if starts(b, b"DSD ") {
        return audio("dsf", "audio/x-dsf", "DSF audio (DSD)", Family::Standalone);
    }

    // ---- Common NON-audio formats, so a mislabelled file is named ---------
    if starts(b, &[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return s("png", "image/png", "PNG image", "image", Family::Standalone);
    }
    if starts(b, &[0xFF, 0xD8, 0xFF]) {
        return s("jpg", "image/jpeg", "JPEG image", "image", Family::Standalone);
    }
    if starts(b, b"GIF87a") || starts(b, b"GIF89a") {
        return s("gif", "image/gif", "GIF image", "image", Family::Standalone);
    }
    if starts(b, b"BM") {
        return s("bmp", "image/bmp", "BMP image", "image", Family::Standalone);
    }
    if starts(b, b"%PDF-") {
        return s("pdf", "application/pdf", "PDF document", "document", Family::Standalone);
    }
    if starts(b, b"PK\x03\x04") || starts(b, b"PK\x05\x06") {
        return s("zip", "application/zip", "ZIP archive (or ZIP-based document)", "archive", Family::Standalone);
    }
    if starts(b, &[0x1F, 0x8B]) {
        return s("gz", "application/gzip", "gzip archive", "archive", Family::Standalone);
    }
    if starts(b, b"7z\xbc\xaf\x27\x1c") {
        return s("7z", "application/x-7z-compressed", "7-Zip archive", "archive", Family::Standalone);
    }
    if starts(b, b"Rar!\x1a\x07") {
        return s("rar", "application/vnd.rar", "RAR archive", "archive", Family::Standalone);
    }
    if starts(b, &[0x7F, b'E', b'L', b'F']) {
        return s("elf", "application/x-elf", "ELF executable", "executable", Family::Standalone);
    }
    if starts(b, b"MZ") {
        return s("exe", "application/vnd.microsoft.portable-executable", "Windows executable", "executable", Family::Standalone);
    }
    if starts(b, b"<?xml") || starts(b, b"<!DOCTYPE") || starts(b, b"<html") {
        return s("xml", "text/xml", "XML/HTML text (often an error page served instead of the file)", "text", Family::Standalone);
    }
    // A byte blob that is entirely printable text is worth naming: a fetch that
    // returned an error page or a base64 body instead of audio.
    let head = &b[..b.len().min(512)];
    if !head.is_empty()
        && head
            .iter()
            .all(|&c| c == b'\n' || c == b'\r' || c == b'\t' || (0x20..0x7F).contains(&c))
    {
        return s("txt", "text/plain", "Plain text (not an audio file)", "text", Family::Standalone);
    }

    s("", "application/octet-stream", "Unrecognised binary data", "unknown", Family::Unknown)
}

/// Normalise a claimed extension/format word to `(canonical_ext, family)`.
/// Returns `None` for anything not a format this tool knows.
fn normalise_claim(raw: &str) -> Option<(&'static str, Family)> {
    let c = raw.trim().trim_start_matches('.').to_ascii_lowercase();
    Some(match c.as_str() {
        "mp3" | "mpga" | "mp2" | "mp1" | "mpeg3" => ("mp3", Family::Mpeg),
        "wav" | "wave" => ("wav", Family::Riff),
        "aiff" | "aif" => ("aiff", Family::Aiff),
        "aifc" => ("aifc", Family::Aiff),
        "flac" => ("flac", Family::Flac),
        "ogg" => ("ogg", Family::Ogg),
        "oga" => ("oga", Family::Ogg),
        "opus" => ("opus", Family::Ogg),
        "spx" => ("spx", Family::Ogg),
        "m4a" => ("m4a", Family::IsoBmff),
        "m4b" => ("m4b", Family::IsoBmff),
        "mp4" | "m4v" | "mov" | "3gp" => ("mp4", Family::IsoBmff),
        "aac" | "adts" => ("aac", Family::Adts),
        "caf" => ("caf", Family::Caf),
        "mka" | "mkv" | "webm" => ("mka", Family::Matroska),
        "wma" | "asf" => ("wma", Family::Asf),
        "amr" => ("amr", Family::Amr),
        "mid" | "midi" => ("mid", Family::Midi),
        "ape" => ("ape", Family::Standalone),
        "wv" => ("wv", Family::Standalone),
        "tta" => ("tta", Family::Standalone),
        "mpc" => ("mpc", Family::Standalone),
        "au" => ("au", Family::Standalone),
        "dsf" => ("dsf", Family::Standalone),
        _ => return None,
    })
}

/// Lowercase extension of `filename`, without the dot. `None` when absent.
fn filename_ext(filename: &str) -> Option<String> {
    let base = filename.rsplit(['/', '\\']).next().unwrap_or(filename);
    let (stem, ext) = base.rsplit_once('.')?;
    if stem.is_empty() || ext.is_empty() || ext.len() > 8 {
        return None;
    }
    Some(ext.to_ascii_lowercase())
}

/// Normalise a codec name (ours or symphonia's friendly name) for comparison.
fn codec_key(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

/// One track's stream-parsed audio properties.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StreamFacts {
    /// Codec reported by the demuxer, e.g. `AAC`, `PCM`, `Vorbis`.
    pub codec: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel_layout: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits_per_sample: Option<u32>,
}

/// The conformance report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Report {
    /// One-line, human-readable verdict.
    pub summary: String,
    /// `conformant` · `container_mismatch` · `codec_mismatch` · `codec_unverified`
    /// · `no_audio_track` · `not_audio` · `unrecognised` · `unknown_claim` · `no_claim`.
    pub verdict: String,
    /// True only when everything checked agreed. Omitted when there was no claim
    /// to check against (`claimed_format=auto` and no filename).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conformant: Option<bool>,

    /// Canonical extension the bytes really are, e.g. `wav`. Empty when unrecognised.
    pub detected_format: String,
    /// Human-readable name of the detected format.
    pub detected_kind: String,
    /// Media type of the detected format.
    pub detected_mime: String,
    /// Coarse bucket of the detected format: audio / video / image / document /
    /// archive / text / executable / data / unknown.
    pub detected_category: String,
    /// Container family label used for relative-extension checks, e.g. `Ogg`.
    pub detected_family: String,
    /// True when the detected format is an audio format.
    pub is_audio: bool,
    /// First bytes of the file as uppercase hex — the signature the verdict rests on.
    pub signature_hex: String,

    /// Normalised format the file claims to be. Omitted when nothing was claimed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claimed_format: Option<String>,
    /// Where the claim came from: `parameter` or `filename`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub claim_source: Option<String>,
    /// Whether the real container matches the claim (relatives count as a match
    /// unless `strict`). Omitted when nothing was claimed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_matches_claim: Option<bool>,

    /// Codec found inside the container. Omitted when the stream parse failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec: Option<String>,
    /// The codec the caller expected, when `expected_codec` was given.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_codec: Option<String>,
    /// Whether `codec` matches `expected_codec`. Omitted when nothing was expected
    /// or the codec could not be read.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codec_matches_expectation: Option<bool>,

    /// Container name as the demuxer sees it, e.g. `WAVE (RIFF)`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parsed_container: Option<String>,
    /// Number of audio tracks found by the demuxer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audio_track_count: Option<usize>,
    /// Per-audio-track stream facts (first track first).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<StreamFacts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_rate: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits_per_sample: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bitrate_kbps: Option<u32>,
    /// Why the stream parse produced nothing, when it failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parse_error: Option<String>,

    /// Extension the file should carry, when it is mislabelled. Omitted when conformant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_extension: Option<String>,
    /// The filename with the corrected extension, when a filename was available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggested_filename: Option<String>,

    /// Size of the inspected file in bytes.
    pub bytes: usize,
    /// Every notable finding, in the order discovered.
    pub issues: Vec<String>,
}

fn hex_signature(b: &[u8], n: usize) -> String {
    b.iter()
        .take(n)
        .map(|byte| format!("{byte:02X}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Check whether `bytes` conform to the format they claim to be.
///
/// * `claimed_format` — the format the file says it is; `None` or `"auto"` falls
///   back to `filename`'s extension.
/// * `expected_codec` — codec the caller expects inside the container; `None` or
///   `"any"` skips the codec check.
/// * `strict` — when true, a claimed extension that is only a RELATIVE of the
///   real one (`.ogg` for Opus-in-Ogg, `.mp4` for an M4A brand) counts as a
///   mismatch too.
/// * `filename` — the source filename, used for the fallback claim and the
///   rename suggestion.
///
/// Errors only on empty input; every other input yields a verdict.
pub fn check(
    bytes: &[u8],
    claimed_format: Option<&str>,
    expected_codec: Option<&str>,
    strict: bool,
    filename: Option<&str>,
) -> Result<Report, String> {
    if bytes.is_empty() {
        return Err("input is empty — provide an audio file to check (0 bytes read)".into());
    }

    let sniffed = sniff(bytes);
    let mut issues: Vec<String> = Vec::new();

    // ---- the claim -------------------------------------------------------
    let param_claim = claimed_format
        .map(str::trim)
        .filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("auto"));
    let file_ext = filename.and_then(filename_ext);
    let (raw_claim, claim_source) = match (param_claim, file_ext.as_deref()) {
        (Some(p), _) => (Some(p.to_string()), Some("parameter")),
        (None, Some(f)) => (Some(f.to_string()), Some("filename")),
        (None, None) => (None, None),
    };
    let claim = raw_claim.as_deref().and_then(normalise_claim);
    if let (Some(raw), None) = (raw_claim.as_deref(), claim) {
        issues.push(format!(
            "claimed format '{raw}' is not an audio format this checker knows, so the container \
             could not be compared"
        ));
    }

    // ---- the stream parse (enrichment; failure is reported, never fatal) --
    let parsed = media_info(bytes);
    let mut parse_error = None;
    let mut parsed_container = None;
    let mut audio_track_count = None;
    let mut tracks: Vec<StreamFacts> = Vec::new();
    let mut duration_seconds = None;
    let mut duration = None;
    let mut bitrate_kbps = None;
    match &parsed {
        Ok(info) => {
            parsed_container = Some(info.container.clone());
            duration_seconds = info.duration_seconds;
            duration = info.duration_human.clone();
            bitrate_kbps = info.overall_bitrate_kbps;
            for t in info.tracks.iter().filter(|t| t.kind == "audio") {
                tracks.push(StreamFacts {
                    codec: t.codec.clone(),
                    sample_rate: t.sample_rate,
                    channels: t.channels,
                    channel_layout: t.channel_layout.clone(),
                    bits_per_sample: t.bits_per_sample,
                });
            }
            audio_track_count = Some(tracks.len());
            if tracks.is_empty() {
                issues.push(format!(
                    "the container parsed as {} but holds no audio track",
                    info.container
                ));
            }
        }
        Err(e) => {
            parse_error = Some(e.clone());
            issues.push(format!("stream parse could not read the codec: {e}"));
        }
    }

    // Codec: the demuxer wins; fall back to what the signature revealed.
    let codec = tracks
        .first()
        .map(|t| t.codec.clone())
        .filter(|c| !c.starts_with("unsupported") && !c.starts_with("codec "))
        .or_else(|| sniffed.codec_hint.map(str::to_string));

    // ---- container conformance -------------------------------------------
    let mut container_matches_claim = None;
    let mut relative_only = false;
    if let Some((claim_ext, claim_family)) = claim {
        let exact = claim_ext == sniffed.ext;
        let same_family = claim_family == sniffed.family && sniffed.family != Family::Unknown;
        relative_only = !exact && same_family;
        let ok = if strict { exact } else { exact || same_family };
        container_matches_claim = Some(ok);
        if relative_only {
            issues.push(format!(
                "claimed .{claim_ext} and detected .{} are both {} containers — {}",
                sniffed.ext,
                sniffed.family.label(),
                if strict {
                    "rejected because strict is on"
                } else {
                    "accepted as relatives (set strict to reject)"
                }
            ));
        }
        if !exact && !same_family {
            issues.push(format!(
                "extension/content mismatch: the file claims .{claim_ext} but the bytes are {}",
                sniffed.kind
            ));
        }
    }

    // ---- codec expectation -----------------------------------------------
    let expected = expected_codec
        .map(str::trim)
        .filter(|c| !c.is_empty() && !c.eq_ignore_ascii_case("any"))
        .map(|c| c.to_ascii_lowercase());
    let mut codec_matches_expectation = None;
    if let Some(exp) = expected.as_deref() {
        match codec.as_deref() {
            Some(found) => {
                let ok = codec_key(found) == exp;
                codec_matches_expectation = Some(ok);
                if !ok {
                    issues.push(format!(
                        "codec mismatch: expected {exp} inside the container but found {found}"
                    ));
                }
            }
            None => issues.push(format!(
                "expected codec {exp} could not be verified — the container's codec is unreadable"
            )),
        }
    }

    // ---- verdict ---------------------------------------------------------
    let no_audio_track = audio_track_count == Some(0);
    let verdict = if claim.is_none() {
        if raw_claim.is_some() {
            "unknown_claim"
        } else {
            "no_claim"
        }
    } else if sniffed.family == Family::Unknown {
        "unrecognised"
    } else if container_matches_claim == Some(false) {
        if sniffed.category == "audio" {
            "container_mismatch"
        } else {
            "not_audio"
        }
    } else if no_audio_track {
        "no_audio_track"
    } else if codec_matches_expectation == Some(false) {
        "codec_mismatch"
    } else if expected.is_some() && codec_matches_expectation.is_none() {
        "codec_unverified"
    } else {
        "conformant"
    };
    let conformant = claim.map(|_| verdict == "conformant");

    // ---- rename suggestion ----------------------------------------------
    let mislabelled = matches!(verdict, "container_mismatch" | "not_audio")
        || (strict && relative_only && container_matches_claim == Some(false));
    let (suggested_extension, suggested_filename) = if mislabelled && !sniffed.ext.is_empty() {
        let sug_name = filename.and_then(|f| {
            let base = f.rsplit(['/', '\\']).next().unwrap_or(f);
            base.rsplit_once('.')
                .map(|(stem, _)| format!("{stem}.{}", sniffed.ext))
        });
        (Some(sniffed.ext.to_string()), sug_name)
    } else {
        (None, None)
    };

    // ---- summary ---------------------------------------------------------
    let claim_txt = claim.map(|(e, _)| e).unwrap_or("");
    let summary = match verdict {
        "conformant" => format!(
            "Conformant: the bytes are {} and the file claims .{claim_txt}.",
            sniffed.kind
        ),
        "container_mismatch" => format!(
            "Mislabelled: the file claims .{claim_txt} but the bytes are {} — rename it to .{}.",
            sniffed.kind, sniffed.ext
        ),
        "not_audio" => format!(
            "Not audio: the file claims .{claim_txt} but the bytes are {} ({}).",
            sniffed.kind, sniffed.category
        ),
        "codec_mismatch" => format!(
            "Container matches (.{claim_txt}) but the codec inside is {}, not {}.",
            codec.as_deref().unwrap_or("unknown"),
            expected.as_deref().unwrap_or("")
        ),
        "codec_unverified" => format!(
            "Container matches (.{claim_txt}), but the codec could not be read, so the {} \
             expectation is unverified.",
            expected.as_deref().unwrap_or("")
        ),
        "no_audio_track" => format!(
            "The container matches .{claim_txt} but carries no audio track."
        ),
        "unrecognised" => format!(
            "Unrecognised: no known audio or file signature matched, so the .{claim_txt} claim \
             could not be confirmed."
        ),
        "unknown_claim" => format!(
            "The bytes are {}, but the claimed format is not one this checker knows.",
            sniffed.kind
        ),
        _ => format!(
            "No claim to check: the bytes are {} (pass claimed_format or a named file to compare).",
            sniffed.kind
        ),
    };

    Ok(Report {
        summary,
        verdict: verdict.to_string(),
        conformant,
        detected_format: sniffed.ext.to_string(),
        detected_kind: sniffed.kind.to_string(),
        detected_mime: sniffed.mime.to_string(),
        detected_category: sniffed.category.to_string(),
        detected_family: sniffed.family.label().to_string(),
        is_audio: sniffed.category == "audio",
        signature_hex: hex_signature(bytes, 12),
        claimed_format: claim.map(|(e, _)| e.to_string()),
        claim_source: claim_source
            .filter(|_| raw_claim.is_some())
            .map(str::to_string),
        container_matches_claim,
        codec,
        expected_codec: expected.clone(),
        codec_matches_expectation,
        parsed_container,
        audio_track_count,
        sample_rate: tracks.first().and_then(|t| t.sample_rate),
        channels: tracks.first().and_then(|t| t.channels),
        bits_per_sample: tracks.first().and_then(|t| t.bits_per_sample),
        tracks,
        duration_seconds,
        duration,
        bitrate_kbps,
        parse_error,
        suggested_extension,
        suggested_filename,
        bytes: bytes.len(),
        issues,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real, parseable 16-bit stereo PCM WAV: 44-byte RIFF header + `frames`
    /// silent frames at 44.1 kHz.
    fn wav(frames: u32) -> Vec<u8> {
        let data_len = frames * 4; // 2 channels × 2 bytes
        let mut v = Vec::with_capacity(44 + data_len as usize);
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_len).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&2u16.to_le_bytes()); // channels
        v.extend_from_slice(&44100u32.to_le_bytes());
        v.extend_from_slice(&(44100u32 * 4).to_le_bytes()); // byte rate
        v.extend_from_slice(&4u16.to_le_bytes()); // block align
        v.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_len.to_le_bytes());
        v.resize(44 + data_len as usize, 0);
        v
    }

    #[test]
    fn real_wav_claiming_wav_is_conformant() {
        let r = check(&wav(44100), Some("wav"), None, false, Some("piano.wav")).unwrap();
        assert_eq!(r.verdict, "conformant");
        assert_eq!(r.conformant, Some(true));
        assert_eq!(r.detected_format, "wav");
        assert_eq!(r.detected_mime, "audio/wav");
        assert!(r.is_audio);
        assert_eq!(r.claim_source.as_deref(), Some("parameter"));
        assert_eq!(r.codec.as_deref(), Some("PCM"));
        assert_eq!(r.sample_rate, Some(44100));
        assert_eq!(r.channels, Some(2));
        assert_eq!(r.bits_per_sample, Some(16));
        assert_eq!(r.audio_track_count, Some(1));
        assert_eq!(r.duration_seconds, Some(1.0));
        assert_eq!(r.signature_hex.split(' ').next().unwrap(), "52");
        assert!(r.suggested_extension.is_none());
        assert!(r.issues.is_empty(), "unexpected issues: {:?}", r.issues);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = check(&[], Some("mp3"), None, false, None).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn wav_renamed_to_mp3_is_flagged_with_a_rename_hint() {
        let r = check(&wav(1000), None, None, false, Some("track.mp3")).unwrap();
        assert_eq!(r.verdict, "container_mismatch");
        assert_eq!(r.conformant, Some(false));
        assert_eq!(r.claimed_format.as_deref(), Some("mp3"));
        assert_eq!(r.claim_source.as_deref(), Some("filename"));
        assert_eq!(r.container_matches_claim, Some(false));
        assert_eq!(r.suggested_extension.as_deref(), Some("wav"));
        assert_eq!(r.suggested_filename.as_deref(), Some("track.wav"));
        assert!(r.summary.contains("Mislabelled"), "{}", r.summary);
    }

    #[test]
    fn parameter_claim_overrides_the_filename() {
        let r = check(&wav(1000), Some("flac"), None, false, Some("ok.wav")).unwrap();
        assert_eq!(r.verdict, "container_mismatch");
        assert_eq!(r.claim_source.as_deref(), Some("parameter"));
        assert_eq!(r.claimed_format.as_deref(), Some("flac"));
    }

    #[test]
    fn png_named_as_audio_is_not_audio() {
        let png = [
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R',
        ];
        let r = check(&png, Some("mp3"), None, false, Some("song.mp3")).unwrap();
        assert_eq!(r.verdict, "not_audio");
        assert_eq!(r.detected_format, "png");
        assert_eq!(r.detected_category, "image");
        assert!(!r.is_audio);
        assert_eq!(r.suggested_filename.as_deref(), Some("song.png"));
    }

    #[test]
    fn ogg_bitstream_codec_comes_from_the_signature() {
        let mut opus = b"OggS\x00\x02".to_vec();
        opus.resize(28, 0);
        opus.extend_from_slice(b"OpusHead\x01\x02\x38\x01\x80\xbb\x00\x00");
        let r = check(&opus, Some("opus"), None, false, None).unwrap();
        assert_eq!(r.detected_format, "opus");
        assert_eq!(r.codec.as_deref(), Some("Opus"));
        // Relatives: the same bytes claimed as .ogg pass by default…
        let lenient = check(&opus, Some("ogg"), None, false, None).unwrap();
        assert_eq!(lenient.verdict, "conformant");
        assert_eq!(lenient.container_matches_claim, Some(true));
        // …and are rejected under strict.
        let strict = check(&opus, Some("ogg"), None, true, None).unwrap();
        assert_eq!(strict.verdict, "container_mismatch");
        assert_eq!(strict.suggested_extension.as_deref(), Some("opus"));
    }

    #[test]
    fn expected_codec_is_checked_inside_the_container() {
        let good = check(&wav(1000), Some("wav"), Some("pcm"), false, None).unwrap();
        assert_eq!(good.verdict, "conformant");
        assert_eq!(good.codec_matches_expectation, Some(true));

        let bad = check(&wav(1000), Some("wav"), Some("aac"), false, None).unwrap();
        assert_eq!(bad.verdict, "codec_mismatch");
        assert_eq!(bad.codec_matches_expectation, Some(false));
        assert!(bad.issues.iter().any(|i| i.contains("codec mismatch")));
        // A container mismatch is not masked by an ok codec expectation.
        assert!(bad.container_matches_claim.unwrap());
    }

    #[test]
    fn adts_aac_is_not_an_m4a() {
        let aac = [0xFF, 0xF1, 0x50, 0x80, 0x00, 0x1F, 0xFC, 0x21, 0x00, 0x00, 0x00, 0x00];
        let r = check(&aac, Some("m4a"), None, false, Some("clip.m4a")).unwrap();
        assert_eq!(r.detected_format, "aac");
        assert_eq!(r.verdict, "container_mismatch");
        assert_eq!(r.suggested_extension.as_deref(), Some("aac"));
        assert_eq!(r.codec.as_deref(), Some("AAC"));
    }

    #[test]
    fn no_claim_still_reports_the_detection() {
        let r = check(&wav(1000), None, None, false, None).unwrap();
        assert_eq!(r.verdict, "no_claim");
        assert!(r.conformant.is_none());
        assert!(r.claimed_format.is_none());
        assert_eq!(r.detected_format, "wav");
    }

    #[test]
    fn unparseable_audio_container_still_yields_a_container_verdict() {
        // A valid FLAC signature followed by junk: the sniff succeeds, the
        // demuxer does not — the report must say so instead of failing.
        let mut flac = b"fLaC".to_vec();
        flac.extend_from_slice(&[0xAA; 64]);
        let r = check(&flac, Some("flac"), None, false, None).unwrap();
        assert_eq!(r.detected_format, "flac");
        assert_eq!(r.codec.as_deref(), Some("FLAC")); // from the signature
        assert!(r.parse_error.is_some());
        assert!(r.issues.iter().any(|i| i.contains("stream parse")));
        assert_eq!(r.verdict, "conformant");
    }

    #[test]
    fn unknown_bytes_are_reported_as_unrecognised() {
        let junk = [0x00u8, 0x01, 0x02, 0xFE, 0xEE, 0x99, 0x00, 0x13];
        let r = check(&junk, Some("wav"), None, false, None).unwrap();
        assert_eq!(r.verdict, "unrecognised");
        assert_eq!(r.detected_category, "unknown");
        assert_eq!(r.conformant, Some(false));
    }

    #[test]
    fn an_unknown_claim_word_is_reported_not_guessed() {
        let r = check(&wav(1000), Some("banana"), None, false, None).unwrap();
        assert_eq!(r.verdict, "unknown_claim");
        assert!(r.claimed_format.is_none());
        assert!(r.issues.iter().any(|i| i.contains("banana")));
    }

    #[test]
    fn html_error_page_served_instead_of_audio_is_named() {
        let page = b"<!DOCTYPE html><html><body>404 not found</body></html>";
        let r = check(page, Some("mp3"), None, false, Some("song.mp3")).unwrap();
        assert_eq!(r.verdict, "not_audio");
        assert_eq!(r.detected_category, "text");
    }

    #[test]
    fn signature_hex_shows_the_leading_bytes() {
        let r = check(&wav(100), Some("wav"), None, false, None).unwrap();
        assert_eq!(r.signature_hex.split(' ').count(), 12);
        assert!(r.signature_hex.starts_with("52 49 46 46"), "{}", r.signature_hex);
    }

    #[test]
    fn filename_ext_handles_paths_and_no_extension() {
        assert_eq!(filename_ext("a/b/c.FLAC").as_deref(), Some("flac"));
        assert_eq!(filename_ext("noext"), None);
        assert_eq!(filename_ext(".hidden"), None);
    }

    #[test]
    fn sniff_covers_the_advertised_containers() {
        assert_eq!(sniff(b"ID3\x04\x00\x00\x00\x00\x00\x00\x00\x00").ext, "mp3");
        assert_eq!(sniff(&[0xFF, 0xFB, 0x90, 0x00]).ext, "mp3");
        assert_eq!(sniff(b"fLaC\x00\x00\x00\x22").ext, "flac");
        assert_eq!(sniff(b"caff\x00\x01\x00\x00").ext, "caf");
        assert_eq!(sniff(b"#!AMR\n").ext, "amr");
        assert_eq!(sniff(b"MThd\x00\x00\x00\x06").ext, "mid");
        assert_eq!(sniff(b"FORM\x00\x00\x10\x00AIFF").ext, "aiff");
        assert_eq!(sniff(b"FORM\x00\x00\x10\x00AIFC").ext, "aifc");
        assert_eq!(sniff(&[0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11]).ext, "wma");
        assert_eq!(sniff(b"\x00\x00\x00\x20ftypM4A \x00\x00\x00\x00").ext, "m4a");
        assert_eq!(sniff(b"\x00\x00\x00\x20ftypisom\x00\x00\x00\x00").ext, "mp4");
        assert_eq!(sniff(b"RIFF\x24\x08\x00\x00WAVEfmt ").ext, "wav");
        assert_eq!(sniff(b"RIFF\x24\x08\x00\x00WEBPVP8 ").category, "image");
        assert_eq!(sniff(b"%PDF-1.7\n").category, "document");
    }
}
