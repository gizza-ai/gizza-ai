//! gizza-ai/ppt-to-text core — extract readable plain text from a **legacy
//! binary PowerPoint `.ppt` (PowerPoint 97–2003)** presentation.
//!
//! Pure Rust with no wafer/wasm-bindgen deps, so it compiles natively for the
//! unit tests and to `wasm32-wasip1` (the block-wasm target) for the chat block.
//!
//! ## What a legacy `.ppt` is (and how this differs from `.pptx`)
//!
//! A PowerPoint 97–2003 `.ppt` is an **OLE2 / Compound File Binary (CFB)**
//! container (magic `D0 CF 11 E0 A1 B1 1A E1`), NOT a ZIP. Inside it the
//! `PowerPoint Document` stream holds a tree of **records**. Each record starts
//! with an 8-byte header:
//!
//! ```text
//!   2 bytes  recVerAndInstance  (low 4 bits = recVer; recVer 0xF ⇒ container)
//!   2 bytes  recType            (u16 LE)
//!   4 bytes  recLen             (u32 LE — length of the record body)
//! ```
//!
//! A container's body is more records; an atom's body is data. The text of a
//! slide (titles, bullets, text boxes, table cells) lives in two atom types
//! inside the slide's drawing:
//! - `TextCharsAtom` (`0x0FA0`) — the characters as **UTF-16LE**;
//! - `TextBytesAtom` (`0x0FA8`) — the characters as **single bytes** (each byte
//!   is the low byte of a Unicode scalar, i.e. Latin-1; PowerPoint only uses this
//!   form when every character fits in `U+0000..=U+00FF`).
//!
//! Speaker notes live in the same atom types but inside a `Notes` (`0x03F0`)
//! container instead of a `Slide` (`0x03EE`) container, so we can separate them.
//!
//! We open the container with the `cfb` crate and walk the record tree, tracking
//! the nearest `Slide`/`Notes` ancestor. Extracting only from `Slide`/`Notes`
//! subtrees deliberately skips the `DocumentContainer`'s `SlideListWithText`
//! outline, which stores a *second* copy of the same text and would otherwise
//! duplicate every line. This is the classic `catppt` / Apache-POI-HSLF path.
//!
//! ## Distinct from the other doc tools
//!
//! `docx-text-extract`/`document-text-extract` parse ZIP-based Office Open XML
//! (`.pptx`/`.docx`) and `doc-to-text` parses the binary Word `.doc`; none read
//! the binary PowerPoint `.ppt` record stream this block handles.

use std::io::{Cursor, Read};

/// OLE2 / Compound File Binary signature that begins every legacy `.ppt`.
const OLE2_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// `RT_Slide` container — a single presentation slide's records.
const RT_SLIDE: u16 = 0x03EE;
/// `RT_Notes` container — a slide's speaker-notes records.
const RT_NOTES: u16 = 0x03F0;
/// `TextCharsAtom` — text stored as UTF-16LE.
const TEXT_CHARS_ATOM: u16 = 0x0FA0;
/// `TextBytesAtom` — text stored as one Latin-1 byte per character.
const TEXT_BYTES_ATOM: u16 = 0x0FA8;
/// `recVer` value marking a record as a container (body = child records).
const REC_VER_CONTAINER: u16 = 0x0F;

/// Guard against pathologically deep record nesting.
const MAX_DEPTH: u32 = 100;
/// Cap on decoded output.
const MAX_TEXT_CHARS: usize = 1_000_000;

/// A successful extraction: the plain text plus a few structure counts the caller
/// surfaces in its JSON response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Extraction {
    /// The extracted plain text (control marks mapped; whitespace policy applied).
    pub text: String,
    /// Number of paragraphs (non-empty lines after mapping/normalisation).
    pub paragraphs: usize,
    /// Number of `Slide` containers found in the presentation.
    pub slides: usize,
    /// Number of text atoms (`TextCharsAtom`/`TextBytesAtom`) included in the output.
    pub text_runs: usize,
    /// True when the text was clipped to the [`MAX_TEXT_CHARS`] cap.
    pub truncated: bool,
}

/// How to post-process the extracted text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    /// Collapse 3+ consecutive newlines to a single blank line and trim trailing
    /// spaces / outer whitespace — the default "clean text" output.
    Clean,
    /// Return the mapped text as-is (no cosmetic collapsing).
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

/// Whether to include speaker notes in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Notes {
    /// Slide text followed by speaker notes (the default).
    Include,
    /// Slide text only; drop speaker notes.
    Exclude,
    /// Speaker notes only; drop slide text.
    Only,
}

impl Notes {
    /// Parse the `notes` param. Defaults to [`Notes::Include`]; rejects unknown values.
    pub fn parse(value: Option<&str>) -> Result<Self, String> {
        match value.unwrap_or("include") {
            "include" => Ok(Notes::Include),
            "exclude" => Ok(Notes::Exclude),
            "only" => Ok(Notes::Only),
            other => Err(format!(
                "invalid notes {other:?}: expected \"include\", \"exclude\" or \"only\""
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

/// Which kind of container a text atom sits inside — decides whether it is slide
/// body text or speaker-notes text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ctx {
    /// Not (yet) inside a Slide or Notes container.
    Outside,
    /// Inside a `Slide` container's subtree.
    Slide,
    /// Inside a `Notes` container's subtree.
    Notes,
}

/// Map one raw PowerPoint character into output text: `\r` (paragraph) and the
/// vertical tab `\x0B` (line break within a paragraph) both become `\n`; real
/// tabs/newlines pass through; other C0 control chars are dropped.
fn push_ppt_char(ch: char, out: &mut String) {
    match ch {
        '\r' | '\u{000B}' => out.push('\n'),
        '\t' | '\n' => out.push(ch),
        c if (c as u32) < 0x20 => {}
        c => out.push(c),
    }
}

/// Accumulator for a record-tree walk.
struct Walker {
    slide_runs: Vec<String>,
    notes_runs: Vec<String>,
    slides: usize,
    /// When true, extract text from every text atom regardless of context (used
    /// by the fallback pass for files that store no per-slide drawings).
    force: bool,
}

impl Walker {
    /// Decode a `TextCharsAtom` body (UTF-16LE) into a mapped run string.
    fn decode_chars(body: &[u8]) -> String {
        let units: Vec<u16> = body
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .collect();
        let mut run = String::with_capacity(units.len());
        for ch in char::decode_utf16(units) {
            push_ppt_char(ch.unwrap_or('\u{FFFD}'), &mut run);
        }
        run
    }

    /// Decode a `TextBytesAtom` body (one Latin-1 byte per char) into a run.
    fn decode_bytes(body: &[u8]) -> String {
        let mut run = String::with_capacity(body.len());
        for &b in body {
            push_ppt_char(b as char, &mut run);
        }
        run
    }

    /// Route a decoded run into the slide or notes bucket per the current context.
    fn record_run(&mut self, ctx: Ctx, run: String) {
        if run.is_empty() {
            return;
        }
        match ctx {
            Ctx::Notes => self.notes_runs.push(run),
            Ctx::Slide => self.slide_runs.push(run),
            Ctx::Outside => {
                if self.force {
                    self.slide_runs.push(run);
                }
            }
        }
    }

    /// Recursively walk a slice of records, tracking the nearest Slide/Notes
    /// ancestor.
    fn walk(&mut self, data: &[u8], ctx: Ctx, depth: u32) {
        if depth > MAX_DEPTH {
            return;
        }
        let mut i = 0usize;
        while i + 8 <= data.len() {
            let rec_ver = u16_at(data, i) & 0x000F;
            let rec_type = u16_at(data, i + 2);
            let rec_len = u32_at(data, i + 4) as usize;
            let body_start = i + 8;
            let body_end = body_start.saturating_add(rec_len).min(data.len());
            let body = &data[body_start..body_end];

            if rec_ver == REC_VER_CONTAINER {
                let child_ctx = match rec_type {
                    RT_SLIDE => {
                        self.slides += 1;
                        Ctx::Slide
                    }
                    RT_NOTES => Ctx::Notes,
                    _ => ctx,
                };
                self.walk(body, child_ctx, depth + 1);
            } else {
                match rec_type {
                    TEXT_CHARS_ATOM => {
                        let run = Self::decode_chars(body);
                        self.record_run(ctx, run);
                    }
                    TEXT_BYTES_ATOM => {
                        let run = Self::decode_bytes(body);
                        self.record_run(ctx, run);
                    }
                    _ => {}
                }
            }

            // Advance past the whole record. `saturating_add` past `data.len()`
            // ends the loop; `body_start` always advances i by ≥8 so a
            // zero-length record can't spin.
            i = body_start.saturating_add(rec_len);
        }
    }
}

/// Apply the `clean` whitespace policy: trim trailing spaces on each line,
/// collapse 3+ consecutive newlines into a single blank line, and trim the outer
/// whitespace.
fn clean_whitespace(text: &str) -> String {
    let trimmed_lines: String = text
        .split('\n')
        .map(|line| line.trim_end_matches([' ', '\t']))
        .collect::<Vec<_>>()
        .join("\n");
    let mut cleaned = String::with_capacity(trimmed_lines.len());
    let mut newline_run = 0usize;
    for ch in trimmed_lines.chars() {
        if ch == '\n' {
            newline_run += 1;
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

/// Extract plain text from the bytes of a legacy PowerPoint 97–2003 `.ppt` file.
///
/// Returns `Err` when the bytes are empty, are not an OLE2 container, do not hold
/// a `PowerPoint Document` stream, or contain no readable text.
pub fn extract(bytes: &[u8], whitespace: Whitespace, notes: Notes) -> Result<Extraction, String> {
    if bytes.is_empty() {
        return Err("input is empty".into());
    }
    if bytes.len() < 8 || bytes[..8] != OLE2_MAGIC {
        return Err(
            "not a legacy PowerPoint .ppt file — expected an OLE2/Compound File \
             container (a modern .pptx is a ZIP; this tool reads the binary 97-2003 .ppt)"
                .into(),
        );
    }

    let mut cfb = cfb::CompoundFile::open(Cursor::new(bytes))
        .map_err(|e| format!("not a valid OLE2 compound file: {e}"))?;

    // The `PowerPoint Document` stream must exist for this to be a .ppt.
    let mut doc = Vec::new();
    cfb.open_stream("PowerPoint Document")
        .map_err(|_| {
            "OLE2 container has no \"PowerPoint Document\" stream — not a PowerPoint .ppt \
             (could be a Word/Excel or other OLE2 file)"
                .to_string()
        })?
        .read_to_end(&mut doc)
        .map_err(|e| format!("failed to read PowerPoint Document stream: {e}"))?;

    // Primary pass: text only from Slide/Notes subtrees (skips the duplicate
    // SlideListWithText outline in the DocumentContainer).
    let mut w = Walker {
        slide_runs: Vec::new(),
        notes_runs: Vec::new(),
        slides: 0,
        force: false,
    };
    w.walk(&doc, Ctx::Outside, 0);

    // Fallback: some minimal files store text only in the outline. If we found
    // nothing in Slide/Notes drawings, re-walk collecting every text atom.
    if w.slide_runs.is_empty() && w.notes_runs.is_empty() {
        w.force = true;
        w.walk(&doc, Ctx::Outside, 0);
    }

    let slides = w.slides;
    let (mut runs, text_runs) = match notes {
        Notes::Include => {
            let mut r = w.slide_runs;
            let n = w.notes_runs.len();
            let s = r.len();
            r.extend(w.notes_runs);
            (r, s + n)
        }
        Notes::Exclude => {
            let s = w.slide_runs.len();
            (w.slide_runs, s)
        }
        Notes::Only => {
            let n = w.notes_runs.len();
            (w.notes_runs, n)
        }
    };

    let mut out = runs.join("\n");
    runs.clear();

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
        return Err(match notes {
            Notes::Only => "no speaker notes found in the .ppt".into(),
            _ => "no readable text found in the .ppt (it may be empty, image-only, or \
                  password-protected/encrypted)"
                .to_string(),
        });
    }

    let paragraphs = text.lines().filter(|l| !l.trim().is_empty()).count();

    Ok(Extraction {
        text,
        paragraphs,
        slides,
        text_runs,
        truncated,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Build a record header + body: `rec_ver` low nibble (0xF for a container),
    /// `rec_type`, u32 length, then the body bytes.
    fn record(rec_ver: u16, rec_type: u16, body: &[u8]) -> Vec<u8> {
        let mut v = Vec::new();
        v.extend_from_slice(&rec_ver.to_le_bytes());
        v.extend_from_slice(&rec_type.to_le_bytes());
        v.extend_from_slice(&(body.len() as u32).to_le_bytes());
        v.extend_from_slice(body);
        v
    }

    fn container(rec_type: u16, children: &[u8]) -> Vec<u8> {
        record(REC_VER_CONTAINER, rec_type, children)
    }

    fn text_bytes_atom(s: &str) -> Vec<u8> {
        let body: Vec<u8> = s.chars().map(|c| c as u8).collect();
        record(0, TEXT_BYTES_ATOM, &body)
    }

    fn text_chars_atom(s: &str) -> Vec<u8> {
        let mut body = Vec::new();
        for u in s.encode_utf16() {
            body.extend_from_slice(&u.to_le_bytes());
        }
        record(0, TEXT_CHARS_ATOM, &body)
    }

    /// Wrap a `PowerPoint Document` stream body into a real CFB container.
    fn build_ppt(doc_stream: &[u8]) -> Vec<u8> {
        let cursor = Cursor::new(Vec::new());
        let mut comp = cfb::CompoundFile::create(cursor).expect("create cfb");
        comp.create_stream("PowerPoint Document")
            .unwrap()
            .write_all(doc_stream)
            .unwrap();
        comp.flush().unwrap();
        comp.into_inner().into_inner()
    }

    /// A slide (bytes atom) + a notes container (chars atom).
    fn sample_stream() -> Vec<u8> {
        let mut slide_children = Vec::new();
        slide_children.extend(text_bytes_atom("Title\rBullet one"));
        let slide = container(RT_SLIDE, &slide_children);

        let mut notes_children = Vec::new();
        notes_children.extend(text_chars_atom("A speaker note"));
        let notes = container(RT_NOTES, &notes_children);

        let mut stream = Vec::new();
        stream.extend(slide);
        stream.extend(notes);
        stream
    }

    #[test]
    fn happy_extracts_slide_and_notes() {
        let ppt = build_ppt(&sample_stream());
        let out = extract(&ppt, Whitespace::Clean, Notes::Include).unwrap();
        assert_eq!(out.text, "Title\nBullet one\nA speaker note");
        assert_eq!(out.slides, 1);
        assert_eq!(out.paragraphs, 3);
        assert_eq!(out.text_runs, 2);
        assert!(!out.truncated);
    }

    #[test]
    fn notes_exclude_drops_notes() {
        let ppt = build_ppt(&sample_stream());
        let out = extract(&ppt, Whitespace::Clean, Notes::Exclude).unwrap();
        assert_eq!(out.text, "Title\nBullet one");
        assert_eq!(out.text_runs, 1);
    }

    #[test]
    fn notes_only_keeps_notes() {
        let ppt = build_ppt(&sample_stream());
        let out = extract(&ppt, Whitespace::Clean, Notes::Only).unwrap();
        assert_eq!(out.text, "A speaker note");
        assert_eq!(out.text_runs, 1);
    }

    #[test]
    fn utf16_chars_atom_decodes_unicode() {
        // A curly apostrophe (U+2019) forces the UTF-16 TextCharsAtom form.
        let mut slide_children = Vec::new();
        slide_children.extend(text_chars_atom("It\u{2019}s here"));
        let stream = container(RT_SLIDE, &slide_children);
        let ppt = build_ppt(&stream);
        let out = extract(&ppt, Whitespace::Clean, Notes::Include).unwrap();
        assert_eq!(out.text, "It\u{2019}s here");
    }

    #[test]
    fn slide_list_outline_is_not_duplicated() {
        // Text also present in a DocumentContainer/SlideListWithText outline must
        // NOT be emitted a second time: only Slide/Notes subtrees count.
        const RT_DOCUMENT: u16 = 0x03E8;
        const RT_SLIDE_LIST_WITH_TEXT: u16 = 0x0FF0;
        let outline = container(
            RT_DOCUMENT,
            &container(RT_SLIDE_LIST_WITH_TEXT, &text_bytes_atom("Title")),
        );
        let slide = container(RT_SLIDE, &text_bytes_atom("Title"));
        let mut stream = Vec::new();
        stream.extend(outline);
        stream.extend(slide);
        let ppt = build_ppt(&stream);
        let out = extract(&ppt, Whitespace::Clean, Notes::Include).unwrap();
        assert_eq!(out.text, "Title"); // once, not twice
        assert_eq!(out.text_runs, 1);
    }

    #[test]
    fn fallback_extracts_orphan_text_atoms() {
        // No Slide/Notes container at all — the fallback pass still recovers text.
        let stream = text_bytes_atom("Loose text");
        let ppt = build_ppt(&stream);
        let out = extract(&ppt, Whitespace::Clean, Notes::Include).unwrap();
        assert_eq!(out.text, "Loose text");
        assert_eq!(out.slides, 0);
    }

    #[test]
    fn clean_collapses_blank_runs_raw_keeps_them() {
        let stream = container(RT_SLIDE, &text_bytes_atom("A\r\r\r\rB"));
        let ppt = build_ppt(&stream);
        let clean = extract(&ppt, Whitespace::Clean, Notes::Include).unwrap();
        assert_eq!(clean.text, "A\n\nB");
        let raw = extract(&ppt, Whitespace::Raw, Notes::Include).unwrap();
        assert_eq!(raw.text, "A\n\n\n\nB");
    }

    #[test]
    fn error_on_non_ole_bytes() {
        let err = extract(b"PK\x03\x04 not a ppt", Whitespace::Clean, Notes::Include).unwrap_err();
        assert!(err.contains("not a legacy PowerPoint .ppt"), "got: {err}");
    }

    #[test]
    fn error_on_empty_input() {
        assert_eq!(
            extract(b"", Whitespace::Clean, Notes::Include).unwrap_err(),
            "input is empty"
        );
    }

    #[test]
    fn error_on_ole_without_powerpoint_stream() {
        let cursor = Cursor::new(Vec::new());
        let mut comp = cfb::CompoundFile::create(cursor).unwrap();
        comp.create_stream("WordDocument")
            .unwrap()
            .write_all(b"doc")
            .unwrap();
        comp.flush().unwrap();
        let bytes = comp.into_inner().into_inner();
        let err = extract(&bytes, Whitespace::Clean, Notes::Include).unwrap_err();
        assert!(err.contains("no \"PowerPoint Document\" stream"), "got: {err}");
    }

    #[test]
    fn notes_only_errors_when_absent() {
        let stream = container(RT_SLIDE, &text_bytes_atom("Slide only"));
        let ppt = build_ppt(&stream);
        let err = extract(&ppt, Whitespace::Clean, Notes::Only).unwrap_err();
        assert!(err.contains("no speaker notes"), "got: {err}");
    }

    #[test]
    fn whitespace_and_notes_parse_maps_and_rejects() {
        assert_eq!(Whitespace::parse(None).unwrap(), Whitespace::Clean);
        assert_eq!(Whitespace::parse(Some("raw")).unwrap(), Whitespace::Raw);
        assert!(Whitespace::parse(Some("nope")).is_err());
        assert_eq!(Notes::parse(None).unwrap(), Notes::Include);
        assert_eq!(Notes::parse(Some("exclude")).unwrap(), Notes::Exclude);
        assert_eq!(Notes::parse(Some("only")).unwrap(), Notes::Only);
        assert!(Notes::parse(Some("nope")).is_err());
    }
}
