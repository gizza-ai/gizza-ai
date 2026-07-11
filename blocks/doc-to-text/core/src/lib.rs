//! gizza-ai/doc-to-text core — extract readable plain text from a **legacy
//! Microsoft Word `.doc` (Word 97–2003)** file.
//!
//! Pure Rust with no wafer/wasm-bindgen deps, so it compiles natively for the
//! unit tests and to `wasm32-wasip1` (the `wafer build` target) for the chat
//! block.
//!
//! ## What a legacy `.doc` is (and how this differs from `.docx`)
//!
//! A Word 97–2003 `.doc` is an **OLE2 / Compound File Binary (CFB)** container
//! (magic `D0 CF 11 E0 A1 B1 1A E1`), NOT a ZIP. Inside it are named streams:
//! - `WordDocument` — starts with the **FIB** (File Information Block) and holds
//!   the raw character bytes of the body text.
//! - `0Table` **or** `1Table` — the "table stream"; which one is selected by the
//!   FIB `fWhichTblStm` flag. It holds the **piece table** (`CLX`), which maps
//!   logical character positions to byte ranges in `WordDocument` and says
//!   whether each range is 8-bit (Windows-1252, "compressed") or 16-bit UTF-16.
//!
//! We open the container with the `cfb` crate, read the FIB fields we need, walk
//! the piece table, decode each piece, then map Word's control marks (paragraph,
//! line break, cell, field codes, …) to plain text. This is the classic
//! antiword/`wvText` extraction path.
//!
//! ## Distinct from `docx-text-extract` / `document-text-extract`
//!
//! Those blocks parse the ZIP-based **`.docx`** (Office Open XML) and reject a
//! binary `.doc`. This block parses the *old* binary format they cannot read.

use std::io::{Cursor, Read};

/// OLE2 / Compound File Binary signature that begins every legacy `.doc`.
const OLE2_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// FIB `wIdent` — every Word FIB starts with this little-endian word.
const FIB_IDENT: u16 = 0xA5EC;

/// Cap on decoded output, applied by the caller's response builder too but also
/// used as a hard guard against a corrupt piece table claiming a huge run.
const MAX_TEXT_CHARS: usize = 1_000_000;

/// A successful extraction: the plain text plus a few structure counts the
/// caller surfaces in its JSON response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// The extracted plain text (control marks mapped; whitespace policy applied).
    pub text: String,
    /// Number of paragraphs (non-empty lines after mapping/normalisation).
    pub paragraphs: usize,
    /// Number of text pieces walked in the piece table.
    pub pieces: usize,
    /// True when the text was clipped to the [`MAX_TEXT_CHARS`] cap.
    pub truncated: bool,
}

/// How to post-process the extracted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    /// Collapse 3+ consecutive newlines to a single blank line and trim trailing
    /// spaces / outer whitespace — the default "clean text" output.
    Clean,
    /// Return the mapped text as-is (no cosmetic collapsing), preserving the
    /// original paragraph spacing.
    Raw,
}

impl Whitespace {
    /// Parse the `whitespace` param. Defaults to [`Whitespace::Clean`]; rejects
    /// unknown values.
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("clean") {
            "clean" => Ok(Whitespace::Clean),
            "raw" => Ok(Whitespace::Raw),
            other => Err(format!(
                "invalid whitespace {other:?}: expected \"clean\" or \"raw\""
            )),
        }
    }
}

/// Read a little-endian `u16` at `off` from `buf` (0 if out of range).
fn u16_at(buf: &[u8], off: usize) -> u16 {
    if off + 2 <= buf.len() {
        u16::from_le_bytes([buf[off], buf[off + 1]])
    } else {
        0
    }
}

/// Read a little-endian `u32` at `off` from `buf` (0 if out of range).
fn u32_at(buf: &[u8], off: usize) -> u32 {
    if off + 4 <= buf.len() {
        u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
    } else {
        0
    }
}

/// Windows-1252 → Unicode for the bytes that differ from Latin-1 (`0x80..=0x9F`).
/// `None` marks the five undefined code points, which we drop.
fn cp1252_high(byte: u8) -> Option<char> {
    Some(match byte {
        0x80 => '\u{20AC}', // €
        0x82 => '\u{201A}',
        0x83 => '\u{0192}',
        0x84 => '\u{201E}',
        0x85 => '\u{2026}', // …
        0x86 => '\u{2020}',
        0x87 => '\u{2021}',
        0x88 => '\u{02C6}',
        0x89 => '\u{2030}',
        0x8A => '\u{0160}',
        0x8B => '\u{2039}',
        0x8C => '\u{0152}',
        0x8E => '\u{017D}',
        0x91 => '\u{2018}', // ‘
        0x92 => '\u{2019}', // ’
        0x93 => '\u{201C}', // “
        0x94 => '\u{201D}', // ”
        0x95 => '\u{2022}', // •
        0x96 => '\u{2013}', // –
        0x97 => '\u{2014}', // —
        0x98 => '\u{02DC}',
        0x99 => '\u{2122}', // ™
        0x9A => '\u{0161}',
        0x9B => '\u{203A}',
        0x9C => '\u{0153}',
        0x9E => '\u{017E}',
        0x9F => '\u{0178}',
        _ => return None, // 0x81, 0x8D, 0x8F, 0x90, 0x9D are undefined
    })
}

/// Decode one Windows-1252 byte to a `char`.
fn cp1252_char(byte: u8) -> Option<char> {
    match byte {
        0x00..=0x7F => Some(byte as char),
        0x80..=0x9F => cp1252_high(byte),
        0xA0..=0xFF => Some(byte as char), // Latin-1 range == Unicode U+00A0..=U+00FF
    }
}

/// Map a raw Word character (from a decoded piece) into output text, honouring
/// the field-code depth so `HYPERLINK`-style field *instructions* are dropped and
/// only their displayed result survives. Pushes onto `out`; mutates `field_depth`
/// and `in_field_result`.
fn push_word_char(ch: char, out: &mut String, field_depth: &mut u32, in_field_result: &mut bool) {
    match ch {
        // Field markers: 0x13 begins a field, 0x14 separates its instruction from
        // the displayed result, 0x15 ends it. Drop the instruction bytes; keep the
        // result.
        '\u{13}' => {
            *field_depth += 1;
            *in_field_result = false;
        }
        '\u{14}' => {
            *in_field_result = true;
        }
        '\u{15}' => {
            if *field_depth > 0 {
                *field_depth -= 1;
            }
            *in_field_result = false;
        }
        _ if *field_depth > 0 && !*in_field_result => {
            // Inside a field instruction — skip.
        }
        '\r' | '\u{0B}' | '\u{0C}' => out.push('\n'), // para / line / page break
        '\u{07}' => out.push('\t'),                   // table cell / row mark
        '\u{1E}' => out.push('-'),                    // non-breaking hyphen
        '\u{1F}' => {}                                // optional hyphen — drop
        '\u{A0}' => out.push(' '),                    // non-breaking space
        '\t' | '\n' => out.push(ch),
        // Drop the remaining control chars (embedded object 0x01/0x08, footnote
        // 0x02, annotation, etc.); keep everything printable.
        c if (c as u32) < 0x20 => {}
        c => out.push(c),
    }
}

/// Decode a single piece's bytes into mapped output text.
#[allow(clippy::too_many_arguments)]
fn decode_piece(
    word_doc: &[u8],
    byte_off: usize,
    nchars: usize,
    compressed: bool,
    out: &mut String,
    field_depth: &mut u32,
    in_field_result: &mut bool,
) {
    if compressed {
        // 8-bit: one Windows-1252 byte per character.
        let end = byte_off.saturating_add(nchars).min(word_doc.len());
        for &b in &word_doc[byte_off.min(word_doc.len())..end] {
            if let Some(ch) = cp1252_char(b) {
                push_word_char(ch, out, field_depth, in_field_result);
            }
        }
    } else {
        // 16-bit: UTF-16LE, two bytes per character.
        let end = byte_off.saturating_add(nchars * 2).min(word_doc.len());
        let slice = &word_doc[byte_off.min(word_doc.len())..end];
        let units: Vec<u16> = slice
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        for ch in char::decode_utf16(units.into_iter()) {
            let ch = ch.unwrap_or('\u{FFFD}');
            push_word_char(ch, out, field_depth, in_field_result);
        }
    }
}

/// Walk the piece table (`PlcPcd`) inside the `CLX` and append every piece's
/// mapped text to `out`. Returns the number of pieces walked.
///
/// `clx` is the raw CLX bytes from the table stream. The `PlcPcd` we want is the
/// `Pcdt` entry (leading byte `0x02`): a `u32` byte length, then an array of
/// `(n+1)` character positions (`u32` each) followed by `n` 8-byte `PCD`s.
fn walk_piece_table(clx: &[u8], word_doc: &[u8], out: &mut String) -> usize {
    // Skip any leading Prc entries (leading byte 0x01): a u16 grpprl length then
    // that many bytes.
    let mut i = 0usize;
    while i < clx.len() && clx[i] == 0x01 {
        let cb = u16_at(clx, i + 1) as usize;
        i += 1 + 2 + cb;
    }
    // Expect the Pcdt marker.
    if i >= clx.len() || clx[i] != 0x02 {
        return 0;
    }
    i += 1;
    let lcb = u32_at(clx, i) as usize;
    i += 4;
    let plc = match clx.get(i..i + lcb) {
        Some(s) => s,
        None => return 0,
    };
    // PlcPcd: (n+1) CPs of 4 bytes + n PCDs of 8 bytes => lcb = 12n + 4.
    if lcb < 4 + 12 {
        return 0;
    }
    let n = (lcb - 4) / 12;
    let cps_len = 4 * (n + 1);
    let mut field_depth = 0u32;
    let mut in_field_result = false;
    for idx in 0..n {
        let cp_start = u32_at(plc, 4 * idx) as usize;
        let cp_end = u32_at(plc, 4 * (idx + 1)) as usize;
        if cp_end <= cp_start {
            continue;
        }
        let nchars = cp_end - cp_start;
        let pcd = cps_len + 8 * idx;
        // PCD: 2 bytes flags, then a 4-byte FcCompressed, then 2 bytes prm.
        let fc_raw = u32_at(plc, pcd + 2);
        let compressed = fc_raw & 0x4000_0000 != 0;
        let fc = (fc_raw & 0x3FFF_FFFF) as usize;
        let byte_off = if compressed { fc / 2 } else { fc };
        decode_piece(
            word_doc,
            byte_off,
            nchars,
            compressed,
            out,
            &mut field_depth,
            &mut in_field_result,
        );
        if out.chars().count() >= MAX_TEXT_CHARS {
            break;
        }
    }
    n
}

/// Apply the `clean` whitespace policy: trim trailing spaces on each line,
/// collapse 3+ consecutive newlines into a single blank line, and trim the outer
/// whitespace.
fn clean_whitespace(text: &str) -> String {
    let mut cleaned = String::with_capacity(text.len());
    let mut newline_run = 0usize;
    // Rebuild line-by-line so we can trim trailing spaces without a second pass.
    let trimmed_lines: String = text
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n");
    for ch in trimmed_lines.chars() {
        if ch == '\n' {
            newline_run += 1;
            // Allow at most two newlines in a row (one blank line).
            if newline_run <= 2 {
                cleaned.push('\n');
            }
        } else {
            newline_run = 0;
            cleaned.push(ch);
        }
    }
    cleaned.trim().to_string()
}

/// Extract plain text from the bytes of a legacy Word 97–2003 `.doc` file.
///
/// Returns `Err` when the bytes are empty, are not an OLE2 container, do not hold
/// a Word `WordDocument` stream, or do not begin with a valid Word FIB (e.g. a
/// non-Word OLE2 document such as an old `.xls`).
pub fn extract(bytes: &[u8], whitespace: Whitespace) -> Result<Extraction, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    if bytes.len() < 8 || bytes[..8] != OLE2_MAGIC {
        return Err(
            "not a legacy Microsoft Word .doc file — expected an OLE2/Compound File \
             container (a modern .docx is a ZIP; use docx-text-extract for that)"
                .into(),
        );
    }

    let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes))
        .map_err(|e| format!("not a valid OLE2 compound file: {e}"))?;

    // The WordDocument stream must exist for this to be a Word document.
    let mut word_doc = Vec::new();
    cfb.open_stream("WordDocument")
        .map_err(|_| {
            "OLE2 container has no WordDocument stream — not a Word .doc (could be an \
             Excel/PowerPoint or other OLE2 file)"
                .to_string()
        })?
        .read_to_end(&mut word_doc)
        .map_err(|e| format!("failed to read WordDocument stream: {e}"))?;

    if u16_at(&word_doc, 0) != FIB_IDENT {
        return Err("WordDocument stream does not start with a valid Word FIB".into());
    }

    // FIB fields (offsets from the Word Binary File Format spec):
    //   0x000A: flags word; bit 0x0200 = fWhichTblStm (1 ⇒ 1Table, 0 ⇒ 0Table)
    //   0x0018: fcMin, 0x001C: fcMac   (raw text range, fallback)
    //   0x01A2: fcClx,  0x01A6: lcbClx (piece table location in the table stream)
    let flags = u16_at(&word_doc, 0x000A);
    let table_name = if flags & 0x0200 != 0 { "1Table" } else { "0Table" };
    let fc_clx = u32_at(&word_doc, 0x01A2) as usize;
    let lcb_clx = u32_at(&word_doc, 0x01A6) as usize;

    let mut out = String::new();
    let mut pieces = 0usize;

    if lcb_clx > 0 {
        let mut table = Vec::new();
        cfb.open_stream(table_name)
            .map_err(|_| format!("Word .doc references a {table_name} stream that is missing"))?
            .read_to_end(&mut table)
            .map_err(|e| format!("failed to read {table_name} stream: {e}"))?;
        if let Some(clx) = table.get(fc_clx..fc_clx + lcb_clx) {
            pieces = walk_piece_table(clx, &word_doc, &mut out);
        }
    }

    if out.is_empty() {
        // Fallback: no usable piece table — read the raw fcMin..fcMac range from
        // the WordDocument stream as Windows-1252 (older/simple documents).
        let fc_min = u32_at(&word_doc, 0x0018) as usize;
        let fc_mac = u32_at(&word_doc, 0x001C) as usize;
        if fc_mac > fc_min {
            let end = fc_mac.min(word_doc.len());
            let mut field_depth = 0u32;
            let mut in_field_result = false;
            for &b in &word_doc[fc_min.min(word_doc.len())..end] {
                if let Some(ch) = cp1252_char(b) {
                    push_word_char(ch, &mut out, &mut field_depth, &mut in_field_result);
                }
            }
            pieces = 1;
        }
    }

    let mut truncated = false;
    if out.chars().count() > MAX_TEXT_CHARS {
        out = out.chars().take(MAX_TEXT_CHARS).collect();
        truncated = true;
    }

    let text = match whitespace {
        Whitespace::Clean => clean_whitespace(&out),
        Whitespace::Raw => out,
    };

    if text.trim().is_empty() {
        return Err(
            "no readable text found in the .doc (it may be empty, image-only, or \
             password-protected/encrypted)"
                .into(),
        );
    }

    let paragraphs = text.lines().filter(|l| !l.trim().is_empty()).count();

    Ok(Extraction {
        text,
        paragraphs,
        pieces,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a minimal but valid legacy `.doc` in memory: a WordDocument stream
    /// (FIB + raw text) and a 0Table stream (a one-piece CLX pointing at the
    /// text). `compressed` picks 8-bit Windows-1252 vs 16-bit UTF-16 storage.
    fn build_doc(text: &str, compressed: bool) -> Vec<u8> {
        // --- WordDocument stream: 0x200-byte FIB, then the raw character bytes.
        const FIB_LEN: usize = 0x200;
        let mut wd = vec![0u8; FIB_LEN];
        wd[0..2].copy_from_slice(&FIB_IDENT.to_le_bytes()); // wIdent
        // flags at 0x0A: leave fWhichTblStm clear ⇒ 0Table.
        let text_off = FIB_LEN;
        let nchars = text.chars().count() as u32;

        if compressed {
            for ch in text.chars() {
                // Test text stays in the Latin-1 range, so the byte is the scalar.
                wd.push(ch as u8);
            }
        } else {
            for u in text.encode_utf16() {
                wd.extend_from_slice(&u.to_le_bytes());
            }
        }

        // fcMin / fcMac (fallback range) — point at the text too.
        wd[0x18..0x1C].copy_from_slice(&(text_off as u32).to_le_bytes());
        let fc_mac = if compressed {
            text_off + text.len()
        } else {
            text_off + text.encode_utf16().count() * 2
        };
        wd[0x1C..0x20].copy_from_slice(&(fc_mac as u32).to_le_bytes());

        // --- 0Table stream: CLX = Pcdt (0x02) + u32 lcb + PlcPcd.
        // PlcPcd: 2 CPs (u32) + 1 PCD (8 bytes) => lcb = 12*1 + 4 = 16.
        let mut clx = Vec::new();
        clx.push(0x02u8);
        clx.extend_from_slice(&16u32.to_le_bytes());
        clx.extend_from_slice(&0u32.to_le_bytes()); // CP[0] = 0
        clx.extend_from_slice(&nchars.to_le_bytes()); // CP[1] = nchars
        // PCD: 2 flag bytes, FcCompressed (u32), 2 prm bytes.
        clx.extend_from_slice(&[0u8, 0u8]);
        let fc_field = if compressed {
            0x4000_0000u32 | ((text_off as u32) * 2)
        } else {
            text_off as u32
        };
        clx.extend_from_slice(&fc_field.to_le_bytes());
        clx.extend_from_slice(&[0u8, 0u8]);

        // fcClx = 0, lcbClx = clx.len() (CLX sits at the start of the table stream).
        wd[0x1A2..0x1A6].copy_from_slice(&0u32.to_le_bytes());
        wd[0x1A6..0x1AA].copy_from_slice(&(clx.len() as u32).to_le_bytes());

        // --- Assemble the CFB container with the two streams.
        let cursor = Cursor::new(Vec::new());
        let mut comp = cfb::CompoundFile::create(cursor).expect("create cfb");
        comp.create_stream("WordDocument")
            .unwrap()
            .write_all(&wd)
            .unwrap();
        comp.create_stream("0Table").unwrap().write_all(&clx).unwrap();
        comp.flush().unwrap();
        comp.into_inner().into_inner()
    }

    #[test]
    fn happy_extracts_compressed_text_exactly() {
        // A paragraph mark (\r) ends the line; the extractor maps it to \n and the
        // clean policy trims the trailing blank line.
        let doc = build_doc("Hello, legacy world!\r", true);
        let out = extract(&doc, Whitespace::Clean).unwrap();
        assert_eq!(out.text, "Hello, legacy world!");
        assert_eq!(out.paragraphs, 1);
        assert_eq!(out.pieces, 1);
        assert!(!out.truncated);
    }

    #[test]
    fn happy_extracts_utf16_multiline() {
        let doc = build_doc("First line\rSecond line\r", false);
        let out = extract(&doc, Whitespace::Clean).unwrap();
        assert_eq!(out.text, "First line\nSecond line");
        assert_eq!(out.paragraphs, 2);
    }

    #[test]
    fn field_instruction_dropped_result_kept() {
        // 0x13 HYPERLINK "http://x" 0x14 click here 0x15  →  "click here"
        let raw = "\u{13}HYPERLINK \"http://x\"\u{14}click here\u{15}\r";
        let doc = build_doc(raw, true);
        let out = extract(&doc, Whitespace::Clean).unwrap();
        assert_eq!(out.text, "click here");
    }

    #[test]
    fn clean_collapses_blank_runs_raw_keeps_them() {
        let doc = build_doc("A\r\r\r\rB\r", true);
        let clean = extract(&doc, Whitespace::Clean).unwrap();
        assert_eq!(clean.text, "A\n\nB");
        let raw = extract(&doc, Whitespace::Raw).unwrap();
        // Raw keeps every paragraph mark; trailing one included.
        assert_eq!(raw.text, "A\n\n\n\nB\n");
    }

    #[test]
    fn error_on_non_ole_bytes() {
        // A .docx (ZIP) or plain text is not an OLE2 container.
        let err = extract(b"PK\x03\x04 not a doc", Whitespace::Clean).unwrap_err();
        assert!(err.contains("not a legacy Microsoft Word .doc"), "got: {err}");
    }

    #[test]
    fn error_on_empty_input() {
        assert_eq!(extract(b"", Whitespace::Clean).unwrap_err(), "input is empty");
    }

    #[test]
    fn error_on_ole_without_worddocument() {
        // A valid OLE2 container but with no WordDocument stream (e.g. a non-Word
        // OLE2 file) must be rejected.
        let cursor = Cursor::new(Vec::new());
        let mut comp = cfb::CompoundFile::create(cursor).unwrap();
        comp.create_stream("Workbook").unwrap().write_all(b"xls").unwrap();
        comp.flush().unwrap();
        let bytes = comp.into_inner().into_inner();
        let err = extract(&bytes, Whitespace::Clean).unwrap_err();
        assert!(err.contains("no WordDocument stream"), "got: {err}");
    }

    #[test]
    fn whitespace_parse_maps_and_rejects() {
        assert_eq!(Whitespace::parse(None).unwrap(), Whitespace::Clean);
        assert_eq!(Whitespace::parse(Some("clean")).unwrap(), Whitespace::Clean);
        assert_eq!(Whitespace::parse(Some("raw")).unwrap(), Whitespace::Raw);
        assert!(Whitespace::parse(Some("nope")).is_err());
    }
}
