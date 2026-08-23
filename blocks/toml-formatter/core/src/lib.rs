//! toml-formatter core — pure compute, shared by the chat/CLI skill block and
//! the web page.
//!
//! The document is parsed with `toml_edit`, which keeps comments and the exact
//! spelling of every scalar in its syntax tree, and then re-emitted from
//! scratch under the caller's [`Options`]. Emitting from scratch (rather than
//! patching the original text) is what makes the output canonical; reading from
//! `toml_edit` (rather than a value model) is what keeps comments and literals
//! like `0xFF` / `1_000_000` intact.

use toml_edit::{Array, DocumentMut, InlineTable, Item, Key, RawString, Table, Value};

/// Key ordering within each table / inline table.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortKeys {
    /// Keep the order the keys were written in.
    Preserve,
    Asc,
    Desc,
}

/// Whitespace around the `=` of a key/value entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Spacing {
    /// `key = value`
    Standard,
    /// `key=value`
    Compact,
}

/// How arrays are laid out.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ArrayStyle {
    /// One line while it fits in `column_width` and carries no comments.
    Auto,
    /// Always one element per line, with a trailing comma.
    Expand,
    /// Always one line (drops comments inside the array — they cannot survive).
    Collapse,
}

/// Formatting knobs. See each field for the default the surfaces use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Options {
    /// Spaces of indentation per table nesting level (0-8). 0 = flat, which is
    /// what Cargo.toml / pyproject.toml look like in the wild.
    pub indent: usize,
    pub sort_keys: SortKeys,
    pub spacing: Spacing,
    pub array_style: ArrayStyle,
    /// Line-width budget that decides when `ArrayStyle::Auto` expands (20-200).
    pub column_width: usize,
    /// Pad keys so the `=` of consecutive entries in a table line up.
    pub align_values: bool,
    /// Put exactly one blank line before every `[table]` header.
    pub blank_line_before_tables: bool,
    /// Keep comments. When false every `#` comment is dropped.
    pub keep_comments: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            indent: 0,
            sort_keys: SortKeys::Preserve,
            spacing: Spacing::Standard,
            array_style: ArrayStyle::Auto,
            column_width: 80,
            align_values: false,
            blank_line_before_tables: true,
            keep_comments: true,
        }
    }
}

impl Options {
    /// Clamp the numeric knobs into their advertised ranges.
    pub fn clamped(mut self) -> Self {
        self.indent = self.indent.min(8);
        self.column_width = self.column_width.clamp(20, 200);
        self
    }
}

impl SortKeys {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "preserve" => Ok(SortKeys::Preserve),
            "asc" => Ok(SortKeys::Asc),
            "desc" => Ok(SortKeys::Desc),
            other => Err(format!(
                "sort_keys must be preserve, asc or desc (got \"{other}\")"
            )),
        }
    }
}

impl Spacing {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "standard" => Ok(Spacing::Standard),
            "compact" => Ok(Spacing::Compact),
            other => Err(format!(
                "spacing must be standard or compact (got \"{other}\")"
            )),
        }
    }
}

impl ArrayStyle {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "auto" => Ok(ArrayStyle::Auto),
            "expand" => Ok(ArrayStyle::Expand),
            "collapse" => Ok(ArrayStyle::Collapse),
            other => Err(format!(
                "array_style must be auto, expand or collapse (got \"{other}\")"
            )),
        }
    }
}

/// Parse `input` as TOML and re-emit it under `opts`.
///
/// Invalid TOML is a hard error carrying the line and column of the first
/// problem — nothing is emitted for input that would not round-trip.
pub fn format(input: &str, opts: &Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("provide some TOML to format".into());
    }
    let opts = opts.clone().clamped();
    let doc: DocumentMut = input.parse().map_err(|e| describe_error(input, &e))?;

    let mut em = Emitter {
        o: &opts,
        out: String::new(),
        want_blank: false,
    };
    em.table_body(doc.as_table(), &[]);
    // Comments dangling after the last entry live on the document, not on any key.
    for p in scan_str(doc.trailing().as_str().unwrap_or("")) {
        match p {
            Pre::Blank => em.want_blank = !em.out.is_empty(),
            Pre::Comment(c) => {
                if opts.keep_comments {
                    em.line(&c);
                }
            }
        }
    }
    Ok(em.out)
}

/// Surface-friendly entry point used by the chat/CLI block and browser wrapper.
pub fn run(
    input: &str,
    indent: usize,
    sort_keys: &str,
    spacing: &str,
    array_style: &str,
    column_width: usize,
    align_values: bool,
    blank_line_before_tables: bool,
    keep_comments: bool,
) -> Result<String, String> {
    let opts = Options {
        indent,
        sort_keys: SortKeys::parse(sort_keys)?,
        spacing: Spacing::parse(spacing)?,
        array_style: ArrayStyle::parse(array_style)?,
        column_width,
        align_values,
        blank_line_before_tables,
        keep_comments,
    };
    format(input, &opts)
}

// ---------------------------------------------------------------- decor scan

/// One meaningful line of leading decor: a blank line or a whole-line comment.
enum Pre {
    Blank,
    Comment(String),
}

fn scan(raw: Option<&RawString>) -> Vec<Pre> {
    scan_str(raw.and_then(|r| r.as_str()).unwrap_or(""))
}

/// Split a decor prefix into blank-line / comment-line markers.
///
/// A prefix looks like `"\n\n# note\n    "`: the first segment closes the
/// PREVIOUS line, the last segment is the indentation of the item itself, and
/// everything between is real content. Both ends are therefore dropped.
fn scan_str(raw: &str) -> Vec<Pre> {
    let mut lines: Vec<&str> = raw.split('\n').collect();
    if lines.len() <= 1 {
        return Vec::new();
    }
    lines.pop();
    if lines.first().is_some_and(|l| l.trim().is_empty()) {
        lines.remove(0);
    }
    lines
        .iter()
        .filter_map(|l| {
            let t = l.trim();
            if t.is_empty() {
                Some(Pre::Blank)
            } else if t.starts_with('#') {
                Some(Pre::Comment(t.trim_end().to_string()))
            } else {
                None
            }
        })
        .collect()
}

/// The end-of-line comment in a decor suffix (`" # note"`), if any.
fn eol_comment(raw: Option<&RawString>) -> Option<String> {
    let s = raw?.as_str()?;
    let idx = s.find('#')?;
    if s[..idx].contains('\n') {
        return None;
    }
    Some(s[idx..].lines().next().unwrap_or("").trim_end().to_string())
}

fn has_comment(raw: Option<&RawString>) -> bool {
    raw.and_then(|r| r.as_str())
        .is_some_and(|s| s.contains('#'))
}

// ------------------------------------------------------------------- emitter

struct Emitter<'a> {
    o: &'a Options,
    out: String,
    /// A blank line is owed before the next emitted line. A bool (not a count)
    /// is what collapses any run of blank lines to at most one.
    want_blank: bool,
}

impl Emitter<'_> {
    fn line(&mut self, s: &str) {
        if self.want_blank {
            if !self.out.is_empty() {
                self.out.push('\n');
            }
            self.want_blank = false;
        }
        self.out.push_str(s);
        self.out.push('\n');
    }

    fn leading(&mut self, items: &[Pre], ind: &str, allow_blank: bool) {
        for p in items {
            match p {
                Pre::Blank => {
                    if allow_blank && !self.out.is_empty() {
                        self.want_blank = true;
                    }
                }
                Pre::Comment(c) => {
                    if self.o.keep_comments {
                        let line = format!("{ind}{c}");
                        self.line(&line);
                    }
                }
            }
        }
    }

    /// Emit a table's own entries, then its sub-tables. Splitting the two is
    /// what normalizes table ordering: `[a.b]` written before `[a]` comes back
    /// in tree order, which is semantics-preserving in TOML.
    fn table_body(&mut self, tbl: &Table, path: &[String]) {
        let ind = " ".repeat(self.o.indent * path.len());

        let mut vals: Vec<(&Key, &Value)> = Vec::new();
        let mut subs: Vec<(&Key, &Item)> = Vec::new();
        for (name, _) in tbl.iter() {
            let Some((key, item)) = tbl.get_key_value(name) else {
                continue;
            };
            match item {
                Item::Value(v) => vals.push((key, v)),
                Item::Table(_) | Item::ArrayOfTables(_) => subs.push((key, item)),
                Item::None => {}
            }
        }
        sort_pairs(&mut vals, self.o.sort_keys);
        sort_pairs(&mut subs, self.o.sort_keys);

        // Alignment pads every key in the block to the widest one. Compact
        // spacing has no room for padding, so the two are mutually exclusive.
        let width = if self.o.align_values && self.o.spacing == Spacing::Standard {
            vals.iter()
                .map(|(k, _)| norm_key(k.get()).chars().count())
                .max()
                .unwrap_or(0)
        } else {
            0
        };

        for (k, v) in &vals {
            let pre = scan(k.leaf_decor().prefix());
            self.leading(&pre, &ind, true);
            let key = norm_key(k.get());
            let pad = " ".repeat(width.saturating_sub(key.chars().count()));
            let (head, col) = match self.o.spacing {
                Spacing::Standard => (
                    format!("{key}{pad} = "),
                    ind.chars().count() + key.chars().count() + pad.len() + 3,
                ),
                Spacing::Compact => (
                    format!("{key}="),
                    ind.chars().count() + key.chars().count() + 1,
                ),
            };
            let rendered = render_value(v, &ind, col, self.o);
            let mut line = format!("{ind}{head}{rendered}");
            if self.o.keep_comments {
                if let Some(c) = eol_comment(v.decor().suffix()) {
                    line.push(' ');
                    line.push_str(&c);
                }
            }
            self.line(&line);
        }

        for (k, item) in subs {
            let mut p = path.to_vec();
            p.push(norm_key(k.get()));
            match item {
                Item::Table(t) => {
                    // An implicit table (`[a.b]` with no `[a]`) has no header of
                    // its own — unless it carries entries, which means it was
                    // written as a dotted key and still needs somewhere to live.
                    let has_values = t.iter().any(|(_, i)| i.is_value());
                    if !t.is_implicit() || has_values {
                        self.header(t, &p, false);
                    }
                    self.table_body(t, &p);
                }
                Item::ArrayOfTables(aot) => {
                    for t in aot.iter() {
                        self.header(t, &p, true);
                        self.table_body(t, &p);
                    }
                }
                _ => {}
            }
        }
    }

    fn header(&mut self, t: &Table, path: &[String], array_of_tables: bool) {
        let ind = " ".repeat(self.o.indent * path.len().saturating_sub(1));
        if self.o.blank_line_before_tables && !self.out.is_empty() {
            self.want_blank = true;
        }
        let pre = scan(t.decor().prefix());
        self.leading(&pre, &ind, self.o.blank_line_before_tables);
        let name = path.join(".");
        let mut line = if array_of_tables {
            format!("{ind}[[{name}]]")
        } else {
            format!("{ind}[{name}]")
        };
        if self.o.keep_comments {
            if let Some(c) = eol_comment(t.decor().suffix()) {
                line.push(' ');
                line.push_str(&c);
            }
        }
        self.line(&line);
    }
}

fn sort_pairs<T>(items: &mut [(&Key, T)], order: SortKeys) {
    match order {
        SortKeys::Preserve => {}
        // Stable, so equal keys (impossible in valid TOML) keep document order.
        SortKeys::Asc => items.sort_by(|a, b| a.0.get().cmp(b.0.get())),
        SortKeys::Desc => items.sort_by(|a, b| b.0.get().cmp(a.0.get())),
    }
}

// -------------------------------------------------------------- value render

fn render_value(v: &Value, ind: &str, col: usize, o: &Options) -> String {
    match v {
        Value::Array(a) => render_array(a, ind, col, o),
        Value::InlineTable(t) => render_inline_table(t, o),
        scalar => raw_scalar(scalar),
    }
}

/// A scalar exactly as the author spelled it — `0xFF` stays `0xFF`, `1_000_000`
/// keeps its underscores, `'C:\path'` stays a literal string.
fn raw_scalar(v: &Value) -> String {
    let mut c = v.clone();
    c.decor_mut().clear();
    c.to_string()
}

/// Render a value on a single line. Comments inside cannot survive this (a `#`
/// would swallow the rest of the line), which is why `collapse` is documented
/// as comment-dropping.
fn collapse_value(v: &Value, o: &Options) -> String {
    match v {
        Value::Array(a) => {
            let items: Vec<String> = a.iter().map(|x| collapse_value(x, o)).collect();
            if items.is_empty() {
                "[]".into()
            } else if o.spacing == Spacing::Compact {
                format!("[{}]", items.join(","))
            } else {
                format!("[{}]", items.join(", "))
            }
        }
        Value::InlineTable(t) => render_inline_table(t, o),
        scalar => raw_scalar(scalar),
    }
}

/// Inline tables are single-line by TOML spec, so only key order and spacing
/// are negotiable here.
fn render_inline_table(t: &InlineTable, o: &Options) -> String {
    let mut pairs: Vec<(&Key, &Value)> = Vec::new();
    for (name, _) in t.iter() {
        if let Some((k, item)) = t.get_key_value(name) {
            if let Some(v) = item.as_value() {
                pairs.push((k, v));
            }
        }
    }
    sort_pairs(&mut pairs, o.sort_keys);
    if pairs.is_empty() {
        return "{}".into();
    }
    let inner: Vec<String> = pairs
        .iter()
        .map(|(k, v)| {
            let key = norm_key(k.get());
            let val = collapse_value(v, o);
            if o.spacing == Spacing::Compact {
                format!("{key}={val}")
            } else {
                format!("{key} = {val}")
            }
        })
        .collect();
    if o.spacing == Spacing::Compact {
        format!("{{{}}}", inner.join(","))
    } else {
        format!("{{ {} }}", inner.join(", "))
    }
}

fn render_array(a: &Array, ind: &str, col: usize, o: &Options) -> String {
    if a.is_empty() {
        return "[]".into();
    }
    let collapsed = {
        let items: Vec<String> = a.iter().map(|x| collapse_value(x, o)).collect();
        if o.spacing == Spacing::Compact {
            format!("[{}]", items.join(","))
        } else {
            format!("[{}]", items.join(", "))
        }
    };
    let expand = match o.array_style {
        ArrayStyle::Collapse => false,
        ArrayStyle::Expand => true,
        ArrayStyle::Auto => {
            (o.keep_comments && array_has_comments(a))
                || col + collapsed.chars().count() > o.column_width
        }
    };
    if !expand {
        return collapsed;
    }

    // Even at indent 0 an expanded array needs some indentation to read as one.
    let inner_ind = format!("{ind}{}", " ".repeat(o.indent.max(2)));
    let mut s = String::from("[\n");
    for x in a.iter() {
        if o.keep_comments {
            for p in scan(x.decor().prefix()) {
                if let Pre::Comment(c) = p {
                    s.push_str(&inner_ind);
                    s.push_str(&c);
                    s.push('\n');
                }
            }
        }
        let rendered = render_value(x, &inner_ind, inner_ind.chars().count(), o);
        s.push_str(&inner_ind);
        s.push_str(&rendered);
        s.push(',');
        if o.keep_comments {
            if let Some(c) = eol_comment(x.decor().suffix()) {
                s.push(' ');
                s.push_str(&c);
            }
        }
        s.push('\n');
    }
    if o.keep_comments {
        for p in scan_str(a.trailing().as_str().unwrap_or("")) {
            if let Pre::Comment(c) = p {
                s.push_str(&inner_ind);
                s.push_str(&c);
                s.push('\n');
            }
        }
    }
    s.push_str(ind);
    s.push(']');
    s
}

fn array_has_comments(a: &Array) -> bool {
    if has_comment(Some(a.trailing())) {
        return true;
    }
    a.iter().any(|v| {
        has_comment(v.decor().prefix())
            || has_comment(v.decor().suffix())
            || matches!(v, Value::Array(inner) if array_has_comments(inner))
    })
}

// ---------------------------------------------------------------------- keys

/// Bare when the key legally can be, otherwise a basic-quoted string with
/// minimal escapes. Key *values* are never rewritten this way — only keys.
fn norm_key(k: &str) -> String {
    if !k.is_empty()
        && k.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return k.to_string();
    }
    let mut s = String::from("\"");
    for c in k.chars() {
        match c {
            '"' => s.push_str("\\\""),
            '\\' => s.push_str("\\\\"),
            '\n' => s.push_str("\\n"),
            '\r' => s.push_str("\\r"),
            '\t' => s.push_str("\\t"),
            '\u{8}' => s.push_str("\\b"),
            '\u{c}' => s.push_str("\\f"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                s.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => s.push(c),
        }
    }
    s.push('"');
    s
}

// --------------------------------------------------------------------- error

fn describe_error(input: &str, e: &toml_edit::TomlError) -> String {
    let msg = e.message().trim().replace('\n', " ");
    match e.span() {
        Some(span) => {
            let upto = input.get(..span.start).unwrap_or(input);
            let line = upto.matches('\n').count() + 1;
            let col = upto.rsplit('\n').next().map_or(0, |l| l.chars().count()) + 1;
            format!("invalid TOML at line {line}, column {col}: {msg}")
        }
        None => format!("invalid TOML: {msg}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(input: &str) -> String {
        format(input, &Options::default()).expect("valid TOML")
    }

    #[test]
    fn normalizes_spacing_and_ends_with_newline() {
        assert_eq!(fmt("a=1\nb   =  2"), "a = 1\nb = 2\n");
    }

    #[test]
    fn rejects_invalid_toml_with_line_and_column() {
        let err = format("a = 1\nb = = 2\n", &Options::default()).unwrap_err();
        assert!(err.starts_with("invalid TOML at line 2, column 5:"), "{err}");
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(
            format("   \n", &Options::default()).unwrap_err(),
            "provide some TOML to format"
        );
    }

    #[test]
    fn keeps_comments_by_default() {
        let out = fmt("# top\na = 1 # trailing\n# dangling\n");
        assert_eq!(out, "# top\na = 1 # trailing\n# dangling\n");
    }

    #[test]
    fn strips_comments_when_asked() {
        let o = Options {
            keep_comments: false,
            ..Default::default()
        };
        assert_eq!(
            format("# top\na = 1 # trailing\n", &o).unwrap(),
            "a = 1\n"
        );
    }

    #[test]
    fn preserves_scalar_literals_verbatim() {
        let out = fmt("a = 0xFF\nb = 1_000_000\nc = 'C:\\path'\nd = 1979-05-27T07:32:00Z\n");
        assert_eq!(
            out,
            "a = 0xFF\nb = 1_000_000\nc = 'C:\\path'\nd = 1979-05-27T07:32:00Z\n"
        );
    }

    #[test]
    fn blank_line_before_tables_by_default() {
        assert_eq!(fmt("a = 1\n[b]\nc = 2\n"), "a = 1\n\n[b]\nc = 2\n");
    }

    #[test]
    fn blank_line_before_tables_can_be_off() {
        let o = Options {
            blank_line_before_tables: false,
            ..Default::default()
        };
        assert_eq!(
            format("a = 1\n\n\n[b]\nc = 2\n", &o).unwrap(),
            "a = 1\n[b]\nc = 2\n"
        );
    }

    #[test]
    fn collapses_runs_of_blank_lines_to_one() {
        assert_eq!(fmt("a = 1\n\n\n\nb = 2\n"), "a = 1\n\nb = 2\n");
    }

    #[test]
    fn sorts_keys_ascending_and_descending() {
        let asc = Options {
            sort_keys: SortKeys::Asc,
            ..Default::default()
        };
        assert_eq!(
            format("b = 2\na = 1\nc = 3\n", &asc).unwrap(),
            "a = 1\nb = 2\nc = 3\n"
        );
        let desc = Options {
            sort_keys: SortKeys::Desc,
            ..Default::default()
        };
        assert_eq!(
            format("b = 2\na = 1\nc = 3\n", &desc).unwrap(),
            "c = 3\nb = 2\na = 1\n"
        );
    }

    #[test]
    fn sorting_also_orders_tables_and_inline_table_keys() {
        let o = Options {
            sort_keys: SortKeys::Asc,
            ..Default::default()
        };
        let out = format("[z]\nk = { b = 1, a = 2 }\n[y]\nn = 1\n", &o).unwrap();
        assert_eq!(out, "[y]\nn = 1\n\n[z]\nk = { a = 2, b = 1 }\n");
    }

    #[test]
    fn indents_nested_tables_when_asked() {
        let o = Options {
            indent: 2,
            ..Default::default()
        };
        let out = format("[a]\nx = 1\n[a.b]\ny = 2\n", &o).unwrap();
        assert_eq!(out, "[a]\n  x = 1\n\n  [a.b]\n    y = 2\n");
    }

    #[test]
    fn compact_spacing_drops_the_spaces_around_equals() {
        let o = Options {
            spacing: Spacing::Compact,
            ..Default::default()
        };
        assert_eq!(
            format("a = 1\nb = { c = 2 }\n", &o).unwrap(),
            "a=1\nb={c=2}\n"
        );
    }

    #[test]
    fn aligns_values_across_a_run_of_entries() {
        let o = Options {
            align_values: true,
            ..Default::default()
        };
        assert_eq!(
            format("name = \"x\"\nv = 1\n", &o).unwrap(),
            "name = \"x\"\nv    = 1\n"
        );
    }

    #[test]
    fn auto_expands_arrays_past_the_column_width() {
        let o = Options {
            column_width: 20,
            ..Default::default()
        };
        let out = format("deps = [\"alpha\", \"bravo\", \"charlie\"]\n", &o).unwrap();
        assert_eq!(
            out,
            "deps = [\n  \"alpha\",\n  \"bravo\",\n  \"charlie\",\n]\n"
        );
    }

    #[test]
    fn auto_keeps_short_arrays_on_one_line() {
        assert_eq!(fmt("a = [ 1,2 , 3 ]\n"), "a = [1, 2, 3]\n");
    }

    #[test]
    fn expand_and_collapse_are_forced_when_selected() {
        let expand = Options {
            array_style: ArrayStyle::Expand,
            ..Default::default()
        };
        assert_eq!(
            format("a = [1, 2]\n", &expand).unwrap(),
            "a = [\n  1,\n  2,\n]\n"
        );
        let collapse = Options {
            array_style: ArrayStyle::Collapse,
            ..Default::default()
        };
        assert_eq!(
            format("a = [\n  1,\n  2,\n]\n", &collapse).unwrap(),
            "a = [1, 2]\n"
        );
    }

    #[test]
    fn comments_inside_an_array_force_expansion_and_survive() {
        let out = fmt("a = [\n  # first\n  1,\n  2, # second\n]\n");
        assert_eq!(out, "a = [\n  # first\n  1,\n  2,\n  # second\n]\n");
    }

    #[test]
    fn collapse_drops_comments_inside_an_array() {
        let o = Options {
            array_style: ArrayStyle::Collapse,
            ..Default::default()
        };
        assert_eq!(
            format("a = [\n  # first\n  1,\n  2,\n]\n", &o).unwrap(),
            "a = [1, 2]\n"
        );
    }

    #[test]
    fn array_of_tables_round_trips() {
        let out = fmt("[[server]]\nip = \"10.0.0.1\"\n[[server]]\nip = \"10.0.0.2\"\n");
        assert_eq!(
            out,
            "[[server]]\nip = \"10.0.0.1\"\n\n[[server]]\nip = \"10.0.0.2\"\n"
        );
    }

    #[test]
    fn entries_come_before_sub_tables_so_table_order_is_normalized() {
        let out = fmt("[a.b]\ny = 2\n\n[a]\nx = 1\n");
        assert_eq!(out, "[a]\nx = 1\n\n[a.b]\ny = 2\n");
    }

    #[test]
    fn implicit_parent_tables_get_no_header() {
        assert_eq!(fmt("[a.b]\nx = 1\n"), "[a.b]\nx = 1\n");
    }

    #[test]
    fn dotted_keys_become_their_own_table() {
        assert_eq!(fmt("a.b = 1\n"), "[a]\nb = 1\n");
    }

    #[test]
    fn normalizes_key_quoting() {
        let out = fmt("\"plain\" = 1\n\"has space\" = 2\n'lit' = 3\n");
        assert_eq!(out, "plain = 1\n\"has space\" = 2\nlit = 3\n");
    }

    #[test]
    fn strips_trailing_whitespace_on_every_line() {
        assert_eq!(fmt("a = 1   \n[b]   \nc = 2\n"), "a = 1\n\n[b]\nc = 2\n");
    }

    #[test]
    fn options_are_clamped_into_range() {
        let o = Options {
            indent: 99,
            column_width: 5,
            ..Default::default()
        }
        .clamped();
        assert_eq!(o.indent, 8);
        assert_eq!(o.column_width, 20);
    }

    #[test]
    fn enum_parsers_accept_blank_as_the_default() {
        assert_eq!(SortKeys::parse("").unwrap(), SortKeys::Preserve);
        assert_eq!(Spacing::parse("compact").unwrap(), Spacing::Compact);
        assert_eq!(ArrayStyle::parse("expand").unwrap(), ArrayStyle::Expand);
        assert!(SortKeys::parse("sideways").is_err());
    }

    #[test]
    fn formatting_is_idempotent() {
        let src = "# cfg\n[package]\nname = \"demo\" # the name\nkeys = [1, 2]\n\n[[bin]]\npath = \"src/main.rs\"\n";
        let once = fmt(src);
        assert_eq!(fmt(&once), once);
    }

    #[test]
    fn output_reparses_as_the_same_document() {
        let src = "b=2\na=1\n[t]\nx=[1,2,3]\ny={ q = 1 }\n[[s]]\nn='v'\n";
        for o in [
            Options::default(),
            Options {
                sort_keys: SortKeys::Asc,
                indent: 4,
                align_values: true,
                array_style: ArrayStyle::Expand,
                ..Default::default()
            },
            Options {
                spacing: Spacing::Compact,
                array_style: ArrayStyle::Collapse,
                keep_comments: false,
                blank_line_before_tables: false,
                ..Default::default()
            },
        ] {
            let out = format(src, &o).unwrap();
            let a: toml_edit::DocumentMut = src.parse().unwrap();
            let b: toml_edit::DocumentMut = out.parse().unwrap_or_else(|e| panic!("{e}\n{out}"));
            assert_eq!(
                a.to_string().len() > 0,
                b.to_string().len() > 0,
                "both parse"
            );
            assert_eq!(b["t"]["x"].as_array().unwrap().len(), 3, "{out}");
            assert_eq!(b["a"].as_integer(), Some(1), "{out}");
        }
    }
}
