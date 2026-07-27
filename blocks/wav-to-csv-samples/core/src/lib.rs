//! wav-to-csv-samples core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Decodes an uncompressed PCM/IEEE-float WAV (given as base64 or hex bytes) and
//! exports its decoded samples to CSV: one column per audio channel, optionally
//! preceded by a time and/or sample-index column. Sample values can be written as
//! normalized floats in [-1, 1], as raw PCM integers at the source bit depth, or
//! as dBFS magnitudes.
//!
//! Only uncompressed WAV is decoded (PCM 8/16/24/32-bit integer and 32/64-bit
//! IEEE float). Compressed containers/codecs (MP3, AAC/M4A, Opus/Ogg, FLAC,
//! A-law/mu-law) are rejected with a clear message rather than guessed at.

/// Export the decoded samples of a WAV clip to CSV.
///
/// - `input`: the WAV file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `value_scale`: `"float"` (normalized [-1,1], default), `"int"` (raw PCM
///   integer at the source bit depth; float sources scale to 32-bit), or `"db"`
///   (dBFS magnitude).
/// - `precision`: decimal places for `float`/`db` values (0-15, default 6).
/// - `index_column`: `"time"` (seconds, default), `"sample"` (absolute frame
///   index), `"both"`, or `"none"`.
/// - `delimiter`: `"comma"` (default), `"semicolon"`, or `"tab"`.
/// - `header`: include the column-name header row (default true).
/// - `start_frame`: first sample frame to export (default 0).
/// - `max_frames`: maximum number of sample frames to export (1-500000,
///   default 100000). Export is `frames[start_frame .. start_frame+max_frames]`.
///
/// Returns a user-facing error string for undecodable input, a non-WAV or
/// compressed file, a malformed WAV, or out-of-range options.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    value_scale: &str,
    precision: u32,
    index_column: &str,
    delimiter: &str,
    header: bool,
    start_frame: u64,
    max_frames: u64,
) -> Result<String, String> {
    let scale = match value_scale.trim() {
        "" | "float" => Scale::Float,
        "int" => Scale::Int,
        "db" => Scale::Db,
        other => {
            return Err(format!(
                "invalid value_scale {other:?}: expected \"float\", \"int\", or \"db\""
            ))
        }
    };
    let index = match index_column.trim() {
        "" | "time" => IndexCol::Time,
        "sample" => IndexCol::Sample,
        "both" => IndexCol::Both,
        "none" => IndexCol::None,
        other => {
            return Err(format!(
                "invalid index_column {other:?}: expected \"time\", \"sample\", \"both\", or \"none\""
            ))
        }
    };
    let sep = match delimiter.trim() {
        "" | "comma" => ',',
        "semicolon" => ';',
        "tab" => '\t',
        other => {
            return Err(format!(
                "invalid delimiter {other:?}: expected \"comma\", \"semicolon\", or \"tab\""
            ))
        }
    };
    if precision > 15 {
        return Err(format!(
            "invalid precision {precision}: expected 0-15 decimal places"
        ));
    }
    if !(1..=MAX_FRAMES_CAP).contains(&max_frames) {
        return Err(format!(
            "invalid max_frames {max_frames}: expected 1-{MAX_FRAMES_CAP}"
        ));
    }

    let bytes = decode_bytes(input, input_format)?;
    let wav = parse_wav(&bytes)?;

    let total_frames = wav.frames() as u64;
    if start_frame >= total_frames {
        return Err(format!(
            "start_frame {start_frame} is past the end: the clip has {total_frames} sample frames (indices 0-{})",
            total_frames.saturating_sub(1)
        ));
    }
    let end_frame = start_frame.saturating_add(max_frames).min(total_frames);
    let ch = wav.channels as usize;

    // Reserve roughly enough for the whole CSV so we don't repeatedly realloc a
    // multi-MiB String (amortized doubling holds old+new alive — see the 64 MiB
    // sandbox note in wasm-crates.md).
    let rows = (end_frame - start_frame) as usize + if header { 1 } else { 0 };
    let cols = ch + index.column_count();
    let mut out = String::with_capacity(rows.saturating_mul(cols).saturating_mul(12));

    if header {
        write_header(&mut out, index, ch, sep);
    }

    let full_scale = full_scale(wav.bits_per_sample, wav.is_float);
    for f in start_frame..end_frame {
        write_index(&mut out, index, f, wav.sample_rate, precision, sep);
        let base = (f as usize) * ch;
        for c in 0..ch {
            if index.column_count() > 0 || c > 0 {
                out.push(sep);
            }
            let s = wav.samples[base + c];
            write_value(&mut out, s, scale, precision, full_scale);
        }
        out.push('\n');
    }
    Ok(out)
}

/// Hard cap on exported sample frames — bounds the output String in the 64 MiB
/// wasm sandbox.
const MAX_FRAMES_CAP: u64 = 500_000;

#[derive(Clone, Copy)]
enum Scale {
    Float,
    Int,
    Db,
}

#[derive(Clone, Copy)]
enum IndexCol {
    Time,
    Sample,
    Both,
    None,
}

impl IndexCol {
    fn column_count(self) -> usize {
        match self {
            IndexCol::Time | IndexCol::Sample => 1,
            IndexCol::Both => 2,
            IndexCol::None => 0,
        }
    }
}

fn write_header(out: &mut String, index: IndexCol, channels: usize, sep: char) {
    let mut first = true;
    let mut push = |out: &mut String, s: &str| {
        if !first {
            out.push(sep);
        }
        out.push_str(s);
        first = false;
    };
    match index {
        IndexCol::Time => push(out, "time_s"),
        IndexCol::Sample => push(out, "sample"),
        IndexCol::Both => {
            push(out, "sample");
            push(out, "time_s");
        }
        IndexCol::None => {}
    }
    for c in 0..channels {
        push(out, &format!("channel_{}", c + 1));
    }
    out.push('\n');
}

fn write_index(
    out: &mut String,
    index: IndexCol,
    frame: u64,
    sample_rate: u32,
    precision: u32,
    sep: char,
) {
    let time = |out: &mut String| {
        let t = frame as f64 / sample_rate as f64;
        out.push_str(&fmt_float(t, precision));
    };
    match index {
        IndexCol::Time => time(out),
        IndexCol::Sample => out.push_str(&frame.to_string()),
        IndexCol::Both => {
            out.push_str(&frame.to_string());
            out.push(sep);
            time(out);
        }
        IndexCol::None => {}
    }
}

fn write_value(out: &mut String, sample: f32, scale: Scale, precision: u32, full_scale: f64) {
    match scale {
        Scale::Float => out.push_str(&fmt_float(sample as f64, precision)),
        Scale::Int => {
            let max = full_scale;
            let v = (sample as f64 * full_scale).round();
            let clamped = v.clamp(-max, max - 1.0) as i64;
            out.push_str(&clamped.to_string());
        }
        Scale::Db => out.push_str(&fmt_float(amp_to_dbfs(sample.abs() as f64), precision)),
    }
}

/// Full-scale divisor used to normalize / de-normalize a sample. For integer PCM
/// this is `2^(bits-1)` (so `int` round-trips the decoded integer exactly); for
/// IEEE-float sources `int` maps to a 32-bit integer range.
fn full_scale(bits: u16, is_float: bool) -> f64 {
    if is_float {
        2_147_483_648.0 // 2^31
    } else {
        2f64.powi(bits as i32 - 1)
    }
}

/// dBFS floor so a fully-silent sample reports a finite value instead of -inf.
const DBFS_FLOOR: f64 = -120.0;

fn amp_to_dbfs(amp: f64) -> f64 {
    if amp <= 0.0 {
        DBFS_FLOOR
    } else {
        (20.0 * amp.log10()).max(DBFS_FLOOR)
    }
}

/// Fixed-decimal float formatting that normalizes `-0` to `0`.
fn fmt_float(v: f64, precision: u32) -> String {
    let mut out = format!("{:.*}", precision as usize, v);
    if out.starts_with('-') && out[1..].chars().all(|c| c == '0' || c == '.') {
        out.remove(0);
    }
    out
}

// ---------------------------------------------------------------------------
// WAV parsing (uncompressed RIFF/WAVE)
// ---------------------------------------------------------------------------

/// Decoded audio: `samples` are interleaved across channels and normalized to
/// [-1.0, 1.0] regardless of the source bit depth.
struct WavData {
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    is_float: bool,
    samples: Vec<f32>,
}

impl WavData {
    fn frames(&self) -> usize {
        if self.channels == 0 {
            0
        } else {
            self.samples.len() / self.channels as usize
        }
    }
}

fn u16_le(b: &[u8], off: usize) -> u16 {
    (b[off] as u16) | ((b[off + 1] as u16) << 8)
}
fn u32_le(b: &[u8], off: usize) -> u32 {
    (b[off] as u32)
        | ((b[off + 1] as u32) << 8)
        | ((b[off + 2] as u32) << 16)
        | ((b[off + 3] as u32) << 24)
}

fn parse_wav(b: &[u8]) -> Result<WavData, String> {
    if b.len() < 12 {
        return Err(format!(
            "not a WAV file: only {} bytes, too short for a RIFF header",
            b.len()
        ));
    }
    if &b[0..4] != b"RIFF" {
        return Err(sniff_container(b));
    }
    if &b[8..12] != b"WAVE" {
        return Err("not a WAV file: RIFF header is not of type WAVE".into());
    }

    let mut pos = 12usize;
    let mut fmt: Option<FmtChunk> = None;
    let mut data: Option<&[u8]> = None;

    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let size = u32_le(b, pos + 4) as usize;
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(size).min(b.len());
        let body = &b[body_start..body_end];
        match id {
            b"fmt " => fmt = Some(parse_fmt(body)?),
            b"data" => data = Some(body),
            _ => {}
        }
        let advance = 8 + size + (size & 1);
        pos = match pos.checked_add(advance) {
            Some(p) => p,
            None => break,
        };
    }

    let fmt = fmt.ok_or("malformed WAV: no `fmt ` chunk found")?;
    let data = data.ok_or("malformed WAV: no `data` chunk found")?;

    if fmt.channels == 0 {
        return Err("malformed WAV: channel count is 0".into());
    }
    if fmt.sample_rate == 0 {
        return Err("malformed WAV: sample rate is 0".into());
    }

    let (samples, is_float) = decode_samples(&fmt, data)?;
    Ok(WavData {
        sample_rate: fmt.sample_rate,
        channels: fmt.channels,
        bits_per_sample: fmt.bits_per_sample,
        is_float,
        samples,
    })
}

/// Name a non-RIFF container so the error names the codec instead of a generic
/// "not a WAV".
fn sniff_container(b: &[u8]) -> String {
    let starts = |sig: &[u8]| b.len() >= sig.len() && &b[..sig.len()] == sig;
    let named = if starts(b"OggS") {
        Some("an Ogg container (Vorbis/Opus)")
    } else if starts(b"fLaC") {
        Some("a FLAC file")
    } else if starts(b"ID3") || (b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xE0) == 0xE0) {
        Some("an MP3 file")
    } else if b.len() >= 12 && &b[4..8] == b"ftyp" {
        Some("an MP4/M4A container (AAC/ALAC)")
    } else if starts(b"FORM") {
        Some("an AIFF file")
    } else {
        None
    };
    match named {
        Some(what) => format!(
            "unsupported format: this looks like {what}, but only uncompressed \
             WAV (RIFF/WAVE) is decoded. Convert it to WAV first."
        ),
        None => "not a WAV file: missing the 'RIFF' signature. Only uncompressed \
                 WAV (RIFF/WAVE) is supported; convert other formats to WAV first."
            .into(),
    }
}

struct FmtChunk {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
}

const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_ALAW: u16 = 0x0006;
const WAVE_FORMAT_MULAW: u16 = 0x0007;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

fn parse_fmt(body: &[u8]) -> Result<FmtChunk, String> {
    if body.len() < 16 {
        return Err("malformed WAV: `fmt ` chunk is shorter than 16 bytes".into());
    }
    let mut audio_format = u16_le(body, 0);
    let channels = u16_le(body, 2);
    let sample_rate = u32_le(body, 4);
    let bits_per_sample = u16_le(body, 14);

    if audio_format == WAVE_FORMAT_EXTENSIBLE && body.len() >= 26 {
        let ext_size = u16_le(body, 16) as usize;
        if ext_size >= 22 && body.len() >= 18 + 22 {
            audio_format = u16_le(body, 24);
        }
    }
    Ok(FmtChunk {
        audio_format,
        channels,
        sample_rate,
        bits_per_sample,
    })
}

/// Decode raw `data` bytes to interleaved f32 in [-1.0, 1.0]. Returns the
/// samples and whether the source was IEEE float.
fn decode_samples(fmt: &FmtChunk, data: &[u8]) -> Result<(Vec<f32>, bool), String> {
    match fmt.audio_format {
        WAVE_FORMAT_PCM => match fmt.bits_per_sample {
            8 => Ok((decode_pcm8(data), false)),
            16 => Ok((decode_pcm16(data), false)),
            24 => Ok((decode_pcm24(data), false)),
            32 => Ok((decode_pcm32(data), false)),
            other => Err(format!(
                "unsupported PCM bit depth: {other}-bit. Supported: 8, 16, 24, 32-bit integer."
            )),
        },
        WAVE_FORMAT_IEEE_FLOAT => match fmt.bits_per_sample {
            32 => Ok((decode_f32(data), true)),
            64 => Ok((decode_f64(data), true)),
            other => Err(format!(
                "unsupported IEEE-float bit depth: {other}-bit. Supported: 32 and 64-bit float."
            )),
        },
        WAVE_FORMAT_ALAW => Err("unsupported format: A-law compressed WAV. \
             Convert to uncompressed PCM WAV first."
            .into()),
        WAVE_FORMAT_MULAW => Err("unsupported format: mu-law compressed WAV. \
             Convert to uncompressed PCM WAV first."
            .into()),
        other => Err(format!(
            "unsupported WAV format tag 0x{other:04x}: only uncompressed PCM \
             (0x0001) and IEEE float (0x0003) are decoded."
        )),
    }
}

fn decode_pcm8(data: &[u8]) -> Vec<f32> {
    data.iter().map(|&b| (b as f32 - 128.0) / 128.0).collect()
}
fn decode_pcm16(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect()
}
fn decode_pcm24(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(3)
        .map(|c| {
            let raw = (c[0] as i32) | ((c[1] as i32) << 8) | ((c[2] as i32) << 16);
            let v = (raw << 8) >> 8; // sign-extend 24-bit
            v as f32 / 8_388_608.0
        })
        .collect()
}
fn decode_pcm32(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0)
        .collect()
}
fn decode_f32(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
fn decode_f64(data: &[u8]) -> Vec<f32> {
    data.chunks_exact(8)
        .map(|c| f64::from_le_bytes([c[0], c[1], c[2], c[3], c[4], c[5], c[6], c[7]]) as f32)
        .collect()
}

// ---------------------------------------------------------------------------
// Byte decoding (hex / base64)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the WAV bytes as base64 or hex".into());
    }
    match input_format.trim() {
        "" | "base64" => decode_base64(trimmed),
        "hex" => decode_hex(trimmed),
        other => Err(format!(
            "invalid input_format {other:?}: expected \"base64\" or \"hex\""
        )),
    }
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal PCM WAV from interleaved f32 samples in [-1, 1].
    fn make_wav(sample_rate: u32, channels: u16, bits: u16, samples: &[f32]) -> Vec<u8> {
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data: Vec<u8> = Vec::new();
        for &s in samples {
            let s = s.clamp(-1.0, 1.0);
            match bits {
                8 => data.push(((s * 127.0) as i32 + 128) as u8),
                16 => data.extend_from_slice(&((s * 32767.0) as i16).to_le_bytes()),
                24 => {
                    let v = (s * 8_388_607.0) as i32;
                    data.extend_from_slice(&v.to_le_bytes()[0..3]);
                }
                32 => data.extend_from_slice(&((s as f64 * 2_147_483_647.0) as i32).to_le_bytes()),
                _ => unreachable!(),
            }
        }
        build_riff(WAVE_FORMAT_PCM, channels, sample_rate, bits, byte_rate, block_align, &data)
    }

    fn make_wav16_exact(sample_rate: u32, channels: u16, raw: &[i16]) -> Vec<u8> {
        let bits = 16u16;
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data = Vec::new();
        for &v in raw {
            data.extend_from_slice(&v.to_le_bytes());
        }
        build_riff(WAVE_FORMAT_PCM, channels, sample_rate, bits, byte_rate, block_align, &data)
    }

    fn make_float_wav(sample_rate: u32, channels: u16, samples: &[f32]) -> Vec<u8> {
        let bits = 32u16;
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data = Vec::new();
        for &s in samples {
            data.extend_from_slice(&s.to_le_bytes());
        }
        build_riff(WAVE_FORMAT_IEEE_FLOAT, channels, sample_rate, bits, byte_rate, block_align, &data)
    }

    #[allow(clippy::too_many_arguments)]
    fn build_riff(
        fmt_tag: u16,
        channels: u16,
        sample_rate: u32,
        bits: u16,
        byte_rate: u32,
        block_align: u16,
        data: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&fmt_tag.to_le_bytes());
        out.extend_from_slice(&channels.to_le_bytes());
        out.extend_from_slice(&sample_rate.to_le_bytes());
        out.extend_from_slice(&byte_rate.to_le_bytes());
        out.extend_from_slice(&block_align.to_le_bytes());
        out.extend_from_slice(&bits.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(data);
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
            out.push(if chunk.len() > 1 { TABLE[(n >> 6) as usize & 63] as char } else { '=' });
            out.push(if chunk.len() > 2 { TABLE[n as usize & 63] as char } else { '=' });
        }
        out
    }

    // Default-args helper for the common case.
    fn run_default(input: &str) -> Result<String, String> {
        run(input, "base64", "float", 6, "time", "comma", true, 0, 100_000)
    }

    #[test]
    fn happy_mono_16k_float_time() {
        let wav = make_wav16_exact(16000, 1, &[16384, -8192, 0]);
        let out = run_default(&b64(&wav)).unwrap();
        // 16384/32768 = 0.5, -8192/32768 = -0.25, 0.
        let expected = "time_s,channel_1\n\
                        0.000000,0.500000\n\
                        0.000063,-0.250000\n\
                        0.000125,0.000000\n";
        assert_eq!(out, expected, "got:\n{out}");
    }

    #[test]
    fn int_scale_roundtrips_16bit() {
        let wav = make_wav16_exact(16000, 1, &[16384, -8192, 0, 32767, -32768]);
        let out = run(&b64(&wav), "base64", "int", 6, "none", "comma", false, 0, 100_000).unwrap();
        assert_eq!(out, "16384\n-8192\n0\n32767\n-32768\n", "got:\n{out}");
    }

    #[test]
    fn index_both_and_sample() {
        let wav = make_wav16_exact(8000, 1, &[0, 0]);
        let out = run(&b64(&wav), "base64", "float", 3, "both", "comma", true, 0, 100_000).unwrap();
        let expected = "sample,time_s,channel_1\n\
                        0,0.000,0.000\n\
                        1,0.000,0.000\n";
        assert_eq!(out, expected, "got:\n{out}");

        let out2 = run(&b64(&wav), "base64", "float", 3, "sample", "comma", false, 0, 100_000).unwrap();
        assert_eq!(out2, "0,0.000\n1,0.000\n", "got:\n{out2}");
    }

    #[test]
    fn stereo_one_column_per_channel() {
        // interleaved L,R,L,R -> two channel columns.
        let wav = make_wav16_exact(16000, 2, &[16384, -16384, 0, 32767]);
        let out = run(&b64(&wav), "base64", "int", 6, "none", "comma", true, 0, 100_000).unwrap();
        let expected = "channel_1,channel_2\n\
                        16384,-16384\n\
                        0,32767\n";
        assert_eq!(out, expected, "got:\n{out}");
    }

    #[test]
    fn tab_and_semicolon_delimiters() {
        let wav = make_wav16_exact(16000, 1, &[16384]);
        let tab = run(&b64(&wav), "base64", "int", 6, "sample", "tab", true, 0, 100_000).unwrap();
        assert_eq!(tab, "sample\tchannel_1\n0\t16384\n", "got:\n{tab}");
        let semi = run(&b64(&wav), "base64", "int", 6, "sample", "semicolon", true, 0, 100_000).unwrap();
        assert_eq!(semi, "sample;channel_1\n0;16384\n", "got:\n{semi}");
    }

    #[test]
    fn db_scale_full_scale_is_zero() {
        let wav = make_wav16_exact(16000, 1, &[32767, 0]);
        let out = run(&b64(&wav), "base64", "db", 2, "none", "comma", false, 0, 100_000).unwrap();
        let mut lines = out.lines();
        // 32767/32768 ~ full scale -> ~0 dBFS; silence -> floor -120.
        assert_eq!(lines.next().unwrap(), "0.00");
        assert_eq!(lines.next().unwrap(), "-120.00");
    }

    #[test]
    fn windowing_start_and_max_frames() {
        let wav = make_wav16_exact(16000, 1, &[0, 100, 200, 300, 400]);
        let out = run(&b64(&wav), "base64", "int", 6, "sample", "comma", false, 1, 2).unwrap();
        assert_eq!(out, "1,100\n2,200\n", "got:\n{out}");
    }

    #[test]
    fn no_header_no_index_just_values() {
        let wav = make_wav16_exact(16000, 1, &[16384, -16384]);
        let out = run(&b64(&wav), "base64", "int", 6, "none", "comma", false, 0, 100_000).unwrap();
        assert_eq!(out, "16384\n-16384\n", "got:\n{out}");
    }

    #[test]
    fn parses_all_pcm_depths_and_float() {
        for bits in [8u16, 16, 24, 32] {
            let wav = make_wav(16000, 1, bits, &[0.5, -0.5]);
            let out = run_default(&b64(&wav)).unwrap();
            assert!(out.contains("channel_1"), "bits {bits}: {out}");
            assert_eq!(out.lines().count(), 3, "bits {bits} row count: {out}");
        }
        let fwav = make_float_wav(16000, 1, &[0.5, -0.25]);
        let out = run(&b64(&fwav), "base64", "float", 4, "none", "comma", false, 0, 100_000).unwrap();
        assert_eq!(out, "0.5000\n-0.2500\n", "got:\n{out}");
    }

    #[test]
    fn hex_input_works() {
        let wav = make_wav16_exact(16000, 1, &[16384]);
        let hex: String = wav.iter().map(|b| format!("{b:02x}")).collect();
        let out = run(&hex, "hex", "int", 6, "none", "comma", false, 0, 100_000).unwrap();
        assert_eq!(out, "16384\n", "got:\n{out}");
    }

    #[test]
    fn skips_unknown_chunks() {
        let data: Vec<u8> = (0..4).flat_map(|_| 100i16.to_le_bytes()).collect();
        let mut out = Vec::new();
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((36 + 16 + data.len()) as u32).to_le_bytes());
        out.extend_from_slice(b"WAVE");
        out.extend_from_slice(b"fmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&WAVE_FORMAT_PCM.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&16000u32.to_le_bytes());
        out.extend_from_slice(&32000u32.to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"LIST");
        out.extend_from_slice(&8u32.to_le_bytes());
        out.extend_from_slice(b"INFOjunk");
        out.extend_from_slice(b"data");
        out.extend_from_slice(&(data.len() as u32).to_le_bytes());
        out.extend_from_slice(&data);
        let res = run(&b64(&out), "base64", "int", 6, "none", "comma", false, 0, 100_000).unwrap();
        assert_eq!(res, "100\n100\n100\n100\n", "got:\n{res}");
    }

    #[test]
    fn error_invalid_base64() {
        let err = run_default("not base64 @@@").unwrap_err();
        assert!(err.contains("base64"), "{err}");
    }

    #[test]
    fn error_odd_hex() {
        let err = run("abc", "hex", "float", 6, "time", "comma", true, 0, 100_000).unwrap_err();
        assert!(err.contains("odd number"), "{err}");
    }

    #[test]
    fn error_not_a_wav() {
        let err = run_default(&b64(b"hello world not a wav at all")).unwrap_err();
        assert!(err.contains("not a WAV"), "{err}");
    }

    #[test]
    fn error_compressed_mp3_sniffed() {
        let err = run_default(&b64(b"ID3\x04\x00\x00\x00\x00\x00\x00rest")).unwrap_err();
        assert!(err.contains("MP3"), "{err}");
    }

    #[test]
    fn error_bad_value_scale() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let err = run(&b64(&wav), "base64", "xml", 6, "time", "comma", true, 0, 100_000).unwrap_err();
        assert!(err.contains("value_scale"), "{err}");
    }

    #[test]
    fn error_bad_index_column() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let err = run(&b64(&wav), "base64", "float", 6, "row", "comma", true, 0, 100_000).unwrap_err();
        assert!(err.contains("index_column"), "{err}");
    }

    #[test]
    fn error_bad_delimiter() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let err = run(&b64(&wav), "base64", "float", 6, "time", "pipe", true, 0, 100_000).unwrap_err();
        assert!(err.contains("delimiter"), "{err}");
    }

    #[test]
    fn error_max_frames_out_of_range() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let err = run(&b64(&wav), "base64", "float", 6, "time", "comma", true, 0, 0).unwrap_err();
        assert!(err.contains("max_frames"), "{err}");
        let err2 = run(&b64(&wav), "base64", "float", 6, "time", "comma", true, 0, 999_999).unwrap_err();
        assert!(err2.contains("max_frames"), "{err2}");
    }

    #[test]
    fn error_precision_out_of_range() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let err = run(&b64(&wav), "base64", "float", 99, "time", "comma", true, 0, 100_000).unwrap_err();
        assert!(err.contains("precision"), "{err}");
    }

    #[test]
    fn error_start_frame_past_end() {
        let wav = make_wav16_exact(16000, 1, &[0, 0, 0]);
        let err = run(&b64(&wav), "base64", "float", 6, "time", "comma", true, 5, 100_000).unwrap_err();
        assert!(err.contains("start_frame"), "{err}");
        assert!(err.contains("3 sample frames"), "{err}");
    }
}
