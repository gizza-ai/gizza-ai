//! latex-to-text core — pure compute, shared by the chat skill block and the web page.
//!
//! Strips LaTeX/TeX markup from a `.tex` source and recovers the readable prose:
//! commands are removed but their visible argument text is kept, comments and
//! preamble go away, math is removed/kept/placeheld, and accents plus symbol
//! macros become Unicode. Hand-rolled single-pass tokenizer — no dependencies,
//! so it builds for wasm32-wasip1 (chat/CLI) and wasm32-unknown-unknown (page).
//!
//! This is emphatically NOT a TeX engine: user-defined macros are not expanded
//! and `\input`/`\include` are not followed (a sandboxed block has no
//! filesystem). See the page copy for the stated limits.

/// Largest accepted input. Documents beyond this should be split per chapter.
pub const MAX_CHARS: usize = 1_000_000;

/// Non-math environments whose *content* is discarded by default. Mirrors the
/// classic detex default list (array, longtable, picture, tabular, verbatim)
/// plus the modern code/graphics environments. Math environments are NOT here —
/// they follow the `math` setting instead.
pub const DEFAULT_DROP_ENVIRONMENTS: &str =
    "array,longtable,picture,tabular,tabularx,verbatim,lstlisting,minted,tikzpicture";

/// Environments treated as math (governed by the `math` param, not the drop list).
const MATH_ENVS: &[&str] = &[
    "equation",
    "align",
    "alignat",
    "gather",
    "multline",
    "displaymath",
    "math",
    "eqnarray",
    "flalign",
    "split",
    "dmath",
    "ieeeeqnarray",
];

/// Environments whose `\begin{env}{spec}` carries a non-text mandatory argument.
const ENV_WITH_SPEC: &[&str] = &[
    "tabular",
    "tabularx",
    "array",
    "minipage",
    "multicols",
    "wrapfigure",
    "thebibliography",
    "lstlisting",
];

/// Reference/citation commands, gated by the `citations` param.
const CITE_CMDS: &[&str] = &[
    "cite",
    "citep",
    "citet",
    "citealp",
    "citealt",
    "citeauthor",
    "citeyear",
    "nocite",
    "parencite",
    "textcite",
    "autocite",
    "ref",
    "eqref",
    "pageref",
    "autoref",
    "nameref",
    "cref",
    "vref",
];

/// Commands dropped together with their mandatory argument(s): metadata,
/// layout, macro definitions, file inclusion — none of it is prose.
/// `(name, number of mandatory groups to discard)`.
const DROP_WITH_ARGS: &[(&str, usize)] = &[
    ("documentclass", 1),
    ("usepackage", 1),
    ("RequirePackage", 1),
    ("label", 1),
    ("index", 1),
    ("bibliography", 1),
    ("bibliographystyle", 1),
    ("addbibresource", 1),
    ("input", 1),
    ("include", 1),
    ("includeonly", 1),
    ("includegraphics", 1),
    ("graphicspath", 1),
    ("usetikzlibrary", 1),
    ("newcommand", 2),
    ("renewcommand", 2),
    ("providecommand", 2),
    ("DeclareMathOperator", 2),
    ("newenvironment", 3),
    ("renewenvironment", 3),
    ("newtheorem", 2),
    ("def", 1),
    ("setlength", 2),
    ("addtolength", 2),
    ("setcounter", 2),
    ("geometry", 1),
    ("hypersetup", 1),
    ("pagestyle", 1),
    ("thispagestyle", 1),
    ("pagenumbering", 1),
    ("definecolor", 3),
    ("addcontentsline", 3),
    ("bibitem", 1),
];

/// Commands with leading non-text arguments followed by the visible text, e.g.
/// `\href{url}{label}`. `(name, groups to discard before the text)`.
const DROP_LEADING_ARGS: &[(&str, usize)] = &[
    ("href", 1),
    ("textcolor", 1),
    ("colorbox", 1),
    ("fcolorbox", 2),
    ("multicolumn", 2),
];

/// Block-level title commands: their argument is kept, on its own paragraph.
const HEADING_CMDS: &[&str] = &[
    "part",
    "chapter",
    "section",
    "subsection",
    "subsubsection",
    "paragraph",
    "subparagraph",
    "title",
    "author",
    "date",
    "caption",
    "captionof",
];

/// No-argument commands that produce no text (font/size/layout switches).
const NOOP_CMDS: &[&str] = &[
    "maketitle",
    "tableofcontents",
    "listoffigures",
    "listoftables",
    "printbibliography",
    "appendix",
    "frontmatter",
    "mainmatter",
    "backmatter",
    "centering",
    "raggedright",
    "raggedleft",
    "noindent",
    "indent",
    "hline",
    "hrule",
    "toprule",
    "midrule",
    "bottomrule",
    "protect",
    "relax",
    "ignorespaces",
    "normalsize",
    "tiny",
    "scriptsize",
    "footnotesize",
    "small",
    "large",
    "Large",
    "LARGE",
    "huge",
    "Huge",
    "bfseries",
    "mdseries",
    "itshape",
    "upshape",
    "slshape",
    "scshape",
    "normalfont",
    "rmfamily",
    "sffamily",
    "ttfamily",
    "em",
    "bf",
    "it",
    "sl",
    "sc",
    "rm",
    "tt",
    "sf",
    "hfill",
    "vfill",
    "hrulefill",
    "dotfill",
    "arraybackslash",
];

/// Commands that force a break in the output.
const BREAK_CMDS: &[&str] = &[
    "par",
    "newline",
    "linebreak",
    "newpage",
    "clearpage",
    "cleardoublepage",
    "pagebreak",
];

/// Commands that emit a single space.
const SPACE_CMDS: &[&str] = &[
    "quad",
    "qquad",
    "enspace",
    "thinspace",
    "space",
    "nobreakspace",
    "hspace",
    "vspace",
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum MathMode {
    Remove,
    Keep,
    Placeholder,
}

struct Opts {
    math: MathMode,
    keep_keys: bool,
    drop_envs: Vec<String>,
    keep_comments: bool,
    unicode: bool,
    source_breaks: bool,
}

/// Convert LaTeX source to readable plain text.
///
/// * `math` — `"remove"` (default), `"keep"` (verbatim source with delimiters)
///   or `"placeholder"` (`[math]`).
/// * `citations` — `"drop"` (default) or `"keys"` (echo the cite/ref keys).
/// * `drop_environments` — comma-separated non-math environments whose content is
///   discarded; empty falls back to [`DEFAULT_DROP_ENVIRONMENTS`].
/// * `keep_comments` — keep `%` comment text instead of dropping it.
/// * `unicode` — render accents/symbol macros as Unicode (else ASCII fallbacks).
/// * `body_only` — convert only what is between `\begin{document}`/`\end{document}`.
/// * `line_breaks` — `"paragraphs"` (reflow) or `"source"` (keep source line breaks).
#[allow(clippy::too_many_arguments)]
pub fn to_text(
    text: &str,
    math: &str,
    citations: &str,
    drop_environments: &str,
    keep_comments: bool,
    unicode: bool,
    body_only: bool,
    line_breaks: &str,
) -> Result<String, String> {
    let math_mode = match math.trim() {
        "" | "remove" => MathMode::Remove,
        "keep" => MathMode::Keep,
        "placeholder" => MathMode::Placeholder,
        other => {
            return Err(format!(
                "unknown math mode '{other}' — use 'remove', 'keep' or 'placeholder'"
            ))
        }
    };
    let keep_keys = match citations.trim() {
        "" | "drop" => false,
        "keys" => true,
        other => {
            return Err(format!(
                "unknown citations mode '{other}' — use 'drop' or 'keys'"
            ))
        }
    };
    let source_breaks = match line_breaks.trim() {
        "" | "paragraphs" => false,
        "source" => true,
        other => {
            return Err(format!(
                "unknown line_breaks mode '{other}' — use 'paragraphs' or 'source'"
            ))
        }
    };
    if text.trim().is_empty() {
        return Err("input is empty — paste the LaTeX source to convert".into());
    }
    let count = text.chars().count();
    if count > MAX_CHARS {
        return Err(format!(
            "input is {count} characters; the limit is {MAX_CHARS}. Convert the document one chapter at a time."
        ));
    }

    let list = if drop_environments.trim().is_empty() {
        DEFAULT_DROP_ENVIRONMENTS
    } else {
        drop_environments
    };
    let drop_envs: Vec<String> = list
        .split(',')
        .map(norm_env)
        .filter(|s| !s.is_empty())
        .collect();

    let body = if body_only { document_body(text) } else { text };

    let mut conv = Conv {
        c: body.chars().collect(),
        i: 0,
        out: String::with_capacity(body.len()),
        o: Opts {
            math: math_mode,
            keep_keys,
            drop_envs,
            keep_comments,
            unicode,
            source_breaks,
        },
    };
    conv.run();
    let out = tidy(&conv.out);
    if out.is_empty() {
        return Err(
            "no readable text remained — the source may be preamble only, or everything fell into a dropped environment"
                .into(),
        );
    }
    Ok(out)
}

/// Slice out `\begin{document}` … `\end{document}`, ignoring commented-out
/// occurrences. Returns the whole input when there is no document environment
/// (a fragment rather than a full source file).
fn document_body(text: &str) -> &str {
    let begin = match find_uncommented(text, "\\begin{document}") {
        Some(p) => p + "\\begin{document}".len(),
        None => return text,
    };
    let rest = &text[begin..];
    match find_uncommented(rest, "\\end{document}") {
        Some(p) => &rest[..p],
        None => rest,
    }
}

/// Byte index of `needle` in `hay`, skipping matches inside `%` comments.
fn find_uncommented(hay: &str, needle: &str) -> Option<usize> {
    let bytes = hay.as_bytes();
    let n = needle.as_bytes();
    let mut i = 0usize;
    let mut in_comment = false;
    while i < bytes.len() {
        let b = bytes[i];
        if in_comment {
            if b == b'\n' {
                in_comment = false;
            }
            i += 1;
            continue;
        }
        if b == b'\\' {
            if hay[i..].starts_with(needle) {
                return Some(i);
            }
            i += 2; // the escaped character can never start a comment
            continue;
        }
        if b == b'%' {
            in_comment = true;
            i += 1;
            continue;
        }
        if b == n[0] && hay[i..].starts_with(needle) {
            return Some(i);
        }
        i += 1;
    }
    None
}

/// Normalize an environment name: trim, drop a starred form, lowercase.
fn norm_env(raw: &str) -> String {
    raw.trim().trim_end_matches('*').trim().to_ascii_lowercase()
}

struct Conv {
    c: Vec<char>,
    i: usize,
    out: String,
    o: Opts,
}

impl Conv {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn at(&self, k: usize) -> Option<char> {
        self.c.get(self.i + k).copied()
    }

    /// Emit a paragraph break (one blank line), never doubling one up.
    fn para(&mut self) {
        while self.out.ends_with(' ') {
            self.out.pop();
        }
        if self.out.is_empty() || self.out.ends_with("\n\n") {
            return;
        }
        if self.out.ends_with('\n') {
            self.out.push('\n');
        } else {
            self.out.push_str("\n\n");
        }
    }

    /// Emit a hard line break.
    fn hard_break(&mut self) {
        while self.out.ends_with(' ') {
            self.out.pop();
        }
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
    }

    fn space(&mut self) {
        if !self.out.is_empty() && !self.out.ends_with(' ') && !self.out.ends_with('\n') {
            self.out.push(' ');
        }
    }

    fn push_sym(&mut self, uni: &str, ascii: &str) {
        let s = if self.o.unicode { uni } else { ascii };
        self.out.push_str(s);
    }

    fn run(&mut self) {
        while self.i < self.c.len() {
            let ch = self.c[self.i];
            match ch {
                '%' => self.comment(),
                '\\' => self.command(),
                '$' => self.dollar_math(),
                '{' | '}' => self.i += 1,
                '&' | '~' => {
                    self.i += 1;
                    self.space();
                }
                '\n' => self.newlines(),
                ' ' | '\t' | '\r' => {
                    self.i += 1;
                    self.space();
                }
                '-' => self.dashes(),
                '`' => {
                    if self.at(1) == Some('`') {
                        self.i += 2;
                        self.push_sym("\u{201C}", "\"");
                    } else {
                        self.i += 1;
                        self.out.push('`');
                    }
                }
                '\'' => {
                    if self.at(1) == Some('\'') {
                        self.i += 2;
                        self.push_sym("\u{201D}", "\"");
                    } else {
                        self.i += 1;
                        self.out.push('\'');
                    }
                }
                _ => {
                    self.i += 1;
                    self.out.push(ch);
                }
            }
        }
    }

    /// A run of newlines: two or more is a paragraph break, one is a soft break
    /// (a space when reflowing, a newline in source mode).
    fn newlines(&mut self) {
        let mut nl = 0usize;
        while let Some(c) = self.peek() {
            match c {
                '\n' => {
                    nl += 1;
                    self.i += 1;
                }
                ' ' | '\t' | '\r' => self.i += 1,
                _ => break,
            }
        }
        if nl >= 2 {
            self.para();
        } else if self.o.source_breaks {
            self.hard_break();
        } else {
            self.space();
        }
    }

    fn comment(&mut self) {
        // A comment that starts its own line takes the whole line with it.
        let mut k = self.i;
        let mut line_leading = true;
        while k > 0 {
            let p = self.c[k - 1];
            if p == '\n' {
                break;
            }
            if !p.is_whitespace() {
                line_leading = false;
                break;
            }
            k -= 1;
        }
        self.i += 1; // past '%'
        let start = self.i;
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.i += 1;
        }
        if self.o.keep_comments {
            let body: String = self.c[start..self.i].iter().collect();
            let body = body.trim().to_string();
            if !body.is_empty() {
                self.space();
                self.out.push_str(&body);
            }
        }
        if line_leading {
            if self.peek() == Some('\n') {
                self.i += 1;
            }
            if self.o.keep_comments {
                self.hard_break();
            }
        }
    }

    fn dashes(&mut self) {
        let mut n = 0usize;
        while self.peek() == Some('-') {
            n += 1;
            self.i += 1;
        }
        match n {
            3 => self.push_sym("\u{2014}", "--"),
            2 => self.push_sym("\u{2013}", "-"),
            _ => {
                for _ in 0..n {
                    self.out.push('-');
                }
            }
        }
    }

    fn emit_math(&mut self, src: &str, open: &str, close: &str) {
        match self.o.math {
            MathMode::Remove => self.space(),
            MathMode::Placeholder => {
                self.space();
                self.out.push_str("[math]");
            }
            MathMode::Keep => {
                self.space();
                let body = src.trim().to_string();
                self.out.push_str(open);
                self.out.push_str(&body);
                self.out.push_str(close);
            }
        }
    }

    fn dollar_math(&mut self) {
        let display = self.at(1) == Some('$');
        let delim = if display { 2 } else { 1 };
        let start = (self.i + delim).min(self.c.len());
        let mut k = start;
        let mut end = self.c.len();
        let mut after = self.c.len();
        while k < self.c.len() {
            if self.c[k] == '\\' {
                k += 2;
                continue;
            }
            if self.c[k] == '$' {
                if display {
                    if self.c.get(k + 1) == Some(&'$') {
                        end = k;
                        after = k + 2;
                        break;
                    }
                    k += 1;
                    continue;
                }
                end = k;
                after = k + 1;
                break;
            }
            k += 1;
        }
        let end = end.min(self.c.len());
        let src: String = self.c[start.min(end)..end].iter().collect();
        self.i = after.min(self.c.len());
        let d = if display { "$$" } else { "$" };
        self.emit_math(&src, d, d);
    }

    /// `\(` … `\)` and `\[` … `\]`.
    fn delim_math(&mut self, close_ch: char, open_out: &str, close_out: &str) {
        let start = self.i;
        let mut k = start;
        let mut end = self.c.len();
        let mut after = self.c.len();
        while k < self.c.len() {
            if self.c[k] == '\\' {
                if self.c.get(k + 1) == Some(&close_ch) {
                    end = k;
                    after = k + 2;
                    break;
                }
                k += 2;
                continue;
            }
            k += 1;
        }
        let end = end.min(self.c.len());
        let src: String = self.c[start.min(end)..end].iter().collect();
        self.i = after.min(self.c.len());
        self.emit_math(&src, open_out, close_out);
    }

    /// Skip `[...]` optional arguments that follow immediately.
    fn skip_optional_args(&mut self) {
        while self.peek() == Some('[') {
            let mut depth = 0usize;
            while self.i < self.c.len() {
                match self.c[self.i] {
                    '\\' => self.i += 1,
                    '[' => depth += 1,
                    ']' => {
                        depth -= 1;
                        if depth == 0 {
                            self.i += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                self.i += 1;
            }
        }
    }

    /// Read `[...]` and return its raw contents, if one follows immediately.
    fn read_optional_raw(&mut self) -> Option<String> {
        if self.peek() != Some('[') {
            return None;
        }
        let start = self.i + 1;
        let mut depth = 0usize;
        let mut end = self.c.len();
        while self.i < self.c.len() {
            match self.c[self.i] {
                '\\' => self.i += 1,
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = self.i;
                        self.i += 1;
                        break;
                    }
                }
                _ => {}
            }
            self.i += 1;
        }
        Some(
            self.c[start.min(end)..end.min(self.c.len())]
                .iter()
                .collect(),
        )
    }

    /// Read a `{...}` group (spaces/tabs may precede it) and return its raw
    /// contents. Returns an empty string and consumes nothing when absent.
    fn read_group_raw(&mut self) -> String {
        let save = self.i;
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.i += 1;
        }
        if self.peek() != Some('{') {
            self.i = save;
            return String::new();
        }
        let start = self.i + 1;
        let mut depth = 0usize;
        let mut end = self.c.len();
        while self.i < self.c.len() {
            match self.c[self.i] {
                '\\' => self.i += 1,
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = self.i;
                        self.i += 1;
                        break;
                    }
                }
                _ => {}
            }
            self.i += 1;
        }
        self.c[start.min(end)..end.min(self.c.len())]
            .iter()
            .collect()
    }

    fn skip_groups(&mut self, n: usize) {
        for _ in 0..n {
            self.read_group_raw();
        }
    }

    /// Convert a raw fragment with the same options and append the result.
    fn append_fragment(&mut self, src: &str) {
        if src.trim().is_empty() {
            return;
        }
        let mut sub = Conv {
            c: src.chars().collect(),
            i: 0,
            out: String::new(),
            o: Opts {
                math: self.o.math,
                keep_keys: self.o.keep_keys,
                drop_envs: self.o.drop_envs.clone(),
                keep_comments: self.o.keep_comments,
                unicode: self.o.unicode,
                source_breaks: self.o.source_breaks,
            },
        };
        sub.run();
        let text = tidy(&sub.out);
        if !text.is_empty() {
            self.out.push_str(&text);
        }
    }

    fn command(&mut self) {
        let next = match self.at(1) {
            Some(c) => c,
            None => {
                self.i += 1;
                return;
            }
        };
        if next.is_ascii_alphabetic() {
            let mut k = self.i + 1;
            while k < self.c.len() && self.c[k].is_ascii_alphabetic() {
                k += 1;
            }
            let name: String = self.c[self.i + 1..k].iter().collect();
            if self.c.get(k) == Some(&'*') {
                k += 1;
            }
            self.i = k;
            self.named_command(&name);
            return;
        }
        self.i += 2;
        match next {
            '\\' => {
                self.skip_optional_args();
                self.hard_break();
            }
            '%' | '$' | '&' | '#' | '_' | '{' | '}' => self.out.push(next),
            '(' => self.delim_math(')', "\\(", "\\)"),
            '[' => self.delim_math(']', "\\[", "\\]"),
            ')' | ']' => {}
            ',' | ';' | ':' | ' ' | '/' => self.space(),
            '!' | '-' | '@' => {}
            '\'' | '`' | '^' | '"' | '~' | '=' | '.' => {
                let arg = self.read_accent_arg();
                self.push_accent(next, &arg);
            }
            _ => {}
        }
    }

    fn named_command(&mut self, name: &str) {
        let lower = name.to_ascii_lowercase();
        match name {
            "begin" => return self.begin_env(),
            "end" => {
                self.read_group_raw();
                self.para();
                return;
            }
            "verb" => return self.verbatim_inline(),
            "item" => {
                self.hard_break();
                if let Some(label) = self.read_optional_raw() {
                    self.append_fragment(&label);
                    self.out.push(' ');
                }
                return;
            }
            "c" | "v" | "u" | "H" | "r" | "k" => {
                let arg = self.read_accent_arg();
                let key = name.chars().next().unwrap_or(' ');
                self.push_accent(key, &arg);
                return;
            }
            "d" | "b" | "t" => {
                let arg = self.read_accent_arg();
                self.out.push_str(&arg);
                return;
            }
            _ => {}
        }

        if CITE_CMDS.contains(&lower.as_str()) {
            self.skip_optional_args();
            let keys = self.read_group_raw();
            if self.o.keep_keys {
                let keys = keys.trim().to_string();
                if !keys.is_empty() {
                    self.space();
                    self.out.push_str(&keys);
                }
            }
            return;
        }
        if let Some((_, n)) = DROP_WITH_ARGS.iter().find(|(c, _)| *c == name) {
            self.skip_optional_args();
            self.skip_groups(*n);
            return;
        }
        if let Some((_, n)) = DROP_LEADING_ARGS.iter().find(|(c, _)| *c == name) {
            self.skip_optional_args();
            self.skip_groups(*n);
            return; // the visible text group is handled by the main loop
        }
        if HEADING_CMDS.contains(&name) {
            self.para();
            self.skip_optional_args();
            let title = self.read_group_raw();
            self.append_fragment(&title);
            self.para();
            return;
        }
        if BREAK_CMDS.contains(&name) {
            self.skip_optional_args();
            self.skip_groups(usize::from(name == "vspace" || name == "hspace"));
            self.para();
            return;
        }
        if SPACE_CMDS.contains(&name) {
            self.skip_optional_args();
            if name == "hspace" || name == "vspace" {
                self.skip_groups(1);
            }
            self.space();
            return;
        }
        if NOOP_CMDS.contains(&name) {
            return;
        }
        if let Some(sym) = symbol(name, self.o.unicode) {
            self.out.push_str(sym);
            return;
        }
        // Unknown command: drop the command itself, keep any visible argument
        // text (the main loop simply ignores the surrounding braces).
        self.skip_optional_args();
    }

    /// `\verb|code|` — the character after the command is the delimiter.
    fn verbatim_inline(&mut self) {
        let delim = match self.peek() {
            Some(c) => c,
            None => return,
        };
        self.i += 1;
        let start = self.i;
        while self.i < self.c.len() && self.c[self.i] != delim {
            self.i += 1;
        }
        let code: String = self.c[start..self.i].iter().collect();
        if self.i < self.c.len() {
            self.i += 1;
        }
        if !code.is_empty() {
            self.space();
            self.out.push_str(&code);
        }
    }

    fn begin_env(&mut self) {
        let raw = self.read_group_raw();
        let name = norm_env(&raw);
        self.skip_optional_args();
        if ENV_WITH_SPEC.contains(&name.as_str()) {
            self.read_group_raw();
        }
        if name == "document" {
            self.para();
            return;
        }
        if MATH_ENVS.contains(&name.as_str()) {
            let src = self.skip_to_end(&name);
            let trimmed = raw.trim().to_string();
            let open = format!("\\begin{{{trimmed}}}");
            let close = format!("\\end{{{trimmed}}}");
            if self.o.math == MathMode::Keep {
                self.para();
                self.emit_math(&src, &open, &close);
                self.para();
            } else {
                self.emit_math(&src, &open, &close);
            }
            return;
        }
        if self.o.drop_envs.iter().any(|e| e == &name) {
            self.skip_to_end(&name);
            self.para();
            return;
        }
        self.para();
    }

    /// Consume up to and including the matching `\end{name}` (same-name nesting
    /// aware) and return the raw inner source.
    fn skip_to_end(&mut self, name: &str) -> String {
        let start = self.i;
        let mut depth = 1usize;
        let mut inner_end = self.c.len();
        while self.i < self.c.len() {
            if self.c[self.i] != '\\' {
                self.i += 1;
                continue;
            }
            match self.peek_begin_end() {
                Some((is_begin, env, after)) => {
                    if norm_env(&env) == name {
                        if is_begin {
                            depth += 1;
                        } else {
                            depth -= 1;
                            if depth == 0 {
                                inner_end = self.i;
                                self.i = after;
                                break;
                            }
                        }
                    }
                    self.i = after;
                }
                None => self.i += 2,
            }
        }
        self.c[start.min(inner_end)..inner_end.min(self.c.len())]
            .iter()
            .collect()
    }

    /// At a `\`, recognise `\begin{env}` / `\end{env}` without consuming.
    /// Returns `(is_begin, env_name, index just past the closing brace)`.
    fn peek_begin_end(&self) -> Option<(bool, String, usize)> {
        let mut k = self.i + 1;
        let mut word = String::new();
        while k < self.c.len() && self.c[k].is_ascii_alphabetic() {
            word.push(self.c[k]);
            k += 1;
        }
        let is_begin = match word.as_str() {
            "begin" => true,
            "end" => false,
            _ => return None,
        };
        while k < self.c.len() && (self.c[k] == ' ' || self.c[k] == '\t') {
            k += 1;
        }
        if self.c.get(k) != Some(&'{') {
            return None;
        }
        k += 1;
        let mut env = String::new();
        while k < self.c.len() && self.c[k] != '}' {
            env.push(self.c[k]);
            k += 1;
        }
        if self.c.get(k) != Some(&'}') {
            return None;
        }
        Some((is_begin, env, k + 1))
    }

    /// The argument of an accent command: `{e}`, a bare `e`, or `\i` / `\j`.
    fn read_accent_arg(&mut self) -> String {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.i += 1;
        }
        let raw = if self.peek() == Some('{') {
            self.read_group_raw()
        } else if self.peek() == Some('\\') {
            let mut k = self.i + 1;
            let mut s = String::new();
            while k < self.c.len() && self.c[k].is_ascii_alphabetic() {
                s.push(self.c[k]);
                k += 1;
            }
            self.i = k;
            s
        } else {
            match self.peek() {
                Some(c) => {
                    self.i += 1;
                    c.to_string()
                }
                None => String::new(),
            }
        };
        let t = raw.trim();
        let t = t.strip_prefix('\\').unwrap_or(t);
        match t {
            "i" => "i".to_string(),
            "j" => "j".to_string(),
            other => other.to_string(),
        }
    }

    fn push_accent(&mut self, accent: char, base: &str) {
        let mut chars = base.chars();
        let first = match chars.next() {
            Some(c) => c,
            None => return,
        };
        let rest: String = chars.collect();
        if self.o.unicode {
            match compose(accent, first) {
                Some(c) => self.out.push(c),
                None => self.out.push(first),
            }
        } else {
            self.out.push(first);
        }
        self.out.push_str(&rest);
    }
}

/// Precomposed Unicode for the common `accent + letter` pairs. Unknown pairs
/// fall back to the bare letter (never a mojibake control sequence).
fn compose(accent: char, base: char) -> Option<char> {
    let c = match (accent, base) {
        ('\'', 'a') => 'á',
        ('\'', 'e') => 'é',
        ('\'', 'i') => 'í',
        ('\'', 'o') => 'ó',
        ('\'', 'u') => 'ú',
        ('\'', 'y') => 'ý',
        ('\'', 'n') => 'ń',
        ('\'', 'c') => 'ć',
        ('\'', 's') => 'ś',
        ('\'', 'z') => 'ź',
        ('\'', 'A') => 'Á',
        ('\'', 'E') => 'É',
        ('\'', 'I') => 'Í',
        ('\'', 'O') => 'Ó',
        ('\'', 'U') => 'Ú',
        ('\'', 'Y') => 'Ý',
        ('\'', 'N') => 'Ń',
        ('\'', 'C') => 'Ć',
        ('\'', 'S') => 'Ś',
        ('\'', 'Z') => 'Ź',
        ('`', 'a') => 'à',
        ('`', 'e') => 'è',
        ('`', 'i') => 'ì',
        ('`', 'o') => 'ò',
        ('`', 'u') => 'ù',
        ('`', 'A') => 'À',
        ('`', 'E') => 'È',
        ('`', 'I') => 'Ì',
        ('`', 'O') => 'Ò',
        ('`', 'U') => 'Ù',
        ('^', 'a') => 'â',
        ('^', 'e') => 'ê',
        ('^', 'i') => 'î',
        ('^', 'o') => 'ô',
        ('^', 'u') => 'û',
        ('^', 'c') => 'ĉ',
        ('^', 'g') => 'ĝ',
        ('^', 's') => 'ŝ',
        ('^', 'w') => 'ŵ',
        ('^', 'y') => 'ŷ',
        ('^', 'A') => 'Â',
        ('^', 'E') => 'Ê',
        ('^', 'I') => 'Î',
        ('^', 'O') => 'Ô',
        ('^', 'U') => 'Û',
        ('"', 'a') => 'ä',
        ('"', 'e') => 'ë',
        ('"', 'i') => 'ï',
        ('"', 'o') => 'ö',
        ('"', 'u') => 'ü',
        ('"', 'y') => 'ÿ',
        ('"', 'A') => 'Ä',
        ('"', 'E') => 'Ë',
        ('"', 'I') => 'Ï',
        ('"', 'O') => 'Ö',
        ('"', 'U') => 'Ü',
        ('"', 'Y') => 'Ÿ',
        ('~', 'a') => 'ã',
        ('~', 'n') => 'ñ',
        ('~', 'o') => 'õ',
        ('~', 'A') => 'Ã',
        ('~', 'N') => 'Ñ',
        ('~', 'O') => 'Õ',
        ('=', 'a') => 'ā',
        ('=', 'e') => 'ē',
        ('=', 'i') => 'ī',
        ('=', 'o') => 'ō',
        ('=', 'u') => 'ū',
        ('=', 'A') => 'Ā',
        ('=', 'E') => 'Ē',
        ('=', 'I') => 'Ī',
        ('=', 'O') => 'Ō',
        ('=', 'U') => 'Ū',
        ('.', 'c') => 'ċ',
        ('.', 'e') => 'ė',
        ('.', 'g') => 'ġ',
        ('.', 'z') => 'ż',
        ('.', 'C') => 'Ċ',
        ('.', 'E') => 'Ė',
        ('.', 'G') => 'Ġ',
        ('.', 'Z') => 'Ż',
        ('c', 'c') => 'ç',
        ('c', 'C') => 'Ç',
        ('c', 's') => 'ş',
        ('c', 'S') => 'Ş',
        ('c', 't') => 'ţ',
        ('c', 'T') => 'Ţ',
        ('c', 'g') => 'ģ',
        ('v', 'c') => 'č',
        ('v', 's') => 'š',
        ('v', 'z') => 'ž',
        ('v', 'r') => 'ř',
        ('v', 'e') => 'ě',
        ('v', 'd') => 'ď',
        ('v', 't') => 'ť',
        ('v', 'n') => 'ň',
        ('v', 'C') => 'Č',
        ('v', 'S') => 'Š',
        ('v', 'Z') => 'Ž',
        ('v', 'R') => 'Ř',
        ('v', 'E') => 'Ě',
        ('v', 'N') => 'Ň',
        ('u', 'a') => 'ă',
        ('u', 'e') => 'ĕ',
        ('u', 'g') => 'ğ',
        ('u', 'u') => 'ŭ',
        ('u', 'A') => 'Ă',
        ('u', 'G') => 'Ğ',
        ('u', 'U') => 'Ŭ',
        ('H', 'o') => 'ő',
        ('H', 'u') => 'ű',
        ('H', 'O') => 'Ő',
        ('H', 'U') => 'Ű',
        ('r', 'a') => 'å',
        ('r', 'u') => 'ů',
        ('r', 'A') => 'Å',
        ('r', 'U') => 'Ů',
        ('k', 'a') => 'ą',
        ('k', 'e') => 'ę',
        ('k', 'A') => 'Ą',
        ('k', 'E') => 'Ę',
        _ => return None,
    };
    Some(c)
}

/// Text-mode symbol macros. Returns the Unicode form, or an ASCII fallback when
/// `unicode` is false.
fn symbol(name: &str, unicode: bool) -> Option<&'static str> {
    let (uni, ascii) = match name {
        "ldots" | "dots" | "textellipsis" => ("\u{2026}", "..."),
        "textemdash" => ("\u{2014}", "--"),
        "textendash" => ("\u{2013}", "-"),
        "ae" => ("æ", "ae"),
        "AE" => ("Æ", "AE"),
        "oe" => ("œ", "oe"),
        "OE" => ("Œ", "OE"),
        "aa" => ("å", "a"),
        "AA" => ("Å", "A"),
        "o" => ("ø", "o"),
        "O" => ("Ø", "O"),
        "ss" => ("ß", "ss"),
        "l" => ("ł", "l"),
        "L" => ("Ł", "L"),
        "i" => ("i", "i"),
        "j" => ("j", "j"),
        "S" => ("§", "S."),
        "P" => ("¶", "P."),
        "copyright" | "textcopyright" => ("©", "(c)"),
        "textregistered" => ("®", "(R)"),
        "texttrademark" => ("\u{2122}", "(TM)"),
        "pounds" | "textsterling" => ("£", "GBP"),
        "euro" | "texteuro" => ("€", "EUR"),
        "textdegree" | "degree" => ("°", " deg"),
        "textbullet" | "bullet" => ("\u{2022}", "*"),
        "dag" | "textdagger" => ("\u{2020}", "+"),
        "ddag" | "textdaggerdbl" => ("\u{2021}", "++"),
        "textbackslash" => ("\\", "\\"),
        "textasciitilde" => ("~", "~"),
        "textasciicircum" => ("^", "^"),
        "textunderscore" => ("_", "_"),
        "textbar" => ("|", "|"),
        "textless" => ("<", "<"),
        "textgreater" => (">", ">"),
        "textquoteleft" => ("\u{2018}", "'"),
        "textquoteright" => ("\u{2019}", "'"),
        "textquotedblleft" => ("\u{201C}", "\""),
        "textquotedblright" => ("\u{201D}", "\""),
        "times" => ("×", "x"),
        "div" => ("÷", "/"),
        "pm" => ("±", "+/-"),
        "LaTeX" => ("LaTeX", "LaTeX"),
        "LaTeXe" => ("LaTeX2e", "LaTeX2e"),
        "TeX" => ("TeX", "TeX"),
        "BibTeX" => ("BibTeX", "BibTeX"),
        _ => return None,
    };
    Some(if unicode { uni } else { ascii })
}

/// Collapse intra-line whitespace, trim every line, and reduce runs of blank
/// lines to a single one.
fn tidy(raw: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    for line in raw.split('\n') {
        let mut s = String::with_capacity(line.len());
        let mut prev_space = false;
        for ch in line.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    s.push(' ');
                }
                prev_space = true;
            } else {
                s.push(ch);
                prev_space = false;
            }
        }
        lines.push(s.trim().to_string());
    }
    let mut out: Vec<String> = Vec::new();
    for line in lines {
        if line.is_empty() && out.last().map(|l: &String| l.is_empty()).unwrap_or(true) {
            continue;
        }
        out.push(line);
    }
    while out.last().map(|l| l.is_empty()).unwrap_or(false) {
        out.pop();
    }
    out.join("\n")
        .replace(" .", ".")
        .replace(" ,", ",")
        .replace(" ;", ";")
        .replace(" :", ":")
        .replace(" !", "!")
        .replace(" ?", "?")
}

/// Default LaTeX-to-text conversion used by the simplest chat/page call.
pub fn run(text: &str) -> Result<String, String> {
    to_text(text, "remove", "drop", "", false, true, true, "paragraphs")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(src: &str) -> String {
        to_text(src, "remove", "drop", "", false, true, true, "paragraphs").unwrap()
    }

    const DOC: &str = r#"\documentclass{article}
\usepackage{amsmath}
% a preamble comment
\title{On Ducks}
\begin{document}
\maketitle
\section{Introduction}
The \textbf{mallard} is a common duck~\cite{smith2020}.  % inline note
It swims at $v = 3$ m/s.

\begin{itemize}
  \item Dabbles for food
  \item Migrates in winter
\end{itemize}
\end{document}
"#;

    #[test]
    fn happy_path_strips_a_whole_document() {
        assert_eq!(
            run(DOC),
            "Introduction\n\nThe mallard is a common duck. It swims at m/s.\n\nDabbles for food\nMigrates in winter"
        );
    }

    #[test]
    fn rejects_empty_input() {
        let err = to_text(
            "   \n ",
            "remove",
            "drop",
            "",
            false,
            true,
            true,
            "paragraphs",
        )
        .unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn rejects_unknown_math_mode() {
        let err = to_text("hi", "fancy", "drop", "", false, true, true, "paragraphs").unwrap_err();
        assert!(err.contains("unknown math mode 'fancy'"), "{err}");
    }

    #[test]
    fn rejects_unknown_citations_and_line_breaks_modes() {
        let e1 = to_text("hi", "remove", "all", "", false, true, true, "paragraphs").unwrap_err();
        assert!(e1.contains("unknown citations mode 'all'"), "{e1}");
        let e2 = to_text("hi", "remove", "drop", "", false, true, true, "wrap").unwrap_err();
        assert!(e2.contains("unknown line_breaks mode 'wrap'"), "{e2}");
    }

    #[test]
    fn rejects_oversized_input() {
        let big = "a".repeat(MAX_CHARS + 1);
        let err = to_text(&big, "remove", "drop", "", false, true, true, "paragraphs").unwrap_err();
        assert!(err.contains("the limit is 1000000"), "{err}");
        let ok = "a".repeat(MAX_CHARS);
        assert!(to_text(&ok, "remove", "drop", "", false, true, true, "paragraphs").is_ok());
    }

    #[test]
    fn errors_when_nothing_readable_remains() {
        let err = to_text(
            "\\documentclass{article}\n\\usepackage{geometry}\n",
            "remove",
            "drop",
            "",
            false,
            true,
            true,
            "paragraphs",
        )
        .unwrap_err();
        assert!(err.contains("no readable text remained"), "{err}");
    }

    #[test]
    fn math_modes() {
        let src = "Let $E = mc^2$ hold.";
        assert_eq!(run(src), "Let hold.");
        assert_eq!(
            to_text(src, "keep", "drop", "", false, true, true, "paragraphs").unwrap(),
            "Let $E = mc^2$ hold."
        );
        assert_eq!(
            to_text(
                src,
                "placeholder",
                "drop",
                "",
                false,
                true,
                true,
                "paragraphs"
            )
            .unwrap(),
            "Let [math] hold."
        );
    }

    #[test]
    fn display_math_and_math_environments() {
        let src = "Before\n\\[ a+b \\]\nAfter\n\\begin{equation}\nx = 1\n\\end{equation}\nEnd";
        assert_eq!(run(src), "Before After End");
        assert_eq!(
            to_text(
                src,
                "placeholder",
                "drop",
                "",
                false,
                true,
                true,
                "paragraphs"
            )
            .unwrap(),
            "Before [math] After [math] End"
        );
    }

    #[test]
    fn citation_keys_can_be_kept() {
        let src = "As shown \\cite{knuth1984} in \\ref{fig:one}.";
        assert_eq!(run(src), "As shown in.");
        assert_eq!(
            to_text(src, "remove", "keys", "", false, true, true, "paragraphs").unwrap(),
            "As shown knuth1984 in fig:one."
        );
    }

    #[test]
    fn comments_are_dropped_by_default_and_can_be_kept() {
        let src = "Visible text. % hidden note\n% whole line\nMore text.";
        assert_eq!(run(src), "Visible text. More text.");
        assert_eq!(
            to_text(src, "remove", "drop", "", true, true, true, "paragraphs").unwrap(),
            "Visible text. hidden note whole line\nMore text."
        );
        // An escaped percent is literal text, not a comment.
        assert_eq!(run("Up 5\\% today."), "Up 5% today.");
    }

    #[test]
    fn accents_and_symbols_become_unicode_or_ascii() {
        let src = "Caf\\'e na\\\"ive \\c{c}a \\ss{} \\ldots{} 10--20 and---so on.";
        assert_eq!(run(src), "Café naïve ça ß … 10\u{2013}20 and\u{2014}so on.");
        assert_eq!(
            to_text(src, "remove", "drop", "", false, false, true, "paragraphs").unwrap(),
            "Cafe naive ca ss... 10-20 and--so on."
        );
    }

    #[test]
    fn dropped_environments_are_configurable() {
        let src = "Intro.\n\\begin{tabular}{ll}\na & b \\\\\n\\end{tabular}\nOutro.";
        assert_eq!(run(src), "Intro.\n\nOutro.");
        // Take tabular off the list and its cell text survives.
        let kept = to_text(
            src,
            "remove",
            "drop",
            "verbatim",
            false,
            true,
            true,
            "paragraphs",
        )
        .unwrap();
        assert_eq!(kept, "Intro.\n\na b\n\nOutro.");
    }

    #[test]
    fn source_line_breaks_can_be_preserved() {
        let src = "One line\nsecond line\n\nNew paragraph";
        assert_eq!(run(src), "One line second line\n\nNew paragraph");
        assert_eq!(
            to_text(src, "remove", "drop", "", false, true, true, "source").unwrap(),
            "One line\nsecond line\n\nNew paragraph"
        );
    }

    #[test]
    fn body_only_can_be_switched_off() {
        let src = "\\title{T}\n\\begin{document}\nBody.\n\\end{document}";
        assert_eq!(run(src), "Body.");
        assert_eq!(
            to_text(src, "remove", "drop", "", false, true, false, "paragraphs").unwrap(),
            "T\n\nBody."
        );
    }

    #[test]
    fn keeps_visible_text_of_unknown_and_multi_arg_commands() {
        assert_eq!(run("A \\emph{bold} claim."), "A bold claim.");
        assert_eq!(
            run("See \\href{https://x.test}{the docs}."),
            "See the docs."
        );
        assert_eq!(run("\\textcolor{red}{Warning} ahead."), "Warning ahead.");
        assert_eq!(run("A \\madeupmacro{kept} word."), "A kept word.");
        assert_eq!(run("Run \\verb|cargo test| now."), "Run cargo test now.");
    }

    #[test]
    fn a_fragment_without_a_document_environment_still_converts() {
        assert_eq!(run("Just \\textit{some} prose."), "Just some prose.");
    }

    #[test]
    fn unterminated_constructs_do_not_panic() {
        assert_eq!(run("Open $x + 1"), "Open");
        assert!(to_text(
            "\\begin{verbatim}\nno end here",
            "remove",
            "drop",
            "",
            false,
            true,
            true,
            "paragraphs"
        )
        .is_err());
    }
}
