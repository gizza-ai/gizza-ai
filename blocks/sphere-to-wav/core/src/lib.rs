//! sphere-to-wav core — parse a NIST SPHERE (`.sph`) file's ASCII header and
//! re-container its samples as a standard RIFF/WAVE file (or headerless raw
//! PCM). Pure compute, shared verbatim by the chat/CLI block and the page.
//!
//! SPHERE layout: a fixed-size ASCII header ("NIST_1A\n", the header size on
//! line 2, then `name -i|-r|-sN value` lines terminated by `end_head`), padded
//! to the declared size, followed by the interleaved sample data. Everything
//! this tool needs lives in those fields: `sample_rate`, `channel_count`,
//! `sample_n_bytes`, `sample_coding`, `sample_byte_format`, `sample_count`.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;

/// Largest decoded `.sph` payload accepted (the block runs in a 64 MiB sandbox).
pub const MAX_INPUT_BYTES: usize = 6 * 1024 * 1024;
/// Largest audio payload produced (mu-law → 16-bit PCM doubles the size).
pub const MAX_OUTPUT_BYTES: usize = 12 * 1024 * 1024;
/// Hex rendering doubles again, so it gets a tighter cap of its own.
pub const MAX_HEX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

const INPUT_FORMATS: [&str; 3] = ["auto", "base64", "hex"];
const OUTPUTS: [&str; 4] = ["data_url", "base64", "hex", "info"];
const ENCODINGS: [&str; 4] = ["pcm16", "source", "ulaw", "alaw"];
const CHANNELS: [&str; 4] = ["all", "1", "2", "mono"];
const CONTAINERS: [&str; 2] = ["wav", "raw"];
const BYTE_ORDERS: [&str; 3] = ["auto", "little", "big"];

// ---------------------------------------------------------------- header ----

/// Sample encoding declared by the header's `sample_coding` field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Coding {
    /// Linear 2's-complement PCM, `sample_n_bytes` wide.
    Pcm,
    /// G.711 mu-law (`ulaw`, `mu-law`, `pculaw`), always 1 byte.
    Ulaw,
    /// G.711 A-law, always 1 byte.
    Alaw,
}

/// One `name -type value` line, kept in file order for the `info` report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub kind: String,
    pub value: String,
}

/// The parsed SPHERE header plus the derived audio properties.
#[derive(Debug, Clone)]
pub struct Header {
    pub fields: Vec<Field>,
    pub header_bytes: usize,
    pub sample_rate: u32,
    pub channel_count: usize,
    pub sample_n_bytes: usize,
    pub sample_count: Option<u64>,
    pub coding_raw: String,
    pub coding: Coding,
    pub byte_format: Option<String>,
    pub sig_bits: Option<u32>,
}

impl Header {
    /// Value of a header field by name, if present.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|f| f.name == name)
            .map(|f| f.value.as_str())
    }
}

/// Parse the ASCII header. Returns an error naming the offending line/field.
pub fn parse_header(bytes: &[u8]) -> Result<Header, String> {
    if bytes.len() < 16 {
        return Err(format!(
            "not a NIST SPHERE file: expected at least 16 bytes, got {}",
            bytes.len()
        ));
    }
    if !bytes.starts_with(b"NIST_1A") {
        let seen: String = bytes
            .iter()
            .take(7)
            .map(|&b| {
                if (0x20..0x7f).contains(&b) {
                    b as char
                } else {
                    '.'
                }
            })
            .collect();
        return Err(format!(
            "not a NIST SPHERE file: expected the magic \"NIST_1A\" at byte 0, got \"{seen}\". \
             Check that the bytes really are a .sph file and that base64/hex decoding picked the \
             right encoding."
        ));
    }

    // The header is ASCII; only scan the first 1 MiB so a corrupt file can't
    // turn into an unbounded string allocation.
    let scan = &bytes[..bytes.len().min(1024 * 1024)];
    let text = String::from_utf8_lossy(scan);
    let mut lines = text.split('\n');
    lines.next(); // "NIST_1A"

    let size_line = lines
        .next()
        .ok_or("truncated SPHERE header: the header-size line (line 2) is missing")?;
    let header_bytes: usize = size_line.trim().parse().map_err(|_| {
        format!(
            "invalid SPHERE header size on line 2: expected an integer byte count \
             (usually 1024), got \"{}\"",
            size_line.trim()
        )
    })?;
    if header_bytes == 0 || header_bytes > bytes.len() {
        return Err(format!(
            "SPHERE header declares a {header_bytes}-byte header but the file is only {} bytes",
            bytes.len()
        ));
    }

    let mut fields: Vec<Field> = Vec::new();
    let mut saw_end_head = false;
    let mut consumed = "NIST_1A\n".len() + size_line.len() + 1;
    for line in lines {
        let line_len = line.len() + 1;
        if consumed >= header_bytes {
            break;
        }
        consumed += line_len;
        let trimmed = line.trim_end_matches('\r');
        let t = trimmed.trim();
        if t == "end_head" {
            saw_end_head = true;
            break;
        }
        if t.is_empty() || t.starts_with(';') {
            continue;
        }
        let mut it = t.splitn(2, ' ');
        let name = it.next().unwrap_or("").to_string();
        let rest = it.next().unwrap_or("").trim_start();
        let mut parts = rest.splitn(2, ' ');
        let kind = parts.next().unwrap_or("").to_string();
        let raw_value = parts.next().unwrap_or("");
        if !kind.starts_with('-') {
            return Err(format!(
                "malformed SPHERE header field \"{name}\": expected a type token \
                 (-i, -r or -sN) after the field name, got \"{kind}\""
            ));
        }
        // `-sN` declares an N-character string; `-i`/`-r` run to end of line.
        let value = if let Some(n) = kind.strip_prefix("-s") {
            match n.parse::<usize>() {
                Ok(n) => raw_value.chars().take(n).collect::<String>(),
                Err(_) => raw_value.trim().to_string(),
            }
        } else {
            raw_value.trim().to_string()
        };
        fields.push(Field { name, kind, value });
    }
    if !saw_end_head {
        return Err(
            "truncated SPHERE header: no \"end_head\" line was found before the declared \
             header size"
                .into(),
        );
    }

    let get = |name: &str| fields.iter().find(|f| f.name == name).map(|f| &f.value);
    let int = |name: &str| -> Result<Option<u64>, String> {
        match get(name) {
            None => Ok(None),
            Some(v) => v
                .trim()
                .parse::<u64>()
                .map(Some)
                .map_err(|_| format!("SPHERE header field {name} is not an integer: \"{v}\"")),
        }
    };

    let sample_rate = int("sample_rate")?.ok_or(
        "SPHERE header is missing sample_rate — the sample rate cannot be guessed from the data",
    )? as u32;
    if sample_rate == 0 {
        return Err("SPHERE header field sample_rate is 0; expected a positive rate".into());
    }
    let channel_count = int("channel_count")?.unwrap_or(1) as usize;
    if channel_count == 0 {
        return Err("SPHERE header field channel_count is 0; expected 1 or more".into());
    }
    let coding_raw = get("sample_coding")
        .cloned()
        .unwrap_or_else(|| "pcm".to_string());
    let coding_lower = coding_raw.to_ascii_lowercase();
    if coding_lower.contains("shorten") {
        return Err(format!(
            "sample_coding is \"{coding_raw}\": this file's samples are shorten-compressed. \
             Decompress it first (for example with the reference sph2pipe converter, which \
             bundles a shorten decoder) and convert the resulting uncompressed .sph here."
        ));
    }
    let coding = match coding_lower.split(',').next().unwrap_or("").trim() {
        "pcm" | "" | "linear" => Coding::Pcm,
        "ulaw" | "mu-law" | "mulaw" | "pculaw" => Coding::Ulaw,
        "alaw" | "a-law" | "pcalaw" => Coding::Alaw,
        other => {
            return Err(format!(
                "unsupported sample_coding \"{other}\": this tool handles pcm, ulaw (mu-law) \
                 and alaw"
            ))
        }
    };
    let default_bytes = if coding == Coding::Pcm { 2 } else { 1 };
    let sample_n_bytes = int("sample_n_bytes")?.unwrap_or(default_bytes) as usize;
    if coding != Coding::Pcm && sample_n_bytes != 1 {
        return Err(format!(
            "sample_coding {coding_raw} implies 1 byte per sample but sample_n_bytes is \
             {sample_n_bytes}"
        ));
    }
    if !(1..=4).contains(&sample_n_bytes) {
        return Err(format!(
            "unsupported sample_n_bytes {sample_n_bytes}: expected 1, 2, 3 or 4"
        ));
    }

    let byte_format = get("sample_byte_format").cloned();
    let sample_count = int("sample_count")?;
    let sig_bits = int("sample_sig_bits")?.map(|v| v as u32);

    Ok(Header {
        fields,
        header_bytes,
        sample_rate,
        channel_count,
        sample_n_bytes,
        sample_count,
        coding_raw,
        coding,
        byte_format,
        sig_bits,
    })
}

// --------------------------------------------------------------- G.711 -----

/// Decode one G.711 mu-law byte to a 16-bit sample.
pub fn ulaw_to_pcm16(u: u8) -> i16 {
    let u = !u;
    let sign = u & 0x80;
    let exponent = ((u >> 4) & 0x07) as u32;
    let mantissa = (u & 0x0f) as i32;
    let magnitude = (((mantissa << 3) + 0x84) << exponent) - 0x84;
    if sign != 0 {
        -magnitude as i16
    } else {
        magnitude as i16
    }
}

/// Encode a 16-bit sample as G.711 mu-law (the standard segmented companding).
pub fn pcm16_to_ulaw(pcm: i16) -> u8 {
    const SEG_END: [i32; 8] = [0x3f, 0x7f, 0xff, 0x1ff, 0x3ff, 0x7ff, 0xfff, 0x1fff];
    let mut v = (pcm >> 2) as i32; // 14-bit magnitude domain
    let mask = if v < 0 {
        v = -v;
        0x7f
    } else {
        0xff
    };
    if v > 8159 {
        v = 8159; // clip
    }
    v += 0x84 >> 2; // bias
    let seg = SEG_END.iter().position(|&e| v <= e).unwrap_or(8);
    if seg >= 8 {
        return (0x7f ^ mask) as u8;
    }
    let uval = ((seg as i32) << 4) | ((v >> (seg as i32 + 1)) & 0x0f);
    (uval ^ mask) as u8
}

/// Decode one G.711 A-law byte to a 16-bit sample.
pub fn alaw_to_pcm16(a: u8) -> i16 {
    let a = a ^ 0x55;
    let sign = a & 0x80;
    let exponent = ((a >> 4) & 0x07) as u32;
    let mantissa = (a & 0x0f) as i32;
    let magnitude = if exponent == 0 {
        (mantissa << 4) + 8
    } else {
        ((mantissa << 4) + 0x108) << (exponent - 1)
    };
    // After the 0x55 mask the sign bit is SET for positive samples.
    if sign != 0 {
        magnitude as i16
    } else {
        -magnitude as i16
    }
}

/// Encode a 16-bit sample as G.711 A-law.
pub fn pcm16_to_alaw(pcm: i16) -> u8 {
    const SEG_END: [i32; 8] = [0x1f, 0x3f, 0x7f, 0xff, 0x1ff, 0x3ff, 0x7ff, 0xfff];
    let mut v = (pcm >> 3) as i32; // 13-bit magnitude domain
    let mask = if v >= 0 {
        0xd5
    } else {
        v = -v - 1;
        0x55
    };
    let seg = SEG_END.iter().position(|&e| v <= e).unwrap_or(8);
    if seg >= 8 {
        return (0x7f ^ mask) as u8;
    }
    let mut aval = (seg as i32) << 4;
    aval |= if seg < 2 {
        (v >> 1) & 0x0f
    } else {
        (v >> seg as i32) & 0x0f
    };
    (aval ^ mask) as u8
}

// ------------------------------------------------------------- conversion ---

/// Sample encoding of the produced audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutEnc {
    /// Linear PCM `bytes` wide (WAV format tag 1).
    Pcm(usize),
    /// G.711 mu-law (WAV format tag 7).
    Ulaw,
    /// G.711 A-law (WAV format tag 6).
    Alaw,
}

impl OutEnc {
    fn bits(self) -> u16 {
        match self {
            OutEnc::Pcm(b) => (b * 8) as u16,
            _ => 8,
        }
    }
    fn bytes(self) -> usize {
        match self {
            OutEnc::Pcm(b) => b,
            _ => 1,
        }
    }
    fn wav_tag(self) -> u16 {
        match self {
            OutEnc::Pcm(_) => 1,
            OutEnc::Ulaw => 7,
            OutEnc::Alaw => 6,
        }
    }
    fn label(self) -> String {
        match self {
            OutEnc::Pcm(1) => "8-bit unsigned PCM (u8)".into(),
            OutEnc::Pcm(2) => "16-bit signed PCM little-endian (s16le)".into(),
            OutEnc::Pcm(3) => "24-bit signed PCM little-endian (s24le)".into(),
            OutEnc::Pcm(b) => format!("{}-bit signed PCM little-endian", b * 8),
            OutEnc::Ulaw => "8-bit G.711 mu-law".into(),
            OutEnc::Alaw => "8-bit G.711 A-law".into(),
        }
    }
    /// ffmpeg `-f` name for the headerless raw output.
    fn raw_format(self) -> &'static str {
        match self {
            OutEnc::Pcm(1) => "u8",
            OutEnc::Pcm(2) => "s16le",
            OutEnc::Pcm(3) => "s24le",
            OutEnc::Pcm(_) => "s32le",
            OutEnc::Ulaw => "mulaw",
            OutEnc::Alaw => "alaw",
        }
    }
}

/// Everything the conversion produced — the audio bytes plus what the `info`
/// report needs to describe them.
pub struct Converted {
    pub header: Header,
    pub audio: Vec<u8>,
    pub big_endian_source: bool,
    pub frames_in_file: u64,
    pub frames_out: u64,
    pub channels_out: usize,
    pub bytes_after_header: usize,
    enc: OutEnc,
    container_wav: bool,
}

impl Converted {
    /// Duration of the produced audio in seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.frames_out as f64 / self.header.sample_rate as f64
    }
    /// MIME type of `audio` (WAV, or an opaque stream for raw samples).
    pub fn mime(&self) -> &'static str {
        if self.container_wav {
            "audio/wav"
        } else {
            "application/octet-stream"
        }
    }
}

fn one_of(name: &str, value: &str, allowed: &[&str], default: &str) -> Result<String, String> {
    let v = value.trim();
    let v = if v.is_empty() { default } else { v };
    let lower = v.to_ascii_lowercase();
    if allowed.contains(&lower.as_str()) {
        Ok(lower)
    } else {
        Err(format!(
            "{name} must be one of {}, got \"{v}\"",
            allowed.join(", ")
        ))
    }
}

/// Decode the pasted payload (base64, hex or a `data:` URI) to bytes.
pub fn decode_input(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let fmt = one_of("input_format", input_format, &INPUT_FORMATS, "auto")?;
    let mut text = input.trim();
    if text.is_empty() {
        return Err("input is empty: paste the .sph file's bytes as base64 or hex".into());
    }
    // data:audio/x-nist;base64,…  →  keep the payload only.
    if text.starts_with("data:") {
        match text.find("base64,") {
            Some(i) => text = &text[i + "base64,".len()..],
            None => {
                return Err(
                    "data: URI is not base64-encoded: expected a \"…;base64,\" payload".into(),
                )
            }
        }
    }
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    let looks_hex = !compact.is_empty()
        && compact.len() % 2 == 0
        && compact.chars().all(|c| c.is_ascii_hexdigit());
    let use_hex = match fmt.as_str() {
        "hex" => true,
        "base64" => false,
        _ => looks_hex,
    };

    let bytes = if use_hex {
        let cleaned: String = compact
            .chars()
            .filter(|&c| c != ':' && c != '-' && c != ',')
            .collect();
        if cleaned.len() % 2 != 0 {
            return Err(format!(
                "hex input must have an even number of digits, got {}",
                cleaned.len()
            ));
        }
        if let Some(bad) = cleaned.chars().find(|c| !c.is_ascii_hexdigit()) {
            return Err(format!(
                "hex input contains a non-hex character '{bad}'; expected 0-9 a-f only"
            ));
        }
        if cleaned.len() / 2 > MAX_INPUT_BYTES {
            return Err(size_error(cleaned.len() / 2));
        }
        (0..cleaned.len() / 2)
            .map(|i| u8::from_str_radix(&cleaned[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    } else {
        if compact.len() / 4 * 3 > MAX_INPUT_BYTES {
            return Err(size_error(compact.len() / 4 * 3));
        }
        // Accept URL-safe alphabets and missing padding, like the rest of the
        // toolkit's base64 inputs.
        let normalized: String = compact
            .chars()
            .map(|c| match c {
                '-' => '+',
                '_' => '/',
                c => c,
            })
            .filter(|&c| c != '=')
            .collect();
        base64::engine::general_purpose::STANDARD_NO_PAD
            .decode(normalized.as_bytes())
            .map_err(|e| {
                format!(
                    "input is not valid base64 ({e}). Set input_format=hex if you pasted hex bytes."
                )
            })?
    };
    if bytes.len() > MAX_INPUT_BYTES {
        return Err(size_error(bytes.len()));
    }
    Ok(bytes)
}

fn size_error(got: usize) -> String {
    format!(
        "input is too large: {} MiB decoded, limit is {} MiB. Convert a shorter excerpt \
         (start_sample / max_samples) or run a desktop converter.",
        got / (1024 * 1024),
        MAX_INPUT_BYTES / (1024 * 1024)
    )
}

/// Parse a whole frame's worth of samples into 32-bit left-aligned values.
fn decode_sample(bytes: &[u8], header: &Header, big_endian: bool) -> i32 {
    match header.coding {
        Coding::Pcm => {
            let n = header.sample_n_bytes;
            let mut raw: u32 = 0;
            if big_endian {
                for (i, &b) in bytes.iter().take(n).enumerate() {
                    raw |= (b as u32) << (8 * (n - 1 - i));
                }
            } else {
                for (i, &b) in bytes.iter().take(n).enumerate() {
                    raw |= (b as u32) << (8 * i);
                }
            }
            // Sign-extend the n-byte value, then left-align to 32 bits.
            let shift = 32 - (n * 8) as u32;
            (raw << shift) as i32
        }
        Coding::Ulaw => (ulaw_to_pcm16(bytes[0]) as i32) << 16,
        Coding::Alaw => (alaw_to_pcm16(bytes[0]) as i32) << 16,
    }
}

fn encode_sample(value: i32, enc: OutEnc, out: &mut Vec<u8>) {
    match enc {
        OutEnc::Pcm(1) => out.push(((value >> 24) as i8 as i32 + 128) as u8), // WAV 8-bit is unsigned
        OutEnc::Pcm(2) => out.extend_from_slice(&((value >> 16) as i16).to_le_bytes()),
        OutEnc::Pcm(3) => {
            let v = value >> 8;
            out.extend_from_slice(&v.to_le_bytes()[..3]);
        }
        OutEnc::Pcm(_) => out.extend_from_slice(&value.to_le_bytes()),
        OutEnc::Ulaw => out.push(pcm16_to_ulaw((value >> 16) as i16)),
        OutEnc::Alaw => out.push(pcm16_to_alaw((value >> 16) as i16)),
    }
}

/// Parse the header, slice the requested frames, and produce the audio bytes.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    bytes: &[u8],
    encoding: &str,
    channel: &str,
    container: &str,
    byte_order: &str,
    start_sample: u64,
    max_samples: u64,
) -> Result<Converted, String> {
    let encoding = one_of("encoding", encoding, &ENCODINGS, "pcm16")?;
    let channel = one_of("channel", channel, &CHANNELS, "all")?;
    let container = one_of("container", container, &CONTAINERS, "wav")?;
    let byte_order = one_of("byte_order", byte_order, &BYTE_ORDERS, "auto")?;

    let header = parse_header(bytes)?;

    // Byte order: the header's sample_byte_format wins unless overridden.
    let big_endian_source = match byte_order.as_str() {
        "little" => false,
        "big" => true,
        _ => match header.byte_format.as_deref().map(str::trim) {
            Some("10") | Some("10 ") => true,
            Some("01") => false,
            Some(other) if header.sample_n_bytes > 1 => {
                if other.starts_with("10") {
                    true
                } else if other.starts_with("01") {
                    false
                } else {
                    return Err(format!(
                        "unrecognized sample_byte_format \"{other}\": expected \"01\" \
                         (little-endian) or \"10\" (big-endian). Set byte_order=little or \
                         byte_order=big to override the header."
                    ));
                }
            }
            _ => {
                if header.sample_n_bytes > 1 {
                    return Err(
                        "SPHERE header has no sample_byte_format but samples are wider than one \
                         byte; set byte_order=little or byte_order=big to say how to read them."
                            .into(),
                    );
                }
                false
            }
        },
    };

    let frame_bytes = header.sample_n_bytes * header.channel_count;
    let data = &bytes[header.header_bytes..];
    let bytes_after_header = data.len();
    let frames_available = (bytes_after_header / frame_bytes) as u64;
    let frames_in_file = match header.sample_count {
        Some(declared) => {
            if declared > frames_available {
                return Err(format!(
                    "truncated audio: the header declares sample_count {declared} \
                     ({} bytes of samples) but only {} bytes follow the {}-byte header",
                    declared * frame_bytes as u64,
                    bytes_after_header,
                    header.header_bytes
                ));
            }
            declared
        }
        None => frames_available,
    };
    if frames_in_file == 0 {
        return Err(format!(
            "no sample data: {bytes_after_header} bytes follow the {}-byte header, which is less \
             than one {frame_bytes}-byte sample frame",
            header.header_bytes
        ));
    }
    if start_sample >= frames_in_file {
        return Err(format!(
            "start_sample {start_sample} is past the end: the file holds {frames_in_file} sample \
             frames (0 to {})",
            frames_in_file - 1
        ));
    }
    let end = if max_samples == 0 {
        frames_in_file
    } else {
        frames_in_file.min(start_sample + max_samples)
    };
    let frames_out = end - start_sample;

    // Which source channels feed the output.
    let selected: Vec<usize> = match channel.as_str() {
        "all" | "mono" => (0..header.channel_count).collect(),
        "1" => vec![0],
        "2" => {
            if header.channel_count < 2 {
                return Err(format!(
                    "channel=2 needs at least 2 channels but the file is {}-channel",
                    header.channel_count
                ));
            }
            vec![1]
        }
        _ => unreachable!(),
    };
    let channels_out = if channel == "mono" { 1 } else { selected.len() };

    let source_enc = match header.coding {
        Coding::Pcm => OutEnc::Pcm(header.sample_n_bytes),
        Coding::Ulaw => OutEnc::Ulaw,
        Coding::Alaw => OutEnc::Alaw,
    };
    let enc = match encoding.as_str() {
        "source" => source_enc,
        "pcm16" => OutEnc::Pcm(2),
        "ulaw" => OutEnc::Ulaw,
        "alaw" => OutEnc::Alaw,
        _ => unreachable!(),
    };

    let sample_bytes = frames_out as usize * channels_out * enc.bytes();
    if sample_bytes > MAX_OUTPUT_BYTES {
        return Err(format!(
            "output would be {} MiB of audio, limit is {} MiB. Use max_samples to convert a \
             shorter window, or channel=1 to keep a single side.",
            sample_bytes / (1024 * 1024),
            MAX_OUTPUT_BYTES / (1024 * 1024)
        ));
    }

    // Write the RIFF header first and grow the buffer to its final size in ONE
    // exact reservation: Vec doubling would hold the old and new buffers alive
    // at once, which a multi-MiB payload cannot afford in a 64 MiB sandbox.
    let container_wav = container == "wav";
    let pad = sample_bytes % 2;
    let mut audio: Vec<u8> = Vec::new();
    if container_wav {
        let head = wav_header(sample_bytes, header.sample_rate, channels_out, enc);
        audio.reserve_exact(head.len() + sample_bytes + pad);
        audio.extend_from_slice(&head);
    } else {
        audio.reserve_exact(sample_bytes);
    }

    let start_byte = start_sample as usize * frame_bytes;
    for f in 0..frames_out as usize {
        let frame = &data[start_byte + f * frame_bytes..start_byte + (f + 1) * frame_bytes];
        if channel == "mono" && header.channel_count > 1 {
            let mut acc: i64 = 0;
            for &c in &selected {
                acc += decode_sample(&frame[c * header.sample_n_bytes..], &header, big_endian_source)
                    as i64;
            }
            encode_sample((acc / selected.len() as i64) as i32, enc, &mut audio);
        } else {
            for &c in &selected {
                let s = &frame[c * header.sample_n_bytes..];
                if enc == source_enc && header.coding == Coding::Pcm {
                    // Same encoding: copy the sample, fixing byte order only.
                    // 8-bit WAV PCM is unsigned, so that one still re-encodes.
                    if header.sample_n_bytes == 1 {
                        encode_sample(decode_sample(s, &header, big_endian_source), enc, &mut audio);
                    } else if big_endian_source {
                        for i in (0..header.sample_n_bytes).rev() {
                            audio.push(s[i]);
                        }
                    } else {
                        audio.extend_from_slice(&s[..header.sample_n_bytes]);
                    }
                } else if enc == source_enc {
                    audio.push(s[0]); // companded byte, unchanged
                } else {
                    encode_sample(decode_sample(s, &header, big_endian_source), enc, &mut audio);
                }
            }
        }
    }

    // A WAVE data chunk of odd length carries a pad byte.
    if container_wav && pad == 1 {
        audio.push(0);
    }

    Ok(Converted {
        header,
        audio,
        big_endian_source,
        frames_in_file,
        frames_out,
        channels_out,
        bytes_after_header,
        enc,
        container_wav,
    })
}

/// Build the RIFF/WAVE header for `data_len` bytes of samples. Companded
/// formats get the 18-byte `fmt ` chunk plus the `fact` chunk WAVE requires.
fn wav_header(data_len: usize, sample_rate: u32, channels: usize, enc: OutEnc) -> Vec<u8> {
    let extensible = enc.wav_tag() != 1;
    let fmt_size: u32 = if extensible { 18 } else { 16 };
    let block_align = (channels * enc.bytes()) as u16;
    let byte_rate = sample_rate * block_align as u32;
    let data_len = data_len as u32;
    let pad = data_len % 2;
    let fact_len: u32 = if extensible { 12 } else { 0 };
    let riff_size = 4 + (8 + fmt_size) + fact_len + (8 + data_len + pad);

    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&fmt_size.to_le_bytes());
    out.extend_from_slice(&enc.wav_tag().to_le_bytes());
    out.extend_from_slice(&(channels as u16).to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&enc.bits().to_le_bytes());
    if extensible {
        out.extend_from_slice(&0u16.to_le_bytes()); // cbSize
        out.extend_from_slice(b"fact");
        out.extend_from_slice(&4u32.to_le_bytes());
        let frames = if block_align == 0 {
            0
        } else {
            data_len / block_align as u32
        };
        out.extend_from_slice(&frames.to_le_bytes());
    }
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    out
}

// ---------------------------------------------------------------- render ----

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

fn human_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} B")
    } else if n < 1024 * 1024 {
        format!("{:.1} KB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MB", n as f64 / (1024.0 * 1024.0))
    }
}

fn info_report(c: &Converted) -> String {
    let h = &c.header;
    let mut s = String::new();
    s.push_str("SPHERE header\n");
    s.push_str("  magic            NIST_1A\n");
    s.push_str(&format!("  header_bytes     {}\n", h.header_bytes));
    for f in &h.fields {
        s.push_str(&format!("  {:<16} {} ({})\n", f.name, f.value, f.kind));
    }
    s.push_str("\nAudio in the file\n");
    s.push_str(&format!("  sample rate      {} Hz\n", h.sample_rate));
    s.push_str(&format!("  channels         {}\n", h.channel_count));
    s.push_str(&format!(
        "  sample coding    {} ({} byte{} per sample)\n",
        h.coding_raw,
        h.sample_n_bytes,
        if h.sample_n_bytes == 1 { "" } else { "s" }
    ));
    s.push_str(&format!(
        "  byte order       {}{}\n",
        if c.big_endian_source {
            "big-endian"
        } else {
            "little-endian"
        },
        match h.byte_format.as_deref() {
            Some(v) => format!(" (sample_byte_format {v})"),
            None => " (assumed; header has no sample_byte_format)".to_string(),
        }
    ));
    if let Some(bits) = h.sig_bits {
        s.push_str(&format!("  significant bits {bits}\n"));
    }
    s.push_str(&format!(
        "  sample frames    {} ({:.4} s)\n",
        c.frames_in_file,
        c.frames_in_file as f64 / h.sample_rate as f64
    ));
    s.push_str(&format!(
        "  sample bytes     {} after the header\n",
        c.bytes_after_header
    ));

    s.push_str("\nConverted output\n");
    s.push_str(&format!(
        "  container        {}\n",
        if c.container_wav {
            "RIFF/WAVE (.wav)"
        } else {
            "raw, headerless (.raw)"
        }
    ));
    s.push_str(&format!("  encoding         {}\n", c.enc.label()));
    s.push_str(&format!("  channels         {}\n", c.channels_out));
    s.push_str(&format!(
        "  sample frames    {} ({:.4} s)\n",
        c.frames_out,
        c.duration_seconds()
    ));
    s.push_str(&format!(
        "  size             {} ({} bytes)\n",
        human_bytes(c.audio.len()),
        c.audio.len()
    ));
    if !c.container_wav {
        s.push_str(&format!(
            "\nRe-import the raw samples with:\n  ffmpeg -f {} -ar {} -ac {} -i out.raw out.wav\n",
            c.enc.raw_format(),
            c.header.sample_rate,
            c.channels_out
        ));
    }
    s
}

/// Convert a pasted `.sph` payload and render the result.
///
/// * `input` — the SPHERE file's bytes as base64, hex, or a `data:` URI.
/// * `output` — `data_url` | `base64` | `hex` | `info`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    encoding: &str,
    channel: &str,
    container: &str,
    byte_order: &str,
    start_sample: u64,
    max_samples: u64,
) -> Result<String, String> {
    let output = one_of("output", output, &OUTPUTS, "data_url")?;
    let bytes = decode_input(input, input_format)?;
    let converted = convert(
        &bytes,
        encoding,
        channel,
        container,
        byte_order,
        start_sample,
        max_samples,
    )?;
    match output.as_str() {
        "info" => Ok(info_report(&converted)),
        "hex" => {
            if converted.audio.len() > MAX_HEX_OUTPUT_BYTES {
                return Err(format!(
                    "hex output would be {} MiB of text for {} of audio; the hex cap is {} MiB. \
                     Use output=base64 or trim with max_samples.",
                    converted.audio.len() * 2 / (1024 * 1024),
                    human_bytes(converted.audio.len()),
                    MAX_HEX_OUTPUT_BYTES / (1024 * 1024)
                ));
            }
            Ok(to_hex(&converted.audio))
        }
        "base64" => Ok(B64.encode(&converted.audio)),
        _ => Ok(format!(
            "data:{};base64,{}",
            converted.mime(),
            B64.encode(&converted.audio)
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a minimal but valid SPHERE file for tests.
    fn sph(fields: &[&str], data: &[u8]) -> Vec<u8> {
        let mut head = String::from("NIST_1A\n   1024\n");
        for f in fields {
            head.push_str(f);
            head.push('\n');
        }
        head.push_str("end_head\n");
        let mut out = head.into_bytes();
        out.resize(1024, b' ');
        out.extend_from_slice(data);
        out
    }

    fn mono16_be() -> Vec<u8> {
        // 4 frames, big-endian 16-bit: 1, -2, 258, -32768
        let data = [0x00, 0x01, 0xff, 0xfe, 0x01, 0x02, 0x80, 0x00];
        sph(
            &[
                "sample_rate -i 8000",
                "channel_count -i 1",
                "sample_n_bytes -i 2",
                "sample_byte_format -s2 10",
                "sample_coding -s3 pcm",
                "sample_count -i 4",
            ],
            &data,
        )
    }

    #[test]
    fn converts_big_endian_pcm_to_little_endian_wav() {
        let file = mono16_be();
        let out = run(&B64.encode(&file), "auto", "hex", "pcm16", "all", "wav", "auto", 0, 0)
            .expect("conversion succeeds");
        // 44-byte canonical PCM header, then byte-swapped samples.
        assert!(out.starts_with("52494646"), "starts with RIFF: {}", &out[..8]);
        assert_eq!(out.len(), (44 + 8) * 2);
        assert_eq!(&out[88..], "0100feff02010080");
        // fmt chunk: size 16, PCM tag 1, 1 channel, 8000 Hz, 16 bits
        assert_eq!(&out[32..40], "10000000");
        assert_eq!(&out[40..44], "0100");
        assert_eq!(&out[44..48], "0100");
        assert_eq!(&out[48..56], "401f0000");
        assert_eq!(&out[68..72], "1000"); // bits per sample
        assert_eq!(&out[72..80], "64617461"); // "data"
    }

    #[test]
    fn info_reports_header_fields_and_duration() {
        let file = mono16_be();
        let report = run(
            &B64.encode(&file),
            "base64",
            "info",
            "pcm16",
            "all",
            "wav",
            "auto",
            0,
            0,
        )
        .expect("info succeeds");
        assert!(report.contains("sample_rate      8000 (-i)"), "{report}");
        assert!(report.contains("sample_byte_format 10 (-s2)"), "{report}");
        assert!(report.contains("byte order       big-endian (sample_byte_format 10)"), "{report}");
        assert!(report.contains("sample frames    4 (0.0005 s)"), "{report}");
        assert!(report.contains("encoding         16-bit signed PCM little-endian (s16le)"), "{report}");
    }

    #[test]
    fn ulaw_expands_to_pcm16_and_round_trips_as_source() {
        let data = [0xff, 0x7f, 0x00, 0x80]; // silence-ish, both polarities
        let file = sph(
            &[
                "sample_rate -i 8000",
                "channel_count -i 1",
                "sample_n_bytes -i 1",
                "sample_coding -s4 ulaw",
                "sample_count -i 4",
            ],
            &data,
        );
        let pcm = run(&B64.encode(&file), "auto", "hex", "pcm16", "all", "raw", "auto", 0, 0)
            .expect("ulaw decode");
        let expect: String = data
            .iter()
            .map(|&b| to_hex(&ulaw_to_pcm16(b).to_le_bytes()))
            .collect();
        assert_eq!(pcm, expect);
        // encoding=source keeps the companded bytes verbatim.
        let same = run(&B64.encode(&file), "auto", "hex", "source", "all", "raw", "auto", 0, 0)
            .expect("ulaw passthrough");
        assert_eq!(same, "ff7f0080");
    }

    #[test]
    fn channel_and_range_selection_slice_the_right_samples() {
        // 3 stereo frames, little-endian 16-bit: (1,2) (3,4) (5,6)
        let data = [
            1, 0, 2, 0, //
            3, 0, 4, 0, //
            5, 0, 6, 0,
        ];
        let file = sph(
            &[
                "sample_rate -i 16000",
                "channel_count -i 2",
                "sample_n_bytes -i 2",
                "sample_byte_format -s2 01",
                "sample_coding -s3 pcm",
                "sample_count -i 3",
            ],
            &data,
        );
        let b64 = B64.encode(&file);
        let right = run(&b64, "auto", "hex", "pcm16", "2", "raw", "auto", 0, 0).unwrap();
        assert_eq!(right, "020004000600");
        let win = run(&b64, "auto", "hex", "pcm16", "1", "raw", "auto", 1, 1).unwrap();
        assert_eq!(win, "0300");
        let mono = run(&b64, "auto", "hex", "pcm16", "mono", "raw", "auto", 0, 0).unwrap();
        assert_eq!(mono, "010003000500"); // (1+2)/2, (3+4)/2, (5+6)/2
    }

    #[test]
    fn g711_codecs_round_trip_every_code() {
        // Both codecs are one-to-one over their 256 codes, with mu-law's single
        // documented exception: 0x7f and 0xff both decode to 0 and 0xff is the
        // canonical encoding of 0.
        let mut ulaw_exceptions = Vec::new();
        for u in 0u8..=255 {
            if pcm16_to_ulaw(ulaw_to_pcm16(u)) != u {
                ulaw_exceptions.push(u);
            }
            assert_eq!(pcm16_to_alaw(alaw_to_pcm16(u)), u, "A-law code {u}");
        }
        assert_eq!(ulaw_exceptions, vec![0x7f]);
        assert_eq!(ulaw_to_pcm16(0x7f), 0);
        assert_eq!(ulaw_to_pcm16(0xff), 0);
        // Reference points from the G.711 tables.
        assert_eq!(ulaw_to_pcm16(0x00), -32124);
        assert_eq!(ulaw_to_pcm16(0x80), 32124);
        assert_eq!(alaw_to_pcm16(0xd5), 8);
        assert_eq!(alaw_to_pcm16(0x55), -8);
    }

    #[test]
    fn hex_and_data_uri_inputs_are_accepted() {
        let file = mono16_be();
        let from_hex = run(&to_hex(&file), "auto", "base64", "pcm16", "all", "wav", "auto", 0, 0)
            .expect("hex input");
        let uri = format!("data:audio/x-nist;base64,{}", B64.encode(&file));
        let from_uri = run(&uri, "auto", "base64", "pcm16", "all", "wav", "auto", 0, 0)
            .expect("data uri input");
        assert_eq!(from_hex, from_uri);
        assert!(
            run(&B64.encode(&file), "auto", "data_url", "pcm16", "all", "wav", "auto", 0, 0)
                .unwrap()
                .starts_with("data:audio/wav;base64,")
        );
    }

    #[test]
    fn rejects_non_sphere_input() {
        let err = run(
            "UklGRiQAAABXQVZFZm10IBAAAAABAAEA",
            "auto",
            "info",
            "pcm16",
            "all",
            "wav",
            "auto",
            0,
            0,
        )
        .unwrap_err();
        assert!(err.contains("not a NIST SPHERE file"), "{err}");
        assert!(err.contains("RIFF"), "{err}");
    }

    #[test]
    fn rejects_shorten_compressed_payloads() {
        let file = sph(
            &[
                "sample_rate -i 16000",
                "channel_count -i 1",
                "sample_n_bytes -i 2",
                "sample_byte_format -s2 01",
                "sample_coding -s26 pcm,embedded-shorten-v2.00",
                "sample_count -i 2",
            ],
            &[0, 0, 0, 0],
        );
        let err = run(&B64.encode(&file), "auto", "info", "pcm16", "all", "wav", "auto", 0, 0)
            .unwrap_err();
        assert!(err.contains("shorten-compressed"), "{err}");
    }

    #[test]
    fn rejects_bad_enum_values_and_out_of_range_windows() {
        let b64 = B64.encode(&mono16_be());
        let err = run(&b64, "auto", "yaml", "pcm16", "all", "wav", "auto", 0, 0).unwrap_err();
        assert_eq!(
            err,
            "output must be one of data_url, base64, hex, info, got \"yaml\""
        );
        let err = run(&b64, "auto", "hex", "pcm16", "2", "wav", "auto", 0, 0).unwrap_err();
        assert!(err.contains("channel=2 needs at least 2 channels"), "{err}");
        let err = run(&b64, "auto", "hex", "pcm16", "all", "wav", "auto", 9, 0).unwrap_err();
        assert!(err.contains("start_sample 9 is past the end"), "{err}");
    }

    #[test]
    fn truncated_sample_data_is_reported() {
        let file = sph(
            &[
                "sample_rate -i 8000",
                "channel_count -i 1",
                "sample_n_bytes -i 2",
                "sample_byte_format -s2 01",
                "sample_coding -s3 pcm",
                "sample_count -i 100",
            ],
            &[1, 0, 2, 0],
        );
        let err = run(&B64.encode(&file), "auto", "info", "pcm16", "all", "wav", "auto", 0, 0)
            .unwrap_err();
        assert!(err.contains("truncated audio"), "{err}");
        assert!(err.contains("sample_count 100"), "{err}");
    }

    #[test]
    fn byte_order_override_beats_the_header() {
        let file = mono16_be();
        let forced = run(
            &B64.encode(&file),
            "auto",
            "hex",
            "pcm16",
            "all",
            "raw",
            "little",
            0,
            0,
        )
        .unwrap();
        assert_eq!(forced, "0001fffe01028000");
    }
}
