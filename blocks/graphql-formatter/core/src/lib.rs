//! gizza-ai/graphql-formatter core — pretty-print, minify, and syntax-check
//! GraphQL documents (queries, mutations, subscriptions, fragments and SDL
//! schema definitions).
//!
//! Pure compute, no dependencies. The document is lexed and parsed with a
//! hand-rolled recursive-descent parser that follows the GraphQL grammar, then
//! re-printed from the resulting AST. Because printing works from the AST and
//! never from the raw text, the output is canonical: formatting an already
//! formatted document is a no-op.
//!
//! Options:
//!   * `indent`  — `2` | `4` | `8` spaces, or `tab`.
//!   * `mode`    — `format` (pretty-print) | `minify` (strip every ignored
//!                 character that is not needed to keep the document parseable).
//!   * `sort_fields`     — sort selection sets and SDL field definitions
//!                         alphabetically.
//!   * `remove_comments` — drop `#` comments (minify always drops them).
//!
//! Syntax errors are reported as `Syntax error at line L, column C: message`.

/// Largest accepted document, in bytes.
pub const MAX_INPUT_BYTES: usize = 500_000;
/// Largest accepted nesting depth (selection sets, list/object values, types).
pub const MAX_DEPTH: usize = 64;

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Name(String),
    Int(String),
    Float(String),
    Str(StrVal),
    Punct(&'static str),
    Comment(String),
}

#[derive(Debug, Clone, PartialEq)]
struct StrVal {
    value: String,
    block: bool,
}

#[derive(Debug, Clone)]
struct Token {
    kind: Tok,
    line: usize,
    col: usize,
}

/// A syntax error with a 1-based source position.
#[derive(Debug, Clone)]
struct PErr {
    line: usize,
    col: usize,
    msg: String,
}

impl PErr {
    fn render(&self) -> String {
        format!(
            "Syntax error at line {}, column {}: {}",
            self.line, self.col, self.msg
        )
    }
}

const PUNCTUATORS: [&str; 12] = ["!", "$", "&", "(", ")", ":", "=", "@", "[", "]", "{", "|"];

struct Lexer {
    src: Vec<char>,
    i: usize,
    line: usize,
    col: usize,
}

impl Lexer {
    fn new(s: &str) -> Self {
        Lexer {
            src: s.chars().collect(),
            i: 0,
            line: 1,
            col: 1,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src.get(self.i).copied()
    }

    fn at(&self, k: usize) -> Option<char> {
        self.src.get(self.i + k).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.src.get(self.i).copied()?;
        self.i += 1;
        match c {
            '\n' => {
                self.line += 1;
                self.col = 1;
            }
            // A CR that starts a CRLF pair leaves the line bump to the LF.
            '\r' if self.peek() == Some('\n') => {}
            '\r' => {
                self.line += 1;
                self.col = 1;
            }
            _ => self.col += 1,
        }
        Some(c)
    }

    fn err(&self, line: usize, col: usize, msg: impl Into<String>) -> PErr {
        let _ = self;
        PErr {
            line,
            col,
            msg: msg.into(),
        }
    }

    fn tokenize(mut self) -> Result<Vec<Token>, PErr> {
        let mut out = Vec::new();
        loop {
            // Ignored tokens: BOM, whitespace, line terminators and commas.
            while let Some(c) = self.peek() {
                if matches!(c, '\u{FEFF}' | ' ' | '\t' | '\n' | '\r' | ',') {
                    self.bump();
                } else {
                    break;
                }
            }
            let (line, col) = (self.line, self.col);
            let c = match self.peek() {
                None => break,
                Some(c) => c,
            };
            let kind = if c == '#' {
                self.bump();
                let mut text = String::new();
                while let Some(c) = self.peek() {
                    if c == '\n' || c == '\r' {
                        break;
                    }
                    text.push(c);
                    self.bump();
                }
                Tok::Comment(text.trim().to_string())
            } else if c == '.' {
                if self.at(1) == Some('.') && self.at(2) == Some('.') {
                    self.bump();
                    self.bump();
                    self.bump();
                    Tok::Punct("...")
                } else {
                    return Err(self.err(line, col, "expected \"...\" but found a single \".\""));
                }
            } else if c == '}' {
                self.bump();
                Tok::Punct("}")
            } else if let Some(p) = PUNCTUATORS.iter().find(|p| p.starts_with(c)) {
                self.bump();
                Tok::Punct(p)
            } else if c == '_' || c.is_ascii_alphabetic() {
                let mut name = String::new();
                while let Some(c) = self.peek() {
                    if c == '_' || c.is_ascii_alphanumeric() {
                        name.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
                Tok::Name(name)
            } else if c == '-' || c.is_ascii_digit() {
                self.lex_number(line, col)?
            } else if c == '"' {
                self.lex_string(line, col)?
            } else {
                return Err(self.err(
                    line,
                    col,
                    format!("unexpected character {:?} in GraphQL source", c),
                ));
            };
            out.push(Token { kind, line, col });
        }
        Ok(out)
    }

    fn lex_number(&mut self, line: usize, col: usize) -> Result<Tok, PErr> {
        let mut s = String::new();
        if self.peek() == Some('-') {
            s.push('-');
            self.bump();
        }
        match self.peek() {
            Some('0') => {
                s.push('0');
                self.bump();
                if matches!(self.peek(), Some(c) if c.is_ascii_digit()) {
                    return Err(self.err(line, col, "invalid number: no digits may follow a leading 0"));
                }
            }
            Some(c) if c.is_ascii_digit() => {
                while let Some(c) = self.peek() {
                    if c.is_ascii_digit() {
                        s.push(c);
                        self.bump();
                    } else {
                        break;
                    }
                }
            }
            _ => return Err(self.err(line, col, "invalid number: expected a digit after \"-\"")),
        }
        let mut is_float = false;
        if self.peek() == Some('.') {
            is_float = true;
            s.push('.');
            self.bump();
            let before = s.len();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    s.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
            if s.len() == before {
                return Err(self.err(line, col, "invalid number: expected a digit after \".\""));
            }
        }
        if matches!(self.peek(), Some('e') | Some('E')) {
            is_float = true;
            s.push('e');
            self.bump();
            if matches!(self.peek(), Some('+') | Some('-')) {
                s.push(self.peek().unwrap());
                self.bump();
            }
            let before = s.len();
            while let Some(c) = self.peek() {
                if c.is_ascii_digit() {
                    s.push(c);
                    self.bump();
                } else {
                    break;
                }
            }
            if s.len() == before {
                return Err(self.err(line, col, "invalid number: expected a digit in the exponent"));
            }
        }
        if let Some(c) = self.peek() {
            if c == '.' || c == '_' || c.is_ascii_alphanumeric() {
                return Err(self.err(
                    line,
                    col,
                    format!("invalid number: unexpected character {c:?} after the numeric value"),
                ));
            }
        }
        Ok(if is_float { Tok::Float(s) } else { Tok::Int(s) })
    }

    fn lex_string(&mut self, line: usize, col: usize) -> Result<Tok, PErr> {
        if self.at(1) == Some('"') && self.at(2) == Some('"') {
            self.bump();
            self.bump();
            self.bump();
            let mut raw = String::new();
            loop {
                match self.peek() {
                    None => return Err(self.err(line, col, "unterminated block string (missing closing \"\"\")")),
                    Some('\\')
                        if self.at(1) == Some('"')
                            && self.at(2) == Some('"')
                            && self.at(3) == Some('"') =>
                    {
                        raw.push_str("\"\"\"");
                        for _ in 0..4 {
                            self.bump();
                        }
                    }
                    Some('"') if self.at(1) == Some('"') && self.at(2) == Some('"') => {
                        self.bump();
                        self.bump();
                        self.bump();
                        break;
                    }
                    Some(c) => {
                        raw.push(c);
                        self.bump();
                    }
                }
            }
            Ok(Tok::Str(StrVal {
                value: dedent_block_string(&raw),
                block: true,
            }))
        } else {
            self.bump();
            let mut value = String::new();
            loop {
                match self.peek() {
                    None | Some('\n') | Some('\r') => {
                        return Err(self.err(line, col, "unterminated string (missing closing \")"))
                    }
                    Some('"') => {
                        self.bump();
                        break;
                    }
                    Some('\\') => {
                        self.bump();
                        let (eline, ecol) = (self.line, self.col);
                        let e = self
                            .bump()
                            .ok_or_else(|| self.err(line, col, "unterminated string escape"))?;
                        match e {
                            '"' => value.push('"'),
                            '\\' => value.push('\\'),
                            '/' => value.push('/'),
                            'b' => value.push('\u{8}'),
                            'f' => value.push('\u{c}'),
                            'n' => value.push('\n'),
                            'r' => value.push('\r'),
                            't' => value.push('\t'),
                            'u' => value.push(self.lex_unicode_escape(eline, ecol)?),
                            other => {
                                return Err(self.err(
                                    eline,
                                    ecol,
                                    format!("invalid string escape \"\\{other}\""),
                                ))
                            }
                        }
                    }
                    Some(c) => {
                        value.push(c);
                        self.bump();
                    }
                }
            }
            Ok(Tok::Str(StrVal {
                value,
                block: false,
            }))
        }
    }

    fn lex_unicode_escape(&mut self, line: usize, col: usize) -> Result<char, PErr> {
        let bad = |s: &Self| s.err(line, col, "invalid unicode escape in string");
        if self.peek() == Some('{') {
            self.bump();
            let mut hex = String::new();
            while let Some(c) = self.peek() {
                if c == '}' {
                    break;
                }
                hex.push(c);
                self.bump();
            }
            if self.peek() != Some('}') {
                return Err(bad(self));
            }
            self.bump();
            let n = u32::from_str_radix(&hex, 16).map_err(|_| bad(self))?;
            return char::from_u32(n).ok_or_else(|| bad(self));
        }
        let mut hex = String::new();
        for _ in 0..4 {
            hex.push(self.bump().ok_or_else(|| bad(self))?);
        }
        let n = u32::from_str_radix(&hex, 16).map_err(|_| bad(self))?;
        // Surrogate pair: \uD83D\uDE00
        if (0xD800..0xDC00).contains(&n) {
            if self.peek() == Some('\\') && self.at(1) == Some('u') {
                self.bump();
                self.bump();
                let mut low = String::new();
                for _ in 0..4 {
                    low.push(self.bump().ok_or_else(|| bad(self))?);
                }
                let l = u32::from_str_radix(&low, 16).map_err(|_| bad(self))?;
                if (0xDC00..0xE000).contains(&l) {
                    let cp = 0x10000 + ((n - 0xD800) << 10) + (l - 0xDC00);
                    return char::from_u32(cp).ok_or_else(|| bad(self));
                }
            }
            return Err(bad(self));
        }
        char::from_u32(n).ok_or_else(|| bad(self))
    }
}

/// Apply the spec's BlockStringValue transform: normalise line terminators,
/// remove the common indentation of every line after the first, then drop
/// leading and trailing blank lines.
fn dedent_block_string(raw: &str) -> String {
    let normalised = raw.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalised.split('\n').collect();
    let mut common = usize::MAX;
    for line in lines.iter().skip(1) {
        let indent = line.chars().take_while(|c| *c == ' ' || *c == '\t').count();
        if indent < line.chars().count() {
            common = common.min(indent);
        }
    }
    if common == usize::MAX {
        common = 0;
    }
    let mut out: Vec<String> = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        if i == 0 {
            out.push((*line).to_string());
        } else {
            out.push(line.chars().skip(common).collect());
        }
    }
    while out.first().map_or(false, |l| l.trim().is_empty()) {
        out.remove(0);
    }
    while out.last().map_or(false, |l| l.trim().is_empty()) {
        out.pop();
    }
    out.join("\n")
}

// ---------------------------------------------------------------------------
// AST
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Value {
    Var(String),
    Int(String),
    Float(String),
    Str(StrVal),
    Bool(bool),
    Null,
    Enum(String),
    List(Vec<Value>),
    Object(Vec<ObjField>),
}

#[derive(Debug, Clone)]
struct ObjField {
    name: String,
    value: Value,
}

#[derive(Debug, Clone)]
struct Argument {
    name: String,
    value: Value,
}

#[derive(Debug, Clone)]
struct Directive {
    name: String,
    args: Vec<Argument>,
}

#[derive(Debug, Clone)]
enum TypeRef {
    Named(String),
    List(Box<TypeRef>),
    NonNull(Box<TypeRef>),
}

#[derive(Debug, Clone)]
struct VariableDef {
    name: String,
    ty: TypeRef,
    default: Option<Value>,
    directives: Vec<Directive>,
}

#[derive(Debug, Clone)]
struct SelectionSet {
    items: Vec<Selection>,
    trailing: Vec<String>,
}

#[derive(Debug, Clone)]
enum Selection {
    Field(Field),
    Spread(FragmentSpread),
    Inline(InlineFragment),
}

impl Selection {
    /// (kind rank, alphabetical key) — the sort key used by `sort_fields`.
    fn sort_key(&self) -> (u8, String) {
        match self {
            Selection::Field(f) => (0, f.alias.clone().unwrap_or_else(|| f.name.clone())),
            Selection::Spread(s) => (1, s.name.clone()),
            Selection::Inline(i) => (2, i.type_cond.clone().unwrap_or_default()),
        }
    }
}

#[derive(Debug, Clone)]
struct Field {
    comments: Vec<String>,
    alias: Option<String>,
    name: String,
    args: Vec<Argument>,
    directives: Vec<Directive>,
    sels: Option<SelectionSet>,
}

#[derive(Debug, Clone)]
struct FragmentSpread {
    comments: Vec<String>,
    name: String,
    directives: Vec<Directive>,
}

#[derive(Debug, Clone)]
struct InlineFragment {
    comments: Vec<String>,
    type_cond: Option<String>,
    directives: Vec<Directive>,
    sels: SelectionSet,
}

#[derive(Debug, Clone)]
struct FieldDef {
    comments: Vec<String>,
    description: Option<StrVal>,
    name: String,
    args: Vec<InputValueDef>,
    ty: TypeRef,
    directives: Vec<Directive>,
}

#[derive(Debug, Clone)]
struct InputValueDef {
    comments: Vec<String>,
    description: Option<StrVal>,
    name: String,
    ty: TypeRef,
    default: Option<Value>,
    directives: Vec<Directive>,
}

#[derive(Debug, Clone)]
struct EnumValueDef {
    comments: Vec<String>,
    description: Option<StrVal>,
    name: String,
    directives: Vec<Directive>,
}

#[derive(Debug, Clone)]
struct RootOpDef {
    comments: Vec<String>,
    operation: String,
    ty: String,
}

#[derive(Debug, Clone)]
enum Def {
    Operation {
        op: String,
        name: Option<String>,
        vars: Vec<VariableDef>,
        directives: Vec<Directive>,
        sels: SelectionSet,
        shorthand: bool,
    },
    Fragment {
        name: String,
        type_cond: String,
        directives: Vec<Directive>,
        sels: SelectionSet,
    },
    Schema {
        extend: bool,
        description: Option<StrVal>,
        directives: Vec<Directive>,
        ops: Option<Vec<RootOpDef>>,
        trailing: Vec<String>,
    },
    Scalar {
        extend: bool,
        description: Option<StrVal>,
        name: String,
        directives: Vec<Directive>,
    },
    /// `type` and `interface` share a shape; `keyword` keeps them apart.
    Object {
        extend: bool,
        keyword: &'static str,
        description: Option<StrVal>,
        name: String,
        interfaces: Vec<String>,
        directives: Vec<Directive>,
        fields: Option<Vec<FieldDef>>,
        trailing: Vec<String>,
    },
    Union {
        extend: bool,
        description: Option<StrVal>,
        name: String,
        directives: Vec<Directive>,
        members: Vec<String>,
    },
    Enum {
        extend: bool,
        description: Option<StrVal>,
        name: String,
        directives: Vec<Directive>,
        values: Option<Vec<EnumValueDef>>,
        trailing: Vec<String>,
    },
    InputObject {
        extend: bool,
        description: Option<StrVal>,
        name: String,
        directives: Vec<Directive>,
        fields: Option<Vec<InputValueDef>>,
        trailing: Vec<String>,
    },
    DirectiveDef {
        description: Option<StrVal>,
        name: String,
        args: Vec<InputValueDef>,
        repeatable: bool,
        locations: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct TopDef {
    comments: Vec<String>,
    def: Def,
}

#[derive(Debug, Clone)]
struct Document {
    defs: Vec<TopDef>,
    trailing: Vec<String>,
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

const DIRECTIVE_LOCATIONS: [&str; 19] = [
    "QUERY",
    "MUTATION",
    "SUBSCRIPTION",
    "FIELD",
    "FRAGMENT_DEFINITION",
    "FRAGMENT_SPREAD",
    "INLINE_FRAGMENT",
    "VARIABLE_DEFINITION",
    "SCHEMA",
    "SCALAR",
    "OBJECT",
    "FIELD_DEFINITION",
    "ARGUMENT_DEFINITION",
    "INTERFACE",
    "UNION",
    "ENUM",
    "ENUM_VALUE",
    "INPUT_OBJECT",
    "INPUT_FIELD_DEFINITION",
];

struct Parser {
    toks: Vec<Token>,
    pos: usize,
    pending: Vec<String>,
    depth: usize,
}

impl Parser {
    fn new(toks: Vec<Token>) -> Self {
        Parser {
            toks,
            pos: 0,
            pending: Vec::new(),
            depth: 0,
        }
    }

    /// Move past any comment tokens, buffering their text for the next item.
    fn skip_comments(&mut self) {
        while let Some(Token {
            kind: Tok::Comment(c),
            ..
        }) = self.toks.get(self.pos)
        {
            self.pending.push(c.clone());
            self.pos += 1;
        }
    }

    fn take_comments(&mut self) -> Vec<String> {
        std::mem::take(&mut self.pending)
    }

    fn cur(&mut self) -> Option<&Token> {
        self.skip_comments();
        self.toks.get(self.pos)
    }

    fn cur_kind(&mut self) -> Option<Tok> {
        self.cur().map(|t| t.kind.clone())
    }

    fn pos_here(&mut self) -> (usize, usize) {
        self.skip_comments();
        match self.toks.get(self.pos) {
            Some(t) => (t.line, t.col),
            None => self
                .toks
                .last()
                .map(|t| (t.line, t.col))
                .unwrap_or((1, 1)),
        }
    }

    fn err(&mut self, msg: impl Into<String>) -> PErr {
        let (line, col) = self.pos_here();
        PErr {
            line,
            col,
            msg: msg.into(),
        }
    }

    fn describe_here(&mut self) -> String {
        match self.cur_kind() {
            None => "end of input".to_string(),
            Some(Tok::Name(n)) => format!("name \"{n}\""),
            Some(Tok::Int(n)) | Some(Tok::Float(n)) => format!("number {n}"),
            Some(Tok::Str(_)) => "a string value".to_string(),
            Some(Tok::Punct(p)) => format!("\"{p}\""),
            Some(Tok::Comment(_)) => "a comment".to_string(),
        }
    }

    fn at_punct(&mut self, p: &str) -> bool {
        matches!(self.cur_kind(), Some(Tok::Punct(q)) if q == p)
    }

    fn at_name(&mut self, n: &str) -> bool {
        matches!(self.cur_kind(), Some(Tok::Name(ref q)) if q == n)
    }

    fn eat_punct(&mut self, p: &str) -> bool {
        if self.at_punct(p) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn eat_name(&mut self, n: &str) -> bool {
        if self.at_name(n) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect_punct(&mut self, p: &str) -> Result<(), PErr> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            let found = self.describe_here();
            Err(self.err(format!("expected \"{p}\" but found {found}")))
        }
    }

    fn expect_name(&mut self) -> Result<String, PErr> {
        match self.cur_kind() {
            Some(Tok::Name(n)) => {
                self.pos += 1;
                Ok(n)
            }
            _ => {
                let found = self.describe_here();
                Err(self.err(format!("expected a name but found {found}")))
            }
        }
    }

    fn enter(&mut self) -> Result<(), PErr> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(self.err(format!(
                "document nests deeper than the supported limit of {MAX_DEPTH} levels"
            )));
        }
        Ok(())
    }

    fn leave(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    // -- document ----------------------------------------------------------

    fn parse_document(&mut self) -> Result<Document, PErr> {
        let mut defs = Vec::new();
        while self.cur().is_some() {
            let comments = self.take_comments();
            let def = self.parse_definition()?;
            defs.push(TopDef { comments, def });
        }
        self.skip_comments();
        let trailing = self.take_comments();
        if defs.is_empty() {
            return Err(PErr {
                line: 1,
                col: 1,
                msg: "the document contains no GraphQL definitions".to_string(),
            });
        }
        Ok(Document { defs, trailing })
    }

    fn parse_definition(&mut self) -> Result<Def, PErr> {
        if self.at_punct("{") {
            let sels = self.parse_selection_set()?;
            return Ok(Def::Operation {
                op: "query".to_string(),
                name: None,
                vars: Vec::new(),
                directives: Vec::new(),
                sels,
                shorthand: true,
            });
        }
        if let Some(Tok::Str(s)) = self.cur_kind() {
            self.pos += 1;
            return self.parse_type_system_definition(false, Some(s));
        }
        let name = match self.cur_kind() {
            Some(Tok::Name(n)) => n,
            _ => {
                let found = self.describe_here();
                return Err(self.err(format!(
                    "expected a definition (query, mutation, subscription, fragment, type, interface, union, enum, input, scalar, schema, directive or extend) but found {found}"
                )));
            }
        };
        match name.as_str() {
            "query" | "mutation" | "subscription" => self.parse_operation(),
            "fragment" => self.parse_fragment(),
            "extend" => {
                self.pos += 1;
                self.parse_type_system_definition(true, None)
            }
            "schema" | "scalar" | "type" | "interface" | "union" | "enum" | "input"
            | "directive" => self.parse_type_system_definition(false, None),
            other => Err(self.err(format!(
                "unexpected name \"{other}\" at the top level of the document"
            ))),
        }
    }

    // -- executable definitions -------------------------------------------

    fn parse_operation(&mut self) -> Result<Def, PErr> {
        let op = self.expect_name()?;
        let name = match self.cur_kind() {
            Some(Tok::Name(n)) => {
                self.pos += 1;
                Some(n)
            }
            _ => None,
        };
        let vars = if self.at_punct("(") {
            self.parse_variable_definitions()?
        } else {
            Vec::new()
        };
        let directives = self.parse_directives(false)?;
        let sels = self.parse_selection_set()?;
        Ok(Def::Operation {
            op,
            name,
            vars,
            directives,
            sels,
            shorthand: false,
        })
    }

    fn parse_fragment(&mut self) -> Result<Def, PErr> {
        self.pos += 1; // "fragment"
        let name = self.expect_name()?;
        if name == "on" {
            return Err(self.err("a fragment cannot be named \"on\""));
        }
        if !self.eat_name("on") {
            let found = self.describe_here();
            return Err(self.err(format!(
                "expected \"on\" after the fragment name but found {found}"
            )));
        }
        let type_cond = self.expect_name()?;
        let directives = self.parse_directives(false)?;
        let sels = self.parse_selection_set()?;
        Ok(Def::Fragment {
            name,
            type_cond,
            directives,
            sels,
        })
    }

    fn parse_variable_definitions(&mut self) -> Result<Vec<VariableDef>, PErr> {
        self.expect_punct("(")?;
        let mut out = Vec::new();
        while !self.at_punct(")") {
            if self.cur().is_none() {
                return Err(self.err("unterminated variable definitions (missing \")\")"));
            }
            self.take_comments();
            self.expect_punct("$")?;
            let name = self.expect_name()?;
            self.expect_punct(":")?;
            let ty = self.parse_type()?;
            let default = if self.eat_punct("=") {
                Some(self.parse_value(true)?)
            } else {
                None
            };
            let directives = self.parse_directives(true)?;
            out.push(VariableDef {
                name,
                ty,
                default,
                directives,
            });
        }
        self.expect_punct(")")?;
        if out.is_empty() {
            return Err(self.err("variable definitions \"()\" must declare at least one variable"));
        }
        Ok(out)
    }

    fn parse_selection_set(&mut self) -> Result<SelectionSet, PErr> {
        self.enter()?;
        self.expect_punct("{")?;
        let mut items = Vec::new();
        while !self.at_punct("}") {
            if self.cur().is_none() {
                return Err(self.err("unterminated selection set (missing \"}\")"));
            }
            let comments = self.take_comments();
            items.push(self.parse_selection(comments)?);
        }
        let trailing = self.take_comments();
        self.expect_punct("}")?;
        self.leave();
        if items.is_empty() {
            return Err(self.err("a selection set \"{}\" must contain at least one field"));
        }
        Ok(SelectionSet { items, trailing })
    }

    fn parse_selection(&mut self, comments: Vec<String>) -> Result<Selection, PErr> {
        if self.eat_punct("...") {
            if self.at_name("on") {
                self.pos += 1;
                let type_cond = self.expect_name()?;
                let directives = self.parse_directives(false)?;
                let sels = self.parse_selection_set()?;
                return Ok(Selection::Inline(InlineFragment {
                    comments,
                    type_cond: Some(type_cond),
                    directives,
                    sels,
                }));
            }
            if let Some(Tok::Name(n)) = self.cur_kind() {
                self.pos += 1;
                let directives = self.parse_directives(false)?;
                return Ok(Selection::Spread(FragmentSpread {
                    comments,
                    name: n,
                    directives,
                }));
            }
            let directives = self.parse_directives(false)?;
            let sels = self.parse_selection_set()?;
            return Ok(Selection::Inline(InlineFragment {
                comments,
                type_cond: None,
                directives,
                sels,
            }));
        }
        let first = match self.cur_kind() {
            Some(Tok::Name(n)) => {
                self.pos += 1;
                n
            }
            _ => {
                let found = self.describe_here();
                return Err(self.err(format!("expected a field name but found {found}")));
            }
        };
        let (alias, name) = if self.eat_punct(":") {
            (Some(first), self.expect_name()?)
        } else {
            (None, first)
        };
        let args = if self.at_punct("(") {
            self.parse_arguments(false)?
        } else {
            Vec::new()
        };
        let directives = self.parse_directives(false)?;
        let sels = if self.at_punct("{") {
            Some(self.parse_selection_set()?)
        } else {
            None
        };
        Ok(Selection::Field(Field {
            comments,
            alias,
            name,
            args,
            directives,
            sels,
        }))
    }

    fn parse_arguments(&mut self, constant: bool) -> Result<Vec<Argument>, PErr> {
        self.expect_punct("(")?;
        let mut out = Vec::new();
        while !self.at_punct(")") {
            if self.cur().is_none() {
                return Err(self.err("unterminated argument list (missing \")\")"));
            }
            self.take_comments();
            let name = self.expect_name()?;
            self.expect_punct(":")?;
            let value = self.parse_value(constant)?;
            out.push(Argument { name, value });
        }
        self.expect_punct(")")?;
        if out.is_empty() {
            return Err(self.err("an argument list \"()\" must contain at least one argument"));
        }
        Ok(out)
    }

    fn parse_directives(&mut self, constant: bool) -> Result<Vec<Directive>, PErr> {
        let mut out = Vec::new();
        while self.at_punct("@") {
            self.pos += 1;
            let name = self.expect_name()?;
            let args = if self.at_punct("(") {
                self.parse_arguments(constant)?
            } else {
                Vec::new()
            };
            out.push(Directive { name, args });
        }
        Ok(out)
    }

    fn parse_value(&mut self, constant: bool) -> Result<Value, PErr> {
        self.enter()?;
        let v = match self.cur_kind() {
            Some(Tok::Punct("$")) => {
                if constant {
                    return Err(self.err("a variable is not allowed in this constant position"));
                }
                self.pos += 1;
                Value::Var(self.expect_name()?)
            }
            Some(Tok::Int(n)) => {
                self.pos += 1;
                Value::Int(n)
            }
            Some(Tok::Float(n)) => {
                self.pos += 1;
                Value::Float(n)
            }
            Some(Tok::Str(s)) => {
                self.pos += 1;
                Value::Str(s)
            }
            Some(Tok::Name(n)) => {
                self.pos += 1;
                match n.as_str() {
                    "true" => Value::Bool(true),
                    "false" => Value::Bool(false),
                    "null" => Value::Null,
                    _ => Value::Enum(n),
                }
            }
            Some(Tok::Punct("[")) => {
                self.pos += 1;
                let mut items = Vec::new();
                while !self.at_punct("]") {
                    if self.cur().is_none() {
                        return Err(self.err("unterminated list value (missing \"]\")"));
                    }
                    self.take_comments();
                    items.push(self.parse_value(constant)?);
                }
                self.expect_punct("]")?;
                Value::List(items)
            }
            Some(Tok::Punct("{")) => {
                self.pos += 1;
                let mut fields = Vec::new();
                while !self.at_punct("}") {
                    if self.cur().is_none() {
                        return Err(self.err("unterminated object value (missing \"}\")"));
                    }
                    self.take_comments();
                    let name = self.expect_name()?;
                    self.expect_punct(":")?;
                    let value = self.parse_value(constant)?;
                    fields.push(ObjField { name, value });
                }
                self.expect_punct("}")?;
                Value::Object(fields)
            }
            _ => {
                let found = self.describe_here();
                return Err(self.err(format!("expected a value but found {found}")));
            }
        };
        self.leave();
        Ok(v)
    }

    fn parse_type(&mut self) -> Result<TypeRef, PErr> {
        self.enter()?;
        let base = if self.eat_punct("[") {
            let inner = self.parse_type()?;
            self.expect_punct("]")?;
            TypeRef::List(Box::new(inner))
        } else {
            match self.cur_kind() {
                Some(Tok::Name(n)) => {
                    self.pos += 1;
                    TypeRef::Named(n)
                }
                _ => {
                    let found = self.describe_here();
                    return Err(self.err(format!("expected a type name but found {found}")));
                }
            }
        };
        let ty = if self.eat_punct("!") {
            TypeRef::NonNull(Box::new(base))
        } else {
            base
        };
        self.leave();
        Ok(ty)
    }

    // -- type system definitions ------------------------------------------

    fn parse_type_system_definition(
        &mut self,
        extend: bool,
        description: Option<StrVal>,
    ) -> Result<Def, PErr> {
        if extend && description.is_some() {
            return Err(self.err("a type extension cannot carry a description"));
        }
        let keyword = match self.cur_kind() {
            Some(Tok::Name(n)) => n,
            _ => {
                let found = self.describe_here();
                return Err(self.err(format!(
                    "expected schema, scalar, type, interface, union, enum, input or directive but found {found}"
                )));
            }
        };
        self.pos += 1;
        match keyword.as_str() {
            "schema" => {
                let directives = self.parse_directives(true)?;
                let (ops, trailing) = if self.at_punct("{") {
                    self.pos += 1;
                    let mut ops = Vec::new();
                    while !self.at_punct("}") {
                        if self.cur().is_none() {
                            return Err(self.err("unterminated schema definition (missing \"}\")"));
                        }
                        let comments = self.take_comments();
                        let operation = self.expect_name()?;
                        if !matches!(operation.as_str(), "query" | "mutation" | "subscription") {
                            return Err(self.err(format!(
                                "expected query, mutation or subscription in a schema definition but found \"{operation}\""
                            )));
                        }
                        self.expect_punct(":")?;
                        let ty = self.expect_name()?;
                        ops.push(RootOpDef {
                            comments,
                            operation,
                            ty,
                        });
                    }
                    let trailing = self.take_comments();
                    self.expect_punct("}")?;
                    if ops.is_empty() {
                        return Err(self.err(
                            "a schema definition must declare at least one root operation type",
                        ));
                    }
                    (Some(ops), trailing)
                } else {
                    (None, Vec::new())
                };
                Ok(Def::Schema {
                    extend,
                    description,
                    directives,
                    ops,
                    trailing,
                })
            }
            "scalar" => {
                let name = self.expect_name()?;
                let directives = self.parse_directives(true)?;
                Ok(Def::Scalar {
                    extend,
                    description,
                    name,
                    directives,
                })
            }
            "type" | "interface" => {
                let kw: &'static str = if keyword == "type" { "type" } else { "interface" };
                let name = self.expect_name()?;
                let mut interfaces = Vec::new();
                if self.eat_name("implements") {
                    self.eat_punct("&");
                    interfaces.push(self.expect_name()?);
                    while self.eat_punct("&") {
                        interfaces.push(self.expect_name()?);
                    }
                }
                let directives = self.parse_directives(true)?;
                let (fields, trailing) = if self.at_punct("{") {
                    let (f, t) = self.parse_fields_definition()?;
                    (Some(f), t)
                } else {
                    (None, Vec::new())
                };
                Ok(Def::Object {
                    extend,
                    keyword: kw,
                    description,
                    name,
                    interfaces,
                    directives,
                    fields,
                    trailing,
                })
            }
            "union" => {
                let name = self.expect_name()?;
                let directives = self.parse_directives(true)?;
                let mut members = Vec::new();
                if self.eat_punct("=") {
                    self.eat_punct("|");
                    members.push(self.expect_name()?);
                    while self.eat_punct("|") {
                        members.push(self.expect_name()?);
                    }
                }
                Ok(Def::Union {
                    extend,
                    description,
                    name,
                    directives,
                    members,
                })
            }
            "enum" => {
                let name = self.expect_name()?;
                let directives = self.parse_directives(true)?;
                let (values, trailing) = if self.eat_punct("{") {
                    let mut values = Vec::new();
                    while !self.at_punct("}") {
                        if self.cur().is_none() {
                            return Err(self.err("unterminated enum definition (missing \"}\")"));
                        }
                        let comments = self.take_comments();
                        let description = self.parse_description();
                        let vname = self.expect_name()?;
                        if matches!(vname.as_str(), "true" | "false" | "null") {
                            return Err(self.err(format!(
                                "\"{vname}\" cannot be used as an enum value name"
                            )));
                        }
                        let vdirectives = self.parse_directives(true)?;
                        values.push(EnumValueDef {
                            comments,
                            description,
                            name: vname,
                            directives: vdirectives,
                        });
                    }
                    let trailing = self.take_comments();
                    self.expect_punct("}")?;
                    if values.is_empty() {
                        return Err(self.err("an enum body \"{}\" must contain at least one value"));
                    }
                    (Some(values), trailing)
                } else {
                    (None, Vec::new())
                };
                Ok(Def::Enum {
                    extend,
                    description,
                    name,
                    directives,
                    values,
                    trailing,
                })
            }
            "input" => {
                let name = self.expect_name()?;
                let directives = self.parse_directives(true)?;
                let (fields, trailing) = if self.eat_punct("{") {
                    let mut fields = Vec::new();
                    while !self.at_punct("}") {
                        if self.cur().is_none() {
                            return Err(self.err("unterminated input type (missing \"}\")"));
                        }
                        fields.push(self.parse_input_value_definition()?);
                    }
                    let trailing = self.take_comments();
                    self.expect_punct("}")?;
                    if fields.is_empty() {
                        return Err(
                            self.err("an input type body \"{}\" must contain at least one field")
                        );
                    }
                    (Some(fields), trailing)
                } else {
                    (None, Vec::new())
                };
                Ok(Def::InputObject {
                    extend,
                    description,
                    name,
                    directives,
                    fields,
                    trailing,
                })
            }
            "directive" => {
                if extend {
                    return Err(self.err("directive definitions cannot be extended"));
                }
                self.expect_punct("@")?;
                let name = self.expect_name()?;
                let args = if self.at_punct("(") {
                    self.parse_arguments_definition()?
                } else {
                    Vec::new()
                };
                let repeatable = self.eat_name("repeatable");
                if !self.eat_name("on") {
                    let found = self.describe_here();
                    return Err(self.err(format!(
                        "expected \"on\" in the directive definition but found {found}"
                    )));
                }
                self.eat_punct("|");
                let mut locations = vec![self.expect_directive_location()?];
                while self.eat_punct("|") {
                    locations.push(self.expect_directive_location()?);
                }
                Ok(Def::DirectiveDef {
                    description,
                    name,
                    args,
                    repeatable,
                    locations,
                })
            }
            other => Err(self.err(format!(
                "unexpected name \"{other}\" where a type system definition was expected"
            ))),
        }
    }

    fn expect_directive_location(&mut self) -> Result<String, PErr> {
        let name = self.expect_name()?;
        if !DIRECTIVE_LOCATIONS.contains(&name.as_str()) {
            return Err(self.err(format!("\"{name}\" is not a valid directive location")));
        }
        Ok(name)
    }

    fn parse_description(&mut self) -> Option<StrVal> {
        match self.cur_kind() {
            Some(Tok::Str(s)) => {
                self.pos += 1;
                Some(s)
            }
            _ => None,
        }
    }

    fn parse_fields_definition(&mut self) -> Result<(Vec<FieldDef>, Vec<String>), PErr> {
        self.expect_punct("{")?;
        let mut fields = Vec::new();
        while !self.at_punct("}") {
            if self.cur().is_none() {
                return Err(self.err("unterminated field definitions (missing \"}\")"));
            }
            let comments = self.take_comments();
            let description = self.parse_description();
            let name = self.expect_name()?;
            let args = if self.at_punct("(") {
                self.parse_arguments_definition()?
            } else {
                Vec::new()
            };
            self.expect_punct(":")?;
            let ty = self.parse_type()?;
            let directives = self.parse_directives(true)?;
            fields.push(FieldDef {
                comments,
                description,
                name,
                args,
                ty,
                directives,
            });
        }
        let trailing = self.take_comments();
        self.expect_punct("}")?;
        if fields.is_empty() {
            return Err(self.err("a field definition body \"{}\" must contain at least one field"));
        }
        Ok((fields, trailing))
    }

    fn parse_arguments_definition(&mut self) -> Result<Vec<InputValueDef>, PErr> {
        self.expect_punct("(")?;
        let mut out = Vec::new();
        while !self.at_punct(")") {
            if self.cur().is_none() {
                return Err(self.err("unterminated argument definitions (missing \")\")"));
            }
            out.push(self.parse_input_value_definition()?);
        }
        self.expect_punct(")")?;
        if out.is_empty() {
            return Err(self.err("an argument definition list \"()\" must not be empty"));
        }
        Ok(out)
    }

    fn parse_input_value_definition(&mut self) -> Result<InputValueDef, PErr> {
        let comments = self.take_comments();
        let description = self.parse_description();
        let name = self.expect_name()?;
        self.expect_punct(":")?;
        let ty = self.parse_type()?;
        let default = if self.eat_punct("=") {
            Some(self.parse_value(true)?)
        } else {
            None
        };
        let directives = self.parse_directives(true)?;
        Ok(InputValueDef {
            comments,
            description,
            name,
            ty,
            default,
            directives,
        })
    }
}

// ---------------------------------------------------------------------------
// Sorting
// ---------------------------------------------------------------------------

fn sort_document(doc: &mut Document) {
    for d in &mut doc.defs {
        match &mut d.def {
            Def::Operation { sels, .. } | Def::Fragment { sels, .. } => sort_selection_set(sels),
            Def::Object { fields, .. } => {
                if let Some(f) = fields {
                    f.sort_by(|a, b| a.name.cmp(&b.name));
                }
            }
            Def::InputObject { fields, .. } => {
                if let Some(f) = fields {
                    f.sort_by(|a, b| a.name.cmp(&b.name));
                }
            }
            _ => {}
        }
    }
}

fn sort_selection_set(set: &mut SelectionSet) {
    set.items.sort_by(|a, b| a.sort_key().cmp(&b.sort_key()));
    for item in &mut set.items {
        match item {
            Selection::Field(f) => {
                if let Some(s) = &mut f.sels {
                    sort_selection_set(s);
                }
            }
            Selection::Inline(i) => sort_selection_set(&mut i.sels),
            Selection::Spread(_) => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Printer
// ---------------------------------------------------------------------------

struct Options {
    unit: String,
    minify: bool,
    keep_comments: bool,
}

struct Printer {
    o: Options,
    out: String,
}

/// In minify mode two adjacent tokens must not merge into one longer token.
fn needs_sep(a: char, b: char) -> bool {
    let name_ish = |c: char| c.is_ascii_alphanumeric() || c == '_';
    (name_ish(a) && name_ish(b))
        || (name_ish(a) && b == '-')
        || (a.is_ascii_digit() && b == '.')
        || (a == '.' && b.is_ascii_digit())
}

impl Printer {
    fn new(o: Options) -> Self {
        Printer {
            o,
            out: String::new(),
        }
    }

    fn w(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        if self.o.minify {
            if let (Some(a), Some(b)) = (self.out.chars().last(), s.chars().next()) {
                if needs_sep(a, b) {
                    self.out.push(' ');
                }
            }
        }
        self.out.push_str(s);
    }

    /// A cosmetic space (dropped when minifying).
    fn sp(&mut self) {
        if !self.o.minify {
            self.out.push(' ');
        }
    }

    /// A newline plus `depth` indent units (dropped when minifying).
    fn nl(&mut self, depth: usize) {
        if !self.o.minify {
            self.out.push('\n');
            for _ in 0..depth {
                self.out.push_str(&self.o.unit);
            }
        }
    }

    fn indent_str(&self, depth: usize) -> String {
        self.o.unit.repeat(depth)
    }

    fn comments(&mut self, comments: &[String], depth: usize) {
        if !self.o.keep_comments {
            return;
        }
        for c in comments {
            if c.is_empty() {
                self.w("#");
            } else {
                self.w("# ");
                self.w(c);
            }
            self.nl(depth);
        }
    }

    fn print(mut self, doc: &Document) -> String {
        for (i, d) in doc.defs.iter().enumerate() {
            if i > 0 && !self.o.minify {
                self.out.push_str("\n\n");
            }
            self.comments(&d.comments, 0);
            self.def(&d.def, 0);
        }
        if self.o.keep_comments && !doc.trailing.is_empty() {
            self.out.push_str("\n\n");
            let n = doc.trailing.len();
            for (i, c) in doc.trailing.iter().enumerate() {
                if c.is_empty() {
                    self.w("#");
                } else {
                    self.w("# ");
                    self.w(c);
                }
                if i + 1 < n {
                    self.nl(0);
                }
            }
        }
        self.out
    }

    fn def(&mut self, def: &Def, depth: usize) {
        match def {
            Def::Operation {
                op,
                name,
                vars,
                directives,
                sels,
                shorthand,
            } => {
                if !*shorthand {
                    self.w(op);
                    if let Some(n) = name {
                        self.w(" ");
                        self.w(n);
                    }
                    if !vars.is_empty() {
                        self.w("(");
                        for (i, v) in vars.iter().enumerate() {
                            if i > 0 && !self.o.minify {
                                self.w(", ");
                            }
                            self.w("$");
                            self.w(&v.name);
                            self.w(":");
                            self.sp();
                            self.type_ref(&v.ty);
                            if let Some(d) = &v.default {
                                self.sp();
                                self.w("=");
                                self.sp();
                                self.value(d, depth);
                            }
                            self.directives(&v.directives, depth);
                        }
                        self.w(")");
                    }
                    self.directives(directives, depth);
                    self.sp();
                }
                self.selection_set(sels, depth);
            }
            Def::Fragment {
                name,
                type_cond,
                directives,
                sels,
            } => {
                self.w("fragment ");
                self.w(name);
                self.w(" on ");
                self.w(type_cond);
                self.directives(directives, depth);
                self.sp();
                self.selection_set(sels, depth);
            }
            Def::Schema {
                extend,
                description,
                directives,
                ops,
                trailing,
            } => {
                self.description(description.as_ref(), depth);
                if *extend {
                    self.w("extend ");
                }
                self.w("schema");
                self.directives(directives, depth);
                if let Some(ops) = ops {
                    self.sp();
                    self.w("{");
                    for o in ops {
                        self.nl(depth + 1);
                        self.comments(&o.comments, depth + 1);
                        self.w(&o.operation);
                        self.w(":");
                        self.sp();
                        self.w(&o.ty);
                    }
                    self.block_trailing(trailing, depth);
                    self.nl(depth);
                    self.w("}");
                }
            }
            Def::Scalar {
                extend,
                description,
                name,
                directives,
            } => {
                self.description(description.as_ref(), depth);
                if *extend {
                    self.w("extend ");
                }
                self.w("scalar ");
                self.w(name);
                self.directives(directives, depth);
            }
            Def::Object {
                extend,
                keyword,
                description,
                name,
                interfaces,
                directives,
                fields,
                trailing,
            } => {
                self.description(description.as_ref(), depth);
                if *extend {
                    self.w("extend ");
                }
                self.w(keyword);
                self.w(" ");
                self.w(name);
                if !interfaces.is_empty() {
                    self.w(" implements ");
                    for (i, iface) in interfaces.iter().enumerate() {
                        if i > 0 {
                            self.sp();
                            self.w("&");
                            self.sp();
                        }
                        self.w(iface);
                    }
                }
                self.directives(directives, depth);
                if let Some(fields) = fields {
                    self.sp();
                    self.w("{");
                    for f in fields {
                        self.nl(depth + 1);
                        self.comments(&f.comments, depth + 1);
                        self.description(f.description.as_ref(), depth + 1);
                        self.w(&f.name);
                        self.arguments_definition(&f.args, depth + 1);
                        self.w(":");
                        self.sp();
                        self.type_ref(&f.ty);
                        self.directives(&f.directives, depth + 1);
                    }
                    self.block_trailing(trailing, depth);
                    self.nl(depth);
                    self.w("}");
                }
            }
            Def::Union {
                extend,
                description,
                name,
                directives,
                members,
            } => {
                self.description(description.as_ref(), depth);
                if *extend {
                    self.w("extend ");
                }
                self.w("union ");
                self.w(name);
                self.directives(directives, depth);
                if !members.is_empty() {
                    self.sp();
                    self.w("=");
                    self.sp();
                    for (i, m) in members.iter().enumerate() {
                        if i > 0 {
                            self.sp();
                            self.w("|");
                            self.sp();
                        }
                        self.w(m);
                    }
                }
            }
            Def::Enum {
                extend,
                description,
                name,
                directives,
                values,
                trailing,
            } => {
                self.description(description.as_ref(), depth);
                if *extend {
                    self.w("extend ");
                }
                self.w("enum ");
                self.w(name);
                self.directives(directives, depth);
                if let Some(values) = values {
                    self.sp();
                    self.w("{");
                    for v in values {
                        self.nl(depth + 1);
                        self.comments(&v.comments, depth + 1);
                        self.description(v.description.as_ref(), depth + 1);
                        self.w(&v.name);
                        self.directives(&v.directives, depth + 1);
                    }
                    self.block_trailing(trailing, depth);
                    self.nl(depth);
                    self.w("}");
                }
            }
            Def::InputObject {
                extend,
                description,
                name,
                directives,
                fields,
                trailing,
            } => {
                self.description(description.as_ref(), depth);
                if *extend {
                    self.w("extend ");
                }
                self.w("input ");
                self.w(name);
                self.directives(directives, depth);
                if let Some(fields) = fields {
                    self.sp();
                    self.w("{");
                    for f in fields {
                        self.nl(depth + 1);
                        self.comments(&f.comments, depth + 1);
                        self.description(f.description.as_ref(), depth + 1);
                        self.input_value(f, depth + 1);
                    }
                    self.block_trailing(trailing, depth);
                    self.nl(depth);
                    self.w("}");
                }
            }
            Def::DirectiveDef {
                description,
                name,
                args,
                repeatable,
                locations,
            } => {
                self.description(description.as_ref(), depth);
                self.w("directive ");
                self.w("@");
                self.w(name);
                self.arguments_definition(args, depth);
                if *repeatable {
                    self.w(" repeatable");
                }
                self.w(" on ");
                for (i, l) in locations.iter().enumerate() {
                    if i > 0 {
                        self.sp();
                        self.w("|");
                        self.sp();
                    }
                    self.w(l);
                }
            }
        }
    }

    fn block_trailing(&mut self, trailing: &[String], depth: usize) {
        if !self.o.keep_comments {
            return;
        }
        for c in trailing {
            self.nl(depth + 1);
            if c.is_empty() {
                self.w("#");
            } else {
                self.w("# ");
                self.w(c);
            }
        }
    }

    fn description(&mut self, d: Option<&StrVal>, depth: usize) {
        if let Some(s) = d {
            self.string_value(s, depth);
            self.nl(depth);
            if self.o.minify {
                // A block string always ends with `"""`, so no separator is needed.
            }
        }
    }

    fn input_value(&mut self, f: &InputValueDef, depth: usize) {
        self.w(&f.name);
        self.w(":");
        self.sp();
        self.type_ref(&f.ty);
        if let Some(d) = &f.default {
            self.sp();
            self.w("=");
            self.sp();
            self.value(d, depth);
        }
        self.directives(&f.directives, depth);
    }

    fn arguments_definition(&mut self, args: &[InputValueDef], depth: usize) {
        if args.is_empty() {
            return;
        }
        let multiline = !self.o.minify
            && args
                .iter()
                .any(|a| a.description.is_some() || (self.o.keep_comments && !a.comments.is_empty()));
        self.w("(");
        for (i, a) in args.iter().enumerate() {
            if multiline {
                self.nl(depth + 1);
                self.comments(&a.comments, depth + 1);
                self.description(a.description.as_ref(), depth + 1);
                self.input_value(a, depth + 1);
            } else {
                if i > 0 && !self.o.minify {
                    self.w(", ");
                }
                self.input_value(a, depth);
            }
        }
        if multiline {
            self.nl(depth);
        }
        self.w(")");
    }

    fn directives(&mut self, dirs: &[Directive], depth: usize) {
        for d in dirs {
            self.sp();
            self.w("@");
            self.w(&d.name);
            self.arguments(&d.args, depth);
        }
    }

    fn arguments(&mut self, args: &[Argument], depth: usize) {
        if args.is_empty() {
            return;
        }
        self.w("(");
        for (i, a) in args.iter().enumerate() {
            if i > 0 && !self.o.minify {
                self.w(", ");
            }
            self.w(&a.name);
            self.w(":");
            self.sp();
            self.value(&a.value, depth);
        }
        self.w(")");
    }

    fn selection_set(&mut self, set: &SelectionSet, depth: usize) {
        self.w("{");
        for item in &set.items {
            self.nl(depth + 1);
            match item {
                Selection::Field(f) => {
                    self.comments(&f.comments, depth + 1);
                    if let Some(a) = &f.alias {
                        self.w(a);
                        self.w(":");
                        self.sp();
                    }
                    self.w(&f.name);
                    self.arguments(&f.args, depth + 1);
                    self.directives(&f.directives, depth + 1);
                    if let Some(s) = &f.sels {
                        self.sp();
                        self.selection_set(s, depth + 1);
                    }
                }
                Selection::Spread(s) => {
                    self.comments(&s.comments, depth + 1);
                    self.w("...");
                    self.w(&s.name);
                    self.directives(&s.directives, depth + 1);
                }
                Selection::Inline(i) => {
                    self.comments(&i.comments, depth + 1);
                    self.w("...");
                    if let Some(t) = &i.type_cond {
                        self.w(" on ");
                        self.w(t);
                    }
                    self.directives(&i.directives, depth + 1);
                    self.sp();
                    self.selection_set(&i.sels, depth + 1);
                }
            }
        }
        self.block_trailing(&set.trailing, depth);
        self.nl(depth);
        self.w("}");
    }

    fn type_ref(&mut self, t: &TypeRef) {
        match t {
            TypeRef::Named(n) => self.w(n),
            TypeRef::List(inner) => {
                self.w("[");
                self.type_ref(inner);
                self.w("]");
            }
            TypeRef::NonNull(inner) => {
                self.type_ref(inner);
                self.w("!");
            }
        }
    }

    fn value(&mut self, v: &Value, depth: usize) {
        match v {
            Value::Var(n) => {
                self.w("$");
                self.w(n);
            }
            Value::Int(n) | Value::Float(n) | Value::Enum(n) => self.w(n),
            Value::Bool(b) => self.w(if *b { "true" } else { "false" }),
            Value::Null => self.w("null"),
            Value::Str(s) => self.string_value(s, depth),
            Value::List(items) => {
                self.w("[");
                for (i, it) in items.iter().enumerate() {
                    if i > 0 && !self.o.minify {
                        self.w(", ");
                    }
                    self.value(it, depth);
                }
                self.w("]");
            }
            Value::Object(fields) => {
                self.w("{");
                for (i, f) in fields.iter().enumerate() {
                    if i > 0 && !self.o.minify {
                        self.w(", ");
                    }
                    self.w(&f.name);
                    self.w(":");
                    self.sp();
                    self.value(&f.value, depth);
                }
                self.w("}");
            }
        }
    }

    fn string_value(&mut self, s: &StrVal, depth: usize) {
        if s.block {
            let escaped = s.value.replace("\"\"\"", "\\\"\"\"");
            let single_line = !escaped.contains('\n');
            let safe_single = single_line
                && !escaped.starts_with(' ')
                && !escaped.starts_with('\t')
                && !escaped.ends_with('"')
                && !escaped.ends_with('\\')
                && escaped.chars().count() <= 70;
            if safe_single || self.o.minify {
                let text = if self.o.minify {
                    escaped
                } else {
                    escaped.clone()
                };
                self.w("\"\"\"");
                self.out.push_str(&text);
                self.out.push_str("\"\"\"");
            } else {
                let pad = self.indent_str(depth);
                self.w("\"\"\"");
                for line in escaped.split('\n') {
                    self.out.push('\n');
                    if !line.is_empty() {
                        self.out.push_str(&pad);
                        self.out.push_str(line);
                    }
                }
                self.out.push('\n');
                self.out.push_str(&pad);
                self.out.push_str("\"\"\"");
            }
        } else {
            let mut q = String::from("\"");
            for c in s.value.chars() {
                match c {
                    '"' => q.push_str("\\\""),
                    '\\' => q.push_str("\\\\"),
                    '\n' => q.push_str("\\n"),
                    '\r' => q.push_str("\\r"),
                    '\t' => q.push_str("\\t"),
                    '\u{8}' => q.push_str("\\b"),
                    '\u{c}' => q.push_str("\\f"),
                    c if (c as u32) < 0x20 => q.push_str(&format!("\\u{:04X}", c as u32)),
                    c => q.push(c),
                }
            }
            q.push('"');
            self.w(&q);
        }
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

fn parse_indent(indent: &str) -> Result<String, String> {
    match indent.trim().to_ascii_lowercase().as_str() {
        "" | "2" => Ok("  ".to_string()),
        "4" => Ok("    ".to_string()),
        "8" => Ok(" ".repeat(8)),
        "tab" | "tabs" | "\t" => Ok("\t".to_string()),
        other => Err(format!(
            "indent must be one of 2, 4, 8 or tab (got \"{other}\")"
        )),
    }
}

/// Format (or minify) a GraphQL document.
///
/// * `source` — the GraphQL query, mutation, subscription, fragment or SDL text.
/// * `indent` — `2` | `4` | `8` | `tab`.
/// * `mode` — `format` | `minify`.
/// * `sort_fields` — sort selections and SDL field definitions alphabetically.
/// * `remove_comments` — drop `#` comments (always dropped when minifying).
///
/// Returns the formatted document, or a `Syntax error at line L, column C: …`
/// message when the input is not valid GraphQL.
pub fn run(
    source: &str,
    indent: &str,
    mode: &str,
    sort_fields: bool,
    remove_comments: bool,
) -> Result<String, String> {
    if source.trim().is_empty() {
        return Err("input is empty — paste a GraphQL query, mutation, or schema".to_string());
    }
    if source.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes (about 500 KB)",
            source.len(),
            MAX_INPUT_BYTES
        ));
    }
    let unit = parse_indent(indent)?;
    let minify = match mode.trim().to_ascii_lowercase().as_str() {
        "" | "format" => false,
        "minify" => true,
        other => {
            return Err(format!(
                "mode must be either format or minify (got \"{other}\")"
            ))
        }
    };
    let toks = Lexer::new(source).tokenize().map_err(|e| e.render())?;
    let mut doc = Parser::new(toks).parse_document().map_err(|e| e.render())?;
    if sort_fields {
        sort_document(&mut doc);
    }
    let printer = Printer::new(Options {
        unit,
        minify,
        keep_comments: !remove_comments && !minify,
    });
    Ok(printer.print(&doc))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn fmt(src: &str) -> String {
        run(src, "2", "format", false, false).unwrap()
    }

    #[test]
    fn formats_a_messy_query() {
        let out = fmt("{hero{name friends{name}}}");
        assert_eq!(
            out,
            "{\n  hero {\n    name\n    friends {\n      name\n    }\n  }\n}"
        );
    }

    #[test]
    fn rejects_unbalanced_braces_with_a_position() {
        let err = run("query Q { hero { name }", "2", "format", false, false).unwrap_err();
        assert!(
            err.starts_with("Syntax error at line 1, column 23:"),
            "unexpected error: {err}"
        );
        assert!(err.contains("unterminated selection set"), "{err}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n ", "2", "format", false, false).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn named_operation_with_variables_and_directives() {
        let out = fmt(
            "query Hero($ep:Episode=JEDI,$with:Boolean!)@cached(ttl:30){hero(episode:$ep){name friends @include(if:$with){name}}}",
        );
        assert_eq!(
            out,
            "query Hero($ep: Episode = JEDI, $with: Boolean!) @cached(ttl: 30) {\n  hero(episode: $ep) {\n    name\n    friends @include(if: $with) {\n      name\n    }\n  }\n}"
        );
    }

    #[test]
    fn aliases_fragments_and_inline_fragments() {
        let out = fmt(
            "{ empire: hero(episode: EMPIRE) { ...heroFields } ... on Droid { primaryFunction } } fragment heroFields on Character { name }",
        );
        assert_eq!(
            out,
            "{\n  empire: hero(episode: EMPIRE) {\n    ...heroFields\n  }\n  ... on Droid {\n    primaryFunction\n  }\n}\n\nfragment heroFields on Character {\n  name\n}"
        );
    }

    #[test]
    fn four_space_and_tab_indent() {
        assert_eq!(
            run("{a{b}}", "4", "format", false, false).unwrap(),
            "{\n    a {\n        b\n    }\n}"
        );
        assert_eq!(
            run("{a{b}}", "tab", "format", false, false).unwrap(),
            "{\n\ta {\n\t\tb\n\t}\n}"
        );
        assert_eq!(
            run("{a{b}}", "8", "format", false, false).unwrap(),
            "{\n        a {\n                b\n        }\n}"
        );
    }

    #[test]
    fn rejects_an_unknown_indent() {
        let err = run("{a}", "3", "format", false, false).unwrap_err();
        assert!(err.contains("indent must be one of 2, 4, 8 or tab"), "{err}");
    }

    #[test]
    fn rejects_an_unknown_mode() {
        let err = run("{a}", "2", "prettify", false, false).unwrap_err();
        assert!(err.contains("mode must be either format or minify"), "{err}");
    }

    #[test]
    fn minify_strips_ignored_characters() {
        let out = run(
            "query Hero($ep: Episode = JEDI) {\n  hero(episode: $ep) {\n    name\n    height\n  }\n}",
            "2",
            "minify",
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "query Hero($ep:Episode=JEDI){hero(episode:$ep){name height}}");
    }

    #[test]
    fn minify_keeps_adjacent_names_separated() {
        let out = run("scalar A\nscalar B\nenum E { X Y }", "2", "minify", false, false).unwrap();
        assert_eq!(out, "scalar A scalar B enum E{X Y}");
    }

    #[test]
    fn minify_drops_comments() {
        let out = run("# top\n{ a # trailing\n }", "2", "minify", false, false).unwrap();
        assert_eq!(out, "{a}");
    }

    #[test]
    fn comments_are_kept_and_reattached_to_the_next_item() {
        let out = fmt("# document note\nquery Q {\n  # about a\n  a\n  b\n  # dangling\n}");
        assert_eq!(
            out,
            "# document note\nquery Q {\n  # about a\n  a\n  b\n  # dangling\n}"
        );
    }

    #[test]
    fn remove_comments_drops_them() {
        let out = run("# note\n{ a # inline\n b }", "2", "format", false, true).unwrap();
        assert_eq!(out, "{\n  a\n  b\n}");
    }

    #[test]
    fn sort_fields_sorts_selections_recursively() {
        let out = run("{ z y x { c b a } }", "2", "format", true, false).unwrap();
        assert_eq!(out, "{\n  x {\n    a\n    b\n    c\n  }\n  y\n  z\n}");
    }

    #[test]
    fn sort_fields_uses_the_alias_and_sorts_sdl_fields() {
        let out = run(
            "{ zed: alpha beta }\ntype T { zeta: Int alpha: String }\ninput I { z: Int a: Int }",
            "2",
            "format",
            true,
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\n  beta\n  zed: alpha\n}\n\ntype T {\n  alpha: String\n  zeta: Int\n}\n\ninput I {\n  a: Int\n  z: Int\n}"
        );
    }

    #[test]
    fn formats_sdl_schema() {
        let out = fmt(
            "schema{query:Query}type Query implements Node&Timestamped @key(fields:\"id\"){user(id:ID!,limit:Int=10):User!}enum Role{ADMIN USER}union Result=User|Error input Filter{q:String tags:[String!]=[\"a\",\"b\"]}scalar DateTime directive @auth(role:Role!)repeatable on FIELD_DEFINITION|OBJECT",
        );
        let expected = concat!(
            "schema {\n  query: Query\n}\n\n",
            "type Query implements Node & Timestamped @key(fields: \"id\") {\n  user(id: ID!, limit: Int = 10): User!\n}\n\n",
            "enum Role {\n  ADMIN\n  USER\n}\n\n",
            "union Result = User | Error\n\n",
            "input Filter {\n  q: String\n  tags: [String!] = [\"a\", \"b\"]\n}\n\n",
            "scalar DateTime\n\n",
            "directive @auth(role: Role!) repeatable on FIELD_DEFINITION | OBJECT"
        );
        assert_eq!(out, expected);
    }

    #[test]
    fn formats_descriptions_and_extensions() {
        let out = fmt(
            "\"\"\"A user.\"\"\" type User { \"\"\"The id.\"\"\" id: ID! } extend type User { email: String }",
        );
        assert_eq!(
            out,
            "\"\"\"A user.\"\"\"\ntype User {\n  \"\"\"The id.\"\"\"\n  id: ID!\n}\n\nextend type User {\n  email: String\n}"
        );
    }

    #[test]
    fn multiline_block_descriptions_round_trip() {
        let src = "\"\"\"\nLine one.\n\nLine two.\n\"\"\"\ntype T {\n  a: Int\n}";
        let once = fmt(src);
        assert_eq!(once, src);
        assert_eq!(fmt(&once), once, "formatting must be idempotent");
    }

    #[test]
    fn object_and_list_values_print_inline() {
        let out = fmt("mutation{add(input:{name:\"a\",tags:[1,2],nested:{x:true,y:null}}){id}}");
        assert_eq!(
            out,
            "mutation {\n  add(input: {name: \"a\", tags: [1, 2], nested: {x: true, y: null}}) {\n    id\n  }\n}"
        );
    }

    #[test]
    fn documented_arguments_break_onto_their_own_lines() {
        let out = fmt("type Q{f(\"\"\"how many\"\"\" first:Int=5 after:String):Int}");
        assert_eq!(
            out,
            "type Q {\n  f(\n    \"\"\"how many\"\"\"\n    first: Int = 5\n    after: String\n  ): Int\n}"
        );
    }

    #[test]
    fn formatting_is_idempotent_for_a_mixed_document() {
        let src = "query Q($a: Int = 1) @dir { alias: field(x: [1, 2], y: {k: \"v\"}) { ...F } }\n\nfragment F on T {\n  id\n}";
        let once = fmt(src);
        assert_eq!(fmt(&once), once);
    }

    #[test]
    fn rejects_an_empty_selection_set() {
        let err = run("query Q { }", "2", "format", false, false).unwrap_err();
        assert!(err.contains("at least one field"), "{err}");
    }

    #[test]
    fn rejects_a_bad_number_literal() {
        let err = run("{ a(x: 01) }", "2", "format", false, false).unwrap_err();
        assert!(err.contains("no digits may follow a leading 0"), "{err}");
    }

    #[test]
    fn rejects_an_unterminated_string() {
        let err = run("{ a(x: \"oops) }", "2", "format", false, false).unwrap_err();
        assert!(err.contains("unterminated string"), "{err}");
    }

    #[test]
    fn rejects_a_stray_character_with_a_line_number() {
        let err = run("query Q {\n  a\n  %\n}", "2", "format", false, false).unwrap_err();
        assert_eq!(
            err,
            "Syntax error at line 3, column 3: unexpected character '%' in GraphQL source"
        );
    }

    #[test]
    fn rejects_an_invalid_directive_location() {
        let err = run("directive @x on NOWHERE", "2", "format", false, false).unwrap_err();
        assert!(err.contains("is not a valid directive location"), "{err}");
    }

    #[test]
    fn rejects_documents_nested_past_the_depth_limit() {
        let mut src = String::new();
        for _ in 0..(MAX_DEPTH + 2) {
            src.push_str("{a");
        }
        for _ in 0..(MAX_DEPTH + 2) {
            src.push('}');
        }
        let err = run(&src, "2", "format", false, false).unwrap_err();
        assert!(err.contains("nests deeper than the supported limit"), "{err}");
    }

    #[test]
    fn rejects_input_over_the_size_cap() {
        let big = format!("{{ a(x: \"{}\") }}", "y".repeat(MAX_INPUT_BYTES));
        let err = run(&big, "2", "format", false, false).unwrap_err();
        assert!(err.contains("the limit is 500000 bytes"), "{err}");
    }

    #[test]
    fn escapes_and_unicode_survive_a_round_trip() {
        let out = fmt("{ a(s: \"tab\\there \\u0041 \\\"q\\\"\") }");
        assert_eq!(out, "{\n  a(s: \"tab\\there A \\\"q\\\"\")\n}");
    }

    #[test]
    fn commas_are_ignored_tokens() {
        assert_eq!(fmt("{a,b,c}"), "{\n  a\n  b\n  c\n}");
    }

    #[test]
    fn accepts_a_schema_extension_without_a_body() {
        assert_eq!(fmt("extend scalar Date @tz"), "extend scalar Date @tz");
    }

    #[test]
    fn minified_output_reparses_to_the_same_formatted_document() {
        let src = "query Q($a:Int=1){f(b:{c:[1,2.5,-3]}){...on T{x}}}\ntype A implements B&C{d(e:Int=0):[String!]!}\nenum E{A B}\n\"\"\"desc\"\"\"\nscalar S";
        let formatted = fmt(src);
        let minified = run(src, "2", "minify", false, false).unwrap();
        assert_eq!(fmt(&minified), formatted);
    }
}
