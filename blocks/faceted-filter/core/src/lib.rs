//! faceted-filter core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Runs a faceted search over a JSON dataset:
//! a text query + a filter expression select records, the survivors are sorted and
//! paginated, and every requested facet field is broken down into value → count
//! buckets computed from the current result set.
//!
//! ## Filter grammar (one expression, evaluated per record)
//! ```text
//! or_expr  := and_expr ( "or"  and_expr )*
//! and_expr := term     ( "and" term     )*
//! term     := "not" term | "(" or_expr ")" | clause
//! clause   := path [ op value ]
//! op       := == | != | > | >= | < | <= | contains | startswith | endswith
//!           | ~ | matches | in
//! ```
//! `path` is a dotted key path (`user.name`, `items.0.id`); a numeric segment
//! indexes an array, and a non-numeric segment applied to an array of objects maps
//! over its elements. A clause holds when ANY resolved value satisfies it, so
//! array-valued fields (`tags`) behave like search-engine collection fields. A bare
//! `path` clause is true when the value exists and is truthy. `in` takes a
//! comma-separated (optionally `[ ]`-bracketed) list. Comparisons are numeric when
//! both sides are numbers, otherwise string. `and` binds tighter than `or`.

use regex::Regex;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// Maximum number of records accepted in one run.
pub const MAX_RECORDS: usize = 50_000;
/// Maximum rows returned per page.
pub const MAX_PER_PAGE: usize = 1_000;
/// Maximum number of values reported per facet (0 in the param means unlimited).
pub const MAX_FACET_LIMIT: usize = 1_000;
/// Maximum number of facet fields auto-detected when `facets` is blank.
pub const MAX_AUTO_FACETS: usize = 20;

// ------------------------------------------------------------------ filter AST

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
    In,
}

#[derive(Clone)]
enum Expr {
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    /// Always-true leaf, produced when a facet's own clauses are neutralized for
    /// disjunctive counting.
    Const(bool),
    Exists(String),
    Cmp {
        path: String,
        op: Op,
        lits: Vec<Lit>,
        re: Option<Regex>,
    },
}

#[derive(Clone, PartialEq, Debug)]
enum Lit {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
}

impl Lit {
    fn from_word(w: &str) -> Lit {
        match w {
            "true" => Lit::Bool(true),
            "false" => Lit::Bool(false),
            "null" => Lit::Null,
            _ => match w.parse::<f64>() {
                Ok(n) if n.is_finite() => Lit::Num(n),
                _ => Lit::Str(w.to_string()),
            },
        }
    }

    fn as_text(&self) -> String {
        match self {
            Lit::Str(s) => s.clone(),
            Lit::Num(n) => fmt_num(*n),
            Lit::Bool(b) => b.to_string(),
            Lit::Null => "null".into(),
        }
    }
}

// ------------------------------------------------------------------ tokenizer

#[derive(Clone, PartialEq, Debug)]
enum Tok {
    LParen,
    RParen,
    Op(Op),
    /// A quoted string literal (always a string value).
    QStr(String),
    /// A bare word: a path, a value, or a word-operator — resolved by the parser.
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
                    return Err(format!(
                        "unterminated string literal starting with {quote} in the filter expression"
                    ));
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
                        '!' => {
                            return Err("'!' must be part of '!=' (use 'not' to negate a clause)"
                                .into())
                        }
                        _ => unreachable!(),
                    },
                };
                out.push(Tok::Op(op));
                i += len;
            }
            _ => {
                let mut w = String::new();
                while i < b.len() {
                    let ch = b[i];
                    if ch.is_whitespace() || "()\"'=!<>~".contains(ch) {
                        break;
                    }
                    w.push(ch);
                    i += 1;
                }
                out.push(Tok::Word(w));
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

    fn is_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Word(w)) if w.eq_ignore_ascii_case(kw))
    }

    fn parse_or(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_and()?;
        while self.is_kw("or") {
            self.pos += 1;
            let right = self.parse_and()?;
            left = Expr::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<Expr, String> {
        let mut left = self.parse_term()?;
        while self.is_kw("and") {
            self.pos += 1;
            let right = self.parse_term()?;
            left = Expr::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<Expr, String> {
        if self.is_kw("not") {
            self.pos += 1;
            return Ok(Expr::Not(Box::new(self.parse_term()?)));
        }
        if self.peek() == Some(&Tok::LParen) {
            self.pos += 1;
            let inner = self.parse_or()?;
            if self.peek() != Some(&Tok::RParen) {
                return Err("expected ')' to close a group in the filter expression".into());
            }
            self.pos += 1;
            return Ok(inner);
        }
        self.parse_clause()
    }

    fn parse_clause(&mut self) -> Result<Expr, String> {
        let path = match self.peek().cloned() {
            Some(Tok::Word(w)) => {
                self.pos += 1;
                w
            }
            Some(Tok::QStr(w)) => {
                self.pos += 1;
                w
            }
            other => {
                return Err(format!(
                    "expected a field path in the filter expression, got {}",
                    describe(other.as_ref())
                ))
            }
        };
        // Symbol operator, or a word operator (contains / startswith / … / in).
        let op = match self.peek().cloned() {
            Some(Tok::Op(o)) => {
                self.pos += 1;
                Some(o)
            }
            Some(Tok::Word(w)) => match w.to_ascii_lowercase().as_str() {
                "contains" => {
                    self.pos += 1;
                    Some(Op::Contains)
                }
                "startswith" => {
                    self.pos += 1;
                    Some(Op::StartsWith)
                }
                "endswith" => {
                    self.pos += 1;
                    Some(Op::EndsWith)
                }
                "matches" => {
                    self.pos += 1;
                    Some(Op::Matches)
                }
                "in" => {
                    self.pos += 1;
                    Some(Op::In)
                }
                _ => None,
            },
            _ => None,
        };
        let Some(op) = op else {
            return Ok(Expr::Exists(path));
        };
        if op == Op::In {
            let lits = self.parse_list(&path)?;
            return Ok(Expr::Cmp {
                path,
                op,
                lits,
                re: None,
            });
        }
        let lit = self.parse_value(&path)?;
        let re = if op == Op::Matches {
            Some(
                Regex::new(&lit.as_text())
                    .map_err(|e| format!("invalid regex for '{path}': {e}"))?,
            )
        } else {
            None
        };
        Ok(Expr::Cmp {
            path,
            op,
            lits: vec![lit],
            re,
        })
    }

    fn parse_value(&mut self, path: &str) -> Result<Lit, String> {
        match self.peek().cloned() {
            Some(Tok::QStr(s)) => {
                self.pos += 1;
                Ok(Lit::Str(s))
            }
            Some(Tok::Word(w)) => {
                self.pos += 1;
                Ok(Lit::from_word(&w))
            }
            other => Err(format!(
                "expected a value after the operator on '{path}', got {}",
                describe(other.as_ref())
            )),
        }
    }

    /// The value list of an `in` clause. Accepts `[a, b]`, `a, b`, `a|b` and
    /// `a b` — `[`, `]`, `,` and `|` are ordinary value characters elsewhere (a
    /// regex like `^[AB]` must stay intact), so the list is split here instead of
    /// in the tokenizer. A quoted item is taken whole, commas included.
    fn parse_list(&mut self, path: &str) -> Result<Vec<Lit>, String> {
        let mut lits: Vec<Lit> = Vec::new();
        loop {
            match self.peek().cloned() {
                Some(Tok::QStr(s)) => {
                    self.pos += 1;
                    lits.push(Lit::Str(s));
                }
                Some(Tok::Word(w)) if !is_keyword(&w) => {
                    self.pos += 1;
                    for part in w.trim_matches(|c| c == '[' || c == ']').split([',', '|']) {
                        let p = part.trim();
                        if !p.is_empty() {
                            lits.push(Lit::from_word(p));
                        }
                    }
                }
                _ => break,
            }
        }
        if lits.is_empty() {
            return Err(format!(
                "expected at least one value after 'in' for '{path}', e.g. {path} in [red, blue]"
            ));
        }
        Ok(lits)
    }
}

fn is_keyword(w: &str) -> bool {
    matches!(w.to_ascii_lowercase().as_str(), "and" | "or" | "not")
}

fn describe(t: Option<&Tok>) -> String {
    match t {
        None => "the end of the expression".into(),
        Some(Tok::LParen) => "'('".into(),
        Some(Tok::RParen) => "')'".into(),
        Some(Tok::Op(_)) => "an operator".into(),
        Some(Tok::QStr(s)) => format!("\"{s}\""),
        Some(Tok::Word(w)) => format!("'{w}'"),
    }
}

fn parse_filter(src: &str) -> Result<Option<Expr>, String> {
    if src.trim().is_empty() {
        return Ok(None);
    }
    let toks = tokenize(src)?;
    if toks.is_empty() {
        return Ok(None);
    }
    let mut p = Parser { toks, pos: 0 };
    let e = p.parse_or()?;
    if p.pos != p.toks.len() {
        return Err(format!(
            "unexpected {} at the end of the filter expression — join clauses with 'and' / 'or'",
            describe(p.peek())
        ));
    }
    Ok(Some(e))
}

// ------------------------------------------------------------------ path + value helpers

/// Resolve a dotted path to every value it addresses. A numeric segment indexes an
/// array; a non-numeric segment applied to an array maps over its elements, so
/// `items.id` reaches every `id` in an array of objects.
fn resolve<'a>(root: &'a Value, path: &str) -> Vec<&'a Value> {
    let mut cur: Vec<&Value> = vec![root];
    for seg in path.split('.') {
        let mut next: Vec<&Value> = Vec::new();
        for v in cur {
            match v {
                Value::Object(m) => {
                    if let Some(x) = m.get(seg) {
                        next.push(x);
                    }
                }
                Value::Array(a) => match seg.parse::<usize>() {
                    Ok(i) => {
                        if let Some(x) = a.get(i) {
                            next.push(x);
                        }
                    }
                    Err(_) => {
                        for e in a {
                            if let Value::Object(m) = e {
                                if let Some(x) = m.get(seg) {
                                    next.push(x);
                                }
                            }
                        }
                    }
                },
                _ => {}
            }
        }
        cur = next;
        if cur.is_empty() {
            return cur;
        }
    }
    cur
}

/// Flatten one array level so a collection field compares element-wise.
fn flatten<'a>(vals: Vec<&'a Value>) -> Vec<&'a Value> {
    let mut out = Vec::new();
    for v in vals {
        match v {
            Value::Array(a) => out.extend(a.iter()),
            other => out.push(other),
        }
    }
    out
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// The facet-bucket / search text for a scalar value; `None` for null, objects and
/// nested arrays (which have no single sensible bucket key).
fn scalar_text(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|x| x != 0.0).unwrap_or(true),
        Value::String(s) => !s.is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
    }
}

fn cmp_one(v: &Value, op: Op, lit: &Lit, re: Option<&Regex>) -> bool {
    // Numeric comparison when both sides are numbers, string comparison otherwise.
    let vnum = v.as_f64();
    let lnum = match lit {
        Lit::Num(n) => Some(*n),
        _ => None,
    };
    let vtext = match v {
        Value::String(s) => s.clone(),
        Value::Null => "null".into(),
        other => scalar_text(other).unwrap_or_else(|| other.to_string()),
    };
    let ltext = lit.as_text();
    match op {
        Op::Eq | Op::Ne => {
            let eq = match (vnum, lnum) {
                (Some(a), Some(b)) => a == b,
                _ => match (v, lit) {
                    (Value::Null, Lit::Null) => true,
                    (Value::Null, _) | (_, Lit::Null) => false,
                    _ => vtext == ltext,
                },
            };
            if op == Op::Eq {
                eq
            } else {
                !eq
            }
        }
        Op::Gt | Op::Ge | Op::Lt | Op::Le => {
            let ord = match (vnum, lnum) {
                (Some(a), Some(b)) => a.partial_cmp(&b),
                _ => Some(vtext.as_str().cmp(ltext.as_str())),
            };
            match ord {
                None => false,
                Some(o) => match op {
                    Op::Gt => o.is_gt(),
                    Op::Ge => o.is_ge(),
                    Op::Lt => o.is_lt(),
                    _ => o.is_le(),
                },
            }
        }
        Op::Contains => vtext.contains(&ltext),
        Op::StartsWith => vtext.starts_with(&ltext),
        Op::EndsWith => vtext.ends_with(&ltext),
        Op::Matches => re.map(|r| r.is_match(&vtext)).unwrap_or(false),
        Op::In => unreachable!("handled by the caller"),
    }
}

fn eval(e: &Expr, rec: &Value) -> bool {
    match e {
        Expr::Const(b) => *b,
        Expr::Or(a, b) => eval(a, rec) || eval(b, rec),
        Expr::And(a, b) => eval(a, rec) && eval(b, rec),
        Expr::Not(a) => !eval(a, rec),
        Expr::Exists(path) => flatten(resolve(rec, path)).iter().any(|v| truthy(v)),
        Expr::Cmp {
            path,
            op,
            lits,
            re,
        } => {
            let vals = flatten(resolve(rec, path));
            match op {
                Op::In => vals
                    .iter()
                    .any(|v| lits.iter().any(|l| cmp_one(v, Op::Eq, l, None))),
                // `!=` on a missing field is true (the record does not hold that value).
                Op::Ne if vals.is_empty() => true,
                _ => vals.iter().any(|v| cmp_one(v, *op, &lits[0], re.as_ref())),
            }
        }
    }
}

/// Rewrite the filter so every clause on `field` becomes always-true — the standard
/// disjunctive-faceting trick that keeps a facet's unselected values countable.
fn neutralize(e: &Expr, field: &str) -> Expr {
    match e {
        Expr::Or(a, b) => Expr::Or(
            Box::new(neutralize(a, field)),
            Box::new(neutralize(b, field)),
        ),
        Expr::And(a, b) => Expr::And(
            Box::new(neutralize(a, field)),
            Box::new(neutralize(b, field)),
        ),
        Expr::Not(a) => match a.as_ref() {
            Expr::Exists(p) if p.as_str() == field => Expr::Const(true),
            Expr::Cmp { path, .. } if path.as_str() == field => Expr::Const(true),
            other => Expr::Not(Box::new(neutralize(other, field))),
        },
        Expr::Exists(p) if p.as_str() == field => Expr::Const(true),
        Expr::Cmp { path, .. } if path.as_str() == field => Expr::Const(true),
        other => other.clone(),
    }
}

// ------------------------------------------------------------------ text query

fn collect_text(v: &Value, out: &mut Vec<String>) {
    match v {
        Value::String(s) => out.push(s.to_lowercase()),
        Value::Number(n) => out.push(n.to_string()),
        Value::Bool(b) => out.push(b.to_string()),
        Value::Array(a) => a.iter().for_each(|e| collect_text(e, out)),
        Value::Object(m) => m.iter().for_each(|(_, e)| collect_text(e, out)),
        Value::Null => {}
    }
}

fn query_matches(rec: &Value, terms: &[String], fields: &[String]) -> bool {
    let mut hay: Vec<String> = Vec::new();
    if fields.is_empty() {
        collect_text(rec, &mut hay);
    } else {
        for f in fields {
            for v in resolve(rec, f) {
                collect_text(v, &mut hay);
            }
        }
    }
    let joined = hay.join("\n");
    terms.iter().all(|t| joined.contains(t))
}

// ------------------------------------------------------------------ facets

struct FacetAcc {
    order: Vec<String>,
    counts: HashMap<String, usize>,
    nums: Vec<f64>,
}

impl FacetAcc {
    fn new() -> Self {
        FacetAcc {
            order: Vec::new(),
            counts: HashMap::new(),
            nums: Vec::new(),
        }
    }

    fn add(&mut self, key: String, num: Option<f64>) {
        if let Some(n) = num {
            self.nums.push(n);
        }
        match self.counts.get_mut(&key) {
            Some(c) => *c += 1,
            None => {
                self.counts.insert(key.clone(), 1);
                self.order.push(key);
            }
        }
    }
}

/// Auto-detect facet fields: the top-level keys, in first-seen order, whose value
/// is a scalar or an array of scalars in at least one record.
fn auto_facets(records: &[Value]) -> Vec<String> {
    let mut order: Vec<String> = Vec::new();
    let mut seen: HashMap<String, ()> = HashMap::new();
    for rec in records {
        let Value::Object(m) = rec else { continue };
        for (k, v) in m {
            let facetable = match v {
                Value::String(_) | Value::Number(_) | Value::Bool(_) => true,
                Value::Array(a) => a.iter().any(|e| scalar_text(e).is_some()),
                _ => false,
            };
            if facetable && !seen.contains_key(k) {
                seen.insert(k.clone(), ());
                order.push(k.clone());
                if order.len() >= MAX_AUTO_FACETS {
                    return order;
                }
            }
        }
    }
    order
}

fn cmp_facet_value(a: &str, b: &str) -> std::cmp::Ordering {
    match (a.parse::<f64>(), b.parse::<f64>()) {
        (Ok(x), Ok(y)) => x.partial_cmp(&y).unwrap_or(std::cmp::Ordering::Equal),
        _ => a.cmp(b),
    }
}

// ------------------------------------------------------------------ item sort

struct SortKey {
    path: String,
    desc: bool,
}

fn parse_sort(spec: &str) -> Result<Vec<SortKey>, String> {
    let mut keys = Vec::new();
    for raw in spec.split(',') {
        let entry = raw.trim();
        if entry.is_empty() {
            continue;
        }
        let (path, desc) = match entry.split_once(':') {
            Some((p, dir)) => {
                let d = dir.trim().to_ascii_lowercase();
                let desc = match d.as_str() {
                    "desc" | "descending" | "-1" => true,
                    "asc" | "ascending" | "1" => false,
                    _ => {
                        return Err(format!(
                            "sort direction for '{}' must be asc or desc, got '{}'",
                            p.trim(),
                            dir.trim()
                        ))
                    }
                };
                (p.trim(), desc)
            }
            None => match entry.strip_prefix('-') {
                Some(p) => (p.trim(), true),
                None => (entry, false),
            },
        };
        if path.is_empty() {
            return Err("sort needs a field path, e.g. price:desc".into());
        }
        keys.push(SortKey {
            path: path.to_string(),
            desc,
        });
    }
    Ok(keys)
}

/// Kind rank so mixed-type columns still order deterministically; missing/null last.
fn kind_rank(v: Option<&Value>) -> u8 {
    match v {
        None | Some(Value::Null) => 4,
        Some(Value::Bool(_)) => 0,
        Some(Value::Number(_)) => 1,
        Some(Value::String(_)) => 2,
        _ => 3,
    }
}

fn cmp_sort_values(a: Option<&Value>, b: Option<&Value>) -> std::cmp::Ordering {
    let (ra, rb) = (kind_rank(a), kind_rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (Some(Value::Bool(x)), Some(Value::Bool(y))) => x.cmp(y),
        (Some(Value::Number(x)), Some(Value::Number(y))) => x
            .as_f64()
            .unwrap_or(f64::NAN)
            .partial_cmp(&y.as_f64().unwrap_or(f64::NAN))
            .unwrap_or(std::cmp::Ordering::Equal),
        (Some(Value::String(x)), Some(Value::String(y))) => x.cmp(y),
        _ => std::cmp::Ordering::Equal,
    }
}

// ------------------------------------------------------------------ dataset parsing

fn parse_dataset(data: &str) -> Result<Vec<Value>, String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err("no data: paste a JSON array of records, e.g. [{\"brand\":\"Apple\"}]".into());
    }
    let records = match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Array(a)) => a,
        Ok(Value::Object(m)) => vec![Value::Object(m)],
        Ok(other) => {
            return Err(format!(
                "expected a JSON array of records, got a JSON {}",
                type_name(&other)
            ))
        }
        Err(whole) => {
            // Fall back to NDJSON / JSON Lines: one record per line.
            let mut out = Vec::new();
            for (i, line) in trimmed.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(line.trim()) {
                    Ok(v) => out.push(v),
                    Err(e) => {
                        return Err(if out.is_empty() {
                            format!("invalid JSON: {whole}")
                        } else {
                            format!("invalid JSON on line {}: {e}", i + 1)
                        })
                    }
                }
            }
            if out.is_empty() {
                return Err(format!("invalid JSON: {whole}"));
            }
            out
        }
    };
    if records.len() > MAX_RECORDS {
        return Err(format!(
            "dataset has {} records; the maximum is {MAX_RECORDS}",
            records.len()
        ));
    }
    Ok(records)
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

// ------------------------------------------------------------------ entry point

/// Run a faceted search over `data`.
///
/// * `query` — space-separated terms; every term must appear in the record (blank = all).
/// * `search_fields` — comma-separated paths to search (blank = every value).
/// * `filters` — the filter expression (grammar above; blank = no filtering).
/// * `facets` — comma-separated facet field paths (blank = auto-detect).
/// * `facet_limit` — values kept per facet; 0 = unlimited.
/// * `facet_sort` — `count_desc` | `count_asc` | `value_asc` | `value_desc`.
/// * `facet_mode` — `conjunctive` | `disjunctive`.
/// * `facet_stats` — add min/max/avg/sum for numeric facets.
/// * `sort` — `field`, `field:desc`, or a comma-separated list (blank = input order).
/// * `page` / `per_page` — 1-based pagination.
/// * `output` — `json` | `items` | `facets` | `summary`.
#[allow(clippy::too_many_arguments)]
pub fn search(
    data: &str,
    query: &str,
    search_fields: &str,
    filters: &str,
    facets: &str,
    facet_limit: usize,
    facet_sort: &str,
    facet_mode: &str,
    facet_stats: bool,
    sort: &str,
    page: usize,
    per_page: usize,
    output: &str,
) -> Result<String, String> {
    let facet_sort = if facet_sort.trim().is_empty() {
        "count_desc"
    } else {
        facet_sort.trim()
    };
    if !matches!(
        facet_sort,
        "count_desc" | "count_asc" | "value_asc" | "value_desc"
    ) {
        return Err(format!(
            "facet_sort must be count_desc, count_asc, value_asc or value_desc, got '{facet_sort}'"
        ));
    }
    let facet_mode = if facet_mode.trim().is_empty() {
        "conjunctive"
    } else {
        facet_mode.trim()
    };
    if !matches!(facet_mode, "conjunctive" | "disjunctive") {
        return Err(format!(
            "facet_mode must be conjunctive or disjunctive, got '{facet_mode}'"
        ));
    }
    let output = if output.trim().is_empty() {
        "json"
    } else {
        output.trim()
    };
    if !matches!(output, "json" | "items" | "facets" | "summary") {
        return Err(format!(
            "output must be json, items, facets or summary, got '{output}'"
        ));
    }
    if page < 1 {
        return Err(format!("page is 1-based; got {page}"));
    }
    if per_page < 1 || per_page > MAX_PER_PAGE {
        return Err(format!(
            "per_page must be between 1 and {MAX_PER_PAGE}, got {per_page}"
        ));
    }
    if facet_limit > MAX_FACET_LIMIT {
        return Err(format!(
            "facet_limit must be between 0 (unlimited) and {MAX_FACET_LIMIT}, got {facet_limit}"
        ));
    }

    let records = parse_dataset(data)?;
    let expr = parse_filter(filters)?;
    let terms: Vec<String> = query
        .split_whitespace()
        .map(|t| t.to_lowercase())
        .collect();
    let qfields = split_list(search_fields);

    // Records surviving the text query — the common base for both facet modes.
    let queried: Vec<&Value> = records
        .iter()
        .filter(|r| terms.is_empty() || query_matches(r, &terms, &qfields))
        .collect();
    let matched: Vec<&Value> = match &expr {
        None => queried.clone(),
        Some(e) => queried.iter().copied().filter(|r| eval(e, r)).collect(),
    };

    // ---- facets
    let facet_fields = if facets.trim().is_empty() {
        auto_facets(&records)
    } else {
        split_list(facets)
    };
    let mut facet_out: Vec<Value> = Vec::new();
    for field in &facet_fields {
        let scope: Vec<&Value> = if facet_mode == "disjunctive" {
            match &expr {
                None => matched.clone(),
                Some(e) => {
                    let pruned = neutralize(e, field);
                    queried
                        .iter()
                        .copied()
                        .filter(|r| eval(&pruned, r))
                        .collect()
                }
            }
        } else {
            matched.clone()
        };
        let mut acc = FacetAcc::new();
        for rec in &scope {
            for v in flatten(resolve(rec, field)) {
                if let Some(key) = scalar_text(v) {
                    acc.add(key, v.as_f64());
                }
            }
        }
        let mut values: Vec<(String, usize)> = acc
            .order
            .iter()
            .map(|k| (k.clone(), acc.counts[k]))
            .collect();
        match facet_sort {
            "count_desc" => values.sort_by(|a, b| b.1.cmp(&a.1).then(cmp_facet_value(&a.0, &b.0))),
            "count_asc" => values.sort_by(|a, b| a.1.cmp(&b.1).then(cmp_facet_value(&a.0, &b.0))),
            "value_asc" => values.sort_by(|a, b| cmp_facet_value(&a.0, &b.0)),
            _ => values.sort_by(|a, b| cmp_facet_value(&b.0, &a.0)),
        }
        let distinct = values.len();
        if facet_limit > 0 && values.len() > facet_limit {
            values.truncate(facet_limit);
        }
        let mut obj = Map::new();
        obj.insert("field".into(), json!(field));
        obj.insert("distinct".into(), json!(distinct));
        obj.insert(
            "values".into(),
            Value::Array(
                values
                    .iter()
                    .map(|(v, c)| json!({ "value": v, "count": c }))
                    .collect(),
            ),
        );
        if facet_stats && !acc.nums.is_empty() {
            let sum: f64 = acc.nums.iter().sum();
            let min = acc.nums.iter().cloned().fold(f64::INFINITY, f64::min);
            let max = acc.nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
            obj.insert(
                "stats".into(),
                json!({
                    "count": acc.nums.len(),
                    "min": min,
                    "max": max,
                    "avg": sum / acc.nums.len() as f64,
                    "sum": sum,
                }),
            );
        }
        facet_out.push(Value::Object(obj));
    }

    // ---- sort + paginate
    let mut items: Vec<&Value> = matched.clone();
    let keys = parse_sort(sort)?;
    if !keys.is_empty() {
        items.sort_by(|a, b| {
            for k in &keys {
                let av = resolve(a, &k.path).into_iter().next();
                let bv = resolve(b, &k.path).into_iter().next();
                let ord = cmp_sort_values(av, bv);
                let ord = if k.desc { ord.reverse() } else { ord };
                if ord != std::cmp::Ordering::Equal {
                    return ord;
                }
            }
            std::cmp::Ordering::Equal
        });
    }
    let total = items.len();
    let total_pages = total.div_ceil(per_page);
    let start = (page - 1).saturating_mul(per_page);
    let page_items: Vec<Value> = if start >= total {
        Vec::new()
    } else {
        items[start..(start + per_page).min(total)]
            .iter()
            .map(|v| (*v).clone())
            .collect()
    };

    // ---- render
    let render = |v: &Value| serde_json::to_string_pretty(v).unwrap_or_else(|e| e.to_string());
    Ok(match output {
        "items" => render(&Value::Array(page_items)),
        "facets" => render(&Value::Array(facet_out)),
        "summary" => summary(
            total,
            page,
            per_page,
            total_pages,
            page_items.len(),
            &facet_out,
        ),
        _ => {
            let mut env = Map::new();
            env.insert("total".into(), json!(total));
            env.insert("page".into(), json!(page));
            env.insert("per_page".into(), json!(per_page));
            env.insert("total_pages".into(), json!(total_pages));
            env.insert("items".into(), Value::Array(page_items));
            env.insert("facets".into(), Value::Array(facet_out));
            render(&Value::Object(env))
        }
    })
}

fn summary(
    total: usize,
    page: usize,
    per_page: usize,
    total_pages: usize,
    shown: usize,
    facets: &[Value],
) -> String {
    let mut s = format!(
        "{total} record{} matched — page {page} of {} ({shown} shown, {per_page} per page)\n",
        if total == 1 { "" } else { "s" },
        total_pages.max(1)
    );
    for f in facets {
        let field = f.get("field").and_then(|v| v.as_str()).unwrap_or("");
        let distinct = f.get("distinct").and_then(|v| v.as_u64()).unwrap_or(0);
        let values = f.get("values").and_then(|v| v.as_array());
        s.push_str(&format!(
            "\n{field} ({distinct} distinct value{})\n",
            if distinct == 1 { "" } else { "s" }
        ));
        let Some(values) = values else { continue };
        if values.is_empty() {
            s.push_str("  (no values)\n");
        }
        let width = values
            .iter()
            .filter_map(|v| v.get("value").and_then(|x| x.as_str()))
            .map(|v| v.chars().count())
            .max()
            .unwrap_or(0)
            .min(40);
        for v in values {
            let val = v.get("value").and_then(|x| x.as_str()).unwrap_or("");
            let count = v.get("count").and_then(|x| x.as_u64()).unwrap_or(0);
            s.push_str(&format!("  {val:<width$}  {count}\n"));
        }
        if let Some(st) = f.get("stats") {
            let g = |k: &str| st.get(k).and_then(|x| x.as_f64()).unwrap_or(0.0);
            s.push_str(&format!(
                "  min {} · max {} · avg {} · sum {}\n",
                fmt_num(g("min")),
                fmt_num(g("max")),
                trim_avg(g("avg")),
                fmt_num(g("sum"))
            ));
        }
    }
    s
}

fn trim_avg(n: f64) -> String {
    if n.fract() == 0.0 {
        fmt_num(n)
    } else {
        format!("{:.2}", n)
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = r#"[
      {"name":"Aero Jacket","brand":"Northwind","price":180,"tags":["outdoor","sale"],"stock":4},
      {"name":"Basalt Boots","brand":"Northwind","price":120,"tags":["outdoor"],"stock":0},
      {"name":"Cirrus Cap","brand":"Trailhead","price":25,"tags":["sale"],"stock":12},
      {"name":"Delta Pack","brand":"Summit","price":95,"tags":["outdoor","travel"],"stock":7}
    ]"#;

    fn run(filters: &str, facets: &str, output: &str) -> String {
        search(
            DATA, "", "", filters, facets, 10, "count_desc", "conjunctive", false, "", 1, 10,
            output,
        )
        .unwrap()
    }

    #[test]
    fn counts_facet_values_over_the_whole_dataset() {
        let v: Value = serde_json::from_str(&run("", "brand", "json")).unwrap();
        assert_eq!(v["total"], 4);
        assert_eq!(v["total_pages"], 1);
        assert_eq!(v["facets"][0]["field"], "brand");
        assert_eq!(v["facets"][0]["distinct"], 3);
        assert_eq!(v["facets"][0]["values"][0]["value"], "Northwind");
        assert_eq!(v["facets"][0]["values"][0]["count"], 2);
        // Ties break alphabetically for a stable result.
        assert_eq!(v["facets"][0]["values"][1]["value"], "Summit");
    }

    #[test]
    fn rejects_a_malformed_dataset() {
        let e = search(
            "{oops", "", "", "", "", 10, "count_desc", "conjunctive", false, "", 1, 10, "json",
        )
        .unwrap_err();
        assert!(e.starts_with("invalid JSON:"), "{e}");
    }

    #[test]
    fn empty_data_explains_what_is_expected() {
        let e = search(
            "  ", "", "", "", "", 10, "count_desc", "conjunctive", false, "", 1, 10, "json",
        )
        .unwrap_err();
        assert!(e.contains("paste a JSON array of records"), "{e}");
    }

    #[test]
    fn filters_records_and_recounts_facets() {
        let v: Value = serde_json::from_str(&run("price > 100", "brand", "json")).unwrap();
        assert_eq!(v["total"], 2);
        assert_eq!(v["facets"][0]["distinct"], 1);
        assert_eq!(v["facets"][0]["values"][0]["count"], 2);
    }

    #[test]
    fn array_fields_count_every_element() {
        let v: Value = serde_json::from_str(&run("", "tags", "json")).unwrap();
        let vals = v["facets"][0]["values"].as_array().unwrap();
        assert_eq!(vals[0]["value"], "outdoor");
        assert_eq!(vals[0]["count"], 3);
        assert_eq!(v["facets"][0]["distinct"], 3); // outdoor, sale, travel
    }

    #[test]
    fn in_operator_selects_multiple_values() {
        let v: Value =
            serde_json::from_str(&run("brand in [Northwind, Summit]", "brand", "json")).unwrap();
        assert_eq!(v["total"], 3);
    }

    #[test]
    fn in_list_accepts_unbracketed_and_piped_forms() {
        for expr in [
            "brand in Northwind, Summit",
            "brand in Northwind|Summit",
            "brand in [Northwind,Summit]",
        ] {
            let v: Value = serde_json::from_str(&run(expr, "brand", "json")).unwrap();
            assert_eq!(v["total"], 3, "{expr}");
        }
    }

    #[test]
    fn disjunctive_mode_keeps_unselected_values_countable() {
        let conj: Value =
            serde_json::from_str(&run("brand == Northwind", "brand", "json")).unwrap();
        assert_eq!(conj["facets"][0]["distinct"], 1);

        let disj = search(
            DATA,
            "",
            "",
            "brand == Northwind",
            "brand",
            10,
            "count_desc",
            "disjunctive",
            false,
            "",
            1,
            10,
            "json",
        )
        .unwrap();
        let d: Value = serde_json::from_str(&disj).unwrap();
        // Items still respect the filter, but the facet shows all three brands.
        assert_eq!(d["total"], 2);
        assert_eq!(d["facets"][0]["distinct"], 3);
    }

    #[test]
    fn text_query_matches_any_value_and_all_terms() {
        let out = search(
            DATA,
            "cap trailhead",
            "",
            "",
            "brand",
            10,
            "count_desc",
            "conjunctive",
            false,
            "",
            1,
            10,
            "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total"], 1);
        assert_eq!(v["items"][0]["name"], "Cirrus Cap");
    }

    #[test]
    fn search_fields_narrows_the_haystack() {
        let out = search(
            DATA,
            "Northwind",
            "name",
            "",
            "",
            10,
            "count_desc",
            "conjunctive",
            false,
            "",
            1,
            10,
            "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total"], 0);
    }

    #[test]
    fn sorts_and_paginates() {
        let out = search(
            DATA,
            "",
            "",
            "",
            "brand",
            10,
            "count_desc",
            "conjunctive",
            false,
            "price:desc",
            2,
            2,
            "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total"], 4);
        assert_eq!(v["total_pages"], 2);
        assert_eq!(v["items"][0]["price"], 95);
        assert_eq!(v["items"][1]["price"], 25);
    }

    #[test]
    fn page_past_the_end_is_empty_not_an_error() {
        let out = search(
            DATA, "", "", "", "brand", 10, "count_desc", "conjunctive", false, "", 9, 10, "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["items"].as_array().unwrap().len(), 0);
        assert_eq!(v["total"], 4);
    }

    #[test]
    fn auto_detects_facet_fields_and_preserves_key_order() {
        let v: Value = serde_json::from_str(&run("", "", "json")).unwrap();
        let fields: Vec<&str> = v["facets"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["field"].as_str().unwrap())
            .collect();
        assert_eq!(fields, ["name", "brand", "price", "tags", "stock"]);
        // Record key order survives the round-trip (serde_json preserve_order).
        let keys: Vec<&str> = v["items"][0].as_object().unwrap().keys().map(|k| k.as_str()).collect();
        assert_eq!(keys, ["name", "brand", "price", "tags", "stock"]);
    }

    #[test]
    fn facet_limit_truncates_but_reports_distinct() {
        let out = search(
            DATA, "", "", "", "name", 2, "value_asc", "conjunctive", false, "", 1, 10, "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["facets"][0]["distinct"], 4);
        assert_eq!(v["facets"][0]["values"].as_array().unwrap().len(), 2);
        assert_eq!(v["facets"][0]["values"][0]["value"], "Aero Jacket");
    }

    #[test]
    fn facet_values_sort_numerically_not_lexically() {
        let out = search(
            DATA, "", "", "", "price", 0, "value_asc", "conjunctive", false, "", 1, 10, "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["facets"][0]["values"][0]["value"], "25");
        assert_eq!(v["facets"][0]["values"][3]["value"], "180");
    }

    #[test]
    fn facet_stats_report_numeric_aggregates() {
        let out = search(
            DATA, "", "", "", "price", 10, "count_desc", "conjunctive", true, "", 1, 10, "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["facets"][0]["stats"]["min"], 25.0);
        assert_eq!(v["facets"][0]["stats"]["max"], 180.0);
        assert_eq!(v["facets"][0]["stats"]["sum"], 420.0);
        assert_eq!(v["facets"][0]["stats"]["avg"], 105.0);
    }

    #[test]
    fn boolean_and_grouping_operators_work() {
        let v: Value = serde_json::from_str(&run(
            "(brand == Trailhead or price < 100) and not tags contains travel",
            "brand",
            "json",
        ))
        .unwrap();
        assert_eq!(v["total"], 1);
        assert_eq!(v["items"][0]["name"], "Cirrus Cap");
    }

    #[test]
    fn bare_path_is_an_exists_and_truthy_test() {
        // stock is 0 for Basalt Boots → falsy, so 3 records remain.
        let v: Value = serde_json::from_str(&run("stock", "brand", "json")).unwrap();
        assert_eq!(v["total"], 3);
    }

    #[test]
    fn regex_and_prefix_operators_work() {
        let v: Value = serde_json::from_str(&run("name ~ ^[AB] and name endswith s", "brand", "json")).unwrap();
        assert_eq!(v["total"], 1);
        assert_eq!(v["items"][0]["name"], "Basalt Boots");
    }

    #[test]
    fn dotted_paths_reach_nested_objects() {
        let data = r#"[{"sku":"a","meta":{"color":"red"}},{"sku":"b","meta":{"color":"blue"}}]"#;
        let out = search(
            data,
            "",
            "",
            "meta.color == red",
            "meta.color",
            10,
            "count_desc",
            "conjunctive",
            false,
            "",
            1,
            10,
            "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total"], 1);
        assert_eq!(v["facets"][0]["values"][0]["value"], "red");
    }

    #[test]
    fn accepts_ndjson_input() {
        let data = "{\"brand\":\"A\"}\n{\"brand\":\"B\"}\n{\"brand\":\"A\"}";
        let out = search(
            data, "", "", "", "brand", 10, "count_desc", "conjunctive", false, "", 1, 10, "json",
        )
        .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total"], 3);
        assert_eq!(v["facets"][0]["values"][0]["count"], 2);
    }

    #[test]
    fn output_modes_render_their_own_shape() {
        let items: Value = serde_json::from_str(&run("", "brand", "items")).unwrap();
        assert!(items.is_array());
        assert_eq!(items[0]["name"], "Aero Jacket");

        let facets: Value = serde_json::from_str(&run("", "brand", "facets")).unwrap();
        assert!(facets.is_array());
        assert_eq!(facets[0]["field"], "brand");

        let text = run("", "brand", "summary");
        assert!(text.starts_with("4 records matched — page 1 of 1"), "{text}");
        assert!(text.contains("brand (3 distinct values)"), "{text}");
        assert!(text.contains("Northwind  2"), "{text}");
    }

    #[test]
    fn rejects_out_of_range_pagination() {
        let e = search(
            DATA, "", "", "", "", 10, "count_desc", "conjunctive", false, "", 1, 1001, "json",
        )
        .unwrap_err();
        assert_eq!(e, "per_page must be between 1 and 1000, got 1001");
    }

    #[test]
    fn rejects_unknown_enum_values() {
        let e = search(
            DATA, "", "", "", "", 10, "loudest", "conjunctive", false, "", 1, 10, "json",
        )
        .unwrap_err();
        assert!(e.starts_with("facet_sort must be"), "{e}");
        let e = search(
            DATA, "", "", "", "", 10, "count_desc", "maybe", false, "", 1, 10, "json",
        )
        .unwrap_err();
        assert!(e.starts_with("facet_mode must be"), "{e}");
        let e = search(
            DATA, "", "", "", "", 10, "count_desc", "conjunctive", false, "", 1, 10, "xml",
        )
        .unwrap_err();
        assert!(e.starts_with("output must be"), "{e}");
    }

    #[test]
    fn reports_a_broken_filter_expression() {
        let e = search(
            DATA,
            "",
            "",
            "price >",
            "",
            10,
            "count_desc",
            "conjunctive",
            false,
            "",
            1,
            10,
            "json",
        )
        .unwrap_err();
        assert!(e.contains("expected a value after the operator on 'price'"), "{e}");
    }

    #[test]
    fn rejects_a_bad_sort_direction() {
        let e = search(
            DATA,
            "",
            "",
            "",
            "",
            10,
            "count_desc",
            "conjunctive",
            false,
            "price:sideways",
            1,
            10,
            "json",
        )
        .unwrap_err();
        assert_eq!(e, "sort direction for 'price' must be asc or desc, got 'sideways'");
    }

    #[test]
    fn enforces_the_record_cap() {
        let mut rows: Vec<String> = Vec::with_capacity(MAX_RECORDS + 1);
        for i in 0..=MAX_RECORDS {
            rows.push(format!("{{\"i\":{i}}}"));
        }
        let data = format!("[{}]", rows.join(","));
        let e = search(
            &data, "", "", "", "i", 1, "count_desc", "conjunctive", false, "", 1, 1, "json",
        )
        .unwrap_err();
        assert_eq!(e, "dataset has 50001 records; the maximum is 50000");
    }
}
