//! rtf-to-markdown core — convert an RTF document into Markdown, keeping the
//! formatting Markdown can express. Pure compute, no deps; shared by the chat
//! skill block and the web page.
//!
//! RTF (Rich Text Format) is plain-ASCII markup: text interleaved with
//! **control words** (`\word`, optional signed integer, optional trailing
//! space), **control symbols** (`\` + one non-letter, e.g. `\\`, `\{`, `\~`),
//! and **groups** delimited by `{` … `}`. Character formatting is
//! group-scoped; paragraph formatting is reset by `\pard`.
//!
//! Unlike a plain-text extractor, this walks the document as *blocks* and
//! *styled runs* so structure survives the trip:
//!
//! - **Emphasis** — `\b` → `**bold**`, `\i` → `*italic*`, `\strike` → `~~…~~`,
//!   `\ul` → `<u>…</u>` (Markdown has no underline; the `underline` option can
//!   drop it instead), `\super`/`\sub` → `<sup>`/`<sub>`.
//! - **Headings** — from `\outlinelevelN`, or from the paragraph's `\sN` style
//!   when the document's `\stylesheet` names that style `heading N`.
//! - **Lists** — a `{\listtext …}`/`{\pntext …}` marker makes the paragraph a
//!   list item; the marker text decides bullet vs. ordered and `\ilvlN` (or the
//!   `\li` indent) decides nesting depth.
//! - **Links** — `{\field{\*\fldinst HYPERLINK "…"}{\fldrslt …}}` →
//!   `[text](url)`.
//! - **Tables** — `\intbl` paragraphs with `\cell`/`\row` become GitHub pipe
//!   tables (or tab-separated plain text).
//! - **Escapes** — `\'hh` decodes as Windows-1252 (the `\ansi` default,
//!   including 0x80–0x9F), `\uN` decodes the (possibly negative) code point and
//!   skips the following `\ucN` ANSI-fallback characters.
//! - Non-text destinations — font/color tables, stylesheet, `\info`, `\pict`,
//!   `\*` ignorable groups — are skipped whole.

use std::collections::HashMap;

/// How `\intbl` tables are rendered.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tables {
    /// GitHub pipe tables (`| a | b |`).
    Markdown,
    /// Tab-separated rows — a lossless fallback for merged/nested cells.
    Text,
}

/// What to do with underlined runs (Markdown has no underline syntax).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Underline {
    /// Wrap in `<u>` … `</u>` (valid inline HTML in Markdown).
    Html,
    /// Drop the underline, keep the text.
    Ignore,
}

#[derive(Clone, Copy)]
struct Opts {
    headings: bool,
    tables: Tables,
    underline: Underline,
    links: bool,
    escape: bool,
}

/// Convert an RTF document to Markdown.
///
/// - `headings`: `""`/`"auto"` (default) detects headings from `\outlinelevel`
///   and `heading N` stylesheet names; `"off"` renders every paragraph as body
///   text.
/// - `tables`: `""`/`"markdown"` (default) emits GitHub pipe tables; `"text"`
///   emits tab-separated rows (keeps merged/nested cells readable).
/// - `underline`: `""`/`"html"` (default) keeps underline as `<u>`; `"ignore"`
///   drops it.
/// - `links`: `true` (default) turns HYPERLINK fields into `[text](url)`;
///   `false` keeps only the visible text.
/// - `escape`: `true` (default) backslash-escapes Markdown punctuation in
///   literal text so `*`, `_`, `[` … render as themselves.
///
/// Returns `Err` if the input does not begin with `{\rtf`, or on an unknown
/// option value. The output is always valid UTF-8.
pub fn rtf_to_markdown(
    rtf: &str,
    headings: &str,
    tables: &str,
    underline: &str,
    links: bool,
    escape: bool,
) -> Result<String, String> {
    let headings = match headings.trim() {
        "" | "auto" => true,
        "off" => false,
        other => {
            return Err(format!(
                "invalid headings {other:?}: expected \"auto\" or \"off\""
            ))
        }
    };
    let tables = match tables.trim() {
        "" | "markdown" => Tables::Markdown,
        "text" => Tables::Text,
        other => {
            return Err(format!(
                "invalid tables {other:?}: expected \"markdown\" or \"text\""
            ))
        }
    };
    let underline = match underline.trim() {
        "" | "html" => Underline::Html,
        "ignore" => Underline::Ignore,
        other => {
            return Err(format!(
                "invalid underline {other:?}: expected \"html\" or \"ignore\""
            ))
        }
    };

    if !rtf.trim_start().starts_with("{\\rtf") {
        return Err(
            "not an RTF document: expected the source to begin with \"{\\rtf\"".to_string(),
        );
    }

    let opts = Opts {
        headings,
        tables,
        underline,
        links,
        escape,
    };
    let styles = parse_stylesheet(rtf);
    let blocks = Parser::new(rtf, opts, styles).parse();
    Ok(render(&blocks, opts))
}

/// Convenience wrapper used by the scaffolded page/CLI defaults.
pub fn run(input: &str) -> Result<String, String> {
    rtf_to_markdown(input, "auto", "markdown", "html", true, true)
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// Character formatting of one run of text.
#[derive(Clone, Default, PartialEq, Eq)]
struct Fmt {
    bold: bool,
    italic: bool,
    strike: bool,
    underline: bool,
    sup: bool,
    sub: bool,
    link: Option<String>,
}

#[derive(Clone)]
struct Span {
    text: String,
    fmt: Fmt,
}

/// Where the text inside the current group goes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Dest {
    /// Visible document text.
    Normal,
    /// A non-text destination (font table, `\*` group, picture, …).
    Skip,
    /// `\fldinst` — captured so `HYPERLINK "…"` can be read out.
    FldInst,
    /// `\listtext`/`\pntext` — captured to classify the list item.
    ListText,
}

#[derive(Clone)]
struct GroupState {
    fmt: Fmt,
    dest: Dest,
    ucskip: i32,
}

/// Paragraph-scoped properties — reset by `\pard` and after every `\par`.
#[derive(Clone, Default)]
struct ParaState {
    style: Option<i64>,
    outline: Option<i64>,
    ilvl: Option<i64>,
    li: i64,
    intbl: bool,
    marker: Option<String>,
}

enum Block {
    Para(Vec<Span>),
    Heading(u8, Vec<Span>),
    Item {
        level: usize,
        ordered: bool,
        number: Option<u64>,
        spans: Vec<Span>,
    },
    /// Rows → cells → styled runs.
    Table(Vec<Vec<Vec<Span>>>),
}

// ---------------------------------------------------------------------------
// Stylesheet pre-pass
// ---------------------------------------------------------------------------

/// Map `\sN` style numbers to their lower-cased stylesheet names, so a
/// paragraph carrying `\s1` can be recognised as `heading 1`.
///
/// The stylesheet is a group of sub-groups, each ending with the style name and
/// a `;` — e.g. `{\stylesheet{\s1\sbasedon0 heading 1;}{\s2 heading 2;}}`.
fn parse_stylesheet(rtf: &str) -> HashMap<i64, String> {
    let mut map = HashMap::new();
    let chars: Vec<char> = rtf.chars().collect();
    let Some(start) = find_group(&chars, "\\stylesheet") else {
        return map;
    };
    // Walk the sub-groups of the stylesheet group.
    let mut i = start;
    let mut depth = 0usize;
    let mut sub_start = None;
    while i < chars.len() {
        match chars[i] {
            '{' => {
                depth += 1;
                if depth == 2 {
                    sub_start = Some(i + 1);
                }
            }
            '}' => {
                if depth == 2 {
                    if let Some(s) = sub_start.take() {
                        if let Some((num, name)) = parse_style_entry(&chars[s..i]) {
                            map.insert(num, name);
                        }
                    }
                }
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            _ => {}
        }
        i += 1;
    }
    map
}

/// Byte offset (in `chars`) of the `{` opening the group introduced by `word`.
fn find_group(chars: &[char], word: &str) -> Option<usize> {
    let w: Vec<char> = word.chars().collect();
    for i in 0..chars.len() {
        if chars[i] == '{' && chars[i + 1..].starts_with(&w[..]) {
            return Some(i);
        }
    }
    None
}

/// Pull `(style number, lower-cased name)` out of one stylesheet sub-group.
fn parse_style_entry(body: &[char]) -> Option<(i64, String)> {
    let mut num: Option<i64> = None;
    let mut name = String::new();
    let mut i = 0;
    let mut depth = 0usize;
    while i < body.len() {
        match body[i] {
            '{' => {
                depth += 1;
                i += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            '\\' => {
                let (word, arg, next) = read_control(body, i);
                // `\s1` is the style's own number; `\sbasedon`/`\snext` are not.
                if word == "s" && num.is_none() {
                    num = arg;
                }
                i = next;
            }
            ';' => break,
            c => {
                if depth == 0 {
                    name.push(c);
                }
                i += 1;
            }
        }
    }
    let name = name.trim().to_ascii_lowercase();
    match (num, name.is_empty()) {
        (Some(n), false) => Some((n, name)),
        _ => None,
    }
}

/// Read the control word starting at `chars[i] == '\\'`; returns
/// `(word, numeric argument, index just past the word and its delimiter)`.
fn read_control(chars: &[char], i: usize) -> (String, Option<i64>, usize) {
    let n = chars.len();
    let mut j = i + 1;
    while j < n && chars[j].is_ascii_alphabetic() {
        j += 1;
    }
    let word: String = chars[i + 1..j].iter().collect();
    let mut arg = None;
    let sign_start = j;
    let mut k = j;
    if k < n && chars[k] == '-' {
        k += 1;
    }
    let digit_start = k;
    while k < n && chars[k].is_ascii_digit() {
        k += 1;
    }
    if k > digit_start {
        let s: String = chars[sign_start..k].iter().collect();
        arg = s.parse::<i64>().ok();
        j = k;
    }
    if j < n && chars[j] == ' ' {
        j += 1; // the single delimiter space
    }
    if word.is_empty() {
        // A control symbol (`\*`, `\\`, …) — consume the backslash + one char.
        return (String::new(), None, (i + 2).min(n));
    }
    (word, arg, j)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    chars: Vec<char>,
    opts: Opts,
    styles: HashMap<i64, String>,
    blocks: Vec<Block>,
    stack: Vec<GroupState>,
    cur: GroupState,
    para: ParaState,
    spans: Vec<Span>,
    /// Remaining `\uN` ANSI-fallback characters to swallow.
    curskip: i32,
    /// Buffer for the active `\fldinst` / `\listtext` capture.
    capture: String,
    /// URL parsed from the last `\fldinst`, waiting for its `\fldrslt`.
    pending_link: Option<String>,
    /// Table rows collected so far, and the row being built.
    table: Vec<Vec<Vec<Span>>>,
    row: Vec<Vec<Span>>,
}

impl Parser {
    fn new(rtf: &str, opts: Opts, styles: HashMap<i64, String>) -> Self {
        Parser {
            chars: rtf.chars().collect(),
            opts,
            styles,
            blocks: Vec::new(),
            stack: Vec::new(),
            cur: GroupState {
                fmt: Fmt::default(),
                dest: Dest::Normal,
                ucskip: 1,
            },
            para: ParaState::default(),
            spans: Vec::new(),
            curskip: 0,
            capture: String::new(),
            pending_link: None,
            table: Vec::new(),
            row: Vec::new(),
        }
    }

    fn parse(mut self) -> Vec<Block> {
        let n = self.chars.len();
        let mut i = self
            .chars
            .iter()
            .position(|c| !c.is_whitespace())
            .unwrap_or(0);
        while i < n {
            let ch = self.chars[i];
            match ch {
                '{' => {
                    self.stack.push(self.cur.clone());
                    i += 1;
                }
                '}' => {
                    self.close_group();
                    i += 1;
                }
                '\\' => {
                    let next = match self.chars.get(i + 1) {
                        Some(&c) => c,
                        None => break,
                    };
                    if next.is_ascii_alphabetic() {
                        let (word, arg, j) = read_control(&self.chars, i);
                        i = j;
                        self.control_word(&word, arg);
                    } else if next == '\'' {
                        let h1 = self.chars.get(i + 2).copied().and_then(hex_val);
                        let h2 = self.chars.get(i + 3).copied().and_then(hex_val);
                        i += 4.min(n - i);
                        if self.curskip > 0 {
                            self.curskip -= 1;
                        } else if let (Some(a), Some(b)) = (h1, h2) {
                            self.push_char(cp1252_decode((a * 16 + b) as u8));
                        }
                    } else {
                        i += 2;
                        self.control_symbol(next);
                    }
                }
                // Raw CR/LF in the source are RTF whitespace, not text.
                '\r' | '\n' => i += 1,
                c => {
                    if self.curskip > 0 {
                        self.curskip -= 1;
                    } else {
                        self.push_char(c);
                    }
                    i += 1;
                }
            }
        }
        self.flush_para();
        self.flush_table();
        self.blocks
    }

    /// Restore the enclosing group's state, first settling any capture that the
    /// closing group was collecting.
    fn close_group(&mut self) {
        match self.cur.dest {
            Dest::FldInst => {
                if let Some(url) = parse_hyperlink(&self.capture) {
                    self.pending_link = Some(url);
                }
                self.capture.clear();
            }
            Dest::ListText => {
                let marker = self.capture.trim().to_string();
                if !marker.is_empty() {
                    self.para.marker = Some(marker);
                }
                self.capture.clear();
            }
            _ => {}
        }
        if let Some(prev) = self.stack.pop() {
            self.cur = prev;
        }
    }

    fn push_char(&mut self, c: char) {
        match self.cur.dest {
            Dest::Skip => {}
            Dest::FldInst | Dest::ListText => self.capture.push(c),
            Dest::Normal => {
                if let Some(last) = self.spans.last_mut() {
                    if last.fmt == self.cur.fmt {
                        last.text.push(c);
                        return;
                    }
                }
                self.spans.push(Span {
                    text: c.to_string(),
                    fmt: self.cur.fmt.clone(),
                });
            }
        }
    }

    fn control_symbol(&mut self, sym: char) {
        // `\*` marks the group ignorable — even mid-skip, so `{\*\generator …}`
        // is dropped whole. A following `\fldinst`/`\listtext` re-claims it.
        if sym == '*' {
            self.cur.dest = Dest::Skip;
            return;
        }
        if self.curskip > 0 {
            self.curskip -= 1;
            return;
        }
        match sym {
            '\\' | '{' | '}' => self.push_char(sym),
            '~' => self.push_char('\u{00A0}'),
            '_' => self.push_char('-'),
            '-' => {}
            '\n' | '\r' => self.end_para(),
            other => self.push_char(other),
        }
    }

    fn control_word(&mut self, word: &str, arg: Option<i64>) {
        // Unicode bookkeeping runs even inside skipped destinations.
        match word {
            "uc" => {
                self.cur.ucskip = arg.unwrap_or(1).max(0) as i32;
                return;
            }
            "u" => {
                if let Some(a) = arg {
                    let cp = if a < 0 { a + 0x10000 } else { a };
                    if let Some(ch) = u32::try_from(cp).ok().and_then(char::from_u32) {
                        self.push_char(ch);
                    }
                    self.curskip = self.cur.ucskip;
                }
                return;
            }
            // Destinations we read instead of skip — these override a preceding `\*`.
            "fldinst" => {
                self.cur.dest = Dest::FldInst;
                self.capture.clear();
                return;
            }
            "listtext" | "pntext" => {
                self.cur.dest = Dest::ListText;
                self.capture.clear();
                return;
            }
            "fldrslt" => {
                self.cur.dest = Dest::Normal;
                if self.opts.links {
                    self.cur.fmt.link = self.pending_link.take();
                } else {
                    self.pending_link = None;
                }
                return;
            }
            _ => {}
        }
        // While skipping a \uN fallback, a control word counts as one unit.
        if self.curskip > 0 {
            self.curskip -= 1;
            return;
        }
        if self.cur.dest == Dest::Skip {
            return;
        }

        let on = arg != Some(0);
        match word {
            // -- character formatting --------------------------------------
            "b" => self.cur.fmt.bold = on,
            "i" => self.cur.fmt.italic = on,
            "strike" | "striked" => self.cur.fmt.strike = on,
            "ul" | "uld" | "uldb" | "ulw" | "ulth" | "ulwave" | "uldash" => {
                self.cur.fmt.underline = on
            }
            "ulnone" => self.cur.fmt.underline = false,
            "super" => {
                self.cur.fmt.sup = on;
                if on {
                    self.cur.fmt.sub = false;
                }
            }
            "sub" => {
                self.cur.fmt.sub = on;
                if on {
                    self.cur.fmt.sup = false;
                }
            }
            "nosupersub" => {
                self.cur.fmt.sup = false;
                self.cur.fmt.sub = false;
            }
            "plain" => {
                let link = self.cur.fmt.link.clone();
                self.cur.fmt = Fmt {
                    link,
                    ..Fmt::default()
                };
            }

            // -- paragraph properties --------------------------------------
            "pard" => {
                let intbl = self.para.intbl && !self.row.is_empty();
                self.para = ParaState {
                    intbl,
                    ..ParaState::default()
                };
            }
            "s" => self.para.style = arg,
            "outlinelevel" => self.para.outline = arg,
            "ilvl" => self.para.ilvl = arg,
            "li" | "lin" => self.para.li = arg.unwrap_or(0),
            "intbl" => self.para.intbl = true,
            "pnlvlblt" => {
                self.para
                    .marker
                    .get_or_insert_with(|| "\u{2022}".to_string());
            }

            // -- structure --------------------------------------------------
            "par" | "sect" | "page" => self.end_para(),
            "line" | "softline" => {
                if self.para.intbl {
                    self.push_char(' ');
                } else {
                    // A hard line break inside a paragraph: Markdown's two-space
                    // continuation, applied when the block is rendered.
                    self.push_char('\n');
                }
            }
            "cell" | "nestcell" => self.end_cell(),
            "row" | "nestrow" => self.end_row(),
            "trowd" => self.para.intbl = true,
            "tab" => self.push_char('\t'),

            // -- typographic characters -------------------------------------
            "emdash" => self.push_char('\u{2014}'),
            "endash" => self.push_char('\u{2013}'),
            "bullet" => self.push_char('\u{2022}'),
            "lquote" => self.push_char('\u{2018}'),
            "rquote" => self.push_char('\u{2019}'),
            "ldblquote" => self.push_char('\u{201C}'),
            "rdblquote" => self.push_char('\u{201D}'),
            "emspace" => self.push_char('\u{2003}'),
            "enspace" => self.push_char('\u{2002}'),
            "qmspace" => self.push_char('\u{2005}'),
            "chdate" | "chtime" | "chpgn" | "chftn" => {}

            _ => {
                if is_destination(word) {
                    self.cur.dest = Dest::Skip;
                }
            }
        }
    }

    fn end_cell(&mut self) {
        let spans = std::mem::take(&mut self.spans);
        self.row.push(spans);
        self.para.marker = None;
    }

    fn end_row(&mut self) {
        if !self.spans.is_empty() {
            self.end_cell();
        }
        let row = std::mem::take(&mut self.row);
        if !row.is_empty() {
            self.table.push(row);
        }
        self.para.marker = None;
    }

    /// End of a paragraph (`\par`): inside a table it is a soft break, outside
    /// it flushes the accumulated spans as a block.
    fn end_para(&mut self) {
        if self.para.intbl {
            self.push_char(' ');
            return;
        }
        self.flush_para();
    }

    fn flush_para(&mut self) {
        let spans = std::mem::take(&mut self.spans);
        let para = std::mem::take(&mut self.para);
        if spans.iter().all(|s| s.text.trim().is_empty()) {
            return;
        }
        // Any table in progress ends where normal text resumes.
        self.flush_table();

        if let Some(marker) = &para.marker {
            let level = para
                .ilvl
                .map(|l| l.clamp(0, 8) as usize)
                .unwrap_or_else(|| ((para.li / 720).clamp(0, 8)) as usize);
            let (ordered, number) = classify_marker(marker);
            self.blocks.push(Block::Item {
                level,
                ordered,
                number,
                spans,
            });
            return;
        }
        if let Some(level) = self.heading_level(&para) {
            self.blocks.push(Block::Heading(level, spans));
            return;
        }
        self.blocks.push(Block::Para(spans));
    }

    fn flush_table(&mut self) {
        if !self.row.is_empty() {
            self.end_row();
        }
        let table = std::mem::take(&mut self.table);
        if !table.is_empty() {
            self.blocks.push(Block::Table(table));
        }
    }

    /// Heading level for a paragraph: `\outlinelevelN` wins, then a `heading N`
    /// stylesheet name for the paragraph's `\sN`.
    fn heading_level(&self, para: &ParaState) -> Option<u8> {
        if !self.opts.headings {
            return None;
        }
        if let Some(o) = para.outline {
            // `\outlinelevel9` is "body text", not a heading.
            if (0..=5).contains(&o) {
                return Some((o + 1) as u8);
            }
        }
        let name = self.styles.get(&para.style?)?;
        heading_level_from_name(name)
    }
}

/// `heading 2` / `heading2` / `h2` → level 2. Anything else → not a heading.
fn heading_level_from_name(name: &str) -> Option<u8> {
    let rest = name
        .strip_prefix("heading")
        .or_else(|| name.strip_prefix("h"))?
        .trim();
    let level: u8 = rest.parse().ok()?;
    (1..=6).contains(&level).then_some(level)
}

/// Classify a list marker: `1.`, `a)`, `iv.` → ordered (with its number when
/// numeric); `•`, `o`, `-`, `§` → a bullet.
fn classify_marker(marker: &str) -> (bool, Option<u64>) {
    let core = marker.trim().trim_end_matches(['.', ')', ']', ':']);
    if core.is_empty() {
        return (false, None);
    }
    if let Ok(n) = core.parse::<u64>() {
        return (true, Some(n));
    }
    // A single letter or a roman numeral is an ordered marker too, but its
    // position is what matters, so let the renderer number it.
    let is_alpha_marker = core.len() <= 2 && core.chars().all(|c| c.is_ascii_alphabetic());
    let ordered = is_alpha_marker && marker.trim().len() > core.len() && core != "o";
    (ordered, None)
}

/// Pull the target out of a field instruction like `HYPERLINK "https://x"`.
fn parse_hyperlink(inst: &str) -> Option<String> {
    let upper = inst.to_ascii_uppercase();
    let at = upper.find("HYPERLINK")?;
    let mut rest = inst[at + "HYPERLINK".len()..].trim_start();
    // Drop leading switches (`\l "anchor"`, `\o "tooltip"`) to reach the target.
    while let Some(after_slash) = rest.strip_prefix('\\') {
        let r = after_slash
            .trim_start_matches(|c: char| c.is_ascii_alphabetic())
            .trim_start();
        rest = match r.strip_prefix('"') {
            Some(q) => q.split_once('"').map(|(_, tail)| tail).unwrap_or(""),
            None => r,
        }
        .trim_start();
    }
    let url = match rest.strip_prefix('"') {
        Some(q) => q.split('"').next().unwrap_or(""),
        None => rest.split_whitespace().next().unwrap_or(""),
    };
    let url = url.trim();
    (!url.is_empty()).then(|| url.to_string())
}

/// Control words introducing a non-text destination to drop wholesale.
fn is_destination(word: &str) -> bool {
    matches!(
        word,
        "fonttbl"
            | "colortbl"
            | "stylesheet"
            | "listtable"
            | "listoverridetable"
            | "revtbl"
            | "rsidtbl"
            | "info"
            | "author"
            | "operator"
            | "company"
            | "manager"
            | "title"
            | "subject"
            | "keywords"
            | "comment"
            | "doccomm"
            | "generator"
            | "creatim"
            | "revtim"
            | "printim"
            | "buptim"
            | "pict"
            | "shppict"
            | "nonshppict"
            | "object"
            | "objdata"
            | "themedata"
            | "colorschememapping"
            | "datastore"
            | "latentstyles"
            | "filetbl"
            | "bkmkstart"
            | "bkmkend"
            | "header"
            | "headerl"
            | "headerr"
            | "headerf"
            | "footer"
            | "footerl"
            | "footerr"
            | "footerf"
            | "footnote"
            | "annotation"
            | "atnid"
            | "atnauthor"
            | "xmlnstbl"
            | "panose"
            | "falt"
            | "pgptbl"
            | "password"
            | "passwordhash"
    )
}

/// Decode a Windows-1252 byte. 0x00–0x7F and 0xA0–0xFF map straight through;
/// 0x80–0x9F use the CP1252 table (€, smart quotes, en/em dash, …).
fn cp1252_decode(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}',
        '\u{017D}', '\u{008F}', '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
    ];
    if (0x80..=0x9F).contains(&b) {
        HIGH[(b - 0x80) as usize]
    } else {
        b as char
    }
}

fn hex_val(c: char) -> Option<u16> {
    c.to_digit(16).map(|d| d as u16)
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render(blocks: &[Block], opts: Opts) -> String {
    let mut out = String::new();
    let mut counters: Vec<u64> = Vec::new();
    let mut prev_item = false;
    for block in blocks {
        let is_item = matches!(block, Block::Item { .. });
        if !out.is_empty() {
            out.push('\n');
            if !(is_item && prev_item) {
                out.push('\n');
            }
        }
        if !is_item {
            counters.clear();
        }
        match block {
            Block::Para(spans) => {
                let text = render_inline(spans, opts);
                out.push_str(&escape_block_start(&text, opts));
            }
            Block::Heading(level, spans) => {
                out.push_str(&"#".repeat(*level as usize));
                out.push(' ');
                out.push_str(&render_inline(spans, opts).replace('\n', " "));
            }
            Block::Item {
                level,
                ordered,
                number,
                spans,
            } => {
                counters.truncate(level + 1);
                while counters.len() <= *level {
                    counters.push(0);
                }
                counters[*level] += 1;
                let text = render_inline(spans, opts);
                // Continuation lines line up under the marker's text.
                let indent = "  ".repeat(*level);
                let marker = if *ordered {
                    format!("{}. ", number.unwrap_or(counters[*level]))
                } else {
                    "- ".to_string()
                };
                out.push_str(&indent);
                out.push_str(&marker);
                let pad = format!("{}{}", indent, " ".repeat(marker.len()));
                out.push_str(&text.replace('\n', &format!("\n{pad}")));
            }
            Block::Table(rows) => out.push_str(&render_table(rows, opts)),
        }
        prev_item = is_item;
    }
    finalize(&out)
}

fn render_table(rows: &[Vec<Vec<Span>>], opts: Opts) -> String {
    let cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    if cols == 0 {
        return String::new();
    }
    let cells: Vec<Vec<String>> = rows
        .iter()
        .map(|r| {
            let mut row: Vec<String> = r
                .iter()
                .map(|c| {
                    let t = render_inline(c, opts);
                    let t = t.replace(['\n', '\t'], " ");
                    t.split_whitespace().collect::<Vec<_>>().join(" ")
                })
                .collect();
            row.resize(cols, String::new());
            row
        })
        .collect();

    if opts.tables == Tables::Text {
        return cells
            .iter()
            .map(|r| r.join("\t"))
            .collect::<Vec<_>>()
            .join("\n");
    }

    let mut out = String::new();
    for (idx, row) in cells.iter().enumerate() {
        let escaped: Vec<String> = row.iter().map(|c| c.replace('|', "\\|")).collect();
        out.push_str(&format!("| {} |\n", escaped.join(" | ")));
        if idx == 0 {
            out.push_str(&format!("| {} |\n", vec!["---"; cols].join(" | ")));
        }
    }
    out.pop();
    out
}

/// Merge adjacent runs that share formatting, then wrap each in its markers,
/// keeping surrounding whitespace OUTSIDE the markers (`**bold** x`, never
/// `**bold ** x`, which Markdown would not emphasise).
fn render_inline(spans: &[Span], opts: Opts) -> String {
    let mut out = String::new();
    let mut merged: Vec<Span> = Vec::new();
    for s in spans {
        match merged.last_mut() {
            Some(last) if last.fmt == s.fmt => last.text.push_str(&s.text),
            _ => merged.push(s.clone()),
        }
    }
    for span in &merged {
        let text = if opts.escape {
            escape_md(&span.text)
        } else {
            span.text.clone()
        };
        let core = text.trim_matches(|c: char| c == ' ' || c == '\t');
        if core.is_empty() {
            out.push_str(&text);
            continue;
        }
        let lead = &text[..text.len() - text.trim_start_matches([' ', '\t']).len()];
        let trail = &text[text.trim_end_matches([' ', '\t']).len()..];
        out.push_str(lead);
        out.push_str(&wrap(core, &span.fmt, opts));
        out.push_str(trail);
    }
    // A hard line break renders as Markdown's two-space continuation.
    out.trim()
        .split('\n')
        .map(|l| l.trim_end())
        .collect::<Vec<_>>()
        .join("  \n")
}

fn wrap(core: &str, fmt: &Fmt, opts: Opts) -> String {
    let mut s = core.to_string();
    if fmt.sup {
        s = format!("<sup>{s}</sup>");
    }
    if fmt.sub {
        s = format!("<sub>{s}</sub>");
    }
    // Word underlines every hyperlink; `[<u>text</u>](url)` is noise, so the
    // link's own underline is implied rather than marked up.
    if fmt.underline && opts.underline == Underline::Html && fmt.link.is_none() {
        s = format!("<u>{s}</u>");
    }
    if fmt.italic {
        s = format!("*{s}*");
    }
    if fmt.bold {
        s = format!("**{s}**");
    }
    if fmt.strike {
        s = format!("~~{s}~~");
    }
    if let Some(url) = &fmt.link {
        s = format!("[{s}]({})", url.replace(' ', "%20"));
    }
    s
}

/// Backslash-escape the Markdown punctuation that would otherwise be read as
/// markup inside a run of literal text.
fn escape_md(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '`' | '*' | '_' | '[' | ']' | '<' | '>' | '|') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// A body paragraph whose first line would otherwise start a heading, list, or
/// quote gets its leading marker escaped.
fn escape_block_start(text: &str, opts: Opts) -> String {
    if !opts.escape {
        return text.to_string();
    }
    let trimmed = text.trim_start();
    let lead = &text[..text.len() - trimmed.len()];
    let digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if digits > 0 && trimmed[digits..].starts_with(". ") {
        // `1. text` would start an ordered list — escape the dot, keep the number.
        let (num, rest) = trimmed.split_at(digits);
        return format!("{lead}{num}\\{rest}");
    }
    if trimmed.starts_with(['#', '+', '=']) || trimmed.starts_with("- ") {
        return format!("{lead}\\{trimmed}");
    }
    text.to_string()
}

/// Collapse runs of blank lines and trim the document edges. Two trailing
/// spaces are Markdown's hard line break, so they survive the right-trim.
fn finalize(s: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut blank_run = 0usize;
    for line in s.split('\n') {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                lines.push(String::new());
            }
        } else {
            blank_run = 0;
            if line.len() >= trimmed.len() + 2 {
                lines.push(format!("{trimmed}  "));
            } else {
                lines.push(trimmed.to_string());
            }
        }
    }
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(rtf: &str) -> String {
        rtf_to_markdown(rtf, "auto", "markdown", "html", true, true).unwrap()
    }

    #[test]
    fn converts_emphasis_runs() {
        let rtf = r"{\rtf1\ansi The quick \b brown\b0  \i fox\i0  jumps.\par}";
        assert_eq!(conv(rtf), "The quick **brown** *fox* jumps.");
    }

    #[test]
    fn strike_underline_and_scripts() {
        let rtf = r"{\rtf1\ansi \strike gone\strike0  \ul under\ulnone  x\super 2\nosupersub \par}";
        assert_eq!(conv(rtf), "~~gone~~ <u>under</u> x<sup>2</sup>");
    }

    #[test]
    fn underline_can_be_dropped() {
        let rtf = r"{\rtf1\ansi \ul under\ulnone  plain\par}";
        let md = rtf_to_markdown(rtf, "auto", "markdown", "ignore", true, true).unwrap();
        assert_eq!(md, "under plain");
    }

    #[test]
    fn detects_headings_from_outline_level() {
        let rtf = r"{\rtf1\ansi{\pard\outlinelevel0 Title\par}{\pard\outlinelevel1 Section\par}{\pard Body text.\par}}";
        assert_eq!(conv(rtf), "# Title\n\n## Section\n\nBody text.");
    }

    #[test]
    fn detects_headings_from_stylesheet_names() {
        let rtf = r"{\rtf1\ansi{\stylesheet{\s1\sbasedon0 heading 1;}{\s2 heading 2;}{\s15 Normal;}}\pard\s1 Chapter\par\pard\s2 Part\par\pard\s15 Body\par}";
        assert_eq!(conv(rtf), "# Chapter\n\n## Part\n\nBody");
    }

    #[test]
    fn headings_off_keeps_body_text() {
        let rtf = r"{\rtf1\ansi\pard\outlinelevel0 Title\par}";
        let md = rtf_to_markdown(rtf, "off", "markdown", "html", true, true).unwrap();
        assert_eq!(md, "Title");
    }

    #[test]
    fn converts_bulleted_and_nested_lists() {
        let rtf = r"{\rtf1\ansi{\pard\ilvl0{\listtext\f3 \'b7\tab}First\par}{\pard\ilvl1{\listtext\f3 o\tab}Nested\par}{\pard\ilvl0{\listtext\f3 \'b7\tab}Second\par}}";
        assert_eq!(conv(rtf), "- First\n  - Nested\n- Second");
    }

    #[test]
    fn converts_numbered_lists() {
        let rtf = r"{\rtf1\ansi{\pard\ilvl0{\listtext 1.\tab}Alpha\par}{\pard\ilvl0{\listtext 2.\tab}Beta\par}}";
        assert_eq!(conv(rtf), "1. Alpha\n2. Beta");
    }

    #[test]
    fn converts_hyperlink_fields() {
        let rtf = r#"{\rtf1\ansi See {\field{\*\fldinst{HYPERLINK "https://example.com/a b"}}{\fldrslt{\ul\cf1 the docs}}} now.\par}"#;
        assert_eq!(conv(rtf), "See [the docs](https://example.com/a%20b) now.");
    }

    #[test]
    fn links_can_be_disabled() {
        let rtf = r#"{\rtf1\ansi {\field{\*\fldinst{HYPERLINK "https://example.com"}}{\fldrslt{docs}}}\par}"#;
        let md = rtf_to_markdown(rtf, "auto", "markdown", "html", false, true).unwrap();
        assert_eq!(md, "docs");
    }

    #[test]
    fn converts_tables_to_pipe_tables() {
        let rtf = r"{\rtf1\ansi\trowd\intbl Name\cell Qty\cell\row\trowd\intbl Bolt\cell 12\cell\row\pard After.\par}";
        assert_eq!(
            conv(rtf),
            "| Name | Qty |\n| --- | --- |\n| Bolt | 12 |\n\nAfter."
        );
    }

    #[test]
    fn tables_can_render_as_text() {
        let rtf = r"{\rtf1\ansi\trowd\intbl A\cell B\cell\row\trowd\intbl 1\cell 2\cell\row}";
        let md = rtf_to_markdown(rtf, "auto", "text", "html", true, true).unwrap();
        assert_eq!(md, "A\tB\n1\t2");
    }

    #[test]
    fn decodes_hex_and_unicode_escapes() {
        let rtf = r"{\rtf1\ansi\uc1 Caf\'e9 costs 5\'80. \u26085?\u26412?\par}";
        assert_eq!(conv(rtf), "Café costs 5€. 日本");
    }

    #[test]
    fn skips_non_text_destinations() {
        let rtf = r"{\rtf1\ansi\deff0{\fonttbl{\f0 Arial;}}{\colortbl;\red0\green0\blue0;}{\info{\author Someone}}{\*\generator Word;}Visible.\par}";
        assert_eq!(conv(rtf), "Visible.");
    }

    #[test]
    fn escapes_markdown_punctuation() {
        let rtf = r"{\rtf1\ansi a_b * c [link] 2*3\par}";
        assert_eq!(conv(rtf), r"a\_b \* c \[link\] 2\*3");
    }

    #[test]
    fn escaping_can_be_disabled() {
        let rtf = r"{\rtf1\ansi a_b * c\par}";
        let md = rtf_to_markdown(rtf, "auto", "markdown", "html", true, false).unwrap();
        assert_eq!(md, "a_b * c");
    }

    #[test]
    fn escapes_a_body_paragraph_that_looks_like_a_list() {
        let rtf = r"{\rtf1\ansi 1. not a list\par}";
        assert_eq!(conv(rtf), "1\\. not a list");
    }

    #[test]
    fn hard_line_breaks_become_markdown_continuations() {
        let rtf = r"{\rtf1\ansi One\line Two\par}";
        assert_eq!(conv(rtf), "One  \nTwo");
    }

    #[test]
    fn rejects_non_rtf_input() {
        let err =
            rtf_to_markdown("hello world", "auto", "markdown", "html", true, true).unwrap_err();
        assert!(err.contains("not an RTF document"), "{err}");
    }

    #[test]
    fn rejects_unknown_option_values() {
        for (h, t, u, needle) in [
            ("weird", "markdown", "html", "invalid headings"),
            ("auto", "grid", "html", "invalid tables"),
            ("auto", "markdown", "dotted", "invalid underline"),
        ] {
            let err = rtf_to_markdown(r"{\rtf1 x}", h, t, u, true, true).unwrap_err();
            assert!(err.contains(needle), "{err}");
        }
    }

    #[test]
    fn empty_body_is_empty_output() {
        assert_eq!(conv(r"{\rtf1\ansi\par}"), "");
    }

    #[test]
    fn blank_option_strings_fall_back_to_defaults() {
        let rtf = r"{\rtf1\ansi \b hi\b0 \par}";
        assert_eq!(
            rtf_to_markdown(rtf, "", "", "", true, true).unwrap(),
            "**hi**"
        );
    }
}
