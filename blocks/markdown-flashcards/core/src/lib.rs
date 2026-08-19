//! markdown-flashcards core — turn Q/A-shaped Markdown notes into a flashcard deck and
//! render it as an Anki-importable text file (TSV/CSV with `#` header directives), a
//! human-readable preview, or JSON. Pure compute, no deps: shared by the chat/CLI block
//! and the browser page.

pub const MAX_INPUT_CHARS: usize = 1_000_000;
pub const MAX_CARDS: usize = 5_000;

/// One flashcard: first field (question / cloze text), second field (answer), and tags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub front: String,
    pub back: String,
    pub tags: Vec<String>,
}

/// How the notes are cut into cards.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Auto,
    Heading,
    Separator,
    Qa,
    Table,
    Cloze,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Mode::Auto),
            "heading" | "headings" => Ok(Mode::Heading),
            "separator" | "delimiter" => Ok(Mode::Separator),
            "qa" | "q&a" | "question-answer" => Ok(Mode::Qa),
            "table" => Ok(Mode::Table),
            "cloze" => Ok(Mode::Cloze),
            other => Err(format!(
                "unknown mode '{other}' (expected auto, heading, separator, qa, table or cloze)"
            )),
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Mode::Auto => "auto",
            Mode::Heading => "heading",
            Mode::Separator => "separator",
            Mode::Qa => "qa",
            Mode::Table => "table",
            Mode::Cloze => "cloze",
        }
    }
}

/// The character between the exported fields (Anki calls this the separator).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldSep {
    Tab,
    Comma,
    Semicolon,
    Pipe,
}

impl FieldSep {
    pub fn parse(s: &str) -> Result<FieldSep, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "tab" | "tsv" | "\t" => Ok(FieldSep::Tab),
            "comma" | "csv" | "," => Ok(FieldSep::Comma),
            "semicolon" | ";" => Ok(FieldSep::Semicolon),
            "pipe" | "|" => Ok(FieldSep::Pipe),
            other => Err(format!(
                "unknown field_separator '{other}' (expected tab, comma, semicolon or pipe)"
            )),
        }
    }
    pub fn ch(self) -> char {
        match self {
            FieldSep::Tab => '\t',
            FieldSep::Comma => ',',
            FieldSep::Semicolon => ';',
            FieldSep::Pipe => '|',
        }
    }
    /// The spelling Anki's `#separator:` directive expects.
    pub fn anki_name(self) -> &'static str {
        match self {
            FieldSep::Tab => "Tab",
            FieldSep::Comma => "Comma",
            FieldSep::Semicolon => "Semicolon",
            FieldSep::Pipe => "Pipe",
        }
    }
}

/// How each field's Markdown is rendered into the exported field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldFormat {
    Html,
    Markdown,
    Plain,
}

impl FieldFormat {
    pub fn parse(s: &str) -> Result<FieldFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "html" => Ok(FieldFormat::Html),
            "markdown" | "md" | "raw" => Ok(FieldFormat::Markdown),
            "plain" | "text" => Ok(FieldFormat::Plain),
            other => Err(format!(
                "unknown field_format '{other}' (expected html, markdown or plain)"
            )),
        }
    }
}

/// What the tool returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    Anki,
    Preview,
    Json,
}

impl OutputKind {
    pub fn parse(s: &str) -> Result<OutputKind, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "anki" | "file" | "csv" => Ok(OutputKind::Anki),
            "preview" => Ok(OutputKind::Preview),
            "json" => Ok(OutputKind::Json),
            other => Err(format!(
                "unknown output '{other}' (expected anki, preview or json)"
            )),
        }
    }
}

pub const NOTETYPES: [&str; 4] = [
    "Basic",
    "Basic (and reversed card)",
    "Basic (type in the answer)",
    "Cloze",
];

fn normalize_notetype(s: &str) -> Result<String, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok("Basic".to_string());
    }
    for n in NOTETYPES {
        if n.eq_ignore_ascii_case(t) {
            return Ok(n.to_string());
        }
    }
    Err(format!(
        "unknown notetype '{t}' (expected one of: {})",
        NOTETYPES.join(", ")
    ))
}

#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    /// `auto`, a name (`tab`, `colon`, `dash`, `pipe`, `semicolon`, `arrow`) or a literal string.
    pub separator: String,
    /// 0 = auto-detect; 1..=6 pins the heading level used as the question.
    pub heading_level: u8,
    pub field_separator: FieldSep,
    pub field_format: FieldFormat,
    pub notetype: String,
    pub deck: String,
    pub tags: String,
    pub tags_from_headings: bool,
    pub include_headers: bool,
    pub dedupe: bool,
    pub output: OutputKind,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Auto,
            separator: "auto".to_string(),
            heading_level: 0,
            field_separator: FieldSep::Tab,
            field_format: FieldFormat::Html,
            notetype: "Basic".to_string(),
            deck: String::new(),
            tags: String::new(),
            tags_from_headings: false,
            include_headers: true,
            dedupe: true,
            output: OutputKind::Anki,
        }
    }
}

// ---------------------------------------------------------------- line helpers

fn strip_bullet(line: &str) -> &str {
    let t = line.trim();
    for p in ["- [ ] ", "- [x] ", "- [X] ", "- ", "* ", "+ ", "> "] {
        if let Some(r) = t.strip_prefix(p) {
            return r.trim_start();
        }
    }
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i > 0 && i < b.len() && (b[i] == b'.' || b[i] == b')') {
        let rest = t[i + 1..].trim_start();
        if !rest.is_empty() {
            return rest;
        }
    }
    t
}

fn is_bullet(line: &str) -> bool {
    let t = line.trim();
    t.starts_with("- ") || t.starts_with("* ") || t.starts_with("+ ")
}

fn is_ordered_item(line: &str) -> bool {
    let t = line.trim();
    let b = t.as_bytes();
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    i > 0 && i < b.len() && (b[i] == b'.' || b[i] == b')') && t[i + 1..].starts_with(' ')
}

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn heading_of(line: &str) -> Option<(u8, &str)> {
    let t = line.trim_start();
    if !t.starts_with('#') {
        return None;
    }
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.starts_with(' ') {
        return None; // `#tag`, not a heading
    }
    let title = rest.trim().trim_end_matches('#').trim();
    if title.is_empty() {
        return None;
    }
    Some((hashes as u8, title))
}

fn is_delim_row(line: &str) -> bool {
    let t = line.trim();
    if !t.contains('-') || !t.contains('|') {
        return false;
    }
    t.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ' | '\t'))
}

fn split_row(line: &str) -> Vec<String> {
    let mut t = line.trim();
    if t.starts_with('|') {
        t = &t[1..];
    }
    if t.ends_with('|') && !t.ends_with("\\|") {
        t = &t[..t.len() - 1];
    }
    let mut cells: Vec<String> = vec![String::new()];
    let mut escaped = false;
    for c in t.chars() {
        if escaped {
            if c != '|' {
                cells.last_mut().unwrap().push('\\');
            }
            cells.last_mut().unwrap().push(c);
            escaped = false;
        } else if c == '\\' {
            escaped = true;
        } else if c == '|' {
            cells.push(String::new());
        } else {
            cells.last_mut().unwrap().push(c);
        }
    }
    if escaped {
        cells.last_mut().unwrap().push('\\');
    }
    cells.iter().map(|c| c.trim().to_string()).collect()
}

fn sanitize_tag(s: &str) -> String {
    let mut out = String::new();
    let mut last_us = false;
    for c in s.trim().chars() {
        if c.is_whitespace() {
            if !last_us && !out.is_empty() {
                out.push('_');
                last_us = true;
            }
        } else if c == '"' || c == '\'' || c == ',' {
            continue;
        } else {
            out.push(c);
            last_us = false;
        }
    }
    while out.ends_with('_') {
        out.pop();
    }
    out
}

/// `auto` → None; otherwise the literal separator string to split on.
fn resolve_separator(s: &str) -> Result<Option<String>, String> {
    let raw = s.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("auto") {
        return Ok(None);
    }
    let lit = match raw.to_ascii_lowercase().as_str() {
        "tab" | "\\t" => "\t".to_string(),
        "colon" => ":".to_string(),
        "double-colon" | "doublecolon" => "::".to_string(),
        "semicolon" => ";".to_string(),
        "pipe" => "|".to_string(),
        "comma" => ",".to_string(),
        "dash" | "hyphen" => " - ".to_string(),
        "arrow" => "=>".to_string(),
        _ => raw.to_string(),
    };
    if lit.is_empty() {
        return Err("separator must not be empty (use 'auto' to detect it)".to_string());
    }
    Ok(Some(lit))
}

const SEP_CANDIDATES: [&str; 7] = ["::", "=>", "\t", "|", ";", " - ", ":"];

fn splittable(line: &str, sep: &str) -> Option<(String, String)> {
    let t = strip_bullet(line);
    if t.is_empty() || heading_of(line).is_some() || is_delim_row(line) || is_fence(line) {
        return None;
    }
    let idx = t.find(sep)?;
    let front = t[..idx].trim();
    let back = t[idx + sep.len()..].trim();
    if front.is_empty() || back.is_empty() {
        return None;
    }
    Some((front.to_string(), back.to_string()))
}

fn count_with_sep(lines: &[&str], sep: &str) -> usize {
    let mut n = 0;
    let mut in_fence = false;
    for l in lines {
        if is_fence(l) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if splittable(l, sep).is_some() {
            n += 1;
        }
    }
    n
}

fn best_separator(lines: &[&str]) -> Option<(String, usize)> {
    let mut best: Option<(String, usize)> = None;
    for cand in SEP_CANDIDATES {
        let n = count_with_sep(lines, cand);
        if n == 0 {
            continue;
        }
        if best.as_ref().map(|(_, bn)| n > *bn).unwrap_or(true) {
            best = Some((cand.to_string(), n));
        }
    }
    best
}

// ---------------------------------------------------------------- Q:/A: prefixes

fn qa_prefix(line: &str) -> Option<(bool, String)> {
    let t = strip_bullet(line);
    let t = t.trim_start_matches("**").trim_start_matches('_').trim_start();
    let lower = t.to_ascii_lowercase();
    for (kw, is_q) in [("question", true), ("answer", false), ("q", true), ("a", false)] {
        if !lower.starts_with(kw) {
            continue;
        }
        let rest = t[kw.len()..].trim_start_matches("**").trim_start_matches('_');
        let mut it = rest.chars();
        match it.next() {
            Some(c) if c == ':' || c == '.' || c == ')' => {
                let after = it
                    .as_str()
                    .trim_start()
                    .trim_start_matches("**")
                    .trim_start_matches('_')
                    .trim();
                return Some((is_q, after.trim_end_matches("**").trim().to_string()));
            }
            _ => {}
        }
    }
    None
}

// ---------------------------------------------------------------- heading blocks

struct HBlock {
    level: u8,
    title: String,
    body: String,
    ancestors: Vec<String>,
}

fn heading_blocks(lines: &[&str]) -> Vec<HBlock> {
    let mut stack: Vec<(u8, String)> = Vec::new();
    let mut blocks: Vec<HBlock> = Vec::new();
    let mut cur: Option<(u8, String, Vec<String>, Vec<String>)> = None;
    let mut in_fence = false;
    for line in lines {
        if is_fence(line) {
            in_fence = !in_fence;
        }
        let h = if in_fence { None } else { heading_of(line) };
        if let Some((lvl, title)) = h {
            if let Some((l, ti, anc, body)) = cur.take() {
                blocks.push(HBlock {
                    level: l,
                    title: ti,
                    body: body.join("\n").trim().to_string(),
                    ancestors: anc,
                });
            }
            while let Some((sl, _)) = stack.last() {
                if *sl >= lvl {
                    stack.pop();
                } else {
                    break;
                }
            }
            let anc: Vec<String> = stack.iter().map(|(_, t)| t.clone()).collect();
            stack.push((lvl, title.to_string()));
            cur = Some((lvl, title.to_string(), anc, Vec::new()));
        } else if let Some((_, _, _, body)) = cur.as_mut() {
            body.push(line.to_string());
        }
    }
    if let Some((l, ti, anc, body)) = cur.take() {
        blocks.push(HBlock {
            level: l,
            title: ti,
            body: body.join("\n").trim().to_string(),
            ancestors: anc,
        });
    }
    blocks
}

fn auto_heading_level(blocks: &[HBlock]) -> Option<u8> {
    let mut best: Option<(u8, usize)> = None;
    for lvl in 1..=6u8 {
        let n = blocks
            .iter()
            .filter(|b| b.level == lvl && !b.body.is_empty())
            .count();
        if n == 0 {
            continue;
        }
        if best.map(|(_, bn)| n > bn).unwrap_or(true) {
            best = Some((lvl, n));
        }
    }
    best.map(|(l, _)| l)
}

// ---------------------------------------------------------------- parsers

fn parse_heading(lines: &[&str], opts: &Options) -> Result<Vec<Card>, String> {
    let blocks = heading_blocks(lines);
    if blocks.is_empty() {
        return Err("no Markdown headings found — heading mode expects `## Question` lines with the answer text underneath".to_string());
    }
    let level = if opts.heading_level > 0 {
        opts.heading_level
    } else {
        auto_heading_level(&blocks).ok_or_else(|| {
            "found headings but none had any text under them — heading mode uses the heading as the question and the lines below it as the answer".to_string()
        })?
    };
    let cards: Vec<Card> = blocks
        .iter()
        .filter(|b| b.level == level && !b.body.is_empty())
        .map(|b| Card {
            front: b.title.clone(),
            back: b.body.clone(),
            tags: heading_tags(b, opts),
        })
        .collect();
    if cards.is_empty() {
        return Err(format!(
            "no level-{level} headings with text under them — try heading_level=0 (auto) or another level"
        ));
    }
    Ok(cards)
}

fn heading_tags(b: &HBlock, opts: &Options) -> Vec<String> {
    if !opts.tags_from_headings {
        return Vec::new();
    }
    let parts: Vec<String> = b
        .ancestors
        .iter()
        .map(|a| sanitize_tag(a))
        .filter(|a| !a.is_empty())
        .collect();
    if parts.is_empty() {
        Vec::new()
    } else {
        vec![parts.join("::")]
    }
}

fn parse_qa(lines: &[&str]) -> Result<Vec<Card>, String> {
    let mut cards: Vec<Card> = Vec::new();
    let mut front = String::new();
    let mut back = String::new();
    let mut in_answer = false;
    for line in lines {
        if let Some((is_q, rest)) = qa_prefix(line) {
            if is_q {
                push_card(&mut cards, &front, &back);
                front = rest;
                back.clear();
                in_answer = false;
            } else {
                back = rest;
                in_answer = true;
            }
            continue;
        }
        if front.is_empty() {
            continue;
        }
        let t = line.trim();
        if t.is_empty() {
            if in_answer && !back.is_empty() {
                back.push('\n');
            }
            continue;
        }
        if in_answer {
            if !back.is_empty() {
                back.push('\n');
            }
            back.push_str(line.trim_end());
        } else {
            front.push(' ');
            front.push_str(t);
        }
    }
    push_card(&mut cards, &front, &back);
    if cards.is_empty() {
        return Err("no `Q:` / `A:` pairs found — qa mode expects lines like `Q: your question` followed by `A: the answer`".to_string());
    }
    Ok(cards)
}

fn push_card(cards: &mut Vec<Card>, front: &str, back: &str) {
    let f = front.trim();
    let b = back.trim();
    if !f.is_empty() && !b.is_empty() {
        cards.push(Card {
            front: f.to_string(),
            back: b.to_string(),
            tags: Vec::new(),
        });
    }
}

fn parse_table(lines: &[&str]) -> Result<Vec<Card>, String> {
    let mut groups: Vec<Vec<&str>> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    for l in lines {
        if l.contains('|') && !l.trim().is_empty() {
            cur.push(l);
        } else if !cur.is_empty() {
            groups.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        groups.push(cur);
    }
    let mut cards: Vec<Card> = Vec::new();
    for g in groups {
        let has_delim = g.iter().any(|l| is_delim_row(l));
        for (idx, l) in g.iter().enumerate() {
            if is_delim_row(l) {
                continue;
            }
            if has_delim && idx + 1 < g.len() && is_delim_row(g[idx + 1]) {
                continue; // the header row directly above `| --- | --- |`
            }
            let cells = split_row(l);
            if cells.len() < 2 {
                continue;
            }
            let front = cells[0].trim().to_string();
            let back = cells[1].trim().to_string();
            if front.is_empty() || back.is_empty() {
                continue;
            }
            let tags: Vec<String> = cells
                .get(2)
                .map(|t| {
                    t.split([' ', ','])
                        .map(sanitize_tag)
                        .filter(|s| !s.is_empty())
                        .collect()
                })
                .unwrap_or_default();
            cards.push(Card { front, back, tags });
        }
    }
    if cards.is_empty() {
        return Err("no table rows found — table mode expects Markdown rows like `| question | answer |` (an optional third column holds tags)".to_string());
    }
    Ok(cards)
}

fn parse_separator(lines: &[&str], sep: &str) -> Result<Vec<Card>, String> {
    let mut cards: Vec<Card> = Vec::new();
    let mut in_fence = false;
    for l in lines {
        if is_fence(l) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some((front, back)) = splittable(l, sep) {
            cards.push(Card {
                front,
                back,
                tags: Vec::new(),
            });
        }
    }
    if cards.is_empty() {
        let shown = if sep == "\t" { "tab".to_string() } else { sep.to_string() };
        return Err(format!(
            "no lines could be split on '{shown}' — separator mode expects one card per line, e.g. `term{shown}definition`"
        ));
    }
    Ok(cards)
}

/// Replace `**bold**` / `==highlight==` spans with Anki cloze deletions.
fn clozeify(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut n = 1;
    while i < chars.len() {
        let marker = if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            Some("**")
        } else if chars[i] == '=' && i + 1 < chars.len() && chars[i + 1] == '=' {
            Some("==")
        } else {
            None
        };
        if let Some(m) = marker {
            let mc: Vec<char> = m.chars().collect();
            let mut j = i + 2;
            let mut end = None;
            while j + 1 < chars.len() {
                if chars[j] == mc[0] && chars[j + 1] == mc[1] {
                    end = Some(j);
                    break;
                }
                j += 1;
            }
            if let Some(j) = end {
                if j > i + 2 {
                    let inner: String = chars[i + 2..j].iter().collect();
                    out.push_str(&format!("{{{{c{n}::{inner}}}}}"));
                    n += 1;
                    i = j + 2;
                    continue;
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    if n == 1 {
        None
    } else {
        Some(out)
    }
}

fn parse_cloze(lines: &[&str]) -> Result<Vec<Card>, String> {
    let mut cards: Vec<Card> = Vec::new();
    let mut in_fence = false;
    for l in lines {
        if is_fence(l) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence || l.trim().is_empty() {
            continue;
        }
        let text = match heading_of(l) {
            Some(_) => continue,
            None => strip_bullet(l),
        };
        if let Some(c) = clozeify(text) {
            cards.push(Card {
                front: c,
                back: String::new(),
                tags: Vec::new(),
            });
        }
    }
    if cards.is_empty() {
        return Err("no `**bold**` or `==highlighted==` text found — cloze mode turns each emphasised span into a `{{c1::…}}` deletion".to_string());
    }
    Ok(cards)
}

// ---------------------------------------------------------------- field rendering

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

fn find_char(chars: &[char], from: usize, target: char) -> Option<usize> {
    (from..chars.len()).find(|&i| chars[i] == target)
}

fn find_pair(chars: &[char], from: usize, a: char, b: char) -> Option<usize> {
    (from..chars.len().saturating_sub(1)).find(|&i| chars[i] == a && chars[i + 1] == b)
}

/// Attribute values are single-quoted so the exported field stays free of `"` (which would
/// force RFC-4180 quoting on every row that contains a link or an image).
fn attr_escape(s: &str) -> String {
    s.replace('\'', "&#39;")
}

/// `[text](url)` starting at `chars[i] == '['` → (text, url, index after `)`).
fn parse_link(chars: &[char], i: usize) -> Option<(String, String, usize)> {
    let close = find_char(chars, i + 1, ']')?;
    if close + 1 >= chars.len() || chars[close + 1] != '(' {
        return None;
    }
    let end = find_char(chars, close + 2, ')')?;
    let text: String = chars[i + 1..close].iter().collect();
    let url: String = chars[close + 2..end].iter().collect();
    if url.trim().is_empty() {
        return None;
    }
    Some((text, url.trim().to_string(), end + 1))
}

fn inline_html(s: &str) -> String {
    let chars: Vec<char> = html_escape(s).chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            if let Some(j) = find_char(&chars, i + 1, '`') {
                if j > i + 1 {
                    let inner: String = chars[i + 1..j].iter().collect();
                    out.push_str(&format!("<code>{inner}</code>"));
                    i = j + 1;
                    continue;
                }
            }
        }
        if c == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some((alt, url, next)) = parse_link(&chars, i + 1) {
                out.push_str(&format!(
                    "<img src='{}' alt='{}'>",
                    attr_escape(&url),
                    attr_escape(&alt)
                ));
                i = next;
                continue;
            }
        }
        if c == '[' {
            if let Some((text, url, next)) = parse_link(&chars, i) {
                out.push_str(&format!("<a href='{}'>{text}</a>", attr_escape(&url)));
                i = next;
                continue;
            }
        }
        if c == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(j) = find_pair(&chars, i + 2, '*', '*') {
                if j > i + 2 {
                    let inner: String = chars[i + 2..j].iter().collect();
                    out.push_str(&format!("<b>{inner}</b>"));
                    i = j + 2;
                    continue;
                }
            }
        }
        if c == '*' {
            if let Some(j) = find_char(&chars, i + 1, '*') {
                if j > i + 1 {
                    let inner: String = chars[i + 1..j].iter().collect();
                    out.push_str(&format!("<i>{inner}</i>"));
                    i = j + 1;
                    continue;
                }
            }
        }
        if c == '_' && (i == 0 || chars[i - 1].is_whitespace()) {
            if let Some(j) = find_char(&chars, i + 1, '_') {
                let ends_word = j + 1 >= chars.len() || !chars[j + 1].is_alphanumeric();
                if j > i + 1 && ends_word {
                    let inner: String = chars[i + 1..j].iter().collect();
                    out.push_str(&format!("<i>{inner}</i>"));
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn to_html(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = String::new();
    let mut i = 0;
    let mut blank = false;
    let mut prev_text = false;
    while i < lines.len() {
        let line = lines[i];
        let t = line.trim();
        if is_fence(line) {
            let fence: String = t.chars().take(3).collect();
            let mut code = String::new();
            i += 1;
            while i < lines.len() && !lines[i].trim_start().starts_with(&fence) {
                code.push_str(lines[i]);
                code.push('\n');
                i += 1;
            }
            if i < lines.len() {
                i += 1;
            }
            out.push_str(&format!(
                "<pre><code>{}</code></pre>",
                html_escape(code.trim_end_matches('\n'))
            ));
            blank = false;
            prev_text = false;
            continue;
        }
        if is_bullet(line) || is_ordered_item(line) {
            let ordered = is_ordered_item(line);
            let tag = if ordered { "ol" } else { "ul" };
            out.push_str(&format!("<{tag}>"));
            while i < lines.len()
                && (is_bullet(lines[i]) || is_ordered_item(lines[i]))
                && is_ordered_item(lines[i]) == ordered
            {
                out.push_str(&format!("<li>{}</li>", inline_html(strip_bullet(lines[i]))));
                i += 1;
            }
            out.push_str(&format!("</{tag}>"));
            blank = false;
            prev_text = false;
            continue;
        }
        if t.is_empty() {
            if !out.is_empty() {
                blank = true;
            }
            i += 1;
            continue;
        }
        if prev_text {
            out.push_str("<br>");
            if blank {
                out.push_str("<br>");
            }
        }
        blank = false;
        out.push_str(&inline_html(t));
        prev_text = true;
        i += 1;
    }
    out
}

fn strip_inline(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '`' {
            if let Some(j) = find_char(&chars, i + 1, '`') {
                if j > i + 1 {
                    out.extend(chars[i + 1..j].iter());
                    i = j + 1;
                    continue;
                }
            }
        }
        if c == '!' && i + 1 < chars.len() && chars[i + 1] == '[' {
            if let Some((alt, _url, next)) = parse_link(&chars, i + 1) {
                out.push_str(&alt);
                i = next;
                continue;
            }
        }
        if c == '[' {
            if let Some((text, _url, next)) = parse_link(&chars, i) {
                out.push_str(&text);
                i = next;
                continue;
            }
        }
        if c == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(j) = find_pair(&chars, i + 2, '*', '*') {
                if j > i + 2 {
                    out.extend(chars[i + 2..j].iter());
                    i = j + 2;
                    continue;
                }
            }
        }
        if c == '*' {
            if let Some(j) = find_char(&chars, i + 1, '*') {
                if j > i + 1 {
                    out.extend(chars[i + 1..j].iter());
                    i = j + 1;
                    continue;
                }
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

fn to_plain(text: &str) -> String {
    let mut kept: Vec<String> = Vec::new();
    for line in text.lines() {
        if is_fence(line) {
            continue;
        }
        let t = match heading_of(line) {
            Some((_, title)) => title.to_string(),
            None => line.trim_end().to_string(),
        };
        kept.push(strip_inline(&t));
    }
    kept.join("\n").trim().to_string()
}

fn format_field(text: &str, fmt: FieldFormat) -> String {
    match fmt {
        FieldFormat::Html => to_html(text),
        FieldFormat::Markdown => text.trim().to_string(),
        FieldFormat::Plain => to_plain(text),
    }
}

// ---------------------------------------------------------------- rendering

fn csv_field(s: &str, sep: char) -> String {
    let needs = s.contains(sep)
        || s.contains('"')
        || s.contains('\n')
        || s.contains('\r')
        || s.starts_with(' ')
        || s.ends_with(' ');
    if needs {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn render_anki(cards: &[Card], opts: &Options, notetype: &str, has_tags: bool) -> String {
    let sep = opts.field_separator.ch();
    let mut lines: Vec<String> = Vec::new();
    if opts.include_headers {
        lines.push(format!("#separator:{}", opts.field_separator.anki_name()));
        lines.push(format!(
            "#html:{}",
            if opts.field_format == FieldFormat::Html { "true" } else { "false" }
        ));
        lines.push(format!("#notetype:{notetype}"));
        let deck = opts.deck.trim();
        if !deck.is_empty() {
            lines.push(format!("#deck:{deck}"));
        }
        let tags = normalize_global_tags(&opts.tags);
        if !tags.is_empty() {
            lines.push(format!("#tags:{}", tags.join(" ")));
        }
        let cols: Vec<&str> = if notetype == "Cloze" {
            vec!["Text", "Back Extra"]
        } else {
            vec!["Front", "Back"]
        };
        let mut cols: Vec<String> = cols.iter().map(|c| c.to_string()).collect();
        if has_tags {
            cols.push("Tags".to_string());
        }
        lines.push(format!("#columns:{}", cols.join(&sep.to_string())));
        if has_tags {
            lines.push("#tags column:3".to_string());
        }
    }
    for c in cards {
        let mut row = vec![csv_field(&c.front, sep), csv_field(&c.back, sep)];
        if has_tags {
            row.push(csv_field(&c.tags.join(" "), sep));
        }
        lines.push(row.join(&sep.to_string()));
    }
    lines.join("\n")
}

fn normalize_global_tags(s: &str) -> Vec<String> {
    s.split([' ', ',', '\t', '\n'])
        .map(sanitize_tag)
        .filter(|t| !t.is_empty())
        .collect()
}

fn render_preview(cards: &[Card], mode: Mode, notetype: &str) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "{} card{} · mode: {} · note type: {}\n",
        cards.len(),
        if cards.len() == 1 { "" } else { "s" },
        mode.name(),
        notetype
    ));
    for (i, c) in cards.iter().enumerate() {
        out.push('\n');
        out.push_str(&format!("{}. Q: {}\n", i + 1, c.front.replace('\n', "\n      ")));
        out.push_str(&format!("   A: {}\n", c.back.replace('\n', "\n      ")));
        if !c.tags.is_empty() {
            out.push_str(&format!("   Tags: {}\n", c.tags.join(" ")));
        }
    }
    out.trim_end().to_string()
}

fn render_json(cards: &[Card], mode: Mode, notetype: &str) -> String {
    let mut out = String::new();
    out.push_str("{\n");
    out.push_str(&format!("  \"mode\": \"{}\",\n", mode.name()));
    out.push_str(&format!("  \"notetype\": \"{}\",\n", json_escape(notetype)));
    out.push_str(&format!("  \"count\": {},\n", cards.len()));
    out.push_str("  \"cards\": [\n");
    for (i, c) in cards.iter().enumerate() {
        let tags: Vec<String> = c
            .tags
            .iter()
            .map(|t| format!("\"{}\"", json_escape(t)))
            .collect();
        out.push_str(&format!(
            "    {{ \"front\": \"{}\", \"back\": \"{}\", \"tags\": [{}] }}{}\n",
            json_escape(&c.front),
            json_escape(&c.back),
            tags.join(", "),
            if i + 1 == cards.len() { "" } else { "," }
        ));
    }
    out.push_str("  ]\n}");
    out
}

// ---------------------------------------------------------------- entry point

fn detect_mode(lines: &[&str], sep_override: Option<&str>) -> Result<Mode, String> {
    if lines.iter().any(|l| matches!(qa_prefix(l), Some((true, _)))) {
        return Ok(Mode::Qa);
    }
    if lines.iter().any(|l| is_delim_row(l)) && lines.iter().filter(|l| l.contains('|')).count() >= 2
    {
        return Ok(Mode::Table);
    }
    let heading_count = auto_heading_level(&heading_blocks(lines))
        .map(|lvl| {
            heading_blocks(lines)
                .iter()
                .filter(|b| b.level == lvl && !b.body.is_empty())
                .count()
        })
        .unwrap_or(0);
    let sep_count = match sep_override {
        Some(s) => count_with_sep(lines, s),
        None => best_separator(lines).map(|(_, n)| n).unwrap_or(0),
    };
    if heading_count == 0 && sep_count == 0 {
        return Err("could not detect a flashcard format — set mode to heading, separator, qa, table or cloze, or use notes like `## Question` + answer, `Q:`/`A:` lines, `| question | answer |` rows, or `term :: definition` lines".to_string());
    }
    if sep_count > heading_count {
        Ok(Mode::Separator)
    } else {
        Ok(Mode::Heading)
    }
}

/// Parse `markdown` into cards and render them per `opts`.
pub fn generate(markdown: &str, opts: &Options) -> Result<String, String> {
    let (cards, mode, notetype) = build(markdown, opts)?;
    let has_tags = cards.iter().any(|c| !c.tags.is_empty());
    Ok(match opts.output {
        OutputKind::Anki => render_anki(&cards, opts, &notetype, has_tags),
        OutputKind::Preview => render_preview(&cards, mode, &notetype),
        OutputKind::Json => render_json(&cards, mode, &notetype),
    })
}

/// Parse `markdown` into the card list plus the resolved mode and note type.
pub fn build(markdown: &str, opts: &Options) -> Result<(Vec<Card>, Mode, String), String> {
    if markdown.chars().count() > MAX_INPUT_CHARS {
        return Err(format!(
            "notes are too long ({} characters); the limit is {MAX_INPUT_CHARS} characters — split them into smaller batches",
            markdown.chars().count()
        ));
    }
    if opts.heading_level > 6 {
        return Err(format!(
            "heading_level must be 0-6 (0 = auto-detect), got {}",
            opts.heading_level
        ));
    }
    let lines: Vec<&str> = markdown.lines().map(|l| l.trim_end_matches('\r')).collect();
    if lines.iter().all(|l| l.trim().is_empty()) {
        return Err("no notes provided — paste Markdown notes with `## Question` headings, `Q:`/`A:` lines, a `| question | answer |` table, or `term :: definition` lines".to_string());
    }
    let notetype = normalize_notetype(&opts.notetype)?;
    let sep_override = resolve_separator(&opts.separator)?;
    let mode = if opts.mode == Mode::Auto {
        detect_mode(&lines, sep_override.as_deref())?
    } else {
        opts.mode
    };
    let mut cards = match mode {
        Mode::Heading => parse_heading(&lines, opts)?,
        Mode::Qa => parse_qa(&lines)?,
        Mode::Table => parse_table(&lines)?,
        Mode::Cloze => parse_cloze(&lines)?,
        Mode::Separator => {
            let sep = match &sep_override {
                Some(s) => s.clone(),
                None => best_separator(&lines)
                    .map(|(s, _)| s)
                    .ok_or_else(|| "could not detect a separator — set separator to the character between the question and the answer (e.g. `::`, `|`, `-`, tab)".to_string())?,
            };
            parse_separator(&lines, &sep)?
        }
        Mode::Auto => unreachable!("auto is resolved above"),
    };
    let notetype = if mode == Mode::Cloze && notetype == "Basic" {
        "Cloze".to_string()
    } else {
        notetype
    };
    for c in cards.iter_mut() {
        c.front = format_field(&c.front, opts.field_format);
        c.back = format_field(&c.back, opts.field_format);
    }
    if opts.dedupe {
        let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
        cards.retain(|c| seen.insert(c.front.trim().to_lowercase()));
    }
    // Without the header directives there is nowhere to put deck-wide tags, so fold them
    // into every card's Tags column instead of silently dropping them.
    if !opts.include_headers {
        let global = normalize_global_tags(&opts.tags);
        if !global.is_empty() {
            for c in cards.iter_mut() {
                for g in &global {
                    if !c.tags.contains(g) {
                        c.tags.push(g.clone());
                    }
                }
            }
        }
    }
    if cards.len() > MAX_CARDS {
        return Err(format!(
            "these notes produced {} cards, over the {MAX_CARDS}-card limit — split them into smaller batches",
            cards.len()
        ));
    }
    Ok((cards, mode, notetype))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn heading_notes_become_tsv_with_directives() {
        let md = "## What is Rust?\nA systems language.\n\n## What is wasm?\nA portable bytecode.\n";
        let out = generate(md, &opts()).unwrap();
        assert_eq!(
            out,
            "#separator:Tab\n#html:true\n#notetype:Basic\n#columns:Front\tBack\n\
             What is Rust?\tA systems language.\nWhat is wasm?\tA portable bytecode."
        );
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = generate("   \n\n", &opts()).unwrap_err();
        assert!(err.contains("no notes provided"), "{err}");
    }

    #[test]
    fn unknown_mode_is_an_error() {
        assert!(Mode::parse("banana").unwrap_err().contains("unknown mode"));
        assert!(FieldSep::parse("nope").unwrap_err().contains("field_separator"));
        assert!(FieldFormat::parse("nope").unwrap_err().contains("field_format"));
        assert!(OutputKind::parse("nope").unwrap_err().contains("unknown output"));
    }

    #[test]
    fn unknown_notetype_is_an_error() {
        let mut o = opts();
        o.notetype = "Fancy".to_string();
        let err = generate("Q: a\nA: b", &o).unwrap_err();
        assert!(err.contains("unknown notetype"), "{err}");
    }

    #[test]
    fn qa_prefix_mode_detected() {
        let md = "Q: Capital of France?\nA: Paris\n\nQ: Capital of Japan?\nA: Tokyo\n";
        let mut o = opts();
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "Capital of France?\tParis\nCapital of Japan?\tTokyo");
    }

    #[test]
    fn qa_answer_can_span_lines() {
        let md = "Q: Name two planets\nA: Mars\nVenus\n";
        let mut o = opts();
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "Name two planets\tMars<br>Venus");
    }

    #[test]
    fn separator_mode_autodetects_double_colon() {
        let md = "photosynthesis :: how plants make sugar\nmitosis :: cell division\n";
        let mut o = opts();
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(
            out,
            "photosynthesis\thow plants make sugar\nmitosis\tcell division"
        );
    }

    #[test]
    fn separator_mode_strips_bullets_and_honours_explicit_separator() {
        let md = "- gato: cat\n- perro: dog\n";
        let mut o = opts();
        o.mode = Mode::Separator;
        o.separator = "colon".to_string();
        o.include_headers = false;
        o.field_separator = FieldSep::Comma;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "gato,cat\nperro,dog");
    }

    #[test]
    fn table_mode_skips_header_and_reads_tag_column() {
        let md = "| Front | Back | Tags |\n| --- | --- | --- |\n| ser | to be | spanish verbs |\n";
        let mut o = opts();
        o.include_headers = true;
        let out = generate(md, &o).unwrap();
        assert_eq!(
            out,
            "#separator:Tab\n#html:true\n#notetype:Basic\n#columns:Front\tBack\tTags\n\
             #tags column:3\nser\tto be\tspanish verbs"
        );
    }

    #[test]
    fn cloze_mode_numbers_each_bold_span() {
        let md = "The **mitochondrion** is the **powerhouse** of the cell.\n";
        let mut o = opts();
        o.mode = Mode::Cloze;
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(
            out,
            "The {{c1::mitochondrion}} is the {{c2::powerhouse}} of the cell.\t"
        );
    }

    #[test]
    fn cloze_mode_upgrades_the_notetype() {
        let md = "Water boils at **100 °C**.\n";
        let mut o = opts();
        o.mode = Mode::Cloze;
        let out = generate(md, &o).unwrap();
        assert!(out.contains("#notetype:Cloze"), "{out}");
        assert!(out.contains("#columns:Text\tBack Extra"), "{out}");
    }

    #[test]
    fn cloze_mode_without_emphasis_errors() {
        let mut o = opts();
        o.mode = Mode::Cloze;
        let err = generate("plain notes with no emphasis", &o).unwrap_err();
        assert!(err.contains("cloze mode"), "{err}");
    }

    #[test]
    fn html_field_format_converts_inline_markdown() {
        let md = "## Term\nUse `cargo test` and see **the book** at [docs](https://example.com).\n";
        let mut o = opts();
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(
            out,
            "Term\tUse <code>cargo test</code> and see <b>the book</b> at <a href='https://example.com'>docs</a>."
        );
    }

    #[test]
    fn plain_field_format_strips_markup_and_quotes_multiline() {
        let md = "## Term\nUse `cargo test`\nand **read** more\n";
        let mut o = opts();
        o.include_headers = false;
        o.field_format = FieldFormat::Plain;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "Term\t\"Use cargo test\nand read more\"");
    }

    #[test]
    fn markdown_field_format_keeps_the_source() {
        let md = "## Term\n**bold** answer\n";
        let mut o = opts();
        o.include_headers = false;
        o.field_format = FieldFormat::Markdown;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "Term\t**bold** answer");
    }

    #[test]
    fn lists_and_code_blocks_become_html() {
        let md = "## Steps\n- one\n- two\n\n```\nlet x = 1 < 2;\n```\n";
        let mut o = opts();
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(
            out,
            "Steps\t<ul><li>one</li><li>two</li></ul><pre><code>let x = 1 &lt; 2;</code></pre>"
        );
    }

    #[test]
    fn deck_tags_and_notetype_headers_are_emitted() {
        let md = "Q: a\nA: b\n";
        let mut o = opts();
        o.deck = "Biology::Cells".to_string();
        o.tags = "exam, week1".to_string();
        o.notetype = "Basic (and reversed card)".to_string();
        let out = generate(md, &o).unwrap();
        assert!(out.contains("#deck:Biology::Cells"), "{out}");
        assert!(out.contains("#tags:exam week1"), "{out}");
        assert!(out.contains("#notetype:Basic (and reversed card)"), "{out}");
    }

    #[test]
    fn global_tags_fold_into_the_tag_column_without_headers() {
        let md = "Q: a\nA: b\n";
        let mut o = opts();
        o.tags = "exam".to_string();
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "a\tb\texam");
    }

    #[test]
    fn heading_tags_use_the_ancestor_path() {
        let md = "# Biology\n\n## Cell Parts\n\n### Nucleus\nHolds the DNA.\n";
        let mut o = opts();
        o.tags_from_headings = true;
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "Nucleus\tHolds the DNA.\tBiology::Cell_Parts");
    }

    #[test]
    fn heading_level_can_be_pinned() {
        let md = "# Chapter 1\nIntro text.\n\n## Term\nDefinition.\n";
        let mut o = opts();
        o.heading_level = 1;
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "Chapter 1\tIntro text.");
    }

    #[test]
    fn duplicate_questions_are_dropped_unless_disabled() {
        let md = "Q: a\nA: b\n\nQ: a\nA: c\n";
        let mut o = opts();
        o.include_headers = false;
        assert_eq!(generate(md, &o).unwrap(), "a\tb");
        o.dedupe = false;
        assert_eq!(generate(md, &o).unwrap(), "a\tb\na\tc");
    }

    #[test]
    fn csv_separator_quotes_fields_containing_commas() {
        let md = "Q: Paris, France?\nA: The capital\n";
        let mut o = opts();
        o.field_separator = FieldSep::Comma;
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "\"Paris, France?\",The capital");
    }

    #[test]
    fn preview_and_json_outputs_render() {
        let md = "Q: a\nA: b\n";
        let mut o = opts();
        o.output = OutputKind::Preview;
        assert_eq!(
            generate(md, &o).unwrap(),
            "1 card · mode: qa · note type: Basic\n\n1. Q: a\n   A: b"
        );
        o.output = OutputKind::Json;
        let json = generate(md, &o).unwrap();
        assert!(json.contains("\"count\": 1"), "{json}");
        assert!(json.contains("\"front\": \"a\""), "{json}");
    }

    #[test]
    fn too_many_cards_is_an_error() {
        let mut md = String::new();
        for i in 0..(MAX_CARDS + 1) {
            md.push_str(&format!("term{i} :: def{i}\n"));
        }
        let err = generate(&md, &opts()).unwrap_err();
        assert!(err.contains("over the 5000-card limit"), "{err}");
    }

    #[test]
    fn oversized_input_is_an_error() {
        let md = "a :: b\n".repeat(MAX_INPUT_CHARS / 6);
        let err = generate(&md, &opts()).unwrap_err();
        assert!(err.contains("too long"), "{err}");
    }

    #[test]
    fn vocab_list_under_a_title_prefers_the_separator() {
        let md = "# Spanish week 1\n\ngato :: cat\nperro :: dog\nlibro :: book\n";
        let mut o = opts();
        o.include_headers = false;
        let out = generate(md, &o).unwrap();
        assert_eq!(out, "gato\tcat\nperro\tdog\nlibro\tbook");
    }

    #[test]
    fn undetectable_notes_error_with_guidance() {
        let err = generate("just a sentence with no structure", &opts()).unwrap_err();
        assert!(err.contains("could not detect"), "{err}");
    }
}
