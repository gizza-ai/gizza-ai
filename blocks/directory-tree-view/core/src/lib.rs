//! directory-tree-view core — turn a pasted file listing (path + byte size per
//! line) into a clean indented tree annotated with per-entry sizes and per-directory
//! counts, like `tree -s -h --du`. No wafer/wasm-bindgen deps; shared by the chat
//! skill block and the web page.
//!
//! The browser has no access to a real folder, so the input is a LISTING that a
//! user pastes — one entry per line, each line a slash-separated path and a byte
//! size. Three line shapes are recognised (`format`):
//! - `size-first`  — `1234\tsrc/main.rs`  (`du -ab`, `find . -printf '%s\t%p\n'`)
//! - `path-first`  — `src/main.rs,1234`   (`path,size` CSV / spreadsheet export)
//! - `auto` (default) detects per line.
//!
//! Directory sizes are the accumulation of everything beneath them (`--du`
//! semantics): any path that turns out to have children is a directory whose size
//! is recomputed from its descendants — so `du -a` output (which also lists
//! directories) self-corrects instead of double-counting.

/// How each input line is split into a path and a size.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    Auto,
    SizeFirst,
    PathFirst,
}

impl Format {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "auto" => Ok(Format::Auto),
            "size-first" => Ok(Format::SizeFirst),
            "path-first" => Ok(Format::PathFirst),
            other => Err(format!(
                "invalid format {other:?}: expected \"auto\", \"size-first\", or \"path-first\""
            )),
        }
    }
}

/// Output size unit style.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Units {
    /// 1024-based K/M/G/T… (like `tree -h`).
    Human,
    /// 1000-based k/M/G/T… (like `tree --si`).
    Si,
    /// Raw bytes with thousands separators (like `tree -s`).
    Bytes,
}

impl Units {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "human" => Ok(Units::Human),
            "si" => Ok(Units::Si),
            "bytes" => Ok(Units::Bytes),
            other => Err(format!(
                "invalid units {other:?}: expected \"human\", \"si\", or \"bytes\""
            )),
        }
    }
}

/// Ordering of entries within each directory.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    /// Alphabetical by name (case-insensitive).
    Name,
    /// Largest cumulative size first.
    SizeDesc,
    /// Preserve the order the paths first appeared in the input.
    Input,
}

impl Sort {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "name" => Ok(Sort::Name),
            "size-desc" => Ok(Sort::SizeDesc),
            "input" => Ok(Sort::Input),
            other => Err(format!(
                "invalid sort {other:?}: expected \"name\", \"size-desc\", or \"input\""
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
    fn pick(ascii: bool) -> Self {
        if ascii {
            Glyphs { tee: "|-- ", elbow: "`-- ", pipe: "|   ", blank: "    " }
        } else {
            Glyphs { tee: "├── ", elbow: "└── ", pipe: "│   ", blank: "    " }
        }
    }
}

/// A node in the tree. `own` is the byte size given on the leaf's own line; it is
/// only used when the node has no children (a file, or an explicitly-listed empty
/// directory). `order` records first-seen input position for `Sort::Input`.
struct Node {
    name: String,
    children: Vec<Node>,
    own: u64,
    order: usize,
}

impl Node {
    fn new(name: String, order: usize) -> Self {
        Node { name, children: Vec::new(), own: 0, order }
    }

    fn child_mut(&mut self, name: &str, next_order: &mut usize) -> &mut Node {
        // Linear scan keeps insertion order and is fine for pasted listings.
        if let Some(i) = self.children.iter().position(|c| c.name == name) {
            return &mut self.children[i];
        }
        let order = *next_order;
        *next_order += 1;
        self.children.push(Node::new(name.to_string(), order));
        self.children.last_mut().unwrap()
    }

    fn is_dir(&self) -> bool {
        !self.children.is_empty()
    }

    /// Cumulative size: a directory (has children) is the sum of its descendants;
    /// a leaf uses its own listed size.
    fn total(&self) -> u64 {
        if self.children.is_empty() {
            self.own
        } else {
            self.children.iter().map(Node::total).sum()
        }
    }

    /// (files, directories) contained within this node's subtree (excluding self).
    fn counts(&self) -> (u64, u64) {
        let (mut files, mut dirs) = (0u64, 0u64);
        for c in &self.children {
            if c.is_dir() {
                dirs += 1;
                let (f, d) = c.counts();
                files += f;
                dirs += d;
            } else {
                files += 1;
            }
        }
        (files, dirs)
    }
}

/// Parse a byte-size token, accepting optional unit suffixes (`1234`, `4K`, `1.5M`,
/// `2MiB`, `500KB`). Binary suffixes (K/Ki/M/Mi…) are 1024-based; explicit decimal
/// suffixes (KB/MB…) are 1000-based.
fn parse_size(tok: &str) -> Result<u64, String> {
    let t = tok.trim();
    if t.is_empty() {
        return Err("missing size".into());
    }
    let split = t
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(t.len());
    let (num_s, unit_s) = t.split_at(split);
    if num_s.is_empty() {
        return Err(format!("invalid size {tok:?}: expected a number"));
    }
    let num: f64 = num_s
        .parse()
        .map_err(|_| format!("invalid size {tok:?}: {num_s:?} is not a number"))?;
    if num < 0.0 {
        return Err(format!("invalid size {tok:?}: must not be negative"));
    }
    let mult: f64 = match unit_s.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "k" | "kib" => 1024.0,
        "kb" => 1000.0,
        "m" | "mib" => 1024.0 * 1024.0,
        "mb" => 1_000_000.0,
        "g" | "gib" => 1024.0 * 1024.0 * 1024.0,
        "gb" => 1_000_000_000.0,
        "t" | "tib" => 1024f64.powi(4),
        "tb" => 1e12,
        "p" | "pib" => 1024f64.powi(5),
        "pb" => 1e15,
        other => return Err(format!("invalid size unit {other:?} in {tok:?}")),
    };
    Ok((num * mult).round() as u64)
}

/// Split one line into (path, size) according to `format`. Returns None for blank
/// lines (they are skipped by the caller).
fn split_line(line: &str, format: Format, lineno: usize) -> Result<Option<(String, u64)>, String> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.trim().is_empty() {
        return Ok(None);
    }
    let err = |m: &str| format!("line {lineno}: {m} in {line:?}");

    let (path, size) = match format {
        Format::SizeFirst => {
            let (a, b) = split_first(line).ok_or_else(|| err("expected a size then a path"))?;
            (b, parse_size(a).map_err(|e| err(&e))?)
        }
        Format::PathFirst => {
            let (a, b) = split_last(line).ok_or_else(|| err("expected a path then a size"))?;
            (a, parse_size(b).map_err(|e| err(&e))?)
        }
        Format::Auto => {
            // CSV → path,size. A tab → du/find size\tpath. Otherwise sniff which
            // end is the number.
            if line.contains(',') && !line.contains('\t') {
                let (a, b) = split_last(line).ok_or_else(|| err("empty CSV line"))?;
                (a, parse_size(b).map_err(|e| err(&e))?)
            } else if let Some((a, b)) = split_first(line).filter(|(a, _)| parse_size(a).is_ok()) {
                (b, parse_size(a).unwrap())
            } else if let Some((a, b)) = split_last(line) {
                (a, parse_size(b).map_err(|e| err(&e))?)
            } else {
                return Err(err("expected a path and a size"));
            }
        }
    };
    let path = path.trim().to_string();
    if path.is_empty() {
        return Err(err("empty path"));
    }
    Ok(Some((path, size)))
}

/// Split off the FIRST whitespace/tab/comma-run: (first token, remainder).
fn split_first(line: &str) -> Option<(&str, &str)> {
    let start = line.find(|c: char| c.is_whitespace() || c == ',')?;
    let rest = line[start..].trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    Some((line[..start].trim(), rest.trim()))
}

/// Split off the LAST whitespace/tab/comma-run: (head, last token).
fn split_last(line: &str) -> Option<(&str, &str)> {
    let end = line.rfind(|c: char| c.is_whitespace() || c == ',')?;
    let head = line[..end].trim_end_matches(|c: char| c.is_whitespace() || c == ',');
    let tail = line[end..].trim_start_matches(|c: char| c.is_whitespace() || c == ',');
    Some((head.trim(), tail.trim()))
}

fn format_size(bytes: u64, units: Units) -> String {
    match units {
        Units::Bytes => group_thousands(bytes),
        Units::Human => human(bytes, 1024.0, &["B", "K", "M", "G", "T", "P", "E"]),
        Units::Si => human(bytes, 1000.0, &["B", "k", "M", "G", "T", "P", "E"]),
    }
}

fn group_thousands(n: u64) -> String {
    let s = n.to_string();
    let bytes = s.as_bytes();
    let mut out = String::new();
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(*b as char);
    }
    out
}

fn human(bytes: u64, base: f64, suffixes: &[&str]) -> String {
    if bytes < base as u64 {
        return format!("{bytes}{}", suffixes[0]);
    }
    let mut val = bytes as f64;
    let mut idx = 0;
    while val >= base && idx < suffixes.len() - 1 {
        val /= base;
        idx += 1;
    }
    // One decimal for values under 10 (e.g. 1.5K), whole numbers above (e.g. 24M).
    if val < 10.0 {
        format!("{val:.1}{}", suffixes[idx])
    } else {
        format!("{}{}", val.round() as u64, suffixes[idx])
    }
}

fn sort_children(node: &mut Node, sort: Sort, dirs_first: bool) {
    for c in node.children.iter_mut() {
        sort_children(c, sort, dirs_first);
    }
    // Precompute per-child keys once (avoids re-walking during comparisons).
    let totals: Vec<u64> = node.children.iter().map(Node::total).collect();
    let names: Vec<String> = node.children.iter().map(|c| c.name.to_ascii_lowercase()).collect();
    let is_dir: Vec<bool> = node.children.iter().map(|c| c.is_dir()).collect();
    let orders: Vec<usize> = node.children.iter().map(|c| c.order).collect();
    let mut order: Vec<usize> = (0..node.children.len()).collect();
    order.sort_by(|&ia, &ib| {
        if dirs_first {
            match is_dir[ib].cmp(&is_dir[ia]) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
        }
        let primary = match sort {
            Sort::Name => names[ia].cmp(&names[ib]),
            Sort::SizeDesc => totals[ib].cmp(&totals[ia]),
            Sort::Input => orders[ia].cmp(&orders[ib]),
        };
        // Stable tie-break on original input order.
        primary.then(orders[ia].cmp(&orders[ib]))
    });
    reorder(&mut node.children, &order);
}

/// Reorder `v` in place to the given index permutation.
fn reorder<T>(v: &mut Vec<T>, order: &[usize]) {
    let mut taken: Vec<Option<T>> = v.drain(..).map(Some).collect();
    for &i in order {
        v.push(taken[i].take().unwrap());
    }
}

struct RenderOpts {
    glyphs: Glyphs,
    units: Units,
    trailing_slash: bool,
    show_counts: bool,
    max_depth: usize,
}

fn render(
    node: &Node,
    prefix: &str,
    is_last: bool,
    is_root: bool,
    depth: usize,
    opts: &RenderOpts,
    out: &mut String,
) {
    let glyphs = &opts.glyphs;
    let label = if is_root {
        node.name.clone()
    } else {
        let connector = if is_last { glyphs.elbow } else { glyphs.tee };
        format!("{prefix}{connector}{}", node.name)
    };
    let is_dir = node.is_dir();
    let name = if is_dir && opts.trailing_slash && !is_root && !label.ends_with('/') {
        format!("{label}/")
    } else {
        label
    };
    // Size annotation (+ per-directory counts).
    let mut line = format!("{name}  {}", format_size(node.total(), opts.units));
    if opts.show_counts && is_dir {
        let (files, dirs) = node.counts();
        line.push_str(&format!("  ({} files, {} dirs)", files, dirs));
    }
    out.push_str(&line);
    out.push('\n');

    // Depth cap: 0 = unlimited. `depth` is this node's depth (root = 0).
    if opts.max_depth != 0 && depth >= opts.max_depth {
        return;
    }
    let child_prefix = if is_root {
        String::new()
    } else if is_last {
        format!("{prefix}{}", glyphs.blank)
    } else {
        format!("{prefix}{}", glyphs.pipe)
    };
    let n = node.children.len();
    for (i, c) in node.children.iter().enumerate() {
        render(c, &child_prefix, i + 1 == n, false, depth + 1, opts, out);
    }
}

/// Build a size-annotated directory tree from a pasted listing.
#[allow(clippy::too_many_arguments)]
pub fn build(
    input: &str,
    format: &str,
    units: &str,
    sort: &str,
    root: &str,
    ascii: bool,
    dirs_first: bool,
    trailing_slash: bool,
    show_counts: bool,
    depth: i64,
) -> Result<String, String> {
    let format = Format::parse(format)?;
    let units = Units::parse(units)?;
    let sort = Sort::parse(sort)?;
    if depth < 0 {
        return Err("depth must be 0 (unlimited) or a positive number".into());
    }
    let root_label = if root.trim().is_empty() { "." } else { root.trim() };

    let mut tree = Node::new(root_label.to_string(), 0);
    let mut next_order = 1usize;
    let mut any = false;
    for (i, raw) in input.lines().enumerate() {
        let Some((path, size)) = split_line(raw, format, i + 1)? else {
            continue;
        };
        any = true;
        let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
        if segments.is_empty() {
            return Err(format!("line {}: path {path:?} has no name segments", i + 1));
        }
        let mut cur = &mut tree;
        for seg in &segments {
            cur = cur.child_mut(seg, &mut next_order);
        }
        // A leaf accumulates its size; if this exact path repeats, sum them.
        cur.own = cur.own.saturating_add(size);
    }
    if !any {
        return Err("no entries: paste at least one 'path  size' line".into());
    }

    sort_children(&mut tree, sort, dirs_first);

    let opts = RenderOpts {
        glyphs: Glyphs::pick(ascii),
        units,
        trailing_slash,
        show_counts,
        max_depth: depth as usize,
    };
    let mut out = String::new();
    render(&tree, "", true, true, 0, &opts, &mut out);

    // Final report line, like `tree`'s "N directories, M files".
    let (files, dirs) = tree.counts();
    out.push('\n');
    out.push_str(&format!(
        "{} total · {} {} · {} {}",
        format_size(tree.total(), units),
        dirs,
        if dirs == 1 { "directory" } else { "directories" },
        files,
        if files == 1 { "file" } else { "files" },
    ));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(input: &str) -> String {
        build(input, "auto", "human", "name", ".", false, true, true, true, 0).unwrap()
    }

    #[test]
    fn size_first_du_style_rolls_up_and_counts() {
        let out = b("1024\tsrc/main.rs\n3072\tsrc/lib.rs\n512\tREADME.md");
        let expected = "\
.  4.5K  (3 files, 1 dirs)
├── src/  4.0K  (2 files, 0 dirs)
│   ├── lib.rs  3.0K
│   └── main.rs  1.0K
└── README.md  512B

4.5K total · 1 directory · 3 files";
        assert_eq!(out, expected);
    }

    #[test]
    fn csv_path_first() {
        let out = b("docs/guide.md,2048\nREADME.md,1024");
        assert!(out.contains("├── docs/  2.0K  (1 files, 0 dirs)"), "got: {out}");
        assert!(out.contains("│   └── guide.md  2.0K"), "got: {out}");
        assert!(out.contains("└── README.md  1.0K"), "got: {out}");
    }

    #[test]
    fn du_a_directory_lines_do_not_double_count() {
        // `du -a` lists the directory (cumulative 4096) AND its file (4096).
        // The directory line must be treated as a dir, not double-counted.
        let out = b("4096\tsrc\n4096\tsrc/main.rs");
        assert!(out.starts_with(".  4.0K"), "got: {out}");
        assert!(out.contains("└── src/  4.0K  (1 files, 0 dirs)"), "got: {out}");
    }

    #[test]
    fn bytes_and_si_units() {
        let bytes =
            build("1500\ta.bin", "auto", "bytes", "name", ".", false, true, true, false, 0).unwrap();
        assert!(bytes.contains("└── a.bin  1,500"), "got: {bytes}");
        let si =
            build("1500\ta.bin", "auto", "si", "name", ".", false, true, true, false, 0).unwrap();
        assert!(si.contains("└── a.bin  1.5k"), "got: {si}");
    }

    #[test]
    fn size_desc_sort_and_dirs_first() {
        let out = build(
            "10\tbig/a\n1\tsmall.txt\n5\tbig/b",
            "auto", "human", "size-desc", ".", false, true, true, false, 0,
        )
        .unwrap();
        // big/ (dir, 11) comes before small.txt (file, 1) with dirs_first.
        let big = out.find("big/").unwrap();
        let small = out.find("small.txt").unwrap();
        assert!(big < small, "dir should precede file: {out}");
    }

    #[test]
    fn depth_limit_hides_deep_entries_but_keeps_sizes() {
        let out = build(
            "1024\ta/b/c.txt\n2048\ta/d.txt",
            "auto", "human", "name", ".", false, true, true, true, 1,
        )
        .unwrap();
        // depth=1 prints only the first level under root; a/ shows cumulative size.
        assert!(out.contains("└── a/  3.0K  (2 files, 1 dirs)"), "got: {out}");
        assert!(!out.contains("c.txt"), "deep entry should be hidden: {out}");
    }

    #[test]
    fn ascii_connectors() {
        let out = build(
            "1\tx/y.txt", "auto", "human", "name", "root", true, true, true, false, 0,
        )
        .unwrap();
        assert!(out.starts_with("root  1B"), "got: {out}");
        assert!(out.contains("`-- x/"), "got: {out}");
        assert!(out.contains("`-- y.txt"), "got: {out}");
    }

    #[test]
    fn human_size_suffixes() {
        assert_eq!(format_size(0, Units::Human), "0B");
        assert_eq!(format_size(1023, Units::Human), "1023B");
        assert_eq!(format_size(1024, Units::Human), "1.0K");
        assert_eq!(format_size(1024 * 1024, Units::Human), "1.0M");
        assert_eq!(format_size(15 * 1024 * 1024, Units::Human), "15M");
    }

    #[test]
    fn parse_size_units() {
        assert_eq!(parse_size("1234").unwrap(), 1234);
        assert_eq!(parse_size("4K").unwrap(), 4096);
        assert_eq!(parse_size("1.5M").unwrap(), (1.5 * 1024.0 * 1024.0) as u64);
        assert_eq!(parse_size("2MiB").unwrap(), 2 * 1024 * 1024);
        assert_eq!(parse_size("500KB").unwrap(), 500_000);
    }

    #[test]
    fn error_empty_input() {
        let err = build("   \n\n", "auto", "human", "name", ".", false, true, true, true, 0)
            .unwrap_err();
        assert!(err.contains("no entries"), "got: {err}");
    }

    #[test]
    fn error_bad_size() {
        let err = build(
            "notasize\tfoo.txt", "size-first", "human", "name", ".", false, true, true, true, 0,
        )
        .unwrap_err();
        assert!(err.contains("line 1"), "got: {err}");
    }

    #[test]
    fn error_bad_format_param() {
        let err = build("1\ta", "sideways", "human", "name", ".", false, true, true, true, 0)
            .unwrap_err();
        assert!(err.contains("invalid format"), "got: {err}");
    }
}
