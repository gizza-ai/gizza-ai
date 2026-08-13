//! gizza-ai/mongodb-query-to-sql — translate a MongoDB find filter into SQL.
//!
//! Accepts either a bare filter document (`{ age: { $gte: 21 } }`) or a full shell call
//! (`db.users.find({...}, {...}).sort({...}).limit(10)`), written in relaxed shell syntax
//! (unquoted keys, single quotes, trailing commas, `//` and `/* */` comments, regex literals,
//! `ObjectId()`/`ISODate()`/`NumberLong()` helpers and MongoDB Extended JSON).
//!
//! Output is a SQL boolean expression, a `WHERE` clause, or a full `SELECT` statement in one of
//! four dialects. Everything is deterministic: same input, same SQL.

/// Largest accepted query text, in characters.
pub const MAX_INPUT: usize = 100_000;
/// Maximum nesting depth of the filter document.
pub const MAX_DEPTH: usize = 64;
/// MySQL's documented "all rows from here" row count, used for `OFFSET` without `LIMIT`.
const MYSQL_MAX_ROWS: &str = "18446744073709551615";

// ---------------------------------------------------------------------------------------------
// Values
// ---------------------------------------------------------------------------------------------

/// A parsed MongoDB literal.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    Arr(Vec<Value>),
    /// Field order is preserved so the generated SQL is stable.
    Obj(Vec<(String, Value)>),
    Regex { pattern: String, flags: String },
    Date(String),
    ObjectId(String),
}

impl Value {
    fn type_name(&self) -> &'static str {
        match self {
            Value::Str(_) => "string",
            Value::Num(_) => "number",
            Value::Bool(_) => "boolean",
            Value::Null => "null",
            Value::Arr(_) => "array",
            Value::Obj(_) => "object",
            Value::Regex { .. } => "regular expression",
            Value::Date(_) => "date",
            Value::ObjectId(_) => "ObjectId",
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Dialects
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialect {
    Ansi,
    Postgres,
    Mysql,
    SqlServer,
}

impl Dialect {
    fn parse(s: &str) -> Result<Dialect, String> {
        match s.trim() {
            "" | "ansi" => Ok(Dialect::Ansi),
            "postgres" => Ok(Dialect::Postgres),
            "mysql" => Ok(Dialect::Mysql),
            "sqlserver" => Ok(Dialect::SqlServer),
            other => Err(format!(
                "dialect must be one of ansi, postgres, mysql, sqlserver (got {other:?})"
            )),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Dialect::Ansi => "ansi",
            Dialect::Postgres => "postgres",
            Dialect::Mysql => "mysql",
            Dialect::SqlServer => "sqlserver",
        }
    }
}

/// What SQL type the surrounding comparison wants an extracted JSON value to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    Any,
    Text,
    Numeric,
    Bool,
}

// ---------------------------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------------------------

struct Parser {
    c: Vec<char>,
    i: usize,
}

impl Parser {
    fn new(src: &str) -> Parser {
        Parser {
            c: src.chars().collect(),
            i: 0,
        }
    }

    fn line_col(&self, idx: usize) -> (usize, usize) {
        let mut line = 1usize;
        let mut col = 1usize;
        for ch in self.c.iter().take(idx.min(self.c.len())) {
            if *ch == '\n' {
                line += 1;
                col = 1;
            } else {
                col += 1;
            }
        }
        (line, col)
    }

    fn err<T>(&self, msg: &str) -> Result<T, String> {
        let (line, col) = self.line_col(self.i);
        Err(format!("{msg} at line {line}, column {col}"))
    }

    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.c.get(self.i + n).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.i += 1;
        }
        ch
    }

    /// Skip whitespace plus `//` line and `/* */` block comments.
    fn ws(&mut self) {
        loop {
            while matches!(self.peek(), Some(c) if c.is_whitespace()) {
                self.i += 1;
            }
            match (self.peek(), self.peek_at(1)) {
                (Some('/'), Some('/')) => {
                    while let Some(c) = self.peek() {
                        self.i += 1;
                        if c == '\n' {
                            break;
                        }
                    }
                }
                (Some('/'), Some('*')) => {
                    self.i += 2;
                    while self.i < self.c.len() {
                        if self.peek() == Some('*') && self.peek_at(1) == Some('/') {
                            self.i += 2;
                            break;
                        }
                        self.i += 1;
                    }
                }
                _ => return,
            }
        }
    }

    fn eat(&mut self, ch: char) -> bool {
        self.ws();
        if self.peek() == Some(ch) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, ch: char) -> Result<(), String> {
        if self.eat(ch) {
            Ok(())
        } else {
            let found = match self.peek() {
                Some(c) => format!("{c:?}"),
                None => "end of input".to_string(),
            };
            self.err(&format!("expected {ch:?} but found {found}"))
        }
    }

    fn at_end(&mut self) -> bool {
        self.ws();
        self.i >= self.c.len()
    }

    /// A bare identifier: letters, digits, `_`, `$` (and `.` inside field keys).
    fn ident(&mut self) -> String {
        self.ws();
        let start = self.i;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '$' {
                self.i += 1;
            } else {
                break;
            }
        }
        self.c[start..self.i].iter().collect()
    }

    fn string_lit(&mut self) -> Result<String, String> {
        self.ws();
        let quote = match self.peek() {
            Some(q @ ('"' | '\'' | '`')) => q,
            _ => return self.err("expected a quoted string"),
        };
        self.i += 1;
        let mut out = String::new();
        loop {
            let ch = match self.bump() {
                Some(c) => c,
                None => return self.err("unterminated string"),
            };
            if ch == quote {
                return Ok(out);
            }
            if ch != '\\' {
                out.push(ch);
                continue;
            }
            match self.bump() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some('r') => out.push('\r'),
                Some('b') => out.push('\u{8}'),
                Some('f') => out.push('\u{c}'),
                Some('0') => out.push('\0'),
                Some('u') => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        match self.bump() {
                            Some(c) => hex.push(c),
                            None => return self.err("unterminated \\u escape"),
                        }
                    }
                    match u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32) {
                        Some(c) => out.push(c),
                        None => return self.err(&format!("invalid \\u escape \\u{hex}")),
                    }
                }
                Some(c) => out.push(c),
                None => return self.err("unterminated escape sequence"),
            }
        }
    }

    fn number(&mut self) -> Result<f64, String> {
        self.ws();
        let start = self.i;
        if matches!(self.peek(), Some('-' | '+')) {
            self.i += 1;
        }
        // Hex literals show up in shell snippets (e.g. bit masks).
        if self.peek() == Some('0') && matches!(self.peek_at(1), Some('x' | 'X')) {
            self.i += 2;
            let hstart = self.i;
            while matches!(self.peek(), Some(c) if c.is_ascii_hexdigit()) {
                self.i += 1;
            }
            let hex: String = self.c[hstart..self.i].iter().collect();
            let neg = self.c[start] == '-';
            return match u64::from_str_radix(&hex, 16) {
                Ok(v) if !hex.is_empty() => Ok(if neg { -(v as f64) } else { v as f64 }),
                _ => self.err("invalid hexadecimal number"),
            };
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
            self.i += 1;
        }
        if self.peek() == Some('.') {
            self.i += 1;
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            self.i += 1;
            if matches!(self.peek(), Some('-' | '+')) {
                self.i += 1;
            }
            while matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                self.i += 1;
            }
        }
        let text: String = self.c[start..self.i].iter().collect();
        match text.parse::<f64>() {
            Ok(n) if n.is_finite() => Ok(n),
            _ => {
                self.i = start;
                self.err(&format!("expected a number but found {text:?}"))
            }
        }
    }

    fn regex_lit(&mut self) -> Result<Value, String> {
        self.expect('/')?;
        let mut pattern = String::new();
        let mut in_class = false;
        loop {
            let ch = match self.bump() {
                Some(c) => c,
                None => return self.err("unterminated regular expression"),
            };
            match ch {
                '\\' => {
                    pattern.push('\\');
                    match self.bump() {
                        Some(c) => pattern.push(c),
                        None => return self.err("unterminated regular expression"),
                    }
                }
                '[' => {
                    in_class = true;
                    pattern.push(ch);
                }
                ']' => {
                    in_class = false;
                    pattern.push(ch);
                }
                '/' if !in_class => break,
                _ => pattern.push(ch),
            }
        }
        let mut flags = String::new();
        while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
            flags.push(self.bump().unwrap());
        }
        Ok(Value::Regex { pattern, flags })
    }

    /// A single call argument: `Helper("...")` / `Helper(123)`.
    fn helper_arg(&mut self, name: &str) -> Result<Value, String> {
        self.expect('(')?;
        self.ws();
        let inner = if self.peek() == Some(')') {
            Value::Str(String::new())
        } else {
            self.value(1)?
        };
        self.expect(')')?;
        match name {
            "ObjectId" => match inner {
                Value::Str(s) => Ok(Value::ObjectId(s)),
                other => self.err(&format!(
                    "ObjectId() takes a quoted hex string (got a {})",
                    other.type_name()
                )),
            },
            "ISODate" | "Date" | "Timestamp" => match inner {
                Value::Str(s) => Ok(Value::Date(s)),
                Value::Num(n) => Ok(Value::Num(n)),
                other => self.err(&format!(
                    "{name}() takes a quoted date string (got a {})",
                    other.type_name()
                )),
            },
            "NumberInt" | "NumberLong" | "NumberDecimal" | "NumberDouble" => match inner {
                Value::Num(n) => Ok(Value::Num(n)),
                Value::Str(s) => match s.trim().parse::<f64>() {
                    Ok(n) if n.is_finite() => Ok(Value::Num(n)),
                    _ => self.err(&format!("{name}({s:?}) is not a number")),
                },
                other => self.err(&format!(
                    "{name}() takes a number (got a {})",
                    other.type_name()
                )),
            },
            "UUID" | "BinData" => match inner {
                Value::Str(s) => Ok(Value::Str(s)),
                other => self.err(&format!(
                    "{name}() takes a quoted string (got a {})",
                    other.type_name()
                )),
            },
            _ => self.err(&format!("unsupported helper {name}()")),
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "query is nested deeper than the {MAX_DEPTH}-level limit"
            ));
        }
        self.ws();
        match self.peek() {
            None => self.err("unexpected end of input, expected a value"),
            Some('{') => self.object(depth),
            Some('[') => self.array(depth),
            Some('"' | '\'' | '`') => Ok(Value::Str(self.string_lit()?)),
            Some('/') => self.regex_lit(),
            Some(c) if c.is_ascii_digit() || c == '-' || c == '+' || c == '.' => {
                Ok(Value::Num(self.number()?))
            }
            Some(c) if c.is_alphabetic() || c == '_' || c == '$' => {
                let word = self.ident();
                match word.as_str() {
                    "true" => Ok(Value::Bool(true)),
                    "false" => Ok(Value::Bool(false)),
                    "null" | "undefined" => Ok(Value::Null),
                    "NaN" | "Infinity" => {
                        self.err("NaN and Infinity cannot be represented in SQL")
                    }
                    "new" => {
                        let name = self.ident();
                        self.helper_arg(&name)
                    }
                    other if !other.is_empty() => {
                        self.ws();
                        if self.peek() == Some('(') {
                            self.helper_arg(other)
                        } else {
                            self.err(&format!(
                                "unexpected {other:?} — string values must be quoted"
                            ))
                        }
                    }
                    _ => self.err("expected a value"),
                }
            }
            Some(c) => self.err(&format!("unexpected character {c:?}")),
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.expect('[')?;
        let mut items = Vec::new();
        loop {
            self.ws();
            if self.eat(']') {
                return Ok(Value::Arr(items));
            }
            items.push(self.value(depth + 1)?);
            self.ws();
            if self.eat(',') {
                continue;
            }
            if self.eat(']') {
                return Ok(Value::Arr(items));
            }
            return self.err("expected ',' or ']' in array");
        }
    }

    fn key(&mut self) -> Result<String, String> {
        self.ws();
        match self.peek() {
            Some('"' | '\'' | '`') => self.string_lit(),
            Some(c) if c.is_alphanumeric() || c == '_' || c == '$' => {
                let start = self.i;
                while let Some(c) = self.peek() {
                    if c.is_alphanumeric() || c == '_' || c == '$' || c == '.' || c == '-' {
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                Ok(self.c[start..self.i].iter().collect())
            }
            _ => self.err("expected a field name"),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.expect('{')?;
        let mut fields: Vec<(String, Value)> = Vec::new();
        loop {
            self.ws();
            if self.eat('}') {
                break;
            }
            let k = self.key()?;
            if k.is_empty() {
                return self.err("empty field name");
            }
            self.expect(':')?;
            let v = self.value(depth + 1)?;
            fields.push((k, v));
            self.ws();
            if self.eat(',') {
                continue;
            }
            if self.eat('}') {
                break;
            }
            return self.err("expected ',' or '}' in object");
        }
        Ok(extended_json(fields))
    }
}

/// Collapse MongoDB Extended JSON wrappers into plain values.
fn extended_json(fields: Vec<(String, Value)>) -> Value {
    if fields.len() == 1 {
        let (k, v) = &fields[0];
        match (k.as_str(), v) {
            ("$oid", Value::Str(s)) => return Value::ObjectId(s.clone()),
            ("$date", Value::Str(s)) => return Value::Date(s.clone()),
            ("$date", Value::Num(n)) => return Value::Num(*n),
            (
                "$numberInt" | "$numberLong" | "$numberDouble" | "$numberDecimal",
                Value::Str(s),
            ) => {
                if let Ok(n) = s.trim().parse::<f64>() {
                    if n.is_finite() {
                        return Value::Num(n);
                    }
                }
            }
            ("$numberInt" | "$numberLong" | "$numberDouble" | "$numberDecimal", Value::Num(n)) => {
                return Value::Num(*n)
            }
            ("$regularExpression", Value::Obj(inner)) => {
                let pattern = inner.iter().find(|(k, _)| k == "pattern");
                let options = inner.iter().find(|(k, _)| k == "options");
                if let Some((_, Value::Str(p))) = pattern {
                    let flags = match options {
                        Some((_, Value::Str(o))) => o.clone(),
                        _ => String::new(),
                    };
                    return Value::Regex {
                        pattern: p.clone(),
                        flags,
                    };
                }
            }
            _ => {}
        }
    }
    Value::Obj(fields)
}

// ---------------------------------------------------------------------------------------------
// Query (call chain)
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct ParsedQuery {
    pub collection: Option<String>,
    pub filter: Vec<(String, Value)>,
    pub projection: Option<Vec<(String, Value)>>,
    pub sort: Option<Vec<(String, Value)>>,
    pub limit: Option<f64>,
    pub skip: Option<f64>,
    pub count: bool,
}

fn as_object(v: Value, what: &str) -> Result<Vec<(String, Value)>, String> {
    match v {
        Value::Obj(f) => Ok(f),
        other => Err(format!(
            "{what} must be a document like {{ field: value }} (got a {})",
            other.type_name()
        )),
    }
}

fn whole_arg(v: &Value, method: &str) -> Result<f64, String> {
    match v {
        Value::Num(n) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 => Ok(*n),
        Value::Num(n) => Err(format!(
            ".{method}() takes a whole non-negative number (got {n})"
        )),
        other => Err(format!(
            ".{method}() takes a number (got a {})",
            other.type_name()
        )),
    }
}

/// Parse either a bare filter document or a `db.<collection>.<method>(...)` chain.
pub fn parse_query(input: &str) -> Result<ParsedQuery, String> {
    let mut src = input.trim();
    while src.ends_with(';') {
        src = src[..src.len() - 1].trim_end();
    }
    if src.is_empty() {
        return Err("query is empty — paste a MongoDB filter or a db.collection.find(...) call"
            .to_string());
    }

    let mut q = ParsedQuery::default();
    let mut p = Parser::new(src);
    p.ws();

    if p.peek() == Some('{') || p.peek() == Some('[') {
        let v = p.value(0)?;
        if !p.at_end() {
            return p.err("unexpected trailing input after the filter document");
        }
        q.filter = as_object(v, "the filter")?;
        return Ok(q);
    }

    // db[.<collection segments>].<method>(...)[.<method>(...)]*
    let head = p.ident();
    if head != "db" {
        return Err(format!(
            "expected a filter document starting with '{{' or a call starting with 'db.' (got {head:?})"
        ));
    }
    p.expect('.')?;

    let mut segments: Vec<String> = Vec::new();
    let mut method;
    loop {
        p.ws();
        let name = if matches!(p.peek(), Some('"' | '\'' | '`')) {
            p.string_lit()?
        } else {
            p.ident()
        };
        if name.is_empty() {
            return p.err("expected a collection or method name");
        }
        p.ws();
        match p.peek() {
            Some('(') if name == "getCollection" && segments.is_empty() => {
                p.expect('(')?;
                let arg = p.value(0)?;
                p.expect(')')?;
                match arg {
                    Value::Str(s) => segments.push(s),
                    other => {
                        return Err(format!(
                            "getCollection() takes a quoted collection name (got a {})",
                            other.type_name()
                        ))
                    }
                }
                p.expect('.')?;
            }
            Some('(') => {
                method = name;
                break;
            }
            Some('.') => {
                p.i += 1;
                segments.push(name);
            }
            _ => return p.err("expected '.' or '(' after the collection name"),
        }
    }
    if !segments.is_empty() {
        q.collection = Some(segments.join("."));
    }

    loop {
        // Read this method's arguments.
        p.expect('(')?;
        let mut args: Vec<Value> = Vec::new();
        p.ws();
        if p.peek() != Some(')') {
            loop {
                args.push(p.value(0)?);
                if p.eat(',') {
                    p.ws();
                    if p.peek() == Some(')') {
                        break;
                    }
                    continue;
                }
                break;
            }
        }
        p.expect(')')?;

        match method.as_str() {
            "find" | "findOne" => {
                if let Some(v) = args.first() {
                    q.filter = as_object(v.clone(), "the filter")?;
                }
                if let Some(v) = args.get(1) {
                    q.projection = Some(as_object(v.clone(), "the projection")?);
                }
                if method == "findOne" {
                    q.limit = Some(1.0);
                }
            }
            "count" | "countDocuments" | "estimatedDocumentCount" => {
                if let Some(v) = args.first() {
                    if q.filter.is_empty() {
                        q.filter = as_object(v.clone(), "the filter")?;
                    }
                }
                q.count = true;
            }
            "sort" => {
                let v = args
                    .first()
                    .ok_or_else(|| ".sort() needs a document like { field: -1 }".to_string())?;
                q.sort = Some(as_object(v.clone(), "the sort")?);
            }
            "project" | "projection" => {
                let v = args
                    .first()
                    .ok_or_else(|| ".project() needs a document like { field: 1 }".to_string())?;
                q.projection = Some(as_object(v.clone(), "the projection")?);
            }
            "limit" => {
                let v = args
                    .first()
                    .ok_or_else(|| ".limit() needs a number".to_string())?;
                q.limit = Some(whole_arg(v, "limit")?);
            }
            "skip" => {
                let v = args
                    .first()
                    .ok_or_else(|| ".skip() needs a number".to_string())?;
                q.skip = Some(whole_arg(v, "skip")?);
            }
            // Cursor decorations that do not affect the SQL.
            "pretty" | "toArray" | "hint" | "explain" | "maxTimeMS" | "collation" | "batchSize"
            | "readPref" | "allowDiskUse" | "noCursorTimeout" | "comment" | "forEach" => {}
            "aggregate" => {
                return Err(
                    "aggregation pipelines are not supported — this tool converts find filters; \
                     rewrite the $match stage as a find filter"
                        .to_string(),
                )
            }
            other => {
                return Err(format!(
                    "unsupported collection method .{other}() — supported: find, findOne, count, \
                     countDocuments, sort, skip, limit, project"
                ))
            }
        }

        p.ws();
        if p.i >= p.c.len() {
            break;
        }
        p.expect('.')?;
        method = p.ident();
        if method.is_empty() {
            return p.err("expected a method name after '.'");
        }
        p.ws();
        if p.peek() != Some('(') {
            return p.err(&format!("expected '(' after .{method}"));
        }
    }

    Ok(q)
}

// ---------------------------------------------------------------------------------------------
// SQL expression tree
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Expr {
    Atom(String),
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Not(Box<Expr>),
}

fn prec(e: &Expr) -> u8 {
    match e {
        Expr::Atom(_) => 3,
        Expr::Not(_) => 2,
        Expr::And(_) => 1,
        Expr::Or(_) => 0,
    }
}

fn render(e: &Expr) -> String {
    match e {
        Expr::Atom(s) => s.clone(),
        Expr::Not(inner) => format!("NOT ({})", render(inner)),
        Expr::And(xs) => xs
            .iter()
            .map(|x| wrap(x, 1))
            .collect::<Vec<_>>()
            .join(" AND "),
        Expr::Or(xs) => xs
            .iter()
            .map(|x| wrap(x, 0))
            .collect::<Vec<_>>()
            .join(" OR "),
    }
}

fn wrap(e: &Expr, parent: u8) -> String {
    if prec(e) < parent {
        format!("({})", render(e))
    } else {
        render(e)
    }
}

fn and_all(mut parts: Vec<Expr>) -> Expr {
    if parts.len() == 1 {
        parts.pop().unwrap()
    } else if parts.is_empty() {
        Expr::Atom("1 = 1".to_string())
    } else {
        Expr::And(parts)
    }
}

/// `None` when there is nothing to OR together.
fn or_all_opt(mut parts: Vec<Expr>) -> Option<Expr> {
    match parts.len() {
        0 => None,
        1 => parts.pop(),
        _ => Some(Expr::Or(parts)),
    }
}

fn or_all(parts: Vec<Expr>) -> Expr {
    // An empty $or matches nothing.
    or_all_opt(parts).unwrap_or_else(|| Expr::Atom("1 = 0".to_string()))
}

// ---------------------------------------------------------------------------------------------
// Translation context
// ---------------------------------------------------------------------------------------------

struct Ctx {
    dialect: Dialect,
    json_paths: bool,
    quote: bool,
    rename_id: bool,
}

impl Ctx {
    fn quote_ident(&self, name: &str) -> String {
        if !self.quote {
            return name.to_string();
        }
        match self.dialect {
            Dialect::Ansi | Dialect::Postgres => format!("\"{}\"", name.replace('"', "\"\"")),
            Dialect::Mysql => format!("`{}`", name.replace('`', "``")),
            Dialect::SqlServer => format!("[{}]", name.replace(']', "]]")),
        }
    }

    /// Table names may be schema-qualified; each part is quoted separately.
    fn quote_table(&self, name: &str) -> String {
        name.split('.')
            .map(|part| self.quote_ident(part))
            .collect::<Vec<_>>()
            .join(".")
    }

    fn apply_rename(&self, field: &str) -> String {
        if !self.rename_id {
            return field.to_string();
        }
        if field == "_id" {
            "id".to_string()
        } else if let Some(rest) = field.strip_prefix("_id.") {
            format!("id.{rest}")
        } else {
            field.to_string()
        }
    }

    /// A column reference for `field`, typed for the comparison it feeds.
    fn column(&self, field: &str, want: Want) -> Result<String, String> {
        let field = self.apply_rename(field);
        if field.trim().is_empty() {
            return Err("field names cannot be empty".to_string());
        }
        let parts: Vec<&str> = field.split('.').collect();
        if !self.json_paths || parts.len() == 1 {
            return Ok(self.quote_ident(&field));
        }
        if parts.iter().any(|p| p.is_empty()) {
            return Err(format!(
                "dotted field path {field:?} has an empty segment"
            ));
        }
        let base = self.quote_ident(parts[0]);
        let rest = &parts[1..];
        Ok(match self.dialect {
            Dialect::Postgres => {
                let mut expr = base;
                for (idx, seg) in rest.iter().enumerate() {
                    let last = idx + 1 == rest.len();
                    let arrow = if last { "->>" } else { "->" };
                    if let Ok(n) = seg.parse::<u32>() {
                        expr = format!("{expr}{arrow}{n}");
                    } else {
                        expr = format!("{expr}{arrow}'{}'", seg.replace('\'', "''"));
                    }
                }
                match want {
                    Want::Numeric => format!("({expr})::numeric"),
                    Want::Bool => format!("({expr})::boolean"),
                    _ => expr,
                }
            }
            Dialect::Mysql => format!("JSON_UNQUOTE(JSON_EXTRACT({base}, '{}'))", json_path(rest)),
            Dialect::Ansi | Dialect::SqlServer => {
                format!("JSON_VALUE({base}, '{}')", json_path(rest))
            }
        })
    }

    /// The JSON *container* (not text) for `$size`.
    fn json_container(&self, field: &str) -> Result<String, String> {
        let field = self.apply_rename(field);
        let parts: Vec<&str> = field.split('.').collect();
        if !self.json_paths || parts.len() == 1 {
            return Ok(self.quote_ident(&field));
        }
        let base = self.quote_ident(parts[0]);
        let rest = &parts[1..];
        Ok(match self.dialect {
            Dialect::Postgres => {
                let mut expr = base;
                for seg in rest {
                    if let Ok(n) = seg.parse::<u32>() {
                        expr = format!("{expr}->{n}");
                    } else {
                        expr = format!("{expr}->'{}'", seg.replace('\'', "''"));
                    }
                }
                expr
            }
            _ => format!("JSON_EXTRACT({base}, '{}')", json_path(rest)),
        })
    }

    fn bool_literal(&self, b: bool) -> String {
        match (self.dialect, b) {
            (Dialect::SqlServer, true) => "1".to_string(),
            (Dialect::SqlServer, false) => "0".to_string(),
            (_, true) => "TRUE".to_string(),
            (_, false) => "FALSE".to_string(),
        }
    }

    fn literal(&self, v: &Value) -> Result<String, String> {
        Ok(match v {
            Value::Str(s) => sql_string(s),
            Value::ObjectId(s) => sql_string(s),
            Value::Num(n) => format_number(*n),
            Value::Bool(b) => self.bool_literal(*b),
            Value::Null => "NULL".to_string(),
            Value::Date(s) => match self.dialect {
                Dialect::Ansi | Dialect::Postgres => format!("TIMESTAMP {}", sql_string(s)),
                Dialect::Mysql | Dialect::SqlServer => sql_string(s),
            },
            Value::Arr(_) => {
                return Err("an array cannot be used as a single SQL value — use $in for a list of \
                            values"
                    .to_string())
            }
            Value::Obj(_) => {
                return Err("matching a whole embedded document is not supported — compare its \
                            fields with dotted paths such as \"address.city\""
                    .to_string())
            }
            Value::Regex { .. } => {
                return Err("a regular expression cannot be used as a plain value".to_string())
            }
        })
    }
}

fn json_path(rest: &[&str]) -> String {
    let mut path = String::from("$");
    for seg in rest {
        if let Ok(n) = seg.parse::<u32>() {
            path.push_str(&format!("[{n}]"));
        } else {
            path.push('.');
            path.push_str(&seg.replace('\'', "''"));
        }
    }
    path
}

fn sql_string(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

fn format_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn want_of(v: &Value) -> Want {
    match v {
        Value::Num(_) => Want::Numeric,
        Value::Bool(_) => Want::Bool,
        Value::Null => Want::Any,
        _ => Want::Text,
    }
}

// ---------------------------------------------------------------------------------------------
// Filter translation
// ---------------------------------------------------------------------------------------------

fn translate_filter(fields: &[(String, Value)], ctx: &Ctx) -> Result<Expr, String> {
    let mut parts: Vec<Expr> = Vec::new();
    for (key, value) in fields {
        match key.as_str() {
            "$and" | "$or" | "$nor" => {
                let branches = match value {
                    Value::Arr(items) => items,
                    other => {
                        return Err(format!(
                            "{key} takes an array of filter documents (got a {})",
                            other.type_name()
                        ))
                    }
                };
                let mut children = Vec::new();
                for b in branches {
                    match b {
                        Value::Obj(f) => children.push(translate_filter(f, ctx)?),
                        other => {
                            return Err(format!(
                                "every entry of {key} must be a filter document (got a {})",
                                other.type_name()
                            ))
                        }
                    }
                }
                parts.push(match key.as_str() {
                    "$and" => and_all(children),
                    "$or" => or_all(children),
                    _ => match or_all_opt(children) {
                        // An empty $nor excludes nothing.
                        None => Expr::Atom("1 = 1".to_string()),
                        Some(inner) => Expr::Not(Box::new(inner)),
                    },
                });
            }
            "$expr" | "$where" | "$text" | "$jsonSchema" | "$comment" => {
                return Err(format!(
                    "{key} is not supported — it has no direct SQL equivalent"
                ))
            }
            "$not" => {
                return Err(
                    "$not applies to one field, as in { age: { $not: { $gt: 30 } } } — use $nor at \
                     the top level"
                        .to_string(),
                )
            }
            k if k.starts_with('$') => {
                return Err(format!(
                    "unsupported top-level operator {k} — supported: $and, $or, $nor"
                ))
            }
            field => parts.push(translate_field(field, value, ctx)?),
        }
    }
    Ok(and_all(parts))
}

fn translate_field(field: &str, value: &Value, ctx: &Ctx) -> Result<Expr, String> {
    match value {
        Value::Obj(ops) if ops.iter().any(|(k, _)| k.starts_with('$')) => {
            if !ops.iter().all(|(k, _)| k.starts_with('$')) {
                return Err(format!(
                    "field {field:?} mixes operators and plain keys — a filter value is either all \
                     $operators or a plain value"
                ));
            }
            field_ops(field, ops, ctx)
        }
        Value::Obj(_) => Err(format!(
            "field {field:?} matches a whole embedded document, which is not supported — compare \
             its fields with dotted paths such as \"{field}.name\""
        )),
        Value::Regex { pattern, flags } => regex_condition(field, pattern, flags, ctx),
        Value::Null => Ok(Expr::Atom(format!(
            "{} IS NULL",
            ctx.column(field, Want::Any)?
        ))),
        Value::Arr(_) => Err(format!(
            "field {field:?} matches an exact array, which has no SQL equivalent — use $in to match \
             any of several values"
        )),
        other => Ok(Expr::Atom(format!(
            "{} = {}",
            ctx.column(field, want_of(other))?,
            ctx.literal(other)?
        ))),
    }
}

fn field_ops(field: &str, ops: &[(String, Value)], ctx: &Ctx) -> Result<Expr, String> {
    let has_regex = ops.iter().any(|(k, _)| k == "$regex");
    let options = ops.iter().find(|(k, _)| k == "$options");
    if options.is_some() && !has_regex {
        return Err(format!(
            "field {field:?} uses $options without $regex"
        ));
    }
    let mut parts: Vec<Expr> = Vec::new();
    for (op, val) in ops {
        let expr = match op.as_str() {
            "$options" => continue,
            "$eq" => match val {
                Value::Null => Expr::Atom(format!("{} IS NULL", ctx.column(field, Want::Any)?)),
                Value::Regex { pattern, flags } => regex_condition(field, pattern, flags, ctx)?,
                v => Expr::Atom(format!(
                    "{} = {}",
                    ctx.column(field, want_of(v))?,
                    ctx.literal(v)?
                )),
            },
            "$ne" => match val {
                Value::Null => Expr::Atom(format!("{} IS NOT NULL", ctx.column(field, Want::Any)?)),
                Value::Regex { pattern, flags } => {
                    Expr::Not(Box::new(regex_condition(field, pattern, flags, ctx)?))
                }
                v => Expr::Atom(format!(
                    "{} <> {}",
                    ctx.column(field, want_of(v))?,
                    ctx.literal(v)?
                )),
            },
            "$gt" | "$gte" | "$lt" | "$lte" => {
                let sym = match op.as_str() {
                    "$gt" => ">",
                    "$gte" => ">=",
                    "$lt" => "<",
                    _ => "<=",
                };
                Expr::Atom(format!(
                    "{} {sym} {}",
                    ctx.column(field, want_of(val))?,
                    ctx.literal(val)?
                ))
            }
            "$in" | "$nin" => in_condition(field, op, val, ctx)?,
            "$exists" => match val {
                Value::Bool(true) => {
                    Expr::Atom(format!("{} IS NOT NULL", ctx.column(field, Want::Any)?))
                }
                Value::Bool(false) => {
                    Expr::Atom(format!("{} IS NULL", ctx.column(field, Want::Any)?))
                }
                other => {
                    return Err(format!(
                        "$exists takes true or false (got a {})",
                        other.type_name()
                    ))
                }
            },
            "$regex" => {
                let (pattern, mut flags) = match val {
                    Value::Str(s) => (s.clone(), String::new()),
                    Value::Regex { pattern, flags } => (pattern.clone(), flags.clone()),
                    other => {
                        return Err(format!(
                            "$regex takes a pattern string or /regex/ literal (got a {})",
                            other.type_name()
                        ))
                    }
                };
                if let Some((_, Value::Str(o))) = options {
                    flags.push_str(o);
                }
                regex_condition(field, &pattern, &flags, ctx)?
            }
            "$not" => match val {
                Value::Obj(inner) if inner.iter().all(|(k, _)| k.starts_with('$')) => {
                    Expr::Not(Box::new(field_ops(field, inner, ctx)?))
                }
                Value::Regex { pattern, flags } => {
                    Expr::Not(Box::new(regex_condition(field, pattern, flags, ctx)?))
                }
                other => {
                    return Err(format!(
                        "$not takes an operator document or a /regex/ literal (got a {})",
                        other.type_name()
                    ))
                }
            },
            "$mod" => match val {
                Value::Arr(items) if items.len() == 2 => {
                    let d = match &items[0] {
                        Value::Num(n) if *n != 0.0 => *n,
                        Value::Num(_) => return Err("$mod divisor cannot be zero".to_string()),
                        other => {
                            return Err(format!(
                                "$mod takes [divisor, remainder] numbers (got a {})",
                                other.type_name()
                            ))
                        }
                    };
                    let r = match &items[1] {
                        Value::Num(n) => *n,
                        other => {
                            return Err(format!(
                                "$mod takes [divisor, remainder] numbers (got a {})",
                                other.type_name()
                            ))
                        }
                    };
                    Expr::Atom(format!(
                        "{} % {} = {}",
                        ctx.column(field, Want::Numeric)?,
                        format_number(d),
                        format_number(r)
                    ))
                }
                _ => return Err("$mod takes a two-element array [divisor, remainder]".to_string()),
            },
            "$size" => {
                let n = match val {
                    Value::Num(n) if n.is_finite() && n.fract() == 0.0 && *n >= 0.0 => *n,
                    _ => {
                        return Err("$size takes a whole non-negative number".to_string());
                    }
                };
                let container = ctx.json_container(field)?;
                match ctx.dialect {
                    Dialect::Postgres => Expr::Atom(format!(
                        "jsonb_array_length({container}) = {}",
                        format_number(n)
                    )),
                    Dialect::Mysql => {
                        Expr::Atom(format!("JSON_LENGTH({container}) = {}", format_number(n)))
                    }
                    d => {
                        return Err(format!(
                            "$size has no portable equivalent in the {} dialect — switch to \
                             postgres or mysql, or filter on an array-length column instead",
                            d.name()
                        ))
                    }
                }
            }
            "$all" | "$elemMatch" | "$type" | "$bitsAllSet" | "$bitsAnySet" | "$bitsAllClear"
            | "$bitsAnyClear" | "$near" | "$nearSphere" | "$geoWithin" | "$geoIntersects" => {
                return Err(format!(
                    "{op} depends on how the document is stored relationally, so it is not \
                     translated — rewrite it by hand for your schema"
                ))
            }
            other => {
                return Err(format!(
                    "unsupported operator {other} on field {field:?} — supported: $eq, $ne, $gt, \
                     $gte, $lt, $lte, $in, $nin, $exists, $regex, $not, $mod, $size"
                ))
            }
        };
        parts.push(expr);
    }
    if parts.is_empty() {
        return Err(format!("field {field:?} has no operators"));
    }
    Ok(and_all(parts))
}

fn in_condition(field: &str, op: &str, val: &Value, ctx: &Ctx) -> Result<Expr, String> {
    let items = match val {
        Value::Arr(items) => items,
        other => {
            return Err(format!(
                "{op} takes an array of values (got a {})",
                other.type_name()
            ))
        }
    };
    let negated = op == "$nin";
    if items.is_empty() {
        return Ok(Expr::Atom(
            if negated { "1 = 1" } else { "1 = 0" }.to_string(),
        ));
    }
    if items.iter().any(|v| matches!(v, Value::Regex { .. })) {
        return Err(format!(
            "{op} with a regular expression is not supported — use $or with one $regex per pattern"
        ));
    }
    let has_null = items.iter().any(|v| matches!(v, Value::Null));
    let concrete: Vec<&Value> = items
        .iter()
        .filter(|v| !matches!(v, Value::Null))
        .collect();
    let want = if concrete.iter().all(|v| matches!(v, Value::Num(_))) && !concrete.is_empty() {
        Want::Numeric
    } else {
        Want::Text
    };
    let col_typed = ctx.column(field, want)?;
    let col_any = ctx.column(field, Want::Any)?;

    if concrete.is_empty() {
        return Ok(Expr::Atom(format!(
            "{col_any} IS {}NULL",
            if negated { "NOT " } else { "" }
        )));
    }
    let list = concrete
        .iter()
        .map(|v| ctx.literal(v))
        .collect::<Result<Vec<_>, _>>()?
        .join(", ");
    let base = Expr::Atom(format!(
        "{col_typed} {}IN ({list})",
        if negated { "NOT " } else { "" }
    ));
    if !has_null {
        return Ok(base);
    }
    Ok(if negated {
        Expr::And(vec![
            base,
            Expr::Atom(format!("{col_any} IS NOT NULL")),
        ])
    } else {
        Expr::Or(vec![base, Expr::Atom(format!("{col_any} IS NULL"))])
    })
}

fn regex_condition(field: &str, pattern: &str, flags: &str, ctx: &Ctx) -> Result<Expr, String> {
    let mut ci = false;
    for f in flags.chars() {
        match f {
            'i' => ci = true,
            other => {
                return Err(format!(
                    "regular expression flag '{other}' is not supported — only 'i' \
                     (case-insensitive) can be translated"
                ))
            }
        }
    }
    let col = ctx.column(field, Want::Text)?;
    Ok(Expr::Atom(match ctx.dialect {
        Dialect::Postgres => format!(
            "{col} {} {}",
            if ci { "~*" } else { "~" },
            sql_string(pattern)
        ),
        Dialect::Mysql => format!(
            "REGEXP_LIKE({col}, {}, '{}')",
            sql_string(pattern),
            if ci { "i" } else { "c" }
        ),
        Dialect::Ansi | Dialect::SqlServer => like_from_regex(&col, pattern, ci, ctx.dialect)?,
    }))
}

/// ANSI SQL and SQL Server have no regex operator, so only anchor-plus-literal patterns convert.
fn like_from_regex(col: &str, pattern: &str, ci: bool, dialect: Dialect) -> Result<String, String> {
    let anchored_start = pattern.starts_with('^');
    let anchored_end = pattern.ends_with('$') && !pattern.ends_with("\\$");
    let body = {
        let mut b = pattern;
        if anchored_start {
            b = &b[1..];
        }
        if anchored_end {
            b = &b[..b.len() - 1];
        }
        b
    };
    let mut literal = String::new();
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(esc) if "\\^$.|?*+()[]{}".contains(esc) => literal.push(esc),
                Some(esc) => {
                    return Err(format!(
                        "the {} dialect has no regex operator, and the escape \\{esc} cannot be \
                         expressed as LIKE — switch to the postgres or mysql dialect",
                        dialect.name()
                    ))
                }
                None => return Err("regular expression ends with a dangling backslash".to_string()),
            },
            c if "^$.|?*+()[]{}".contains(c) => {
                return Err(format!(
                    "the {} dialect has no regex operator, so only plain anchored patterns such as \
                     /^abc/ convert to LIKE — {pattern:?} uses {c:?}; switch to the postgres or \
                     mysql dialect",
                    dialect.name()
                ))
            }
            c => literal.push(c),
        }
    }
    // LIKE wildcards inside the literal must be escaped.
    let needs_escape = literal.contains('%') || literal.contains('_') || literal.contains('\\');
    let escaped: String = literal
        .chars()
        .flat_map(|c| {
            if c == '%' || c == '_' || c == '\\' {
                vec!['\\', c]
            } else {
                vec![c]
            }
        })
        .collect();
    let body = match (anchored_start, anchored_end) {
        (true, true) => escaped,
        (true, false) => format!("{escaped}%"),
        (false, true) => format!("%{escaped}"),
        (false, false) => format!("%{escaped}%"),
    };
    let (lhs, rhs) = if ci {
        (
            format!("UPPER({col})"),
            sql_string(&body.to_uppercase()),
        )
    } else {
        (col.to_string(), sql_string(&body))
    };
    Ok(if needs_escape {
        format!("{lhs} LIKE {rhs} ESCAPE '\\'")
    } else {
        format!("{lhs} LIKE {rhs}")
    })
}

// ---------------------------------------------------------------------------------------------
// SELECT assembly
// ---------------------------------------------------------------------------------------------

fn select_list(q: &ParsedQuery, ctx: &Ctx) -> Result<String, String> {
    if q.count {
        return Ok("COUNT(*)".to_string());
    }
    let proj = match &q.projection {
        Some(p) if !p.is_empty() => p,
        _ => return Ok("*".to_string()),
    };
    let mut included: Vec<String> = Vec::new();
    let mut excluded = 0usize;
    for (field, v) in proj {
        if field.starts_with('$') {
            return Err(format!(
                "projection operator {field} is not supported — list plain fields as {{ field: 1 }}"
            ));
        }
        let on = match v {
            Value::Num(n) if *n == 1.0 => true,
            Value::Num(n) if *n == 0.0 => false,
            Value::Bool(b) => *b,
            other => {
                return Err(format!(
                    "projection value for {field:?} must be 1 or 0 (got a {})",
                    other.type_name()
                ))
            }
        };
        if on {
            included.push(ctx.column(field, Want::Any)?);
        } else {
            excluded += 1;
        }
    }
    if included.is_empty() {
        if excluded > 0 {
            return Err(
                "an exclusion-only projection such as { field: 0 } cannot become a SELECT list — \
                 list the columns you want with { field: 1 } instead"
                    .to_string(),
            );
        }
        return Ok("*".to_string());
    }
    Ok(included.join(", "))
}

fn order_by(q: &ParsedQuery, ctx: &Ctx) -> Result<Option<String>, String> {
    let sort = match &q.sort {
        Some(s) if !s.is_empty() => s,
        _ => return Ok(None),
    };
    let mut terms = Vec::new();
    for (field, v) in sort {
        let dir = match v {
            Value::Num(n) if *n > 0.0 => "ASC",
            Value::Num(n) if *n < 0.0 => "DESC",
            other => {
                return Err(format!(
                    "sort direction for {field:?} must be 1 or -1 (got a {})",
                    other.type_name()
                ))
            }
        };
        terms.push(format!("{} {dir}", ctx.column(field, Want::Any)?));
    }
    Ok(Some(terms.join(", ")))
}

fn build_select(
    q: &ParsedQuery,
    where_clause: Option<String>,
    table: &str,
    ctx: &Ctx,
) -> Result<String, String> {
    let mut lines = vec![format!("SELECT {}", select_list(q, ctx)?)];
    lines.push(format!("FROM {}", ctx.quote_table(table)));
    if let Some(w) = where_clause {
        lines.push(format!("WHERE {w}"));
    }
    let order = order_by(q, ctx)?;
    let (limit, skip) = if q.count {
        (None, None)
    } else {
        (q.limit, q.skip)
    };

    if ctx.dialect == Dialect::SqlServer {
        if skip.is_some() {
            let order = order.ok_or_else(|| {
                "SQL Server needs an ORDER BY before OFFSET — add .sort({ field: 1 }) to the query"
                    .to_string()
            })?;
            lines.push(format!("ORDER BY {order}"));
            let mut tail = format!("OFFSET {} ROWS", format_number(skip.unwrap()));
            if let Some(l) = limit {
                tail.push_str(&format!(" FETCH NEXT {} ROWS ONLY", format_number(l)));
            }
            lines.push(tail);
        } else {
            if let Some(order) = order {
                lines.push(format!("ORDER BY {order}"));
            }
            if let Some(l) = limit {
                lines[0] = format!("SELECT TOP ({}) {}", format_number(l), select_list(q, ctx)?);
            }
        }
        return Ok(format!("{};", lines.join("\n")));
    }

    if let Some(order) = order {
        lines.push(format!("ORDER BY {order}"));
    }
    match (limit, skip) {
        (Some(l), Some(s)) => lines.push(format!(
            "LIMIT {} OFFSET {}",
            format_number(l),
            format_number(s)
        )),
        (Some(l), None) => lines.push(format!("LIMIT {}", format_number(l))),
        (None, Some(s)) => {
            if ctx.dialect == Dialect::Mysql {
                lines.push(format!("LIMIT {MYSQL_MAX_ROWS} OFFSET {}", format_number(s)));
            } else {
                lines.push(format!("OFFSET {}", format_number(s)));
            }
        }
        (None, None) => {}
    }
    Ok(format!("{};", lines.join("\n")))
}

// ---------------------------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------------------------

/// Translate `query` into SQL.
///
/// * `output` — `where` (default), `condition`, or `select`
/// * `dialect` — `ansi` (default), `postgres`, `mysql`, or `sqlserver`
/// * `table` — table name for `select` output; falls back to the collection in the query
/// * `nested` — `column` (default: treat `a.b` as one column name) or `json` (extract a JSON path)
#[allow(clippy::too_many_arguments)]
pub fn run(
    query: &str,
    output: &str,
    dialect: &str,
    table: &str,
    nested: &str,
    quote_identifiers: bool,
    rename_id: bool,
) -> Result<String, String> {
    if query.chars().count() > MAX_INPUT {
        return Err(format!(
            "query is {} characters, which exceeds the maximum of {MAX_INPUT}",
            query.chars().count()
        ));
    }
    let json_paths = match nested.trim() {
        "" | "column" => false,
        "json" => true,
        other => {
            return Err(format!(
                "nested must be one of column, json (got {other:?})"
            ))
        }
    };
    let ctx = Ctx {
        dialect: Dialect::parse(dialect)?,
        json_paths,
        quote: quote_identifiers,
        rename_id,
    };

    let q = parse_query(query)?;
    let has_filter = !q.filter.is_empty();
    let condition = render(&translate_filter(&q.filter, &ctx)?);

    match output.trim() {
        "condition" => Ok(condition),
        "" | "where" => Ok(format!("WHERE {condition}")),
        "select" => {
            let table_name = if !table.trim().is_empty() {
                table.trim().to_string()
            } else {
                q.collection.clone().ok_or_else(|| {
                    "no collection in the query — paste a db.<collection>.find(...) call or fill \
                     in the Table name option"
                        .to_string()
                })?
            };
            let where_clause = if has_filter { Some(condition) } else { None };
            build_select(&q, where_clause, &table_name, &ctx)
        }
        other => Err(format!(
            "output must be one of where, condition, select (got {other:?})"
        )),
    }
}

// ---------------------------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn w(q: &str) -> String {
        run(q, "where", "ansi", "", "column", true, false).unwrap()
    }
    fn cond(q: &str) -> String {
        run(q, "condition", "ansi", "", "column", true, false).unwrap()
    }
    fn err(q: &str) -> String {
        run(q, "where", "ansi", "", "column", true, false).unwrap_err()
    }

    #[test]
    fn translates_a_basic_find_call() {
        assert_eq!(
            w("db.users.find({ age: { $gte: 21 }, status: \"active\" })"),
            "WHERE \"age\" >= 21 AND \"status\" = 'active'"
        );
    }

    #[test]
    fn rejects_an_unknown_operator_with_a_helpful_message() {
        let msg = err("{ age: { $wat: 3 } }");
        assert!(msg.contains("unsupported operator $wat"), "{msg}");
        assert!(msg.contains("$gte"), "{msg}");
    }

    #[test]
    fn accepts_a_bare_filter_document() {
        assert_eq!(cond("{ name: 'ada' }"), "\"name\" = 'ada'");
    }

    #[test]
    fn accepts_relaxed_shell_syntax() {
        assert_eq!(
            cond("{ /* note */ name: 'a\\'b', n: 0x10, ok: true, } // trailing"),
            "\"name\" = 'a''b' AND \"n\" = 16 AND \"ok\" = TRUE"
        );
    }

    #[test]
    fn empty_filter_is_always_true() {
        assert_eq!(w("db.users.find()"), "WHERE 1 = 1");
        assert_eq!(cond("{}"), "1 = 1");
    }

    #[test]
    fn comparison_operators_map_to_sql() {
        assert_eq!(
            cond("{ a: { $gt: 1 }, b: { $lt: 2 }, c: { $lte: 3 }, d: { $ne: 4 } }"),
            "\"a\" > 1 AND \"b\" < 2 AND \"c\" <= 3 AND \"d\" <> 4"
        );
    }

    #[test]
    fn in_and_nin_become_in_lists() {
        assert_eq!(
            cond("{ s: { $in: ['a', 'b'] }, t: { $nin: [1, 2] } }"),
            "\"s\" IN ('a', 'b') AND \"t\" NOT IN (1, 2)"
        );
        assert_eq!(cond("{ s: { $in: [] } }"), "1 = 0");
        assert_eq!(cond("{ s: { $nin: [] } }"), "1 = 1");
    }

    #[test]
    fn in_with_null_also_tests_is_null() {
        assert_eq!(
            cond("{ s: { $in: ['a', null] } }"),
            "\"s\" IN ('a') OR \"s\" IS NULL"
        );
        assert_eq!(
            cond("{ s: { $nin: ['a', null] } }"),
            "\"s\" NOT IN ('a') AND \"s\" IS NOT NULL"
        );
    }

    #[test]
    fn null_and_exists_use_is_null() {
        assert_eq!(cond("{ a: null }"), "\"a\" IS NULL");
        assert_eq!(cond("{ a: { $ne: null } }"), "\"a\" IS NOT NULL");
        assert_eq!(cond("{ a: { $exists: true } }"), "\"a\" IS NOT NULL");
        assert_eq!(cond("{ a: { $exists: false } }"), "\"a\" IS NULL");
    }

    #[test]
    fn or_inside_and_is_parenthesised() {
        assert_eq!(
            cond("{ active: true, $or: [ { a: 1 }, { b: 2 } ] }"),
            "\"active\" = TRUE AND (\"a\" = 1 OR \"b\" = 2)"
        );
    }

    #[test]
    fn nor_and_not_become_sql_not() {
        assert_eq!(
            cond("{ $nor: [ { a: 1 }, { b: 2 } ] }"),
            "NOT (\"a\" = 1 OR \"b\" = 2)"
        );
        assert_eq!(cond("{ a: { $not: { $gt: 5 } } }"), "NOT (\"a\" > 5)");
    }

    #[test]
    fn nested_boolean_groups_keep_their_shape() {
        assert_eq!(
            cond("{ $and: [ { $or: [ { a: 1 }, { b: 2 } ] }, { c: 3 } ] }"),
            "(\"a\" = 1 OR \"b\" = 2) AND \"c\" = 3"
        );
    }

    #[test]
    fn mod_and_size_map_per_dialect() {
        assert_eq!(cond("{ n: { $mod: [4, 1] } }"), "\"n\" % 4 = 1");
        assert_eq!(
            run("{ tags: { $size: 3 } }", "condition", "postgres", "", "column", true, false)
                .unwrap(),
            "jsonb_array_length(\"tags\") = 3"
        );
        assert_eq!(
            run("{ tags: { $size: 3 } }", "condition", "mysql", "", "column", true, false).unwrap(),
            "JSON_LENGTH(`tags`) = 3"
        );
        let msg =
            run("{ tags: { $size: 3 } }", "condition", "ansi", "", "column", true, false).unwrap_err();
        assert!(msg.contains("$size has no portable equivalent"), "{msg}");
    }

    #[test]
    fn regex_maps_per_dialect() {
        assert_eq!(
            run("{ name: /^ada/i }", "condition", "postgres", "", "column", true, false).unwrap(),
            "\"name\" ~* '^ada'"
        );
        assert_eq!(
            run("{ name: /^ada/ }", "condition", "mysql", "", "column", true, false).unwrap(),
            "REGEXP_LIKE(`name`, '^ada', 'c')"
        );
        assert_eq!(
            run("{ name: /^ada/ }", "condition", "ansi", "", "column", true, false).unwrap(),
            "\"name\" LIKE 'ada%'"
        );
        assert_eq!(
            run("{ name: /ada$/i }", "condition", "sqlserver", "", "column", true, false).unwrap(),
            "UPPER([name]) LIKE '%ADA'"
        );
        assert_eq!(
            run(
                "{ name: { $regex: 'ada', $options: 'i' } }",
                "condition",
                "postgres",
                "",
                "column",
                true,
                false
            )
            .unwrap(),
            "\"name\" ~* 'ada'"
        );
    }

    #[test]
    fn regex_like_escapes_wildcards_and_rejects_metacharacters() {
        assert_eq!(
            cond("{ code: /^a_b/ }"),
            "\"code\" LIKE 'a\\_b%' ESCAPE '\\'"
        );
        let msg = err("{ name: /^a.*b/ }");
        assert!(msg.contains("no regex operator"), "{msg}");
        let msg = err("{ name: /ada/m }");
        assert!(msg.contains("flag 'm' is not supported"), "{msg}");
    }

    #[test]
    fn dotted_paths_stay_columns_by_default_and_extract_in_json_mode() {
        assert_eq!(cond("{ 'a.b': 1 }"), "\"a.b\" = 1");
        assert_eq!(
            run("{ 'a.b': 1 }", "condition", "postgres", "", "json", true, false).unwrap(),
            "(\"a\"->>'b')::numeric = 1"
        );
        assert_eq!(
            run("{ 'a.b': 'x' }", "condition", "postgres", "", "json", true, false).unwrap(),
            "\"a\"->>'b' = 'x'"
        );
        assert_eq!(
            run("{ 'a.0.b': 'x' }", "condition", "postgres", "", "json", true, false).unwrap(),
            "\"a\"->0->>'b' = 'x'"
        );
        assert_eq!(
            run("{ 'a.b': 'x' }", "condition", "mysql", "", "json", true, false).unwrap(),
            "JSON_UNQUOTE(JSON_EXTRACT(`a`, '$.b')) = 'x'"
        );
        assert_eq!(
            run("{ 'a.b': 'x' }", "condition", "sqlserver", "", "json", true, false).unwrap(),
            "JSON_VALUE([a], '$.b') = 'x'"
        );
    }

    #[test]
    fn quoting_and_id_rename_are_switchable() {
        assert_eq!(
            run("{ _id: 'abc' }", "condition", "ansi", "", "column", false, true).unwrap(),
            "id = 'abc'"
        );
        assert_eq!(
            run("{ _id: ObjectId('64b1') }", "condition", "mysql", "", "column", true, false)
                .unwrap(),
            "`_id` = '64b1'"
        );
    }

    #[test]
    fn extended_json_and_shell_helpers_are_understood() {
        assert_eq!(
            cond("{ _id: { $oid: '64b1' }, n: { $numberLong: '42' } }"),
            "\"_id\" = '64b1' AND \"n\" = 42"
        );
        assert_eq!(
            cond("{ at: ISODate('2026-01-02T03:04:05Z') }"),
            "\"at\" = TIMESTAMP '2026-01-02T03:04:05Z'"
        );
        assert_eq!(
            run("{ at: new Date('2026-01-02') }", "condition", "mysql", "", "column", true, false)
                .unwrap(),
            "`at` = '2026-01-02'"
        );
        assert_eq!(
            cond("{ name: { $regularExpression: { pattern: '^a', options: 'i' } } }"),
            "UPPER(\"name\") LIKE 'A%'"
        );
    }

    #[test]
    fn select_output_uses_projection_sort_and_paging() {
        let sql = run(
            "db.orders.find({ status: { $in: ['paid', 'shipped'] }, total: { $gt: 100 } }, \
             { _id: 0, orderId: 1, total: 1 }).sort({ total: -1 }).limit(10).skip(20)",
            "select",
            "ansi",
            "",
            "column",
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            sql,
            "SELECT \"orderId\", \"total\"\n\
             FROM \"orders\"\n\
             WHERE \"status\" IN ('paid', 'shipped') AND \"total\" > 100\n\
             ORDER BY \"total\" DESC\n\
             LIMIT 10 OFFSET 20;"
        );
    }

    #[test]
    fn select_omits_where_for_an_empty_filter_and_counts() {
        assert_eq!(
            run("db.users.find()", "select", "ansi", "", "column", true, false).unwrap(),
            "SELECT *\nFROM \"users\";"
        );
        assert_eq!(
            run(
                "db.users.countDocuments({ active: true })",
                "select",
                "ansi",
                "",
                "column",
                true,
                false
            )
            .unwrap(),
            "SELECT COUNT(*)\nFROM \"users\"\nWHERE \"active\" = TRUE;"
        );
        assert_eq!(
            run("db.users.findOne({ a: 1 })", "select", "ansi", "", "column", true, false).unwrap(),
            "SELECT *\nFROM \"users\"\nWHERE \"a\" = 1\nLIMIT 1;"
        );
    }

    #[test]
    fn sqlserver_paging_uses_top_and_offset_fetch() {
        assert_eq!(
            run(
                "db.users.find({ a: 1 }).limit(5)",
                "select",
                "sqlserver",
                "",
                "column",
                true,
                false
            )
            .unwrap(),
            "SELECT TOP (5) *\nFROM [users]\nWHERE [a] = 1;"
        );
        assert_eq!(
            run(
                "db.users.find({ a: 1 }).sort({ a: 1 }).skip(10).limit(5)",
                "select",
                "sqlserver",
                "",
                "column",
                true,
                false
            )
            .unwrap(),
            "SELECT *\nFROM [users]\nWHERE [a] = 1\nORDER BY [a] ASC\n\
             OFFSET 10 ROWS FETCH NEXT 5 ROWS ONLY;"
        );
        let msg = run(
            "db.users.find().skip(10)",
            "select",
            "sqlserver",
            "",
            "column",
            true,
            false,
        )
        .unwrap_err();
        assert!(msg.contains("ORDER BY before OFFSET"), "{msg}");
    }

    #[test]
    fn mysql_offset_without_limit_uses_the_max_row_count() {
        assert_eq!(
            run("db.t.find().skip(5)", "select", "mysql", "", "column", true, false).unwrap(),
            format!("SELECT *\nFROM `t`\nLIMIT {MYSQL_MAX_ROWS} OFFSET 5;")
        );
        assert_eq!(
            run("db.t.find().skip(5)", "select", "postgres", "", "column", true, false).unwrap(),
            "SELECT *\nFROM \"t\"\nOFFSET 5;"
        );
    }

    #[test]
    fn table_option_overrides_and_is_required_without_a_collection() {
        assert_eq!(
            run("{ a: 1 }", "select", "ansi", "sales.orders", "column", true, false).unwrap(),
            "SELECT *\nFROM \"sales\".\"orders\"\nWHERE \"a\" = 1;"
        );
        let msg = run("{ a: 1 }", "select", "ansi", "", "column", true, false).unwrap_err();
        assert!(msg.contains("no collection in the query"), "{msg}");
    }

    #[test]
    fn get_collection_and_dotted_collections_are_supported() {
        assert_eq!(
            run(
                "db.getCollection('my orders').find({ a: 1 })",
                "select",
                "ansi",
                "",
                "column",
                true,
                false
            )
            .unwrap(),
            "SELECT *\nFROM \"my orders\"\nWHERE \"a\" = 1;"
        );
        assert_eq!(
            parse_query("db.audit.events.find({})").unwrap().collection,
            Some("audit.events".to_string())
        );
    }

    #[test]
    fn exclusion_only_projection_is_rejected() {
        let msg = run(
            "db.t.find({}, { secret: 0 })",
            "select",
            "ansi",
            "",
            "column",
            true,
            false,
        )
        .unwrap_err();
        assert!(msg.contains("exclusion-only projection"), "{msg}");
    }

    #[test]
    fn aggregate_and_writes_are_reported_clearly() {
        let msg = err("db.t.aggregate([{ $match: { a: 1 } }])");
        assert!(msg.contains("aggregation pipelines are not supported"), "{msg}");
        let msg = err("db.t.insertOne({ a: 1 })");
        assert!(msg.contains("unsupported collection method .insertOne()"), "{msg}");
        let msg = err("{ a: { $elemMatch: { b: 1 } } }");
        assert!(msg.contains("$elemMatch"), "{msg}");
    }

    #[test]
    fn syntax_errors_report_line_and_column() {
        let msg = err("{ a: 1,\n  b: }");
        assert!(msg.contains("line 2"), "{msg}");
        let msg = err("{ a: 1 b: 2 }");
        assert!(msg.contains("expected ',' or '}'"), "{msg}");
        let msg = err("{ a: bare }");
        assert!(msg.contains("string values must be quoted"), "{msg}");
    }

    #[test]
    fn empty_and_oversized_input_are_rejected() {
        assert!(err("   ").contains("query is empty"));
        let big = format!("{{ a: '{}' }}", "x".repeat(MAX_INPUT));
        let msg = run(&big, "where", "ansi", "", "column", true, false).unwrap_err();
        assert!(msg.contains("exceeds the maximum"), "{msg}");
    }

    #[test]
    fn depth_limit_is_enforced() {
        let deep = format!(
            "{{ a: {}1{} }}",
            "{ $not: { $not: ".repeat(20),
            " } }".repeat(20)
        );
        // Well within the depth cap but structurally invalid; the cap itself is checked below.
        assert!(run(&deep, "where", "ansi", "", "column", true, false).is_err());
        let nested = format!("{}{}", "[".repeat(MAX_DEPTH + 2), "]".repeat(MAX_DEPTH + 2));
        let msg = run(&nested, "where", "ansi", "", "column", true, false).unwrap_err();
        assert!(msg.contains("nested deeper"), "{msg}");
    }

    #[test]
    fn invalid_option_values_are_reported() {
        assert!(run("{}", "nope", "ansi", "", "column", true, false)
            .unwrap_err()
            .contains("output must be one of"));
        assert!(run("{}", "where", "oracle", "", "column", true, false)
            .unwrap_err()
            .contains("dialect must be one of"));
        assert!(run("{}", "where", "ansi", "", "deep", true, false)
            .unwrap_err()
            .contains("nested must be one of"));
    }
}
