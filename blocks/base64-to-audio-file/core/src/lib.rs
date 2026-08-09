//! base64-to-audio-file core — decode a Base64 string (or an audio `data:`
//! URI) back into the original audio bytes, with the container sniffed from
//! its magic header so the file gets the right MIME type and extension.
//!
//! Pure compute: no ffmpeg, no re-encoding, no host calls. The bytes that come
//! out are byte-for-byte the bytes that went in — the work is the tolerant
//! Base64 cleanup plus the header sniff that names the result.

use base64::alphabet;
use base64::engine::general_purpose::{GeneralPurpose, GeneralPurposeConfig};
use base64::engine::DecodePaddingMode;
use base64::Engine as _;

/// Hard cap on the decoded payload. Matches the chat/CLI envelope cap, so a
/// payload that decodes here is a payload that can actually be delivered.
pub const MAX_DECODED_BYTES: usize = 32 * 1024 * 1024;

/// Tolerant decoder: padding is optional (`Indifferent`) and a non-canonical
/// final symbol is accepted rather than rejected, because real-world Base64
/// blobs copied out of JSON/XML payloads are routinely unpadded. The URL-safe
/// alphabet is handled by translating `-_` to `+/` during cleanup, so one
/// standard-alphabet engine covers both.
const ENGINE: GeneralPurpose = GeneralPurpose::new(
    &alphabet::STANDARD,
    GeneralPurposeConfig::new()
        .with_decode_allow_trailing_bits(true)
        .with_decode_padding_mode(DecodePaddingMode::Indifferent),
);

/// One audio container this tool can name. `key` is the `format` enum value.
struct Container {
    key: &'static str,
    mime: &'static str,
    ext: &'static str,
    label: &'static str,
}

/// The `format` enum, in the order the page and chat schema list it. `auto`
/// sniffs; every other entry forces the MIME/extension of the named container.
const CONTAINERS: &[Container] = &[
    Container {
        key: "mp3",
        mime: "audio/mpeg",
        ext: "mp3",
        label: "MP3",
    },
    Container {
        key: "wav",
        mime: "audio/wav",
        ext: "wav",
        label: "WAV",
    },
    Container {
        key: "ogg",
        mime: "audio/ogg",
        ext: "ogg",
        label: "Ogg",
    },
    Container {
        key: "flac",
        mime: "audio/flac",
        ext: "flac",
        label: "FLAC",
    },
    Container {
        key: "m4a",
        mime: "audio/mp4",
        ext: "m4a",
        label: "MP4/M4A",
    },
    Container {
        key: "aac",
        mime: "audio/aac",
        ext: "aac",
        label: "ADTS AAC",
    },
    Container {
        key: "webm",
        mime: "audio/webm",
        ext: "webm",
        label: "WebM",
    },
    Container {
        key: "aiff",
        mime: "audio/aiff",
        ext: "aiff",
        label: "AIFF",
    },
    Container {
        key: "amr",
        mime: "audio/amr",
        ext: "amr",
        label: "AMR",
    },
    Container {
        key: "wma",
        mime: "audio/x-ms-wma",
        ext: "wma",
        label: "WMA",
    },
    Container {
        key: "midi",
        mime: "audio/midi",
        ext: "mid",
        label: "MIDI",
    },
    Container {
        key: "bin",
        mime: "application/octet-stream",
        ext: "bin",
        label: "raw bytes",
    },
];

fn container(key: &str) -> Option<&'static Container> {
    CONTAINERS.iter().find(|c| c.key == key)
}

/// The decoded file, ready to be wrapped in a download envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedAudio {
    /// The decoded bytes, unmodified.
    pub bytes: Vec<u8>,
    /// MIME type of the resolved container.
    pub mime: String,
    /// Extension of the resolved container, without the dot.
    pub ext: String,
    /// The resolved `format` enum key (never `auto`).
    pub format: String,
    /// `<filename>.<ext>`, safe to use as a download name.
    pub filename: String,
    /// Human summary: what was decoded, as what, how big.
    pub summary: String,
}

/// Decode `data` into an audio file.
///
/// - `data` — raw Base64, or a `data:…;base64,…` URI. Whitespace, wrapping
///   quotes, the URL-safe alphabet and missing padding are all tolerated.
/// - `filename` — download stem (no extension); blank means `audio`.
/// - `format` — `auto` to sniff the container from its magic header, or a
///   container key to force the MIME type and extension.
/// - `strict` — when sniffing, reject bytes that are not a recognized audio
///   container instead of saving them as `application/octet-stream`.
pub fn decode(
    data: &str,
    filename: &str,
    format: &str,
    strict: bool,
) -> Result<DecodedAudio, String> {
    if !format.is_empty() && format != "auto" && container(format).is_none() {
        let keys: Vec<&str> = CONTAINERS.iter().map(|c| c.key).collect();
        return Err(format!(
            "unknown format '{format}' — use auto or one of: {}",
            keys.join(", ")
        ));
    }

    let (payload, declared_mime) = strip_data_uri(data)?;
    let cleaned = clean_base64(payload);
    if cleaned.is_empty() {
        return Err("no Base64 data — paste the encoded audio, or a data: URI".into());
    }
    // Reject on the encoded length before allocating: 4 Base64 chars carry 3
    // bytes, so this bounds the decode without decoding first.
    if cleaned.len() / 4 * 3 > MAX_DECODED_BYTES {
        return Err(format!(
            "Base64 payload is too large — it decodes to more than {} MiB",
            MAX_DECODED_BYTES / (1024 * 1024)
        ));
    }
    let bytes = ENGINE.decode(&cleaned).map_err(describe_decode_error)?;
    if bytes.is_empty() {
        return Err("the Base64 payload decoded to zero bytes".into());
    }
    if bytes.len() > MAX_DECODED_BYTES {
        return Err(format!(
            "decoded {} — larger than the {} MiB limit",
            human_bytes(bytes.len()),
            MAX_DECODED_BYTES / (1024 * 1024)
        ));
    }

    let sniffed = sniff_audio(&bytes);
    let auto = format.is_empty() || format == "auto";
    let resolved = if auto {
        match sniffed {
            Some(c) => c,
            None if strict => return Err(unrecognized_error(&bytes)),
            // strict = false: keep the bytes, admit we don't know what they are.
            None => container("bin").expect("bin container is defined"),
        }
    } else {
        container(format).expect("format validated above")
    };

    let filename = format!("{}.{}", clean_stem(filename), resolved.ext);
    let mut summary = format!(
        "decoded {} of Base64 into {} — {} ({})",
        human_bytes(bytes.len()),
        filename,
        resolved.label,
        resolved.mime
    );
    if !auto {
        match sniffed {
            Some(c) if c.key != resolved.key => summary.push_str(&format!(
                ". Note: the bytes look like {} ({}), but format={} was requested",
                c.label, c.mime, resolved.key
            )),
            None if resolved.key != "bin" => summary.push_str(&format!(
                ". Note: no {} header was found in the decoded bytes — format={} was applied as requested",
                resolved.label, resolved.key
            )),
            _ => {}
        }
    } else if resolved.key == "bin" {
        summary.push_str(". No known audio header was found, so the bytes were saved as-is");
    }
    if let Some(declared) = declared_mime {
        if !declared.eq_ignore_ascii_case(resolved.mime) {
            summary.push_str(&format!(
                ". The data: URI declared {declared}; the sniffed bytes win"
            ));
        }
    }
    summary.push('.');

    Ok(DecodedAudio {
        bytes,
        mime: resolved.mime.to_string(),
        ext: resolved.ext.to_string(),
        format: resolved.key.to_string(),
        filename,
        summary,
    })
}

/// Decode and re-emit as a `data:` URL — the shape the browser page renders
/// (play it, save it, or paste it straight into the address bar).
pub fn render(data: &str, filename: &str, format: &str, strict: bool) -> Result<String, String> {
    let out = decode(data, filename, format, strict)?;
    Ok(format!(
        "data:{};base64,{}",
        out.mime,
        base64::engine::general_purpose::STANDARD.encode(&out.bytes)
    ))
}

// ---------------------------------------------------------------------------
// Input normalization
// ---------------------------------------------------------------------------

/// Strip a `data:` URI prefix, returning the Base64 payload and the MIME type
/// the URI declared (used only to flag a mismatch in the summary — the sniffed
/// bytes decide the real type).
fn strip_data_uri(data: &str) -> Result<(&str, Option<String>), String> {
    let trimmed = unquote(data.trim());
    if trimmed.len() < 5 || !trimmed[..5].eq_ignore_ascii_case("data:") {
        return Ok((trimmed, None));
    }
    let rest = &trimmed[5..];
    let comma = rest
        .find(',')
        .ok_or("this looks like a data: URI but has no comma before the payload")?;
    let header = &rest[..comma];
    let payload = &rest[comma + 1..];
    if !header
        .rsplit(';')
        .next()
        .is_some_and(|p| p.trim().eq_ignore_ascii_case("base64"))
    {
        return Err(
            "this data: URI is not Base64-encoded — only data:<mime>;base64,… is supported".into(),
        );
    }
    let mime = header.split(';').next().unwrap_or("").trim();
    let mime = (!mime.is_empty()).then(|| mime.to_ascii_lowercase());
    Ok((payload, mime))
}

/// Drop one layer of wrapping quotes, the way a blob pasted out of JSON or a
/// shell command arrives.
fn unquote(s: &str) -> &str {
    for q in ['"', '\''] {
        if s.len() >= 2 && s.starts_with(q) && s.ends_with(q) {
            return s[1..s.len() - 1].trim();
        }
    }
    s
}

/// Drop whitespace and translate the URL-safe alphabet to the standard one.
/// Everything else is left in place so the decoder can point at it.
fn clean_base64(s: &str) -> String {
    s.chars()
        .filter(|c| !c.is_whitespace())
        .map(|c| match c {
            '-' => '+',
            '_' => '/',
            other => other,
        })
        .collect()
}

/// Turn a `base64::DecodeError` into a message that says what to fix.
fn describe_decode_error(e: base64::DecodeError) -> String {
    use base64::DecodeError::*;
    match e {
        InvalidByte(idx, b) => format!(
            "invalid Base64: {} at position {} is not a Base64 character",
            printable(b),
            idx
        ),
        InvalidLength(len) => format!(
            "invalid Base64: {len} characters is not a valid length (Base64 comes in 4-character groups)"
        ),
        InvalidLastSymbol(idx, b) => format!(
            "invalid Base64: the final character {} at position {} does not encode a whole byte",
            printable(b),
            idx
        ),
        InvalidPadding => "invalid Base64: the '=' padding is malformed".into(),
    }
}

fn printable(b: u8) -> String {
    if b.is_ascii_graphic() {
        format!("'{}'", b as char)
    } else {
        format!("byte 0x{b:02X}")
    }
}

/// Sanitize a download stem: basename only, safe characters, no extension.
fn clean_stem(filename: &str) -> String {
    let base = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .trim()
        .trim_matches('.');
    // A stem the caller already extended (`beep.wav`) keeps its name, not its
    // extension — the resolved container decides the extension.
    let base = match base.rsplit_once('.') {
        Some((stem, ext))
            if !stem.is_empty()
                && ext.len() <= 5
                && ext.chars().all(|c| c.is_ascii_alphanumeric()) =>
        {
            stem
        }
        _ => base,
    };
    let cleaned: String = base
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | ' ') {
                c
            } else {
                '-'
            }
        })
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() {
        "audio".to_string()
    } else {
        cleaned
    }
}

fn human_bytes(n: usize) -> String {
    if n < 1024 {
        format!("{n} bytes")
    } else if n < 1024 * 1024 {
        format!("{:.1} KiB", n as f64 / 1024.0)
    } else {
        format!("{:.1} MiB", n as f64 / (1024.0 * 1024.0))
    }
}

// ---------------------------------------------------------------------------
// Magic-header sniffing
// ---------------------------------------------------------------------------

fn starts(b: &[u8], magic: &[u8]) -> bool {
    b.len() >= magic.len() && &b[..magic.len()] == magic
}

fn at(b: &[u8], off: usize, magic: &[u8]) -> bool {
    b.len() >= off + magic.len() && &b[off..off + magic.len()] == magic
}

/// Identify the audio container from its magic header, or `None` when the
/// bytes carry no header this tool recognizes.
fn sniff_audio(b: &[u8]) -> Option<&'static Container> {
    let key = if starts(b, b"RIFF") && at(b, 8, b"WAVE") {
        "wav"
    } else if starts(b, b"RIFF") && at(b, 8, b"RMID") {
        "midi"
    } else if starts(b, b"OggS") {
        "ogg"
    } else if starts(b, b"fLaC") {
        "flac"
    } else if starts(b, b"FORM") && (at(b, 8, b"AIFF") || at(b, 8, b"AIFC")) {
        "aiff"
    } else if starts(b, b"MThd") {
        "midi"
    } else if starts(b, b"#!AMR") {
        "amr"
    } else if starts(b, &ASF_HEADER_GUID) {
        "wma"
    } else if starts(b, b"\x1A\x45\xDF\xA3") {
        // EBML — WebM and Matroska share it; audio-only .webm is the common
        // case for a Base64 blob, so it gets the audio/webm name.
        "webm"
    } else if at(b, 4, b"ftyp") {
        // ISO base media (MP4/M4A/3GP). Audio-only payloads are .m4a.
        "m4a"
    } else if starts(b, b"ID3") {
        "mp3"
    } else if is_adts(b) {
        "aac"
    } else if is_mpeg_frame(b) {
        "mp3"
    } else {
        return None;
    };
    container(key)
}

/// ASF/WMA header object GUID — the first 16 bytes of every ASF file.
const ASF_HEADER_GUID: [u8; 16] = [
    0x30, 0x26, 0xB2, 0x75, 0x8E, 0x66, 0xCF, 0x11, 0xA6, 0xD9, 0x00, 0xAA, 0x00, 0x62, 0xCE, 0x6C,
];

/// ADTS AAC: 12 sync bits, then layer `00` — checked before the MPEG audio
/// sync below, whose mask would otherwise swallow it.
fn is_adts(b: &[u8]) -> bool {
    b.len() >= 2 && b[0] == 0xFF && (b[1] & 0xF6) == 0xF0
}

/// A bare MPEG audio frame header (an MP3 with no ID3 tag): 11 sync bits, a
/// version that isn't the reserved `01`, and a layer that isn't reserved `00`.
fn is_mpeg_frame(b: &[u8]) -> bool {
    b.len() >= 2
        && b[0] == 0xFF
        && (b[1] & 0xE0) == 0xE0
        && (b[1] >> 3) & 0b11 != 0b01
        && (b[1] >> 1) & 0b11 != 0b00
}

/// The strict-mode rejection message: say what the bytes look like instead,
/// and name both escape hatches.
fn unrecognized_error(b: &[u8]) -> String {
    let looks_like = match sniff_non_audio(b) {
        Some(kind) => format!(" — they look like {kind}"),
        None => String::new(),
    };
    format!(
        "the Base64 decoded fine, but the bytes are not a recognized audio file{looks_like}. \
         Set format to the container you expect, or strict=false to save the bytes anyway."
    )
}

/// Best-effort "what is this then?" for the strict-mode error. Deliberately
/// short: it only has to make the rejection actionable.
fn sniff_non_audio(b: &[u8]) -> Option<&'static str> {
    if starts(b, b"\x89PNG\r\n\x1A\n") {
        Some("image/png")
    } else if starts(b, b"\xFF\xD8\xFF") {
        Some("image/jpeg")
    } else if starts(b, b"GIF87a") || starts(b, b"GIF89a") {
        Some("image/gif")
    } else if starts(b, b"RIFF") && at(b, 8, b"WEBP") {
        Some("image/webp")
    } else if starts(b, b"RIFF") && at(b, 8, b"AVI ") {
        Some("video/x-msvideo")
    } else if starts(b, b"%PDF-") {
        Some("application/pdf")
    } else if starts(b, b"PK\x03\x04") {
        Some("a zip archive")
    } else if starts(b, b"\x1F\x8B") {
        Some("gzip data")
    } else if starts(b, b"\x7FELF") {
        Some("an ELF binary")
    } else if std::str::from_utf8(&b[..b.len().min(256)]).is_ok() {
        Some("plain text")
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as B64;

    /// A minimal but structurally real 8-bit mono WAV holding four samples.
    fn wav() -> Vec<u8> {
        let pcm: [u8; 4] = [0x80, 0x90, 0x70, 0x80];
        let mut v = Vec::new();
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36u32 + pcm.len() as u32).to_le_bytes());
        v.extend_from_slice(b"WAVEfmt ");
        v.extend_from_slice(&16u32.to_le_bytes()); // PCM fmt chunk size
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&1u16.to_le_bytes()); // mono
        v.extend_from_slice(&8000u32.to_le_bytes()); // 8 kHz
        v.extend_from_slice(&8000u32.to_le_bytes()); // byte rate
        v.extend_from_slice(&1u16.to_le_bytes()); // block align
        v.extend_from_slice(&8u16.to_le_bytes()); // bits per sample
        v.extend_from_slice(b"data");
        v.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
        v.extend_from_slice(&pcm);
        v
    }

    #[test]
    fn decodes_raw_base64_wav() {
        let out = decode(&B64.encode(wav()), "", "auto", true).unwrap();
        assert_eq!(out.bytes, wav());
        assert_eq!(out.mime, "audio/wav");
        assert_eq!(out.ext, "wav");
        assert_eq!(out.format, "wav");
        assert_eq!(out.filename, "audio.wav");
        assert!(out.summary.contains("WAV"), "{}", out.summary);
    }

    #[test]
    fn decodes_a_data_uri_and_honors_the_filename() {
        let uri = format!("data:audio/wav;base64,{}", B64.encode(wav()));
        let out = decode(&uri, "beep", "auto", true).unwrap();
        assert_eq!(out.bytes, wav());
        assert_eq!(out.filename, "beep.wav");
    }

    #[test]
    fn tolerates_whitespace_quotes_url_safe_alphabet_and_missing_padding() {
        // 0xFB 0xFF 0xFE encodes as "+//+" in standard Base64 and "-__-" in the
        // URL-safe alphabet, so this payload exercises both substitutions.
        let bytes = [b'O', b'g', b'g', b'S', 0xFB, 0xFF, 0xFE];
        let url_safe = B64
            .encode(bytes)
            .replace('+', "-")
            .replace('/', "_")
            .replace('=', "");
        let messy = format!("  \"{}\n{}\"  ", &url_safe[..4], &url_safe[4..]);
        let out = decode(&messy, "", "auto", true).unwrap();
        assert_eq!(out.bytes, bytes);
        assert_eq!(out.format, "ogg");
    }

    #[test]
    fn rejects_invalid_base64_with_a_pointed_message() {
        let err = decode("SGVsbG8h***", "", "auto", true).unwrap_err();
        assert!(err.contains("invalid Base64"), "{err}");
        assert!(err.contains('*'), "{err}");
    }

    #[test]
    fn strict_rejects_non_audio_bytes_and_names_them() {
        let png = B64.encode(b"\x89PNG\r\n\x1A\n\x00\x00\x00\x0DIHDR");
        let err = decode(&png, "", "auto", true).unwrap_err();
        assert!(err.contains("image/png"), "{err}");
        // The same bytes are saved as-is once strict is off.
        let out = decode(&png, "", "auto", false).unwrap();
        assert_eq!(out.mime, "application/octet-stream");
        assert_eq!(out.filename, "audio.bin");
    }

    #[test]
    fn an_explicit_format_overrides_the_sniff_and_bypasses_strict() {
        // Headerless bytes strict mode would reject, forced to MP3.
        let raw = B64.encode([0x00u8, 0x01, 0x02, 0x03, 0xC0, 0xDE]);
        let out = decode(&raw, "voice", "mp3", true).unwrap();
        assert_eq!(out.mime, "audio/mpeg");
        assert_eq!(out.filename, "voice.mp3");
        assert!(out.summary.contains("no MP3 header"), "{}", out.summary);

        // And a WAV payload forced to .bin keeps the bytes but takes the name.
        let out = decode(&B64.encode(wav()), "", "bin", true).unwrap();
        assert_eq!(out.bytes, wav());
        assert_eq!(out.ext, "bin");
        assert!(out.summary.contains("look like WAV"), "{}", out.summary);
    }

    #[test]
    fn sniffs_each_supported_container() {
        let cases: &[(&[u8], &str)] = &[
            (b"ID3\x04\x00\x00\x00\x00\x00\x00", "mp3"),
            (b"\xFF\xFB\x90\x00", "mp3"),
            (b"\xFF\xF1\x50\x80", "aac"),
            (b"OggS\x00\x02\x00\x00", "ogg"),
            (b"fLaC\x00\x00\x00\x22", "flac"),
            (b"\x00\x00\x00\x20ftypM4A \x00\x00\x00\x00", "m4a"),
            (b"\x1A\x45\xDF\xA3\x01\x00\x00\x00", "webm"),
            (b"FORM\x00\x00\x00\x12AIFFCOMM", "aiff"),
            (b"#!AMR\n\x00\x00", "amr"),
            (b"MThd\x00\x00\x00\x06\x00\x01", "midi"),
            (&ASF_HEADER_GUID, "wma"),
        ];
        for (bytes, want) in cases {
            let out = decode(&B64.encode(bytes), "", "auto", true).unwrap();
            assert_eq!(&out.format, want, "sniffing {want}");
        }
        assert_eq!(
            decode(&B64.encode(wav()), "", "auto", true).unwrap().format,
            "wav"
        );
    }

    #[test]
    fn a_declared_data_uri_mime_that_disagrees_is_reported() {
        let uri = format!("data:audio/mpeg;base64,{}", B64.encode(wav()));
        let out = decode(&uri, "", "auto", true).unwrap();
        assert_eq!(out.format, "wav");
        assert!(
            out.summary.contains("declared audio/mpeg"),
            "{}",
            out.summary
        );
    }

    #[test]
    fn filenames_are_sanitized_to_a_bare_stem() {
        let b64 = B64.encode(wav());
        assert_eq!(
            decode(&b64, "../../etc/passwd", "auto", true)
                .unwrap()
                .filename,
            "passwd.wav"
        );
        assert_eq!(
            decode(&b64, "clip.mp3", "auto", true).unwrap().filename,
            "clip.wav"
        );
        assert_eq!(
            decode(&b64, "  ", "auto", true).unwrap().filename,
            "audio.wav"
        );
    }

    #[test]
    fn rejects_empty_input_and_unknown_formats() {
        assert!(decode("   ", "", "auto", true)
            .unwrap_err()
            .contains("no Base64 data"));
        assert!(decode("SGk=", "", "opus", true)
            .unwrap_err()
            .contains("unknown format"));
        assert!(decode("data:audio/wav,abc", "", "auto", true)
            .unwrap_err()
            .contains("not Base64-encoded"));
    }

    #[test]
    fn render_emits_a_playable_data_url() {
        let url = render(&B64.encode(wav()), "", "auto", true).unwrap();
        assert!(url.starts_with("data:audio/wav;base64,"), "{url}");
        let payload = url.split_once(',').unwrap().1;
        assert_eq!(B64.decode(payload).unwrap(), wav());
    }
}
