//! markdown-deck-to-pdf core — turn a Markdown deck into a paginated PDF with
//! **one slide per page**. Pure logic shared by the chat skill block, the CLI and
//! the web page — no wafer / wasm-bindgen deps.
//!
//! A thematic break (`---`, `***`, `___`) always starts a new slide; headings can
//! additionally cut the deck at `#`, `##`, both, or not at all. Each slide is laid
//! out onto a single fixed-size landscape page (16:9, 4:3, A4 or Letter), with the
//! slide heading as the page title and the remaining Markdown — paragraphs, nested
//! lists, code blocks, quotes, tables, inline `**bold**` / `*italic*` / `` `code` ``
//! — flowed beneath it. Body text automatically shrinks to fit; a slide that still
//! overflows continues onto extra pages rather than losing content.
//!
//! Rendering uses `lopdf` with the base-14 PDF fonts (Helvetica family for text,
//! Courier for code), so the output is small, deterministic and needs no font
//! embedding. Text is folded to Latin-1 (WinAnsi); characters outside it render as
//! `?`. Images are rendered as their alt text.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, ObjectId, Stream, StringFormat};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// Hard cap on how many slides one deck may contain.
pub const MAX_SLIDES: usize = 300;

/// Default body font size in points.
pub const DEFAULT_FONT_SIZE: f64 = 20.0;
/// Smallest accepted body font size in points.
pub const MIN_FONT_SIZE: f64 = 8.0;
/// Largest accepted body font size in points.
pub const MAX_FONT_SIZE: f64 = 48.0;

const LINE_FACTOR: f64 = 1.32;
const PARA_GAP: f64 = 0.45; // blank space after a block, in line-heights
const INDENT_PT: f64 = 16.0; // per list-nesting / quote level
const TAB_WIDTH: usize = 4;
/// Shrink-to-fit never goes below half the requested body size.
const MIN_SCALE: f64 = 0.5;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Which heading levels start a new slide (a `---` break always does).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SplitLevel {
    /// Start a new slide at each `#`.
    H1,
    /// Start a new slide at each `##`.
    H2,
    /// Start a new slide at every `#` and `##`.
    Both,
    /// Never split on headings — only `---` breaks separate slides.
    None,
}

impl SplitLevel {
    /// Parse a split-level name (canonical value + common aliases).
    pub fn parse(s: &str) -> Result<SplitLevel, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "h1" | "1" => Ok(SplitLevel::H1),
            "h2" | "2" => Ok(SplitLevel::H2),
            "both" | "h1h2" | "h1-h2" => Ok(SplitLevel::Both),
            "none" | "off" | "rule" | "rules" => Ok(SplitLevel::None),
            other => Err(format!(
                "unknown split_level '{other}' (use h1, h2, both, or none)"
            )),
        }
    }

    fn splits_at(self, depth: u8) -> bool {
        match self {
            SplitLevel::H1 => depth == 1,
            SplitLevel::H2 => depth == 2,
            SplitLevel::Both => depth == 1 || depth == 2,
            SplitLevel::None => false,
        }
    }
}

/// Fixed page geometry for every slide (all landscape).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SlideSize {
    /// 16:9 widescreen — 960 × 540 pt (13.33 × 7.5 in).
    Wide169,
    /// 4:3 standard — 720 × 540 pt (10 × 7.5 in).
    Standard43,
    /// ISO A4 landscape — 842 × 595 pt.
    A4Landscape,
    /// US Letter landscape — 792 × 612 pt.
    LetterLandscape,
}

impl SlideSize {
    /// Parse a slide-size name (canonical value + common aliases).
    pub fn parse(s: &str) -> Result<SlideSize, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "16:9" | "169" | "16-9" | "widescreen" | "wide" => Ok(SlideSize::Wide169),
            "4:3" | "43" | "4-3" | "standard" | "std" => Ok(SlideSize::Standard43),
            "a4-landscape" | "a4" | "a4landscape" => Ok(SlideSize::A4Landscape),
            "letter-landscape" | "letter" | "us-letter" => Ok(SlideSize::LetterLandscape),
            other => Err(format!(
                "unknown slide_size '{other}' (use 16:9, 4:3, a4-landscape, or letter-landscape)"
            )),
        }
    }

    /// `(width, height)` of the page in PDF points.
    pub fn dims(self) -> (f64, f64) {
        match self {
            SlideSize::Wide169 => (960.0, 540.0),
            SlideSize::Standard43 => (720.0, 540.0),
            SlideSize::A4Landscape => (842.0, 595.0),
            SlideSize::LetterLandscape => (792.0, 612.0),
        }
    }
}

/// Slide colour theme.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Theme {
    /// Dark text on a white background.
    Light,
    /// Light text on a near-black background.
    Dark,
}

impl Theme {
    /// Parse a theme name (canonical value + common aliases).
    pub fn parse(s: &str) -> Result<Theme, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "light" | "white" => Ok(Theme::Light),
            "dark" | "black" | "night" => Ok(Theme::Dark),
            other => Err(format!("unknown theme '{other}' (use light or dark)")),
        }
    }

    fn palette(self) -> Palette {
        match self {
            Theme::Light => Palette {
                bg: (1.0, 1.0, 1.0),
                title: (0.08, 0.09, 0.11),
                body: (0.16, 0.17, 0.20),
                muted: (0.48, 0.50, 0.54),
            },
            Theme::Dark => Palette {
                bg: (0.09, 0.10, 0.12),
                title: (1.0, 1.0, 1.0),
                body: (0.88, 0.89, 0.91),
                muted: (0.58, 0.60, 0.64),
            },
        }
    }
}

#[derive(Clone, Copy)]
struct Palette {
    bg: (f64, f64, f64),
    title: (f64, f64, f64),
    body: (f64, f64, f64),
    muted: (f64, f64, f64),
}

/// Everything that shapes the rendered deck besides the Markdown itself.
#[derive(Clone, Copy, Debug)]
pub struct DeckOptions<'a> {
    /// Optional deck title — rendered as a centered cover slide when non-empty.
    pub title: &'a str,
    /// Which heading levels start a new slide.
    pub split: SplitLevel,
    /// Page geometry.
    pub size: SlideSize,
    /// Colour theme.
    pub theme: Theme,
    /// Base body font size in points (8–48); shrinks automatically to fit.
    pub font_size: f64,
    /// Repeated header text, top-left of every slide (empty = none).
    pub header: &'a str,
    /// Repeated footer text, bottom-left of every slide (empty = none).
    pub footer: &'a str,
    /// Print `n / total` in the bottom-right corner.
    pub page_numbers: bool,
    /// Add a PDF outline (bookmarks) with one entry per slide.
    pub outline: bool,
}

impl Default for DeckOptions<'_> {
    fn default() -> Self {
        DeckOptions {
            title: "",
            split: SplitLevel::H1,
            size: SlideSize::Wide169,
            theme: Theme::Light,
            font_size: DEFAULT_FONT_SIZE,
            header: "",
            footer: "",
            page_numbers: true,
            outline: true,
        }
    }
}

// ---------------------------------------------------------------------------
// Fonts + text primitives
// ---------------------------------------------------------------------------

/// The base-14 fonts we map onto. Index = resource name F1..F5.
#[derive(Clone, Copy, PartialEq, Debug)]
enum FontKind {
    Regular,    // Helvetica
    Bold,       // Helvetica-Bold
    Italic,     // Helvetica-Oblique
    BoldItalic, // Helvetica-BoldOblique
    Mono,       // Courier
}

impl FontKind {
    fn resource(self) -> &'static str {
        match self {
            FontKind::Regular => "F1",
            FontKind::Bold => "F2",
            FontKind::Italic => "F3",
            FontKind::BoldItalic => "F4",
            FontKind::Mono => "F5",
        }
    }
    fn base_font(self) -> &'static str {
        match self {
            FontKind::Regular => "Helvetica",
            FontKind::Bold => "Helvetica-Bold",
            FontKind::Italic => "Helvetica-Oblique",
            FontKind::BoldItalic => "Helvetica-BoldOblique",
            FontKind::Mono => "Courier",
        }
    }
    /// Average glyph advance as a fraction of the em, for wrap estimation.
    /// Courier is exactly 0.6; the Helvetica family averages ~0.5 (bold ~0.53).
    fn char_em(self) -> f64 {
        match self {
            FontKind::Mono => 0.6,
            FontKind::Bold | FontKind::BoldItalic => 0.53,
            _ => 0.5,
        }
    }
    fn from_flags(bold: bool, italic: bool, code: bool) -> FontKind {
        if code {
            FontKind::Mono
        } else {
            match (bold, italic) {
                (true, true) => FontKind::BoldItalic,
                (true, false) => FontKind::Bold,
                (false, true) => FontKind::Italic,
                (false, false) => FontKind::Regular,
            }
        }
    }
}

/// A run of text with a single font, the atom of a laid-out line.
#[derive(Clone, Debug)]
struct Run {
    text: String,
    font: FontKind,
    size: f64,
}

impl Run {
    fn width(&self) -> f64 {
        self.text.chars().count() as f64 * self.size * self.font.char_em()
    }
}

/// A laid-out line: runs plus the left indent (points) and the line height.
#[derive(Clone, Debug, Default)]
struct Line {
    runs: Vec<Run>,
    indent: f64,
    height: f64,
}

impl Line {
    fn width(&self) -> f64 {
        self.runs.iter().map(Run::width).sum::<f64>()
    }
}

/// Escape a string for a PDF literal and fold it to Latin-1.
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

// ---------------------------------------------------------------------------
// Slide splitting (line level, so `---` and fences behave predictably)
// ---------------------------------------------------------------------------

/// A thematic break: a line of 3+ of `-`, `*`, or `_` (whitespace ignored).
fn is_thematic_break(line: &str) -> bool {
    let stripped: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    if stripped.len() < 3 {
        return false;
    }
    ['-', '*', '_']
        .iter()
        .any(|m| stripped.chars().all(|c| c == *m))
}

/// If `line` is an ATX heading (`#`…`######`), return `(depth, raw text)`.
fn atx_heading(line: &str) -> Option<(u8, String)> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    Some((hashes as u8, rest.trim().trim_end_matches('#').trim().to_string()))
}

/// One source slide: an optional title heading plus the raw Markdown body.
#[derive(Debug, Default)]
struct SlideSrc {
    title: String,
    body: String,
}

impl SlideSrc {
    fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.body.trim().is_empty()
    }
}

/// Cut the Markdown source into slides. `---` always breaks; headings break per
/// `split`. Non-splitting headings stay in the body and render as sub-headings.
fn split_slides(markdown: &str, split: SplitLevel) -> Vec<SlideSrc> {
    let mut slides: Vec<SlideSrc> = Vec::new();
    let mut cur = SlideSrc::default();
    let mut in_fence = false;

    for raw in markdown.lines() {
        if raw.trim_start().starts_with("```") || raw.trim_start().starts_with("~~~") {
            in_fence = !in_fence;
            cur.body.push_str(raw);
            cur.body.push('\n');
            continue;
        }
        if in_fence {
            cur.body.push_str(raw);
            cur.body.push('\n');
            continue;
        }
        if is_thematic_break(raw) {
            if !cur.is_empty() {
                slides.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if let Some((depth, text)) = atx_heading(raw) {
            if split.splits_at(depth) {
                if !split.splits_at(1)
                    && !cur.title.is_empty()
                    && cur.body.trim().is_empty()
                    && slides.is_empty()
                {
                    // A leading non-splitting heading is commonly the deck title.
                    // When the requested divider is a lower heading (for example
                    // split_level=h2), keep the first matching lower heading inside
                    // that titled opening slide instead of emitting a title-only
                    // cover page the user did not ask for.
                    cur.body.push_str(raw);
                    cur.body.push('\n');
                    continue;
                }
                if !cur.is_empty() {
                    slides.push(std::mem::take(&mut cur));
                }
                cur.title = text;
                continue;
            }
            if cur.title.is_empty() && cur.body.trim().is_empty() {
                // A leading non-splitting heading titles the slide anyway.
                cur.title = text;
                continue;
            }
        }
        cur.body.push_str(raw);
        cur.body.push('\n');
    }
    if !cur.is_empty() {
        slides.push(cur);
    }
    slides
}

// ---------------------------------------------------------------------------
// Body layout (pulldown-cmark → wrapped lines of runs)
// ---------------------------------------------------------------------------

fn sub_heading_size(level: HeadingLevel, base: f64) -> f64 {
    let scale = match level {
        HeadingLevel::H1 => 1.3,
        HeadingLevel::H2 => 1.18,
        HeadingLevel::H3 => 1.08,
        HeadingLevel::H4 => 1.0,
        HeadingLevel::H5 | HeadingLevel::H6 => 0.95,
    };
    base * scale
}

fn blank_line(base: f64) -> Line {
    Line { runs: Vec::new(), indent: 0.0, height: base * LINE_FACTOR * PARA_GAP }
}

fn rule_line(base: f64, text_w: f64) -> Line {
    let n = (text_w / (base * 0.5)).floor().max(1.0) as usize;
    Line {
        runs: vec![Run { text: "_".repeat(n), font: FontKind::Regular, size: base * 0.7 }],
        indent: 0.0,
        height: base * LINE_FACTOR,
    }
}

/// Emit `buf` as one verbatim (unwrapped) line at `indent`, then clear it.
fn emit_verbatim(buf: &mut Vec<Run>, lines: &mut Vec<Line>, indent: f64) {
    let runs = std::mem::take(buf);
    let height = runs.iter().map(|r| r.size).fold(0.0_f64, f64::max).max(1.0) * LINE_FACTOR;
    lines.push(Line { runs, indent, height });
}

/// Word-wrap a run-stream into lines fitting `avail` points wide, at `indent`.
fn wrap_runs(buf: &[Run], indent: f64, avail: f64, lines: &mut Vec<Line>) {
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
            words.push(Word { text: " ".to_string(), font: r.font, size: r.size });
        }
    }

    let word_w = |w: &Word| w.text.chars().count() as f64 * w.size * w.font.char_em();

    let mut cur: Vec<Run> = Vec::new();
    let mut cur_w = 0.0;
    let mut max_size = 0.0_f64;

    let push_line = |cur: &mut Vec<Run>, max_size: &mut f64, lines: &mut Vec<Line>| {
        if cur.is_empty() {
            return;
        }
        let height = (*max_size).max(1.0) * LINE_FACTOR;
        lines.push(Line { runs: std::mem::take(cur), indent, height });
        *max_size = 0.0;
    };

    for w in &words {
        let ww = word_w(w);
        if cur_w + ww > avail && !cur.is_empty() {
            push_line(&mut cur, &mut max_size, lines);
            cur_w = 0.0;
            let trimmed = w.text.trim_start().to_string();
            let tw = trimmed.chars().count() as f64 * w.size * w.font.char_em();
            cur.push(Run { text: trimmed, font: w.font, size: w.size });
            cur_w += tw;
            max_size = max_size.max(w.size);
            continue;
        }
        cur.push(Run { text: w.text.clone(), font: w.font, size: w.size });
        cur_w += ww;
        max_size = max_size.max(w.size);
    }
    push_line(&mut cur, &mut max_size, lines);
}

/// Lay a slide body out into wrapped lines at body size `base`.
fn layout_body(markdown: &str, base: f64, text_w: f64) -> Vec<Line> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(markdown, opts);

    let mut lines: Vec<Line> = Vec::new();
    let (mut bold, mut italic, mut code) = (false, false, false);
    let mut buf: Vec<Run> = Vec::new();
    let mut cur_size = base;
    let mut indent_level: f64 = 0.0;
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_code_block = false;
    let mut quote_depth: f64 = 0.0;

    let flush = |buf: &mut Vec<Run>, lines: &mut Vec<Line>, indent: f64, gap: bool| {
        if buf.is_empty() {
            return;
        }
        let avail = (text_w - indent).max(base * 4.0);
        wrap_runs(buf, indent, avail, lines);
        buf.clear();
        if gap {
            lines.push(blank_line(base));
        }
    };

    for ev in parser {
        match ev {
            Event::Start(tag) => match tag {
                Tag::Heading { level, .. } => {
                    cur_size = sub_heading_size(level, base);
                    bold = true;
                }
                Tag::CodeBlock(_) => {
                    in_code_block = true;
                    code = true;
                    cur_size = base * 0.88;
                }
                Tag::List(start) => {
                    list_stack.push(start);
                    indent_level += 1.0;
                }
                Tag::Item => {
                    let marker = match list_stack.last_mut() {
                        Some(Some(n)) => {
                            let m = format!("{n}. ");
                            *n += 1;
                            m
                        }
                        _ => "\u{2022} ".to_string(),
                    };
                    buf.push(Run { text: marker, font: FontKind::Regular, size: base });
                }
                Tag::BlockQuote(_) => quote_depth += 1.0,
                Tag::Emphasis => italic = true,
                Tag::Strong => bold = true,
                _ => {}
            },
            Event::End(tag) => match tag {
                TagEnd::Heading(_) => {
                    flush(&mut buf, &mut lines, quote_depth * INDENT_PT, true);
                    bold = false;
                    cur_size = base;
                }
                TagEnd::Paragraph => {
                    let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT;
                    flush(&mut buf, &mut lines, indent, true);
                }
                TagEnd::CodeBlock => {
                    let indent =
                        indent_level * INDENT_PT + quote_depth * INDENT_PT + INDENT_PT * 0.5;
                    flush(&mut buf, &mut lines, indent, true);
                    in_code_block = false;
                    code = false;
                    cur_size = base;
                }
                TagEnd::List(_) => {
                    list_stack.pop();
                    indent_level = (indent_level - 1.0).max(0.0);
                    if indent_level == 0.0 && quote_depth == 0.0 {
                        lines.push(blank_line(base));
                    }
                }
                TagEnd::Item => {
                    let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT;
                    flush(&mut buf, &mut lines, indent, false);
                }
                TagEnd::BlockQuote(_) => quote_depth = (quote_depth - 1.0).max(0.0),
                TagEnd::Emphasis => italic = false,
                TagEnd::Strong => bold = false,
                TagEnd::TableHead | TagEnd::TableRow => {
                    flush(&mut buf, &mut lines, quote_depth * INDENT_PT, false);
                }
                TagEnd::TableCell => {
                    buf.push(Run { text: " | ".to_string(), font: FontKind::Regular, size: base });
                }
                TagEnd::Table => lines.push(blank_line(base)),
                _ => {}
            },
            Event::Text(t) => {
                let font = FontKind::from_flags(bold, italic, code);
                if in_code_block {
                    let expanded = t.replace('\t', &" ".repeat(TAB_WIDTH));
                    let mut parts = expanded.split('\n').peekable();
                    while let Some(seg) = parts.next() {
                        buf.push(Run { text: seg.to_string(), font, size: cur_size });
                        if parts.peek().is_some() {
                            let indent = indent_level * INDENT_PT
                                + quote_depth * INDENT_PT
                                + INDENT_PT * 0.5;
                            emit_verbatim(&mut buf, &mut lines, indent);
                        }
                    }
                } else {
                    buf.push(Run { text: t.to_string(), font, size: cur_size });
                }
            }
            Event::Code(t) => {
                buf.push(Run { text: t.to_string(), font: FontKind::Mono, size: cur_size * 0.92 });
            }
            Event::SoftBreak => {
                buf.push(Run { text: " ".to_string(), font: FontKind::Regular, size: cur_size });
            }
            Event::HardBreak => {
                let indent = indent_level * INDENT_PT + quote_depth * INDENT_PT;
                emit_verbatim(&mut buf, &mut lines, indent);
            }
            Event::Rule => {
                flush(&mut buf, &mut lines, 0.0, false);
                lines.push(rule_line(base, text_w));
            }
            Event::TaskListMarker(done) => {
                let mark = if done { "[x] " } else { "[ ] " };
                buf.push(Run { text: mark.to_string(), font: FontKind::Mono, size: base });
            }
            _ => {}
        }
    }
    flush(&mut buf, &mut lines, quote_depth * INDENT_PT, false);

    while lines.first().map(|l| l.runs.is_empty()).unwrap_or(false) {
        lines.remove(0);
    }
    while lines.last().map(|l| l.runs.is_empty()).unwrap_or(false) {
        lines.pop();
    }
    lines
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// One physical PDF page: the slide it belongs to plus its laid-out content.
struct PagePlan {
    slide_no: usize,
    title: String,
    body: Vec<Line>,
    is_cover: bool,
    first_of_slide: bool,
}

fn set_fill(ops: &mut Vec<Operation>, c: (f64, f64, f64)) {
    ops.push(Operation::new("rg", vec![c.0.into(), c.1.into(), c.2.into()]));
}

/// Draw one laid-out line's runs starting at `(x, baseline)`.
fn draw_line(ops: &mut Vec<Operation>, line: &Line, x0: f64, baseline: f64) {
    let mut x = x0;
    for run in &line.runs {
        if run.text.is_empty() {
            continue;
        }
        ops.push(Operation::new("Tf", vec![run.font.resource().into(), run.size.into()]));
        ops.push(Operation::new(
            "Tm",
            vec![1.into(), 0.into(), 0.into(), 1.into(), x.into(), baseline.into()],
        ));
        ops.push(Operation::new(
            "Tj",
            vec![Object::String(pdf_escape(&run.text), StringFormat::Literal)],
        ));
        x += run.width();
    }
}

/// Draw a single-font text label at `(x, baseline)`.
fn draw_label(ops: &mut Vec<Operation>, text: &str, font: FontKind, size: f64, x: f64, y: f64) {
    if text.is_empty() {
        return;
    }
    ops.push(Operation::new("Tf", vec![font.resource().into(), size.into()]));
    ops.push(Operation::new(
        "Tm",
        vec![1.into(), 0.into(), 0.into(), 1.into(), x.into(), y.into()],
    ));
    ops.push(Operation::new(
        "Tj",
        vec![Object::String(pdf_escape(text), StringFormat::Literal)],
    ));
}

fn label_width(text: &str, font: FontKind, size: f64) -> f64 {
    text.chars().count() as f64 * size * font.char_em()
}

/// Convert a Markdown deck into a slide-per-page PDF.
pub fn to_pdf(markdown: &str, opts: &DeckOptions) -> Result<Vec<u8>, String> {
    to_pdf_with_counts(markdown, opts).map(|(bytes, _, _)| bytes)
}

/// Like [`to_pdf`] but also returns `(slide count, page count)` so callers can
/// summarise the deck without re-parsing the PDF.
pub fn to_pdf_with_counts(
    markdown: &str,
    opts: &DeckOptions,
) -> Result<(Vec<u8>, usize, usize), String> {
    if !opts.font_size.is_finite()
        || opts.font_size < MIN_FONT_SIZE
        || opts.font_size > MAX_FONT_SIZE
    {
        return Err(format!(
            "font_size must be between {MIN_FONT_SIZE} and {MAX_FONT_SIZE} points"
        ));
    }
    if markdown.trim().is_empty() && opts.title.trim().is_empty() {
        return Err("markdown is empty — there is nothing to put on a slide".into());
    }

    let sources = split_slides(markdown, opts.split);
    let cover = !opts.title.trim().is_empty();
    let slide_count = sources.len() + usize::from(cover);
    if slide_count == 0 {
        return Err("markdown produced no slides — add some content or a deck title".into());
    }
    if slide_count > MAX_SLIDES {
        return Err(format!(
            "deck has {slide_count} slides, over the {MAX_SLIDES}-slide cap"
        ));
    }

    let (page_w, page_h) = opts.size.dims();
    let pal = opts.theme.palette();
    let mx = page_w * 0.075;
    let my = page_h * 0.085;
    let content_w = page_w - 2.0 * mx;

    let chrome_size = (opts.font_size * 0.55).clamp(7.0, 14.0);
    let has_header = !opts.header.trim().is_empty();
    let has_footer = !opts.footer.trim().is_empty() || opts.page_numbers;

    let top = page_h - my - if has_header { chrome_size * 2.0 } else { 0.0 };
    let bottom = my + if has_footer { chrome_size * 2.0 } else { 0.0 };
    let avail_h = (top - bottom).max(opts.font_size * 2.0);

    // Lay every slide out, shrinking body text until it fits (or MIN_SCALE).
    let mut plans: Vec<PagePlan> = Vec::new();
    let mut slide_no = 0usize;

    if cover {
        slide_no += 1;
        plans.push(PagePlan {
            slide_no,
            title: opts.title.trim().to_string(),
            body: Vec::new(),
            is_cover: true,
            first_of_slide: true,
        });
    }

    for src in &sources {
        slide_no += 1;
        let title = src.title.trim().to_string();
        let title_size = opts.font_size * 1.55;
        let title_block = if title.is_empty() { 0.0 } else { title_size * 1.9 };
        let body_h = (avail_h - title_block).max(opts.font_size);

        let mut scale = 1.0_f64;
        let mut body = layout_body(&src.body, opts.font_size, content_w);
        loop {
            let total: f64 = body.iter().map(|l| l.height).sum();
            if total <= body_h || scale <= MIN_SCALE + 1e-9 {
                break;
            }
            scale = (scale * 0.9).max(MIN_SCALE);
            body = layout_body(&src.body, opts.font_size * scale, content_w);
        }

        // Still too tall at the smallest size → continue onto extra pages.
        let mut chunks: Vec<Vec<Line>> = Vec::new();
        let mut cur: Vec<Line> = Vec::new();
        let mut used = 0.0;
        for line in body {
            if used + line.height > body_h && !cur.is_empty() {
                chunks.push(std::mem::take(&mut cur));
                used = 0.0;
            }
            used += line.height;
            cur.push(line);
        }
        if !cur.is_empty() || chunks.is_empty() {
            chunks.push(cur);
        }
        for (i, chunk) in chunks.into_iter().enumerate() {
            plans.push(PagePlan {
                slide_no,
                title: title.clone(),
                body: chunk,
                is_cover: false,
                first_of_slide: i == 0,
            });
        }
    }

    // ---- Build the PDF ----
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
    let f5 = mk_font(&mut doc, FontKind::Mono);
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => f1, "F2" => f2, "F3" => f3, "F4" => f4, "F5" => f5 },
    });

    let total_pages = plans.len();
    let mut page_ids: Vec<ObjectId> = Vec::new();
    // (slide title, page object id) for the outline, one per slide.
    let mut bookmarks: Vec<(String, usize)> = Vec::new();

    for (idx, plan) in plans.iter().enumerate() {
        let mut ops: Vec<Operation> = Vec::new();

        // Background fill (outside BT/ET).
        set_fill(&mut ops, pal.bg);
        ops.push(Operation::new(
            "re",
            vec![0.into(), 0.into(), page_w.into(), page_h.into()],
        ));
        ops.push(Operation::new("f", vec![]));

        ops.push(Operation::new("BT", vec![]));

        if has_header {
            set_fill(&mut ops, pal.muted);
            draw_label(
                &mut ops,
                opts.header.trim(),
                FontKind::Regular,
                chrome_size,
                mx,
                page_h - my * 0.55 - chrome_size,
            );
        }

        if plan.is_cover {
            let size = opts.font_size * 2.2;
            let mut lines: Vec<Line> = Vec::new();
            wrap_runs(
                &[Run { text: plan.title.clone(), font: FontKind::Bold, size }],
                0.0,
                content_w,
                &mut lines,
            );
            let block_h: f64 = lines.iter().map(|l| l.height).sum();
            let mut y = (page_h + block_h) / 2.0;
            set_fill(&mut ops, pal.title);
            for line in &lines {
                y -= line.height;
                let x = mx + (content_w - line.width()) / 2.0;
                draw_line(&mut ops, line, x, y + line.height * 0.25);
            }
        } else {
            let mut y = top;
            if !plan.title.is_empty() {
                let title_size = opts.font_size * 1.55;
                let mut tlines: Vec<Line> = Vec::new();
                wrap_runs(
                    &[Run { text: plan.title.clone(), font: FontKind::Bold, size: title_size }],
                    0.0,
                    content_w,
                    &mut tlines,
                );
                set_fill(&mut ops, pal.title);
                for line in &tlines {
                    y -= line.height;
                    draw_line(&mut ops, line, mx, y + line.height * 0.25);
                }
                y -= title_size * 0.45;
            }
            set_fill(&mut ops, pal.body);
            for line in &plan.body {
                y -= line.height;
                if line.runs.is_empty() {
                    continue;
                }
                draw_line(&mut ops, line, mx + line.indent, y + line.height * 0.25);
            }
        }

        if has_footer {
            set_fill(&mut ops, pal.muted);
            let fy = my * 0.55;
            if !opts.footer.trim().is_empty() {
                draw_label(&mut ops, opts.footer.trim(), FontKind::Regular, chrome_size, mx, fy);
            }
            if opts.page_numbers {
                let label = format!("{} / {}", idx + 1, total_pages);
                let w = label_width(&label, FontKind::Regular, chrome_size);
                draw_label(
                    &mut ops,
                    &label,
                    FontKind::Regular,
                    chrome_size,
                    page_w - mx - w,
                    fy,
                );
            }
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
        if plan.first_of_slide {
            let label = if plan.title.is_empty() {
                format!("Slide {}", plan.slide_no)
            } else {
                plan.title.clone()
            };
            bookmarks.push((label, page_ids.len()));
        }
        page_ids.push(page_id);
    }

    let count = page_ids.len() as i64;
    let kids: Vec<Object> = page_ids.iter().map(|id| Object::Reference(*id)).collect();
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        }),
    );

    let mut catalog = dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    };

    if opts.outline && !bookmarks.is_empty() {
        let outlines_id = doc.new_object_id();
        let item_ids: Vec<ObjectId> = bookmarks.iter().map(|_| doc.new_object_id()).collect();
        for (i, (label, page_idx)) in bookmarks.iter().enumerate() {
            let mut item = dictionary! {
                "Title" => Object::String(pdf_escape(label), StringFormat::Literal),
                "Parent" => outlines_id,
                "Dest" => Object::Array(vec![
                    Object::Reference(page_ids[*page_idx]),
                    "Fit".into(),
                ]),
            };
            if i > 0 {
                item.set("Prev", Object::Reference(item_ids[i - 1]));
            }
            if i + 1 < item_ids.len() {
                item.set("Next", Object::Reference(item_ids[i + 1]));
            }
            doc.objects.insert(item_ids[i], Object::Dictionary(item));
        }
        doc.objects.insert(
            outlines_id,
            Object::Dictionary(dictionary! {
                "Type" => "Outlines",
                "First" => Object::Reference(item_ids[0]),
                "Last" => Object::Reference(item_ids[item_ids.len() - 1]),
                "Count" => item_ids.len() as i64,
            }),
        );
        catalog.set("Outlines", Object::Reference(outlines_id));
        catalog.set("PageMode", Object::Name(b"UseOutlines".to_vec()));
    }

    let catalog_id = doc.add_object(Object::Dictionary(catalog));
    doc.trailer.set("Root", catalog_id);

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("failed to write PDF: {e}"))?;
    Ok((out, slide_count, total_pages))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn page_count(pdf: &[u8]) -> usize {
        Document::load_mem(pdf).unwrap().page_iter().count()
    }

    fn opts<'a>() -> DeckOptions<'a> {
        DeckOptions::default()
    }

    #[test]
    fn one_page_per_h1_slide() {
        let md = "# One\n\n- a\n- b\n\n# Two\n\n- c\n";
        let (pdf, slides, pages) = to_pdf_with_counts(md, &opts()).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(slides, 2);
        assert_eq!(pages, 2);
        assert_eq!(page_count(&pdf), 2);
    }

    #[test]
    fn thematic_breaks_always_split() {
        let md = "just text\n\n---\n\nmore text\n\n***\n\nlast";
        let o = DeckOptions { split: SplitLevel::None, ..opts() };
        let (_, slides, _) = to_pdf_with_counts(md, &o).unwrap();
        assert_eq!(slides, 3, "--- and *** both break, even with split=none");
    }

    #[test]
    fn split_level_none_keeps_headings_inline() {
        let md = "# One\n\ntext\n\n# Two\n\ntext";
        let o = DeckOptions { split: SplitLevel::None, ..opts() };
        let (_, slides, _) = to_pdf_with_counts(md, &o).unwrap();
        assert_eq!(slides, 1);
    }

    #[test]
    fn split_level_h2_and_both() {
        let md = "# Deck\n\n## A\n\n- x\n\n## B\n\n- y";
        let (_, h2, _) =
            to_pdf_with_counts(md, &DeckOptions { split: SplitLevel::H2, ..opts() }).unwrap();
        assert_eq!(h2, 2, "two ## slides (the leading # titles the first)");
        let (_, both, _) =
            to_pdf_with_counts(md, &DeckOptions { split: SplitLevel::Both, ..opts() }).unwrap();
        assert_eq!(both, 3, "# and both ## each start a slide");
    }

    #[test]
    fn cover_slide_adds_a_page() {
        let md = "# One\n\ntext";
        let (_, slides, pages) =
            to_pdf_with_counts(md, &DeckOptions { title: "My Deck", ..opts() }).unwrap();
        assert_eq!(slides, 2);
        assert_eq!(pages, 2);
    }

    #[test]
    fn slide_sizes_set_the_mediabox() {
        for (size, w, h) in [
            (SlideSize::Wide169, "960", "540"),
            (SlideSize::Standard43, "720", "540"),
            (SlideSize::A4Landscape, "842", "595"),
            (SlideSize::LetterLandscape, "792", "612"),
        ] {
            let pdf = to_pdf("# S\n\nbody", &DeckOptions { size, ..opts() }).unwrap();
            let doc = Document::load_mem(&pdf).unwrap();
            let (page_id, _) = doc.page_iter().next().map(|id| (id, ())).unwrap();
            let mb = doc
                .get_dictionary(page_id)
                .unwrap()
                .get(b"MediaBox")
                .unwrap()
                .as_array()
                .unwrap();
            let got_w = format!("{:?}", mb[2]);
            let got_h = format!("{:?}", mb[3]);
            assert!(got_w.contains(w), "{size:?} width {got_w} should contain {w}");
            assert!(got_h.contains(h), "{size:?} height {got_h} should contain {h}");
        }
    }

    #[test]
    fn dark_theme_paints_a_dark_background() {
        let light = to_pdf("# S\n\nbody", &opts()).unwrap();
        let dark = to_pdf("# S\n\nbody", &DeckOptions { theme: Theme::Dark, ..opts() }).unwrap();
        assert_ne!(light, dark, "themes must produce different bytes");
        assert_eq!(&dark[..5], b"%PDF-");
    }

    #[test]
    fn outline_adds_bookmarks_and_can_be_turned_off() {
        let md = "# One\n\na\n\n# Two\n\nb";
        let with = to_pdf(md, &opts()).unwrap();
        let without = to_pdf(md, &DeckOptions { outline: false, ..opts() }).unwrap();
        let doc = Document::load_mem(&with).unwrap();
        let root = doc.catalog().unwrap();
        assert!(root.get(b"Outlines").is_ok(), "outline on → /Outlines in catalog");
        let doc2 = Document::load_mem(&without).unwrap();
        assert!(doc2.catalog().unwrap().get(b"Outlines").is_err(), "outline off → none");
    }

    #[test]
    fn header_footer_and_page_numbers_change_the_output() {
        let md = "# One\n\na\n\n# Two\n\nb";
        let plain = to_pdf(md, &DeckOptions { page_numbers: false, ..opts() }).unwrap();
        let chrome = to_pdf(
            md,
            &DeckOptions { header: "Acme", footer: "Confidential", ..opts() },
        )
        .unwrap();
        assert!(chrome.len() > plain.len(), "header/footer/numbers add bytes");
    }

    #[test]
    fn long_slide_overflows_onto_extra_pages() {
        let body: String = (0..200).map(|i| format!("- bullet number {i}\n")).collect();
        let md = format!("# Big\n\n{body}");
        let (_, slides, pages) = to_pdf_with_counts(&md, &opts()).unwrap();
        assert_eq!(slides, 1);
        assert!(pages > 1, "an overflowing slide continues onto extra pages, got {pages}");
    }

    #[test]
    fn shrink_to_fit_keeps_a_medium_slide_on_one_page() {
        let body: String = (0..14).map(|i| format!("- bullet number {i}\n")).collect();
        let md = format!("# Medium\n\n{body}");
        let (_, _, pages) = to_pdf_with_counts(&md, &opts()).unwrap();
        assert_eq!(pages, 1, "shrink-to-fit should keep 14 bullets on one slide");
    }

    #[test]
    fn renders_rich_markdown_without_panicking() {
        let md = "# Rich\n\n**bold** *italic* `code` ~~strike~~ [link](https://example.com)\n\n\
                  1. one\n2. two\n   - nested\n\n```rust\nlet x = 1;\n```\n\n\
                  > quote\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n![alt text](x.png)\n";
        let pdf = to_pdf(md, &opts()).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn font_size_is_validated() {
        assert!(to_pdf("# a", &DeckOptions { font_size: 4.0, ..opts() }).is_err());
        assert!(to_pdf("# a", &DeckOptions { font_size: 60.0, ..opts() }).is_err());
        assert!(to_pdf("# a", &DeckOptions { font_size: 8.0, ..opts() }).is_ok());
        assert!(to_pdf("# a", &DeckOptions { font_size: 48.0, ..opts() }).is_ok());
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = to_pdf("   \n\n", &opts()).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn slide_cap_is_enforced_at_the_boundary() {
        let at: String = (0..MAX_SLIDES).map(|i| format!("# Slide {i}\n\ntext\n\n")).collect();
        let (_, slides, _) = to_pdf_with_counts(&at, &opts()).unwrap();
        assert_eq!(slides, MAX_SLIDES);

        let over: String =
            (0..MAX_SLIDES + 1).map(|i| format!("# Slide {i}\n\ntext\n\n")).collect();
        let err = to_pdf(&over, &opts()).unwrap_err();
        assert!(err.contains("over the"), "got: {err}");
    }

    #[test]
    fn parsers_accept_aliases_and_reject_junk() {
        assert_eq!(SplitLevel::parse("H2").unwrap(), SplitLevel::H2);
        assert_eq!(SplitLevel::parse("").unwrap(), SplitLevel::H1);
        assert!(SplitLevel::parse("h7").is_err());
        assert_eq!(SlideSize::parse("wide").unwrap(), SlideSize::Wide169);
        assert_eq!(SlideSize::parse("A4-Landscape").unwrap(), SlideSize::A4Landscape);
        assert!(SlideSize::parse("tabloid").is_err());
        assert_eq!(Theme::parse("Dark").unwrap(), Theme::Dark);
        assert!(Theme::parse("solarized").is_err());
    }

    #[test]
    fn non_latin1_text_folds_rather_than_failing() {
        let pdf = to_pdf("# Résumé — 日本語\n\ncafé", &opts()).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
    }

    #[test]
    fn fenced_rules_do_not_split_slides() {
        let md = "# Code\n\n```\n---\n***\n```\n";
        let (_, slides, _) = to_pdf_with_counts(md, &opts()).unwrap();
        assert_eq!(slides, 1, "a --- inside a fence is code, not a slide break");
    }
}
