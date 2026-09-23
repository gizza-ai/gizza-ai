//! raw-pcm-to-wav core — wrap headerless raw PCM samples in a RIFF/WAVE header.
//! Pure compute, shared verbatim by the chat/CLI block and the page.
//!
//! Raw PCM carries nothing but sample values: no magic bytes, no sample rate,
//! no channel count, no statement of how wide a sample is or which way round
//! its bytes go. Those five facts have to be supplied by whoever dumped the
//! data (`sample_rate`, `channels`, `bit_depth`, `encoding`, `byte_order`), and
//! this module turns them into the 44-byte canonical header — or the 18-byte
//! `fmt ` chunk plus `fact` chunk that WAVE requires for float and G.711 — that
//! makes the same bytes a playable file.
//!
//! Samples are never decoded or resampled. The only edits are the two WAVE
//! demands: little-endian byte order, and 8-bit PCM stored unsigned while every
//! wider integer depth is stored signed.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;

/// Largest decoded raw-PCM payload accepted (the block runs in a 64 MiB sandbox).
pub const MAX_INPUT_BYTES: usize = 12 * 1024 * 1024;
/// Largest WAV file produced. Wrapping only adds a header, so this tracks the input cap.
pub const MAX_OUTPUT_BYTES: usize = 12 * 1024 * 1024;
/// Hex rendering doubles the size again, so it gets a tighter cap of its own.
pub const MAX_HEX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

/// Highest sample rate a WAVE header can sensibly advertise here.
pub const MAX_SAMPLE_RATE: u32 = 768_000;
/// Highest channel count accepted (a plain `fmt ` chunk carries no channel mask).
pub const MAX_CHANNELS: u32 = 16;

const INPUT_FORMATS: [&str; 3] = ["auto", "base64", "hex"];
const BIT_DEPTHS: [&str; 5] = ["8", "16", "24", "32", "64"];
const ENCODINGS: [&str; 5] = ["signed", "unsigned", "float", "mulaw", "alaw"];
const BYTE_ORDERS: [&str; 2] = ["little", "big"];
const OUTPUTS: [&str; 4] = ["data_url", "base64", "hex", "info"];

// ------------------------------------------------------------------ specs ----

/// How the incoming bytes encode one sample value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Two's-complement integer, `bits` wide.
    Signed,
    /// Offset-binary (unsigned) integer, `bits` wide.
    Unsigned,
    /// IEEE 754 float, 32 or 64 bits wide.
    Float,
    /// G.711 mu-law, always one byte per sample.
    Mulaw,
    /// G.711 A-law, always one byte per sample.
    Alaw,
}

/// The fully validated description of the raw data plus the header it implies.
#[derive(Debug, Clone)]
pub struct Spec {
    pub sample_rate: u32,
    pub channels: u32,
    /// Bits per sample as read from the input (8 for mu-law/A-law).
    pub bits: u16,
    pub encoding: Encoding,
    pub big_endian: bool,
    /// True when `bit_depth` was overridden because the encoding fixes it.
    pub depth_forced: bool,
}

impl Spec {
    /// Bytes occupied by one sample of one channel.
    pub fn sample_bytes(&self) -> usize {
        (self.bits / 8) as usize
    }
    /// Bytes occupied by one sample frame (one sample per channel).
    pub fn frame_bytes(&self) -> usize {
        self.sample_bytes() * self.channels as usize
    }
    /// WAVE format tag written into the `fmt ` chunk.
    pub fn wav_tag(&self) -> u16 {
        match self.encoding {
            Encoding::Signed | Encoding::Unsigned => 1, // WAVE_FORMAT_PCM
            Encoding::Float => 3,                       // WAVE_FORMAT_IEEE_FLOAT
            Encoding::Alaw => 6,                        // WAVE_FORMAT_ALAW
            Encoding::Mulaw => 7,                       // WAVE_FORMAT_MULAW
        }
    }
    /// Human name of the format tag, for the `info` report.
    pub fn wav_tag_name(&self) -> &'static str {
        match self.wav_tag() {
            1 => "WAVE_FORMAT_PCM",
            3 => "WAVE_FORMAT_IEEE_FLOAT",
            6 => "WAVE_FORMAT_ALAW",
            _ => "WAVE_FORMAT_MULAW",
        }
    }
    /// Only WAVE_FORMAT_PCM uses the bare 16-byte `fmt ` chunk; everything else
    /// needs `cbSize` and a `fact` chunk.
    fn extended_header(&self) -> bool {
        self.wav_tag() != 1
    }
    /// Size in bytes of the header this spec writes before the sample data.
    pub fn header_bytes(&self) -> usize {
        if self.extended_header() {
            12 + 8 + 18 + 12 + 8 // RIFF/WAVE + fmt(18) + fact + data
        } else {
            44
        }
    }
    /// ffmpeg raw-demuxer name (`-f`) for the INPUT bytes.
    pub fn ffmpeg_input_format(&self) -> String {
        let end = if self.big_endian { "be" } else { "le" };
        match (self.encoding, self.bits) {
            (Encoding::Mulaw, _) => "mulaw".into(),
            (Encoding::Alaw, _) => "alaw".into(),
            (Encoding::Signed, 8) => "s8".into(),
            (Encoding::Unsigned, 8) => "u8".into(),
            (Encoding::Signed, b) => format!("s{b}{end}"),
            (Encoding::Unsigned, b) => format!("u{b}{end}"),
            (Encoding::Float, b) => format!("f{b}{end}"),
        }
    }
    /// SoX `-e` encoding name for the INPUT bytes.
    pub fn sox_encoding(&self) -> &'static str {
        match self.encoding {
            Encoding::Signed => "signed-integer",
            Encoding::Unsigned => "unsigned-integer",
            Encoding::Float => "floating-point",
            Encoding::Mulaw => "u-law",
            Encoding::Alaw => "a-law",
        }
    }
    /// How the input encoding reads in prose, for the `info` report.
    pub fn input_label(&self) -> String {
        let order = if self.sample_bytes() == 1 {
            String::new()
        } else if self.big_endian {
            ", big-endian".into()
        } else {
            ", little-endian".into()
        };
        match self.encoding {
            Encoding::Signed => format!("signed integer, {}-bit{order}", self.bits),
            Encoding::Unsigned => format!("unsigned integer, {}-bit{order}", self.bits),
            Encoding::Float => format!("IEEE float, {}-bit{order}", self.bits),
            Encoding::Mulaw => "G.711 mu-law, 8-bit".into(),
            Encoding::Alaw => "G.711 A-law, 8-bit".into(),
        }
    }
    /// How the WAV stores the same samples once the two WAVE rules are applied.
    pub fn output_label(&self) -> String {
        match self.encoding {
            Encoding::Signed | Encoding::Unsigned if self.bits == 8 => {
                "unsigned 8-bit PCM (WAVE stores 8-bit as unsigned)".into()
            }
            Encoding::Signed | Encoding::Unsigned => {
                format!("signed {}-bit PCM, little-endian", self.bits)
            }
            Encoding::Float => format!("IEEE float {}-bit, little-endian", self.bits),
            Encoding::Mulaw => "G.711 mu-law, 8-bit".into(),
            Encoding::Alaw => "G.711 A-law, 8-bit".into(),
        }
    }
    /// The matching entry in a raw-import dialog's encoding list, if there is one.
    pub fn import_dialog_encoding(&self) -> String {
        match (self.encoding, self.bits) {
            (Encoding::Signed, 8) => "Signed 8-bit PCM".into(),
            (Encoding::Unsigned, 8) => "Unsigned 8-bit PCM".into(),
            (Encoding::Signed, b) => format!("Signed {b}-bit PCM"),
            (Encoding::Float, b) => format!("{b}-bit float"),
            (Encoding::Mulaw, _) => "U-Law".into(),
            (Encoding::Alaw, _) => "A-Law".into(),
            (Encoding::Unsigned, b) => format!(
                "no unsigned {b}-bit entry exists — convert here, or import as Signed {b}-bit PCM \
                 and expect the waveform offset by half scale"
            ),
        }
    }
}

// ----------------------------------------------------------------- input -----

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

fn size_error(got: usize) -> String {
    format!(
        "input is too large: {:.1} MiB decoded, limit is {} MiB. Wrap a shorter excerpt \
         (skip_bytes / max_frames) or run ffmpeg locally on the full dump.",
        got as f64 / (1024.0 * 1024.0),
        MAX_INPUT_BYTES / (1024 * 1024)
    )
}

/// Decode the pasted payload (base64, hex or a `data:` URI) to bytes.
pub fn decode_input(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let fmt = one_of("input_format", input_format, &INPUT_FORMATS, "auto")?;
    let mut text = input.trim();
    if text.is_empty() {
        return Err(
            "input is empty: paste the raw PCM sample bytes as base64 (e.g. `base64 dump.pcm`) \
             or as hex (e.g. `xxd -p dump.pcm`)"
                .into(),
        );
    }
    // data:application/octet-stream;base64,…  →  keep the payload only.
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

/// Validate the five facts raw PCM cannot carry, rejecting impossible pairings.
pub fn build_spec(
    sample_rate: u32,
    channels: u32,
    bit_depth: &str,
    encoding: &str,
    byte_order: &str,
) -> Result<Spec, String> {
    if sample_rate == 0 || sample_rate > MAX_SAMPLE_RATE {
        return Err(format!(
            "sample_rate must be between 1 and {MAX_SAMPLE_RATE} Hz, got {sample_rate}. Raw PCM \
             stores no rate, so this is the value the dump was recorded at (commonly 8000, 16000, \
             22050, 44100 or 48000)."
        ));
    }
    if channels == 0 || channels > MAX_CHANNELS {
        return Err(format!(
            "channels must be between 1 and {MAX_CHANNELS}, got {channels}. Use 1 for a mono dump \
             and 2 for interleaved stereo."
        ));
    }
    let depth = one_of("bit_depth", bit_depth, &BIT_DEPTHS, "16")?;
    let enc = one_of("encoding", encoding, &ENCODINGS, "signed")?;
    let order = one_of("byte_order", byte_order, &BYTE_ORDERS, "little")?;

    let mut bits: u16 = depth.parse().unwrap();
    let encoding = match enc.as_str() {
        "signed" => Encoding::Signed,
        "unsigned" => Encoding::Unsigned,
        "float" => Encoding::Float,
        "mulaw" => Encoding::Mulaw,
        _ => Encoding::Alaw,
    };

    let mut depth_forced = false;
    match encoding {
        Encoding::Mulaw | Encoding::Alaw => {
            // G.711 is one companded byte per sample by definition; ffmpeg's
            // `-f mulaw` has no depth knob either, so take 8 rather than error.
            if bits != 8 {
                depth_forced = true;
            }
            bits = 8;
        }
        Encoding::Float => {
            if bits != 32 && bits != 64 {
                return Err(format!(
                    "encoding=float needs bit_depth 32 or 64, got {bits}. IEEE floats come in \
                     those two widths only — pick encoding=signed or encoding=unsigned for \
                     {bits}-bit integer samples."
                ));
            }
        }
        Encoding::Signed | Encoding::Unsigned => {
            if bits == 64 {
                return Err(format!(
                    "bit_depth 64 is float-only in WAVE, but encoding={enc}. Use encoding=float \
                     for 64-bit doubles, or bit_depth 8, 16, 24 or 32 for integer samples."
                ));
            }
        }
    }

    Ok(Spec {
        sample_rate,
        channels,
        bits,
        encoding,
        big_endian: order == "big",
        depth_forced,
    })
}

// ------------------------------------------------------------ conversion -----

/// The wrapped file plus everything the `info` report needs to describe it.
pub struct Wrapped {
    pub spec: Spec,
    /// The complete WAV file (header + data chunk), or just the data chunk
    /// bytes when the caller only wanted the numbers.
    pub wav: Vec<u8>,
    pub input_bytes: usize,
    pub skipped_bytes: usize,
    pub usable_bytes: usize,
    pub leftover_bytes: usize,
    pub frames_available: u64,
    pub frames_out: u64,
    pub data_bytes: usize,
}

impl Wrapped {
    /// Duration of the wrapped audio in seconds.
    pub fn duration_seconds(&self) -> f64 {
        self.frames_out as f64 / self.spec.sample_rate as f64
    }
}

/// Copy one sample into the output, applying the only two edits WAVE requires:
/// little-endian byte order, and 8-bit PCM stored unsigned.
fn push_sample(src: &[u8], spec: &Spec, out: &mut Vec<u8>) {
    let n = spec.sample_bytes();
    if n == 1 {
        match spec.encoding {
            // Offset-binary conversion: two's-complement 8-bit becomes the
            // unsigned 8-bit WAVE mandates (and vice versa is never needed).
            Encoding::Signed => out.push((src[0] as i8 as i16 + 128) as u8),
            _ => out.push(src[0]),
        }
        return;
    }
    let start = out.len();
    if spec.big_endian {
        for i in (0..n).rev() {
            out.push(src[i]);
        }
    } else {
        out.extend_from_slice(&src[..n]);
    }
    if spec.encoding == Encoding::Unsigned {
        // Flip the sign bit of the now-little-endian value: offset binary and
        // two's complement differ by exactly that bit at every width.
        out[start + n - 1] ^= 0x80;
    }
}

/// Build the RIFF/WAVE header for `data_len` bytes of samples.
pub fn wav_header(spec: &Spec, data_len: usize) -> Vec<u8> {
    let extended = spec.extended_header();
    let fmt_size: u32 = if extended { 18 } else { 16 };
    let block_align = (spec.channels as usize * spec.sample_bytes()) as u16;
    let byte_rate = spec.sample_rate * block_align as u32;
    let data_len = data_len as u32;
    let pad = data_len % 2;
    let fact_len: u32 = if extended { 12 } else { 0 };
    let riff_size = 4 + (8 + fmt_size) + fact_len + (8 + data_len + pad);

    let mut out = Vec::with_capacity(64);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&fmt_size.to_le_bytes());
    out.extend_from_slice(&spec.wav_tag().to_le_bytes());
    out.extend_from_slice(&(spec.channels as u16).to_le_bytes());
    out.extend_from_slice(&spec.sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&spec.bits.to_le_bytes());
    if extended {
        out.extend_from_slice(&0u16.to_le_bytes()); // cbSize — no extra fields
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

/// Slice the requested window out of the raw bytes and wrap it in a WAV header.
pub fn wrap(
    bytes: &[u8],
    spec: Spec,
    skip_bytes: u64,
    max_frames: u64,
) -> Result<Wrapped, String> {
    let input_bytes = bytes.len();
    let skip = skip_bytes as usize;
    if skip >= input_bytes {
        return Err(format!(
            "skip_bytes {skip} is at or past the end of the {input_bytes}-byte input: there would \
             be no samples left to wrap"
        ));
    }
    let data = &bytes[skip..];
    let frame_bytes = spec.frame_bytes();
    let frames_available = (data.len() / frame_bytes) as u64;
    let leftover_bytes = data.len() % frame_bytes;
    if frames_available == 0 {
        return Err(format!(
            "not enough data for one sample frame: {} bytes remain after skip_bytes but one frame \
             of {} channel(s) at {}-bit is {frame_bytes} bytes. Check bit_depth and channels — a \
             wrong frame size is the usual cause.",
            data.len(),
            spec.channels,
            spec.bits
        ));
    }
    let frames_out = if max_frames == 0 {
        frames_available
    } else {
        frames_available.min(max_frames)
    };

    let data_bytes = frames_out as usize * frame_bytes;
    if data_bytes > MAX_OUTPUT_BYTES {
        return Err(format!(
            "output would be {:.1} MiB of audio, limit is {} MiB. Use max_frames to wrap a shorter \
             window.",
            data_bytes as f64 / (1024.0 * 1024.0),
            MAX_OUTPUT_BYTES / (1024 * 1024)
        ));
    }

    // Grow the buffer to its final size in ONE exact reservation: Vec doubling
    // would hold the old and new buffers alive at once, which a multi-MiB
    // payload cannot afford in a 64 MiB sandbox.
    let header = wav_header(&spec, data_bytes);
    let pad = data_bytes % 2;
    let mut wav = Vec::new();
    wav.reserve_exact(header.len() + data_bytes + pad);
    wav.extend_from_slice(&header);

    let sample_bytes = spec.sample_bytes();
    let plain_copy = !spec.big_endian
        && spec.encoding != Encoding::Unsigned
        && !(sample_bytes == 1 && spec.encoding == Encoding::Signed);
    if plain_copy {
        // Nothing to fix: the bytes are already exactly what WAVE stores.
        wav.extend_from_slice(&data[..data_bytes]);
    } else {
        for s in 0..data_bytes / sample_bytes {
            push_sample(&data[s * sample_bytes..], &spec, &mut wav);
        }
    }
    // A WAVE data chunk of odd length carries a pad byte.
    if pad == 1 {
        wav.push(0);
    }

    Ok(Wrapped {
        spec,
        wav,
        input_bytes,
        skipped_bytes: skip,
        usable_bytes: data.len(),
        leftover_bytes,
        frames_available,
        frames_out,
        data_bytes,
    })
}

// ---------------------------------------------------------------- render -----

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

fn info_report(w: &Wrapped) -> String {
    let s = &w.spec;
    let mut out = String::new();
    out.push_str("Raw input as described\n");
    out.push_str(&format!("  encoding         {}\n", s.input_label()));
    out.push_str(&format!("  sample rate      {} Hz\n", s.sample_rate));
    out.push_str(&format!("  channels         {}\n", s.channels));
    out.push_str(&format!(
        "  frame size       {} bytes ({} ch x {} byte{})\n",
        s.frame_bytes(),
        s.channels,
        s.sample_bytes(),
        if s.sample_bytes() == 1 { "" } else { "s" }
    ));
    out.push_str(&format!(
        "  bytes given      {} ({})\n",
        w.input_bytes,
        human_bytes(w.input_bytes)
    ));
    out.push_str(&format!("  skipped          {} bytes\n", w.skipped_bytes));
    out.push_str(&format!(
        "  usable           {} bytes = {} sample frame{}\n",
        w.usable_bytes,
        w.frames_available,
        if w.frames_available == 1 { "" } else { "s" }
    ));
    out.push_str(&format!(
        "  leftover         {} bytes{}\n",
        w.leftover_bytes,
        if w.leftover_bytes == 0 {
            " (the data divides evenly into frames)".to_string()
        } else {
            " (an incomplete trailing frame — dropped; a non-zero leftover usually means \
             bit_depth or channels is wrong)"
                .to_string()
        }
    ));
    if s.depth_forced {
        out.push_str("  note             bit_depth was set to 8: G.711 is one byte per sample\n");
    }
    if s.sample_bytes() == 1 {
        out.push_str("  note             byte_order is irrelevant for single-byte samples\n");
    }

    out.push_str("\nWAV that will be written\n");
    out.push_str(&format!(
        "  format tag       {} ({})\n",
        s.wav_tag(),
        s.wav_tag_name()
    ));
    out.push_str(&format!("  stored samples   {}\n", s.output_label()));
    out.push_str(&format!("  bits per sample  {}\n", s.bits));
    out.push_str(&format!("  channels         {}\n", s.channels));
    out.push_str(&format!("  sample rate      {} Hz\n", s.sample_rate));
    out.push_str(&format!(
        "  byte rate        {} bytes/s\n",
        s.sample_rate as usize * s.frame_bytes()
    ));
    out.push_str(&format!("  block align      {} bytes\n", s.frame_bytes()));
    out.push_str(&format!("  header           {} bytes\n", s.header_bytes()));
    out.push_str(&format!("  data chunk       {} bytes\n", w.data_bytes));
    out.push_str(&format!(
        "  file size        {} bytes ({})\n",
        w.wav.len(),
        human_bytes(w.wav.len())
    ));
    out.push_str(&format!(
        "  sample frames    {} ({:.6} s)\n",
        w.frames_out,
        w.duration_seconds()
    ));

    out.push_str("\nThe same conversion elsewhere\n");
    let skip_note = if w.skipped_bytes > 0 {
        format!(" (after dropping the first {} bytes)", w.skipped_bytes)
    } else {
        String::new()
    };
    out.push_str(&format!(
        "  ffmpeg           ffmpeg -f {} -ar {} -ac {} -i in.pcm out.wav{}\n",
        s.ffmpeg_input_format(),
        s.sample_rate,
        s.channels,
        skip_note
    ));
    out.push_str(&format!(
        "  sox              sox -t raw -r {} -b {} -e {} -{} -c {} in.pcm out.wav\n",
        s.sample_rate,
        s.bits,
        s.sox_encoding(),
        if s.big_endian { "B" } else { "L" },
        s.channels
    ));
    out.push_str(&format!(
        "  import dialog    {}, {}-endian, {} channel{}, start offset {} bytes, {} Hz\n",
        s.import_dialog_encoding(),
        if s.big_endian { "big" } else { "little" },
        s.channels,
        if s.channels == 1 { "" } else { "s" },
        w.skipped_bytes,
        s.sample_rate
    ));
    out
}

/// Wrap a pasted raw-PCM payload in a WAV header and render the result.
///
/// * `input` — the raw sample bytes as base64, hex, or a `data:` URI.
/// * `output` — `data_url` | `base64` | `hex` | `info`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    sample_rate: u32,
    channels: u32,
    bit_depth: &str,
    encoding: &str,
    byte_order: &str,
    skip_bytes: u64,
    max_frames: u64,
    output: &str,
) -> Result<String, String> {
    let output = one_of("output", output, &OUTPUTS, "data_url")?;
    let spec = build_spec(sample_rate, channels, bit_depth, encoding, byte_order)?;
    let bytes = decode_input(input, input_format)?;
    let wrapped = wrap(&bytes, spec, skip_bytes, max_frames)?;

    match output.as_str() {
        "info" => Ok(info_report(&wrapped)),
        "hex" => {
            if wrapped.wav.len() > MAX_HEX_OUTPUT_BYTES {
                return Err(format!(
                    "hex output would be {:.1} MiB of text for {} of audio; the hex cap is {} MiB. \
                     Use output=base64 or trim with max_frames.",
                    wrapped.wav.len() as f64 * 2.0 / (1024.0 * 1024.0),
                    human_bytes(wrapped.wav.len()),
                    MAX_HEX_OUTPUT_BYTES / (1024 * 1024)
                ));
            }
            Ok(to_hex(&wrapped.wav))
        }
        "base64" => Ok(B64.encode(&wrapped.wav)),
        _ => Ok(format!("data:audio/wav;base64,{}", B64.encode(&wrapped.wav))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 8 stereo frames of 16-bit signed little-endian PCM — the page's demo dump.
    const DEMO_HEX: &str = "00000000001000f0002000e0003000d0004000c0005000b0006000a000700090";

    fn demo() -> Vec<u8> {
        (0..DEMO_HEX.len() / 2)
            .map(|i| u8::from_str_radix(&DEMO_HEX[i * 2..i * 2 + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn wraps_cd_style_stereo_pcm_in_a_44_byte_header() {
        let out = run(
            DEMO_HEX, "hex", 44100, 2, "16", "signed", "little", 0, 0, "hex",
        )
        .expect("wrap succeeds");
        // 44-byte canonical header + the 32 sample bytes, unchanged.
        assert_eq!(out.len(), (44 + 32) * 2);
        assert!(out.starts_with("52494646"), "RIFF magic: {}", &out[..8]);
        assert_eq!(&out[16..24], "57415645"); // "WAVE"
        assert_eq!(&out[24..32], "666d7420"); // "fmt "
        assert_eq!(&out[32..40], "10000000"); // fmt chunk size 16
        assert_eq!(&out[40..44], "0100"); // WAVE_FORMAT_PCM
        assert_eq!(&out[44..48], "0200"); // 2 channels
        assert_eq!(&out[48..56], "44ac0000"); // 44100 Hz
        assert_eq!(&out[56..64], "10b10200"); // byte rate 176400
        assert_eq!(&out[64..68], "0400"); // block align 4
        assert_eq!(&out[68..72], "1000"); // 16 bits
        assert_eq!(&out[72..80], "64617461"); // "data"
        assert_eq!(&out[80..88], "20000000"); // data chunk 32 bytes
        assert_eq!(&out[88..], DEMO_HEX); // samples copied verbatim
        // RIFF size = file size - 8.
        assert_eq!(&out[8..16], "44000000");
    }

    #[test]
    fn big_endian_input_is_byte_swapped_into_wave_order() {
        // Two 16-bit frames, big-endian: 0x0102, 0xfffe.
        let be = "0102fffe";
        let out = run(be, "hex", 8000, 1, "16", "signed", "big", 0, 0, "hex").unwrap();
        assert_eq!(&out[88..], "0201feff");
        // The same bytes read little-endian pass through untouched.
        let le = run(be, "hex", 8000, 1, "16", "signed", "little", 0, 0, "hex").unwrap();
        assert_eq!(&le[88..], be);
    }

    #[test]
    fn signedness_is_converted_to_what_wave_requires() {
        // 8-bit signed input becomes unsigned: -128, -1, 0, 127 → 0, 127, 128, 255.
        let out = run("80ff007f", "hex", 8000, 1, "8", "signed", "little", 0, 0, "hex").unwrap();
        assert_eq!(&out[88..], "007f80ff");
        assert_eq!(&out[68..72], "0800"); // 8 bits per sample
        // 8-bit unsigned input is already what WAVE stores.
        let same = run("80ff007f", "hex", 8000, 1, "8", "unsigned", "little", 0, 0, "hex").unwrap();
        assert_eq!(&same[88..], "80ff007f");
        // 16-bit unsigned (offset binary) flips to signed: 0x0000 → -32768, 0x8000 → 0.
        let u16le = run("00000080", "hex", 8000, 1, "16", "unsigned", "little", 0, 0, "hex").unwrap();
        assert_eq!(&u16le[88..], "00800000");
    }

    #[test]
    fn float_and_g711_get_an_extended_fmt_chunk_plus_fact() {
        // One 32-bit float frame (1.0f = 0x3f800000 little-endian).
        let out = run("0000803f", "hex", 48000, 1, "32", "float", "little", 0, 0, "hex").unwrap();
        assert_eq!(&out[24..32], "666d7420");
        assert_eq!(&out[32..40], "12000000"); // fmt size 18
        assert_eq!(&out[40..44], "0300"); // WAVE_FORMAT_IEEE_FLOAT
        assert_eq!(&out[68..72], "2000"); // 32 bits
        assert_eq!(&out[72..76], "0000"); // cbSize 0
        assert_eq!(&out[76..84], "66616374"); // "fact"
        assert_eq!(&out[84..92], "04000000");
        assert_eq!(&out[92..100], "01000000"); // 1 sample frame
        assert_eq!(&out[100..108], "64617461"); // "data"
        assert_eq!(&out[116..], "0000803f");

        // mu-law keeps its companded bytes and takes format tag 7 at 8 bits,
        // with bit_depth coerced from the default 16.
        let ulaw = run("ff7f0080", "hex", 8000, 1, "16", "mulaw", "little", 0, 0, "hex").unwrap();
        assert_eq!(&ulaw[40..44], "0700");
        assert_eq!(&ulaw[68..72], "0800");
        assert_eq!(&ulaw[116..], "ff7f0080");
        // A-law is tag 6 on the same shape.
        let alaw = run("ff7f0080", "hex", 8000, 1, "8", "alaw", "little", 0, 0, "hex").unwrap();
        assert_eq!(&alaw[40..44], "0600");
    }

    #[test]
    fn skip_bytes_and_max_frames_cut_a_window() {
        // Drop the first 4-byte stereo frame, then keep 2 of the remaining frames.
        let out = run(
            DEMO_HEX, "hex", 44100, 2, "16", "signed", "little", 4, 2, "hex",
        )
        .unwrap();
        assert_eq!(&out[88..], "001000f0002000e0");
        assert_eq!(&out[80..88], "08000000"); // data chunk 8 bytes
    }

    #[test]
    fn odd_length_data_chunks_get_the_riff_pad_byte() {
        // 3 frames of 8-bit mono = 3 bytes, an odd data chunk.
        let out = run("010203", "hex", 8000, 1, "8", "unsigned", "little", 0, 0, "hex").unwrap();
        assert_eq!(&out[80..88], "03000000"); // data chunk length is the real 3
        assert_eq!(&out[88..], "01020300"); // one pad byte follows
        assert_eq!(&out[8..16], "28000000"); // RIFF size counts the pad: 40
    }

    #[test]
    fn data_url_and_base64_outputs_agree_with_the_hex_one() {
        let hex = run(DEMO_HEX, "hex", 8000, 1, "16", "signed", "little", 0, 0, "hex").unwrap();
        let b64 = run(DEMO_HEX, "hex", 8000, 1, "16", "signed", "little", 0, 0, "base64").unwrap();
        let url = run(DEMO_HEX, "hex", 8000, 1, "16", "signed", "little", 0, 0, "data_url").unwrap();
        let bytes = (0..hex.len() / 2)
            .map(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
            .collect::<Vec<u8>>();
        assert_eq!(b64, B64.encode(&bytes));
        assert_eq!(url, format!("data:audio/wav;base64,{b64}"));
    }

    #[test]
    fn base64_hex_and_data_uri_inputs_decode_to_the_same_wav() {
        let raw = demo();
        let from_b64 = run(
            &B64.encode(&raw), "base64", 44100, 2, "16", "signed", "little", 0, 0, "base64",
        )
        .unwrap();
        let from_hex = run(
            DEMO_HEX, "hex", 44100, 2, "16", "signed", "little", 0, 0, "base64",
        )
        .unwrap();
        let from_auto = run(
            &B64.encode(&raw), "auto", 44100, 2, "16", "signed", "little", 0, 0, "base64",
        )
        .unwrap();
        let uri = format!("data:application/octet-stream;base64,{}", B64.encode(&raw));
        let from_uri = run(&uri, "auto", 44100, 2, "16", "signed", "little", 0, 0, "base64").unwrap();
        assert_eq!(from_hex, from_b64);
        assert_eq!(from_auto, from_b64);
        assert_eq!(from_uri, from_b64);
    }

    #[test]
    fn info_reports_the_derived_numbers_and_equivalent_commands() {
        let report = run(
            DEMO_HEX, "hex", 44100, 2, "16", "signed", "little", 0, 0, "info",
        )
        .unwrap();
        assert!(report.contains("encoding         signed integer, 16-bit, little-endian"), "{report}");
        assert!(report.contains("frame size       4 bytes (2 ch x 2 bytes)"), "{report}");
        assert!(report.contains("usable           32 bytes = 8 sample frames"), "{report}");
        assert!(report.contains("leftover         0 bytes (the data divides evenly"), "{report}");
        assert!(report.contains("format tag       1 (WAVE_FORMAT_PCM)"), "{report}");
        assert!(report.contains("byte rate        176400 bytes/s"), "{report}");
        assert!(report.contains("file size        76 bytes"), "{report}");
        assert!(report.contains("sample frames    8 (0.000181 s)"), "{report}");
        assert!(
            report.contains("ffmpeg -f s16le -ar 44100 -ac 2 -i in.pcm out.wav"),
            "{report}"
        );
        assert!(
            report.contains("sox -t raw -r 44100 -b 16 -e signed-integer -L -c 2 in.pcm out.wav"),
            "{report}"
        );
        assert!(report.contains("Signed 16-bit PCM, little-endian, 2 channels"), "{report}");
    }

    #[test]
    fn info_flags_an_incomplete_trailing_frame() {
        // 5 bytes at 16-bit stereo = one 4-byte frame plus a stray byte.
        let report = run("0102030405", "hex", 16000, 2, "16", "signed", "little", 0, 0, "info")
            .unwrap();
        assert!(report.contains("usable           5 bytes = 1 sample frame\n"), "{report}");
        assert!(report.contains("leftover         1 bytes (an incomplete trailing frame"), "{report}");
    }

    #[test]
    fn rejects_impossible_depth_and_encoding_pairings() {
        let err = run(DEMO_HEX, "hex", 44100, 2, "16", "float", "little", 0, 0, "hex").unwrap_err();
        assert!(err.contains("encoding=float needs bit_depth 32 or 64"), "{err}");
        let err = run(DEMO_HEX, "hex", 44100, 2, "64", "signed", "little", 0, 0, "hex").unwrap_err();
        assert!(err.contains("bit_depth 64 is float-only in WAVE"), "{err}");
        let err = run(DEMO_HEX, "hex", 44100, 2, "12", "signed", "little", 0, 0, "hex").unwrap_err();
        assert_eq!(err, "bit_depth must be one of 8, 16, 24, 32, 64, got \"12\"");
        let err = run(DEMO_HEX, "hex", 44100, 2, "16", "signed", "middle", 0, 0, "hex").unwrap_err();
        assert_eq!(err, "byte_order must be one of little, big, got \"middle\"");
        let err = run(DEMO_HEX, "hex", 44100, 2, "16", "signed", "little", 0, 0, "yaml").unwrap_err();
        assert_eq!(
            err,
            "output must be one of data_url, base64, hex, info, got \"yaml\""
        );
    }

    #[test]
    fn rejects_out_of_range_rates_channels_and_windows() {
        let err = run(DEMO_HEX, "hex", 0, 2, "16", "signed", "little", 0, 0, "hex").unwrap_err();
        assert!(err.contains("sample_rate must be between 1 and 768000"), "{err}");
        let err = run(DEMO_HEX, "hex", 1_000_000, 2, "16", "signed", "little", 0, 0, "hex")
            .unwrap_err();
        assert!(err.contains("sample_rate must be between"), "{err}");
        let err = run(DEMO_HEX, "hex", 44100, 0, "16", "signed", "little", 0, 0, "hex").unwrap_err();
        assert!(err.contains("channels must be between 1 and 16"), "{err}");
        let err = run(DEMO_HEX, "hex", 44100, 2, "16", "signed", "little", 32, 0, "hex")
            .unwrap_err();
        assert!(err.contains("skip_bytes 32 is at or past the end"), "{err}");
        // Three bytes cannot make a 24-bit stereo frame.
        let err = run("010203", "hex", 44100, 2, "24", "signed", "little", 0, 0, "hex").unwrap_err();
        assert!(err.contains("not enough data for one sample frame"), "{err}");
        assert!(err.contains("is 6 bytes"), "{err}");
    }

    #[test]
    fn rejects_unusable_input_payloads() {
        let err = run("", "auto", 44100, 2, "16", "signed", "little", 0, 0, "hex").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
        let err = run("zz!!", "base64", 44100, 2, "16", "signed", "little", 0, 0, "hex").unwrap_err();
        assert!(err.contains("not valid base64"), "{err}");
        let err = run("0102030", "hex", 44100, 1, "8", "signed", "little", 0, 0, "hex").unwrap_err();
        assert!(err.contains("even number of digits"), "{err}");
    }

    #[test]
    fn twenty_four_bit_samples_keep_all_three_bytes() {
        // Two 24-bit mono frames, big-endian: 0x123456, 0xfedcba.
        let out = run("123456fedcba", "hex", 48000, 1, "24", "signed", "big", 0, 0, "hex").unwrap();
        assert_eq!(&out[68..72], "1800"); // 24 bits
        assert_eq!(&out[64..68], "0300"); // block align 3
        assert_eq!(&out[88..], "563412badcfe");
    }
}
