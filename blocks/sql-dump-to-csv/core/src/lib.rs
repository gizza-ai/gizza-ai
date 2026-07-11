//! sql-dump-to-csv core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps → compiles to both `wasm32-wasip1`
//! (wafer chat block) and native (CLI + host tests).
//!
//! Extracts the row data from `INSERT` statements in a SQL dump and renders it
//! as RFC-4180 CSV — one CSV section per table. Column names come from the
//! INSERT column list when present, else from a matching `CREATE TABLE`, else
//! from generated `col1..colN` placeholders. `CREATE TABLE`, comments, and any
//! non-INSERT statements are used for context or skipped.
//!
//! Dialect notes (documented on the page): single-quoted string literals accept
//! both SQL-standard doubled quotes (`''`) and MySQL backslash escapes (`\'`,
//! `\n`, …); identifiers may be bare or quoted with backticks, double quotes, or
//! `[brackets]`; `-- …`, `# …`, and `/* … */` comments are stripped. `INSERT …
//! SET …` and `INSERT … SELECT` forms carry no literal rows and are ignored.

use std::collections::HashMap;

/// One extracted value: SQL `NULL`, or the literal text of the cell.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Cell {
    Null,
    Text(String),
}

/// A parsed `INSERT` statement.
struct Insert {
    table: String,
    cols: Option<Vec<String>>,
    rows: Vec<Vec<Cell>>,
}

/// Convert a SQL dump into CSV, one section per table.
///
/// * `sql` — the dump text (paste of a `.sql` file).
/// * `table_filter` — export only this table (case-insensitive); blank = all.
/// * `delimiter` — `comma` | `tab` | `semicolon` | `pipe`.
/// * `header` — emit a first row of column names.
/// * `null_value` — text written for a SQL `NULL` cell (blank = empty field).
/// * `quote` — `minimal` (quote only when needed) | `all` (quote every field).
/// * `bom` — prepend a UTF-8 byte-order mark (helps Excel detect UTF-8).
pub fn convert(
    sql: &str,
    table_filter: &str,
    delimiter: &str,
    header: bool,
    null_value: &str,
    quote: &str,
    bom: bool,
) -> Result<String, String> {
    let delim = parse_delim(delimiter)?;
    let quote_all = match quote.trim().to_ascii_lowercase().as_str() {
        "" | "minimal" => false,
        "all" => true,
        o => return Err(format!("unknown quote mode {o:?} (expected 'minimal' or 'all')")),
    };

    let stmts = split_statements(sql);

    // table key (lowercased) -> data; `order` keeps first-seen display names.
    let mut create_cols: HashMap<String, Vec<String>> = HashMap::new();
    let mut insert_cols: HashMap<String, Vec<String>> = HashMap::new();
    let mut table_rows: HashMap<String, Vec<Vec<Cell>>> = HashMap::new();
    let mut order: Vec<String> = Vec::new();

    for stmt in &stmts {
        if let Some((t, cols)) = parse_create_table(stmt) {
            create_cols.entry(t.to_ascii_lowercase()).or_insert(cols);
            continue;
        }
        if let Some(ins) = parse_insert(stmt) {
            let key = ins.table.to_ascii_lowercase();
            if !table_rows.contains_key(&key) {
                order.push(ins.table.clone());
                table_rows.insert(key.clone(), Vec::new());
            }
            if let Some(cs) = ins.cols {
                insert_cols.entry(key.clone()).or_insert(cs);
            }
            table_rows.get_mut(&key).unwrap().extend(ins.rows);
        }
    }

    if order.is_empty() {
        return Err("no INSERT statements found in the SQL input".into());
    }

    let selected: Vec<String> = if table_filter.trim().is_empty() {
        order.clone()
    } else {
        let f = table_filter.trim().to_ascii_lowercase();
        let m: Vec<String> = order.iter().filter(|d| d.to_ascii_lowercase() == f).cloned().collect();
        if m.is_empty() {
            return Err(format!(
                "no INSERT statements found for table {:?}",
                table_filter.trim()
            ));
        }
        m
    };

    let multi = selected.len() > 1;
    let mut out = String::new();
    if bom {
        out.push('\u{FEFF}');
    }

    for (idx, disp) in selected.iter().enumerate() {
        let key = disp.to_ascii_lowercase();
        let rows = &table_rows[&key];
        let cols: Vec<String> = if let Some(cs) = insert_cols.get(&key) {
            cs.clone()
        } else if let Some(cs) = create_cols.get(&key) {
            cs.clone()
        } else {
            let n = rows.iter().map(|r| r.len()).max().unwrap_or(0);
            (1..=n).map(|i| format!("col{i}")).collect()
        };

        if multi {
            if idx > 0 {
                out.push('\n');
            }
            out.push_str("### TABLE: ");
            out.push_str(disp);
            out.push('\n');
        }

        if header && !cols.is_empty() {
            push_row(&mut out, cols.iter().map(|s| s.as_str()), delim, quote_all);
        }
        for r in rows {
            let fields: Vec<String> = r
                .iter()
                .map(|c| match c {
                    Cell::Null => null_value.to_string(),
                    Cell::Text(t) => t.clone(),
                })
                .collect();
            push_row(&mut out, fields.iter().map(|s| s.as_str()), delim, quote_all);
        }
    }

    Ok(out)
}

fn parse_delim(s: &str) -> Result<char, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "comma" | "," => Ok(','),
        "tab" | "\\t" => Ok('\t'),
        "semicolon" | ";" => Ok(';'),
        "pipe" | "|" => Ok('|'),
        o => Err(format!(
            "unknown delimiter {o:?} (expected one of: comma, tab, semicolon, pipe)"
        )),
    }
}

// ---------- CSV emission (RFC 4180, LF line endings) ----------

fn push_row<'a>(out: &mut String, fields: impl Iterator<Item = &'a str>, delim: char, quote_all: bool) {
    let mut first = true;
    for f in fields {
        if !first {
            out.push(delim);
        }
        first = false;
        out.push_str(&csv_field(f, delim, quote_all));
    }
    out.push('\n');
}

fn csv_field(f: &str, delim: char, quote_all: bool) -> String {
    let needs =
        quote_all || f.contains(delim) || f.contains('"') || f.contains('\n') || f.contains('\r');
    if needs {
        format!("\"{}\"", f.replace('"', "\"\""))
    } else {
        f.to_string()
    }
}

// ---------- scanner ----------

struct Scan {
    c: Vec<char>,
    i: usize,
}

impl Scan {
    fn new(s: &str) -> Self {
        Scan { c: s.chars().collect(), i: 0 }
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn peek2(&self) -> Option<char> {
        self.c.get(self.i + 1).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.i += 1;
        }
        ch
    }
    fn skip_ws(&mut self) {
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() {
                self.i += 1;
            } else {
                break;
            }
        }
    }
    /// Case-insensitive keyword with a trailing word boundary. Consumes leading
    /// whitespace and the keyword on success; restores position on failure.
    fn eat_kw(&mut self, kw: &str) -> bool {
        let save = self.i;
        self.skip_ws();
        let mut j = self.i;
        for k in kw.chars() {
            match self.c.get(j) {
                Some(ch) if ch.eq_ignore_ascii_case(&k) => j += 1,
                _ => {
                    self.i = save;
                    return false;
                }
            }
        }
        if let Some(ch) = self.c.get(j) {
            if ch.is_alphanumeric() || *ch == '_' {
                self.i = save;
                return false;
            }
        }
        self.i = j;
        true
    }
    /// Read a bare or quoted identifier (``` `x` ```, `"x"`, `[x]`, or `x`).
    fn read_ident(&mut self) -> Option<String> {
        self.skip_ws();
        match self.peek() {
            Some('`') => self.read_delim_ident('`', '`'),
            Some('"') => self.read_delim_ident('"', '"'),
            Some('[') => self.read_delim_ident('[', ']'),
            Some(ch) if ch.is_alphabetic() || ch == '_' => {
                let mut s = String::new();
                while let Some(c) = self.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '$' {
                        s.push(c);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                Some(s)
            }
            _ => None,
        }
    }
    fn read_delim_ident(&mut self, open: char, close: char) -> Option<String> {
        if self.peek() != Some(open) {
            return None;
        }
        self.bump();
        let mut s = String::new();
        loop {
            let ch = self.bump()?;
            if ch == close {
                // doubled close char = literal (e.g. `` `a``b` `` or `"a""b"`).
                if open == close && self.peek() == Some(close) {
                    self.bump();
                    s.push(close);
                    continue;
                }
                break;
            }
            s.push(ch);
        }
        Some(s)
    }
    /// Read a (possibly schema-qualified) table name; keep the final component.
    fn read_qualified_ident(&mut self) -> Option<String> {
        let mut last = self.read_ident()?;
        loop {
            self.skip_ws();
            if self.peek() == Some('.') {
                self.bump();
                last = self.read_ident()?;
            } else {
                break;
            }
        }
        Some(last)
    }
}

/// Read a single-quoted SQL string literal (cursor on the opening `'`),
/// returning the decoded text. Handles `''` doubling and MySQL `\` escapes.
fn read_sql_string(sc: &mut Scan) -> Option<String> {
    if sc.peek() != Some('\'') {
        return None;
    }
    sc.bump();
    let mut out = String::new();
    loop {
        let ch = sc.bump()?;
        match ch {
            '\\' => {
                if let Some(n) = sc.bump() {
                    out.push(match n {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '0' => '\0',
                        'b' => '\u{8}',
                        'Z' => '\u{1a}',
                        other => other, // \', \", \\ and any other → literal char
                    });
                }
            }
            '\'' => {
                if sc.peek() == Some('\'') {
                    sc.bump();
                    out.push('\'');
                } else {
                    break;
                }
            }
            _ => out.push(ch),
        }
    }
    Some(out)
}

// ---------- statement splitting ----------

/// Split the dump into statements on top-level `;`, stripping comments. Aware of
/// string literals and quoted identifiers so a `;`/`--` inside them is safe.
fn split_statements(input: &str) -> Vec<String> {
    let mut sc = Scan::new(input);
    let mut out = Vec::new();
    let mut buf = String::new();
    while let Some(ch) = sc.peek() {
        match ch {
            '\'' | '"' | '`' | '[' => {
                let (open, close) = match ch {
                    '[' => ('[', ']'),
                    c => (c, c),
                };
                // Copy the quoted run verbatim into buf, tracking escapes.
                buf.push(sc.bump().unwrap());
                loop {
                    match sc.bump() {
                        None => break,
                        Some('\\') if open == '\'' => {
                            buf.push('\\');
                            if let Some(n) = sc.bump() {
                                buf.push(n);
                            }
                        }
                        Some(c) if c == close => {
                            if open == close && sc.peek() == Some(close) {
                                buf.push(c);
                                buf.push(sc.bump().unwrap());
                            } else {
                                buf.push(c);
                                break;
                            }
                        }
                        Some(c) => buf.push(c),
                    }
                }
            }
            '-' if sc.peek2() == Some('-') => {
                while let Some(c) = sc.peek() {
                    sc.bump();
                    if c == '\n' {
                        buf.push('\n');
                        break;
                    }
                }
            }
            '#' => {
                while let Some(c) = sc.peek() {
                    sc.bump();
                    if c == '\n' {
                        buf.push('\n');
                        break;
                    }
                }
            }
            '/' if sc.peek2() == Some('*') => {
                sc.bump();
                sc.bump();
                loop {
                    match sc.bump() {
                        None => break,
                        Some('*') if sc.peek() == Some('/') => {
                            sc.bump();
                            break;
                        }
                        _ => {}
                    }
                }
                buf.push(' ');
            }
            ';' => {
                sc.bump();
                if !buf.trim().is_empty() {
                    out.push(buf.trim().to_string());
                }
                buf.clear();
            }
            _ => {
                buf.push(sc.bump().unwrap());
            }
        }
    }
    if !buf.trim().is_empty() {
        out.push(buf.trim().to_string());
    }
    out
}

// ---------- CREATE TABLE ----------

fn parse_create_table(stmt: &str) -> Option<(String, Vec<String>)> {
    let mut sc = Scan::new(stmt);
    if !sc.eat_kw("CREATE") {
        return None;
    }
    let _ = sc.eat_kw("TEMPORARY") || sc.eat_kw("TEMP");
    if !sc.eat_kw("TABLE") {
        return None;
    }
    let _ = sc.eat_kw("IF") && sc.eat_kw("NOT") && sc.eat_kw("EXISTS");
    let table = sc.read_qualified_ident()?;
    sc.skip_ws();
    if sc.peek() != Some('(') {
        return None;
    }
    sc.bump();

    let mut cols = Vec::new();
    loop {
        sc.skip_ws();
        if sc.peek() == Some(')') || sc.peek().is_none() {
            break;
        }
        let ident = sc.read_ident();
        let is_constraint = ident
            .as_ref()
            .map(|s| {
                matches!(
                    s.to_ascii_uppercase().as_str(),
                    "PRIMARY"
                        | "FOREIGN"
                        | "UNIQUE"
                        | "KEY"
                        | "CONSTRAINT"
                        | "CHECK"
                        | "INDEX"
                        | "FULLTEXT"
                        | "SPATIAL"
                )
            })
            .unwrap_or(true);
        if let Some(id) = ident {
            if !is_constraint {
                cols.push(id);
            }
        }
        // Skip the rest of this definition to the next top-level comma / `)`.
        let mut depth = 0i32;
        loop {
            match sc.peek() {
                None => {
                    return if cols.is_empty() { None } else { Some((table, cols)) };
                }
                Some('(') => {
                    depth += 1;
                    sc.bump();
                }
                Some(')') => {
                    if depth == 0 {
                        return Some((table, cols));
                    }
                    depth -= 1;
                    sc.bump();
                }
                Some('\'') => {
                    let _ = read_sql_string(&mut sc);
                }
                Some(',') if depth == 0 => {
                    sc.bump();
                    break;
                }
                Some(_) => {
                    sc.bump();
                }
            }
        }
    }
    if cols.is_empty() {
        None
    } else {
        Some((table, cols))
    }
}

// ---------- INSERT ----------

fn parse_insert(stmt: &str) -> Option<Insert> {
    let mut sc = Scan::new(stmt);
    if !(sc.eat_kw("INSERT") || sc.eat_kw("REPLACE")) {
        return None;
    }
    loop {
        if sc.eat_kw("IGNORE")
            || sc.eat_kw("LOW_PRIORITY")
            || sc.eat_kw("DELAYED")
            || sc.eat_kw("HIGH_PRIORITY")
        {
            continue;
        }
        if sc.eat_kw("OR") {
            let _ = sc.eat_kw("REPLACE")
                || sc.eat_kw("IGNORE")
                || sc.eat_kw("ROLLBACK")
                || sc.eat_kw("ABORT")
                || sc.eat_kw("FAIL");
            continue;
        }
        break;
    }
    if !sc.eat_kw("INTO") {
        return None;
    }
    let table = sc.read_qualified_ident()?;

    // Optional column list — present only when a `(` precedes the VALUES keyword.
    let mut cols: Option<Vec<String>> = None;
    sc.skip_ws();
    if sc.peek() == Some('(') {
        sc.bump();
        let mut cs = Vec::new();
        loop {
            let id = sc.read_ident()?;
            cs.push(id);
            sc.skip_ws();
            match sc.peek() {
                Some(',') => {
                    sc.bump();
                }
                Some(')') => {
                    sc.bump();
                    break;
                }
                _ => return None,
            }
        }
        cols = Some(cs);
    }

    if !(sc.eat_kw("VALUES") || sc.eat_kw("VALUE")) {
        // SET-form / SELECT-form INSERT: no literal tuples to extract.
        return None;
    }

    let mut rows = Vec::new();
    loop {
        sc.skip_ws();
        if sc.peek() != Some('(') {
            break;
        }
        sc.bump();
        let tuple = parse_value_tuple(&mut sc)?;
        rows.push(tuple);
        sc.skip_ws();
        if sc.peek() == Some(',') {
            sc.bump();
            continue;
        }
        break;
    }

    if rows.is_empty() {
        return None;
    }
    Some(Insert { table, cols, rows })
}

/// Parse the values inside one `( … )` tuple (cursor just past the `(`).
fn parse_value_tuple(sc: &mut Scan) -> Option<Vec<Cell>> {
    let mut vals = Vec::new();
    // Empty tuple `()`.
    sc.skip_ws();
    if sc.peek() == Some(')') {
        sc.bump();
        return Some(vals);
    }
    loop {
        let cell = parse_value(sc)?;
        vals.push(cell);
        sc.skip_ws();
        match sc.peek() {
            Some(',') => {
                sc.bump();
            }
            Some(')') => {
                sc.bump();
                break;
            }
            _ => return None,
        }
    }
    Some(vals)
}

/// Parse one value: a string literal, `NULL`, or a bare token (number, boolean,
/// hex/blob, or a parenthesized expression kept as raw text).
fn parse_value(sc: &mut Scan) -> Option<Cell> {
    sc.skip_ws();
    match sc.peek() {
        Some('\'') => Some(Cell::Text(read_sql_string(sc)?)),
        // N'...' national-charset string prefix (SQL Server / MySQL).
        Some(c) if (c == 'N' || c == 'n') && sc.peek2() == Some('\'') => {
            sc.bump();
            Some(Cell::Text(read_sql_string(sc)?))
        }
        Some(_) => {
            let mut depth = 0i32;
            let mut raw = String::new();
            loop {
                match sc.peek() {
                    None => break,
                    Some(',') | Some(')') if depth == 0 => break,
                    Some('(') => {
                        depth += 1;
                        raw.push('(');
                        sc.bump();
                    }
                    Some(')') => {
                        depth -= 1;
                        raw.push(')');
                        sc.bump();
                    }
                    Some('\'') => {
                        let s = read_sql_string(sc)?;
                        raw.push('\'');
                        raw.push_str(&s);
                        raw.push('\'');
                    }
                    Some(ch) => {
                        raw.push(ch);
                        sc.bump();
                    }
                }
            }
            let t = raw.trim();
            if t.eq_ignore_ascii_case("NULL") {
                Some(Cell::Null)
            } else {
                Some(Cell::Text(t.to_string()))
            }
        }
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(sql: &str) -> String {
        convert(sql, "", "comma", true, "", "minimal", false).unwrap()
    }

    #[test]
    fn happy_single_table_with_column_list() {
        let sql = "INSERT INTO users (id, name) VALUES (1, 'Alice'), (2, 'Bob');";
        assert_eq!(c(sql), "id,name\n1,Alice\n2,Bob\n");
    }

    #[test]
    fn header_from_create_table_when_insert_omits_columns() {
        let sql = "CREATE TABLE t (a INT, b TEXT, PRIMARY KEY (a));\n\
                   INSERT INTO t VALUES (1, 'x'), (2, 'y');";
        assert_eq!(c(sql), "a,b\n1,x\n2,y\n");
    }

    #[test]
    fn synthesized_columns_without_schema() {
        let sql = "INSERT INTO t VALUES (1, 'x');";
        assert_eq!(c(sql), "col1,col2\n1,x\n");
    }

    #[test]
    fn null_and_quoting_and_escapes() {
        let sql = "INSERT INTO t (a, b, c) VALUES (NULL, 'a,b', 'she said ''hi''');";
        // comma inside a field forces quoting; doubled '' → single quote.
        assert_eq!(c(sql), "a,b,c\n,\"a,b\",she said 'hi'\n");
    }

    #[test]
    fn null_value_override_and_quote_all() {
        let sql = "INSERT INTO t (a, b) VALUES (NULL, 'x');";
        let got = convert(sql, "", "comma", true, "\\N", "all", false).unwrap();
        assert_eq!(got, "\"a\",\"b\"\n\"\\N\",\"x\"\n");
    }

    #[test]
    fn multiple_tables_get_sections_and_filter() {
        let sql = "INSERT INTO a (x) VALUES (1);\nINSERT INTO b (y) VALUES (2);";
        let all = c(sql);
        assert_eq!(all, "### TABLE: a\nx\n1\n\n### TABLE: b\ny\n2\n");
        // Filtering to one table drops the section marker → clean CSV.
        let just_b = convert(sql, "b", "comma", true, "", "minimal", false).unwrap();
        assert_eq!(just_b, "y\n2\n");
    }

    #[test]
    fn tab_delimiter_and_no_header_and_backslash_escape() {
        let sql = "INSERT INTO `t` (`a`, `b`) VALUES ('x\\ty', 'line1\\nline2');";
        // Field values contain a real tab / newline → quoted; header suppressed.
        let got = convert(sql, "", "tab", false, "", "minimal", false).unwrap();
        assert_eq!(got, "\"x\ty\"\t\"line1\nline2\"\n");
    }

    #[test]
    fn comments_and_multiple_inserts_same_table_accumulate() {
        let sql = "-- a dump\nINSERT INTO t (a) VALUES (1); # trailing\n\
                   /* block */ INSERT INTO t (a) VALUES (2);";
        assert_eq!(c(sql), "a\n1\n2\n");
    }

    #[test]
    fn bom_prefix() {
        let sql = "INSERT INTO t (a) VALUES (1);";
        let got = convert(sql, "", "comma", true, "", "minimal", true).unwrap();
        assert!(got.starts_with('\u{FEFF}'));
        assert_eq!(&got[3..], "a\n1\n");
    }

    #[test]
    fn error_when_no_inserts() {
        let sql = "CREATE TABLE t (a INT);\nSELECT * FROM t;";
        let err = convert(sql, "", "comma", true, "", "minimal", false).unwrap_err();
        assert!(err.contains("no INSERT statements"), "got: {err}");
    }

    #[test]
    fn error_when_table_filter_misses() {
        let sql = "INSERT INTO t (a) VALUES (1);";
        let err = convert(sql, "nope", "comma", true, "", "minimal", false).unwrap_err();
        assert!(err.contains("table \"nope\""), "got: {err}");
    }

    #[test]
    fn error_on_bad_delimiter() {
        let sql = "INSERT INTO t (a) VALUES (1);";
        let err = convert(sql, "", "colon", true, "", "minimal", false).unwrap_err();
        assert!(err.contains("unknown delimiter"), "got: {err}");
    }

    #[test]
    fn semicolon_inside_string_is_not_a_statement_break() {
        let sql = "INSERT INTO t (a) VALUES ('a;b'), ('c');";
        // semicolon delimiter would collide, so use pipe to see the field intact.
        let got = convert(sql, "", "pipe", true, "", "minimal", false).unwrap();
        assert_eq!(got, "a\na;b\nc\n");
    }
}
