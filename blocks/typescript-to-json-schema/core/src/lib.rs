//! gizza-ai/typescript-to-json-schema core — turn a pasted TypeScript interface,
//! type alias or enum into an equivalent JSON Schema. Pure: no wafer/wasm-bindgen
//! deps, `serde_json` only (with `preserve_order` so emitted keys keep source order).
//!
//! Deliberately a *practical subset* of TypeScript, not a type checker: object
//! literals, primitives, arrays, tuples, optional members, literal unions, enums,
//! index signatures, `extends`/`&` merges and references between locally declared
//! types. Anything needing real type resolution (generics, utility/mapped types,
//! imports, functions) is rejected with a line-numbered message naming the
//! construct rather than silently mis-converted.

use serde_json::{json, Map, Value};

const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Draft {
    Draft2020,
    Draft07,
}

impl Draft {
    fn schema_uri(self) -> &'static str {
        match self {
            Draft::Draft2020 => "https://json-schema.org/draft/2020-12/schema",
            Draft::Draft07 => "http://json-schema.org/draft-07/schema#",
        }
    }
    fn defs_key(self) -> &'static str {
        match self {
            Draft::Draft2020 => "$defs",
            Draft::Draft07 => "definitions",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    /// JSON Schema dialect to emit.
    pub draft: Draft,
    /// Name of the declaration to use as the schema root; empty = the first one.
    pub root_type: String,
    /// Emit a `required` array listing non-optional members.
    pub required: bool,
    /// Allow properties beyond those declared (omit `additionalProperties: false`).
    pub additional_properties: bool,
    /// Read `/** … */` JSDoc comments for descriptions and constraint annotations.
    pub jsdoc: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            draft: Draft::Draft2020,
            root_type: String::new(),
            required: true,
            additional_properties: false,
            jsdoc: true,
        }
    }
}

// ---------------------------------------------------------------- tokenizer

#[derive(Debug, Clone, PartialEq)]
enum Kind {
    Ident(String),
    Str(String),
    Num(String),
    Punct(char),
    Doc(String),
}

#[derive(Debug, Clone)]
struct Tok {
    kind: Kind,
    line: usize,
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' || c == '$'
}
fn is_ident_part(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '$'
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let chars: Vec<char> = src.chars().collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;
    while i < chars.len() {
        let c = chars[i];
        if c == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if c.is_whitespace() {
            i += 1;
            continue;
        }
        // comments
        if c == '/' && i + 1 < chars.len() {
            if chars[i + 1] == '/' {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                continue;
            }
            if chars[i + 1] == '*' {
                let doc = i + 2 < chars.len() && chars[i + 2] == '*';
                let start_line = line;
                let mut body = String::new();
                i += if doc { 3 } else { 2 };
                let mut closed = false;
                while i < chars.len() {
                    if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '/' {
                        i += 2;
                        closed = true;
                        break;
                    }
                    if chars[i] == '\n' {
                        line += 1;
                    }
                    body.push(chars[i]);
                    i += 1;
                }
                if !closed {
                    return Err(format!("line {start_line}: unterminated block comment"));
                }
                if doc {
                    out.push(Tok {
                        kind: Kind::Doc(body),
                        line: start_line,
                    });
                }
                continue;
            }
        }
        // strings (template literals without substitutions are treated as strings)
        if c == '"' || c == '\'' || c == '`' {
            let quote = c;
            let start_line = line;
            let mut s = String::new();
            i += 1;
            let mut closed = false;
            while i < chars.len() {
                let ch = chars[i];
                if ch == '\\' && i + 1 < chars.len() {
                    let esc = chars[i + 1];
                    s.push(match esc {
                        'n' => '\n',
                        't' => '\t',
                        'r' => '\r',
                        '0' => '\0',
                        other => other,
                    });
                    i += 2;
                    continue;
                }
                if ch == quote {
                    i += 1;
                    closed = true;
                    break;
                }
                if ch == '\n' {
                    line += 1;
                }
                if ch == '$' && quote == '`' && i + 1 < chars.len() && chars[i + 1] == '{' {
                    return Err(format!(
                        "line {start_line}: template literal types with substitutions are not supported"
                    ));
                }
                s.push(ch);
                i += 1;
            }
            if !closed {
                return Err(format!("line {start_line}: unterminated string literal"));
            }
            out.push(Tok {
                kind: Kind::Str(s),
                line: start_line,
            });
            continue;
        }
        if c.is_ascii_digit() {
            let start = i;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || chars[i] == '.' || chars[i] == '_')
            {
                i += 1;
            }
            let raw: String = chars[start..i].iter().filter(|c| **c != '_').collect();
            out.push(Tok {
                kind: Kind::Num(raw),
                line,
            });
            continue;
        }
        if is_ident_start(c) {
            let start = i;
            while i < chars.len() && is_ident_part(chars[i]) {
                i += 1;
            }
            out.push(Tok {
                kind: Kind::Ident(chars[start..i].iter().collect()),
                line,
            });
            continue;
        }
        out.push(Tok {
            kind: Kind::Punct(c),
            line,
        });
        i += 1;
    }
    Ok(out)
}

// ------------------------------------------------------------------- AST

#[derive(Debug, Clone, PartialEq)]
enum Lit {
    Str(String),
    Num(f64),
    Bool(bool),
}

#[derive(Debug, Clone, PartialEq)]
enum Ty {
    /// A JSON Schema primitive type name.
    Prim(&'static str),
    /// `any` / `unknown` / `object` — `object` keeps its type keyword.
    Any,
    Never,
    Undefined,
    Date,
    Lit(Lit),
    Array(Box<Ty>),
    Tuple(Vec<Ty>),
    Object(Obj),
    Union(Vec<Ty>),
    Intersection(Vec<Ty>),
    Record(Box<Ty>),
    Ref(String),
}

#[derive(Debug, Clone, PartialEq, Default)]
struct Obj {
    props: Vec<Prop>,
    /// `[k: string]: T` → additionalProperties
    index: Option<Box<Ty>>,
    /// `interface X extends A, B`
    extends: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct Prop {
    name: String,
    ty: Ty,
    optional: bool,
    doc: Option<String>,
}

#[derive(Debug, Clone)]
struct Decl {
    name: String,
    ty: Ty,
    doc: Option<String>,
}

// ------------------------------------------------------------------ parser

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn new(toks: Vec<Tok>) -> Self {
        Parser {
            toks,
            pos: 0,
            depth: 0,
        }
    }
    fn line(&self) -> usize {
        self.toks
            .get(self.pos)
            .or_else(|| self.toks.last())
            .map(|t| t.line)
            .unwrap_or(1)
    }
    fn peek(&self) -> Option<&Kind> {
        self.toks.get(self.pos).map(|t| &t.kind)
    }
    fn next(&mut self) -> Option<Kind> {
        let k = self.toks.get(self.pos).map(|t| t.kind.clone());
        if k.is_some() {
            self.pos += 1;
        }
        k
    }
    fn at_punct(&self, c: char) -> bool {
        matches!(self.peek(), Some(Kind::Punct(p)) if *p == c)
    }
    fn eat_punct(&mut self, c: char) -> bool {
        if self.at_punct(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn expect_punct(&mut self, c: char) -> Result<(), String> {
        if self.eat_punct(c) {
            Ok(())
        } else {
            Err(format!("line {}: expected '{}' {}", self.line(), c, self.saw()))
        }
    }
    fn at_ident(&self, s: &str) -> bool {
        matches!(self.peek(), Some(Kind::Ident(i)) if i == s)
    }
    fn eat_ident(&mut self, s: &str) -> bool {
        if self.at_ident(s) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    /// Consume a pending JSDoc comment, if any.
    fn take_doc(&mut self) -> Option<String> {
        let mut doc = None;
        while let Some(Kind::Doc(d)) = self.peek() {
            doc = Some(d.clone());
            self.pos += 1;
        }
        doc
    }
    fn saw(&self) -> String {
        match self.peek() {
            None => "but reached the end of the input".to_string(),
            Some(Kind::Ident(i)) => format!("but found `{i}`"),
            Some(Kind::Str(s)) => format!("but found the string \"{s}\""),
            Some(Kind::Num(n)) => format!("but found `{n}`"),
            Some(Kind::Punct(p)) => format!("but found `{p}`"),
            Some(Kind::Doc(_)) => "but found a comment".to_string(),
        }
    }
    fn expect_name(&mut self) -> Result<String, String> {
        let line = self.line();
        match self.next() {
            Some(Kind::Ident(i)) => Ok(i),
            _ => Err(format!("line {line}: expected a type name")),
        }
    }
    fn reject_generics(&mut self, what: &str) -> Result<(), String> {
        if self.at_punct('<') {
            return Err(format!(
                "line {}: generic type parameters are not supported ({what}); \
                 paste a concrete, already-instantiated type instead",
                self.line()
            ));
        }
        Ok(())
    }

    // ---- declarations

    fn parse_program(&mut self) -> Result<Vec<Decl>, String> {
        let mut decls = Vec::new();
        loop {
            let doc = self.take_doc();
            if self.peek().is_none() {
                break;
            }
            // stray separators between declarations
            if self.eat_punct(';') || self.eat_punct(',') {
                continue;
            }
            // A bare type literal / union as the whole input.
            if decls.is_empty() && (self.at_punct('{') || self.at_punct('[')) {
                let ty = self.parse_type()?;
                decls.push(Decl {
                    name: "Root".to_string(),
                    ty,
                    doc,
                });
                self.eat_punct(';');
                continue;
            }
            // modifiers
            while self.eat_ident("export")
                || self.eat_ident("declare")
                || self.eat_ident("default")
                || self.eat_ident("abstract")
            {}
            if self.at_ident("import") || self.at_ident("require") || self.at_ident("from") {
                return Err(format!(
                    "line {}: cross-file imports are not supported — paste every type this schema \
                     needs into a single input",
                    self.line()
                ));
            }
            for kw in ["class", "function", "namespace", "module"] {
                if self.at_ident(kw) {
                    return Err(format!(
                        "line {}: `{kw}` declarations are not supported — this tool converts \
                         `interface`, `type` and `enum` declarations",
                        self.line()
                    ));
                }
            }
            if self.at_ident("interface") {
                decls.push(self.parse_interface(doc)?);
                continue;
            }
            if self.at_ident("type") {
                decls.push(self.parse_type_alias(doc)?);
                continue;
            }
            if self.at_ident("enum") || (self.at_ident("const") && self.peek_ident_at(1, "enum")) {
                decls.push(self.parse_enum(doc)?);
                continue;
            }
            return Err(format!(
                "line {}: expected an `interface`, `type` or `enum` declaration {}",
                self.line(),
                self.saw()
            ));
        }
        if decls.is_empty() {
            return Err("no TypeScript declarations found — paste an `interface`, `type` or `enum`"
                .to_string());
        }
        Ok(decls)
    }

    fn peek_ident_at(&self, off: usize, s: &str) -> bool {
        matches!(self.toks.get(self.pos + off).map(|t| &t.kind), Some(Kind::Ident(i)) if i == s)
    }

    fn parse_interface(&mut self, doc: Option<String>) -> Result<Decl, String> {
        self.pos += 1; // `interface`
        let name = self.expect_name()?;
        self.reject_generics("interfaces must be non-generic")?;
        let mut extends = Vec::new();
        if self.eat_ident("extends") {
            loop {
                let base = self.expect_name()?;
                self.reject_generics("generic base types cannot be resolved")?;
                if self.at_punct('.') {
                    return Err(format!(
                        "line {}: qualified type names (`A.B`) are not supported",
                        self.line()
                    ));
                }
                extends.push(base);
                if !self.eat_punct(',') {
                    break;
                }
            }
        }
        if self.at_ident("implements") {
            return Err(format!(
                "line {}: `implements` clauses are not supported",
                self.line()
            ));
        }
        let mut obj = self.parse_object_body()?;
        obj.extends = extends;
        Ok(Decl {
            name,
            ty: Ty::Object(obj),
            doc,
        })
    }

    fn parse_type_alias(&mut self, doc: Option<String>) -> Result<Decl, String> {
        self.pos += 1; // `type`
        let name = self.expect_name()?;
        self.reject_generics("type aliases must be non-generic")?;
        self.expect_punct('=')?;
        let ty = self.parse_type()?;
        self.eat_punct(';');
        Ok(Decl { name, ty, doc })
    }

    fn parse_enum(&mut self, doc: Option<String>) -> Result<Decl, String> {
        self.eat_ident("const");
        self.pos += 1; // `enum`
        let name = self.expect_name()?;
        self.expect_punct('{')?;
        let mut members: Vec<Ty> = Vec::new();
        let mut auto = 0f64;
        while !self.at_punct('}') {
            self.take_doc();
            if self.eat_punct(',') || self.eat_punct(';') {
                continue;
            }
            if self.peek().is_none() {
                return Err(format!("line {}: unterminated enum body", self.line()));
            }
            let line = self.line();
            match self.next() {
                Some(Kind::Ident(_)) | Some(Kind::Str(_)) => {}
                _ => return Err(format!("line {line}: expected an enum member name")),
            }
            if self.eat_punct('=') {
                let lit = self.parse_literal()?;
                if let Lit::Num(n) = lit {
                    auto = n + 1.0;
                }
                members.push(Ty::Lit(lit));
            } else {
                members.push(Ty::Lit(Lit::Num(auto)));
                auto += 1.0;
            }
        }
        self.expect_punct('}')?;
        self.eat_punct(';');
        if members.is_empty() {
            return Err(format!("enum `{name}` has no members"));
        }
        let ty = if members.len() == 1 {
            members.remove(0)
        } else {
            Ty::Union(members)
        };
        Ok(Decl { name, ty, doc })
    }

    fn parse_literal(&mut self) -> Result<Lit, String> {
        let line = self.line();
        let neg = self.eat_punct('-');
        match self.next() {
            Some(Kind::Str(s)) if !neg => Ok(Lit::Str(s)),
            Some(Kind::Num(raw)) => {
                let n: f64 = raw
                    .parse()
                    .map_err(|_| format!("line {line}: `{raw}` is not a valid number literal"))?;
                Ok(Lit::Num(if neg { -n } else { n }))
            }
            Some(Kind::Ident(i)) if !neg && i == "true" => Ok(Lit::Bool(true)),
            Some(Kind::Ident(i)) if !neg && i == "false" => Ok(Lit::Bool(false)),
            _ => Err(format!(
                "line {line}: computed enum values are not supported — use a string or number literal"
            )),
        }
    }

    // ---- types

    fn parse_type(&mut self) -> Result<Ty, String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.depth -= 1;
            return Err(format!(
                "line {}: type nesting is deeper than the {MAX_DEPTH}-level limit",
                self.line()
            ));
        }
        let out = self.parse_union();
        self.depth -= 1;
        out
    }

    fn parse_union(&mut self) -> Result<Ty, String> {
        self.eat_punct('|'); // leading `|` in a multi-line union
        let mut parts = vec![self.parse_intersection()?];
        while self.eat_punct('|') {
            parts.push(self.parse_intersection()?);
        }
        Ok(if parts.len() == 1 {
            parts.remove(0)
        } else {
            Ty::Union(parts)
        })
    }

    fn parse_intersection(&mut self) -> Result<Ty, String> {
        self.eat_punct('&');
        let mut parts = vec![self.parse_postfix()?];
        while self.eat_punct('&') {
            parts.push(self.parse_postfix()?);
        }
        Ok(if parts.len() == 1 {
            parts.remove(0)
        } else {
            Ty::Intersection(parts)
        })
    }

    fn parse_postfix(&mut self) -> Result<Ty, String> {
        let mut ty = self.parse_primary()?;
        loop {
            if self.at_punct('[') {
                // `T[]` (array) vs `T["k"]` (indexed access — unsupported)
                if matches!(self.toks.get(self.pos + 1).map(|t| &t.kind), Some(Kind::Punct(']'))) {
                    self.pos += 2;
                    ty = Ty::Array(Box::new(ty));
                    continue;
                }
                return Err(format!(
                    "line {}: indexed access types (`T[\"key\"]`) are not supported",
                    self.line()
                ));
            }
            break;
        }
        Ok(ty)
    }

    /// `readonly` is a modifier only when a type/name follows it — a member
    /// literally called `readonly` (`readonly: boolean`) must stay a property.
    fn eat_readonly(&mut self) -> bool {
        if !self.at_ident("readonly") {
            return false;
        }
        let follows_type = matches!(
            self.toks.get(self.pos + 1).map(|t| &t.kind),
            Some(Kind::Ident(_)) | Some(Kind::Punct('[')) | Some(Kind::Punct('{'))
        );
        if follows_type {
            self.pos += 1;
        }
        follows_type
    }

    fn parse_primary(&mut self) -> Result<Ty, String> {
        while self.eat_readonly() {}
        let line = self.line();
        if self.at_punct('(') {
            self.pos += 1;
            let inner = self.parse_type()?;
            if self.at_punct(':') || self.at_punct('=') {
                return Err(format!(
                    "line {line}: function types are not expressible as JSON Schema"
                ));
            }
            self.expect_punct(')')?;
            if self.at_punct('=') {
                return Err(format!(
                    "line {line}: function types are not expressible as JSON Schema"
                ));
            }
            return Ok(inner);
        }
        if self.at_punct('{') {
            return Ok(Ty::Object(self.parse_object_body()?));
        }
        if self.at_punct('[') {
            self.pos += 1;
            let mut items = Vec::new();
            while !self.at_punct(']') {
                if self.peek().is_none() {
                    return Err(format!("line {line}: unterminated tuple type"));
                }
                if self.at_punct('.') {
                    return Err(format!(
                        "line {}: variadic tuple elements (`...T[]`) are not supported",
                        self.line()
                    ));
                }
                items.push(self.parse_type()?);
                if !self.eat_punct(',') {
                    break;
                }
            }
            self.expect_punct(']')?;
            return Ok(Ty::Tuple(items));
        }
        if self.at_punct('-') || matches!(self.peek(), Some(Kind::Str(_)) | Some(Kind::Num(_))) {
            return Ok(Ty::Lit(self.parse_literal()?));
        }
        let name = match self.next() {
            Some(Kind::Ident(i)) => i,
            _ => {
                self.pos = self.pos.saturating_sub(1);
                return Err(format!("line {line}: expected a type {}", self.saw()));
            }
        };
        if self.at_punct('.') {
            return Err(format!(
                "line {line}: qualified type names (`A.B`) are not supported"
            ));
        }
        match name.as_str() {
            "string" => return Ok(Ty::Prim("string")),
            "number" => return Ok(Ty::Prim("number")),
            "bigint" => return Ok(Ty::Prim("integer")),
            "boolean" => return Ok(Ty::Prim("boolean")),
            "null" => return Ok(Ty::Prim("null")),
            "true" => return Ok(Ty::Lit(Lit::Bool(true))),
            "false" => return Ok(Ty::Lit(Lit::Bool(false))),
            "undefined" | "void" => return Ok(Ty::Undefined),
            "any" | "unknown" => return Ok(Ty::Any),
            "never" => return Ok(Ty::Never),
            "object" => return Ok(Ty::Prim("object")),
            "Date" => return Ok(Ty::Date),
            "Array" | "ReadonlyArray" => {
                self.expect_punct('<')?;
                let inner = self.parse_type()?;
                self.expect_close_angle()?;
                return Ok(Ty::Array(Box::new(inner)));
            }
            "Record" => {
                self.expect_punct('<')?;
                let key = self.parse_type()?;
                if key != Ty::Prim("string") {
                    return Err(format!(
                        "line {line}: only `Record<string, T>` is supported — other key types need \
                         a TypeScript type checker"
                    ));
                }
                self.expect_punct(',')?;
                let val = self.parse_type()?;
                self.expect_close_angle()?;
                return Ok(Ty::Record(Box::new(val)));
            }
            "Partial" | "Required" | "Readonly" | "Pick" | "Omit" | "Exclude" | "Extract"
            | "NonNullable" | "ReturnType" | "Parameters" | "Awaited" | "Promise" | "Map"
            | "Set" | "keyof" | "typeof" | "infer" => {
                return Err(format!(
                    "line {line}: `{name}` needs TypeScript type resolution, which this converter \
                     does not do — paste the resulting concrete type instead"
                ))
            }
            _ => {}
        }
        if self.at_punct('<') {
            return Err(format!(
                "line {line}: generic type arguments (`{name}<…>`) are not supported — paste the \
                 concrete, already-instantiated type instead"
            ));
        }
        Ok(Ty::Ref(name))
    }

    /// Close a `<…>` type-argument list. The tokenizer emits `>` as a plain punct.
    fn expect_close_angle(&mut self) -> Result<(), String> {
        self.expect_punct('>')
    }

    fn parse_object_body(&mut self) -> Result<Obj, String> {
        self.expect_punct('{')?;
        let mut obj = Obj::default();
        loop {
            let doc = self.take_doc();
            if self.at_punct('}') {
                break;
            }
            if self.peek().is_none() {
                return Err(format!("line {}: unterminated object type", self.line()));
            }
            if self.eat_punct(';') || self.eat_punct(',') {
                continue;
            }
            let line = self.line();
            self.eat_readonly();
            // index signature: [key: string]: T
            if self.at_punct('[') {
                self.pos += 1;
                let _key_name = self.expect_name()?;
                self.expect_punct(':')?;
                let key_ty = self.parse_type()?;
                self.expect_punct(']')?;
                self.expect_punct(':')?;
                let val = self.parse_type()?;
                if key_ty != Ty::Prim("string") {
                    return Err(format!(
                        "line {line}: only string index signatures (`[k: string]: T`) are supported"
                    ));
                }
                obj.index = Some(Box::new(val));
                self.eat_punct(';');
                self.eat_punct(',');
                continue;
            }
            let name = match self.next() {
                Some(Kind::Ident(i)) => i,
                Some(Kind::Str(s)) => s,
                Some(Kind::Num(n)) => n,
                _ => return Err(format!("line {line}: expected a property name")),
            };
            if self.at_punct('(') || self.at_punct('<') {
                return Err(format!(
                    "line {line}: method signatures (`{name}(…)`) are not expressible as JSON Schema"
                ));
            }
            let optional = self.eat_punct('?');
            self.expect_punct(':')?;
            let ty = self.parse_type()?;
            self.eat_punct(';');
            self.eat_punct(',');
            obj.props.push(Prop {
                name,
                ty,
                optional,
                doc,
            });
        }
        self.expect_punct('}')?;
        Ok(obj)
    }
}

// ------------------------------------------------------------------ JSDoc

#[derive(Debug, Clone, Default)]
struct Doc {
    description: Option<String>,
    tags: Vec<(String, String)>,
}

fn parse_doc(raw: &str) -> Doc {
    let mut description_lines: Vec<String> = Vec::new();
    let mut tags: Vec<(String, String)> = Vec::new();
    for line in raw.lines() {
        let line = line.trim().trim_start_matches('*').trim();
        if let Some(rest) = line.strip_prefix('@') {
            // A line may chain flag tags before a valued one: `@deprecated @default 3`.
            let mut rest = rest.to_string();
            loop {
                let mut it = rest.splitn(2, char::is_whitespace);
                let tag = it.next().unwrap_or("").to_string();
                let val = it.next().unwrap_or("").trim().to_string();
                if tag.is_empty() {
                    break;
                }
                match val.strip_prefix('@') {
                    Some(more) => {
                        tags.push((tag, String::new()));
                        rest = more.to_string();
                    }
                    None => {
                        tags.push((tag, val));
                        break;
                    }
                }
            }
        } else if !tags.is_empty() {
            // continuation of the previous tag value
            if let Some(last) = tags.last_mut() {
                if !line.is_empty() {
                    if !last.1.is_empty() {
                        last.1.push(' ');
                    }
                    last.1.push_str(line);
                }
            }
        } else {
            description_lines.push(line.to_string());
        }
    }
    let description = description_lines
        .join("\n")
        .trim()
        .trim_matches('\n')
        .to_string();
    Doc {
        description: if description.is_empty() {
            None
        } else {
            Some(description)
        },
        tags,
    }
}

/// Apply JSDoc description + constraint annotations onto an already-built schema.
fn apply_doc(schema: &mut Value, raw: &str) {
    let doc = parse_doc(raw);
    let Some(map) = schema.as_object_mut() else {
        return;
    };
    if let Some(d) = doc.description {
        map.insert("description".into(), Value::String(d));
    }
    let mut nullable = false;
    for (tag, val) in &doc.tags {
        let v = val.as_str();
        match tag.as_str() {
            "title" | "format" | "pattern" | "contentEncoding" | "contentMediaType" => {
                if !v.is_empty() {
                    map.insert(tag.clone(), Value::String(v.to_string()));
                }
            }
            "description" => {
                if !v.is_empty() {
                    map.insert("description".into(), Value::String(v.to_string()));
                }
            }
            "minimum" | "maximum" | "exclusiveMinimum" | "exclusiveMaximum" | "multipleOf"
            | "minLength" | "maxLength" | "minItems" | "maxItems" | "minProperties"
            | "maxProperties" => {
                if let Ok(n) = v.parse::<f64>() {
                    map.insert(tag.clone(), num_value(n));
                }
            }
            "uniqueItems" | "readOnly" | "writeOnly" => {
                map.insert(tag.clone(), Value::Bool(v.is_empty() || v == "true"));
            }
            "deprecated" => {
                map.insert("deprecated".into(), Value::Bool(true));
            }
            "default" | "example" | "examples" => {
                let parsed = serde_json::from_str::<Value>(v)
                    .unwrap_or_else(|_| Value::String(v.to_string()));
                if tag == "default" {
                    map.insert("default".into(), parsed);
                } else if let Some(a) = map
                    .entry("examples")
                    .or_insert_with(|| Value::Array(Vec::new()))
                    .as_array_mut()
                {
                    a.push(parsed);
                }
            }
            "nullable" => nullable = v.is_empty() || v == "true",
            // `@asType` (and the older `@TJS-type`) override the emitted JSON type.
            "asType" | "TJS-type" => {
                if !v.is_empty() {
                    map.insert("type".into(), Value::String(v.to_string()));
                }
            }
            _ => {}
        }
    }
    if nullable {
        let widened = match map.get("type") {
            Some(Value::String(t)) => Some(json!([t.clone(), "null"])),
            Some(Value::Array(a)) if !a.iter().any(|v| v == "null") => {
                let mut a = a.clone();
                a.push(Value::String("null".into()));
                Some(Value::Array(a))
            }
            _ => None,
        };
        if let Some(w) = widened {
            map.insert("type".into(), w);
        }
    }
}

// ------------------------------------------------------------------ lowering

struct Ctx<'a> {
    decls: &'a [Decl],
    opts: &'a Options,
    /// Named types actually reached from the root, in discovery order.
    used: Vec<String>,
}

impl<'a> Ctx<'a> {
    fn find(&self, name: &str) -> Option<&'a Decl> {
        self.decls.iter().find(|d| d.name == name)
    }

    fn mark_used(&mut self, name: &str) {
        if !self.used.iter().any(|n| n == name) {
            self.used.push(name.to_string());
        }
    }

    /// Flatten `extends` / object intersections into one object.
    fn merge_object(&mut self, obj: &Obj, seen: &mut Vec<String>) -> Result<Obj, String> {
        let mut out = Obj::default();
        for base in &obj.extends {
            if seen.iter().any(|s| s == base) {
                return Err(format!(
                    "type `{base}` extends itself — circular `extends` chains cannot be merged"
                ));
            }
            let decl = self.find(base).ok_or_else(|| {
                format!("unknown base type `{base}` — declare it in the same input or remove the `extends`")
            })?;
            let Ty::Object(parent) = decl.ty.clone() else {
                return Err(format!(
                    "`extends {base}` is not supported: `{base}` is not an object type"
                ));
            };
            seen.push(base.clone());
            let merged = self.merge_object(&parent, seen)?;
            seen.pop();
            for p in merged.props {
                out.props.retain(|e| e.name != p.name);
                out.props.push(p);
            }
            if merged.index.is_some() {
                out.index = merged.index;
            }
        }
        for p in &obj.props {
            out.props.retain(|e| e.name != p.name);
            out.props.push(p.clone());
        }
        if obj.index.is_some() {
            out.index = obj.index.clone();
        }
        Ok(out)
    }

    /// If every operand of an intersection resolves to an object type, merge them.
    fn try_merge_intersection(&mut self, parts: &[Ty]) -> Result<Option<Obj>, String> {
        let mut objs: Vec<Obj> = Vec::new();
        for p in parts {
            match p {
                Ty::Object(o) => objs.push(o.clone()),
                Ty::Ref(name) => match self.find(name).map(|d| d.ty.clone()) {
                    Some(Ty::Object(o)) => objs.push(o),
                    _ => return Ok(None),
                },
                _ => return Ok(None),
            }
        }
        let mut out = Obj::default();
        for o in &objs {
            let merged = self.merge_object(o, &mut Vec::new())?;
            for p in merged.props {
                out.props.retain(|e| e.name != p.name);
                out.props.push(p);
            }
            if merged.index.is_some() {
                out.index = merged.index;
            }
        }
        Ok(Some(out))
    }

    fn ref_to(&mut self, name: &str) -> Value {
        self.mark_used(name);
        json!({ "$ref": format!("#/{}/{}", self.opts.draft.defs_key(), name) })
    }

    fn lower(&mut self, ty: &Ty, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "schema nesting is deeper than the {MAX_DEPTH}-level limit"
            ));
        }
        Ok(match ty {
            Ty::Prim(t) => json!({ "type": t }),
            Ty::Any => json!({}),
            Ty::Never => json!({ "not": {} }),
            // A bare `undefined`/`void` has no JSON counterpart; the nearest honest
            // rendering is "nothing is valid here".
            Ty::Undefined => json!({ "not": {} }),
            Ty::Date => json!({ "type": "string", "format": "date-time" }),
            Ty::Lit(l) => {
                let (t, v) = lit_parts(l);
                json!({ "type": t, "const": v })
            }
            Ty::Array(inner) => json!({
                "type": "array",
                "items": self.lower(inner, depth + 1)?,
            }),
            Ty::Tuple(items) => {
                let lowered: Result<Vec<Value>, String> =
                    items.iter().map(|t| self.lower(t, depth + 1)).collect();
                let lowered = lowered?;
                let n = lowered.len();
                let mut m = Map::new();
                m.insert("type".into(), json!("array"));
                match self.opts.draft {
                    Draft::Draft2020 => {
                        m.insert("prefixItems".into(), Value::Array(lowered));
                        m.insert("items".into(), Value::Bool(false));
                    }
                    Draft::Draft07 => {
                        m.insert("items".into(), Value::Array(lowered));
                        m.insert("additionalItems".into(), Value::Bool(false));
                    }
                }
                m.insert("minItems".into(), json!(n));
                m.insert("maxItems".into(), json!(n));
                Value::Object(m)
            }
            Ty::Record(inner) => json!({
                "type": "object",
                "additionalProperties": self.lower(inner, depth + 1)?,
            }),
            Ty::Ref(name) => {
                if self.find(name).is_none() {
                    return Err(format!(
                        "unknown type `{name}` — declare it in the same input (this converter does \
                         not follow imports)"
                    ));
                }
                self.ref_to(name)
            }
            Ty::Object(obj) => {
                let merged = self.merge_object(obj, &mut Vec::new())?;
                self.lower_object(&merged, depth)?
            }
            Ty::Intersection(parts) => {
                if let Some(obj) = self.try_merge_intersection(parts)? {
                    self.lower_object(&obj, depth)?
                } else {
                    let lowered: Result<Vec<Value>, String> =
                        parts.iter().map(|t| self.lower(t, depth + 1)).collect();
                    json!({ "allOf": lowered? })
                }
            }
            Ty::Union(parts) => self.lower_union(parts, depth)?,
        })
    }

    fn lower_object(&mut self, obj: &Obj, depth: usize) -> Result<Value, String> {
        let mut props = Map::new();
        let mut required: Vec<Value> = Vec::new();
        for p in &obj.props {
            // `a?: string` and `a: string | undefined` both mean "may be absent".
            let (ty, undef) = strip_undefined(&p.ty);
            let mut schema = self.lower(&ty, depth + 1)?;
            if self.opts.jsdoc {
                if let Some(raw) = &p.doc {
                    apply_doc(&mut schema, raw);
                }
            }
            if !p.optional && !undef {
                required.push(Value::String(p.name.clone()));
            }
            props.insert(p.name.clone(), schema);
        }
        let mut m = Map::new();
        m.insert("type".into(), json!("object"));
        m.insert("properties".into(), Value::Object(props));
        if self.opts.required && !required.is_empty() {
            m.insert("required".into(), Value::Array(required));
        }
        if let Some(idx) = &obj.index {
            let v = self.lower(idx, depth + 1)?;
            m.insert("additionalProperties".into(), v);
        } else if !self.opts.additional_properties {
            m.insert("additionalProperties".into(), Value::Bool(false));
        }
        Ok(Value::Object(m))
    }

    fn lower_union(&mut self, parts: &[Ty], depth: usize) -> Result<Value, String> {
        let mut flat: Vec<Ty> = Vec::new();
        flatten_union(parts, &mut flat);
        // `T | undefined` at a non-property position degrades to just `T`.
        flat.retain(|t| !matches!(t, Ty::Undefined));
        if flat.is_empty() {
            return Ok(json!({ "not": {} }));
        }
        if flat.len() == 1 {
            return self.lower(&flat[0], depth);
        }
        // All literals → `enum` (with a single `type` when they agree).
        if flat.iter().all(|t| matches!(t, Ty::Lit(_))) {
            let mut types: Vec<&'static str> = Vec::new();
            let mut values: Vec<Value> = Vec::new();
            for t in &flat {
                if let Ty::Lit(l) = t {
                    let (ty, v) = lit_parts(l);
                    if !types.contains(&ty) {
                        types.push(ty);
                    }
                    if !values.contains(&v) {
                        values.push(v);
                    }
                }
            }
            let mut m = Map::new();
            if types.len() == 1 {
                m.insert("type".into(), json!(types[0]));
            }
            m.insert("enum".into(), Value::Array(values));
            return Ok(Value::Object(m));
        }
        // All bare primitives (incl. `null`) → a `type` array.
        if flat.iter().all(|t| matches!(t, Ty::Prim(_))) {
            let mut types: Vec<Value> = Vec::new();
            for t in &flat {
                if let Ty::Prim(p) = t {
                    let v = json!(p);
                    if !types.contains(&v) {
                        types.push(v);
                    }
                }
            }
            if types.len() == 1 {
                return Ok(json!({ "type": types.remove(0) }));
            }
            return Ok(json!({ "type": types }));
        }
        let lowered: Result<Vec<Value>, String> =
            flat.iter().map(|t| self.lower(t, depth + 1)).collect();
        Ok(json!({ "anyOf": lowered? }))
    }
}

/// Whole numbers serialize as JSON integers (`200`, not `200.0`) so the emitted
/// schema reads like hand-written JSON Schema.
fn num_value(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.007_199_254_740_992e15 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

fn lit_parts(l: &Lit) -> (&'static str, Value) {
    match l {
        Lit::Str(s) => ("string", Value::String(s.clone())),
        Lit::Num(n) => (
            if n.fract() == 0.0 { "integer" } else { "number" },
            num_value(*n),
        ),
        Lit::Bool(b) => ("boolean", Value::Bool(*b)),
    }
}

fn flatten_union(parts: &[Ty], out: &mut Vec<Ty>) {
    for p in parts {
        match p {
            Ty::Union(inner) => flatten_union(inner, out),
            other => out.push(other.clone()),
        }
    }
}

/// Split `T | undefined` into (`T`, true).
fn strip_undefined(ty: &Ty) -> (Ty, bool) {
    let Ty::Union(parts) = ty else {
        return (ty.clone(), matches!(ty, Ty::Undefined));
    };
    let mut flat = Vec::new();
    flatten_union(parts, &mut flat);
    let had = flat.iter().any(|t| matches!(t, Ty::Undefined));
    if !had {
        return (ty.clone(), false);
    }
    flat.retain(|t| !matches!(t, Ty::Undefined));
    let inner = match flat.len() {
        0 => Ty::Never,
        1 => flat.remove(0),
        _ => Ty::Union(flat),
    };
    (inner, true)
}

// -------------------------------------------------------------------- API

/// Convert TypeScript source to a pretty-printed JSON Schema document.
pub fn convert(src: &str, opts: &Options) -> Result<String, String> {
    if src.trim().is_empty() {
        return Err("no TypeScript source provided".to_string());
    }
    let toks = tokenize(src)?;
    let decls = Parser::new(toks).parse_program()?;

    let root = if opts.root_type.trim().is_empty() {
        &decls[0]
    } else {
        let want = opts.root_type.trim();
        decls.iter().find(|d| d.name == want).ok_or_else(|| {
            let names: Vec<&str> = decls.iter().map(|d| d.name.as_str()).collect();
            format!(
                "root type `{want}` was not found — declared types are: {}",
                names.join(", ")
            )
        })?
    };

    let mut ctx = Ctx {
        decls: &decls,
        opts,
        used: Vec::new(),
    };
    let root_schema = ctx.lower(&root.ty, 0)?;

    // Expand the reachable set transitively; `used` grows while we walk it.
    let mut defs = Map::new();
    let mut i = 0usize;
    while i < ctx.used.len() {
        let name = ctx.used[i].clone();
        i += 1;
        if defs.contains_key(&name) {
            continue;
        }
        let decl = ctx
            .find(&name)
            .ok_or_else(|| format!("unknown type `{name}`"))?
            .clone();
        let mut schema = ctx.lower(&decl.ty, 0)?;
        if let Some(map) = schema.as_object_mut() {
            if !map.contains_key("$ref") {
                map.insert("title".into(), Value::String(name.clone()));
            }
        }
        if opts.jsdoc {
            if let Some(raw) = &decl.doc {
                apply_doc(&mut schema, raw);
            }
        }
        defs.insert(name, schema);
    }

    let mut out = Map::new();
    out.insert("$schema".into(), json!(opts.draft.schema_uri()));
    if root.name != "Root" || root.doc.is_some() {
        out.insert("title".into(), json!(root.name.clone()));
    }
    // Seed `description` before the body so the root doc reads at the top; the
    // apply_doc pass below overwrites it in place (insertion order is preserved).
    if opts.jsdoc {
        if let Some(d) = root.doc.as_deref().and_then(|raw| parse_doc(raw).description) {
            out.insert("description".into(), Value::String(d));
        }
    }
    if let Value::Object(m) = root_schema {
        for (k, v) in m {
            out.insert(k, v);
        }
    }
    if opts.jsdoc {
        if let Some(raw) = &root.doc {
            let mut tmp = Value::Object(out);
            apply_doc(&mut tmp, raw);
            out = tmp.as_object().cloned().unwrap_or_default();
        }
    }
    if !defs.is_empty() {
        // Sort defs by name so output is stable regardless of discovery order.
        let mut names: Vec<String> = defs.keys().cloned().collect();
        names.sort();
        let mut sorted = Map::new();
        for n in names {
            if let Some(v) = defs.get(&n) {
                sorted.insert(n, v.clone());
            }
        }
        out.insert(opts.draft.defs_key().into(), Value::Object(sorted));
    }

    serde_json::to_string_pretty(&Value::Object(out))
        .map_err(|e| format!("could not serialize the schema: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> Value {
        serde_json::from_str(&convert(src, &Options::default()).unwrap()).unwrap()
    }

    #[test]
    fn interface_with_optional_and_primitives() {
        let s = run("interface User { id: number; name: string; admin?: boolean; bio: string | null }");
        assert_eq!(s["$schema"], "https://json-schema.org/draft/2020-12/schema");
        assert_eq!(s["title"], "User");
        assert_eq!(s["type"], "object");
        assert_eq!(s["properties"]["id"]["type"], "number");
        assert_eq!(s["properties"]["name"]["type"], "string");
        assert_eq!(s["properties"]["admin"]["type"], "boolean");
        assert_eq!(s["properties"]["bio"]["type"], json!(["string", "null"]));
        assert_eq!(s["required"], json!(["id", "name", "bio"]));
        assert_eq!(s["additionalProperties"], json!(false));
    }

    #[test]
    fn properties_keep_source_order() {
        let s = run("interface Z { zeta: string; alpha: string; mid: string }");
        let keys: Vec<&String> = s["properties"].as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["zeta", "alpha", "mid"]);
    }

    #[test]
    fn arrays_in_both_spellings() {
        let s = run("interface A { a: string[]; b: Array<number>; c: readonly boolean[]; d: number[][] }");
        let p = &s["properties"];
        assert_eq!(p["a"], json!({"type":"array","items":{"type":"string"}}));
        assert_eq!(p["b"], json!({"type":"array","items":{"type":"number"}}));
        assert_eq!(p["c"], json!({"type":"array","items":{"type":"boolean"}}));
        assert_eq!(p["d"]["items"]["items"]["type"], "number");
    }

    #[test]
    fn string_literal_union_becomes_enum() {
        let s = run(r#"type Status = "open" | "closed" | "merged";"#);
        assert_eq!(s["type"], "string");
        assert_eq!(s["enum"], json!(["open", "closed", "merged"]));
    }

    #[test]
    fn numeric_literal_union_and_single_literal() {
        let s = run("interface A { code: 200 | 404 | 500; kind: \"a\" }");
        assert_eq!(s["properties"]["code"]["type"], "integer");
        assert_eq!(s["properties"]["code"]["enum"], json!([200, 404, 500]));
        assert_eq!(s["properties"]["kind"], json!({"type":"string","const":"a"}));
    }

    #[test]
    fn mixed_literal_union_drops_the_type_keyword() {
        let s = run(r#"type T = "a" | 1 | true;"#);
        assert!(s.get("type").is_none());
        assert_eq!(s["enum"], json!(["a", 1, true]));
    }

    #[test]
    fn nested_object_literals() {
        let s = run("interface P { user: { name: string; tags?: string[] } }");
        let u = &s["properties"]["user"];
        assert_eq!(u["type"], "object");
        assert_eq!(u["properties"]["name"]["type"], "string");
        assert_eq!(u["required"], json!(["name"]));
        assert_eq!(u["additionalProperties"], json!(false));
    }

    #[test]
    fn named_references_become_defs() {
        let s = run("interface Order { customer: Customer } interface Customer { id: string }");
        assert_eq!(s["properties"]["customer"]["$ref"], "#/$defs/Customer");
        assert_eq!(s["$defs"]["Customer"]["properties"]["id"]["type"], "string");
        assert_eq!(s["$defs"]["Customer"]["title"], "Customer");
    }

    #[test]
    fn unreachable_declarations_are_pruned() {
        let s = run("interface A { x: string } interface Unused { y: string }");
        assert!(s.get("$defs").is_none());
    }

    #[test]
    fn recursive_types_terminate() {
        let s = run("interface Node { value: string; children: Node[] }");
        assert_eq!(s["properties"]["children"]["items"]["$ref"], "#/$defs/Node");
        assert_eq!(s["$defs"]["Node"]["properties"]["value"]["type"], "string");
    }

    #[test]
    fn extends_merges_base_members() {
        let s = run(
            "interface Item extends Base { qty: number } interface Base { id: string; note?: string }",
        );
        let keys: Vec<&String> = s["properties"].as_object().unwrap().keys().collect();
        assert_eq!(keys, vec!["id", "note", "qty"]);
        assert_eq!(s["required"], json!(["id", "qty"]));
    }

    #[test]
    fn intersection_of_objects_merges() {
        let s = run("type T = { a: string } & { b: number };");
        assert_eq!(s["properties"]["a"]["type"], "string");
        assert_eq!(s["properties"]["b"]["type"], "number");
        assert!(s.get("allOf").is_none());
    }

    #[test]
    fn index_signature_and_record_become_additional_properties() {
        let s = run("interface A { m: { [key: string]: number }; r: Record<string, string> }");
        assert_eq!(
            s["properties"]["m"],
            json!({"type":"object","properties":{},"additionalProperties":{"type":"number"}})
        );
        assert_eq!(
            s["properties"]["r"],
            json!({"type":"object","additionalProperties":{"type":"string"}})
        );
    }

    #[test]
    fn tuples_use_prefix_items_in_2020_12() {
        let s = run("type Point = [number, string];");
        assert_eq!(s["prefixItems"][0]["type"], "number");
        assert_eq!(s["prefixItems"][1]["type"], "string");
        assert_eq!(s["items"], json!(false));
        assert_eq!(s["minItems"], json!(2));
    }

    #[test]
    fn tuples_use_items_array_in_draft_07() {
        let opts = Options {
            draft: Draft::Draft07,
            ..Options::default()
        };
        let s: Value =
            serde_json::from_str(&convert("type Point = [number, string];", &opts).unwrap()).unwrap();
        assert_eq!(s["$schema"], "http://json-schema.org/draft-07/schema#");
        assert_eq!(s["items"][1]["type"], "string");
        assert_eq!(s["additionalItems"], json!(false));
    }

    #[test]
    fn draft_07_uses_definitions_pointer() {
        let opts = Options {
            draft: Draft::Draft07,
            ..Options::default()
        };
        let s: Value = serde_json::from_str(
            &convert("interface A { b: B } interface B { c: string }", &opts).unwrap(),
        )
        .unwrap();
        assert_eq!(s["properties"]["b"]["$ref"], "#/definitions/B");
        assert!(s["definitions"]["B"].is_object());
    }

    #[test]
    fn string_enum_declaration() {
        let s = run(r#"enum Color { Red = "red", Green = "green" }"#);
        assert_eq!(s["type"], "string");
        assert_eq!(s["enum"], json!(["red", "green"]));
    }

    #[test]
    fn numeric_enum_auto_increments() {
        let s = run("enum Level { Low, Mid, High = 10, Higher }");
        assert_eq!(s["enum"], json!([0, 1, 10, 11]));
        assert_eq!(s["type"], "integer");
    }

    #[test]
    fn const_enum_is_accepted() {
        let s = run(r#"const enum E { A = "a" }"#);
        assert_eq!(s["const"], "a");
    }

    #[test]
    fn date_maps_to_date_time_string() {
        let s = run("interface A { at: Date }");
        assert_eq!(
            s["properties"]["at"],
            json!({"type":"string","format":"date-time"})
        );
    }

    #[test]
    fn any_and_unknown_are_open_schemas() {
        let s = run("interface A { a: any; b: unknown; c: never }");
        assert_eq!(s["properties"]["a"], json!({}));
        assert_eq!(s["properties"]["b"], json!({}));
        assert_eq!(s["properties"]["c"], json!({"not": {}}));
    }

    #[test]
    fn undefined_union_makes_a_property_optional() {
        let s = run("interface A { a: string | undefined; b: string }");
        assert_eq!(s["properties"]["a"]["type"], "string");
        assert_eq!(s["required"], json!(["b"]));
    }

    #[test]
    fn jsdoc_description_and_annotations() {
        let src = r#"
            /** A person. */
            interface Person {
              /**
               * Their age in years.
               * @minimum 0
               * @maximum 130
               * @asType integer
               */
              age: number;
              /** @format email */
              email: string;
              /** @deprecated @default "n/a" */
              nickname?: string;
            }
        "#;
        let s = run(src);
        assert_eq!(s["description"], "A person.");
        let age = &s["properties"]["age"];
        assert_eq!(age["description"], "Their age in years.");
        assert_eq!(age["minimum"], json!(0));
        assert_eq!(age["maximum"], json!(130));
        assert_eq!(age["type"], "integer");
        assert_eq!(s["properties"]["email"]["format"], "email");
        assert_eq!(s["properties"]["nickname"]["deprecated"], json!(true));
        assert_eq!(s["properties"]["nickname"]["default"], "n/a");
    }

    #[test]
    fn jsdoc_nullable_widens_the_type() {
        let s = run("interface A { /** @nullable */ a: string }");
        assert_eq!(s["properties"]["a"]["type"], json!(["string", "null"]));
    }

    #[test]
    fn jsdoc_can_be_turned_off() {
        let opts = Options {
            jsdoc: false,
            ..Options::default()
        };
        let s: Value = serde_json::from_str(
            &convert("interface A { /** hi @format email */ a: string }", &opts).unwrap(),
        )
        .unwrap();
        assert_eq!(s["properties"]["a"], json!({"type":"string"}));
    }

    #[test]
    fn root_type_selects_a_later_declaration() {
        let opts = Options {
            root_type: "Second".into(),
            ..Options::default()
        };
        let s: Value = serde_json::from_str(
            &convert("interface First { a: string } interface Second { b: number }", &opts).unwrap(),
        )
        .unwrap();
        assert_eq!(s["title"], "Second");
        assert!(s["properties"]["b"].is_object());
    }

    #[test]
    fn options_toggle_required_and_additional_properties() {
        let opts = Options {
            required: false,
            additional_properties: true,
            ..Options::default()
        };
        let s: Value =
            serde_json::from_str(&convert("interface A { a: string }", &opts).unwrap()).unwrap();
        assert!(s.get("required").is_none());
        assert!(s.get("additionalProperties").is_none());
    }

    #[test]
    fn bare_type_literal_is_accepted() {
        let s = run("{ a: string; b?: number }");
        assert_eq!(s["type"], "object");
        assert_eq!(s["required"], json!(["a"]));
        assert!(s.get("title").is_none());
    }

    #[test]
    fn export_and_comments_are_tolerated() {
        let s = run("// leading\nexport interface A { /* inline */ a: string; } // trailing");
        assert_eq!(s["properties"]["a"]["type"], "string");
    }

    #[test]
    fn quoted_and_numeric_property_names() {
        let s = run(r#"interface A { "content-type": string; 200: boolean }"#);
        assert_eq!(s["properties"]["content-type"]["type"], "string");
        assert_eq!(s["properties"]["200"]["type"], "boolean");
    }

    #[test]
    fn union_of_objects_uses_any_of() {
        let s = run(r#"type T = { kind: "a" } | { kind: "b"; n: number };"#);
        assert_eq!(s["anyOf"][0]["properties"]["kind"]["const"], "a");
        assert_eq!(s["anyOf"][1]["properties"]["n"]["type"], "number");
    }

    // ---- error paths

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", &Options::default())
            .unwrap_err()
            .contains("no TypeScript source"));
    }

    #[test]
    fn generics_are_rejected_with_a_line_number() {
        let e = convert("interface Box<T> { value: T }", &Options::default()).unwrap_err();
        assert!(e.contains("line 1"), "{e}");
        assert!(e.contains("generic type parameters"), "{e}");
    }

    #[test]
    fn utility_types_are_rejected_by_name() {
        let e = convert(
            "interface A { a: string }\ntype B = Partial<A>;",
            &Options::default(),
        )
        .unwrap_err();
        assert!(e.contains("line 2"), "{e}");
        assert!(e.contains("Partial"), "{e}");
    }

    #[test]
    fn imports_are_rejected_clearly() {
        let e = convert("import { A } from './a';", &Options::default()).unwrap_err();
        assert!(e.contains("imports are not supported"), "{e}");
    }

    #[test]
    fn methods_are_rejected() {
        let e = convert("interface A { go(): void }", &Options::default()).unwrap_err();
        assert!(e.contains("method signatures"), "{e}");
    }

    #[test]
    fn unknown_type_reference_is_reported() {
        let e = convert("interface A { b: Missing }", &Options::default()).unwrap_err();
        assert!(e.contains("unknown type `Missing`"), "{e}");
    }

    #[test]
    fn unknown_base_type_is_reported() {
        let e = convert("interface A extends Nope { a: string }", &Options::default()).unwrap_err();
        assert!(e.contains("unknown base type `Nope`"), "{e}");
    }

    #[test]
    fn missing_root_type_lists_declared_names() {
        let opts = Options {
            root_type: "Nope".into(),
            ..Options::default()
        };
        let e = convert("interface A { a: string }", &opts).unwrap_err();
        assert!(e.contains("declared types are: A"), "{e}");
    }

    #[test]
    fn unterminated_object_is_reported() {
        let e = convert("interface A { a: string", &Options::default()).unwrap_err();
        assert!(e.contains("unterminated object type"), "{e}");
    }

    #[test]
    fn non_declaration_input_is_reported() {
        let e = convert("const x = 5;", &Options::default()).unwrap_err();
        assert!(e.contains("expected an `interface`"), "{e}");
    }

    #[test]
    fn circular_extends_is_reported() {
        let e = convert(
            "interface A extends B { a: string } interface B extends A { b: string }",
            &Options::default(),
        )
        .unwrap_err();
        assert!(e.contains("extends itself"), "{e}");
    }
}
