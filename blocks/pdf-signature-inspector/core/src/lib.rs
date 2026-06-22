//! gizza-ai/pdf-signature-inspector core — extract embedded digital signatures
//! from a PDF and report, per signature: the signer (certificate subject DN +
//! issuer), the signing time (from the signature dictionary `/M` and/or the CMS
//! signed `signingTime` attribute), the signature handler/sub-filter, and
//! whether the `/ByteRange` digest window is *structurally* intact (i.e. the
//! ranges cover the whole file except the `/Contents` gap, so no bytes were
//! appended or altered outside the protected region).
//!
//! No wafer/wasm-bindgen deps: compiles natively for unit tests and to
//! `wasm32-wasip1` (the `wafer build` target).
//!
//! ## Crates
//! - `lopdf` (default-features=false) — pure-Rust PDF object model; we walk the
//!   AcroForm `/Fields` (and any `/Kids`) looking for signature value (`/V`)
//!   dictionaries, then read `/ByteRange`, `/Contents`, `/M`, `/Name`,
//!   `/Reason`, `/Location`, `/SubFilter`, `/Filter`.
//! - `cms` + `x509-cert` + `der` (RustCrypto, pure-Rust) — parse the PKCS#7 /
//!   CMS `SignedData` blob in `/Contents` to recover the signer certificate's
//!   subject/issuer Distinguished Names and the signed `signingTime` attribute.
//!
//! ## ByteRange integrity (what "intact" means here)
//! A PDF signature signs the bytes named by `/ByteRange [a b c d]`: the file
//! from offset `a` for `b` bytes, then from offset `c` for `d` bytes. The gap
//! between the two ranges is exactly the hex `/Contents` string (the signature
//! can't sign itself). We check that the byte range is well-formed and that
//! `c + d == file length` (the protected window reaches the end of the file),
//! which means nothing was appended *outside* the signed region. This is a
//! structural integrity check — it does NOT recompute and verify the
//! cryptographic message digest against the certificate's public key (that
//! requires the full CMS digest/signature verification path); the report makes
//! that distinction explicit so callers don't over-trust the result.

use cms::content_info::ContentInfo;
use cms::signed_data::SignedData;
use der::{Decode, Tag, Tagged};
use lopdf::{Dictionary, Document, Object, ObjectId};

/// One embedded signature's extracted facts.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SignatureInfo {
    /// The signature field's fully-qualified name (`/T`), if present.
    pub field_name: Option<String>,
    /// `/Filter` — the signature handler (e.g. `Adobe.PPKLite`).
    pub filter: Option<String>,
    /// `/SubFilter` — the signature encoding (e.g.
    /// `adbe.pkcs7.detached`, `ETSI.CAdES.detached`).
    pub sub_filter: Option<String>,
    /// `/Name` — the human name the signer gave (free text, untrusted).
    pub name: Option<String>,
    /// `/Reason` for signing.
    pub reason: Option<String>,
    /// `/Location` of signing.
    pub location: Option<String>,
    /// `/M` — the claimed signing time from the signature dictionary, as the
    /// raw PDF date string (e.g. `D:20240115120000Z`).
    pub signing_time_dict: Option<String>,
    /// The signer certificate subject Distinguished Name (from the CMS), e.g.
    /// `CN=Alice, O=Acme`. `None` if the CMS could not be parsed.
    pub signer_subject: Option<String>,
    /// The signer certificate issuer Distinguished Name (from the CMS).
    pub signer_issuer: Option<String>,
    /// The CMS signed `signingTime` attribute, RFC3339-ish (e.g.
    /// `2024-01-15T12:00:00Z`). `None` if absent or unparsable.
    pub signing_time_cms: Option<String>,
    /// The `/ByteRange` array as `[a, b, c, d]`, if present.
    pub byte_range: Option<[i64; 4]>,
    /// True when the byte range is well-formed and the second range reaches the
    /// end of the file (no bytes appended outside the signed window). False
    /// when the range is malformed or the file was extended past it.
    pub byte_range_intact: bool,
    /// Human-readable note about the byte-range check / parse caveats.
    pub note: Option<String>,
}

/// The full inspection result.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inspection {
    pub signatures: Vec<SignatureInfo>,
}

/// Inspect a PDF's embedded signatures.
///
/// Returns `Err` only when the bytes don't parse as a PDF. A PDF with no
/// signatures parses fine and yields an empty `signatures` vec.
pub fn inspect(bytes: &[u8]) -> Result<Inspection, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let file_len = bytes.len() as i64;

    let mut sigs = Vec::new();
    for (field_id, sig_dict) in collect_signature_value_dicts(&doc) {
        sigs.push(parse_signature(&doc, field_id, &sig_dict, file_len));
    }

    Ok(Inspection { signatures: sigs })
}

/// Walk the AcroForm field tree and return every `(field_object_id,
/// signature_value_dict)` pair: a field whose `/V` is a signature dictionary
/// (`/Type /Sig`, or carrying `/ByteRange` + `/Contents`).
fn collect_signature_value_dicts(doc: &Document) -> Vec<(Option<ObjectId>, Dictionary)> {
    let mut out = Vec::new();

    // AcroForm /Fields is the canonical place; fall back to scanning all
    // objects so we still find signatures in odd documents.
    let mut field_ids: Vec<ObjectId> = Vec::new();
    if let Ok(catalog) = doc.catalog() {
        if let Some(acroform) = catalog.get(b"AcroForm").ok().and_then(|o| resolve_dict(doc, o)) {
            if let Ok(fields) = acroform.get(b"Fields") {
                collect_field_ids(doc, fields, &mut field_ids);
            }
        }
    }

    let mut seen_v: Vec<ObjectId> = Vec::new();
    for fid in &field_ids {
        if let Ok(Object::Dictionary(field)) = doc.get_object(*fid) {
            if let Ok(v) = field.get(b"V") {
                if let Some((vid, vdict)) = resolve_dict_with_id(doc, v) {
                    if is_signature_dict(&vdict) {
                        if let Some(id) = vid {
                            if seen_v.contains(&id) {
                                continue;
                            }
                            seen_v.push(id);
                        }
                        out.push((Some(*fid), vdict));
                    }
                }
            }
        }
    }

    // Fallback: if the AcroForm path found nothing, scan every object for a
    // bare signature dictionary.
    if out.is_empty() {
        for (id, obj) in &doc.objects {
            if let Object::Dictionary(d) = obj {
                if is_signature_dict(d) {
                    out.push((Some(*id), d.clone()));
                }
            }
        }
    }

    out
}

/// Recursively collect field object ids from a `/Fields` (or `/Kids`) array.
fn collect_field_ids(doc: &Document, fields: &Object, out: &mut Vec<ObjectId>) {
    let Ok(arr) = fields.as_array() else {
        return;
    };
    for item in arr {
        if let Object::Reference(id) = item {
            out.push(*id);
            // Descend into /Kids so signatures on nested widgets are found.
            if let Ok(Object::Dictionary(d)) = doc.get_object(*id) {
                if let Ok(kids) = d.get(b"Kids") {
                    collect_field_ids(doc, kids, out);
                }
            }
        }
    }
}

/// A signature value dictionary either declares `/Type /Sig` or carries the
/// signing fields `/ByteRange` + `/Contents`.
fn is_signature_dict(d: &Dictionary) -> bool {
    if let Ok(Object::Name(t)) = d.get(b"Type") {
        if t == b"Sig" {
            return true;
        }
    }
    d.has(b"ByteRange") && d.has(b"Contents")
}

fn resolve_dict<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Dictionary> {
    match obj {
        Object::Dictionary(d) => Some(d),
        Object::Reference(id) => doc.get_object(*id).ok().and_then(|o| o.as_dict().ok()),
        _ => None,
    }
}

/// Resolve an object (possibly a reference) to an owned dictionary, returning
/// its object id when it was a reference.
fn resolve_dict_with_id(doc: &Document, obj: &Object) -> Option<(Option<ObjectId>, Dictionary)> {
    match obj {
        Object::Dictionary(d) => Some((None, d.clone())),
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .map(|d| (Some(*id), d.clone())),
        _ => None,
    }
}

fn dict_string(d: &Dictionary, key: &[u8]) -> Option<String> {
    match d.get(key) {
        Ok(Object::String(bytes, _)) => Some(decode_pdf_text(bytes)),
        Ok(Object::Name(bytes)) => Some(String::from_utf8_lossy(bytes).into_owned()),
        _ => None,
    }
}

/// Decode a PDF text string: UTF-16BE with a BOM, else assume Latin-1/PDFDoc.
fn decode_pdf_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

fn parse_signature(
    doc: &Document,
    field_id: Option<ObjectId>,
    sig: &Dictionary,
    file_len: i64,
) -> SignatureInfo {
    let mut info = SignatureInfo {
        filter: dict_string(sig, b"Filter"),
        sub_filter: dict_string(sig, b"SubFilter"),
        name: dict_string(sig, b"Name"),
        reason: dict_string(sig, b"Reason"),
        location: dict_string(sig, b"Location"),
        signing_time_dict: dict_string(sig, b"M"),
        ..Default::default()
    };

    // Field name (/T) lives on the field dict, not the value dict.
    if let Some(fid) = field_id {
        if let Ok(Object::Dictionary(field)) = doc.get_object(fid) {
            info.field_name = dict_string(field, b"T");
        }
    }

    // /ByteRange — four integers.
    let byte_range = read_byte_range(sig);
    info.byte_range = byte_range;
    let (intact, mut note) = check_byte_range(byte_range, file_len);
    info.byte_range_intact = intact;

    // /Contents — the DER-encoded PKCS#7 / CMS SignedData (hex or literal bytes).
    if let Some(cms_bytes) = read_contents(sig) {
        match parse_cms(&cms_bytes) {
            Ok((subject, issuer, signing_time)) => {
                info.signer_subject = subject;
                info.signer_issuer = issuer;
                info.signing_time_cms = signing_time;
            }
            Err(e) => {
                let msg = format!("could not parse PKCS#7 signature: {e}");
                note = Some(match note {
                    Some(n) => format!("{n}; {msg}"),
                    None => msg,
                });
            }
        }
    }

    info.note = note;
    info
}

fn read_byte_range(sig: &Dictionary) -> Option<[i64; 4]> {
    let arr = sig.get(b"ByteRange").ok()?.as_array().ok()?;
    if arr.len() != 4 {
        return None;
    }
    let mut out = [0i64; 4];
    for (i, v) in arr.iter().enumerate() {
        out[i] = v.as_i64().ok()?;
    }
    Some(out)
}

/// Returns `(intact, note)`. Intact = the range is well-formed and the second
/// segment reaches the end of the file.
fn check_byte_range(br: Option<[i64; 4]>, file_len: i64) -> (bool, Option<String>) {
    let Some([a, b, c, d]) = br else {
        return (
            false,
            Some("no /ByteRange present; cannot assess integrity".to_string()),
        );
    };
    if a < 0 || b < 0 || c < 0 || d < 0 {
        return (false, Some("/ByteRange has negative offsets".to_string()));
    }
    // First range must start at 0 for the signature to cover the document head.
    if a != 0 {
        return (
            false,
            Some(format!("/ByteRange does not start at offset 0 (starts at {a})")),
        );
    }
    // The two ranges must be ordered and non-overlapping: a+b <= c.
    if a.saturating_add(b) > c {
        return (
            false,
            Some("/ByteRange segments overlap or are out of order".to_string()),
        );
    }
    let end = c.saturating_add(d);
    if end == file_len {
        (true, None)
    } else if end < file_len {
        (
            false,
            Some(format!(
                "{} byte(s) exist after the signed range — the file was modified/extended after signing",
                file_len - end
            )),
        )
    } else {
        (
            false,
            Some("/ByteRange extends past the end of the file (malformed)".to_string()),
        )
    }
}

/// Read `/Contents`. lopdf stores it as an `Object::String`; the raw bytes are
/// the DER blob (lopdf already decodes a hex string `<...>` into bytes).
fn read_contents(sig: &Dictionary) -> Option<Vec<u8>> {
    match sig.get(b"Contents") {
        Ok(Object::String(bytes, _)) => Some(bytes.clone()),
        _ => None,
    }
}

/// Decode a `ContentInfo` from a slice that may have trailing padding bytes
/// (PDF zero-pads the fixed-size `/Contents` buffer). Reads one top-level DER
/// value from the front and ignores anything after it.
fn decode_content_info(bytes: &[u8]) -> Result<ContentInfo, der::Error> {
    use der::SliceReader;
    let mut reader = SliceReader::new(bytes)?;
    let ci = ContentInfo::decode(&mut reader)?;
    // Do NOT call `reader.finish(..)` — trailing padding is expected.
    Ok(ci)
}

/// Parse the CMS SignedData and return `(subject_dn, issuer_dn, signing_time)`.
fn parse_cms(
    der_bytes: &[u8],
) -> Result<(Option<String>, Option<String>, Option<String>), String> {
    // PDF signature /Contents is a fixed-size buffer the signer over-allocates
    // and right-pads with zero bytes (the DER blob is shorter than the slot).
    // `from_der` is strict about trailing data, so decode from a reader and
    // accept whatever DER bytes precede the padding rather than requiring the
    // whole slice to be consumed.
    let ci = decode_content_info(der_bytes)
        .map_err(|e| format!("not valid DER ContentInfo: {e}"))?;
    let sd: SignedData = ci
        .content
        .decode_as()
        .map_err(|e| format!("ContentInfo is not SignedData: {e}"))?;

    // Subject + issuer from the *signer's* certificate. The certificates bag
    // holds the whole chain (often root-first), so picking the first cert can
    // name a CA, not the signer. Match the SignerInfo's SID
    // (IssuerAndSerialNumber) against the bag; fall back to the first cert if no
    // SignerInfo / no match.
    let (mut subject, mut issuer) = (None, None);
    if let Some(certs) = &sd.certificates {
        let target = signer_issuer_and_serial(&sd);
        let mut chosen: Option<&x509_cert::Certificate> = None;
        let mut first: Option<&x509_cert::Certificate> = None;
        for c in certs.0.iter() {
            if let cms::cert::CertificateChoices::Certificate(cert) = c {
                if first.is_none() {
                    first = Some(cert);
                }
                if let Some((ref iss, ref serial)) = target {
                    if &cert.tbs_certificate.issuer == iss
                        && &cert.tbs_certificate.serial_number == serial
                    {
                        chosen = Some(cert);
                        break;
                    }
                }
            }
        }
        if let Some(cert) = chosen.or(first) {
            subject = Some(cert.tbs_certificate.subject.to_string());
            issuer = Some(cert.tbs_certificate.issuer.to_string());
        }
    }

    // signingTime signed attribute (OID 1.2.840.113549.1.9.5).
    let signing_time = extract_signing_time(&sd);

    Ok((subject, issuer, signing_time))
}

/// The first SignerInfo's `IssuerAndSerialNumber`, used to find that signer's
/// certificate in the bag. Returns `None` for the (rare) subject-key-identifier
/// SID form.
fn signer_issuer_and_serial(
    sd: &SignedData,
) -> Option<(x509_cert::name::Name, x509_cert::serial_number::SerialNumber)> {
    let si = sd.signer_infos.0.iter().next()?;
    match &si.sid {
        cms::signed_data::SignerIdentifier::IssuerAndSerialNumber(ias) => {
            Some((ias.issuer.clone(), ias.serial_number.clone()))
        }
        _ => None,
    }
}

/// Pull the `signingTime` (PKCS#9) signed attribute out of the first signer.
fn extract_signing_time(sd: &SignedData) -> Option<String> {
    const SIGNING_TIME_OID: const_oid::ObjectIdentifier =
        const_oid::ObjectIdentifier::new_unwrap("1.2.840.113549.1.9.5");
    for si in sd.signer_infos.0.iter() {
        let Some(attrs) = &si.signed_attrs else {
            continue;
        };
        for attr in attrs.iter() {
            if attr.oid == SIGNING_TIME_OID {
                if let Some(val) = attr.values.iter().next() {
                    if let Some(t) = decode_time(val) {
                        return Some(t);
                    }
                }
            }
        }
    }
    None
}

/// Decode a CMS `Time` (UTCTime or GeneralizedTime) ASN.1 value to RFC3339-ish.
fn decode_time(val: &der::Any) -> Option<String> {
    use der::asn1::{GeneralizedTime, UtcTime};
    match val.tag() {
        Tag::UtcTime => val
            .decode_as::<UtcTime>()
            .ok()
            .map(|t| fmt_unix(t.to_unix_duration().as_secs())),
        Tag::GeneralizedTime => val
            .decode_as::<GeneralizedTime>()
            .ok()
            .map(|t| fmt_unix(t.to_unix_duration().as_secs())),
        _ => None,
    }
}

/// Format seconds-since-epoch as a UTC `YYYY-MM-DDTHH:MM:SSZ` string without a
/// date crate (keeps the tree thread-free and tiny).
fn fmt_unix(secs: u64) -> String {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let (h, mi, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (y, mo, d) = civil_from_days(days);
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Howard Hinnant's days-from-civil inverse (proleptic Gregorian).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    /// Build a minimal one-page PDF and return (bytes). Helper for the
    /// no-signature + malformed-signature tests (we can't easily forge a valid
    /// CMS blob here, so CMS parsing is exercised against real fixtures via the
    /// CLI; here we test the PDF/byte-range plumbing).
    fn build_plain_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = Content {
            operations: vec![Operation::new("BT", vec![]), Operation::new("ET", vec![])],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    /// Build a PDF carrying one signature dictionary with the given byte range
    /// and (optional) contents bytes, wired through an AcroForm field.
    fn build_signed_pdf(byte_range: [i64; 4], contents: Vec<u8>, name: &str) -> Vec<u8> {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));

        let br: Vec<Object> = byte_range.iter().map(|&n| n.into()).collect();
        let sig_id = doc.add_object(dictionary! {
            "Type" => "Sig",
            "Filter" => Object::Name(b"Adobe.PPKLite".to_vec()),
            "SubFilter" => Object::Name(b"adbe.pkcs7.detached".to_vec()),
            "Name" => Object::string_literal(name),
            "Reason" => Object::string_literal("Approval"),
            "M" => Object::string_literal("D:20240115120000Z"),
            "ByteRange" => br,
            "Contents" => Object::String(contents, lopdf::StringFormat::Hexadecimal),
        });
        let field_id = doc.add_object(dictionary! {
            "FT" => Object::Name(b"Sig".to_vec()),
            "T" => Object::string_literal("Signature1"),
            "V" => Object::Reference(sig_id),
        });
        let acroform_id = doc.add_object(dictionary! {
            "Fields" => vec![Object::Reference(field_id)],
            "SigFlags" => 3,
        });
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
            "AcroForm" => Object::Reference(acroform_id),
        });
        doc.trailer.set("Root", catalog_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = inspect(b"definitely not a pdf").unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn unsigned_pdf_has_no_signatures() {
        let pdf = build_plain_pdf();
        let out = inspect(&pdf).unwrap();
        assert!(out.signatures.is_empty(), "got: {:?}", out.signatures);
    }

    #[test]
    fn finds_signature_dict_fields() {
        // A non-PKCS7 contents blob: CMS parse fails (noted), but the dict
        // fields + field name are extracted.
        let pdf = build_signed_pdf([0, 100, 200, 50], vec![0xDE, 0xAD], "Alice Example");
        let out = inspect(&pdf).unwrap();
        assert_eq!(out.signatures.len(), 1);
        let s = &out.signatures[0];
        assert_eq!(s.field_name.as_deref(), Some("Signature1"));
        assert_eq!(s.filter.as_deref(), Some("Adobe.PPKLite"));
        assert_eq!(s.sub_filter.as_deref(), Some("adbe.pkcs7.detached"));
        assert_eq!(s.name.as_deref(), Some("Alice Example"));
        assert_eq!(s.reason.as_deref(), Some("Approval"));
        assert_eq!(s.signing_time_dict.as_deref(), Some("D:20240115120000Z"));
        assert_eq!(s.byte_range, Some([0, 100, 200, 50]));
        // Bad contents → CMS parse note.
        assert!(s.note.as_deref().unwrap().contains("PKCS#7"), "{:?}", s.note);
    }

    #[test]
    fn byte_range_intact_when_reaches_eof() {
        // We don't know the exact saved length up-front, so build, measure,
        // then assert the check against the real length.
        let pdf = build_signed_pdf([0, 100, 200, 50], vec![0x01], "X");
        let len = pdf.len() as i64;
        let (ok, note) = check_byte_range(Some([0, 100, 200, len - 200]), len);
        assert!(ok, "note: {note:?}");
        assert!(note.is_none());
    }

    #[test]
    fn byte_range_detects_trailing_bytes() {
        let (ok, note) = check_byte_range(Some([0, 100, 200, 50]), 300);
        assert!(!ok);
        assert!(note.unwrap().contains("modified/extended"));
    }

    #[test]
    fn byte_range_rejects_non_zero_start() {
        let (ok, note) = check_byte_range(Some([10, 100, 200, 50]), 250);
        assert!(!ok);
        assert!(note.unwrap().contains("offset 0"));
    }

    #[test]
    fn byte_range_rejects_overlap() {
        let (ok, note) = check_byte_range(Some([0, 250, 200, 50]), 250);
        assert!(!ok);
        assert!(note.unwrap().contains("overlap"));
    }

    #[test]
    fn missing_byte_range_is_not_intact() {
        let (ok, note) = check_byte_range(None, 100);
        assert!(!ok);
        assert!(note.unwrap().contains("no /ByteRange"));
    }

    #[test]
    fn fmt_unix_epoch_is_1970() {
        assert_eq!(fmt_unix(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn fmt_unix_known_timestamp() {
        // 2024-01-15T12:00:00Z = 1705320000
        assert_eq!(fmt_unix(1_705_320_000), "2024-01-15T12:00:00Z");
    }
}
