//! ndjson-filter core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Filters + reshapes newline-delimited JSON
//! (NDJSON / JSON Lines) records with a small deterministic predicate language and
//! a field-selection expression.
//!
//! ## Predicate grammar (one expression, evaluated per record)
//! ```text
//! or_expr  := and_expr ( "or"  and_expr )*
//! and_expr := term     ( "and" term     )*
//! term     := "not" term | "(" or_expr ")" | clause
//! clause   := path [ op value ]
//! op       := == | != | > | >= | < | <= | contains | startswith | endswith | ~ | matches
//! ```
//! `path` is a dotted key path (`user.name`, `items.0.id`); numeric segments index
//! into arrays. A bare `path` clause is true when the value exists and is truthy
//! (non-null, non-false, non-zero, non-empty string/array/object). Comparisons are
//! numeric when both operands are numbers, otherwise string (lexicographic).
//! `~`/`matches` is a regex test; `contains`/`startswith`/`endswith` are
//! case-sensitive substring tests. `and` binds tighter than `or`; use `( )` to group.

use serde_json::Value;

// ------------------------------------------------------------------ predicate AST

#[derive(Clone, Copy, PartialEq, Debug)]
enum Op {
    Eq,
    Ne,
    Gt,
    Ge,
    Lt,
    Le,
    Contains,
    StartsWith,
    EndsWith,
    Matches,
}

enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Exists(String),
    Cmp { path: String, op: Op, lit: Lit },
}

#[derive(Clone)]
enum Lit {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

// ------------------------------------------------------------------ tokenizer

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    LParen,
    RParen,
    And,
    Or,
    Not,
    Op(Op),
    // A quoted string literal (always a string value).
    QStr(String),
    // A bare word: could be a path, a value, or a word-operator — resolved by the parser.
    Word(String),
}

fn tokenize(s: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = s.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        match c {
            '(' => {
                out.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                out.push(Tok::RParen);
                i += 1;
            }
            '"' | '\'' => {
                let quote = c;
                i += 1;
                let mut val = String::new();
                let mut closed = false;
                while i < b.len() {
                    let ch = b[i];
                    if ch == '\\' && i + 1 < b.len() {
                        // Allow escaping the quote and backslash inside a literal.
                        let n = b[i + 1];
                        val.push(match n {
                            'n' => '\n',
                            't' => '\t',
                            other => other,
                        });
                        i += 2;
                        continue;
                    }
                    if ch == quote {
                        closed = true;
                        i += 1;
                        break;
                    }
                    val.push(ch);
                    i += 1;
                }
                if !closed {
                    return Err(format!("unterminated string literal starting with {quote}"));
                }
                out.push(Tok::QStr(val));
            }
            '=' | '!' | '<' | '>' | '~' => {
                let two: String = b[i..(i + 2).min(b.len())].iter().collect();
                let (op, len) = match two.as_str() {
                    "==" => (Op::Eq, 2),
                    "!=" => (Op::Ne, 2),
                    ">=" => (Op::Ge, 2),
                    "<=" => (Op::Le, 2),
                    _ => match c {
                        '=' => (Op::Eq, 1), // tolerate a single '='
                        '>' => (Op::Gt, 1),
                        '<' => (Op::Lt, 1),
                        '~' => (Op::Matches, 1),
                        '!' => return Err("'!' must be part of '!=' (use 'not' to negate)".into()),
                        _ => unreachable!(),
                    },
                };
                out.push(Tok::Op(op));
                i += len;
            }
            _ => {
                // A bare word: read until whitespace, paren, or the start of a symbolic op.
                let start = i;
                while i < b.len() {
                    let ch = b[i];
                    if ch.is_whitespace()
                        || ch == '('
                        || ch == ')'
                        || matches!(ch, '=' | '!' | '<' | '>' | '~')
                    {
                        break;
                    }
                    i += 1;
                }
                let w: String = b[start..i].iter().collect();
                out.push(match w.to_ascii_lowercase().as_str() {
                    "and" => Tok::And,
                    "or" => Tok::Or,
                    "not" => Tok::Not,
                    _ => Tok::Word(w),
                });
            }
        }
    }
    Ok(out)
}

// ------------------------------------------------------------------ parser

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn next(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Tok::Or)) {
            self.next();
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;
        while matches!(self.peek(), Some(Tok::And)) {
            self.next();
            let right = self.parse_term()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        match self.peek() {
            Some(Tok::Not) => {
                self.next();
                Ok(Expr::Not(Box::new(self.parse_term()?)))
            }
            Some(Tok::LParen) => {
                self.next();
                let e = self.parse_or()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(e),
                    _ => Err("missing closing ')'".into()),
                }
            }
            Some(Tok::Word(_)) => self.parse_clause(),
            Some(Tok::QStr(_)) => {
                Err("a bare string is not a condition — write '<path> <op> <value>'".into())
            }
            Some(t) => Err(format!("unexpected token {t:?} in predicate")),
            None => Err("predicate ended unexpectedly".into()),
        }
    }

    fn parse_clause(&mut self) -> Result<Expr, String> {
        let path = match self.next() {
            Some(Tok::Word(w)) => w,
            _ => return Err("expected a field path".into()),
        };
        // Word-operators (contains/startswith/endswith/matches) arrive as bare words.
        let word_op = match self.peek() {
            Some(Tok::Word(w)) => match w.to_ascii_lowercase().as_str() {
                "contains" => Some(Op::Contains),
                "startswith" => Some(Op::StartsWith),
                "endswith" => Some(Op::EndsWith),
                "matches" => Some(Op::Matches),
                _ => None,
            },
            _ => None,
        };
        let op = if let Some(op) = word_op {
            self.next();
            Some(op)
        } else if let Some(Tok::Op(op)) = self.peek() {
            let op = *op;
            self.next();
            Some(op)
        } else {
            None
        };
        let Some(op) = op else {
            // Bare path → exists-and-truthy.
            return Ok(Expr::Exists(path));
        };
        let lit = match self.next() {
            Some(Tok::QStr(s)) => Lit::Str(s),
            Some(Tok::Word(w)) => bareword_to_lit(&w),
            _ => return Err(format!("operator after '{path}' needs a value on its right")),
        };
        Ok(Expr::Cmp { path, op, lit })
    }
}

fn bareword_to_lit(w: &str) -> Lit {
    match w {
        "true" => Lit::Bool(true),
        "false" => Lit::Bool(false),
        "null" => Lit::Null,
        _ => match w.parse::<f64>() {
            Ok(n) => Lit::Num(n),
            Err(_) => Lit::Str(w.to_string()),
        },
    }
}

fn parse_predicate(src: &str) -> Result<Expr, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let e = p.parse_or()?;
    if p.pos != p.toks.len() {
        return Err(format!(
            "unexpected trailing tokens in predicate near {:?}",
            p.toks[p.pos]
        ));
    }
    Ok(e)
}

// ------------------------------------------------------------------ evaluation

/// Resolve a dotted path against a record. Numeric segments index arrays.
fn resolve<'a>(rec: &'a Value, path: &str) -> Option<&'a Value> {
    let mut cur = rec;
    for seg in path.split('.') {
        match cur {
            Value::Object(map) => {
                cur = map.get(seg)?;
            }
            Value::Array(arr) => {
                let idx: usize = seg.parse().ok()?;
                cur = arr.get(idx)?;
            }
            _ => return None,
        }
    }
    Some(cur)
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
    }
}

/// String form of a JSON value for string/regex operators.
fn as_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        other => other.to_string(),
    }
}

fn cmp(v: Option<&Value>, op: Op, lit: &Lit, res: &EvalRes) -> Result<bool, String> {
    // Missing path.
    let Some(v) = v else {
        return Ok(match op {
            Op::Eq => matches!(lit, Lit::Null),
            Op::Ne => !matches!(lit, Lit::Null),
            _ => false,
        });
    };
    match op {
        Op::Eq | Op::Ne => {
            let eq = value_eq(v, lit);
            Ok(if op == Op::Eq { eq } else { !eq })
        }
        Op::Gt | Op::Ge | Op::Lt | Op::Le => {
            let ord = order(v, lit);
            Ok(match ord {
                Some(std::cmp::Ordering::Less) => matches!(op, Op::Lt | Op::Le),
                Some(std::cmp::Ordering::Equal) => matches!(op, Op::Ge | Op::Le),
                Some(std::cmp::Ordering::Greater) => matches!(op, Op::Gt | Op::Ge),
                None => false,
            })
        }
        Op::Contains => Ok(as_text(v).contains(&lit_text(lit))),
        Op::StartsWith => Ok(as_text(v).starts_with(&lit_text(lit))),
        Op::EndsWith => Ok(as_text(v).ends_with(&lit_text(lit))),
        Op::Matches => {
            let re = res.regex(&lit_text(lit))?;
            Ok(re.is_match(&as_text(v)))
        }
    }
}

fn value_eq(v: &Value, lit: &Lit) -> bool {
    match lit {
        Lit::Null => v.is_null(),
        Lit::Bool(b) => v.as_bool() == Some(*b),
        Lit::Num(n) => match v.as_f64() {
            Some(f) => f == *n,
            None => false,
        },
        Lit::Str(s) => match v {
            Value::String(vs) => vs == s,
            other => as_text(other) == *s,
        },
    }
}

fn order(v: &Value, lit: &Lit) -> Option<std::cmp::Ordering> {
    // Numeric when both sides are numbers, else string.
    if let (Some(a), Lit::Num(b)) = (v.as_f64(), lit) {
        return a.partial_cmp(b);
    }
    Some(as_text(v).cmp(&lit_text(lit)))
}

fn lit_text(lit: &Lit) -> String {
    match lit {
        Lit::Str(s) => s.clone(),
        Lit::Num(n) => {
            // Render whole numbers without a trailing ".0" for clean substring/regex tests.
            if n.fract() == 0.0 && n.is_finite() {
                format!("{}", *n as i64)
            } else {
                n.to_string()
            }
        }
        Lit::Bool(b) => b.to_string(),
        Lit::Null => "null".into(),
    }
}

/// Per-invocation evaluation state: caches compiled regexes.
struct EvalRes {
    cache: std::cell::RefCell<std::collections::HashMap<String, regex::Regex>>,
}
impl EvalRes {
    fn new() -> Self {
        Self { cache: std::cell::RefCell::new(std::collections::HashMap::new()) }
    }
    fn regex(&self, pat: &str) -> Result<regex::Regex, String> {
        if let Some(re) = self.cache.borrow().get(pat) {
            return Ok(re.clone());
        }
        let re = regex::Regex::new(pat).map_err(|e| format!("invalid regex '{pat}': {e}"))?;
        self.cache.borrow_mut().insert(pat.to_string(), re.clone());
        Ok(re)
    }
}

fn eval(e: &Expr, rec: &Value, res: &EvalRes) -> Result<bool, String> {
    match e {
        Expr::Or(a, b) => Ok(eval(a, rec, res)? || eval(b, rec, res)?),
        Expr::And(a, b) => Ok(eval(a, rec, res)? && eval(b, rec, res)?),
        Expr::Not(a) => Ok(!eval(a, rec, res)?),
        Expr::Exists(p) => Ok(resolve(rec, p).map(is_truthy).unwrap_or(false)),
        Expr::Cmp { path, op, lit } => cmp(resolve(rec, path), *op, lit, res),
    }
}

// ------------------------------------------------------------------ field selection

struct FieldSpec {
    out: String,
    path: String,
}

fn parse_fields(spec: &str) -> Vec<FieldSpec> {
    spec.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| match s.split_once('=') {
            Some((out, path)) => {
                FieldSpec { out: out.trim().to_string(), path: path.trim().to_string() }
            }
            None => FieldSpec { out: s.to_string(), path: s.to_string() },
        })
        .collect()
}

fn project(rec: &Value, fields: &[FieldSpec]) -> Value {
    if fields.is_empty() {
        return rec.clone();
    }
    let mut map = serde_json::Map::new();
    for f in fields {
        let v = resolve(rec, &f.path).cloned().unwrap_or(Value::Null);
        map.insert(f.out.clone(), v);
    }
    Value::Object(map)
}

// ------------------------------------------------------------------ output

#[derive(Clone, Copy, PartialEq)]
enum Format {
    Ndjson,
    Array,
    Csv,
}

fn parse_format(f: &str) -> Result<Format, String> {
    match f.trim().to_ascii_lowercase().as_str() {
        "" | "ndjson" | "jsonl" => Ok(Format::Ndjson),
        "array" | "json" => Ok(Format::Array),
        "csv" => Ok(Format::Csv),
        other => Err(format!("format must be ndjson, array, or csv (got '{other}')")),
    }
}

fn csv_cell(v: &Value) -> String {
    let raw = as_text(v);
    if raw.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", raw.replace('"', "\"\""))
    } else {
        raw
    }
}

fn to_csv(records: &[Value]) -> String {
    // Column order = first-seen union of object keys across the kept records.
    let mut cols: Vec<String> = Vec::new();
    for r in records {
        if let Value::Object(m) = r {
            for k in m.keys() {
                if !cols.iter().any(|c| c == k) {
                    cols.push(k.clone());
                }
            }
        }
    }
    let mut out = String::new();
    out.push_str(
        &cols
            .iter()
            .map(|c| csv_cell(&Value::String(c.clone())))
            .collect::<Vec<_>>()
            .join(","),
    );
    out.push('\n');
    for r in records {
        let row: Vec<String> = cols
            .iter()
            .map(|c| match r {
                Value::Object(m) => m.get(c).map(csv_cell).unwrap_or_default(),
                _ => String::new(),
            })
            .collect();
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

// ------------------------------------------------------------------ public API

/// Filter + reshape NDJSON. `predicate` empty = keep all rows; `fields` empty =
/// keep whole records. `limit` 0 = unlimited. `invert` keeps the NON-matching rows.
/// With `skip_invalid`, malformed JSON lines are dropped; otherwise the first one
/// errors with its line number. Blank lines are always skipped.
pub fn filter(
    data: &str,
    predicate: &str,
    fields: &str,
    format: &str,
    invert: bool,
    limit: usize,
    skip_invalid: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty — paste one JSON object per line (NDJSON / JSON Lines)".into());
    }
    let fmt = parse_format(format)?;
    let pred = if predicate.trim().is_empty() {
        None
    } else {
        Some(parse_predicate(predicate)?)
    };
    let specs = parse_fields(fields);
    let res = EvalRes::new();

    let mut kept: Vec<Value> = Vec::new();
    for (n, raw) in data.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let rec: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                if skip_invalid {
                    continue;
                }
                return Err(format!(
                    "line {}: invalid JSON: {} (set skip_invalid to drop bad lines)",
                    n + 1,
                    e
                ));
            }
        };
        let matched = match &pred {
            Some(e) => eval(e, &rec, &res)?,
            None => true,
        };
        if matched == invert {
            continue;
        }
        kept.push(project(&rec, &specs));
        if limit != 0 && kept.len() >= limit {
            break;
        }
    }

    Ok(match fmt {
        Format::Ndjson => {
            let mut out = String::new();
            for r in &kept {
                out.push_str(&r.to_string());
                out.push('\n');
            }
            out
        }
        Format::Array => serde_json::to_string_pretty(&Value::Array(kept))
            .map_err(|e| format!("serialize error: {e}"))?,
        Format::Csv => to_csv(&kept),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = r#"{"name":"Alice","age":30,"city":"NYC","active":true}
{"name":"Bob","age":25,"city":"LA","active":false}
{"name":"Carol","age":40,"city":"NYC","active":true}"#;

    #[test]
    fn numeric_predicate() {
        let out = filter(DATA, "age > 28", "", "ndjson", false, 0, false).unwrap();
        assert_eq!(
            out,
            "{\"name\":\"Alice\",\"age\":30,\"city\":\"NYC\",\"active\":true}\n{\"name\":\"Carol\",\"age\":40,\"city\":\"NYC\",\"active\":true}\n"
        );
    }

    #[test]
    fn and_or_grouping() {
        let out =
            filter(DATA, "city == NYC and age < 35", "name", "ndjson", false, 0, false).unwrap();
        assert_eq!(out, "{\"name\":\"Alice\"}\n");
        let out2 =
            filter(DATA, "city == LA or age == 40", "name", "ndjson", false, 0, false).unwrap();
        assert_eq!(out2, "{\"name\":\"Bob\"}\n{\"name\":\"Carol\"}\n");
    }

    #[test]
    fn bare_path_truthy_and_not() {
        let out = filter(DATA, "active", "name", "ndjson", false, 0, false).unwrap();
        assert_eq!(out, "{\"name\":\"Alice\"}\n{\"name\":\"Carol\"}\n");
        let out2 = filter(DATA, "not active", "name", "ndjson", false, 0, false).unwrap();
        assert_eq!(out2, "{\"name\":\"Bob\"}\n");
    }

    #[test]
    fn reshape_rename_and_nested() {
        let d = r#"{"user":{"name":"Al","id":7},"city":"NYC"}"#;
        let out =
            filter(d, "", "name=user.name, uid=user.id, city", "ndjson", false, 0, false).unwrap();
        assert_eq!(out, "{\"name\":\"Al\",\"uid\":7,\"city\":\"NYC\"}\n");
    }

    #[test]
    fn contains_and_regex() {
        let d = "{\"email\":\"a@x.com\"}\n{\"email\":\"b@y.org\"}";
        assert_eq!(
            filter(d, "email contains @x", "", "ndjson", false, 0, false).unwrap(),
            "{\"email\":\"a@x.com\"}\n"
        );
        assert_eq!(
            filter(d, r"email ~ \.org$", "", "ndjson", false, 0, false).unwrap(),
            "{\"email\":\"b@y.org\"}\n"
        );
    }

    #[test]
    fn array_and_csv_output() {
        let arr = filter(DATA, "active", "name,age", "array", false, 0, false).unwrap();
        assert_eq!(
            arr,
            "[\n  {\n    \"name\": \"Alice\",\n    \"age\": 30\n  },\n  {\n    \"name\": \"Carol\",\n    \"age\": 40\n  }\n]"
        );
        let csv = filter(DATA, "active", "name,age", "csv", false, 0, false).unwrap();
        assert_eq!(csv, "name,age\nAlice,30\nCarol,40\n");
    }

    #[test]
    fn csv_quotes_commas() {
        let d = r#"{"a":"x,y","b":"he said \"hi\""}"#;
        let csv = filter(d, "", "a,b", "csv", false, 0, false).unwrap();
        assert_eq!(csv, "a,b\n\"x,y\",\"he said \"\"hi\"\"\"\n");
    }

    #[test]
    fn invert_and_limit() {
        let out = filter(DATA, "active", "name", "ndjson", true, 0, false).unwrap();
        assert_eq!(out, "{\"name\":\"Bob\"}\n");
        let lim = filter(DATA, "", "name", "ndjson", false, 2, false).unwrap();
        assert_eq!(lim, "{\"name\":\"Alice\"}\n{\"name\":\"Bob\"}\n");
    }

    #[test]
    fn skip_invalid_lines() {
        let d = "{\"a\":1}\nnot json\n{\"a\":2}";
        assert_eq!(
            filter(d, "", "a", "ndjson", false, 0, true).unwrap(),
            "{\"a\":1}\n{\"a\":2}\n"
        );
    }

    #[test]
    fn errors() {
        assert!(filter("   ", "", "", "ndjson", false, 0, false).is_err()); // empty input
        let e = filter("{\"a\":1}\noops", "", "", "ndjson", false, 0, false).unwrap_err();
        assert!(e.contains("line 2"), "got: {e}"); // line-numbered parse error
        assert!(filter("{\"a\":1}", "a >", "", "ndjson", false, 0, false).is_err()); // dangling op
        assert!(filter("{\"a\":1}", "", "", "yaml", false, 0, false).is_err()); // bad format
        assert!(filter("{\"a\":1}", "a ~ [", "", "ndjson", false, 0, false).is_err()); // bad regex
    }
}
