//! import-graph-extractor core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Paste one or more source files (delimited by `=== path ===` / `--- path ---`
//! header lines) and get the import/require/use dependency graph: file↔file edges,
//! external dependencies, dependents ("who imports X"), orphans, leaves, and
//! circular dependencies — rendered as a text report, JSON, Graphviz DOT, or Mermaid.
//!
//! Resolution is strong for JavaScript/TypeScript (relative paths incl. `index`
//! files) and Python (dotted-module ↔ file mapping, incl. relative imports). Rust
//! resolves `mod` declarations to sibling files and classifies external crates; Go
//! extracts and classifies `import` specifiers without file↔file resolution. These
//! limits are documented on the tool page.

use std::collections::{BTreeMap, BTreeSet};

/// A source language we can extract imports for.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Lang {
    Js,
    Py,
    Rust,
    Go,
}

impl Lang {
    fn name(self) -> &'static str {
        match self {
            Lang::Js => "javascript",
            Lang::Py => "python",
            Lang::Rust => "rust",
            Lang::Go => "go",
        }
    }
    fn ext(self) -> &'static str {
        match self {
            Lang::Js => "js",
            Lang::Py => "py",
            Lang::Rust => "rs",
            Lang::Go => "go",
        }
    }
    fn from_flag(s: &str) -> Option<Lang> {
        match s.trim().to_ascii_lowercase().as_str() {
            "javascript" | "js" | "typescript" | "ts" => Some(Lang::Js),
            "python" | "py" => Some(Lang::Py),
            "rust" | "rs" => Some(Lang::Rust),
            "go" | "golang" => Some(Lang::Go),
            _ => None,
        }
    }
    fn from_ext(path: &str) -> Option<Lang> {
        let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
        match ext.as_str() {
            "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" => Some(Lang::Js),
            "py" | "pyi" => Some(Lang::Py),
            "rs" => Some(Lang::Rust),
            "go" => Some(Lang::Go),
            _ => None,
        }
    }
}

/// One pasted source file after splitting on header lines.
struct SrcFile {
    path: String,
    lang: Lang,
    source: String,
}

/// Classification of an external (non file-to-file) dependency.
struct ExternalInfo {
    /// "stdlib" (node/go/rust builtin) or "package" (third-party).
    kind: &'static str,
    used_by: BTreeSet<String>,
}

/// The resolved dependency graph.
struct Graph {
    files: Vec<(String, Lang)>,
    /// Internal file → file edges (both endpoints are pasted files).
    edges: BTreeSet<(String, String)>,
    /// External dependency name → info.
    external: BTreeMap<String, ExternalInfo>,
    /// (importing file, raw specifier) that looks internal but has no matching
    /// pasted file, or a Rust `crate::`/`super::`/`self::` path we don't resolve.
    unresolved: BTreeSet<(String, String)>,
}

const MAX_INPUT_BYTES: usize = 2_000_000;

/// Extract the import graph from pasted source and render it in `format`.
///
/// - `language`: `auto` (per-file from the header extension, or a single-file
///   content sniff) or one of `javascript`/`python`/`rust`/`go` to force it.
/// - `format`: `text` (default), `json`, `dot` (Graphviz), or `mermaid`.
/// - `include_external`: include third-party / stdlib dependencies in the report
///   and graph output.
/// - `detect_cycles`: compute and report circular dependencies.
pub fn run(
    input: &str,
    language: &str,
    format: &str,
    include_external: bool,
    detect_cycles: bool,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("expected source code to analyze, got empty input".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input too large: {} bytes (max {} bytes / ~2 MB) — paste fewer files",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }
    let lang_flag = language.trim();
    let forced = if lang_flag.is_empty() || lang_flag.eq_ignore_ascii_case("auto") {
        None
    } else {
        Some(Lang::from_flag(lang_flag).ok_or_else(|| {
            format!(
                "unknown language '{}' — expected one of: auto, javascript, python, rust, go",
                lang_flag
            )
        })?)
    };

    let fmt = format.trim().to_ascii_lowercase();
    let fmt = if fmt.is_empty() { "text" } else { fmt.as_str() };
    if !matches!(fmt, "text" | "json" | "dot" | "mermaid") {
        return Err(format!(
            "unknown format '{}' — expected one of: text, json, dot, mermaid",
            fmt
        ));
    }

    let files = split_files(input, forced)?;
    let graph = build_graph(&files, include_external);

    let cycles = if detect_cycles {
        Some(find_cycles(&graph))
    } else {
        None
    };

    Ok(match fmt {
        "json" => render_json(&graph, include_external, cycles.as_deref()),
        "dot" => render_dot(&graph, include_external, cycles.as_deref()),
        "mermaid" => render_mermaid(&graph, include_external, cycles.as_deref()),
        _ => render_text(&graph, include_external, cycles.as_deref()),
    })
}

// ---------------------------------------------------------------------------
// File splitting + language detection
// ---------------------------------------------------------------------------

/// A header line is `=== path ===` or `--- path ---` (any run of ≥3 of the marker
/// on each side). Returns the trimmed path if `line` is a header.
fn header_path(line: &str) -> Option<String> {
    let t = line.trim();
    for m in ['=', '-'] {
        let mut chars = t.chars();
        let lead: String = chars.by_ref().take_while(|&c| c == m).collect();
        if lead.len() < 3 {
            continue;
        }
        // strip the leading run already consumed, then a trailing run of the same char
        let rest = &t[lead.len()..];
        let trailing_len = rest.chars().rev().take_while(|&c| c == m).count();
        if trailing_len < 3 {
            continue;
        }
        let inner = rest[..rest.len() - trailing_len].trim();
        if !inner.is_empty() && !inner.contains(char::is_whitespace) {
            return Some(inner.trim_start_matches("./").to_string());
        }
    }
    None
}

fn split_files(input: &str, forced: Option<Lang>) -> Result<Vec<SrcFile>, String> {
    // Collect (path, body) sections split on header lines.
    let mut sections: Vec<(Option<String>, String)> = Vec::new();
    let mut current_path: Option<String> = None;
    let mut buf = String::new();
    let mut saw_header = false;
    for line in input.lines() {
        if let Some(p) = header_path(line) {
            if saw_header || !buf.trim().is_empty() {
                sections.push((current_path.take(), std::mem::take(&mut buf)));
            }
            current_path = Some(p);
            saw_header = true;
        } else {
            buf.push_str(line);
            buf.push('\n');
        }
    }
    if !buf.trim().is_empty() || current_path.is_some() {
        sections.push((current_path, buf));
    }

    let mut out = Vec::new();
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for (idx, (path, body)) in sections.into_iter().enumerate() {
        if body.trim().is_empty() {
            continue;
        }
        let (path, lang) = match path {
            Some(p) => {
                let lang = forced
                    .or_else(|| Lang::from_ext(&p))
                    .or_else(|| sniff_lang(&body));
                match lang {
                    Some(l) => (p, l),
                    None => {
                        return Err(format!(
                            "cannot determine language for '{}' — add a known extension \
                             (.js/.ts/.py/.rs/.go) to the header path or set the language parameter",
                            p
                        ))
                    }
                }
            }
            None => {
                let lang = match forced.or_else(|| sniff_lang(&body)) {
                    Some(l) => l,
                    None => return Err(
                        "could not auto-detect the language — set the language parameter to \
                         javascript, python, rust, or go, or prefix each file with a \
                         `=== path/to/file.ext ===` header"
                            .into(),
                    ),
                };
                // No header: infer a stable filename from the language.
                let name = if idx == 0 {
                    format!("input.{}", lang.ext())
                } else {
                    format!("input-{}.{}", idx + 1, lang.ext())
                };
                (name, lang)
            }
        };
        // De-duplicate identical header paths so the graph nodes stay unique.
        let mut path = path;
        if !seen.insert(path.clone()) {
            let mut n = 2;
            while !seen.insert(format!("{}#{}", path, n)) {
                n += 1;
            }
            path = format!("{}#{}", path, n);
        }
        out.push(SrcFile {
            path,
            lang,
            source: body,
        });
    }
    if out.is_empty() {
        return Err("no source files found in the input".into());
    }
    Ok(out)
}

/// Best-effort content sniff used only when the language is `auto` and the file
/// has no header extension to key off. Deliberately conservative.
fn sniff_lang(src: &str) -> Option<Lang> {
    let mut go = 0i32;
    let mut rust = 0i32;
    let mut py = 0i32;
    let mut js = 0i32;
    for raw in src.lines() {
        let l = raw.trim();
        if l.starts_with("package ") || l.starts_with("func ") || l.contains(":=") {
            go += 2;
        }
        if l.starts_with("use ")
            || l.starts_with("fn ")
            || l.starts_with("pub fn ")
            || l.starts_with("mod ")
            || l.contains("-> ")
            || l.contains("::")
        {
            rust += 2;
        }
        if l.starts_with("def ")
            || l.starts_with("class ")
            || (l.starts_with("from ") && l.contains(" import "))
            || (l.starts_with("import ") && !l.contains(';') && !l.contains('{'))
        {
            py += 2;
        }
        if l.contains("=>")
            || l.contains("require(")
            || l.contains("console.")
            || l.starts_with("const ")
            || l.starts_with("let ")
            || l.starts_with("export ")
            || (l.starts_with("import ") && (l.contains(" from ") || l.contains('{')))
            || l.contains("function ")
        {
            js += 2;
        }
    }
    let best = [(Lang::Js, js), (Lang::Py, py), (Lang::Rust, rust), (Lang::Go, go)]
        .into_iter()
        .max_by_key(|&(_, s)| s)?;
    if best.1 == 0 {
        None
    } else {
        Some(best.0)
    }
}

// ---------------------------------------------------------------------------
// Graph construction
// ---------------------------------------------------------------------------

fn build_graph(files: &[SrcFile], include_external: bool) -> Graph {
    let paths: BTreeSet<String> = files.iter().map(|f| f.path.clone()).collect();

    // Python module map: dotted module name → file path.
    let py_modules = python_module_map(files);

    let mut graph = Graph {
        files: files.iter().map(|f| (f.path.clone(), f.lang)).collect(),
        edges: BTreeSet::new(),
        external: BTreeMap::new(),
        unresolved: BTreeSet::new(),
    };
    graph.files.sort();

    for f in files {
        match f.lang {
            Lang::Js => resolve_js(f, &paths, include_external, &mut graph),
            Lang::Py => resolve_py(f, &py_modules, include_external, &mut graph),
            Lang::Rust => resolve_rust(f, &paths, include_external, &mut graph),
            Lang::Go => resolve_go(f, include_external, &mut graph),
        }
    }
    graph
}

fn add_external(graph: &mut Graph, name: &str, kind: &'static str, from: &str) {
    let e = graph
        .external
        .entry(name.to_string())
        .or_insert_with(|| ExternalInfo {
            kind,
            used_by: BTreeSet::new(),
        });
    e.used_by.insert(from.to_string());
}

// ---------------------------------------------------------------------------
// JavaScript / TypeScript
// ---------------------------------------------------------------------------

const JS_EXTS: &[&str] = &["js", "ts", "jsx", "tsx", "mjs", "cjs", "mts", "cts"];
const NODE_BUILTINS: &[&str] = &[
    "assert", "buffer", "child_process", "cluster", "console", "crypto", "dgram", "dns",
    "events", "fs", "http", "http2", "https", "net", "os", "path", "process", "punycode",
    "querystring", "readline", "repl", "stream", "string_decoder", "timers", "tls", "tty",
    "url", "util", "v8", "vm", "worker_threads", "zlib",
];

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

fn extract_js_specs(src: &str) -> Vec<String> {
    let mut specs = Vec::new();
    for line in src.lines() {
        let stripped = strip_line_comment(line);
        // require(...) and dynamic import(...)
        for pat in ["require(", "import("] {
            let mut start = 0;
            while let Some(p) = stripped[start..].find(pat) {
                let after = &stripped[start + p + pat.len()..];
                if let Some(q) = first_quoted(after) {
                    specs.push(q);
                }
                start += p + pat.len();
            }
        }
        let t = stripped.trim_start();
        if t.starts_with("import") || t.starts_with("export") {
            if let Some(q) = from_module(&stripped) {
                specs.push(q);
            } else if t.starts_with("import") {
                // side-effect import '...';
                if let Some(q) = first_quoted(&t["import".len()..]) {
                    specs.push(q);
                }
            }
        }
    }
    specs.sort();
    specs.dedup();
    specs
}

/// Find the module string after a standalone `from` keyword (avoids matching the
/// `from` inside identifiers like `transform`).
fn from_module(line: &str) -> Option<String> {
    let bytes = line.as_bytes();
    let mut i = 0;
    while let Some(rel) = line[i..].find("from") {
        let pos = i + rel;
        let before_ok = pos == 0 || !is_ident_byte(bytes[pos - 1]);
        let after = pos + 4;
        let after_ok = after >= bytes.len() || !is_ident_byte(bytes[after]);
        if before_ok && after_ok {
            if let Some(q) = first_quoted(&line[after..]) {
                return Some(q);
            }
        }
        i = pos + 4;
    }
    None
}

fn resolve_js(f: &SrcFile, paths: &BTreeSet<String>, include_external: bool, graph: &mut Graph) {
    let dir = parent_dir(&f.path);
    for spec in extract_js_specs(&f.source) {
        if spec.starts_with('.') {
            // relative → resolve against the importing file's directory
            let base = join_path(dir, &spec);
            if let Some(target) = resolve_js_file(&base, paths) {
                if target != f.path {
                    graph.edges.insert((f.path.clone(), target));
                }
            } else {
                graph.unresolved.insert((f.path.clone(), spec));
            }
        } else if include_external {
            let name = js_package_name(&spec);
            let kind = if spec.starts_with("node:") || NODE_BUILTINS.contains(&name.as_str()) {
                "stdlib"
            } else {
                "package"
            };
            add_external(graph, &name, kind, &f.path);
        }
    }
}

/// Given a normalized base path with no extension, find the matching pasted file.
fn resolve_js_file(base: &str, paths: &BTreeSet<String>) -> Option<String> {
    if paths.contains(base) {
        return Some(base.to_string());
    }
    for ext in JS_EXTS {
        let cand = format!("{}.{}", base, ext);
        if paths.contains(&cand) {
            return Some(cand);
        }
    }
    for ext in JS_EXTS {
        let cand = if base.is_empty() {
            format!("index.{}", ext)
        } else {
            format!("{}/index.{}", base, ext)
        };
        if paths.contains(&cand) {
            return Some(cand);
        }
    }
    None
}

fn js_package_name(spec: &str) -> String {
    let spec = spec.strip_prefix("node:").unwrap_or(spec);
    if let Some(rest) = spec.strip_prefix('@') {
        // scoped: @scope/pkg
        let mut it = rest.splitn(3, '/');
        let scope = it.next().unwrap_or("");
        let pkg = it.next().unwrap_or("");
        if pkg.is_empty() {
            format!("@{}", scope)
        } else {
            format!("@{}/{}", scope, pkg)
        }
    } else {
        spec.split('/').next().unwrap_or(spec).to_string()
    }
}

// ---------------------------------------------------------------------------
// Python
// ---------------------------------------------------------------------------

/// Map every pasted Python file to the dotted module name(s) it provides.
fn python_module_map(files: &[SrcFile]) -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();
    for f in files {
        if f.lang != Lang::Py {
            continue;
        }
        let p = f.path.trim_start_matches("./");
        let stem = p.trim_end_matches(".pyi").trim_end_matches(".py");
        let module = if stem.ends_with("__init__") {
            // package: drop the trailing /__init__
            let dir = stem.trim_end_matches("__init__").trim_end_matches('/');
            dir.replace('/', ".")
        } else {
            stem.replace('/', ".")
        };
        if !module.is_empty() {
            map.insert(module, f.path.clone());
        }
    }
    map
}

struct PyImport {
    level: u32,
    /// module after the dots (may be empty for `from . import x`)
    module: String,
    names: Vec<String>,
    is_from: bool,
}

fn extract_py_imports(src: &str) -> Vec<PyImport> {
    let mut out = Vec::new();
    for line in py_logical_lines(src) {
        let l = strip_hash_comment(&line);
        let t = l.trim();
        if let Some(rest) = t.strip_prefix("from ") {
            // rest = "<dots><module> import <names>"
            let (mod_part, names_part) = match rest.split_once(" import ") {
                Some(x) => x,
                None => continue,
            };
            let mp = mod_part.trim();
            let level = mp.chars().take_while(|&c| c == '.').count() as u32;
            let module = mp.trim_start_matches('.').trim().to_string();
            let names = parse_py_names(names_part);
            out.push(PyImport {
                level,
                module,
                names,
                is_from: true,
            });
        } else if let Some(rest) = t.strip_prefix("import ") {
            // import a, b.c as d
            for part in rest.split(',') {
                let name = part.trim().split_whitespace().next().unwrap_or("").to_string();
                if !name.is_empty() {
                    out.push(PyImport {
                        level: 0,
                        module: name,
                        names: Vec::new(),
                        is_from: false,
                    });
                }
            }
        }
    }
    out
}

fn parse_py_names(s: &str) -> Vec<String> {
    let s = s.trim().trim_start_matches('(').trim_end_matches(')');
    if s.trim() == "*" {
        return Vec::new();
    }
    s.split(',')
        .filter_map(|p| {
            let n = p.trim().split_whitespace().next().unwrap_or("");
            if n.is_empty() {
                None
            } else {
                Some(n.to_string())
            }
        })
        .collect()
}

/// Join Python physical lines into logical lines (handles `( … )` and trailing
/// backslash continuations) so multi-line imports parse as one statement.
fn py_logical_lines(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut depth: i32 = 0;
    let mut cont = false;
    for raw in src.lines() {
        if depth > 0 || cont {
            buf.push(' ');
            buf.push_str(raw.trim());
        } else {
            buf = raw.to_string();
        }
        for c in raw.chars() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    if depth > 0 {
                        depth -= 1
                    }
                }
                _ => {}
            }
        }
        cont = raw.trim_end().ends_with('\\');
        if cont {
            let l = buf.trim_end().to_string();
            buf = l.trim_end_matches('\\').to_string();
        }
        if depth == 0 && !cont {
            out.push(std::mem::take(&mut buf));
        }
    }
    if !buf.is_empty() {
        out.push(buf);
    }
    out
}

fn resolve_py(
    f: &SrcFile,
    modules: &BTreeMap<String, String>,
    include_external: bool,
    graph: &mut Graph,
) {
    let self_pkg = python_self_package(&f.path);
    for imp in extract_py_imports(&f.source) {
        if imp.level > 0 {
            // relative import: resolve against the file's package, walking up (level-1) times
            let mut base: Vec<&str> = if self_pkg.is_empty() {
                Vec::new()
            } else {
                self_pkg.split('.').collect()
            };
            for _ in 0..(imp.level.saturating_sub(1)) {
                base.pop();
            }
            let base_mod = base.join(".");
            let targets: Vec<String> = if imp.module.is_empty() {
                // from . import a, b  → base.a, base.b
                imp.names
                    .iter()
                    .map(|n| join_module(&base_mod, n))
                    .collect()
            } else {
                let m = join_module(&base_mod, &imp.module);
                let mut v = vec![m.clone()];
                // also try each imported name as a submodule of m
                for n in &imp.names {
                    v.push(join_module(&m, n));
                }
                v
            };
            let mut hit = false;
            for tmod in &targets {
                if let Some(path) = modules.get(tmod) {
                    if path != &f.path {
                        graph.edges.insert((f.path.clone(), path.clone()));
                    }
                    hit = true;
                }
            }
            if !hit {
                let dots = ".".repeat(imp.level as usize);
                let shown = format!("{}{}", dots, imp.module);
                graph.unresolved.insert((f.path.clone(), shown));
            }
        } else {
            // absolute import
            let mut hit = false;
            if let Some(path) = modules.get(&imp.module) {
                if path != &f.path {
                    graph.edges.insert((f.path.clone(), path.clone()));
                }
                hit = true;
            }
            if imp.is_from {
                // from pkg import submod → pkg.submod may be a pasted file
                for n in &imp.names {
                    let sub = join_module(&imp.module, n);
                    if let Some(path) = modules.get(&sub) {
                        if path != &f.path {
                            graph.edges.insert((f.path.clone(), path.clone()));
                        }
                        hit = true;
                    }
                }
            }
            if !hit && include_external {
                let top = imp.module.split('.').next().unwrap_or(&imp.module);
                if !top.is_empty() {
                    add_external(graph, top, "package", &f.path);
                }
            }
        }
    }
}

/// The dotted package that contains file `path` (its directory as a module path).
fn python_self_package(path: &str) -> String {
    let p = path.trim_start_matches("./");
    match p.rsplit_once('/') {
        Some((dir, _)) => dir.replace('/', "."),
        None => String::new(),
    }
}

fn join_module(base: &str, name: &str) -> String {
    if base.is_empty() {
        name.to_string()
    } else if name.is_empty() {
        base.to_string()
    } else {
        format!("{}.{}", base, name)
    }
}

// ---------------------------------------------------------------------------
// Rust
// ---------------------------------------------------------------------------

const RUST_STDLIB: &[&str] = &["std", "core", "alloc", "proc_macro", "test"];

fn resolve_rust(f: &SrcFile, paths: &BTreeSet<String>, include_external: bool, graph: &mut Graph) {
    let dir = parent_dir(&f.path);
    let stem = file_stem(&f.path);
    for line in f.source.lines() {
        let t = strip_line_comment(line);
        let t = t.trim();
        // `mod name;` (file module) — skip inline `mod name { … }`
        if let Some(rest) = t.strip_prefix("mod ") {
            let name = rest.trim_end_matches(';').trim();
            if !name.is_empty() && !rest.contains('{') && is_ident(name) {
                if let Some(target) = resolve_rust_mod(dir, &stem, name, paths) {
                    if target != f.path {
                        graph.edges.insert((f.path.clone(), target));
                    }
                } else {
                    graph
                        .unresolved
                        .insert((f.path.clone(), format!("mod {}", name)));
                }
            }
            continue;
        }
        if t.starts_with("pub mod ") {
            let name = t["pub mod ".len()..].trim_end_matches(';').trim();
            if !name.is_empty() && !t.contains('{') && is_ident(name) {
                if let Some(target) = resolve_rust_mod(dir, &stem, name, paths) {
                    if target != f.path {
                        graph.edges.insert((f.path.clone(), target));
                    }
                } else {
                    graph
                        .unresolved
                        .insert((f.path.clone(), format!("mod {}", name)));
                }
            }
            continue;
        }
        // `extern crate name;`
        if let Some(rest) = t.strip_prefix("extern crate ") {
            let name = rest.trim_end_matches(';').split(" as ").next().unwrap_or("").trim();
            if include_external && !name.is_empty() {
                add_external(graph, name, "package", &f.path);
            }
            continue;
        }
        // `use seg::…;` / `pub use seg::…;`
        let use_body = t
            .strip_prefix("use ")
            .or_else(|| t.strip_prefix("pub use "))
            .or_else(|| {
                t.strip_prefix("pub(crate) use ")
                    .or_else(|| t.strip_prefix("pub(super) use "))
            });
        if let Some(body) = use_body {
            let first = body
                .trim_start_matches("::")
                .split(&[':', '{', ' ', ';'][..])
                .next()
                .unwrap_or("")
                .trim();
            if first.is_empty() {
                continue;
            }
            match first {
                "crate" | "super" | "self" => {
                    graph
                        .unresolved
                        .insert((f.path.clone(), format!("use {}::…", first)));
                }
                _ if RUST_STDLIB.contains(&first) => {
                    if include_external {
                        add_external(graph, first, "stdlib", &f.path);
                    }
                }
                _ => {
                    if include_external && is_ident(first) {
                        add_external(graph, first, "package", &f.path);
                    }
                }
            }
        }
    }
}

/// Resolve `mod name;` in a Rust file to a sibling module file.
fn resolve_rust_mod(dir: &str, stem: &str, name: &str, paths: &BTreeSet<String>) -> Option<String> {
    let mut cands = Vec::new();
    let base_dir = if dir.is_empty() {
        String::new()
    } else {
        format!("{}/", dir)
    };
    // src/name.rs, src/name/mod.rs
    cands.push(format!("{}{}.rs", base_dir, name));
    cands.push(format!("{}{}/mod.rs", base_dir, name));
    // src/<stem>/name.rs, src/<stem>/name/mod.rs (2018 nesting under the declaring file)
    if stem != "lib" && stem != "main" && stem != "mod" {
        cands.push(format!("{}{}/{}.rs", base_dir, stem, name));
        cands.push(format!("{}{}/{}/mod.rs", base_dir, stem, name));
    }
    cands.into_iter().find(|c| paths.contains(c))
}

fn is_ident(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !s.chars().next().unwrap().is_ascii_digit()
}

// ---------------------------------------------------------------------------
// Go
// ---------------------------------------------------------------------------

fn resolve_go(f: &SrcFile, include_external: bool, graph: &mut Graph) {
    if !include_external {
        return;
    }
    let mut in_block = false;
    for line in f.source.lines() {
        let t = strip_line_comment(line).trim().to_string();
        if in_block {
            if t.starts_with(')') {
                in_block = false;
                continue;
            }
            if let Some(spec) = first_quoted(&t) {
                add_go_dep(graph, &spec, &f.path);
            }
            continue;
        }
        if t == "import (" || t.starts_with("import (") {
            in_block = true;
            // a spec may sit on the same line after `import (`
            if let Some(spec) = first_quoted(t.trim_start_matches("import (")) {
                add_go_dep(graph, &spec, &f.path);
            }
            continue;
        }
        if let Some(rest) = t.strip_prefix("import ") {
            if let Some(spec) = first_quoted(rest) {
                add_go_dep(graph, &spec, &f.path);
            }
        }
    }
}

fn add_go_dep(graph: &mut Graph, spec: &str, from: &str) {
    if spec.is_empty() {
        return;
    }
    let first = spec.split('/').next().unwrap_or(spec);
    // A dot in the first path segment (github.com, golang.org) ⇒ third-party.
    let kind = if first.contains('.') { "package" } else { "stdlib" };
    add_external(graph, spec, kind, from);
}

// ---------------------------------------------------------------------------
// Shared parsing helpers
// ---------------------------------------------------------------------------

/// Extract the contents of the first single/double/back-quoted string in `s`.
fn first_quoted(s: &str) -> Option<String> {
    let mut iter = s.char_indices();
    while let Some((_, c)) = iter.next() {
        if c == '\'' || c == '"' || c == '`' {
            let close = c;
            let mut buf = String::new();
            while let Some((_, d)) = iter.next() {
                if d == '\\' {
                    if let Some((_, e)) = iter.next() {
                        buf.push(e);
                    }
                    continue;
                }
                if d == close {
                    return Some(buf);
                }
                buf.push(d);
            }
            return None;
        }
    }
    None
}

/// Strip a `//` line comment (not inside a string). Conservative: stops at the
/// first `//` that isn't preceded by a `:` (to spare `https://`).
fn strip_line_comment(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut i = 0;
    let mut in_str: Option<u8> = None;
    while i + 1 < bytes.len() {
        let b = bytes[i];
        match in_str {
            Some(q) => {
                if b == b'\\' {
                    i += 2;
                    continue;
                }
                if b == q {
                    in_str = None;
                }
            }
            None => {
                if b == b'"' || b == b'\'' || b == b'`' {
                    in_str = Some(b);
                } else if b == b'/' && bytes[i + 1] == b'/' {
                    return line[..i].to_string();
                }
            }
        }
        i += 1;
    }
    line.to_string()
}

fn strip_hash_comment(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut in_str: Option<u8> = None;
    for (i, &b) in bytes.iter().enumerate() {
        match in_str {
            Some(q) => {
                if b == q {
                    in_str = None;
                }
            }
            None => {
                if b == b'"' || b == b'\'' {
                    in_str = Some(b);
                } else if b == b'#' {
                    return line[..i].to_string();
                }
            }
        }
    }
    line.to_string()
}

fn parent_dir(path: &str) -> &str {
    match path.rsplit_once('/') {
        Some((dir, _)) => dir,
        None => "",
    }
}

fn file_stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, _)) => stem.to_string(),
        None => name.to_string(),
    }
}

/// Join a directory with a relative spec and normalize `.`/`..` segments.
fn join_path(dir: &str, spec: &str) -> String {
    let combined = if dir.is_empty() {
        spec.to_string()
    } else {
        format!("{}/{}", dir, spec)
    };
    normalize_path(&combined)
}

fn normalize_path(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            p => out.push(p),
        }
    }
    out.join("/")
}

// ---------------------------------------------------------------------------
// Derived views: dependents, orphans, leaves, cycles
// ---------------------------------------------------------------------------

fn dependents_map(graph: &Graph) -> BTreeMap<String, BTreeSet<String>> {
    let mut m: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (from, to) in &graph.edges {
        m.entry(to.clone()).or_default().insert(from.clone());
    }
    m
}

fn orphans(graph: &Graph) -> Vec<String> {
    let imported: BTreeSet<&String> = graph.edges.iter().map(|(_, to)| to).collect();
    graph
        .files
        .iter()
        .map(|(p, _)| p)
        .filter(|p| !imported.contains(p))
        .cloned()
        .collect()
}

fn leaves(graph: &Graph) -> Vec<String> {
    let importers: BTreeSet<&String> = graph.edges.iter().map(|(from, _)| from).collect();
    graph
        .files
        .iter()
        .map(|(p, _)| p)
        .filter(|p| !importers.contains(p))
        .cloned()
        .collect()
}

/// Find circular dependencies via Tarjan strongly-connected components. Each SCC
/// with ≥2 files (or a self-import) is a cycle; the members are rendered as an
/// ordered loop.
fn find_cycles(graph: &Graph) -> Vec<Vec<String>> {
    // adjacency over internal files only
    let nodes: Vec<String> = graph.files.iter().map(|(p, _)| p.clone()).collect();
    let index_of: BTreeMap<&String, usize> =
        nodes.iter().enumerate().map(|(i, n)| (n, i)).collect();
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); nodes.len()];
    let mut self_loop = vec![false; nodes.len()];
    for (from, to) in &graph.edges {
        let (fi, ti) = (index_of[from], index_of[to]);
        if fi == ti {
            self_loop[fi] = true;
        } else {
            adj[fi].push(ti);
        }
    }
    for v in adj.iter_mut() {
        v.sort_unstable();
        v.dedup();
    }

    // Iterative Tarjan.
    let n = nodes.len();
    let mut idx = vec![usize::MAX; n];
    let mut low = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack: Vec<usize> = Vec::new();
    let mut counter = 0usize;
    let mut sccs: Vec<Vec<usize>> = Vec::new();

    for start in 0..n {
        if idx[start] != usize::MAX {
            continue;
        }
        // work stack of (node, next-child-pointer)
        let mut work: Vec<(usize, usize)> = vec![(start, 0)];
        idx[start] = counter;
        low[start] = counter;
        counter += 1;
        stack.push(start);
        on_stack[start] = true;
        while let Some(&(v, ci)) = work.last() {
            if ci < adj[v].len() {
                let w = adj[v][ci];
                work.last_mut().unwrap().1 += 1;
                if idx[w] == usize::MAX {
                    idx[w] = counter;
                    low[w] = counter;
                    counter += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    work.push((w, 0));
                } else if on_stack[w] {
                    low[v] = low[v].min(idx[w]);
                }
            } else {
                // done with v
                if low[v] == idx[v] {
                    let mut comp = Vec::new();
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack[w] = false;
                        comp.push(w);
                        if w == v {
                            break;
                        }
                    }
                    sccs.push(comp);
                }
                work.pop();
                if let Some(&(parent, _)) = work.last() {
                    low[parent] = low[parent].min(low[v]);
                }
            }
        }
    }

    let mut cycles: Vec<Vec<String>> = Vec::new();
    for comp in sccs {
        if comp.len() >= 2 {
            let members: BTreeSet<usize> = comp.iter().copied().collect();
            if let Some(path) = cycle_path(&members, &adj, &nodes) {
                cycles.push(path);
            }
        } else if comp.len() == 1 && self_loop[comp[0]] {
            cycles.push(vec![nodes[comp[0]].clone(), nodes[comp[0]].clone()]);
        }
    }
    cycles.sort();
    cycles
}

/// Build a concrete cyclic path through the SCC `members`, starting at the
/// lowest-indexed member and returning to it.
fn cycle_path(members: &BTreeSet<usize>, adj: &[Vec<usize>], nodes: &[String]) -> Option<Vec<String>> {
    let start = *members.iter().next()?;
    let mut path = vec![start];
    let mut visited: BTreeSet<usize> = BTreeSet::new();
    visited.insert(start);
    let mut current = start;
    loop {
        // prefer an edge back to start to close the loop (only if we've moved)
        if path.len() > 1 && adj[current].contains(&start) {
            break;
        }
        let next = adj[current]
            .iter()
            .copied()
            .find(|w| members.contains(w) && !visited.contains(w));
        match next {
            Some(w) => {
                visited.insert(w);
                path.push(w);
                current = w;
            }
            None => {
                // dead end inside the SCC without a fresh node — close if possible
                if adj[current].contains(&start) {
                    break;
                }
                return None;
            }
        }
    }
    let mut names: Vec<String> = path.into_iter().map(|i| nodes[i].clone()).collect();
    names.push(nodes[start].clone());
    Some(names)
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

fn language_summary(graph: &Graph) -> String {
    let langs: BTreeSet<&'static str> = graph.files.iter().map(|(_, l)| l.name()).collect();
    if langs.len() == 1 {
        langs.into_iter().next().unwrap().to_string()
    } else {
        "mixed".to_string()
    }
}

fn render_text(graph: &Graph, include_external: bool, cycles: Option<&[Vec<String>]>) -> String {
    let mut s = String::new();
    let n_files = graph.files.len();
    let n_edges = graph.edges.len();
    s.push_str(&format!(
        "Import graph ({}): {} file{}, {} internal edge{}\n",
        language_summary(graph),
        n_files,
        plural(n_files),
        n_edges,
        plural(n_edges),
    ));

    s.push_str("\nFiles:\n");
    for (p, l) in &graph.files {
        s.push_str(&format!("  {}  [{}]\n", p, l.name()));
    }

    s.push_str("\nImports (internal edges):\n");
    if graph.edges.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (from, to) in &graph.edges {
            s.push_str(&format!("  {} -> {}\n", from, to));
        }
    }

    s.push_str("\nExternal dependencies:\n");
    if !include_external {
        s.push_str("  (hidden — set include_external to list third-party/stdlib deps)\n");
    } else if graph.external.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (name, info) in &graph.external {
            let by: Vec<&str> = info.used_by.iter().map(|s| s.as_str()).collect();
            s.push_str(&format!(
                "  {} [{}]  <- {}\n",
                name,
                info.kind,
                by.join(", ")
            ));
        }
    }

    s.push_str("\nDependents (imported by):\n");
    let deps = dependents_map(graph);
    if deps.is_empty() {
        s.push_str("  (none)\n");
    } else {
        for (file, importers) in &deps {
            let by: Vec<&str> = importers.iter().map(|s| s.as_str()).collect();
            s.push_str(&format!("  {} <- {}\n", file, by.join(", ")));
        }
    }

    let orph = orphans(graph);
    s.push_str("\nOrphans (imported by no pasted file):\n");
    s.push_str(&list_or_none(&orph));

    let lv = leaves(graph);
    s.push_str("\nLeaves (import no pasted file):\n");
    s.push_str(&list_or_none(&lv));

    if !graph.unresolved.is_empty() {
        s.push_str("\nUnresolved (referenced file not pasted / non-file path):\n");
        for (from, spec) in &graph.unresolved {
            s.push_str(&format!("  {} -> {}\n", from, spec));
        }
    }

    if let Some(cycles) = cycles {
        s.push_str("\nCycles (circular dependencies):\n");
        if cycles.is_empty() {
            s.push_str("  (none)\n");
        } else {
            for cyc in cycles {
                s.push_str(&format!("  {}\n", cyc.join(" -> ")));
            }
        }
    }

    s
}

fn list_or_none(items: &[String]) -> String {
    if items.is_empty() {
        "  (none)\n".to_string()
    } else {
        let mut s = String::new();
        for it in items {
            s.push_str(&format!("  {}\n", it));
        }
        s
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        ""
    } else {
        "s"
    }
}

fn render_json(graph: &Graph, include_external: bool, cycles: Option<&[Vec<String>]>) -> String {
    use serde_json::{json, Map, Value};
    let files: Vec<Value> = graph
        .files
        .iter()
        .map(|(p, l)| json!({ "path": p, "language": l.name() }))
        .collect();
    let edges: Vec<Value> = graph
        .edges
        .iter()
        .map(|(f, t)| json!({ "from": f, "to": t }))
        .collect();
    let mut root = Map::new();
    root.insert("files".into(), json!(files));
    root.insert("edges".into(), json!(edges));

    if include_external {
        let ext: Vec<Value> = graph
            .external
            .iter()
            .map(|(name, info)| {
                json!({
                    "name": name,
                    "kind": info.kind,
                    "used_by": info.used_by.iter().collect::<Vec<_>>(),
                })
            })
            .collect();
        root.insert("external".into(), json!(ext));
    }

    let deps = dependents_map(graph);
    let dependents: Map<String, Value> = deps
        .into_iter()
        .map(|(k, v)| (k, json!(v.into_iter().collect::<Vec<_>>())))
        .collect();
    root.insert("dependents".into(), Value::Object(dependents));
    root.insert("orphans".into(), json!(orphans(graph)));
    root.insert("leaves".into(), json!(leaves(graph)));

    if !graph.unresolved.is_empty() {
        let unres: Vec<Value> = graph
            .unresolved
            .iter()
            .map(|(f, s)| json!({ "from": f, "spec": s }))
            .collect();
        root.insert("unresolved".into(), json!(unres));
    }

    if let Some(cycles) = cycles {
        root.insert("cycles".into(), json!(cycles));
    }

    serde_json::to_string_pretty(&Value::Object(root)).unwrap()
}

fn dot_escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn render_dot(graph: &Graph, include_external: bool, cycles: Option<&[Vec<String>]>) -> String {
    let cyclic: BTreeSet<&String> = match cycles {
        Some(cs) => cs.iter().flatten().collect(),
        None => BTreeSet::new(),
    };
    let mut s = String::from("digraph imports {\n  rankdir=LR;\n  node [shape=box, style=rounded];\n");
    for (p, _) in &graph.files {
        let attrs = if cyclic.contains(p) {
            " [color=\"red\"]"
        } else {
            ""
        };
        s.push_str(&format!("  \"{}\"{};\n", dot_escape(p), attrs));
    }
    if include_external {
        for name in graph.external.keys() {
            s.push_str(&format!(
                "  \"{}\" [shape=ellipse, style=dashed];\n",
                dot_escape(name)
            ));
        }
    }
    for (from, to) in &graph.edges {
        s.push_str(&format!(
            "  \"{}\" -> \"{}\";\n",
            dot_escape(from),
            dot_escape(to)
        ));
    }
    if include_external {
        for (name, info) in &graph.external {
            for from in &info.used_by {
                s.push_str(&format!(
                    "  \"{}\" -> \"{}\" [style=dashed];\n",
                    dot_escape(from),
                    dot_escape(name)
                ));
            }
        }
    }
    s.push_str("}\n");
    s
}

fn render_mermaid(graph: &Graph, include_external: bool, cycles: Option<&[Vec<String>]>) -> String {
    let cyclic: BTreeSet<&String> = match cycles {
        Some(cs) => cs.iter().flatten().collect(),
        None => BTreeSet::new(),
    };
    let mut ids: BTreeMap<String, String> = BTreeMap::new();
    let mut s = String::from("graph LR\n");
    for (i, (p, _)) in graph.files.iter().enumerate() {
        let id = format!("f{}", i);
        s.push_str(&format!("  {}[\"{}\"]\n", id, mermaid_escape(p)));
        ids.insert(p.clone(), id);
    }
    if include_external {
        for (i, name) in graph.external.keys().enumerate() {
            let id = format!("e{}", i);
            s.push_str(&format!("  {}([\"{}\"])\n", id, mermaid_escape(name)));
            ids.insert(name.clone(), id);
        }
    }
    for (from, to) in &graph.edges {
        s.push_str(&format!("  {} --> {}\n", ids[from], ids[to]));
    }
    if include_external {
        for (name, info) in &graph.external {
            for from in &info.used_by {
                s.push_str(&format!("  {} -.-> {}\n", ids[from], ids[name]));
            }
        }
    }
    for p in &cyclic {
        if let Some(id) = ids.get(*p) {
            s.push_str(&format!("  class {} cycle;\n", id));
        }
    }
    if !cyclic.is_empty() {
        s.push_str("  classDef cycle stroke:#e00,stroke-width:2px;\n");
    }
    s
}

fn mermaid_escape(s: &str) -> String {
    s.replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js_multi_file_happy_path() {
        let input = "\
=== src/app.js ===
import { greet } from './util/greet';
import React from 'react';
const fs = require('fs');
=== src/util/greet.js ===
export const greet = () => 'hi';
";
        let out = run(input, "auto", "text", true, true).unwrap();
        assert!(out.contains("src/app.js -> src/util/greet.js"), "edge:\n{out}");
        assert!(out.contains("react [package]"), "external react:\n{out}");
        assert!(out.contains("fs [stdlib]"), "node builtin:\n{out}");
        // greet.js is imported → not an orphan; app.js is an orphan
        assert!(out.contains("src/util/greet.js <- src/app.js"));
        // greet.js imports nothing → leaf; app.js is not a leaf
        let leaves_section = out.split("Leaves").nth(1).unwrap();
        assert!(leaves_section.contains("src/util/greet.js"));
        assert!(out.contains("Cycles"));
        assert!(out.contains("(none)")); // no cycles
    }

    #[test]
    fn python_relative_import_and_cycle() {
        let input = "\
=== pkg/a.py ===
from .b import thing
import os
=== pkg/b.py ===
from pkg.a import other
";
        let out = run(input, "python", "text", true, true).unwrap();
        assert!(out.contains("pkg/a.py -> pkg/b.py"), "relative edge:\n{out}");
        assert!(out.contains("pkg/b.py -> pkg/a.py"), "absolute edge:\n{out}");
        assert!(out.contains("os [package]"), "stdlib listed as external:\n{out}");
        // a↔b is a cycle
        let cyc = out.split("Cycles").nth(1).unwrap();
        assert!(cyc.contains("pkg/a.py -> pkg/b.py -> pkg/a.py"), "cycle:\n{out}");
    }

    #[test]
    fn single_file_no_header_infers_path() {
        let input = "import os\nfrom sys import argv\n";
        let out = run(input, "python", "text", true, true).unwrap();
        assert!(out.contains("input.py  [python]"), "inferred path:\n{out}");
        assert!(out.contains("os [package]"));
        assert!(out.contains("sys [package]"));
    }

    #[test]
    fn rust_mod_and_crate() {
        let input = "\
=== src/lib.rs ===
mod helper;
use serde::Serialize;
use crate::helper::run;
=== src/helper.rs ===
pub fn run() {}
";
        let out = run(input, "auto", "text", true, true).unwrap();
        assert!(out.contains("src/lib.rs -> src/helper.rs"), "mod edge:\n{out}");
        assert!(out.contains("serde [package]"), "crate:\n{out}");
        assert!(out.contains("use crate::"), "unresolved crate path:\n{out}");
    }

    #[test]
    fn go_import_block() {
        let input = "\
=== main.go ===
package main
import (
    \"fmt\"
    \"github.com/gin-gonic/gin\"
)
";
        let out = run(input, "go", "text", true, true).unwrap();
        assert!(out.contains("fmt [stdlib]"), "go stdlib:\n{out}");
        assert!(
            out.contains("github.com/gin-gonic/gin [package]"),
            "go third-party:\n{out}"
        );
    }

    #[test]
    fn include_external_toggle_hides_deps() {
        let input = "=== a.js ===\nimport x from 'lodash';\n";
        let with = run(input, "auto", "text", true, true).unwrap();
        assert!(with.contains("lodash [package]"));
        let without = run(input, "auto", "text", false, true).unwrap();
        assert!(!without.contains("lodash [package]"));
        assert!(without.contains("(hidden"));
    }

    #[test]
    fn detect_cycles_off_omits_section() {
        let input = "=== a.js ===\nimport './b';\n=== b.js ===\nimport './a';\n";
        let on = run(input, "auto", "text", true, true).unwrap();
        assert!(on.contains("Cycles"));
        let off = run(input, "auto", "text", true, false).unwrap();
        assert!(!off.contains("Cycles"));
    }

    #[test]
    fn json_format_is_valid_and_has_edges() {
        let input = "=== a.js ===\nimport './b';\n=== b.js ===\nexport const b = 1;\n";
        let out = run(input, "auto", "json", true, true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["edges"][0]["from"], "a.js");
        assert_eq!(v["edges"][0]["to"], "b.js");
        assert!(v["cycles"].is_array());
    }

    #[test]
    fn dot_and_mermaid_render() {
        let input = "=== a.js ===\nimport './b';\nimport 'react';\n=== b.js ===\nexport const b=1;\n";
        let dot = run(input, "auto", "dot", true, true).unwrap();
        assert!(dot.starts_with("digraph imports {"));
        assert!(dot.contains("\"a.js\" -> \"b.js\""));
        let mmd = run(input, "auto", "mermaid", true, true).unwrap();
        assert!(mmd.starts_with("graph LR"));
        assert!(mmd.contains("-->"));
    }

    #[test]
    fn empty_input_errors() {
        assert!(run("   \n  ", "auto", "text", true, true).is_err());
    }

    #[test]
    fn invalid_language_and_format_error() {
        assert!(run("import os", "cobol", "text", true, true).is_err());
        assert!(run("=== a.py ===\nimport os", "auto", "svg", true, true).is_err());
    }

    #[test]
    fn exact_text_output_small_case() {
        let input = "\
=== a.js ===
import './b';
=== b.js ===
export const b = 1;
";
        let out = run(input, "auto", "text", true, true).unwrap();
        let expected = "\
Import graph (javascript): 2 files, 1 internal edge

Files:
  a.js  [javascript]
  b.js  [javascript]

Imports (internal edges):
  a.js -> b.js

External dependencies:
  (none)

Dependents (imported by):
  b.js <- a.js

Orphans (imported by no pasted file):
  a.js

Leaves (import no pasted file):
  b.js

Cycles (circular dependencies):
  (none)
";
        assert_eq!(out, expected, "\n--- got ---\n{out}");
    }
}
