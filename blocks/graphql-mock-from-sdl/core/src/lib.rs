//! graphql-mock-from-sdl core — parse a GraphQL SDL document and emit mock JSON
//! response data for every type it defines.
//!
//! Pure Rust (serde_json only, hand-rolled SDL parser) → runs on ALL backends
//! including the chat Service Worker. Deterministic: the same `seed` + inputs
//! always produce byte-identical output.
//!
//! In-model SDL subset: `type` / `interface` / `union` / `enum` / `input` /
//! `scalar` / `schema` definitions and their `extend` forms, descriptions
//! (`"…"` and `"""…"""`), comments, field arguments with default values,
//! directives, and arbitrarily nested list / non-null wrappers. Executable
//! definitions (`query`/`mutation` operation documents, fragments) are not part
//! of a schema document and are rejected with a clear error.

use serde_json::{Map, Number, Value};

/// Largest SDL document accepted (bytes).
pub const MAX_SDL_BYTES: usize = 200_000;
/// Largest `list_length` accepted.
pub const MAX_LIST_LENGTH: u32 = 10;
/// Largest `depth` accepted.
pub const MAX_DEPTH: u32 = 6;

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Name(String),
    /// A string / block-string literal (used for descriptions and default values).
    Str(String),
    /// A number literal (default values only).
    Num(String),
    Punct(char),
    /// `...` spread — only ever appears in executable documents; kept so the
    /// parser can produce a precise error instead of a generic one.
    Spread,
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut out = Vec::new();
    while i < b.len() {
        let c = b[i];
        // Commas are insignificant in GraphQL, exactly like whitespace.
        if c.is_whitespace() || c == ',' || c == '\u{feff}' {
            i += 1;
            continue;
        }
        if c == '#' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if c == '"' {
            // Block string """…""" or single-line string "…".
            if i + 2 < b.len() && b[i + 1] == '"' && b[i + 2] == '"' {
                i += 3;
                let mut s = String::new();
                loop {
                    if i >= b.len() {
                        return Err("unterminated block string (\"\"\") in SDL".into());
                    }
                    if b[i] == '\\' && i + 3 < b.len() && b[i + 1] == '"' && b[i + 2] == '"' && b[i + 3] == '"' {
                        s.push_str("\"\"\"");
                        i += 4;
                        continue;
                    }
                    if b[i] == '"' && i + 2 < b.len() && b[i + 1] == '"' && b[i + 2] == '"' {
                        i += 3;
                        break;
                    }
                    s.push(b[i]);
                    i += 1;
                }
                out.push(Tok::Str(s));
                continue;
            }
            i += 1;
            let mut s = String::new();
            loop {
                if i >= b.len() || b[i] == '\n' {
                    return Err("unterminated string literal in SDL".into());
                }
                if b[i] == '\\' && i + 1 < b.len() {
                    // `@examples(values: [...])` strings reach the output, so
                    // escapes are decoded rather than kept verbatim.
                    match b[i + 1] {
                        '"' => s.push('"'),
                        '\\' => s.push('\\'),
                        '/' => s.push('/'),
                        'n' => s.push('\n'),
                        'r' => s.push('\r'),
                        't' => s.push('\t'),
                        'b' => s.push('\u{8}'),
                        'f' => s.push('\u{c}'),
                        'u' => {
                            let hex: String = b.iter().skip(i + 2).take(4).collect();
                            let cp = u32::from_str_radix(&hex, 16).map_err(|_| {
                                format!("invalid \\u escape {hex:?} in SDL string literal")
                            })?;
                            s.push(char::from_u32(cp).unwrap_or('\u{fffd}'));
                            i += 4;
                        }
                        other => {
                            return Err(format!("invalid escape \"\\{other}\" in SDL string literal"))
                        }
                    }
                    i += 2;
                    continue;
                }
                if b[i] == '"' {
                    i += 1;
                    break;
                }
                s.push(b[i]);
                i += 1;
            }
            out.push(Tok::Str(s));
            continue;
        }
        if c == '.' {
            if i + 2 < b.len() && b[i + 1] == '.' && b[i + 2] == '.' {
                out.push(Tok::Spread);
                i += 3;
                continue;
            }
            return Err("unexpected '.' in SDL (expected '...' or a type definition)".into());
        }
        if c.is_ascii_digit() || (c == '-' && i + 1 < b.len() && b[i + 1].is_ascii_digit()) {
            let start = i;
            i += 1;
            while i < b.len()
                && (b[i].is_ascii_digit()
                    || b[i] == '.'
                    || b[i] == 'e'
                    || b[i] == 'E'
                    || b[i] == '+'
                    || b[i] == '-')
            {
                i += 1;
            }
            out.push(Tok::Num(b[start..i].iter().collect()));
            continue;
        }
        if c == '_' || c.is_ascii_alphabetic() {
            let start = i;
            while i < b.len() && (b[i] == '_' || b[i].is_ascii_alphanumeric()) {
                i += 1;
            }
            out.push(Tok::Name(b[start..i].iter().collect()));
            continue;
        }
        if "{}()[]:=|&@!$".contains(c) {
            out.push(Tok::Punct(c));
            i += 1;
            continue;
        }
        return Err(format!("unexpected character {c:?} in SDL"));
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Schema model
// ---------------------------------------------------------------------------

/// A field type reference: a named type wrapped in any number of list / non-null
/// wrappers, e.g. `[[String!]!]` .
#[derive(Debug, Clone, PartialEq)]
enum TypeRef {
    Named(String),
    List(Box<TypeRef>),
    NonNull(Box<TypeRef>),
}

impl TypeRef {
    fn is_non_null(&self) -> bool {
        matches!(self, TypeRef::NonNull(_))
    }
}

/// A directive argument value, kept only for the mocking directives below.
#[derive(Debug, Clone, PartialEq)]
enum DirValue {
    Str(String),
    Num(String),
    /// An unquoted name — an enum value or `true`/`false`/`null`.
    Name(String),
    List(Vec<DirValue>),
    /// An object literal; nothing we read needs its contents.
    Object,
}

impl DirValue {
    /// Render an `@examples` entry as JSON.
    fn to_json(&self) -> Value {
        match self {
            DirValue::Str(s) => Value::String(s.clone()),
            DirValue::Num(n) => n
                .parse::<i64>()
                .map(Value::from)
                .or_else(|_| n.parse::<f64>().map(|f| {
                    Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null)
                }))
                .unwrap_or_else(|_| Value::String(n.clone())),
            DirValue::Name(n) => match n.as_str() {
                "true" => Value::Bool(true),
                "false" => Value::Bool(false),
                "null" => Value::Null,
                other => Value::String(other.to_string()),
            },
            DirValue::List(items) => Value::Array(items.iter().map(DirValue::to_json).collect()),
            DirValue::Object => Value::Object(Map::new()),
        }
    }
    fn as_u32(&self) -> Option<u32> {
        match self {
            DirValue::Num(n) => n.parse::<u32>().ok(),
            _ => None,
        }
    }
    fn as_name(&self) -> Option<&str> {
        match self {
            DirValue::Name(n) => Some(n),
            DirValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
struct Directive {
    name: String,
    args: Vec<(String, DirValue)>,
}

impl Directive {
    fn arg(&self, name: &str) -> Option<&DirValue> {
        self.args.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

/// Per-field mocking hints read off `@fake` / `@examples` / `@listLength`
/// (the directive vocabulary mainstream SDL mock servers use).
#[derive(Debug, Clone, Default)]
struct Hints {
    examples: Vec<Value>,
    fake: Option<String>,
    list_min: Option<u32>,
    list_max: Option<u32>,
}

impl Hints {
    fn from_directives(dirs: &[Directive]) -> Self {
        let mut h = Hints::default();
        for d in dirs {
            match d.name.as_str() {
                "examples" => {
                    if let Some(DirValue::List(items)) = d.arg("values") {
                        h.examples = items.iter().map(DirValue::to_json).collect();
                    }
                }
                "fake" => {
                    if let Some(t) = d.arg("type").and_then(DirValue::as_name) {
                        h.fake = Some(t.to_ascii_lowercase());
                    }
                }
                "listLength" => {
                    h.list_min = d.arg("min").and_then(DirValue::as_u32);
                    h.list_max = d.arg("max").and_then(DirValue::as_u32);
                }
                _ => {}
            }
        }
        h
    }
    /// Per-field list length from `@listLength(min:, max:)`, capped like the
    /// global option so a hostile schema cannot blow the output up.
    fn list_length(&self, fallback: u32, rng: &mut Rng) -> u32 {
        let (min, max) = match (self.list_min, self.list_max) {
            (None, None) => return fallback,
            (Some(a), None) => (a, a),
            (None, Some(b)) => (b, b),
            (Some(a), Some(b)) => (a.min(b), a.max(b)),
        };
        let min = min.min(MAX_LIST_LENGTH);
        let max = max.min(MAX_LIST_LENGTH);
        min + rng.below((max - min) as u64 + 1) as u32
    }
}

#[derive(Debug, Clone)]
struct Field {
    name: String,
    ty: TypeRef,
    hints: Hints,
}

#[derive(Debug, Clone)]
enum TypeDef {
    Object {
        fields: Vec<Field>,
        interfaces: Vec<String>,
    },
    Interface {
        fields: Vec<Field>,
    },
    Union {
        members: Vec<String>,
    },
    Enum {
        values: Vec<String>,
    },
    Input {
        fields: Vec<Field>,
    },
    Scalar,
}

#[derive(Debug, Default)]
struct Schema {
    /// Type name → definition, in document order.
    types: Vec<(String, TypeDef)>,
    query_root: Option<String>,
    mutation_root: Option<String>,
    subscription_root: Option<String>,
}

impl Schema {
    fn get(&self, name: &str) -> Option<&TypeDef> {
        self.types
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| d)
    }
    fn fields_of(&self, name: &str) -> Option<&[Field]> {
        match self.get(name) {
            Some(TypeDef::Object { fields, .. })
            | Some(TypeDef::Interface { fields })
            | Some(TypeDef::Input { fields }) => Some(fields),
            _ => None,
        }
    }
    /// First object type implementing `iface`, in document order.
    fn first_implementor(&self, iface: &str) -> Option<&str> {
        self.types.iter().find_map(|(n, d)| match d {
            TypeDef::Object { interfaces, .. } if interfaces.iter().any(|i| i == iface) => {
                Some(n.as_str())
            }
            _ => None,
        })
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    fn eat_punct(&mut self, c: char) -> bool {
        if self.peek() == Some(&Tok::Punct(c)) {
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
            Err(format!("expected {c:?} in SDL, found {}", self.describe_here()))
        }
    }
    fn expect_name(&mut self) -> Result<String, String> {
        match self.bump() {
            Some(Tok::Name(n)) => Ok(n),
            _ => {
                self.pos = self.pos.saturating_sub(1);
                Err(format!("expected a name in SDL, found {}", self.describe_here()))
            }
        }
    }
    fn describe_here(&self) -> String {
        match self.peek() {
            None => "end of document".into(),
            Some(Tok::Name(n)) => format!("{n:?}"),
            Some(Tok::Str(_)) => "a string literal".into(),
            Some(Tok::Num(n)) => format!("number {n}"),
            Some(Tok::Punct(c)) => format!("{c:?}"),
            Some(Tok::Spread) => "\"...\"".into(),
        }
    }

    /// Skip a leading description, if any.
    fn skip_description(&mut self) {
        while matches!(self.peek(), Some(Tok::Str(_))) {
            self.pos += 1;
        }
    }

    /// Parse an `@dir` / `@dir(a: 1, b: [X])` sequence, keeping the arguments so
    /// the mocking directives can be honoured.
    fn parse_directives(&mut self) -> Result<Vec<Directive>, String> {
        let mut out = Vec::new();
        while self.peek() == Some(&Tok::Punct('@')) {
            self.pos += 1;
            let name = self.expect_name()?;
            let mut args = Vec::new();
            if self.peek() == Some(&Tok::Punct('(')) {
                self.expect_punct('(')?;
                while !self.eat_punct(')') {
                    if self.peek().is_none() {
                        return Err(format!("unterminated argument list on directive @{name}"));
                    }
                    let arg = self.expect_name()?;
                    self.expect_punct(':')?;
                    args.push((arg, self.parse_dir_value()?));
                }
            }
            out.push(Directive { name, args });
        }
        Ok(out)
    }

    /// Directives never affect anything outside the mocking vocabulary, so most
    /// call sites just drop the result.
    fn skip_directives(&mut self) -> Result<(), String> {
        self.parse_directives().map(|_| ())
    }

    /// Parse one directive argument value.
    fn parse_dir_value(&mut self) -> Result<DirValue, String> {
        match self.peek().cloned() {
            Some(Tok::Str(s)) => {
                self.pos += 1;
                Ok(DirValue::Str(s))
            }
            Some(Tok::Num(n)) => {
                self.pos += 1;
                Ok(DirValue::Num(n))
            }
            Some(Tok::Name(n)) => {
                self.pos += 1;
                Ok(DirValue::Name(n))
            }
            Some(Tok::Punct('[')) => {
                self.pos += 1;
                let mut items = Vec::new();
                while !self.eat_punct(']') {
                    if self.peek().is_none() {
                        return Err("unterminated list in a directive argument".into());
                    }
                    items.push(self.parse_dir_value()?);
                }
                Ok(DirValue::List(items))
            }
            Some(Tok::Punct('{')) => {
                self.skip_balanced('{', '}')?;
                Ok(DirValue::Object)
            }
            Some(Tok::Punct('$')) => {
                self.pos += 1;
                self.expect_name()?;
                Ok(DirValue::Object)
            }
            _ => Err(format!(
                "expected a directive argument value, found {}",
                self.describe_here()
            )),
        }
    }

    /// Consume a balanced `open`…`close` run (used for argument lists and
    /// default values, whose contents never affect mock generation).
    fn skip_balanced(&mut self, open: char, close: char) -> Result<(), String> {
        self.expect_punct(open)?;
        let mut depth = 1usize;
        while depth > 0 {
            match self.bump() {
                None => return Err(format!("unbalanced {open:?} in SDL")),
                Some(Tok::Punct(c)) if c == open => depth += 1,
                Some(Tok::Punct(c)) if c == close => depth -= 1,
                _ => {}
            }
        }
        Ok(())
    }

    fn parse_type_ref(&mut self) -> Result<TypeRef, String> {
        let base = if self.eat_punct('[') {
            let inner = self.parse_type_ref()?;
            self.expect_punct(']')?;
            TypeRef::List(Box::new(inner))
        } else {
            TypeRef::Named(self.expect_name()?)
        };
        if self.eat_punct('!') {
            Ok(TypeRef::NonNull(Box::new(base)))
        } else {
            Ok(base)
        }
    }

    /// `{ name(args): Type @dir  … }`
    fn parse_fields(&mut self) -> Result<Vec<Field>, String> {
        let mut fields = Vec::new();
        self.expect_punct('{')?;
        loop {
            if self.eat_punct('}') {
                break;
            }
            if self.peek().is_none() {
                return Err("unterminated field block '{' in SDL".into());
            }
            self.skip_description();
            if self.eat_punct('}') {
                break;
            }
            let name = self.expect_name()?;
            if self.peek() == Some(&Tok::Punct('(')) {
                self.skip_balanced('(', ')')?;
            }
            self.expect_punct(':')?;
            let ty = self.parse_type_ref()?;
            // An input field may carry a default value: `= <value>`.
            if self.eat_punct('=') {
                self.skip_value()?;
            }
            let hints = Hints::from_directives(&self.parse_directives()?);
            fields.push(Field { name, ty, hints });
        }
        Ok(fields)
    }

    /// Skip one GraphQL value (scalar, enum, list, or object).
    fn skip_value(&mut self) -> Result<(), String> {
        match self.peek() {
            Some(Tok::Punct('[')) => self.skip_balanced('[', ']'),
            Some(Tok::Punct('{')) => self.skip_balanced('{', '}'),
            Some(Tok::Punct('$')) => {
                self.pos += 1;
                self.expect_name()?;
                Ok(())
            }
            Some(Tok::Name(_)) | Some(Tok::Str(_)) | Some(Tok::Num(_)) => {
                self.pos += 1;
                Ok(())
            }
            _ => Err(format!("expected a default value, found {}", self.describe_here())),
        }
    }

    fn parse_enum_values(&mut self) -> Result<Vec<String>, String> {
        let mut values = Vec::new();
        self.expect_punct('{')?;
        loop {
            if self.eat_punct('}') {
                break;
            }
            if self.peek().is_none() {
                return Err("unterminated enum block '{' in SDL".into());
            }
            self.skip_description();
            if self.eat_punct('}') {
                break;
            }
            let name = self.expect_name()?;
            self.skip_directives()?;
            values.push(name);
        }
        Ok(values)
    }

    fn parse_union_members(&mut self) -> Result<Vec<String>, String> {
        let mut members = Vec::new();
        self.expect_punct('=')?;
        self.eat_punct('|'); // optional leading pipe
        members.push(self.expect_name()?);
        while self.eat_punct('|') {
            members.push(self.expect_name()?);
        }
        Ok(members)
    }

    fn parse_implements(&mut self) -> Result<Vec<String>, String> {
        let mut ifaces = Vec::new();
        if self.peek() == Some(&Tok::Name("implements".into())) {
            self.pos += 1;
            self.eat_punct('&'); // optional leading ampersand
            ifaces.push(self.expect_name()?);
            while self.eat_punct('&') {
                ifaces.push(self.expect_name()?);
            }
        }
        Ok(ifaces)
    }
}

fn parse_sdl(src: &str) -> Result<Schema, String> {
    let toks = lex(src)?;
    let mut p = Parser { toks, pos: 0 };
    let mut schema = Schema::default();

    while p.peek().is_some() {
        p.skip_description();
        if p.peek().is_none() {
            break;
        }
        let mut extending = false;
        let kw = match p.peek() {
            Some(Tok::Name(n)) => n.clone(),
            _ => {
                return Err(format!(
                    "expected a type definition (type, interface, enum, union, input, scalar, \
                     schema), found {}",
                    p.describe_here()
                ))
            }
        };
        p.pos += 1;
        let kw = if kw == "extend" {
            extending = true;
            p.expect_name()?
        } else {
            kw
        };

        match kw.as_str() {
            "schema" => {
                p.skip_directives()?;
                p.expect_punct('{')?;
                loop {
                    if p.eat_punct('}') {
                        break;
                    }
                    if p.peek().is_none() {
                        return Err("unterminated schema block '{' in SDL".into());
                    }
                    let op = p.expect_name()?;
                    p.expect_punct(':')?;
                    let ty = p.expect_name()?;
                    match op.as_str() {
                        "query" => schema.query_root = Some(ty),
                        "mutation" => schema.mutation_root = Some(ty),
                        "subscription" => schema.subscription_root = Some(ty),
                        other => {
                            return Err(format!(
                                "unknown root operation {other:?} in schema block (expected query, \
                                 mutation, or subscription)"
                            ))
                        }
                    }
                }
            }
            "scalar" => {
                let name = p.expect_name()?;
                p.skip_directives()?;
                upsert(&mut schema, name, TypeDef::Scalar, extending)?;
            }
            "type" => {
                let name = p.expect_name()?;
                let interfaces = p.parse_implements()?;
                p.skip_directives()?;
                let fields = if p.peek() == Some(&Tok::Punct('{')) {
                    p.parse_fields()?
                } else {
                    Vec::new()
                };
                upsert(
                    &mut schema,
                    name,
                    TypeDef::Object { fields, interfaces },
                    extending,
                )?;
            }
            "interface" => {
                let name = p.expect_name()?;
                let _ = p.parse_implements()?;
                p.skip_directives()?;
                let fields = if p.peek() == Some(&Tok::Punct('{')) {
                    p.parse_fields()?
                } else {
                    Vec::new()
                };
                upsert(&mut schema, name, TypeDef::Interface { fields }, extending)?;
            }
            "input" => {
                let name = p.expect_name()?;
                p.skip_directives()?;
                let fields = if p.peek() == Some(&Tok::Punct('{')) {
                    p.parse_fields()?
                } else {
                    Vec::new()
                };
                upsert(&mut schema, name, TypeDef::Input { fields }, extending)?;
            }
            "enum" => {
                let name = p.expect_name()?;
                p.skip_directives()?;
                let values = if p.peek() == Some(&Tok::Punct('{')) {
                    p.parse_enum_values()?
                } else {
                    Vec::new()
                };
                upsert(&mut schema, name, TypeDef::Enum { values }, extending)?;
            }
            "union" => {
                let name = p.expect_name()?;
                p.skip_directives()?;
                let members = if p.peek() == Some(&Tok::Punct('=')) {
                    p.parse_union_members()?
                } else {
                    Vec::new()
                };
                upsert(&mut schema, name, TypeDef::Union { members }, extending)?;
            }
            "directive" => {
                // `directive @name(args) repeatable on LOC | LOC` — nothing to mock.
                p.expect_punct('@')?;
                p.expect_name()?;
                if p.peek() == Some(&Tok::Punct('(')) {
                    p.skip_balanced('(', ')')?;
                }
                if p.peek() == Some(&Tok::Name("repeatable".into())) {
                    p.pos += 1;
                }
                if p.peek() == Some(&Tok::Name("on".into())) {
                    p.pos += 1;
                }
                p.eat_punct('|');
                p.expect_name()?;
                while p.eat_punct('|') {
                    p.expect_name()?;
                }
            }
            "query" | "mutation" | "subscription" | "fragment" => {
                return Err(format!(
                    "{kw:?} starts an executable operation, not a schema definition — paste the \
                     SDL type definitions (type / interface / enum / union / input / scalar), not \
                     a query document"
                ))
            }
            other => {
                return Err(format!(
                    "unknown SDL keyword {other:?} (expected type, interface, union, enum, input, \
                     scalar, schema, extend, or directive)"
                ))
            }
        }
    }

    Ok(schema)
}

/// Insert a definition, or merge it into an existing one for `extend`.
fn upsert(schema: &mut Schema, name: String, def: TypeDef, extending: bool) -> Result<(), String> {
    if let Some(slot) = schema.types.iter_mut().find(|(n, _)| *n == name) {
        if !extending {
            return Err(format!("type {name:?} is defined more than once"));
        }
        match (&mut slot.1, def) {
            (
                TypeDef::Object { fields, interfaces },
                TypeDef::Object {
                    fields: more,
                    interfaces: more_ifaces,
                },
            ) => {
                fields.extend(more);
                interfaces.extend(more_ifaces);
            }
            (TypeDef::Interface { fields }, TypeDef::Interface { fields: more })
            | (TypeDef::Input { fields }, TypeDef::Input { fields: more }) => fields.extend(more),
            (TypeDef::Enum { values }, TypeDef::Enum { values: more }) => values.extend(more),
            (TypeDef::Union { members }, TypeDef::Union { members: more }) => members.extend(more),
            (TypeDef::Scalar, TypeDef::Scalar) => {}
            _ => return Err(format!("`extend` for {name:?} does not match its original kind")),
        }
        return Ok(());
    }
    if extending {
        // Extending an as-yet-undefined type is legal in a multi-file schema;
        // treat the extension as the definition rather than failing.
        schema.types.push((name, def));
        return Ok(());
    }
    schema.types.push((name, def));
    Ok(())
}

// ---------------------------------------------------------------------------
// Deterministic PRNG
// ---------------------------------------------------------------------------

/// Tiny deterministic PRNG (splitmix64) — no external dep, stable output.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next_u64() % n
        }
    }
    fn pick<'a>(&mut self, items: &[&'a str]) -> &'a str {
        items[self.below(items.len() as u64) as usize]
    }
    fn range_i(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        lo + self.below((hi - lo) as u64 + 1) as i64
    }
    fn range_f(&mut self, lo: f64, hi: f64) -> f64 {
        if hi <= lo {
            return lo;
        }
        let u = (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64;
        let v = lo + u * (hi - lo);
        (v * 100.0).round() / 100.0
    }
    fn bool(&mut self) -> bool {
        self.next_u64() & 1 == 0
    }
}

// ---- synthetic value pools (never real people or real domains) -------------
const FIRST_NAMES: &[&str] = &[
    "Ava", "Liam", "Mia", "Noah", "Emma", "Oliver", "Sophia", "Lucas", "Isabella", "Mason",
    "Amelia", "Ethan", "Harper", "James", "Evelyn", "Aria", "Leo", "Nora", "Owen", "Zoe",
];
const LAST_NAMES: &[&str] = &[
    "Smith", "Jones", "Brown", "Taylor", "Wilson", "Davies", "Evans", "Thomas", "Roberts",
    "Walker", "Wright", "Green", "Hall", "Clark", "Patel", "Khan", "Lee", "Young", "King", "Scott",
];
const DOMAINS: &[&str] = &["example.com", "example.org", "example.net"];
const WORDS: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing", "elit", "sed", "tempor",
    "labore", "magna", "aliqua", "veniam", "nostrud", "aliquip", "commodo", "consequat",
];
const CITIES: &[&str] = &["Springfield", "Rivertown", "Lakeview", "Fairport", "Westfield"];
const COUNTRIES: &[&str] = &["United Kingdom", "Netherlands", "Canada", "Australia", "Japan"];
const STATUSES: &[&str] = &["active", "pending", "archived", "draft"];

// ---------------------------------------------------------------------------
// Options + entry point
// ---------------------------------------------------------------------------

/// How nullable fields are mocked.
#[derive(Debug, Clone, Copy, PartialEq)]
enum NullMode {
    /// Always generate a value (default — matches mainstream mock libraries).
    Fill,
    /// Emit JSON `null` for every nullable field.
    Null,
    /// Leave every nullable field out of the object entirely.
    Omit,
}

struct Opts {
    list_length: u32,
    depth: u32,
    nulls: NullMode,
    smart: bool,
    typename: bool,
}

/// Public entry point.
///
/// * `mode` — `all-types` (one mock per object/interface/union type),
///   `query-response` (a `{"data": …}` envelope for the Query root), or
///   `single-type` (just `type_name`).
/// * `type_name` — the type to mock in `single-type` mode.
/// * `list_length` — items generated per list field (0–10).
/// * `depth` — maximum object nesting depth (1–6).
/// * `nullable_fields` — `fill`, `null`, or `omit`.
/// * `smart_values` — infer realistic values from field names.
/// * `typename` — add `__typename` to every mocked object.
/// * `seed` — deterministic seed.
/// * `pretty` — pretty-print the JSON.
#[allow(clippy::too_many_arguments)]
pub fn generate(
    sdl: &str,
    mode: &str,
    type_name: &str,
    list_length: u32,
    depth: u32,
    nullable_fields: &str,
    smart_values: bool,
    typename: bool,
    seed: u64,
    pretty: bool,
) -> Result<String, String> {
    if sdl.trim().is_empty() {
        return Err("sdl is empty: paste a GraphQL SDL schema".into());
    }
    if sdl.len() > MAX_SDL_BYTES {
        return Err(format!(
            "SDL is {} bytes, which exceeds the {MAX_SDL_BYTES}-byte limit",
            sdl.len()
        ));
    }
    if list_length > MAX_LIST_LENGTH {
        return Err(format!(
            "list_length {list_length} exceeds the maximum of {MAX_LIST_LENGTH}"
        ));
    }
    if depth == 0 || depth > MAX_DEPTH {
        return Err(format!("depth must be between 1 and {MAX_DEPTH}"));
    }
    let mode_norm = mode.trim().to_ascii_lowercase();
    if !matches!(
        mode_norm.as_str(),
        "all-types" | "query-response" | "single-type"
    ) {
        return Err(format!(
            "invalid mode {mode_norm:?}: expected \"all-types\", \"query-response\", or \
             \"single-type\""
        ));
    }
    let nulls = match nullable_fields.trim().to_ascii_lowercase().as_str() {
        "fill" => NullMode::Fill,
        "null" => NullMode::Null,
        "omit" => NullMode::Omit,
        other => {
            return Err(format!(
                "invalid nullable_fields {other:?}: expected \"fill\", \"null\", or \"omit\""
            ))
        }
    };

    let schema = parse_sdl(sdl)?;
    if schema.types.is_empty() {
        return Err("no type definitions found in the SDL document".into());
    }

    let opts = Opts {
        list_length,
        depth,
        nulls,
        smart: smart_values,
        typename,
    };
    let mut rng = Rng::new(if seed == 0 { 0xC0FF_EE12_3456_789A } else { seed });

    let out: Value = match mode_norm.as_str() {
        "single-type" => {
            let want = type_name.trim();
            if want.is_empty() {
                return Err(
                    "single-type mode needs type_name — name the SDL type you want mocked".into(),
                );
            }
            if schema.get(want).is_none() {
                return Err(format!(
                    "type {want:?} is not defined in this SDL document (defined types: {})",
                    type_names(&schema)
                ));
            }
            mock_named_type(&schema, want, &opts, &mut rng, 0)?
        }
        "query-response" => {
            let root = schema
                .query_root
                .clone()
                .filter(|r| schema.get(r).is_some())
                .or_else(|| schema.get("Query").map(|_| "Query".to_string()))
                .ok_or_else(|| {
                    "no Query root type found: define `type Query { … }` or a `schema { query: … }` \
                     block to mock a query response"
                        .to_string()
                })?;
            let data = mock_named_type(&schema, &root, &opts, &mut rng, 0)?;
            let mut env = Map::new();
            env.insert("data".into(), data);
            Value::Object(env)
        }
        _ => {
            let mut out = Map::new();
            for (name, def) in &schema.types {
                if name.starts_with("__") {
                    continue; // introspection types are never mocked
                }
                match def {
                    // Input types describe arguments, not response data.
                    TypeDef::Input { .. } | TypeDef::Enum { .. } | TypeDef::Scalar => continue,
                    _ => {}
                }
                out.insert(name.clone(), mock_named_type(&schema, name, &opts, &mut rng, 0)?);
            }
            if out.is_empty() {
                return Err(
                    "this SDL defines no object, interface, or union types to mock (only enums, \
                     inputs, or scalars) — use single-type mode to mock one of those instead"
                        .into(),
                );
            }
            Value::Object(out)
        }
    };

    if pretty {
        serde_json::to_string_pretty(&out)
    } else {
        serde_json::to_string(&out)
    }
    .map_err(|e| format!("failed to serialize JSON: {e}"))
}

fn type_names(schema: &Schema) -> String {
    let mut names: Vec<&str> = schema.types.iter().map(|(n, _)| n.as_str()).collect();
    names.sort_unstable();
    names.join(", ")
}

// ---------------------------------------------------------------------------
// Mock generation
// ---------------------------------------------------------------------------

/// Mock a named type. Interfaces and unions resolve to a concrete member and
/// always carry `__typename` so the chosen shape is unambiguous.
fn mock_named_type(
    schema: &Schema,
    name: &str,
    opts: &Opts,
    rng: &mut Rng,
    depth: u32,
) -> Result<Value, String> {
    match schema.get(name) {
        Some(TypeDef::Object { .. }) | Some(TypeDef::Input { .. }) => {
            mock_object(schema, name, name, opts, rng, depth, false)
        }
        Some(TypeDef::Interface { .. }) => {
            // Prefer a concrete implementor so the mock is usable as a response.
            match schema.first_implementor(name) {
                Some(concrete) => {
                    let concrete = concrete.to_string();
                    mock_object(schema, &concrete, &concrete, opts, rng, depth, true)
                }
                None => mock_object(schema, name, name, opts, rng, depth, true),
            }
        }
        Some(TypeDef::Union { members }) => {
            let member = members
                .first()
                .ok_or_else(|| format!("union {name:?} lists no member types"))?
                .clone();
            if schema.fields_of(&member).is_none() {
                return Err(format!(
                    "union {name:?} member {member:?} is not an object type defined in this SDL"
                ));
            }
            mock_object(schema, &member, &member, opts, rng, depth, true)
        }
        Some(TypeDef::Enum { values }) => Ok(enum_value(values, name, rng)),
        Some(TypeDef::Scalar) => Ok(custom_scalar(name, rng)),
        None => Err(format!("type {name:?} is not defined in this SDL document")),
    }
}

/// Build the JSON object for `type_name`. `leaves_only` (set at the depth cap)
/// keeps scalar/enum fields and collapses nested objects.
#[allow(clippy::too_many_arguments)]
fn mock_object(
    schema: &Schema,
    type_name: &str,
    typename_value: &str,
    opts: &Opts,
    rng: &mut Rng,
    depth: u32,
    force_typename: bool,
) -> Result<Value, String> {
    let fields = schema
        .fields_of(type_name)
        .ok_or_else(|| format!("type {type_name:?} has no fields to mock"))?
        .to_vec();
    let mut out = Map::new();
    if opts.typename || force_typename {
        out.insert("__typename".into(), Value::String(typename_value.to_string()));
    }
    for f in &fields {
        if f.name.starts_with("__") {
            continue;
        }
        match mock_field(schema, f, opts, rng, depth)? {
            Some(v) => {
                out.insert(f.name.clone(), v);
            }
            None => continue, // omitted nullable field
        }
    }
    Ok(Value::Object(out))
}

/// `Ok(None)` means "leave this key out" (only ever in `omit` mode).
fn mock_field(
    schema: &Schema,
    field: &Field,
    opts: &Opts,
    rng: &mut Rng,
    depth: u32,
) -> Result<Option<Value>, String> {
    let nullable = !field.ty.is_non_null();
    if nullable {
        match opts.nulls {
            NullMode::Omit => return Ok(None),
            NullMode::Null => return Ok(Some(Value::Null)),
            NullMode::Fill => {}
        }
    }
    Ok(Some(mock_type_ref(
        schema,
        &field.ty,
        &field.name,
        &field.hints,
        opts,
        rng,
        depth,
    )?))
}

fn mock_type_ref(
    schema: &Schema,
    ty: &TypeRef,
    field_name: &str,
    hints: &Hints,
    opts: &Opts,
    rng: &mut Rng,
    depth: u32,
) -> Result<Value, String> {
    match ty {
        TypeRef::NonNull(inner) => {
            mock_type_ref(schema, inner, field_name, hints, opts, rng, depth)
        }
        TypeRef::List(inner) => {
            let n = hints.list_length(opts.list_length, rng);
            let mut items = Vec::with_capacity(n as usize);
            for _ in 0..n {
                items.push(mock_type_ref(schema, inner, field_name, hints, opts, rng, depth)?);
            }
            Ok(Value::Array(items))
        }
        TypeRef::Named(name) => {
            mock_leaf_or_object(schema, name, field_name, hints, opts, rng, depth)
        }
    }
}

fn mock_leaf_or_object(
    schema: &Schema,
    name: &str,
    field_name: &str,
    hints: &Hints,
    opts: &Opts,
    rng: &mut Rng,
    depth: u32,
) -> Result<Value, String> {
    // `@examples` / `@fake` only make sense on leaf positions; on an object
    // field they are ignored rather than treated as an error.
    let is_leaf = matches!(name, "String" | "Int" | "Float" | "Boolean" | "ID")
        || matches!(schema.get(name), Some(TypeDef::Enum { .. }) | Some(TypeDef::Scalar));
    if is_leaf {
        if !hints.examples.is_empty() {
            let i = rng.below(hints.examples.len() as u64) as usize;
            return Ok(hints.examples[i].clone());
        }
        if let Some(kind) = &hints.fake {
            if let Some(v) = fake_value(kind, rng) {
                return Ok(v);
            }
        }
    }
    // Built-in scalars first — they can be shadowed only by an explicit
    // redefinition, which `parse_sdl` would have rejected as a duplicate.
    match name {
        "String" => return Ok(Value::String(string_value(field_name, opts.smart, rng))),
        "Int" => return Ok(Value::from(int_value(field_name, opts.smart, rng))),
        "Float" => return Ok(float_value(field_name, opts.smart, rng)),
        "Boolean" => return Ok(Value::Bool(bool_value(field_name, opts.smart, rng))),
        "ID" => return Ok(Value::String(uuid_v4(rng))),
        _ => {}
    }
    match schema.get(name) {
        Some(TypeDef::Enum { values }) => Ok(enum_value(values, name, rng)),
        Some(TypeDef::Scalar) => Ok(custom_scalar(name, rng)),
        Some(TypeDef::Object { .. })
        | Some(TypeDef::Interface { .. })
        | Some(TypeDef::Union { .. })
        | Some(TypeDef::Input { .. }) => {
            if depth + 1 >= opts.depth {
                // Depth cap: a nullable slot would already have been handled by
                // `mock_field`, so emit an empty object rather than recursing.
                return Ok(Value::Object(Map::new()));
            }
            mock_named_type(schema, name, opts, rng, depth + 1)
        }
        None => Err(format!(
            "field {field_name:?} references undefined type {name:?} — add it to the SDL (or \
             declare it with `scalar {name}`)"
        )),
    }
}

/// `@fake(type: <name>)` generator registry. Names follow the mainstream SDL
/// faker vocabulary; an unrecognised name returns `None` so the caller falls
/// back to field-name inference instead of failing the whole document.
fn fake_value(kind: &str, rng: &mut Rng) -> Option<Value> {
    let s = |v: String| Some(Value::String(v));
    match kind {
        "firstname" | "givenname" => s(rng.pick(FIRST_NAMES).to_string()),
        "lastname" | "surname" => s(rng.pick(LAST_NAMES).to_string()),
        "fullname" | "findname" | "name" => {
            s(format!("{} {}", rng.pick(FIRST_NAMES), rng.pick(LAST_NAMES)))
        }
        "username" | "login" | "nickname" => s(format!(
            "{}{}",
            rng.pick(FIRST_NAMES).to_ascii_lowercase(),
            rng.range_i(10, 99)
        )),
        "email" => s(email(rng)),
        "password" => s("not-a-real-password".to_string()),
        "url" | "uri" => s(url(rng)),
        "imageurl" | "avatarurl" | "image" | "avatar" => s(format!(
            "https://cdn.{}/images/{}.png",
            rng.pick(DOMAINS),
            rng.pick(WORDS)
        )),
        "phonenumber" | "phone" => s(phone(rng)),
        "streetaddress" | "address" => {
            s(format!("{} {} Street", rng.range_i(1, 200), rng.pick(LAST_NAMES)))
        }
        "city" => s(rng.pick(CITIES).to_string()),
        "country" => s(rng.pick(COUNTRIES).to_string()),
        "countrycode" => s(rng.pick(&["GB", "NL", "CA", "AU", "JP"]).to_string()),
        "zipcode" | "postcode" => s(format!("{}{} {}AB", rng.pick(&["N", "SW", "EC"]), rng.range_i(1, 20), rng.range_i(1, 9))),
        "companyname" | "company" => {
            s(format!("{} {}", rng.pick(LAST_NAMES), rng.pick(&["Ltd", "Group", "Labs"])))
        }
        "jobtitle" => s(rng.pick(&["Engineer", "Designer", "Analyst", "Manager"]).to_string()),
        "productname" | "product" => s(title_case(&words(rng, 2))),
        "word" => s(rng.pick(WORDS).to_string()),
        "words" | "lorem" => s(words(rng, 3)),
        "sentence" | "sentences" => s(sentence(rng, 8)),
        "paragraph" | "paragraphs" => s(format!("{} {}", sentence(rng, 9), sentence(rng, 7))),
        "hexcolor" | "color" => s(hex_color(rng)),
        "uuid" | "guid" => s(uuid_v4(rng)),
        "filename" => s(format!("{}.png", rng.pick(WORDS))),
        "mimetype" => s(rng.pick(&["image/png", "application/json", "text/plain"]).to_string()),
        "ipv4" | "ip" => s(format!(
            "{}.{}.{}.{}",
            rng.below(256),
            rng.below(256),
            rng.below(256),
            rng.below(256)
        )),
        "semver" => s(format!(
            "{}.{}.{}",
            rng.range_i(0, 5),
            rng.range_i(0, 20),
            rng.range_i(0, 9)
        )),
        "locale" | "language" => s(rng.pick(&["en-GB", "en-US", "nl-NL", "fr-FR"]).to_string()),
        "currencycode" | "currency" => s(rng.pick(&["GBP", "EUR", "USD", "JPY"]).to_string()),
        "date" | "pastdate" | "futuredate" | "recentdate" => s(date(rng)),
        "datetime" | "isodatetime" | "timestamp" => s(date_time(rng)),
        "money" | "price" | "amount" => Number::from_f64(rng.range_f(1.0, 999.0))
            .map(Value::Number)
            .or(Some(Value::Null)),
        "latitude" => Number::from_f64(rng.range_f(-90.0, 90.0)).map(Value::Number),
        "longitude" => Number::from_f64(rng.range_f(-180.0, 180.0)).map(Value::Number),
        "number" | "integer" | "int" => Some(Value::from(rng.range_i(1, 1000))),
        "float" | "double" => Number::from_f64(rng.range_f(0.0, 1000.0)).map(Value::Number),
        "boolean" | "bool" => Some(Value::Bool(rng.bool())),
        _ => None,
    }
}

fn enum_value(values: &[String], name: &str, rng: &mut Rng) -> Value {
    if values.is_empty() {
        return Value::String(format!("{name}_VALUE"));
    }
    Value::String(values[rng.below(values.len() as u64) as usize].clone())
}

/// Well-known custom scalar names get shaped values; anything else gets a
/// clearly-labelled placeholder string.
fn custom_scalar(name: &str, rng: &mut Rng) -> Value {
    let lower = name.to_ascii_lowercase();
    match lower.as_str() {
        "datetime" | "timestamp" | "isodatetime" | "iso8601datetime" | "utcdatetime"
        | "datetimeiso" => Value::String(date_time(rng)),
        "date" | "isodate" | "localdate" => Value::String(date(rng)),
        "time" | "localtime" => Value::String(format!(
            "{:02}:{:02}:{:02}",
            rng.range_i(0, 23),
            rng.range_i(0, 59),
            rng.range_i(0, 59)
        )),
        "json" | "jsonobject" | "object" | "map" => {
            let mut m = Map::new();
            m.insert("key".into(), Value::String(rng.pick(WORDS).to_string()));
            Value::Object(m)
        }
        "url" | "uri" | "urlstring" | "link" => Value::String(url(rng)),
        "email" | "emailaddress" => Value::String(email(rng)),
        "uuid" | "guid" => Value::String(uuid_v4(rng)),
        "bigint" | "long" | "int64" | "biginteger" => Value::from(rng.range_i(1, 9_007_199_254_740_991)),
        "decimal" | "money" | "bigdecimal" | "currencyamount" => {
            Number::from_f64(rng.range_f(1.0, 9999.0))
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        "positiveint" | "nonnegativeint" | "unsignedint" => Value::from(rng.range_i(1, 1000)),
        "phone" | "phonenumber" => Value::String(phone(rng)),
        "hexcolor" | "hexcolorcode" | "color" => Value::String(hex_color(rng)),
        "base64" | "byte" | "bytes" => Value::String(hex_string(rng, 24)),
        "upload" | "file" => Value::String(format!("{}.png", rng.pick(WORDS))),
        "void" => Value::Null,
        _ => Value::String(format!("mock-{lower}")),
    }
}

// ---- field-name-aware value generators -------------------------------------

fn has(hay: &str, needle: &str) -> bool {
    hay.contains(needle)
}

fn string_value(field: &str, smart: bool, rng: &mut Rng) -> String {
    if !smart {
        return words(rng, 2);
    }
    let f = field.to_ascii_lowercase();
    if has(&f, "email") {
        return email(rng);
    }
    if has(&f, "phone") || has(&f, "mobile") || has(&f, "tel") {
        return phone(rng);
    }
    if has(&f, "avatar") || has(&f, "image") || has(&f, "photo") || has(&f, "thumbnail")
        || has(&f, "picture")
    {
        return format!("https://cdn.{}/images/{}.png", rng.pick(DOMAINS), rng.pick(WORDS));
    }
    if has(&f, "url") || has(&f, "uri") || has(&f, "website") || has(&f, "link")
        || has(&f, "homepage")
    {
        return url(rng);
    }
    if has(&f, "slug") || has(&f, "handle") {
        return format!("{}-{}", rng.pick(WORDS), rng.pick(WORDS));
    }
    if has(&f, "username") || has(&f, "login") || has(&f, "nickname") {
        return format!(
            "{}{}",
            rng.pick(FIRST_NAMES).to_ascii_lowercase(),
            rng.range_i(10, 99)
        );
    }
    if has(&f, "firstname") || f == "given" || has(&f, "givenname") {
        return rng.pick(FIRST_NAMES).to_string();
    }
    if has(&f, "lastname") || has(&f, "surname") || has(&f, "familyname") {
        return rng.pick(LAST_NAMES).to_string();
    }
    if has(&f, "name") {
        return format!("{} {}", rng.pick(FIRST_NAMES), rng.pick(LAST_NAMES));
    }
    if has(&f, "city") || has(&f, "town") {
        return rng.pick(CITIES).to_string();
    }
    if has(&f, "country") {
        return rng.pick(COUNTRIES).to_string();
    }
    if has(&f, "postcode") || has(&f, "zip") || has(&f, "postal") {
        return format!("{}{} {}{}{}",
            rng.pick(&["N", "SW", "EC"]),
            rng.range_i(1, 20),
            rng.range_i(1, 9),
            rng.pick(&["A", "B", "D"]),
            rng.pick(&["P", "Q", "R"])
        );
    }
    if has(&f, "street") || has(&f, "address") {
        return format!("{} {} Street", rng.range_i(1, 200), rng.pick(LAST_NAMES));
    }
    if has(&f, "currency") {
        return rng.pick(&["GBP", "EUR", "USD", "JPY"]).to_string();
    }
    if has(&f, "locale") || has(&f, "language") {
        return rng.pick(&["en-GB", "en-US", "nl-NL", "fr-FR"]).to_string();
    }
    if has(&f, "timezone") || has(&f, "tz") {
        return rng.pick(&["Europe/London", "UTC", "America/New_York"]).to_string();
    }
    if has(&f, "color") || has(&f, "colour") {
        return hex_color(rng);
    }
    if has(&f, "ip") && !has(&f, "zip") {
        return format!(
            "{}.{}.{}.{}",
            rng.below(256),
            rng.below(256),
            rng.below(256),
            rng.below(256)
        );
    }
    if has(&f, "token") || has(&f, "hash") || has(&f, "checksum") || has(&f, "signature")
        || has(&f, "digest")
    {
        return hex_string(rng, 32);
    }
    if has(&f, "password") {
        return "not-a-real-password".to_string();
    }
    if has(&f, "status") || has(&f, "state") {
        return rng.pick(STATUSES).to_string();
    }
    if has(&f, "createdat") || has(&f, "updatedat") || has(&f, "deletedat") || has(&f, "publishedat")
        || has(&f, "timestamp") || f.ends_with("at") || has(&f, "datetime")
    {
        return date_time(rng);
    }
    if has(&f, "birthday") || has(&f, "dob") || has(&f, "date") {
        return date(rng);
    }
    if has(&f, "description") || has(&f, "summary") || has(&f, "bio") || has(&f, "body")
        || has(&f, "content") || has(&f, "message") || has(&f, "comment") || has(&f, "text")
        || has(&f, "excerpt")
    {
        return sentence(rng, 8);
    }
    if has(&f, "title") || has(&f, "headline") || has(&f, "label") || has(&f, "subject") {
        return title_case(&words(rng, 3));
    }
    if has(&f, "id") {
        return uuid_v4(rng);
    }
    words(rng, 2)
}

fn int_value(field: &str, smart: bool, rng: &mut Rng) -> i64 {
    if !smart {
        return rng.range_i(1, 1000);
    }
    let f = field.to_ascii_lowercase();
    if has(&f, "age") {
        return rng.range_i(18, 80);
    }
    if has(&f, "year") {
        return rng.range_i(2015, 2026);
    }
    if has(&f, "month") {
        return rng.range_i(1, 12);
    }
    if has(&f, "day") {
        return rng.range_i(1, 28);
    }
    if has(&f, "percent") {
        return rng.range_i(0, 100);
    }
    if has(&f, "count") || has(&f, "total") || has(&f, "quantity") || has(&f, "qty")
        || has(&f, "stock") || has(&f, "views") || has(&f, "likes")
    {
        return rng.range_i(0, 500);
    }
    if has(&f, "price") || has(&f, "amount") || has(&f, "cost") || has(&f, "cents") {
        return rng.range_i(100, 99_999);
    }
    if has(&f, "rating") || has(&f, "score") || has(&f, "stars") {
        return rng.range_i(1, 5);
    }
    if has(&f, "id") {
        return rng.range_i(1, 9999);
    }
    rng.range_i(1, 1000)
}

fn float_value(field: &str, smart: bool, rng: &mut Rng) -> Value {
    let v = if !smart {
        rng.range_f(0.0, 1000.0)
    } else {
        let f = field.to_ascii_lowercase();
        if has(&f, "latitude") || f == "lat" {
            rng.range_f(-90.0, 90.0)
        } else if has(&f, "longitude") || f == "lng" || f == "lon" {
            rng.range_f(-180.0, 180.0)
        } else if has(&f, "rating") || has(&f, "score") || has(&f, "stars") {
            rng.range_f(1.0, 5.0)
        } else if has(&f, "percent") || has(&f, "ratio") {
            rng.range_f(0.0, 100.0)
        } else if has(&f, "price") || has(&f, "amount") || has(&f, "cost") || has(&f, "total") {
            rng.range_f(1.0, 999.0)
        } else if has(&f, "weight") || has(&f, "height") || has(&f, "width") || has(&f, "length") {
            rng.range_f(0.5, 200.0)
        } else {
            rng.range_f(0.0, 1000.0)
        }
    };
    Number::from_f64(v).map(Value::Number).unwrap_or(Value::Null)
}

fn bool_value(field: &str, smart: bool, rng: &mut Rng) -> bool {
    if smart {
        let f = field.to_ascii_lowercase();
        // Fields that gate visibility read better as `true` in a fixture.
        if has(&f, "active") || has(&f, "enabled") || has(&f, "published") || has(&f, "visible")
            || has(&f, "verified")
        {
            return true;
        }
        if has(&f, "deleted") || has(&f, "archived") || has(&f, "banned") || has(&f, "hidden") {
            return false;
        }
    }
    rng.bool()
}

// ---- primitive shapes ------------------------------------------------------

fn words(rng: &mut Rng, n: usize) -> String {
    (0..n.max(1))
        .map(|_| rng.pick(WORDS))
        .collect::<Vec<_>>()
        .join(" ")
}

fn sentence(rng: &mut Rng, n: usize) -> String {
    let mut s = title_case(&words(rng, n));
    s.push('.');
    s
}

fn title_case(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

fn email(rng: &mut Rng) -> String {
    format!(
        "{}.{}@{}",
        rng.pick(FIRST_NAMES).to_ascii_lowercase(),
        rng.pick(LAST_NAMES).to_ascii_lowercase(),
        rng.pick(DOMAINS)
    )
}

fn url(rng: &mut Rng) -> String {
    format!("https://{}/{}", rng.pick(DOMAINS), rng.pick(WORDS))
}

fn phone(rng: &mut Rng) -> String {
    format!("+44 20 7946 {:04}", rng.range_i(0, 9999))
}

fn hex_color(rng: &mut Rng) -> String {
    format!("#{:06x}", rng.below(0x100_0000))
}

fn hex_string(rng: &mut Rng, len: usize) -> String {
    let mut s = String::with_capacity(len);
    while s.len() < len {
        s.push_str(&format!("{:016x}", rng.next_u64()));
    }
    s.truncate(len);
    s
}

fn date(rng: &mut Rng) -> String {
    format!(
        "{:04}-{:02}-{:02}",
        rng.range_i(2019, 2026),
        rng.range_i(1, 12),
        rng.range_i(1, 28)
    )
}

fn date_time(rng: &mut Rng) -> String {
    format!(
        "{}T{:02}:{:02}:{:02}Z",
        date(rng),
        rng.range_i(0, 23),
        rng.range_i(0, 59),
        rng.range_i(0, 59)
    )
}

fn uuid_v4(rng: &mut Rng) -> String {
    let a = rng.next_u64();
    let b = rng.next_u64();
    let mut by = [
        (a >> 56) as u8,
        (a >> 48) as u8,
        (a >> 40) as u8,
        (a >> 32) as u8,
        (a >> 24) as u8,
        (a >> 16) as u8,
        (a >> 8) as u8,
        a as u8,
        (b >> 56) as u8,
        (b >> 48) as u8,
        (b >> 40) as u8,
        (b >> 32) as u8,
        (b >> 24) as u8,
        (b >> 16) as u8,
        (b >> 8) as u8,
        b as u8,
    ];
    by[6] = (by[6] & 0x0f) | 0x40; // version 4
    by[8] = (by[8] & 0x3f) | 0x80; // variant
    format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        by[0], by[1], by[2], by[3], by[4], by[5], by[6], by[7],
        by[8], by[9], by[10], by[11], by[12], by[13], by[14], by[15]
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SDL: &str = r#"
        "A blog post."
        type Post implements Node {
          id: ID!
          title: String!
          body: String
          status: Status!
          author: User!
          tags: [String!]!
          views: Int!
        }

        interface Node {
          id: ID!
        }

        type User implements Node {
          id: ID!
          email: String!
          fullName: String!
          age: Int
        }

        enum Status { DRAFT PUBLISHED ARCHIVED }

        type Query {
          post(id: ID!): Post
          posts(first: Int = 10, filter: PostFilter): [Post!]!
        }

        input PostFilter { status: Status, search: String }
    "#;

    fn gen(mode: &str, ty: &str) -> Value {
        let s = generate(SDL, mode, ty, 2, 3, "fill", true, false, 42, false).expect("generate ok");
        serde_json::from_str(&s).expect("valid JSON")
    }

    #[test]
    fn mocks_every_object_type_and_respects_scalars() {
        let v = gen("all-types", "");
        let obj = v.as_object().unwrap();
        // Object + interface types are mocked; enum and input types are not.
        for k in ["Post", "User", "Query", "Node"] {
            assert!(obj.contains_key(k), "missing mock for {k}");
        }
        assert!(!obj.contains_key("Status"), "enums are leaves, not mocks");
        assert!(!obj.contains_key("PostFilter"), "input types are not response data");

        let post = &obj["Post"];
        assert!(post["title"].is_string());
        assert!(post["views"].is_i64());
        let status = post["status"].as_str().unwrap();
        assert!(
            ["DRAFT", "PUBLISHED", "ARCHIVED"].contains(&status),
            "enum leak {status}"
        );
        let id = post["id"].as_str().unwrap();
        assert_eq!(id.len(), 36, "ID renders as a uuid");
        let tags = post["tags"].as_array().unwrap();
        assert_eq!(tags.len(), 2, "list_length = 2");
        assert!(post["author"].is_object(), "nested object type expanded");
        assert!(post["author"]["email"].as_str().unwrap().contains('@'));
    }

    #[test]
    fn interface_resolves_to_first_implementor_with_typename() {
        let node = gen("all-types", "");
        let node = &node["Node"];
        assert_eq!(
            node["__typename"].as_str().unwrap(),
            "Post",
            "interface mock names the concrete type it chose"
        );
        assert!(node["title"].is_string(), "concrete fields present");
    }

    #[test]
    fn query_response_wraps_in_a_data_envelope() {
        let v = gen("query-response", "");
        let data = v.get("data").expect("data envelope");
        assert!(data["post"].is_object());
        let posts = data["posts"].as_array().unwrap();
        assert_eq!(posts.len(), 2);
        assert!(posts[0]["title"].is_string());
    }

    #[test]
    fn deterministic_for_seed() {
        let a = generate(SDL, "all-types", "", 2, 3, "fill", true, false, 42, true).unwrap();
        let b = generate(SDL, "all-types", "", 2, 3, "fill", true, false, 42, true).unwrap();
        assert_eq!(a, b, "same seed → same output");
        let c = generate(SDL, "all-types", "", 2, 3, "fill", true, false, 43, true).unwrap();
        assert_ne!(a, c, "different seed → different output");
    }

    #[test]
    fn nullable_modes_fill_null_and_omit() {
        // `body` and `age` are nullable; `title` and `id` are not.
        let filled = gen("single-type", "Post");
        assert!(filled["body"].is_string());

        let nulled: Value = serde_json::from_str(
            &generate(SDL, "single-type", "Post", 2, 3, "null", true, false, 42, false).unwrap(),
        )
        .unwrap();
        assert!(nulled["body"].is_null(), "nullable field forced to null");
        assert!(nulled["title"].is_string(), "non-null field still generated");

        let omitted: Value = serde_json::from_str(
            &generate(SDL, "single-type", "Post", 2, 3, "omit", true, false, 42, false).unwrap(),
        )
        .unwrap();
        assert!(omitted.get("body").is_none(), "nullable field dropped");
        assert!(omitted.get("title").is_some(), "non-null field kept");
    }

    #[test]
    fn typename_flag_and_list_length_and_depth() {
        let with_tn: Value = serde_json::from_str(
            &generate(SDL, "single-type", "Post", 0, 3, "fill", true, true, 42, false).unwrap(),
        )
        .unwrap();
        assert_eq!(with_tn["__typename"].as_str().unwrap(), "Post");
        assert_eq!(with_tn["author"]["__typename"].as_str().unwrap(), "User");
        assert_eq!(with_tn["tags"].as_array().unwrap().len(), 0, "list_length 0");

        // depth 1 collapses the nested author object.
        let shallow: Value = serde_json::from_str(
            &generate(SDL, "single-type", "Post", 2, 1, "fill", true, false, 42, false).unwrap(),
        )
        .unwrap();
        assert_eq!(
            shallow["author"].as_object().unwrap().len(),
            0,
            "depth cap collapses nested objects"
        );
    }

    #[test]
    fn smart_values_can_be_switched_off() {
        let dumb: Value = serde_json::from_str(
            &generate(SDL, "single-type", "User", 2, 3, "fill", false, false, 42, false).unwrap(),
        )
        .unwrap();
        assert!(
            !dumb["email"].as_str().unwrap().contains('@'),
            "smart_values=false gives generic strings"
        );
        let smart = gen("single-type", "User");
        assert!(smart["email"].as_str().unwrap().contains('@'));
        assert!(smart["fullName"].as_str().unwrap().contains(' '));
        let age = smart["age"].as_i64().unwrap();
        assert!((18..=80).contains(&age), "age {age} outside the human range");
    }

    #[test]
    fn custom_scalars_and_unions() {
        let sdl = r#"
            scalar DateTime
            scalar Weird
            type Image { url: String! }
            type Video { seconds: Int! }
            union Media = Image | Video
            type Asset { media: Media!, createdAt: DateTime!, extra: Weird! }
        "#;
        let v: Value = serde_json::from_str(
            &generate(sdl, "single-type", "Asset", 1, 3, "fill", true, false, 7, false).unwrap(),
        )
        .unwrap();
        let dt = v["createdAt"].as_str().unwrap();
        assert!(dt.ends_with('Z') && dt.len() == 20, "DateTime shape {dt}");
        assert_eq!(v["extra"].as_str().unwrap(), "mock-weird");
        assert_eq!(
            v["media"]["__typename"].as_str().unwrap(),
            "Image",
            "union picks its first member and labels it"
        );
    }

    #[test]
    fn parses_descriptions_comments_extends_and_directives() {
        let sdl = r#"
            # a comment
            """
            Block description
            """
            type Thing @key(fields: "id") {
              "field description"
              id: ID!
              matrix: [[Int!]!]!
            }
            extend type Thing { extra: String! }
            schema { query: Root }
            type Root { thing: Thing! }
            directive @key(fields: String!) on OBJECT
        "#;
        let v: Value = serde_json::from_str(
            &generate(sdl, "query-response", "", 2, 3, "fill", true, false, 5, false).unwrap(),
        )
        .unwrap();
        let thing = &v["data"]["thing"];
        assert!(thing["extra"].is_string(), "extend merged its field");
        let m = thing["matrix"].as_array().unwrap();
        assert_eq!(m.len(), 2);
        assert_eq!(m[0].as_array().unwrap().len(), 2, "nested list");
        assert!(m[0][0].is_i64());
    }

    #[test]
    fn honours_fake_examples_and_list_length_directives() {
        let sdl = r#"
            type Person {
              handle: String! @fake(type: firstName)
              salutation: String! @examples(values: ["Mx", "Dr"])
              luckyNumber: Int! @examples(values: [7])
              nicknames: [String!]! @listLength(min: 4, max: 4) @fake(type: word)
              website: String! @fake(type: url)
              # An unknown @fake type falls back to field-name inference.
              email: String! @fake(type: notARealFakerType)
            }
        "#;
        let v: Value = serde_json::from_str(
            // list_length 2 globally — the directive must win for `nicknames`.
            &generate(sdl, "single-type", "Person", 2, 3, "fill", true, false, 9, false).unwrap(),
        )
        .unwrap();
        assert!(
            FIRST_NAMES.contains(&v["handle"].as_str().unwrap()),
            "@fake(type: firstName) picked {:?}",
            v["handle"]
        );
        assert!(
            ["Mx", "Dr"].contains(&v["salutation"].as_str().unwrap()),
            "@examples confined to its list"
        );
        assert_eq!(v["luckyNumber"], Value::from(7), "numeric @examples stay numbers");
        assert_eq!(
            v["nicknames"].as_array().unwrap().len(),
            4,
            "@listLength overrides the global list_length"
        );
        assert!(v["website"].as_str().unwrap().starts_with("https://"));
        assert!(
            v["email"].as_str().unwrap().contains('@'),
            "unknown @fake type falls back to name inference"
        );
    }

    #[test]
    fn directive_escapes_and_object_args_parse() {
        // Escapes are decoded, object/list directive args are tolerated, and
        // @listLength is clamped to the global maximum.
        let sdl = r#"
            type T {
              quote: String! @examples(values: ["a \"quoted\" é word"])
              styled: String! @fake(type: word, options: { case: "upper" }, locale: "en")
              many: [Int!]! @listLength(min: 99, max: 99)
            }
        "#;
        let v: Value = serde_json::from_str(
            &generate(sdl, "single-type", "T", 2, 3, "fill", true, false, 3, false).unwrap(),
        )
        .unwrap();
        assert_eq!(v["quote"].as_str().unwrap(), "a \"quoted\" é word");
        assert!(v["styled"].is_string());
        assert_eq!(
            v["many"].as_array().unwrap().len(),
            MAX_LIST_LENGTH as usize,
            "@listLength clamped to the cap"
        );
    }

    #[test]
    fn rejects_bad_input() {
        // Executable document, not SDL.
        assert!(generate("query { me { id } }", "all-types", "", 2, 3, "fill", true, false, 1, false)
            .is_err());
        // Undefined field type.
        assert!(generate(
            "type A { b: Missing! }",
            "all-types",
            "",
            2,
            3,
            "fill",
            true,
            false,
            1,
            false
        )
        .is_err());
        // Empty / structurally broken SDL.
        assert!(generate("", "all-types", "", 2, 3, "fill", true, false, 1, false).is_err());
        assert!(generate("type A { b: }", "all-types", "", 2, 3, "fill", true, false, 1, false)
            .is_err());
        assert!(generate("type A {", "all-types", "", 2, 3, "fill", true, false, 1, false).is_err());
        // Duplicate definition.
        assert!(generate(
            "type A { x: Int! } type A { y: Int! }",
            "all-types",
            "",
            2,
            3,
            "fill",
            true,
            false,
            1,
            false
        )
        .is_err());
    }

    #[test]
    fn rejects_bad_options_and_honours_caps() {
        let ok = |ll, d, m, nf, tn: &str| generate(SDL, m, tn, ll, d, nf, true, false, 1, false);
        assert!(ok(MAX_LIST_LENGTH + 1, 3, "all-types", "fill", "").is_err(), "list cap");
        assert!(ok(MAX_LIST_LENGTH, 3, "all-types", "fill", "").is_ok(), "list cap boundary");
        assert!(ok(2, MAX_DEPTH + 1, "all-types", "fill", "").is_err(), "depth cap");
        assert!(ok(2, MAX_DEPTH, "all-types", "fill", "").is_ok(), "depth cap boundary");
        assert!(ok(2, 0, "all-types", "fill", "").is_err(), "depth zero");
        assert!(ok(2, 3, "yaml", "fill", "").is_err(), "bad mode");
        assert!(ok(2, 3, "all-types", "maybe", "").is_err(), "bad nullable_fields");
        assert!(ok(2, 3, "single-type", "fill", "").is_err(), "single-type needs type_name");
        assert!(ok(2, 3, "single-type", "fill", "Nope").is_err(), "unknown type_name");
        // Query-response with no Query root.
        assert!(generate(
            "type A { x: Int! }",
            "query-response",
            "",
            2,
            3,
            "fill",
            true,
            false,
            1,
            false
        )
        .is_err());
        // Over the SDL byte cap.
        let big = format!("type A {{ x: Int! }}\n{}", "# pad\n".repeat(40_000));
        assert!(big.len() > MAX_SDL_BYTES);
        assert!(generate(&big, "all-types", "", 2, 3, "fill", true, false, 1, false).is_err());
    }
}
