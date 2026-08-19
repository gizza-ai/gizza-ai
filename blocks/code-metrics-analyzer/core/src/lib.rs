//! code-metrics-analyzer core — approximate source metrics for pasted code.
//!
//! This is intentionally a dependency-light lexer/heuristic analyzer, not a full AST parser:
//! it gives stable LOC, comment, function, and complexity estimates across common languages in
//! every gizza surface, including wasm.

use serde_json::json;

const MAX_SOURCE_CHARS: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    Auto,
    C,
    Cpp,
    Csharp,
    Go,
    Java,
    Javascript,
    Typescript,
    Kotlin,
    Lua,
    Php,
    Python,
    Ruby,
    Rust,
    Scala,
    Shell,
    Sql,
    Swift,
}

impl Language {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace(['_', ' '], "-").as_str() {
            "" | "auto" => Ok(Self::Auto),
            "c" => Ok(Self::C),
            "cpp" | "c++" => Ok(Self::Cpp),
            "csharp" | "c#" | "cs" => Ok(Self::Csharp),
            "go" | "golang" => Ok(Self::Go),
            "java" => Ok(Self::Java),
            "javascript" | "js" => Ok(Self::Javascript),
            "typescript" | "ts" => Ok(Self::Typescript),
            "kotlin" | "kt" => Ok(Self::Kotlin),
            "lua" => Ok(Self::Lua),
            "php" => Ok(Self::Php),
            "python" | "py" => Ok(Self::Python),
            "ruby" | "rb" => Ok(Self::Ruby),
            "rust" | "rs" => Ok(Self::Rust),
            "scala" => Ok(Self::Scala),
            "shell" | "sh" | "bash" => Ok(Self::Shell),
            "sql" => Ok(Self::Sql),
            "swift" => Ok(Self::Swift),
            other => Err(format!(
                "unknown language '{other}' (use auto, c, cpp, csharp, go, java, javascript, typescript, kotlin, lua, php, python, ruby, rust, scala, shell, sql, or swift)"
            )),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::C => "c",
            Self::Cpp => "cpp",
            Self::Csharp => "csharp",
            Self::Go => "go",
            Self::Java => "java",
            Self::Javascript => "javascript",
            Self::Typescript => "typescript",
            Self::Kotlin => "kotlin",
            Self::Lua => "lua",
            Self::Php => "php",
            Self::Python => "python",
            Self::Ruby => "ruby",
            Self::Rust => "rust",
            Self::Scala => "scala",
            Self::Shell => "shell",
            Self::Sql => "sql",
            Self::Swift => "swift",
        }
    }

    fn line_comment_prefixes(self) -> &'static [&'static str] {
        match self {
            Self::Python | Self::Ruby | Self::Shell => &["#"],
            Self::Sql => &["--"],
            Self::Lua => &["--"],
            _ => &["//"],
        }
    }

    fn has_block_comments(self) -> bool {
        !matches!(self, Self::Python | Self::Ruby | Self::Shell)
    }

    fn uses_indent(self) -> bool {
        matches!(self, Self::Python | Self::Ruby | Self::Shell)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Summary,
    Functions,
    Json,
    Csv,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "summary" | "text" => Ok(Self::Summary),
            "functions" => Ok(Self::Functions),
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            other => Err(format!(
                "unknown output '{other}' (use summary, functions, json, or csv)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    Line,
    Complexity,
    Cognitive,
    Length,
    Name,
}

impl SortBy {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "line" => Ok(Self::Line),
            "complexity" => Ok(Self::Complexity),
            "cognitive" => Ok(Self::Cognitive),
            "length" => Ok(Self::Length),
            "name" => Ok(Self::Name),
            other => Err(format!(
                "unknown sort '{other}' (use line, complexity, cognitive, length, or name)"
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionMetric {
    pub name: String,
    pub line: usize,
    pub end_line: usize,
    pub length_lines: usize,
    pub nloc: usize,
    pub cyclomatic: u32,
    pub cognitive: u32,
    pub params: usize,
    pub max_nesting: u32,
}

impl FunctionMetric {
    fn risk(&self) -> &'static str {
        match self.cyclomatic {
            0..=5 => "low",
            6..=10 => "moderate",
            11..=20 => "high",
            _ => "very-high",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Report {
    pub language: Language,
    pub language_detected: bool,
    pub total_lines: usize,
    pub code_lines: usize,
    pub blank_lines: usize,
    pub comment_lines: usize,
    pub max_line_length: usize,
    pub average_line_length: f64,
    pub function_count: usize,
    pub cyclomatic_total: u32,
    pub cyclomatic_average: f64,
    pub cyclomatic_max: u32,
    pub cognitive_total: u32,
    pub maintainability_index: f64,
    pub maintainability_grade: char,
    pub halstead_volume: f64,
    pub over_threshold_count: usize,
    pub functions: Vec<FunctionMetric>,
    pub warnings: Vec<String>,
}

pub fn run(source: &str) -> Result<String, String> {
    run_with_options(source, "auto", "summary", 10, 50, "line")
}

pub fn run_with_options(
    source: &str,
    language: &str,
    output: &str,
    complexity_threshold: u32,
    max_functions: usize,
    sort: &str,
) -> Result<String, String> {
    let requested = Language::parse(language)?;
    let output = Output::parse(output)?;
    let sort = SortBy::parse(sort)?;
    if source.trim().is_empty() {
        return Err("no source code given: paste one source file or snippet to analyze".into());
    }
    if source.chars().count() > MAX_SOURCE_CHARS {
        return Err(format!(
            "source is too large ({} characters); limit is {MAX_SOURCE_CHARS}",
            source.chars().count()
        ));
    }
    let threshold = complexity_threshold.max(1);
    let mut report = analyze(source, requested, threshold);
    sort_functions(&mut report.functions, sort);
    render(&report, output, max_functions, threshold)
}

fn analyze(source: &str, requested: Language, threshold: u32) -> Report {
    let language_detected = requested == Language::Auto;
    let language = if language_detected {
        detect_language(source)
    } else {
        requested
    };
    let raw_lines: Vec<&str> = source.lines().collect();
    let total_lines = raw_lines.len();
    let max_line_length = raw_lines
        .iter()
        .map(|l| l.chars().count())
        .max()
        .unwrap_or(0);
    let average_line_length = if total_lines == 0 {
        0.0
    } else {
        raw_lines.iter().map(|l| l.chars().count()).sum::<usize>() as f64 / total_lines as f64
    };

    let stripped = strip_comments(&raw_lines, language);
    let blank_lines = raw_lines.iter().filter(|l| l.trim().is_empty()).count();
    let comment_lines = stripped
        .iter()
        .filter(|l| !l.original_blank && l.had_comment && l.code.trim().is_empty())
        .count();
    let code_lines = stripped
        .iter()
        .filter(|l| !l.code.trim().is_empty())
        .count();

    let starts = find_functions(&stripped, language);
    let mut functions = Vec::new();
    for (idx, start) in starts.iter().enumerate() {
        let end_line = starts
            .get(idx + 1)
            .map(|s| s.line.saturating_sub(1))
            .unwrap_or(total_lines);
        let span = &stripped[start.line - 1..end_line];
        let nloc = span.iter().filter(|l| !l.code.trim().is_empty()).count();
        let cyclomatic = 1 + decision_points(span, language);
        let max_nesting = nesting_depth(span, language);
        let cognitive = cognitive_complexity(span, language, max_nesting);
        functions.push(FunctionMetric {
            name: start.name.clone(),
            line: start.line,
            end_line,
            length_lines: end_line.saturating_sub(start.line) + 1,
            nloc,
            cyclomatic,
            cognitive,
            params: start.params,
            max_nesting,
        });
    }

    let file_decisions = decision_points(&stripped, language);
    let cyclomatic_total = if functions.is_empty() {
        1 + file_decisions
    } else {
        functions.iter().map(|f| f.cyclomatic).sum()
    };
    let cyclomatic_max = functions
        .iter()
        .map(|f| f.cyclomatic)
        .max()
        .unwrap_or(cyclomatic_total);
    let cyclomatic_average = if functions.is_empty() {
        cyclomatic_total as f64
    } else {
        cyclomatic_total as f64 / functions.len() as f64
    };
    let cognitive_total = if functions.is_empty() {
        file_decisions
    } else {
        functions.iter().map(|f| f.cognitive).sum()
    };
    let halstead_volume = halstead_volume(&stripped);
    let maintainability_index =
        maintainability_index(halstead_volume, cyclomatic_total, code_lines);
    let maintainability_grade = grade(maintainability_index);
    let over_threshold_count = functions
        .iter()
        .filter(|f| f.cyclomatic > threshold)
        .count();
    let mut warnings = Vec::new();
    if language_detected {
        warnings.push(format!("language auto-detected as {}", language.as_str()));
    }
    if functions.is_empty() {
        warnings.push("no function declarations matched the heuristic patterns; file-level complexity is still reported".into());
    }

    Report {
        language,
        language_detected,
        total_lines,
        code_lines,
        blank_lines,
        comment_lines,
        max_line_length,
        average_line_length,
        function_count: functions.len(),
        cyclomatic_total,
        cyclomatic_average,
        cyclomatic_max,
        cognitive_total,
        maintainability_index,
        maintainability_grade,
        halstead_volume,
        over_threshold_count,
        functions,
        warnings,
    }
}

#[derive(Debug, Clone)]
struct CleanLine {
    code: String,
    had_comment: bool,
    original_blank: bool,
}

fn strip_comments(lines: &[&str], lang: Language) -> Vec<CleanLine> {
    let prefixes = lang.line_comment_prefixes();
    let mut in_block = false;
    let mut out = Vec::with_capacity(lines.len());
    for line in lines {
        let mut s = (*line).to_string();
        let original_blank = s.trim().is_empty();
        let mut had_comment = false;
        if lang.has_block_comments() {
            let mut cleaned = String::new();
            let bytes = s.as_bytes();
            let mut i = 0;
            while i < bytes.len() {
                if in_block {
                    had_comment = true;
                    if i + 1 < bytes.len() && bytes[i] == b'*' && bytes[i + 1] == b'/' {
                        in_block = false;
                        i += 2;
                    } else {
                        i += 1;
                    }
                } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'*' {
                    had_comment = true;
                    in_block = true;
                    i += 2;
                } else {
                    cleaned.push(bytes[i] as char);
                    i += 1;
                }
            }
            s = cleaned;
        }
        let cut = prefixes
            .iter()
            .filter_map(|p| find_prefix_outside_string(&s, p))
            .min();
        if let Some(i) = cut {
            had_comment = true;
            s.truncate(i);
        }
        out.push(CleanLine {
            code: s,
            had_comment,
            original_blank,
        });
    }
    out
}

fn find_prefix_outside_string(s: &str, prefix: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let p = prefix.as_bytes();
    let mut quote: Option<u8> = None;
    let mut escape = false;
    let mut i = 0;
    while i + p.len() <= bytes.len() {
        let b = bytes[i];
        if let Some(q) = quote {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == q {
                quote = None;
            }
            i += 1;
            continue;
        }
        if b == b'\'' || b == b'"' || b == b'`' {
            quote = Some(b);
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(p) {
            return Some(i);
        }
        i += 1;
    }
    None
}

#[derive(Debug, Clone)]
struct FunctionStart {
    name: String,
    line: usize,
    params: usize,
}

fn find_functions(lines: &[CleanLine], lang: Language) -> Vec<FunctionStart> {
    let mut out = Vec::new();
    for (idx, line) in lines.iter().enumerate() {
        let l = line.code.trim();
        if l.is_empty() {
            continue;
        }
        if let Some((name, params)) = detect_function(l, lang) {
            out.push(FunctionStart {
                name,
                line: idx + 1,
                params,
            });
        }
    }
    out
}

fn detect_function(line: &str, lang: Language) -> Option<(String, usize)> {
    match lang {
        Language::Python => after_keyword(line, "def "),
        Language::Ruby => after_keyword(line, "def "),
        Language::Rust => after_keyword(line, "fn "),
        Language::Go => detect_go_func(line),
        Language::Javascript | Language::Typescript => {
            after_keyword(line, "function ").or_else(|| detect_assignment_arrow(line))
        }
        Language::Shell => detect_shell_func(line),
        Language::Sql => detect_sql_func(line),
        _ => detect_c_family_func(line),
    }
}

fn after_keyword(line: &str, kw: &str) -> Option<(String, usize)> {
    let pos = line.find(kw)? + kw.len();
    let rest = &line[pos..];
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '$' || *c == '.')
        .collect();
    if name.is_empty() {
        return None;
    }
    Some((name, count_params(rest)))
}

fn detect_go_func(line: &str) -> Option<(String, usize)> {
    let rest = line.strip_prefix("func ")?;
    let rest = if rest.starts_with('(') {
        let close = rest.find(')')?;
        rest[close + 1..].trim_start()
    } else {
        rest
    };
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some((name, count_params(rest)))
    }
}

fn detect_assignment_arrow(line: &str) -> Option<(String, usize)> {
    if !line.contains("=>")
        || !(line.contains("const ") || line.contains("let ") || line.contains("var "))
    {
        return None;
    }
    let eq = line.find('=')?;
    let left = line[..eq].split_whitespace().last()?.trim();
    let name = left.trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '$');
    if name.is_empty() {
        None
    } else {
        Some((name.to_string(), count_params(&line[eq + 1..])))
    }
}

fn detect_shell_func(line: &str) -> Option<(String, usize)> {
    let before = line.find("()")?;
    let name = line[..before].trim();
    if name.is_empty() || name.contains(' ') {
        None
    } else {
        Some((name.to_string(), 0))
    }
}

fn detect_sql_func(line: &str) -> Option<(String, usize)> {
    let lower = line.to_ascii_lowercase();
    let marker = "create function ";
    let pos = lower.find(marker)? + marker.len();
    let rest = &line[pos..];
    let name: String = rest
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '.')
        .collect();
    if name.is_empty() {
        None
    } else {
        Some((name, count_params(rest)))
    }
}

fn detect_c_family_func(line: &str) -> Option<(String, usize)> {
    if !line.contains('(') || line.ends_with(';') {
        return None;
    }
    let lower = line.to_ascii_lowercase();
    let controls = [
        "if", "for", "while", "switch", "catch", "return", "new", "throw",
    ];
    let paren = line.find('(')?;
    let before = line[..paren].trim_end();
    let name = before
        .split_whitespace()
        .last()?
        .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '~');
    if name.is_empty() || controls.contains(&name.to_ascii_lowercase().as_str()) {
        return None;
    }
    if lower.contains(" class ") || lower.starts_with("class ") || lower.starts_with("namespace ") {
        return None;
    }
    Some((name.to_string(), count_params(line)))
}

fn count_params(s: &str) -> usize {
    let Some(open) = s.find('(') else {
        return 0;
    };
    let mut depth = 0_i32;
    let mut start = open + 1;
    let mut count = 0;
    let bytes = s.as_bytes();
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] as char {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    let param = s[start..i].trim();
                    if !param.is_empty() {
                        count += 1;
                    }
                    return count;
                }
            }
            ',' if depth == 1 => {
                if !s[start..i].trim().is_empty() {
                    count += 1;
                }
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    0
}

fn decision_points(lines: &[CleanLine], _lang: Language) -> u32 {
    let mut n = 0;
    for line in lines {
        let l = line.code.as_str();
        let words = words(l);
        for w in words {
            if matches!(
                w.as_str(),
                "if" | "elif"
                    | "for"
                    | "while"
                    | "case"
                    | "catch"
                    | "except"
                    | "match"
                    | "when"
                    | "guard"
            ) {
                n += 1;
            }
        }
        n += l.matches("&&").count() as u32;
        n += l.matches("||").count() as u32;
        n += l.matches('?').count() as u32;
    }
    n
}

fn nesting_depth(lines: &[CleanLine], lang: Language) -> u32 {
    if lang.uses_indent() {
        return lines
            .iter()
            .filter(|l| !l.code.trim().is_empty())
            .map(|l| l.code.chars().take_while(|c| *c == ' ').count() as u32 / 4)
            .max()
            .unwrap_or(0);
    }
    let mut depth = 0_i32;
    let mut max_depth = 0_i32;
    for line in lines {
        for c in line.code.chars() {
            match c {
                '{' => {
                    depth += 1;
                    max_depth = max_depth.max(depth);
                }
                '}' => depth = (depth - 1).max(0),
                _ => {}
            }
        }
    }
    max_depth as u32
}

fn cognitive_complexity(lines: &[CleanLine], lang: Language, max_nesting: u32) -> u32 {
    let decisions = decision_points(lines, lang);
    decisions + max_nesting.min(20)
}

fn words(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            cur.push(c.to_ascii_lowercase());
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn halstead_volume(lines: &[CleanLine]) -> f64 {
    let mut operators: Vec<String> = Vec::new();
    let mut operands: Vec<String> = Vec::new();
    for line in lines {
        for token in lexical_tokens(&line.code) {
            if is_operator(&token) {
                operators.push(token);
            } else if token.chars().any(|c| c.is_ascii_alphanumeric()) {
                operands.push(token);
            }
        }
    }
    let n1 = unique_count(&operators) as f64;
    let n2 = unique_count(&operands) as f64;
    let n = (operators.len() + operands.len()) as f64;
    let vocab = n1 + n2;
    if vocab <= 1.0 || n == 0.0 {
        0.0
    } else {
        n * vocab.log2()
    }
}

fn lexical_tokens(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut cur = String::new();
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || c == '_' {
            cur.push(c);
        } else {
            if !cur.is_empty() {
                tokens.push(std::mem::take(&mut cur));
            }
            if !c.is_whitespace() {
                tokens.push(c.to_string());
            }
        }
    }
    if !cur.is_empty() {
        tokens.push(cur);
    }
    tokens
}

fn is_operator(t: &str) -> bool {
    matches!(
        t,
        "{" | "}"
            | "("
            | ")"
            | "["
            | "]"
            | "+"
            | "-"
            | "*"
            | "/"
            | "%"
            | "="
            | "!"
            | "<"
            | ">"
            | "&"
            | "|"
            | "?"
            | ":"
            | ";"
            | ","
            | "."
    )
}

fn unique_count(v: &[String]) -> usize {
    let mut u = v.to_vec();
    u.sort();
    u.dedup();
    u.len()
}

fn maintainability_index(volume: f64, complexity: u32, loc: usize) -> f64 {
    if loc == 0 {
        return 100.0;
    }
    let raw =
        171.0 - 5.2 * volume.max(1.0).ln() - 0.23 * complexity as f64 - 16.2 * (loc as f64).ln();
    (raw * 100.0 / 171.0).clamp(0.0, 100.0)
}

fn grade(mi: f64) -> char {
    if mi >= 80.0 {
        'A'
    } else if mi >= 65.0 {
        'B'
    } else if mi >= 50.0 {
        'C'
    } else if mi >= 35.0 {
        'D'
    } else {
        'F'
    }
}

fn detect_language(source: &str) -> Language {
    let s = source.to_ascii_lowercase();
    let mut scores = vec![
        (
            Language::Rust,
            score(&s, &["fn ", "let mut", "match ", "impl ", "pub ", "::"]),
        ),
        (
            Language::Python,
            score(
                &s,
                &["def ", "elif ", "import ", "from ", "self", ":\n    "],
            ),
        ),
        (
            Language::Javascript,
            score(&s, &["function ", "const ", "let ", "=>", "console."]),
        ),
        (
            Language::Typescript,
            score(&s, &["interface ", ": string", ": number", "type ", "=>"]),
        ),
        (
            Language::Go,
            score(&s, &["func ", "package ", "fmt.", ":=", "defer "]),
        ),
        (
            Language::Java,
            score(&s, &["public class", "private ", "system.out", "throws "]),
        ),
        (
            Language::Sql,
            score(
                &s,
                &["select ", " from ", " where ", " join ", "create function"],
            ),
        ),
        (
            Language::Shell,
            score(
                &s,
                &["#!/bin/sh", "#!/usr/bin/env bash", " fi", " then", "$1"],
            ),
        ),
    ];
    scores.sort_by(|a, b| b.1.cmp(&a.1));
    if scores[0].1 == 0 {
        Language::Javascript
    } else {
        scores[0].0
    }
}

fn score(s: &str, needles: &[&str]) -> i32 {
    needles.iter().map(|n| s.matches(n).count() as i32).sum()
}

fn sort_functions(functions: &mut [FunctionMetric], sort: SortBy) {
    match sort {
        SortBy::Line => functions.sort_by_key(|f| f.line),
        SortBy::Complexity => {
            functions.sort_by(|a, b| b.cyclomatic.cmp(&a.cyclomatic).then(a.line.cmp(&b.line)))
        }
        SortBy::Cognitive => {
            functions.sort_by(|a, b| b.cognitive.cmp(&a.cognitive).then(a.line.cmp(&b.line)))
        }
        SortBy::Length => functions.sort_by(|a, b| {
            b.length_lines
                .cmp(&a.length_lines)
                .then(a.line.cmp(&b.line))
        }),
        SortBy::Name => functions.sort_by(|a, b| a.name.cmp(&b.name)),
    }
}

fn render(
    report: &Report,
    output: Output,
    max_functions: usize,
    threshold: u32,
) -> Result<String, String> {
    match output {
        Output::Summary => Ok(render_summary(report, max_functions, threshold)),
        Output::Functions => Ok(render_functions(report, max_functions, threshold)),
        Output::Csv => Ok(render_csv(report, max_functions)),
        Output::Json => Ok(render_json(report, max_functions, threshold)),
    }
}

fn shown_functions(report: &Report, max_functions: usize) -> &[FunctionMetric] {
    if max_functions == 0 || max_functions >= report.functions.len() {
        &report.functions
    } else {
        &report.functions[..max_functions]
    }
}

fn render_summary(report: &Report, max_functions: usize, threshold: u32) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "Language: {}{}\n",
        report.language.as_str(),
        if report.language_detected {
            " (auto)"
        } else {
            ""
        }
    ));
    out.push_str(&format!(
        "Lines: total {}, code {}, comment {}, blank {}\n",
        report.total_lines, report.code_lines, report.comment_lines, report.blank_lines
    ));
    out.push_str(&format!("Functions: {}\n", report.function_count));
    out.push_str(&format!(
        "Cyclomatic complexity: total {}, average {:.1}, max {}\n",
        report.cyclomatic_total, report.cyclomatic_average, report.cyclomatic_max
    ));
    out.push_str(&format!(
        "Cognitive complexity: total {}\n",
        report.cognitive_total
    ));
    out.push_str(&format!(
        "Maintainability: {:.1}/100 (grade {})\n",
        report.maintainability_index, report.maintainability_grade
    ));
    out.push_str(&format!("Halstead volume: {:.1}\n", report.halstead_volume));
    out.push_str(&format!(
        "Over threshold (>{threshold}): {}\n",
        report.over_threshold_count
    ));
    if !report.functions.is_empty() {
        out.push_str("\nFunctions:\n");
        for f in shown_functions(report, max_functions) {
            out.push_str(&format!(
                "- {} (line {}, {} LOC): CCN {}, cognitive {}, params {}, nesting {}, risk {}\n",
                f.name,
                f.line,
                f.nloc,
                f.cyclomatic,
                f.cognitive,
                f.params,
                f.max_nesting,
                f.risk()
            ));
        }
        if max_functions > 0 && report.functions.len() > max_functions {
            out.push_str(&format!(
                "... {} more function(s) not shown\n",
                report.functions.len() - max_functions
            ));
        }
    }
    if !report.warnings.is_empty() {
        out.push_str("\nWarnings:\n");
        for w in &report.warnings {
            out.push_str(&format!("- {w}\n"));
        }
    }
    out.trim_end().to_string()
}

fn render_functions(report: &Report, max_functions: usize, threshold: u32) -> String {
    let mut out = format!("Name | Line | LOC | CCN | Cognitive | Params | Nesting | Risk\n--- | ---: | ---: | ---: | ---: | ---: | ---: | ---\n");
    for f in shown_functions(report, max_functions) {
        let marker = if f.cyclomatic > threshold { " ⚠" } else { "" };
        out.push_str(&format!(
            "{}{} | {} | {} | {} | {} | {} | {} | {}\n",
            f.name,
            marker,
            f.line,
            f.nloc,
            f.cyclomatic,
            f.cognitive,
            f.params,
            f.max_nesting,
            f.risk()
        ));
    }
    out.trim_end().to_string()
}

fn render_csv(report: &Report, max_functions: usize) -> String {
    let mut out = String::from(
        "name,line,end_line,length_lines,nloc,cyclomatic,cognitive,params,max_nesting,risk\n",
    );
    for f in shown_functions(report, max_functions) {
        out.push_str(&format!(
            "{},{},{},{},{},{},{},{},{},{}\n",
            csv(&f.name),
            f.line,
            f.end_line,
            f.length_lines,
            f.nloc,
            f.cyclomatic,
            f.cognitive,
            f.params,
            f.max_nesting,
            f.risk()
        ));
    }
    out.trim_end().to_string()
}

fn render_json(report: &Report, max_functions: usize, threshold: u32) -> String {
    let functions: Vec<_> = shown_functions(report, max_functions)
        .iter()
        .map(|f| {
            json!({
                "name": f.name,
                "line": f.line,
                "end_line": f.end_line,
                "length_lines": f.length_lines,
                "nloc": f.nloc,
                "cyclomatic": f.cyclomatic,
                "cognitive": f.cognitive,
                "params": f.params,
                "max_nesting": f.max_nesting,
                "risk": f.risk(),
                "over_threshold": f.cyclomatic > threshold,
            })
        })
        .collect();
    let value = json!({
        "language": report.language.as_str(),
        "language_detected": report.language_detected,
        "line_counts": {
            "total": report.total_lines,
            "code": report.code_lines,
            "comment": report.comment_lines,
            "blank": report.blank_lines,
            "max_line_length": report.max_line_length,
            "average_line_length": (report.average_line_length * 10.0).round() / 10.0,
        },
        "functions_total": report.function_count,
        "functions_shown": functions.len(),
        "complexity": {
            "cyclomatic_total": report.cyclomatic_total,
            "cyclomatic_average": (report.cyclomatic_average * 10.0).round() / 10.0,
            "cyclomatic_max": report.cyclomatic_max,
            "cognitive_total": report.cognitive_total,
            "threshold": threshold,
            "over_threshold_count": report.over_threshold_count,
        },
        "maintainability": {
            "index": (report.maintainability_index * 10.0).round() / 10.0,
            "grade": report.maintainability_grade.to_string(),
            "halstead_volume": (report.halstead_volume * 10.0).round() / 10.0,
        },
        "functions": functions,
        "warnings": report.warnings,
    });
    serde_json::to_string_pretty(&value).unwrap()
}

fn csv(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUST_SAMPLE: &str = r#"
fn score(xs: &[i32]) -> i32 {
    let mut total = 0;
    for x in xs {
        if *x > 0 && *x < 10 {
            total += x;
        } else if *x > 100 {
            total += 1;
        }
    }
    total
}
"#;

    #[test]
    fn reports_rust_loc_and_complexity() {
        let out = run_with_options(RUST_SAMPLE, "rust", "json", 3, 0, "line").unwrap();
        assert!(out.contains("\"language\": \"rust\""), "{out}");
        assert!(out.contains("\"name\": \"score\""), "{out}");
        assert!(out.contains("\"cyclomatic\": 5"), "{out}");
        assert!(out.contains("\"over_threshold_count\": 1"), "{out}");
    }

    #[test]
    fn parses_javascript_arrow_function_and_masks_comments_from_loc() {
        let js = "// file comment\nconst grade = (x) => {\n  if (x > 90) return 'A';\n  return 'B'; // tail\n};";
        let out = run_with_options(js, "javascript", "summary", 10, 10, "line").unwrap();
        assert!(out.contains("Language: javascript"), "{out}");
        assert!(
            out.contains("Lines: total 5, code 4, comment 1, blank 0"),
            "{out}"
        );
        assert!(out.contains("- grade"), "{out}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = run_with_options("  ", "auto", "summary", 10, 50, "line").unwrap_err();
        assert!(err.contains("no source code"));
    }

    #[test]
    fn csv_escapes_function_names() {
        let py = "def hello(name):\n    if name:\n        return name\n";
        let out = run_with_options(py, "python", "csv", 10, 0, "name").unwrap();
        assert!(out.starts_with("name,line"), "{out}");
        assert!(out.contains("hello,1,3,3,3,2"), "{out}");
    }
}
