//! A small, self-contained evaluator for the JSONata-style subset that the
//! data-reshape tool advertises. It is deliberately *not* a full JSONata engine:
//! it supports exactly the table-stakes features documented on the page and used
//! by the tests, and — crucially — depends on nothing platform-specific (no
//! monotonic clock, no RNG), so it evaluates identically on native, WASI, and the
//! browser `wasm32-unknown-unknown` target.
//!
//! Supported grammar:
//!   - literals: numbers, `"..."`/`'...'` strings, `true`/`false`/`null`
//!   - `$` — the whole input document (root); bare `name` — a field of the context
//!   - path navigation `a.b.c`, mapping over arrays as JSONata does
//!   - mapping steps `.(expr)` and object-construction steps `.{ ... }`
//!   - predicate filters `arr[ expr ]` (boolean filter or numeric index)
//!   - object `{ "k": expr, ... }` and array `[ a, b ]` construction
//!   - aggregates `$sum`, `$count`, `$min`, `$max`, `$average`
//!   - `&` string concatenation, `+ - * /` arithmetic, `= != < > <= >=` comparison
//!
//! Sequence semantics follow JSONata: navigation produces flattened sequences that
//! collapse to a single value (one item) or `undefined`/null (no items) at the end.

use serde_json::Value;

// ----------------------------------------------------------------------------
// AST
// ----------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum Ast {
    Num(f64),
    Str(String),
    Bool(bool),
    Null,
    /// `$` — the root input document.
    Root,
    /// A bare identifier: navigate this field of the current context.
    Field(String),
    /// `lhs . rhs` — evaluate `rhs` for each item of `lhs` and flatten.
    Path(Box<Ast>, Box<Ast>),
    /// `lhs [ filter ]` — filter/index the sequence produced by `lhs`.
    Predicate(Box<Ast>, Box<Ast>),
    /// `{ "k": v, ... }` object construction.
    Object(Vec<(Ast, Ast)>),
    /// `[ a, b, ... ]` array construction.
    Array(Vec<Ast>),
    /// `$name( args )` function call.
    Func(String, Vec<Ast>),
    Binary(Op, Box<Ast>, Box<Ast>),
    Neg(Box<Ast>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Add,
    Sub,
    Mul,
    Div,
    Concat,
    Eq,
    Ne,
    Lt,
    Gt,
    Le,
    Ge,
}

// ----------------------------------------------------------------------------
// Tokenizer
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Str(String),
    Ident(String),
    /// `$name` (name is non-empty).
    Var(String),
    /// `$` alone.
    Root,
    /// A single-char punctuation token: ( ) { } [ ] , : .
    Punct(char),
    /// An operator token.
    Op(Op),
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    let mut out = Vec::new();
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\r' | '\n' => {
                i += 1;
            }
            '"' | '\'' => {
                let quote = c;
                i += 1;
                let mut s = String::new();
                loop {
                    if i >= chars.len() {
                        return Err("unterminated string literal".into());
                    }
                    let ch = chars[i];
                    if ch == quote {
                        i += 1;
                        break;
                    }
                    if ch == '\\' {
                        i += 1;
                        if i >= chars.len() {
                            return Err("unterminated escape in string".into());
                        }
                        let esc = chars[i];
                        s.push(match esc {
                            'n' => '\n',
                            't' => '\t',
                            'r' => '\r',
                            '"' => '"',
                            '\'' => '\'',
                            '\\' => '\\',
                            '/' => '/',
                            other => other,
                        });
                        i += 1;
                    } else {
                        s.push(ch);
                        i += 1;
                    }
                }
                out.push(Tok::Str(s));
            }
            '0'..='9' => {
                let start = i;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
                if i < chars.len() && chars[i] == '.' && i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                    i += 1;
                    while i < chars.len() && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                }
                let text: String = chars[start..i].iter().collect();
                let n: f64 = text
                    .parse()
                    .map_err(|_| format!("invalid number '{text}'"))?;
                out.push(Tok::Num(n));
            }
            '$' => {
                i += 1;
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                if i > start {
                    out.push(Tok::Var(chars[start..i].iter().collect()));
                } else {
                    out.push(Tok::Root);
                }
            }
            c if c.is_ascii_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                out.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            '(' | ')' | '{' | '}' | '[' | ']' | ',' | ':' | '.' => {
                out.push(Tok::Punct(c));
                i += 1;
            }
            '&' => {
                out.push(Tok::Op(Op::Concat));
                i += 1;
            }
            '+' => {
                out.push(Tok::Op(Op::Add));
                i += 1;
            }
            '-' => {
                out.push(Tok::Op(Op::Sub));
                i += 1;
            }
            '*' => {
                out.push(Tok::Op(Op::Mul));
                i += 1;
            }
            '/' => {
                out.push(Tok::Op(Op::Div));
                i += 1;
            }
            '=' => {
                out.push(Tok::Op(Op::Eq));
                i += 1;
            }
            '!' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    out.push(Tok::Op(Op::Ne));
                    i += 2;
                } else {
                    return Err("unexpected '!' (expected '!=')".into());
                }
            }
            '<' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    out.push(Tok::Op(Op::Le));
                    i += 2;
                } else {
                    out.push(Tok::Op(Op::Lt));
                    i += 1;
                }
            }
            '>' => {
                if i + 1 < chars.len() && chars[i + 1] == '=' {
                    out.push(Tok::Op(Op::Ge));
                    i += 2;
                } else {
                    out.push(Tok::Op(Op::Gt));
                    i += 1;
                }
            }
            other => return Err(format!("unexpected character '{other}'")),
        }
    }
    Ok(out)
}

// ----------------------------------------------------------------------------
// Parser (recursive descent)
// ----------------------------------------------------------------------------

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

    fn eat_punct(&mut self, c: char) -> Result<(), String> {
        match self.next() {
            Some(Tok::Punct(p)) if p == c => Ok(()),
            other => Err(format!("expected '{c}', found {other:?}")),
        }
    }

    fn is_punct(&self, c: char) -> bool {
        matches!(self.peek(), Some(Tok::Punct(p)) if *p == c)
    }

    fn parse_expr(&mut self) -> Result<Ast, String> {
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_concat()?;
        while let Some(Tok::Op(op)) = self.peek() {
            let op = *op;
            if matches!(op, Op::Eq | Op::Ne | Op::Lt | Op::Gt | Op::Le | Op::Ge) {
                self.pos += 1;
                let right = self.parse_concat()?;
                left = Ast::Binary(op, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_additive()?;
        while matches!(self.peek(), Some(Tok::Op(Op::Concat))) {
            self.pos += 1;
            let right = self.parse_additive()?;
            left = Ast::Binary(Op::Concat, Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_additive(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_mul()?;
        while let Some(Tok::Op(op)) = self.peek() {
            let op = *op;
            if matches!(op, Op::Add | Op::Sub) {
                self.pos += 1;
                let right = self.parse_mul()?;
                left = Ast::Binary(op, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_mul(&mut self) -> Result<Ast, String> {
        let mut left = self.parse_unary()?;
        while let Some(Tok::Op(op)) = self.peek() {
            let op = *op;
            if matches!(op, Op::Mul | Op::Div) {
                self.pos += 1;
                let right = self.parse_unary()?;
                left = Ast::Binary(op, Box::new(left), Box::new(right));
            } else {
                break;
            }
        }
        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Ast, String> {
        if matches!(self.peek(), Some(Tok::Op(Op::Sub))) {
            self.pos += 1;
            let inner = self.parse_unary()?;
            return Ok(Ast::Neg(Box::new(inner)));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> Result<Ast, String> {
        let mut node = self.parse_primary()?;
        loop {
            if self.is_punct('.') {
                self.pos += 1;
                let step = self.parse_primary()?;
                node = Ast::Path(Box::new(node), Box::new(step));
            } else if self.is_punct('[') {
                self.pos += 1;
                let filter = self.parse_expr()?;
                self.eat_punct(']')?;
                node = Ast::Predicate(Box::new(node), Box::new(filter));
            } else {
                break;
            }
        }
        Ok(node)
    }

    fn parse_primary(&mut self) -> Result<Ast, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Ast::Num(n)),
            Some(Tok::Str(s)) => Ok(Ast::Str(s)),
            Some(Tok::Root) => Ok(Ast::Root),
            Some(Tok::Var(name)) => {
                if self.is_punct('(') {
                    self.pos += 1;
                    let args = self.parse_args()?;
                    Ok(Ast::Func(name, args))
                } else {
                    Err(format!(
                        "unsupported variable '${name}' (only $ and the $sum/$count/$min/$max/$average functions are supported)"
                    ))
                }
            }
            Some(Tok::Ident(name)) => match name.as_str() {
                "true" => Ok(Ast::Bool(true)),
                "false" => Ok(Ast::Bool(false)),
                "null" => Ok(Ast::Null),
                _ => Ok(Ast::Field(name)),
            },
            Some(Tok::Punct('(')) => {
                let inner = self.parse_expr()?;
                self.eat_punct(')')?;
                Ok(inner)
            }
            Some(Tok::Punct('{')) => self.parse_object(),
            Some(Tok::Punct('[')) => self.parse_array(),
            other => Err(format!("unexpected token {other:?}")),
        }
    }

    fn parse_args(&mut self) -> Result<Vec<Ast>, String> {
        let mut args = Vec::new();
        if self.is_punct(')') {
            self.pos += 1;
            return Ok(args);
        }
        loop {
            args.push(self.parse_expr()?);
            if self.is_punct(',') {
                self.pos += 1;
                continue;
            }
            self.eat_punct(')')?;
            break;
        }
        Ok(args)
    }

    fn parse_object(&mut self) -> Result<Ast, String> {
        let mut pairs = Vec::new();
        if self.is_punct('}') {
            self.pos += 1;
            return Ok(Ast::Object(pairs));
        }
        loop {
            let key = self.parse_expr()?;
            self.eat_punct(':')?;
            let val = self.parse_expr()?;
            pairs.push((key, val));
            if self.is_punct(',') {
                self.pos += 1;
                continue;
            }
            self.eat_punct('}')?;
            break;
        }
        Ok(Ast::Object(pairs))
    }

    fn parse_array(&mut self) -> Result<Ast, String> {
        let mut items = Vec::new();
        if self.is_punct(']') {
            self.pos += 1;
            return Ok(Ast::Array(items));
        }
        loop {
            items.push(self.parse_expr()?);
            if self.is_punct(',') {
                self.pos += 1;
                continue;
            }
            self.eat_punct(']')?;
            break;
        }
        Ok(Ast::Array(items))
    }
}

/// Parse a reshape expression into an [`Ast`].
pub fn parse(src: &str) -> Result<Ast, String> {
    let toks = tokenize(src)?;
    let mut p = Parser { toks, pos: 0 };
    let ast = p.parse_expr()?;
    if p.pos != p.toks.len() {
        return Err(format!("unexpected trailing input at token {}", p.pos));
    }
    Ok(ast)
}

// ----------------------------------------------------------------------------
// Evaluator
// ----------------------------------------------------------------------------

/// The result of evaluating a node. `Seq` is a JSONata-style sequence produced by
/// navigation/mapping; `Val` is a single value (an array here is a value, not a
/// sequence); `Undef` is "no match".
#[derive(Debug, Clone)]
enum Res {
    Undef,
    Val(Value),
    Seq(Vec<Value>),
}

/// Recursion guard so a pathologically nested expression can't overflow the stack.
const MAX_DEPTH: usize = 512;

/// Evaluate `ast` against the whole input document and collapse the result to a
/// single JSON value (or `None` for a no-match, which the caller renders as null).
pub fn eval_root(ast: &Ast, root: &Value) -> Result<Option<Value>, String> {
    let res = eval(ast, &Res::Val(root.clone()), root, 0)?;
    Ok(collapse(res))
}

/// Expand a result into the list of context values it iterates over. An array
/// *value* is iterated element-wise (this is what makes `arr.field` map over the
/// array); a scalar/object is a single context.
fn expand(r: &Res) -> Vec<Value> {
    match r {
        Res::Undef => Vec::new(),
        Res::Val(Value::Array(a)) => a.clone(),
        Res::Val(v) => vec![v.clone()],
        Res::Seq(s) => s.clone(),
    }
}

/// Collapse a result to a single output value: a sequence of one unwraps, an empty
/// one is a no-match (`None`), and many become an array.
fn collapse(r: Res) -> Option<Value> {
    match r {
        Res::Undef => None,
        Res::Val(v) => Some(v),
        Res::Seq(s) => match s.len() {
            0 => None,
            1 => Some(s.into_iter().next().unwrap()),
            _ => Some(Value::Array(s)),
        },
    }
}

/// Collapse to a single scalar for use as an operand.
fn scalar(r: Res) -> Option<Value> {
    collapse(r)
}

fn eval(ast: &Ast, ctx: &Res, root: &Value, depth: usize) -> Result<Res, String> {
    if depth > MAX_DEPTH {
        return Err("expression nested too deeply".into());
    }
    let d = depth + 1;
    match ast {
        Ast::Num(n) => Ok(Res::Val(num_value(*n))),
        Ast::Str(s) => Ok(Res::Val(Value::String(s.clone()))),
        Ast::Bool(b) => Ok(Res::Val(Value::Bool(*b))),
        Ast::Null => Ok(Res::Val(Value::Null)),
        Ast::Root => Ok(Res::Val(root.clone())),
        Ast::Field(name) => {
            let mut out = Vec::new();
            for c in expand(ctx) {
                if let Value::Object(m) = c {
                    if let Some(v) = m.get(name) {
                        match v {
                            // Flatten an array field into the sequence so the next
                            // path step maps over it (JSONata semantics).
                            Value::Array(a) => out.extend(a.iter().cloned()),
                            other => out.push(other.clone()),
                        }
                    }
                }
            }
            Ok(if out.is_empty() { Res::Undef } else { Res::Seq(out) })
        }
        Ast::Path(lhs, rhs) => {
            let l = eval(lhs, ctx, root, d)?;
            let mut out = Vec::new();
            for c in expand(&l) {
                match eval(rhs, &Res::Val(c), root, d)? {
                    Res::Undef => {}
                    Res::Val(v) => out.push(v),
                    Res::Seq(s) => out.extend(s),
                }
            }
            Ok(if out.is_empty() { Res::Undef } else { Res::Seq(out) })
        }
        Ast::Predicate(lhs, filter) => {
            let l = eval(lhs, ctx, root, d)?;
            let items = expand(&l);
            // A numeric predicate selects by index (supports negatives); anything
            // else is a boolean filter evaluated per item.
            let probe = eval(filter, &Res::Val(Value::Null), root, d);
            if let Ok(Res::Val(Value::Number(_))) = &probe {
                // Constant numeric index.
                if let Ok(Res::Val(Value::Number(n))) = probe {
                    let idx = n.as_f64().unwrap_or(f64::NAN);
                    return Ok(index_seq(&items, idx));
                }
            }
            let mut out = Vec::new();
            for it in items {
                let f = eval(filter, &Res::Val(it.clone()), root, d)?;
                if truthy(&scalar(f)) {
                    out.push(it);
                }
            }
            Ok(if out.is_empty() { Res::Undef } else { Res::Seq(out) })
        }
        Ast::Object(pairs) => {
            let mut m = serde_json::Map::new();
            for (k, v) in pairs {
                let key = match scalar(eval(k, ctx, root, d)?) {
                    Some(Value::String(s)) => s,
                    Some(other) => stringify(&other),
                    None => String::new(),
                };
                let val = collapse(eval(v, ctx, root, d)?).unwrap_or(Value::Null);
                m.insert(key, val);
            }
            Ok(Res::Val(Value::Object(m)))
        }
        Ast::Array(items) => {
            let mut out = Vec::new();
            for it in items {
                if let Some(v) = collapse(eval(it, ctx, root, d)?) {
                    out.push(v);
                }
            }
            Ok(Res::Val(Value::Array(out)))
        }
        Ast::Func(name, args) => eval_func(name, args, ctx, root, d),
        Ast::Neg(inner) => {
            let v = scalar(eval(inner, ctx, root, d)?);
            match to_number(&v) {
                Some(n) => Ok(Res::Val(num_value(-n))),
                None => Ok(Res::Undef),
            }
        }
        Ast::Binary(op, lhs, rhs) => {
            let l = scalar(eval(lhs, ctx, root, d)?);
            let r = scalar(eval(rhs, ctx, root, d)?);
            eval_binary(*op, l, r)
        }
    }
}

fn eval_binary(op: Op, l: Option<Value>, r: Option<Value>) -> Result<Res, String> {
    match op {
        Op::Concat => {
            let s = format!(
                "{}{}",
                l.as_ref().map(stringify).unwrap_or_default(),
                r.as_ref().map(stringify).unwrap_or_default()
            );
            Ok(Res::Val(Value::String(s)))
        }
        Op::Add | Op::Sub | Op::Mul | Op::Div => {
            match (to_number(&l), to_number(&r)) {
                (Some(a), Some(b)) => {
                    let out = match op {
                        Op::Add => a + b,
                        Op::Sub => a - b,
                        Op::Mul => a * b,
                        Op::Div => a / b,
                        _ => unreachable!(),
                    };
                    Ok(Res::Val(num_value(out)))
                }
                _ => Ok(Res::Undef),
            }
        }
        Op::Eq | Op::Ne => {
            let eq = json_eq(&l, &r);
            let b = if op == Op::Eq { eq } else { !eq };
            Ok(Res::Val(Value::Bool(b)))
        }
        Op::Lt | Op::Gt | Op::Le | Op::Ge => {
            let ord = compare(&l, &r);
            let b = match ord {
                Some(o) => match op {
                    Op::Lt => o == std::cmp::Ordering::Less,
                    Op::Gt => o == std::cmp::Ordering::Greater,
                    Op::Le => o != std::cmp::Ordering::Greater,
                    Op::Ge => o != std::cmp::Ordering::Less,
                    _ => unreachable!(),
                },
                None => false,
            };
            Ok(Res::Val(Value::Bool(b)))
        }
    }
}

fn eval_func(name: &str, args: &[Ast], ctx: &Res, root: &Value, depth: usize) -> Result<Res, String> {
    // Every supported aggregate takes exactly one argument.
    let arg = args
        .first()
        .ok_or_else(|| format!("${name} expects one argument"))?;
    if args.len() > 1 {
        return Err(format!("${name} expects a single argument"));
    }
    let res = eval(arg, ctx, root, depth)?;
    let items = expand(&res);
    match name {
        "count" => Ok(Res::Val(num_value(items.len() as f64))),
        "sum" | "min" | "max" | "average" => {
            let nums: Vec<f64> = items.iter().filter_map(|v| to_number(&Some(v.clone()))).collect();
            if nums.is_empty() {
                // $sum of nothing is 0 in JSONata; the others are undefined.
                return Ok(if name == "sum" {
                    Res::Val(num_value(0.0))
                } else {
                    Res::Undef
                });
            }
            let out = match name {
                "sum" => nums.iter().sum(),
                "min" => nums.iter().cloned().fold(f64::INFINITY, f64::min),
                "max" => nums.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                "average" => nums.iter().sum::<f64>() / nums.len() as f64,
                _ => unreachable!(),
            };
            Ok(Res::Val(num_value(out)))
        }
        other => Err(format!(
            "unknown function '${other}' (supported: $sum, $count, $min, $max, $average)"
        )),
    }
}

// ----------------------------------------------------------------------------
// Value helpers
// ----------------------------------------------------------------------------

/// Build a JSON number, preferring an integer representation for whole values so
/// `$sum` of `2,3,5` renders as `10`, not `10.0`.
fn num_value(f: f64) -> Value {
    if f.is_finite() && f.fract() == 0.0 && f.abs() < 9_007_199_254_740_992.0 {
        Value::from(f as i64)
    } else {
        serde_json::Number::from_f64(f)
            .map(Value::Number)
            .unwrap_or(Value::Null)
    }
}

fn to_number(v: &Option<Value>) -> Option<f64> {
    match v {
        Some(Value::Number(n)) => n.as_f64(),
        _ => None,
    }
}

fn stringify(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Number(n) => n
            .as_i64()
            .map(|i| i.to_string())
            .unwrap_or_else(|| n.to_string()),
        Value::Bool(b) => b.to_string(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn truthy(v: &Option<Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(a)) => !a.is_empty(),
        Some(Value::Object(o)) => !o.is_empty(),
    }
}

fn json_eq(l: &Option<Value>, r: &Option<Value>) -> bool {
    match (l, r) {
        (Some(a), Some(b)) => a == b,
        (None, None) => true,
        _ => false,
    }
}

fn compare(l: &Option<Value>, r: &Option<Value>) -> Option<std::cmp::Ordering> {
    match (l, r) {
        (Some(Value::Number(a)), Some(Value::Number(b))) => {
            a.as_f64()?.partial_cmp(&b.as_f64()?)
        }
        (Some(Value::String(a)), Some(Value::String(b))) => Some(a.cmp(b)),
        _ => None,
    }
}

/// Index a sequence with an integer (negatives count from the end), returning the
/// single item or `Undef`.
fn index_seq(items: &[Value], idx: f64) -> Res {
    let len = items.len() as isize;
    let mut i = idx.trunc() as isize;
    if i < 0 {
        i += len;
    }
    if i >= 0 && i < len {
        Res::Val(items[i as usize].clone())
    } else {
        Res::Undef
    }
}
