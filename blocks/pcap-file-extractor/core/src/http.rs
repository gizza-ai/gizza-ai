//! HTTP/1.x message parsing over a reassembled TCP stream: framing
//! (Content-Length / chunked / close-delimited), de-chunking, and
//! `Content-Encoding` inflation.

use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use std::io::Read;

/// Ceiling on a single inflated body — a decompression bomb must not take the
/// sandbox down with it.
const MAX_INFLATED: usize = 16 * 1024 * 1024;
/// Ceiling on messages parsed from one stream.
const MAX_MESSAGES: usize = 2000;

const METHODS: [&str; 9] =
    ["GET", "POST", "PUT", "HEAD", "DELETE", "OPTIONS", "PATCH", "TRACE", "CONNECT"];

#[derive(Debug)]
pub struct Message {
    /// Byte offset of the start line within the stream.
    pub offset: usize,
    pub is_response: bool,
    pub method: String,
    pub uri: String,
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    /// Transformations applied to reach `body` (e.g. `chunked`, `gzip`).
    pub decodings: Vec<String>,
    /// False when the body was cut short (capture ended mid-transfer, a bad
    /// chunk length, or an unsupported content coding).
    pub complete: bool,
}

pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v.as_str())
}

/// Cheap sniff so we only run the parser on streams that really are HTTP.
pub fn looks_like_http(data: &[u8]) -> bool {
    if data.starts_with(b"HTTP/1.") {
        return true;
    }
    METHODS.iter().any(|m| {
        data.len() > m.len() && data.starts_with(m.as_bytes()) && data[m.len()] == b' '
    })
}

fn find_head_end(data: &[u8], from: usize) -> Option<(usize, usize)> {
    let mut i = from;
    while i + 1 < data.len() {
        if data[i] == b'\n' {
            // "\n\n" or "\n\r\n"
            if data[i + 1] == b'\n' {
                return Some((i, i + 2));
            }
            if i + 2 < data.len() && data[i + 1] == b'\r' && data[i + 2] == b'\n' {
                return Some((i, i + 3));
            }
        }
        i += 1;
    }
    None
}

fn parse_head(head: &[u8]) -> Option<(String, Vec<(String, String)>)> {
    let text = String::from_utf8_lossy(head);
    let mut lines = text.split('\n');
    let start = lines.next()?.trim_end_matches('\r').to_string();
    let mut headers: Vec<(String, String)> = Vec::new();
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if (line.starts_with(' ') || line.starts_with('\t')) && !headers.is_empty() {
            // Obsolete line folding.
            let last = headers.len() - 1;
            headers[last].1.push(' ');
            headers[last].1.push_str(line.trim());
            continue;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    Some((start, headers))
}

/// Decode a chunked body. Returns `(bytes, consumed, complete)`.
fn dechunk(data: &[u8]) -> (Vec<u8>, usize, bool) {
    let mut out = Vec::new();
    let mut pos = 0usize;
    loop {
        let Some(nl) = data[pos..].iter().position(|&c| c == b'\n') else {
            return (out, data.len(), false);
        };
        let line = &data[pos..pos + nl];
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        let hex: String = String::from_utf8_lossy(line)
            .split(';')
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        let Ok(size) = usize::from_str_radix(&hex, 16) else {
            return (out, pos, false);
        };
        pos += nl + 1;
        if size == 0 {
            // Skip trailers up to the terminating blank line.
            while pos < data.len() {
                let Some(nl) = data[pos..].iter().position(|&c| c == b'\n') else { break };
                let line = &data[pos..pos + nl];
                pos += nl + 1;
                if line.strip_suffix(b"\r").unwrap_or(line).is_empty() {
                    break;
                }
            }
            return (out, pos, true);
        }
        if pos + size > data.len() {
            out.extend_from_slice(&data[pos..]);
            return (out, data.len(), false);
        }
        if out.len() + size > MAX_INFLATED {
            out.extend_from_slice(&data[pos..pos + (MAX_INFLATED - out.len())]);
            return (out, data.len(), false);
        }
        out.extend_from_slice(&data[pos..pos + size]);
        pos += size;
        // Trailing CRLF after the chunk data.
        if data.get(pos) == Some(&b'\r') {
            pos += 1;
        }
        if data.get(pos) == Some(&b'\n') {
            pos += 1;
        }
    }
}

fn inflate(kind: &str, body: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let ok = match kind {
        "gzip" | "x-gzip" => GzDecoder::new(body).take(MAX_INFLATED as u64).read_to_end(&mut out).is_ok(),
        "deflate" => {
            let zlib = ZlibDecoder::new(body).take(MAX_INFLATED as u64).read_to_end(&mut out).is_ok();
            if zlib && !out.is_empty() {
                true
            } else {
                out.clear();
                DeflateDecoder::new(body).take(MAX_INFLATED as u64).read_to_end(&mut out).is_ok()
            }
        }
        _ => return None,
    };
    // A truncated capture leaves a partial deflate stream: keep whatever came
    // out rather than discarding the object entirely.
    if ok || !out.is_empty() {
        Some(out)
    } else {
        None
    }
}

/// Parse every HTTP message on one direction of a conversation.
pub fn parse_stream(data: &[u8]) -> Vec<Message> {
    let mut msgs = Vec::new();
    let mut pos = 0usize;
    while pos < data.len() && msgs.len() < MAX_MESSAGES {
        if !looks_like_http(&data[pos..]) {
            break;
        }
        let Some((head_end, body_start)) = find_head_end(data, pos) else { break };
        let Some((start_line, headers)) = parse_head(&data[pos..head_end]) else { break };
        let is_response = start_line.starts_with("HTTP/");
        let mut method = String::new();
        let mut uri = String::new();
        let mut status = 0u16;
        if is_response {
            status = start_line
                .split_whitespace()
                .nth(1)
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
        } else {
            let mut parts = start_line.split_whitespace();
            method = parts.next().unwrap_or("").to_string();
            uri = parts.next().unwrap_or("").to_string();
        }

        let rest = &data[body_start..];
        let chunked = header(&headers, "Transfer-Encoding")
            .map(|v| v.to_ascii_lowercase().contains("chunked"))
            .unwrap_or(false);
        let content_length: Option<usize> =
            header(&headers, "Content-Length").and_then(|v| v.trim().parse().ok());
        let bodyless_status = is_response
            && ((100..200).contains(&status) || status == 204 || status == 304);
        let head_request = !is_response && method.eq_ignore_ascii_case("HEAD");

        let mut decodings: Vec<String> = Vec::new();
        let (mut body, consumed, mut complete) = if bodyless_status || head_request {
            (Vec::new(), 0usize, true)
        } else if chunked {
            decodings.push("chunked".into());
            dechunk(rest)
        } else if let Some(n) = content_length {
            let take = n.min(rest.len());
            (rest[..take].to_vec(), take, take == n)
        } else if is_response {
            // Close-delimited: the remainder of the stream is the body.
            (rest.to_vec(), rest.len(), true)
        } else {
            (Vec::new(), 0usize, true)
        };

        if !body.is_empty() {
            if let Some(enc) = header(&headers, "Content-Encoding") {
                let enc = enc.trim().to_ascii_lowercase();
                for step in enc.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                    match step {
                        "identity" => {}
                        "gzip" | "x-gzip" | "deflate" => match inflate(step, &body) {
                            Some(v) => {
                                body = v;
                                decodings.push(step.to_string());
                            }
                            None => complete = false,
                        },
                        other => {
                            // br / zstd / compress: reported, not decoded.
                            decodings.push(format!("{other} (not decoded)"));
                            complete = false;
                        }
                    }
                }
            }
        }

        msgs.push(Message {
            offset: pos,
            is_response,
            method,
            uri,
            status,
            headers,
            body,
            decodings,
            complete,
        });
        let next = body_start + consumed;
        if next <= pos {
            break;
        }
        pos = next;
    }
    msgs
}

/// `filename=` / `filename*=` out of a Content-Disposition header.
pub fn disposition_filename(value: &str) -> Option<String> {
    for part in value.split(';') {
        let part = part.trim();
        if let Some(rest) = part.strip_prefix("filename*=") {
            // RFC 5987: charset'lang'percent-encoded-value
            let raw = rest.rsplit('\'').next().unwrap_or(rest);
            let decoded = percent_decode(raw);
            if !decoded.is_empty() {
                return Some(decoded);
            }
        }
        if let Some(rest) = part.strip_prefix("filename=") {
            let v = rest.trim().trim_matches('"').trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

pub fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hexval(b[i + 1]), hexval(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hexval(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}
