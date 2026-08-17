//! markdown-runbook-extractor core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Turns a Markdown *runbook* (a how-to doc whose steps are fenced code blocks)
//! into ONE runnable script plus an ordered task list. Each fenced block becomes
//! a named, numbered step; the name comes from the fence's info string
//! (`bash "Install deps"`, `bash name=install-deps`, `{.bash #install-deps}`),
//! else the nearest preceding heading, else a preceding bold label, else
//! `step-N`. Shell prompts (`$ `, `>>> `, `PS>`) are stripped and pasted command
//! output is dropped, so copied README sessions become executable.

use std::collections::BTreeMap;

/// Hard cap on the input document (characters). Keeps a pasted mega-doc from
/// pinning the browser tab.
pub const MAX_INPUT_CHARS: usize = 1_000_000;
/// Hard cap on how many matching code blocks one runbook may contribute.
pub const MAX_STEPS: usize = 500;
/// Longest step name kept (longer names are truncated with an ellipsis).
const MAX_NAME_CHARS: usize = 120;

/// Info-string tags that mark a block as "documentation only, do not run".
const SKIP_TAGS: &[&str] = &[
    "skip",
    "noexec",
    "no-exec",
    "norun",
    "no-run",
    "dontrun",
    "dont-run",
    "ignore",
    "example",
    "output",
];

/// Which family of fenced blocks to collect, and which script to emit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Language {
    /// Pick the family with the most blocks in the document.
    Auto,
    Shell,
    Python,
    PowerShell,
    JavaScript,
    /// Every fenced block that carries any language tag.
    Any,
}

impl Language {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Language::Auto),
            "shell" => Ok(Language::Shell),
            "python" => Ok(Language::Python),
            "powershell" => Ok(Language::PowerShell),
            "javascript" => Ok(Language::JavaScript),
            "any" => Ok(Language::Any),
            other => Err(format!(
                "expected language to be auto, shell, python, powershell, javascript or any, got '{other}'"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Language::Auto => "auto",
            Language::Shell => "shell",
            Language::Python => "python",
            Language::PowerShell => "powershell",
            Language::JavaScript => "javascript",
            Language::Any => "any",
        }
    }

    /// The concrete families `auto` may resolve to, in tie-break order.
    fn concrete() -> [Language; 4] {
        [
            Language::Shell,
            Language::Python,
            Language::PowerShell,
            Language::JavaScript,
        ]
    }

    fn comment(self) -> &'static str {
        match self {
            Language::JavaScript => "//",
            _ => "#",
        }
    }

    fn shebang(self) -> Option<&'static str> {
        match self {
            Language::Shell | Language::Any | Language::Auto => Some("#!/usr/bin/env bash"),
            Language::Python => Some("#!/usr/bin/env python3"),
            Language::JavaScript => Some("#!/usr/bin/env node"),
            Language::PowerShell => None,
        }
    }

    fn fail_fast_line(self) -> Option<&'static str> {
        match self {
            Language::Shell | Language::Any | Language::Auto => Some("set -euo pipefail"),
            Language::PowerShell => Some("$ErrorActionPreference = 'Stop'"),
            // Python and Node already abort on an unhandled error.
            Language::Python | Language::JavaScript => None,
        }
    }

    /// Render a progress line announcing `text` in this language.
    fn echo(self, text: &str) -> String {
        match self {
            Language::Python => format!("print(\"{}\")", escape(text, '\\', &['\\', '"'])),
            Language::JavaScript => {
                format!("console.log(\"{}\")", escape(text, '\\', &['\\', '"']))
            }
            // PowerShell escapes inside a double-quoted string with a backtick.
            Language::PowerShell => {
                format!("Write-Host \"{}\"", escape(text, '`', &['`', '"', '$']))
            }
            _ => format!("echo \"{}\"", escape(text, '\\', &['\\', '"', '$', '`'])),
        }
    }
}

/// What to render from the extracted steps.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OutputFormat {
    /// One runnable script whose header lists the ordered tasks.
    Script,
    /// A Markdown checklist of the ordered tasks.
    Tasks,
    /// Structured `{language, count, runnable, steps: [...]}`.
    Json,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "script" => Ok(OutputFormat::Script),
            "tasks" => Ok(OutputFormat::Tasks),
            "json" => Ok(OutputFormat::Json),
            other => Err(format!(
                "expected output to be script, tasks or json, got '{other}'"
            )),
        }
    }
}

/// All knobs, mirroring the descriptor params one-for-one.
#[derive(Clone, Debug)]
pub struct Options {
    pub language: Language,
    pub output: OutputFormat,
    /// Comma/space separated tag filter; a leading `-` or `!` excludes.
    pub tags: String,
    pub strip_prompts: bool,
    pub echo_steps: bool,
    pub fail_fast: bool,
    pub skip_marked: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            language: Language::Auto,
            output: OutputFormat::Script,
            tags: String::new(),
            strip_prompts: true,
            echo_steps: true,
            fail_fast: true,
            skip_marked: true,
        }
    }
}

/// One extracted, named task.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// 1-based position in the runbook.
    pub index: usize,
    pub name: String,
    /// The fence's own language tag, verbatim (`bash`, `console`, `py`…).
    pub language: String,
    /// 1-based source line of the opening fence.
    pub line: usize,
    pub tags: Vec<String>,
    /// True when the block is tagged do-not-run and `skip_marked` is on.
    pub skipped: bool,
    /// Which tag caused the skip.
    pub skip_reason: Option<String>,
    /// The code, after optional prompt stripping.
    pub code: String,
}

// ---------------------------------------------------------------------------
// fence scanning
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct RawBlock {
    info: String,
    code: Vec<String>,
    line: usize,
    heading: Option<String>,
    label: Option<String>,
}

/// Leading indent (spaces) of `line`, counting at most `max`.
fn indent_of(line: &str, max: usize) -> Option<usize> {
    let n = line.len() - line.trim_start_matches(' ').len();
    if n <= max {
        Some(n)
    } else {
        None
    }
}

/// Recognize a CommonMark fence opener: ≤3 spaces of indent then 3+ backticks
/// or tildes. Returns `(indent, fence_char, fence_len, info_string)`.
fn fence_open(line: &str) -> Option<(usize, char, usize, String)> {
    let indent = indent_of(line, 3)?;
    let rest = &line[indent..];
    let ch = rest.chars().next()?;
    if ch != '`' && ch != '~' {
        return None;
    }
    let len = rest.chars().take_while(|&c| c == ch).count();
    if len < 3 {
        return None;
    }
    let info: String = rest.chars().skip(len).collect();
    // A backtick fence's info string may not contain a backtick (CommonMark).
    if ch == '`' && info.contains('`') {
        return None;
    }
    Some((indent, ch, len, info.trim().to_string()))
}

/// A closing fence: same char, at least as long, nothing else on the line.
fn fence_closes(line: &str, ch: char, len: usize) -> bool {
    let Some(indent) = indent_of(line, 3) else {
        return false;
    };
    let rest = &line[indent..];
    let run = rest.chars().take_while(|&c| c == ch).count();
    run >= len && rest.chars().skip(run).all(|c| c == ' ' || c == '\t')
}

/// ATX heading text (`## Deploy` → `Deploy`), if this line is one.
fn heading_text(line: &str) -> Option<String> {
    let indent = indent_of(line, 3)?;
    let rest = &line[indent..];
    let hashes = rest.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let after: String = rest.chars().skip(hashes).collect();
    if !after.is_empty() && !after.starts_with(' ') && !after.starts_with('\t') {
        return None; // `#hashtag`, not a heading
    }
    let text = after.trim().trim_end_matches('#').trim().to_string();
    Some(text)
}

/// A standalone bold line (`**Install dependencies**`) used as a step label.
fn bold_label(line: &str) -> Option<String> {
    let t = line.trim();
    for delim in ["**", "__"] {
        if t.len() > 2 * delim.len() && t.starts_with(delim) && t.ends_with(delim) {
            let inner = &t[delim.len()..t.len() - delim.len()];
            if !inner.is_empty() && !inner.contains(delim) {
                return Some(inner.trim().trim_end_matches(':').trim().to_string());
            }
        }
    }
    None
}

fn scan_blocks(markdown: &str) -> Vec<RawBlock> {
    let mut out = Vec::new();
    let mut heading: Option<String> = None;
    let mut last_meaningful: Option<String> = None;
    let mut lines = markdown.lines().enumerate().peekable();

    while let Some((idx, line)) = lines.next() {
        if let Some((indent, ch, len, info)) = fence_open(line) {
            let mut code = Vec::new();
            for (_, body) in lines.by_ref() {
                if fence_closes(body, ch, len) {
                    break;
                }
                // CommonMark strips up to the opening fence's indent.
                let strip = indent_of(body, indent).unwrap_or(indent).min(indent);
                code.push(body[strip..].to_string());
            }
            out.push(RawBlock {
                info,
                code,
                line: idx + 1,
                heading: heading.clone(),
                label: last_meaningful.as_deref().and_then(bold_label),
            });
            last_meaningful = None;
            continue;
        }
        if let Some(h) = heading_text(line) {
            heading = Some(h);
            last_meaningful = None;
            continue;
        }
        if !line.trim().is_empty() {
            last_meaningful = Some(line.to_string());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// info-string parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct Info {
    lang: String,
    name: Option<String>,
    tags: Vec<String>,
}

/// Split on whitespace while keeping `"quoted values"` (and `key="a b"`) whole.
fn tokenize_info(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    for c in s.chars() {
        match quote {
            Some(q) if c == q => {
                quote = None;
                cur.push(c);
            }
            Some(_) => cur.push(c),
            None if c == '"' || c == '\'' => {
                quote = Some(c);
                cur.push(c);
            }
            None if c.is_whitespace() || c == ',' => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            None => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn unquote(s: &str) -> String {
    let t = s.trim();
    for q in ['"', '\''] {
        if t.len() >= 2 && t.starts_with(q) && t.ends_with(q) {
            return t[1..t.len() - 1].to_string();
        }
    }
    t.to_string()
}

/// Parse every info-string dialect we support into `(lang, name, tags)`:
/// `bash "Install deps"` · `bash name=install-deps` · `bash title="Install deps"`
/// · `{.bash #install-deps .skip}` · `bash#deploy#v2` · `bash skip`.
fn parse_info(info: &str) -> Info {
    let mut s = info.trim();
    if s.starts_with('{') && s.ends_with('}') && s.len() >= 2 {
        s = s[1..s.len() - 1].trim();
    }
    let mut out = Info::default();
    for (i, tok) in tokenize_info(s).iter().enumerate() {
        let quoted = tok.starts_with('"') || tok.starts_with('\'');
        if i == 0 && !quoted {
            // `bash`, `.bash`, or rundoc's `bash#tag1#tag2`.
            let head = tok.trim_start_matches('.');
            let mut parts = head.split('#');
            out.lang = parts.next().unwrap_or("").to_ascii_lowercase();
            out.tags
                .extend(parts.filter(|p| !p.is_empty()).map(|p| p.to_ascii_lowercase()));
            continue;
        }
        if quoted {
            if out.name.is_none() {
                out.name = Some(unquote(tok));
            }
            continue;
        }
        if let Some((k, v)) = tok.split_once('=') {
            let key = k.trim_start_matches('.').to_ascii_lowercase();
            if matches!(key.as_str(), "name" | "title" | "id" | "label") {
                if out.name.is_none() {
                    out.name = Some(unquote(v));
                }
            } else {
                out.tags.push(key);
            }
            continue;
        }
        if let Some(rest) = tok.strip_prefix('#') {
            // Pandoc/Entangled block id — a name if we don't have one yet.
            if out.name.is_none() && !rest.is_empty() {
                out.name = Some(rest.to_string());
            } else if !rest.is_empty() {
                out.tags.push(rest.to_ascii_lowercase());
            }
            continue;
        }
        let bare = tok.trim_start_matches('.').to_ascii_lowercase();
        if !bare.is_empty() {
            out.tags.push(bare);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// language families
// ---------------------------------------------------------------------------

fn family_of(lang: &str) -> Option<Language> {
    match lang {
        "bash" | "sh" | "zsh" | "ksh" | "shell" | "console" | "shell-session" | "shellsession"
        | "sh-session" | "bash-session" | "terminal" | "command" | "shell-script" => {
            Some(Language::Shell)
        }
        "python" | "py" | "python3" | "py3" | "pycon" | "python-repl" | "doctest" => {
            Some(Language::Python)
        }
        "powershell" | "pwsh" | "ps1" | "ps" | "posh" | "ps1con" => Some(Language::PowerShell),
        "javascript" | "js" | "node" | "nodejs" | "mjs" | "cjs" | "jsx" => {
            Some(Language::JavaScript)
        }
        _ => None,
    }
}

/// Blocks whose tag means "a transcript with output interleaved".
fn is_session_lang(lang: &str) -> bool {
    matches!(
        lang,
        "console"
            | "shell-session"
            | "shellsession"
            | "sh-session"
            | "bash-session"
            | "terminal"
            | "pycon"
            | "python-repl"
            | "doctest"
            | "ps1con"
    )
}

// ---------------------------------------------------------------------------
// prompt stripping
// ---------------------------------------------------------------------------

/// Strip a leading interactive prompt from `line`, returning the command.
/// `session` widens the set (a bare `# ` is a root prompt there, not a comment).
fn strip_prompt(line: &str, family: Language, session: bool) -> Option<String> {
    let t = line.trim_start();
    if family == Language::Python {
        for p in [">>> ", "... "] {
            if let Some(rest) = t.strip_prefix(p) {
                return Some(rest.to_string());
            }
        }
        if t == ">>>" || t == "..." {
            return Some(String::new());
        }
        return None;
    }
    // PowerShell / cmd style: `PS C:\src> cmd`, `PS> cmd`, `C:\src> cmd`.
    if t.starts_with("PS ") || t.starts_with("PS>") || t.starts_with("PS C:") {
        if let Some(pos) = t.find('>') {
            return Some(t[pos + 1..].trim_start().to_string());
        }
    }
    for p in ["$ ", "% ", "❯ ", "➜ "] {
        if let Some(rest) = t.strip_prefix(p) {
            return Some(rest.to_string());
        }
    }
    if t == "$" || t == "%" {
        return Some(String::new());
    }
    if session {
        if let Some(rest) = t.strip_prefix("# ") {
            return Some(rest.to_string());
        }
        if let Some(rest) = t.strip_prefix("> ") {
            return Some(rest.to_string());
        }
    }
    None
}

/// Turn a pasted terminal session into runnable commands: if ANY line carries a
/// prompt, keep only prompted lines (the rest is command *output*); otherwise
/// the block is already plain code and is returned verbatim.
fn strip_prompts(code: &[String], lang: &str, family: Language) -> Vec<String> {
    let session = is_session_lang(lang);
    let any_prompt = code
        .iter()
        .any(|l| !l.trim().is_empty() && strip_prompt(l, family, session).is_some());
    if !any_prompt {
        return code.to_vec();
    }
    let mut out: Vec<String> = Vec::new();
    let mut continuing = false;
    for l in code {
        if let Some(cmd) = strip_prompt(l, family, session) {
            continuing = cmd.trim_end().ends_with('\\');
            out.push(cmd);
        } else if continuing {
            // A backslash continuation line belongs to the previous command.
            continuing = l.trim_end().ends_with('\\');
            out.push(l.clone());
        }
    }
    out
}

// ---------------------------------------------------------------------------
// naming
// ---------------------------------------------------------------------------

/// Flatten inline Markdown in a heading/label into a plain step name.
fn clean_name(raw: &str) -> String {
    let mut s = raw.replace(['\n', '\r', '\t'], " ");
    // `[text](url)` → `text`
    while let Some(open) = s.find('[') {
        let Some(close_rel) = s[open..].find("](") else {
            break;
        };
        let close = open + close_rel;
        let Some(end_rel) = s[close..].find(')') else {
            break;
        };
        let end = close + end_rel;
        let text = s[open + 1..close].to_string();
        s.replace_range(open..=end, &text);
    }
    let s = s.replace("**", "").replace("__", "").replace(['`', '*'], "");
    let mut name = s.split_whitespace().collect::<Vec<_>>().join(" ");
    name = name.trim().trim_end_matches(':').trim().to_string();
    if name.chars().count() > MAX_NAME_CHARS {
        name = name.chars().take(MAX_NAME_CHARS - 1).collect::<String>() + "…";
    }
    name
}

// ---------------------------------------------------------------------------
// tag filtering
// ---------------------------------------------------------------------------

struct TagFilter {
    include: Vec<String>,
    exclude: Vec<String>,
}

impl TagFilter {
    fn parse(spec: &str) -> Self {
        let mut include = Vec::new();
        let mut exclude = Vec::new();
        for raw in spec.split([',', ' ', '\t', '\n']) {
            let t = raw.trim();
            if t.is_empty() {
                continue;
            }
            if let Some(rest) = t.strip_prefix('-').or_else(|| t.strip_prefix('!')) {
                if !rest.is_empty() {
                    exclude.push(rest.to_ascii_lowercase());
                }
            } else {
                include.push(t.to_ascii_lowercase());
            }
        }
        TagFilter { include, exclude }
    }

    fn is_empty(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty()
    }

    fn allows(&self, tags: &[String]) -> bool {
        if self.exclude.iter().any(|x| tags.iter().any(|t| t == x)) {
            return false;
        }
        self.include.is_empty() || self.include.iter().any(|x| tags.iter().any(|t| t == x))
    }
}

// ---------------------------------------------------------------------------
// extraction
// ---------------------------------------------------------------------------

/// Parse `markdown` into the ordered, named steps `opts` selects.
/// Returns the resolved concrete language alongside them.
pub fn extract_steps(markdown: &str, opts: &Options) -> Result<(Language, Vec<Step>), String> {
    if markdown.trim().is_empty() {
        return Err("expected a Markdown runbook, got an empty document".to_string());
    }
    let n = markdown.chars().count();
    if n > MAX_INPUT_CHARS {
        return Err(format!(
            "runbook is too large: {n} characters (limit {MAX_INPUT_CHARS}) — split the document and extract each part"
        ));
    }

    let raw = scan_blocks(markdown);
    if raw.is_empty() {
        return Err(
            "no fenced code blocks found — a runbook's steps must be inside ``` fences".to_string(),
        );
    }

    // Parse every block once, then decide which family we are extracting.
    let parsed: Vec<(&RawBlock, Info)> = raw
        .iter()
        .map(|b| {
            let info = parse_info(&b.info);
            (b, info)
        })
        .collect();

    let mut counts: BTreeMap<&'static str, usize> = BTreeMap::new();
    for (_, info) in &parsed {
        if let Some(f) = family_of(&info.lang) {
            *counts.entry(f.label()).or_insert(0) += 1;
        }
    }

    let resolved = match opts.language {
        Language::Auto => {
            // Most blocks wins; ties break in `concrete()` order (shell first).
            let mut best: Option<(Language, usize)> = None;
            for f in Language::concrete() {
                let c = counts.get(f.label()).copied().unwrap_or(0);
                if c > 0 && best.map(|(_, b)| c > b).unwrap_or(true) {
                    best = Some((f, c));
                }
            }
            match best.map(|(f, _)| f) {
                Some(f) => f,
                None => {
                    let seen = tagged_languages(&parsed);
                    return Err(format!(
                        "no runnable code blocks found — none of the fenced blocks are tagged with a shell, Python, PowerShell or JavaScript language.{} Tag your fences (```bash) or set language to any.",
                        seen
                    ));
                }
            }
        }
        other => other,
    };

    let filter = TagFilter::parse(&opts.tags);
    let mut steps: Vec<Step> = Vec::new();
    let mut all_tags: Vec<String> = Vec::new();
    let mut family_matched = 0usize;

    for (block, info) in &parsed {
        let matches_family = match resolved {
            Language::Any => !info.lang.is_empty(),
            want => family_of(&info.lang) == Some(want),
        };
        if !matches_family {
            continue;
        }
        family_matched += 1;
        for t in &info.tags {
            if !all_tags.contains(t) {
                all_tags.push(t.clone());
            }
        }
        if !filter.allows(&info.tags) {
            continue;
        }
        if steps.len() >= MAX_STEPS {
            return Err(format!(
                "runbook has more than {MAX_STEPS} matching code blocks — narrow it with tags or split the document"
            ));
        }

        let family = family_of(&info.lang).unwrap_or(resolved);
        let code_lines = if opts.strip_prompts {
            strip_prompts(&block.code, &info.lang, family)
        } else {
            block.code.clone()
        };
        let code = code_lines.join("\n").trim_end().to_string();
        if code.trim().is_empty() {
            continue; // an empty fence is not a task
        }

        let skip_reason = info
            .tags
            .iter()
            .find(|t| SKIP_TAGS.contains(&t.as_str()))
            .cloned();
        let index = steps.len() + 1;
        let name = info
            .name
            .as_deref()
            .map(clean_name)
            .filter(|s| !s.is_empty())
            .or_else(|| block.label.as_deref().map(clean_name).filter(|s| !s.is_empty()))
            .or_else(|| {
                block
                    .heading
                    .as_deref()
                    .map(clean_name)
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or_else(|| format!("step-{index}"));

        steps.push(Step {
            index,
            name,
            language: info.lang.clone(),
            line: block.line,
            tags: info.tags.clone(),
            skipped: opts.skip_marked && skip_reason.is_some(),
            skip_reason: if opts.skip_marked { skip_reason } else { None },
            code,
        });
    }

    if steps.is_empty() {
        if family_matched > 0 && !filter.is_empty() {
            let present = if all_tags.is_empty() {
                " No tags are present in this runbook.".to_string()
            } else {
                format!(" Tags present: {}.", all_tags.join(", "))
            };
            return Err(format!(
                "no code blocks matched tags '{}' — {family_matched} block(s) matched the language filter.{present}",
                opts.tags.trim()
            ));
        }
        let seen = tagged_languages(&parsed);
        return Err(format!(
            "no {} code blocks found in the runbook.{} Tag your fences with a language (```bash) or set language to any.",
            resolved.label(),
            seen
        ));
    }

    Ok((resolved, steps))
}

/// " Languages tagged in this document: yaml, json." — for actionable errors.
fn tagged_languages(parsed: &[(&RawBlock, Info)]) -> String {
    let mut seen: Vec<&str> = Vec::new();
    for (_, info) in parsed {
        if !info.lang.is_empty() && !seen.contains(&info.lang.as_str()) {
            seen.push(&info.lang);
        }
    }
    if seen.is_empty() {
        " Every fence in this document is untagged.".to_string()
    } else {
        format!(" Languages tagged in this document: {}.", seen.join(", "))
    }
}

/// Full pipeline: Markdown runbook → script / task list / JSON.
pub fn extract(markdown: &str, opts: &Options) -> Result<String, String> {
    let (resolved, steps) = extract_steps(markdown, opts)?;
    Ok(match opts.output {
        OutputFormat::Script => render_script(resolved, &steps, opts),
        OutputFormat::Tasks => render_tasks(&steps),
        OutputFormat::Json => render_json(resolved, &steps),
    })
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

/// Prefix each char of `s` that appears in `specials` with `esc`.
fn escape(s: &str, esc: char, specials: &[char]) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if specials.contains(&c) {
            out.push(esc);
        }
        out.push(c);
    }
    out
}

fn render_script(lang: Language, steps: &[Step], opts: &Options) -> String {
    let cs = lang.comment();
    let total = steps.len();
    let runnable = steps.iter().filter(|s| !s.skipped).count();
    let mut out: Vec<String> = Vec::new();

    if let Some(sb) = lang.shebang() {
        out.push(sb.to_string());
    }
    out.push(format!(
        "{cs} Runbook: {runnable} runnable task(s) of {total} extracted from Markdown."
    ));
    out.push(format!("{cs} Tasks:"));
    for s in steps {
        let mark = match (&s.skipped, &s.skip_reason) {
            (true, Some(r)) => format!("  [skipped: tagged {r}]"),
            (true, None) => "  [skipped]".to_string(),
            _ => String::new(),
        };
        out.push(format!("{cs}   {}. {}{mark}", s.index, s.name));
    }
    out.push(String::new());
    if opts.fail_fast {
        if let Some(ff) = lang.fail_fast_line() {
            out.push(ff.to_string());
            out.push(String::new());
        }
    }

    for s in steps {
        let suffix = match (&s.skipped, &s.skip_reason) {
            (true, Some(r)) => format!(" — SKIPPED, tagged {r}"),
            (true, None) => " — SKIPPED".to_string(),
            _ => String::new(),
        };
        out.push(format!(
            "{cs} --- {}/{} · {} ({}, line {}){suffix} ---",
            s.index, total, s.name, s.language, s.line
        ));
        if s.skipped {
            for l in s.code.lines() {
                if l.trim().is_empty() {
                    out.push(cs.to_string());
                } else {
                    out.push(format!("{cs} {l}"));
                }
            }
        } else {
            if opts.echo_steps {
                out.push(lang.echo(&format!("==> [{}/{}] {}", s.index, total, s.name)));
            }
            for l in s.code.lines() {
                out.push(l.to_string());
            }
        }
        out.push(String::new());
    }

    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
}

fn render_tasks(steps: &[Step]) -> String {
    let total = steps.len();
    let runnable = steps.iter().filter(|s| !s.skipped).count();
    let mut out = vec![
        format!("# Runbook tasks ({runnable} runnable of {total})"),
        String::new(),
    ];
    for s in steps {
        let lines = s.code.lines().count();
        let plural = if lines == 1 { "line" } else { "lines" };
        let name = if s.skipped {
            format!("~~{}~~", s.name)
        } else {
            s.name.clone()
        };
        let mut detail = format!("`{}`, {lines} {plural}, line {}", s.language, s.line);
        if !s.tags.is_empty() {
            detail.push_str(&format!(", tags: {}", s.tags.join(" ")));
        }
        if let Some(r) = &s.skip_reason {
            detail.push_str(&format!(" — skipped, tagged {r}"));
        }
        out.push(format!("- [ ] {}. {name} — {detail}", s.index));
    }
    out.join("\n")
}

fn render_json(lang: Language, steps: &[Step]) -> String {
    let items: Vec<serde_json::Value> = steps
        .iter()
        .map(|s| {
            serde_json::json!({
                "index": s.index,
                "name": s.name,
                "language": s.language,
                "line": s.line,
                "tags": s.tags,
                "skipped": s.skipped,
                "skip_reason": s.skip_reason,
                "lines": s.code.lines().count(),
                "code": s.code,
            })
        })
        .collect();
    let doc = serde_json::json!({
        "language": lang.label(),
        "count": steps.len(),
        "runnable": steps.iter().filter(|s| !s.skipped).count(),
        "steps": items,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const RUNBOOK: &str = r#"# Deploy the API

Some prose that is not a step.

## Install dependencies

```bash
npm ci
```

## Run migrations

```bash name=migrate
./manage.py migrate
```

## Rollback (do not run)

```bash skip
./manage.py migrate --rollback
```
"#;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn happy_path_builds_a_runnable_script_with_a_task_manifest() {
        let out = extract(RUNBOOK, &opts()).unwrap();
        assert!(out.starts_with("#!/usr/bin/env bash\n"), "{out}");
        assert!(out.contains("# Runbook: 2 runnable task(s) of 3 extracted from Markdown."));
        assert!(out.contains("#   1. Install dependencies"));
        assert!(out.contains("#   2. migrate"));
        assert!(out.contains("#   3. Rollback (do not run)  [skipped: tagged skip]"));
        assert!(out.contains("set -euo pipefail"));
        assert!(out.contains("echo \"==> [1/3] Install dependencies\""));
        assert!(out.contains("\nnpm ci\n"));
        // The skipped step is commented out, never silently dropped.
        assert!(out.contains("# ./manage.py migrate --rollback"));
        assert!(!out.contains("\n./manage.py migrate --rollback"));
    }

    #[test]
    fn error_on_empty_document() {
        let err = extract("   \n\n", &opts()).unwrap_err();
        assert_eq!(err, "expected a Markdown runbook, got an empty document");
    }

    #[test]
    fn error_lists_the_languages_actually_present() {
        let md = "# Config\n\n```yaml\nkey: value\n```\n";
        let err = extract(md, &opts()).unwrap_err();
        assert!(err.contains("no runnable code blocks found"), "{err}");
        assert!(err.contains("Languages tagged in this document: yaml."), "{err}");
    }

    #[test]
    fn error_when_no_fences_at_all() {
        let err = extract("# Just prose\n\nNothing here.\n", &opts()).unwrap_err();
        assert!(err.contains("no fenced code blocks found"), "{err}");
    }

    #[test]
    fn names_come_from_every_supported_info_string_dialect() {
        let md = concat!(
            "```bash \"Install deps\"\na\n```\n\n",
            "```bash title=\"Run tests\"\nb\n```\n\n",
            "```{.bash #deploy-api}\nc\n```\n\n",
            "```bash id=cleanup\nd\n```\n",
        );
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        let names: Vec<_> = steps.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["Install deps", "Run tests", "deploy-api", "cleanup"]);
    }

    #[test]
    fn name_falls_back_to_bold_label_then_heading_then_step_n() {
        let md = concat!(
            "## Section heading\n\n**Bold label**\n\n```bash\na\n```\n\n",
            "```bash\nb\n```\n",
        );
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        assert_eq!(steps[0].name, "Bold label");
        assert_eq!(steps[1].name, "Section heading");

        let (_, anon) = extract_steps("```bash\na\n```\n", &opts()).unwrap();
        assert_eq!(anon[0].name, "step-1");
    }

    #[test]
    fn heading_markdown_is_flattened_into_the_name() {
        let md = "## Run `make build` for [the API](https://x.test)\n\n```bash\nmake build\n```\n";
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        assert_eq!(steps[0].name, "Run make build for the API");
    }

    #[test]
    fn console_prompts_are_stripped_and_output_lines_dropped() {
        let md = "```console\n$ npm ci\nadded 42 packages\n$ npm test\nok\n```\n";
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        assert_eq!(steps[0].code, "npm ci\nnpm test");
    }

    #[test]
    fn a_plain_block_without_prompts_is_kept_verbatim() {
        let md = "```bash\n# a real comment\nnpm ci\n\nnpm test\n```\n";
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        assert_eq!(steps[0].code, "# a real comment\nnpm ci\n\nnpm test");
    }

    #[test]
    fn backslash_continuations_survive_prompt_stripping() {
        let md = "```console\n$ curl \\\n  --silent https://x.test\nHTTP 200\n```\n";
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        assert_eq!(steps[0].code, "curl \\\n  --silent https://x.test");
    }

    #[test]
    fn strip_prompts_off_keeps_the_transcript_verbatim() {
        let md = "```console\n$ npm ci\nadded 42 packages\n```\n";
        let o = Options {
            strip_prompts: false,
            ..opts()
        };
        let (_, steps) = extract_steps(md, &o).unwrap();
        assert_eq!(steps[0].code, "$ npm ci\nadded 42 packages");
    }

    #[test]
    fn auto_picks_the_most_common_family_and_python_gets_its_own_shebang() {
        let md = "```bash\na\n```\n\n```python\nx = 1\n```\n\n```python\ny = 2\n```\n";
        let out = extract(md, &opts()).unwrap();
        assert!(out.starts_with("#!/usr/bin/env python3\n"), "{out}");
        assert!(out.contains("print(\"==> [1/2] step-1\")"), "{out}");
        assert!(!out.contains("set -euo pipefail"));
    }

    #[test]
    fn doctest_prompts_are_stripped_for_python() {
        let md = "```pycon\n>>> import os\n>>> os.getcwd()\n'/tmp'\n```\n";
        let o = Options {
            language: Language::Python,
            ..opts()
        };
        let (_, steps) = extract_steps(md, &o).unwrap();
        assert_eq!(steps[0].code, "import os\nos.getcwd()");
    }

    #[test]
    fn powershell_gets_its_own_header_and_prompt_handling() {
        let md = "```powershell\nPS C:\\src> Get-ChildItem\nPS C:\\src> Restart-Service api\n```\n";
        let o = Options {
            language: Language::PowerShell,
            ..opts()
        };
        let out = extract(md, &o).unwrap();
        assert!(out.contains("$ErrorActionPreference = 'Stop'"), "{out}");
        assert!(out.contains("Write-Host \"==> [1/1] step-1\""), "{out}");
        assert!(out.contains("\nGet-ChildItem\n"), "{out}");
        assert!(!out.contains("#!"), "PowerShell scripts get no shebang: {out}");
    }

    #[test]
    fn tag_filters_include_and_exclude() {
        let md = concat!(
            "```bash#deploy\na\n```\n\n",
            "```bash#test\nb\n```\n\n",
            "```bash deploy slow\nc\n```\n",
        );
        let inc = Options {
            tags: "deploy".into(),
            ..opts()
        };
        let (_, steps) = extract_steps(md, &inc).unwrap();
        assert_eq!(steps.iter().map(|s| s.code.as_str()).collect::<Vec<_>>(), ["a", "c"]);

        let exc = Options {
            tags: "-slow".into(),
            ..opts()
        };
        let (_, steps) = extract_steps(md, &exc).unwrap();
        assert_eq!(steps.iter().map(|s| s.code.as_str()).collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn error_when_a_tag_filter_matches_nothing_lists_available_tags() {
        let md = "```bash#deploy\na\n```\n";
        let o = Options {
            tags: "nope".into(),
            ..opts()
        };
        let err = extract(md, &o).unwrap_err();
        assert!(err.contains("no code blocks matched tags 'nope'"), "{err}");
        assert!(err.contains("Tags present: deploy."), "{err}");
    }

    #[test]
    fn skip_marked_off_makes_marked_blocks_runnable() {
        let o = Options {
            skip_marked: false,
            ..opts()
        };
        let out = extract(RUNBOOK, &o).unwrap();
        assert!(out.contains("# Runbook: 3 runnable task(s) of 3 extracted from Markdown."));
        assert!(out.contains("\n./manage.py migrate --rollback"), "{out}");
    }

    #[test]
    fn echo_and_fail_fast_can_be_turned_off() {
        let o = Options {
            echo_steps: false,
            fail_fast: false,
            ..opts()
        };
        let out = extract(RUNBOOK, &o).unwrap();
        assert!(!out.contains("set -euo pipefail"), "{out}");
        assert!(!out.contains("echo \"==>"), "{out}");
    }

    #[test]
    fn task_list_output_is_a_markdown_checklist() {
        let o = Options {
            output: OutputFormat::Tasks,
            ..opts()
        };
        let out = extract(RUNBOOK, &o).unwrap();
        assert_eq!(
            out,
            "# Runbook tasks (2 runnable of 3)\n\n\
             - [ ] 1. Install dependencies — `bash`, 1 line, line 7\n\
             - [ ] 2. migrate — `bash`, 1 line, line 13\n\
             - [ ] 3. ~~Rollback (do not run)~~ — `bash`, 1 line, line 19, tags: skip — skipped, tagged skip"
        );
    }

    #[test]
    fn json_output_carries_every_step_field() {
        let o = Options {
            output: OutputFormat::Json,
            ..opts()
        };
        let out = extract(RUNBOOK, &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["language"], "shell");
        assert_eq!(v["count"], 3);
        assert_eq!(v["runnable"], 2);
        assert_eq!(v["steps"][1]["name"], "migrate");
        assert_eq!(v["steps"][1]["line"], 13);
        assert_eq!(v["steps"][2]["skipped"], true);
        assert_eq!(v["steps"][2]["skip_reason"], "skip");
    }

    #[test]
    fn any_language_collects_every_tagged_fence() {
        let md = "```bash\na\n```\n\n```sql\nSELECT 1;\n```\n\n```\nuntagged\n```\n";
        let o = Options {
            language: Language::Any,
            output: OutputFormat::Tasks,
            ..opts()
        };
        let (_, steps) = extract_steps(md, &o).unwrap();
        assert_eq!(steps.len(), 2, "untagged fences are never steps: {steps:?}");
        assert_eq!(steps[1].language, "sql");
    }

    #[test]
    fn a_longer_outer_fence_can_contain_a_fence() {
        let md = "````bash\ncat <<'EOF' > a.md\n```\ninner\n```\nEOF\n````\n";
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        assert_eq!(steps.len(), 1);
        assert!(steps[0].code.contains("inner"), "{:?}", steps[0].code);
    }

    #[test]
    fn tilde_fences_and_indented_fences_work() {
        let md = "  ~~~bash\n  npm ci\n  ~~~\n";
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        assert_eq!(steps[0].code, "npm ci");
    }

    #[test]
    fn names_with_shell_metacharacters_are_escaped_in_the_echo() {
        let md = "## Set $HOME and \"quote\"\n\n```bash\ntrue\n```\n";
        let out = extract(md, &opts()).unwrap();
        assert!(out.contains(r#"echo "==> [1/1] Set \$HOME and \"quote\"""#), "{out}");
    }

    #[test]
    fn empty_fences_are_not_steps() {
        let md = "```bash\n\n```\n\n```bash\nnpm ci\n```\n";
        let (_, steps) = extract_steps(md, &opts()).unwrap();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].index, 1);
    }

    #[test]
    fn oversized_input_is_rejected_with_the_limit() {
        let md = format!("```bash\n{}\n```\n", "x".repeat(MAX_INPUT_CHARS + 1));
        let err = extract(&md, &opts()).unwrap_err();
        assert!(err.contains("runbook is too large"), "{err}");
        assert!(err.contains("limit 1000000"), "{err}");
    }

    #[test]
    fn too_many_blocks_is_rejected_with_the_limit() {
        let md = "```bash\na\n```\n".repeat(MAX_STEPS + 1);
        let err = extract(&md, &opts()).unwrap_err();
        assert!(err.contains("more than 500 matching code blocks"), "{err}");
    }

    #[test]
    fn parse_rejects_unknown_enum_values_with_the_allowed_set() {
        let err = Language::parse("perl").unwrap_err();
        assert!(err.contains("expected language to be auto, shell"), "{err}");
        assert!(err.contains("got 'perl'"), "{err}");
        let err = OutputFormat::parse("yaml").unwrap_err();
        assert!(err.contains("expected output to be script, tasks or json"), "{err}");
    }
}
