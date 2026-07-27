//! pem-bundle-splitter core — split a multi-block PEM bundle (a `.pem` /
//! `fullchain.pem`-style file holding several `-----BEGIN <label>-----` blocks)
//! into individual, labeled blocks and report the **type** and **order** of
//! each. Pure Rust, no wafer/wasm-bindgen deps.
//!
//! A PEM bundle is just a concatenation of base64-armored DER objects — the
//! label after `-----BEGIN ` (e.g. `CERTIFICATE`, `PRIVATE KEY`,
//! `CERTIFICATE REQUEST`) says what each block is. This tool reads that label
//! for every block, maps it to a friendly type / PKCS format, records the
//! block's DER byte length and (optionally) a SHA-256 fingerprint, and re-emits
//! each block as its own clean, copy-pasteable PEM. It is a **generic,
//! label-driven splitter**: it does not parse the inner ASN.1, so it works for
//! certs, keys, CSRs, params, PGP and OpenSSH blocks alike.

use sha2::{Digest, Sha256};

/// What the caller wants back.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputMode {
    /// Human-readable summary + per-block details + the re-armored PEM.
    Report,
    /// A structured JSON object (count, summary, blocks[]).
    Json,
    /// Just the cleaned individual PEM blocks, each preceded by a comment header.
    Pem,
}

pub fn parse_output(s: &str) -> Result<OutputMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "report" | "" => Ok(OutputMode::Report),
        "json" => Ok(OutputMode::Json),
        "pem" | "blocks" => Ok(OutputMode::Pem),
        other => Err(format!(
            "unknown output '{other}'. Use 'report', 'json' or 'pem'."
        )),
    }
}

/// Coarse category used for the summary counts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Category {
    Certificate,
    PrivateKey,
    PublicKey,
    Csr,
    Other,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Certificate => "certificate",
            Category::PrivateKey => "private key",
            Category::PublicKey => "public key",
            Category::Csr => "certificate signing request",
            Category::Other => "other",
        }
    }
    fn json_key(self) -> &'static str {
        match self {
            Category::Certificate => "certificates",
            Category::PrivateKey => "private_keys",
            Category::PublicKey => "public_keys",
            Category::Csr => "csrs",
            Category::Other => "other",
        }
    }
}

/// One parsed block of the bundle.
#[derive(Clone, Debug)]
pub struct BlockInfo {
    /// 1-based position in the bundle.
    pub index: usize,
    /// The raw PEM label, e.g. `CERTIFICATE`, `EC PRIVATE KEY`.
    pub label: String,
    /// Friendly type description, e.g. `X.509 certificate`.
    pub type_name: String,
    /// Coarse category for the summary.
    pub category: Category,
    /// Length of the decoded DER contents, in bytes.
    pub der_bytes: usize,
    /// SHA-256 of the DER contents, lowercase hex — only when requested.
    pub sha256: Option<String>,
    /// The block re-emitted as a clean, 64-column PEM string (LF, trailing LF).
    pub pem: String,
    /// A suggested numbered filename, e.g. `block-1-certificate.pem`.
    pub filename: String,
}

/// Map a PEM label to a friendly type + coarse category. Unknown labels fall
/// through to a generic description so the tool never rejects a valid block.
fn classify(label: &str) -> (String, Category) {
    let up = label.trim().to_uppercase();
    let (name, cat): (&str, Category) = match up.as_str() {
        "CERTIFICATE" => ("X.509 certificate", Category::Certificate),
        "TRUSTED CERTIFICATE" => ("Trusted X.509 certificate", Category::Certificate),
        "ATTRIBUTE CERTIFICATE" => ("X.509 attribute certificate", Category::Certificate),
        "X509 CRL" | "X.509 CRL" => (
            "X.509 certificate revocation list (CRL)",
            Category::Other,
        ),
        "CERTIFICATE REQUEST" | "NEW CERTIFICATE REQUEST" => {
            ("PKCS#10 certificate signing request (CSR)", Category::Csr)
        }
        "PRIVATE KEY" => ("PKCS#8 private key", Category::PrivateKey),
        "ENCRYPTED PRIVATE KEY" => ("PKCS#8 encrypted private key", Category::PrivateKey),
        "RSA PRIVATE KEY" => ("PKCS#1 RSA private key", Category::PrivateKey),
        "EC PRIVATE KEY" => ("SEC1 EC private key", Category::PrivateKey),
        "DSA PRIVATE KEY" => ("DSA private key", Category::PrivateKey),
        "OPENSSH PRIVATE KEY" => ("OpenSSH private key", Category::PrivateKey),
        "PGP PRIVATE KEY BLOCK" => ("PGP private key block", Category::PrivateKey),
        "PUBLIC KEY" => ("PKIX/SPKI public key", Category::PublicKey),
        "RSA PUBLIC KEY" => ("PKCS#1 RSA public key", Category::PublicKey),
        "DSA PUBLIC KEY" => ("DSA public key", Category::PublicKey),
        "SSH2 PUBLIC KEY" => ("SSH2 public key", Category::PublicKey),
        "PGP PUBLIC KEY BLOCK" => ("PGP public key block", Category::PublicKey),
        "EC PARAMETERS" => ("EC parameters", Category::Other),
        "DH PARAMETERS" => ("Diffie-Hellman parameters", Category::Other),
        "DSA PARAMETERS" => ("DSA parameters", Category::Other),
        "PARAMETERS" => ("Algorithm parameters", Category::Other),
        "PKCS7" | "PKCS #7 SIGNED DATA" => ("PKCS#7 signed data", Category::Other),
        "CMS" => ("CMS message", Category::Other),
        "PGP MESSAGE" => ("PGP message", Category::Other),
        "PGP SIGNATURE" => ("PGP signature", Category::Other),
        _ => return (format!("Unknown / other ({})", label.trim()), Category::Other),
    };
    (name.to_string(), cat)
}

/// A short, filesystem-safe slug for a label (for the numbered filename).
fn label_slug(label: &str) -> String {
    let s: String = label
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let s = s.trim_matches('-').to_string();
    // collapse runs of '-'
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c == '-' {
            if !prev_dash {
                out.push(c);
            }
            prev_dash = true;
        } else {
            out.push(c);
            prev_dash = false;
        }
    }
    if out.is_empty() {
        "block".to_string()
    } else {
        out
    }
}

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Re-encode one block as a clean 64-column PEM (LF line endings, trailing LF).
fn reencode(label: &str, contents: &[u8]) -> String {
    let block = pem::Pem::new(label.to_string(), contents.to_vec());
    let conf = pem::EncodeConfig::new().set_line_ending(pem::LineEnding::LF);
    pem::encode_config(&block, conf)
}

/// Parse the bundle into per-block info, in input order. `fingerprints`
/// controls whether each block's SHA-256 is computed.
pub fn split(input: &str, fingerprints: bool) -> Result<Vec<BlockInfo>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("no PEM input — paste a bundle containing one or more -----BEGIN ...----- blocks".into());
    }
    if !trimmed.contains("-----BEGIN") {
        return Err(
            "input does not look like PEM (no '-----BEGIN ...-----' header found). Paste the contents of a .pem / .crt / fullchain file.".into(),
        );
    }
    // parse_many tolerates human-readable text between blocks (e.g. the
    // `openssl x509 -text` bundle shape).
    let blocks = pem::parse_many(input.as_bytes()).map_err(|e| format!("not valid PEM: {e}"))?;
    if blocks.is_empty() {
        return Err("found '-----BEGIN' but no complete PEM block — check for a missing or truncated -----END line.".into());
    }
    let mut out = Vec::with_capacity(blocks.len());
    for (i, b) in blocks.iter().enumerate() {
        let label = b.tag().to_string();
        let contents = b.contents();
        let (type_name, category) = classify(&label);
        let sha256 = if fingerprints {
            let mut h = Sha256::new();
            h.update(contents);
            Some(to_hex(&h.finalize()))
        } else {
            None
        };
        out.push(BlockInfo {
            index: i + 1,
            filename: format!("block-{}-{}.pem", i + 1, label_slug(&label)),
            label,
            type_name,
            category,
            der_bytes: contents.len(),
            sha256,
            pem: reencode(b.tag(), contents),
        });
    }
    Ok(out)
}

/// Summary counts by category, in a stable order.
fn summary_counts(blocks: &[BlockInfo]) -> Vec<(Category, usize)> {
    let order = [
        Category::Certificate,
        Category::PrivateKey,
        Category::PublicKey,
        Category::Csr,
        Category::Other,
    ];
    order
        .into_iter()
        .map(|c| (c, blocks.iter().filter(|b| b.category == c).count()))
        .filter(|(_, n)| *n > 0)
        .collect()
}

fn render_report(blocks: &[BlockInfo]) -> String {
    let n = blocks.len();
    let mut out = String::new();
    out.push_str(&format!(
        "PEM bundle: {n} block{}\n",
        if n == 1 { "" } else { "s" }
    ));
    let counts = summary_counts(blocks);
    let summary: Vec<String> = counts
        .iter()
        .map(|(c, k)| format!("{}: {k}", c.as_str()))
        .collect();
    out.push_str(&format!("  {}\n", summary.join(" · ")));
    for b in blocks {
        out.push('\n');
        out.push_str(&format!("Block {} of {n}\n", b.index));
        out.push_str(&format!("  type:     {}\n", b.type_name));
        out.push_str(&format!("  label:    {}\n", b.label));
        out.push_str(&format!("  DER size: {} bytes\n", b.der_bytes));
        out.push_str(&format!("  filename: {}\n", b.filename));
        if let Some(fp) = &b.sha256 {
            out.push_str(&format!("  SHA-256:  {fp}\n"));
        }
        out.push('\n');
        out.push_str(b.pem.trim_end());
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn render_pem(blocks: &[BlockInfo]) -> String {
    let n = blocks.len();
    let mut out = String::new();
    for b in blocks {
        out.push_str(&format!(
            "# Block {} of {n}: {} ({})\n",
            b.index, b.type_name, b.filename
        ));
        out.push_str(b.pem.trim_end());
        out.push_str("\n\n");
    }
    out.trim_end().to_string()
}

fn render_json(blocks: &[BlockInfo]) -> String {
    use serde_json::{json, Map, Value};
    let mut summary = Map::new();
    for (c, k) in summary_counts(blocks) {
        summary.insert(c.json_key().to_string(), json!(k));
    }
    let items: Vec<Value> = blocks
        .iter()
        .map(|b| {
            let mut m = Map::new();
            m.insert("index".into(), json!(b.index));
            m.insert("label".into(), json!(b.label));
            m.insert("type".into(), json!(b.type_name));
            m.insert("category".into(), json!(b.category.as_str()));
            m.insert("der_bytes".into(), json!(b.der_bytes));
            m.insert("filename".into(), json!(b.filename));
            if let Some(fp) = &b.sha256 {
                m.insert("sha256".into(), json!(fp));
            }
            m.insert("pem".into(), json!(b.pem));
            Value::Object(m)
        })
        .collect();
    let root = json!({
        "count": blocks.len(),
        "summary": Value::Object(summary),
        "blocks": items,
    });
    serde_json::to_string_pretty(&root).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Top-level entry: split the bundle and render it in the requested mode.
pub fn run(input: &str, mode: OutputMode, fingerprints: bool) -> Result<String, String> {
    let blocks = split(input, fingerprints)?;
    Ok(match mode {
        OutputMode::Report => render_report(&blocks),
        OutputMode::Json => render_json(&blocks),
        OutputMode::Pem => render_pem(&blocks),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;

    // Build a PEM block from a label + raw bytes (base64-armored).
    fn mk(label: &str, bytes: &[u8]) -> String {
        reencode(label, bytes)
    }

    #[test]
    fn splits_multi_block_bundle_in_order() {
        let bundle = format!(
            "{}{}{}",
            mk("CERTIFICATE", &[1, 2, 3]),
            mk("CERTIFICATE", &[4, 5, 6, 7]),
            mk("PRIVATE KEY", &[8, 9]),
        );
        let blocks = split(&bundle, false).unwrap();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].index, 1);
        assert_eq!(blocks[0].type_name, "X.509 certificate");
        assert_eq!(blocks[0].category, Category::Certificate);
        assert_eq!(blocks[0].der_bytes, 3);
        assert_eq!(blocks[2].index, 3);
        assert_eq!(blocks[2].type_name, "PKCS#8 private key");
        assert_eq!(blocks[2].category, Category::PrivateKey);
        assert_eq!(blocks[2].der_bytes, 2);
        assert!(blocks[0].sha256.is_none());
    }

    #[test]
    fn classifies_common_labels() {
        assert_eq!(classify("EC PRIVATE KEY").0, "SEC1 EC private key");
        assert_eq!(classify("RSA PRIVATE KEY").0, "PKCS#1 RSA private key");
        assert_eq!(
            classify("CERTIFICATE REQUEST"),
            (
                "PKCS#10 certificate signing request (CSR)".to_string(),
                Category::Csr
            )
        );
        assert_eq!(classify("PUBLIC KEY").1, Category::PublicKey);
        // unknown label falls through, does not panic
        let (name, cat) = classify("WIDGET BLOCK");
        assert!(name.starts_with("Unknown / other"));
        assert_eq!(cat, Category::Other);
    }

    #[test]
    fn fingerprints_are_sha256_of_der() {
        let bundle = mk("CERTIFICATE", &[1, 2, 3]);
        let blocks = split(&bundle, true).unwrap();
        // SHA-256 of the bytes 01 02 03
        assert_eq!(
            blocks[0].sha256.as_deref().unwrap(),
            "039058c6f2c0cb492c533b0a4d14ef77cc0f78abccced5287d84a1a2011cfb81"
        );
    }

    #[test]
    fn report_mode_lists_type_and_order() {
        let bundle = format!("{}{}", mk("CERTIFICATE", &[1, 2, 3]), mk("PRIVATE KEY", &[9]));
        let out = run(&bundle, OutputMode::Report, false).unwrap();
        assert!(out.starts_with("PEM bundle: 2 blocks"));
        assert!(out.contains("certificate: 1 · private key: 1"));
        assert!(out.contains("Block 1 of 2"));
        assert!(out.contains("type:     X.509 certificate"));
        assert!(out.contains("Block 2 of 2"));
        assert!(out.contains("-----BEGIN PRIVATE KEY-----"));
    }

    #[test]
    fn json_mode_is_structured() {
        let bundle = format!("{}{}", mk("CERTIFICATE", &[1, 2, 3]), mk("PRIVATE KEY", &[9]));
        let out = run(&bundle, OutputMode::Json, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["summary"]["certificates"], 1);
        assert_eq!(v["summary"]["private_keys"], 1);
        assert_eq!(v["blocks"][0]["type"], "X.509 certificate");
        assert_eq!(v["blocks"][0]["der_bytes"], 3);
        assert!(v["blocks"][0]["sha256"].is_string());
        assert!(v["blocks"][0]["pem"]
            .as_str()
            .unwrap()
            .contains("-----BEGIN CERTIFICATE-----"));
    }

    #[test]
    fn pem_mode_reemits_blocks_with_headers() {
        let bundle = format!("{}{}", mk("CERTIFICATE", &[1, 2, 3]), mk("EC PRIVATE KEY", &[9]));
        let out = run(&bundle, OutputMode::Pem, false).unwrap();
        assert!(out.contains("# Block 1 of 2: X.509 certificate (block-1-certificate.pem)"));
        assert!(out.contains("# Block 2 of 2: SEC1 EC private key (block-2-ec-private-key.pem)"));
        assert!(out.contains("-----BEGIN EC PRIVATE KEY-----"));
    }

    #[test]
    fn tolerates_text_between_blocks() {
        let bundle = format!(
            "subject=CN=leaf\nissuer=CN=ca\n{}\nsome notes here\n{}\n",
            mk("CERTIFICATE", &[1, 2, 3]),
            mk("CERTIFICATE", &[4, 5, 6]),
        );
        let blocks = split(&bundle, false).unwrap();
        assert_eq!(blocks.len(), 2);
    }

    #[test]
    fn filename_slug_is_clean() {
        let blocks = split(&mk("EC PRIVATE KEY", &[1]), false).unwrap();
        assert_eq!(blocks[0].filename, "block-1-ec-private-key.pem");
    }

    #[test]
    fn rejects_empty_input() {
        let err = run("   ", OutputMode::Report, true).unwrap_err();
        assert!(err.contains("no PEM input"));
    }

    #[test]
    fn rejects_non_pem() {
        let err = run("just some text", OutputMode::Report, true).unwrap_err();
        assert!(err.contains("does not look like PEM"));
    }

    #[test]
    fn parse_output_variants() {
        assert_eq!(parse_output("report").unwrap(), OutputMode::Report);
        assert_eq!(parse_output("JSON").unwrap(), OutputMode::Json);
        assert_eq!(parse_output("pem").unwrap(), OutputMode::Pem);
        assert_eq!(parse_output("blocks").unwrap(), OutputMode::Pem);
        assert!(parse_output("xml").is_err());
    }

    #[test]
    fn base64_roundtrip_matches_contents() {
        // sanity that reencode produces a body that decodes back to the bytes
        let pem_text = mk("CERTIFICATE", &[10, 20, 30, 40]);
        let parsed = pem::parse(pem_text.as_bytes()).unwrap();
        assert_eq!(parsed.contents(), &[10, 20, 30, 40]);
        // and that the armored body is base64 (decodable)
        let body: String = pem_text
            .lines()
            .filter(|l| !l.starts_with("-----"))
            .collect();
        assert!(B64.decode(body.as_bytes()).is_ok());
    }
}
