//! json-to-types core — pure compute, shared by the chat skill block and the web page.
//!
//! Infers type definitions from a JSON **sample** (real data, not a schema) and emits them as
//! TypeScript interfaces, Rust structs, Go structs or Python dataclasses.
//!
//! The inference is structural and order-preserving:
//!
//! * every JSON value becomes a [`Node`] — a shape (primitive set / array / object / unknown)
//!   plus a `nullable` flag, so `null` never destroys the type it appears next to;
//! * array elements are **merged**, so `[{"a":1},{"a":2,"b":"x"}]` yields ONE type where `b` is
//!   marked missing-in-some-elements rather than two unrelated types;
//! * merging is recursive and commutative: primitive sets union, arrays merge element-wise,
//!   objects union their keys (first-seen order wins), and genuinely conflicting shapes
//!   (object vs string) collapse to the language's escape-hatch type;
//! * structurally identical objects are emitted ONCE and reused, so a repeated address block
//!   doesn't produce `Address`, `Address2`, `Address3`.
//!
//! Everything is deterministic: the same JSON always produces byte-identical output. There is no
//! I/O, no clock and no randomness.

use std::collections::{BTreeSet, HashMap, HashSet};

use serde_json::Value;

/// Biggest JSON sample accepted. Inference is linear, but the emitted code has to stay
/// something a human can read and paste.
pub const MAX_INPUT_BYTES: usize = 2_000_000;
/// Deepest nesting walked before giving up (each level can add a generated type).
pub const MAX_DEPTH: usize = 64;
/// Most named types emitted from one sample.
pub const MAX_TYPES: usize = 300;

// ---------------------------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------------------------

/// Target language for the emitted declarations.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    TypeScript,
    Rust,
    Go,
    Python,
}

impl Language {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "typescript" | "ts" | "tsx" => Ok(Language::TypeScript),
            "rust" | "rs" => Ok(Language::Rust),
            "go" | "golang" => Ok(Language::Go),
            "python" | "py" | "dataclass" | "dataclasses" => Ok(Language::Python),
            other => Err(format!(
                "unknown output_language '{other}': expected typescript, rust, go or python"
            )),
        }
    }
}

/// How a field that is `null`, or missing from some array elements, is rendered.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OptionalStrategy {
    /// Missing-in-some-samples AND null-valued fields both become optional
    /// (`name?: T` / `Option<T>` / `*T` / `Optional[T] = None`).
    Optional,
    /// Only missing-in-some-samples fields become optional; `null` widens the TYPE instead
    /// (`T | null` / `Option<T>` / `*T` / `Optional[T]`), keeping the field required.
    Nullable,
    /// Every key is required and `null` is ignored — the narrowest possible types.
    Required,
}

impl OptionalStrategy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "optional" | "" => Ok(OptionalStrategy::Optional),
            "nullable" | "null" => Ok(OptionalStrategy::Nullable),
            "required" | "strict" => Ok(OptionalStrategy::Required),
            other => Err(format!(
                "unknown optional_strategy '{other}': expected optional, nullable or required"
            )),
        }
    }
}

/// Everything the generator needs besides the JSON itself.
#[derive(Clone, Debug)]
pub struct Options {
    pub language: Language,
    /// Name of the top-level type. Non-identifier characters are dropped; empty falls back to `Root`.
    pub root_name: String,
    pub optional_strategy: OptionalStrategy,
    /// Emit serde derives + `#[serde(rename)]` (Rust) and `json:"…"` struct tags (Go).
    /// No effect on TypeScript or Python.
    pub json_annotations: bool,
    /// TypeScript `export`, Rust `pub`, Go exported (capitalised) type names. No effect on Python.
    pub export: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            language: Language::TypeScript,
            root_name: "Root".to_string(),
            optional_strategy: OptionalStrategy::Optional,
            json_annotations: true,
            export: true,
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Inferred shape
// ---------------------------------------------------------------------------------------------

/// A non-null primitive kind. Ordering is the canonical union order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
enum Prim {
    Bool,
    Int,
    Float,
    Str,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
enum Shape {
    /// Nothing observed yet (an empty array's element, or a value that was only ever `null`).
    Unknown,
    Prims(BTreeSet<Prim>),
    Arr(Box<Node>),
    /// Object fields in first-seen order. Empty = an object with no observed keys (a map).
    Obj(Vec<(String, Field)>),
    /// Two irreconcilable shapes were merged (e.g. object and string).
    Mixed,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Node {
    shape: Shape,
    /// `null` was observed in this position.
    nullable: bool,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
struct Field {
    node: Node,
    /// The key was absent from at least one observed object of this shape.
    missing: bool,
}

impl Node {
    fn unknown() -> Node {
        Node { shape: Shape::Unknown, nullable: false }
    }
    fn prim(p: Prim) -> Node {
        let mut set = BTreeSet::new();
        set.insert(p);
        Node { shape: Shape::Prims(set), nullable: false }
    }
}

fn from_value(v: &Value, depth: usize) -> Result<Node, String> {
    if depth > MAX_DEPTH {
        return Err(format!(
            "JSON is nested deeper than {MAX_DEPTH} levels — flatten the sample or trim a branch"
        ));
    }
    Ok(match v {
        Value::Null => Node { shape: Shape::Unknown, nullable: true },
        Value::Bool(_) => Node::prim(Prim::Bool),
        Value::Number(n) => {
            if n.is_i64() || n.is_u64() {
                Node::prim(Prim::Int)
            } else {
                Node::prim(Prim::Float)
            }
        }
        Value::String(_) => Node::prim(Prim::Str),
        Value::Array(items) => {
            let mut inner = Node::unknown();
            for it in items {
                inner = merge(inner, from_value(it, depth + 1)?);
            }
            Node { shape: Shape::Arr(Box::new(inner)), nullable: false }
        }
        Value::Object(map) => {
            let mut fields = Vec::with_capacity(map.len());
            for (k, val) in map {
                fields.push((k.clone(), Field { node: from_value(val, depth + 1)?, missing: false }));
            }
            Node { shape: Shape::Obj(fields), nullable: false }
        }
    })
}

/// Unify two observations of the same position.
fn merge(a: Node, b: Node) -> Node {
    let nullable = a.nullable || b.nullable;
    let shape = match (a.shape, b.shape) {
        (Shape::Unknown, s) | (s, Shape::Unknown) => s,
        (Shape::Prims(mut s1), Shape::Prims(s2)) => {
            s1.extend(s2);
            Shape::Prims(s1)
        }
        (Shape::Arr(x), Shape::Arr(y)) => Shape::Arr(Box::new(merge(*x, *y))),
        (Shape::Obj(f1), Shape::Obj(f2)) => Shape::Obj(merge_fields(f1, f2)),
        // object vs string, array vs number, … — no single type describes both.
        _ => Shape::Mixed,
    };
    Node { shape, nullable }
}

fn merge_fields(
    f1: Vec<(String, Field)>,
    f2: Vec<(String, Field)>,
) -> Vec<(String, Field)> {
    let keys2: HashMap<&str, &Field> = f2.iter().map(|(k, f)| (k.as_str(), f)).collect();
    let keys1: HashSet<&str> = f1.iter().map(|(k, _)| k.as_str()).collect();
    let mut out: Vec<(String, Field)> = Vec::with_capacity(f1.len() + f2.len());
    // First-seen order wins, so walk the left side first.
    for (k, a) in &f1 {
        match keys2.get(k.as_str()) {
            Some(b) => out.push((
                k.clone(),
                Field {
                    node: merge(a.node.clone(), b.node.clone()),
                    missing: a.missing || b.missing,
                },
            )),
            // Present on the left, absent on the right → optional.
            None => out.push((k.clone(), Field { node: a.node.clone(), missing: true })),
        }
    }
    for (k, b) in &f2 {
        if !keys1.contains(k.as_str()) {
            out.push((k.clone(), Field { node: b.node.clone(), missing: true }));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Naming
// ---------------------------------------------------------------------------------------------

/// Go initialisms that stay fully capitalised in identifiers (the `golint` list, trimmed).
const INITIALISMS: &[&str] = &[
    "acl", "api", "ascii", "cpu", "css", "dns", "eof", "guid", "html", "http", "https", "id", "ip",
    "json", "lhs", "qps", "ram", "rhs", "rpc", "sla", "smtp", "sql", "ssh", "tcp", "tls", "ttl",
    "udp", "ui", "uid", "uuid", "uri", "url", "utf8", "vm", "xml", "xmpp", "xsrf", "xss",
];

/// Split an arbitrary JSON key into lower-case words: `userID` → `["user","id"]`,
/// `first-name` → `["first","name"]`, `HTTPServer` → `["http","server"]`.
fn words(s: &str) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut prev_lower_or_digit = false;
    for (i, &c) in chars.iter().enumerate() {
        if c.is_ascii_alphanumeric() {
            if c.is_ascii_uppercase() {
                let next_lower = chars.get(i + 1).is_some_and(|n| n.is_ascii_lowercase());
                if !cur.is_empty() && (prev_lower_or_digit || next_lower) {
                    out.push(std::mem::take(&mut cur));
                }
                cur.push(c.to_ascii_lowercase());
                prev_lower_or_digit = false;
            } else {
                cur.push(c);
                prev_lower_or_digit = true;
            }
        } else {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            prev_lower_or_digit = false;
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn snake(s: &str) -> String {
    let joined = words(s).join("_");
    if joined.is_empty() {
        "field".to_string()
    } else if joined.starts_with(|c: char| c.is_ascii_digit()) {
        format!("f_{joined}")
    } else {
        joined
    }
}

fn capitalize(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

/// PascalCase without initialism handling — used for TypeScript/Rust/Python type names.
fn pascal(s: &str) -> String {
    let out: String = words(s).iter().map(|w| capitalize(w)).collect();
    if out.is_empty() {
        "T".to_string()
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("T{out}")
    } else {
        out
    }
}

/// PascalCase with Go initialisms kept upper-case: `user_id` → `UserID`.
fn go_pascal(s: &str) -> String {
    let out: String = words(s)
        .iter()
        .map(|w| {
            if INITIALISMS.contains(&w.as_str()) {
                w.to_ascii_uppercase()
            } else {
                capitalize(w)
            }
        })
        .collect();
    if out.is_empty() {
        "Field".to_string()
    } else if out.starts_with(|c: char| c.is_ascii_digit()) {
        format!("F{out}")
    } else {
        out
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_ascii_lowercase().to_string() + c.as_str(),
        None => String::new(),
    }
}

/// `items` → `Item`, `entries` → `Entry`, `data` → `DataItem`.
fn singularize(hint: &str) -> String {
    if let Some(stripped) = hint.strip_suffix("ies") {
        if !stripped.is_empty() {
            return format!("{stripped}y");
        }
    }
    for suffix in ["sses", "shes", "ches", "xes", "zes"] {
        if let Some(stripped) = hint.strip_suffix("es") {
            if hint.ends_with(suffix) && !stripped.is_empty() {
                return stripped.to_string();
            }
        }
    }
    if hint.len() > 1 && hint.ends_with('s') && !hint.ends_with("ss") && !hint.ends_with("us") {
        return hint[..hint.len() - 1].to_string();
    }
    format!("{hint}Item")
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use",
    "where", "while", "abstract", "become", "box", "do", "final", "macro", "override", "priv",
    "try", "typeof", "unsized", "virtual", "yield",
];

const PYTHON_KEYWORDS: &[&str] = &[
    "False", "None", "True", "and", "as", "assert", "async", "await", "break", "class", "continue",
    "def", "del", "elif", "else", "except", "finally", "for", "from", "global", "if", "import",
    "in", "is", "lambda", "nonlocal", "not", "or", "pass", "raise", "return", "try", "while",
    "with", "yield", "match", "case",
];

const GO_KEYWORDS: &[&str] = &[
    "break", "case", "chan", "const", "continue", "default", "defer", "else", "fallthrough", "for",
    "func", "go", "goto", "if", "import", "interface", "map", "package", "range", "return",
    "select", "struct", "switch", "type", "var", "any",
];

fn is_ts_ident(s: &str) -> bool {
    !s.is_empty()
        && s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '$')
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
}

fn escape_dq(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

// ---------------------------------------------------------------------------------------------
// Emission
// ---------------------------------------------------------------------------------------------

/// One rendered struct/interface/class member.
struct Member {
    /// Language-legal member name.
    name: String,
    /// Original JSON key.
    key: String,
    ty: String,
    optional: bool,
}

/// Which `typing` names the Python output ended up using.
#[derive(Default)]
struct PyImports {
    any: bool,
    dict: bool,
    list: bool,
    optional: bool,
    union: bool,
}

struct Gen<'a> {
    opts: &'a Options,
    /// Rendered declarations in dependency order (children before parents).
    decls: Vec<String>,
    used_names: HashSet<String>,
    /// Structurally identical objects share one declared type.
    shape_names: HashMap<Node, String>,
    py: PyImports,
    emitted_class: bool,
    err: Option<String>,
}

impl<'a> Gen<'a> {
    fn new(opts: &'a Options) -> Self {
        Gen {
            opts,
            decls: Vec::new(),
            used_names: HashSet::new(),
            shape_names: HashMap::new(),
            py: PyImports::default(),
            emitted_class: false,
            err: None,
        }
    }

    fn strategy(&self) -> OptionalStrategy {
        self.opts.optional_strategy
    }

    fn unique_type_name(&mut self, hint: &str) -> String {
        let base = match self.opts.language {
            Language::Go => go_pascal(hint),
            _ => pascal(hint),
        };
        let base = if base.is_empty() { "T".to_string() } else { base };
        let mut candidate = base.clone();
        let mut i = 2;
        while !self.used_names.insert(candidate.clone()) {
            candidate = format!("{base}{i}");
            i += 1;
        }
        candidate
    }

    /// The language's "no idea / anything goes" type.
    fn any_type(&mut self) -> String {
        match self.opts.language {
            Language::TypeScript => "unknown".to_string(),
            Language::Rust => "serde_json::Value".to_string(),
            Language::Go => "any".to_string(),
            Language::Python => {
                self.py.any = true;
                "Any".to_string()
            }
        }
    }

    /// The language's "object with unknown keys" type.
    fn map_type(&mut self) -> String {
        match self.opts.language {
            Language::TypeScript => "Record<string, unknown>".to_string(),
            Language::Rust => "serde_json::Map<String, serde_json::Value>".to_string(),
            Language::Go => "map[string]any".to_string(),
            Language::Python => {
                self.py.dict = true;
                self.py.any = true;
                "Dict[str, Any]".to_string()
            }
        }
    }

    fn prim_union(&mut self, set: &BTreeSet<Prim>) -> String {
        // A sample holding both 3 and 3.5 is a float column, not a union.
        let float = set.contains(&Prim::Float);
        let mut names: Vec<&'static str> = Vec::new();
        for p in set {
            if float && *p == Prim::Int {
                continue;
            }
            let n = match (self.opts.language, p) {
                (Language::TypeScript, Prim::Bool) => "boolean",
                (Language::TypeScript, Prim::Int | Prim::Float) => "number",
                (Language::TypeScript, Prim::Str) => "string",
                (Language::Rust, Prim::Bool) => "bool",
                (Language::Rust, Prim::Int) => "i64",
                (Language::Rust, Prim::Float) => "f64",
                (Language::Rust, Prim::Str) => "String",
                (Language::Go, Prim::Bool) => "bool",
                (Language::Go, Prim::Int) => "int64",
                (Language::Go, Prim::Float) => "float64",
                (Language::Go, Prim::Str) => "string",
                (Language::Python, Prim::Bool) => "bool",
                (Language::Python, Prim::Int) => "int",
                (Language::Python, Prim::Float) => "float",
                (Language::Python, Prim::Str) => "str",
            };
            if !names.contains(&n) {
                names.push(n);
            }
        }
        match names.len() {
            0 => self.any_type(),
            1 => names[0].to_string(),
            _ => match self.opts.language {
                // TypeScript and Python spell unions; Rust and Go have to fall back.
                Language::TypeScript => names.join(" | "),
                Language::Python => {
                    self.py.union = true;
                    format!("Union[{}]", names.join(", "))
                }
                _ => self.any_type(),
            },
        }
    }

    fn array_of(&mut self, elem: &str) -> String {
        match self.opts.language {
            Language::TypeScript => {
                if elem.contains(' ') {
                    format!("({elem})[]")
                } else {
                    format!("{elem}[]")
                }
            }
            Language::Rust => format!("Vec<{elem}>"),
            Language::Go => format!("[]{elem}"),
            Language::Python => {
                self.py.list = true;
                format!("List[{elem}]")
            }
        }
    }

    /// Wrap `ty` so it can also hold "absent" (a missing key) or `null`.
    fn make_optional(&mut self, ty: String) -> String {
        match self.opts.language {
            Language::TypeScript => ty, // expressed by the `?` marker / a `| null` union
            Language::Rust => {
                if ty.starts_with("Option<") {
                    ty
                } else {
                    format!("Option<{ty}>")
                }
            }
            Language::Go => {
                // Slices, maps and interfaces are already nilable — a pointer to one is noise.
                if ty.starts_with('*') || ty.starts_with('[') || ty.starts_with("map[") || ty == "any" {
                    ty
                } else {
                    format!("*{ty}")
                }
            }
            Language::Python => {
                if ty.starts_with("Optional[") || ty == "Any" {
                    ty
                } else {
                    self.py.optional = true;
                    format!("Optional[{ty}]")
                }
            }
        }
    }

    /// Type expression for `node`. `suppress_null` drops the nullability (the caller is
    /// expressing it some other way, e.g. a `?` marker).
    fn type_expr(&mut self, node: &Node, hint: &str, suppress_null: bool) -> String {
        let base = self.base_expr(node, hint);
        let show_null =
            node.nullable && !suppress_null && self.strategy() != OptionalStrategy::Required;
        if !show_null {
            return base;
        }
        match self.opts.language {
            Language::TypeScript => {
                if base == "unknown" {
                    // `unknown | null` is just `unknown`; a null-only field is exactly `null`.
                    if matches!(node.shape, Shape::Unknown) {
                        "null".to_string()
                    } else {
                        base
                    }
                } else {
                    format!("{base} | null")
                }
            }
            // serde_json::Value / any / Any already include null.
            Language::Rust if base == "serde_json::Value" => base,
            Language::Go if base == "any" => base,
            Language::Python if base == "Any" => base,
            _ => self.make_optional(base),
        }
    }

    fn base_expr(&mut self, node: &Node, hint: &str) -> String {
        match &node.shape {
            Shape::Unknown | Shape::Mixed => self.any_type(),
            Shape::Prims(set) => self.prim_union(set),
            Shape::Arr(inner) => {
                let elem = self.type_expr(inner, &singularize(hint), false);
                self.array_of(&elem)
            }
            Shape::Obj(fields) if fields.is_empty() => self.map_type(),
            Shape::Obj(fields) => {
                if let Some(existing) = self.shape_names.get(node) {
                    return existing.clone();
                }
                if self.used_names.len() >= MAX_TYPES {
                    if self.err.is_none() {
                        self.err = Some(format!(
                            "sample would generate more than {MAX_TYPES} types — trim the JSON to one representative record"
                        ));
                    }
                    return self.any_type();
                }
                let name = self.unique_type_name(hint);
                self.shape_names.insert(node.clone(), name.clone());
                let members = self.members(fields);
                let decl = self.render_decl(&name, &members);
                self.decls.push(decl);
                name
            }
        }
    }

    fn members(&mut self, fields: &[(String, Field)]) -> Vec<Member> {
        let mut out = Vec::with_capacity(fields.len());
        for (key, f) in fields {
            // In `optional` mode nullability is folded into the optional marker; the other
            // modes keep it in the type (or drop it entirely).
            let fold_null = self.strategy() == OptionalStrategy::Optional;
            let mut ty = self.type_expr(&f.node, &pascal(key), fold_null);
            let optional = match self.strategy() {
                OptionalStrategy::Optional => f.missing || f.node.nullable,
                OptionalStrategy::Nullable => f.missing,
                OptionalStrategy::Required => false,
            };
            if optional {
                ty = self.make_optional(ty);
            }
            let name = match self.opts.language {
                Language::TypeScript => key.clone(),
                Language::Rust => {
                    let s = snake(key);
                    if RUST_KEYWORDS.contains(&s.as_str()) {
                        format!("r#{s}")
                    } else {
                        s
                    }
                }
                Language::Go => go_pascal(key),
                Language::Python => {
                    let s = snake(key);
                    if PYTHON_KEYWORDS.contains(&s.as_str()) {
                        format!("{s}_")
                    } else {
                        s
                    }
                }
            };
            out.push(Member { name, key: key.clone(), ty, optional });
        }
        out
    }

    fn render_decl(&mut self, name: &str, members: &[Member]) -> String {
        match self.opts.language {
            Language::TypeScript => self.render_ts(name, members),
            Language::Rust => self.render_rust(name, members),
            Language::Go => self.render_go(name, members),
            Language::Python => self.render_python(name, members),
        }
    }

    fn render_ts(&self, name: &str, members: &[Member]) -> String {
        let kw = if self.opts.export { "export interface" } else { "interface" };
        let mut s = format!("{kw} {name} {{\n");
        for m in members {
            let key =
                if is_ts_ident(&m.key) { m.key.clone() } else { format!("\"{}\"", escape_dq(&m.key)) };
            let opt = if m.optional { "?" } else { "" };
            s.push_str(&format!("  {key}{opt}: {};\n", m.ty));
        }
        s.push_str("}\n");
        s
    }

    fn render_rust(&self, name: &str, members: &[Member]) -> String {
        let derives = if self.opts.json_annotations {
            "#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]"
        } else {
            "#[derive(Debug, Clone, PartialEq)]"
        };
        let vis = if self.opts.export { "pub " } else { "" };
        let mut s = format!("{derives}\n{vis}struct {name} {{\n");
        for m in members {
            let bare = m.name.strip_prefix("r#").unwrap_or(&m.name);
            if self.opts.json_annotations && bare != m.key {
                s.push_str(&format!("    #[serde(rename = \"{}\")]\n", escape_dq(&m.key)));
            } else if !self.opts.json_annotations && bare != m.key {
                s.push_str(&format!("    // JSON key: \"{}\"\n", escape_dq(&m.key)));
            }
            s.push_str(&format!("    {vis}{}: {},\n", m.name, m.ty));
        }
        s.push_str("}\n");
        s
    }

    fn render_go(&self, name: &str, members: &[Member]) -> String {
        let type_name = if self.opts.export { name.to_string() } else { lower_first(name) };
        let type_name = if GO_KEYWORDS.contains(&type_name.as_str()) {
            format!("{type_name}Type")
        } else {
            type_name
        };
        let name_w = members.iter().map(|m| m.name.chars().count()).max().unwrap_or(0);
        let ty_w = members.iter().map(|m| m.ty.chars().count()).max().unwrap_or(0);
        let mut s = format!("type {type_name} struct {{\n");
        for m in members {
            let tag = if self.opts.json_annotations {
                let omit = if m.optional { ",omitempty" } else { "" };
                if m.key.contains('`') {
                    format!(" \"json:\\\"{}{omit}\\\"\"", escape_dq(&m.key))
                } else {
                    format!(" `json:\"{}{omit}\"`", m.key)
                }
            } else {
                String::new()
            };
            if tag.is_empty() {
                s.push_str(&format!(
                    "\t{:<nw$} {}\n",
                    m.name,
                    m.ty,
                    nw = name_w
                ));
            } else {
                s.push_str(&format!(
                    "\t{:<nw$} {:<tw$}{}\n",
                    m.name,
                    m.ty,
                    tag,
                    nw = name_w,
                    tw = ty_w
                ));
            }
        }
        s.push_str("}\n");
        s
    }

    fn render_python(&mut self, name: &str, members: &[Member]) -> String {
        self.emitted_class = true;
        let mut s = format!("@dataclass\nclass {name}:\n");
        if members.is_empty() {
            s.push_str("    pass\n");
            return s;
        }
        // A dataclass field with a default may not precede one without.
        let (required, defaulted): (Vec<&Member>, Vec<&Member>) =
            members.iter().partition(|m| !m.optional);
        for m in required.into_iter().chain(defaulted) {
            let default = if m.optional { " = None" } else { "" };
            let note = if m.name != m.key {
                format!("  # JSON key: \"{}\"", escape_dq(&m.key))
            } else {
                String::new()
            };
            s.push_str(&format!("    {}: {}{}{}\n", m.name, m.ty, default, note));
        }
        s
    }

    fn header(&self) -> String {
        match self.opts.language {
            Language::Rust if self.opts.json_annotations && !self.decls.is_empty() => {
                "use serde::{Deserialize, Serialize};\n\n".to_string()
            }
            Language::Python => {
                let mut typing: Vec<&str> = Vec::new();
                if self.py.any {
                    typing.push("Any");
                }
                if self.py.dict {
                    typing.push("Dict");
                }
                if self.py.list {
                    typing.push("List");
                }
                if self.py.optional {
                    typing.push("Optional");
                }
                if self.py.union {
                    typing.push("Union");
                }
                let mut h = String::from("from __future__ import annotations\n\n");
                if self.emitted_class {
                    h.push_str("from dataclasses import dataclass\n");
                }
                if !typing.is_empty() {
                    h.push_str(&format!("from typing import {}\n", typing.join(", ")));
                }
                h.push('\n');
                h
            }
            _ => String::new(),
        }
    }

    fn alias(&self, root: &str, target: &str) -> String {
        match self.opts.language {
            Language::TypeScript => {
                let kw = if self.opts.export { "export type" } else { "type" };
                format!("{kw} {root} = {target};\n")
            }
            Language::Rust => {
                let vis = if self.opts.export { "pub " } else { "" };
                format!("{vis}type {root} = {target};\n")
            }
            Language::Go => {
                let n = if self.opts.export { root.to_string() } else { lower_first(root) };
                format!("type {n} = {target}\n")
            }
            Language::Python => format!("{root} = {target}\n"),
        }
    }
}

// ---------------------------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------------------------

/// Infer type declarations for `json` in the language selected by `opts`.
pub fn generate(json: &str, opts: &Options) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err(
            "input JSON is empty — paste a JSON object or array to infer types from".to_string()
        );
    }
    if json.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "JSON sample is {} bytes, over the {MAX_INPUT_BYTES}-byte limit — one representative record is enough",
            json.len()
        ));
    }
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let node = from_value(&value, 0)?;

    let root_hint = {
        let trimmed = opts.root_name.trim();
        let p = if trimmed.is_empty() { "Root".to_string() } else { pascal(trimmed) };
        if p.is_empty() {
            "Root".to_string()
        } else {
            p
        }
    };
    let root_name = match opts.language {
        Language::Go => go_pascal(&root_hint),
        _ => root_hint.clone(),
    };

    let mut gen = Gen::new(opts);
    // Reserve the root name so a nested `root` key can't steal it.
    gen.used_names.insert(root_name.clone());
    gen.used_names.remove(&root_name);
    let top = gen.type_expr(&node, &root_name, false);

    if let Some(e) = gen.err {
        return Err(e);
    }

    let mut out = gen.header();
    out.push_str(&gen.decls.join("\n"));
    if top != root_name && top != lower_first(&root_name) {
        if !gen.decls.is_empty() {
            out.push('\n');
        }
        out.push_str(&gen.alias(&root_name, &top));
    }
    let out = out.trim_end().to_string();
    if out.is_empty() {
        return Err("no types could be inferred from that sample".to_string());
    }
    Ok(out + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(lang: Language) -> Options {
        Options { language: lang, root_name: "Root".into(), ..Default::default() }
    }

    const SAMPLE: &str = r#"{"id":1,"name":"Ada","active":true,"score":9.5,
        "address":{"city":"London","zip":null},
        "roles":["admin","dev"],
        "tags":[{"k":"a","n":2},{"k":"b"}]}"#;

    // ---- happy paths, one per language --------------------------------------------------

    #[test]
    fn typescript_happy_path() {
        let out = generate(SAMPLE, &Options { root_name: "User".into(), ..opts(Language::TypeScript) })
            .unwrap();
        assert!(out.contains("export interface User {"), "got:\n{out}");
        assert!(out.contains("  id: number;"), "got:\n{out}");
        assert!(out.contains("  name: string;"), "got:\n{out}");
        assert!(out.contains("  active: boolean;"), "got:\n{out}");
        assert!(out.contains("  score: number;"), "got:\n{out}");
        assert!(out.contains("  address: Address;"), "got:\n{out}");
        assert!(out.contains("  roles: string[];"), "got:\n{out}");
        assert!(out.contains("  tags: Tag[];"), "got:\n{out}");
        // nested + array-element interfaces are declared before the root
        assert!(out.contains("export interface Address {"), "got:\n{out}");
        assert!(out.contains("  zip?: unknown;"), "got:\n{out}");
        assert!(out.contains("export interface Tag {"), "got:\n{out}");
        assert!(out.contains("  n?: number;"), "b/n missing in one element:\n{out}");
        assert!(out.find("interface Address").unwrap() < out.find("interface User").unwrap());
    }

    #[test]
    fn rust_happy_path() {
        let out =
            generate(SAMPLE, &Options { root_name: "User".into(), ..opts(Language::Rust) }).unwrap();
        assert!(out.starts_with("use serde::{Deserialize, Serialize};"), "got:\n{out}");
        assert!(out.contains("#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]"));
        assert!(out.contains("pub struct User {"), "got:\n{out}");
        assert!(out.contains("    pub id: i64,"), "got:\n{out}");
        assert!(out.contains("    pub score: f64,"), "got:\n{out}");
        assert!(out.contains("    pub roles: Vec<String>,"), "got:\n{out}");
        assert!(out.contains("    pub tags: Vec<Tag>,"), "got:\n{out}");
        assert!(out.contains("    pub n: Option<i64>,"), "got:\n{out}");
    }

    #[test]
    fn go_happy_path() {
        let out =
            generate(SAMPLE, &Options { root_name: "User".into(), ..opts(Language::Go) }).unwrap();
        assert!(out.contains("type User struct {"), "got:\n{out}");
        assert!(out.contains("`json:\"id\"`"), "got:\n{out}");
        assert!(out.contains("int64"), "got:\n{out}");
        assert!(out.contains("Roles   []string `json:\"roles\"`"), "aligned columns:\n{out}");
        assert!(out.contains("`json:\"n,omitempty\"`"), "got:\n{out}");
        assert!(out.contains("N *int64"), "nullable scalar becomes a pointer:\n{out}");
    }

    #[test]
    fn python_happy_path() {
        let out =
            generate(SAMPLE, &Options { root_name: "User".into(), ..opts(Language::Python) })
                .unwrap();
        assert!(out.starts_with("from __future__ import annotations"), "got:\n{out}");
        assert!(out.contains("from dataclasses import dataclass"), "got:\n{out}");
        assert!(out.contains("@dataclass\nclass User:"), "got:\n{out}");
        assert!(out.contains("    id: int"), "got:\n{out}");
        assert!(out.contains("    score: float"), "got:\n{out}");
        assert!(out.contains("    roles: List[str]"), "got:\n{out}");
        assert!(out.contains("    tags: List[Tag]"), "got:\n{out}");
        assert!(out.contains("    n: Optional[int] = None"), "got:\n{out}");
    }

    // ---- errors -------------------------------------------------------------------------

    #[test]
    fn invalid_json_errors() {
        let e = generate("{not json}", &opts(Language::TypeScript)).unwrap_err();
        assert!(e.starts_with("invalid JSON:"), "got: {e}");
    }

    #[test]
    fn empty_input_errors() {
        let e = generate("   ", &opts(Language::Go)).unwrap_err();
        assert!(e.contains("empty"), "got: {e}");
    }

    #[test]
    fn unknown_language_errors() {
        let e = Language::parse("kotlin").unwrap_err();
        assert!(e.contains("expected typescript, rust, go or python"), "got: {e}");
    }

    #[test]
    fn unknown_strategy_errors() {
        assert!(OptionalStrategy::parse("maybe").is_err());
    }

    #[test]
    fn too_deep_errors() {
        let mut s = String::new();
        for _ in 0..(MAX_DEPTH + 3) {
            s.push_str("{\"a\":");
        }
        s.push('1');
        for _ in 0..(MAX_DEPTH + 3) {
            s.push('}');
        }
        let e = generate(&s, &opts(Language::TypeScript)).unwrap_err();
        assert!(e.contains("nested deeper"), "got: {e}");
    }

    #[test]
    fn oversized_input_errors() {
        let big = format!("{{\"a\":\"{}\"}}", "x".repeat(MAX_INPUT_BYTES));
        let e = generate(&big, &opts(Language::Rust)).unwrap_err();
        assert!(e.contains("over the"), "got: {e}");
    }

    // ---- inference behaviour ------------------------------------------------------------

    #[test]
    fn key_order_is_preserved() {
        let out = generate(r#"{"zeta":1,"alpha":2,"mid":3}"#, &opts(Language::TypeScript)).unwrap();
        let z = out.find("zeta").unwrap();
        let a = out.find("alpha").unwrap();
        let m = out.find("mid").unwrap();
        assert!(z < a && a < m, "insertion order, not alphabetical:\n{out}");
    }

    #[test]
    fn heterogeneous_array_elements_merge_into_one_type() {
        let out = generate(r#"{"items":[{"a":1},{"a":2,"b":"x"}]}"#, &opts(Language::TypeScript))
            .unwrap();
        assert_eq!(out.matches("interface Item").count(), 1, "one merged type:\n{out}");
        assert!(out.contains("  a: number;"), "got:\n{out}");
        assert!(out.contains("  b?: string;"), "got:\n{out}");
        assert!(out.contains("  items: Item[];"), "got:\n{out}");
    }

    #[test]
    fn identical_shapes_are_deduplicated() {
        let out = generate(
            r#"{"home":{"city":"A","zip":"1"},"work":{"city":"B","zip":"2"}}"#,
            &opts(Language::TypeScript),
        )
        .unwrap();
        assert_eq!(out.matches("export interface").count(), 2, "Home reused for work:\n{out}");
        assert!(out.contains("  work: Home;"), "got:\n{out}");
    }

    #[test]
    fn int_and_float_widen_to_float() {
        let out = generate(r#"{"n":[1,2.5]}"#, &opts(Language::Rust)).unwrap();
        assert!(out.contains("pub n: Vec<f64>,"), "got:\n{out}");
    }

    #[test]
    fn primitive_union_per_language() {
        let ts = generate(r#"{"mixed":[1,"two",true]}"#, &opts(Language::TypeScript)).unwrap();
        assert!(ts.contains("mixed: (boolean | number | string)[];"), "got:\n{ts}");
        let py = generate(r#"{"mixed":[1,"two",true]}"#, &opts(Language::Python)).unwrap();
        assert!(py.contains("mixed: List[Union[bool, int, str]]"), "got:\n{py}");
        let rs = generate(r#"{"mixed":[1,"two",true]}"#, &opts(Language::Rust)).unwrap();
        assert!(rs.contains("pub mixed: Vec<serde_json::Value>,"), "got:\n{rs}");
        let go = generate(r#"{"mixed":[1,"two",true]}"#, &opts(Language::Go)).unwrap();
        assert!(go.contains("[]any"), "got:\n{go}");
    }

    #[test]
    fn conflicting_shapes_fall_back_to_any() {
        let out = generate(r#"{"x":[{"a":1},"plain"]}"#, &opts(Language::TypeScript)).unwrap();
        assert!(out.contains("x: unknown[];"), "got:\n{out}");
    }

    #[test]
    fn empty_object_and_array_become_open_types() {
        let ts = generate(r#"{"meta":{},"list":[]}"#, &opts(Language::TypeScript)).unwrap();
        assert!(ts.contains("meta: Record<string, unknown>;"), "got:\n{ts}");
        assert!(ts.contains("list: unknown[];"), "got:\n{ts}");
        let go = generate(r#"{"meta":{},"list":[]}"#, &opts(Language::Go)).unwrap();
        assert!(go.contains("map[string]any"), "got:\n{go}");
        assert!(go.contains("[]any"), "got:\n{go}");
    }

    #[test]
    fn root_array_gets_a_type_alias() {
        let ts = generate(r#"[{"id":1}]"#, &Options { root_name: "Row".into(), ..opts(Language::TypeScript) }).unwrap();
        assert!(ts.contains("export interface RowItem {"), "got:\n{ts}");
        assert!(ts.contains("export type Row = RowItem[];"), "got:\n{ts}");
        let py = generate(r#"[{"id":1}]"#, &Options { root_name: "Row".into(), ..opts(Language::Python) }).unwrap();
        assert!(py.contains("= List["), "got:\n{py}");
    }

    #[test]
    fn root_primitive_gets_an_alias() {
        let out = generate("\"hello\"", &opts(Language::TypeScript)).unwrap();
        assert_eq!(out, "export type Root = string;\n");
    }

    // ---- optional strategies -------------------------------------------------------------

    #[test]
    fn strategy_optional_folds_null_into_the_marker() {
        let out = generate(
            r#"{"a":null,"b":[{"x":1},{}]}"#,
            &Options {
                optional_strategy: OptionalStrategy::Optional,
                ..opts(Language::TypeScript)
            },
        )
        .unwrap();
        assert!(out.contains("a?: unknown;"), "got:\n{out}");
        assert!(out.contains("x?: number;"), "got:\n{out}");
    }

    #[test]
    fn strategy_nullable_keeps_the_field_required() {
        let out = generate(
            r#"{"a":null,"b":"s","c":[1,null]}"#,
            &Options {
                optional_strategy: OptionalStrategy::Nullable,
                ..opts(Language::TypeScript)
            },
        )
        .unwrap();
        assert!(out.contains("a: null;"), "got:\n{out}");
        assert!(out.contains("b: string;"), "got:\n{out}");
        assert!(out.contains("c: (number | null)[];"), "got:\n{out}");
    }

    #[test]
    fn strategy_required_drops_optionality() {
        let out = generate(
            r#"[{"x":1,"y":null},{"x":2}]"#,
            &Options {
                optional_strategy: OptionalStrategy::Required,
                root_name: "Row".into(),
                ..opts(Language::TypeScript)
            },
        )
        .unwrap();
        assert!(!out.contains('?'), "no optional markers:\n{out}");
        assert!(out.contains("x: number;"), "got:\n{out}");
        assert!(out.contains("y: unknown;"), "got:\n{out}");
    }

    #[test]
    fn python_defaulted_fields_sort_last() {
        let out = generate(
            r#"[{"a":1,"b":2},{"b":3}]"#,
            &Options { root_name: "Row".into(), ..opts(Language::Python) },
        )
        .unwrap();
        let b = out.find("    b: int").unwrap();
        let a = out.find("    a: Optional[int] = None").unwrap();
        assert!(b < a, "defaults must come last or Python raises at import:\n{out}");
    }

    // ---- naming --------------------------------------------------------------------------

    #[test]
    fn rust_renames_non_snake_keys() {
        let out = generate(r#"{"userID":1,"first-name":"Ada","type":"x"}"#, &opts(Language::Rust))
            .unwrap();
        assert!(out.contains("#[serde(rename = \"userID\")]"), "got:\n{out}");
        assert!(out.contains("pub user_id: i64,"), "got:\n{out}");
        assert!(out.contains("pub first_name: String,"), "got:\n{out}");
        // `r#type` already serialises as "type", so no rename attribute is needed.
        assert!(out.contains("pub r#type: String,"), "keyword escaped:\n{out}");
        assert!(!out.contains("rename = \"type\""), "redundant rename:\n{out}");
    }

    #[test]
    fn go_uses_initialisms_and_tags() {
        let out = generate(r#"{"user_id":1,"api_url":"x"}"#, &opts(Language::Go)).unwrap();
        assert!(out.contains("UserID"), "got:\n{out}");
        assert!(out.contains("APIURL"), "got:\n{out}");
        assert!(out.contains("`json:\"user_id\"`"), "got:\n{out}");
    }

    #[test]
    fn python_escapes_keywords_and_notes_the_original_key() {
        let out = generate(r#"{"class":"a","first-name":"b"}"#, &opts(Language::Python)).unwrap();
        assert!(out.contains("class_: str  # JSON key: \"class\""), "got:\n{out}");
        assert!(out.contains("first_name: str  # JSON key: \"first-name\""), "got:\n{out}");
    }

    #[test]
    fn typescript_quotes_non_identifier_keys() {
        let out = generate(r#"{"first-name":"Ada"}"#, &opts(Language::TypeScript)).unwrap();
        assert!(out.contains("\"first-name\": string;"), "got:\n{out}");
    }

    #[test]
    fn plural_keys_singularize_element_types() {
        let out = generate(
            r#"{"entries":[{"a":1}],"categories":[{"b":1}],"boxes":[{"c":1}]}"#,
            &opts(Language::TypeScript),
        )
        .unwrap();
        assert!(out.contains("interface Entry {"), "got:\n{out}");
        assert!(out.contains("interface Category {"), "got:\n{out}");
        assert!(out.contains("interface Box {"), "got:\n{out}");
    }

    // ---- toggles ---------------------------------------------------------------------------

    #[test]
    fn export_toggle_applies_per_language() {
        let ts = generate(r#"{"a":1}"#, &Options { export: false, ..opts(Language::TypeScript) })
            .unwrap();
        assert!(ts.starts_with("interface Root {"), "got:\n{ts}");
        let rs =
            generate(r#"{"a":1}"#, &Options { export: false, ..opts(Language::Rust) }).unwrap();
        assert!(rs.contains("struct Root {"), "got:\n{rs}");
        assert!(!rs.contains("pub "), "got:\n{rs}");
        let go = generate(r#"{"a":1}"#, &Options { export: false, ..opts(Language::Go) }).unwrap();
        assert!(go.contains("type root struct {"), "got:\n{go}");
    }

    #[test]
    fn annotations_toggle_strips_serde_and_tags() {
        let rs = generate(
            r#"{"userID":1}"#,
            &Options { json_annotations: false, ..opts(Language::Rust) },
        )
        .unwrap();
        assert!(!rs.contains("use serde"), "got:\n{rs}");
        assert!(!rs.contains("Serialize"), "got:\n{rs}");
        assert!(rs.contains("// JSON key: \"userID\""), "got:\n{rs}");
        let go = generate(
            r#"{"user_id":1}"#,
            &Options { json_annotations: false, ..opts(Language::Go) },
        )
        .unwrap();
        assert!(!go.contains("json:"), "got:\n{go}");
    }

    #[test]
    fn output_is_deterministic() {
        let a = generate(SAMPLE, &opts(Language::Go)).unwrap();
        let b = generate(SAMPLE, &opts(Language::Go)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn language_aliases_parse() {
        assert_eq!(Language::parse("TS").unwrap(), Language::TypeScript);
        assert_eq!(Language::parse("golang").unwrap(), Language::Go);
        assert_eq!(Language::parse("py").unwrap(), Language::Python);
        assert_eq!(Language::parse(" Rust ").unwrap(), Language::Rust);
    }
}
