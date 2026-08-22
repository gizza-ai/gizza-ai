//! zstd-decompress core — decode a Zstandard-compressed (RFC 8878) payload that
//! has been pasted as **Base64** or **hex** back to the original bytes, and
//! render those bytes as text, hex, or Base64.
//!
//! This is the inline/readable half of the zstd story in this repo: the
//! `file-compressor` block takes a real `.zst` FILE (url/ref) and hands back a
//! download, while this block takes a pasted blob — an HTTP `content-encoding:
//! zstd` body, a Kafka/ClickHouse record, a log chunk — and prints what is
//! inside it, plus what the frame headers say about it.
//!
//! Unlike Brotli, zstd **has** a magic number (`28 B5 2F FD`), so the transport
//! encoding can be verified rather than guessed and a wrong-codec blob can be
//! named up front instead of after a failed decode.
//!
//! The stream is driven **frame by frame** through `ruzstd`'s low-level
//! `FrameDecoder` rather than through the one-shot `StreamingDecoder`, because a
//! zstd stream is legally a *sequence* of frames (`zstd -c a b > out.zst`,
//! parallel compressors) and `StreamingDecoder` stops after the first one — it
//! would silently return truncated data. Driving the frames directly also
//! exposes skippable frames, the frame headers, and the trailing xxHash-32
//! content checksum, which this block verifies.
//!
//! Pure compute, no wafer/wasm-bindgen deps — shared by the chat skill block and
//! the web page. Runs on every backend including the chat Service Worker.

use base64::engine::general_purpose::{
    STANDARD as B64, STANDARD_NO_PAD as B64_NP, URL_SAFE_NO_PAD as B64_URL,
};
use base64::Engine;
use ruzstd::decoding::{BlockDecodingStrategy, FrameDecoder};

/// Max compressed input accepted, in bytes. The payload lives in wasm linear
/// memory, so an unbounded paste would OOM-trap the tab instead of erroring.
pub const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// Max decompressed output, in bytes — the decompression-bomb guard. Matches
/// `file-compressor` / `brotli-decompress` / `lz4-decompress` / `lzma-decompress`.
pub const MAX_OUTPUT_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

/// Magic number of a zstd data frame, as it reads little-endian (`28 B5 2F FD`).
const ZSTD_MAGIC: u32 = 0xFD2F_B528;
/// Skippable frames use any magic in this inclusive range (RFC 8878 §3.1.2).
const SKIPPABLE_LO: u32 = 0x184D_2A50;
const SKIPPABLE_HI: u32 = 0x184D_2A5F;

/// How the pasted payload in `data` is encoded.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Encoding {
    /// Decode as both hex and Base64 and keep whichever yields zstd bytes.
    Auto,
    /// Standard or URL-safe Base64 (RFC 4648), padding optional.
    Base64,
    /// A hex string; ASCII whitespace and an optional `0x` prefix are ignored.
    Hex,
}

impl Encoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "auto" => Ok(Encoding::Auto),
            "base64" | "b64" => Ok(Encoding::Base64),
            "hex" => Ok(Encoding::Hex),
            other => Err(format!(
                "invalid encoding {other:?}: expected \"auto\", \"base64\", or \"hex\""
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Encoding::Auto => "auto",
            Encoding::Base64 => "base64",
            Encoding::Hex => "hex",
        }
    }
}

/// How to render the decompressed bytes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// UTF-8 text — the default; errors if the bytes are not valid UTF-8.
    Text,
    /// A lowercase hex string.
    Hex,
    /// Standard Base64 (RFC 4648), padded.
    Base64,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "text" => Ok(Output::Text),
            "hex" => Ok(Output::Hex),
            "base64" | "b64" => Ok(Output::Base64),
            other => Err(format!(
                "invalid output {other:?}: expected \"text\", \"hex\", or \"base64\""
            )),
        }
    }

    fn render(self, bytes: &[u8]) -> Result<String, String> {
        match self {
            Output::Text => String::from_utf8(bytes.to_vec()).map_err(|e| {
                format!(
                    "the decompressed data is not valid UTF-8 text (bad byte at offset {}) \
                     — set output to \"hex\" or \"base64\" to view the raw bytes",
                    e.utf8_error().valid_up_to()
                )
            }),
            Output::Hex => Ok(to_hex(bytes)),
            Output::Base64 => Ok(B64.encode(bytes)),
        }
    }
}

/// What a single zstd **data** frame declared and produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataFrame {
    /// Bytes the frame occupied in the compressed stream, header and checksum included.
    pub compressed_bytes: usize,
    /// Bytes the frame produced.
    pub decompressed_bytes: usize,
    /// Decoder window the frame header asks for.
    pub window_size: u64,
    /// Single-segment frames carry no window descriptor; the window is the content size.
    pub single_segment: bool,
    /// The content size declared in the header, when the encoder declared one at all.
    pub declared_content_size: Option<u64>,
    /// The dictionary the frame needs, if it needs one.
    pub dictionary_id: Option<u32>,
    /// The trailing xxHash-32 content checksum, when the frame carries one.
    pub checksum: Option<u32>,
}

/// One frame of the stream, in stream order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Frame {
    /// A zstd data frame that was decoded.
    Data(DataFrame),
    /// A skippable frame (magic `0x184D2A50`–`0x184D2A5F`) that was stepped over.
    Skippable {
        magic: u32,
        /// Size of the application payload, excluding the 8-byte skippable header.
        payload_bytes: usize,
    },
}

/// The result of a decompression, before rendering.
#[derive(Clone, Debug)]
pub struct Decoded {
    /// The decompressed bytes, all data frames concatenated.
    pub bytes: Vec<u8>,
    /// Size of the compressed payload that was fed to the zstd decoder.
    pub compressed_bytes: usize,
    /// Which transport encoding was actually used (useful when `encoding = auto`).
    pub encoding: Encoding,
    /// Every frame found in the stream, in order.
    pub frames: Vec<Frame>,
}

impl Decoded {
    /// How many of the frames were zstd data frames.
    pub fn data_frames(&self) -> usize {
        self.frames
            .iter()
            .filter(|f| matches!(f, Frame::Data(_)))
            .count()
    }

    /// How many of the frames were skippable frames.
    pub fn skippable_frames(&self) -> usize {
        self.frames.len() - self.data_frames()
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Strip ASCII whitespace (so a line-wrapped paste keeps working).
fn squeeze(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_whitespace()).collect()
}

/// Parse a hex string into bytes, ignoring ASCII whitespace and an optional
/// `0x` prefix. Rejects odd length and non-hex digits.
fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned = squeeze(s);
    let cleaned = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned)
        .to_string();
    if cleaned.is_empty() {
        return Err("hex input is empty".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex input has an odd number of digits ({}); each byte needs two",
            cleaned.len()
        ));
    }
    (0..cleaned.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&cleaned[i..i + 2], 16)
                .map_err(|_| format!("invalid hex byte {:?}", &cleaned[i..i + 2]))
        })
        .collect()
}

/// Decode standard Base64, falling back to unpadded and URL-safe alphabets so a
/// payload lifted out of a URL or a JSON field still works.
fn from_base64(s: &str) -> Result<Vec<u8>, String> {
    let cleaned = squeeze(s);
    if cleaned.is_empty() {
        return Err("Base64 input is empty".into());
    }
    B64.decode(cleaned.as_bytes())
        .or_else(|_| B64_NP.decode(cleaned.trim_end_matches('=').as_bytes()))
        .or_else(|_| B64_URL.decode(cleaned.trim_end_matches('=').as_bytes()))
        .map_err(|e| format!("invalid Base64 input: {e}"))
}

/// Read a little-endian u32 at `off`, if the slice is long enough.
fn le_u32(b: &[u8], off: usize) -> Option<u32> {
    b.get(off..off + 4)
        .map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Does `b` start with a zstd data-frame or skippable-frame magic number?
fn looks_like_zstd(b: &[u8]) -> bool {
    match le_u32(b, 0) {
        Some(ZSTD_MAGIC) => true,
        Some(m) => (SKIPPABLE_LO..=SKIPPABLE_HI).contains(&m),
        None => false,
    }
}

/// Identify a non-zstd compressed blob from its magic bytes, so the error can
/// name the codec AND the sibling tool that handles it. zstd has its own magic
/// number, so this check runs **up front** — a payload that is not zstd is
/// diagnosed before any decode is attempted.
fn other_codec(bytes: &[u8]) -> Option<(&'static str, &'static str)> {
    let b = bytes;
    let starts = |sig: &[u8]| b.len() >= sig.len() && &b[..sig.len()] == sig;

    if starts(&[0x1F, 0x8B]) {
        return Some(("gzip", "gunzip"));
    }
    if starts(&[0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00]) {
        return Some(("xz", "lzma-decompress"));
    }
    if starts(&[0x5D, 0x00, 0x00]) {
        return Some(("raw LZMA", "lzma-decompress"));
    }
    if starts(&[0x04, 0x22, 0x4D, 0x18]) {
        return Some(("LZ4", "lz4-decompress"));
    }
    if starts(&[0x42, 0x5A, 0x68]) {
        return Some(("bzip2", "archive-extractor"));
    }
    if starts(&[0x50, 0x4B, 0x03, 0x04]) || starts(&[0x50, 0x4B, 0x05, 0x06]) {
        return Some(("ZIP", "unzip"));
    }
    if starts(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C]) {
        return Some(("7-Zip", "7z-extract"));
    }
    if b.len() > 262 && &b[257..262] == b"ustar" {
        return Some(("tar", "archive-extractor"));
    }
    // zlib: CMF/FLG where the low nibble of CMF is 8 (deflate) and the 16-bit
    // big-endian header is a multiple of 31.
    if b.len() >= 2 && b[0] & 0x0F == 0x08 && ((b[0] as u16) << 8 | b[1] as u16) % 31 == 0 {
        return Some(("zlib", "raw-inflate"));
    }
    None
}

/// The fields of a zstd frame header this block reports on.
struct Header {
    window_size: u64,
    single_segment: bool,
    declared_content_size: Option<u64>,
    dictionary_id: Option<u32>,
}

/// Parse a zstd frame header (RFC 8878 §3.1.1) from the start of `b`.
///
/// `ruzstd` parses the same bytes internally but keeps the parsed header
/// private, so the reportable fields — window size, declared content size,
/// dictionary ID — are read here directly.
fn parse_frame_header(b: &[u8], frame_no: usize) -> Result<Header, String> {
    let short = || format!("frame {frame_no} ends inside its header — the payload is truncated");
    let desc = *b.get(4).ok_or_else(short)?;

    let fcs_flag = desc >> 6;
    let single_segment = (desc >> 5) & 1 == 1;
    let reserved = (desc >> 3) & 1 == 1;
    let dict_flag = desc & 0x3;
    if reserved {
        return Err(format!(
            "frame {frame_no} sets the reserved bit in its header descriptor (0x{desc:02x}) — \
             this is not a valid zstd frame"
        ));
    }

    let mut p = 5usize;
    let mut window_descriptor = 0u8;
    if !single_segment {
        window_descriptor = *b.get(p).ok_or_else(short)?;
        p += 1;
    }

    let did_len = [0usize, 1, 2, 4][dict_flag as usize];
    let mut dictionary_id = 0u32;
    for i in 0..did_len {
        dictionary_id |= (*b.get(p + i).ok_or_else(short)? as u32) << (8 * i);
    }
    p += did_len;

    let fcs_len = match fcs_flag {
        0 if single_segment => 1,
        0 => 0,
        1 => 2,
        2 => 4,
        _ => 8,
    };
    let mut fcs = 0u64;
    for i in 0..fcs_len {
        fcs |= (*b.get(p + i).ok_or_else(short)? as u64) << (8 * i);
    }
    // A 2-byte Frame_Content_Size is stored biased by 256 (RFC 8878 §3.1.1.1.4).
    if fcs_len == 2 {
        fcs += 256;
    }
    let declared_content_size = (fcs_len != 0).then_some(fcs);

    let window_size = if single_segment {
        fcs
    } else {
        let exp = u64::from(window_descriptor >> 3);
        let mantissa = u64::from(window_descriptor & 0x7);
        let window_base = 1u64 << (10 + exp);
        window_base + (window_base / 8) * mantissa
    };

    Ok(Header {
        window_size,
        single_segment,
        declared_content_size,
        dictionary_id: (dictionary_id != 0).then_some(dictionary_id),
    })
}

/// Decode one zstd data frame starting at `b[0]`, appending its output to `out`.
/// Returns the frame report and how many bytes of `b` the frame occupied.
fn decode_one_frame(
    dec: &mut FrameDecoder,
    b: &[u8],
    frame_no: usize,
    out: &mut Vec<u8>,
) -> Result<(DataFrame, usize), String> {
    let header = parse_frame_header(b, frame_no)?;
    if let Some(id) = header.dictionary_id {
        return Err(format!(
            "frame {frame_no} was compressed with dictionary 0x{id:08x} and cannot be decoded \
             without it — a dictionary-compressed payload needs the same dictionary file the \
             encoder used, which this paste-in tool has nowhere to accept"
        ));
    }

    let produced_before = out.len();
    let mut cursor = std::io::Cursor::new(b);
    dec.reset(&mut cursor)
        .map_err(|e| format!("frame {frame_no} has an unusable header: {e}"))?;

    // Drive the frame to completion, draining as we go so a bomb is caught
    // early instead of after the whole thing is in memory.
    while !dec.is_finished() {
        dec.decode_blocks(&mut cursor, BlockDecodingStrategy::UptoBytes(256 * 1024))
            .map_err(|e| format!("zstd decompression failed in frame {frame_no}: {e}"))?;
        if let Some(chunk) = dec.collect() {
            out.extend_from_slice(&chunk);
            check_output_cap(out.len())?;
        }
    }
    while dec.can_collect() > 0 {
        match dec.collect() {
            Some(chunk) => {
                out.extend_from_slice(&chunk);
                check_output_cap(out.len())?;
            }
            None => break,
        }
    }

    // The checksum is only meaningful once every byte has been drained, because
    // the running xxHash is fed as bytes leave the decode buffer.
    let checksum = dec.get_checksum_from_data();
    if let Some(expected) = checksum {
        if let Some(actual) = dec.get_calculated_checksum() {
            if expected != actual {
                return Err(format!(
                    "frame {frame_no} failed its content checksum: the frame declares \
                     0x{expected:08x} but the decoded bytes hash to 0x{actual:08x} — the payload \
                     is corrupt"
                ));
            }
        }
    }

    let consumed = dec.bytes_read_from_source() as usize;
    Ok((
        DataFrame {
            compressed_bytes: consumed,
            decompressed_bytes: out.len() - produced_before,
            window_size: header.window_size,
            single_segment: header.single_segment,
            declared_content_size: header.declared_content_size,
            dictionary_id: None,
            checksum,
        },
        consumed,
    ))
}

fn check_output_cap(len: usize) -> Result<(), String> {
    if len as u64 > MAX_OUTPUT_BYTES {
        return Err(format!(
            "the decompressed data exceeds the {} MiB limit of this browser-local tool",
            MAX_OUTPUT_BYTES / (1024 * 1024)
        ));
    }
    Ok(())
}

/// Decode a whole zstd stream: every data frame concatenated, skippable frames
/// stepped over and reported.
fn decompress_stream(data: &[u8]) -> Result<(Vec<u8>, Vec<Frame>), String> {
    if let Some((codec, tool)) = other_codec(data) {
        return Err(format!(
            "this payload is {codec}-compressed, not zstd — use the {tool} tool instead"
        ));
    }
    if !looks_like_zstd(data) {
        return Err(format!(
            "this is not a zstd payload: it starts with {} instead of the zstd magic number \
             28 b5 2f fd",
            match data.len() {
                0 => "no bytes at all".to_string(),
                _ => format!("{}", to_hex(&data[..data.len().min(4)])),
            }
        ));
    }

    let mut out = Vec::new();
    let mut frames = Vec::new();
    let mut dec = FrameDecoder::new();
    let mut pos = 0usize;

    while pos < data.len() {
        let frame_no = frames.len() + 1;
        let Some(magic) = le_u32(data, pos) else {
            return Err(format!(
                "{} trailing byte(s) after frame {} are not a complete frame header — the \
                 payload looks truncated",
                data.len() - pos,
                frames.len()
            ));
        };

        if (SKIPPABLE_LO..=SKIPPABLE_HI).contains(&magic) {
            let size = le_u32(data, pos + 4).ok_or_else(|| {
                format!("skippable frame {frame_no} is truncated before its size field")
            })? as usize;
            let end = pos
                .checked_add(8)
                .and_then(|p| p.checked_add(size))
                .filter(|e| *e <= data.len())
                .ok_or_else(|| {
                    format!(
                        "skippable frame {frame_no} declares {size} bytes of payload but only \
                         {} remain",
                        data.len() - pos - 8.min(data.len() - pos)
                    )
                })?;
            frames.push(Frame::Skippable {
                magic,
                payload_bytes: size,
            });
            pos = end;
            continue;
        }

        if magic != ZSTD_MAGIC {
            return Err(format!(
                "byte offset {pos} starts neither a zstd frame nor a skippable frame (found \
                 magic 0x{magic:08x}) — the stream has {} trailing byte(s) that are not part of \
                 any frame",
                data.len() - pos
            ));
        }

        let (info, consumed) = decode_one_frame(&mut dec, &data[pos..], frame_no, &mut out)?;
        if consumed == 0 {
            return Err(format!(
                "frame {frame_no} made no progress — the payload is malformed"
            ));
        }
        frames.push(Frame::Data(info));
        pos += consumed;
    }

    if !frames.iter().any(|f| matches!(f, Frame::Data(_))) {
        return Err(
            "this payload contains only skippable frames — there is no compressed data in it"
                .into(),
        );
    }
    Ok((out, frames))
}

/// Decode `data` from `encoding` and zstd-decompress it.
pub fn decode(data: &str, encoding: Encoding) -> Result<Decoded, String> {
    if data.trim().is_empty() {
        return Err("input is empty: paste a zstd payload as Base64 or hex".into());
    }

    let candidates: Vec<(Encoding, Result<Vec<u8>, String>)> = match encoding {
        Encoding::Base64 => vec![(Encoding::Base64, from_base64(data))],
        Encoding::Hex => vec![(Encoding::Hex, from_hex(data))],
        // A short Base64 string can consist entirely of hex characters, so
        // `auto` must not pick by shape — it decodes both ways and keeps
        // whichever actually yields a zstd magic number.
        Encoding::Auto => vec![
            (Encoding::Hex, from_hex(data)),
            (Encoding::Base64, from_base64(data)),
        ],
    };

    let decoded: Vec<(Encoding, Vec<u8>)> = candidates
        .iter()
        .filter_map(|(e, r)| r.as_ref().ok().map(|b| (*e, b.clone())))
        .collect();

    if decoded.is_empty() {
        return Err(match encoding {
            Encoding::Auto => format!(
                "could not read the input as hex or Base64 — hex: {}; Base64: {}",
                candidates[0].1.as_ref().err().cloned().unwrap_or_default(),
                candidates[1].1.as_ref().err().cloned().unwrap_or_default()
            ),
            _ => candidates[0].1.as_ref().err().cloned().unwrap_or_default(),
        });
    }

    // Prefer the transport whose bytes actually start with a zstd magic number;
    // otherwise keep the first that decoded, so the error can still name the
    // real codec.
    let (enc, bytes) = decoded
        .iter()
        .find(|(_, b)| looks_like_zstd(b))
        .unwrap_or(&decoded[0]);

    if bytes.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "compressed input is {} bytes, over the {} MiB limit of this browser-local tool",
            bytes.len(),
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }

    let (out, frames) = decompress_stream(bytes)?;
    Ok(Decoded {
        bytes: out,
        compressed_bytes: bytes.len(),
        encoding: *enc,
        frames,
    })
}

fn plural(n: usize, one: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {one}s")
    }
}

/// The size summary prepended to the payload when `stats` is on.
fn stats_block(d: &Decoded) -> String {
    let compressed = d.compressed_bytes;
    let decompressed = d.bytes.len();
    let mut s = format!("Compressed:   {compressed} bytes\nDecompressed: {decompressed} bytes\n");
    if compressed > 0 && decompressed > 0 {
        let ratio = decompressed as f64 / compressed as f64;
        let saved = (1.0 - compressed as f64 / decompressed as f64) * 100.0;
        s.push_str(&format!(
            "Ratio:        {ratio:.2}x (decompressed / compressed)\nSpace saved:  {saved:.1}%\n"
        ));
    }
    s.push_str(&format!(
        "Frames:       {}",
        plural(d.data_frames(), "data frame")
    ));
    let skipped = d.skippable_frames();
    if skipped > 0 {
        s.push_str(&format!(", {}", plural(skipped, "skippable frame")));
    }
    s.push_str("\n\n");
    s
}

/// The per-frame structural report prepended to the payload when `frame_info` is on.
fn frame_info_block(d: &Decoded) -> String {
    let mut s = String::new();
    for (i, f) in d.frames.iter().enumerate() {
        let n = i + 1;
        match f {
            Frame::Data(df) => {
                s.push_str(&format!("Frame {n} — zstd data frame\n"));
                s.push_str(&format!(
                    "  Compressed:       {} bytes\n",
                    df.compressed_bytes
                ));
                s.push_str(&format!(
                    "  Decompressed:     {} bytes\n",
                    df.decompressed_bytes
                ));
                s.push_str(&format!(
                    "  Window size:      {} bytes{}\n",
                    df.window_size,
                    if df.single_segment {
                        " (single-segment frame)"
                    } else {
                        ""
                    }
                ));
                s.push_str(&match df.declared_content_size {
                    Some(v) => format!("  Declared size:    {v} bytes\n"),
                    None => "  Declared size:    not declared by the encoder\n".to_string(),
                });
                s.push_str(&match df.dictionary_id {
                    Some(id) => format!("  Dictionary ID:    0x{id:08x}\n"),
                    None => "  Dictionary ID:    none\n".to_string(),
                });
                s.push_str(&match df.checksum {
                    Some(c) => format!("  Content checksum: 0x{c:08x}, verified\n"),
                    None => "  Content checksum: not present in this frame\n".to_string(),
                });
            }
            Frame::Skippable {
                magic,
                payload_bytes,
            } => {
                s.push_str(&format!(
                    "Frame {n} — skippable frame (magic 0x{magic:08x})\n"
                ));
                s.push_str(&format!(
                    "  Skipped:          {payload_bytes} bytes of application metadata\n"
                ));
            }
        }
        s.push('\n');
    }
    s
}

/// Decompress a pasted zstd payload and render the result.
///
/// - `data` — the zstd payload, encoded per `encoding`.
/// - `encoding` (`"auto"` | `"base64"` | `"hex"`, blank → `"auto"`).
/// - `output` (`"text"` | `"hex"` | `"base64"`, blank → `"text"`).
/// - `stats` — prepend a compressed/decompressed size summary.
/// - `frame_info` — prepend a per-frame structural report.
pub fn run(
    data: &str,
    encoding: &str,
    output: &str,
    stats: bool,
    frame_info: bool,
) -> Result<String, String> {
    let enc = Encoding::parse(encoding)?;
    let out_fmt = Output::parse(output)?;
    let decoded = decode(data, enc)?;
    let rendered = out_fmt.render(&decoded.bytes)?;

    let mut prefix = String::new();
    if stats {
        prefix.push_str(&stats_block(&decoded));
    }
    if frame_info {
        prefix.push_str(&frame_info_block(&decoded));
    }
    let _ = enc.label();
    Ok(format!("{prefix}{rendered}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ruzstd::encoding::{compress_to_vec, CompressionLevel};

    // ── fixtures produced by the reference `zstd` CLI (v1.5, level 19) ───────
    // These prove interop with the real encoder, which the pure-Rust encoder
    // used elsewhere in these tests cannot. All carry a content checksum,
    // which `zstd` writes by default.

    /// `{"user":"ada","roles":["admin","deploy"],"active":true}`
    const JSON_B64: &str =
        "KLUv/SQ3nQEAcsMLEaBtDNw9rldW0gajapZ9/r8hcx9z4YXGER2Mozpry+BkUNFFuO1RbLAWPcy2vWUA22iyIw==";
    const JSON_TEXT: &str = r#"{"user":"ada","roles":["admin","deploy"],"active":true}"#;
    /// `hello zstd`
    const SMALL_HEX: &str = "28b52ffd240a51000068656c6c6f207a737464cfdb609c";
    /// `frame one. ` followed by `frame two.`, two frames concatenated.
    const MULTI_B64: &str = "KLUv/SQLWQAAZnJhbWUgb25lLiC8bgb+KLUv/SQKUQAAZnJhbWUgdHdvLsJz+sY=";
    /// A skippable frame carrying `gizza-meta:v1`, then the `hello zstd` frame.
    const SKIP_B64: &str = "UCpNGA0AAABnaXp6YS1tZXRhOnYxKLUv/SQKUQAAaGVsbG8genN0ZM/bYJw=";
    /// `'gizza zstd decompress ' * 3000` — 66 000 bytes in, 46 bytes out.
    const BIG_B64: &str = "KLUv/aTQAQEA9QAAsGdpenphIHpzdGQgZGVjb21wcmVzcyABAG4DMp9NjjSaNQ==";

    fn zs(data: &[u8]) -> Vec<u8> {
        compress_to_vec(data, CompressionLevel::Fastest)
    }

    fn zs_b64(data: &[u8]) -> String {
        B64.encode(zs(data))
    }

    // ── happy paths ─────────────────────────────────────────────────────────

    #[test]
    fn decompresses_reference_cli_base64_payload_to_text() {
        assert_eq!(
            run(JSON_B64, "base64", "text", false, false).unwrap(),
            JSON_TEXT
        );
    }

    #[test]
    fn decompresses_reference_cli_hex_payload_to_text() {
        assert_eq!(
            run(SMALL_HEX, "hex", "text", false, false).unwrap(),
            "hello zstd"
        );
    }

    #[test]
    fn decompresses_round_tripped_payload() {
        let payload = zs_b64(b"round-tripped through the pure-Rust encoder");
        assert_eq!(
            run(&payload, "base64", "text", false, false).unwrap(),
            "round-tripped through the pure-Rust encoder"
        );
    }

    #[test]
    fn auto_detects_base64() {
        assert_eq!(
            run(JSON_B64, "auto", "text", false, false).unwrap(),
            JSON_TEXT
        );
        assert_eq!(
            decode(JSON_B64, Encoding::Auto).unwrap().encoding,
            Encoding::Base64
        );
    }

    #[test]
    fn auto_detects_hex() {
        assert_eq!(
            run(SMALL_HEX, "auto", "text", false, false).unwrap(),
            "hello zstd"
        );
        assert_eq!(
            decode(SMALL_HEX, Encoding::Auto).unwrap().encoding,
            Encoding::Hex
        );
    }

    #[test]
    fn auto_picks_the_transport_that_yields_the_magic_number() {
        // The Base64 form of this payload is a candidate hex string too — the
        // magic-number check, not the string shape, decides.
        let payload = zs_b64(b"ambiguity resolved by the magic number, not by shape");
        let d = decode(&payload, Encoding::Auto).unwrap();
        assert_eq!(d.encoding, Encoding::Base64);
        assert_eq!(
            d.bytes,
            b"ambiguity resolved by the magic number, not by shape"
        );
    }

    #[test]
    fn blank_options_fall_back_to_defaults() {
        assert_eq!(run(JSON_B64, "", "", false, false).unwrap(), JSON_TEXT);
    }

    #[test]
    fn accepts_line_wrapped_and_prefixed_input() {
        let wrapped = JSON_B64
            .as_bytes()
            .chunks(20)
            .map(|c| String::from_utf8(c.to_vec()).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            run(&wrapped, "base64", "text", false, false).unwrap(),
            JSON_TEXT
        );
        let prefixed = format!("0x{}", SMALL_HEX.to_uppercase());
        assert_eq!(
            run(&prefixed, "hex", "text", false, false).unwrap(),
            "hello zstd"
        );
    }

    #[test]
    fn url_safe_base64_is_accepted() {
        let raw = zs(b"payload lifted straight out of a query string ~~~ \xff\xfe");
        let url = B64_URL.encode(&raw);
        assert_eq!(
            decode(&url, Encoding::Base64).unwrap().bytes,
            b"payload lifted straight out of a query string ~~~ \xff\xfe"
        );
    }

    #[test]
    fn binary_output_as_hex_and_base64() {
        let raw: Vec<u8> = (0..=255u8).collect();
        let payload = zs_b64(&raw);
        assert_eq!(
            run(&payload, "base64", "hex", false, false).unwrap(),
            to_hex(&raw)
        );
        assert_eq!(
            run(&payload, "base64", "base64", false, false).unwrap(),
            B64.encode(&raw)
        );
    }

    #[test]
    fn empty_payload_round_trips() {
        let payload = zs_b64(b"");
        assert_eq!(run(&payload, "base64", "text", false, false).unwrap(), "");
    }

    #[test]
    fn decompresses_a_large_repetitive_payload() {
        let expected = "gizza zstd decompress ".repeat(3000);
        let d = decode(BIG_B64, Encoding::Base64).unwrap();
        assert_eq!(d.bytes, expected.as_bytes());
        assert_eq!(d.compressed_bytes, 46);
    }

    // ── multi-frame + skippable frames ──────────────────────────────────────

    #[test]
    fn concatenated_frames_are_all_decoded() {
        // The whole point of driving FrameDecoder directly: ruzstd's
        // StreamingDecoder stops after the first frame and would return only
        // "frame one. " here, with no error at all.
        let d = decode(MULTI_B64, Encoding::Base64).unwrap();
        assert_eq!(d.bytes, b"frame one. frame two.");
        assert_eq!(d.data_frames(), 2);
        assert_eq!(d.skippable_frames(), 0);
    }

    #[test]
    fn many_concatenated_frames_decode_in_order() {
        let mut stream = Vec::new();
        let mut expected = Vec::new();
        for i in 0..8u8 {
            let part = format!("part-{i};");
            stream.extend_from_slice(&zs(part.as_bytes()));
            expected.extend_from_slice(part.as_bytes());
        }
        let d = decode(&B64.encode(&stream), Encoding::Base64).unwrap();
        assert_eq!(d.bytes, expected);
        assert_eq!(d.data_frames(), 8);
    }

    #[test]
    fn skippable_frames_are_stepped_over_and_reported() {
        let d = decode(SKIP_B64, Encoding::Base64).unwrap();
        assert_eq!(d.bytes, b"hello zstd");
        assert_eq!(d.data_frames(), 1);
        assert_eq!(d.skippable_frames(), 1);
        assert!(matches!(
            d.frames[0],
            Frame::Skippable {
                magic: 0x184D_2A50,
                payload_bytes: 13
            }
        ));
    }

    #[test]
    fn a_stream_of_only_skippable_frames_is_an_error() {
        let mut stream = Vec::new();
        stream.extend_from_slice(&0x184D_2A50u32.to_le_bytes());
        stream.extend_from_slice(&4u32.to_le_bytes());
        stream.extend_from_slice(b"meta");
        let e = decode(&B64.encode(&stream), Encoding::Base64).unwrap_err();
        assert!(e.contains("only skippable frames"), "{e}");
    }

    #[test]
    fn a_skippable_frame_longer_than_the_stream_is_an_error() {
        let mut stream = Vec::new();
        stream.extend_from_slice(&0x184D_2A50u32.to_le_bytes());
        stream.extend_from_slice(&999u32.to_le_bytes());
        stream.extend_from_slice(b"short");
        let e = decode(&B64.encode(&stream), Encoding::Base64).unwrap_err();
        assert!(e.contains("999"), "{e}");
    }

    // ── stats + frame info ──────────────────────────────────────────────────

    #[test]
    fn stats_report_sizes_ratio_and_frame_counts() {
        let out = run(BIG_B64, "base64", "text", true, false).unwrap();
        assert!(out.contains("Compressed:   46 bytes"), "{out}");
        assert!(out.contains("Decompressed: 66000 bytes"), "{out}");
        assert!(
            out.contains("Ratio:        1434.78x (decompressed / compressed)"),
            "{out}"
        );
        assert!(out.contains("Space saved:  99.9%"), "{out}");
        assert!(out.contains("Frames:       1 data frame"), "{out}");
        assert!(
            out.ends_with(&"gizza zstd decompress ".repeat(3000)),
            "{out}"
        );
    }

    #[test]
    fn stats_count_skippable_frames_separately() {
        let out = run(SKIP_B64, "base64", "text", true, false).unwrap();
        assert!(
            out.contains("Frames:       1 data frame, 1 skippable frame"),
            "{out}"
        );
    }

    #[test]
    fn frame_info_reports_the_header_fields_of_a_reference_frame() {
        let out = run(JSON_B64, "base64", "text", false, true).unwrap();
        assert!(out.contains("Frame 1 — zstd data frame"), "{out}");
        assert!(out.contains("Compressed:       64 bytes"), "{out}");
        assert!(out.contains("Decompressed:     55 bytes"), "{out}");
        assert!(
            out.contains("Window size:      55 bytes (single-segment frame)"),
            "{out}"
        );
        assert!(out.contains("Declared size:    55 bytes"), "{out}");
        assert!(out.contains("Dictionary ID:    none"), "{out}");
        assert!(out.contains("Content checksum: 0x"), "{out}");
        assert!(out.contains(", verified"), "{out}");
        assert!(out.ends_with(JSON_TEXT), "{out}");
    }

    #[test]
    fn frame_info_reports_every_frame_of_a_multi_frame_stream() {
        let out = run(SKIP_B64, "base64", "text", false, true).unwrap();
        assert!(
            out.contains("Frame 1 — skippable frame (magic 0x184d2a50)"),
            "{out}"
        );
        assert!(
            out.contains("Skipped:          13 bytes of application metadata"),
            "{out}"
        );
        assert!(out.contains("Frame 2 — zstd data frame"), "{out}");
        assert!(out.ends_with("hello zstd"), "{out}");
    }

    #[test]
    fn frame_info_reports_an_undeclared_content_size() {
        // The pure-Rust encoder writes multi-segment frames with no declared
        // content size — the report must say so rather than print a zero.
        let payload = zs_b64(&b"undeclared content size ".repeat(200));
        let out = run(&payload, "base64", "hex", false, true).unwrap();
        assert!(
            out.contains("Declared size:    not declared by the encoder"),
            "{out}"
        );
        assert!(out.contains("Content checksum: 0x"), "{out}");
        assert!(out.contains(", verified"), "{out}");
    }

    #[test]
    fn stats_and_frame_info_can_be_combined() {
        let out = run(MULTI_B64, "base64", "text", true, true).unwrap();
        let stats_at = out.find("Compressed:   ").unwrap();
        let frames_at = out.find("Frame 1 — zstd data frame").unwrap();
        assert!(stats_at < frames_at, "{out}");
        assert!(out.contains("Frame 2 — zstd data frame"), "{out}");
        assert!(out.ends_with("frame one. frame two."), "{out}");
    }

    // ── error paths ─────────────────────────────────────────────────────────

    #[test]
    fn empty_input_is_an_error() {
        let e = run("   ", "auto", "text", false, false).unwrap_err();
        assert!(e.contains("empty"), "{e}");
    }

    #[test]
    fn invalid_encoding_is_an_error() {
        let e = run("AA", "rot13", "text", false, false).unwrap_err();
        assert!(e.contains("invalid encoding") && e.contains("auto"), "{e}");
    }

    #[test]
    fn invalid_output_is_an_error() {
        let e = run("AA", "auto", "yaml", false, false).unwrap_err();
        assert!(e.contains("invalid output"), "{e}");
    }

    #[test]
    fn odd_length_hex_is_an_error() {
        let e = run("abc", "hex", "text", false, false).unwrap_err();
        assert!(e.contains("odd number of digits"), "{e}");
    }

    #[test]
    fn non_base64_input_is_an_error() {
        let e = run("not valid base64 !!!", "base64", "text", false, false).unwrap_err();
        assert!(e.contains("invalid Base64 input"), "{e}");
    }

    #[test]
    fn auto_reports_both_transports_when_neither_decodes() {
        let e = decode("!!! neither !!!", Encoding::Auto).unwrap_err();
        assert!(e.contains("hex") && e.contains("Base64"), "{e}");
    }

    #[test]
    fn a_non_zstd_blob_is_rejected_before_decoding() {
        let junk: Vec<u8> = (0..64u8)
            .map(|i| i.wrapping_mul(37).wrapping_add(11))
            .collect();
        let e = decode(&B64.encode(&junk), Encoding::Base64).unwrap_err();
        assert!(e.contains("not a zstd payload"), "{e}");
        assert!(e.contains("28 b5 2f fd"), "{e}");
    }

    #[test]
    fn brotli_and_gzip_payloads_name_the_right_tool() {
        let gz = [0x1F, 0x8B, 0x08, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03];
        let e = decode(&B64.encode(gz), Encoding::Base64).unwrap_err();
        assert!(e.contains("gzip") && e.contains("gunzip"), "{e}");
    }

    #[test]
    fn zip_lz4_xz_and_zlib_payloads_name_their_tools() {
        let zip = [0x50, 0x4B, 0x03, 0x04, 0x14, 0x00, 0x00, 0x00, 0x08, 0x00];
        let e = decode(&B64.encode(zip), Encoding::Base64).unwrap_err();
        assert!(e.contains("ZIP") && e.contains("unzip"), "{e}");

        let lz4 = [0x04, 0x22, 0x4D, 0x18, 0x60, 0x40, 0x82, 0x00, 0x00, 0x00];
        let e = decode(&B64.encode(lz4), Encoding::Base64).unwrap_err();
        assert!(e.contains("LZ4") && e.contains("lz4-decompress"), "{e}");

        let xz = [0xFD, 0x37, 0x7A, 0x58, 0x5A, 0x00, 0x00, 0x04, 0xE6, 0xD6];
        let e = decode(&B64.encode(xz), Encoding::Base64).unwrap_err();
        assert!(e.contains("xz") && e.contains("lzma-decompress"), "{e}");

        let z = [0x78, 0x9C, 0x03, 0x00, 0x00, 0x00, 0x00, 0x01];
        let e = decode(&B64.encode(z), Encoding::Base64).unwrap_err();
        assert!(e.contains("zlib") && e.contains("raw-inflate"), "{e}");
    }

    #[test]
    fn truncated_zstd_stream_errors_instead_of_returning_partial_data() {
        let compressed = zs(&b"a payload long enough to span several blocks ".repeat(2000));
        let truncated = &compressed[..compressed.len() / 2];
        let e = decode(&B64.encode(truncated), Encoding::Base64).unwrap_err();
        assert!(e.contains("frame 1"), "{e}");
    }

    #[test]
    fn a_corrupted_checksum_is_a_hard_error() {
        // Flip a bit in the payload of a checksummed reference frame: the bytes
        // still decode, but the trailing xxHash-32 no longer matches.
        let mut raw = B64.decode(JSON_B64).unwrap();
        let n = raw.len();
        raw[n - 8] ^= 0x01;
        let e = decode(&B64.encode(&raw), Encoding::Base64).unwrap_err();
        assert!(
            e.contains("checksum") || e.contains("decompression failed"),
            "{e}"
        );
    }

    #[test]
    fn a_dictionary_compressed_frame_names_the_dictionary_id() {
        // Hand-built header: magic, descriptor with dict_id_flag = 1 (one byte)
        // and single_segment, dictionary 0x2A, content size 1.
        let frame = [0x28, 0xB5, 0x2F, 0xFD, 0x21, 0x2A, 0x01];
        let e = decode(&B64.encode(frame), Encoding::Base64).unwrap_err();
        assert!(e.contains("dictionary 0x0000002a"), "{e}");
        assert!(e.contains("cannot be decoded without it"), "{e}");
    }

    #[test]
    fn a_reserved_bit_in_the_frame_header_is_rejected() {
        let frame = [0x28, 0xB5, 0x2F, 0xFD, 0x28, 0x00, 0x01];
        let e = decode(&B64.encode(frame), Encoding::Base64).unwrap_err();
        assert!(e.contains("reserved bit"), "{e}");
    }

    #[test]
    fn trailing_bytes_after_the_last_frame_are_reported() {
        let mut stream = B64.decode(JSON_B64).unwrap();
        stream.extend_from_slice(b"junk");
        let e = decode(&B64.encode(&stream), Encoding::Base64).unwrap_err();
        assert!(
            e.contains("neither a zstd frame nor a skippable frame"),
            "{e}"
        );
    }

    #[test]
    fn a_short_tail_after_the_last_frame_is_reported() {
        let mut stream = B64.decode(JSON_B64).unwrap();
        stream.push(0x00);
        let e = decode(&B64.encode(&stream), Encoding::Base64).unwrap_err();
        assert!(e.contains("truncated"), "{e}");
    }

    #[test]
    fn non_utf8_output_as_text_explains_the_fix() {
        let payload = zs_b64(&[0xFF, 0xFE, 0x00, 0x01]);
        let e = run(&payload, "base64", "text", false, false).unwrap_err();
        assert!(e.contains("not valid UTF-8"), "{e}");
        assert!(e.contains("hex") && e.contains("base64"), "{e}");
    }

    #[test]
    fn oversized_compressed_input_is_rejected() {
        let mut big = Vec::with_capacity(MAX_INPUT_BYTES + 16);
        big.extend_from_slice(&[0x28, 0xB5, 0x2F, 0xFD]);
        big.extend((0..MAX_INPUT_BYTES + 12).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8));
        let e = decode(&B64.encode(&big), Encoding::Base64).unwrap_err();
        assert!(e.contains("over the 8 MiB limit"), "{e}");
    }

    #[test]
    fn a_decompression_bomb_is_stopped_at_the_output_cap() {
        // 20 MiB of zeros in 662 compressed bytes, from the reference CLI.
        const BOMB_B64: &str = concat!(
            "KLUv/YRoAABAAUwAAAgAAQD8/zkQAgIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIAEAACABAAAgAQAAIA",
            "EAACABAAAgAQAAIAEAACABAAAgAQAAMAEADoaKtR"
        );
        let e = decode(BOMB_B64, Encoding::Base64).unwrap_err();
        assert!(e.contains("exceeds the 16 MiB limit"), "{e}");
    }
}
