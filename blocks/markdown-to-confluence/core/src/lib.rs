//! markdown-to-confluence core — convert a Markdown document into Confluence
//! markup.
//!
//! Pure compute, no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page. A hand-rolled, line-oriented Markdown parser (a CommonMark-ish
//! subset plus a few GitHub-flavored extensions) that builds a small block/inline
//! AST and renders it either as Confluence **storage format** (the XHTML-based
//! markup the Cloud REST API consumes, with `<ac:…>` structured macros for code
//! and info/note/warning/tip panels) or **wiki markup** (the legacy Data
//! Center / Server "Insert markup" dialect).
//!
//! Supported block constructs: ATX headings (`#`..`######`, demotable via
//! `heading_offset`), unordered/ordered lists with nesting, fenced code blocks
//! (with an optional language tag), GitHub pipe tables, blockquotes,
//! Note/Warning/Info/Tip panel blockquotes, thematic breaks, and paragraphs.
//! Inline: `**bold**`/`__bold__`, `*italic*`/`_italic_`, `~~strikethrough~~`,
//! `` `code` ``, `[text](url)` links, and `![alt](src)` images. Output is
//! escaped so prose is safe by construction.

/// Which Confluence dialect to emit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    /// XHTML-based storage format (Cloud REST API canonical; carries macros).
    Storage,
    /// Legacy wiki markup (Data Center / Server "Insert markup").
    Wiki,
}

impl Format {
    /// Parse the `format` param. Blank is treated as the default (`storage`).
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "storage" => Ok(Format::Storage),
            "wiki" => Ok(Format::Wiki),
            other => Err(format!(
                "unknown format '{other}' (expected 'storage' or 'wiki')"
            )),
        }
    }
}

/// Convert `markdown` to Confluence markup.
///
/// - `format`: `"storage"` (default) or `"wiki"`.
/// - `panel_blockquotes`: when `true`, a blockquote whose first line starts
///   `Note:` / `Warning:` / `Info:` / `Tip:` (case-insensitive) becomes the
///   matching Confluence panel macro; when `false`, every blockquote stays a
///   literal quote.
/// - `heading_offset`: demote every heading by this many levels, clamped so the
///   result never exceeds h6.
///
/// Returns `Err` only when `markdown` is empty (after trimming) or `format` is
/// unrecognized — otherwise malformed Markdown degrades to literal text rather
/// than erroring, matching how Markdown renderers behave.
pub fn convert(
    markdown: &str,
    format: &str,
    panel_blockquotes: bool,
    heading_offset: u32,
) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("input is empty: paste some Markdown to convert".into());
    }
    let format = Format::parse(format)?;
    let offset = heading_offset.min(5) as usize;
    let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<String> = normalized.lines().map(|l| l.to_string()).collect();
    let blocks = parse_blocks(&lines, offset, panel_blockquotes);
    Ok(match format {
        Format::Storage => render_blocks_storage(&blocks),
        Format::Wiki => render_blocks_wiki(&blocks),
    })
}

/// Thin entry: storage format, panels on, no heading offset.
pub fn run(markdown: &str) -> Result<String, String> {
    convert(markdown, "storage", true, 0)
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum PanelKind {
    Info,
    Note,
    Warning,
    Tip,
}

impl PanelKind {
    /// Confluence macro name / wiki keyword (they coincide).
    fn keyword(self) -> &'static str {
        match self {
            PanelKind::Info => "info",
            PanelKind::Note => "note",
            PanelKind::Warning => "warning",
            PanelKind::Tip => "tip",
        }
    }
}

#[derive(Debug)]
enum Block {
    Heading(usize, Vec<Inline>),
    Paragraph(Vec<Inline>),
    Code(Option<String>, String),
    List(List),
    Table(Vec<Vec<Inline>>, Vec<Vec<Vec<Inline>>>),
    Quote(Vec<Block>),
    Panel(PanelKind, Vec<Block>),
    ThematicBreak,
}

#[derive(Debug)]
struct List {
    ordered: bool,
    items: Vec<ListItem>,
}

#[derive(Debug)]
struct ListItem {
    content: Vec<Inline>,
    sublist: Option<List>,
}

#[derive(Debug)]
enum Inline {
    Text(String),
    Bold(Vec<Inline>),
    Italic(Vec<Inline>),
    Strike(Vec<Inline>),
    Code(String),
    Link(Vec<Inline>, String),
    Image(String, String),
}

// ---------------------------------------------------------------------------
// Block parser
// ---------------------------------------------------------------------------

fn parse_blocks(lines: &[String], offset: usize, panels: bool) -> Vec<Block> {
    let mut blocks = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = &lines[i];
        if line.trim().is_empty() {
            i += 1;
            continue;
        }

        // Fenced code block.
        if let Some((fence_ch, fence_n, lang)) = fence_open(line) {
            i += 1;
            let mut body: Vec<String> = Vec::new();
            while i < lines.len() && !is_fence_close(&lines[i], fence_ch, fence_n) {
                body.push(lines[i].clone());
                i += 1;
            }
            if i < lines.len() {
                i += 1; // consume the closing fence
            }
            blocks.push(Block::Code(lang, body.join("\n")));
            continue;
        }

        // ATX heading.
        if let Some((level, text)) = atx_heading(line) {
            let level = (level + offset).min(6);
            blocks.push(Block::Heading(level, parse_inlines(&text)));
            i += 1;
            continue;
        }

        // Thematic break.
        if is_thematic_break(line) {
            blocks.push(Block::ThematicBreak);
            i += 1;
            continue;
        }

        // Blockquote (possibly a panel).
        if line.trim_start().starts_with('>') {
            let mut inner: Vec<String> = Vec::new();
            while i < lines.len() && lines[i].trim_start().starts_with('>') {
                inner.push(strip_quote_marker(&lines[i]));
                i += 1;
            }
            let panel = if panels { detect_panel(&mut inner) } else { None };
            let inner_blocks = parse_blocks(&inner, offset, panels);
            match panel {
                Some(kind) => blocks.push(Block::Panel(kind, inner_blocks)),
                None => blocks.push(Block::Quote(inner_blocks)),
            }
            continue;
        }

        // Pipe table (header row + delimiter row).
        if line.contains('|') && i + 1 < lines.len() && is_delimiter_row(&lines[i + 1]) {
            let header: Vec<Vec<Inline>> = split_cells(line)
                .iter()
                .map(|c| parse_inlines(c))
                .collect();
            i += 2;
            let mut rows: Vec<Vec<Vec<Inline>>> = Vec::new();
            while i < lines.len() && lines[i].contains('|') && !lines[i].trim().is_empty() {
                rows.push(split_cells(&lines[i]).iter().map(|c| parse_inlines(c)).collect());
                i += 1;
            }
            blocks.push(Block::Table(header, rows));
            continue;
        }

        // List.
        if list_marker(line).is_some() {
            let list = parse_list(lines, &mut i, indent_width(line));
            blocks.push(Block::List(list));
            continue;
        }

        // Paragraph: collect until a blank line or the start of another block.
        let mut para: Vec<String> = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() && !is_block_start(lines, i, panels) {
            para.push(lines[i].trim().to_string());
            i += 1;
        }
        if para.is_empty() {
            // Defensive: a line that looked like a block-start but matched
            // nothing above — emit it as a one-line paragraph to guarantee
            // progress.
            para.push(lines[i].trim().to_string());
            i += 1;
        }
        blocks.push(Block::Paragraph(parse_inlines(&para.join(" "))));
    }
    blocks
}

/// True if line `i` begins a non-paragraph block (used to end a paragraph).
fn is_block_start(lines: &[String], i: usize, panels: bool) -> bool {
    let line = &lines[i];
    let _ = panels;
    fence_open(line).is_some()
        || atx_heading(line).is_some()
        || is_thematic_break(line)
        || line.trim_start().starts_with('>')
        || list_marker(line).is_some()
        || (line.contains('|') && i + 1 < lines.len() && is_delimiter_row(&lines[i + 1]))
}

fn parse_list(lines: &[String], i: &mut usize, level_indent: usize) -> List {
    let ordered = list_marker(&lines[*i]).map(|m| m.0).unwrap_or(false);
    let mut items: Vec<ListItem> = Vec::new();
    while *i < lines.len() {
        let line = &lines[*i];
        if line.trim().is_empty() {
            break;
        }
        let indent = indent_width(line);
        let marker = match list_marker(line) {
            Some(m) => m,
            None => break,
        };
        if indent < level_indent || indent >= level_indent + 2 {
            break;
        }
        let mut item = ListItem {
            content: parse_inlines(marker.1),
            sublist: None,
        };
        *i += 1;
        // A nested list is a following list item indented at least 2 further.
        if *i < lines.len() && !lines[*i].trim().is_empty() {
            let nindent = indent_width(&lines[*i]);
            if list_marker(&lines[*i]).is_some() && nindent >= indent + 2 {
                item.sublist = Some(parse_list(lines, i, nindent));
            }
        }
        items.push(item);
    }
    List { ordered, items }
}

// ---------------------------------------------------------------------------
// Line classifiers
// ---------------------------------------------------------------------------

/// Leading-whitespace width in columns (tab counts as 4).
fn indent_width(line: &str) -> usize {
    let mut w = 0;
    for c in line.chars() {
        match c {
            ' ' => w += 1,
            '\t' => w += 4,
            _ => break,
        }
    }
    w
}

/// A list-item line → (ordered, content-after-marker).
fn list_marker(line: &str) -> Option<(bool, &str)> {
    let t = line.trim_start();
    for m in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(m) {
            return Some((false, rest));
        }
    }
    let bytes = t.as_bytes();
    let mut j = 0;
    while j < bytes.len() && bytes[j].is_ascii_digit() {
        j += 1;
    }
    if j > 0 && j < bytes.len() && (bytes[j] == b'.' || bytes[j] == b')') {
        let rest = &t[j + 1..];
        if let Some(r) = rest.strip_prefix(' ') {
            return Some((true, r));
        }
    }
    None
}

/// An ATX heading line → (level, text).
fn atx_heading(line: &str) -> Option<(usize, String)> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    let trimmed = rest.trim();
    // Drop an optional closing `###` sequence.
    let closing = trimmed.trim_end_matches('#');
    let content = if closing.len() < trimmed.len()
        && (closing.is_empty() || closing.ends_with(char::is_whitespace))
    {
        closing.trim_end()
    } else {
        trimmed
    };
    Some((hashes, content.to_string()))
}

/// A `---` / `***` / `___` thematic break (≥3 markers, only marker + spaces).
fn is_thematic_break(line: &str) -> bool {
    let t = line.trim();
    let ch = match t.chars().next() {
        Some(c @ ('-' | '*' | '_')) => c,
        _ => return false,
    };
    let count = t.chars().filter(|&c| c == ch).count();
    count >= 3 && t.chars().all(|c| c == ch || c == ' ')
}

/// A table delimiter row like `|---|:--:|`.
fn is_delimiter_row(line: &str) -> bool {
    let t = line.trim();
    if !t.contains('-') {
        return false;
    }
    let core = t.trim_matches('|');
    let cells: Vec<&str> = core.split('|').collect();
    if cells.is_empty() {
        return false;
    }
    for cell in cells {
        let c = cell.trim();
        if c.is_empty() || !c.contains('-') || !c.chars().all(|x| x == '-' || x == ':') {
            return false;
        }
    }
    true
}

/// Open-fence line → (fence char, fence length, optional language).
fn fence_open(line: &str) -> Option<(char, usize, Option<String>)> {
    let t = line.trim_start();
    let ch = match t.chars().next() {
        Some(c @ ('`' | '~')) => c,
        _ => return None,
    };
    let n = t.chars().take_while(|&c| c == ch).count();
    if n < 3 {
        return None;
    }
    let info: String = t.chars().skip(n).collect();
    let info = info.trim();
    // Backtick info strings can't contain backticks.
    if ch == '`' && info.contains('`') {
        return None;
    }
    let lang = info
        .split_whitespace()
        .next()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());
    Some((ch, n, lang))
}

fn is_fence_close(line: &str, ch: char, n: usize) -> bool {
    let t = line.trim();
    let count = t.chars().count();
    count >= n && t.chars().all(|c| c == ch)
}

/// Strip one leading `>` (and one optional following space) from a quote line.
fn strip_quote_marker(line: &str) -> String {
    let t = line.trim_start();
    let rest = &t[1..]; // drop the '>'
    rest.strip_prefix(' ').unwrap_or(rest).to_string()
}

/// If the first non-empty inner line starts with a panel prefix, strip it and
/// return the matching kind (mutating `inner` in place).
fn detect_panel(inner: &mut [String]) -> Option<PanelKind> {
    let idx = inner.iter().position(|l| !l.trim().is_empty())?;
    let line = inner[idx].trim_start();
    let lower = line.to_ascii_lowercase();
    let table = [
        ("note:", PanelKind::Note),
        ("warning:", PanelKind::Warning),
        ("info:", PanelKind::Info),
        ("tip:", PanelKind::Tip),
    ];
    for (prefix, kind) in table {
        if lower.starts_with(prefix) {
            let remainder = line[prefix.len()..].trim_start().to_string();
            inner[idx] = remainder;
            return Some(kind);
        }
    }
    None
}

/// Split a table row into trimmed cells, honoring `\|` escapes.
fn split_cells(line: &str) -> Vec<String> {
    let t = line.trim();
    let t = t.strip_prefix('|').unwrap_or(t);
    let t = t.strip_suffix('|').unwrap_or(t);
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = t.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&'|') = chars.peek() {
                cur.push('|');
                chars.next();
                continue;
            }
            cur.push('\\');
        } else if c == '|' {
            cells.push(cur.trim().to_string());
            cur = String::new();
        } else {
            cur.push(c);
        }
    }
    cells.push(cur.trim().to_string());
    cells
}

// ---------------------------------------------------------------------------
// Inline parser
// ---------------------------------------------------------------------------

fn parse_inlines(s: &str) -> Vec<Inline> {
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<Inline> = Vec::new();
    let mut buf = String::new();
    let mut i = 0;
    let n = chars.len();

    macro_rules! flush {
        () => {
            if !buf.is_empty() {
                out.push(Inline::Text(std::mem::take(&mut buf)));
            }
        };
    }

    while i < n {
        let c = chars[i];

        // Inline code — a run of backticks closed by an equal run.
        if c == '`' {
            let fence = run_len(&chars, i, '`');
            if let Some(close) = find_run(&chars, i + fence, '`', fence) {
                flush!();
                let code: String = chars[i + fence..close].iter().collect();
                out.push(Inline::Code(trim_code(&code)));
                i = close + fence;
                continue;
            }
        }

        // Image `![alt](src)`.
        if c == '!' && i + 1 < n && chars[i + 1] == '[' {
            if let Some((text, url, next)) = parse_link(&chars, i + 1) {
                flush!();
                let alt: String = text.iter().collect();
                out.push(Inline::Image(alt, url));
                i = next;
                continue;
            }
        }

        // Link `[text](url)`.
        if c == '[' {
            if let Some((text, url, next)) = parse_link(&chars, i) {
                flush!();
                let inner: String = text.iter().collect();
                out.push(Inline::Link(parse_inlines(&inner), url));
                i = next;
                continue;
            }
        }

        // Bold `**` / `__`.
        if (c == '*' || c == '_') && i + 1 < n && chars[i + 1] == c {
            if let Some(close) = find_run(&chars, i + 2, c, 2) {
                flush!();
                let inner: String = chars[i + 2..close].iter().collect();
                out.push(Inline::Bold(parse_inlines(&inner)));
                i = close + 2;
                continue;
            }
        }

        // Strikethrough `~~`.
        if c == '~' && i + 1 < n && chars[i + 1] == '~' {
            if let Some(close) = find_run(&chars, i + 2, '~', 2) {
                flush!();
                let inner: String = chars[i + 2..close].iter().collect();
                out.push(Inline::Strike(parse_inlines(&inner)));
                i = close + 2;
                continue;
            }
        }

        // Italic `*` / `_`.
        if c == '*' || c == '_' {
            if let Some(close) = find_single(&chars, i + 1, c) {
                flush!();
                let inner: String = chars[i + 1..close].iter().collect();
                out.push(Inline::Italic(parse_inlines(&inner)));
                i = close + 1;
                continue;
            }
        }

        buf.push(c);
        i += 1;
    }
    flush!();
    out
}

fn run_len(chars: &[char], start: usize, ch: char) -> usize {
    let mut k = start;
    while k < chars.len() && chars[k] == ch {
        k += 1;
    }
    k - start
}

/// Find the start index of a run of exactly `len` `ch` at/after `from`.
fn find_run(chars: &[char], from: usize, ch: char, len: usize) -> Option<usize> {
    let mut k = from;
    while k < chars.len() {
        if chars[k] == ch {
            let r = run_len(chars, k, ch);
            if r >= len {
                return Some(k);
            }
            k += r;
        } else {
            k += 1;
        }
    }
    None
}

/// Find a single `ch` at/after `from` that is not part of a double run.
fn find_single(chars: &[char], from: usize, ch: char) -> Option<usize> {
    let mut k = from;
    while k < chars.len() {
        if chars[k] == ch && (k + 1 >= chars.len() || chars[k + 1] != ch) {
            if k > from {
                return Some(k);
            }
            // An immediately-empty span (`**`) isn't emphasis — skip.
        }
        k += 1;
    }
    None
}

/// Parse `[text](url)` starting at `chars[start] == '['`.
/// Returns (text chars, url, index past the closing `)`).
fn parse_link(chars: &[char], start: usize) -> Option<(Vec<char>, String, usize)> {
    if chars.get(start) != Some(&'[') {
        return None;
    }
    // Match the closing `]`, allowing balanced nested brackets.
    let mut depth = 0;
    let mut j = start;
    let mut close_bracket = None;
    while j < chars.len() {
        match chars[j] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close_bracket = Some(j);
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    let close_bracket = close_bracket?;
    if chars.get(close_bracket + 1) != Some(&'(') {
        return None;
    }
    // Find the matching `)`.
    let mut k = close_bracket + 2;
    let mut pdepth = 1;
    let url_start = k;
    while k < chars.len() {
        match chars[k] {
            '(' => pdepth += 1,
            ')' => {
                pdepth -= 1;
                if pdepth == 0 {
                    break;
                }
            }
            _ => {}
        }
        k += 1;
    }
    if k >= chars.len() {
        return None;
    }
    let text = chars[start + 1..close_bracket].to_vec();
    let url: String = chars[url_start..k].iter().collect();
    // Drop an optional "title" after the URL.
    let url = url.split_whitespace().next().unwrap_or("").to_string();
    Some((text, url, k + 1))
}

/// Trim one leading/trailing space from inline code per CommonMark.
fn trim_code(code: &str) -> String {
    if code.len() >= 2
        && code.starts_with(' ')
        && code.ends_with(' ')
        && code.trim().len() != code.len()
        && !code.chars().all(|c| c == ' ')
    {
        code[1..code.len() - 1].to_string()
    } else {
        code.to_string()
    }
}

// ---------------------------------------------------------------------------
// Storage-format renderer
// ---------------------------------------------------------------------------

fn render_blocks_storage(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(render_block_storage)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_block_storage(block: &Block) -> String {
    match block {
        Block::Heading(level, inl) => {
            format!("<h{level}>{}</h{level}>", inlines_storage(inl))
        }
        Block::Paragraph(inl) => format!("<p>{}</p>", inlines_storage(inl)),
        Block::ThematicBreak => "<hr/>".to_string(),
        Block::Code(lang, raw) => {
            let param = match lang {
                Some(l) => format!(
                    "<ac:parameter ac:name=\"language\">{}</ac:parameter>",
                    escape_text(l)
                ),
                None => String::new(),
            };
            format!(
                "<ac:structured-macro ac:name=\"code\">{param}<ac:plain-text-body><![CDATA[{}]]></ac:plain-text-body></ac:structured-macro>",
                cdata_safe(raw)
            )
        }
        Block::List(list) => render_list_storage(list),
        Block::Table(header, rows) => {
            let mut s = String::from("<table><tbody>");
            s.push_str("<tr>");
            for cell in header {
                s.push_str(&format!("<th>{}</th>", inlines_storage(cell)));
            }
            s.push_str("</tr>");
            for row in rows {
                s.push_str("<tr>");
                for cell in row {
                    s.push_str(&format!("<td>{}</td>", inlines_storage(cell)));
                }
                s.push_str("</tr>");
            }
            s.push_str("</tbody></table>");
            s
        }
        Block::Quote(inner) => {
            format!("<blockquote>{}</blockquote>", render_blocks_storage(inner))
        }
        Block::Panel(kind, inner) => format!(
            "<ac:structured-macro ac:name=\"{}\"><ac:rich-text-body>{}</ac:rich-text-body></ac:structured-macro>",
            kind.keyword(),
            render_blocks_storage(inner)
        ),
    }
}

fn render_list_storage(list: &List) -> String {
    let tag = if list.ordered { "ol" } else { "ul" };
    let mut s = format!("<{tag}>");
    for item in &list.items {
        s.push_str("<li>");
        s.push_str(&inlines_storage(&item.content));
        if let Some(sub) = &item.sublist {
            s.push_str(&render_list_storage(sub));
        }
        s.push_str("</li>");
    }
    s.push_str(&format!("</{tag}>"));
    s
}

fn inlines_storage(inlines: &[Inline]) -> String {
    inlines.iter().map(inline_storage).collect()
}

fn inline_storage(inline: &Inline) -> String {
    match inline {
        Inline::Text(s) => escape_text(s),
        Inline::Bold(v) => format!("<strong>{}</strong>", inlines_storage(v)),
        Inline::Italic(v) => format!("<em>{}</em>", inlines_storage(v)),
        Inline::Strike(v) => format!(
            "<span style=\"text-decoration: line-through;\">{}</span>",
            inlines_storage(v)
        ),
        Inline::Code(s) => format!("<code>{}</code>", escape_text(s)),
        Inline::Link(text, url) => {
            format!("<a href=\"{}\">{}</a>", escape_attr(url), inlines_storage(text))
        }
        Inline::Image(alt, src) => {
            if alt.is_empty() {
                format!("<ac:image><ri:url ri:value=\"{}\"/></ac:image>", escape_attr(src))
            } else {
                format!(
                    "<ac:image ac:alt=\"{}\"><ri:url ri:value=\"{}\"/></ac:image>",
                    escape_attr(alt),
                    escape_attr(src)
                )
            }
        }
    }
}

/// Escape text content for XHTML storage format.
fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Escape an attribute value (adds `"`).
fn escape_attr(s: &str) -> String {
    escape_text(s).replace('"', "&quot;")
}

/// Make raw text safe inside a `<![CDATA[…]]>` block.
fn cdata_safe(s: &str) -> String {
    s.replace("]]>", "]]]]><![CDATA[>")
}

// ---------------------------------------------------------------------------
// Wiki-markup renderer
// ---------------------------------------------------------------------------

fn render_blocks_wiki(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(render_block_wiki)
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_block_wiki(block: &Block) -> String {
    match block {
        Block::Heading(level, inl) => format!("h{level}. {}", inlines_wiki(inl)),
        Block::Paragraph(inl) => inlines_wiki(inl),
        Block::ThematicBreak => "----".to_string(),
        Block::Code(lang, raw) => match lang {
            Some(l) => format!("{{code:{}}}\n{}\n{{code}}", l, raw),
            None => format!("{{code}}\n{}\n{{code}}", raw),
        },
        Block::List(list) => render_list_wiki(list, "").join("\n"),
        Block::Table(header, rows) => {
            let mut lines = Vec::new();
            let mut h = String::new();
            for cell in header {
                h.push_str("||");
                h.push_str(&inlines_wiki(cell));
            }
            h.push_str("||");
            lines.push(h);
            for row in rows {
                let mut r = String::new();
                for cell in row {
                    r.push('|');
                    r.push_str(&inlines_wiki(cell));
                }
                r.push('|');
                lines.push(r);
            }
            lines.join("\n")
        }
        Block::Quote(inner) => {
            format!("{{quote}}\n{}\n{{quote}}", inner_wiki(inner))
        }
        Block::Panel(kind, inner) => {
            let kw = kind.keyword();
            format!("{{{kw}}}\n{}\n{{{kw}}}", inner_wiki(inner))
        }
    }
}

/// Render blocks nested inside a quote/panel, joined by single newlines.
fn inner_wiki(blocks: &[Block]) -> String {
    blocks
        .iter()
        .map(render_block_wiki)
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_list_wiki(list: &List, parent_prefix: &str) -> Vec<String> {
    let ch = if list.ordered { '#' } else { '*' };
    let mut lines = Vec::new();
    for item in &list.items {
        let prefix = format!("{parent_prefix}{ch}");
        lines.push(format!("{prefix} {}", inlines_wiki(&item.content)));
        if let Some(sub) = &item.sublist {
            lines.extend(render_list_wiki(sub, &prefix));
        }
    }
    lines
}

fn inlines_wiki(inlines: &[Inline]) -> String {
    inlines.iter().map(inline_wiki).collect()
}

fn inline_wiki(inline: &Inline) -> String {
    match inline {
        Inline::Text(s) => escape_wiki(s),
        Inline::Bold(v) => format!("*{}*", inlines_wiki(v)),
        Inline::Italic(v) => format!("_{}_", inlines_wiki(v)),
        Inline::Strike(v) => format!("-{}-", inlines_wiki(v)),
        Inline::Code(s) => {
            let mut r = String::from("{{");
            r.push_str(s);
            r.push_str("}}");
            r
        }
        Inline::Link(text, url) => format!("[{}|{}]", inlines_wiki(text), url),
        Inline::Image(_alt, src) => format!("!{}!", src),
    }
}

/// Backslash-escape the wiki characters that would otherwise break structure.
fn escape_wiki(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '{' | '}' | '[' | ']' | '|') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_happy_headings_and_inline() {
        let out = convert(
            "# Title\n\nSome **bold**, *italic*, ~~gone~~ and `code`.",
            "storage",
            true,
            0,
        )
        .unwrap();
        assert!(out.contains("<h1>Title</h1>"), "{out}");
        assert!(out.contains("<strong>bold</strong>"), "{out}");
        assert!(out.contains("<em>italic</em>"), "{out}");
        assert!(
            out.contains("<span style=\"text-decoration: line-through;\">gone</span>"),
            "{out}"
        );
        assert!(out.contains("<code>code</code>"), "{out}");
        assert!(out.contains("<p>"), "{out}");
    }

    #[test]
    fn wiki_happy_headings_and_inline() {
        let out = convert(
            "## Heading\n\nSome **bold** and *italic* and `code` text.",
            "wiki",
            true,
            0,
        )
        .unwrap();
        assert!(out.contains("h2. Heading"), "{out}");
        assert!(out.contains("*bold*"), "{out}");
        assert!(out.contains("_italic_"), "{out}");
        assert!(out.contains("{{code}}"), "{out}");
    }

    #[test]
    fn storage_code_block_with_language() {
        let out = convert("```rust\nfn main() {}\n```", "storage", true, 0).unwrap();
        assert!(out.contains("ac:name=\"code\""), "{out}");
        assert!(
            out.contains("<ac:parameter ac:name=\"language\">rust</ac:parameter>"),
            "{out}"
        );
        assert!(out.contains("<![CDATA[fn main() {}]]>"), "{out}");
    }

    #[test]
    fn wiki_code_block_with_language() {
        let out = convert("```python\nprint(1)\n```", "wiki", true, 0).unwrap();
        assert!(out.contains("{code:python}"), "{out}");
        assert!(out.contains("print(1)"), "{out}");
        assert!(out.contains("{code}"), "{out}");
    }

    #[test]
    fn storage_table() {
        let md = "| A | B |\n| --- | --- |\n| 1 | 2 |";
        let out = convert(md, "storage", true, 0).unwrap();
        assert!(out.contains("<table><tbody>"), "{out}");
        assert!(out.contains("<th>A</th>"), "{out}");
        assert!(out.contains("<th>B</th>"), "{out}");
        assert!(out.contains("<td>1</td>"), "{out}");
        assert!(out.contains("<td>2</td>"), "{out}");
    }

    #[test]
    fn wiki_table() {
        let md = "| A | B |\n| --- | --- |\n| 1 | 2 |";
        let out = convert(md, "wiki", true, 0).unwrap();
        assert!(out.contains("||A||B||"), "{out}");
        assert!(out.contains("|1|2|"), "{out}");
    }

    #[test]
    fn panel_blockquote_note() {
        let md = "> Note: back up first.";
        let storage = convert(md, "storage", true, 0).unwrap();
        assert!(storage.contains("ac:name=\"note\""), "{storage}");
        assert!(storage.contains("back up first."), "{storage}");
        assert!(!storage.contains("Note:"), "prefix should be stripped: {storage}");

        let wiki = convert(md, "wiki", true, 0).unwrap();
        assert!(wiki.contains("{note}"), "{wiki}");
        assert!(wiki.contains("back up first."), "{wiki}");
    }

    #[test]
    fn panel_kinds_all_map() {
        for (prefix, kw) in [
            ("Info", "info"),
            ("Warning", "warning"),
            ("Tip", "tip"),
        ] {
            let md = format!("> {prefix}: hello");
            let out = convert(&md, "storage", true, 0).unwrap();
            assert!(out.contains(&format!("ac:name=\"{kw}\"")), "{out}");
        }
    }

    #[test]
    fn panel_disabled_keeps_blockquote() {
        let out = convert("> Note: keep me literal.", "storage", false, 0).unwrap();
        assert!(out.contains("<blockquote>"), "{out}");
        assert!(out.contains("Note: keep me literal."), "{out}");
        assert!(!out.contains("ac:name=\"note\""), "{out}");
    }

    #[test]
    fn heading_offset_demotes_and_caps() {
        let out = convert("# Top", "storage", true, 2).unwrap();
        assert!(out.contains("<h3>Top</h3>"), "{out}");
        // Cap at h6 even when offset would exceed it.
        let capped = convert("##### Deep", "storage", true, 5).unwrap();
        assert!(capped.contains("<h6>Deep</h6>"), "{capped}");
    }

    #[test]
    fn nested_lists_storage_and_wiki() {
        let md = "- a\n  - a1\n- b";
        let storage = convert(md, "storage", true, 0).unwrap();
        assert!(storage.contains("<ul><li>a<ul><li>a1</li></ul></li><li>b</li></ul>"), "{storage}");
        let wiki = convert(md, "wiki", true, 0).unwrap();
        assert!(wiki.contains("* a"), "{wiki}");
        assert!(wiki.contains("** a1"), "{wiki}");
        assert!(wiki.contains("* b"), "{wiki}");
    }

    #[test]
    fn ordered_list_wiki() {
        let out = convert("1. one\n2. two", "wiki", true, 0).unwrap();
        assert!(out.contains("# one"), "{out}");
        assert!(out.contains("# two"), "{out}");
    }

    #[test]
    fn links_and_images_storage() {
        let out = convert("[site](https://example.com) ![alt](https://x/y.png)", "storage", true, 0)
            .unwrap();
        assert!(out.contains("<a href=\"https://example.com\">site</a>"), "{out}");
        assert!(
            out.contains("<ac:image ac:alt=\"alt\"><ri:url ri:value=\"https://x/y.png\"/></ac:image>"),
            "{out}"
        );
    }

    #[test]
    fn links_wiki() {
        let out = convert("[site](https://example.com)", "wiki", true, 0).unwrap();
        assert!(out.contains("[site|https://example.com]"), "{out}");
    }

    #[test]
    fn thematic_break_both_formats() {
        assert!(convert("a\n\n---\n\nb", "storage", true, 0).unwrap().contains("<hr/>"));
        assert!(convert("a\n\n---\n\nb", "wiki", true, 0).unwrap().contains("----"));
    }

    #[test]
    fn html_is_escaped_in_storage() {
        let out = convert("a < b & c > d", "storage", true, 0).unwrap();
        assert!(out.contains("a &lt; b &amp; c &gt; d"), "{out}");
        assert!(!out.contains("< b"), "{out}");
    }

    #[test]
    fn wiki_special_chars_escaped() {
        let out = convert("use {braces} and [brackets]", "wiki", true, 0).unwrap();
        assert!(out.contains("\\{braces\\}"), "{out}");
        assert!(out.contains("\\[brackets\\]"), "{out}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("", "storage", true, 0).is_err());
        assert!(convert("   \n  \n", "wiki", true, 0).is_err());
    }

    #[test]
    fn unknown_format_errors() {
        assert!(convert("# hi", "markdown", true, 0).is_err());
    }

    #[test]
    fn run_defaults_to_storage() {
        let out = run("# Hi").unwrap();
        assert!(out.contains("<h1>Hi</h1>"), "{out}");
    }
}
