//! markdown-to-docx core — turn Markdown text into a real, binary Microsoft
//! Word `.docx` document (an Office Open XML ZIP of XML parts). Pure logic
//! shared by the chat skill block, the CLI and the web page — no wafer /
//! wasm-bindgen deps.
//!
//! Headings become Word heading styles, paragraphs carry inline **bold**,
//! *italic*, `code` and ~~strikethrough~~, `-`/`1.` become native bullet /
//! numbered lists, `> ` becomes a block quote, fenced ``` blocks become
//! monospace shaded code, and a `---` thematic break becomes a horizontal rule
//! (or a page break when `page_break` is on). The output is a genuine `.docx`
//! package — a `[Content_Types].xml`, package + document relationships,
//! `word/document.xml`, `word/styles.xml`, `word/numbering.xml` and core / app
//! properties — not a renamed text file, so Word, Google Docs, Pages and
//! LibreOffice Writer all open it natively.

use std::io::{Cursor, Write};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Printed page size, driving the section `<w:pgSz>` (in twips, 1/1440").
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageSize {
    /// US Letter, 8.5" × 11" (12240 × 15840 twips).
    Letter,
    /// ISO A4, 210mm × 297mm (11906 × 16838 twips).
    A4,
}

impl PageSize {
    /// Parse a page-size name (canonical + common aliases).
    pub fn parse(s: &str) -> Result<PageSize, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "letter" | "us" | "us-letter" => Ok(PageSize::Letter),
            "a4" => Ok(PageSize::A4),
            other => Err(format!("unknown page_size '{other}' (use letter or a4)")),
        }
    }

    /// `(width, height)` of the page in twips.
    fn twips(self) -> (u32, u32) {
        match self {
            PageSize::Letter => (12240, 15840),
            PageSize::A4 => (11906, 16838),
        }
    }
}

/// Body font family, written into `styles.xml`'s default run properties.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontFamily {
    Calibri,
    Aptos,
    TimesNewRoman,
    Arial,
}

impl FontFamily {
    /// Parse a font-family name (canonical + common aliases).
    pub fn parse(s: &str) -> Result<FontFamily, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "calibri" => Ok(FontFamily::Calibri),
            "aptos" => Ok(FontFamily::Aptos),
            "times_new_roman" | "times new roman" | "times" | "serif" => {
                Ok(FontFamily::TimesNewRoman)
            }
            "arial" => Ok(FontFamily::Arial),
            other => Err(format!(
                "unknown font_family '{other}' (use calibri, aptos, times_new_roman, or arial)"
            )),
        }
    }

    /// The exact typeface name Word expects in `<w:rFonts>`.
    fn typeface(self) -> &'static str {
        match self {
            FontFamily::Calibri => "Calibri",
            FontFamily::Aptos => "Aptos",
            FontFamily::TimesNewRoman => "Times New Roman",
            FontFamily::Arial => "Arial",
        }
    }
}

/// Smallest / largest body font size (points) the tool accepts.
pub const MIN_FONT_SIZE: u32 = 8;
pub const MAX_FONT_SIZE: u32 = 24;

/// Numbering `numId` shared by every bullet list (bullets need no per-list
/// restart). Ordered lists get their own ids starting at [`ORDERED_NUM_BASE`].
const BULLET_NUM_ID: u32 = 1;
/// First `numId` handed out to an ordered list run; each contiguous ordered
/// list gets the next id so its counter restarts at 1.
const ORDERED_NUM_BASE: u32 = 10;

/// One inline run: a slice of text plus the emphasis flags active over it.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
struct Seg {
    text: String,
    bold: bool,
    italic: bool,
    code: bool,
    strike: bool,
}

/// A parsed block-level element.
enum Block {
    /// The optional leading document title (from the `title` param).
    Title(String),
    /// An ATX heading of `level` 1..=6.
    Heading(u8, Vec<Seg>),
    /// A body paragraph.
    Para(Vec<Seg>),
    /// A list item: `ordered` picks bullet vs number, `level` is the indent
    /// depth (0-based), `num_id` its numbering instance.
    ListItem {
        ordered: bool,
        level: u8,
        num_id: u32,
        segs: Vec<Seg>,
    },
    /// A `> ` block quote line.
    Quote(Vec<Seg>),
    /// A fenced code block, one string per line (kept verbatim).
    Code(Vec<String>),
    /// A `---` thematic break — a horizontal rule, or a page break when the
    /// `page_break` option is on.
    Rule,
}

/// A thematic break: a line of 3+ of `-`, `*`, or `_` (optionally spaced).
fn is_thematic_break(line: &str) -> bool {
    let t = line.trim();
    for m in ['-', '*', '_'] {
        let stripped: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        if stripped.len() >= 3 && stripped.chars().all(|c| c == m) {
            return true;
        }
    }
    false
}

/// If `line` is an ATX heading (`#`…`######`), return `(level, text)`.
fn heading(line: &str) -> Option<(u8, &str)> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    Some((hashes as u8, rest.trim()))
}

/// If `line` is a list item, return `(ordered, level, text)`; the level comes
/// from the leading indentation (every 2 spaces / tab = one level, capped at 8).
fn list_item(line: &str) -> Option<(bool, u8, &str)> {
    let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
    let t = &line[indent..];
    let level = (indent / 2).min(8) as u8;
    if let Some(rest) = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("* "))
        .or_else(|| t.strip_prefix("+ "))
    {
        return Some((false, level, rest.trim()));
    }
    if let Some(rest) = ordered_marker(t) {
        return Some((true, level, rest.trim()));
    }
    None
}

/// Strip a leading ordered-list marker like `1.` / `12)` and return the rest.
fn ordered_marker(t: &str) -> Option<&str> {
    let digits = t.chars().take_while(char::is_ascii_digit).count();
    if digits == 0 {
        return None;
    }
    let after = &t[digits..];
    after.strip_prefix(". ").or_else(|| after.strip_prefix(") "))
}

/// If `line` is a `> ` block quote, return the quoted text.
fn block_quote(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let rest = t.strip_prefix('>')?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

/// Replace every `[label](target)` with just `label` (links render as their
/// visible text — this is a pure, offline block, so no clickable relationships).
fn unwrap_links(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        if bytes[i] == b'[' {
            if let Some(close) = s[i + 1..].find(']') {
                let label_end = i + 1 + close;
                if s.as_bytes().get(label_end + 1) == Some(&b'(') {
                    if let Some(paren) = s[label_end + 2..].find(')') {
                        out.push_str(&s[i + 1..label_end]);
                        i = label_end + 2 + paren + 1;
                        continue;
                    }
                }
            }
        }
        let ch_len = s[i..].chars().next().map(char::len_utf8).unwrap_or(1);
        out.push_str(&s[i..i + ch_len]);
        i += ch_len;
    }
    out
}

/// Tokenize a line of inline Markdown into styled [`Seg`]s. Handles `` `code` ``
/// spans (verbatim, no nested marks), `**`/`__` bold, `*`/`_` italic and `~~`
/// strikethrough as toggles. Links are flattened to their label first.
fn parse_inline(line: &str) -> Vec<Seg> {
    let s = unwrap_links(line);
    let chars: Vec<char> = s.chars().collect();
    let mut segs: Vec<Seg> = Vec::new();
    let (mut bold, mut italic, mut strike) = (false, false, false);
    let mut buf = String::new();

    let flush = |buf: &mut String, segs: &mut Vec<Seg>, b: bool, i: bool, st: bool| {
        if !buf.is_empty() {
            segs.push(Seg {
                text: std::mem::take(buf),
                bold: b,
                italic: i,
                code: false,
                strike: st,
            });
        }
    };

    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        match c {
            '`' => {
                // Inline code span: consume up to the next backtick verbatim.
                flush(&mut buf, &mut segs, bold, italic, strike);
                let mut code = String::new();
                let mut j = k + 1;
                let mut closed = false;
                while j < chars.len() {
                    if chars[j] == '`' {
                        closed = true;
                        break;
                    }
                    code.push(chars[j]);
                    j += 1;
                }
                if closed {
                    segs.push(Seg {
                        text: code,
                        code: true,
                        ..Default::default()
                    });
                    k = j + 1;
                } else {
                    // No closing backtick — treat as a literal character.
                    buf.push('`');
                    k += 1;
                }
            }
            '*' | '_' => {
                let double = k + 1 < chars.len() && chars[k + 1] == c;
                flush(&mut buf, &mut segs, bold, italic, strike);
                if double {
                    bold = !bold;
                    k += 2;
                } else {
                    italic = !italic;
                    k += 1;
                }
            }
            '~' if k + 1 < chars.len() && chars[k + 1] == '~' => {
                flush(&mut buf, &mut segs, bold, italic, strike);
                strike = !strike;
                k += 2;
            }
            _ => {
                buf.push(c);
                k += 1;
            }
        }
    }
    flush(&mut buf, &mut segs, bold, italic, strike);
    if segs.is_empty() {
        segs.push(Seg::default());
    }
    segs
}

/// Parse the whole Markdown document into block elements, assigning a fresh
/// numbering id to each contiguous ordered-list run so its numbers restart.
fn parse_blocks(markdown: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut in_code = false;
    let mut code_lines: Vec<String> = Vec::new();
    // Ordered-list run tracking: `run_ordered_id` is the numId for the current
    // run of list items; `in_list` marks whether the previous non-blank line
    // was a list item (a blank line alone does not end the run).
    let mut in_list = false;
    let mut run_ordered_id: u32 = ORDERED_NUM_BASE;
    let mut next_ordered_id: u32 = ORDERED_NUM_BASE;

    for raw in markdown.lines() {
        if raw.trim_start().starts_with("```") {
            if in_code {
                blocks.push(Block::Code(std::mem::take(&mut code_lines)));
                in_code = false;
            } else {
                in_code = true;
                in_list = false;
            }
            continue;
        }
        if in_code {
            code_lines.push(raw.to_string());
            continue;
        }

        if raw.trim().is_empty() {
            // A blank line ends paragraphs but is tolerated inside a list run.
            continue;
        }

        if is_thematic_break(raw) {
            blocks.push(Block::Rule);
            in_list = false;
            continue;
        }
        if let Some((level, text)) = heading(raw) {
            blocks.push(Block::Heading(level, parse_inline(text)));
            in_list = false;
            continue;
        }
        if let Some((ordered, level, text)) = list_item(raw) {
            if !in_list {
                // Start of a new list run — reserve a fresh ordered numId.
                run_ordered_id = next_ordered_id;
                next_ordered_id += 1;
                in_list = true;
            }
            let num_id = if ordered { run_ordered_id } else { BULLET_NUM_ID };
            blocks.push(Block::ListItem {
                ordered,
                level,
                num_id,
                segs: parse_inline(text),
            });
            continue;
        }
        if let Some(text) = block_quote(raw) {
            blocks.push(Block::Quote(parse_inline(text)));
            in_list = false;
            continue;
        }
        blocks.push(Block::Para(parse_inline(raw.trim())));
        in_list = false;
    }
    if in_code && !code_lines.is_empty() {
        blocks.push(Block::Code(code_lines));
    }
    blocks
}

/// Escape a string for XML text / attribute content.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c if (c as u32) < 0x20 && c != '\t' && c != '\n' && c != '\r' => {}
            c => out.push(c),
        }
    }
    out
}

/// Convert Markdown text into a real binary `.docx` package.
///
/// - `title` — optional document title. When non-empty it becomes a leading
///   Title-styled paragraph and the document's `dc:title`.
/// - `page_size` / `font_family` / `font_size` — the section page size and the
///   body font written into `styles.xml`. `font_size` is clamped to
///   [`MIN_FONT_SIZE`]..=[`MAX_FONT_SIZE`] points.
/// - `page_break` — when true a `---` thematic break emits a page break instead
///   of a horizontal rule.
///
/// Returns an error when there is nothing to write (empty markdown and title).
pub fn to_docx(
    markdown: &str,
    title: &str,
    page_size: PageSize,
    font_family: FontFamily,
    font_size: u32,
    page_break: bool,
) -> Result<Vec<u8>, String> {
    to_docx_with_count(markdown, title, page_size, font_family, font_size, page_break)
        .map(|(b, _)| b)
}

/// Like [`to_docx`] but also returns the number of body blocks emitted so
/// callers can summarize the document without unzipping it.
pub fn to_docx_with_count(
    markdown: &str,
    title: &str,
    page_size: PageSize,
    font_family: FontFamily,
    font_size: u32,
    page_break: bool,
) -> Result<(Vec<u8>, usize), String> {
    let font_size = font_size.clamp(MIN_FONT_SIZE, MAX_FONT_SIZE);
    let mut blocks = parse_blocks(markdown);
    let title = title.trim();
    if !title.is_empty() {
        blocks.insert(0, Block::Title(title.to_string()));
    }
    if blocks.is_empty() {
        return Err(
            "input is empty — add a heading, a paragraph, or a title to produce a document"
                .to_string(),
        );
    }

    // Which ordered numbering ids were actually used (for numbering.xml).
    let mut ordered_ids: Vec<u32> = Vec::new();
    for b in &blocks {
        if let Block::ListItem {
            ordered: true,
            num_id,
            ..
        } = b
        {
            if !ordered_ids.contains(num_id) {
                ordered_ids.push(*num_id);
            }
        }
    }
    let block_count = blocks.len();

    let parts: Vec<(String, String)> = vec![
        ("[Content_Types].xml".to_string(), content_types()),
        ("_rels/.rels".to_string(), root_rels()),
        ("docProps/core.xml".to_string(), core_props(title)),
        ("docProps/app.xml".to_string(), app_props()),
        (
            "word/document.xml".to_string(),
            document_xml(&blocks, page_size, page_break),
        ),
        ("word/_rels/document.xml.rels".to_string(), document_rels()),
        (
            "word/styles.xml".to_string(),
            styles_xml(font_family, font_size),
        ),
        ("word/numbering.xml".to_string(), numbering_xml(&ordered_ids)),
    ];

    let bytes = build_zip(&parts)?;
    Ok((bytes, block_count))
}

/// Assemble the parts into a single ZIP (OOXML package). Deflate-compressed with
/// the crate default 1980 DOS timestamp, so it is deterministic and clock-free.
fn build_zip(parts: &[(String, String)]) -> Result<Vec<u8>, String> {
    let mut zw = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, content) in parts {
        zw.start_file(name.as_str(), opts)
            .map_err(|e| format!("zip error on {name}: {e}"))?;
        zw.write_all(content.as_bytes())
            .map_err(|e| format!("zip write error on {name}: {e}"))?;
    }
    let cursor = zw.finish().map_err(|e| format!("zip finalize error: {e}"))?;
    Ok(cursor.into_inner())
}

// ---------------------------------------------------------------------------
// OOXML part builders. Each returns a complete, standalone XML document.
// ---------------------------------------------------------------------------

const DECL: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\r\n";
const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
/// Monospace face used for inline code and fenced code blocks.
const MONO: &str = "Consolas";
/// Light-grey shading behind code, as a 6-hex RGB fill.
const CODE_FILL: &str = "F2F2F2";

fn content_types() -> String {
    format!(
        "{DECL}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
<Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
<Default Extension=\"xml\" ContentType=\"application/xml\"/>\
<Override PartName=\"/word/document.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml\"/>\
<Override PartName=\"/word/styles.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.styles+xml\"/>\
<Override PartName=\"/word/numbering.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.wordprocessingml.numbering+xml\"/>\
<Override PartName=\"/docProps/core.xml\" ContentType=\"application/vnd.openxmlformats-package.core-properties+xml\"/>\
<Override PartName=\"/docProps/app.xml\" ContentType=\"application/vnd.openxmlformats-officedocument.extended-properties+xml\"/>\
</Types>"
    )
}

fn root_rels() -> String {
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument\" Target=\"word/document.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties\" Target=\"docProps/core.xml\"/>\
<Relationship Id=\"rId3\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties\" Target=\"docProps/app.xml\"/>\
</Relationships>"
    )
}

fn document_rels() -> String {
    format!(
        "{DECL}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">\
<Relationship Id=\"rId1\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/styles\" Target=\"styles.xml\"/>\
<Relationship Id=\"rId2\" Type=\"http://schemas.openxmlformats.org/officeDocument/2006/relationships/numbering\" Target=\"numbering.xml\"/>\
</Relationships>"
    )
}

fn core_props(title: &str) -> String {
    format!(
        "{DECL}<cp:coreProperties xmlns:cp=\"http://schemas.openxmlformats.org/package/2006/metadata/core-properties\" xmlns:dc=\"http://purl.org/dc/elements/1.1/\" xmlns:dcterms=\"http://purl.org/dc/terms/\" xmlns:dcmitype=\"http://purl.org/dc/dcmitype/\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\">\
<dc:title>{}</dc:title>\
<dc:creator>gizza-ai/markdown-to-docx</dc:creator>\
</cp:coreProperties>",
        esc(title)
    )
}

fn app_props() -> String {
    format!(
        "{DECL}<Properties xmlns=\"http://schemas.openxmlformats.org/officeDocument/2006/extended-properties\">\
<Application>gizza-ai/markdown-to-docx</Application>\
</Properties>"
    )
}

/// Render the run properties (`<w:rPr>`) implied by a [`Seg`]'s emphasis flags.
fn seg_rpr(seg: &Seg) -> String {
    let mut rpr = String::new();
    if seg.code {
        rpr.push_str(&format!(
            "<w:rFonts w:ascii=\"{MONO}\" w:hAnsi=\"{MONO}\" w:cs=\"{MONO}\"/><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{CODE_FILL}\"/>"
        ));
    }
    if seg.bold {
        rpr.push_str("<w:b/>");
    }
    if seg.italic {
        rpr.push_str("<w:i/>");
    }
    if seg.strike {
        rpr.push_str("<w:strike/>");
    }
    rpr
}

/// Render one styled [`Seg`] as a `<w:r>` run.
fn seg_run(seg: &Seg) -> String {
    let rpr = seg_rpr(seg);
    let rpr_xml = if rpr.is_empty() {
        String::new()
    } else {
        format!("<w:rPr>{rpr}</w:rPr>")
    };
    format!(
        "<w:r>{rpr_xml}<w:t xml:space=\"preserve\">{}</w:t></w:r>",
        esc(&seg.text)
    )
}

/// Render a sequence of [`Seg`]s into concatenated runs.
fn runs(segs: &[Seg]) -> String {
    segs.iter().map(seg_run).collect()
}

/// A paragraph with an optional `<w:pPr>` body and pre-rendered runs.
fn paragraph(ppr: &str, runs_xml: &str) -> String {
    let ppr_xml = if ppr.is_empty() {
        String::new()
    } else {
        format!("<w:pPr>{ppr}</w:pPr>")
    };
    format!("<w:p>{ppr_xml}{runs_xml}</w:p>")
}

fn document_xml(blocks: &[Block], page_size: PageSize, page_break: bool) -> String {
    let mut body = String::new();
    for b in blocks {
        match b {
            Block::Title(t) => {
                body.push_str(&paragraph(
                    "<w:pStyle w:val=\"Title\"/>",
                    &seg_run(&Seg {
                        text: t.clone(),
                        ..Default::default()
                    }),
                ));
            }
            Block::Heading(level, segs) => {
                let ppr = format!("<w:pStyle w:val=\"Heading{level}\"/>");
                body.push_str(&paragraph(&ppr, &runs(segs)));
            }
            Block::Para(segs) => {
                body.push_str(&paragraph("", &runs(segs)));
            }
            Block::ListItem {
                level,
                num_id,
                segs,
                ..
            } => {
                let ppr = format!(
                    "<w:pStyle w:val=\"ListParagraph\"/><w:numPr><w:ilvl w:val=\"{level}\"/><w:numId w:val=\"{num_id}\"/></w:numPr>"
                );
                body.push_str(&paragraph(&ppr, &runs(segs)));
            }
            Block::Quote(segs) => {
                body.push_str(&paragraph("<w:pStyle w:val=\"Quote\"/>", &runs(segs)));
            }
            Block::Code(lines) => {
                for line in lines {
                    let run =
                        format!("<w:r><w:t xml:space=\"preserve\">{}</w:t></w:r>", esc(line));
                    body.push_str(&paragraph("<w:pStyle w:val=\"CodeBlock\"/>", &run));
                }
            }
            Block::Rule => {
                if page_break {
                    body.push_str("<w:p><w:r><w:br w:type=\"page\"/></w:r></w:p>");
                } else {
                    body.push_str(
                        "<w:p><w:pPr><w:pBdr><w:bottom w:val=\"single\" w:sz=\"6\" w:space=\"1\" w:color=\"auto\"/></w:pBdr></w:pPr></w:p>",
                    );
                }
            }
        }
    }

    let (w, h) = page_size.twips();
    let sect = format!(
        "<w:sectPr><w:pgSz w:w=\"{w}\" w:h=\"{h}\"/><w:pgMar w:top=\"1440\" w:right=\"1440\" w:bottom=\"1440\" w:left=\"1440\" w:header=\"720\" w:footer=\"720\" w:gutter=\"0\"/></w:sectPr>"
    );

    format!(
        "{DECL}<w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\"><w:body>{body}{sect}</w:body></w:document>"
    )
}

fn styles_xml(font: FontFamily, font_size: u32) -> String {
    let face = font.typeface();
    let sz = font_size * 2; // half-points

    // Heading sizes in half-points (level 1 largest → level 6 smallest).
    let heading_sz = [48u32, 36, 28, 26, 24, 22];
    let mut headings = String::new();
    for (i, hsz) in heading_sz.iter().enumerate() {
        let level = i + 1;
        let outline = i; // outlineLvl is 0-based
        headings.push_str(&format!(
            "<w:style w:type=\"paragraph\" w:styleId=\"Heading{level}\"><w:name w:val=\"heading {level}\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/>\
<w:pPr><w:keepNext/><w:keepLines/><w:spacing w:before=\"240\" w:after=\"80\"/><w:outlineLvl w:val=\"{outline}\"/></w:pPr>\
<w:rPr><w:b/><w:color w:val=\"2E4A6E\"/><w:sz w:val=\"{hsz}\"/><w:szCs w:val=\"{hsz}\"/></w:rPr></w:style>"
        ));
    }

    format!(
        "{DECL}<w:styles xmlns:w=\"{NS_W}\">\
<w:docDefaults>\
<w:rPrDefault><w:rPr><w:rFonts w:ascii=\"{face}\" w:hAnsi=\"{face}\" w:cs=\"{face}\"/><w:sz w:val=\"{sz}\"/><w:szCs w:val=\"{sz}\"/></w:rPr></w:rPrDefault>\
<w:pPrDefault><w:pPr><w:spacing w:after=\"160\" w:line=\"259\" w:lineRule=\"auto\"/></w:pPr></w:pPrDefault>\
</w:docDefaults>\
<w:style w:type=\"paragraph\" w:default=\"1\" w:styleId=\"Normal\"><w:name w:val=\"Normal\"/><w:qFormat/></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Title\"><w:name w:val=\"Title\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/>\
<w:pPr><w:spacing w:after=\"120\"/></w:pPr>\
<w:rPr><w:b/><w:color w:val=\"1A1A1A\"/><w:sz w:val=\"56\"/><w:szCs w:val=\"56\"/></w:rPr></w:style>\
{headings}\
<w:style w:type=\"paragraph\" w:styleId=\"ListParagraph\"><w:name w:val=\"List Paragraph\"/><w:basedOn w:val=\"Normal\"/><w:qFormat/><w:pPr><w:contextualSpacing/></w:pPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"Quote\"><w:name w:val=\"Quote\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/><w:qFormat/>\
<w:pPr><w:ind w:left=\"720\"/><w:pBdr><w:left w:val=\"single\" w:sz=\"18\" w:space=\"8\" w:color=\"CCCCCC\"/></w:pBdr></w:pPr>\
<w:rPr><w:i/><w:color w:val=\"555555\"/></w:rPr></w:style>\
<w:style w:type=\"paragraph\" w:styleId=\"CodeBlock\"><w:name w:val=\"Code Block\"/><w:basedOn w:val=\"Normal\"/><w:next w:val=\"Normal\"/>\
<w:pPr><w:spacing w:after=\"0\" w:line=\"240\" w:lineRule=\"auto\"/><w:contextualSpacing/><w:shd w:val=\"clear\" w:color=\"auto\" w:fill=\"{CODE_FILL}\"/></w:pPr>\
<w:rPr><w:rFonts w:ascii=\"{MONO}\" w:hAnsi=\"{MONO}\" w:cs=\"{MONO}\"/><w:sz w:val=\"20\"/><w:szCs w:val=\"20\"/></w:rPr></w:style>\
</w:styles>"
    )
}

/// A single abstractNum's nine level definitions. `bullet` picks bullet glyphs
/// vs `%n.` decimal formats; `id` is the abstractNumId.
fn abstract_num(id: u32, bullet: bool) -> String {
    let bullets = ['\u{2022}', '\u{25E6}', '\u{25AA}'];
    let mut lvls = String::new();
    for ilvl in 0..9u32 {
        let left = (ilvl + 1) * 720;
        let (num_fmt, lvl_text) = if bullet {
            let glyph = bullets[(ilvl as usize) % bullets.len()];
            ("bullet".to_string(), glyph.to_string())
        } else {
            ("decimal".to_string(), format!("%{}.", ilvl + 1))
        };
        lvls.push_str(&format!(
            "<w:lvl w:ilvl=\"{ilvl}\"><w:start w:val=\"1\"/><w:numFmt w:val=\"{num_fmt}\"/><w:lvlText w:val=\"{}\"/><w:lvlJc w:val=\"left\"/>\
<w:pPr><w:ind w:left=\"{left}\" w:hanging=\"360\"/></w:pPr></w:lvl>",
            esc(&lvl_text)
        ));
    }
    format!("<w:abstractNum w:abstractNumId=\"{id}\">{lvls}</w:abstractNum>")
}

fn numbering_xml(ordered_ids: &[u32]) -> String {
    // abstractNum 0 = bullets, abstractNum 1 = decimal.
    let mut body = String::new();
    body.push_str(&abstract_num(0, true));
    body.push_str(&abstract_num(1, false));
    // The shared bullet list instance.
    body.push_str(&format!(
        "<w:num w:numId=\"{BULLET_NUM_ID}\"><w:abstractNumId w:val=\"0\"/></w:num>"
    ));
    // One numbering instance per ordered-list run (each restarts at 1).
    for id in ordered_ids {
        body.push_str(&format!(
            "<w:num w:numId=\"{id}\"><w:abstractNumId w:val=\"1\"/></w:num>"
        ));
    }
    format!("{DECL}<w:numbering xmlns:w=\"{NS_W}\">{body}</w:numbering>")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Read;
    use zip::ZipArchive;

    fn unzip(bytes: &[u8]) -> (Vec<String>, HashMap<String, String>) {
        let mut archive = ZipArchive::new(Cursor::new(bytes.to_vec())).expect("valid zip");
        let mut names = Vec::new();
        let mut map = HashMap::new();
        for i in 0..archive.len() {
            let mut f = archive.by_index(i).unwrap();
            let name = f.name().to_string();
            let mut s = String::new();
            f.read_to_string(&mut s).unwrap();
            names.push(name.clone());
            map.insert(name, s);
        }
        names.sort();
        (names, map)
    }

    fn build(md: &str) -> Vec<u8> {
        to_docx(md, "", PageSize::Letter, FontFamily::Calibri, 11, false).unwrap()
    }

    #[test]
    fn produces_a_real_zip_with_required_docx_parts() {
        let bytes = build("# Title\n\nHello world.");
        assert_eq!(&bytes[0..4], &[0x50, 0x4b, 0x03, 0x04]);
        let (names, _) = unzip(&bytes);
        for required in [
            "[Content_Types].xml",
            "_rels/.rels",
            "word/document.xml",
            "word/_rels/document.xml.rels",
            "word/styles.xml",
            "word/numbering.xml",
            "docProps/core.xml",
            "docProps/app.xml",
        ] {
            assert!(
                names.contains(&required.to_string()),
                "missing part {required}; got {names:?}"
            );
        }
    }

    #[test]
    fn headings_map_to_word_heading_styles() {
        let bytes = build("# Big\n\n## Small\n\n###### Tiny");
        let (_, map) = unzip(&bytes);
        let doc = &map["word/document.xml"];
        assert!(doc.contains("w:val=\"Heading1\""));
        assert!(doc.contains("w:val=\"Heading2\""));
        assert!(doc.contains("w:val=\"Heading6\""));
        assert!(doc.contains("<w:t xml:space=\"preserve\">Big</w:t>"));
    }

    #[test]
    fn inline_bold_italic_code_strike_render_runs() {
        let bytes = build("This is **bold** and *italic* and `code` and ~~gone~~.");
        let (_, map) = unzip(&bytes);
        let doc = &map["word/document.xml"];
        assert!(doc.contains("<w:b/>"), "bold run missing: {doc}");
        assert!(doc.contains("<w:i/>"), "italic run missing");
        assert!(doc.contains("<w:strike/>"), "strike run missing");
        assert!(
            doc.contains(&format!("w:ascii=\"{MONO}\"")),
            "code font missing"
        );
        assert!(doc.contains("<w:t xml:space=\"preserve\">bold</w:t>"));
        assert!(doc.contains("<w:t xml:space=\"preserve\">code</w:t>"));
    }

    #[test]
    fn special_characters_are_xml_escaped() {
        let bytes = build("A & B <tag> \"quoted\"");
        let (_, map) = unzip(&bytes);
        let doc = &map["word/document.xml"];
        assert!(
            doc.contains("A &amp; B &lt;tag&gt; &quot;quoted&quot;"),
            "not escaped: {doc}"
        );
        assert!(!doc.contains("<tag>"));
    }

    #[test]
    fn bullet_and_ordered_lists_use_numbering() {
        let bytes = build("- one\n- two\n\n1. first\n2. second");
        let (_, map) = unzip(&bytes);
        let doc = &map["word/document.xml"];
        assert!(doc.contains("<w:numId w:val=\"1\"/>"), "bullet numId missing");
        assert!(
            doc.contains(&format!("<w:numId w:val=\"{ORDERED_NUM_BASE}\"/>")),
            "ordered numId missing"
        );
        let num = &map["word/numbering.xml"];
        assert!(num.contains("w:val=\"bullet\""));
        assert!(num.contains("w:val=\"decimal\""));
        assert!(num.contains(&format!("<w:num w:numId=\"{ORDERED_NUM_BASE}\">")));
    }

    #[test]
    fn separate_ordered_lists_get_fresh_numbering_ids() {
        let bytes = build("1. a\n2. b\n\ntext\n\n1. c\n2. d");
        let (_, map) = unzip(&bytes);
        let doc = &map["word/document.xml"];
        // Two distinct ordered runs → two numIds (base and base+1).
        assert!(doc.contains(&format!("<w:numId w:val=\"{ORDERED_NUM_BASE}\"/>")));
        assert!(doc.contains(&format!("<w:numId w:val=\"{}\"/>", ORDERED_NUM_BASE + 1)));
    }

    #[test]
    fn blockquote_and_code_block_styles() {
        let bytes = build("> quoted line\n\n```\nlet x = 1;\n```");
        let (_, map) = unzip(&bytes);
        let doc = &map["word/document.xml"];
        assert!(doc.contains("w:val=\"Quote\""));
        assert!(doc.contains("w:val=\"CodeBlock\""));
        assert!(doc.contains("<w:t xml:space=\"preserve\">let x = 1;</w:t>"));
        // A `#` inside a fence is verbatim text, not a heading.
        let bytes2 = build("```\n# not a heading\n```");
        let (_, map2) = unzip(&bytes2);
        assert!(!map2["word/document.xml"].contains("Heading1"));
    }

    #[test]
    fn thematic_break_is_rule_or_page_break() {
        let (_, map) = unzip(&build("a\n\n---\n\nb"));
        assert!(map["word/document.xml"].contains("<w:pBdr>"), "rule missing");
        let bytes = to_docx(
            "a\n\n---\n\nb",
            "",
            PageSize::Letter,
            FontFamily::Calibri,
            11,
            true,
        )
        .unwrap();
        let (_, map2) = unzip(&bytes);
        assert!(
            map2["word/document.xml"].contains("<w:br w:type=\"page\"/>"),
            "page break missing"
        );
    }

    #[test]
    fn title_param_prepends_title_paragraph_and_metadata() {
        let bytes = to_docx(
            "Body text",
            "My Doc",
            PageSize::Letter,
            FontFamily::Calibri,
            11,
            false,
        )
        .unwrap();
        let (_, map) = unzip(&bytes);
        assert!(map["word/document.xml"].contains("w:val=\"Title\""));
        assert!(map["word/document.xml"].contains("<w:t xml:space=\"preserve\">My Doc</w:t>"));
        assert!(map["docProps/core.xml"].contains("<dc:title>My Doc</dc:title>"));
    }

    #[test]
    fn page_size_font_family_and_size_are_applied() {
        let bytes = to_docx(
            "Hi",
            "",
            PageSize::A4,
            FontFamily::TimesNewRoman,
            14,
            false,
        )
        .unwrap();
        let (_, map) = unzip(&bytes);
        assert!(
            map["word/document.xml"].contains("w:w=\"11906\""),
            "A4 width missing"
        );
        assert!(
            map["word/document.xml"].contains("w:h=\"16838\""),
            "A4 height missing"
        );
        let styles = &map["word/styles.xml"];
        assert!(
            styles.contains("w:ascii=\"Times New Roman\""),
            "font family missing"
        );
        assert!(
            styles.contains("<w:sz w:val=\"28\"/>"),
            "14pt (sz 28) body missing: {styles}"
        );
    }

    #[test]
    fn letter_default_dimensions() {
        let (_, map) = unzip(&build("Hi"));
        assert!(map["word/document.xml"].contains("w:w=\"12240\""));
        assert!(map["word/document.xml"].contains("w:h=\"15840\""));
    }

    #[test]
    fn font_size_is_clamped_to_range() {
        let bytes = to_docx("Hi", "", PageSize::Letter, FontFamily::Calibri, 99, false).unwrap();
        let (_, map) = unzip(&bytes);
        // 24pt max → sz 48.
        assert!(map["word/styles.xml"].contains("<w:sz w:val=\"48\"/>"));
    }

    #[test]
    fn links_flatten_to_their_label() {
        let (_, map) = unzip(&build("See [the docs](https://example.com) now"));
        let doc = &map["word/document.xml"];
        assert!(doc.contains("the docs"), "link label missing: {doc}");
        assert!(!doc.contains("https://example.com"), "url should not appear");
    }

    #[test]
    fn empty_input_errors() {
        let err = to_docx("   \n\n", "", PageSize::Letter, FontFamily::Calibri, 11, false)
            .unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(PageSize::parse("A4").unwrap(), PageSize::A4);
        assert!(PageSize::parse("legal").is_err());
        assert_eq!(
            FontFamily::parse("times_new_roman").unwrap(),
            FontFamily::TimesNewRoman
        );
        assert!(FontFamily::parse("comic sans").is_err());
    }
}
