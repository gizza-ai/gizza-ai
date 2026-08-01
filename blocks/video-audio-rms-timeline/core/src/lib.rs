//! video-audio-rms-timeline core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Extracts a windowed audio-level time series from a video (or plain audio)
//! file: for each fixed-length analysis window the tool reports the window's
//! **RMS** level (average energy) and **peak** level (loudest sample), as a CSV
//! or JSON time series.
//!
//! Pipeline: decode the pasted file bytes (base64/hex) → symphonia demuxes the
//! container and decodes the FIRST decodable audio track (video tracks are
//! `CODEC_TYPE_NULL` and skipped) → downmix every frame to mono at the native
//! sample rate → slide a `window_ms` window every `hop_ms` and compute
//! RMS + peak per window → render CSV (`window,start_s,end_s,rms,peak`) or JSON.
//!
//! Levels are measured on the **mono downmix** of the audio track (per-channel
//! columns are intentionally out of scope — see the page copy). RMS/peak are
//! reported either in dBFS (0 dB = full scale; digital silence is floored to
//! −120 dB so the CSV stays numeric) or as a linear 0..1 amplitude fraction.
//!
//! Pure Rust → runs on every backend, including the chat Service Worker.

use std::io::Cursor;

use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Amplitude of digital silence in dBFS. RMS/peak of 0 is `-inf` dB, which is
/// not CSV-friendly, so it is floored to this value.
pub const SILENCE_FLOOR_DBFS: f64 = -120.0;

/// Hard cap on decoded mono samples (~5 min at 48 kHz). Longer audio is
/// truncated to keep memory/time bounded; the truncation is flagged in JSON.
pub const MAX_SAMPLES: usize = 15_000_000;

/// Hard cap on emitted windows — protects against a tiny window/hop producing a
/// multi-million-row CSV. Exceeding it is a clear error, not a silent trim.
pub const MAX_WINDOWS: usize = 500_000;

/// Smallest / largest window and hop, in milliseconds.
pub const MIN_MS: f64 = 1.0;
pub const MAX_MS: f64 = 60_000.0;

const SUPPORTED: &str = "supported containers: MP4/MOV/M4A, MKV/WebM, OGG, WAV, AIFF, CAF, \
                         FLAC, MP3, AAC-ADTS; audio codecs: AAC-LC, ALAC, MP3, Vorbis, FLAC, \
                         PCM, ADPCM (Opus, AC-3 and DTS are not supported)";

/// One decoded audio track: mono samples at the native rate + stream facts.
struct Decoded {
    mono: Vec<f32>,
    sample_rate: u32,
    channels: u16,
    truncated: bool,
}

/// Level unit for the RMS/peak columns.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Unit {
    Dbfs,
    Linear,
}

/// Output rendering.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OutMode {
    Csv,
    Json,
}

/// Analyze one file and render the windowed RMS/peak time series.
///
/// - `input`: the video/audio file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `window_ms`: analysis window length in ms (1–60000, default 100).
/// - `hop_ms`: step between window starts in ms (0–60000; 0 or blank →
///   non-overlapping, i.e. equal to `window_ms`). A hop smaller than the window
///   overlaps windows; larger than the window skips audio between them.
/// - `unit`: `"dbfs"` (default) or `"linear"` — the RMS/peak column unit.
/// - `output`: `"csv"` (default) or `"json"`.
///
/// Returns a user-facing error string for undecodable input, an unsupported
/// container/codec, a video with no audio track, or out-of-range parameters.
pub fn run(
    input: &str,
    input_format: &str,
    window_ms: f64,
    hop_ms: f64,
    unit: &str,
    output: &str,
) -> Result<String, String> {
    let unit = match unit.trim() {
        "" | "dbfs" => Unit::Dbfs,
        "linear" => Unit::Linear,
        other => {
            return Err(format!(
                "invalid unit {other:?}: expected \"dbfs\" or \"linear\""
            ))
        }
    };
    let out_mode = match output.trim() {
        "" | "csv" => OutMode::Csv,
        "json" => OutMode::Json,
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"csv\" or \"json\""
            ))
        }
    };
    if !window_ms.is_finite() || !(MIN_MS..=MAX_MS).contains(&window_ms) {
        return Err(format!(
            "invalid window_ms {window_ms}: expected {MIN_MS}-{MAX_MS}"
        ));
    }
    // 0 (or blank, which the wrapper maps to 0.0) means "non-overlapping".
    if !hop_ms.is_finite() || hop_ms < 0.0 || hop_ms > MAX_MS {
        return Err(format!("invalid hop_ms {hop_ms}: expected 0-{MAX_MS}"));
    }

    let bytes = decode_bytes(input, input_format)?;
    let dec = decode_audio(bytes)?;

    let rate = dec.sample_rate as f64;
    let win = ((window_ms / 1000.0 * rate).round() as usize).max(1);
    let hop = if hop_ms <= 0.0 {
        win
    } else {
        ((hop_ms / 1000.0 * rate).round() as usize).max(1)
    };
    let total = dec.mono.len();

    // Number of windows: every start position < total yields a window (the last
    // is truncated to whatever samples remain).
    let n_windows = if total == 0 { 0 } else { (total - 1) / hop + 1 };
    if n_windows > MAX_WINDOWS {
        return Err(format!(
            "this window/hop would produce {n_windows} rows (limit {MAX_WINDOWS}) — \
             use a larger window_ms/hop_ms or a shorter clip"
        ));
    }

    let mut rows: Vec<Row> = Vec::with_capacity(n_windows);
    let mut start = 0usize;
    let mut idx = 0usize;
    while start < total {
        let end = (start + win).min(total);
        let slice = &dec.mono[start..end];
        let (rms_lin, peak_lin) = window_levels(slice);
        rows.push(Row {
            index: idx,
            start_s: start as f64 / rate,
            end_s: end as f64 / rate,
            rms: level(rms_lin, unit),
            peak: level(peak_lin, unit),
        });
        idx += 1;
        start += hop;
    }

    Ok(match out_mode {
        OutMode::Csv => render_csv(&rows, unit),
        OutMode::Json => render_json(&rows, &dec, unit, window_ms, hop, rate),
    })
}

/// One analysis window's rendered row.
struct Row {
    index: usize,
    start_s: f64,
    end_s: f64,
    rms: f64,
    peak: f64,
}

/// Linear RMS and peak (both in 0..1 full-scale) of one window's samples.
fn window_levels(samples: &[f32]) -> (f64, f64) {
    if samples.is_empty() {
        return (0.0, 0.0);
    }
    let mut sum_sq = 0.0f64;
    let mut peak = 0.0f64;
    for &s in samples {
        let v = s as f64;
        sum_sq += v * v;
        let a = v.abs();
        if a > peak {
            peak = a;
        }
    }
    let rms = (sum_sq / samples.len() as f64).sqrt();
    (rms, peak)
}

/// Convert a linear 0..1 amplitude to the requested unit.
fn level(linear: f64, unit: Unit) -> f64 {
    match unit {
        Unit::Linear => round6(linear.min(1.0)),
        Unit::Dbfs => {
            if linear <= 0.0 {
                SILENCE_FLOOR_DBFS
            } else {
                round3((20.0 * linear.log10()).max(SILENCE_FLOOR_DBFS))
            }
        }
    }
}

fn round3(v: f64) -> f64 {
    (v * 1000.0).round() / 1000.0
}
fn round6(v: f64) -> f64 {
    (v * 1_000_000.0).round() / 1_000_000.0
}

fn cols(unit: Unit) -> (&'static str, &'static str) {
    match unit {
        Unit::Dbfs => ("rms_dbfs", "peak_dbfs"),
        Unit::Linear => ("rms", "peak"),
    }
}

/// Trim a trailing `.0` and redundant zeros so the CSV/JSON stays compact while
/// preserving the value (e.g. `-6.02`, `0`, `0.5`, `-120`).
fn num(v: f64) -> String {
    let mut s = format!("{v}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    s
}

fn render_csv(rows: &[Row], unit: Unit) -> String {
    let (rms_col, peak_col) = cols(unit);
    let mut out = String::with_capacity(rows.len() * 32 + 40);
    out.push_str(&format!("window,start_s,end_s,{rms_col},{peak_col}\n"));
    for r in rows {
        out.push_str(&format!(
            "{},{},{},{},{}\n",
            r.index,
            num(round3(r.start_s)),
            num(round3(r.end_s)),
            num(r.rms),
            num(r.peak),
        ));
    }
    out
}

fn render_json(
    rows: &[Row],
    dec: &Decoded,
    unit: Unit,
    window_ms: f64,
    hop_samples: usize,
    rate: f64,
) -> String {
    let (rms_col, peak_col) = cols(unit);
    let unit_str = match unit {
        Unit::Dbfs => "dbfs",
        Unit::Linear => "linear",
    };
    let mut out = String::with_capacity(rows.len() * 48 + 200);
    out.push_str("{\n");
    out.push_str(&format!("  \"sample_rate\": {},\n", dec.sample_rate));
    out.push_str(&format!("  \"channels\": {},\n", dec.channels));
    out.push_str(&format!(
        "  \"duration_s\": {},\n",
        num(round3(dec.mono.len() as f64 / rate))
    ));
    out.push_str(&format!("  \"unit\": \"{unit_str}\",\n"));
    out.push_str(&format!("  \"window_ms\": {},\n", num(window_ms)));
    out.push_str(&format!(
        "  \"hop_ms\": {},\n",
        num(round3(hop_samples as f64 / rate * 1000.0))
    ));
    out.push_str(&format!("  \"window_count\": {},\n", rows.len()));
    out.push_str(&format!("  \"truncated\": {},\n", dec.truncated));
    out.push_str("  \"windows\": [\n");
    for (i, r) in rows.iter().enumerate() {
        let comma = if i + 1 < rows.len() { "," } else { "" };
        out.push_str(&format!(
            "    {{\"window\": {}, \"start_s\": {}, \"end_s\": {}, \"{rms_col}\": {}, \"{peak_col}\": {}}}{comma}\n",
            r.index,
            num(round3(r.start_s)),
            num(round3(r.end_s)),
            num(r.rms),
            num(r.peak),
        ));
    }
    out.push_str("  ]\n}");
    out
}

// ---------------------------------------------------------------------------
// Decode (base64/hex → symphonia → mono native-rate samples)
// ---------------------------------------------------------------------------

/// Demux `bytes`, decode the first decodable audio track, downmix each frame to
/// mono at the native sample rate. Caps total samples at [`MAX_SAMPLES`].
fn decode_audio(bytes: Vec<u8>) -> Result<Decoded, String> {
    let mss = MediaSourceStream::new(Box::new(Cursor::new(bytes)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("unrecognized or unsupported media format ({SUPPORTED}): {e}"))?;
    let mut format = probed.format;

    // Pick the first track a decoder can be built for. Video tracks are
    // CODEC_TYPE_NULL; known-but-undecodable audio (e.g. Opus) fails decoder
    // construction and is reported honestly.
    let mut decoder = None;
    let mut track_id = 0;
    let mut seen: Vec<String> = Vec::new();
    for t in format.tracks() {
        if t.codec_params.codec == CODEC_TYPE_NULL {
            continue;
        }
        match symphonia::default::get_codecs().make(&t.codec_params, &DecoderOptions::default()) {
            Ok(d) => {
                decoder = Some(d);
                track_id = t.id;
                break;
            }
            Err(_) => seen.push(format!("{}", t.codec_params.codec)),
        }
    }
    let mut decoder = decoder.ok_or_else(|| {
        if seen.is_empty() {
            format!("no decodable audio track found (a silent/video-only file has no levels to measure); {SUPPORTED}")
        } else {
            format!("the audio track's codec is not supported ({}); {SUPPORTED}", seen.join(", "))
        }
    })?;

    let mut mono: Vec<f32> = Vec::new();
    let mut rate: u32 = 0;
    let mut channels: usize = 0;
    let mut truncated = false;
    let mut sbuf: Option<SampleBuffer<f32>> = None;

    'outer: loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(SymError::ResetRequired) => break,
            Err(e) => return Err(format!("error reading audio: {e}")),
        };
        if packet.track_id() != track_id {
            continue;
        }
        match decoder.decode(&packet) {
            Ok(decoded) => {
                if decoded.frames() == 0 {
                    continue;
                }
                let spec = *decoded.spec();
                let ch = spec.channels.count();
                if rate == 0 {
                    rate = spec.rate;
                    channels = ch;
                    if rate == 0 {
                        return Err("stream reports a zero sample rate".into());
                    }
                    if channels == 0 || channels > 8 {
                        return Err(format!(
                            "unsupported channel count {channels} (1–8 channels are supported)"
                        ));
                    }
                } else if spec.rate != rate || ch != channels {
                    return Err(
                        "variable sample rate / channel layout is not supported".into(),
                    );
                }
                let recreate = match &sbuf {
                    Some(b) => b.capacity() < decoded.capacity() * channels,
                    None => true,
                };
                if recreate {
                    sbuf = Some(SampleBuffer::<f32>::new(decoded.capacity() as u64, spec));
                }
                let buf = sbuf.as_mut().expect("sample buffer just created");
                buf.copy_interleaved_ref(decoded);
                for frame in buf.samples().chunks_exact(channels) {
                    if mono.len() >= MAX_SAMPLES {
                        truncated = true;
                        break 'outer;
                    }
                    let m = frame.iter().map(|v| *v as f64).sum::<f64>() / channels as f64;
                    mono.push(m as f32);
                }
            }
            Err(SymError::DecodeError(_)) => continue,
            Err(e) => return Err(format!("decode failed: {e}")),
        }
    }

    if rate == 0 || mono.is_empty() {
        return Err("no audio could be decoded".into());
    }
    Ok(Decoded {
        mono,
        sample_rate: rate,
        channels: channels as u16,
        truncated,
    })
}

// ---------------------------------------------------------------------------
// Byte input decode (base64 / hex)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the video/audio file bytes as base64 or hex".into());
    }
    match input_format.trim() {
        "" | "base64" => decode_base64(trimmed),
        "hex" => decode_hex(trimmed),
        other => Err(format!(
            "invalid input_format {other:?}: expected \"base64\" or \"hex\""
        )),
    }
}

/// Standard + URL-safe base64, padding optional.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    const INVALID: u8 = 255;
    let val = |c: u8| -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => INVALID,
        }
    };
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!("invalid base64 character {:?}", c as char));
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("invalid hex: odd number of digits".into());
    }
    let bytes = compact.as_bytes();
    let mut out = Vec::with_capacity(compact.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = hex_val(pair[0])?;
        let lo = hex_val(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit {:?}", c as char)),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PCM16 WAV from interleaved f32 samples in [-1, 1].
    fn make_wav(sample_rate: u32, channels: u16, samples: &[f32]) -> Vec<u8> {
        let bits = 16u16;
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data: Vec<u8> = Vec::new();
        for &s in samples {
            let s = s.clamp(-1.0, 1.0);
            data.extend_from_slice(&((s * 32767.0) as i16).to_le_bytes());
        }
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        out
    }

    fn b64(bytes: &[u8]) -> String {
        const TABLE: &[u8; 64] =
            b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
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
                TABLE[(n & 63) as usize] as char
            } else {
                '='
            });
        }
        out
    }

    /// A 1 s full-scale tone: RMS of a ±1.0 square is 1.0 (0 dBFS), peak 1.0.
    fn full_scale_square(rate: u32) -> Vec<f32> {
        (0..rate)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect()
    }

    #[test]
    fn csv_windows_full_scale_square_are_zero_dbfs() {
        // 8000 Hz, 1 s, 100 ms non-overlapping windows → 10 rows.
        let wav = make_wav(8000, 1, &full_scale_square(8000));
        let out = run(&b64(&wav), "base64", 100.0, 0.0, "dbfs", "csv").unwrap();
        let lines: Vec<&str> = out.trim_end().lines().collect();
        assert_eq!(lines[0], "window,start_s,end_s,rms_dbfs,peak_dbfs");
        assert_eq!(lines.len(), 11, "header + 10 windows");
        // First window: [0, 0.1) s, full-scale → ~0 dBFS RMS and peak.
        let first: Vec<&str> = lines[1].split(',').collect();
        assert_eq!(first[0], "0");
        assert_eq!(first[1], "0");
        assert_eq!(first[2], "0.1");
        // 16-bit full-scale square is within ~0.001 dB of 0.
        assert!(first[3].parse::<f64>().unwrap().abs() < 0.05, "rms ~0 dBFS");
        assert!(first[4].parse::<f64>().unwrap().abs() < 0.05, "peak ~0 dBFS");
    }

    #[test]
    fn silence_windows_hit_the_floor() {
        let wav = make_wav(8000, 1, &vec![0.0f32; 8000]);
        let out = run(&b64(&wav), "base64", 250.0, 0.0, "dbfs", "csv").unwrap();
        for line in out.trim_end().lines().skip(1) {
            let cols: Vec<&str> = line.split(',').collect();
            assert_eq!(cols[3], num(SILENCE_FLOOR_DBFS), "silent RMS floored");
            assert_eq!(cols[4], num(SILENCE_FLOOR_DBFS), "silent peak floored");
        }
    }

    #[test]
    fn linear_unit_reports_amplitude_fraction() {
        // Constant 0.5 amplitude → RMS 0.5, peak 0.5 in linear units.
        let wav = make_wav(8000, 1, &vec![0.5f32; 8000]);
        let out = run(&b64(&wav), "base64", 1000.0, 0.0, "linear", "csv").unwrap();
        let row: Vec<&str> = out.trim_end().lines().nth(1).unwrap().split(',').collect();
        // 0.5 stored as PCM16 quantizes to 16383/32768 ≈ 0.499969.
        assert!((row[3].parse::<f64>().unwrap() - 0.5).abs() < 1e-3, "rms ~0.5 linear");
        assert!((row[4].parse::<f64>().unwrap() - 0.5).abs() < 1e-3, "peak ~0.5 linear");
    }

    #[test]
    fn overlapping_hop_produces_more_windows() {
        // 1 s, 200 ms window, 100 ms hop → windows start at 0,0.1,...,0.9 = 10.
        let wav = make_wav(8000, 1, &full_scale_square(8000));
        let out = run(&b64(&wav), "base64", 200.0, 100.0, "dbfs", "csv").unwrap();
        let rows = out.trim_end().lines().count() - 1;
        assert_eq!(rows, 10, "hop < window overlaps");
    }

    #[test]
    fn json_output_carries_metadata() {
        let wav = make_wav(8000, 1, &full_scale_square(8000));
        let out = run(&b64(&wav), "base64", 500.0, 0.0, "linear", "json").unwrap();
        assert!(out.contains("\"sample_rate\": 8000"));
        assert!(out.contains("\"channels\": 1"));
        assert!(out.contains("\"unit\": \"linear\""));
        assert!(out.contains("\"window_count\": 2"));
        assert!(out.contains("\"windows\": ["));
        // Rough structural check: balanced braces.
        assert_eq!(out.matches('{').count(), out.matches('}').count());
    }

    #[test]
    fn stereo_downmixes_to_mono() {
        // Left = +1, Right = -1 → downmix is 0 → silence floor.
        let samples: Vec<f32> = (0..16000)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let wav = make_wav(8000, 2, &samples);
        let out = run(&b64(&wav), "base64", 1000.0, 0.0, "dbfs", "csv").unwrap();
        let row: Vec<&str> = out.trim_end().lines().nth(1).unwrap().split(',').collect();
        assert_eq!(row[3], num(SILENCE_FLOOR_DBFS), "opposite channels cancel");
    }

    #[test]
    fn hex_input_is_accepted() {
        let wav = make_wav(8000, 1, &vec![0.5f32; 8000]);
        let hex: String = wav.iter().map(|b| format!("{b:02x}")).collect();
        let out = run(&hex, "hex", 1000.0, 0.0, "linear", "csv").unwrap();
        let row: Vec<&str> = out.trim_end().lines().nth(1).unwrap().split(',').collect();
        assert!((row[4].parse::<f64>().unwrap() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn empty_input_errors() {
        let e = run("", "base64", 100.0, 0.0, "dbfs", "csv").unwrap_err();
        assert!(e.contains("empty"), "got: {e}");
    }

    #[test]
    fn bad_bytes_error_clearly() {
        let e = run(&b64(b"not a media file at all"), "base64", 100.0, 0.0, "dbfs", "csv")
            .unwrap_err();
        assert!(e.contains("unrecognized or unsupported media format"), "got: {e}");
    }

    #[test]
    fn out_of_range_window_errors() {
        let wav = make_wav(8000, 1, &vec![0.1f32; 8000]);
        let e = run(&b64(&wav), "base64", 0.0, 0.0, "dbfs", "csv").unwrap_err();
        assert!(e.contains("window_ms"), "got: {e}");
    }

    #[test]
    fn invalid_unit_errors() {
        let wav = make_wav(8000, 1, &vec![0.1f32; 8000]);
        let e = run(&b64(&wav), "base64", 100.0, 0.0, "bogus", "csv").unwrap_err();
        assert!(e.contains("unit"), "got: {e}");
    }

    #[test]
    fn tiny_window_and_hop_row_count() {
        // 1 s at 8000 Hz with a 1 ms window + 1 ms hop → 1000 rows (under cap).
        let wav = make_wav(8000, 1, &vec![0.1f32; 8000]);
        let out = run(&b64(&wav), "base64", 1.0, 1.0, "dbfs", "csv").unwrap();
        assert_eq!(out.trim_end().lines().count() - 1, 1000);
    }
}
