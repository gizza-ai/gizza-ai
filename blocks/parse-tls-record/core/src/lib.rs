//! gizza-ai/parse-tls-record core — decode TLS record-layer bytes (given as a
//! hex string) into the record header (content type, version, length) and, for
//! handshake records, the handshake messages — fully decoding ClientHello and
//! ServerHello (legacy/record version, the 32-byte random, session id, the
//! offered/selected cipher suites by name, compression methods, and the
//! extensions: SNI server names, ALPN protocols, supported_versions,
//! supported_groups, signature_algorithms, key_share groups, etc.).
//! Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Record {
    /// Record content type number (20–24).
    pub content_type: u8,
    /// Human name of the content type (e.g. handshake, application_data).
    pub content_type_name: String,
    /// Record-layer protocol version, e.g. "TLS 1.0 (0x0301)".
    pub version: String,
    /// Declared record payload length in bytes.
    pub length: u16,
    /// Number of payload bytes actually present (may be < length if truncated).
    pub payload_present: usize,
    /// Decoded handshake messages, when content_type == 22 (handshake).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub handshake: Vec<Handshake>,
    /// Alert level + description, when content_type == 21 (alert).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alert: Option<Alert>,
    /// Notes about anything truncated or not fully decoded.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Alert {
    pub level: u8,
    pub level_name: String,
    pub description: u8,
    pub description_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Handshake {
    /// Handshake message type number.
    pub msg_type: u8,
    /// Human name of the handshake type (e.g. client_hello, server_hello).
    pub msg_type_name: String,
    /// Declared length of the handshake message body in bytes.
    pub length: u32,
    /// Decoded ClientHello, when msg_type == 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_hello: Option<ClientHello>,
    /// Decoded ServerHello, when msg_type == 2.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_hello: Option<ServerHello>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ClientHello {
    /// The legacy_version field, e.g. "TLS 1.2 (0x0303)".
    pub legacy_version: String,
    /// The 32-byte client random as hex.
    pub random: String,
    /// The (legacy) session id as hex (empty when absent).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub session_id: String,
    /// Offered cipher suites, named where known.
    pub cipher_suites: Vec<CipherSuite>,
    /// Legacy compression methods (e.g. ["null"]).
    pub compression_methods: Vec<String>,
    /// Decoded extensions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<Extension>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ServerHello {
    /// The legacy_version field, e.g. "TLS 1.2 (0x0303)".
    pub legacy_version: String,
    /// The 32-byte server random as hex.
    pub random: String,
    /// The echoed (legacy) session id as hex (empty when absent).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub session_id: String,
    /// The single selected cipher suite.
    pub cipher_suite: CipherSuite,
    /// The selected compression method.
    pub compression_method: String,
    /// Decoded extensions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<Extension>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CipherSuite {
    /// The 2-byte cipher suite id as 0xNNNN.
    pub id: String,
    /// IANA name where known (e.g. TLS_AES_128_GCM_SHA256), else "Unknown".
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Extension {
    /// Extension type number.
    pub ext_type: u16,
    /// Human name of the extension (e.g. server_name, application_layer_protocol_negotiation).
    pub name: String,
    /// Extension data length in bytes.
    pub length: u16,
    /// Decoded SNI host names (server_name extension).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub server_names: Vec<String>,
    /// Decoded ALPN protocol identifiers.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub alpn_protocols: Vec<String>,
    /// Decoded supported TLS versions (supported_versions extension).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub supported_versions: Vec<String>,
    /// Decoded named groups (supported_groups / key_share).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub named_groups: Vec<String>,
    /// Decoded signature schemes (signature_algorithms).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub signature_algorithms: Vec<String>,
    /// The selected version (server-side supported_versions).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected_version: Option<String>,
    /// Raw extension payload as hex when it isn't otherwise decoded.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub data_hex: String,
}

fn content_type_name(t: u8) -> &'static str {
    match t {
        20 => "change_cipher_spec",
        21 => "alert",
        22 => "handshake",
        23 => "application_data",
        24 => "heartbeat",
        _ => "unknown",
    }
}

fn version_name(v: u16) -> String {
    let label = match v {
        0x0300 => "SSL 3.0",
        0x0301 => "TLS 1.0",
        0x0302 => "TLS 1.1",
        0x0303 => "TLS 1.2",
        0x0304 => "TLS 1.3",
        _ => "Unknown",
    };
    format!("{label} (0x{v:04x})")
}

fn handshake_type_name(t: u8) -> &'static str {
    match t {
        0 => "hello_request",
        1 => "client_hello",
        2 => "server_hello",
        4 => "new_session_ticket",
        5 => "end_of_early_data",
        8 => "encrypted_extensions",
        11 => "certificate",
        12 => "server_key_exchange",
        13 => "certificate_request",
        14 => "server_hello_done",
        15 => "certificate_verify",
        16 => "client_key_exchange",
        20 => "finished",
        24 => "key_update",
        _ => "unknown",
    }
}

fn alert_level_name(l: u8) -> &'static str {
    match l {
        1 => "warning",
        2 => "fatal",
        _ => "unknown",
    }
}

fn alert_description_name(d: u8) -> &'static str {
    match d {
        0 => "close_notify",
        10 => "unexpected_message",
        20 => "bad_record_mac",
        40 => "handshake_failure",
        42 => "bad_certificate",
        43 => "unsupported_certificate",
        44 => "certificate_revoked",
        45 => "certificate_expired",
        46 => "certificate_unknown",
        47 => "illegal_parameter",
        48 => "unknown_ca",
        49 => "access_denied",
        50 => "decode_error",
        51 => "decrypt_error",
        70 => "protocol_version",
        71 => "insufficient_security",
        80 => "internal_error",
        86 => "inappropriate_fallback",
        90 => "user_canceled",
        109 => "missing_extension",
        110 => "unsupported_extension",
        112 => "unrecognized_name",
        116 => "certificate_required",
        120 => "no_application_protocol",
        _ => "unknown",
    }
}

fn extension_name(t: u16) -> &'static str {
    match t {
        0 => "server_name",
        1 => "max_fragment_length",
        5 => "status_request",
        10 => "supported_groups",
        11 => "ec_point_formats",
        13 => "signature_algorithms",
        14 => "use_srtp",
        15 => "heartbeat",
        16 => "application_layer_protocol_negotiation",
        18 => "signed_certificate_timestamp",
        21 => "padding",
        22 => "encrypt_then_mac",
        23 => "extended_master_secret",
        27 => "compress_certificate",
        28 => "record_size_limit",
        35 => "session_ticket",
        41 => "pre_shared_key",
        42 => "early_data",
        43 => "supported_versions",
        44 => "cookie",
        45 => "psk_key_exchange_modes",
        47 => "certificate_authorities",
        49 => "post_handshake_auth",
        50 => "signature_algorithms_cert",
        51 => "key_share",
        65281 => "renegotiation_info",
        _ => "unknown",
    }
}

fn named_group_name(g: u16) -> &'static str {
    match g {
        0x0017 => "secp256r1",
        0x0018 => "secp384r1",
        0x0019 => "secp521r1",
        0x001d => "x25519",
        0x001e => "x448",
        0x0100 => "ffdhe2048",
        0x0101 => "ffdhe3072",
        0x0102 => "ffdhe4096",
        0x0103 => "ffdhe6144",
        0x0104 => "ffdhe8192",
        0x4588 => "X25519MLKEM768",
        0x11ec => "X25519Kyber768Draft00",
        _ => "unknown",
    }
}

fn signature_scheme_name(s: u16) -> &'static str {
    match s {
        0x0201 => "rsa_pkcs1_sha1",
        0x0203 => "ecdsa_sha1",
        0x0401 => "rsa_pkcs1_sha256",
        0x0403 => "ecdsa_secp256r1_sha256",
        0x0501 => "rsa_pkcs1_sha384",
        0x0503 => "ecdsa_secp384r1_sha384",
        0x0601 => "rsa_pkcs1_sha512",
        0x0603 => "ecdsa_secp521r1_sha512",
        0x0804 => "rsa_pss_rsae_sha256",
        0x0805 => "rsa_pss_rsae_sha384",
        0x0806 => "rsa_pss_rsae_sha512",
        0x0807 => "ed25519",
        0x0808 => "ed448",
        0x0809 => "rsa_pss_pss_sha256",
        0x080a => "rsa_pss_pss_sha384",
        0x080b => "rsa_pss_pss_sha512",
        _ => "unknown",
    }
}

/// Map an IANA cipher suite id to its registered name. Covers the TLS 1.3 suites
/// and the common TLS 1.2 ECDHE suites; everything else is reported as "Unknown".
fn cipher_suite_name(id: u16) -> &'static str {
    match id {
        // TLS 1.3
        0x1301 => "TLS_AES_128_GCM_SHA256",
        0x1302 => "TLS_AES_256_GCM_SHA384",
        0x1303 => "TLS_CHACHA20_POLY1305_SHA256",
        0x1304 => "TLS_AES_128_CCM_SHA256",
        0x1305 => "TLS_AES_128_CCM_8_SHA256",
        // Signalling
        0x00ff => "TLS_EMPTY_RENEGOTIATION_INFO_SCSV",
        0x5600 => "TLS_FALLBACK_SCSV",
        // Common TLS 1.2 ECDHE
        0xc02b => "TLS_ECDHE_ECDSA_WITH_AES_128_GCM_SHA256",
        0xc02c => "TLS_ECDHE_ECDSA_WITH_AES_256_GCM_SHA384",
        0xc02f => "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256",
        0xc030 => "TLS_ECDHE_RSA_WITH_AES_256_GCM_SHA384",
        0xcca8 => "TLS_ECDHE_RSA_WITH_CHACHA20_POLY1305_SHA256",
        0xcca9 => "TLS_ECDHE_ECDSA_WITH_CHACHA20_POLY1305_SHA256",
        0xc013 => "TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA",
        0xc014 => "TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA",
        0xc009 => "TLS_ECDHE_ECDSA_WITH_AES_128_CBC_SHA",
        0xc00a => "TLS_ECDHE_ECDSA_WITH_AES_256_CBC_SHA",
        0xc027 => "TLS_ECDHE_RSA_WITH_AES_128_CBC_SHA256",
        0xc028 => "TLS_ECDHE_RSA_WITH_AES_256_CBC_SHA384",
        // Common TLS 1.2 RSA
        0x009c => "TLS_RSA_WITH_AES_128_GCM_SHA256",
        0x009d => "TLS_RSA_WITH_AES_256_GCM_SHA384",
        0x002f => "TLS_RSA_WITH_AES_128_CBC_SHA",
        0x0035 => "TLS_RSA_WITH_AES_256_CBC_SHA",
        0x003c => "TLS_RSA_WITH_AES_128_CBC_SHA256",
        0x003d => "TLS_RSA_WITH_AES_256_CBC_SHA256",
        0x000a => "TLS_RSA_WITH_3DES_EDE_CBC_SHA",
        _ => "Unknown",
    }
}

/// Strip whitespace and common separators from a hex string and parse it into
/// raw bytes.
fn parse_hex(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-' && *c != '.' && *c != ',')
        .collect();
    let cleaned = cleaned.strip_prefix("0x").unwrap_or(&cleaned);
    let cleaned = cleaned.strip_prefix("0X").unwrap_or(cleaned);
    if cleaned.is_empty() {
        return Err("input is empty — paste the TLS record bytes as a hex string".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex has an odd number of digits ({}) — each byte needs two hex digits",
            cleaned.len()
        ));
    }
    let mut bytes = Vec::with_capacity(cleaned.len() / 2);
    let chars: Vec<char> = cleaned.chars().collect();
    for pair in chars.chunks(2) {
        let s: String = pair.iter().collect();
        let b = u8::from_str_radix(&s, 16).map_err(|_| format!("'{s}' is not a valid hex byte"))?;
        bytes.push(b);
    }
    Ok(bytes)
}

fn be16(b: &[u8], i: usize) -> u16 {
    u16::from_be_bytes([b[i], b[i + 1]])
}

/// Build a CipherSuite entry from its 2-byte id.
fn cipher(id: u16) -> CipherSuite {
    CipherSuite {
        id: format!("0x{id:04x}"),
        name: cipher_suite_name(id).to_string(),
    }
}

fn empty_extension(ext_type: u16, length: u16) -> Extension {
    Extension {
        ext_type,
        name: extension_name(ext_type).to_string(),
        length,
        server_names: Vec::new(),
        alpn_protocols: Vec::new(),
        supported_versions: Vec::new(),
        named_groups: Vec::new(),
        signature_algorithms: Vec::new(),
        selected_version: None,
        data_hex: String::new(),
    }
}

/// Parse the extensions block (a sequence of type(2)+len(2)+data entries).
/// `is_server` controls supported_versions / key_share decoding (a server
/// selects a single value, a client lists several).
fn parse_extensions(data: &[u8], is_server: bool) -> Vec<Extension> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= data.len() {
        let ext_type = be16(data, i);
        let len = be16(data, i + 2) as usize;
        i += 4;
        if i + len > data.len() {
            // Truncated extension; record what we have and stop.
            let mut e = empty_extension(ext_type, len as u16);
            e.data_hex = data[i..].iter().map(|b| format!("{b:02x}")).collect();
            out.push(e);
            break;
        }
        let body = &data[i..i + len];
        let mut e = empty_extension(ext_type, len as u16);
        match ext_type {
            0 => {
                // server_name: list-len(2), then entries name_type(1)+host_len(2)+host.
                if body.len() >= 2 {
                    let list_len = be16(body, 0) as usize;
                    let mut j = 2;
                    let end = (2 + list_len).min(body.len());
                    while j + 3 <= end {
                        let host_len = be16(body, j + 1) as usize;
                        j += 3;
                        if j + host_len <= body.len() {
                            if let Ok(s) = std::str::from_utf8(&body[j..j + host_len]) {
                                e.server_names.push(s.to_string());
                            }
                            j += host_len;
                        } else {
                            break;
                        }
                    }
                }
            }
            16 => {
                // ALPN: list-len(2), then protocol entries len(1)+name.
                if body.len() >= 2 {
                    let list_len = be16(body, 0) as usize;
                    let mut j = 2;
                    let end = (2 + list_len).min(body.len());
                    while j < end {
                        let plen = body[j] as usize;
                        j += 1;
                        if j + plen <= body.len() {
                            if let Ok(s) = std::str::from_utf8(&body[j..j + plen]) {
                                e.alpn_protocols.push(s.to_string());
                            }
                            j += plen;
                        } else {
                            break;
                        }
                    }
                }
            }
            43 => {
                // supported_versions: client = len(1) then list of u16; server = single u16.
                if is_server {
                    if body.len() >= 2 {
                        e.selected_version = Some(version_name(be16(body, 0)));
                    }
                } else if !body.is_empty() {
                    let list_len = body[0] as usize;
                    let mut j = 1;
                    let end = (1 + list_len).min(body.len());
                    while j + 2 <= end {
                        e.supported_versions.push(version_name(be16(body, j)));
                        j += 2;
                    }
                }
            }
            10 => {
                // supported_groups: list-len(2) then u16 group ids.
                if body.len() >= 2 {
                    let list_len = be16(body, 0) as usize;
                    let mut j = 2;
                    let end = (2 + list_len).min(body.len());
                    while j + 2 <= end {
                        e.named_groups.push(named_group_name(be16(body, j)).to_string());
                        j += 2;
                    }
                }
            }
            51 => {
                // key_share: client = list-len(2) then entries group(2)+klen(2)+key;
                // server = single entry group(2)+klen(2)+key. Report the group(s).
                if is_server {
                    if body.len() >= 2 {
                        e.named_groups.push(named_group_name(be16(body, 0)).to_string());
                    }
                } else if body.len() >= 2 {
                    let list_len = be16(body, 0) as usize;
                    let mut j = 2;
                    let end = (2 + list_len).min(body.len());
                    while j + 4 <= end {
                        e.named_groups.push(named_group_name(be16(body, j)).to_string());
                        let klen = be16(body, j + 2) as usize;
                        j += 4 + klen;
                    }
                }
            }
            13 | 50 => {
                // signature_algorithms(_cert): list-len(2) then u16 scheme ids.
                if body.len() >= 2 {
                    let list_len = be16(body, 0) as usize;
                    let mut j = 2;
                    let end = (2 + list_len).min(body.len());
                    while j + 2 <= end {
                        e.signature_algorithms
                            .push(signature_scheme_name(be16(body, j)).to_string());
                        j += 2;
                    }
                }
            }
            _ => {
                if !body.is_empty() {
                    e.data_hex = body.iter().map(|b| format!("{b:02x}")).collect();
                }
            }
        }
        out.push(e);
        i += len;
    }
    out
}

/// Parse a ClientHello body (the bytes after the 4-byte handshake header).
fn parse_client_hello(b: &[u8]) -> Result<ClientHello, String> {
    let mut i = 0;
    if b.len() < 2 + 32 + 1 {
        return Err(
            "ClientHello is truncated (need version + 32-byte random + session id length)".into(),
        );
    }
    let legacy_version = version_name(be16(b, i));
    i += 2;
    let random: String = b[i..i + 32].iter().map(|x| format!("{x:02x}")).collect();
    i += 32;
    let sid_len = b[i] as usize;
    i += 1;
    if i + sid_len > b.len() {
        return Err("ClientHello session id runs past the message".into());
    }
    let session_id: String = b[i..i + sid_len].iter().map(|x| format!("{x:02x}")).collect();
    i += sid_len;
    // cipher suites
    if i + 2 > b.len() {
        return Err("ClientHello truncated before cipher suites".into());
    }
    let cs_len = be16(b, i) as usize;
    i += 2;
    if i + cs_len > b.len() {
        return Err("ClientHello cipher-suite list runs past the message".into());
    }
    let mut cipher_suites = Vec::new();
    let mut j = i;
    while j + 2 <= i + cs_len {
        cipher_suites.push(cipher(be16(b, j)));
        j += 2;
    }
    i += cs_len;
    // compression methods
    if i + 1 > b.len() {
        return Err("ClientHello truncated before compression methods".into());
    }
    let cm_len = b[i] as usize;
    i += 1;
    let mut compression_methods = Vec::new();
    for k in 0..cm_len {
        if i + k < b.len() {
            let m = b[i + k];
            compression_methods.push(match m {
                0 => "null".to_string(),
                1 => "deflate".to_string(),
                _ => format!("unknown(0x{m:02x})"),
            });
        }
    }
    i += cm_len;
    // extensions (optional)
    let extensions = if i + 2 <= b.len() {
        let ext_len = be16(b, i) as usize;
        i += 2;
        let end = (i + ext_len).min(b.len());
        parse_extensions(&b[i..end], false)
    } else {
        Vec::new()
    };
    Ok(ClientHello {
        legacy_version,
        random,
        session_id,
        cipher_suites,
        compression_methods,
        extensions,
    })
}

/// Parse a ServerHello body (the bytes after the 4-byte handshake header).
fn parse_server_hello(b: &[u8]) -> Result<ServerHello, String> {
    let mut i = 0;
    if b.len() < 2 + 32 + 1 {
        return Err(
            "ServerHello is truncated (need version + 32-byte random + session id length)".into(),
        );
    }
    let legacy_version = version_name(be16(b, i));
    i += 2;
    let random: String = b[i..i + 32].iter().map(|x| format!("{x:02x}")).collect();
    i += 32;
    let sid_len = b[i] as usize;
    i += 1;
    if i + sid_len > b.len() {
        return Err("ServerHello session id runs past the message".into());
    }
    let session_id: String = b[i..i + sid_len].iter().map(|x| format!("{x:02x}")).collect();
    i += sid_len;
    if i + 3 > b.len() {
        return Err("ServerHello truncated before cipher suite".into());
    }
    let cipher_suite = cipher(be16(b, i));
    i += 2;
    let compression_method = match b[i] {
        0 => "null".to_string(),
        1 => "deflate".to_string(),
        m => format!("unknown(0x{m:02x})"),
    };
    i += 1;
    let extensions = if i + 2 <= b.len() {
        let ext_len = be16(b, i) as usize;
        i += 2;
        let end = (i + ext_len).min(b.len());
        parse_extensions(&b[i..end], true)
    } else {
        Vec::new()
    };
    Ok(ServerHello {
        legacy_version,
        random,
        session_id,
        cipher_suite,
        compression_method,
        extensions,
    })
}

/// Parse the handshake messages packed into a handshake record payload.
fn parse_handshakes(payload: &[u8], notes: &mut Vec<String>) -> Vec<Handshake> {
    let mut out = Vec::new();
    let mut i = 0;
    while i + 4 <= payload.len() {
        let msg_type = payload[i];
        let length = u32::from_be_bytes([0, payload[i + 1], payload[i + 2], payload[i + 3]]);
        let body_start = i + 4;
        let avail = payload.len() - body_start;
        let take = (length as usize).min(avail);
        let body = &payload[body_start..body_start + take];

        let mut hs = Handshake {
            msg_type,
            msg_type_name: handshake_type_name(msg_type).to_string(),
            length,
            client_hello: None,
            server_hello: None,
        };
        if (length as usize) > avail {
            notes.push(format!(
                "{} message declares {} body bytes but only {} are present (record truncated/fragmented)",
                hs.msg_type_name, length, avail
            ));
        }
        match msg_type {
            1 => match parse_client_hello(body) {
                Ok(ch) => hs.client_hello = Some(ch),
                Err(e) => notes.push(format!("ClientHello: {e}")),
            },
            2 => match parse_server_hello(body) {
                Ok(sh) => hs.server_hello = Some(sh),
                Err(e) => notes.push(format!("ServerHello: {e}")),
            },
            _ => {}
        }
        out.push(hs);
        // Advance by the declared length (handshake messages are concatenated).
        i = body_start + length as usize;
        if (length as usize) > avail {
            break;
        }
    }
    out
}

/// Parse one TLS record starting at `bytes[0]`, returning the decoded record and
/// the number of bytes consumed (header + the present part of the payload).
fn parse_one(bytes: &[u8]) -> Result<(Record, usize), String> {
    if bytes.len() < 5 {
        return Err(format!(
            "input is only {} bytes — a TLS record needs at least the 5-byte header (type, version, length)",
            bytes.len()
        ));
    }
    let content_type = bytes[0];
    // A first byte outside 20–24 is the classic "this is plaintext, not a TLS
    // record" mistake (e.g. pasting "GET /" → 0x47). Flag it early.
    if !(20..=24).contains(&content_type) {
        return Err(format!(
            "first byte is 0x{content_type:02x}, which is not a TLS content type (expected 20–24: \
             change_cipher_spec/alert/handshake/application_data/heartbeat) — is this really a TLS record?"
        ));
    }
    let version_raw = be16(bytes, 1);
    let length = be16(bytes, 3) as usize;
    // The payload available for THIS record is the declared length, clamped to
    // what's actually present (so trailing records aren't swallowed).
    let payload_avail = bytes.len() - 5;
    let payload_len = length.min(payload_avail);
    let payload = &bytes[5..5 + payload_len];
    let payload_present = payload.len();

    let mut notes = Vec::new();
    if length != payload_avail && length > payload_avail {
        notes.push(format!(
            "record header declares {length} payload bytes but only {payload_avail} are present"
        ));
    }

    let mut handshake = Vec::new();
    let mut alert = None;
    match content_type {
        22 => {
            handshake = parse_handshakes(payload, &mut notes);
        }
        21 => {
            if payload_present >= 2 {
                alert = Some(Alert {
                    level: payload[0],
                    level_name: alert_level_name(payload[0]).to_string(),
                    description: payload[1],
                    description_name: alert_description_name(payload[1]).to_string(),
                });
            } else {
                notes.push("alert record payload is shorter than 2 bytes".into());
            }
        }
        23 => {
            notes.push("application_data is encrypted; only the record header is shown".into());
        }
        _ => {}
    }

    let record = Record {
        content_type,
        content_type_name: content_type_name(content_type).to_string(),
        version: version_name(version_raw),
        length: length as u16,
        payload_present,
        handshake,
        alert,
        notes,
    };
    Ok((record, 5 + payload_len))
}

/// Parse a single TLS record (the first one) from hex. Kept for the common
/// single-record case and tests; see `parse_records` for multi-record input.
pub fn parse(input: &str) -> Result<Record, String> {
    let bytes = parse_hex(input)?;
    parse_one(&bytes).map(|(r, _)| r)
}

/// Parse one or more sequential TLS records from hex. TLS data on the wire is
/// often several concatenated records (e.g. ServerHello + Certificate + ...),
/// so this walks the byte stream record-by-record.
pub fn parse_records(input: &str) -> Result<Vec<Record>, String> {
    let bytes = parse_hex(input)?;
    let mut out = Vec::new();
    let mut off = 0;
    while off + 5 <= bytes.len() {
        let (rec, consumed) = parse_one(&bytes[off..])?;
        out.push(rec);
        // A truncated final record consumes the rest; stop to avoid a 0-step loop.
        if consumed == 0 {
            break;
        }
        off += consumed;
    }
    if off < bytes.len() && !out.is_empty() {
        // Some trailing bytes are shorter than a record header.
        if let Some(last) = out.last_mut() {
            last.notes.push(format!(
                "{} trailing byte(s) after the last record are shorter than a 5-byte record header",
                bytes.len() - off
            ));
        }
    }
    if out.is_empty() {
        // Re-run parse_one to surface the precise error (too short / bad type).
        parse_one(&bytes)?;
    }
    Ok(out)
}

/// Parse and return as pretty JSON (chat / programmatic surface). Returns a
/// single record object for one-record input, or an array for multiple records.
pub fn run(input: &str) -> Result<String, String> {
    let recs = parse_records(input)?;
    if recs.len() == 1 {
        serde_json::to_string_pretty(&recs[0]).map_err(|e| e.to_string())
    } else {
        serde_json::to_string_pretty(&recs).map_err(|e| e.to_string())
    }
}

/// Human-readable rendering (used by the page). Renders every record present.
pub fn render(input: &str) -> Result<String, String> {
    let recs = parse_records(input)?;
    let mut out = String::new();
    for (idx, r) in recs.iter().enumerate() {
        if recs.len() > 1 {
            if idx > 0 {
                out.push('\n');
            }
            out.push_str(&format!("=== Record {} of {} ===\n", idx + 1, recs.len()));
        }
        render_record(&mut out, r);
    }
    Ok(out.trim_end().to_string())
}

fn render_record(out: &mut String, r: &Record) {
    out.push_str(&format!(
        "Content Type:   {} ({})\n",
        r.content_type, r.content_type_name
    ));
    out.push_str(&format!("Record Version: {}\n", r.version));
    out.push_str(&format!(
        "Length:         {} bytes ({} present)\n",
        r.length, r.payload_present
    ));
    if let Some(a) = &r.alert {
        out.push_str(&format!(
            "Alert:          {} ({}) — {} ({})\n",
            a.level, a.level_name, a.description, a.description_name
        ));
    }
    for h in &r.handshake {
        out.push_str(&format!(
            "\nHandshake:      {} ({}), {} bytes\n",
            h.msg_type, h.msg_type_name, h.length
        ));
        if let Some(ch) = &h.client_hello {
            out.push_str(&format!("  Legacy Version: {}\n", ch.legacy_version));
            out.push_str(&format!("  Random:         {}\n", ch.random));
            if !ch.session_id.is_empty() {
                out.push_str(&format!("  Session ID:     {}\n", ch.session_id));
            }
            out.push_str(&format!("  Cipher Suites ({}):\n", ch.cipher_suites.len()));
            for c in &ch.cipher_suites {
                out.push_str(&format!("    - {} {}\n", c.id, c.name));
            }
            out.push_str(&format!(
                "  Compression:    {}\n",
                ch.compression_methods.join(", ")
            ));
            render_extensions(out, &ch.extensions);
        }
        if let Some(sh) = &h.server_hello {
            out.push_str(&format!("  Legacy Version: {}\n", sh.legacy_version));
            out.push_str(&format!("  Random:         {}\n", sh.random));
            if !sh.session_id.is_empty() {
                out.push_str(&format!("  Session ID:     {}\n", sh.session_id));
            }
            out.push_str(&format!(
                "  Cipher Suite:   {} {}\n",
                sh.cipher_suite.id, sh.cipher_suite.name
            ));
            out.push_str(&format!("  Compression:    {}\n", sh.compression_method));
            render_extensions(out, &sh.extensions);
        }
    }
    if !r.notes.is_empty() {
        out.push_str("\nNotes:\n");
        for n in &r.notes {
            out.push_str(&format!("  - {n}\n"));
        }
    }
}

fn render_extensions(out: &mut String, exts: &[Extension]) {
    if exts.is_empty() {
        return;
    }
    out.push_str(&format!("  Extensions ({}):\n", exts.len()));
    for e in exts {
        out.push_str(&format!("    - {} ({})", e.ext_type, e.name));
        if !e.server_names.is_empty() {
            out.push_str(&format!(": {}", e.server_names.join(", ")));
        }
        if !e.alpn_protocols.is_empty() {
            out.push_str(&format!(": {}", e.alpn_protocols.join(", ")));
        }
        if !e.supported_versions.is_empty() {
            out.push_str(&format!(": {}", e.supported_versions.join(", ")));
        }
        if let Some(v) = &e.selected_version {
            out.push_str(&format!(": {v}"));
        }
        if !e.named_groups.is_empty() {
            out.push_str(&format!(": {}", e.named_groups.join(", ")));
        }
        if !e.signature_algorithms.is_empty() {
            out.push_str(&format!(": {}", e.signature_algorithms.join(", ")));
        }
        out.push('\n');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real-ish TLS 1.2 ClientHello record. Built by hand to exercise the
    // parser end-to-end (SNI, ALPN, supported_versions).
    fn client_hello_record() -> String {
        let mut ch: Vec<u8> = Vec::new();
        ch.extend_from_slice(&[0x03, 0x03]); // legacy_version TLS 1.2
        ch.extend_from_slice(&[0xaa; 32]); // random
        ch.push(0x00); // session id len 0
                       // cipher suites: TLS_AES_128_GCM_SHA256 (0x1301), 0xc02f
        ch.extend_from_slice(&[0x00, 0x04, 0x13, 0x01, 0xc0, 0x2f]);
        ch.extend_from_slice(&[0x01, 0x00]); // compression: 1 method, null

        let mut exts: Vec<u8> = Vec::new();
        // server_name "example.com"
        let host = b"example.com";
        let mut sni: Vec<u8> = Vec::new();
        let inner_len = 3 + host.len();
        sni.extend_from_slice(&(inner_len as u16).to_be_bytes());
        sni.push(0x00); // name_type host_name
        sni.extend_from_slice(&(host.len() as u16).to_be_bytes());
        sni.extend_from_slice(host);
        exts.extend_from_slice(&[0x00, 0x00]); // server_name
        exts.extend_from_slice(&(sni.len() as u16).to_be_bytes());
        exts.extend_from_slice(&sni);
        // ALPN: h2, http/1.1
        let mut protos: Vec<u8> = Vec::new();
        protos.push(2);
        protos.extend_from_slice(b"h2");
        protos.push(8);
        protos.extend_from_slice(b"http/1.1");
        let mut alpn: Vec<u8> = Vec::new();
        alpn.extend_from_slice(&(protos.len() as u16).to_be_bytes());
        alpn.extend_from_slice(&protos);
        exts.extend_from_slice(&[0x00, 0x10]); // ALPN
        exts.extend_from_slice(&(alpn.len() as u16).to_be_bytes());
        exts.extend_from_slice(&alpn);
        // supported_versions: TLS 1.3, TLS 1.2
        let mut sv: Vec<u8> = Vec::new();
        sv.push(4); // list len
        sv.extend_from_slice(&[0x03, 0x04, 0x03, 0x03]);
        exts.extend_from_slice(&[0x00, 0x2b]); // 43
        exts.extend_from_slice(&(sv.len() as u16).to_be_bytes());
        exts.extend_from_slice(&sv);

        ch.extend_from_slice(&(exts.len() as u16).to_be_bytes());
        ch.extend_from_slice(&exts);

        let mut hs: Vec<u8> = Vec::new();
        hs.push(0x01); // client_hello
        let l = ch.len();
        hs.extend_from_slice(&[(l >> 16) as u8, (l >> 8) as u8, l as u8]);
        hs.extend_from_slice(&ch);

        let mut rec: Vec<u8> = Vec::new();
        rec.push(0x16); // handshake
        rec.extend_from_slice(&[0x03, 0x01]); // record version TLS 1.0
        rec.extend_from_slice(&(hs.len() as u16).to_be_bytes());
        rec.extend_from_slice(&hs);

        rec.iter().map(|b| format!("{b:02x}")).collect()
    }

    #[test]
    fn parses_client_hello() {
        let hex = client_hello_record();
        let r = parse(&hex).unwrap();
        assert_eq!(r.content_type, 22);
        assert_eq!(r.content_type_name, "handshake");
        assert_eq!(r.version, "TLS 1.0 (0x0301)");
        assert_eq!(r.handshake.len(), 1);
        let h = &r.handshake[0];
        assert_eq!(h.msg_type, 1);
        assert_eq!(h.msg_type_name, "client_hello");
        let ch = h.client_hello.as_ref().unwrap();
        assert_eq!(ch.legacy_version, "TLS 1.2 (0x0303)");
        assert_eq!(ch.random, "aa".repeat(32));
        assert_eq!(ch.cipher_suites.len(), 2);
        assert_eq!(ch.cipher_suites[0].name, "TLS_AES_128_GCM_SHA256");
        assert_eq!(ch.cipher_suites[0].id, "0x1301");
        assert_eq!(
            ch.cipher_suites[1].name,
            "TLS_ECDHE_RSA_WITH_AES_128_GCM_SHA256"
        );
        assert_eq!(ch.compression_methods, vec!["null".to_string()]);
        let sni = ch.extensions.iter().find(|e| e.ext_type == 0).unwrap();
        assert_eq!(sni.name, "server_name");
        assert_eq!(sni.server_names, vec!["example.com".to_string()]);
        let alpn = ch.extensions.iter().find(|e| e.ext_type == 16).unwrap();
        assert_eq!(
            alpn.alpn_protocols,
            vec!["h2".to_string(), "http/1.1".to_string()]
        );
        let sv = ch.extensions.iter().find(|e| e.ext_type == 43).unwrap();
        assert_eq!(
            sv.supported_versions,
            vec!["TLS 1.3 (0x0304)".to_string(), "TLS 1.2 (0x0303)".to_string()]
        );
    }

    #[test]
    fn parses_server_hello() {
        let mut sh: Vec<u8> = Vec::new();
        sh.extend_from_slice(&[0x03, 0x03]); // legacy version
        sh.extend_from_slice(&[0xbb; 32]); // random
        sh.push(0x00); // session id len 0
        sh.extend_from_slice(&[0x13, 0x02]); // TLS_AES_256_GCM_SHA384
        sh.push(0x00); // compression null
        let mut exts: Vec<u8> = Vec::new();
        exts.extend_from_slice(&[0x00, 0x2b]); // supported_versions
        exts.extend_from_slice(&[0x00, 0x02, 0x03, 0x04]); // len 2 = TLS 1.3
        sh.extend_from_slice(&(exts.len() as u16).to_be_bytes());
        sh.extend_from_slice(&exts);

        let mut hs: Vec<u8> = Vec::new();
        hs.push(0x02); // server_hello
        let l = sh.len();
        hs.extend_from_slice(&[(l >> 16) as u8, (l >> 8) as u8, l as u8]);
        hs.extend_from_slice(&sh);

        let mut rec: Vec<u8> = Vec::new();
        rec.push(0x16);
        rec.extend_from_slice(&[0x03, 0x03]);
        rec.extend_from_slice(&(hs.len() as u16).to_be_bytes());
        rec.extend_from_slice(&hs);
        let hex: String = rec.iter().map(|b| format!("{b:02x}")).collect();

        let r = parse(&hex).unwrap();
        let h = &r.handshake[0];
        assert_eq!(h.msg_type_name, "server_hello");
        let s = h.server_hello.as_ref().unwrap();
        assert_eq!(s.cipher_suite.name, "TLS_AES_256_GCM_SHA384");
        assert_eq!(s.compression_method, "null");
        let sv = s.extensions.iter().find(|e| e.ext_type == 43).unwrap();
        assert_eq!(sv.selected_version, Some("TLS 1.3 (0x0304)".to_string()));
    }

    #[test]
    fn parses_alert_record() {
        // type 0x15 (alert), version 0x0303, length 2, fatal handshake_failure.
        let r = parse("15 03 03 00 02 02 28").unwrap();
        assert_eq!(r.content_type_name, "alert");
        let a = r.alert.unwrap();
        assert_eq!(a.level, 2);
        assert_eq!(a.level_name, "fatal");
        assert_eq!(a.description, 40);
        assert_eq!(a.description_name, "handshake_failure");
    }

    #[test]
    fn application_data_only_header() {
        let r = parse("17 03 03 00 05 de ad be ef 00").unwrap();
        assert_eq!(r.content_type_name, "application_data");
        assert_eq!(r.length, 5);
        assert!(r.handshake.is_empty());
        assert!(r.notes.iter().any(|n| n.contains("encrypted")));
    }

    #[test]
    fn parses_multiple_sequential_records() {
        // change_cipher_spec (14 03 03 00 01 01) followed by an alert
        // (15 03 03 00 02 01 00 = warning close_notify), concatenated.
        let recs = parse_records("140303000101150303000201 00").unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0].content_type_name, "change_cipher_spec");
        assert_eq!(recs[0].length, 1);
        assert_eq!(recs[1].content_type_name, "alert");
        let a = recs[1].alert.as_ref().unwrap();
        assert_eq!(a.level_name, "warning");
        assert_eq!(a.description_name, "close_notify");
    }

    #[test]
    fn run_emits_array_for_multiple_records() {
        let j = run("140303000101150303000201 00").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert!(v.is_array());
        assert_eq!(v[0]["content_type_name"], "change_cipher_spec");
        assert_eq!(v[1]["content_type_name"], "alert");
    }

    #[test]
    fn run_emits_object_for_single_record() {
        let j = run("15 03 03 00 02 02 28").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert!(v.is_object());
        assert_eq!(v["content_type_name"], "alert");
    }

    #[test]
    fn run_emits_json() {
        let hex = client_hello_record();
        let j = run(&hex).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["content_type_name"], "handshake");
        assert_eq!(v["handshake"][0]["msg_type_name"], "client_hello");
        assert_eq!(
            v["handshake"][0]["client_hello"]["cipher_suites"][0]["name"],
            "TLS_AES_128_GCM_SHA256"
        );
    }

    #[test]
    fn render_includes_fields() {
        let hex = client_hello_record();
        let s = render(&hex).unwrap();
        assert!(s.contains("Content Type:"));
        assert!(s.contains("client_hello"));
        assert!(s.contains("example.com"));
        assert!(s.contains("TLS_AES_128_GCM_SHA256"));
        assert!(s.contains("h2"));
    }

    #[test]
    fn accepts_separators_and_prefix() {
        let r = parse("0x16:03:01-00.02 02 28").unwrap();
        assert_eq!(r.content_type_name, "handshake");
        assert_eq!(r.version, "TLS 1.0 (0x0301)");
    }

    #[test]
    fn errors_on_empty() {
        assert!(parse("").is_err());
    }

    #[test]
    fn errors_on_odd_hex() {
        assert!(parse("abc").is_err());
    }

    #[test]
    fn errors_on_too_short() {
        assert!(parse("1603").is_err());
    }

    #[test]
    fn errors_on_non_tls_first_byte() {
        // "GET " → 0x47 ... not a TLS content type.
        let err = parse("47 45 54 20 2f").unwrap_err();
        assert!(err.contains("not a TLS content type"));
    }

    #[test]
    fn errors_on_bad_hex() {
        assert!(parse("zz 03 03 00 02 02 28").is_err());
    }
}
