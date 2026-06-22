//! markdown-to-latex core — convert a Markdown document into LaTeX source.
//!
//! Pure compute, no wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page. A hand-rolled, line-oriented Markdown parser (CommonMark-ish
//! subset + a few GitHub-flavored extensions) that emits LaTeX, escaping the ten
//! TeX special characters so prose is safe by construction.
//!
//! Supported block constructs: ATX headings (`#`..`######`), unordered and
//! ordered lists (with nesting by indentation), fenced (```` ``` ````) and
//! indented code blocks, blockquotes (`>`), GitHub pipe tables, thematic breaks
//! (`---`/`***`/`___`), and paragraphs. Inline: `**bold**`/`__bold__`,
//! `*italic*`/`_italic_`, `` `code` ``, `[text](url)` links, `![alt](src)`
//! images, autolinks `<https://…>`, and raw `$math$` / `$$math$$` passthrough.
//!
//! Output is a LaTeX *fragment* by default; pass `full_document = true` to wrap
//! it in a compilable `article`-class document with the packages the constructs
//! need (`hyperref`, `graphicx`, `listings`, `booktabs`/`array`).

/// Convert `markdown` to LaTeX.
///
/// - `full_document`: when `true`, wrap the body in a standalone `\documentclass`
///   preamble (+ `\begin{document}`/`\end{document}`); otherwise emit only the
///   body fragment.
/// - `heading_offset`: shift every heading down by this many levels (e.g. `1`
///   turns `#` from `\section` into `\subsection`). Clamped to `0..=5`.
///
/// Always returns `Ok` — malformed Markdown degrades to literal text rather than
/// erroring, matching how Markdown renderers behave.
pub fn convert(markdown: &str, full_document: bool, heading_offset: u32) -> Result<String, String> {
    let offset = heading_offset.min(5) as usize;
    // First pass: pull out reference-style footnote definitions
    // (`[^id]: text` lines), leaving the body for the block parser. References
    // `[^id]` in the body are then expanded to `\footnote{...}` inline.
    let (stripped, footnotes) = extract_footnotes(markdown);
    let body = Converter::new(offset, footnotes).run(&stripped);
    if full_document {
        Ok(wrap_document(&body))
    } else {
        Ok(body)
    }
}

/// Split a document into (body-without-footnote-defs, id→definition map).
/// A footnote definition is a line of the form `[^id]: definition text`.
fn extract_footnotes(markdown: &str) -> (String, std::collections::HashMap<String, String>) {
    use std::collections::HashMap;
    let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
    let mut defs: HashMap<String, String> = HashMap::new();
    let mut body_lines: Vec<&str> = Vec::new();
    for line in normalized.lines() {
        if let Some((id, text)) = parse_footnote_def(line) {
            defs.insert(id.to_string(), text.to_string());
        } else {
            body_lines.push(line);
        }
    }
    (body_lines.join("\n"), defs)
}

/// Parse a `[^id]: text` footnote-definition line. Returns (id, text).
fn parse_footnote_def(line: &str) -> Option<(&str, &str)> {
    let rest = line.strip_prefix("[^")?;
    let close = rest.find("]:")?;
    let id = &rest[..close];
    if id.is_empty() || id.contains(char::is_whitespace) {
        return None;
    }
    let text = rest[close + 2..].trim_start();
    Some((id, text))
}

/// Thin entry: fragment output, no heading offset.
pub fn run(markdown: &str) -> Result<String, String> {
    convert(markdown, false, 0)
}

/// The LaTeX sectioning commands `#`..`######` map onto, indexed by level-1.
const SECTIONS: [&str; 6] = [
    "section",
    "subsection",
    "subsubsection",
    "paragraph",
    "subparagraph",
    "subparagraph",
];

fn wrap_document(body: &str) -> String {
    let mut s = String::new();
    s.push_str("\\documentclass{article}\n");
    s.push_str("\\usepackage[utf8]{inputenc}\n");
    s.push_str("\\usepackage[T1]{fontenc}\n");
    s.push_str("\\usepackage{hyperref}\n");
    s.push_str("\\usepackage{graphicx}\n");
    s.push_str("\\usepackage{listings}\n");
    s.push_str("\\usepackage{booktabs}\n");
    s.push_str("\\usepackage{array}\n");
    s.push_str("\\usepackage[normalem]{ulem}\n");
    s.push_str("\\begin{document}\n\n");
    s.push_str(body.trim_end());
    s.push_str("\n\n\\end{document}\n");
    s
}

/// Escape the LaTeX special characters in a run of plain prose text.
///
/// The ten TeX specials need escaping: `& % $ # _ { } ~ ^ \`. `~` and `^` have
/// no `\`-prefixed literal form, so they use `\textasciitilde{}` /
/// `\textasciicircum{}`; backslash becomes `\textbackslash{}`.
fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\textbackslash{}"),
            '&' => out.push_str("\\&"),
            '%' => out.push_str("\\%"),
            '$' => out.push_str("\\$"),
            '#' => out.push_str("\\#"),
            '_' => out.push_str("\\_"),
            '{' => out.push_str("\\{"),
            '}' => out.push_str("\\}"),
            '~' => out.push_str("\\textasciitilde{}"),
            '^' => out.push_str("\\textasciicircum{}"),
            other => out.push(other),
        }
    }
    out
}

/// Escape a URL for use inside `\href{...}` / `\url{...}`. Only `%` and `#`
/// genuinely need escaping inside hyperref's argument; other chars are kept so
/// the link stays valid.
fn escape_url(url: &str) -> String {
    url.replace('%', "\\%").replace('#', "\\#")
}

struct Converter {
    /// Heading levels are shifted down by this many steps.
    offset: usize,
    /// Reference-style footnote definitions, keyed by id.
    footnotes: std::collections::HashMap<String, String>,
    out: String,
}

impl Converter {
    fn new(offset: usize, footnotes: std::collections::HashMap<String, String>) -> Self {
        Converter {
            offset,
            footnotes,
            out: String::new(),
        }
    }

    /// Convert inline spans on a line, threading this converter's footnote map.
    fn inline(&self, text: &str) -> String {
        inline(text, &self.footnotes)
    }

    fn run(mut self, markdown: &str) -> String {
        // Normalize line endings, then split into lines we can look ahead over
        // (tables and lists need to peek at the following line).
        let normalized = markdown.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<&str> = normalized.lines().collect();

        let mut i = 0;
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim_start();

            // Blank line — paragraph separator.
            if line.trim().is_empty() {
                i += 1;
                continue;
            }

            // Fenced code block.
            if let Some(fence) = code_fence(trimmed) {
                i = self.emit_fenced_code(&lines, i, fence);
                continue;
            }

            // Thematic break.
            if is_thematic_break(trimmed) {
                self.out
                    .push_str("\\par\\noindent\\rule{\\textwidth}{0.4pt}\n\n");
                i += 1;
                continue;
            }

            // ATX heading.
            if let Some((level, text)) = atx_heading(trimmed) {
                self.emit_heading(level, text);
                i += 1;
                continue;
            }

            // Setext heading: a text line underlined by `===` (level 1) or
            // `---` (level 2). Only when the current line is plain text (not a
            // list/quote/table) and the next line is a pure underline.
            if i + 1 < lines.len()
                && list_marker(line).is_none()
                && !trimmed.starts_with('>')
            {
                if let Some(level) = setext_underline(lines[i + 1]) {
                    self.emit_heading(level, line.trim());
                    i += 2;
                    continue;
                }
            }

            // Blockquote.
            if trimmed.starts_with('>') {
                i = self.emit_blockquote(&lines, i);
                continue;
            }

            // Table (a header row followed by a delimiter row).
            if i + 1 < lines.len() && is_table_row(line) && is_table_delim(lines[i + 1]) {
                i = self.emit_table(&lines, i);
                continue;
            }

            // List (ordered or unordered).
            if list_marker(line).is_some() {
                i = self.emit_list(&lines, i);
                continue;
            }

            // Indented code block (4 spaces / a tab), not inside a list.
            if line.starts_with("    ") || line.starts_with('\t') {
                i = self.emit_indented_code(&lines, i);
                continue;
            }

            // Paragraph: gather consecutive non-blank, non-special lines.
            i = self.emit_paragraph(&lines, i);
        }

        // Collapse any 3+ newline runs to a single blank line.
        squeeze_blank_lines(&self.out)
    }

    fn emit_heading(&mut self, level: usize, text: &str) {
        let idx = (level - 1 + self.offset).min(SECTIONS.len() - 1);
        let cmd = SECTIONS[idx];
        self.out
            .push_str(&format!("\\{}{{{}}}\n\n", cmd, self.inline(text)));
    }

    fn emit_fenced_code(&mut self, lines: &[&str], start: usize, fence: Fence) -> usize {
        let mut i = start + 1;
        let mut body = String::new();
        while i < lines.len() {
            let t = lines[i].trim_start();
            if let Some(close) = code_fence(t) {
                if close.ch == fence.ch && close.len >= fence.len && close.info.is_empty() {
                    i += 1;
                    break;
                }
            }
            body.push_str(lines[i]);
            body.push('\n');
            i += 1;
        }
        // listings handles arbitrary text verbatim; pass the language if given.
        let lang = fence.info.split_whitespace().next().unwrap_or("");
        if lang.is_empty() {
            self.out.push_str("\\begin{lstlisting}\n");
        } else {
            self.out
                .push_str(&format!("\\begin{{lstlisting}}[language={}]\n", lang));
        }
        self.out.push_str(body.trim_end_matches('\n'));
        self.out.push_str("\n\\end{lstlisting}\n\n");
        i
    }

    fn emit_indented_code(&mut self, lines: &[&str], start: usize) -> usize {
        let mut i = start;
        let mut body = String::new();
        while i < lines.len() {
            let line = lines[i];
            if line.trim().is_empty() {
                // A blank line may continue the block if more indented code follows.
                if i + 1 < lines.len()
                    && (lines[i + 1].starts_with("    ") || lines[i + 1].starts_with('\t'))
                {
                    body.push('\n');
                    i += 1;
                    continue;
                }
                break;
            }
            if let Some(rest) = line.strip_prefix("    ") {
                body.push_str(rest);
            } else if let Some(rest) = line.strip_prefix('\t') {
                body.push_str(rest);
            } else {
                break;
            }
            body.push('\n');
            i += 1;
        }
        self.out.push_str("\\begin{verbatim}\n");
        self.out.push_str(body.trim_end_matches('\n'));
        self.out.push_str("\n\\end{verbatim}\n\n");
        i
    }

    fn emit_blockquote(&mut self, lines: &[&str], start: usize) -> usize {
        let mut i = start;
        let mut inner = String::new();
        while i < lines.len() {
            let t = lines[i].trim_start();
            if !t.starts_with('>') {
                break;
            }
            // Strip the leading '>' and one optional space.
            let rest = t[1..].strip_prefix(' ').unwrap_or(&t[1..]);
            inner.push_str(rest);
            inner.push('\n');
            i += 1;
        }
        // Recurse so nested constructs inside the quote convert too; share the
        // footnote map so references inside a quote still resolve.
        let body = Converter::new(self.offset, self.footnotes.clone()).run(&inner);
        self.out.push_str("\\begin{quote}\n");
        self.out.push_str(body.trim_end());
        self.out.push_str("\n\\end{quote}\n\n");
        i
    }

    fn emit_list(&mut self, lines: &[&str], start: usize) -> usize {
        // Items at the first item's indent belong here; deeper indents recurse,
        // shallower ends the list.
        let base_indent = indent_width(lines[start]);
        let first = list_marker(lines[start]).unwrap();
        let ordered = first.ordered;
        let env = if ordered { "enumerate" } else { "itemize" };
        self.out.push_str(&format!("\\begin{{{}}}\n", env));

        let mut i = start;
        while i < lines.len() {
            let line = lines[i];
            if line.trim().is_empty() {
                // Allow a single blank line between items; stop if the next
                // non-blank line isn't part of this list.
                let mut j = i + 1;
                while j < lines.len() && lines[j].trim().is_empty() {
                    j += 1;
                }
                if j >= lines.len() {
                    i = j;
                    break;
                }
                let next_indent = indent_width(lines[j]);
                if list_marker(lines[j]).is_some() && next_indent >= base_indent {
                    i = j;
                    continue;
                }
                break;
            }
            let this_indent = indent_width(line);
            let marker = match list_marker(line) {
                Some(m) if this_indent == base_indent => m,
                Some(_) if this_indent > base_indent => {
                    // Deeper list — recurse; it attaches to the current item.
                    i = self.emit_list(lines, i);
                    continue;
                }
                _ if this_indent > base_indent => {
                    // Continuation text of the current item.
                    self.out.push(' ');
                    self.out.push_str(&self.inline(line.trim()));
                    self.out.push('\n');
                    i += 1;
                    continue;
                }
                _ => break,
            };
            // Task-list checkbox rendering.
            let content = marker.content;
            let rendered = if let Some((checked, text)) = task_item(content) {
                let box_ = if checked {
                    "[\\checkmark]"
                } else {
                    "[$\\square$]"
                };
                format!("\\item{} {}", box_, self.inline(text))
            } else {
                format!("\\item {}", self.inline(content))
            };
            self.out.push_str(&rendered);
            self.out.push('\n');
            i += 1;
        }
        self.out.push_str(&format!("\\end{{{}}}\n\n", env));
        i
    }

    fn emit_table(&mut self, lines: &[&str], start: usize) -> usize {
        let header = parse_table_row(lines[start]);
        let aligns = parse_table_aligns(lines[start + 1]);
        let ncols = header.len().max(aligns.len());
        let mut rows: Vec<Vec<String>> = Vec::new();
        let mut i = start + 2;
        while i < lines.len() && !lines[i].trim().is_empty() && is_table_row(lines[i]) {
            rows.push(parse_table_row(lines[i]));
            i += 1;
        }

        let colspec: String = (0..ncols)
            .map(|c| match aligns.get(c) {
                Some(Align::Center) => 'c',
                Some(Align::Right) => 'r',
                _ => 'l',
            })
            .collect();

        self.out
            .push_str(&format!("\\begin{{tabular}}{{{}}}\n\\toprule\n", colspec));
        self.out.push_str(&self.render_table_row(&header, ncols));
        self.out.push_str(" \\\\\n\\midrule\n");
        for row in &rows {
            self.out.push_str(&self.render_table_row(row, ncols));
            self.out.push_str(" \\\\\n");
        }
        self.out.push_str("\\bottomrule\n\\end{tabular}\n\n");
        i
    }

    fn emit_paragraph(&mut self, lines: &[&str], start: usize) -> usize {
        let mut i = start;
        let mut buf: Vec<&str> = Vec::new();
        while i < lines.len() {
            let line = lines[i];
            let trimmed = line.trim_start();
            if line.trim().is_empty()
                || atx_heading(trimmed).is_some()
                || code_fence(trimmed).is_some()
                || is_thematic_break(trimmed)
                || trimmed.starts_with('>')
                || list_marker(line).is_some()
            {
                break;
            }
            buf.push(line.trim());
            i += 1;
        }
        // A trailing "  " in Markdown means a hard line break (`\\`); otherwise
        // soft-wrap lines into one paragraph.
        let mut para = String::new();
        for (idx, raw) in buf.iter().enumerate() {
            let hard = lines[start + idx].ends_with("  ");
            para.push_str(&self.inline(raw));
            if idx + 1 < buf.len() {
                para.push_str(if hard { " \\\\\n" } else { "\n" });
            }
        }
        self.out.push_str(&para);
        self.out.push_str("\n\n");
        i
    }

    /// Render one table row (cells already raw) padded/truncated to `ncols`.
    fn render_table_row(&self, cells: &[String], ncols: usize) -> String {
        (0..ncols)
            .map(|c| self.inline(cells.get(c).map(|s| s.as_str()).unwrap_or("")))
            .collect::<Vec<_>>()
            .join(" & ")
    }
}

#[derive(Clone, Copy)]
enum Align {
    Center,
    Right,
    Left,
}

struct Fence {
    ch: char,
    len: usize,
    info: String,
}

/// Detect a code fence (``` ``` ``` or `~~~`). Returns the fence char, length,
/// and info string (language).
fn code_fence(trimmed: &str) -> Option<Fence> {
    let ch = trimmed.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = trimmed.chars().take_while(|&c| c == ch).count();
    if len < 3 {
        return None;
    }
    let info: String = trimmed
        .chars()
        .skip(len)
        .collect::<String>()
        .trim()
        .to_string();
    // An info string can't contain backticks for a ``` fence.
    if ch == '`' && info.contains('`') {
        return None;
    }
    Some(Fence { ch, len, info })
}

/// Detect a setext-heading underline: a line of one-or-more `=` (level 1) or
/// `-` (level 2), nothing else. Returns the heading level.
fn setext_underline(line: &str) -> Option<usize> {
    let t = line.trim();
    if t.is_empty() {
        return None;
    }
    if t.chars().all(|c| c == '=') {
        Some(1)
    } else if t.chars().all(|c| c == '-') {
        Some(2)
    } else {
        None
    }
}

fn is_thematic_break(trimmed: &str) -> bool {
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && (compact.chars().all(|c| c == '-')
            || compact.chars().all(|c| c == '*')
            || compact.chars().all(|c| c == '_'))
}

fn atx_heading(trimmed: &str) -> Option<(usize, &str)> {
    if !trimmed.starts_with('#') {
        return None;
    }
    let level = trimmed.chars().take_while(|&c| c == '#').count();
    if level == 0 || level > 6 {
        return None;
    }
    let rest = &trimmed[level..];
    // A heading needs a space (or end-of-line) after the hashes.
    if !rest.is_empty() && !rest.starts_with(' ') {
        return None;
    }
    // Strip optional trailing `#`s.
    Some((level, rest.trim().trim_end_matches('#').trim()))
}

fn indent_width(line: &str) -> usize {
    line.chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .map(|c| if c == '\t' { 4 } else { 1 })
        .sum()
}

struct ListMarker<'a> {
    ordered: bool,
    content: &'a str,
}

/// Detect a list item; returns the marker kind and the item content (after the
/// marker + one space).
fn list_marker(line: &str) -> Option<ListMarker<'_>> {
    let trimmed = line.trim_start();
    // Unordered: -, *, +
    for m in ['-', '*', '+'] {
        if let Some(rest) = trimmed.strip_prefix(m) {
            if rest.starts_with(' ') || rest.is_empty() {
                // A bare `***`/`---` is a thematic break, not a list.
                if is_thematic_break(trimmed) {
                    return None;
                }
                return Some(ListMarker {
                    ordered: false,
                    content: rest.trim_start(),
                });
            }
        }
    }
    // Ordered: digits followed by '.' or ')'.
    let digits: String = trimmed.chars().take_while(|c| c.is_ascii_digit()).collect();
    if !digits.is_empty() {
        let after = &trimmed[digits.len()..];
        if let Some(rest) = after.strip_prefix('.').or_else(|| after.strip_prefix(')')) {
            if rest.starts_with(' ') || rest.is_empty() {
                return Some(ListMarker {
                    ordered: true,
                    content: rest.trim_start(),
                });
            }
        }
    }
    None
}

/// A GitHub task-list item: `[ ] text` / `[x] text`. Returns (checked, text).
fn task_item(content: &str) -> Option<(bool, &str)> {
    let rest = content.strip_prefix('[')?;
    let mark = rest.chars().next()?;
    let after = &rest[mark.len_utf8()..];
    let after = after.strip_prefix(']')?;
    let text = after.strip_prefix(' ').unwrap_or(after);
    match mark {
        ' ' => Some((false, text)),
        'x' | 'X' => Some((true, text)),
        _ => None,
    }
}

fn is_table_row(line: &str) -> bool {
    line.contains('|')
}

fn is_table_delim(line: &str) -> bool {
    let t = line.trim();
    let cells = split_table_cells(t);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim();
            !c.is_empty() && c.chars().all(|ch| ch == '-' || ch == ':') && c.contains('-')
        })
}

fn parse_table_row(line: &str) -> Vec<String> {
    split_table_cells(line.trim())
        .into_iter()
        .map(|c| c.trim().to_string())
        .collect()
}

fn parse_table_aligns(line: &str) -> Vec<Align> {
    split_table_cells(line.trim())
        .into_iter()
        .map(|c| {
            let c = c.trim();
            let left = c.starts_with(':');
            let right = c.ends_with(':');
            match (left, right) {
                (true, true) => Align::Center,
                (false, true) => Align::Right,
                _ => Align::Left,
            }
        })
        .collect()
}

/// Split a pipe-delimited table row into cells, honoring `\|` escapes and
/// stripping the optional leading/trailing pipes.
fn split_table_cells(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                if chars.peek() == Some(&'|') {
                    cur.push('|');
                    chars.next();
                    continue;
                }
                cur.push('\\');
            }
            '|' => {
                cells.push(cur.clone());
                cur.clear();
            }
            other => cur.push(other),
        }
    }
    cells.push(cur);
    // Drop the empty leading/trailing cells produced by border pipes.
    if cells.first().map(|s| s.trim().is_empty()).unwrap_or(false) {
        cells.remove(0);
    }
    if cells.last().map(|s| s.trim().is_empty()).unwrap_or(false) {
        cells.pop();
    }
    cells
}

/// Convert inline Markdown spans within one line of text to LaTeX. Handles
/// code spans, images, links, autolinks, bold, italic, strikethrough, footnote
/// references, and `$math$` passthrough; everything else is escaped as plain
/// prose. `footnotes` maps a footnote id to its (raw) definition text.
fn inline(text: &str, footnotes: &std::collections::HashMap<String, String>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            // Inline code span: `...` (one or more backticks).
            '`' => {
                let tick_len = run_len(&chars, i, '`');
                if let Some(end) = find_closing(&chars, i + tick_len, '`', tick_len) {
                    let inner: String = chars[i + tick_len..end].iter().collect();
                    out.push_str(&format!("\\texttt{{{}}}", escape_text(inner.trim())));
                    i = end + tick_len;
                    continue;
                }
                out.push('`');
                i += 1;
            }
            // Math passthrough: $$...$$ or $...$ — emitted verbatim.
            '$' => {
                let dollar_len = if i + 1 < chars.len() && chars[i + 1] == '$' {
                    2
                } else {
                    1
                };
                if let Some(end) = find_closing(&chars, i + dollar_len, '$', dollar_len) {
                    let inner: String = chars[i..end + dollar_len].iter().collect();
                    out.push_str(&inner);
                    i = end + dollar_len;
                    continue;
                }
                out.push_str("\\$");
                i += 1;
            }
            // Image: ![alt](src)
            '!' if i + 1 < chars.len() && chars[i + 1] == '[' => {
                if let Some((_alt, src, next)) = parse_link(&chars, i + 1) {
                    out.push_str(&format!(
                        "\\includegraphics[width=\\linewidth]{{{}}}",
                        escape_url(&src)
                    ));
                    i = next;
                    continue;
                }
                out.push('!');
                i += 1;
            }
            // Footnote reference: [^id] → \footnote{definition}.
            '[' if i + 1 < chars.len() && chars[i + 1] == '^' => {
                if let Some(close) = chars[i + 2..].iter().position(|&c| c == ']') {
                    let id: String = chars[i + 2..i + 2 + close].iter().collect();
                    if let Some(def) = footnotes.get(&id) {
                        out.push_str(&format!("\\footnote{{{}}}", inline(def, footnotes)));
                        i = i + 2 + close + 1;
                        continue;
                    }
                }
                out.push_str(&escape_text("["));
                i += 1;
            }
            // Link: [text](url)
            '[' => {
                if let Some((label, url, next)) = parse_link(&chars, i) {
                    out.push_str(&format!(
                        "\\href{{{}}}{{{}}}",
                        escape_url(&url),
                        inline(&label, footnotes)
                    ));
                    i = next;
                    continue;
                }
                out.push_str(&escape_text("["));
                i += 1;
            }
            // Autolink: <https://...>
            '<' => {
                if let Some(pos) = chars[i + 1..].iter().position(|&c| c == '>') {
                    let url: String = chars[i + 1..i + 1 + pos].iter().collect();
                    if url.starts_with("http://") || url.starts_with("https://") {
                        out.push_str(&format!("\\url{{{}}}", escape_url(&url)));
                        i = i + 1 + pos + 1;
                        continue;
                    }
                }
                out.push('<');
                i += 1;
            }
            // Strikethrough: ~~text~~ → \sout{...} (GitHub-flavored).
            '~' if run_len(&chars, i, '~') >= 2 => {
                if let Some(end) = find_closing(&chars, i + 2, '~', 2) {
                    let inner: String = chars[i + 2..end].iter().collect();
                    out.push_str(&format!("\\sout{{{}}}", inline(&inner, footnotes)));
                    i = end + 2;
                    continue;
                }
                // Lone `~~` with no close → escape both tildes literally.
                out.push_str(&escape_text("~~"));
                i += 2;
            }
            // Strong: ** or __
            '*' | '_' if run_len(&chars, i, c) >= 2 => {
                if let Some(end) = find_closing(&chars, i + 2, c, 2) {
                    let inner: String = chars[i + 2..end].iter().collect();
                    out.push_str(&format!("\\textbf{{{}}}", inline(&inner, footnotes)));
                    i = end + 2;
                    continue;
                }
                out.push_str(&escape_text(&c.to_string()));
                i += 1;
            }
            // Emphasis: * or _
            '*' | '_' => {
                if let Some(end) = find_closing(&chars, i + 1, c, 1) {
                    let inner: String = chars[i + 1..end].iter().collect();
                    out.push_str(&format!("\\textit{{{}}}", inline(&inner, footnotes)));
                    i = end + 1;
                    continue;
                }
                out.push_str(&escape_text(&c.to_string()));
                i += 1;
            }
            other => {
                out.push_str(&escape_text(&other.to_string()));
                i += 1;
            }
        }
    }
    out
}

/// Count the run of `ch` starting at index `i`.
fn run_len(chars: &[char], i: usize, ch: char) -> usize {
    chars[i..].iter().take_while(|&&c| c == ch).count()
}

/// Find the next run of at least `len` `ch` (delimiter close) at/after `from`.
/// Returns the start index of that closing run.
fn find_closing(chars: &[char], from: usize, ch: char, len: usize) -> Option<usize> {
    let mut i = from;
    while i < chars.len() {
        if chars[i] == ch {
            let r = run_len(chars, i, ch);
            if r >= len {
                return Some(i);
            }
            i += r;
        } else {
            i += 1;
        }
    }
    None
}

/// Parse a `[label](url)` link starting at the `[` index. Returns
/// (label, url, index-after-the-close-paren).
fn parse_link(chars: &[char], start: usize) -> Option<(String, String, usize)> {
    if chars.get(start) != Some(&'[') {
        return None;
    }
    // Find the matching ] (track nesting of brackets).
    let mut depth = 0i32;
    let mut close = None;
    let mut j = start;
    while j < chars.len() {
        match chars[j] {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(j);
                    break;
                }
            }
            _ => {}
        }
        j += 1;
    }
    let close = close?;
    if chars.get(close + 1) != Some(&'(') {
        return None;
    }
    // Find the matching ).
    let mut paren = close + 2;
    let mut pdepth = 1i32;
    while paren < chars.len() {
        match chars[paren] {
            '(' => pdepth += 1,
            ')' => {
                pdepth -= 1;
                if pdepth == 0 {
                    break;
                }
            }
            _ => {}
        }
        paren += 1;
    }
    if paren >= chars.len() {
        return None;
    }
    let label: String = chars[start + 1..close].iter().collect();
    let dest: String = chars[close + 2..paren].iter().collect();
    // A destination may carry a "title" — drop it for the href target.
    let url = dest.split_whitespace().next().unwrap_or("").to_string();
    Some((label, url, paren + 1))
}

/// Collapse runs of 3+ newlines down to exactly two (one blank line), then add a
/// single trailing newline.
fn squeeze_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut newlines = 0;
    for c in s.chars() {
        if c == '\n' {
            newlines += 1;
            if newlines <= 2 {
                out.push('\n');
            }
        } else {
            newlines = 0;
            out.push(c);
        }
    }
    let trimmed = out.trim_end().to_string();
    if trimmed.is_empty() {
        String::new()
    } else {
        trimmed + "\n"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headings_map_to_sections() {
        let out = run("# Title\n\n## Sub").unwrap();
        assert!(out.contains("\\section{Title}"), "got: {out}");
        assert!(out.contains("\\subsection{Sub}"), "got: {out}");
    }

    #[test]
    fn heading_offset_shifts_levels() {
        let out = convert("# Title", false, 1).unwrap();
        assert!(out.contains("\\subsection{Title}"), "got: {out}");
        let out2 = convert("# Title", false, 2).unwrap();
        assert!(out2.contains("\\subsubsection{Title}"), "got: {out2}");
    }

    #[test]
    fn inline_bold_italic_code() {
        let out = run("This is **bold**, *italic*, and `code`.").unwrap();
        assert!(out.contains("\\textbf{bold}"), "got: {out}");
        assert!(out.contains("\\textit{italic}"), "got: {out}");
        assert!(out.contains("\\texttt{code}"), "got: {out}");
    }

    #[test]
    fn underscore_emphasis() {
        let out = run("a __b__ _c_").unwrap();
        assert!(out.contains("\\textbf{b}"), "got: {out}");
        assert!(out.contains("\\textit{c}"), "got: {out}");
    }

    #[test]
    fn special_chars_are_escaped() {
        let out = run("100% of a & b $5 #1 a_b {x} ~y ^z \\d").unwrap();
        assert!(out.contains("100\\%"), "got: {out}");
        assert!(out.contains("a \\& b"), "got: {out}");
        assert!(out.contains("\\$5"), "got: {out}");
        assert!(out.contains("\\#1"), "got: {out}");
        assert!(out.contains("a\\_b"), "got: {out}");
        assert!(out.contains("\\{x\\}"), "got: {out}");
        assert!(out.contains("\\textasciitilde{}"), "got: {out}");
        assert!(out.contains("\\textasciicircum{}"), "got: {out}");
        assert!(out.contains("\\textbackslash{}"), "got: {out}");
    }

    #[test]
    fn links_and_images() {
        let out = run("See [the site](https://ex.com/a#b).").unwrap();
        assert!(
            out.contains("\\href{https://ex.com/a\\#b}{the site}"),
            "got: {out}"
        );
        let img = run("![alt text](pic.png)").unwrap();
        assert!(img.contains("\\includegraphics"), "got: {img}");
        assert!(img.contains("{pic.png}"), "got: {img}");
    }

    #[test]
    fn autolink() {
        let out = run("<https://example.com>").unwrap();
        assert!(out.contains("\\url{https://example.com}"), "got: {out}");
    }

    #[test]
    fn unordered_list() {
        let out = run("- one\n- two\n- three").unwrap();
        assert!(out.contains("\\begin{itemize}"), "got: {out}");
        assert!(out.contains("\\item one"), "got: {out}");
        assert!(out.contains("\\end{itemize}"), "got: {out}");
    }

    #[test]
    fn ordered_list() {
        let out = run("1. first\n2. second").unwrap();
        assert!(out.contains("\\begin{enumerate}"), "got: {out}");
        assert!(out.contains("\\item first"), "got: {out}");
        assert!(out.contains("\\end{enumerate}"), "got: {out}");
    }

    #[test]
    fn nested_list() {
        let out = run("- a\n    - b\n    - c\n- d").unwrap();
        // The nested itemize is opened inside the outer one.
        assert_eq!(out.matches("\\begin{itemize}").count(), 2, "got: {out}");
        assert!(out.contains("\\item b"), "got: {out}");
    }

    #[test]
    fn task_list() {
        let out = run("- [x] done\n- [ ] todo").unwrap();
        assert!(out.contains("\\checkmark"), "got: {out}");
        assert!(out.contains("\\square"), "got: {out}");
    }

    #[test]
    fn fenced_code_block_with_lang() {
        let out = run("```rust\nlet x = 1; // 50% off\n```").unwrap();
        assert!(
            out.contains("\\begin{lstlisting}[language=rust]"),
            "got: {out}"
        );
        // Code is verbatim — specials NOT escaped inside listings.
        assert!(out.contains("let x = 1; // 50% off"), "got: {out}");
        assert!(out.contains("\\end{lstlisting}"), "got: {out}");
    }

    #[test]
    fn fenced_code_block_plain() {
        let out = run("```\nplain code\n```").unwrap();
        assert!(out.contains("\\begin{lstlisting}\n"), "got: {out}");
        assert!(!out.contains("language="), "got: {out}");
    }

    #[test]
    fn blockquote() {
        let out = run("> quoted **text**").unwrap();
        assert!(out.contains("\\begin{quote}"), "got: {out}");
        assert!(out.contains("\\textbf{text}"), "got: {out}");
        assert!(out.contains("\\end{quote}"), "got: {out}");
    }

    #[test]
    fn thematic_break() {
        let out = run("a\n\n---\n\nb").unwrap();
        assert!(out.contains("\\rule{\\textwidth}"), "got: {out}");
    }

    #[test]
    fn table_with_alignment() {
        let md = "| Name | Qty |\n|:-----|----:|\n| Apple | 5 |\n| Pear | 12 |";
        let out = run(md).unwrap();
        assert!(out.contains("\\begin{tabular}{lr}"), "got: {out}");
        assert!(out.contains("\\toprule"), "got: {out}");
        assert!(out.contains("Name & Qty"), "got: {out}");
        assert!(out.contains("Apple & 5"), "got: {out}");
        assert!(out.contains("\\bottomrule"), "got: {out}");
    }

    #[test]
    fn full_document_wraps_preamble() {
        let out = convert("# Hi", true, 0).unwrap();
        assert!(out.starts_with("\\documentclass{article}"), "got: {out}");
        assert!(out.contains("\\begin{document}"), "got: {out}");
        assert!(out.contains("\\section{Hi}"), "got: {out}");
        assert!(out.trim_end().ends_with("\\end{document}"), "got: {out}");
    }

    #[test]
    fn fragment_has_no_preamble() {
        let out = run("# Hi").unwrap();
        assert!(!out.contains("\\documentclass"), "got: {out}");
    }

    #[test]
    fn math_passthrough() {
        let out = run("Euler: $e^{i\\pi}+1=0$ and $$a^2+b^2=c^2$$").unwrap();
        assert!(out.contains("$e^{i\\pi}+1=0$"), "got: {out}");
        assert!(out.contains("$$a^2+b^2=c^2$$"), "got: {out}");
    }

    #[test]
    fn paragraph_soft_wrap() {
        let out = run("line one\nline two").unwrap();
        // Soft-wrapped into one paragraph, no forced break.
        assert!(out.contains("line one\nline two"), "got: {out}");
    }

    #[test]
    fn empty_input_ok() {
        let out = run("").unwrap();
        assert_eq!(out.trim(), "");
    }

    #[test]
    fn indented_code_block() {
        let out = run("text\n\n    code line\n    more code\n\nafter").unwrap();
        assert!(out.contains("\\begin{verbatim}"), "got: {out}");
        assert!(out.contains("code line"), "got: {out}");
    }

    #[test]
    fn strikethrough() {
        let out = run("This is ~~gone~~ now.").unwrap();
        assert!(out.contains("\\sout{gone}"), "got: {out}");
        // A lone ~~ with no close escapes literally (no \sout).
        let out2 = run("a ~~ b").unwrap();
        assert!(!out2.contains("\\sout"), "got: {out2}");
        assert!(out2.contains("\\textasciitilde{}"), "got: {out2}");
    }

    #[test]
    fn footnote_reference_and_definition() {
        let out = run("Text with a note[^1].\n\n[^1]: The note body.").unwrap();
        assert!(out.contains("\\footnote{The note body.}"), "got: {out}");
        // The definition line itself is removed from the body.
        assert!(!out.contains("[^1]:"), "got: {out}");
    }

    #[test]
    fn footnote_with_inline_formatting_in_definition() {
        let out = run("See[^n].\n\n[^n]: a **bold** note").unwrap();
        assert!(out.contains("\\footnote{a \\textbf{bold} note}"), "got: {out}");
    }

    #[test]
    fn unresolved_footnote_ref_is_literal() {
        // No matching definition → the [^x] is emitted as escaped literal text.
        let out = run("dangling[^x] ref").unwrap();
        assert!(!out.contains("\\footnote"), "got: {out}");
    }

    #[test]
    fn setext_headings() {
        let out = run("Big Title\n=========\n\nSub Title\n---------").unwrap();
        assert!(out.contains("\\section{Big Title}"), "got: {out}");
        assert!(out.contains("\\subsection{Sub Title}"), "got: {out}");
    }

    #[test]
    fn setext_does_not_eat_thematic_break() {
        // A `---` after a blank line stays a thematic break, not a heading.
        let out = run("para\n\n---\n\nnext").unwrap();
        assert!(out.contains("\\rule{\\textwidth}"), "got: {out}");
        assert!(!out.contains("\\subsection{para}"), "got: {out}");
    }

    #[test]
    fn full_document_includes_ulem_for_strikethrough() {
        let out = convert("~~x~~", true, 0).unwrap();
        assert!(out.contains("\\usepackage[normalem]{ulem}"), "got: {out}");
    }
}
