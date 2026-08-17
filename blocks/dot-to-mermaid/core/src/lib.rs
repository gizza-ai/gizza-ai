//! dot-to-mermaid core — translate Graphviz DOT source into Mermaid flowchart
//! syntax. Pure compute: text in, text out, no I/O and no rendering, so the same
//! code backs the chat block, the CLI and the browser page.
//!
//! The DOT grammar is parsed by a hand-written lexer + recursive-descent parser
//! (nodes, edges, chained edges, ports, attribute lists, `node`/`edge`/`graph`
//! defaults, nested subgraphs and `cluster_*` clusters, comments, quoted and
//! HTML-ish labels). The Mermaid emitter then maps what has an equivalent —
//! direction, node shapes, edge labels, link styles, subgraphs, colors — and
//! records a note for everything Graphviz can express and Mermaid cannot.

use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;

/// Largest accepted DOT source.
pub const MAX_BYTES: usize = 1_000_000;
/// Largest number of distinct nodes in one graph.
pub const MAX_NODES: usize = 2000;
/// Largest number of edges in one graph.
pub const MAX_EDGES: usize = 5000;
/// Largest number of conversion notes emitted (further notes are summarised).
pub const MAX_NOTES: usize = 20;

/// Conversion options. Every boolean is on by default: the defaults produce the
/// highest-fidelity translation, and turning one off yields plainer Mermaid.
#[derive(Clone, Debug)]
pub struct Options {
    /// `auto` (follow the graph's `rankdir`) or an explicit `TD`/`LR`/`BT`/`RL`.
    pub direction: String,
    /// Map DOT `shape=` attributes onto Mermaid node shapes.
    pub shapes: bool,
    /// Carry DOT edge `label=` / `xlabel=` text onto the Mermaid link.
    pub edge_labels: bool,
    /// Map DOT edge `style=`/`dir=` onto Mermaid link types (dotted/thick/open/…).
    pub link_styles: bool,
    /// Translate `subgraph cluster_*` blocks into Mermaid `subgraph` blocks.
    pub subgraphs: bool,
    /// Emit `style` / `linkStyle` lines for DOT colors and pen widths.
    pub colors: bool,
    /// Append `%%` comment notes for DOT features with no Mermaid equivalent.
    pub warnings: bool,
    /// Diagram title (YAML front matter). Empty falls back to the graph `label`.
    pub title: String,
    /// Wrap the output in a ```` ```mermaid ```` fence for Markdown files.
    pub fence: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            direction: "auto".into(),
            shapes: true,
            edge_labels: true,
            link_styles: true,
            subgraphs: true,
            colors: true,
            warnings: true,
            title: String::new(),
            fence: false,
        }
    }
}

/// Convert Graphviz DOT source into Mermaid flowchart source.
pub fn convert(dot: &str, opts: &Options) -> Result<String, String> {
    if dot.len() > MAX_BYTES {
        return Err(format!(
            "DOT source is {} bytes; the limit is {MAX_BYTES} bytes.",
            dot.len()
        ));
    }
    let src = dot.trim();
    if src.is_empty() {
        return Err(
            "DOT source is empty — paste a graph such as 'digraph { a -> b; }'.".into(),
        );
    }
    let direction = resolve_requested_direction(&opts.direction)?;
    let toks = lex(src)?;
    let doc = Parser::new(toks).parse()?;
    if doc.nodes.len() > MAX_NODES {
        return Err(format!(
            "graph has {} nodes; the limit is {MAX_NODES}.",
            doc.nodes.len()
        ));
    }
    if doc.edges.len() > MAX_EDGES {
        return Err(format!(
            "graph has {} edges; the limit is {MAX_EDGES}.",
            doc.edges.len()
        ));
    }
    Ok(emit(&doc, opts, direction))
}

/// `None` = follow the graph's own `rankdir`.
fn resolve_requested_direction(requested: &str) -> Result<Option<&'static str>, String> {
    match requested.trim().to_ascii_uppercase().as_str() {
        "" | "AUTO" => Ok(None),
        "TD" | "TB" => Ok(Some("TD")),
        "LR" => Ok(Some("LR")),
        "BT" => Ok(Some("BT")),
        "RL" => Ok(Some("RL")),
        other => Err(format!(
            "unknown direction '{other}' — expected auto, TD, LR, BT or RL."
        )),
    }
}

// ---------------------------------------------------------------------------
// Lexer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    /// An identifier, numeral, quoted string or HTML-ish string. The flag marks
    /// quoted/HTML strings, which are never keywords.
    Word(String, bool),
    Arrow,
    Line,
    LBrace,
    RBrace,
    LBrack,
    RBrack,
    Semi,
    Comma,
    Eq,
    Colon,
}

impl Tok {
    fn describe(&self) -> String {
        match self {
            Tok::Word(w, _) => format!("'{w}'"),
            Tok::Arrow => "'->'".into(),
            Tok::Line => "'--'".into(),
            Tok::LBrace => "'{'".into(),
            Tok::RBrace => "'}'".into(),
            Tok::LBrack => "'['".into(),
            Tok::RBrack => "']'".into(),
            Tok::Semi => "';'".into(),
            Tok::Comma => "','".into(),
            Tok::Eq => "'='".into(),
            Tok::Colon => "':'".into(),
        }
    }
}

fn lex(src: &str) -> Result<Vec<Tok>, String> {
    let c: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut line = 1usize;
    let mut out: Vec<Tok> = Vec::new();

    while i < c.len() {
        let ch = c[i];
        if ch == '\n' {
            line += 1;
            i += 1;
            continue;
        }
        if ch.is_whitespace() {
            i += 1;
            continue;
        }
        // `#` line is a C-preprocessor line; `//` and `/* */` are comments.
        if ch == '#' || (ch == '/' && i + 1 < c.len() && c[i + 1] == '/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if ch == '/' && i + 1 < c.len() && c[i + 1] == '*' {
            let start = line;
            i += 2;
            loop {
                if i + 1 >= c.len() {
                    return Err(format!(
                        "unterminated /* comment opened on line {start} — add a closing */."
                    ));
                }
                if c[i] == '*' && c[i + 1] == '/' {
                    i += 2;
                    break;
                }
                if c[i] == '\n' {
                    line += 1;
                }
                i += 1;
            }
            continue;
        }

        match ch {
            '{' => {
                out.push(Tok::LBrace);
                i += 1;
            }
            '}' => {
                out.push(Tok::RBrace);
                i += 1;
            }
            '[' => {
                out.push(Tok::LBrack);
                i += 1;
            }
            ']' => {
                out.push(Tok::RBrack);
                i += 1;
            }
            ';' => {
                out.push(Tok::Semi);
                i += 1;
            }
            ',' => {
                out.push(Tok::Comma);
                i += 1;
            }
            '=' => {
                out.push(Tok::Eq);
                i += 1;
            }
            ':' => {
                out.push(Tok::Colon);
                i += 1;
            }
            '"' => {
                let (mut text, next, nl) = read_quoted(&c, i, line)?;
                i = next;
                line = nl;
                // DOT concatenates adjacent quoted strings joined by `+`.
                loop {
                    let mut j = i;
                    while j < c.len() && c[j].is_whitespace() {
                        j += 1;
                    }
                    if j < c.len() && c[j] == '+' {
                        let mut k = j + 1;
                        while k < c.len() && c[k].is_whitespace() {
                            k += 1;
                        }
                        if k < c.len() && c[k] == '"' {
                            let (more, next2, nl2) = read_quoted(&c, k, line)?;
                            text.push_str(&more);
                            i = next2;
                            line = nl2;
                            continue;
                        }
                    }
                    break;
                }
                out.push(Tok::Word(text, true));
            }
            '<' => {
                let (text, next, nl) = read_html(&c, i, line)?;
                i = next;
                line = nl;
                out.push(Tok::Word(text, true));
            }
            '-' => {
                if i + 1 < c.len() && c[i + 1] == '>' {
                    out.push(Tok::Arrow);
                    i += 2;
                } else if i + 1 < c.len() && c[i + 1] == '-' {
                    out.push(Tok::Line);
                    i += 2;
                } else {
                    let (text, next) = read_numeral(&c, i, line)?;
                    i = next;
                    out.push(Tok::Word(text, false));
                }
            }
            _ if ch.is_ascii_digit() || ch == '.' => {
                let (text, next) = read_numeral(&c, i, line)?;
                i = next;
                out.push(Tok::Word(text, false));
            }
            _ if ch.is_alphabetic() || ch == '_' || !ch.is_ascii() => {
                let start = i;
                while i < c.len()
                    && (c[i].is_alphanumeric() || c[i] == '_' || !c[i].is_ascii())
                {
                    i += 1;
                }
                out.push(Tok::Word(c[start..i].iter().collect(), false));
            }
            other => {
                return Err(format!(
                    "unexpected character '{other}' on line {line} — this does not look like DOT source."
                ));
            }
        }
    }
    Ok(out)
}

/// Read a `"…"` string starting at `i`. `\"` becomes `"`, a backslash before a
/// newline is a line continuation; every other escape is left intact so the
/// label escaper can see `\n` / `\l` / `\N`.
fn read_quoted(c: &[char], i: usize, line: usize) -> Result<(String, usize, usize), String> {
    let mut i = i + 1;
    let mut line = line;
    let start = line;
    let mut out = String::new();
    while i < c.len() {
        match c[i] {
            '"' => return Ok((out, i + 1, line)),
            '\\' if i + 1 < c.len() => {
                match c[i + 1] {
                    '"' => out.push('"'),
                    '\n' => line += 1, // line continuation: drop both chars
                    other => {
                        out.push('\\');
                        out.push(other);
                    }
                }
                i += 2;
            }
            '\n' => {
                line += 1;
                out.push('\n');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    Err(format!(
        "unterminated quoted string opened on line {start} — add a closing \"."
    ))
}

/// Read an HTML-like `<…>` label and flatten it to text (tags dropped, `<br/>`
/// kept as a line break, the common entities decoded).
fn read_html(c: &[char], i: usize, line: usize) -> Result<(String, usize, usize), String> {
    let start = line;
    let mut line = line;
    let mut depth = 0usize;
    let mut raw = String::new();
    let mut i = i;
    while i < c.len() {
        match c[i] {
            '<' => {
                depth += 1;
                if depth > 1 {
                    raw.push('<');
                }
                i += 1;
            }
            '>' => {
                depth -= 1;
                if depth == 0 {
                    let text = strip_html(&raw);
                    return Ok((text, i + 1, line));
                }
                raw.push('>');
                i += 1;
            }
            '\n' => {
                line += 1;
                raw.push('\n');
                i += 1;
            }
            other => {
                raw.push(other);
                i += 1;
            }
        }
    }
    Err(format!(
        "unterminated HTML-style label opened on line {start} — add a closing >."
    ))
}

fn strip_html(raw: &str) -> String {
    let mut out = String::new();
    let mut tag = String::new();
    let mut in_tag = false;
    for ch in raw.chars() {
        match ch {
            '<' => {
                in_tag = true;
                tag.clear();
            }
            '>' if in_tag => {
                in_tag = false;
                let t = tag.trim().to_ascii_lowercase();
                if t.starts_with("br") || t == "tr" || t == "/tr" {
                    out.push('\n');
                } else if t == "td" || t == "/td" {
                    out.push(' ');
                }
            }
            _ if in_tag => tag.push(ch),
            _ => out.push(ch),
        }
    }
    out.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&amp;", "&")
        .trim()
        .to_string()
}

fn read_numeral(c: &[char], i: usize, line: usize) -> Result<(String, usize), String> {
    let start = i;
    let mut i = i;
    if c[i] == '-' {
        i += 1;
    }
    let digits_start = i;
    while i < c.len() && (c[i].is_ascii_digit() || c[i] == '.') {
        i += 1;
    }
    if i == digits_start {
        return Err(format!(
            "stray '-' on line {line} — expected a number or an edge operator ('->' or '--')."
        ));
    }
    Ok((c[start..i].iter().collect(), i))
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct NodeRec {
    name: String,
    attrs: HashMap<String, String>,
    cluster: Option<usize>,
    degree: usize,
}

struct EdgeRec {
    from: usize,
    to: usize,
    attrs: HashMap<String, String>,
    undirected: bool,
}

struct Cluster {
    name: String,
    label: String,
    parent: Option<usize>,
    nodes: Vec<usize>,
    children: Vec<usize>,
}

#[derive(Clone, Default)]
struct Scope {
    node_defaults: HashMap<String, String>,
    edge_defaults: HashMap<String, String>,
    cluster: Option<usize>,
}

struct Doc {
    directed: bool,
    graph_attrs: HashMap<String, String>,
    nodes: Vec<NodeRec>,
    edges: Vec<EdgeRec>,
    clusters: Vec<Cluster>,
    warnings: Vec<String>,
}

struct Parser {
    toks: Vec<Tok>,
    pos: usize,
    directed: bool,
    strict: bool,
    graph_attrs: HashMap<String, String>,
    nodes: Vec<NodeRec>,
    index: HashMap<String, usize>,
    edges: Vec<EdgeRec>,
    clusters: Vec<Cluster>,
    warnings: Vec<String>,
}

impl Parser {
    fn new(toks: Vec<Tok>) -> Self {
        Parser {
            toks,
            pos: 0,
            directed: true,
            strict: false,
            graph_attrs: HashMap::new(),
            nodes: Vec::new(),
            index: HashMap::new(),
            edges: Vec::new(),
            clusters: Vec::new(),
            warnings: Vec::new(),
        }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos)
    }

    fn peek_at(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.pos + n)
    }

    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn expect(&mut self, t: Tok) -> Result<(), String> {
        if self.eat(&t) {
            Ok(())
        } else {
            Err(format!(
                "expected {} but found {}.",
                t.describe(),
                self.peek().map(|p| p.describe()).unwrap_or_else(|| "end of input".into())
            ))
        }
    }

    fn is_kw(&self, n: usize, kw: &str) -> bool {
        matches!(self.peek_at(n), Some(Tok::Word(w, false)) if w.eq_ignore_ascii_case(kw))
    }

    fn word(&mut self) -> Result<String, String> {
        match self.bump() {
            Some(Tok::Word(w, _)) => Ok(w),
            Some(other) => Err(format!("expected a name but found {}.", other.describe())),
            None => Err("expected a name but reached the end of the input.".into()),
        }
    }

    fn warn(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if !self.warnings.contains(&msg) {
            self.warnings.push(msg);
        }
    }

    fn parse(mut self) -> Result<Doc, String> {
        if self.is_kw(0, "strict") {
            self.pos += 1;
            self.strict = true;
        }
        if self.is_kw(0, "digraph") {
            self.directed = true;
            self.pos += 1;
        } else if self.is_kw(0, "graph") {
            self.directed = false;
            self.pos += 1;
        } else {
            return Err(format!(
                "expected 'graph' or 'digraph' at the start of the source but found {}.",
                self.peek().map(|p| p.describe()).unwrap_or_else(|| "end of input".into())
            ));
        }
        // Optional graph name.
        if matches!(self.peek(), Some(Tok::Word(_, _))) {
            self.pos += 1;
        }
        self.expect(Tok::LBrace)?;
        let mut scope = Scope::default();
        let mut gattrs = HashMap::new();
        let mut members = Vec::new();
        self.stmt_list(&mut scope, &mut gattrs, &mut members)?;
        self.expect(Tok::RBrace)?;
        if self.pos < self.toks.len() {
            return Err(format!(
                "unexpected {} after the closing '}}'.",
                self.toks[self.pos].describe()
            ));
        }
        self.graph_attrs = gattrs;

        if self.strict {
            self.dedupe_edges();
        }
        for e in &self.edges {
            self.nodes[e.from].degree += 1;
            self.nodes[e.to].degree += 1;
        }
        Ok(Doc {
            directed: self.directed,
            graph_attrs: self.graph_attrs,
            nodes: self.nodes,
            edges: self.edges,
            clusters: self.clusters,
            warnings: self.warnings,
        })
    }

    fn dedupe_edges(&mut self) {
        let mut seen: HashSet<(usize, usize)> = HashSet::new();
        let mut kept = Vec::new();
        for e in std::mem::take(&mut self.edges) {
            let key = if self.directed && !e.undirected {
                (e.from, e.to)
            } else if e.from <= e.to {
                (e.from, e.to)
            } else {
                (e.to, e.from)
            };
            if seen.insert(key) {
                kept.push(e);
            }
        }
        self.edges = kept;
    }

    fn stmt_list(
        &mut self,
        scope: &mut Scope,
        gattrs: &mut HashMap<String, String>,
        members: &mut Vec<usize>,
    ) -> Result<(), String> {
        loop {
            match self.peek() {
                None | Some(Tok::RBrace) => return Ok(()),
                Some(Tok::Semi) | Some(Tok::Comma) => {
                    self.pos += 1;
                    continue;
                }
                _ => {}
            }
            let before = self.pos;

            // `node [..]` / `edge [..]` / `graph [..]` attribute defaults.
            if (self.is_kw(0, "node") || self.is_kw(0, "edge") || self.is_kw(0, "graph"))
                && matches!(self.peek_at(1), Some(Tok::LBrack))
            {
                let which = self.word()?.to_ascii_lowercase();
                let attrs = self.attr_list()?;
                match which.as_str() {
                    "node" => scope.node_defaults.extend(attrs),
                    "edge" => scope.edge_defaults.extend(attrs),
                    _ => gattrs.extend(attrs),
                }
                continue;
            }

            // A subgraph (or a plain `{ … }` group) — possibly an edge endpoint.
            if self.is_kw(0, "subgraph") || matches!(self.peek(), Some(Tok::LBrace)) {
                let group = self.subgraph(scope, members)?;
                if matches!(self.peek(), Some(Tok::Arrow) | Some(Tok::Line)) {
                    self.edge_chain(group, scope, members)?;
                }
                continue;
            }

            // `name = value` graph attribute.
            if matches!(self.peek(), Some(Tok::Word(_, _)))
                && matches!(self.peek_at(1), Some(Tok::Eq))
            {
                let key = self.word()?;
                self.expect(Tok::Eq)?;
                let value = self.word()?;
                gattrs.insert(key.to_ascii_lowercase(), value);
                continue;
            }

            // Otherwise: a node statement, possibly the head of an edge chain.
            let lhs = self.endpoint(scope, members)?;
            if matches!(self.peek(), Some(Tok::Arrow) | Some(Tok::Line)) {
                self.edge_chain(lhs, scope, members)?;
            } else if matches!(self.peek(), Some(Tok::LBrack)) {
                let attrs = self.attr_list()?;
                for &n in &lhs {
                    self.nodes[n].attrs.extend(attrs.clone());
                }
            }

            if self.pos == before {
                return Err(format!(
                    "unexpected {} — could not read a statement here.",
                    self.peek().map(|p| p.describe()).unwrap_or_else(|| "end of input".into())
                ));
            }
        }
    }

    /// One edge endpoint: a node id (with optional `:port:compass`) or a
    /// subgraph, which stands for every node inside it.
    fn endpoint(
        &mut self,
        scope: &mut Scope,
        members: &mut Vec<usize>,
    ) -> Result<Vec<usize>, String> {
        if self.is_kw(0, "subgraph") || matches!(self.peek(), Some(Tok::LBrace)) {
            return self.subgraph(scope, members);
        }
        let name = self.word()?;
        // Ports (`node:f0:sw`) have no Mermaid equivalent — read and drop them.
        let mut had_port = false;
        while self.eat(&Tok::Colon) {
            self.word()?;
            had_port = true;
        }
        if had_port {
            self.warn("edge ports (node:port) are not supported by Mermaid; the port was dropped");
        }
        let idx = self.node(&name, scope);
        members.push(idx);
        Ok(vec![idx])
    }

    fn subgraph(
        &mut self,
        scope: &Scope,
        parent_members: &mut Vec<usize>,
    ) -> Result<Vec<usize>, String> {
        let mut name = String::new();
        if self.is_kw(0, "subgraph") {
            self.pos += 1;
            if matches!(self.peek(), Some(Tok::Word(_, _))) {
                name = self.word()?;
            }
        }
        self.expect(Tok::LBrace)?;

        let is_cluster = name.to_ascii_lowercase().starts_with("cluster");
        let mut sub = scope.clone();
        if is_cluster {
            let idx = self.clusters.len();
            self.clusters.push(Cluster {
                name: name.clone(),
                label: String::new(),
                parent: scope.cluster,
                nodes: Vec::new(),
                children: Vec::new(),
            });
            if let Some(p) = scope.cluster {
                self.clusters[p].children.push(idx);
            }
            sub.cluster = Some(idx);
        }

        let mut gattrs = HashMap::new();
        let mut members = Vec::new();
        self.stmt_list(&mut sub, &mut gattrs, &mut members)?;
        self.expect(Tok::RBrace)?;

        if is_cluster {
            let idx = sub.cluster.expect("cluster index set above");
            if let Some(label) = gattrs.get("label") {
                self.clusters[idx].label = label.clone();
            }
        } else if !gattrs.is_empty() {
            self.warn(
                "attributes on an anonymous subgraph (rank, style) have no Mermaid equivalent and were dropped",
            );
        }

        parent_members.extend(members.iter().copied());
        Ok(members)
    }

    fn edge_chain(
        &mut self,
        first: Vec<usize>,
        scope: &mut Scope,
        members: &mut Vec<usize>,
    ) -> Result<(), String> {
        let mut lhs = first;
        let mut pairs: Vec<(usize, usize, bool)> = Vec::new();
        while matches!(self.peek(), Some(Tok::Arrow) | Some(Tok::Line)) {
            let undirected = self.peek() == Some(&Tok::Line);
            self.pos += 1;
            let rhs = self.endpoint(scope, members)?;
            for &a in &lhs {
                for &b in &rhs {
                    pairs.push((a, b, undirected));
                }
            }
            lhs = rhs;
        }
        let explicit = if matches!(self.peek(), Some(Tok::LBrack)) {
            self.attr_list()?
        } else {
            HashMap::new()
        };
        for (a, b, undirected) in pairs {
            let mut attrs = scope.edge_defaults.clone();
            attrs.extend(explicit.clone());
            self.edges.push(EdgeRec {
                from: a,
                to: b,
                attrs,
                undirected,
            });
        }
        Ok(())
    }

    fn attr_list(&mut self) -> Result<HashMap<String, String>, String> {
        let mut out = HashMap::new();
        while self.eat(&Tok::LBrack) {
            loop {
                match self.peek() {
                    Some(Tok::RBrack) => {
                        self.pos += 1;
                        break;
                    }
                    Some(Tok::Semi) | Some(Tok::Comma) => {
                        self.pos += 1;
                        continue;
                    }
                    None => {
                        return Err(
                            "unterminated attribute list — add a closing ']'.".into()
                        )
                    }
                    _ => {}
                }
                let key = self.word()?;
                let value = if self.eat(&Tok::Eq) {
                    self.word()?
                } else {
                    "true".to_string()
                };
                out.insert(key.to_ascii_lowercase(), value);
            }
        }
        Ok(out)
    }

    fn node(&mut self, name: &str, scope: &Scope) -> usize {
        if let Some(&idx) = self.index.get(name) {
            return idx;
        }
        let idx = self.nodes.len();
        self.nodes.push(NodeRec {
            name: name.to_string(),
            attrs: scope.node_defaults.clone(),
            cluster: scope.cluster,
            degree: 0,
        });
        self.index.insert(name.to_string(), idx);
        if let Some(c) = scope.cluster {
            self.clusters[c].nodes.push(idx);
        }
        idx
    }
}

// ---------------------------------------------------------------------------
// Mermaid emitter
// ---------------------------------------------------------------------------

/// Mermaid keywords that break flowchart parsing when used bare as a node id.
/// (`o` and `x` are safe here because every link this emitter writes is
/// surrounded by spaces, which is what Mermaid needs to not read them as
/// circle/cross edge endings.)
const RESERVED: &[&str] = &[
    "end", "graph", "subgraph", "flowchart", "class", "classdef", "click", "style", "linkstyle",
    "default", "direction", "call", "href", "link", "callback", "interpolate",
];

fn shape_wrap(shape: &str) -> Option<(&'static str, &'static str)> {
    Some(match shape.trim().to_ascii_lowercase().as_str() {
        "box" | "rect" | "rectangle" | "square" | "none" | "plaintext" | "plain" | "note"
        | "tab" | "folder" => ("[", "]"),
        "ellipse" | "oval" | "egg" => ("(", ")"),
        "circle" | "point" => ("((", "))"),
        "doublecircle" => ("(((", ")))"),
        "diamond" | "mdiamond" => ("{", "}"),
        "hexagon" => ("{{", "}}"),
        "cylinder" => ("[(", ")]"),
        "component" | "box3d" | "msquare" => ("[[", "]]"),
        "parallelogram" => ("[/", "/]"),
        "trapezium" => ("[/", "\\]"),
        "invtrapezium" => ("[\\", "/]"),
        _ => return None,
    })
}

#[derive(PartialEq, Clone, Copy)]
enum LineKind {
    Solid,
    Dotted,
    Thick,
    Invisible,
}

#[derive(PartialEq, Clone, Copy)]
enum ArrowKind {
    Normal,
    Open,
    Both,
}

fn connector(line: LineKind, arrow: ArrowKind, label: &str) -> String {
    if line == LineKind::Invisible {
        return "~~~".into();
    }
    if label.is_empty() {
        return match (line, arrow) {
            (LineKind::Solid, ArrowKind::Normal) => "-->".into(),
            (LineKind::Solid, ArrowKind::Open) => "---".into(),
            (LineKind::Solid, ArrowKind::Both) => "<-->".into(),
            (LineKind::Dotted, ArrowKind::Normal) => "-.->".into(),
            (LineKind::Dotted, ArrowKind::Open) => "-.-".into(),
            (LineKind::Dotted, ArrowKind::Both) => "<-.->".into(),
            (LineKind::Thick, ArrowKind::Normal) => "==>".into(),
            (LineKind::Thick, ArrowKind::Open) => "===".into(),
            (LineKind::Thick, ArrowKind::Both) => "<==>".into(),
            (LineKind::Invisible, _) => "~~~".into(),
        };
    }
    match (line, arrow) {
        // The pipe form is the documented way to label solid links.
        (LineKind::Solid, ArrowKind::Normal) => format!("-->|{label}|"),
        (LineKind::Solid, ArrowKind::Open) => format!("---|{label}|"),
        (LineKind::Solid, ArrowKind::Both) => format!("<-->|{label}|"),
        // Dotted and thick links carry their text in the middle of the link.
        (LineKind::Dotted, ArrowKind::Normal) => format!("-. {label} .->"),
        (LineKind::Dotted, ArrowKind::Open) => format!("-. {label} .-"),
        (LineKind::Dotted, ArrowKind::Both) => format!("<-. {label} .->"),
        (LineKind::Thick, ArrowKind::Normal) => format!("== {label} ==>"),
        (LineKind::Thick, ArrowKind::Open) => format!("== {label} ==="),
        (LineKind::Thick, ArrowKind::Both) => format!("<== {label} ==>"),
        (LineKind::Invisible, _) => "~~~".into(),
    }
}

/// Escape DOT label text for a Mermaid label. DOT's `\n` / `\l` / `\r` line
/// breaks become `<br/>`, `\N` expands to the node name, and every character
/// Mermaid treats as syntax becomes its numeric entity.
fn escape_label(raw: &str, node_name: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') | Some('l') | Some('r') => out.push_str("<br/>"),
                Some('N') => out.push_str(&escape_label(node_name, "")),
                Some('\\') => out.push('\\'),
                Some(other) => out.push(other),
                None => {}
            }
            continue;
        }
        match ch {
            '"' => out.push_str("#quot;"),
            '<' => out.push_str("#lt;"),
            '>' => out.push_str("#gt;"),
            '|' => out.push_str("#124;"),
            '{' => out.push_str("#123;"),
            '}' => out.push_str("#125;"),
            '(' => out.push_str("#40;"),
            ')' => out.push_str("#41;"),
            '[' => out.push_str("#91;"),
            ']' => out.push_str("#93;"),
            '#' => out.push_str("#35;"),
            '\n' => out.push_str("<br/>"),
            other => out.push(other),
        }
    }
    out
}

fn sanitize_id(raw: &str, used: &mut HashSet<String>) -> String {
    let mut base: String = raw
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    if base.is_empty() {
        base = "n".into();
    }
    if base.starts_with(|c: char| c.is_ascii_digit()) {
        base = format!("n{base}");
    }
    if RESERVED.iter().any(|r| r.eq_ignore_ascii_case(&base)) {
        base = format!("{base}_");
    }
    let mut candidate = base.clone();
    let mut n = 2;
    while !used.insert(candidate.clone()) {
        candidate = format!("{base}_{n}");
        n += 1;
    }
    candidate
}

/// A DOT color that Mermaid/CSS can use verbatim, or `None` for color lists,
/// colorscheme references and other Graphviz-only forms.
fn css_color(value: &str) -> Option<String> {
    let v = value.trim();
    if v.is_empty() {
        return None;
    }
    if let Some(hex) = v.strip_prefix('#') {
        let ok = matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit());
        return ok.then(|| v.to_ascii_lowercase());
    }
    v.chars()
        .all(|c| c.is_ascii_alphabetic())
        .then(|| v.to_ascii_lowercase())
}

struct Emitter<'a> {
    doc: &'a Doc,
    opts: &'a Options,
    ids: Vec<String>,
    cluster_ids: Vec<String>,
    notes: Vec<String>,
}

fn emit(doc: &Doc, opts: &Options, requested: Option<&'static str>) -> String {
    let mut used: HashSet<String> = HashSet::new();
    let ids: Vec<String> = doc
        .nodes
        .iter()
        .map(|n| sanitize_id(&n.name, &mut used))
        .collect();
    let cluster_ids: Vec<String> = doc
        .clusters
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let raw = if c.name.is_empty() {
                format!("cluster_{i}")
            } else {
                c.name.clone()
            };
            sanitize_id(&raw, &mut used)
        })
        .collect();

    let mut em = Emitter {
        doc,
        opts,
        ids,
        cluster_ids,
        notes: doc.warnings.clone(),
    };
    em.render(requested)
}

impl Emitter<'_> {
    fn note(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        if !self.notes.contains(&msg) {
            self.notes.push(msg);
        }
    }

    fn direction(&mut self, requested: Option<&'static str>) -> &'static str {
        if let Some(d) = requested {
            return d;
        }
        match self
            .doc
            .graph_attrs
            .get("rankdir")
            .map(|s| s.trim().to_ascii_uppercase())
            .as_deref()
        {
            Some("LR") => "LR",
            Some("BT") => "BT",
            Some("RL") => "RL",
            Some("TB") | Some("TD") | None => "TD",
            Some(other) => {
                self.note(format!("unknown rankdir '{other}'; used TD"));
                "TD"
            }
        }
    }

    /// The label a node should display, and whether it carries information the
    /// bare Mermaid id would lose.
    fn node_label(&self, i: usize) -> (String, bool) {
        let n = &self.doc.nodes[i];
        match n.attrs.get("label") {
            Some(l) => (escape_label(l, &n.name), true),
            None => (escape_label(&n.name, &n.name), self.ids[i] != n.name),
        }
    }

    fn node_decl(&mut self, i: usize, force: bool) -> Option<String> {
        let (label, informative) = self.node_label(i);
        let shape = if self.opts.shapes {
            self.doc.nodes[i].attrs.get("shape").cloned()
        } else {
            None
        };
        let wrap = match shape.as_deref() {
            None | Some("") => None,
            Some(s) => match shape_wrap(s) {
                Some(w) => Some(w),
                None => {
                    self.note(format!(
                        "Graphviz shape '{s}' has no Mermaid equivalent; a rectangle was used"
                    ));
                    Some(("[", "]"))
                }
            },
        };
        if wrap.is_none() && !informative && !force {
            return None;
        }
        let (open, close) = wrap.unwrap_or(("[", "]"));
        Some(format!("{}{open}\"{label}\"{close}", self.ids[i]))
    }

    fn render(&mut self, requested: Option<&'static str>) -> String {
        let direction = self.direction(requested);
        let mut out = String::new();

        let title = if !self.opts.title.trim().is_empty() {
            self.opts.title.trim().to_string()
        } else {
            self.doc
                .graph_attrs
                .get("label")
                .map(|l| l.replace("\\n", " ").replace('\n', " ").trim().to_string())
                .unwrap_or_default()
        };
        if !title.is_empty() {
            let needs_quotes = title.contains(':') || title.contains('#') || title.contains('"');
            let value = if needs_quotes {
                format!("\"{}\"", title.replace('"', "\\\""))
            } else {
                title.clone()
            };
            let _ = writeln!(out, "---\ntitle: {value}\n---");
        }
        let _ = writeln!(out, "flowchart {direction}");

        // Top-level node declarations (nodes not inside a cluster).
        let clustered = self.opts.subgraphs;
        for i in 0..self.doc.nodes.len() {
            if clustered && self.doc.nodes[i].cluster.is_some() {
                continue;
            }
            let force = self.doc.nodes[i].degree == 0;
            if let Some(decl) = self.node_decl(i, force) {
                let _ = writeln!(out, "    {decl}");
            }
        }

        // Subgraphs.
        if clustered {
            let roots: Vec<usize> = (0..self.doc.clusters.len())
                .filter(|&i| self.doc.clusters[i].parent.is_none())
                .collect();
            for root in roots {
                self.render_cluster(root, 1, &mut out);
            }
        } else if !self.doc.clusters.is_empty() {
            let n = self.doc.clusters.len();
            self.note(format!(
                "{n} subgraph cluster(s) flattened because subgraph output is off"
            ));
        }

        // Edges.
        for i in 0..self.doc.edges.len() {
            let line = self.render_edge(i);
            let _ = writeln!(out, "    {line}");
        }

        // Colors.
        if self.opts.colors {
            let styles = self.render_styles();
            for line in styles {
                let _ = writeln!(out, "    {line}");
            }
        }

        // Conversion notes.
        if self.opts.warnings && !self.notes.is_empty() {
            let total = self.notes.len();
            let shown = total.min(MAX_NOTES);
            let _ = writeln!(out, "    %% Conversion notes:");
            for note in self.notes.iter().take(shown) {
                let _ = writeln!(out, "    %% - {note}");
            }
            if total > shown {
                let _ = writeln!(out, "    %% - …and {} more note(s)", total - shown);
            }
        }

        let body = out.trim_end().to_string();
        if self.opts.fence {
            format!("```mermaid\n{body}\n```")
        } else {
            body
        }
    }

    fn render_cluster(&mut self, ci: usize, depth: usize, out: &mut String) {
        let indent = "    ".repeat(depth);
        let c = &self.doc.clusters[ci];
        let raw_label = if c.label.is_empty() {
            c.name.clone()
        } else {
            c.label.clone()
        };
        let label = escape_label(&raw_label, &c.name);
        let id = self.cluster_ids[ci].clone();
        let nodes = c.nodes.clone();
        let children = c.children.clone();
        let _ = writeln!(out, "{indent}subgraph {id}[\"{label}\"]");
        for i in nodes {
            let decl = self
                .node_decl(i, true)
                .unwrap_or_else(|| self.ids[i].clone());
            let _ = writeln!(out, "{indent}    {decl}");
        }
        for child in children {
            self.render_cluster(child, depth + 1, out);
        }
        let _ = writeln!(out, "{indent}end");
    }

    fn render_edge(&mut self, i: usize) -> String {
        let e = &self.doc.edges[i];
        let attrs = e.attrs.clone();
        let (mut a, mut b) = (self.ids[e.from].clone(), self.ids[e.to].clone());
        let undirected = e.undirected;

        let style = attrs
            .get("style")
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        let dir = attrs
            .get("dir")
            .map(|s| s.trim().to_ascii_lowercase())
            .unwrap_or_default();

        let mut arrow = if !self.doc.directed || undirected || dir == "none" {
            ArrowKind::Open
        } else if dir == "both" {
            ArrowKind::Both
        } else {
            ArrowKind::Normal
        };
        if dir == "back" {
            std::mem::swap(&mut a, &mut b);
            arrow = ArrowKind::Normal;
            self.note("dir=back edges were reversed (Mermaid has no backward-arrow link)");
        }
        let mut line = LineKind::Solid;
        if self.opts.link_styles {
            if style.contains("invis") {
                line = LineKind::Invisible;
            } else if style.contains("dashed") || style.contains("dotted") {
                line = LineKind::Dotted;
            } else if style.contains("bold") {
                line = LineKind::Thick;
            }
        } else {
            arrow = if self.doc.directed && !undirected {
                ArrowKind::Normal
            } else {
                ArrowKind::Open
            };
        }
        if style.contains("dashed") && style.contains("dotted") {
            self.note("Mermaid renders both dashed and dotted DOT edges as a dotted link");
        }

        let label = if self.opts.edge_labels {
            attrs
                .get("label")
                .or_else(|| attrs.get("xlabel"))
                .or_else(|| attrs.get("headlabel"))
                .map(|l| escape_label(l, ""))
                .unwrap_or_default()
        } else {
            String::new()
        };
        if !label.is_empty() && line == LineKind::Invisible {
            self.note("labels on invisible edges were dropped (Mermaid's ~~~ link takes no text)");
        }
        let conn = connector(line, arrow, &label);
        format!("{a} {conn} {b}")
    }

    fn render_styles(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        let mut bad_color = false;

        for i in 0..self.doc.nodes.len() {
            let attrs = self.doc.nodes[i].attrs.clone();
            let style = attrs
                .get("style")
                .map(|s| s.to_ascii_lowercase())
                .unwrap_or_default();
            let filled = style.contains("filled");
            let mut parts: Vec<String> = Vec::new();
            let fill_src = attrs
                .get("fillcolor")
                .or_else(|| if filled { attrs.get("color") } else { None });
            if let Some(raw) = fill_src {
                match css_color(raw) {
                    Some(c) => parts.push(format!("fill:{c}")),
                    None => bad_color = true,
                }
            }
            if let Some(raw) = attrs.get("color") {
                match css_color(raw) {
                    Some(c) => parts.push(format!("stroke:{c}")),
                    None => bad_color = true,
                }
            }
            if let Some(raw) = attrs.get("fontcolor") {
                match css_color(raw) {
                    Some(c) => parts.push(format!("color:{c}")),
                    None => bad_color = true,
                }
            }
            if let Some(w) = attrs.get("penwidth").and_then(|w| w.parse::<f64>().ok()) {
                parts.push(format!("stroke-width:{w}px"));
            }
            if !parts.is_empty() {
                lines.push(format!("style {} {}", self.ids[i], parts.join(",")));
            }
        }

        for i in 0..self.doc.edges.len() {
            let attrs = self.doc.edges[i].attrs.clone();
            let mut parts: Vec<String> = Vec::new();
            if let Some(raw) = attrs.get("color") {
                match css_color(raw) {
                    Some(c) => parts.push(format!("stroke:{c}")),
                    None => bad_color = true,
                }
            }
            if let Some(w) = attrs.get("penwidth").and_then(|w| w.parse::<f64>().ok()) {
                parts.push(format!("stroke-width:{w}px"));
            }
            if !parts.is_empty() {
                lines.push(format!("linkStyle {i} {}", parts.join(",")));
            }
        }

        if bad_color {
            self.note(
                "Graphviz color lists and colorscheme references (e.g. 'red:blue', '/set19/3') were skipped",
            );
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(dot: &str) -> String {
        convert(dot, &Options::default()).unwrap()
    }

    #[test]
    fn simple_digraph_becomes_flowchart() {
        assert_eq!(conv("digraph { a -> b; }"), "flowchart TD\n    a --> b");
    }

    #[test]
    fn chained_edges_expand() {
        assert_eq!(
            conv("digraph { a -> b -> c; }"),
            "flowchart TD\n    a --> b\n    b --> c"
        );
    }

    #[test]
    fn rankdir_drives_direction() {
        assert!(conv("digraph { rankdir=LR; a -> b; }").starts_with("flowchart LR\n"));
        assert!(conv("digraph { graph [rankdir=BT]; a -> b; }").starts_with("flowchart BT\n"));
    }

    #[test]
    fn explicit_direction_overrides_rankdir() {
        let opts = Options {
            direction: "RL".into(),
            ..Options::default()
        };
        let out = convert("digraph { rankdir=LR; a -> b; }", &opts).unwrap();
        assert!(out.starts_with("flowchart RL\n"), "got: {out}");
    }

    #[test]
    fn labels_and_shapes_map() {
        let out = conv("digraph { a [label=\"Start\", shape=circle]; b [shape=diamond, label=\"OK?\"]; a -> b [label=\"go\"]; }");
        assert!(out.contains("a((\"Start\"))"), "got: {out}");
        assert!(out.contains("b{\"OK?\"}"), "got: {out}");
        assert!(out.contains("a -->|go| b"), "got: {out}");
    }

    #[test]
    fn undirected_graph_uses_open_links() {
        assert_eq!(conv("graph { x -- y; }"), "flowchart TD\n    x --- y");
    }

    #[test]
    fn edge_styles_map_to_link_types() {
        let out = conv(
            "digraph { a -> b [style=dashed]; b -> c [style=bold]; c -> d [dir=none]; d -> e [dir=both]; }",
        );
        assert!(out.contains("a -.-> b"), "got: {out}");
        assert!(out.contains("b ==> c"), "got: {out}");
        assert!(out.contains("c --- d"), "got: {out}");
        assert!(out.contains("d <--> e"), "got: {out}");
    }

    #[test]
    fn dotted_and_thick_labels_use_midtext_form() {
        let out = conv("digraph { a -> b [style=dotted, label=\"maybe\"]; b -> c [style=bold, label=\"hot\"]; }");
        assert!(out.contains("a -. maybe .-> b"), "got: {out}");
        assert!(out.contains("b == hot ==> c"), "got: {out}");
    }

    #[test]
    fn clusters_become_subgraphs() {
        let out = conv(
            "digraph { subgraph cluster_api { label=\"API\"; a; b; } c; a -> c; }",
        );
        assert!(out.contains("subgraph cluster_api[\"API\"]"), "got: {out}");
        assert!(out.contains("    end"), "got: {out}");
        assert!(out.contains("a --> c"), "got: {out}");
    }

    #[test]
    fn subgraphs_off_flattens_clusters() {
        let opts = Options {
            subgraphs: false,
            ..Options::default()
        };
        let out = convert("digraph { subgraph cluster_a { a; } a -> b; }", &opts).unwrap();
        assert!(
            !out.lines().any(|l| l.trim_start().starts_with("subgraph ")),
            "got: {out}"
        );
        assert!(out.contains("flattened"), "got: {out}");
    }

    #[test]
    fn node_and_edge_defaults_apply() {
        let out = conv("digraph { node [shape=box]; edge [style=dashed]; a -> b; }");
        assert!(out.contains("a[\"a\"]"), "got: {out}");
        assert!(out.contains("a -.-> b"), "got: {out}");
    }

    #[test]
    fn colors_become_style_lines() {
        let out = conv(
            "digraph { a [style=filled, fillcolor=\"#ffcc00\"]; a -> b [color=red, penwidth=2]; }",
        );
        assert!(out.contains("style a fill:#ffcc00"), "got: {out}");
        assert!(out.contains("linkStyle 0 stroke:red,stroke-width:2px"), "got: {out}");
    }

    #[test]
    fn colors_off_drops_style_lines() {
        let opts = Options {
            colors: false,
            ..Options::default()
        };
        let out = convert("digraph { a [style=filled, fillcolor=red]; a -> b; }", &opts).unwrap();
        assert!(!out.contains("style a"), "got: {out}");
    }

    #[test]
    fn unsafe_ids_are_sanitised_and_labelled() {
        let out = conv("digraph { \"my node\" -> end; }");
        assert!(out.contains("my_node[\"my node\"]"), "got: {out}");
        assert!(out.contains("end_[\"end\"]"), "got: {out}");
        assert!(out.contains("my_node --> end_"), "got: {out}");
    }

    #[test]
    fn label_special_characters_are_escaped() {
        let out = conv("digraph { a [label=\"a\\nb <c> {d}\"]; a -> b; }");
        assert!(out.contains("a[\"a<br/>b #lt;c#gt; #123;d#125;\"]"), "got: {out}");
    }

    #[test]
    fn graph_label_becomes_title() {
        let out = conv("digraph { label=\"My pipeline\"; a -> b; }");
        assert!(out.starts_with("---\ntitle: My pipeline\n---\nflowchart TD"), "got: {out}");
    }

    #[test]
    fn title_option_wins_and_fence_wraps() {
        let opts = Options {
            title: "Build".into(),
            fence: true,
            ..Options::default()
        };
        let out = convert("digraph { label=\"Ignored\"; a -> b; }", &opts).unwrap();
        assert!(out.starts_with("```mermaid\n---\ntitle: Build\n"), "got: {out}");
        assert!(out.ends_with("\n```"), "got: {out}");
    }

    #[test]
    fn isolated_nodes_are_declared() {
        let out = conv("digraph { lonely; a -> b; }");
        assert!(out.contains("lonely[\"lonely\"]"), "got: {out}");
    }

    #[test]
    fn subgraph_endpoint_expands_to_cross_product() {
        let out = conv("digraph { {a b} -> c; }");
        assert!(out.contains("a --> c"), "got: {out}");
        assert!(out.contains("b --> c"), "got: {out}");
    }

    #[test]
    fn strict_deduplicates_parallel_edges() {
        let out = conv("strict digraph { a -> b; a -> b; }");
        assert_eq!(out.matches("a --> b").count(), 1, "got: {out}");
    }

    #[test]
    fn comments_ports_and_html_labels_are_handled() {
        let out = conv(
            "digraph { /* block */ // line\n a:f0 -> b; c [label=<<b>Bold</b>>]; c -> b; }",
        );
        assert!(out.contains("a --> b"), "got: {out}");
        assert!(out.contains("c[\"Bold\"]"), "got: {out}");
        assert!(out.contains("%% - edge ports"), "got: {out}");
    }

    #[test]
    fn unknown_shape_notes_and_falls_back_to_rectangle() {
        let out = conv("digraph { a [shape=star]; a -> b; }");
        assert!(out.contains("a[\"a\"]"), "got: {out}");
        assert!(out.contains("Graphviz shape 'star' has no Mermaid equivalent"), "got: {out}");
    }

    #[test]
    fn warnings_off_removes_notes() {
        let opts = Options {
            warnings: false,
            ..Options::default()
        };
        let out = convert("digraph { a [shape=star]; a -> b; }", &opts).unwrap();
        assert!(!out.contains("%%"), "got: {out}");
    }

    #[test]
    fn empty_input_errors() {
        let err = convert("   ", &Options::default()).unwrap_err();
        assert!(err.to_lowercase().contains("empty"), "got: {err}");
    }

    #[test]
    fn missing_graph_keyword_errors() {
        let err = convert("a -> b;", &Options::default()).unwrap_err();
        assert!(err.contains("expected 'graph' or 'digraph'"), "got: {err}");
    }

    #[test]
    fn unterminated_string_errors() {
        let err = convert("digraph { a [label=\"oops]; }", &Options::default()).unwrap_err();
        assert!(err.contains("unterminated quoted string"), "got: {err}");
    }

    #[test]
    fn missing_brace_errors() {
        let err = convert("digraph { a -> b;", &Options::default()).unwrap_err();
        assert!(err.contains("expected '}'"), "got: {err}");
    }

    #[test]
    fn bad_direction_errors() {
        let opts = Options {
            direction: "sideways".into(),
            ..Options::default()
        };
        let err = convert("digraph { a -> b; }", &opts).unwrap_err();
        assert!(err.contains("unknown direction"), "got: {err}");
    }

    #[test]
    fn oversized_input_errors() {
        let big = format!("digraph {{ {} }}", "a -> b; ".repeat(1));
        assert!(convert(&big, &Options::default()).is_ok());
        let huge = "x".repeat(MAX_BYTES + 1);
        let err = convert(&huge, &Options::default()).unwrap_err();
        assert!(err.contains("limit is"), "got: {err}");
    }
}
