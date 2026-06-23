//! file-tree-generator core — turn a list of file paths or an indented outline
//! into an ASCII tree diagram for READMEs and docs.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//!
//! Two input modes:
//! - `paths` (default): each non-blank line is a slash-separated path
//!   (`src/main.rs`); shared leading segments are merged so the lines build one
//!   tree. A trailing `/` marks an explicit directory.
//! - `outline`: each non-blank line is a node and its leading whitespace (spaces
//!   or tabs) sets the nesting, like a hand-drawn outline.
//!
//! Output is the familiar `├──`/`└──`/`│` box-drawing tree (or ASCII `|--`/`` `-- ``
//! when `ascii` is set).

/// Input interpretation.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Slash-separated paths, merged on shared prefixes.
    Paths,
    /// Indentation-based outline (like `outline-to-json`).
    Outline,
}

impl Mode {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "paths" => Ok(Mode::Paths),
            "outline" => Ok(Mode::Outline),
            other => Err(format!(
                "invalid mode {other:?}: expected \"paths\" or \"outline\""
            )),
        }
    }
}

/// Box-drawing glyphs for the tree connectors.
struct Glyphs {
    tee: &'static str,   // a non-last child:  "├── "
    elbow: &'static str, // the last child:    "└── "
    pipe: &'static str,  // an open ancestor:  "│   "
    blank: &'static str, // a closed ancestor: "    "
}

impl Glyphs {
    fn unicode() -> Self {
        Glyphs {
            tee: "├── ",
            elbow: "└── ",
            pipe: "│   ",
            blank: "    ",
        }
    }
    fn ascii() -> Self {
        Glyphs {
            tee: "|-- ",
            elbow: "`-- ",
            pipe: "|   ",
            blank: "    ",
        }
    }
}

/// A node in the tree. `is_dir` is set when the node was given a trailing slash
/// or has children; it controls the trailing `/` in the rendered label.
struct Node {
    name: String,
    children: Vec<Node>,
    is_dir: bool,
}

impl Node {
    fn new(name: String) -> Self {
        Node {
            name,
            children: Vec::new(),
            is_dir: false,
        }
    }

    /// Find a direct child by name, or create + return it.
    fn child_mut(&mut self, name: &str) -> &mut Node {
        if let Some(i) = self.children.iter().position(|c| c.name == name) {
            return &mut self.children[i];
        }
        self.children.push(Node::new(name.to_string()));
        let last = self.children.len() - 1;
        &mut self.children[last]
    }
}

/// Render `input` as an ASCII/Unicode file tree.
///
/// - `mode` (`"paths"` | `"outline"`, blank → `"paths"`): how to read the input.
/// - `root`: the label for the root line. Blank → `"."`.
/// - `ascii`: use plain-ASCII connectors (`|--`, `` `-- ``) instead of Unicode
///   box-drawing (`├──`, `└──`). Default false.
/// - `trailing_slash`: append `/` to directory names. Default true.
/// - `sort`: sort each directory's entries (directories first, then files,
///   each alphabetical). Default false → input order is preserved.
///
/// Returns `Err` on an invalid `mode`, empty input, or — in outline mode — when
/// the first content line is indented (there is no root to attach it to).
pub fn generate(
    input: &str,
    mode: &str,
    root: &str,
    ascii: bool,
    trailing_slash: bool,
    sort: bool,
) -> Result<String, String> {
    let mode = Mode::parse(mode)?;
    let root_label = {
        let r = root.trim();
        if r.is_empty() {
            ".".to_string()
        } else {
            r.to_string()
        }
    };

    let mut tree = Node::new(root_label);
    tree.is_dir = true;

    match mode {
        Mode::Paths => build_from_paths(input, &mut tree)?,
        Mode::Outline => build_from_outline(input, &mut tree)?,
    }

    if sort {
        sort_tree(&mut tree);
    }

    let glyphs = if ascii { Glyphs::ascii() } else { Glyphs::unicode() };

    let mut out = String::new();
    // The root is a label, not a tree entry — never decorate it with a slash.
    out.push_str(&tree.name);
    out.push('\n');
    let mut prefix = String::new();
    render_children(&tree, &mut prefix, &glyphs, trailing_slash, &mut out);
    Ok(out)
}

/// Build the tree from slash-separated paths, merging shared prefixes.
fn build_from_paths(input: &str, tree: &mut Node) -> Result<(), String> {
    let mut any = false;
    for raw in input.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let explicit_dir = line.ends_with('/') || line.ends_with('\\');
        // Split on both '/' and '\' so Windows-style paths also work; drop empty
        // segments from leading/trailing/duplicate separators.
        let segments: Vec<&str> = line
            .split(['/', '\\'])
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if segments.is_empty() {
            continue;
        }
        any = true;
        let last = segments.len() - 1;
        let mut cur = &mut *tree;
        for (i, seg) in segments.iter().enumerate() {
            cur = cur.child_mut(seg);
            if i != last {
                cur.is_dir = true;
            } else if explicit_dir {
                cur.is_dir = true;
            }
        }
    }
    if !any {
        return Err("input is empty: provide at least one non-blank path".into());
    }
    Ok(())
}

/// Build the tree from an indented outline (whitespace sets nesting).
fn build_from_outline(input: &str, tree: &mut Node) -> Result<(), String> {
    // (indent_columns, name, explicit_dir) for each meaningful line.
    let lines: Vec<(usize, String, bool)> = input
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let indent = indent_columns(l);
            let t = l.trim();
            let explicit_dir = t.ends_with('/');
            let name = t.trim_end_matches('/').trim().to_string();
            (indent, name, explicit_dir)
        })
        .filter(|(_, name, _)| !name.is_empty())
        .collect();

    if lines.is_empty() {
        return Err("input is empty: provide at least one non-blank line".into());
    }
    if lines[0].0 != 0 {
        return Err("the first line must not be indented (it is attached to the root)".into());
    }

    // path = child indices from the root down to the current ancestor.
    // stack = indent column for each open level (parallel to path).
    let mut stack: Vec<usize> = Vec::new();
    let mut path: Vec<usize> = Vec::new();

    for (indent, name, explicit_dir) in &lines {
        while let Some(&top) = stack.last() {
            if top >= *indent {
                stack.pop();
                path.pop();
            } else {
                break;
            }
        }
        let parent = node_at_path(tree, &path);
        parent.is_dir = true;
        let mut n = Node::new(name.clone());
        n.is_dir = *explicit_dir;
        parent.children.push(n);
        let idx = parent.children.len() - 1;
        stack.push(*indent);
        path.push(idx);
    }
    Ok(())
}

/// Walk `path` (a chain of child indices) from `root` and return the node there.
fn node_at_path<'a>(root: &'a mut Node, path: &[usize]) -> &'a mut Node {
    let mut cur = root;
    for &idx in path {
        cur = &mut cur.children[idx];
    }
    cur
}

/// Count leading-whitespace columns; a tab counts as 4. Stops at first non-WS.
fn indent_columns(line: &str) -> usize {
    let mut cols = 0;
    for ch in line.chars() {
        match ch {
            ' ' => cols += 1,
            '\t' => cols += 4,
            _ => break,
        }
    }
    cols
}

/// Sort every directory: directories before files, each group alphabetical
/// (case-insensitive). A node counts as a directory if it has children or is
/// explicitly flagged.
fn sort_tree(node: &mut Node) {
    node.children.sort_by(|a, b| {
        let a_dir = a.is_dir || !a.children.is_empty();
        let b_dir = b.is_dir || !b.children.is_empty();
        b_dir
            .cmp(&a_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for c in &mut node.children {
        sort_tree(c);
    }
}

/// The display label for a node (adds a trailing `/` for directories).
fn render_label(node: &Node, trailing_slash: bool) -> String {
    let is_dir = node.is_dir || !node.children.is_empty();
    if trailing_slash && is_dir {
        format!("{}/\n", node.name)
    } else {
        format!("{}\n", node.name)
    }
}

/// Recursively render the children of `node`, appending to `out`. `prefix` holds
/// the accumulated `│   `/`    ` columns for the current depth.
fn render_children(
    node: &Node,
    prefix: &mut String,
    glyphs: &Glyphs,
    trailing_slash: bool,
    out: &mut String,
) {
    let n = node.children.len();
    for (i, child) in node.children.iter().enumerate() {
        let last = i == n - 1;
        out.push_str(prefix);
        out.push_str(if last { glyphs.elbow } else { glyphs.tee });
        out.push_str(&render_label(child, trailing_slash));

        let added = if last { glyphs.blank } else { glyphs.pipe };
        prefix.push_str(added);
        render_children(child, prefix, glyphs, trailing_slash, out);
        prefix.truncate(prefix.len() - added.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_merge_on_shared_prefix() {
        let out = generate(
            "src/main.rs\nsrc/lib.rs\nREADME.md",
            "paths",
            ".",
            false,
            true,
            false,
        )
        .unwrap();
        let expected = "\
.\n\
├── src/\n\
│   ├── main.rs\n\
│   └── lib.rs\n\
└── README.md\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn ascii_connectors() {
        let out = generate("a/b\nc", "paths", "root", true, true, false).unwrap();
        let expected = "\
root\n\
|-- a/\n\
|   `-- b\n\
`-- c\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn explicit_trailing_slash_marks_dir() {
        let out = generate("docs/\nfile.txt", "paths", ".", false, true, false).unwrap();
        assert!(out.contains("├── docs/"), "got: {out}");
        assert!(out.contains("└── file.txt"), "got: {out}");
    }

    #[test]
    fn trailing_slash_disabled() {
        let out = generate("src/main.rs", "paths", ".", false, false, false).unwrap();
        assert!(out.contains("└── src\n"), "got: {out}");
        assert!(!out.contains("src/"), "got: {out}");
    }

    #[test]
    fn outline_mode_nests_by_indentation() {
        let out = generate("a\n  b\n  c\n    d\ne", "outline", ".", false, true, false).unwrap();
        let expected = "\
.\n\
├── a/\n\
│   ├── b\n\
│   └── c/\n\
│       └── d\n\
└── e\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn outline_tabs_count_as_four() {
        let out = generate("a\n\tb", "outline", ".", false, true, false).unwrap();
        assert!(out.contains("└── b"), "got: {out}");
    }

    #[test]
    fn sort_directories_first_then_alpha() {
        let out = generate(
            "zebra.txt\nalpha/x\nbeta.md\nalpha/a",
            "paths",
            ".",
            false,
            true,
            true,
        )
        .unwrap();
        let expected = "\
.\n\
├── alpha/\n\
│   ├── a\n\
│   └── x\n\
├── beta.md\n\
└── zebra.txt\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn windows_backslash_paths() {
        let out = generate("src\\main.rs", "paths", ".", false, true, false).unwrap();
        assert!(
            out.contains("├── src/") || out.contains("└── src/"),
            "got: {out}"
        );
        assert!(out.contains("main.rs"), "got: {out}");
    }

    #[test]
    fn duplicate_paths_collapse() {
        let out = generate("a/b\na/b", "paths", ".", false, true, false).unwrap();
        assert_eq!(out.matches("b").count(), 1, "got: {out}");
    }

    #[test]
    fn custom_root_label() {
        let out = generate("x", "paths", "myproject", false, true, false).unwrap();
        assert!(out.starts_with("myproject\n"), "got: {out}");
    }

    #[test]
    fn blank_root_defaults_to_dot() {
        let out = generate("x", "paths", "", false, true, false).unwrap();
        assert!(out.starts_with(".\n"), "got: {out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = generate("   \n\n", "paths", ".", false, true, false).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn outline_first_line_indented_is_an_error() {
        let err = generate("  a\nb", "outline", ".", false, true, false).unwrap_err();
        assert!(err.contains("first line"), "got: {err}");
    }

    #[test]
    fn invalid_mode_is_an_error() {
        let err = generate("a", "bogus", ".", false, true, false).unwrap_err();
        assert!(err.contains("invalid mode"), "got: {err}");
    }

    #[test]
    fn leading_slash_is_ignored() {
        let out = generate("/etc/hosts", "paths", ".", false, true, false).unwrap();
        assert!(
            out.contains("├── etc/") || out.contains("└── etc/"),
            "got: {out}"
        );
        assert!(out.contains("hosts"), "got: {out}");
    }
}
