//! rust-module-map core — turn pasted Rust source into a module/item map.
//!
//! Pure compute shared by the chat skill block and the web page. The source is
//! parsed with `syn` (an exact Rust parser — the same crate `blocks/ast-diff`
//! proves instantiates under the wafer wasm runtime), so the hierarchy comes
//! from the real grammar rather than a brace/regex heuristic: nested `mod`
//! blocks, `mod foo;` file declarations, structs/enums/unions, traits and trait
//! aliases, type aliases, free functions, `impl` blocks (inherent and
//! `impl Trait for Type`) with their associated items, consts/statics,
//! `macro_rules!` definitions, and `#[cfg(test)]` / `#[test]` items.
//!
//! Four renderings are available: an indented `tree`, a `mermaid` flowchart, a
//! structured `json` document, and a flat `paths` listing.
//!
//! Multiple files can be pasted at once using `=== src/foo.rs ===` separator
//! headers (the same convention `blocks/import-graph-extractor` accepts); each
//! file's path is mapped to its module path so `mod foo;` declarations resolve
//! into real subtrees.

use serde::Serialize;

/// Largest accepted source, in bytes. Anything above this is rejected with an
/// actionable message rather than being truncated.
pub const MAX_SOURCE_BYTES: usize = 512 * 1024;

/// Hard cap on nested module depth, so pathological input can't blow the stack
/// inside the 64 MiB wasm sandbox.
pub const MAX_MODULE_DEPTH: usize = 64;

/// Every rendering option. Mirrors the block descriptor one-for-one.
#[derive(Debug, Clone)]
pub struct Options {
    /// `tree` | `mermaid` | `json` | `paths`.
    pub format: String,
    /// Levels below the crate root to keep; `0` = unlimited.
    pub max_depth: u32,
    /// Module path to restrict the map to, e.g. `crate::config` (empty = whole crate).
    pub focus_on: String,
    /// `source` | `name` | `kind` | `visibility`.
    pub sort_by: String,
    /// Include structs, enums, unions, and type aliases.
    pub show_types: bool,
    /// Include traits and trait aliases.
    pub show_traits: bool,
    /// Include free functions and methods.
    pub show_fns: bool,
    /// Include `impl` blocks and their associated items.
    pub show_impls: bool,
    /// Include `const` and `static` items.
    pub show_consts: bool,
    /// Include `#[cfg(test)]` modules and `#[test]` functions.
    pub include_tests: bool,
    /// Annotate each node with its visibility.
    pub show_visibility: bool,
    /// Label for the crate root (empty renders as a bare `crate`).
    pub crate_name: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: "tree".into(),
            max_depth: 0,
            focus_on: String::new(),
            sort_by: "source".into(),
            show_types: true,
            show_traits: true,
            show_fns: true,
            show_impls: true,
            show_consts: false,
            include_tests: false,
            show_visibility: true,
            crate_name: String::new(),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Crate,
    Mod,
    Struct,
    Enum,
    Union,
    Trait,
    TraitAlias,
    TypeAlias,
    Fn,
    Impl,
    Const,
    Static,
    Macro,
}

impl Kind {
    /// The Rust keyword shown in front of the node name.
    fn keyword(self) -> &'static str {
        match self {
            Kind::Crate => "crate",
            Kind::Mod => "mod",
            Kind::Struct => "struct",
            Kind::Enum => "enum",
            Kind::Union => "union",
            Kind::Trait => "trait",
            Kind::TraitAlias => "trait alias",
            Kind::TypeAlias => "type",
            Kind::Fn => "fn",
            Kind::Impl => "impl",
            Kind::Const => "const",
            Kind::Static => "static",
            Kind::Macro => "macro_rules!",
        }
    }

    /// Stable machine-readable name used by the `json` and `paths` renderings.
    fn tag(self) -> &'static str {
        match self {
            Kind::Crate => "crate",
            Kind::Mod => "mod",
            Kind::Struct => "struct",
            Kind::Enum => "enum",
            Kind::Union => "union",
            Kind::Trait => "trait",
            Kind::TraitAlias => "trait_alias",
            Kind::TypeAlias => "type_alias",
            Kind::Fn => "fn",
            Kind::Impl => "impl",
            Kind::Const => "const",
            Kind::Static => "static",
            Kind::Macro => "macro",
        }
    }

    /// Ordering used by `sort_by = "kind"`.
    fn rank(self) -> u8 {
        match self {
            Kind::Crate => 0,
            Kind::Mod => 1,
            Kind::Struct => 2,
            Kind::Enum => 3,
            Kind::Union => 4,
            Kind::Trait => 5,
            Kind::TraitAlias => 6,
            Kind::TypeAlias => 7,
            Kind::Fn => 8,
            Kind::Impl => 9,
            Kind::Const => 10,
            Kind::Static => 11,
            Kind::Macro => 12,
        }
    }
}

/// One node of the module map.
#[derive(Debug, Clone)]
struct Node {
    kind: Kind,
    /// Display name, e.g. `config`, `Config`, or `Display for Config`.
    name: String,
    /// Segment used when building a `crate::a::b` path (an `impl` contributes
    /// its self type, so its methods read as real Rust paths).
    path_name: String,
    /// `pub`, `pub(crate)`, `pub(super)`, `pub(in crate::a)`, `pub(self)`, or
    /// empty when the concept doesn't apply (crate root, `impl`, inferred mod).
    vis: String,
    /// Rendered attributes worth showing, e.g. `#[cfg(test)]`, `#[test]`.
    attrs: Vec<String>,
    /// `mod foo;` whose file was not pasted in.
    unresolved: bool,
    children: Vec<Node>,
}

impl Node {
    fn new(kind: Kind, name: impl Into<String>) -> Node {
        let name = name.into();
        Node {
            kind,
            path_name: name.clone(),
            name,
            vis: String::new(),
            attrs: Vec::new(),
            unresolved: false,
            children: Vec::new(),
        }
    }
}

/// Parse `source` and render it according to `opts`.
pub fn module_map(source: &str, opts: &Options) -> Result<String, String> {
    if source.trim().is_empty() {
        return Err("source is empty — paste a Rust file (lib.rs, main.rs, or any module)".into());
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(format!(
            "source is {} bytes; the limit is {} bytes ({} KiB). Paste a single crate root or one module at a time.",
            source.len(),
            MAX_SOURCE_BYTES,
            MAX_SOURCE_BYTES / 1024
        ));
    }
    let format = opts.format.trim();
    if !matches!(format, "tree" | "mermaid" | "json" | "paths") {
        return Err(format!(
            "format must be one of tree, mermaid, json, paths (got {format:?})"
        ));
    }
    if !matches!(
        opts.sort_by.trim(),
        "" | "source" | "name" | "kind" | "visibility"
    ) {
        return Err(format!(
            "sort_by must be one of source, name, kind, visibility (got {:?})",
            opts.sort_by
        ));
    }

    let files = split_files(source)?;
    let mut parsed: Vec<(Vec<String>, syn::File)> = Vec::with_capacity(files.len());
    for (label, body) in &files {
        let file = syn::parse_file(body).map_err(|e| {
            let at = e.span().start();
            format!(
                "could not parse {label} as Rust (line {}, column {}): {e}",
                at.line,
                at.column + 1
            )
        })?;
        parsed.push((module_path_for_file(label), file));
    }

    let mut builder = Builder {
        files: parsed,
        opts,
        consumed: Vec::new(),
    };
    let crate_label = opts.crate_name.trim();
    let mut root = Node::new(Kind::Crate, crate_label);
    root.path_name = if crate_label.is_empty() {
        "crate".to_string()
    } else {
        crate_label.to_string()
    };
    root.children = builder.build_module(&[], 0)?;

    let mut root = focus(root, opts.focus_on.trim(), &root_path_name(opts))?;
    if opts.max_depth > 0 {
        prune_depth(&mut root, 0, opts.max_depth as usize);
    }
    sort_tree(&mut root, opts.sort_by.trim());

    Ok(match format {
        "tree" => render_tree(&root, opts),
        "mermaid" => render_mermaid(&root, opts),
        "json" => render_json(&root, opts)?,
        _ => render_paths(&root, opts),
    })
}

fn root_path_name(opts: &Options) -> String {
    let n = opts.crate_name.trim();
    if n.is_empty() {
        "crate".to_string()
    } else {
        n.to_string()
    }
}

// ---------------------------------------------------------------------------
// Input splitting: one paste may hold several files.
// ---------------------------------------------------------------------------

/// Split a paste into `(label, body)` files. `=== src/foo.rs ===` header lines
/// start a new file; a paste with no headers is one file called `source`.
fn split_files(source: &str) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, String)> = None;
    for line in source.lines() {
        if let Some(label) = file_header(line) {
            if let Some(prev) = current.take() {
                out.push(prev);
            }
            current = Some((label, String::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push_str(line);
            body.push('\n');
        } else if !line.trim().is_empty() {
            // Content before any header — treat the whole paste as one file.
            current = Some(("source".to_string(), String::new()));
            if let Some((_, body)) = current.as_mut() {
                body.push_str(line);
                body.push('\n');
            }
        }
    }
    if let Some(prev) = current.take() {
        out.push(prev);
    }
    if out.is_empty() {
        return Err("source is empty — paste a Rust file (lib.rs, main.rs, or any module)".into());
    }
    Ok(out)
}

/// `=== src/foo.rs ===` (any run of 3+ `=` on both sides) → `src/foo.rs`.
fn file_header(line: &str) -> Option<String> {
    let t = line.trim();
    if !t.starts_with("===") || !t.ends_with("===") || t.len() < 7 {
        return None;
    }
    let inner = t.trim_matches('=').trim();
    if inner.is_empty() {
        None
    } else {
        Some(inner.to_string())
    }
}

/// `src/lib.rs` → `[]`, `src/foo.rs` → `["foo"]`, `src/a/mod.rs` → `["a"]`,
/// `src/a/b.rs` → `["a","b"]`.
fn module_path_for_file(label: &str) -> Vec<String> {
    let p = label.trim().trim_start_matches("./");
    let p = p.strip_suffix(".rs").unwrap_or(p);
    let mut segs: Vec<String> = p
        .split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != ".")
        .map(|s| s.to_string())
        .collect();
    // Drop everything up to and including a `src` directory component.
    if let Some(i) = segs.iter().rposition(|s| s == "src") {
        segs.drain(..=i);
    }
    if segs
        .last()
        .map(|s| s == "mod" || s == "lib" || s == "main" || s == "source")
        .unwrap_or(false)
    {
        segs.pop();
    }
    segs
}

// ---------------------------------------------------------------------------
// Tree construction
// ---------------------------------------------------------------------------

struct Builder<'a> {
    files: Vec<(Vec<String>, syn::File)>,
    opts: &'a Options,
    /// Module paths already pulled in by a `mod foo;` declaration.
    consumed: Vec<Vec<String>>,
}

impl Builder<'_> {
    /// Build every child node of the module at `path`.
    fn build_module(&mut self, path: &[String], depth: usize) -> Result<Vec<Node>, String> {
        if depth > MAX_MODULE_DEPTH {
            return Err(format!(
                "module nesting is deeper than {MAX_MODULE_DEPTH} levels — this looks like generated or pathological input"
            ));
        }
        let items: Vec<syn::Item> = self
            .files
            .iter()
            .filter(|(p, _)| p.as_slice() == path)
            .flat_map(|(_, f)| f.items.iter().cloned())
            .collect();
        let mut nodes = self.build_items(&items, path, depth)?;

        // Any pasted file that is a direct child module here but was never
        // named by a `mod x;` declaration still belongs in the map.
        let mut orphans: Vec<Vec<String>> = self
            .files
            .iter()
            .map(|(p, _)| p.clone())
            .filter(|p| p.len() == path.len() + 1 && p.starts_with(path))
            .filter(|p| !self.consumed.contains(p))
            .collect();
        orphans.sort();
        orphans.dedup();
        for p in orphans {
            self.consumed.push(p.clone());
            let name = p.last().cloned().unwrap_or_default();
            let mut node = Node::new(Kind::Mod, name);
            node.children = self.build_module(&p, depth + 1)?;
            nodes.push(node);
        }
        Ok(nodes)
    }

    fn build_items(
        &mut self,
        items: &[syn::Item],
        path: &[String],
        depth: usize,
    ) -> Result<Vec<Node>, String> {
        let o = self.opts;
        let mut out = Vec::new();
        for item in items {
            let node = match item {
                syn::Item::Mod(m) => {
                    let attrs = render_attrs(&m.attrs);
                    if !o.include_tests && is_test(&attrs) {
                        continue;
                    }
                    let name = m.ident.to_string();
                    let mut node = Node::new(Kind::Mod, name.clone());
                    node.vis = visibility(&m.vis);
                    node.attrs = attrs;
                    match &m.content {
                        Some((_, inner)) => {
                            let mut child_path = path.to_vec();
                            child_path.push(name);
                            node.children = self.build_items(inner, &child_path, depth + 1)?;
                        }
                        None => {
                            let mut child_path = path.to_vec();
                            child_path.push(name);
                            if self.files.iter().any(|(p, _)| *p == child_path) {
                                self.consumed.push(child_path.clone());
                                node.children = self.build_module(&child_path, depth + 1)?;
                            } else {
                                node.unresolved = true;
                            }
                        }
                    }
                    node
                }
                syn::Item::Struct(s) if o.show_types => {
                    simple(Kind::Struct, s.ident.to_string(), &s.vis, &s.attrs)
                }
                syn::Item::Enum(e) if o.show_types => {
                    simple(Kind::Enum, e.ident.to_string(), &e.vis, &e.attrs)
                }
                syn::Item::Union(u) if o.show_types => {
                    simple(Kind::Union, u.ident.to_string(), &u.vis, &u.attrs)
                }
                syn::Item::Type(t) if o.show_types => {
                    simple(Kind::TypeAlias, t.ident.to_string(), &t.vis, &t.attrs)
                }
                syn::Item::Trait(t) if o.show_traits => {
                    simple(Kind::Trait, t.ident.to_string(), &t.vis, &t.attrs)
                }
                syn::Item::TraitAlias(t) if o.show_traits => {
                    simple(Kind::TraitAlias, t.ident.to_string(), &t.vis, &t.attrs)
                }
                syn::Item::Fn(f) if o.show_fns => {
                    let attrs = render_attrs(&f.attrs);
                    if !o.include_tests && is_test(&attrs) {
                        continue;
                    }
                    let mut n = Node::new(Kind::Fn, f.sig.ident.to_string());
                    n.vis = visibility(&f.vis);
                    n.attrs = attrs;
                    n
                }
                syn::Item::Const(c) if o.show_consts => {
                    simple(Kind::Const, c.ident.to_string(), &c.vis, &c.attrs)
                }
                syn::Item::Static(s) if o.show_consts => {
                    simple(Kind::Static, s.ident.to_string(), &s.vis, &s.attrs)
                }
                syn::Item::Impl(i) if o.show_impls => {
                    let attrs = render_attrs(&i.attrs);
                    if !o.include_tests && is_test(&attrs) {
                        continue;
                    }
                    let self_ty = tokens_to_string(&*i.self_ty);
                    let label = match &i.trait_ {
                        Some((bang, tr, _)) => format!(
                            "{}{} for {self_ty}",
                            if bang.is_some() { "!" } else { "" },
                            tokens_to_string(tr)
                        ),
                        None => self_ty.clone(),
                    };
                    let mut n = Node::new(Kind::Impl, label);
                    n.path_name = self_ty;
                    n.attrs = attrs;
                    n.children = self.build_impl_items(&i.items);
                    n
                }
                syn::Item::Macro(m) => match &m.ident {
                    Some(id) => {
                        let mut n = Node::new(Kind::Macro, id.to_string());
                        n.attrs = render_attrs(&m.attrs);
                        if !o.include_tests && is_test(&n.attrs) {
                            continue;
                        }
                        n
                    }
                    // A bare macro invocation such as `include!(…)` declares
                    // nothing on its own.
                    None => continue,
                },
                _ => continue,
            };
            out.push(node);
        }
        Ok(out)
    }

    /// Associated items of an `impl` block.
    fn build_impl_items(&self, items: &[syn::ImplItem]) -> Vec<Node> {
        let o = self.opts;
        let mut out = Vec::new();
        for it in items {
            let node = match it {
                syn::ImplItem::Fn(f) if o.show_fns => {
                    let attrs = render_attrs(&f.attrs);
                    if !o.include_tests && is_test(&attrs) {
                        continue;
                    }
                    let mut n = Node::new(Kind::Fn, f.sig.ident.to_string());
                    n.vis = visibility(&f.vis);
                    n.attrs = attrs;
                    n
                }
                syn::ImplItem::Const(c) if o.show_consts => {
                    simple(Kind::Const, c.ident.to_string(), &c.vis, &c.attrs)
                }
                syn::ImplItem::Type(t) if o.show_types => {
                    simple(Kind::TypeAlias, t.ident.to_string(), &t.vis, &t.attrs)
                }
                _ => continue,
            };
            out.push(node);
        }
        out
    }
}

fn simple(kind: Kind, name: String, vis: &syn::Visibility, attrs: &[syn::Attribute]) -> Node {
    let mut n = Node::new(kind, name);
    n.vis = visibility(vis);
    n.attrs = render_attrs(attrs);
    n
}

fn visibility(v: &syn::Visibility) -> String {
    match v {
        syn::Visibility::Public(_) => "pub".to_string(),
        syn::Visibility::Inherited => "pub(self)".to_string(),
        syn::Visibility::Restricted(r) => {
            let path = tokens_to_string(&*r.path);
            match path.as_str() {
                "crate" => "pub(crate)".to_string(),
                "super" => "pub(super)".to_string(),
                "self" => "pub(self)".to_string(),
                other => format!("pub(in {other})"),
            }
        }
    }
}

/// Keep the attributes that say something about a node's role; ignore the noisy
/// ones (`derive`, doc comments, lints).
fn render_attrs(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut out = Vec::new();
    for a in attrs {
        let name = a
            .path()
            .segments
            .last()
            .map(|s| s.ident.to_string())
            .unwrap_or_default();
        match name.as_str() {
            "cfg" => {
                if let syn::Meta::List(l) = &a.meta {
                    out.push(format!("#[cfg({})]", tidy(&l.tokens.to_string())));
                }
            }
            "test" => out.push("#[test]".to_string()),
            "bench" => out.push("#[bench]".to_string()),
            "deprecated" => out.push("#[deprecated]".to_string()),
            "no_mangle" => out.push("#[no_mangle]".to_string()),
            _ => {}
        }
    }
    out
}

/// Is this node test-only? `#[test]`, `#[bench]`, or `#[cfg(test)]`.
fn is_test(attrs: &[String]) -> bool {
    attrs
        .iter()
        .any(|a| a == "#[test]" || a == "#[bench]" || a == "#[cfg(test)]")
}

fn tokens_to_string<T: quote::ToTokens>(t: &T) -> String {
    tidy(&t.to_token_stream().to_string())
}

/// `proc-macro2` stringifies a token stream with a space around every
/// punctuation mark (`Vec < T >`). Squeeze the spaces that Rust never writes so
/// labels read like source.
fn tidy(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' {
            let prev = out.chars().last().unwrap_or(' ');
            let next = chars.get(i + 1).copied().unwrap_or(' ');
            // `-> u32` must keep its space; that `>` closes an arrow, not a
            // generic argument list.
            let after_arrow = prev == '>' && out.chars().rev().nth(1) == Some('-');
            let drop_before = matches!(prev, '<' | '(' | '[' | '&' | '\'' | ':' | '!' | '#')
                || (prev == '>' && !after_arrow);
            let drop_after = matches!(next, '<' | '>' | ')' | ']' | ',' | ':' | ';')
                && !(next == '>' && prev == '-');
            if drop_before || drop_after {
                i += 1;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Focus / depth / sorting
// ---------------------------------------------------------------------------

/// Restrict the map to one module subtree.
fn focus(root: Node, spec: &str, crate_label: &str) -> Result<Node, String> {
    if spec.is_empty() {
        return Ok(root);
    }
    let mut segs: Vec<&str> = spec
        .split("::")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if segs.first() == Some(&"crate") || segs.first().map(|s| *s == crate_label).unwrap_or(false) {
        segs.remove(0);
    }
    if segs.is_empty() {
        return Ok(root);
    }
    let mut available: Vec<String> = Vec::new();
    collect_module_paths(&root, crate_label, &mut available);

    let mut cursor = root;
    let mut walked: Vec<String> = Vec::new();
    for seg in segs {
        let found = std::mem::take(&mut cursor.children)
            .into_iter()
            .find(|c| c.kind == Kind::Mod && c.name == seg);
        match found {
            Some(next) => {
                walked.push(seg.to_string());
                cursor = next;
            }
            None => {
                return Err(format!(
                    "focus_on: no module {spec:?} in this source. Module paths available: {}",
                    if available.is_empty() {
                        "(none — this source declares no modules)".to_string()
                    } else {
                        available.join(", ")
                    }
                ))
            }
        }
    }
    cursor.name = format!("{crate_label}::{}", walked.join("::"));
    cursor.path_name = cursor.name.clone();
    Ok(cursor)
}

fn collect_module_paths(node: &Node, prefix: &str, out: &mut Vec<String>) {
    for c in &node.children {
        if c.kind == Kind::Mod {
            let path = format!("{prefix}::{}", c.name);
            out.push(path.clone());
            collect_module_paths(c, &path, out);
        }
    }
}

fn prune_depth(node: &mut Node, depth: usize, max: usize) {
    if depth >= max {
        node.children.clear();
        return;
    }
    for c in node.children.iter_mut() {
        prune_depth(c, depth + 1, max);
    }
}

fn sort_tree(node: &mut Node, mode: &str) {
    if !matches!(mode, "name" | "kind" | "visibility") {
        return;
    }
    node.children.sort_by(|a, b| match mode {
        "name" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
        "kind" => a
            .kind
            .rank()
            .cmp(&b.kind.rank())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
        _ => vis_rank(&a.vis)
            .cmp(&vis_rank(&b.vis))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())),
    });
    for c in node.children.iter_mut() {
        sort_tree(c, mode);
    }
}

/// Most visible first.
fn vis_rank(vis: &str) -> u8 {
    match vis {
        "pub" => 0,
        "pub(crate)" => 1,
        "pub(super)" => 2,
        "pub(self)" => 4,
        "" => 5,
        _ => 3, // pub(in some::path)
    }
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

/// `mod config: pub #[cfg(feature = "x")]`
fn label(node: &Node, opts: &Options) -> String {
    let mut s = if node.kind == Kind::Crate && node.name.is_empty() {
        "crate".to_string()
    } else {
        format!("{} {}", node.kind.keyword(), node.name)
    };
    if opts.show_visibility && !node.vis.is_empty() {
        s.push_str(": ");
        s.push_str(&node.vis);
    }
    for a in &node.attrs {
        s.push(' ');
        s.push_str(a);
    }
    if node.unresolved {
        s.push_str(" (external)");
    }
    s
}

fn render_tree(root: &Node, opts: &Options) -> String {
    let mut out = label(root, opts);
    out.push('\n');
    tree_children(root, "", &mut out, opts);
    out
}

fn tree_children(node: &Node, prefix: &str, out: &mut String, opts: &Options) {
    let last = node.children.len().saturating_sub(1);
    for (i, child) in node.children.iter().enumerate() {
        let is_last = i == last;
        out.push_str(prefix);
        out.push_str(if is_last { "└── " } else { "├── " });
        out.push_str(&label(child, opts));
        out.push('\n');
        let next = format!("{prefix}{}", if is_last { "    " } else { "│   " });
        tree_children(child, &next, out, opts);
    }
}

fn render_mermaid(root: &Node, opts: &Options) -> String {
    let mut out = String::from("flowchart TD\n");
    let mut counter = 0usize;
    mermaid_node(root, None, &mut counter, &mut out, opts);
    out
}

fn mermaid_node(
    node: &Node,
    parent: Option<usize>,
    counter: &mut usize,
    out: &mut String,
    opts: &Options,
) {
    let id = *counter;
    *counter += 1;
    out.push_str(&format!(
        "    n{id}{}\n",
        mermaid_shape(node.kind, &mermaid_escape(&label(node, opts)))
    ));
    if let Some(p) = parent {
        out.push_str(&format!("    n{p} --> n{id}\n"));
    }
    for c in &node.children {
        mermaid_node(c, Some(id), counter, out, opts);
    }
}

/// A distinct Mermaid shape per item kind, so the graph is readable without a
/// legend or a colour scheme.
fn mermaid_shape(kind: Kind, label: &str) -> String {
    match kind {
        Kind::Crate => format!("[[\"{label}\"]]"),
        Kind::Mod => format!("[\"{label}\"]"),
        Kind::Struct | Kind::Enum | Kind::Union | Kind::TypeAlias => format!("(\"{label}\")"),
        Kind::Trait | Kind::TraitAlias => format!("{{{{\"{label}\"}}}}"),
        Kind::Fn => format!("([\"{label}\"])"),
        Kind::Impl => format!("[/\"{label}\"/]"),
        Kind::Const | Kind::Static => format!("[(\"{label}\")]"),
        Kind::Macro => format!(">\"{label}\"]"),
    }
}

/// Mermaid renders labels as HTML, so the characters that would be read as
/// markup become numeric entity codes.
fn mermaid_escape(s: &str) -> String {
    s.replace('&', "#38;")
        .replace('"', "#34;")
        .replace('<', "#60;")
        .replace('>', "#62;")
}

#[derive(Serialize)]
struct JsonNode {
    kind: &'static str,
    name: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    visibility: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    attributes: Vec<String>,
    #[serde(skip_serializing_if = "is_false")]
    external: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<JsonNode>,
}

fn is_false(b: &bool) -> bool {
    !*b
}

#[derive(Serialize, Default)]
struct Counts {
    modules: usize,
    structs: usize,
    enums: usize,
    unions: usize,
    traits: usize,
    trait_aliases: usize,
    type_aliases: usize,
    functions: usize,
    impls: usize,
    consts: usize,
    statics: usize,
    macros: usize,
}

#[derive(Serialize)]
struct JsonDoc {
    root: String,
    counts: Counts,
    tree: JsonNode,
}

fn render_json(root: &Node, opts: &Options) -> Result<String, String> {
    let mut counts = Counts::default();
    count(root, &mut counts);
    let doc = JsonDoc {
        root: root.path_name.clone(),
        counts,
        tree: to_json(root, opts),
    };
    serde_json::to_string_pretty(&doc).map_err(|e| format!("could not serialize JSON: {e}"))
}

fn to_json(node: &Node, opts: &Options) -> JsonNode {
    JsonNode {
        kind: node.kind.tag(),
        name: if node.kind == Kind::Crate && node.name.is_empty() {
            "crate".to_string()
        } else {
            node.name.clone()
        },
        visibility: if opts.show_visibility {
            node.vis.clone()
        } else {
            String::new()
        },
        attributes: node.attrs.clone(),
        external: node.unresolved,
        children: node.children.iter().map(|c| to_json(c, opts)).collect(),
    }
}

fn count(node: &Node, c: &mut Counts) {
    match node.kind {
        Kind::Crate => {}
        Kind::Mod => c.modules += 1,
        Kind::Struct => c.structs += 1,
        Kind::Enum => c.enums += 1,
        Kind::Union => c.unions += 1,
        Kind::Trait => c.traits += 1,
        Kind::TraitAlias => c.trait_aliases += 1,
        Kind::TypeAlias => c.type_aliases += 1,
        Kind::Fn => c.functions += 1,
        Kind::Impl => c.impls += 1,
        Kind::Const => c.consts += 1,
        Kind::Static => c.statics += 1,
        Kind::Macro => c.macros += 1,
    }
    for ch in &node.children {
        count(ch, c);
    }
}

fn render_paths(root: &Node, opts: &Options) -> String {
    let mut lines: Vec<String> = Vec::new();
    let base = root.path_name.clone();
    lines.push(paths_line(&base, root, opts));
    for c in &root.children {
        paths_walk(c, &base, &mut lines, opts);
    }
    let mut out = lines.join("\n");
    out.push('\n');
    out
}

fn paths_walk(node: &Node, prefix: &str, lines: &mut Vec<String>, opts: &Options) {
    let path = format!("{prefix}::{}", node.path_name);
    // An `impl` block has no path of its own — it lends its self type to the
    // associated items so they read as `crate::Type::method`.
    if node.kind != Kind::Impl {
        lines.push(paths_line(&path, node, opts));
    }
    for c in &node.children {
        paths_walk(c, &path, lines, opts);
    }
}

fn paths_line(path: &str, node: &Node, opts: &Options) -> String {
    let mut meta = node.kind.tag().to_string();
    if opts.show_visibility && !node.vis.is_empty() {
        meta.push_str(", ");
        meta.push_str(&node.vis);
    }
    if node.unresolved {
        meta.push_str(", external");
    }
    format!("{path}  ({meta})")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
pub mod config {
    pub struct Config { pub port: u16 }
    impl Config {
        pub fn new() -> Self { Config { port: 80 } }
    }
    pub(crate) fn load() -> Config { Config::new() }
}

pub trait Greet { fn hi(&self); }

fn main() {}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {}
}
"#;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn tree_happy_path() {
        let out = module_map(SAMPLE, &opts()).unwrap();
        assert_eq!(
            out,
            "crate\n\
             ├── mod config: pub\n\
             │   ├── struct Config: pub\n\
             │   ├── impl Config\n\
             │   │   └── fn new: pub\n\
             │   └── fn load: pub(crate)\n\
             ├── trait Greet: pub\n\
             └── fn main: pub(self)\n"
        );
    }

    #[test]
    fn rejects_invalid_rust() {
        let err = module_map("fn main( {", &opts()).unwrap_err();
        assert!(err.contains("could not parse"), "{err}");
        assert!(err.contains("line"), "{err}");
    }

    #[test]
    fn rejects_empty_source() {
        assert!(module_map("   \n", &opts()).unwrap_err().contains("empty"));
    }

    #[test]
    fn rejects_unknown_format() {
        let mut o = opts();
        o.format = "dot".into();
        assert!(module_map(SAMPLE, &o).unwrap_err().contains("format must be"));
    }

    #[test]
    fn rejects_oversized_source() {
        let big = format!("// {}\nfn a() {{}}\n", "x".repeat(MAX_SOURCE_BYTES));
        let err = module_map(&big, &opts()).unwrap_err();
        assert!(err.contains("the limit is"), "{err}");
    }

    #[test]
    fn tests_are_opt_in() {
        let mut o = opts();
        o.include_tests = true;
        let out = module_map(SAMPLE, &o).unwrap();
        assert!(out.contains("mod tests: pub(self) #[cfg(test)]"), "{out}");
        assert!(out.contains("fn it_works: pub(self) #[test]"), "{out}");
    }

    #[test]
    fn crate_name_labels_the_root() {
        let mut o = opts();
        o.crate_name = "my_crate".into();
        let out = module_map("fn main() {}", &o).unwrap();
        assert!(out.starts_with("crate my_crate\n"), "{out}");
    }

    #[test]
    fn visibility_can_be_hidden() {
        let mut o = opts();
        o.show_visibility = false;
        let out = module_map("pub fn a() {}", &o).unwrap();
        assert_eq!(out, "crate\n└── fn a\n");
    }

    #[test]
    fn filters_drop_kinds() {
        let src = "pub struct S; pub trait T {} pub fn f() {} pub const C: u8 = 1;";
        let mut o = opts();
        o.show_types = false;
        o.show_traits = false;
        let out = module_map(src, &o).unwrap();
        assert_eq!(out, "crate\n└── fn f: pub\n");

        let mut o = opts();
        o.show_fns = false;
        o.show_consts = true;
        let out = module_map(src, &o).unwrap();
        assert_eq!(
            out,
            "crate\n├── struct S: pub\n├── trait T: pub\n└── const C: pub\n"
        );
    }

    #[test]
    fn impls_carry_their_trait() {
        let src = "struct S; impl std::fmt::Display for S { fn fmt(&self) {} }";
        let out = module_map(src, &opts()).unwrap();
        assert!(out.contains("impl std::fmt::Display for S"), "{out}");
        assert!(out.contains("└── fn fmt: pub(self)"), "{out}");
    }

    #[test]
    fn nested_modules_nest() {
        let src = "mod a { mod b { mod c { pub fn deep() {} } } }";
        let out = module_map(src, &opts()).unwrap();
        assert_eq!(
            out,
            "crate\n\
             └── mod a: pub(self)\n    \
             └── mod b: pub(self)\n        \
             └── mod c: pub(self)\n            \
             └── fn deep: pub\n"
        );
    }

    #[test]
    fn max_depth_truncates() {
        let src = "mod a { mod b { pub fn deep() {} } }";
        let mut o = opts();
        o.max_depth = 2;
        let out = module_map(src, &o).unwrap();
        assert_eq!(out, "crate\n└── mod a: pub(self)\n    └── mod b: pub(self)\n");
    }

    #[test]
    fn unresolved_mod_declaration_is_marked() {
        let out = module_map("pub mod util;", &opts()).unwrap();
        assert_eq!(out, "crate\n└── mod util: pub (external)\n");
    }

    #[test]
    fn multi_file_paste_resolves_mod_declarations() {
        let src = "=== src/lib.rs ===\npub mod util;\n\n=== src/util.rs ===\npub fn slug() {}\n";
        let out = module_map(src, &opts()).unwrap();
        assert_eq!(out, "crate\n└── mod util: pub\n    └── fn slug: pub\n");
    }

    #[test]
    fn multi_file_nested_paths() {
        let src = "=== src/lib.rs ===\nmod a;\n=== src/a/mod.rs ===\npub mod b;\n=== src/a/b.rs ===\npub struct Deep;\n";
        let out = module_map(src, &opts()).unwrap();
        assert_eq!(
            out,
            "crate\n└── mod a: pub(self)\n    └── mod b: pub\n        └── struct Deep: pub\n"
        );
    }

    #[test]
    fn focus_on_selects_a_subtree() {
        let mut o = opts();
        o.focus_on = "crate::config".into();
        let out = module_map(SAMPLE, &o).unwrap();
        assert!(out.starts_with("mod crate::config: pub\n"), "{out}");
        assert!(!out.contains("fn main"), "{out}");
    }

    #[test]
    fn focus_on_unknown_module_errors() {
        let mut o = opts();
        o.focus_on = "crate::nope".into();
        assert!(module_map(SAMPLE, &o).unwrap_err().contains("focus_on"));
    }

    #[test]
    fn sort_by_name_orders_alphabetically() {
        let src = "fn zeta() {} fn alpha() {} fn mid() {}";
        let mut o = opts();
        o.sort_by = "name".into();
        let out = module_map(src, &o).unwrap();
        assert_eq!(
            out,
            "crate\n├── fn alpha: pub(self)\n├── fn mid: pub(self)\n└── fn zeta: pub(self)\n"
        );
    }

    #[test]
    fn sort_by_visibility_puts_public_first() {
        let src = "fn hidden() {} pub fn open() {} pub(crate) fn shared() {}";
        let mut o = opts();
        o.sort_by = "visibility".into();
        let out = module_map(src, &o).unwrap();
        assert_eq!(
            out,
            "crate\n├── fn open: pub\n├── fn shared: pub(crate)\n└── fn hidden: pub(self)\n"
        );
    }

    #[test]
    fn sort_by_kind_groups_kinds() {
        let src = "pub fn f() {} pub struct S; pub mod m {}";
        let mut o = opts();
        o.sort_by = "kind".into();
        let out = module_map(src, &o).unwrap();
        assert_eq!(
            out,
            "crate\n├── mod m: pub\n├── struct S: pub\n└── fn f: pub\n"
        );
    }

    #[test]
    fn rejects_unknown_sort() {
        let mut o = opts();
        o.sort_by = "size".into();
        assert!(module_map(SAMPLE, &o).unwrap_err().contains("sort_by must be"));
    }

    #[test]
    fn mermaid_is_a_flowchart_with_shapes() {
        let mut o = opts();
        o.format = "mermaid".into();
        let out = module_map("pub mod a { pub struct S; }", &o).unwrap();
        assert_eq!(
            out,
            "flowchart TD\n    \
             n0[[\"crate\"]]\n    \
             n1[\"mod a: pub\"]\n    \
             n0 --> n1\n    \
             n2(\"struct S: pub\")\n    \
             n1 --> n2\n"
        );
    }

    #[test]
    fn mermaid_escapes_generics_in_labels() {
        let mut o = opts();
        o.format = "mermaid".into();
        let out = module_map("struct S<T>(T); impl<T> S<T> { fn n() {} }", &o).unwrap();
        assert!(out.contains("#60;T#62;"), "{out}");
        assert!(!out.contains("<T>"), "{out}");
    }

    #[test]
    fn json_carries_counts_and_tree() {
        let mut o = opts();
        o.format = "json".into();
        let out = module_map(SAMPLE, &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["root"], "crate");
        assert_eq!(v["counts"]["modules"], 1);
        assert_eq!(v["counts"]["structs"], 1);
        assert_eq!(v["counts"]["impls"], 1);
        assert_eq!(v["tree"]["children"][0]["name"], "config");
        assert_eq!(v["tree"]["children"][0]["visibility"], "pub");
    }

    #[test]
    fn paths_flattens_with_impl_self_types() {
        let mut o = opts();
        o.format = "paths".into();
        let out = module_map(SAMPLE, &o).unwrap();
        assert_eq!(
            out,
            "crate  (crate)\n\
             crate::config  (mod, pub)\n\
             crate::config::Config  (struct, pub)\n\
             crate::config::Config::new  (fn, pub)\n\
             crate::config::load  (fn, pub(crate))\n\
             crate::Greet  (trait, pub)\n\
             crate::main  (fn, pub(self))\n"
        );
    }

    #[test]
    fn restricted_visibility_is_spelled_out() {
        let src = "mod a { pub(in crate::a) fn f() {} pub(super) fn g() {} }";
        let out = module_map(src, &opts()).unwrap();
        assert!(out.contains("fn f: pub(in crate::a)"), "{out}");
        assert!(out.contains("fn g: pub(super)"), "{out}");
    }

    #[test]
    fn macro_rules_definitions_show_up() {
        let out = module_map("macro_rules! shout { () => {} }", &opts()).unwrap();
        assert_eq!(out, "crate\n└── macro_rules! shout\n");
    }

    #[test]
    fn cfg_attributes_are_shown() {
        let src = "#[cfg(feature = \"extra\")]\npub fn extra() {}";
        let out = module_map(src, &opts()).unwrap();
        assert_eq!(
            out,
            "crate\n└── fn extra: pub #[cfg(feature = \"extra\")]\n"
        );
    }

    #[test]
    fn tidy_squeezes_token_spacing() {
        assert_eq!(tidy("Vec < T >"), "Vec<T>");
        assert_eq!(tidy("std :: fmt :: Display"), "std::fmt::Display");
        assert_eq!(tidy("HashMap < String , u8 >"), "HashMap<String, u8>");
        assert_eq!(tidy("& 'a str"), "&'a str");
        assert_eq!(tidy("feature = \"x\""), "feature = \"x\"");
    }
}
