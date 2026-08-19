//! gizza-ai/pcap-file-extractor — recover files transferred over HTTP, FTP, and
//! SMB out of a libpcap/pcapng capture.
//!
//! Pipeline: container parse (pcap + pcapng) → link/IP/TCP decode → per-direction
//! TCP stream reassembly → protocol-aware object extraction:
//!
//! * **HTTP/1.x** — response bodies (downloads) and POST/PUT/PATCH bodies
//!   (uploads), de-chunked and gzip/deflate-inflated, named from
//!   `Content-Disposition` or the request URI.
//! * **FTP** — the control channel is parsed for `RETR`/`STOR`/`LIST` plus the
//!   negotiated PASV/EPSV/PORT/EPRT endpoint, so the data connection is emitted
//!   under its real filename.
//! * **SMB2/3** — CREATE names a handle, READ/WRITE carry the bytes at explicit
//!   file offsets, assembled sparsely with coverage reported.
//!
//! Every object carries the packet number, endpoints, declared content type, the
//! magic-byte-sniffed type, MD5/SHA-256, a completeness percentage, and its bytes
//! inline as base64 within a budget. Pure Rust — no I/O, no network, no clock.

pub mod capture;
pub mod ftp;
pub mod http;
pub mod smb;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use capture::Conn;
use serde::Serialize;
use sha2::{Digest, Sha256};

/// Largest capture we will parse. The sandbox is 64 MiB and reassembly plus
/// base64 output both cost memory on top of the input.
pub const MAX_CAPTURE_BYTES: usize = 32 * 1024 * 1024;
pub const DEFAULT_LIMIT: u64 = 100;
pub const MAX_LIMIT: u64 = 5000;
pub const DEFAULT_CONTENT_BUDGET: u64 = 4 * 1024 * 1024;
pub const MAX_CONTENT_BUDGET: u64 = 16 * 1024 * 1024;
/// Reassembled-stream ceiling, re-exported so the descriptor can quote it.
pub const STREAM_BUDGET: usize = capture::STREAM_BUDGET;

const PROTOCOLS: [&str; 3] = ["http", "ftp", "smb"];

#[derive(Debug, Clone)]
pub struct Options {
    pub http: bool,
    pub ftp: bool,
    pub smb: bool,
    pub filter: String,
    pub min_size: usize,
    pub include_incomplete: bool,
    pub include_content: bool,
    pub content_budget: u64,
    pub limit: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            http: true,
            ftp: true,
            smb: true,
            filter: String::new(),
            min_size: 0,
            include_incomplete: true,
            include_content: true,
            content_budget: DEFAULT_CONTENT_BUDGET,
            limit: DEFAULT_LIMIT as usize,
        }
    }
}

impl Options {
    /// Parse the `protocols` parameter: `all` or a comma-separated subset.
    pub fn with_protocols(mut self, spec: &str) -> Result<Self, String> {
        let spec = spec.trim();
        if spec.is_empty() || spec.eq_ignore_ascii_case("all") {
            return Ok(self);
        }
        self.http = false;
        self.ftp = false;
        self.smb = false;
        for raw in spec.split(',') {
            let name = raw.trim().to_ascii_lowercase();
            if name.is_empty() {
                continue;
            }
            match name.as_str() {
                "http" => self.http = true,
                "ftp" => self.ftp = true,
                "smb" | "smb2" | "cifs" => self.smb = true,
                other => {
                    return Err(format!(
                        "unknown protocol '{other}' in protocols — expected 'all' or a \
                         comma-separated subset of {}",
                        PROTOCOLS.join(", ")
                    ))
                }
            }
        }
        if !(self.http || self.ftp || self.smb) {
            return Err(
                "protocols selected nothing — pass 'all' or a comma-separated subset of \
                 http, ftp, smb"
                    .into(),
            );
        }
        Ok(self)
    }
}

#[derive(Serialize, Debug)]
pub struct ExtractedFile {
    /// `http`, `ftp`, or `smb`.
    pub protocol: &'static str,
    pub filename: String,
    /// Request URI, FTP path, or SMB share path the object came from.
    pub path: String,
    /// HTTP `Host`, or the server endpoint for FTP/SMB.
    pub host: String,
    /// `download` (server → client) or `upload` (client → server).
    pub direction: &'static str,
    /// Declared MIME type (HTTP only; empty otherwise).
    pub content_type: String,
    /// Type sniffed from the recovered bytes' magic signature.
    pub detected_type: &'static str,
    /// True when the declared type and the sniffed bytes clearly disagree.
    pub type_mismatch: bool,
    pub size: usize,
    pub source: String,
    pub destination: String,
    /// 1-based packet number where the object's first byte was captured.
    pub packet: usize,
    pub timestamp: f64,
    /// True when every byte of the object was present in the capture.
    pub complete: bool,
    pub completeness_percent: f64,
    /// Transformations applied to reach the bytes (`chunked`, `gzip`, …).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decodings: Vec<String>,
    pub md5: String,
    pub sha256: String,
    /// True when `content_base64` holds the whole object.
    pub content_included: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_base64: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct ExtractResult {
    pub format: &'static str,
    pub link_type: String,
    pub total_packets: usize,
    pub tcp_conversations: usize,
    /// Objects matching the filters (before `limit`).
    pub files_total: usize,
    pub returned: usize,
    pub http_objects: usize,
    pub ftp_objects: usize,
    pub smb_objects: usize,
    pub bytes_recovered: u64,
    pub bytes_inlined: u64,
    pub limit: usize,
    pub content_budget: u64,
    pub files: Vec<ExtractedFile>,
    /// Anything the caller should know: skipped fragments, budget hits,
    /// SMB1 traffic, encrypted transports.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

/// An object before filtering/budgeting.
struct Candidate {
    protocol: &'static str,
    filename: String,
    path: String,
    host: String,
    direction: &'static str,
    content_type: String,
    data: Vec<u8>,
    source: String,
    destination: String,
    packet: usize,
    timestamp: f64,
    complete: bool,
    completeness_percent: f64,
    decodings: Vec<String>,
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn md5_hex(data: &[u8]) -> String {
    let mut h = md5::Md5::new();
    h.update(data);
    hex(&h.finalize())
}

fn sha256_hex(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    hex(&h.finalize())
}

/// Magic-byte sniff of the recovered bytes.
pub fn detect_type(d: &[u8]) -> &'static str {
    const SIGS: &[(&[u8], &str)] = &[
        (b"\x89PNG\r\n\x1a\n", "png"),
        (b"\xff\xd8\xff", "jpeg"),
        (b"GIF87a", "gif"),
        (b"GIF89a", "gif"),
        (b"BM", "bmp"),
        (b"%PDF-", "pdf"),
        (b"PK\x03\x04", "zip"),
        (b"PK\x05\x06", "zip"),
        (b"\x1f\x8b", "gzip"),
        (b"BZh", "bzip2"),
        (b"\xfd7zXZ\x00", "xz"),
        (b"7z\xbc\xaf\x27\x1c", "7z"),
        (b"Rar!\x1a\x07", "rar"),
        (b"\x7fELF", "elf"),
        (b"MZ", "pe"),
        (b"\xca\xfe\xba\xbe", "java-class"),
        (b"SQLite format 3\x00", "sqlite"),
        (b"OggS", "ogg"),
        (b"fLaC", "flac"),
        (b"ID3", "mp3"),
        (b"\x25\x21PS", "postscript"),
        (b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1", "ole2"),
        (b"{\\rtf", "rtf"),
    ];
    for (sig, name) in SIGS {
        if d.starts_with(sig) {
            return name;
        }
    }
    if d.len() >= 12 && d.starts_with(b"RIFF") {
        return match &d[8..12] {
            b"WEBP" => "webp",
            b"WAVE" => "wav",
            b"AVI " => "avi",
            _ => "riff",
        };
    }
    if d.len() >= 12 && &d[4..8] == b"ftyp" {
        return "mp4";
    }
    let head = &d[..d.len().min(512)];
    let lower = String::from_utf8_lossy(head).to_ascii_lowercase();
    let trimmed = lower.trim_start();
    if trimmed.starts_with("<!doctype html") || trimmed.starts_with("<html") {
        return "html";
    }
    if trimmed.starts_with("<?xml") {
        return "xml";
    }
    if (trimmed.starts_with('{') || trimmed.starts_with('[')) && head.is_ascii() {
        return "json";
    }
    if !head.is_empty() && head.iter().all(|&b| b == b'\t' || b == b'\n' || b == b'\r' || (0x20..0x7f).contains(&b)) {
        return "text";
    }
    "unknown"
}

/// Classic delivery tell: bytes that are obviously executable/archive/media
/// while the server declared them as text or an image.
fn mismatched(declared: &str, detected: &str) -> bool {
    const BINARY: [&str; 15] = [
        "pe", "elf", "java-class", "zip", "gzip", "bzip2", "xz", "7z", "rar", "pdf", "png", "jpeg",
        "gif", "sqlite", "ole2",
    ];
    let d = declared.split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if d.is_empty() || detected == "unknown" {
        return false;
    }
    if d.starts_with("text/") && BINARY.contains(&detected) {
        return true;
    }
    if d.starts_with("image/") && matches!(detected, "pe" | "elf" | "java-class") {
        return true;
    }
    false
}

fn sanitize_name(raw: &str) -> String {
    let base = raw.rsplit(['/', '\\']).next().unwrap_or(raw);
    let cleaned: String = base
        .chars()
        .filter(|c| !c.is_control() && *c != '\0')
        .take(200)
        .collect();
    let cleaned = cleaned.trim().to_string();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        String::new()
    } else {
        cleaned
    }
}

fn ext_for_mime(mime: &str) -> &'static str {
    match mime.split(';').next().unwrap_or("").trim() {
        "text/html" | "application/xhtml+xml" => ".html",
        "text/plain" => ".txt",
        "text/css" => ".css",
        "application/javascript" | "text/javascript" => ".js",
        "application/json" => ".json",
        "image/png" => ".png",
        "image/jpeg" => ".jpg",
        "image/gif" => ".gif",
        "application/pdf" => ".pdf",
        "application/zip" => ".zip",
        _ => ".bin",
    }
}

/// Turn a request URI into a filename, falling back to the host + a
/// content-type-derived extension for directory-style paths.
fn http_name(uri: &str, host: &str, content_type: &str, index: usize) -> String {
    let path = uri.split(['?', '#']).next().unwrap_or(uri);
    let decoded = http::percent_decode(path);
    let candidate = sanitize_name(&decoded);
    if !candidate.is_empty() {
        return candidate;
    }
    let h = sanitize_name(host);
    if !h.is_empty() {
        return format!("{h}{}", ext_for_mime(content_type));
    }
    format!("http-object-{index}{}", ext_for_mime(content_type))
}

fn push_http(conn: &Conn, out: &mut Vec<Candidate>) -> bool {
    let requests: Vec<http::Message> = http::parse_stream(&conn.c2s.data)
        .into_iter()
        .filter(|m| !m.is_response)
        .collect();
    let responses: Vec<http::Message> = http::parse_stream(&conn.s2c.data)
        .into_iter()
        .filter(|m| m.is_response)
        .collect();
    if requests.is_empty() && responses.is_empty() {
        return false;
    }
    let start = out.len();

    for (i, resp) in responses.iter().enumerate() {
        if resp.body.is_empty() {
            continue;
        }
        let req = requests.get(i);
        let uri = req.map(|r| r.uri.as_str()).unwrap_or("");
        let host = req
            .and_then(|r| http::header(&r.headers, "Host"))
            .unwrap_or("")
            .to_string();
        let content_type = http::header(&resp.headers, "Content-Type").unwrap_or("").to_string();
        let filename = http::header(&resp.headers, "Content-Disposition")
            .and_then(http::disposition_filename)
            .map(|n| sanitize_name(&n))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| http_name(uri, &host, &content_type, i));
        let (packet, timestamp) = conn.s2c.locate(resp.offset);
        let declared: usize = http::header(&resp.headers, "Content-Length")
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let complete = resp.complete && conn.s2c.missing == 0 && !conn.s2c.truncated;
        let pct = if complete {
            100.0
        } else if conn.s2c.missing > 0 || conn.s2c.truncated {
            // Holes in the reassembled stream dominate: the body region can't be
            // more complete than the stream carrying it.
            conn.s2c.completeness()
        } else if declared > 0 && resp.decodings.is_empty() {
            ((resp.body.len() as f64 / declared as f64) * 100.0).min(100.0)
        } else {
            conn.s2c.completeness()
        };
        out.push(Candidate {
            protocol: "http",
            filename,
            path: uri.to_string(),
            host,
            direction: "download",
            content_type,
            data: resp.body.clone(),
            source: conn.server(),
            destination: conn.client(),
            packet,
            timestamp,
            complete,
            completeness_percent: pct,
            decodings: resp.decodings.clone(),
        });
    }

    for (i, req) in requests.iter().enumerate() {
        if req.body.is_empty()
            || !matches!(req.method.to_ascii_uppercase().as_str(), "POST" | "PUT" | "PATCH")
        {
            continue;
        }
        let host = http::header(&req.headers, "Host").unwrap_or("").to_string();
        let content_type = http::header(&req.headers, "Content-Type").unwrap_or("").to_string();
        let filename = http::header(&req.headers, "Content-Disposition")
            .and_then(http::disposition_filename)
            .map(|n| sanitize_name(&n))
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| http_name(&req.uri, &host, &content_type, i));
        let (packet, timestamp) = conn.c2s.locate(req.offset);
        let complete = req.complete && conn.c2s.missing == 0 && !conn.c2s.truncated;
        out.push(Candidate {
            protocol: "http",
            filename,
            path: req.uri.clone(),
            host,
            direction: "upload",
            content_type,
            data: req.body.clone(),
            source: conn.client(),
            destination: conn.server(),
            packet,
            timestamp,
            complete,
            completeness_percent: if complete { 100.0 } else { conn.c2s.completeness() },
            decodings: req.decodings.clone(),
        });
    }
    out.len() > start
}

fn looks_like_smb(conn: &Conn) -> bool {
    if conn.involves_port(445) || conn.involves_port(139) {
        return true;
    }
    let nbss_smb2 = |d: &[u8]| d.len() > 8 && d[0] == 0x00 && &d[4..8] == b"\xfeSMB";
    nbss_smb2(&conn.c2s.data) || nbss_smb2(&conn.s2c.data)
}

/// Extract files from a capture.
pub fn extract(bytes: &[u8], opts: &Options) -> Result<ExtractResult, String> {
    if bytes.len() > MAX_CAPTURE_BYTES {
        return Err(format!(
            "capture is {} bytes — the limit is {} bytes ({} MiB). Slice it first (for example \
             `editcap -r big.pcap small.pcap 1 20000`) and extract from the slice.",
            bytes.len(),
            MAX_CAPTURE_BYTES,
            MAX_CAPTURE_BYTES / (1024 * 1024)
        ));
    }
    let cap = capture::parse(bytes)?;
    let mut notes: Vec<String> = Vec::new();
    if cap.ip_fragments_skipped > 0 {
        notes.push(format!(
            "{} non-first IP fragment(s) skipped — IP fragment reassembly is not performed",
            cap.ip_fragments_skipped
        ));
    }
    if cap.stream_budget_hit {
        notes.push(format!(
            "TCP reassembly budget of {} MiB reached — some streams were cut short and their \
             objects are reported incomplete",
            capture::STREAM_BUDGET / (1024 * 1024)
        ));
    }

    // Pass 1: FTP control channels announce the data endpoints.
    let mut is_control = vec![false; cap.conns.len()];
    let mut transfers: Vec<ftp::Transfer> = Vec::new();
    if opts.ftp {
        for (i, conn) in cap.conns.iter().enumerate() {
            if ftp::is_control(&conn.c2s.data, &conn.s2c.data) {
                is_control[i] = true;
                transfers.extend(ftp::scan(&conn.c2s.data, &conn.s2c.data, conn.server_ip));
            }
        }
    }
    let mut used = vec![false; transfers.len()];

    // Pass 2: every other conversation is an object source.
    let mut cands: Vec<Candidate> = Vec::new();
    let mut smb1_seen = false;
    let mut tls_seen = false;
    for (i, conn) in cap.conns.iter().enumerate() {
        if is_control[i] {
            continue;
        }
        // TLS record + handshake: nothing to recover, but worth telling the user.
        if conn.c2s.data.len() > 5 && conn.c2s.data[0] == 0x16 && conn.c2s.data[1] == 0x03 {
            tls_seen = true;
            continue;
        }
        if looks_like_smb(conn) {
            if !opts.smb {
                continue;
            }
            let (objects, smb1) = smb::extract(&conn.c2s.data, &conn.s2c.data);
            smb1_seen |= smb1;
            for obj in objects {
                let stream = if obj.first_from_server { &conn.s2c } else { &conn.c2s };
                let (packet, timestamp) = stream.locate(obj.first_offset);
                let denom = obj.declared_size.max(obj.data.len() as u64);
                let pct = if denom == 0 {
                    100.0
                } else {
                    ((obj.covered as f64 / denom as f64) * 100.0).min(100.0)
                };
                let filename = sanitize_name(&obj.filename);
                cands.push(Candidate {
                    protocol: "smb",
                    filename: if filename.is_empty() { "smb-object.bin".into() } else { filename },
                    path: obj.path.clone(),
                    host: conn.server(),
                    direction: if obj.is_read { "download" } else { "upload" },
                    content_type: String::new(),
                    data: obj.data,
                    source: if obj.is_read { conn.server() } else { conn.client() },
                    destination: if obj.is_read { conn.client() } else { conn.server() },
                    packet,
                    timestamp,
                    complete: pct >= 100.0 && stream.missing == 0,
                    completeness_percent: pct,
                    decodings: Vec::new(),
                });
            }
            continue;
        }
        if opts.http
            && (http::looks_like_http(&conn.c2s.data) || http::looks_like_http(&conn.s2c.data))
            && push_http(conn, &mut cands)
        {
            continue;
        }
        if !opts.ftp {
            continue;
        }
        // FTP data connection: match the endpoint the control channel announced.
        let matched = transfers
            .iter()
            .enumerate()
            .find(|(idx, t)| !used[*idx] && t.endpoint == Some((conn.server_ip, conn.server_port)))
            .map(|(idx, _)| idx);
        let generic = conn.involves_port(20);
        let Some(idx) = matched.or_else(|| generic.then_some(usize::MAX)) else { continue };
        // Whichever direction carried the payload is the file.
        let (stream, from_server) = if conn.s2c.data.len() >= conn.c2s.data.len() {
            (&conn.s2c, true)
        } else {
            (&conn.c2s, false)
        };
        if stream.data.is_empty() {
            continue;
        }
        let (filename, dir) = if idx == usize::MAX {
            (format!("ftp-data-{}.bin", conn.first_pkt), "download")
        } else {
            used[idx] = true;
            let t = &transfers[idx];
            (
                t.filename.clone(),
                if t.dir == ftp::Dir::Upload { "upload" } else { "download" },
            )
        };
        let (packet, timestamp) = stream.locate(0);
        let path = filename.clone();
        let name = sanitize_name(&filename);
        cands.push(Candidate {
            protocol: "ftp",
            filename: if name.is_empty() { format!("ftp-data-{}.bin", conn.first_pkt) } else { name },
            path,
            host: conn.server(),
            direction: dir,
            content_type: String::new(),
            data: stream.data.clone(),
            source: if from_server { conn.server() } else { conn.client() },
            destination: if from_server { conn.client() } else { conn.server() },
            packet,
            timestamp,
            complete: stream.missing == 0 && !stream.truncated,
            completeness_percent: stream.completeness(),
            decodings: Vec::new(),
        });
    }

    if smb1_seen {
        notes.push(
            "SMB1/CIFS traffic detected — this tool carves SMB2/3 objects only; re-capture or \
             analyse SMB1 transfers with a dedicated CIFS decoder"
                .into(),
        );
    }
    if tls_seen {
        notes.push(
            "TLS-encrypted connections were present and skipped — encrypted payloads (HTTPS, \
             FTPS, SMB3 encryption) cannot be recovered without key material"
                .into(),
        );
    }

    let http_objects = cands.iter().filter(|c| c.protocol == "http").count();
    let ftp_objects = cands.iter().filter(|c| c.protocol == "ftp").count();
    let smb_objects = cands.iter().filter(|c| c.protocol == "smb").count();

    // Filters.
    let needle = opts.filter.trim().to_ascii_lowercase();
    let mut kept: Vec<Candidate> = cands
        .into_iter()
        .filter(|c| c.data.len() >= opts.min_size)
        .filter(|c| opts.include_incomplete || c.complete)
        .filter(|c| {
            needle.is_empty()
                || c.filename.to_ascii_lowercase().contains(&needle)
                || c.path.to_ascii_lowercase().contains(&needle)
                || c.host.to_ascii_lowercase().contains(&needle)
                || c.content_type.to_ascii_lowercase().contains(&needle)
        })
        .collect();
    kept.sort_by(|a, b| a.packet.cmp(&b.packet).then(a.filename.cmp(&b.filename)));
    let files_total = kept.len();
    kept.truncate(opts.limit);

    let mut budget = opts.content_budget;
    let mut bytes_inlined = 0u64;
    let mut bytes_recovered = 0u64;
    let mut files = Vec::with_capacity(kept.len());
    for c in kept {
        let size = c.data.len();
        bytes_recovered += size as u64;
        let detected = detect_type(&c.data);
        let inline = opts.include_content && (size as u64) <= budget;
        if inline {
            budget -= size as u64;
            bytes_inlined += size as u64;
        }
        files.push(ExtractedFile {
            protocol: c.protocol,
            filename: c.filename,
            path: c.path,
            host: c.host,
            direction: c.direction,
            type_mismatch: mismatched(&c.content_type, detected),
            content_type: c.content_type,
            detected_type: detected,
            size,
            source: c.source,
            destination: c.destination,
            packet: c.packet,
            timestamp: c.timestamp,
            complete: c.complete,
            completeness_percent: (c.completeness_percent * 100.0).round() / 100.0,
            decodings: c.decodings,
            md5: md5_hex(&c.data),
            sha256: sha256_hex(&c.data),
            content_included: inline,
            content_base64: inline.then(|| B64.encode(&c.data)),
        });
    }
    if opts.include_content && files.iter().any(|f| !f.content_included) {
        notes.push(format!(
            "inline-content budget of {} bytes exhausted — the remaining objects are listed with \
             hashes and sizes but no base64; raise max_content_bytes or narrow the results with \
             filter/min_size",
            opts.content_budget
        ));
    }

    Ok(ExtractResult {
        format: cap.format,
        link_type: cap.link_type,
        total_packets: cap.total_packets,
        tcp_conversations: cap.conns.len(),
        files_total,
        returned: files.len(),
        http_objects,
        ftp_objects,
        smb_objects,
        bytes_recovered,
        bytes_inlined,
        limit: opts.limit,
        content_budget: opts.content_budget,
        files,
        notes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- synthetic capture builders -------------------------------------

    struct PcapBuilder {
        out: Vec<u8>,
    }

    impl PcapBuilder {
        fn new() -> Self {
            let mut out = Vec::new();
            out.extend_from_slice(&[0xd4, 0xc3, 0xb2, 0xa1]); // little-endian, microseconds
            out.extend_from_slice(&2u16.to_le_bytes());
            out.extend_from_slice(&4u16.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&65535u32.to_le_bytes());
            out.extend_from_slice(&1u32.to_le_bytes()); // Ethernet
            PcapBuilder { out }
        }

        fn packet(&mut self, ts: u32, frame: &[u8]) {
            self.out.extend_from_slice(&ts.to_le_bytes());
            self.out.extend_from_slice(&0u32.to_le_bytes());
            self.out.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            self.out.extend_from_slice(&(frame.len() as u32).to_le_bytes());
            self.out.extend_from_slice(frame);
        }

        #[allow(clippy::too_many_arguments)]
        fn tcp(
            &mut self,
            ts: u32,
            src: [u8; 4],
            sport: u16,
            dst: [u8; 4],
            dport: u16,
            seq: u32,
            flags: u8,
            payload: &[u8],
        ) {
            let mut tcp = Vec::new();
            tcp.extend_from_slice(&sport.to_be_bytes());
            tcp.extend_from_slice(&dport.to_be_bytes());
            tcp.extend_from_slice(&seq.to_be_bytes());
            tcp.extend_from_slice(&0u32.to_be_bytes());
            tcp.push(0x50); // data offset 20 bytes
            tcp.push(flags);
            tcp.extend_from_slice(&8192u16.to_be_bytes());
            tcp.extend_from_slice(&0u16.to_be_bytes());
            tcp.extend_from_slice(&0u16.to_be_bytes());
            tcp.extend_from_slice(payload);

            let total = 20 + tcp.len();
            let mut ip = vec![0x45, 0x00];
            ip.extend_from_slice(&(total as u16).to_be_bytes());
            ip.extend_from_slice(&[0, 0, 0, 0]);
            ip.push(64);
            ip.push(6);
            ip.extend_from_slice(&0u16.to_be_bytes());
            ip.extend_from_slice(&src);
            ip.extend_from_slice(&dst);

            let mut frame = Vec::new();
            frame.extend_from_slice(&[0x11, 0x22, 0x33, 0x44, 0x55, 0x66]);
            frame.extend_from_slice(&[0xaa, 0xbb, 0xcc, 0xdd, 0xee, 0xff]);
            frame.extend_from_slice(&0x0800u16.to_be_bytes());
            frame.extend_from_slice(&ip);
            frame.extend_from_slice(&tcp);
            self.packet(ts, &frame);
        }
    }

    const CLIENT: [u8; 4] = [192, 168, 0, 10];
    const SERVER: [u8; 4] = [192, 168, 0, 20];
    const PNG: &[u8] = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR-tiny-fixture";

    fn http_capture() -> Vec<u8> {
        let mut b = PcapBuilder::new();
        b.tcp(1, CLIENT, 40000, SERVER, 80, 1000, 0x02, &[]); // SYN
        b.tcp(1, SERVER, 80, CLIENT, 40000, 5000, 0x12, &[]); // SYN/ACK
        let req = b"GET /images/logo.png HTTP/1.1\r\nHost: files.example\r\n\r\n";
        b.tcp(2, CLIENT, 40000, SERVER, 80, 1001, 0x18, req);
        let mut resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: image/png\r\nContent-Length: {}\r\n\r\n",
            PNG.len()
        )
        .into_bytes();
        resp.extend_from_slice(PNG);
        // Split the response across two segments so reassembly is exercised.
        b.tcp(3, SERVER, 80, CLIENT, 40000, 5001, 0x18, &resp[..30]);
        b.tcp(4, SERVER, 80, CLIENT, 40000, 5001 + 30, 0x18, &resp[30..]);
        b.out
    }

    #[test]
    fn extracts_an_http_download_with_metadata() {
        let res = extract(&http_capture(), &Options::default()).unwrap();
        assert_eq!(res.format, "pcap");
        assert_eq!(res.link_type, "ethernet");
        assert_eq!(res.total_packets, 5);
        assert_eq!(res.returned, 1);
        assert_eq!(res.http_objects, 1);
        let f = &res.files[0];
        assert_eq!(f.protocol, "http");
        assert_eq!(f.filename, "logo.png");
        assert_eq!(f.path, "/images/logo.png");
        assert_eq!(f.host, "files.example");
        assert_eq!(f.direction, "download");
        assert_eq!(f.content_type, "image/png");
        assert_eq!(f.detected_type, "png");
        assert_eq!(f.size, PNG.len());
        assert_eq!(f.source, "192.168.0.20:80");
        assert_eq!(f.destination, "192.168.0.10:40000");
        assert!(f.complete);
        assert_eq!(f.completeness_percent, 100.0);
        assert!(f.content_included);
        assert_eq!(B64.decode(f.content_base64.as_ref().unwrap()).unwrap(), PNG);
        assert_eq!(f.sha256, sha256_hex(PNG));
        assert_eq!(f.md5, md5_hex(PNG));
        assert!(!f.type_mismatch);
    }

    #[test]
    fn dechunks_and_gunzips_a_response() {
        use flate2::write::GzEncoder;
        use flate2::Compression;
        use std::io::Write;

        let body = b"hello gzip world, this is the recovered payload";
        let mut enc = GzEncoder::new(Vec::new(), Compression::default());
        enc.write_all(body).unwrap();
        let gz = enc.finish().unwrap();

        let mut b = PcapBuilder::new();
        b.tcp(1, CLIENT, 40001, SERVER, 8080, 1, 0x02, &[]);
        b.tcp(
            2,
            CLIENT,
            40001,
            SERVER,
            8080,
            2,
            0x18,
            b"GET /report.txt HTTP/1.1\r\nHost: app.example\r\n\r\n",
        );
        let mut resp =
            b"HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Encoding: gzip\r\nTransfer-Encoding: chunked\r\n\r\n"
                .to_vec();
        resp.extend_from_slice(format!("{:x}\r\n", gz.len()).as_bytes());
        resp.extend_from_slice(&gz);
        resp.extend_from_slice(b"\r\n0\r\n\r\n");
        b.tcp(3, SERVER, 8080, CLIENT, 40001, 1, 0x18, &resp);

        let res = extract(&b.out, &Options::default()).unwrap();
        assert_eq!(res.returned, 1);
        let f = &res.files[0];
        assert_eq!(f.filename, "report.txt");
        assert_eq!(f.decodings, vec!["chunked".to_string(), "gzip".to_string()]);
        assert_eq!(B64.decode(f.content_base64.as_ref().unwrap()).unwrap(), body);
        assert!(f.complete);
    }

    #[test]
    fn extracts_a_post_upload() {
        let payload = b"%PDF-1.4 uploaded document bytes";
        let mut b = PcapBuilder::new();
        b.tcp(1, CLIENT, 40002, SERVER, 80, 1, 0x02, &[]);
        let mut req = format!(
            "POST /upload HTTP/1.1\r\nHost: intake.example\r\nContent-Disposition: attachment; filename=\"secret.pdf\"\r\nContent-Length: {}\r\n\r\n",
            payload.len()
        )
        .into_bytes();
        req.extend_from_slice(payload);
        b.tcp(2, CLIENT, 40002, SERVER, 80, 2, 0x18, &req);
        b.tcp(3, SERVER, 80, CLIENT, 40002, 1, 0x18, b"HTTP/1.1 204 No Content\r\n\r\n");

        let res = extract(&b.out, &Options::default()).unwrap();
        assert_eq!(res.returned, 1);
        let f = &res.files[0];
        assert_eq!(f.direction, "upload");
        assert_eq!(f.filename, "secret.pdf");
        assert_eq!(f.detected_type, "pdf");
        assert_eq!(f.source, "192.168.0.10:40002");
    }

    #[test]
    fn flags_an_executable_served_as_text() {
        let payload = b"MZ\x90\x00\x03 this is a windows executable body";
        let mut b = PcapBuilder::new();
        b.tcp(1, CLIENT, 40003, SERVER, 80, 1, 0x02, &[]);
        b.tcp(2, CLIENT, 40003, SERVER, 80, 2, 0x18, b"GET /update HTTP/1.1\r\nHost: cdn.example\r\n\r\n");
        let mut resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\n\r\n",
            payload.len()
        )
        .into_bytes();
        resp.extend_from_slice(payload);
        b.tcp(3, SERVER, 80, CLIENT, 40003, 1, 0x18, &resp);

        let res = extract(&b.out, &Options::default()).unwrap();
        let f = &res.files[0];
        assert_eq!(f.detected_type, "pe");
        assert!(f.type_mismatch, "text/plain declared for MZ bytes must be flagged");
        // The URI has no basename extension, so the host + declared type is used.
        assert_eq!(f.filename, "update");
    }

    fn ftp_capture() -> Vec<u8> {
        let mut b = PcapBuilder::new();
        // Control channel on 21.
        b.tcp(1, CLIENT, 50000, SERVER, 21, 1, 0x02, &[]);
        b.tcp(1, SERVER, 21, CLIENT, 50000, 1, 0x12, &[]);
        b.tcp(2, SERVER, 21, CLIENT, 50000, 2, 0x18, b"220 test ftp\r\n");
        b.tcp(3, CLIENT, 50000, SERVER, 21, 2, 0x18, b"USER anonymous\r\nPASV\r\n");
        b.tcp(
            4,
            SERVER,
            21,
            CLIENT,
            50000,
            16,
            0x18,
            b"331 ok\r\n227 Entering Passive Mode (192,168,0,20,156,64)\r\n",
        );
        b.tcp(5, CLIENT, 50000, SERVER, 21, 24, 0x18, b"RETR /pub/notes.txt\r\n");
        // Passive endpoint announced above: 156*256 + 64 = 40000.
        b.tcp(6, CLIENT, 50001, SERVER, 40000, 1, 0x02, &[]);
        b.tcp(6, SERVER, 40000, CLIENT, 50001, 1, 0x12, &[]);
        b.tcp(7, SERVER, 40000, CLIENT, 50001, 2, 0x18, b"line one\nline two\n");
        b.out
    }

    #[test]
    fn extracts_an_ftp_retr_with_its_real_filename() {
        let res = extract(&ftp_capture(), &Options::default()).unwrap();
        assert_eq!(res.ftp_objects, 1, "the data channel must be recognised");
        let f = res.files.iter().find(|f| f.protocol == "ftp").unwrap();
        assert_eq!(f.filename, "notes.txt");
        assert_eq!(f.path, "/pub/notes.txt");
        assert_eq!(f.direction, "download");
        assert_eq!(f.size, 18);
        assert_eq!(
            B64.decode(f.content_base64.as_ref().unwrap()).unwrap(),
            b"line one\nline two\n"
        );
    }

    // ---- SMB2 fixture ---------------------------------------------------

    fn smb2_header(command: u16, flags: u32, message_id: u64, body_len: usize) -> Vec<u8> {
        let mut h = vec![0u8; 64];
        h[0..4].copy_from_slice(b"\xfeSMB");
        h[4..6].copy_from_slice(&64u16.to_le_bytes());
        h[12..14].copy_from_slice(&command.to_le_bytes());
        h[16..20].copy_from_slice(&flags.to_le_bytes());
        h[24..32].copy_from_slice(&message_id.to_le_bytes());
        let _ = body_len;
        h
    }

    fn nbss(msg: &[u8]) -> Vec<u8> {
        let mut out = vec![0u8];
        let len = msg.len();
        out.push((len >> 16) as u8);
        out.push((len >> 8) as u8);
        out.push(len as u8);
        out.extend_from_slice(msg);
        out
    }

    fn smb_capture() -> Vec<u8> {
        let content = b"SMB payload bytes for the recovered document.";
        let name: Vec<u8> = "share\\docs\\plan.docx".encode_utf16().flat_map(|u| u.to_le_bytes()).collect();

        // CREATE request (message id 1).
        let mut create_req_body = vec![0u8; 56];
        create_req_body[0..2].copy_from_slice(&57u16.to_le_bytes());
        create_req_body[44..46].copy_from_slice(&(64u16 + 56).to_le_bytes()); // NameOffset
        create_req_body[46..48].copy_from_slice(&(name.len() as u16).to_le_bytes());
        let mut create_req = smb2_header(5, 0, 1, create_req_body.len());
        create_req.extend_from_slice(&create_req_body);
        create_req.extend_from_slice(&name);

        let fid = [0x11u8; 16];
        // CREATE response (message id 1).
        let mut create_resp_body = vec![0u8; 88];
        create_resp_body[0..2].copy_from_slice(&89u16.to_le_bytes());
        create_resp_body[48..56].copy_from_slice(&(content.len() as u64).to_le_bytes());
        create_resp_body[64..80].copy_from_slice(&fid);
        let mut create_resp = smb2_header(5, 1, 1, create_resp_body.len());
        create_resp.extend_from_slice(&create_resp_body);

        // READ request (message id 2): offset 0, whole file.
        let mut read_req_body = vec![0u8; 49];
        read_req_body[0..2].copy_from_slice(&49u16.to_le_bytes());
        read_req_body[4..8].copy_from_slice(&(content.len() as u32).to_le_bytes());
        read_req_body[8..16].copy_from_slice(&0u64.to_le_bytes());
        read_req_body[16..32].copy_from_slice(&fid);
        let mut read_req = smb2_header(8, 0, 2, read_req_body.len());
        read_req.extend_from_slice(&read_req_body);

        // READ response (message id 2).
        let mut read_resp_body = vec![0u8; 16];
        read_resp_body[0..2].copy_from_slice(&17u16.to_le_bytes());
        read_resp_body[2] = 80; // DataOffset from the SMB2 header
        read_resp_body[4..8].copy_from_slice(&(content.len() as u32).to_le_bytes());
        let mut read_resp = smb2_header(8, 1, 2, read_resp_body.len());
        read_resp.extend_from_slice(&read_resp_body);
        read_resp.extend_from_slice(content);
        assert_eq!(read_resp.len(), 80 + content.len());

        let mut b = PcapBuilder::new();
        b.tcp(1, CLIENT, 51000, SERVER, 445, 1, 0x02, &[]);
        b.tcp(1, SERVER, 445, CLIENT, 51000, 1, 0x12, &[]);
        let c1 = nbss(&create_req);
        b.tcp(2, CLIENT, 51000, SERVER, 445, 2, 0x18, &c1);
        let s1 = nbss(&create_resp);
        b.tcp(3, SERVER, 445, CLIENT, 51000, 2, 0x18, &s1);
        let c2 = nbss(&read_req);
        b.tcp(4, CLIENT, 51000, SERVER, 445, 2 + c1.len() as u32, 0x18, &c2);
        let s2 = nbss(&read_resp);
        b.tcp(5, SERVER, 445, CLIENT, 51000, 2 + s1.len() as u32, 0x18, &s2);
        b.out
    }

    #[test]
    fn extracts_an_smb2_read() {
        let res = extract(&smb_capture(), &Options::default()).unwrap();
        assert_eq!(res.smb_objects, 1);
        let f = &res.files[0];
        assert_eq!(f.protocol, "smb");
        assert_eq!(f.filename, "plan.docx");
        assert_eq!(f.path, "share\\docs\\plan.docx");
        assert_eq!(f.direction, "download");
        assert_eq!(f.completeness_percent, 100.0);
        assert_eq!(
            B64.decode(f.content_base64.as_ref().unwrap()).unwrap(),
            b"SMB payload bytes for the recovered document."
        );
    }

    // ---- options + errors ------------------------------------------------

    #[test]
    fn protocol_filter_excludes_other_protocols() {
        let opts = Options::default().with_protocols("ftp").unwrap();
        let res = extract(&http_capture(), &opts).unwrap();
        assert_eq!(res.returned, 0);
        assert_eq!(res.http_objects, 0);
    }

    #[test]
    fn filter_and_min_size_narrow_the_list() {
        let cap = http_capture();
        let hit = Options { filter: "logo".into(), ..Options::default() };
        assert_eq!(extract(&cap, &hit).unwrap().returned, 1);
        let miss = Options { filter: "invoice".into(), ..Options::default() };
        assert_eq!(extract(&cap, &miss).unwrap().returned, 0);
        let too_big = Options { min_size: 10_000, ..Options::default() };
        assert_eq!(extract(&cap, &too_big).unwrap().returned, 0);
    }

    #[test]
    fn content_budget_lists_without_inlining() {
        let opts = Options { content_budget: 4, ..Options::default() };
        let res = extract(&http_capture(), &opts).unwrap();
        let f = &res.files[0];
        assert!(!f.content_included);
        assert!(f.content_base64.is_none());
        assert_eq!(f.sha256, sha256_hex(PNG), "hashes stay available without the bytes");
        assert!(res.notes.iter().any(|n| n.contains("inline-content budget")));
    }

    #[test]
    fn include_content_false_still_reports_hashes() {
        let opts = Options { include_content: false, ..Options::default() };
        let res = extract(&http_capture(), &opts).unwrap();
        assert!(res.files[0].content_base64.is_none());
        assert_eq!(res.files[0].size, PNG.len());
        assert_eq!(res.bytes_inlined, 0);
    }

    #[test]
    fn rejects_a_file_that_is_not_a_capture() {
        let err = extract(b"this is definitely not a pcap file at all!!", &Options::default())
            .unwrap_err();
        assert!(err.contains("unrecognised capture format"), "{err}");
    }

    #[test]
    fn rejects_a_truncated_header() {
        let err = extract(b"\xd4\xc3\xb2\xa1", &Options::default()).unwrap_err();
        assert!(err.contains("not a capture file"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_protocol_name() {
        let err = Options::default().with_protocols("http,smtp").unwrap_err();
        assert!(err.contains("unknown protocol 'smtp'"), "{err}");
    }

    #[test]
    fn detects_common_signatures() {
        assert_eq!(detect_type(b"\x89PNG\r\n\x1a\n"), "png");
        assert_eq!(detect_type(b"%PDF-1.7"), "pdf");
        assert_eq!(detect_type(b"RIFF\x00\x00\x00\x00WEBP"), "webp");
        assert_eq!(detect_type(b"<!DOCTYPE html><body>"), "html");
        assert_eq!(detect_type(b"plain readable text"), "text");
        assert_eq!(detect_type(&[0x00, 0x01, 0x02, 0xff]), "unknown");
    }

    #[test]
    fn pcapng_captures_parse_too() {
        // Wrap the same HTTP exchange in a minimal pcapng: SHB + IDB + EPBs.
        let classic = http_capture();
        let mut ng = Vec::new();
        // Section header block.
        ng.extend_from_slice(&0x0a0d_0d0au32.to_le_bytes());
        ng.extend_from_slice(&28u32.to_le_bytes());
        ng.extend_from_slice(&0x1a2b_3c4du32.to_le_bytes());
        ng.extend_from_slice(&1u16.to_le_bytes());
        ng.extend_from_slice(&0u16.to_le_bytes());
        ng.extend_from_slice(&(-1i64).to_le_bytes());
        ng.extend_from_slice(&28u32.to_le_bytes());
        // Interface description block (Ethernet).
        ng.extend_from_slice(&1u32.to_le_bytes());
        ng.extend_from_slice(&20u32.to_le_bytes());
        ng.extend_from_slice(&1u16.to_le_bytes());
        ng.extend_from_slice(&0u16.to_le_bytes());
        ng.extend_from_slice(&65535u32.to_le_bytes());
        ng.extend_from_slice(&20u32.to_le_bytes());
        // Re-frame each classic record as an enhanced packet block.
        let mut pos = 24usize;
        while pos + 16 <= classic.len() {
            let incl =
                u32::from_le_bytes(classic[pos + 8..pos + 12].try_into().unwrap()) as usize;
            let data = &classic[pos + 16..pos + 16 + incl];
            let pad = (4 - (incl % 4)) % 4;
            let total = 32 + incl + pad;
            ng.extend_from_slice(&6u32.to_le_bytes());
            ng.extend_from_slice(&(total as u32).to_le_bytes());
            ng.extend_from_slice(&0u32.to_le_bytes()); // interface id
            ng.extend_from_slice(&0u32.to_le_bytes()); // ts high
            ng.extend_from_slice(&0u32.to_le_bytes()); // ts low
            ng.extend_from_slice(&(incl as u32).to_le_bytes());
            ng.extend_from_slice(&(incl as u32).to_le_bytes());
            ng.extend_from_slice(data);
            ng.extend_from_slice(&vec![0u8; pad]);
            ng.extend_from_slice(&(total as u32).to_le_bytes());
            pos += 16 + incl;
        }

        let res = extract(&ng, &Options::default()).unwrap();
        assert_eq!(res.format, "pcapng");
        assert_eq!(res.total_packets, 5);
        assert_eq!(res.returned, 1);
        assert_eq!(res.files[0].filename, "logo.png");
    }

    #[test]
    fn a_gap_in_the_capture_is_reported_as_incomplete() {
        let mut b = PcapBuilder::new();
        b.tcp(1, CLIENT, 40004, SERVER, 80, 1, 0x02, &[]);
        b.tcp(2, CLIENT, 40004, SERVER, 80, 2, 0x18, b"GET /big.bin HTTP/1.1\r\nHost: x.example\r\n\r\n");
        let head = b"HTTP/1.1 200 OK\r\nContent-Type: application/octet-stream\r\nContent-Length: 40\r\n\r\n";
        b.tcp(3, SERVER, 80, CLIENT, 40004, 1, 0x18, head);
        // Skip 20 bytes of sequence space, then send the tail.
        let tail = b"TAILTAILTAILTAILTAIL";
        b.tcp(4, SERVER, 80, CLIENT, 40004, 1 + head.len() as u32 + 20, 0x18, tail);

        let res = extract(&b.out, &Options::default()).unwrap();
        let f = &res.files[0];
        assert!(!f.complete);
        assert!(f.completeness_percent < 100.0);
        let strict = Options { include_incomplete: false, ..Options::default() };
        assert_eq!(extract(&b.out, &strict).unwrap().returned, 0);
    }
}
