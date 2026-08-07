//! wav-samples-to-json core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Decodes an uncompressed PCM/IEEE-float WAV (given as base64 or hex bytes) and
//! emits JSON: the `fmt`-chunk format metadata (sample rate, channels, bit depth,
//! encoding, duration, frame count, byte rate, block align) alongside the decoded
//! PCM samples as a JSON array — interleaved, or one array per channel. Sample
//! values can be normalized floats in [-1, 1], raw PCM integers at the source bit
//! depth, or dBFS magnitudes.
//!
//! Only uncompressed WAV is decoded (PCM 8/16/24/32-bit integer and 32/64-bit
//! IEEE float). Compressed containers/codecs (MP3, AAC/M4A, Opus/Ogg, FLAC,
//! A-law/mu-law) are rejected with a clear message rather than guessed at.

/// Export a WAV clip's format metadata and decoded PCM samples as JSON.
///
/// - `input`: the WAV file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `output`: `"full"` (metadata + export info + samples, default),
///   `"samples"` (the bare sample array only), or `"metadata"` (format info
///   only, no samples decoded into the output).
/// - `layout`: `"interleaved"` (default — one flat array, channels interleaved
///   L,R,L,R…) or `"channels"` (an array per channel, de-interleaved).
/// - `value_scale`: `"float"` (normalized [-1,1], default), `"int"` (raw PCM
///   integer at the source bit depth; float sources scale to 32-bit), or `"db"`
///   (dBFS magnitude).
/// - `precision`: decimal places for `float`/`db` values (0-15, default 6).
/// - `start_frame`: first sample frame to export (default 0).
/// - `frame_step`: keep every Nth sample frame (1-10000, default 1). Use it to
///   decimate a long clip down to a waveform-preview-sized array.
/// - `max_frames`: maximum number of frames to EXPORT after step-decimation
///   (1-200000, default 50000).
/// - `indent`: JSON indent spaces (0-8, default 2). `0` emits compact
///   single-line JSON. Numeric sample arrays always stay on one line.
///
/// Returns a user-facing error string for undecodable input, a non-WAV or
/// compressed file, a malformed WAV, or out-of-range options.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    layout: &str,
    value_scale: &str,
    precision: u32,
    start_frame: u64,
    frame_step: u64,
    max_frames: u64,
    indent: u32,
) -> Result<String, String> {
    let out_mode = match output.trim() {
        "" | "full" => OutMode::Full,
        "samples" => OutMode::Samples,
        "metadata" => OutMode::Metadata,
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"full\", \"samples\", or \"metadata\""
            ))
        }
    };
    let layout = match layout.trim() {
        "" | "interleaved" => Layout::Interleaved,
        "channels" => Layout::Channels,
        other => {
            return Err(format!(
                "invalid layout {other:?}: expected \"interleaved\" or \"channels\""
            ))
        }
    };
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
    if precision > 15 {
        return Err(format!(
            "invalid precision {precision}: expected 0-15 decimal places"
        ));
    }
    if indent > MAX_INDENT {
        return Err(format!(
            "invalid indent {indent}: expected 0-{MAX_INDENT} spaces (0 = compact JSON)"
        ));
    }
    if !(1..=MAX_STEP_CAP).contains(&frame_step) {
        return Err(format!(
            "invalid frame_step {frame_step}: expected 1-{MAX_STEP_CAP} (1 = keep every frame)"
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
    // Metadata-only never touches the sample window, so an empty or fully
    // skipped-past clip still reports its format instead of erroring.
    if out_mode != OutMode::Metadata && start_frame >= total_frames {
        return Err(format!(
            "start_frame {start_frame} is past the end: the clip has {total_frames} sample frames (indices 0-{})",
            total_frames.saturating_sub(1)
        ));
    }

    // Frames actually exported: start_frame, +step, +2*step … capped by max_frames.
    let kept: Vec<u64> = if out_mode == OutMode::Metadata {
        Vec::new()
    } else {
        (0..max_frames)
            .map(|k| start_frame.saturating_add(k.saturating_mul(frame_step)))
            .take_while(|f| *f < total_frames)
            .collect()
    };

    let f = Json { ind: indent as usize };
    let ch = wav.channels as usize;
    let full_scale = full_scale(wav.bits_per_sample, wav.is_float);

    // Roughly reserve the whole document up front so a multi-MiB String doesn't
    // repeatedly realloc in the 64 MiB wasm sandbox.
    let mut out = String::with_capacity(kept.len().saturating_mul(ch).saturating_mul(12) + 512);

    match out_mode {
        OutMode::Metadata => write_metadata(&mut out, &f, &wav, total_frames, 0),
        OutMode::Samples => write_samples(&mut out, &f, &wav, &kept, layout, scale, precision, full_scale, 0),
        OutMode::Full => {
            out.push('{');
            out.push_str(&f.nl(1));
            out.push_str("\"metadata\"");
            out.push_str(f.colon());
            write_metadata(&mut out, &f, &wav, total_frames, 1);
            out.push(',');
            out.push_str(&f.nl(1));
            out.push_str("\"export\"");
            out.push_str(f.colon());
            write_export(&mut out, &f, start_frame, frame_step, kept.len(), scale, layout, 1);
            out.push(',');
            out.push_str(&f.nl(1));
            out.push_str("\"samples\"");
            out.push_str(f.colon());
            write_samples(&mut out, &f, &wav, &kept, layout, scale, precision, full_scale, 1);
            out.push_str(&f.nl(0));
            out.push('}');
        }
    }
    out.push('\n');
    Ok(out)
}

/// Hard cap on exported sample frames — bounds the output String in the 64 MiB
/// wasm sandbox. JSON is more verbose per value than CSV, so this sits below the
/// CSV exporter's cap.
const MAX_FRAMES_CAP: u64 = 200_000;
/// Hard cap on the decimation stride.
const MAX_STEP_CAP: u64 = 10_000;
/// Hard cap on JSON indentation width.
const MAX_INDENT: u32 = 8;

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutMode {
    Full,
    Samples,
    Metadata,
}

#[derive(Clone, Copy)]
enum Layout {
    Interleaved,
    Channels,
}

impl Layout {
    fn name(self) -> &'static str {
        match self {
            Layout::Interleaved => "interleaved",
            Layout::Channels => "channels",
        }
    }
}

#[derive(Clone, Copy)]
enum Scale {
    Float,
    Int,
    Db,
}

impl Scale {
    fn name(self) -> &'static str {
        match self {
            Scale::Float => "float",
            Scale::Int => "int",
            Scale::Db => "db",
        }
    }
}

// ---------------------------------------------------------------------------
// JSON writing
// ---------------------------------------------------------------------------

/// Indentation-aware JSON punctuation. `ind == 0` means compact single-line.
struct Json {
    ind: usize,
}

impl Json {
    /// Newline + indentation for nesting `level`, or nothing in compact mode.
    fn nl(&self, level: usize) -> String {
        if self.ind == 0 {
            String::new()
        } else {
            let mut s = String::with_capacity(1 + level * self.ind);
            s.push('\n');
            for _ in 0..level * self.ind {
                s.push(' ');
            }
            s
        }
    }
    fn colon(&self) -> &'static str {
        if self.ind == 0 {
            ":"
        } else {
            ": "
        }
    }
    /// Separator between values inside a one-line numeric array.
    fn comma(&self) -> &'static str {
        if self.ind == 0 {
            ","
        } else {
            ", "
        }
    }
}

fn write_metadata(out: &mut String, f: &Json, wav: &WavData, total_frames: u64, level: usize) {
    let inner = level + 1;
    let duration = total_frames as f64 / wav.sample_rate as f64;
    out.push('{');
    let fields: [(&str, String); 9] = [
        ("sampleRate", wav.sample_rate.to_string()),
        ("channels", wav.channels.to_string()),
        ("bitDepth", wav.bits_per_sample.to_string()),
        (
            "encoding",
            format!("\"{}\"", if wav.is_float { "ieee-float" } else { "pcm-int" }),
        ),
        ("formatTag", wav.format_tag.to_string()),
        ("byteRate", wav.byte_rate.to_string()),
        ("blockAlign", wav.block_align.to_string()),
        ("totalFrames", total_frames.to_string()),
        ("durationSeconds", trim_float(duration, 6)),
    ];
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&f.nl(inner));
        out.push('"');
        out.push_str(k);
        out.push('"');
        out.push_str(f.colon());
        out.push_str(v);
    }
    out.push_str(&f.nl(level));
    out.push('}');
}

#[allow(clippy::too_many_arguments)]
fn write_export(
    out: &mut String,
    f: &Json,
    start_frame: u64,
    frame_step: u64,
    frame_count: usize,
    scale: Scale,
    layout: Layout,
    level: usize,
) {
    let inner = level + 1;
    out.push('{');
    let fields: [(&str, String); 5] = [
        ("startFrame", start_frame.to_string()),
        ("frameStep", frame_step.to_string()),
        ("frameCount", frame_count.to_string()),
        ("valueScale", format!("\"{}\"", scale.name())),
        ("layout", format!("\"{}\"", layout.name())),
    ];
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&f.nl(inner));
        out.push('"');
        out.push_str(k);
        out.push('"');
        out.push_str(f.colon());
        out.push_str(v);
    }
    out.push_str(&f.nl(level));
    out.push('}');
}

/// Sample arrays are always written on ONE line — a 50 000-element array with one
/// value per line is unusable, and this keeps pretty output diff-friendly.
#[allow(clippy::too_many_arguments)]
fn write_samples(
    out: &mut String,
    f: &Json,
    wav: &WavData,
    kept: &[u64],
    layout: Layout,
    scale: Scale,
    precision: u32,
    full_scale: f64,
    level: usize,
) {
    let ch = wav.channels as usize;
    match layout {
        Layout::Interleaved => {
            out.push('[');
            let mut first = true;
            for &frame in kept {
                let base = frame as usize * ch;
                for c in 0..ch {
                    if !first {
                        out.push_str(f.comma());
                    }
                    first = false;
                    write_value(out, wav.samples[base + c], scale, precision, full_scale);
                }
            }
            out.push(']');
        }
        Layout::Channels => {
            out.push('[');
            for c in 0..ch {
                if c > 0 {
                    out.push(',');
                }
                out.push_str(&f.nl(level + 1));
                out.push('[');
                for (i, &frame) in kept.iter().enumerate() {
                    if i > 0 {
                        out.push_str(f.comma());
                    }
                    write_value(
                        out,
                        wav.samples[frame as usize * ch + c],
                        scale,
                        precision,
                        full_scale,
                    );
                }
                out.push(']');
            }
            out.push_str(&f.nl(level));
            out.push(']');
        }
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

/// Fixed-decimal float formatting that normalizes `-0` to `0`. JSON has no `-0`
/// distinction worth preserving and `-0.000000` reads as a bug.
fn fmt_float(v: f64, precision: u32) -> String {
    let mut out = format!("{:.*}", precision as usize, v);
    if out.starts_with('-') && out[1..].chars().all(|c| c == '0' || c == '.') {
        out.remove(0);
    }
    out
}

/// Float formatting for metadata numbers: fixed precision with trailing zeros
/// (and a bare trailing `.`) trimmed, so `0.5` stays `0.5` and `2.0` becomes `2`.
fn trim_float(v: f64, precision: u32) -> String {
    let s = format!("{:.*}", precision as usize, v);
    if !s.contains('.') {
        return s;
    }
    let t = s.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-" {
        "0".to_string()
    } else {
        t.to_string()
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

/// dBFS floor so a fully-silent sample reports a finite value instead of -inf
/// (JSON cannot express `-Infinity`).
const DBFS_FLOOR: f64 = -120.0;

fn amp_to_dbfs(amp: f64) -> f64 {
    if amp <= 0.0 {
        DBFS_FLOOR
    } else {
        (20.0 * amp.log10()).max(DBFS_FLOOR)
    }
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
    byte_rate: u32,
    block_align: u16,
    format_tag: u16,
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
        byte_rate: fmt.byte_rate,
        block_align: fmt.block_align,
        format_tag: fmt.audio_format,
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
    byte_rate: u32,
    block_align: u16,
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
    let byte_rate = u32_le(body, 8);
    let block_align = u16_le(body, 12);
    let bits_per_sample = u16_le(body, 14);

    // WAVE_FORMAT_EXTENSIBLE hides the real codec in the first two bytes of the
    // 22-byte extension SubFormat GUID.
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
        byte_rate,
        block_align,
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
        build_riff(
            WAVE_FORMAT_PCM,
            channels,
            sample_rate,
            bits,
            byte_rate,
            block_align,
            &data,
        )
    }

    fn make_wav16_exact(sample_rate: u32, channels: u16, raw: &[i16]) -> Vec<u8> {
        let bits = 16u16;
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data = Vec::new();
        for &v in raw {
            data.extend_from_slice(&v.to_le_bytes());
        }
        build_riff(
            WAVE_FORMAT_PCM,
            channels,
            sample_rate,
            bits,
            byte_rate,
            block_align,
            &data,
        )
    }

    fn make_float_wav(sample_rate: u32, channels: u16, samples: &[f32]) -> Vec<u8> {
        let bits = 32u16;
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut data = Vec::new();
        for &s in samples {
            data.extend_from_slice(&s.to_le_bytes());
        }
        build_riff(
            WAVE_FORMAT_IEEE_FLOAT,
            channels,
            sample_rate,
            bits,
            byte_rate,
            block_align,
            &data,
        )
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

    /// Default-args helper for the common case.
    fn run_default(input: &str) -> Result<String, String> {
        run(input, "base64", "full", "interleaved", "float", 6, 0, 1, 50_000, 2)
    }

    #[test]
    fn happy_full_document_mono_16bit() {
        let wav = make_wav16_exact(16000, 1, &[16384, -8192, 0]);
        let out = run_default(&b64(&wav)).unwrap();
        // 16384/32768 = 0.5, -8192/32768 = -0.25, 0.
        let expected = "{\n\
                        \x20 \"metadata\": {\n\
                        \x20   \"sampleRate\": 16000,\n\
                        \x20   \"channels\": 1,\n\
                        \x20   \"bitDepth\": 16,\n\
                        \x20   \"encoding\": \"pcm-int\",\n\
                        \x20   \"formatTag\": 1,\n\
                        \x20   \"byteRate\": 32000,\n\
                        \x20   \"blockAlign\": 2,\n\
                        \x20   \"totalFrames\": 3,\n\
                        \x20   \"durationSeconds\": 0.000188\n\
                        \x20 },\n\
                        \x20 \"export\": {\n\
                        \x20   \"startFrame\": 0,\n\
                        \x20   \"frameStep\": 1,\n\
                        \x20   \"frameCount\": 3,\n\
                        \x20   \"valueScale\": \"float\",\n\
                        \x20   \"layout\": \"interleaved\"\n\
                        \x20 },\n\
                        \x20 \"samples\": [0.500000, -0.250000, 0.000000]\n\
                        }\n";
        assert_eq!(out, expected, "got:\n{out}");
    }

    #[test]
    fn samples_only_int_scale_roundtrips_16bit() {
        let wav = make_wav16_exact(16000, 1, &[16384, -8192, 0, 32767, -32768]);
        let out = run(
            &b64(&wav),
            "base64",
            "samples",
            "interleaved",
            "int",
            6,
            0,
            1,
            50_000,
            2,
        )
        .unwrap();
        assert_eq!(out, "[16384, -8192, 0, 32767, -32768]\n", "got:\n{out}");
    }

    #[test]
    fn compact_indent_zero_is_single_line() {
        let wav = make_wav16_exact(8000, 1, &[16384, -16384]);
        let out = run(
            &b64(&wav),
            "base64",
            "samples",
            "interleaved",
            "int",
            6,
            0,
            1,
            50_000,
            0,
        )
        .unwrap();
        assert_eq!(out, "[16384,-16384]\n", "got:\n{out}");

        let full = run(
            &b64(&wav),
            "base64",
            "full",
            "interleaved",
            "int",
            6,
            0,
            1,
            50_000,
            0,
        )
        .unwrap();
        assert!(full.lines().count() == 1, "expected one line, got:\n{full}");
        assert!(full.contains("\"samples\":[16384,-16384]"), "got:\n{full}");
        assert!(full.contains("\"sampleRate\":8000"), "got:\n{full}");
    }

    #[test]
    fn metadata_only_omits_samples() {
        let wav = make_wav16_exact(44100, 2, &[0, 0, 0, 0]);
        let out = run(
            &b64(&wav),
            "base64",
            "metadata",
            "interleaved",
            "float",
            6,
            0,
            1,
            50_000,
            0,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"sampleRate\":44100,\"channels\":2,\"bitDepth\":16,\"encoding\":\"pcm-int\",\
             \"formatTag\":1,\"byteRate\":176400,\"blockAlign\":4,\"totalFrames\":2,\
             \"durationSeconds\":0.000045}\n",
            "got:\n{out}"
        );
    }

    #[test]
    fn channels_layout_deinterleaves() {
        // interleaved L,R = 16384,-16384 then 0,32767.
        let wav = make_wav16_exact(16000, 2, &[16384, -16384, 0, 32767]);
        let out = run(
            &b64(&wav),
            "base64",
            "samples",
            "channels",
            "int",
            6,
            0,
            1,
            50_000,
            2,
        )
        .unwrap();
        assert_eq!(
            out,
            "[\n  [16384, 0],\n  [-16384, 32767]\n]\n",
            "got:\n{out}"
        );
    }

    #[test]
    fn frame_step_decimates() {
        let wav = make_wav16_exact(16000, 1, &[0, 100, 200, 300, 400, 500, 600]);
        let out = run(
            &b64(&wav),
            "base64",
            "samples",
            "interleaved",
            "int",
            6,
            0,
            3,
            50_000,
            0,
        )
        .unwrap();
        assert_eq!(out, "[0,300,600]\n", "got:\n{out}");
    }

    #[test]
    fn windowing_start_and_max_frames() {
        let wav = make_wav16_exact(16000, 1, &[0, 100, 200, 300, 400]);
        let out = run(
            &b64(&wav),
            "base64",
            "samples",
            "interleaved",
            "int",
            6,
            1,
            1,
            2,
            0,
        )
        .unwrap();
        assert_eq!(out, "[100,200]\n", "got:\n{out}");
    }

    #[test]
    fn export_block_reports_actual_frame_count() {
        let wav = make_wav16_exact(16000, 1, &[0, 100, 200, 300, 400]);
        let out = run(
            &b64(&wav),
            "base64",
            "full",
            "interleaved",
            "int",
            6,
            1,
            2,
            50_000,
            0,
        )
        .unwrap();
        // frames 1, 3 → count 2 (frame 5 is past the end).
        assert!(out.contains("\"startFrame\":1"), "got:\n{out}");
        assert!(out.contains("\"frameStep\":2"), "got:\n{out}");
        assert!(out.contains("\"frameCount\":2"), "got:\n{out}");
        assert!(out.contains("\"samples\":[100,300]"), "got:\n{out}");
    }

    #[test]
    fn db_scale_full_scale_is_zero() {
        let wav = make_wav16_exact(16000, 1, &[32767, 0]);
        let out = run(
            &b64(&wav),
            "base64",
            "samples",
            "interleaved",
            "db",
            2,
            0,
            1,
            50_000,
            0,
        )
        .unwrap();
        // 32767/32768 ~ full scale -> ~0 dBFS; silence -> the -120 floor.
        assert_eq!(out, "[0.00,-120.00]\n", "got:\n{out}");
    }

    #[test]
    fn parses_all_pcm_depths_and_float() {
        for bits in [8u16, 16, 24, 32] {
            let wav = make_wav(16000, 1, bits, &[0.5, -0.5]);
            let out = run_default(&b64(&wav)).unwrap();
            assert!(out.contains(&format!("\"bitDepth\": {bits}")), "bits {bits}: {out}");
            assert!(out.contains("\"frameCount\": 2"), "bits {bits}: {out}");
        }
        let fwav = make_float_wav(16000, 1, &[0.5, -0.25]);
        let out = run(
            &b64(&fwav),
            "base64",
            "full",
            "interleaved",
            "float",
            4,
            0,
            1,
            50_000,
            0,
        )
        .unwrap();
        assert!(out.contains("\"encoding\":\"ieee-float\""), "got:\n{out}");
        assert!(out.contains("\"formatTag\":3"), "got:\n{out}");
        assert!(out.contains("\"samples\":[0.5000,-0.2500]"), "got:\n{out}");
    }

    #[test]
    fn hex_input_works() {
        let wav = make_wav16_exact(16000, 1, &[16384]);
        let hex: String = wav.iter().map(|b| format!("{b:02x}")).collect();
        let out = run(
            &hex,
            "hex",
            "samples",
            "interleaved",
            "int",
            6,
            0,
            1,
            50_000,
            0,
        )
        .unwrap();
        assert_eq!(out, "[16384]\n", "got:\n{out}");
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
        let res = run(
            &b64(&out),
            "base64",
            "samples",
            "interleaved",
            "int",
            6,
            0,
            1,
            50_000,
            0,
        )
        .unwrap();
        assert_eq!(res, "[100,100,100,100]\n", "got:\n{res}");
    }

    #[test]
    fn error_invalid_base64() {
        let err = run_default("not base64 @@@").unwrap_err();
        assert!(err.contains("base64"), "{err}");
    }

    #[test]
    fn error_odd_hex() {
        let err = run("abc", "hex", "full", "interleaved", "float", 6, 0, 1, 50_000, 2)
            .unwrap_err();
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
    fn error_bad_output_mode() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let err = run(&b64(&wav), "base64", "xml", "interleaved", "float", 6, 0, 1, 50_000, 2)
            .unwrap_err();
        assert!(err.contains("output"), "{err}");
    }

    #[test]
    fn error_bad_layout() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let err = run(&b64(&wav), "base64", "full", "planar", "float", 6, 0, 1, 50_000, 2)
            .unwrap_err();
        assert!(err.contains("layout"), "{err}");
    }

    #[test]
    fn error_bad_value_scale() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let err = run(&b64(&wav), "base64", "full", "interleaved", "u8", 6, 0, 1, 50_000, 2)
            .unwrap_err();
        assert!(err.contains("value_scale"), "{err}");
    }

    #[test]
    fn error_out_of_range_numbers() {
        let wav = make_wav16_exact(16000, 1, &[0]);
        let b = b64(&wav);
        let e = run(&b, "base64", "full", "interleaved", "float", 99, 0, 1, 50_000, 2).unwrap_err();
        assert!(e.contains("precision"), "{e}");
        let e = run(&b, "base64", "full", "interleaved", "float", 6, 0, 1, 0, 2).unwrap_err();
        assert!(e.contains("max_frames"), "{e}");
        let e = run(&b, "base64", "full", "interleaved", "float", 6, 0, 1, 200_001, 2).unwrap_err();
        assert!(e.contains("max_frames"), "{e}");
        let e = run(&b, "base64", "full", "interleaved", "float", 6, 0, 0, 50_000, 2).unwrap_err();
        assert!(e.contains("frame_step"), "{e}");
        let e = run(&b, "base64", "full", "interleaved", "float", 6, 0, 10_001, 50_000, 2)
            .unwrap_err();
        assert!(e.contains("frame_step"), "{e}");
        let e = run(&b, "base64", "full", "interleaved", "float", 6, 0, 1, 50_000, 9).unwrap_err();
        assert!(e.contains("indent"), "{e}");
    }

    #[test]
    fn error_start_frame_past_end() {
        let wav = make_wav16_exact(16000, 1, &[0, 0, 0]);
        let err = run(
            &b64(&wav),
            "base64",
            "full",
            "interleaved",
            "float",
            6,
            5,
            1,
            50_000,
            2,
        )
        .unwrap_err();
        assert!(err.contains("start_frame"), "{err}");
        assert!(err.contains("3 sample frames"), "{err}");
    }
}
