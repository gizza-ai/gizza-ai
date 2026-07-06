//! gizza-ai/docx-to-pdf core — convert a Word `.docx` document into a paginated
//! PDF.
//!
//! Pure-Rust: a `.docx` is a ZIP whose `word/document.xml` part is
//! WordprocessingML. `zip` opens the container, `quick-xml` streams the body,
//! and `lopdf` writes the PDF using the built-in base-14 fonts (Helvetica
//! family) so the output stays tiny and deterministic — no font embedding.
//!
//! What is carried across: paragraphs, heading/title styles (scaled + bold),
//! bold / italic run formatting, explicit run font sizes (`w:sz`, half-points),
//! paragraph alignment (left / center / right; justify renders left-aligned),
//! bullet/number list items (rendered with a bullet marker, indented by level),
//! hard line breaks (`<w:br/>`), explicit page breaks (`<w:br w:type="page"/>`),
//! and tables (flattened to readable pipe-separated rows with a header rule).
//! Content flows across as many US-Letter (or A4) pages as needed.
//!
//! Out of scope (stated on the tool): embedded images, footnotes/headers/
//! footers, and exact WYSIWYG line breaking — this is a lightweight structural
//! converter, not a full Word layout engine.

use std::io::{Cursor, Read};

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};
use quick_xml::events::Event;
use quick_xml::Reader;

// US Letter, the default page size (points).
const LETTER_W: f64 = 612.0;
const LETTER_H: f64 = 792.0;
// ISO A4 in points (210 x 297 mm).
const A4_W: f64 = 595.0;
const A4_H: f64 = 842.0;
const LINE_FACTOR: f64 = 1.35;
const PARA_GAP: f64 = 0.5; // extra blank space (in line-heights) after a block
const INDENT_PT: f64 = 18.0; // per list-nesting level

// ---------------------------------------------------------------------------
// Fonts
// ---------------------------------------------------------------------------

/// The base-14 fonts we map onto. Index = resource name F1..F4.
#[derive(Clone, Copy, PartialEq, Debug)]
enum FontKind {
    Regular,    // Helvetica
    Bold,       // Helvetica-Bold
    Italic,     // Helvetica-Oblique
    BoldItalic, // Helvetica-BoldOblique
}

impl FontKind {
    fn resource(self) -> &'static str {
        match self {
            FontKind::Regular => "F1",
            FontKind::Bold => "F2",
            FontKind::Italic => "F3",
            FontKind::BoldItalic => "F4",
        }
    }
    fn base_font(self) -> &'static str {
        match self {
            FontKind::Regular => "Helvetica",
            FontKind::Bold => "Helvetica-Bold",
            FontKind::Italic => "Helvetica-Oblique",
            FontKind::BoldItalic => "Helvetica-BoldOblique",
        }
    }
    /// Average glyph advance as a fraction of the em (for word-wrap width
    /// estimation). The Helvetica family averages ~0.5.
    fn char_em(self) -> f64 {
        0.5
    }
    fn from_flags(bold: bool, italic: bool) -> FontKind {
        match (bold, italic) {
            (true, true) => FontKind::BoldItalic,
            (true, false) => FontKind::Bold,
            (false, true) => FontKind::Italic,
            (false, false) => FontKind::Regular,
        }
    }
}

// ---------------------------------------------------------------------------
// Parsed document model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Debug)]
enum Align {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum ParaStyle {
    Normal,
    Title,
    Heading(u8),
}

/// A run of text with resolved formatting flags. An explicit `w:sz` wins;
/// otherwise the layout stage supplies the base/heading size.
#[derive(Clone, Debug, PartialEq)]
struct SRun {
    text: String,
    bold: bool,
    italic: bool,
    sz_pt: Option<f64>,
}

/// A parsed paragraph. `segments` are separated by hard line breaks
/// (`<w:br/>`); each segment is independently word-wrapped.
#[derive(Clone, Debug, PartialEq)]
struct Para {
    segments: Vec<Vec<SRun>>,
    style: ParaStyle,
    align: Align,
    list_level: Option<u32>,
}

#[derive(Clone, Debug, PartialEq)]
enum Block {
    Para(Para),
    /// Rows of cells; each cell is the flattened run list of its paragraphs.
    Table(Vec<Vec<Vec<SRun>>>),
    PageBreak,
}

// ---------------------------------------------------------------------------
// DOCX parsing
// ---------------------------------------------------------------------------

/// Local element name with any `ns:` prefix stripped (WordprocessingML tags are
/// namespaced, e.g. `w:t`, `w:p`).
fn local_name(raw: &[u8]) -> &[u8] {
    match raw.iter().position(|&b| b == b':') {
        Some(i) => &raw[i + 1..],
        None => raw,
    }
}

/// Value of an attribute selected by its local name.
fn local_attr(e: &quick_xml::events::BytesStart, key: &[u8]) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if local_name(a.key.as_ref()) == key {
            Some(String::from_utf8_lossy(&a.value).into_owned())
        } else {
            None
        }
    })
}

fn parse_style(val: &str) -> ParaStyle {
    let v = val.to_ascii_lowercase().replace(' ', "");
    if v == "title" {
        return ParaStyle::Title;
    }
    if let Some(rest) = v.strip_prefix("heading") {
        if let Ok(n) = rest.parse::<u8>() {
            return ParaStyle::Heading(n.clamp(1, 6));
        }
    }
    ParaStyle::Normal
}

fn parse_align(val: &str) -> Align {
    match val.to_ascii_lowercase().as_str() {
        "center" => Align::Center,
        "right" | "end" => Align::Right,
        "both" | "distribute" | "justify" => Align::Justify,
        _ => Align::Left,
    }
}

/// A `<w:b/>` / `<w:i/>` toggle is ON unless it carries `w:val="false|0|none|off"`.
fn toggle_on(e: &quick_xml::events::BytesStart) -> bool {
    !matches!(
        local_attr(e, b"val").as_deref(),
        Some("false") | Some("0") | Some("none") | Some("off")
    )
}

/// State machine that turns a WordprocessingML `document.xml` into `Block`s.
struct Parser {
    blocks: Vec<Block>,

    // Current paragraph builder (document level — not inside a table).
    cur_segments: Vec<Vec<SRun>>,
    cur_runs: Vec<SRun>,
    cur_style: ParaStyle,
    cur_align: Align,
    cur_list: Option<u32>,

    // Current run formatting.
    run_bold: bool,
    run_italic: bool,
    run_sz: Option<f64>,
    in_run: bool,
    in_text: bool,

    // pPr / numPr context.
    in_ppr: bool,
    in_numpr: bool,

    // Table context (single-level; nested tables are not modelled).
    in_table: bool,
    cur_table: Vec<Vec<Vec<SRun>>>,
    cur_row: Vec<Vec<SRun>>,
    cur_cell: Vec<SRun>,
    cell_has_content: bool,
}

impl Parser {
    fn new() -> Parser {
        Parser {
            blocks: Vec::new(),
            cur_segments: Vec::new(),
            cur_runs: Vec::new(),
            cur_style: ParaStyle::Normal,
            cur_align: Align::Left,
            cur_list: None,
            run_bold: false,
            run_italic: false,
            run_sz: None,
            in_run: false,
            in_text: false,
            in_ppr: false,
            in_numpr: false,
            in_table: false,
            cur_table: Vec::new(),
            cur_row: Vec::new(),
            cur_cell: Vec::new(),
            cell_has_content: false,
        }
    }

    fn reset_para(&mut self) {
        self.cur_segments.clear();
        self.cur_runs.clear();
        self.cur_style = ParaStyle::Normal;
        self.cur_align = Align::Left;
        self.cur_list = None;
    }

    /// Route a chunk of text (a `<w:t>` fragment or a tab) to the current table
    /// cell or the document paragraph, tagged with the active run formatting.
    fn push_text(&mut self, text: String) {
        if text.is_empty() {
            return;
        }
        let run = SRun {
            text,
            bold: self.run_bold,
            italic: self.run_italic,
            sz_pt: self.run_sz,
        };
        if self.in_table {
            self.cur_cell.push(run);
            self.cell_has_content = true;
        } else {
            self.cur_runs.push(run);
        }
    }

    /// End the current wrap segment (a hard line break inside a paragraph).
    fn hard_break(&mut self) {
        if self.in_table {
            self.cur_cell.push(SRun { text: " ".into(), bold: false, italic: false, sz_pt: None });
        } else {
            self.cur_segments.push(std::mem::take(&mut self.cur_runs));
        }
    }

    /// Emit the accumulated document paragraph as a `Block::Para`.
    fn flush_para(&mut self) {
        if !self.cur_runs.is_empty() {
            self.cur_segments.push(std::mem::take(&mut self.cur_runs));
        }
        let has_text = self.cur_segments.iter().flatten().any(|r| !r.text.trim().is_empty());
        let para = Para {
            segments: std::mem::take(&mut self.cur_segments),
            style: self.cur_style,
            align: self.cur_align,
            list_level: self.cur_list,
        };
        if has_text || para.list_level.is_some() {
            self.blocks.push(Block::Para(para));
        } else {
            // Blank paragraph → a vertical gap.
            self.blocks.push(Block::Para(Para {
                segments: vec![vec![]],
                style: ParaStyle::Normal,
                align: Align::Left,
                list_level: None,
            }));
        }
        self.reset_para();
    }

    fn on_start(&mut self, e: &quick_xml::events::BytesStart) {
        match local_name(e.name().as_ref()) {
            b"p" if !self.in_table => self.reset_para(),
            b"pPr" if !self.in_table => self.in_ppr = true,
            b"numPr" if !self.in_table => self.in_numpr = true,
            b"r" => {
                self.in_run = true;
                self.run_bold = false;
                self.run_italic = false;
                self.run_sz = None;
            }
            b"b" if self.in_run => self.run_bold = toggle_on(e),
            b"i" if self.in_run => self.run_italic = toggle_on(e),
            b"t" if self.in_run => self.in_text = true,
            b"tbl" => {
                if !self.cur_runs.is_empty() || !self.cur_segments.is_empty() {
                    self.flush_para();
                }
                self.in_table = true;
                self.cur_table.clear();
            }
            b"tr" if self.in_table => self.cur_row.clear(),
            b"tc" if self.in_table => {
                self.cur_cell.clear();
                self.cell_has_content = false;
            }
            _ => {}
        }
    }

    fn on_empty(&mut self, e: &quick_xml::events::BytesStart) {
        match local_name(e.name().as_ref()) {
            b"pStyle" if self.in_ppr => {
                if let Some(v) = local_attr(e, b"val") {
                    self.cur_style = parse_style(&v);
                }
            }
            b"jc" if self.in_ppr => {
                if let Some(v) = local_attr(e, b"val") {
                    self.cur_align = parse_align(&v);
                }
            }
            b"ilvl" if self.in_numpr => {
                let lvl = local_attr(e, b"val").and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
                self.cur_list = Some(lvl);
            }
            b"numId" if self.in_numpr => {
                if self.cur_list.is_none() {
                    self.cur_list = Some(0);
                }
            }
            b"b" if self.in_run => self.run_bold = toggle_on(e),
            b"i" if self.in_run => self.run_italic = toggle_on(e),
            b"sz" if self.in_run => {
                // w:sz is in half-points.
                if let Some(hp) = local_attr(e, b"val").and_then(|v| v.parse::<f64>().ok()) {
                    self.run_sz = Some(hp / 2.0);
                }
            }
            b"tab" if self.in_run => self.push_text("    ".into()),
            b"br" if self.in_run => {
                if local_attr(e, b"type").as_deref() == Some("page") {
                    if !self.in_table {
                        self.flush_para();
                    }
                    self.blocks.push(Block::PageBreak);
                } else {
                    self.hard_break();
                }
            }
            _ => {}
        }
    }

    fn on_end(&mut self, e: &quick_xml::events::BytesEnd) {
        match local_name(e.name().as_ref()) {
            b"t" => self.in_text = false,
            b"r" => self.in_run = false,
            b"pPr" => self.in_ppr = false,
            b"numPr" => self.in_numpr = false,
            b"p" => {
                if self.in_table {
                    if self.cell_has_content {
                        self.cur_cell.push(SRun { text: " ".into(), bold: false, italic: false, sz_pt: None });
                    }
                } else {
                    self.flush_para();
                }
            }
            b"tc" if self.in_table => {
                while matches!(self.cur_cell.last(), Some(r) if r.text == " ") {
                    self.cur_cell.pop();
                }
                self.cur_row.push(std::mem::take(&mut self.cur_cell));
                self.cell_has_content = false;
            }
            b"tr" if self.in_table => self.cur_table.push(std::mem::take(&mut self.cur_row)),
            b"tbl" => {
                let table = std::mem::take(&mut self.cur_table);
                if !table.is_empty() {
                    self.blocks.push(Block::Table(table));
                }
                self.in_table = false;
            }
            _ => {}
        }
    }
}

/// Parse a WordprocessingML `document.xml` body into structured blocks.
fn parse_document(xml: &[u8]) -> Result<Vec<Block>, String> {
    let mut reader = Reader::from_reader(xml);
    let mut buf = Vec::new();
    let mut p = Parser::new();
    loop {
        match reader
            .read_event_into(&mut buf)
            .map_err(|e| format!("malformed DOCX XML: {e}"))?
        {
            Event::Start(e) => p.on_start(&e),
            Event::Empty(e) => p.on_empty(&e),
            Event::End(e) => p.on_end(&e),
            Event::Text(t) if p.in_text => {
                let decoded = t.decode().map_err(|e| format!("failed to decode DOCX text: {e}"))?;
                p.push_text(decoded.into_owned());
            }
            // quick-xml 0.40 emits entity/char references as separate events.
            Event::GeneralRef(r) if p.in_text => {
                let ch = if let Some(c) = r
                    .resolve_char_ref()
                    .map_err(|e| format!("bad character reference in DOCX: {e}"))?
                {
                    Some(c)
                } else {
                    let name = r.decode().map_err(|e| format!("bad entity reference in DOCX: {e}"))?;
                    match name.as_ref() {
                        "amp" => Some('&'),
                        "lt" => Some('<'),
                        "gt" => Some('>'),
                        "quot" => Some('"'),
                        "apos" => Some('\''),
                        _ => None,
                    }
                };
                if let Some(c) = ch {
                    p.push_text(c.to_string());
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buf.clear();
    }
    // Trim trailing blank paragraphs (a DOCX body usually ends with one).
    while matches!(p.blocks.last(), Some(Block::Para(pp)) if pp.list_level.is_none()
        && pp.segments.iter().flatten().all(|r| r.text.trim().is_empty()))
    {
        p.blocks.pop();
    }
    Ok(p.blocks)
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

/// A run of text in a laid-out line (font + resolved size).
#[derive(Clone, Debug)]
struct LRun {
    text: String,
    font: FontKind,
    size: f64,
}

/// A laid-out line: runs plus left indent (points), height, and alignment.
#[derive(Clone, Debug)]
struct Line {
    runs: Vec<LRun>,
    indent: f64,
    height: f64,
    align: Align,
}

/// One item in the pagination stream.
enum Item {
    Line(Line),
    PageBreak,
}

fn heading_scale(style: ParaStyle) -> f64 {
    match style {
        ParaStyle::Title => 2.2,
        ParaStyle::Heading(1) => 2.0,
        ParaStyle::Heading(2) => 1.6,
        ParaStyle::Heading(3) => 1.3,
        ParaStyle::Heading(4) => 1.15,
        ParaStyle::Heading(5) => 1.0,
        ParaStyle::Heading(_) => 0.9,
        ParaStyle::Normal => 1.0,
    }
}

fn resolve_size(run: &SRun, style: ParaStyle, base: f64) -> f64 {
    match run.sz_pt {
        Some(sz) => sz.clamp(4.0, 96.0),
        None => base * heading_scale(style),
    }
}

fn resolve_bold(run: &SRun, style: ParaStyle) -> bool {
    run.bold || matches!(style, ParaStyle::Title | ParaStyle::Heading(_))
}

fn blank_line(base: f64) -> Line {
    Line { runs: Vec::new(), indent: 0.0, height: base * LINE_FACTOR * PARA_GAP, align: Align::Left }
}

fn rule_line(base: f64, text_w: f64) -> Line {
    let n = (text_w / (base * 0.5)).floor().max(1.0) as usize;
    Line {
        runs: vec![LRun { text: "_".repeat(n), font: FontKind::Regular, size: base * 0.8 }],
        indent: 0.0,
        height: base * LINE_FACTOR,
        align: Align::Left,
    }
}

/// Width of a laid-out run in points (average-advance estimate).
fn run_width(r: &LRun) -> f64 {
    r.text.chars().count() as f64 * r.size * r.font.char_em()
}

/// Word-wrap a run stream into lines fitting `avail` points wide, at `indent`.
fn wrap(buf: &[LRun], indent: f64, avail: f64, align: Align, out: &mut Vec<Item>) {
    struct Word {
        text: String,
        font: FontKind,
        size: f64,
    }
    let mut words: Vec<Word> = Vec::new();
    for r in buf {
        let mut first = true;
        let only_spaces = !r.text.is_empty() && r.text.chars().all(|c| c == ' ');
        for part in r.text.split(' ') {
            if part.is_empty() {
                first = false;
                continue;
            }
            let text = if first { part.to_string() } else { format!(" {part}") };
            words.push(Word { text, font: r.font, size: r.size });
            first = false;
        }
        if only_spaces {
            words.push(Word { text: " ".into(), font: r.font, size: r.size });
        }
    }

    let word_w = |w: &Word| w.text.chars().count() as f64 * w.size * w.font.char_em();

    let mut cur: Vec<LRun> = Vec::new();
    let mut cur_w = 0.0;
    let mut max_size = 0.0_f64;

    let push_line = |cur: &mut Vec<LRun>, max_size: &mut f64, out: &mut Vec<Item>| {
        if cur.is_empty() {
            return;
        }
        let height = (*max_size).max(1.0) * LINE_FACTOR;
        out.push(Item::Line(Line { runs: std::mem::take(cur), indent, height, align }));
        *max_size = 0.0;
    };

    for w in &words {
        let ww = word_w(w);
        if cur_w + ww > avail && !cur.is_empty() {
            push_line(&mut cur, &mut max_size, out);
            cur_w = 0.0;
            let trimmed = w.text.trim_start().to_string();
            let tw = trimmed.chars().count() as f64 * w.size * w.font.char_em();
            cur.push(LRun { text: trimmed, font: w.font, size: w.size });
            cur_w += tw;
            max_size = max_size.max(w.size);
            continue;
        }
        cur.push(LRun { text: w.text.clone(), font: w.font, size: w.size });
        cur_w += ww;
        max_size = max_size.max(w.size);
    }
    push_line(&mut cur, &mut max_size, out);
}

/// Flatten a cell's runs into laid-out runs at (mostly) the base size.
fn cell_runs(cell: &[SRun], base: f64) -> Vec<LRun> {
    cell.iter()
        .filter(|r| !r.text.is_empty())
        .map(|r| LRun {
            text: r.text.clone(),
            font: FontKind::from_flags(r.bold, r.italic),
            size: r.sz_pt.unwrap_or(base).clamp(4.0, 96.0),
        })
        .collect()
}

/// Build the pagination item stream from parsed blocks.
fn layout(blocks: &[Block], base: f64, text_w: f64) -> Vec<Item> {
    let mut out: Vec<Item> = Vec::new();

    for block in blocks {
        match block {
            Block::PageBreak => out.push(Item::PageBreak),
            Block::Para(para) => {
                let list_indent =
                    para.list_level.map(|l| (l as f64 + 1.0) * INDENT_PT).unwrap_or(0.0);
                let avail = (text_w - list_indent).max(base * 4.0);

                let mut first_segment = true;
                for seg in &para.segments {
                    let mut lruns: Vec<LRun> = Vec::new();
                    if first_segment && para.list_level.is_some() {
                        lruns.push(LRun { text: "\u{2022}  ".into(), font: FontKind::Regular, size: base });
                    }
                    for r in seg {
                        if r.text.is_empty() {
                            continue;
                        }
                        lruns.push(LRun {
                            text: r.text.clone(),
                            font: FontKind::from_flags(resolve_bold(r, para.style), r.italic),
                            size: resolve_size(r, para.style, base),
                        });
                    }
                    if lruns.is_empty() {
                        out.push(Item::Line(blank_line(base)));
                    } else {
                        wrap(&lruns, list_indent, avail, para.align, &mut out);
                    }
                    first_segment = false;
                }
                out.push(Item::Line(blank_line(base)));
            }
            Block::Table(rows) => {
                for (ri, row) in rows.iter().enumerate() {
                    let mut lruns: Vec<LRun> = Vec::new();
                    for (ci, cell) in row.iter().enumerate() {
                        if ci > 0 {
                            lruns.push(LRun { text: "  |  ".into(), font: FontKind::Regular, size: base });
                        }
                        let mut cr = cell_runs(cell, base);
                        if cr.is_empty() {
                            cr.push(LRun { text: " ".into(), font: FontKind::Regular, size: base });
                        }
                        lruns.extend(cr);
                    }
                    if lruns.is_empty() {
                        continue;
                    }
                    wrap(&lruns, 0.0, text_w, Align::Left, &mut out);
                    if ri == 0 {
                        out.push(Item::Line(rule_line(base, text_w)));
                    }
                }
                out.push(Item::Line(blank_line(base)));
            }
        }
    }

    while matches!(out.first(), Some(Item::Line(l)) if l.runs.is_empty()) {
        out.remove(0);
    }
    while matches!(out.last(), Some(Item::Line(l)) if l.runs.is_empty()) {
        out.pop();
    }
    out
}

// ---------------------------------------------------------------------------
// PDF writing
// ---------------------------------------------------------------------------

/// Escape a string for a PDF literal and fold to Latin-1.
fn pdf_escape(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 2);
    for ch in s.chars() {
        let b = if (ch as u32) <= 0xFF { ch as u8 } else { b'?' };
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'(' => out.extend_from_slice(b"\\("),
            b')' => out.extend_from_slice(b"\\)"),
            _ => out.push(b),
        }
    }
    out
}

/// Convert a `.docx` byte stream into a paginated PDF.
///
/// `font_size` is the base body size in points (6–48) used for text without its
/// own size; `margin` is the page margin in points (72 = 1 inch); `page_size`
/// is `"letter"` (default) or `"a4"`; `page_numbers` draws a centered
/// `n / total` footer.
pub fn docx_to_pdf(
    docx: &[u8],
    font_size: f64,
    margin: f64,
    page_size: &str,
    page_numbers: bool,
) -> Result<Vec<u8>, String> {
    if docx.is_empty() {
        return Err("input is empty".into());
    }
    if !font_size.is_finite() || font_size < 6.0 || font_size > 48.0 {
        return Err("font_size must be between 6 and 48 points".into());
    }
    let (page_w, page_h) = match page_size.trim().to_ascii_lowercase().as_str() {
        "" | "letter" | "us-letter" | "us letter" => (LETTER_W, LETTER_H),
        "a4" => (A4_W, A4_H),
        other => return Err(format!("unknown page_size '{other}' (use 'letter' or 'a4')")),
    };
    if !margin.is_finite() || margin < 0.0 || margin * 2.0 >= page_h.min(page_w) {
        return Err("margin is too large for the page".into());
    }

    // Open the .docx (a ZIP) and read the main document part.
    let mut zip = zip::ZipArchive::new(Cursor::new(docx))
        .map_err(|e| format!("not a valid .docx (ZIP) file: {e}"))?;
    let mut xml = Vec::new();
    {
        let mut entry = zip
            .by_name("word/document.xml")
            .map_err(|_| "this file is not a Word .docx (missing word/document.xml)".to_string())?;
        entry
            .read_to_end(&mut xml)
            .map_err(|e| format!("failed to read word/document.xml: {e}"))?;
    }

    let blocks = parse_document(&xml)?;

    let text_w = page_w - 2.0 * margin;
    let text_h = page_h - 2.0 * margin;

    let items = layout(&blocks, font_size, text_w);

    // Paginate.
    let mut pages: Vec<Vec<Line>> = Vec::new();
    let mut cur: Vec<Line> = Vec::new();
    let mut used = 0.0;
    for item in items {
        match item {
            Item::PageBreak => {
                if !cur.is_empty() {
                    pages.push(std::mem::take(&mut cur));
                    used = 0.0;
                }
            }
            Item::Line(line) => {
                let h = line.height;
                if used + h > text_h && !cur.is_empty() {
                    pages.push(std::mem::take(&mut cur));
                    used = 0.0;
                }
                cur.push(line);
                used += h;
            }
        }
    }
    if !cur.is_empty() {
        pages.push(cur);
    }
    if pages.is_empty() {
        pages.push(Vec::new());
    }

    // Assemble the PDF.
    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();

    let mk_font = |doc: &mut Document, kind: FontKind| {
        doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => kind.base_font(),
            "Encoding" => "WinAnsiEncoding",
        })
    };
    let f1 = mk_font(&mut doc, FontKind::Regular);
    let f2 = mk_font(&mut doc, FontKind::Bold);
    let f3 = mk_font(&mut doc, FontKind::Italic);
    let f4 = mk_font(&mut doc, FontKind::BoldItalic);
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => f1, "F2" => f2, "F3" => f3, "F4" => f4 },
    });

    let total_pages = pages.len();
    let mut page_ids: Vec<Object> = Vec::new();
    for (page_idx, page_lines) in pages.iter().enumerate() {
        let mut ops: Vec<Operation> = Vec::new();
        ops.push(Operation::new("BT", vec![]));
        let mut active: Option<(FontKind, f64)> = None;
        let mut y = page_h - margin;
        for line in page_lines {
            y -= line.height;
            if line.runs.is_empty() {
                continue;
            }
            let baseline = y + line.height * 0.25;
            let line_w: f64 = line.runs.iter().map(run_width).sum();
            let base_x = margin + line.indent;
            let avail = text_w - line.indent;
            let mut x = match line.align {
                Align::Left | Align::Justify => base_x,
                Align::Center => base_x + ((avail - line_w) / 2.0).max(0.0),
                Align::Right => base_x + (avail - line_w).max(0.0),
            };
            for run in &line.runs {
                if run.text.is_empty() {
                    continue;
                }
                if active != Some((run.font, run.size)) {
                    ops.push(Operation::new("Tf", vec![run.font.resource().into(), run.size.into()]));
                    active = Some((run.font, run.size));
                }
                ops.push(Operation::new(
                    "Tm",
                    vec![1.into(), 0.into(), 0.into(), 1.into(), x.into(), baseline.into()],
                ));
                ops.push(Operation::new(
                    "Tj",
                    vec![Object::String(pdf_escape(&run.text), lopdf::StringFormat::Literal)],
                ));
                x += run_width(run);
            }
        }
        if page_numbers {
            let foot_size = (font_size * 0.8).max(7.0);
            let label = format!("{} / {}", page_idx + 1, total_pages);
            let label_w = label.chars().count() as f64 * foot_size * FontKind::Regular.char_em();
            let fx = (page_w - label_w) / 2.0;
            let fy = (margin - foot_size * 1.5).max(foot_size);
            ops.push(Operation::new("Tf", vec![FontKind::Regular.resource().into(), foot_size.into()]));
            ops.push(Operation::new(
                "Tm",
                vec![1.into(), 0.into(), 0.into(), 1.into(), fx.into(), fy.into()],
            ));
            ops.push(Operation::new(
                "Tj",
                vec![Object::String(pdf_escape(&label), lopdf::StringFormat::Literal)],
            ));
        }
        ops.push(Operation::new("ET", vec![]));

        let content = Content { operations: ops };
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().map_err(|e| format!("content encode: {e}"))?,
        ));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), page_w.into(), page_h.into()],
        });
        page_ids.push(page_id.into());
    }

    let count = page_ids.len() as i64;
    let pages_dict = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids,
        "Count" => count,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut buf = Vec::new();
    doc.save_to(&mut buf).map_err(|e| format!("failed to write PDF: {e}"))?;
    Ok(buf)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;

    /// Wrap a `word/document.xml` body in a minimal .docx (ZIP). Only the
    /// document part is read, so a single entry suffices.
    fn docx_of(document_xml: &str) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            w.start_file("word/document.xml", opts).unwrap();
            w.write_all(document_xml.as_bytes()).unwrap();
            w.finish().unwrap();
        }
        buf
    }

    fn body(inner: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">
<w:body>{inner}</w:body></w:document>"#
        )
    }

    fn page_count(pdf: &[u8]) -> usize {
        Document::load_mem(pdf).unwrap().page_iter().count()
    }

    #[test]
    fn converts_a_simple_paragraph() {
        let xml = body(r#"<w:p><w:r><w:t>Hello world</w:t></w:r></w:p>"#);
        let pdf = docx_to_pdf(&docx_of(&xml), 12.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn parses_bold_italic_and_headings() {
        let xml = body(
            r#"<w:p><w:pPr><w:pStyle w:val="Heading1"/></w:pPr><w:r><w:t>Title</w:t></w:r></w:p>
<w:p><w:r><w:rPr><w:b/></w:rPr><w:t>bold</w:t></w:r><w:r><w:t> and </w:t></w:r><w:r><w:rPr><w:i/></w:rPr><w:t>italic</w:t></w:r></w:p>"#,
        );
        let blocks = parse_document(xml.as_bytes()).unwrap();
        match &blocks[0] {
            Block::Para(p) => {
                assert_eq!(p.style, ParaStyle::Heading(1));
                assert_eq!(p.segments[0][0].text, "Title");
            }
            _ => panic!("expected a paragraph"),
        }
        match &blocks[1] {
            Block::Para(p) => {
                let runs: Vec<&SRun> = p.segments.iter().flatten().collect();
                assert!(runs.iter().any(|r| r.text == "bold" && r.bold && !r.italic));
                assert!(runs.iter().any(|r| r.text == "italic" && r.italic && !r.bold));
            }
            _ => panic!("expected a paragraph"),
        }
        let pdf = docx_to_pdf(&docx_of(&xml), 11.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn explicit_page_break_makes_two_pages() {
        let xml = body(
            r#"<w:p><w:r><w:t>Page one</w:t><w:br w:type="page"/><w:t>Page two</w:t></w:r></w:p>"#,
        );
        let pdf = docx_to_pdf(&docx_of(&xml), 12.0, 72.0, "letter", false).unwrap();
        assert_eq!(page_count(&pdf), 2, "an explicit page break should split pages");
    }

    #[test]
    fn long_document_paginates() {
        let paras: String = (0..400)
            .map(|i| format!("<w:p><w:r><w:t>Paragraph number {i} with some words.</w:t></w:r></w:p>"))
            .collect();
        let pdf = docx_to_pdf(&docx_of(&body(&paras)), 11.0, 72.0, "letter", false).unwrap();
        assert!(page_count(&pdf) > 1, "400 paragraphs should span multiple pages");
    }

    #[test]
    fn renders_a_table() {
        let xml = body(
            r#"<w:tbl>
<w:tr><w:tc><w:p><w:r><w:t>Name</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>Age</w:t></w:r></w:p></w:tc></w:tr>
<w:tr><w:tc><w:p><w:r><w:t>Ada</w:t></w:r></w:p></w:tc><w:tc><w:p><w:r><w:t>36</w:t></w:r></w:p></w:tc></w:tr>
</w:tbl>"#,
        );
        let blocks = parse_document(xml.as_bytes()).unwrap();
        match blocks.iter().find(|b| matches!(b, Block::Table(_))) {
            Some(Block::Table(rows)) => {
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0][0][0].text, "Name");
                assert_eq!(rows[1][1][0].text, "36");
            }
            _ => panic!("expected a table block"),
        }
        let pdf = docx_to_pdf(&docx_of(&xml), 11.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn honors_center_alignment_and_explicit_size() {
        let xml = body(
            r#"<w:p><w:pPr><w:jc w:val="center"/></w:pPr><w:r><w:rPr><w:sz w:val="48"/></w:rPr><w:t>Big centered</w:t></w:r></w:p>"#,
        );
        let blocks = parse_document(xml.as_bytes()).unwrap();
        match &blocks[0] {
            Block::Para(p) => {
                assert_eq!(p.align, Align::Center);
                assert_eq!(p.segments[0][0].sz_pt, Some(24.0)); // 48 half-points = 24pt
            }
            _ => panic!("expected a paragraph"),
        }
        let pdf = docx_to_pdf(&docx_of(&xml), 11.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn parses_list_items() {
        let xml = body(
            r#"<w:p><w:pPr><w:numPr><w:ilvl w:val="0"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>first</w:t></w:r></w:p>
<w:p><w:pPr><w:numPr><w:ilvl w:val="1"/><w:numId w:val="1"/></w:numPr></w:pPr><w:r><w:t>nested</w:t></w:r></w:p>"#,
        );
        let blocks = parse_document(xml.as_bytes()).unwrap();
        match &blocks[0] {
            Block::Para(p) => assert_eq!(p.list_level, Some(0)),
            _ => panic!("expected a list paragraph"),
        }
        match &blocks[1] {
            Block::Para(p) => assert_eq!(p.list_level, Some(1)),
            _ => panic!("expected a nested list paragraph"),
        }
    }

    #[test]
    fn decodes_entities() {
        let xml = body(r#"<w:p><w:r><w:t>Tom &amp; Jerry &lt;3</w:t></w:r></w:p>"#);
        let blocks = parse_document(xml.as_bytes()).unwrap();
        let text: String = match &blocks[0] {
            Block::Para(p) => p.segments.iter().flatten().map(|r| r.text.clone()).collect(),
            _ => panic!(),
        };
        assert_eq!(text, "Tom & Jerry <3");
    }

    #[test]
    fn a4_page_size_uses_a4_mediabox() {
        let xml = body(r#"<w:p><w:r><w:t>body</w:t></w:r></w:p>"#);
        let pdf = docx_to_pdf(&docx_of(&xml), 11.0, 72.0, "A4", false).unwrap();
        assert!(String::from_utf8_lossy(&pdf).contains("842"), "A4 height 842");
        let letter = docx_to_pdf(&docx_of(&xml), 11.0, 72.0, "letter", false).unwrap();
        assert!(String::from_utf8_lossy(&letter).contains("792"), "letter height 792");
    }

    #[test]
    fn page_numbers_add_a_footer() {
        let paras: String =
            (0..150).map(|i| format!("<w:p><w:r><w:t>Line {i}.</w:t></w:r></w:p>")).collect();
        let docx = docx_of(&body(&paras));
        let with = docx_to_pdf(&docx, 11.0, 72.0, "letter", true).unwrap();
        let without = docx_to_pdf(&docx, 11.0, 72.0, "letter", false).unwrap();
        assert!(page_count(&with) >= 2);
        assert!(with.len() > without.len(), "footer should add bytes");
    }

    #[test]
    fn empty_body_still_makes_a_page() {
        let pdf = docx_to_pdf(&docx_of(&body("")), 11.0, 72.0, "letter", false).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn errors_on_bad_input() {
        assert!(docx_to_pdf(b"", 12.0, 72.0, "letter", false).is_err()); // empty
        assert!(docx_to_pdf(b"not a zip", 12.0, 72.0, "letter", false).is_err()); // not a zip
        // A ZIP without word/document.xml is not a .docx.
        let mut buf = Vec::new();
        {
            let mut w = zip::ZipWriter::new(Cursor::new(&mut buf));
            w.start_file("hello.txt", SimpleFileOptions::default()).unwrap();
            w.write_all(b"hi").unwrap();
            w.finish().unwrap();
        }
        assert!(docx_to_pdf(&buf, 12.0, 72.0, "letter", false).is_err());
        // Bad params.
        let xml = docx_of(&body(r#"<w:p><w:r><w:t>x</w:t></w:r></w:p>"#));
        assert!(docx_to_pdf(&xml, 2.0, 72.0, "letter", false).is_err()); // font too small
        assert!(docx_to_pdf(&xml, 60.0, 72.0, "letter", false).is_err()); // font too large
        assert!(docx_to_pdf(&xml, 12.0, 400.0, "letter", false).is_err()); // margin too big
        assert!(docx_to_pdf(&xml, 12.0, 72.0, "legal", false).is_err()); // unknown page size
    }
}
