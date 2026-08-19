//! gizza-ai/mongo-query core — run a MongoDB-style query document against a pasted
//! JSON array of documents. Pure compute, shared by the chat skill block and the web
//! page; no wafer/wasm-bindgen deps and no host calls.
//!
//! The query language is MongoDB's *query and projection* language (the `find()` filter),
//! not the aggregation pipeline:
//!
//! ```text
//! { "status": "active" }                     equality
//! { "age": { "$gte": 21, "$lt": 65 } }       comparison
//! { "tags": { "$in": ["a", "b"] } }          membership (matches array fields elementwise)
//! { "$or": [ {...}, {...} ] }                logical
//! { "items": { "$elemMatch": { "qty": { "$gt": 5 } } } }
//! ```
//!
//! Semantics follow the MongoDB manual where they are observable on plain JSON:
//! dotted paths traverse objects AND arrays, a predicate on an array field holds when ANY
//! element satisfies it, `{f: null}` also matches documents missing `f`, and `$ne` / `$nin` /
//! `$not` match missing fields too. Comparisons only relate values of the same JSON type;
//! sorting uses MongoDB's cross-type ordering (null < numbers < strings < objects < arrays <
//! booleans).
//!
//! The query is parsed with a relaxed (mongo-shell flavoured) reader: unquoted keys, single
//! quotes, `//` and `/* */` comments, trailing commas, `/pattern/flags` regex literals and the
//! `ObjectId()` / `ISODate()` / `NumberLong()` helpers are all accepted. Strict JSON is a subset.

use regex::Regex;
use serde_json::{Map, Value};
use std::cmp::Ordering;

/// Largest accepted documents input, in bytes.
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// Largest accepted query/projection/sort text, in bytes.
pub const MAX_QUERY_BYTES: usize = 20_000;
/// Largest number of input documents.
pub const MAX_DOCS: usize = 50_000;
/// Deepest nesting accepted inside a query document.
pub const MAX_DEPTH: usize = 64;

// =================================================================================================
// Relaxed (mongo-shell flavoured) JSON reader
// =================================================================================================

struct Reader<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(src: &'a str) -> Self {
        Reader { src: src.as_bytes(), pos: 0 }
    }

    fn err<T>(&self, msg: &str) -> Result<T, String> {
        Err(format!("{msg} (at character {})", self.pos + 1))
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    /// Skip whitespace plus `//` line and `/* */` block comments.
    fn skip_ws(&mut self) -> Result<(), String> {
        loop {
            while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
                self.pos += 1;
            }
            if self.src[self.pos..].starts_with(b"//") {
                self.pos += 2;
                while !matches!(self.peek(), None | Some(b'\n')) {
                    self.pos += 1;
                }
            } else if self.src[self.pos..].starts_with(b"/*") {
                self.pos += 2;
                loop {
                    match self.peek() {
                        None => return self.err("unterminated /* comment"),
                        Some(_) if self.src[self.pos..].starts_with(b"*/") => {
                            self.pos += 2;
                            break;
                        }
                        Some(_) => self.pos += 1,
                    }
                }
            } else {
                return Ok(());
            }
        }
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        self.skip_ws()?;
        if self.peek() == Some(b) {
            self.pos += 1;
            Ok(())
        } else {
            self.err(&format!("expected '{}'", b as char))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return self.err(&format!("query nested deeper than {MAX_DEPTH} levels"));
        }
        self.skip_ws()?;
        match self.peek() {
            None => self.err("unexpected end of query"),
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') | Some(b'\'') => Ok(Value::String(self.string()?)),
            Some(b'/') => self.regex_literal(),
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            Some(c) if c.is_ascii_alphabetic() || c == b'_' || c == b'$' => self.word(depth),
            Some(c) => self.err(&format!("unexpected character '{}'", c as char)),
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        self.expect(b'{')?;
        let mut map = Map::new();
        loop {
            self.skip_ws()?;
            if self.peek() == Some(b'}') {
                self.pos += 1;
                return Ok(Value::Object(map));
            }
            let key = match self.peek() {
                Some(b'"') | Some(b'\'') => self.string()?,
                Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'.' => {
                    self.bare_key()?
                }
                _ => return self.err("expected a field name"),
            };
            self.expect(b':')?;
            let val = self.value(depth + 1)?;
            map.insert(key, val);
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Value::Object(map));
                }
                _ => return self.err("expected ',' or '}'"),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        loop {
            self.skip_ws()?;
            if self.peek() == Some(b']') {
                self.pos += 1;
                return Ok(Value::Array(items));
            }
            items.push(self.value(depth + 1)?);
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Value::Array(items));
                }
                _ => return self.err("expected ',' or ']'"),
            }
        }
    }

    fn bare_key(&mut self) -> Result<String, String> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'$' || c == b'.')
        {
            self.pos += 1;
        }
        Ok(String::from_utf8_lossy(&self.src[start..self.pos]).into_owned())
    }

    fn string(&mut self) -> Result<String, String> {
        let quote = match self.peek() {
            Some(q @ (b'"' | b'\'')) => q,
            _ => return self.err("expected a quoted string"),
        };
        self.pos += 1;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return self.err("unterminated string"),
                Some(c) if c == quote => {
                    self.pos += 1;
                    return Ok(out);
                }
                Some(b'\\') => {
                    self.pos += 1;
                    let esc = match self.peek() {
                        None => return self.err("unterminated escape sequence"),
                        Some(e) => e,
                    };
                    self.pos += 1;
                    match esc {
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'u' => {
                            let hex = self
                                .src
                                .get(self.pos..self.pos + 4)
                                .ok_or_else(|| "truncated \\u escape in string".to_string())?;
                            let code = u32::from_str_radix(&String::from_utf8_lossy(hex), 16)
                                .map_err(|_| "invalid \\u escape in string".to_string())?;
                            self.pos += 4;
                            out.push(char::from_u32(code).unwrap_or('\u{fffd}'));
                        }
                        other => out.push(other as char),
                    }
                }
                Some(_) => {
                    // Copy one whole UTF-8 character.
                    let rest = &self.src[self.pos..];
                    let len = utf8_len(rest[0]);
                    let s = String::from_utf8_lossy(&rest[..len.min(rest.len())]).into_owned();
                    out.push_str(&s);
                    self.pos += len.min(rest.len());
                }
            }
        }
    }

    /// `/pattern/flags` → `{"$regex": pattern, "$options": flags}`.
    fn regex_literal(&mut self) -> Result<Value, String> {
        self.pos += 1; // opening slash
        let mut pat = String::new();
        loop {
            match self.peek() {
                None => return self.err("unterminated /regex/ literal"),
                Some(b'\\') => {
                    pat.push('\\');
                    self.pos += 1;
                    if let Some(c) = self.peek() {
                        pat.push(c as char);
                        self.pos += 1;
                    }
                }
                Some(b'/') => {
                    self.pos += 1;
                    break;
                }
                Some(_) => {
                    let rest = &self.src[self.pos..];
                    let len = utf8_len(rest[0]).min(rest.len());
                    pat.push_str(&String::from_utf8_lossy(&rest[..len]));
                    self.pos += len;
                }
            }
        }
        let mut flags = String::new();
        while matches!(self.peek(), Some(c) if c.is_ascii_alphabetic()) {
            flags.push(self.src[self.pos] as char);
            self.pos += 1;
        }
        let mut map = Map::new();
        map.insert("$regex".into(), Value::String(pat));
        if !flags.is_empty() {
            map.insert("$options".into(), Value::String(flags));
        }
        Ok(Value::Object(map))
    }

    fn number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E' || c == b'+' || c == b'-')
        {
            self.pos += 1;
        }
        let text = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
        serde_json::from_str::<Value>(&text).map_err(|_| format!("invalid number '{text}' in query"))
    }

    /// A bare word: `true` / `false` / `null`, or a shell helper like `ObjectId("…")`.
    fn word(&mut self, depth: usize) -> Result<Value, String> {
        let start = self.pos;
        while matches!(self.peek(), Some(c) if c.is_ascii_alphanumeric() || c == b'_' || c == b'$') {
            self.pos += 1;
        }
        let word = String::from_utf8_lossy(&self.src[start..self.pos]).into_owned();
        match word.as_str() {
            "true" => return Ok(Value::Bool(true)),
            "false" => return Ok(Value::Bool(false)),
            "null" | "undefined" => return Ok(Value::Null),
            "new" => {
                // `new Date("…")` — skip the keyword and read the constructor call.
                self.skip_ws()?;
                return self.value(depth);
            }
            _ => {}
        }
        self.skip_ws()?;
        if self.peek() != Some(b'(') {
            return self
                .err(&format!("unexpected value '{word}' — quote it as \"{word}\" if it is a string"));
        }
        self.pos += 1;
        self.skip_ws()?;
        let arg = if self.peek() == Some(b')') {
            Value::String(String::new())
        } else {
            self.value(depth + 1)?
        };
        self.expect(b')')?;
        match word.as_str() {
            // Identifier-ish helpers keep their string payload.
            "ObjectId" | "ISODate" | "Date" | "UUID" | "BinData" | "Timestamp" => match arg {
                Value::String(s) => Ok(Value::String(s)),
                other => Ok(other),
            },
            // Numeric helpers unwrap to a JSON number.
            "NumberLong" | "NumberInt" | "NumberDecimal" | "NumberDouble" => match arg {
                Value::String(s) => serde_json::from_str::<Value>(s.trim())
                    .map_err(|_| format!("{word}(\"{s}\") is not a number")),
                other => Ok(other),
            },
            other => self.err(&format!(
                "unsupported helper '{other}(…)' in query — supported helpers are ObjectId, ISODate, \
                 Date, UUID, BinData, Timestamp, NumberLong, NumberInt, NumberDecimal"
            )),
        }
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        _ => 4,
    }
}

/// Parse relaxed mongo-shell JSON into a `serde_json::Value`, rejecting trailing junk.
pub fn parse_relaxed(text: &str) -> Result<Value, String> {
    let mut r = Reader::new(text);
    let v = r.value(0)?;
    r.skip_ws()?;
    if r.pos < r.src.len() {
        return r.err("unexpected trailing characters after the value");
    }
    Ok(v)
}

// =================================================================================================
// Compiled query
// =================================================================================================

#[derive(Debug)]
enum Node {
    And(Vec<Node>),
    Or(Vec<Node>),
    Nor(Vec<Node>),
    Field { path: Vec<String>, preds: Vec<Pred> },
    /// Matches every document (an empty query document).
    True,
}

#[derive(Debug)]
enum Pred {
    Eq(Value),
    Ne(Value),
    Gt(Value),
    Gte(Value),
    Lt(Value),
    Lte(Value),
    In(Vec<Member>),
    Nin(Vec<Member>),
    Exists(bool),
    Type(Vec<String>),
    Regex(Regex),
    Mod(f64, f64),
    All(Vec<Value>),
    Size(usize),
    ElemMatch(Box<Node>),
    Not(Vec<Pred>),
}

/// One entry of an `$in` / `$nin` list: a literal value or a regex.
#[derive(Debug)]
enum Member {
    Value(Value),
    Regex(Regex),
}

/// `$where`-class operators that need an engine this sandbox does not have.
const UNSUPPORTED_OPS: &[(&str, &str)] = &[
    ("$where", "needs a JavaScript interpreter"),
    ("$expr", "needs the aggregation-expression evaluator"),
    ("$jsonSchema", "needs a JSON Schema validator"),
    ("$text", "needs a text index"),
    ("$geoWithin", "needs geospatial indexes"),
    ("$geoIntersects", "needs geospatial indexes"),
    ("$near", "needs geospatial indexes"),
    ("$nearSphere", "needs geospatial indexes"),
    ("$bitsAllSet", "operates on BSON binary data"),
    ("$bitsAnySet", "operates on BSON binary data"),
    ("$bitsAllClear", "operates on BSON binary data"),
    ("$bitsAnyClear", "operates on BSON binary data"),
    ("$rand", "is not deterministic"),
    ("$sampleRate", "is not deterministic"),
    ("$comment", "is a server-side hint"),
];

fn unsupported(op: &str) -> Option<String> {
    UNSUPPORTED_OPS.iter().find(|(name, _)| *name == op).map(|(name, why)| {
        format!("operator '{name}' is not supported here — it {why}. Rewrite the query with the supported operators, or see the tool page for the full list.")
    })
}

fn split_path(field: &str) -> Vec<String> {
    field.split('.').map(|s| s.to_string()).collect()
}

fn compile(query: &Value, depth: usize) -> Result<Node, String> {
    if depth > MAX_DEPTH {
        return Err(format!("query nested deeper than {MAX_DEPTH} levels"));
    }
    let map = match query {
        Value::Object(m) => m,
        other => {
            return Err(format!(
                "a query must be an object like {{\"age\": {{\"$gt\": 21}}}}, got {}",
                kind_of(other)
            ))
        }
    };
    let mut nodes: Vec<Node> = Vec::new();
    for (key, val) in map {
        if let Some(msg) = unsupported(key.as_str()) {
            return Err(msg);
        }
        match key.as_str() {
            "$and" | "$or" | "$nor" => {
                let arr = match val {
                    Value::Array(a) if !a.is_empty() => a,
                    Value::Array(_) => return Err(format!("{key} needs a non-empty array of query objects")),
                    other => {
                        return Err(format!("{key} takes an array of query objects, got {}", kind_of(other)))
                    }
                };
                let subs = arr
                    .iter()
                    .map(|q| compile(q, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                nodes.push(match key.as_str() {
                    "$and" => Node::And(subs),
                    "$or" => Node::Or(subs),
                    _ => Node::Nor(subs),
                });
            }
            "$not" => {
                return Err("$not applies to a single field, e.g. {\"age\": {\"$not\": {\"$gt\": 21}}} — use $nor at the top level".into())
            }
            other if other.starts_with('$') => {
                return Err(format!(
                    "unknown top-level operator '{other}' — the top level takes field names, $and, $or or $nor"
                ))
            }
            field => {
                let preds = compile_field(field, val, depth)?;
                nodes.push(Node::Field { path: split_path(field), preds });
            }
        }
    }
    Ok(match nodes.len() {
        0 => Node::True,
        1 => nodes.pop().unwrap(),
        _ => Node::And(nodes),
    })
}

/// Is this object an operator document (`{"$gt": 5}`) rather than a literal to match?
fn is_operator_doc(v: &Value) -> bool {
    matches!(v, Value::Object(m) if m.keys().any(|k| k.starts_with('$')))
}

fn compile_field(field: &str, val: &Value, depth: usize) -> Result<Vec<Pred>, String> {
    if !is_operator_doc(val) {
        return Ok(vec![Pred::Eq(val.clone())]);
    }
    let map = match val {
        Value::Object(m) => m,
        _ => unreachable!(),
    };
    let mut preds = Vec::new();
    let mut regex_pat: Option<String> = None;
    let mut regex_opts: Option<String> = None;
    for (op, arg) in map {
        if !op.starts_with('$') {
            return Err(format!(
                "field '{field}' mixes the operator '{}' with the plain key '{op}' — an operator document may only contain $operators",
                map.keys().find(|k| k.starts_with('$')).map(String::as_str).unwrap_or("$?")
            ));
        }
        if let Some(msg) = unsupported(op.as_str()) {
            return Err(msg);
        }
        match op.as_str() {
            "$eq" => preds.push(Pred::Eq(arg.clone())),
            "$ne" => preds.push(Pred::Ne(arg.clone())),
            "$gt" => preds.push(Pred::Gt(arg.clone())),
            "$gte" => preds.push(Pred::Gte(arg.clone())),
            "$lt" => preds.push(Pred::Lt(arg.clone())),
            "$lte" => preds.push(Pred::Lte(arg.clone())),
            "$in" | "$nin" => {
                let arr = match arg {
                    Value::Array(a) => a,
                    other => {
                        return Err(format!(
                            "{op} on '{field}' takes an array, got {} — write {op}: [\"a\", \"b\"]",
                            kind_of(other)
                        ))
                    }
                };
                let members = arr
                    .iter()
                    .map(|v| {
                        if is_operator_doc(v) {
                            let (p, o) = regex_parts(v);
                            match p {
                                Some(pat) => build_regex(&pat, o.as_deref(), field).map(Member::Regex),
                                None => Err(format!(
                                    "{op} on '{field}' only accepts plain values and /regex/ entries"
                                )),
                            }
                        } else {
                            Ok(Member::Value(v.clone()))
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                preds.push(if op == "$in" { Pred::In(members) } else { Pred::Nin(members) });
            }
            "$exists" => preds.push(Pred::Exists(truthy(arg))),
            "$type" => {
                let names = match arg {
                    Value::Array(a) => a.iter().map(|v| type_alias(v, field)).collect::<Result<Vec<_>, _>>()?,
                    other => vec![type_alias(other, field)?],
                };
                preds.push(Pred::Type(names));
            }
            "$regex" => {
                regex_pat = Some(match arg {
                    Value::String(s) => s.clone(),
                    Value::Object(m) if m.contains_key("$regex") => match m.get("$regex") {
                        Some(Value::String(s)) => s.clone(),
                        _ => return Err(format!("$regex on '{field}' must be a pattern string")),
                    },
                    other => {
                        return Err(format!(
                            "$regex on '{field}' must be a pattern string, got {}",
                            kind_of(other)
                        ))
                    }
                });
            }
            "$options" => {
                regex_opts = Some(match arg {
                    Value::String(s) => s.clone(),
                    other => {
                        return Err(format!(
                            "$options on '{field}' must be a flag string like \"i\", got {}",
                            kind_of(other)
                        ))
                    }
                });
            }
            "$mod" => {
                let arr = match arg {
                    Value::Array(a) if a.len() == 2 => a,
                    _ => {
                        return Err(format!(
                            "$mod on '{field}' takes exactly [divisor, remainder], e.g. $mod: [4, 0]"
                        ))
                    }
                };
                let d = arr[0].as_f64().ok_or_else(|| format!("$mod divisor on '{field}' must be a number"))?;
                let r = arr[1].as_f64().ok_or_else(|| format!("$mod remainder on '{field}' must be a number"))?;
                if d == 0.0 {
                    return Err(format!("$mod divisor on '{field}' must not be zero"));
                }
                preds.push(Pred::Mod(d, r));
            }
            "$all" => {
                let arr = match arg {
                    Value::Array(a) => a.clone(),
                    other => {
                        return Err(format!("$all on '{field}' takes an array, got {}", kind_of(other)))
                    }
                };
                if arr.iter().any(is_operator_doc) {
                    return Err(format!(
                        "$all on '{field}' only accepts plain values here — $all with $elemMatch entries is not supported"
                    ));
                }
                preds.push(Pred::All(arr));
            }
            "$size" => {
                let n = arg
                    .as_f64()
                    .filter(|n| *n >= 0.0 && n.fract() == 0.0)
                    .ok_or_else(|| format!("$size on '{field}' must be a non-negative whole number"))?;
                preds.push(Pred::Size(n as usize));
            }
            "$elemMatch" => {
                let node = if is_operator_doc(arg) {
                    // Operator form: {$elemMatch: {$gt: 5}} — conditions apply to each element itself.
                    Node::Field { path: Vec::new(), preds: compile_field(field, arg, depth + 1)? }
                } else {
                    compile(arg, depth + 1)?
                };
                preds.push(Pred::ElemMatch(Box::new(node)));
            }
            "$not" => {
                let inner = match arg {
                    Value::Object(m) if !m.is_empty() => arg,
                    other => {
                        return Err(format!(
                            "$not on '{field}' takes an operator document or a regex, got {}",
                            kind_of(other)
                        ))
                    }
                };
                if !is_operator_doc(inner) {
                    return Err(format!(
                        "$not on '{field}' cannot take a plain value — use $ne for that"
                    ));
                }
                preds.push(Pred::Not(compile_field(field, inner, depth + 1)?));
            }
            other => {
                return Err(format!(
                    "unknown operator '{other}' on field '{field}' — supported: $eq $ne $gt $gte $lt $lte $in $nin $exists $type $regex $options $mod $all $size $elemMatch $not"
                ))
            }
        }
    }
    if let Some(pat) = regex_pat {
        preds.push(Pred::Regex(build_regex(&pat, regex_opts.as_deref(), field)?));
    } else if regex_opts.is_some() {
        return Err(format!("$options on '{field}' needs a $regex alongside it"));
    }
    Ok(preds)
}

/// Pull `($regex, $options)` out of an operator document, if present.
fn regex_parts(v: &Value) -> (Option<String>, Option<String>) {
    match v {
        Value::Object(m) => (
            m.get("$regex").and_then(Value::as_str).map(str::to_string),
            m.get("$options").and_then(Value::as_str).map(str::to_string),
        ),
        _ => (None, None),
    }
}

fn build_regex(pattern: &str, options: Option<&str>, field: &str) -> Result<Regex, String> {
    let mut prefix = String::new();
    for flag in options.unwrap_or("").chars() {
        match flag {
            'i' => prefix.push('i'),
            'm' => prefix.push('m'),
            's' => prefix.push('s'),
            'x' => prefix.push('x'),
            'u' => {}
            other => {
                return Err(format!(
                    "unknown regex flag '{other}' in $options on '{field}' — supported flags are i (case-insensitive), m (^/$ match line breaks), s (. matches newline) and x (ignore whitespace)"
                ))
            }
        }
    }
    let full = if prefix.is_empty() { pattern.to_string() } else { format!("(?{prefix}){pattern}") };
    Regex::new(&full).map_err(|e| format!("invalid regex on '{field}': {e}"))
}

fn type_alias(v: &Value, field: &str) -> Result<String, String> {
    let name = match v {
        Value::String(s) => s.to_ascii_lowercase(),
        // BSON numeric type codes that have a JSON equivalent.
        Value::Number(n) => match n.as_i64() {
            Some(1) => "double".into(),
            Some(2) => "string".into(),
            Some(3) => "object".into(),
            Some(4) => "array".into(),
            Some(8) => "bool".into(),
            Some(10) => "null".into(),
            Some(16) => "int".into(),
            Some(18) => "long".into(),
            Some(19) => "decimal".into(),
            Some(code) => {
                return Err(format!(
                    "$type code {code} on '{field}' has no JSON equivalent — use one of: double, string, object, array, bool, null, int, long, decimal, number"
                ))
            }
            None => return Err(format!("$type on '{field}' must be a type name or BSON type code")),
        },
        other => {
            return Err(format!(
                "$type on '{field}' must be a type name like \"string\", got {}",
                kind_of(other)
            ))
        }
    };
    match name.as_str() {
        "double" | "string" | "object" | "array" | "bool" | "boolean" | "null" | "int" | "long"
        | "decimal" | "number" => Ok(name),
        other => Err(format!(
            "unknown $type alias '{other}' on '{field}' — JSON supports: double, int, long, decimal, number, string, object, array, bool, null"
        )),
    }
}

fn truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Value::Null => false,
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

fn kind_of(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "a boolean",
        Value::Number(_) => "a number",
        Value::String(_) => "a string",
        Value::Array(_) => "an array",
        Value::Object(_) => "an object",
    }
}

// =================================================================================================
// Matching
// =================================================================================================

/// Resolve a dotted path, MongoDB style: object keys step in, and an array is traversed
/// both by numeric index and elementwise (so `items.qty` reaches every element's `qty`).
fn resolve<'a>(doc: &'a Value, path: &[String]) -> Vec<&'a Value> {
    let mut current: Vec<&Value> = vec![doc];
    for seg in path {
        let mut next: Vec<&Value> = Vec::new();
        for v in current {
            match v {
                Value::Object(m) => {
                    if let Some(child) = m.get(seg) {
                        next.push(child);
                    }
                }
                Value::Array(a) => {
                    if let Ok(idx) = seg.parse::<usize>() {
                        if let Some(child) = a.get(idx) {
                            next.push(child);
                        }
                    }
                    for el in a {
                        if let Value::Object(m) = el {
                            if let Some(child) = m.get(seg) {
                                next.push(child);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        current = next;
        if current.is_empty() {
            return current;
        }
    }
    current
}

fn matches(node: &Node, doc: &Value) -> bool {
    match node {
        Node::True => true,
        Node::And(subs) => subs.iter().all(|s| matches(s, doc)),
        Node::Or(subs) => subs.iter().any(|s| matches(s, doc)),
        Node::Nor(subs) => !subs.iter().any(|s| matches(s, doc)),
        Node::Field { path, preds } => {
            let values = if path.is_empty() { vec![doc] } else { resolve(doc, path) };
            preds.iter().all(|p| pred_matches(p, &values))
        }
    }
}

fn pred_matches(pred: &Pred, values: &[&Value]) -> bool {
    match pred {
        Pred::Exists(want) => !values.is_empty() == *want,
        // A missing field is JSON null for equality purposes, exactly as in MongoDB.
        Pred::Eq(Value::Null) if values.is_empty() => true,
        Pred::Ne(Value::Null) if values.is_empty() => false,
        // $ne / $nin / $not hold for documents that lack the field entirely.
        Pred::Ne(_) | Pred::Nin(_) | Pred::Not(_) if values.is_empty() => true,
        Pred::Size(n) => values.iter().any(|v| matches!(v, Value::Array(a) if a.len() == *n)),
        Pred::All(items) => values.iter().any(|v| match v {
            Value::Array(a) => items.iter().all(|want| a.iter().any(|got| got == want)),
            other => items.iter().all(|want| want == *other),
        }),
        Pred::ElemMatch(node) => values.iter().any(|v| match v {
            Value::Array(a) => a.iter().any(|el| matches(node, el)),
            _ => false,
        }),
        Pred::Not(inner) => !inner.iter().all(|p| pred_matches(p, values)),
        Pred::Ne(want) => !values.iter().any(|v| scalar_or_element(v, &|x| x == want)),
        Pred::Nin(members) => !values
            .iter()
            .any(|v| scalar_or_element(v, &|x| members.iter().any(|m| member_matches(m, x)))),
        Pred::Eq(want) => values.iter().any(|v| scalar_or_element(v, &|x| x == want)),
        Pred::In(members) => values
            .iter()
            .any(|v| scalar_or_element(v, &|x| members.iter().any(|m| member_matches(m, x)))),
        Pred::Gt(want) => cmp_any(values, want, &|o| o == Ordering::Greater),
        Pred::Gte(want) => cmp_any(values, want, &|o| o != Ordering::Less),
        Pred::Lt(want) => cmp_any(values, want, &|o| o == Ordering::Less),
        Pred::Lte(want) => cmp_any(values, want, &|o| o != Ordering::Greater),
        Pred::Type(names) => values
            .iter()
            .any(|v| scalar_or_element(v, &|x| names.iter().any(|n| type_matches(n, x)))),
        Pred::Regex(re) => values.iter().any(|v| {
            scalar_or_element(v, &|x| matches!(x, Value::String(s) if re.is_match(s)))
        }),
        Pred::Mod(d, r) => values.iter().any(|v| {
            scalar_or_element(v, &|x| match x.as_f64() {
                Some(n) => (n.trunc() % d - r).abs() < f64::EPSILON,
                None => false,
            })
        }),
    }
}

/// MongoDB applies a value predicate to the field itself AND, when the field holds an
/// array, to each element.
fn scalar_or_element(v: &Value, f: &dyn Fn(&Value) -> bool) -> bool {
    if f(v) {
        return true;
    }
    matches!(v, Value::Array(a) if a.iter().any(f))
}

fn member_matches(member: &Member, value: &Value) -> bool {
    match member {
        Member::Value(want) => value == want,
        Member::Regex(re) => matches!(value, Value::String(s) if re.is_match(s)),
    }
}

fn cmp_any(values: &[&Value], want: &Value, ok: &dyn Fn(Ordering) -> bool) -> bool {
    values
        .iter()
        .any(|v| scalar_or_element(v, &|x| same_type_cmp(x, want).map(ok).unwrap_or(false)))
}

/// Compare two JSON values, but only when they are the same kind — MongoDB never
/// relates values of different BSON types with `$gt`/`$lt`.
fn same_type_cmp(a: &Value, b: &Value) -> Option<Ordering> {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => {
            x.as_f64().and_then(|x| y.as_f64().and_then(|y| x.partial_cmp(&y)))
        }
        (Value::String(x), Value::String(y)) => Some(x.cmp(y)),
        (Value::Bool(x), Value::Bool(y)) => Some(x.cmp(y)),
        (Value::Null, Value::Null) => Some(Ordering::Equal),
        _ => None,
    }
}

fn type_matches(name: &str, v: &Value) -> bool {
    match name {
        "number" => v.is_number(),
        "double" | "decimal" => v.as_f64().is_some(),
        "int" | "long" => v.as_i64().is_some() || v.as_u64().is_some(),
        "string" => v.is_string(),
        "object" => v.is_object(),
        "array" => v.is_array(),
        "bool" | "boolean" => v.is_boolean(),
        "null" => v.is_null(),
        _ => false,
    }
}

// =================================================================================================
// Sort / projection / output
// =================================================================================================

/// MongoDB's cross-type sort order, restricted to the types JSON can express.
fn type_rank(v: &Value) -> u8 {
    match v {
        Value::Null => 0,
        Value::Number(_) => 1,
        Value::String(_) => 2,
        Value::Object(_) => 3,
        Value::Array(_) => 4,
        Value::Bool(_) => 5,
    }
}

fn sort_cmp(a: &Value, b: &Value) -> Ordering {
    let (ra, rb) = (type_rank(a), type_rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x
            .as_f64()
            .unwrap_or(f64::NAN)
            .partial_cmp(&y.as_f64().unwrap_or(f64::NAN))
            .unwrap_or(Ordering::Equal),
        (Value::String(x), Value::String(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        (Value::Array(x), Value::Array(y)) => {
            for (ax, by) in x.iter().zip(y.iter()) {
                let c = sort_cmp(ax, by);
                if c != Ordering::Equal {
                    return c;
                }
            }
            x.len().cmp(&y.len())
        }
        (Value::Object(x), Value::Object(y)) => {
            for ((_, ax), (_, by)) in x.iter().zip(y.iter()) {
                let c = sort_cmp(ax, by);
                if c != Ordering::Equal {
                    return c;
                }
            }
            x.len().cmp(&y.len())
        }
        _ => Ordering::Equal,
    }
}

/// Parse `sort` as either a Mongo sort document (`{"age": -1}`) or the short form
/// (`age:desc, name`).
fn parse_sort(spec: &str) -> Result<Vec<(Vec<String>, bool)>, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    if spec.starts_with('{') {
        let v = parse_relaxed(spec).map_err(|e| format!("invalid sort: {e}"))?;
        let map = match v {
            Value::Object(m) => m,
            other => return Err(format!("sort must be an object like {{\"age\": -1}}, got {}", kind_of(&other))),
        };
        for (field, dir) in map {
            let d = dir.as_f64().ok_or_else(|| {
                format!("sort direction for '{field}' must be 1 (ascending) or -1 (descending)")
            })?;
            if d != 1.0 && d != -1.0 {
                return Err(format!(
                    "sort direction for '{field}' must be 1 (ascending) or -1 (descending), got {d}"
                ));
            }
            out.push((split_path(&field), d < 0.0));
        }
    } else {
        for part in spec.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (name, desc) = if let Some(rest) = part.strip_prefix('-') {
                (rest.trim(), true)
            } else if let Some((n, d)) = part.rsplit_once(':') {
                let dir = d.trim().to_ascii_lowercase();
                match dir.as_str() {
                    "desc" | "-1" | "descending" => (n.trim(), true),
                    "asc" | "1" | "ascending" => (n.trim(), false),
                    other => {
                        return Err(format!(
                            "unknown sort direction '{other}' — use 'asc'/'desc' (or 1 / -1)"
                        ))
                    }
                }
            } else {
                (part, false)
            };
            if name.is_empty() {
                return Err("sort entry is missing a field name".into());
            }
            out.push((split_path(name), desc));
        }
    }
    Ok(out)
}

enum Projection {
    None,
    Include { paths: Vec<Vec<String>>, id: bool },
    Exclude { paths: Vec<Vec<String>> },
}

/// Parse `projection` as either a Mongo projection document (`{"name": 1, "_id": 0}`) or
/// the short form (`name, email` / `-password`).
fn parse_projection(spec: &str) -> Result<Projection, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(Projection::None);
    }
    let mut include: Vec<Vec<String>> = Vec::new();
    let mut exclude: Vec<Vec<String>> = Vec::new();
    let mut id_flag: Option<bool> = None;

    let mut push = |field: &str, keep: bool| -> Result<(), String> {
        if field.is_empty() {
            return Err("projection entry is missing a field name".into());
        }
        if field == "_id" {
            id_flag = Some(keep);
            if keep {
                include.push(split_path(field));
            }
            return Ok(());
        }
        if keep {
            include.push(split_path(field));
        } else {
            exclude.push(split_path(field));
        }
        Ok(())
    };

    if spec.starts_with('{') {
        let v = parse_relaxed(spec).map_err(|e| format!("invalid projection: {e}"))?;
        let map = match v {
            Value::Object(m) => m,
            other => {
                return Err(format!(
                    "projection must be an object like {{\"name\": 1}}, got {}",
                    kind_of(&other)
                ))
            }
        };
        for (field, flag) in map {
            if field.starts_with('$') {
                return Err(format!(
                    "projection operator '{field}' is not supported — list plain field names with 1 (keep) or 0 (drop)"
                ));
            }
            push(&field, truthy(&flag))?;
        }
    } else {
        for part in spec.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match part.strip_prefix('-') {
                Some(rest) => push(rest.trim(), false)?,
                None => push(part.trim_start_matches('+').trim(), true)?,
            }
        }
    }

    match (include.is_empty(), exclude.is_empty()) {
        (true, true) => Ok(Projection::None),
        (false, true) => Ok(Projection::Include { paths: include, id: id_flag.unwrap_or(true) }),
        (true, false) => Ok(Projection::Exclude { paths: exclude }),
        (false, false) => Err(
            "a projection cannot mix kept and dropped fields (except _id) — list only the fields to keep, or only the fields to drop"
                .into(),
        ),
    }
}

fn project(doc: &Value, proj: &Projection) -> Value {
    match proj {
        Projection::None => doc.clone(),
        Projection::Include { paths, id } => {
            let mut out = Value::Object(Map::new());
            if *id {
                if let Some(v) = doc.get("_id") {
                    if let Value::Object(m) = &mut out {
                        m.insert("_id".into(), v.clone());
                    }
                }
            }
            for path in paths {
                if let Some(v) = pick(doc, path) {
                    insert_path(&mut out, path, v);
                }
            }
            out
        }
        Projection::Exclude { paths } => {
            let mut out = doc.clone();
            for path in paths {
                remove_path(&mut out, path);
            }
            out
        }
    }
}

/// Straight (non-array-traversing) lookup used by projection.
fn pick(doc: &Value, path: &[String]) -> Option<Value> {
    let mut cur = doc;
    for seg in path {
        cur = match cur {
            Value::Object(m) => m.get(seg)?,
            Value::Array(a) => a.get(seg.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(cur.clone())
}

fn insert_path(target: &mut Value, path: &[String], value: Value) {
    let (first, rest) = match path.split_first() {
        Some(p) => p,
        None => return,
    };
    if let Value::Object(m) = target {
        if rest.is_empty() {
            m.insert(first.clone(), value);
        } else {
            let child = m.entry(first.clone()).or_insert_with(|| Value::Object(Map::new()));
            if !child.is_object() {
                *child = Value::Object(Map::new());
            }
            insert_path(child, rest, value);
        }
    }
}

fn remove_path(target: &mut Value, path: &[String]) {
    let (first, rest) = match path.split_first() {
        Some(p) => p,
        None => return,
    };
    match target {
        Value::Object(m) => {
            if rest.is_empty() {
                m.remove(first);
            } else if let Some(child) = m.get_mut(first) {
                remove_path(child, rest);
            }
        }
        Value::Array(a) => {
            for el in a.iter_mut() {
                remove_path(el, path);
            }
        }
        _ => {}
    }
}

// =================================================================================================
// Input parsing + output rendering
// =================================================================================================

fn parse_docs(data: &str) -> Result<Vec<Value>, String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err("no documents — paste a JSON array of documents, e.g. [{\"name\":\"Ada\",\"age\":36}]".into());
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "documents input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            data.len()
        ));
    }
    let docs = match serde_json::from_str::<Value>(trimmed) {
        Ok(Value::Array(a)) => a,
        Ok(Value::Object(m)) => vec![Value::Object(m)],
        Ok(other) => {
            return Err(format!(
                "documents must be a JSON array of objects, got {} — wrap it in [ ]",
                kind_of(&other)
            ))
        }
        Err(whole_err) => {
            // Fall back to NDJSON / JSON Lines, one document per line.
            let mut out = Vec::new();
            for (i, line) in trimmed.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(line.trim()) {
                    Ok(v) => out.push(v),
                    Err(e) => {
                        return Err(format!(
                            "invalid JSON documents: {whole_err} (also tried NDJSON — line {} failed: {e})",
                            i + 1
                        ))
                    }
                }
            }
            if out.is_empty() {
                return Err(format!("invalid JSON documents: {whole_err}"));
            }
            out
        }
    };
    if docs.len() > MAX_DOCS {
        return Err(format!("{} documents, over the {MAX_DOCS}-document limit", docs.len()));
    }
    Ok(docs)
}

fn render_csv(docs: &[Value]) -> String {
    let mut cols: Vec<String> = Vec::new();
    for doc in docs {
        if let Value::Object(m) = doc {
            for k in m.keys() {
                if !cols.iter().any(|c| c == k) {
                    cols.push(k.clone());
                }
            }
        }
    }
    let mut out = String::new();
    if cols.is_empty() {
        return out;
    }
    out.push_str(&cols.iter().map(|c| csv_cell(c)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for doc in docs {
        let row: Vec<String> = cols
            .iter()
            .map(|c| {
                let v = doc.get(c);
                csv_cell(&match v {
                    None | Some(Value::Null) => String::new(),
                    Some(Value::String(s)) => s.clone(),
                    Some(other) => other.to_string(),
                })
            })
            .collect();
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Run a MongoDB-style query against a JSON collection.
///
/// - `data`: a JSON array of documents (a single object or NDJSON is accepted too).
/// - `query`: the query document; blank or `{}` matches every document.
/// - `projection`: fields to keep or drop (`{"name":1}` or `name,email` / `-password`).
/// - `sort`: `{"age":-1}` or `age:desc,name`; blank keeps the input order.
/// - `skip` / `limit`: cursor paging; `limit` 0 means unlimited.
/// - `format`: `json` (default), `ndjson`, `csv`, or `count`.
/// - `pretty`: indent the `json` output.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    query: &str,
    projection: &str,
    sort: &str,
    skip: usize,
    limit: usize,
    format: &str,
    pretty: bool,
) -> Result<String, String> {
    let format = match format.trim() {
        "" | "json" => "json",
        "ndjson" => "ndjson",
        "csv" => "csv",
        "count" => "count",
        other => {
            return Err(format!(
                "unknown format '{other}' — use json, ndjson, csv or count"
            ))
        }
    };
    for (name, text) in [("query", query), ("projection", projection), ("sort", sort)] {
        if text.len() > MAX_QUERY_BYTES {
            return Err(format!(
                "{name} is {} bytes, over the {MAX_QUERY_BYTES}-byte limit",
                text.len()
            ));
        }
    }

    let docs = parse_docs(data)?;
    let query_text = query.trim();
    let node = if query_text.is_empty() {
        Node::True
    } else {
        let parsed = parse_relaxed(query_text).map_err(|e| format!("invalid query: {e}"))?;
        compile(&parsed, 0)?
    };
    let sort_keys = parse_sort(sort)?;
    let proj = parse_projection(projection)?;

    let mut hits: Vec<&Value> = docs.iter().filter(|d| matches(&node, d)).collect();
    let total = hits.len();

    if !sort_keys.is_empty() {
        hits.sort_by(|a, b| {
            for (path, desc) in &sort_keys {
                let va = resolve(a, path).into_iter().next().cloned().unwrap_or(Value::Null);
                let vb = resolve(b, path).into_iter().next().cloned().unwrap_or(Value::Null);
                let ord = sort_cmp(&va, &vb);
                let ord = if *desc { ord.reverse() } else { ord };
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            Ordering::Equal
        });
    }

    let paged: Vec<Value> = hits
        .into_iter()
        .skip(skip)
        .take(if limit == 0 { usize::MAX } else { limit })
        .map(|d| project(d, &proj))
        .collect();

    Ok(match format {
        "count" => total.to_string(),
        "csv" => render_csv(&paged),
        "ndjson" => paged
            .iter()
            .map(|d| serde_json::to_string(d).unwrap_or_default())
            .collect::<Vec<_>>()
            .join("\n"),
        _ if pretty => serde_json::to_string_pretty(&paged).map_err(|e| e.to_string())?,
        _ => serde_json::to_string(&paged).map_err(|e| e.to_string())?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = r#"[
      {"name":"Ada","age":36,"tags":["math","code"],"team":{"id":1,"name":"core"}},
      {"name":"Bo","age":24,"tags":["ops"],"team":{"id":2,"name":"infra"}},
      {"name":"Cy","age":41,"tags":["code","ops"],"team":{"id":1,"name":"core"}},
      {"name":"Di","tags":[],"team":{"id":3,"name":"data"}}
    ]"#;

    fn names(query: &str) -> Vec<String> {
        let out = run(DATA, query, "name,-_id", "", 0, 0, "ndjson", false).unwrap();
        out.lines()
            .map(|l| {
                serde_json::from_str::<Value>(l).unwrap()["name"].as_str().unwrap().to_string()
            })
            .collect()
    }

    #[test]
    fn comparison_and_logical_operators() {
        assert_eq!(names(r#"{"age": {"$gt": 30}}"#), ["Ada", "Cy"]);
        assert_eq!(names(r#"{"age": {"$gte": 24, "$lt": 41}}"#), ["Ada", "Bo"]);
        assert_eq!(names(r#"{"$or":[{"name":"Bo"},{"age":{"$gt":40}}]}"#), ["Bo", "Cy"]);
        assert_eq!(names(r#"{"$nor":[{"age":{"$gt":30}}]}"#), ["Bo", "Di"]);
        assert_eq!(names(r#"{"$and":[{"age":{"$gt":20}},{"team.id":1}]}"#), ["Ada", "Cy"]);
    }

    #[test]
    fn array_dotted_and_element_operators() {
        // A predicate on an array field holds when ANY element matches.
        assert_eq!(names(r#"{"tags":"code"}"#), ["Ada", "Cy"]);
        assert_eq!(names(r#"{"tags":{"$all":["code","ops"]}}"#), ["Cy"]);
        assert_eq!(names(r#"{"tags":{"$size":0}}"#), ["Di"]);
        assert_eq!(names(r#"{"tags":{"$in":["ops"]}}"#), ["Bo", "Cy"]);
        assert_eq!(names(r#"{"team.name":"core"}"#), ["Ada", "Cy"]);
        assert_eq!(names(r#"{"age":{"$exists":false}}"#), ["Di"]);
        assert_eq!(names(r#"{"age":{"$type":"number"}}"#), ["Ada", "Bo", "Cy"]);
    }

    #[test]
    fn missing_fields_follow_mongodb_semantics() {
        // {field: null} also matches documents that lack the field.
        assert_eq!(names(r#"{"age": null}"#), ["Di"]);
        // $ne / $nin / $not match missing fields too.
        assert_eq!(names(r#"{"age":{"$ne":24}}"#), ["Ada", "Cy", "Di"]);
        assert_eq!(names(r#"{"age":{"$nin":[24,36]}}"#), ["Cy", "Di"]);
        assert_eq!(names(r#"{"age":{"$not":{"$gt":30}}}"#), ["Bo", "Di"]);
    }

    #[test]
    fn regex_mod_and_elemmatch() {
        assert_eq!(names(r#"{"name":{"$regex":"^a","$options":"i"}}"#), ["Ada"]);
        assert_eq!(names("{name: /^[BC]/}"), ["Bo", "Cy"]);
        assert_eq!(names(r#"{"age":{"$mod":[2,0]}}"#), ["Ada", "Bo"]);
        assert_eq!(names(r#"{"age":{"$mod":[5,1]}}"#), ["Ada", "Cy"]);
        let orders = r#"[{"id":1,"items":[{"sku":"a","qty":2},{"sku":"b","qty":9}]},
                          {"id":2,"items":[{"sku":"a","qty":9}]}]"#;
        let out = run(
            orders,
            r#"{"items":{"$elemMatch":{"sku":"a","qty":{"$gt":5}}}}"#,
            "id",
            "",
            0,
            0,
            "ndjson",
            false,
        )
        .unwrap();
        assert_eq!(out, r#"{"id":2}"#);
    }

    #[test]
    fn relaxed_shell_syntax_is_accepted() {
        // Unquoted keys, single quotes, a comment, a trailing comma and a helper call.
        assert_eq!(
            names("{ name: 'Ada', /* by name */ age: { $gte: NumberInt(30), }, }"),
            ["Ada"]
        );
        let ids = r#"[{"_id":"64b8f0","n":1},{"_id":"64b8f1","n":2}]"#;
        let out = run(ids, r#"{_id: ObjectId("64b8f1")}"#, "", "", 0, 0, "ndjson", false).unwrap();
        assert_eq!(out, r#"{"_id":"64b8f1","n":2}"#);
    }

    #[test]
    fn sort_skip_limit_and_projection() {
        assert_eq!(
            run(DATA, "{}", "name,-_id", "age:desc", 0, 2, "ndjson", false).unwrap(),
            "{\"name\":\"Cy\"}\n{\"name\":\"Ada\"}"
        );
        // Missing values sort first ascending, and skip pages past them.
        assert_eq!(
            run(DATA, "{}", "name", r#"{"age": 1}"#, 1, 1, "ndjson", false).unwrap(),
            "{\"name\":\"Bo\"}"
        );
        // Exclusion projection drops a nested field and keeps the rest.
        assert_eq!(
            run(DATA, r#"{"name":"Bo"}"#, "-tags,-team.id", "", 0, 0, "ndjson", false).unwrap(),
            r#"{"name":"Bo","age":24,"team":{"name":"infra"}}"#
        );
        // _id is kept by an inclusion projection unless it is explicitly dropped.
        let ids = r#"[{"_id":7,"n":1,"z":2}]"#;
        assert_eq!(
            run(ids, "{}", r#"{"n":1}"#, "", 0, 0, "ndjson", false).unwrap(),
            r#"{"_id":7,"n":1}"#
        );
        assert_eq!(
            run(ids, "{}", r#"{"n":1,"_id":0}"#, "", 0, 0, "ndjson", false).unwrap(),
            r#"{"n":1}"#
        );
    }

    #[test]
    fn output_formats() {
        assert_eq!(run(DATA, r#"{"team.id":1}"#, "", "", 0, 0, "count", true).unwrap(), "2");
        assert_eq!(
            run(DATA, r#"{"age":{"$gt":30}}"#, "name,age,-_id", "name", 0, 0, "csv", true).unwrap(),
            "name,age\nAda,36\nCy,41\n"
        );
        assert_eq!(
            run(DATA, r#"{"name":"Bo"}"#, "name,-_id", "", 0, 0, "json", true).unwrap(),
            "[\n  {\n    \"name\": \"Bo\"\n  }\n]"
        );
        assert_eq!(
            run(DATA, r#"{"name":"Bo"}"#, "name,-_id", "", 0, 0, "json", false).unwrap(),
            r#"[{"name":"Bo"}]"#
        );
        // count reports every match, not just the current page.
        assert_eq!(run(DATA, "{}", "", "", 0, 1, "count", true).unwrap(), "4");
    }

    #[test]
    fn ndjson_and_single_object_input_are_accepted() {
        let nd = "{\"n\":1}\n{\"n\":2}\n";
        assert_eq!(run(nd, r#"{"n":{"$gt":1}}"#, "", "", 0, 0, "ndjson", false).unwrap(), r#"{"n":2}"#);
        assert_eq!(run(r#"{"n":5}"#, "{}", "", "", 0, 0, "count", true).unwrap(), "1");
    }

    #[test]
    fn errors_are_actionable() {
        let e = run(DATA, r#"{"age":{"$whoops":1}}"#, "", "", 0, 0, "json", true).unwrap_err();
        assert!(e.contains("unknown operator '$whoops'"), "{e}");

        let e = run(DATA, r#"{"$where":"this.age > 3"}"#, "", "", 0, 0, "json", true).unwrap_err();
        assert!(e.contains("$where") && e.contains("JavaScript"), "{e}");

        let e = run("not json", "{}", "", "", 0, 0, "json", true).unwrap_err();
        assert!(e.contains("invalid JSON documents"), "{e}");

        let e = run("", "{}", "", "", 0, 0, "json", true).unwrap_err();
        assert!(e.contains("no documents"), "{e}");

        let e = run(DATA, "{age: }", "", "", 0, 0, "json", true).unwrap_err();
        assert!(e.contains("invalid query"), "{e}");

        let e = run(DATA, "{}", "name,-age", "", 0, 0, "json", true).unwrap_err();
        assert!(e.contains("cannot mix"), "{e}");

        let e = run(DATA, "{}", "", "age:sideways", 0, 0, "json", true).unwrap_err();
        assert!(e.contains("unknown sort direction"), "{e}");

        let e = run(DATA, "{}", "", "", 0, 0, "yaml", true).unwrap_err();
        assert!(e.contains("unknown format 'yaml'"), "{e}");

        let e = run(DATA, r#"{"name":{"$regex":"(","$options":"i"}}"#, "", "", 0, 0, "json", true)
            .unwrap_err();
        assert!(e.contains("invalid regex"), "{e}");

        let e = run(DATA, r#"{"name":{"$regex":"a","$options":"q"}}"#, "", "", 0, 0, "json", true)
            .unwrap_err();
        assert!(e.contains("unknown regex flag 'q'"), "{e}");
    }

    #[test]
    fn caps_are_enforced() {
        let big = format!("[{{\"q\":\"{}\"}}]", "x".repeat(10));
        assert!(run(&big, &"x".repeat(MAX_QUERY_BYTES + 1), "", "", 0, 0, "json", true)
            .unwrap_err()
            .contains("over the"));
        let docs: Vec<String> = (0..MAX_DOCS + 1).map(|i| format!("{{\"i\":{i}}}")).collect();
        let e = run(&format!("[{}]", docs.join(",")), "{}", "", "", 0, 0, "count", true).unwrap_err();
        assert!(e.contains("over the"), "{e}");
    }
}
