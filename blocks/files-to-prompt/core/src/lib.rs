//! files-to-prompt core — turn a set of pasted files into one LLM-ready digest:
//! a directory tree, each file's contents (fenced / plain / Claude-XML), and a
//! rough token estimate. Pure compute, no wafer/wasm-bindgen deps — shared by
//! the chat skill block and the web page.
//!
//! Input format: the pasted `files` blob is split into files by **header
//! lines**. A header line begins (after any leading whitespace) with the
//! `separator` token followed by whitespace, then the file's path, e.g.
//! `=== src/main.rs` (a trailing repeat of the separator, `=== src/main.rs ===`,
//! is also accepted). Everything up to the next header (or end of input) is that
//! file's content; a run of blank lines directly above/below each file's content
//! is trimmed. Text before the first header is ignored.

use std::collections::BTreeMap;

/// The default file-header marker when the caller passes an empty `separator`.
pub const DEFAULT_SEPARATOR: &str = "===";

/// Output rendering style.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Format {
    /// A `## path` heading + a language-fenced code block per file.
    Markdown,
    /// A single Claude-style `<documents>` wrapper with one indexed
    /// `<document>` per file.
    Xml,
    /// files-to-prompt's default: `path` / `---` / contents / `---` per file.
    Plain,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "markdown" => Ok(Format::Markdown),
            "xml" => Ok(Format::Xml),
            "plain" => Ok(Format::Plain),
            other => Err(format!(
                "invalid format {other:?}: expected \"markdown\", \"xml\", or \"plain\""
            )),
        }
    }
}

/// One parsed file: its declared path + its (blank-line-trimmed) content.
struct File {
    path: String,
    content: String,
}

/// Build the LLM-ready digest.
///
/// - `files`: the concatenated files, each preceded by a header line
///   `<separator> <path>` (see the module docs for the exact format).
/// - `format` (`"markdown"` default | `"xml"` | `"plain"`): the output style.
/// - `separator` (blank → `"==="`): the marker that begins each header line.
/// - `line_numbers`: prefix every content line with its right-aligned line number.
/// - `include_tree`: prepend a `Directory structure:` tree built from the paths.
///
/// Returns `Err` for an unknown `format` or when no header lines are found.
pub fn build_digest(
    files: &str,
    format: &str,
    separator: &str,
    line_numbers: bool,
    include_tree: bool,
) -> Result<String, String> {
    let fmt = Format::parse(format)?;
    let sep = {
        let s = separator.trim();
        if s.is_empty() {
            DEFAULT_SEPARATOR
        } else {
            s
        }
    };

    let parsed = parse_files(files, sep);
    if parsed.is_empty() {
        return Err(format!(
            "no files found: start each file with a header line like \"{sep} path/to/file.ext\""
        ));
    }

    let mut sections: Vec<String> = Vec::new();
    if include_tree {
        sections.push(render_tree(&parsed));
    }
    sections.push(render_files(&parsed, fmt, line_numbers));
    let body = sections.join("\n\n");

    // Estimate tokens of the digest body (~4 chars/token — the common rule of
    // thumb; exact model tokenizers need per-model BPE tables we don't embed).
    let chars = body.chars().count();
    let tokens = chars.div_ceil(4);
    let file_word = if parsed.len() == 1 { "file" } else { "files" };
    let summary = format!(
        "{} {}, {} characters, ~{} tokens (estimate)",
        parsed.len(),
        file_word,
        chars,
        tokens
    );

    Ok(format!("{body}\n\n{summary}"))
}

/// Split `blob` into files at header lines beginning with `sep`.
fn parse_files(blob: &str, sep: &str) -> Vec<File> {
    let mut files: Vec<File> = Vec::new();
    let mut cur_path: Option<String> = None;
    let mut cur_lines: Vec<&str> = Vec::new();

    let flush = |path: Option<String>, lines: &[&str], out: &mut Vec<File>| {
        if let Some(path) = path {
            out.push(File {
                path,
                content: trim_blank_edges(lines),
            });
        }
    };

    for line in blob.lines() {
        if let Some(path) = parse_header(line, sep) {
            flush(cur_path.take(), &cur_lines, &mut files);
            cur_lines.clear();
            cur_path = Some(path);
        } else if cur_path.is_some() {
            cur_lines.push(line);
        }
        // lines before the first header (preamble) are ignored.
    }
    flush(cur_path.take(), &cur_lines, &mut files);
    files
}

/// If `line` is a file-header line for `sep`, return its path. A header is a
/// line that, after leading whitespace, starts with `sep` **followed by
/// whitespace**, then a non-empty path (an optional trailing `sep` is stripped).
/// The required whitespace stops content rules like `====` or a bare `===`
/// setext underline from being misread as headers.
fn parse_header(line: &str, sep: &str) -> Option<String> {
    let rest = line.trim_start().strip_prefix(sep)?;
    // The char immediately after the separator must be whitespace.
    if !rest.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }
    let mut path = rest.trim();
    if let Some(stripped) = path.strip_suffix(sep) {
        path = stripped.trim();
    }
    if path.is_empty() {
        None
    } else {
        Some(path.to_string())
    }
}

/// Join `lines` with `\n`, dropping runs of blank (whitespace-only) lines at the
/// very start and end so each file's content is tightly framed.
fn trim_blank_edges(lines: &[&str]) -> String {
    let is_blank = |l: &&str| l.trim().is_empty();
    let Some(start) = lines.iter().position(|l| !is_blank(l)) else {
        return String::new();
    };
    let end = lines.iter().rposition(|l| !is_blank(l)).unwrap();
    lines[start..=end].join("\n")
}

/// Render every file per `fmt`.
fn render_files(files: &[File], fmt: Format, line_numbers: bool) -> String {
    match fmt {
        Format::Markdown => files
            .iter()
            .map(|f| {
                let body = maybe_number(&f.content, line_numbers);
                let fence = fence_for(&body);
                let lang = detect_language(&f.path);
                format!("## {}\n{fence}{lang}\n{body}\n{fence}", f.path)
            })
            .collect::<Vec<_>>()
            .join("\n\n"),
        Format::Plain => files
            .iter()
            .map(|f| {
                let body = maybe_number(&f.content, line_numbers);
                format!("{}\n---\n{body}\n---", f.path)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Xml => {
            let mut s = String::from("<documents>\n");
            for (i, f) in files.iter().enumerate() {
                let body = maybe_number(&f.content, line_numbers);
                s.push_str(&format!(
                    "<document index=\"{}\">\n<source>{}</source>\n<document_contents>\n{body}\n</document_contents>\n</document>\n",
                    i + 1,
                    f.path
                ));
            }
            s.push_str("</documents>");
            s
        }
    }
}

/// Prefix each line of `content` with its right-aligned 1-based line number
/// (two spaces then the line), when `on`.
fn maybe_number(content: &str, on: bool) -> String {
    if !on {
        return content.to_string();
    }
    let lines: Vec<&str> = content.split('\n').collect();
    let width = lines.len().to_string().len();
    lines
        .iter()
        .enumerate()
        .map(|(i, l)| format!("{:>width$}  {l}", i + 1))
        .collect::<Vec<_>>()
        .join("\n")
}

/// The code-fence to use for `content`: at least three backticks, and one more
/// than the longest backtick run inside the content so a fenced block that
/// itself contains ``` is not closed early.
fn fence_for(content: &str) -> String {
    let mut longest = 0usize;
    let mut run = 0usize;
    for ch in content.chars() {
        if ch == '`' {
            run += 1;
            longest = longest.max(run);
        } else {
            run = 0;
        }
    }
    "`".repeat((longest + 1).max(3))
}

/// A `Directory structure:` header + a box-drawing tree built from the file
/// paths (sorted, deterministic).
fn render_tree(files: &[File]) -> String {
    #[derive(Default)]
    struct Node {
        children: BTreeMap<String, Node>,
    }
    let mut root = Node::default();
    for f in files {
        let mut node = &mut root;
        for seg in f.path.split('/').filter(|s| !s.is_empty() && *s != ".") {
            node = node.children.entry(seg.to_string()).or_default();
        }
    }
    fn walk(node: &BTreeMap<String, Node>, prefix: &str, out: &mut Vec<String>) {
        let n = node.len();
        for (i, (name, child)) in node.iter().enumerate() {
            let last = i == n - 1;
            let connector = if last { "└── " } else { "├── " };
            out.push(format!("{prefix}{connector}{name}"));
            if !child.children.is_empty() {
                let ext = if last { "    " } else { "│   " };
                walk(&child.children, &format!("{prefix}{ext}"), out);
            }
        }
    }
    let mut lines = vec!["Directory structure:".to_string()];
    walk(&root.children, "", &mut lines);
    lines.join("\n")
}

/// Map a path to a Markdown code-fence language tag (`""` when unknown).
fn detect_language(path: &str) -> &'static str {
    let name = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    match name.as_str() {
        "dockerfile" => return "dockerfile",
        "makefile" | "gnumakefile" => return "makefile",
        "cmakelists.txt" => return "cmake",
        _ => {}
    }
    let ext = name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
    match ext {
        "rs" => "rust",
        "py" | "pyw" => "python",
        "js" | "mjs" | "cjs" => "javascript",
        "jsx" => "jsx",
        "ts" | "mts" | "cts" => "typescript",
        "tsx" => "tsx",
        "java" => "java",
        "kt" | "kts" => "kotlin",
        "c" | "h" => "c",
        "cpp" | "cc" | "cxx" | "hpp" | "hh" | "hxx" => "cpp",
        "cs" => "csharp",
        "go" => "go",
        "rb" => "ruby",
        "php" => "php",
        "swift" => "swift",
        "scala" | "sc" => "scala",
        "sh" | "bash" | "zsh" => "bash",
        "ps1" | "psm1" => "powershell",
        "bat" | "cmd" => "batch",
        "sql" => "sql",
        "html" | "htm" => "html",
        "xml" | "svg" => "xml",
        "css" => "css",
        "scss" => "scss",
        "sass" => "sass",
        "less" => "less",
        "json" | "jsonc" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        "ini" | "cfg" | "conf" => "ini",
        "md" | "markdown" => "markdown",
        "csv" => "csv",
        "tsv" => "tsv",
        "r" => "r",
        "lua" => "lua",
        "dart" => "dart",
        "ex" | "exs" => "elixir",
        "erl" | "hrl" => "erlang",
        "hs" => "haskell",
        "clj" | "cljs" | "cljc" => "clojure",
        "vue" => "vue",
        "svelte" => "svelte",
        "proto" => "protobuf",
        "graphql" | "gql" => "graphql",
        "tf" => "hcl",
        "vim" => "vim",
        "pl" | "pm" => "perl",
        "diff" | "patch" => "diff",
        _ => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "=== src/main.rs\nfn main() {}\n\n=== README.md\n# Title";

    /// Split the digest into (body, summary-line) at the final `\n\n` — the
    /// summary is a single line with no interior `\n\n`, so this is unambiguous.
    fn split(out: &str) -> (&str, &str) {
        out.rsplit_once("\n\n").expect("digest has a summary footer")
    }

    #[test]
    fn markdown_digest_body_has_tree_then_fenced_files_in_input_order() {
        let out = build_digest(SAMPLE, "markdown", "", false, true).unwrap();
        let (body, summary) = split(&out);
        // Tree is sorted (README before src); files keep input order (main first).
        // Built with explicit "\n" (no `\`-continuation, which would strip the
        // leading spaces on the indented "    └── main.rs" line).
        let expected = concat!(
            "Directory structure:\n",
            "├── README.md\n",
            "└── src\n",
            "    └── main.rs\n",
            "\n",
            "## src/main.rs\n",
            "```rust\n",
            "fn main() {}\n",
            "```\n",
            "\n",
            "## README.md\n",
            "```markdown\n",
            "# Title\n",
            "```",
        );
        assert_eq!(body, expected);
        assert!(summary.starts_with("2 files,"), "got: {summary}");
        assert!(summary.ends_with("tokens (estimate)"), "got: {summary}");
    }

    #[test]
    fn plain_format_matches_files_to_prompt_shape() {
        let out = build_digest(SAMPLE, "plain", "", false, false).unwrap();
        let (body, _) = split(&out);
        let expected = "src/main.rs\n\
---\n\
fn main() {}\n\
---\n\
README.md\n\
---\n\
# Title\n\
---";
        assert_eq!(body, expected);
    }

    #[test]
    fn xml_format_wraps_files_in_one_documents_block() {
        let out = build_digest("=== a.txt\nhello", "xml", "", false, false).unwrap();
        let (body, summary) = split(&out);
        let expected = "<documents>\n\
<document index=\"1\">\n\
<source>a.txt</source>\n\
<document_contents>\n\
hello\n\
</document_contents>\n\
</document>\n\
</documents>";
        assert_eq!(body, expected);
        assert!(summary.starts_with("1 file,"), "singular: {summary}");
    }

    #[test]
    fn summary_token_estimate_is_chars_div_ceil_four() {
        // Body = plain single file "a\n---\nhi\n---" = 12 ASCII chars → ceil(12/4)=3.
        let out = build_digest("=== a\nhi", "plain", "", false, false).unwrap();
        assert!(
            out.ends_with("1 file, 12 characters, ~3 tokens (estimate)"),
            "got: {out}"
        );
    }

    #[test]
    fn line_numbers_prefix_each_content_line_right_aligned() {
        let out = build_digest("=== a.py\nfirst\nsecond", "plain", "", true, false).unwrap();
        assert!(out.contains("1  first\n2  second"), "got: {out}");
    }

    #[test]
    fn custom_separator_and_trailing_marker_both_parse() {
        let out = build_digest(">>> a.txt >>>\nhi\n>>> b.txt\nbye", "plain", ">>>", false, false)
            .unwrap();
        assert!(out.contains("a.txt\n---\nhi\n---"), "got: {out}");
        assert!(out.contains("b.txt\n---\nbye\n---"), "got: {out}");
    }

    #[test]
    fn backtick_fence_escalates_past_content_backticks() {
        let out = build_digest("=== doc.md\n```js\nx\n```", "markdown", "", false, false).unwrap();
        assert!(out.contains("````markdown\n```js"), "got: {out}");
    }

    #[test]
    fn nested_tree_renders_connectors() {
        let out = build_digest("=== a/b/c.rs\nx\n=== a/d.rs\ny\n=== top.md\nz", "plain", "", false, true)
            .unwrap();
        let tree = out.split("\n\n").next().unwrap();
        let expected = "Directory structure:\n\
├── a\n\
│   ├── b\n\
│   │   └── c.rs\n\
│   └── d.rs\n\
└── top.md";
        assert_eq!(tree, expected);
    }

    #[test]
    fn blank_edges_trimmed_interior_preserved() {
        let out = build_digest(
            "=== a.txt\n\n\nkeep\n\nmiddle\n\n\n=== b.txt\nx",
            "plain",
            "",
            false,
            false,
        )
        .unwrap();
        assert!(out.contains("a.txt\n---\nkeep\n\nmiddle\n---"), "got: {out}");
    }

    #[test]
    fn preamble_before_first_header_is_ignored() {
        let out = build_digest("some notes\nmore notes\n=== a.txt\nbody", "plain", "", false, false)
            .unwrap();
        assert!(out.contains("a.txt\n---\nbody\n---"), "got: {out}");
        assert!(!out.contains("some notes"), "preamble leaked: {out}");
    }

    #[test]
    fn content_rule_lines_are_not_misread_as_headers() {
        // A markdown setext underline `===` and a `====` rule are content, not
        // headers (no whitespace-delimited path follows the separator).
        let out = build_digest("=== notes.md\nTitle\n===\nmore\n====", "plain", "", false, false)
            .unwrap();
        assert!(
            out.contains("notes.md\n---\nTitle\n===\nmore\n====\n---"),
            "got: {out}"
        );
    }

    #[test]
    fn language_detected_from_extension_and_special_names() {
        assert_eq!(detect_language("src/main.rs"), "rust");
        assert_eq!(detect_language("app/index.tsx"), "tsx");
        assert_eq!(detect_language("Dockerfile"), "dockerfile");
        assert_eq!(detect_language("deep/dir/Makefile"), "makefile");
        assert_eq!(detect_language("data.unknownext"), "");
        assert_eq!(detect_language("noext"), "");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = build_digest(SAMPLE, "yaml", "", false, false).unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }

    #[test]
    fn errors_when_no_header_lines_present() {
        let err =
            build_digest("just some text\nno headers here", "plain", "", false, false).unwrap_err();
        assert!(err.contains("no files found"), "got: {err}");
    }
}
