//! gizza-ai/stack-trace-analyzer — core logic.
//!
//! Parses a raw stack trace from one of eight languages into a structured
//! chain of exceptions and frames, identifies the **root cause** and the
//! **first frame of your own code**, and marks every frame as `user` or
//! `framework`.
//!
//! Everything is normalised across languages so the output reads the same way
//! whatever you paste:
//!
//! * **frames** are listed innermost first (the throw/panic site first). Python,
//!   which prints outermost first, is reversed;
//! * **the exception chain** is listed reported-exception first, root cause
//!   last. Python and PHP, which print the original first, are reversed and
//!   their chain markers re-attached as `Caused by` style relations.
//!
//! No I/O, no clock, no host calls — pure text in, text out.

use regex::Regex;
use std::sync::OnceLock;

/// Largest accepted input, in bytes. A trace this size is already thousands of
/// frames; beyond it the quadratic-ish rendering gets slow in a browser tab.
pub const MAX_INPUT: usize = 200_000;
/// Hard ceiling for the `limit` parameter (frames kept per exception).
pub const MAX_LIMIT: u32 = 2000;
/// `limit` used when the caller passes 0 / leaves the field blank.
pub const DEFAULT_LIMIT: u32 = 100;

/// Lazily-compiled regex, cached per call site.
macro_rules! rx {
    ($re:literal) => {{
        static RE: OnceLock<Regex> = OnceLock::new();
        RE.get_or_init(|| Regex::new($re).expect("static regex"))
    }};
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The languages whose trace formats this tool understands.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Lang {
    Java,
    Python,
    Javascript,
    Go,
    Ruby,
    Csharp,
    Rust,
    Php,
}

impl Lang {
    /// The canonical parameter value (also used in JSON output).
    pub fn id(self) -> &'static str {
        match self {
            Lang::Java => "java",
            Lang::Python => "python",
            Lang::Javascript => "javascript",
            Lang::Go => "go",
            Lang::Ruby => "ruby",
            Lang::Csharp => "csharp",
            Lang::Rust => "rust",
            Lang::Php => "php",
        }
    }

    /// Human-facing name used in the report header.
    pub fn label(self) -> &'static str {
        match self {
            Lang::Java => "Java / Kotlin / Scala",
            Lang::Python => "Python",
            Lang::Javascript => "JavaScript / TypeScript",
            Lang::Go => "Go",
            Lang::Ruby => "Ruby",
            Lang::Csharp => "C# / .NET",
            Lang::Rust => "Rust",
            Lang::Php => "PHP",
        }
    }

    fn from_id(s: &str) -> Option<Lang> {
        Some(match s {
            "java" => Lang::Java,
            "python" => Lang::Python,
            "javascript" => Lang::Javascript,
            "go" => Lang::Go,
            "ruby" => Lang::Ruby,
            "csharp" => Lang::Csharp,
            "rust" => Lang::Rust,
            "php" => Lang::Php,
            _ => return None,
        })
    }
}

/// Whether a frame belongs to the code being debugged or to a library.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Kind {
    User,
    /// The placeholder a freshly parsed `Frame` carries: `analyze` classifies
    /// every frame before rendering, and an unclassified frame must never be
    /// claimed as the caller's own code.
    #[default]
    Framework,
}

impl Kind {
    fn id(self) -> &'static str {
        match self {
            Kind::User => "user",
            Kind::Framework => "framework",
        }
    }
}

/// One call-stack entry.
#[derive(Clone, Debug, Default)]
pub struct Frame {
    /// Function / method / class path as printed, e.g. `com.example.App.start`.
    pub function: String,
    /// Source file as printed, e.g. `App.java` or `/app/src/user.js`.
    pub file: String,
    pub line: Option<u32>,
    pub column: Option<u32>,
    /// Where the language printed something instead of a location, e.g.
    /// `Native Method`, `Unknown Source`, or a repeat marker.
    pub note: String,
    /// The source line Python (and only Python) prints under each frame.
    pub context: String,
    kind: Kind,
}

impl Frame {
    /// `function(file:line:col)` — the one-line form used in the report.
    pub fn render(&self) -> String {
        let name = if self.function.is_empty() {
            "<anonymous>"
        } else {
            self.function.as_str()
        };
        let mut loc = String::new();
        if !self.file.is_empty() {
            loc.push_str(&self.file);
            if let Some(l) = self.line {
                loc.push(':');
                loc.push_str(&l.to_string());
                if let Some(c) = self.column {
                    loc.push(':');
                    loc.push_str(&c.to_string());
                }
            }
        } else if !self.note.is_empty() {
            loc.push_str(&self.note);
        }
        if loc.is_empty() {
            name.to_string()
        } else {
            format!("{name}({loc})")
        }
    }

    /// The frame's classification. Only meaningful after [`analyze`] has run.
    pub fn kind(&self) -> Kind {
        self.kind
    }
}

/// One exception in the chain, with the frames printed beneath it.
#[derive(Clone, Debug, Default)]
pub struct Exc {
    /// Exception / error class, e.g. `java.lang.IllegalStateException`.
    pub exception: String,
    pub message: String,
    /// How this exception relates to the one above it: empty for the reported
    /// exception, otherwise `Caused by`, `Suppressed`, `Inner exception`, …
    pub relation: String,
    pub frames: Vec<Frame>,
    /// Java's `... 12 more` — frames identical to the enclosing trace.
    pub elided: u32,
}

impl Exc {
    fn new(relation: String, exception: String, message: String) -> Exc {
        Exc {
            exception,
            message,
            relation,
            ..Default::default()
        }
    }

    /// `Type: message`, or just `Type` when there is no message.
    pub fn title(&self) -> String {
        match (self.exception.is_empty(), self.message.is_empty()) {
            (true, true) => "(unnamed exception)".to_string(),
            (true, false) => self.message.clone(),
            (false, true) => self.exception.clone(),
            (false, false) => format!("{}: {}", self.exception, first_line(&self.message)),
        }
    }
}

fn first_line(s: &str) -> &str {
    s.split('\n').next().unwrap_or(s)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Parse and analyse `trace`.
///
/// * `language` — `auto` (or blank) to detect, else one of [`Lang::id`].
/// * `output` — `report` (blank → report), `table`, or `json`.
/// * `user_packages` — comma-separated prefixes that mark *your* code. When
///   non-empty this becomes an allow-list: a frame is `user` only if it matches.
/// * `hide_framework` — drop framework frames from the output.
/// * `reverse` — list frames outermost first instead of innermost first.
/// * `limit` — frames kept per exception (0 → [`DEFAULT_LIMIT`]).
pub fn analyze(
    trace: &str,
    language: &str,
    output: &str,
    user_packages: &str,
    hide_framework: bool,
    reverse: bool,
    limit: u32,
) -> Result<String, String> {
    if trace.trim().is_empty() {
        return Err("no stack trace provided — paste the trace text, for example a Java \"Exception in thread ...\" block or a Python \"Traceback (most recent call last):\" block".to_string());
    }
    if trace.len() > MAX_INPUT {
        return Err(format!(
            "stack trace is too large: {} bytes, maximum is {} bytes — paste just the failing trace instead of the whole log",
            trace.len(),
            MAX_INPUT
        ));
    }

    let lang_arg = language.trim();
    let requested = if lang_arg.is_empty() || lang_arg == "auto" {
        None
    } else {
        Some(Lang::from_id(lang_arg).ok_or_else(|| {
            format!(
                "unknown language '{lang_arg}' (use auto, java, python, javascript, go, ruby, csharp, rust, or php)"
            )
        })?)
    };

    let out_arg = output.trim();
    let out_kind = match out_arg {
        "" | "report" => "report",
        "table" => "table",
        "json" => "json",
        other => {
            return Err(format!(
                "unknown output '{other}' (use report, table, or json)"
            ))
        }
    };

    let detected = requested.is_none();
    let lang = match requested {
        Some(l) => l,
        None => detect(trace).ok_or_else(|| {
            "could not detect the language of this stack trace — pick one explicitly (java, python, javascript, go, ruby, csharp, rust, php), or check that the frame lines were pasted along with the error message".to_string()
        })?,
    };

    let mut chain = parse(lang, trace);
    chain.retain(|e| !(e.frames.is_empty() && e.exception.is_empty() && e.message.is_empty()));
    if chain.is_empty() {
        return Err(format!(
            "no frames or exception header found after reading the input as {} — check the language, or paste the full trace including the \"{}\" lines",
            lang.label(),
            frame_hint(lang)
        ));
    }

    let prefixes: Vec<String> = user_packages
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    for e in chain.iter_mut() {
        for f in e.frames.iter_mut() {
            f.kind = classify(lang, f, &prefixes);
        }
    }

    let limit = if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.clamp(1, MAX_LIMIT)
    };

    let view = View::build(&chain, hide_framework, reverse, limit);
    Ok(match out_kind {
        "table" => render_table(lang, detected, &chain, &view),
        "json" => render_json(lang, detected, &chain, &view),
        _ => render_report(lang, detected, &chain, &view),
    })
}

fn frame_hint(lang: Lang) -> &'static str {
    match lang {
        Lang::Java => "at com.example.Class.method(File.java:42)",
        Lang::Python => "File \"app.py\", line 42, in main",
        Lang::Javascript => "at fn (/app/file.js:42:7)",
        Lang::Go => "goroutine 1 [running]:",
        Lang::Ruby => "app.rb:42:in `method'",
        Lang::Csharp => "at App.Class.Method() in C:\\src\\File.cs:line 42",
        Lang::Rust => "stack backtrace:",
        Lang::Php => "#0 /app/file.php(42): doWork()",
    }
}

// ---------------------------------------------------------------------------
// The rendered view: which frames survive hide_framework / limit, in what order
// ---------------------------------------------------------------------------

struct ExcView {
    /// Indices into `Exc::frames`, in display order.
    shown: Vec<usize>,
    /// Frames dropped by `limit` (after `hide_framework`).
    truncated: usize,
    /// Frames dropped by `hide_framework`.
    hidden: usize,
}

struct View {
    per_exc: Vec<ExcView>,
    total: usize,
    user: usize,
    framework: usize,
    /// (exception index, frame index) of the first user frame, root cause first.
    first_user: Option<(usize, usize)>,
    reversed: bool,
}

impl View {
    fn build(chain: &[Exc], hide_framework: bool, reverse: bool, limit: u32) -> View {
        let mut per_exc = Vec::with_capacity(chain.len());
        let (mut total, mut user, mut framework) = (0usize, 0usize, 0usize);
        for e in chain {
            let mut keep: Vec<usize> = Vec::new();
            let mut hidden = 0usize;
            for (i, f) in e.frames.iter().enumerate() {
                total += 1;
                match f.kind {
                    Kind::User => user += 1,
                    Kind::Framework => framework += 1,
                }
                if hide_framework && f.kind == Kind::Framework {
                    hidden += 1;
                } else {
                    keep.push(i);
                }
            }
            if reverse {
                keep.reverse();
            }
            let truncated = keep.len().saturating_sub(limit as usize);
            keep.truncate(limit as usize);
            per_exc.push(ExcView {
                shown: keep,
                truncated,
                hidden,
            });
        }

        // "First user frame": the root cause is the interesting exception, so
        // look there first, then walk the rest of the chain toward the reported
        // exception. Always innermost-first, regardless of `reverse`.
        let mut order: Vec<usize> = (0..chain.len()).rev().collect();
        if order.is_empty() {
            order.push(0);
        }
        let mut first_user = None;
        'outer: for &ei in &order {
            for (fi, f) in chain[ei].frames.iter().enumerate() {
                if f.kind == Kind::User {
                    first_user = Some((ei, fi));
                    break 'outer;
                }
            }
        }

        View {
            per_exc,
            total,
            user,
            framework,
            first_user,
            reversed: reverse,
        }
    }

    fn shown_count(&self) -> usize {
        self.per_exc.iter().map(|e| e.shown.len()).sum()
    }
}

// ---------------------------------------------------------------------------
// Renderers
// ---------------------------------------------------------------------------

fn render_report(lang: Lang, detected: bool, chain: &[Exc], view: &View) -> String {
    let mut s = String::new();
    let how = if detected {
        "auto-detected"
    } else {
        "as selected"
    };
    s.push_str(&format!("Language: {} ({how})\n", lang.label()));
    s.push_str(&format!("Reported: {}\n", chain[0].title()));
    let root = chain.len() - 1;
    if root > 0 {
        s.push_str(&format!("Root cause: {}\n", chain[root].title()));
    } else {
        s.push_str("Root cause: same as reported (single exception)\n");
    }
    match view.first_user {
        Some((ei, fi)) => s.push_str(&format!(
            "First user frame: {}\n",
            chain[ei].frames[fi].render()
        )),
        None => s.push_str(
            "First user frame: none found (every frame looks like framework or standard-library code)\n",
        ),
    }
    s.push_str(&format!(
        "Exceptions: {} | Frames: {} total, {} user, {} framework, {} shown\n",
        chain.len(),
        view.total,
        view.user,
        view.framework,
        view.shown_count()
    ));

    for (ei, e) in chain.iter().enumerate() {
        let v = &view.per_exc[ei];
        s.push('\n');
        let head = if e.relation.is_empty() {
            e.title()
        } else {
            format!("{}: {}", e.relation, e.title())
        };
        s.push_str(&format!("[{}] {}\n", ei + 1, head));
        for extra in e.message.split('\n').skip(1) {
            s.push_str(&format!("    {extra}\n"));
        }
        if v.shown.is_empty() {
            s.push_str("    (no frames)\n");
        }
        for (n, &fi) in v.shown.iter().enumerate() {
            let f = &e.frames[fi];
            let mark = if f.kind == Kind::User { '*' } else { ' ' };
            s.push_str(&format!("  {:>3}. {}  {}\n", n + 1, mark, f.render()));
            if !f.context.is_empty() {
                // 10 spaces = the width of the "  nnn. m  " frame prefix above,
                // so a Python context line sits under its frame's function.
                s.push_str(&format!("          {}\n", f.context));
            }
        }
        if v.truncated > 0 {
            s.push_str(&format!(
                "       ... {} more frame(s) not shown (raise the frame limit)\n",
                v.truncated
            ));
        }
        if v.hidden > 0 {
            s.push_str(&format!(
                "       ... {} framework frame(s) hidden\n",
                v.hidden
            ));
        }
        if e.elided > 0 {
            s.push_str(&format!(
                "       ... {} frame(s) elided by the runtime (identical to the enclosing trace)\n",
                e.elided
            ));
        }
    }

    let order = if view.reversed {
        "outermost (entry point) first"
    } else {
        "innermost (throw site) first"
    };
    s.push_str(&format!("\n* = your code. Frames listed {order}.\n"));
    s
}

fn render_table(lang: Lang, detected: bool, chain: &[Exc], view: &View) -> String {
    let mut s = String::new();
    let how = if detected { "auto-detected" } else { "selected" };
    s.push_str(&format!(
        "{} ({how}) · {} exception(s) · {} frame(s) shown of {} · {} user · {} framework\n",
        lang.label(),
        chain.len(),
        view.shown_count(),
        view.total,
        view.user,
        view.framework
    ));

    for (ei, e) in chain.iter().enumerate() {
        let v = &view.per_exc[ei];
        let head = if e.relation.is_empty() {
            e.title()
        } else {
            format!("{}: {}", e.relation, e.title())
        };
        s.push_str(&format!("\n### [{}] {}\n\n", ei + 1, md_cell(&head)));
        s.push_str("| # | Code | Function | File | Line | Col |\n");
        s.push_str("| --- | --- | --- | --- | --- | --- |\n");
        if v.shown.is_empty() {
            s.push_str("| — | — | (no frames) | — | — | — |\n");
        }
        for (n, &fi) in v.shown.iter().enumerate() {
            let f = &e.frames[fi];
            let file = if !f.file.is_empty() {
                md_cell(&f.file)
            } else if !f.note.is_empty() {
                md_cell(&f.note)
            } else {
                "—".to_string()
            };
            s.push_str(&format!(
                "| {} | {} | {} | {} | {} | {} |\n",
                n + 1,
                f.kind.id(),
                md_cell(if f.function.is_empty() {
                    "<anonymous>"
                } else {
                    &f.function
                }),
                file,
                f.line.map(|l| l.to_string()).unwrap_or_else(|| "—".into()),
                f.column.map(|c| c.to_string()).unwrap_or_else(|| "—".into()),
            ));
        }
        if v.truncated > 0 {
            s.push_str(&format!(
                "\n_{} more frame(s) not shown (raise the frame limit)._\n",
                v.truncated
            ));
        }
        if v.hidden > 0 {
            s.push_str(&format!("\n_{} framework frame(s) hidden._\n", v.hidden));
        }
        if e.elided > 0 {
            s.push_str(&format!(
                "\n_{} frame(s) elided by the runtime (identical to the enclosing trace)._\n",
                e.elided
            ));
        }
    }
    s
}

/// Escape the two characters that would break a Markdown table cell.
fn md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

fn render_json(lang: Lang, detected: bool, chain: &[Exc], view: &View) -> String {
    let root = chain.len() - 1;
    let exc_obj = |e: &Exc| {
        serde_json::json!({
            "type": e.exception,
            "message": e.message,
        })
    };

    let chain_json: Vec<serde_json::Value> = chain
        .iter()
        .enumerate()
        .map(|(ei, e)| {
            let v = &view.per_exc[ei];
            let frames: Vec<serde_json::Value> = v
                .shown
                .iter()
                .enumerate()
                .map(|(n, &fi)| frame_json(n + 1, &e.frames[fi]))
                .collect();
            serde_json::json!({
                "index": ei + 1,
                "relation": e.relation,
                "type": e.exception,
                "message": e.message,
                "frames": frames,
                "frames_hidden": v.hidden,
                "frames_truncated": v.truncated,
                "frames_elided_by_runtime": e.elided,
            })
        })
        .collect();

    let value = serde_json::json!({
        "language": lang.id(),
        "language_detected": detected,
        "frame_order": if view.reversed { "outermost-first" } else { "innermost-first" },
        "reported": exc_obj(&chain[0]),
        "root_cause": exc_obj(&chain[root]),
        "first_user_frame": view
            .first_user
            .map(|(ei, fi)| frame_json(fi + 1, &chain[ei].frames[fi])),
        "counts": {
            "exceptions": chain.len(),
            "frames_total": view.total,
            "frames_shown": view.shown_count(),
            "user": view.user,
            "framework": view.framework,
        },
        "chain": chain_json,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn frame_json(n: usize, f: &Frame) -> serde_json::Value {
    serde_json::json!({
        "index": n,
        "kind": f.kind.id(),
        "function": f.function,
        "file": f.file,
        "line": f.line,
        "column": f.column,
        "note": f.note,
        "source": f.context,
        "text": f.render(),
    })
}

// ---------------------------------------------------------------------------
// Language detection
// ---------------------------------------------------------------------------

/// Score each language's marker lines and take the winner. Returns `None` when
/// nothing matched at all.
pub fn detect(text: &str) -> Option<Lang> {
    let mut score = [0i32; 8];
    const JAVA: usize = 0;
    const PYTHON: usize = 1;
    const JS: usize = 2;
    const GO: usize = 3;
    const RUBY: usize = 4;
    const CSHARP: usize = 5;
    const RUST: usize = 6;
    const PHP: usize = 7;

    for line in text.lines().take(4000) {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }

        // Python
        if t.starts_with("Traceback (most recent call last)") {
            score[PYTHON] += 12;
        }
        if rx!(r#"^File ".*", line \d+"#).is_match(t) {
            score[PYTHON] += 5;
        }

        // Java / Kotlin / Scala
        if rx!(r"^at\s+[^\s(]+\((?:[\w$. +-]+\.(?:java|kt|kts|scala|groovy|jsp):\d+|Native Method|Unknown Source.*)\)$")
            .is_match(t)
        {
            score[JAVA] += 6;
        }
        if t.starts_with("Caused by:") || t.starts_with("Suppressed:") {
            score[JAVA] += 4;
        }
        if rx!(r"^\.\.\.\s+\d+\s+more$").is_match(t) {
            score[JAVA] += 5;
        }
        if t.starts_with("Exception in thread ") {
            score[JAVA] += 6;
        }

        // JavaScript / TypeScript
        if rx!(r"^at\s+.*:\d+:\d+\)?$").is_match(t) {
            score[JS] += 5;
        }
        if t.contains("node_modules") || t.contains("node:internal") {
            score[JS] += 4;
        }

        // Go
        if rx!(r"^goroutine \d+ \[").is_match(t) {
            score[GO] += 12;
        }
        if rx!(r"^.*\.go:\d+ \+0x").is_match(t) {
            score[GO] += 6;
        }
        if t.starts_with("panic: ") || t.starts_with("fatal error: ") {
            score[GO] += 3;
        }

        // Ruby
        if rx!(r"^(from )?.*:\d+:in [`'].*'").is_match(t) {
            score[RUBY] += 7;
        }

        // C# / .NET
        if rx!(r"^at\s+.+\s+in\s+.+:line\s+\d+$").is_match(t) {
            score[CSHARP] += 8;
        }
        if t.starts_with("--->") {
            score[CSHARP] += 5;
        }
        if t.contains("End of inner exception stack trace") {
            score[CSHARP] += 8;
        }
        if rx!(r"^at\s+(?:System|Microsoft|Newtonsoft)\..*\(.*\)$").is_match(t) {
            score[CSHARP] += 4;
        }

        // Rust
        if t.contains("panicked at") {
            score[RUST] += 7;
        }
        if t == "stack backtrace:" {
            score[RUST] += 10;
        }
        if t.contains("library/std/src") || t.contains("library/core/src") {
            score[RUST] += 4;
        }
        if rx!(r"^\d+:\s+[\w:<>{}. ]+$").is_match(t) && t.contains("::") {
            score[RUST] += 3;
        }

        // PHP
        if rx!(r"^#\d+\s+").is_match(t) {
            score[PHP] += 5;
        }
        if t == "Stack trace:" {
            score[PHP] += 6;
        }
        if t.contains("PHP Fatal error") || t.contains("PHP Warning") {
            score[PHP] += 8;
        }
        if rx!(r"^#\d+ \{main\}$").is_match(t) {
            score[PHP] += 6;
        }
    }

    let order = [
        (PYTHON, Lang::Python),
        (GO, Lang::Go),
        (RUST, Lang::Rust),
        (PHP, Lang::Php),
        (CSHARP, Lang::Csharp),
        (RUBY, Lang::Ruby),
        (JAVA, Lang::Java),
        (JS, Lang::Javascript),
    ];
    let mut best: Option<(i32, Lang)> = None;
    for (idx, lang) in order {
        let s = score[idx];
        if s > 0 && best.map(|(b, _)| s > b).unwrap_or(true) {
            best = Some((s, lang));
        }
    }
    best.map(|(_, l)| l)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

fn parse(lang: Lang, text: &str) -> Vec<Exc> {
    match lang {
        Lang::Java => parse_java(text),
        Lang::Python => parse_python(text),
        Lang::Javascript => parse_javascript(text),
        Lang::Go => parse_go(text),
        Lang::Ruby => parse_ruby(text),
        Lang::Csharp => parse_csharp(text),
        Lang::Rust => parse_rust(text),
        Lang::Php => parse_php(text),
    }
}

/// Split `com.foo.BarException: message` into its two halves. Only accepts a
/// left-hand side that actually looks like an exception class, so prose lines
/// ("Stack trace:", "note: run with ...") are not mistaken for headers.
fn split_exception_header(s: &str) -> Option<(String, String)> {
    let c = rx!(r"^([A-Za-z_$][\w$]*(?:[.$\\/][A-Za-z_$][\w$]*)*)(?::[ \t]?([\s\S]*))?$")
        .captures(s.trim())?;
    let ty = c.get(1)?.as_str().to_string();
    let msg = c.get(2).map(|m| m.as_str().to_string());
    let looks_like_class = ty.contains('.')
        || ty.contains('\\')
        || ty.ends_with("Exception")
        || ty.ends_with("Error")
        || ty.ends_with("Throwable")
        || ty.ends_with("Interrupt")
        || ty.ends_with("Exit")
        || ty.ends_with("Iteration")
        || ty.ends_with("Warning");
    if !looks_like_class {
        return None;
    }
    Some((ty, msg.unwrap_or_default()))
}

// --- Java / Kotlin / Scala -------------------------------------------------

fn parse_java(text: &str) -> Vec<Exc> {
    let mut out: Vec<Exc> = Vec::new();
    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if let Some(c) = rx!(r"^\.\.\.\s+(\d+)\s+more$").captures(t) {
            if let Some(e) = out.last_mut() {
                e.elided = c[1].parse().unwrap_or(0);
            }
            continue;
        }

        if let Some(c) = rx!(r"^at\s+([^\s(][^(]*)\((.*)\)$").captures(t) {
            let f = java_frame(&c[1], &c[2]);
            if let Some(e) = out.last_mut() {
                e.frames.push(f);
            } else {
                // Frames before any header — synthesise a holder.
                let mut e = Exc::new(String::new(), String::new(), String::new());
                e.frames.push(f);
                out.push(e);
            }
            continue;
        }

        // Header line, possibly prefixed with a relation or a thread banner.
        let (relation, rest) = java_relation(t);
        let rest = rx!(r#"^Exception in thread "[^"]*"\s+"#)
            .replace(rest, "")
            .to_string();
        if let Some((ty, msg)) = split_exception_header(&rest) {
            out.push(Exc::new(relation, ty, msg));
            continue;
        }

        // Anything else while a header is still open continues its message.
        if let Some(e) = out.last_mut() {
            if e.frames.is_empty() && !e.exception.is_empty() {
                if !e.message.is_empty() {
                    e.message.push('\n');
                }
                e.message.push_str(t);
            }
        }
    }
    out
}

fn java_relation(t: &str) -> (String, &str) {
    for (prefix, label) in [("Caused by:", "Caused by"), ("Suppressed:", "Suppressed")] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return (label.to_string(), rest.trim_start());
        }
    }
    (String::new(), t)
}

fn java_frame(func: &str, loc: &str) -> Frame {
    let mut f = Frame {
        function: func.trim().to_string(),
        ..Default::default()
    };
    let l = loc.trim();
    if l.is_empty() {
        return f;
    }
    if l.eq_ignore_ascii_case("native method") {
        f.note = "Native Method".to_string();
    } else if l.starts_with("Unknown Source") {
        f.note = l.to_string();
    } else if let Some(i) = l.rfind(':') {
        match l[i + 1..].trim().parse::<u32>() {
            Ok(n) => {
                f.file = l[..i].to_string();
                f.line = Some(n);
            }
            Err(_) => f.file = l.to_string(),
        }
    } else {
        f.file = l.to_string();
    }
    f
}

// --- Python ----------------------------------------------------------------

fn parse_python(text: &str) -> Vec<Exc> {
    let mut out: Vec<Exc> = Vec::new();
    let mut cur: Option<Exc> = None;
    let mut pending = String::new();
    let mut just_closed = false;

    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if t.starts_with("Traceback (most recent call last)") {
            if let Some(c) = cur.take() {
                out.push(c);
            }
            cur = Some(Exc::new(
                std::mem::take(&mut pending),
                String::new(),
                String::new(),
            ));
            just_closed = false;
            continue;
        }
        if t == "During handling of the above exception, another exception occurred:" {
            if let Some(c) = cur.take() {
                out.push(c);
            }
            pending = "During handling".to_string();
            just_closed = false;
            continue;
        }
        if t == "The above exception was the direct cause of the following exception:" {
            if let Some(c) = cur.take() {
                out.push(c);
            }
            pending = "Direct cause".to_string();
            just_closed = false;
            continue;
        }

        if let Some(c) = rx!(r#"^File "(.*)", line (\d+)(?:, in (.*?))?$"#).captures(t) {
            let f = Frame {
                function: c.get(3).map(|m| m.as_str().to_string()).unwrap_or_default(),
                file: c[1].to_string(),
                line: c[2].parse().ok(),
                ..Default::default()
            };
            let e = cur.get_or_insert_with(|| {
                Exc::new(std::mem::take(&mut pending), String::new(), String::new())
            });
            e.frames.push(f);
            just_closed = false;
            continue;
        }

        if let Some(c) = rx!(r"^\[Previous line repeated (\d+) more times?\]$").captures(t) {
            if let Some(f) = cur.as_mut().and_then(|e| e.frames.last_mut()) {
                f.note = format!("previous frame repeated {} more time(s)", &c[1]);
            }
            continue;
        }

        let indented = raw.starts_with(' ') || raw.starts_with('\t');
        let anchor_only = t.chars().all(|ch| matches!(ch, '~' | '^' | ' '));

        // The source line Python prints under each frame.
        if indented && !anchor_only && !just_closed {
            if let Some(f) = cur.as_mut().and_then(|e| e.frames.last_mut()) {
                if f.context.is_empty() {
                    f.context = t.to_string();
                    continue;
                }
            }
        }
        if anchor_only {
            continue;
        }

        // A header at column 0 terminates the current traceback.
        if !indented {
            if let Some((ty, msg)) = split_exception_header(t) {
                let mut e = cur.take().unwrap_or_else(|| {
                    Exc::new(std::mem::take(&mut pending), String::new(), String::new())
                });
                e.exception = ty;
                e.message = msg;
                out.push(e);
                pending.clear();
                just_closed = true;
                continue;
            }
        }

        // Continuation of a multi-line exception message.
        if just_closed {
            if let Some(e) = out.last_mut() {
                if !e.message.is_empty() {
                    e.message.push('\n');
                }
                e.message.push_str(t);
            }
        }
    }
    if let Some(c) = cur.take() {
        out.push(c);
    }

    // Python prints outermost-first; normalise to innermost-first.
    for e in out.iter_mut() {
        e.frames.reverse();
    }
    reverse_chain(out)
}

/// Turn a printed-oldest-first chain (Python, PHP) into the reported-first
/// order the rest of the tool uses, re-attaching each marker as a relation on
/// the exception it points at.
fn reverse_chain(mut chain: Vec<Exc>) -> Vec<Exc> {
    if chain.len() < 2 {
        if let Some(e) = chain.first_mut() {
            e.relation.clear();
        }
        return chain;
    }
    let relations: Vec<String> = chain.iter().map(|e| e.relation.clone()).collect();
    for i in 0..chain.len() {
        chain[i].relation = match relations.get(i + 1).map(String::as_str) {
            Some("Direct cause") => "Caused by".to_string(),
            Some("During handling") => "Raised while handling".to_string(),
            Some("Next") => "Caused by".to_string(),
            Some(other) if !other.is_empty() => other.to_string(),
            _ => String::new(),
        };
    }
    chain.reverse();
    chain
}

// --- JavaScript / TypeScript ----------------------------------------------

fn parse_javascript(text: &str) -> Vec<Exc> {
    let mut out: Vec<Exc> = Vec::new();
    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if let Some(c) = rx!(r"^at\s+(?:(.+?)\s+\((.*)\)|(.*))$").captures(t) {
            let (func, loc) = match c.get(2) {
                Some(l) => (c.get(1).map(|m| m.as_str()).unwrap_or(""), l.as_str()),
                None => ("", c.get(3).map(|m| m.as_str()).unwrap_or("")),
            };
            let f = js_frame(func, loc);
            if let Some(e) = out.last_mut() {
                e.frames.push(f);
            } else {
                let mut e = Exc::new(String::new(), String::new(), String::new());
                e.frames.push(f);
                out.push(e);
            }
            continue;
        }

        let (relation, rest) = js_relation(t);
        if let Some((ty, msg)) = split_exception_header(rest) {
            out.push(Exc::new(relation, ty, msg));
            continue;
        }
        if let Some(e) = out.last_mut() {
            if e.frames.is_empty() && !e.exception.is_empty() {
                if !e.message.is_empty() {
                    e.message.push('\n');
                }
                e.message.push_str(t);
            }
        }
    }
    out
}

fn js_relation(t: &str) -> (String, &str) {
    for (prefix, label) in [
        ("Caused by:", "Caused by"),
        ("[cause]:", "Caused by"),
        ("Uncaught", ""),
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return (label.to_string(), rest.trim_start());
        }
    }
    (String::new(), t)
}

fn js_frame(func: &str, loc: &str) -> Frame {
    let mut f = Frame {
        function: func
            .trim()
            .trim_start_matches("async ")
            .trim_start_matches("new ")
            .to_string(),
        ..Default::default()
    };
    let l = loc.trim();
    if l.is_empty() {
        return f;
    }
    if let Some(c) = rx!(r"^(.*):(\d+):(\d+)$").captures(l) {
        f.file = c[1].to_string();
        f.line = c[2].parse().ok();
        f.column = c[3].parse().ok();
    } else if let Some(c) = rx!(r"^(.*):(\d+)$").captures(l) {
        f.file = c[1].to_string();
        f.line = c[2].parse().ok();
    } else if l == "<anonymous>" || l == "native" {
        f.note = l.to_string();
    } else {
        f.file = l.to_string();
    }
    f
}

// --- Go --------------------------------------------------------------------

fn parse_go(text: &str) -> Vec<Exc> {
    let mut e = Exc::new(String::new(), String::new(), String::new());
    let mut pending_func: Option<String> = None;
    let mut seen_header = false;

    let flush = |pending: &mut Option<String>, e: &mut Exc| {
        if let Some(func) = pending.take() {
            e.frames.push(Frame {
                function: func,
                ..Default::default()
            });
        }
    };

    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if !seen_header {
            if let Some(rest) = t.strip_prefix("panic: ") {
                e.exception = "panic".to_string();
                e.message = rest.to_string();
                seen_header = true;
                continue;
            }
            if let Some(rest) = t.strip_prefix("fatal error: ") {
                e.exception = "fatal error".to_string();
                e.message = rest.to_string();
                seen_header = true;
                continue;
            }
        }
        if rx!(r"^goroutine \d+ \[").is_match(t) {
            flush(&mut pending_func, &mut e);
            continue;
        }
        if t.starts_with("[signal ") || t.starts_with("exit status ") {
            continue;
        }

        // A tab-indented location line completes the pending function line.
        if raw.starts_with('\t') || raw.starts_with("    ") {
            if let Some(c) = rx!(r"^(.*?):(\d+)(?:\s+\+0x[0-9a-fA-F]+)?$").captures(t) {
                let func = pending_func.take().unwrap_or_default();
                e.frames.push(Frame {
                    function: func,
                    file: c[1].to_string(),
                    line: c[2].parse().ok(),
                    ..Default::default()
                });
                continue;
            }
        }

        if let Some(rest) = t.strip_prefix("created by ") {
            flush(&mut pending_func, &mut e);
            pending_func = Some(format!(
                "created by {}",
                rest.split(" in goroutine").next().unwrap_or(rest)
            ));
            continue;
        }

        if t.ends_with(')') && t.contains('(') {
            flush(&mut pending_func, &mut e);
            pending_func = Some(t.split('(').next().unwrap_or(t).to_string());
            continue;
        }

        if !seen_header && e.exception.is_empty() {
            if let Some((ty, msg)) = split_exception_header(t) {
                e.exception = ty;
                e.message = msg;
                seen_header = true;
            }
        }
    }
    flush(&mut pending_func, &mut e);
    vec![e]
}

// --- Ruby ------------------------------------------------------------------

fn parse_ruby(text: &str) -> Vec<Exc> {
    let mut e = Exc::new(String::new(), String::new(), String::new());
    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        // First line carries the location, the message and the class.
        if e.exception.is_empty() && !t.starts_with("from ") {
            if let Some(c) =
                rx!(r"^(.*?):(\d+):in [`'](.*?)':\s*([\s\S]*?)\s*\(([A-Z][\w:]*)\)$").captures(t)
            {
                e.exception = c[5].to_string();
                e.message = c[4].to_string();
                e.frames.push(Frame {
                    function: c[3].to_string(),
                    file: c[1].to_string(),
                    line: c[2].parse().ok(),
                    ..Default::default()
                });
                continue;
            }
        }

        if let Some(c) = rx!(r"^(?:from )?(.*?):(\d+):in [`'](.*?)'$").captures(t) {
            e.frames.push(Frame {
                function: c[3].to_string(),
                file: c[1].to_string(),
                line: c[2].parse().ok(),
                ..Default::default()
            });
            continue;
        }

        if e.exception.is_empty() {
            if let Some((ty, msg)) = split_exception_header(t) {
                e.exception = ty;
                e.message = msg;
            }
        }
    }
    vec![e]
}

// --- C# / .NET -------------------------------------------------------------

fn parse_csharp(text: &str) -> Vec<Exc> {
    let mut out: Vec<Exc> = Vec::new();
    // Frames after `--- End of inner exception stack trace ---` belong to the
    // enclosing exception again, so track the open exception as a stack.
    let mut open: Vec<usize> = Vec::new();

    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if t.contains("End of inner exception stack trace") {
            if open.len() > 1 {
                open.pop();
            }
            continue;
        }
        if t.starts_with("--- End of stack trace") {
            continue;
        }

        if let Some(c) = rx!(r"^at\s+(.+?)(?:\s+in\s+(.*):line\s+(\d+))?$").captures(t) {
            let f = Frame {
                function: c[1].trim().to_string(),
                file: c.get(2).map(|m| m.as_str().to_string()).unwrap_or_default(),
                line: c.get(3).and_then(|m| m.as_str().parse().ok()),
                ..Default::default()
            };
            match open.last() {
                Some(&i) => out[i].frames.push(f),
                None => {
                    let mut e = Exc::new(String::new(), String::new(), String::new());
                    e.frames.push(f);
                    out.push(e);
                    open.push(out.len() - 1);
                }
            }
            continue;
        }

        let inner = t.starts_with("--->");
        let rest = if inner {
            t.trim_start_matches("--->").trim_start()
        } else {
            t
        };
        if let Some((ty, msg)) = split_exception_header(rest) {
            let relation = if inner || !out.is_empty() {
                "Inner exception".to_string()
            } else {
                String::new()
            };
            out.push(Exc::new(relation, ty, msg));
            open.push(out.len() - 1);
            continue;
        }

        if let Some(&i) = open.last() {
            let e = &mut out[i];
            if e.frames.is_empty() && !e.exception.is_empty() {
                if !e.message.is_empty() {
                    e.message.push('\n');
                }
                e.message.push_str(t);
            }
        }
    }
    if let Some(e) = out.first_mut() {
        e.relation.clear();
    }
    out
}

// --- Rust ------------------------------------------------------------------

fn parse_rust(text: &str) -> Vec<Exc> {
    let mut e = Exc::new(String::new(), "panic".to_string(), String::new());
    let mut expect_message = false;
    let mut in_backtrace = false;

    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        // Rust 1.73+: "thread 'main' panicked at src/main.rs:4:5:" then message.
        if let Some(c) = rx!(r"^thread '(.*?)' panicked at (.*?):(\d+):(\d+):?$").captures(t) {
            e.message.clear();
            e.frames.insert(
                0,
                Frame {
                    function: format!("thread '{}'", &c[1]),
                    file: c[2].to_string(),
                    line: c[3].parse().ok(),
                    column: c[4].parse().ok(),
                    note: "panic site".to_string(),
                    ..Default::default()
                },
            );
            expect_message = true;
            continue;
        }
        // Older: thread 'main' panicked at 'msg', src/main.rs:4:5
        if let Some(c) =
            rx!(r"^thread '(.*?)' panicked at '([\s\S]*)', (.*?):(\d+):(\d+)$").captures(t)
        {
            e.message = c[2].to_string();
            e.frames.insert(
                0,
                Frame {
                    function: format!("thread '{}'", &c[1]),
                    file: c[3].to_string(),
                    line: c[4].parse().ok(),
                    column: c[5].parse().ok(),
                    note: "panic site".to_string(),
                    ..Default::default()
                },
            );
            expect_message = false;
            continue;
        }
        if t == "stack backtrace:" {
            in_backtrace = true;
            expect_message = false;
            continue;
        }
        if t.starts_with("note:") || t.starts_with("error:") {
            expect_message = false;
            continue;
        }

        if let Some(c) = rx!(r"^(\d+):\s+(.*)$").captures(t) {
            in_backtrace = true;
            e.frames.push(Frame {
                function: c[2].trim().to_string(),
                ..Default::default()
            });
            expect_message = false;
            continue;
        }
        if in_backtrace {
            if let Some(rest) = t.strip_prefix("at ") {
                if let Some(f) = e.frames.last_mut() {
                    if let Some(c) = rx!(r"^(.*?):(\d+)(?::(\d+))?$").captures(rest.trim()) {
                        f.file = c[1].to_string();
                        f.line = c[2].parse().ok();
                        f.column = c.get(3).and_then(|m| m.as_str().parse().ok());
                    } else {
                        f.file = rest.trim().to_string();
                    }
                }
                continue;
            }
        }

        if expect_message {
            if !e.message.is_empty() {
                e.message.push('\n');
            }
            e.message.push_str(t);
            continue;
        }
    }
    vec![e]
}

// --- PHP -------------------------------------------------------------------

fn parse_php(text: &str) -> Vec<Exc> {
    let mut out: Vec<Exc> = Vec::new();

    for raw in text.lines() {
        let t = raw.trim();
        if t.is_empty() {
            continue;
        }

        if let Some(c) =
            rx!(r"(?:Uncaught|Next)\s+([\w\\]+)(?::\s*([\s\S]*?))?\s+in\s+(.*):(\d+)$").captures(t)
        {
            let relation = if t.contains("Next ") && !out.is_empty() {
                "Next".to_string()
            } else {
                String::new()
            };
            let mut e = Exc::new(
                relation,
                c[1].to_string(),
                c.get(2).map(|m| m.as_str().to_string()).unwrap_or_default(),
            );
            e.frames.push(Frame {
                function: "(throw site)".to_string(),
                file: c[3].to_string(),
                line: c[4].parse().ok(),
                ..Default::default()
            });
            out.push(e);
            continue;
        }

        if t == "Stack trace:" || rx!(r"^thrown in .* on line \d+$").is_match(t) {
            continue;
        }

        // "#0 /app/file.php(20): doWork()" — or "#3 {main}".
        if let Some(c) = rx!(r"^#\d+\s+(.*?)\((\d+)\):\s*(.*)$").captures(t) {
            let f = Frame {
                function: c[3].trim().to_string(),
                file: c[1].to_string(),
                line: c[2].parse().ok(),
                ..Default::default()
            };
            push_php_frame(&mut out, f);
            continue;
        }
        if rx!(r"^#\d+\s+\{main\}$").is_match(t) {
            push_php_frame(
                &mut out,
                Frame {
                    function: "{main}".to_string(),
                    note: "entry point".to_string(),
                    ..Default::default()
                },
            );
            continue;
        }
        if let Some(c) = rx!(r"^#\d+\s+(.*)$").captures(t) {
            push_php_frame(
                &mut out,
                Frame {
                    function: c[1].trim().to_string(),
                    ..Default::default()
                },
            );
            continue;
        }

        if out.is_empty() {
            if let Some((ty, msg)) = split_exception_header(t) {
                out.push(Exc::new(String::new(), ty, msg));
            }
        }
    }

    // PHP prints the original exception first and "Next" wrappers after it.
    reverse_chain(out)
}

fn push_php_frame(out: &mut Vec<Exc>, f: Frame) {
    match out.last_mut() {
        Some(e) => e.frames.push(f),
        None => {
            let mut e = Exc::new(String::new(), String::new(), String::new());
            e.frames.push(f);
            out.push(e);
        }
    }
}

// ---------------------------------------------------------------------------
// user vs framework classification
// ---------------------------------------------------------------------------

/// Function-name prefixes that mark library / runtime code, per language.
fn framework_functions(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Java => &[
            "java.", "javax.", "jakarta.", "jdk.", "sun.", "com.sun.", "kotlin.", "kotlinx.",
            "scala.", "akka.", "groovy.", "org.springframework.", "org.apache.", "org.hibernate.",
            "org.junit.", "junit.", "org.testng.", "org.gradle.", "org.mockito.", "io.netty.",
            "io.micronaut.", "ch.qos.logback.", "org.slf4j.", "com.fasterxml.", "org.eclipse.",
            "net.bytebuddy.", "org.jboss.", "reactor.", "io.reactivex.", "com.google.common.",
            "com.zaxxer.", "org.postgresql.", "com.mysql.",
        ],
        Lang::Python => &["<frozen importlib", "<frozen runpy"],
        Lang::Javascript => &["Module.", "Object.Module.", "Function.Module."],
        Lang::Go => &[
            "runtime.",
            "net/http.",
            "net.",
            "os.",
            "sync.",
            "reflect.",
            "testing.",
            "internal/",
        ],
        Lang::Ruby => &["<internal:"],
        Lang::Csharp => &[
            "System.",
            "Microsoft.",
            "Newtonsoft.",
            "Xunit.",
            "NUnit.",
            "MS.Internal.",
            "Internal.",
            "Castle.",
            "Moq.",
        ],
        Lang::Rust => &[
            "core::",
            "std::",
            "alloc::",
            "rust_begin_unwind",
            "__rust_",
            "_start",
            "backtrace::",
            "tokio::",
            "futures_",
            "hyper::",
            "__libc_start",
            "main",
        ],
        Lang::Php => &[],
    }
}

/// File-path fragments that mark library / runtime code, per language.
fn framework_paths(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Java => &["/gradle/", "/.m2/"],
        Lang::Python => &[
            "site-packages",
            "dist-packages",
            "/lib/python",
            "\\lib\\site-packages",
            "<frozen ",
            "/usr/lib/python",
        ],
        Lang::Javascript => &["node_modules", "node:", "internal/", "webpack/bootstrap"],
        Lang::Go => &["/usr/local/go/src/", "/pkg/mod/", "/go/src/runtime/", "GOROOT"],
        Lang::Ruby => &["/gems/", "/ruby/", "<internal:"],
        Lang::Csharp => &["\\Microsoft\\", "/dotnet/shared/"],
        Lang::Rust => &["/rustc/", "library/std", "library/core", "/.cargo/registry/"],
        Lang::Php => &["/vendor/", "\\vendor\\"],
    }
}

fn classify(lang: Lang, f: &Frame, user_prefixes: &[String]) -> Kind {
    // Java 9+ prints `java.base/java.lang.Thread.run`; classify on the part
    // after the module name.
    let func = match (lang, f.function.split_once('/')) {
        (Lang::Java, Some((module, rest))) if module.contains('.') && !rest.is_empty() => rest,
        _ => f.function.as_str(),
    };

    if !user_prefixes.is_empty() {
        // Explicit allow-list: only what the caller named is user code.
        let hit = user_prefixes
            .iter()
            .any(|p| func.contains(p.as_str()) || f.file.contains(p.as_str()));
        return if hit { Kind::User } else { Kind::Framework };
    }

    if framework_functions(lang).iter().any(|p| func.starts_with(p))
        || framework_paths(lang).iter().any(|p| f.file.contains(p))
    {
        return Kind::Framework;
    }
    Kind::User
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const JAVA: &str = "Exception in thread \"main\" com.example.SvcException: could not start\n\tat com.example.App.start(App.java:42)\n\tat org.springframework.boot.SpringApplication.run(SpringApplication.java:301)\n\tat java.base/java.lang.Thread.run(Thread.java:840)\nCaused by: java.net.ConnectException: Connection refused\n\tat java.base/sun.nio.ch.Net.pollConnect(Native Method)\n\tat com.example.Db.connect(Db.java:17)\n\t... 12 more";

    const PY: &str = "Traceback (most recent call last):\n  File \"/app/main.py\", line 10, in <module>\n    main()\n  File \"/app/svc.py\", line 4, in divide\n    return a / b\nZeroDivisionError: division by zero";

    const JS: &str = "TypeError: Cannot read properties of undefined (reading 'name')\n    at getName (/app/src/user.js:12:18)\n    at /app/src/index.js:4:3\n    at Module._compile (node:internal/modules/cjs/loader:1105:14)\n    at Array.forEach (<anonymous>)";

    const GO: &str = "panic: runtime error: index out of range [3] with length 3\n\ngoroutine 1 [running]:\nmain.doWork(0x0?)\n\t/home/u/app/main.go:12 +0x1d\nmain.main()\n\t/home/u/app/main.go:6 +0x18";

    // --- happy paths -------------------------------------------------------

    #[test]
    fn java_chain_root_cause_and_first_user_frame() {
        let out = analyze(JAVA, "auto", "report", "", false, false, 0).unwrap();
        assert!(out.contains("Language: Java / Kotlin / Scala (auto-detected)"));
        assert!(out.contains("Reported: com.example.SvcException: could not start"));
        assert!(out.contains("Root cause: java.net.ConnectException: Connection refused"));
        // The root cause is searched first, so Db.connect wins over App.start.
        assert!(out.contains("First user frame: com.example.Db.connect(Db.java:17)"));
        assert!(out.contains("Caused by: java.net.ConnectException"));
        assert!(out.contains("12 frame(s) elided by the runtime"));
        // Spring + the java.base module frame are framework; ours are marked.
        assert!(out.contains("*  com.example.App.start(App.java:42)"));
        assert!(out.contains("   org.springframework.boot.SpringApplication.run"));
        assert!(out.contains("Frames: 5 total, 2 user, 3 framework, 5 shown"));
    }

    #[test]
    fn java_native_method_and_module_prefix() {
        let out = analyze(JAVA, "java", "json", "", false, false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let root = &v["chain"][1]["frames"];
        assert_eq!(root[0]["function"], "java.base/sun.nio.ch.Net.pollConnect");
        assert_eq!(root[0]["kind"], "framework");
        assert_eq!(root[0]["note"], "Native Method");
        assert_eq!(root[1]["kind"], "user");
        assert_eq!(root[1]["line"], 17);
        assert_eq!(v["root_cause"]["type"], "java.net.ConnectException");
        assert_eq!(v["counts"]["exceptions"], 2);
    }

    #[test]
    fn python_is_reversed_to_innermost_first() {
        let out = analyze(PY, "auto", "json", "", false, false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["language"], "python");
        assert_eq!(v["reported"]["type"], "ZeroDivisionError");
        assert_eq!(v["reported"]["message"], "division by zero");
        let frames = &v["chain"][0]["frames"];
        // Printed outermost-first; we show the throw site first.
        assert_eq!(frames[0]["function"], "divide");
        assert_eq!(frames[0]["line"], 4);
        assert_eq!(frames[0]["source"], "return a / b");
        assert_eq!(frames[1]["function"], "<module>");
    }

    #[test]
    fn python_cause_chain_is_reported_first() {
        let text = "Traceback (most recent call last):\n  File \"a.py\", line 2, in load\n    raise ValueError('bad row')\nValueError: bad row\n\nThe above exception was the direct cause of the following exception:\n\nTraceback (most recent call last):\n  File \"a.py\", line 9, in <module>\n    load()\nRuntimeError: import failed";
        let out = analyze(text, "python", "report", "", false, false, 0).unwrap();
        assert!(out.contains("Reported: RuntimeError: import failed"));
        assert!(out.contains("Root cause: ValueError: bad row"));
        assert!(out.contains("[2] Caused by: ValueError: bad row"));
    }

    #[test]
    fn javascript_columns_and_node_internals() {
        let out = analyze(JS, "auto", "json", "", false, false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["language"], "javascript");
        assert_eq!(v["reported"]["type"], "TypeError");
        let f = &v["chain"][0]["frames"];
        assert_eq!(f[0]["function"], "getName");
        assert_eq!(f[0]["column"], 18);
        assert_eq!(f[0]["kind"], "user");
        assert_eq!(f[1]["function"], "");
        assert_eq!(f[1]["text"], "<anonymous>(/app/src/index.js:4:3)");
        assert_eq!(f[2]["kind"], "framework"); // node:internal
        assert_eq!(f[3]["note"], "<anonymous>");
    }

    #[test]
    fn go_pairs_function_with_location() {
        let out = analyze(GO, "auto", "report", "", false, false, 0).unwrap();
        assert!(out.contains("Language: Go (auto-detected)"));
        assert!(out.contains("Reported: panic: runtime error: index out of range [3] with length 3"));
        assert!(out.contains("*  main.doWork(/home/u/app/main.go:12)"));
        assert!(out.contains("*  main.main(/home/u/app/main.go:6)"));
    }

    #[test]
    fn ruby_first_line_carries_class_and_message() {
        let text =
            "/app/svc.rb:4:in `divide': divided by 0 (ZeroDivisionError)\n\tfrom /app/main.rb:9:in `<main>'";
        let out = analyze(text, "auto", "json", "", false, false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["language"], "ruby");
        assert_eq!(v["reported"]["type"], "ZeroDivisionError");
        assert_eq!(v["reported"]["message"], "divided by 0");
        assert_eq!(v["chain"][0]["frames"][0]["function"], "divide");
        assert_eq!(v["chain"][0]["frames"][1]["file"], "/app/main.rb");
    }

    #[test]
    fn ruby_modern_quote_style_and_gem_frames() {
        let text = "app.rb:4:in 'Integer#/': divided by 0 (ZeroDivisionError)\n\tfrom /usr/lib/ruby/gems/3.3.0/rails.rb:10:in 'call'";
        let out = analyze(text, "ruby", "json", "", false, false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["chain"][0]["frames"][0]["function"], "Integer#/");
        assert_eq!(v["chain"][0]["frames"][1]["kind"], "framework");
    }

    #[test]
    fn csharp_inner_exception_becomes_root_cause() {
        let text = "System.InvalidOperationException: Operation failed\n ---> System.IO.FileNotFoundException: Could not find file 'x.txt'.\n   at System.IO.FileStream..ctor(String path)\n   at MyApp.Store.Read(String p) in C:\\src\\MyApp\\Store.cs:line 12\n   --- End of inner exception stack trace ---\n   at MyApp.Service.Load(String path) in C:\\src\\MyApp\\Service.cs:line 42\n   at MyApp.Program.Main()";
        let out = analyze(text, "auto", "report", "", false, false, 0).unwrap();
        assert!(out.contains("Language: C# / .NET (auto-detected)"));
        assert!(out.contains("Reported: System.InvalidOperationException: Operation failed"));
        assert!(out.contains("Root cause: System.IO.FileNotFoundException"));
        assert!(out.contains("First user frame: MyApp.Store.Read(String p)(C:\\src\\MyApp\\Store.cs:12)"));
        assert!(out.contains("*  MyApp.Service.Load(String path)(C:\\src\\MyApp\\Service.cs:42)"));
    }

    #[test]
    fn rust_panic_site_and_backtrace() {
        let text = "thread 'main' panicked at src/main.rs:4:5:\nattempt to divide by zero\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\nstack backtrace:\n   0: rust_begin_unwind\n             at /rustc/abc/library/std/src/panicking.rs:665:5\n   5: myapp::divide\n             at ./src/main.rs:4:5";
        let out = analyze(text, "auto", "json", "", false, false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["language"], "rust");
        assert_eq!(v["reported"]["message"], "attempt to divide by zero");
        assert_eq!(v["chain"][0]["frames"][0]["note"], "panic site");
        assert_eq!(v["chain"][0]["frames"][1]["kind"], "framework");
        assert_eq!(v["chain"][0]["frames"][2]["function"], "myapp::divide");
        assert_eq!(v["chain"][0]["frames"][2]["kind"], "user");
        assert_eq!(v["chain"][0]["frames"][2]["column"], 5);
    }

    #[test]
    fn php_next_chain_and_vendor_frames() {
        let text = "PHP Fatal error:  Uncaught PDOException: connection refused in /app/Db.php:31\nStack trace:\n#0 /app/vendor/orm/Conn.php(88): PDO->__construct()\n#1 /app/Service.php(12): Orm\\Conn->open()\n#2 {main}\n  thrown in /app/Db.php on line 31\nNext RuntimeException: startup failed in /app/Boot.php:9\nStack trace:\n#0 /app/Boot.php(9): Service->boot()\n#1 {main}";
        let out = analyze(text, "auto", "report", "", false, false, 0).unwrap();
        assert!(out.contains("Language: PHP (auto-detected)"));
        assert!(out.contains("Reported: RuntimeException: startup failed"));
        assert!(out.contains("Root cause: PDOException: connection refused"));
        assert!(out.contains("[2] Caused by: PDOException: connection refused"));
        // /app/vendor/ is framework, /app/Service.php is ours.
        let json = analyze(text, "php", "json", "", false, false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["chain"][1]["frames"][1]["kind"], "framework");
        assert_eq!(v["chain"][1]["frames"][2]["kind"], "user");
    }

    // --- options -----------------------------------------------------------

    #[test]
    fn hide_framework_keeps_only_user_frames() {
        let out = analyze(JAVA, "java", "report", "", true, false, 0).unwrap();
        assert!(out.contains("*  com.example.App.start(App.java:42)"));
        assert!(!out.contains("org.springframework"));
        assert!(out.contains("2 framework frame(s) hidden"));
        assert!(out.contains("1 framework frame(s) hidden"));
    }

    #[test]
    fn reverse_flips_frame_order() {
        let out = analyze(JAVA, "java", "report", "", false, true, 0).unwrap();
        let first = out
            .lines()
            .find(|l| l.trim_start().starts_with("1. "))
            .unwrap();
        assert!(first.contains("java.base/java.lang.Thread.run"));
        assert!(out.contains("Frames listed outermost (entry point) first."));
    }

    #[test]
    fn user_packages_is_an_explicit_allow_list() {
        // Spring becomes "user" and our own code becomes "framework".
        let out = analyze(JAVA, "java", "json", "org.springframework", false, false, 0).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["chain"][0]["frames"][0]["kind"], "framework");
        assert_eq!(v["chain"][0]["frames"][1]["kind"], "user");
        assert_eq!(v["counts"]["user"], 1);
    }

    #[test]
    fn limit_truncates_per_exception() {
        let out = analyze(JAVA, "java", "report", "", false, false, 1).unwrap();
        assert!(out.contains("2 more frame(s) not shown"));
        assert!(out.contains("1 more frame(s) not shown"));
        assert!(out.contains("Frames: 5 total, 2 user, 3 framework, 2 shown"));
    }

    #[test]
    fn table_output_has_a_row_per_frame() {
        let out = analyze(PY, "python", "table", "", false, false, 0).unwrap();
        assert!(out.contains("| # | Code | Function | File | Line | Col |"));
        assert!(out.contains("| 1 | user | divide | /app/svc.py | 4 | — |"));
        assert!(out.contains("### [1] ZeroDivisionError: division by zero"));
    }

    #[test]
    fn explicit_language_overrides_detection() {
        // A Java trace read as JavaScript still parses its `at` lines.
        let out = analyze(JAVA, "javascript", "report", "", false, false, 0).unwrap();
        assert!(out.contains("Language: JavaScript / TypeScript (as selected)"));
    }

    #[test]
    fn single_exception_reports_itself_as_the_root() {
        let out = analyze(PY, "python", "report", "", false, false, 0).unwrap();
        assert!(out.contains("Root cause: same as reported (single exception)"));
    }

    // --- errors ------------------------------------------------------------

    #[test]
    fn empty_input_is_an_error() {
        let err = analyze("   \n ", "auto", "report", "", false, false, 0).unwrap_err();
        assert!(err.contains("no stack trace provided"));
    }

    #[test]
    fn unknown_language_is_an_error() {
        let err = analyze(JAVA, "cobol", "report", "", false, false, 0).unwrap_err();
        assert!(err.contains("unknown language 'cobol'"));
        assert!(err.contains("javascript"));
    }

    #[test]
    fn unknown_output_is_an_error() {
        let err = analyze(JAVA, "auto", "yaml", "", false, false, 0).unwrap_err();
        assert!(err.contains("unknown output 'yaml'"));
    }

    #[test]
    fn undetectable_input_is_an_error() {
        let err = analyze("hello world\njust some prose", "auto", "report", "", false, false, 0)
            .unwrap_err();
        assert!(err.contains("could not detect the language"));
    }

    #[test]
    fn oversized_input_is_an_error() {
        let big = "at a.b.C.d(C.java:1)\n".repeat(MAX_INPUT / 10);
        let err = analyze(&big, "java", "report", "", false, false, 0).unwrap_err();
        assert!(err.contains("too large"));
        assert!(err.contains("200000"));
    }

    #[test]
    fn recognised_language_with_no_frames_is_an_error() {
        let err = analyze("goroutine 1 [running]:", "auto", "report", "", false, false, 0)
            .unwrap_err();
        assert!(err.contains("no frames or exception header found"));
        assert!(err.contains("Go"));
    }

    #[test]
    fn limit_is_clamped_to_the_maximum() {
        // Above MAX_LIMIT is clamped, not rejected.
        let out = analyze(JAVA, "java", "report", "", false, false, 99_999).unwrap();
        assert!(out.contains("Frames: 5 total, 2 user, 3 framework, 5 shown"));
    }

    #[test]
    fn detect_prefers_the_strongest_signal() {
        assert_eq!(detect(JAVA), Some(Lang::Java));
        assert_eq!(detect(PY), Some(Lang::Python));
        assert_eq!(detect(JS), Some(Lang::Javascript));
        assert_eq!(detect(GO), Some(Lang::Go));
        assert_eq!(detect("nothing to see"), None);
    }
}
