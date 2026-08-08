//! wav-to-raw-pcm-extractor core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Takes an uncompressed RIFF/WAVE file (pasted as base64 or hex bytes), walks
//! its chunk chain, and returns ONLY the `data` chunk payload — the bare
//! interleaved PCM sample bytes, with the 44-byte-ish RIFF header and every
//! other chunk (`fmt `, `LIST`, `fact`, `cue `, trailing metadata) removed.
//!
//! The default `sample_format = "source"` with `channels = "all"` slices the
//! payload out byte-for-byte — no decode, no re-encode, so the output is exactly
//! what the container stored. Asking for a different sample format or a channel
//! selection decodes each sample and re-encodes it (u8 / s16 / s24 / s32 / f32,
//! little- or big-endian), which is what `ffmpeg -f s16le` or `sox -t raw` would
//! give you.
//!
//! Raw PCM carries no header at all, so the `info` output prints the sample
//! rate / channel count / encoding a player needs, plus ready-to-run ffmpeg,
//! SoX, and Audacity re-import settings.

#![allow(clippy::too_many_arguments)]

/// Extract the bare PCM payload of a WAV file.
///
/// - `input`: the WAV file bytes as a base64 or hex string.
/// - `input_format`: `"base64"` (default) or `"hex"` (blank → base64).
/// - `output`: `"base64"` (default), `"hex"`, `"c_array"`, or `"info"`
///   (the format report + re-import commands, no sample bytes).
/// - `sample_format`: `"source"` (default, verbatim payload bytes) or a target
///   encoding — `u8`, `s16le`, `s16be`, `s24le`, `s24be`, `s32le`, `s32be`,
///   `f32le`, `f32be`.
/// - `channels`: `"all"` (default, interleaved as stored), `"mono"` (average
///   downmix), `"left"` (channel 1 only), or `"right"` (channel 2 only).
/// - `start_frame`: first sample frame to extract (default 0).
/// - `max_frames`: how many frames to extract; `0` (default) = to the end.
/// - `line_bytes`: bytes per line for the `hex` / `c_array` outputs (0-64,
///   default 16; 0 = one unbroken line). Ignored by `base64` / `info`.
///
/// Returns a user-facing error string for undecodable input, a non-WAV or
/// compressed file, a malformed WAV, out-of-range options, or an extraction
/// larger than the output cap.
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    sample_format: &str,
    channels: &str,
    start_frame: u64,
    max_frames: u64,
    line_bytes: u32,
) -> Result<String, String> {
    let out_mode = OutputMode::parse(output)?;
    let sel = ChannelSel::parse(channels)?;
    if line_bytes > MAX_LINE_BYTES {
        return Err(format!(
            "invalid line_bytes {line_bytes}: expected 0-{MAX_LINE_BYTES} \
             (0 = one unbroken line)"
        ));
    }

    let bytes = decode_bytes(input, input_format)?;
    let wav = parse_wav(&bytes)?;

    // Byte-for-byte passthrough only when nothing has to be re-encoded. That is
    // also the only path that accepts companded (A-law / mu-law) payloads: we
    // never interpret the samples, we just cut the chunk out.
    let verbatim = sample_format_is_source(sample_format) && sel == ChannelSel::All;
    let source_enc = wav.source_encoding();
    if !verbatim && source_enc.is_none() {
        return Err(format!(
            "{} cannot be decoded, so it can only be extracted verbatim: \
             leave sample_format=\"source\" and channels=\"all\", or convert the \
             file to plain PCM WAV first (e.g. `ffmpeg -i clip.wav -c:a pcm_s16le plain.wav`)",
            wav.codec_label()
        ));
    }
    let target = match parse_sample_format(sample_format)? {
        Some(t) => t,
        None => match source_enc {
            Some(enc) => Target {
                enc,
                endian: Endian::Le,
            },
            // Verbatim companded payload: report the source tag, never claim a
            // PCM encoding the bytes do not have.
            None => Target {
                enc: Encoding::U8,
                endian: Endian::Le,
            },
        },
    };

    let total_frames = wav.total_frames();
    if total_frames == 0 {
        return Err(format!(
            "the `data` chunk holds {} bytes, which is less than one {}-byte \
             sample frame — there is nothing to extract",
            wav.data.len(),
            wav.block_align
        ));
    }
    if start_frame >= total_frames {
        return Err(format!(
            "start_frame {start_frame} is past the end: the clip has {total_frames} \
             sample frames (indices 0-{})",
            total_frames - 1
        ));
    }
    let end_frame = if max_frames == 0 {
        total_frames
    } else {
        start_frame.saturating_add(max_frames).min(total_frames)
    };
    let frames_out = (end_frame - start_frame) as usize;
    let out_channels = sel.out_channels(wav.channels)?;
    let out_len = frames_out
        .saturating_mul(out_channels as usize)
        .saturating_mul(if verbatim {
            wav.block_align as usize / wav.channels.max(1) as usize
        } else {
            target.bytes()
        });

    if out_mode == OutputMode::Info {
        return Ok(render_info(
            &bytes, &wav, target, sel, verbatim, start_frame, end_frame, out_channels, out_len,
        ));
    }

    let cap = out_mode.max_pcm_bytes();
    if out_len > cap {
        return Err(format!(
            "the requested range is {out_len} bytes of PCM, over the {cap}-byte cap \
             for the \"{}\" output — narrow it with start_frame / max_frames \
             (one frame is {} bytes here), pick a smaller sample_format, or use \
             output=\"base64\" (cap {} bytes)",
            out_mode.name(),
            wav.block_align,
            OutputMode::Base64.max_pcm_bytes()
        ));
    }

    let pcm = extract(&wav, target, sel, verbatim, start_frame, end_frame, out_len);
    Ok(match out_mode {
        OutputMode::Base64 => encode_base64(&pcm),
        OutputMode::Hex => encode_hex(&pcm, line_bytes),
        OutputMode::CArray => encode_c_array(&pcm, line_bytes, &wav, target, out_channels),
        OutputMode::Info => unreachable!(),
    })
}

/// Bytes-per-line ceiling for the `hex` / `c_array` outputs.
const MAX_LINE_BYTES: u32 = 64;

/// Per-output caps on the extracted PCM size. Base64 grows the payload ~1.33x,
/// spaced hex ~3x and a C array ~6x, so each mode gets a cap that keeps the
/// rendered string inside the 64 MiB wasm sandbox.
const CAP_BASE64: usize = 6 << 20;
const CAP_HEX: usize = 3 << 20;
const CAP_C_ARRAY: usize = 1 << 20;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum OutputMode {
    Base64,
    Hex,
    CArray,
    Info,
}

impl OutputMode {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "base64" => Ok(OutputMode::Base64),
            "hex" => Ok(OutputMode::Hex),
            "c_array" => Ok(OutputMode::CArray),
            "info" => Ok(OutputMode::Info),
            other => Err(format!(
                "invalid output {other:?}: expected \"base64\", \"hex\", \
                 \"c_array\", or \"info\""
            )),
        }
    }
    fn name(self) -> &'static str {
        match self {
            OutputMode::Base64 => "base64",
            OutputMode::Hex => "hex",
            OutputMode::CArray => "c_array",
            OutputMode::Info => "info",
        }
    }
    fn max_pcm_bytes(self) -> usize {
        match self {
            OutputMode::Base64 => CAP_BASE64,
            OutputMode::Hex => CAP_HEX,
            OutputMode::CArray => CAP_C_ARRAY,
            OutputMode::Info => usize::MAX,
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum ChannelSel {
    All,
    Mono,
    Left,
    Right,
}

impl ChannelSel {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "all" => Ok(ChannelSel::All),
            "mono" => Ok(ChannelSel::Mono),
            "left" => Ok(ChannelSel::Left),
            "right" => Ok(ChannelSel::Right),
            other => Err(format!(
                "invalid channels {other:?}: expected \"all\", \"mono\", \
                 \"left\", or \"right\""
            )),
        }
    }
    fn out_channels(self, source: u16) -> Result<u16, String> {
        match self {
            ChannelSel::All => Ok(source),
            ChannelSel::Mono | ChannelSel::Left => Ok(1),
            ChannelSel::Right => {
                if source < 2 {
                    Err(format!(
                        "channels=\"right\" needs at least 2 channels, but this \
                         clip has {source} — use \"left\" or \"all\""
                    ))
                } else {
                    Ok(1)
                }
            }
        }
    }
    fn label(self) -> &'static str {
        match self {
            ChannelSel::All => "as stored",
            ChannelSel::Mono => "mono downmix",
            ChannelSel::Left => "channel 1 only",
            ChannelSel::Right => "channel 2 only",
        }
    }
}

/// Sample encodings this tool can read from a WAV and write to raw PCM.
#[derive(Clone, Copy, PartialEq)]
enum Encoding {
    U8,
    S16,
    S24,
    S32,
    F32,
    F64,
}

#[derive(Clone, Copy, PartialEq)]
enum Endian {
    Le,
    Be,
}

#[derive(Clone, Copy)]
struct Target {
    enc: Encoding,
    endian: Endian,
}

impl Target {
    fn bytes(self) -> usize {
        match self.enc {
            Encoding::U8 => 1,
            Encoding::S16 => 2,
            Encoding::S24 => 3,
            Encoding::S32 | Encoding::F32 => 4,
            Encoding::F64 => 8,
        }
    }
    fn bits(self) -> u16 {
        (self.bytes() * 8) as u16
    }
    /// ffmpeg / SoX raw-format name, e.g. `s16le`.
    fn name(self) -> &'static str {
        match (self.enc, self.endian) {
            (Encoding::U8, _) => "u8",
            (Encoding::S16, Endian::Le) => "s16le",
            (Encoding::S16, Endian::Be) => "s16be",
            (Encoding::S24, Endian::Le) => "s24le",
            (Encoding::S24, Endian::Be) => "s24be",
            (Encoding::S32, Endian::Le) => "s32le",
            (Encoding::S32, Endian::Be) => "s32be",
            (Encoding::F32, Endian::Le) => "f32le",
            (Encoding::F32, Endian::Be) => "f32be",
            (Encoding::F64, Endian::Le) => "f64le",
            (Encoding::F64, Endian::Be) => "f64be",
        }
    }
    fn long_name(self) -> String {
        let order = match (self.enc, self.endian) {
            (Encoding::U8, _) => "",
            (_, Endian::Le) => " little-endian",
            (_, Endian::Be) => " big-endian",
        };
        let kind = match self.enc {
            Encoding::U8 => "unsigned",
            Encoding::S16 | Encoding::S24 | Encoding::S32 => "signed",
            Encoding::F32 | Encoding::F64 => "IEEE float",
        };
        format!("{kind} {}-bit{order}", self.bits())
    }
    /// SoX `-e` encoding name.
    fn sox_encoding(self) -> &'static str {
        match self.enc {
            Encoding::U8 => "unsigned-integer",
            Encoding::S16 | Encoding::S24 | Encoding::S32 => "signed-integer",
            Encoding::F32 | Encoding::F64 => "floating-point",
        }
    }
    /// Audacity "Import Raw Data" encoding label.
    fn audacity_encoding(self) -> String {
        match self.enc {
            Encoding::U8 => "Unsigned 8-bit PCM".into(),
            Encoding::F32 => "32-bit float".into(),
            Encoding::F64 => "64-bit float".into(),
            _ => format!("Signed {}-bit PCM", self.bits()),
        }
    }
}

fn sample_format_is_source(s: &str) -> bool {
    matches!(s.trim(), "" | "source")
}

/// `None` = "source" (derive the target from the file's own `fmt ` chunk).
fn parse_sample_format(s: &str) -> Result<Option<Target>, String> {
    let t = |enc, endian| Ok(Some(Target { enc, endian }));
    match s.trim() {
        "" | "source" => Ok(None),
        "u8" => t(Encoding::U8, Endian::Le),
        "s16le" => t(Encoding::S16, Endian::Le),
        "s16be" => t(Encoding::S16, Endian::Be),
        "s24le" => t(Encoding::S24, Endian::Le),
        "s24be" => t(Encoding::S24, Endian::Be),
        "s32le" => t(Encoding::S32, Endian::Le),
        "s32be" => t(Encoding::S32, Endian::Be),
        "f32le" => t(Encoding::F32, Endian::Le),
        "f32be" => t(Encoding::F32, Endian::Be),
        other => Err(format!(
            "invalid sample_format {other:?}: expected \"source\", \"u8\", \
             \"s16le\", \"s16be\", \"s24le\", \"s24be\", \"s32le\", \"s32be\", \
             \"f32le\", or \"f32be\""
        )),
    }
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

fn extract(
    wav: &Wav,
    target: Target,
    sel: ChannelSel,
    verbatim: bool,
    start_frame: u64,
    end_frame: u64,
    out_len: usize,
) -> Vec<u8> {
    let ba = wav.block_align as usize;
    let start = start_frame as usize * ba;
    let end = end_frame as usize * ba;
    if verbatim {
        return wav.data[start..end].to_vec();
    }

    let enc = wav.source_encoding().expect("checked by run()");
    let src_bytes = Target {
        enc,
        endian: Endian::Le,
    }
    .bytes();
    let nch = wav.channels as usize;
    let mut out = Vec::with_capacity(out_len);
    for f in start_frame as usize..end_frame as usize {
        let base = f * ba;
        let sample = |c: usize| decode_sample(&wav.data[base + c * src_bytes..], enc);
        match sel {
            ChannelSel::All => {
                for c in 0..nch {
                    push_sample(&mut out, sample(c), target);
                }
            }
            ChannelSel::Mono => {
                let sum: f64 = (0..nch).map(sample).sum();
                push_sample(&mut out, sum / nch as f64, target);
            }
            ChannelSel::Left => push_sample(&mut out, sample(0), target),
            ChannelSel::Right => push_sample(&mut out, sample(1), target),
        }
    }
    out
}

/// Read one sample as a normalized amplitude in [-1, 1].
fn decode_sample(b: &[u8], enc: Encoding) -> f64 {
    match enc {
        Encoding::U8 => (b[0] as f64 - 128.0) / 128.0,
        Encoding::S16 => i16::from_le_bytes([b[0], b[1]]) as f64 / 32_768.0,
        Encoding::S24 => {
            let raw = (b[0] as i32) | ((b[1] as i32) << 8) | ((b[2] as i32) << 16);
            (((raw << 8) >> 8) as f64) / 8_388_608.0
        }
        Encoding::S32 => i32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64 / 2_147_483_648.0,
        Encoding::F32 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
        Encoding::F64 => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
    }
}

/// Write one normalized sample in the target encoding + byte order.
fn push_sample(out: &mut Vec<u8>, v: f64, target: Target) {
    let be = target.endian == Endian::Be;
    let mut push = |bytes: &[u8]| {
        if be {
            out.extend(bytes.iter().rev());
        } else {
            out.extend_from_slice(bytes);
        }
    };
    match target.enc {
        Encoding::U8 => {
            let q = (v * 128.0).round().clamp(-128.0, 127.0) as i32;
            out.push((q + 128) as u8);
        }
        Encoding::S16 => {
            let q = (v * 32_768.0).round().clamp(-32_768.0, 32_767.0) as i16;
            push(&q.to_le_bytes());
        }
        Encoding::S24 => {
            let q = (v * 8_388_608.0).round().clamp(-8_388_608.0, 8_388_607.0) as i32;
            push(&q.to_le_bytes()[..3]);
        }
        Encoding::S32 => {
            let q = (v * 2_147_483_648.0)
                .round()
                .clamp(-2_147_483_648.0, 2_147_483_647.0) as i32;
            push(&q.to_le_bytes());
        }
        Encoding::F32 => push(&(v as f32).to_le_bytes()),
        Encoding::F64 => push(&v.to_le_bytes()),
    }
}

// ---------------------------------------------------------------------------
// Output rendering
// ---------------------------------------------------------------------------

fn encode_base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

const HEX: &[u8; 16] = b"0123456789abcdef";

fn hex_byte(out: &mut String, b: u8) {
    out.push(HEX[(b >> 4) as usize] as char);
    out.push(HEX[(b & 15) as usize] as char);
}

/// `line_bytes = 0` → one unbroken lowercase hex run (what `xxd -r -p` wants);
/// otherwise space-separated byte pairs, `line_bytes` per line.
fn encode_hex(bytes: &[u8], line_bytes: u32) -> String {
    if line_bytes == 0 {
        let mut out = String::with_capacity(bytes.len() * 2);
        for &b in bytes {
            hex_byte(&mut out, b);
        }
        return out;
    }
    let per = line_bytes as usize;
    let mut out = String::with_capacity(bytes.len() * 3 + bytes.len() / per + 1);
    for (i, &b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(if i % per == 0 { '\n' } else { ' ' });
        }
        hex_byte(&mut out, b);
    }
    out
}

fn encode_c_array(
    bytes: &[u8],
    line_bytes: u32,
    wav: &Wav,
    target: Target,
    out_channels: u16,
) -> String {
    let per = if line_bytes == 0 {
        bytes.len().max(1)
    } else {
        line_bytes as usize
    };
    let mut out = String::with_capacity(bytes.len() * 6 + 200);
    out.push_str(&format!(
        "/* raw PCM: {}, {} channel{}, {} Hz */\n",
        target.name(),
        out_channels,
        if out_channels == 1 { "" } else { "s" },
        wav.sample_rate
    ));
    out.push_str("const unsigned char pcm_data[] = {\n");
    for (i, &b) in bytes.iter().enumerate() {
        if i % per == 0 {
            out.push_str("  ");
        }
        out.push_str("0x");
        hex_byte(&mut out, b);
        if i + 1 < bytes.len() {
            out.push(',');
        }
        if (i + 1) % per == 0 || i + 1 == bytes.len() {
            out.push('\n');
        } else {
            out.push(' ');
        }
    }
    out.push_str("};\n");
    out.push_str(&format!(
        "const unsigned int pcm_data_len = {};\n",
        bytes.len()
    ));
    out
}

fn row(out: &mut String, label: &str, value: &str) {
    out.push_str(&format!("  {label:<16}{value}\n"));
}

fn render_info(
    file: &[u8],
    wav: &Wav,
    target: Target,
    sel: ChannelSel,
    verbatim: bool,
    start_frame: u64,
    end_frame: u64,
    out_channels: u16,
    out_len: usize,
) -> String {
    let total_frames = wav.total_frames();
    let mut out = String::with_capacity(1024);

    out.push_str("Source WAV\n");
    row(&mut out, "file bytes", &file.len().to_string());
    row(&mut out, "codec", &wav.codec_label());
    row(&mut out, "sample rate", &format!("{} Hz", wav.sample_rate));
    row(&mut out, "channels", &wav.channels.to_string());
    row(&mut out, "bit depth", &format!("{}-bit", wav.bits_per_sample));
    row(
        &mut out,
        "block align",
        &format!("{} bytes per frame", wav.block_align),
    );
    row(&mut out, "total frames", &total_frames.to_string());
    row(
        &mut out,
        "duration",
        &format!("{:.6} s", total_frames as f64 / wav.sample_rate as f64),
    );
    row(&mut out, "chunks", &wav.chunk_summary());
    row(
        &mut out,
        "data chunk",
        &format!("offset {}, {} bytes", wav.data_offset, wav.data.len()),
    );

    out.push_str("\nExtracted PCM\n");
    row(
        &mut out,
        "encoding",
        &format!(
            "{} ({}){}",
            target.name(),
            target.long_name(),
            if verbatim { ", verbatim payload" } else { "" }
        ),
    );
    row(
        &mut out,
        "channels",
        &format!("{out_channels} interleaved ({})", sel.label()),
    );
    row(
        &mut out,
        "frames",
        &format!(
            "{} of {total_frames} (index {start_frame} - {})",
            end_frame - start_frame,
            end_frame - 1
        ),
    );
    row(&mut out, "bytes", &out_len.to_string());

    out.push_str("\nRe-import (raw PCM has no header — state the format yourself)\n");
    row(
        &mut out,
        "ffmpeg",
        &format!(
            "ffmpeg -f {} -ar {} -ac {out_channels} -i out.pcm out.wav",
            target.name(),
            wav.sample_rate
        ),
    );
    let byte_order = match (target.enc, target.endian) {
        (Encoding::U8, _) => "",
        (_, Endian::Le) => " -L",
        (_, Endian::Be) => " -B",
    };
    row(
        &mut out,
        "sox",
        &format!(
            "sox -t raw -e {} -b {}{byte_order} -r {} -c {out_channels} out.pcm out.wav",
            target.sox_encoding(),
            target.bits(),
            wav.sample_rate
        ),
    );
    let audacity_order = match (target.enc, target.endian) {
        (Encoding::U8, _) => String::new(),
        (_, Endian::Le) => ", Little-endian".into(),
        (_, Endian::Be) => ", Big-endian".into(),
    };
    row(
        &mut out,
        "Audacity",
        &format!(
            "Import > Raw Data: {}{audacity_order}, {out_channels} channel{}, {} Hz",
            target.audacity_encoding(),
            if out_channels == 1 { "" } else { "s" },
            wav.sample_rate
        ),
    );
    out
}

// ---------------------------------------------------------------------------
// WAV parsing (chunk walk — we want the data chunk's bytes, not its samples)
// ---------------------------------------------------------------------------

struct Wav<'a> {
    audio_format: u16,
    channels: u16,
    sample_rate: u32,
    bits_per_sample: u16,
    block_align: u16,
    data: &'a [u8],
    data_offset: usize,
    chunks: Vec<(String, usize)>,
}

const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_ALAW: u16 = 0x0006;
const WAVE_FORMAT_MULAW: u16 = 0x0007;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

impl Wav<'_> {
    fn total_frames(&self) -> u64 {
        if self.block_align == 0 {
            0
        } else {
            (self.data.len() / self.block_align as usize) as u64
        }
    }
    /// The sample encoding of the stored payload, or `None` when the payload is
    /// companded/compressed and can only be passed through verbatim.
    fn source_encoding(&self) -> Option<Encoding> {
        match (self.audio_format, self.bits_per_sample) {
            (WAVE_FORMAT_PCM, 8) => Some(Encoding::U8),
            (WAVE_FORMAT_PCM, 16) => Some(Encoding::S16),
            (WAVE_FORMAT_PCM, 24) => Some(Encoding::S24),
            (WAVE_FORMAT_PCM, 32) => Some(Encoding::S32),
            (WAVE_FORMAT_IEEE_FLOAT, 32) => Some(Encoding::F32),
            (WAVE_FORMAT_IEEE_FLOAT, 64) => Some(Encoding::F64),
            _ => None,
        }
    }
    fn codec_label(&self) -> String {
        let named = match self.audio_format {
            WAVE_FORMAT_PCM => "PCM integer",
            WAVE_FORMAT_IEEE_FLOAT => "IEEE float",
            WAVE_FORMAT_ALAW => "A-law companded",
            WAVE_FORMAT_MULAW => "mu-law companded",
            _ => "unknown codec",
        };
        format!("{named} (format tag 0x{:04x})", self.audio_format)
    }
    fn chunk_summary(&self) -> String {
        self.chunks
            .iter()
            .map(|(id, size)| format!("{id} ({size} B)"))
            .collect::<Vec<_>>()
            .join(", ")
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

fn parse_wav(b: &[u8]) -> Result<Wav<'_>, String> {
    // Sniff BEFORE the length check so a truncated MP3/FLAC/Ogg paste is still
    // named by its real format instead of the generic "too short".
    if b.len() < 4 || &b[0..4] != b"RIFF" {
        return Err(sniff_container(b));
    }
    if b.len() < 12 {
        return Err(format!(
            "not a WAV file: only {} bytes, too short for a RIFF header",
            b.len()
        ));
    }
    if &b[8..12] != b"WAVE" {
        return Err("not a WAV file: the RIFF header is not of type WAVE".into());
    }

    let mut pos = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16, u16)> = None;
    let mut data: Option<(usize, &[u8])> = None;
    let mut chunks: Vec<(String, usize)> = Vec::new();

    while pos + 8 <= b.len() {
        let id = &b[pos..pos + 4];
        let size = u32_le(b, pos + 4) as usize;
        let body_start = pos + 8;
        let body_end = body_start.saturating_add(size).min(b.len());
        let body = &b[body_start..body_end];
        chunks.push((chunk_id(id), size));
        match id {
            b"fmt " => fmt = Some(parse_fmt(body)?),
            b"data" if data.is_none() => data = Some((body_start, body)),
            _ => {}
        }
        let advance = 8 + size + (size & 1);
        pos = match pos.checked_add(advance) {
            Some(p) => p,
            None => break,
        };
    }

    let (audio_format, channels, sample_rate, bits_per_sample, fmt_block_align) = fmt.ok_or(
        "malformed WAV: no `fmt ` chunk, so the sample layout is unknown",
    )?;
    let (data_offset, data) =
        data.ok_or("malformed WAV: no `data` chunk, so there are no sample bytes to extract")?;

    if channels == 0 {
        return Err("malformed WAV: the `fmt ` chunk declares 0 channels".into());
    }
    if sample_rate == 0 {
        return Err("malformed WAV: the `fmt ` chunk declares a sample rate of 0".into());
    }
    if bits_per_sample == 0 || bits_per_sample % 8 != 0 {
        return Err(format!(
            "unsupported bit depth {bits_per_sample}: only whole-byte sample \
             widths (8, 16, 24, 32, 64-bit) can be sliced out of the data chunk"
        ));
    }
    // Trust the fmt chunk's block_align when it is consistent; otherwise derive
    // it, so a writer that left the field at 0 still extracts correctly.
    let derived = channels as usize * (bits_per_sample as usize / 8);
    let block_align = if fmt_block_align as usize == derived {
        fmt_block_align
    } else {
        derived as u16
    };
    if block_align == 0 {
        return Err("malformed WAV: the frame size works out to 0 bytes".into());
    }

    Ok(Wav {
        audio_format,
        channels,
        sample_rate,
        bits_per_sample,
        block_align,
        data,
        data_offset,
        chunks,
    })
}

/// Render a four-character chunk id for the report, escaping anything that is
/// not printable ASCII so a corrupt file can't emit control characters.
fn chunk_id(id: &[u8]) -> String {
    id.iter()
        .map(|&c| {
            if (0x20..0x7f).contains(&c) {
                c as char
            } else {
                '.'
            }
        })
        .collect::<String>()
        .trim_end()
        .to_string()
}

/// Name a non-RIFF container so the error names the format instead of a generic
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
    } else if starts(b"RIFX") {
        Some("a big-endian RIFX file")
    } else {
        None
    };
    match named {
        Some(what) => format!(
            "unsupported format: this looks like {what}, but only RIFF/WAVE files \
             have a `data` chunk to strip. Convert it to WAV first, e.g. \
             `ffmpeg -i clip.mp3 clip.wav`."
        ),
        None => "not a WAV file: the `RIFF` signature is missing. Paste an \
                 uncompressed RIFF/WAVE file; convert other formats with \
                 `ffmpeg -i clip.<ext> clip.wav` first."
            .into(),
    }
}

fn parse_fmt(body: &[u8]) -> Result<(u16, u16, u32, u16, u16), String> {
    if body.len() < 16 {
        return Err(format!(
            "malformed WAV: the `fmt ` chunk is {} bytes, but at least 16 are required",
            body.len()
        ));
    }
    let mut audio_format = u16_le(body, 0);
    let channels = u16_le(body, 2);
    let sample_rate = u32_le(body, 4);
    let block_align = u16_le(body, 12);
    let bits_per_sample = u16_le(body, 14);

    // WAVE_FORMAT_EXTENSIBLE hides the real tag in the first 2 bytes of the
    // SubFormat GUID inside the extension block.
    if audio_format == WAVE_FORMAT_EXTENSIBLE && body.len() >= 26 {
        let ext_size = u16_le(body, 16) as usize;
        if ext_size >= 22 && body.len() >= 18 + 22 {
            audio_format = u16_le(body, 24);
        }
    }
    Ok((
        audio_format,
        channels,
        sample_rate,
        bits_per_sample,
        block_align,
    ))
}

// ---------------------------------------------------------------------------
// Byte decoding (base64 / hex)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the WAV file bytes as base64 or hex".into());
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
        return Err(format!(
            "invalid hex: {} digits is an odd count — every byte needs two",
            compact.len()
        ));
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
        _ => Err(format!(
            "invalid hex digit {:?}: expected 0-9, a-f (whitespace, ':' and '-' are ignored)",
            c as char
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
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!(
                "invalid base64 character {:?}: expected A-Z, a-z, 0-9, '+', '/' \
                 (or the URL-safe '-' and '_')",
                c as char
            ));
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

    /// 16 kHz mono 16-bit WAV whose three frames are 16384, -8192, 0.
    const MONO16: &str = "UklGRi4AAABXQVZFZm10IBAAAAABAAEAgD4AAAB9AAACABAAZGF0YQYAAAAAQADgAAA=";
    /// 16 kHz stereo 16-bit WAV, two frames: (16384, -16384) and (0, 32767).
    const STEREO16: &str = "UklGRjAAAABXQVZFZm10IBAAAAABAAIAgD4AAAD6AAAEABAAZGF0YQgAAAAAQADAAAD/fw==";

    fn wav_bytes(sample_rate: u32, channels: u16, bits: u16, tag: u16, data: &[u8]) -> Vec<u8> {
        let block_align = channels * (bits / 8);
        let byte_rate = sample_rate * block_align as u32;
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&(36u32 + data.len() as u32).to_le_bytes());
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&tag.to_le_bytes());
        b.extend_from_slice(&channels.to_le_bytes());
        b.extend_from_slice(&sample_rate.to_le_bytes());
        b.extend_from_slice(&byte_rate.to_le_bytes());
        b.extend_from_slice(&block_align.to_le_bytes());
        b.extend_from_slice(&bits.to_le_bytes());
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(data);
        b
    }

    fn hex_of(v: &[u8]) -> String {
        let mut s = String::new();
        for &b in v {
            hex_byte(&mut s, b);
        }
        s
    }

    fn run_hex(input: &str, sample_format: &str, channels: &str) -> String {
        run(input, "base64", "hex", sample_format, channels, 0, 0, 0).unwrap()
    }

    // --- happy paths -------------------------------------------------------

    #[test]
    fn strips_header_and_returns_payload_base64() {
        // The whole file is 50 bytes; the payload is the last 6.
        let out = run(MONO16, "base64", "base64", "source", "all", 0, 0, 16).unwrap();
        assert_eq!(out, "AEAA4AAA");
    }

    #[test]
    fn verbatim_payload_matches_the_data_chunk_bytes() {
        assert_eq!(run_hex(MONO16, "source", "all"), "004000e00000");
        assert_eq!(run_hex(STEREO16, "source", "all"), "004000c00000ff7f");
    }

    #[test]
    fn skips_extra_chunks_before_and_after_data() {
        // LIST chunk before `data`, and a trailing `cue ` chunk after it.
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF");
        b.extend_from_slice(&0u32.to_le_bytes()); // size field is not trusted
        b.extend_from_slice(b"WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&8000u32.to_le_bytes());
        b.extend_from_slice(&16000u32.to_le_bytes());
        b.extend_from_slice(&2u16.to_le_bytes());
        b.extend_from_slice(&16u16.to_le_bytes());
        b.extend_from_slice(b"LIST");
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(b"INFO");
        b.extend_from_slice(b"data");
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(&[0x11, 0x22, 0x33, 0x44]);
        b.extend_from_slice(b"cue ");
        b.extend_from_slice(&4u32.to_le_bytes());
        b.extend_from_slice(&[0, 0, 0, 0]);
        let input = encode_base64(&b);
        assert_eq!(
            run(&input, "base64", "hex", "source", "all", 0, 0, 0).unwrap(),
            "11223344"
        );
    }

    #[test]
    fn odd_sized_chunk_is_word_aligned_before_data() {
        // A 3-byte chunk is followed by a pad byte; miscounting it would slice
        // the data chunk from the wrong offset.
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF\0\0\0\0WAVEfmt ");
        b.extend_from_slice(&16u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&8000u32.to_le_bytes());
        b.extend_from_slice(&8000u32.to_le_bytes());
        b.extend_from_slice(&1u16.to_le_bytes());
        b.extend_from_slice(&8u16.to_le_bytes());
        b.extend_from_slice(b"junk");
        b.extend_from_slice(&3u32.to_le_bytes());
        b.extend_from_slice(&[1, 2, 3, 0]); // 3 bytes + pad
        b.extend_from_slice(b"data");
        b.extend_from_slice(&2u32.to_le_bytes());
        b.extend_from_slice(&[0xaa, 0xbb]);
        let input = encode_base64(&b);
        assert_eq!(
            run(&input, "base64", "hex", "source", "all", 0, 0, 0).unwrap(),
            "aabb"
        );
    }

    #[test]
    fn hex_output_groups_by_line_bytes() {
        assert_eq!(
            run(MONO16, "base64", "hex", "source", "all", 0, 0, 4).unwrap(),
            "00 40 00 e0\n00 00"
        );
    }

    #[test]
    fn c_array_output_is_compilable() {
        let out = run(MONO16, "base64", "c_array", "source", "all", 0, 0, 4).unwrap();
        assert_eq!(
            out,
            "/* raw PCM: s16le, 1 channel, 16000 Hz */\n\
             const unsigned char pcm_data[] = {\n  \
             0x00, 0x40, 0x00, 0xe0,\n  0x00, 0x00\n};\n\
             const unsigned int pcm_data_len = 6;\n"
        );
    }

    #[test]
    fn frame_window_selects_a_slice() {
        // Frame 1 only, of the three mono frames.
        assert_eq!(
            run(MONO16, "base64", "hex", "source", "all", 1, 1, 0).unwrap(),
            "00e0"
        );
        // max_frames = 0 means "to the end".
        assert_eq!(
            run(MONO16, "base64", "hex", "source", "all", 1, 0, 0).unwrap(),
            "00e00000"
        );
    }

    #[test]
    fn hex_input_is_accepted() {
        let raw = decode_base64(MONO16).unwrap();
        let as_hex = hex_of(&raw);
        assert_eq!(
            run(&as_hex, "hex", "hex", "source", "all", 0, 0, 0).unwrap(),
            "004000e00000"
        );
    }

    // --- conversions -------------------------------------------------------

    #[test]
    fn converts_16_bit_to_8_bit_and_32_bit() {
        // 16384 -> 0.5 -> u8 192; -8192 -> -0.25 -> u8 96; 0 -> 128.
        assert_eq!(run_hex(MONO16, "u8", "all"), "c06080");
        // s32le: 0.5 * 2^31 = 0x40000000.
        assert_eq!(run_hex(MONO16, "s32le", "all"), "00000040000000e000000000");
        // s24le: 0.5 * 2^23 = 0x400000, little-endian → 00 00 40.
        assert_eq!(
            run_hex(MONO16, "s24le", "all"),
            "000040 0000e0 000000".replace(' ', "")
        );
    }

    #[test]
    fn big_endian_reverses_each_sample() {
        assert_eq!(run_hex(MONO16, "s16le", "all"), "004000e00000");
        assert_eq!(run_hex(MONO16, "s16be", "all"), "4000e0000000");
        assert_eq!(run_hex(MONO16, "s32be", "all"), "40000000e000000000000000");
    }

    #[test]
    fn float_output_is_ieee_little_endian() {
        // 0.5f = 0x3f000000, -0.25f = 0xbe800000, 0.0f = 0x00000000.
        assert_eq!(run_hex(MONO16, "f32le", "all"), "0000003f000080be00000000");
        assert_eq!(run_hex(MONO16, "f32be", "all"), "3f000000be80000000000000");
    }

    #[test]
    fn source_round_trip_is_byte_identical() {
        // Re-encoding to the source's own format must reproduce the payload,
        // including the -1.0 full-scale edge (0x8000).
        let data: Vec<u8> = vec![0x00, 0x80, 0xff, 0x7f, 0x01, 0x00];
        let wav = wav_bytes(44100, 1, 16, WAVE_FORMAT_PCM, &data);
        let input = encode_base64(&wav);
        // channels="mono" on a mono clip forces the decode/re-encode path.
        assert_eq!(
            run(&input, "base64", "hex", "s16le", "mono", 0, 0, 0).unwrap(),
            hex_of(&data)
        );
    }

    #[test]
    fn channel_selection_splits_a_stereo_clip() {
        // Frames: (16384, -16384), (0, 32767).
        assert_eq!(run_hex(STEREO16, "source", "left"), "00400000");
        assert_eq!(run_hex(STEREO16, "source", "right"), "00c0ff7f");
        // mono = average: (16384 + -16384)/2 = 0; (0 + 32767)/2 = 16383.5 which
        // rounds away from zero to 16384 = 0x4000.
        assert_eq!(run_hex(STEREO16, "source", "mono"), "00000040");
    }

    #[test]
    fn eight_bit_wav_payload_round_trips() {
        let data: Vec<u8> = vec![0x00, 0x80, 0xff, 0x40];
        let wav = wav_bytes(8000, 1, 8, WAVE_FORMAT_PCM, &data);
        let input = encode_base64(&wav);
        assert_eq!(
            run(&input, "base64", "hex", "u8", "mono", 0, 0, 0).unwrap(),
            "0080ff40"
        );
    }

    #[test]
    fn float_wav_source_is_passed_through_verbatim() {
        let mut data = Vec::new();
        for v in [0.5f32, -0.25] {
            data.extend_from_slice(&v.to_le_bytes());
        }
        let wav = wav_bytes(48000, 1, 32, WAVE_FORMAT_IEEE_FLOAT, &data);
        let input = encode_base64(&wav);
        assert_eq!(
            run(&input, "base64", "hex", "source", "all", 0, 0, 0).unwrap(),
            hex_of(&data)
        );
        // ...and converts down to 16-bit on request.
        assert_eq!(
            run(&input, "base64", "hex", "s16le", "all", 0, 0, 0).unwrap(),
            "004000e0"
        );
    }

    #[test]
    fn alaw_payload_is_extractable_verbatim_only() {
        let data: Vec<u8> = vec![0xd5, 0x55, 0x2a];
        let wav = wav_bytes(8000, 1, 8, WAVE_FORMAT_ALAW, &data);
        let input = encode_base64(&wav);
        assert_eq!(
            run(&input, "base64", "hex", "source", "all", 0, 0, 0).unwrap(),
            "d5552a"
        );
        let err = run(&input, "base64", "hex", "s16le", "all", 0, 0, 0).unwrap_err();
        assert!(err.contains("A-law"), "unexpected error: {err}");
        assert!(err.contains("verbatim"), "unexpected error: {err}");
    }

    #[test]
    fn extensible_wav_resolves_the_real_format_tag() {
        // WAVE_FORMAT_EXTENSIBLE with a PCM SubFormat GUID.
        let data: Vec<u8> = vec![0x11, 0x22];
        let mut fmt = Vec::new();
        fmt.extend_from_slice(&WAVE_FORMAT_EXTENSIBLE.to_le_bytes());
        fmt.extend_from_slice(&1u16.to_le_bytes()); // channels
        fmt.extend_from_slice(&44100u32.to_le_bytes());
        fmt.extend_from_slice(&88200u32.to_le_bytes());
        fmt.extend_from_slice(&2u16.to_le_bytes()); // block align
        fmt.extend_from_slice(&16u16.to_le_bytes()); // bits
        fmt.extend_from_slice(&22u16.to_le_bytes()); // cbSize
        fmt.extend_from_slice(&16u16.to_le_bytes()); // valid bits
        fmt.extend_from_slice(&4u32.to_le_bytes()); // channel mask
        fmt.extend_from_slice(&WAVE_FORMAT_PCM.to_le_bytes()); // SubFormat GUID head
        fmt.extend_from_slice(&[0u8; 14]);
        let mut b = Vec::new();
        b.extend_from_slice(b"RIFF\0\0\0\0WAVEfmt ");
        b.extend_from_slice(&(fmt.len() as u32).to_le_bytes());
        b.extend_from_slice(&fmt);
        b.extend_from_slice(b"data");
        b.extend_from_slice(&(data.len() as u32).to_le_bytes());
        b.extend_from_slice(&data);
        let input = encode_base64(&b);
        // s16be proves the tag resolved to PCM (a decode happened).
        assert_eq!(
            run(&input, "base64", "hex", "s16be", "all", 0, 0, 0).unwrap(),
            "2211"
        );
    }

    #[test]
    fn info_reports_the_layout_and_reimport_commands() {
        let out = run(MONO16, "base64", "info", "source", "all", 0, 0, 0).unwrap();
        assert!(out.contains("codec           PCM integer (format tag 0x0001)"));
        assert!(out.contains("sample rate     16000 Hz"));
        assert!(out.contains("chunks          fmt (16 B), data (6 B)"));
        assert!(out.contains("data chunk      offset 44, 6 bytes"));
        assert!(out.contains("encoding        s16le (signed 16-bit little-endian), verbatim payload"));
        assert!(out.contains("bytes           6"));
        assert!(out.contains("ffmpeg -f s16le -ar 16000 -ac 1 -i out.pcm out.wav"));
        assert!(out.contains("sox -t raw -e signed-integer -b 16 -L -r 16000 -c 1 out.pcm out.wav"));
        assert!(out.contains("Signed 16-bit PCM, Little-endian, 1 channel, 16000 Hz"));
    }

    #[test]
    fn info_reflects_a_converted_target() {
        let out = run(STEREO16, "base64", "info", "u8", "mono", 0, 0, 0).unwrap();
        assert!(out.contains("encoding        u8 (unsigned 8-bit)"));
        assert!(out.contains("channels        1 interleaved (mono downmix)"));
        assert!(out.contains("bytes           2"));
        assert!(out.contains("ffmpeg -f u8 -ar 16000 -ac 1 -i out.pcm out.wav"));
        assert!(out.contains("sox -t raw -e unsigned-integer -b 8 -r 16000 -c 1 out.pcm out.wav"));
        assert!(out.contains("Unsigned 8-bit PCM, 1 channel, 16000 Hz"));
    }

    // --- errors ------------------------------------------------------------

    #[test]
    fn rejects_a_non_wav_file() {
        let err = run("SUQzBAAAAAA=", "base64", "base64", "source", "all", 0, 0, 0).unwrap_err();
        assert!(err.contains("MP3"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = run("  ", "base64", "base64", "source", "all", 0, 0, 0).unwrap_err();
        assert!(err.contains("input is empty"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_a_start_frame_past_the_end() {
        let err = run(MONO16, "base64", "base64", "source", "all", 9, 0, 0).unwrap_err();
        assert!(err.contains("3 sample frames"), "unexpected error: {err}");
        assert!(err.contains("indices 0-2"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_right_channel_on_a_mono_clip() {
        let err = run(MONO16, "base64", "base64", "source", "right", 0, 0, 0).unwrap_err();
        assert!(err.contains("at least 2 channels"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_unknown_option_values() {
        assert!(run(MONO16, "base64", "yaml", "source", "all", 0, 0, 0)
            .unwrap_err()
            .contains("invalid output"));
        assert!(run(MONO16, "base64", "hex", "s12le", "all", 0, 0, 0)
            .unwrap_err()
            .contains("invalid sample_format"));
        assert!(run(MONO16, "base64", "hex", "source", "middle", 0, 0, 0)
            .unwrap_err()
            .contains("invalid channels"));
        assert!(run(MONO16, "base64", "hex", "source", "all", 0, 0, 999)
            .unwrap_err()
            .contains("invalid line_bytes"));
        assert!(run(MONO16, "yaml", "hex", "source", "all", 0, 0, 0)
            .unwrap_err()
            .contains("invalid input_format"));
    }

    #[test]
    fn rejects_a_wav_without_a_data_chunk() {
        let wav = wav_bytes(44100, 1, 16, WAVE_FORMAT_PCM, &[]);
        let headers_only = &wav[..36];
        let input = encode_base64(headers_only);
        let err = run(&input, "base64", "base64", "source", "all", 0, 0, 0).unwrap_err();
        assert!(err.contains("no `data` chunk"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_a_data_chunk_shorter_than_one_frame() {
        let wav = wav_bytes(44100, 2, 16, WAVE_FORMAT_PCM, &[0x01, 0x02]);
        let input = encode_base64(&wav);
        let err = run(&input, "base64", "base64", "source", "all", 0, 0, 0).unwrap_err();
        assert!(err.contains("nothing to extract"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_an_extraction_over_the_output_cap() {
        // A 3 MiB+ stereo payload is fine as base64 but over the c_array cap.
        let frames = 900_000;
        let data = vec![0u8; frames * 4];
        let wav = wav_bytes(44100, 2, 16, WAVE_FORMAT_PCM, &data);
        let input = encode_base64(&wav);
        let err = run(&input, "base64", "c_array", "source", "all", 0, 0, 16).unwrap_err();
        assert!(err.contains("over the"), "unexpected error: {err}");
        assert!(err.contains("c_array"), "unexpected error: {err}");
        assert!(err.contains("start_frame"), "unexpected error: {err}");
        // The same range fits the base64 cap.
        assert!(run(&input, "base64", "base64", "source", "all", 0, 0, 0).is_ok());
    }

    #[test]
    fn rejects_a_bad_hex_payload() {
        let err = run("0040 0g", "hex", "base64", "source", "all", 0, 0, 0).unwrap_err();
        assert!(err.contains("invalid hex digit"), "unexpected error: {err}");
    }
}
