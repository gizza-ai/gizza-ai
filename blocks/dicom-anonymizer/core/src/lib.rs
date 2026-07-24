//! dicom-anonymizer core — pure compute, shared by the chat skill block and the CLI.
//!
//! Walks a Part-10 DICOM byte stream and overwrites the value bytes of common
//! patient-identifying (PHI) data elements **in place**, keeping every byte
//! offset — and therefore all lengths, the file-meta group, and the pixel data
//! — untouched. Because the replacement is exactly as long as the original
//! value, no length fields need rewriting and the structure is preserved
//! byte-for-byte apart from the redacted values.
//!
//! Supported encodings (dataset): Explicit VR Little Endian
//! (`1.2.840.10008.1.2.1`) and Implicit VR Little Endian (`1.2.840.10008.1.2`).
//! The File Meta Information group (group 0002) is always Explicit VR LE. Big
//! endian / deflated / encapsulated (compressed) transfer syntaxes are rejected
//! with a clear error.
//!
//! No wafer/wasm deps — natively unit-testable.

// ---------------------------------------------------------------------------
// Public surface
// ---------------------------------------------------------------------------

/// Redaction profile — which elements get their values wiped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// Common patient-identifying tags only (names, IDs, dates, descriptions).
    Basic,
    /// Common PHI tags **plus** every private (odd-group) data element, whose
    /// meaning is vendor-defined and may hide identifiers.
    Strict,
}

impl Profile {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "basic" => Ok(Profile::Basic),
            "strict" => Ok(Profile::Strict),
            other => Err(format!(
                "unknown profile {other:?} — use \"basic\" or \"strict\""
            )),
        }
    }
}

/// Summary of what the anonymizer did, surfaced to the LLM/CLI.
#[derive(Debug, Clone, PartialEq)]
pub struct Report {
    /// Number of data elements whose value bytes were overwritten.
    pub elements_redacted: usize,
    /// Total bytes overwritten across all redacted elements.
    pub bytes_redacted: usize,
    /// Bytes left untouched (`input_len - bytes_redacted`) — pixel data,
    /// structure, and non-PHI values.
    pub bytes_preserved: usize,
    /// Total input size in bytes (output is identical in length).
    pub total_bytes: usize,
    /// Dataset encoding that was walked.
    pub encoding: &'static str,
}

/// Anonymize a DICOM byte stream. Returns the sanitized bytes (same length as
/// the input) and a [`Report`]. Errors on non-DICOM input or an unsupported
/// transfer syntax.
pub fn anonymize(dicom: &[u8], profile: Profile, placeholder: &str) -> Result<(Vec<u8>, Report), String> {
    // Part-10: 128-byte preamble then the "DICM" magic.
    if dicom.len() < 132 || &dicom[128..132] != b"DICM" {
        return Err(
            "not a DICOM file: missing the 128-byte preamble + \"DICM\" magic (only Part-10 \
             DICOM files are supported)"
                .to_string(),
        );
    }

    let mut r = Reader::new(dicom);
    r.pos = 132;
    let transfer_syntax = read_meta_transfer_syntax(&mut r)?;
    let enc = encoding_for(&transfer_syntax)?;

    // Collect redaction ranges over the immutable input, then apply to a copy.
    // Collecting first keeps the walk borrow-free of the output buffer.
    let mut reds: Vec<Redaction> = Vec::new();
    walk(&mut r, enc, None, false, profile, &mut reds)?;

    let mut out = dicom.to_vec();
    let mut bytes_redacted = 0usize;
    for red in &reds {
        let end = red.offset + red.len;
        if end <= out.len() {
            apply_redaction(&mut out[red.offset..end], placeholder, red.kind);
            bytes_redacted += red.len;
        }
    }

    let report = Report {
        elements_redacted: reds.len(),
        bytes_redacted,
        bytes_preserved: dicom.len().saturating_sub(bytes_redacted),
        total_bytes: dicom.len(),
        encoding: enc.label(),
    };
    Ok((out, report))
}

// ---------------------------------------------------------------------------
// Byte reader (little-endian), mirroring blocks/dicom-to-image.
// ---------------------------------------------------------------------------

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let b = self.take(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| "corrupt DICOM: length overflow".to_string())?;
        if end > self.buf.len() {
            return Err("corrupt DICOM: unexpected end of file".to_string());
        }
        let s = &self.buf[self.pos..end];
        self.pos = end;
        Ok(s)
    }
    fn skip(&mut self, n: usize) -> Result<(), String> {
        self.take(n).map(|_| ())
    }
    fn peek_u16(&self) -> Option<u16> {
        if self.pos + 2 <= self.buf.len() {
            Some(u16::from_le_bytes([self.buf[self.pos], self.buf[self.pos + 1]]))
        } else {
            None
        }
    }
}

const UNDEFINED_LEN: u32 = 0xFFFF_FFFF;
const TAG_ITEM: (u16, u16) = (0xFFFE, 0xE000);
const TAG_ITEM_DELIM: (u16, u16) = (0xFFFE, 0xE00D);
const TAG_SEQ_DELIM: (u16, u16) = (0xFFFE, 0xE0DD);
const TAG_PIXEL_DATA: (u16, u16) = (0x7FE0, 0x0010);

#[derive(Clone, Copy)]
enum Encoding {
    ImplicitLe,
    ExplicitLe,
}

impl Encoding {
    fn label(self) -> &'static str {
        match self {
            Encoding::ImplicitLe => "Implicit VR Little Endian",
            Encoding::ExplicitLe => "Explicit VR Little Endian",
        }
    }
}

/// Explicit-VR long-form VRs carry a 2-byte reserved field then a 4-byte length;
/// all others use a 2-byte length.
fn is_long_vr(vr: &[u8]) -> bool {
    matches!(
        vr,
        b"OB" | b"OW" | b"OF" | b"OD" | b"OL" | b"OV" | b"SQ" | b"UT" | b"UN" | b"UC" | b"UR"
    )
}

/// Text-valued VRs — redacted with the (space-padded) placeholder so the field
/// stays a readable, valid string. Everything else is zero-filled.
fn is_text_vr(vr: &[u8]) -> bool {
    matches!(
        vr,
        b"AE" | b"AS" | b"CS" | b"DA" | b"DS" | b"DT" | b"IS" | b"LO" | b"LT" | b"PN"
            | b"SH" | b"ST" | b"TM" | b"UC" | b"UI" | b"UR" | b"UT"
    )
}

fn encoding_for(uid: &str) -> Result<Encoding, String> {
    match uid {
        "1.2.840.10008.1.2" => Ok(Encoding::ImplicitLe),
        "1.2.840.10008.1.2.1" => Ok(Encoding::ExplicitLe),
        "1.2.840.10008.1.2.1.99" => {
            Err("unsupported: Deflated Explicit VR Little Endian transfer syntax".to_string())
        }
        "1.2.840.10008.1.2.2" => {
            Err("unsupported: Explicit VR Big Endian transfer syntax".to_string())
        }
        other => Err(format!(
            "unsupported transfer syntax {other:?} — this tool anonymizes only uncompressed \
             Implicit/Explicit VR Little Endian DICOM (compressed/encapsulated JPEG/JPEG2000/\
             JPEG-LS/RLE pixel data is not supported)"
        )),
    }
}

/// Parse the File Meta Information group (group 0002, always Explicit VR LE) and
/// return the Transfer Syntax UID (0002,0010). Leaves the reader positioned at
/// the first dataset element.
fn read_meta_transfer_syntax(r: &mut Reader) -> Result<String, String> {
    let mut transfer_syntax: Option<String> = None;
    while let Some(group) = r.peek_u16() {
        if group != 0x0002 {
            break;
        }
        let (el_group, el_elem, _vr, len, undefined) = read_header(r, Encoding::ExplicitLe)?;
        if undefined {
            return Err("corrupt DICOM: undefined length in file meta group".to_string());
        }
        let value = r.take(len as usize)?;
        if (el_group, el_elem) == (0x0002, 0x0010) {
            transfer_syntax = Some(parse_str(value));
        }
    }
    transfer_syntax.ok_or_else(|| {
        "not a supported DICOM file: missing Transfer Syntax UID (0002,0010)".to_string()
    })
}

/// Parse a DICOM string value: strip trailing NUL/space padding.
fn parse_str(bytes: &[u8]) -> String {
    let s = String::from_utf8_lossy(bytes);
    s.trim_matches(|c: char| c == '\0' || c == ' ').to_string()
}

// ---------------------------------------------------------------------------
// Element header
// ---------------------------------------------------------------------------

/// Read one element header, returning `(group, elem, vr, length, undefined)`.
/// `vr` is `None` under Implicit VR. The reader is left at the element's value.
/// Group `0xFFFE` items/delimiters are always encoded tag + 4-byte length.
fn read_header(
    r: &mut Reader,
    enc: Encoding,
) -> Result<(u16, u16, Option<[u8; 2]>, u32, bool), String> {
    let group = r.u16()?;
    let elem = r.u16()?;
    if group == 0xFFFE {
        let length = r.u32()?;
        return Ok((group, elem, None, length, length == UNDEFINED_LEN));
    }
    let (vr, length) = match enc {
        Encoding::ImplicitLe => (None, r.u32()?),
        Encoding::ExplicitLe => {
            let v = r.take(2)?;
            let vr = [v[0], v[1]];
            let length = if is_long_vr(&vr) {
                r.skip(2)?; // reserved
                r.u32()?
            } else {
                r.u16()? as u32
            };
            (Some(vr), length)
        }
    };
    Ok((group, elem, vr, length, length == UNDEFINED_LEN))
}

// ---------------------------------------------------------------------------
// Walk + redaction planning
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Fill {
    Text,
    Binary,
}

struct Redaction {
    offset: usize,
    len: usize,
    kind: Fill,
}

/// Walk a dataset region collecting redaction ranges. `end` bounds the region
/// (`None` = to EOF); `in_item` means an Item-Delimitation tag terminates it
/// (undefined-length sequence item). Recurses into sequences.
fn walk(
    r: &mut Reader,
    enc: Encoding,
    end: Option<usize>,
    in_item: bool,
    profile: Profile,
    reds: &mut Vec<Redaction>,
) -> Result<(), String> {
    loop {
        if let Some(e) = end {
            if r.pos >= e {
                break;
            }
        }
        if r.remaining() < 8 {
            break;
        }
        let (group, elem, vr, len, undefined) = read_header(r, enc)?;

        // Item/sequence delimiters that bound an undefined-length item.
        if (group, elem) == TAG_ITEM_DELIM {
            if in_item {
                return Ok(());
            }
            continue;
        }
        if (group, elem) == TAG_SEQ_DELIM {
            return Ok(());
        }

        // Pixel data — everything we redact precedes it; stop and preserve it.
        if (group, elem) == TAG_PIXEL_DATA {
            return Ok(());
        }

        let is_sq = matches!(vr, Some(v) if &v == b"SQ");
        if undefined {
            // Undefined length ⇒ a sequence (SQ), even under Implicit VR.
            walk_sequence(r, enc, None, profile, reds)?;
            continue;
        }
        if is_sq {
            let value_end = r.pos + len as usize;
            walk_sequence(r, enc, Some(value_end), profile, reds)?;
            r.pos = value_end.min(r.buf.len());
            continue;
        }

        // Leaf element.
        let value_off = r.pos;
        if len > 0 && should_redact(group, elem, profile) {
            reds.push(Redaction {
                offset: value_off,
                len: len as usize,
                kind: fill_kind(vr, group, elem),
            });
        }
        r.skip(len as usize)?;
    }
    Ok(())
}

/// Walk the items of a sequence, recursing into each item's dataset. `end`
/// bounds a defined-length sequence (`None` = undefined, terminated by the
/// Sequence-Delimitation item).
fn walk_sequence(
    r: &mut Reader,
    enc: Encoding,
    end: Option<usize>,
    profile: Profile,
    reds: &mut Vec<Redaction>,
) -> Result<(), String> {
    loop {
        if let Some(e) = end {
            if r.pos >= e {
                return Ok(());
            }
        }
        if r.remaining() < 8 {
            return Ok(());
        }
        let g = r.u16()?;
        let el = r.u16()?;
        let len = r.u32()?;
        if (g, el) == TAG_SEQ_DELIM {
            return Ok(());
        }
        if (g, el) == TAG_ITEM {
            if len == UNDEFINED_LEN {
                walk(r, enc, None, true, profile, reds)?;
            } else {
                let item_end = r.pos + len as usize;
                walk(r, enc, Some(item_end), false, profile, reds)?;
                r.pos = item_end.min(r.buf.len());
            }
        } else {
            return Err("corrupt DICOM: malformed sequence item".to_string());
        }
    }
}

/// The common patient-identifying data elements redacted under every profile.
fn is_known_phi(group: u16, elem: u16) -> bool {
    matches!(
        (group, elem),
        (0x0008, 0x0050) // AccessionNumber
            | (0x0008, 0x0080) // InstitutionName
            | (0x0008, 0x0090) // ReferringPhysicianName
            | (0x0008, 0x1030) // StudyDescription
            | (0x0008, 0x103E) // SeriesDescription
            | (0x0008, 0x1070) // OperatorsName
            | (0x0010, 0x0010) // PatientName
            | (0x0010, 0x0020) // PatientID
            | (0x0010, 0x0030) // PatientBirthDate
            | (0x0010, 0x0040) // PatientSex
            | (0x0010, 0x1000) // OtherPatientIDs
            | (0x0010, 0x1001) // OtherPatientNames
            | (0x0010, 0x1010) // PatientAge
            | (0x0010, 0x1040) // PatientAddress
            | (0x0010, 0x2154) // PatientTelephoneNumbers
            | (0x0020, 0x0010) // StudyID
            | (0x0032, 0x1032) // RequestingPhysician
            | (0x0032, 0x1060) // RequestedProcedureDescription
            | (0x0040, 0x0254) // PerformedProcedureStepDescription
    )
}

/// Whether an element's value should be wiped under `profile`. Private
/// (odd-group) elements are redacted only under `Strict`. Group `0002` (file
/// meta) is never a dataset element here, and group-length elements (elem 0)
/// carry no PHI, so we leave them intact.
fn should_redact(group: u16, elem: u16, profile: Profile) -> bool {
    if is_known_phi(group, elem) {
        return true;
    }
    if profile == Profile::Strict && group % 2 == 1 && group != 0xFFFE && elem != 0x0000 {
        return true;
    }
    false
}

/// How to overwrite a redacted value. Explicit VR chooses by the element's VR;
/// Implicit VR falls back to "known PHI tags are textual, unknown/private are
/// binary" so we never write ASCII into a numeric/binary field.
fn fill_kind(vr: Option<[u8; 2]>, group: u16, elem: u16) -> Fill {
    match vr {
        Some(v) if is_text_vr(&v) => Fill::Text,
        Some(_) => Fill::Binary,
        None => {
            if is_known_phi(group, elem) {
                Fill::Text
            } else {
                Fill::Binary
            }
        }
    }
}

/// Overwrite `buf` in place, preserving its length. Text fields get the
/// placeholder (truncated as needed) then space padding; binary fields are
/// zeroed.
fn apply_redaction(buf: &mut [u8], placeholder: &str, kind: Fill) {
    match kind {
        Fill::Binary => buf.fill(0),
        Fill::Text => {
            let ph = placeholder.as_bytes();
            for (i, b) in buf.iter_mut().enumerate() {
                *b = if i < ph.len() { ph[i] } else { b' ' };
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- tiny DICOM byte builder (mirrors blocks/dicom-to-image) ------------

    fn push_tag(out: &mut Vec<u8>, g: u16, e: u16) {
        out.extend_from_slice(&g.to_le_bytes());
        out.extend_from_slice(&e.to_le_bytes());
    }

    fn explicit_short(out: &mut Vec<u8>, g: u16, e: u16, vr: &[u8; 2], value: &[u8]) {
        push_tag(out, g, e);
        out.extend_from_slice(vr);
        out.extend_from_slice(&(value.len() as u16).to_le_bytes());
        out.extend_from_slice(value);
    }

    fn explicit_long(out: &mut Vec<u8>, g: u16, e: u16, vr: &[u8; 2], value: &[u8]) {
        push_tag(out, g, e);
        out.extend_from_slice(vr);
        out.extend_from_slice(&[0, 0]); // reserved
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value);
    }

    fn implicit(out: &mut Vec<u8>, g: u16, e: u16, value: &[u8]) {
        push_tag(out, g, e);
        out.extend_from_slice(&(value.len() as u32).to_le_bytes());
        out.extend_from_slice(value);
    }

    /// Even-length pad a string with a trailing space.
    fn dcm_str(s: &str) -> Vec<u8> {
        let mut b = s.as_bytes().to_vec();
        if b.len() % 2 == 1 {
            b.push(b' ');
        }
        b
    }

    fn preamble_and_meta(transfer_syntax: &str) -> Vec<u8> {
        let mut out = vec![0u8; 128];
        out.extend_from_slice(b"DICM");
        explicit_short(&mut out, 0x0002, 0x0010, b"UI", &dcm_str(transfer_syntax));
        out
    }

    /// Find `needle` anywhere in `hay`.
    fn contains(hay: &[u8], needle: &[u8]) -> bool {
        hay.windows(needle.len()).any(|w| w == needle)
    }

    #[test]
    fn explicit_vr_happy_path_redacts_phi_keeps_pixels() {
        let mut dcm = preamble_and_meta("1.2.840.10008.1.2.1");
        explicit_short(&mut dcm, 0x0010, 0x0010, b"PN", &dcm_str("DOE^JOHN")); // PatientName
        explicit_short(&mut dcm, 0x0010, 0x0020, b"LO", &dcm_str("MRN-12345")); // PatientID
        // Non-PHI element that must survive.
        explicit_short(&mut dcm, 0x0028, 0x0010, b"US", &4u16.to_le_bytes());
        let pixels: &[u8] = &[0xDE, 0xAD, 0xBE, 0xEF, 0x01, 0x02, 0x03, 0x04];
        explicit_long(&mut dcm, 0x7FE0, 0x0010, b"OW", pixels);

        let (out, report) = anonymize(&dcm, Profile::Basic, "ANON").unwrap();

        assert_eq!(out.len(), dcm.len(), "length preserved (no offset rewrite)");
        assert!(!contains(&out, b"DOE^JOHN"), "PatientName gone");
        assert!(!contains(&out, b"MRN-12345"), "PatientID gone");
        assert!(contains(&out, b"ANON"), "placeholder written");
        // Pixel data untouched.
        assert!(contains(&out, pixels), "pixel bytes unchanged");
        // Non-PHI US value (4) survives.
        assert!(contains(&out, &4u16.to_le_bytes()));
        assert_eq!(report.elements_redacted, 2);
        assert_eq!(report.encoding, "Explicit VR Little Endian");
        assert_eq!(report.total_bytes, dcm.len());
        assert_eq!(report.bytes_preserved, dcm.len() - report.bytes_redacted);
    }

    #[test]
    fn implicit_vr_redacts_known_phi() {
        let mut dcm = preamble_and_meta("1.2.840.10008.1.2"); // Implicit VR LE dataset
        implicit(&mut dcm, 0x0010, 0x0010, &dcm_str("SMITH^JANE"));
        implicit(&mut dcm, 0x0010, 0x0020, &dcm_str("PID-999"));
        implicit(&mut dcm, 0x7FE0, 0x0010, &[0x11, 0x22, 0x33, 0x44]);

        let (out, report) = anonymize(&dcm, Profile::Basic, "ANON").unwrap();
        assert!(!contains(&out, b"SMITH^JANE"));
        assert!(!contains(&out, b"PID-999"));
        assert!(contains(&out, &[0x11, 0x22, 0x33, 0x44]), "pixels kept");
        assert_eq!(report.elements_redacted, 2);
        assert_eq!(report.encoding, "Implicit VR Little Endian");
    }

    #[test]
    fn non_dicom_errors() {
        let junk = vec![0u8; 200];
        let err = anonymize(&junk, Profile::Basic, "ANON").unwrap_err();
        assert!(err.contains("DICM"), "clear missing-magic error: {err}");

        let too_short = vec![0u8; 10];
        assert!(anonymize(&too_short, Profile::Basic, "ANON").is_err());
    }

    #[test]
    fn strict_redacts_private_odd_group_tags() {
        let mut dcm = preamble_and_meta("1.2.840.10008.1.2.1");
        // Private (odd group) element carrying a secret; VR LO (text).
        explicit_short(&mut dcm, 0x0009, 0x0010, b"LO", &dcm_str("SECRET-PRIVATE"));
        // A non-PHI standard (even group) element that must survive.
        explicit_short(&mut dcm, 0x0028, 0x0010, b"US", &4u16.to_le_bytes());

        // Basic leaves the private tag alone.
        let (basic_out, basic_rep) = anonymize(&dcm, Profile::Basic, "ANON").unwrap();
        assert!(contains(&basic_out, b"SECRET-PRIVATE"), "basic keeps private");
        assert_eq!(basic_rep.elements_redacted, 0);

        // Strict wipes it.
        let (strict_out, strict_rep) = anonymize(&dcm, Profile::Strict, "ANON").unwrap();
        assert!(!contains(&strict_out, b"SECRET-PRIVATE"), "strict wipes private");
        assert!(contains(&strict_out, &4u16.to_le_bytes()), "even-group US survives");
        assert_eq!(strict_rep.elements_redacted, 1);
    }

    #[test]
    fn unsupported_transfer_syntax_errors() {
        let dcm = preamble_and_meta("1.2.840.10008.1.2.2"); // Explicit VR Big Endian
        let err = anonymize(&dcm, Profile::Basic, "ANON").unwrap_err();
        assert!(err.contains("Big Endian"), "{err}");
    }

    #[test]
    fn profile_parse() {
        assert_eq!(Profile::parse("basic").unwrap(), Profile::Basic);
        assert_eq!(Profile::parse("STRICT").unwrap(), Profile::Strict);
        assert!(Profile::parse("bogus").is_err());
    }

    #[test]
    fn text_fill_preserves_length_and_pads() {
        let mut buf = *b"DOE^JOHN"; // 8 bytes
        apply_redaction(&mut buf, "ANON", Fill::Text);
        assert_eq!(&buf, b"ANON    ", "placeholder then space pad, same length");
    }
}
