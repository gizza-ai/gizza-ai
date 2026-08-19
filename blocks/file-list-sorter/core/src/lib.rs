//! file-list-sorter core — sort a pasted list of filenames/paths by path-aware keys.
//!
//! Unlike a generic line sorter this understands path structure: it splits every
//! entry into directory / basename / extension / depth, infers which entries are
//! folders, optionally reads a size column, and can keep folders above files.
//! Pure and deterministic: same input + options → same output, no filesystem access.

use std::cmp::Ordering;
use std::collections::HashSet;

/// Hard cap on entries per run.
pub const MAX_ENTRIES: usize = 20_000;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SortBy {
    /// Natural order on the whole path: digit runs compare as numbers (img2 < img10).
    Natural,
    /// Classic codepoint order on the whole path (img10 < img2).
    Alpha,
    /// Natural order on the file name only, ignoring the folders above it.
    Basename,
    /// By file extension, then naturally by path.
    Extension,
    /// By how many folders deep the entry sits, then naturally by path.
    Depth,
    /// By a size column found on the line; entries without one sort last.
    Size,
}

impl SortBy {
    pub fn parse(s: &str) -> Result<SortBy, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "natural" => Ok(SortBy::Natural),
            "alpha" | "alphabetical" | "lexicographic" => Ok(SortBy::Alpha),
            "basename" | "filename" => Ok(SortBy::Basename),
            "extension" | "ext" => Ok(SortBy::Extension),
            "depth" => Ok(SortBy::Depth),
            "size" => Ok(SortBy::Size),
            other => Err(format!(
                "unknown sort_by '{other}' (use natural, alpha, basename, extension, depth or size)"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortBy::Natural => "natural order",
            SortBy::Alpha => "alphabetical order",
            SortBy::Basename => "file name",
            SortBy::Extension => "extension",
            SortBy::Depth => "folder depth",
            SortBy::Size => "size",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Order {
    Asc,
    Desc,
}

impl Order {
    pub fn parse(s: &str) -> Result<Order, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "asc" | "ascending" => Ok(Order::Asc),
            "desc" | "descending" => Ok(Order::Desc),
            other => Err(format!("unknown order '{other}' (use asc or desc)")),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    List,
    Numbered,
    Table,
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "list" => Ok(Format::List),
            "numbered" => Ok(Format::Numbered),
            "table" => Ok(Format::Table),
            "json" => Ok(Format::Json),
            other => Err(format!(
                "unknown format '{other}' (use list, numbered, table or json)"
            )),
        }
    }
}

/// One parsed list entry.
#[derive(Clone, Debug)]
pub struct Entry {
    /// The path as it will be printed (size column stripped, trimmed if enabled).
    pub path: String,
    /// Path with `\` folded to `/`, a leading `./` dropped and any trailing `/` removed.
    pub norm: String,
    /// Parent folder of `norm` ("" for a top-level entry).
    pub dir: String,
    /// Last path component.
    pub name: String,
    /// Extension without the dot ("" for folders and extension-less names).
    pub ext: String,
    /// How many folders sit above the entry (`a.txt` = 0, `src/a.txt` = 1).
    pub depth: usize,
    /// Trailing-slash entries, plus any entry that is a parent of another entry.
    pub is_dir: bool,
    /// Size in bytes when the line carried a size column.
    pub size: Option<u64>,
    /// The size column exactly as written ("" when absent).
    pub size_text: String,
}

fn unit_mult(unit: &str) -> Option<u64> {
    match unit.to_ascii_uppercase().as_str() {
        "B" => Some(1),
        "K" | "KB" | "KIB" => Some(1024),
        "M" | "MB" | "MIB" => Some(1024 * 1024),
        "G" | "GB" | "GIB" => Some(1024u64.pow(3)),
        "T" | "TB" | "TIB" => Some(1024u64.pow(4)),
        _ => None,
    }
}

/// A size token WITH a unit suffix: `4.0K`, `12MB`, `3GiB`, `512B`.
fn parse_unit_size(tok: &str) -> Option<u64> {
    let t = tok.trim();
    let cut = t.find(|c: char| c.is_ascii_alphabetic())?;
    let (num, unit) = t.split_at(cut);
    if num.is_empty() {
        return None;
    }
    let mult = unit_mult(unit)?;
    let v: f64 = num.parse().ok()?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    Some((v * mult as f64).round() as u64)
}

fn parse_bare_size(tok: &str) -> Option<u64> {
    let t = tok.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    t.parse().ok()
}

/// Split `line` into (path, size bytes, size text).
///
/// A size column is only recognised when it is unambiguous: either the token
/// carries a unit (`4.0K`, `12MB`) or it is a bare byte count separated from the
/// path by a TAB (`1234\tsrc/app.js`). A bare number separated by spaces is
/// treated as part of the name, so `2024 report.txt` keeps its year.
fn split_size(line: &str) -> (String, Option<u64>, String) {
    let chars: Vec<(usize, char)> = line.char_indices().collect();

    // Leading token: "<size><sep><path>"
    if let Some(first_ws) = chars.iter().position(|(_, c)| c.is_whitespace()) {
        let tok_end = chars[first_ws].0;
        if let Some(rest_rel) = chars[first_ws..].iter().position(|(_, c)| !c.is_whitespace()) {
            let rest_start = chars[first_ws + rest_rel].0;
            let tok = &line[..tok_end];
            let sep = &line[tok_end..rest_start];
            let rest = &line[rest_start..];
            let size = parse_unit_size(tok)
                .or_else(|| sep.contains('\t').then(|| parse_bare_size(tok)).flatten());
            if let Some(bytes) = size {
                return (rest.trim_end().to_string(), Some(bytes), tok.to_string());
            }
        }
    }

    // Trailing token: "<path><sep><size>"
    if let Some(last_ws) = chars.iter().rposition(|(_, c)| c.is_whitespace()) {
        let tok_start = chars[last_ws].0 + chars[last_ws].1.len_utf8();
        if let Some(head_rel) = chars[..=last_ws].iter().rposition(|(_, c)| !c.is_whitespace()) {
            let head_end = chars[head_rel].0 + chars[head_rel].1.len_utf8();
            let head = &line[..head_end];
            let sep = &line[head_end..tok_start];
            let tok = &line[tok_start..];
            let size = parse_unit_size(tok)
                .or_else(|| sep.contains('\t').then(|| parse_bare_size(tok)).flatten());
            if let (Some(bytes), false) = (size, head.is_empty()) {
                return (head.to_string(), Some(bytes), tok.to_string());
            }
        }
    }

    (line.to_string(), None, String::new())
}

fn split_ext(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() => &name[i + 1..],
        _ => "",
    }
}

fn fold(s: &str, ignore_case: bool) -> String {
    if ignore_case {
        s.to_lowercase()
    } else {
        s.to_string()
    }
}

/// Human ("natural") comparison: runs of digits compare as numbers, so
/// `img2.png` sorts before `img10.png`.
pub fn natural_cmp(a: &str, b: &str, ignore_case: bool) -> Ordering {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let (mut i, mut j) = (0usize, 0usize);
    while i < av.len() && j < bv.len() {
        let (ca, cb) = (av[i], bv[j]);
        if ca.is_ascii_digit() && cb.is_ascii_digit() {
            let si = i;
            while i < av.len() && av[i].is_ascii_digit() {
                i += 1;
            }
            let sj = j;
            while j < bv.len() && bv[j].is_ascii_digit() {
                j += 1;
            }
            let da: String = av[si..i].iter().collect();
            let db: String = bv[sj..j].iter().collect();
            let ta = da.trim_start_matches('0');
            let tb = db.trim_start_matches('0');
            let c = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb));
            if c != Ordering::Equal {
                return c;
            }
            // Same value: fewer leading zeros first, so file01 precedes file001.
            let c = da.len().cmp(&db.len());
            if c != Ordering::Equal {
                return c;
            }
        } else {
            let (xa, xb) = if ignore_case {
                (
                    ca.to_lowercase().next().unwrap_or(ca),
                    cb.to_lowercase().next().unwrap_or(cb),
                )
            } else {
                (ca, cb)
            };
            let c = xa.cmp(&xb);
            if c != Ordering::Equal {
                return c;
            }
            i += 1;
            j += 1;
        }
    }
    (av.len() - i).cmp(&(bv.len() - j))
}

fn alpha_cmp(a: &str, b: &str, ignore_case: bool) -> Ordering {
    if ignore_case {
        a.to_lowercase()
            .cmp(&b.to_lowercase())
            .then_with(|| a.cmp(b))
    } else {
        a.cmp(b)
    }
}

fn parse_entries(paths: &str, trim: bool) -> Result<Vec<Entry>, String> {
    let mut out: Vec<Entry> = Vec::new();
    for raw in paths.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if line.trim().is_empty() {
            continue;
        }
        let line = if trim { line.trim() } else { line };
        if out.len() >= MAX_ENTRIES {
            return Err(format!(
                "too many entries: this run is capped at {MAX_ENTRIES} paths — split the list and sort it in batches"
            ));
        }
        let (path, size, size_text) = split_size(line);
        if path.trim().is_empty() {
            continue;
        }

        let mut norm = path.replace('\\', "/");
        while let Some(rest) = norm.strip_prefix("./") {
            norm = rest.to_string();
        }
        let had_trailing = norm.len() > 1 && norm.ends_with('/');
        let norm = norm.trim_end_matches('/').to_string();
        let comps: Vec<&str> = norm.split('/').filter(|s| !s.is_empty()).collect();
        let name = comps.last().copied().unwrap_or("").to_string();
        let dir = if comps.len() > 1 {
            comps[..comps.len() - 1].join("/")
        } else {
            String::new()
        };
        let depth = comps.len().saturating_sub(1);
        let ext = if had_trailing {
            String::new()
        } else {
            split_ext(&name).to_string()
        };

        out.push(Entry {
            path,
            norm,
            dir,
            name,
            ext,
            depth,
            is_dir: had_trailing,
            size,
            size_text,
        });
    }
    Ok(out)
}

/// Mark an entry as a folder when another entry lives underneath it.
fn infer_dirs(entries: &mut [Entry], ignore_case: bool) {
    let mut parents: HashSet<String> = HashSet::new();
    for e in entries.iter() {
        let comps: Vec<&str> = e.norm.split('/').filter(|s| !s.is_empty()).collect();
        for cut in 1..comps.len() {
            parents.insert(fold(&comps[..cut].join("/"), ignore_case));
        }
    }
    for e in entries.iter_mut() {
        if !e.is_dir && parents.contains(&fold(&e.norm, ignore_case)) {
            e.is_dir = true;
            e.ext = String::new();
        }
    }
}

fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes}B")
    } else if v < 10.0 {
        format!("{v:.1}{}", UNITS[u])
    } else {
        format!("{v:.0}{}", UNITS[u])
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    paths: &str,
    sort_by: &str,
    order: &str,
    ignore_case: bool,
    dirs_first: bool,
    group_by_dir: bool,
    unique: bool,
    trim: bool,
    format: &str,
) -> Result<String, String> {
    let sort_by = SortBy::parse(sort_by)?;
    let order = Order::parse(order)?;
    let format = Format::parse(format)?;

    if paths.trim().is_empty() {
        return Err("input is empty — paste a list of file paths, one per line".to_string());
    }

    let mut entries = parse_entries(paths, trim)?;
    if entries.is_empty() {
        return Err("no file paths found — paste one path per line".to_string());
    }
    infer_dirs(&mut entries, ignore_case);

    if unique {
        let mut seen: HashSet<String> = HashSet::new();
        entries.retain(|e| seen.insert(fold(&e.norm, ignore_case)));
    }

    if sort_by == SortBy::Size && !entries.iter().any(|e| e.size.is_some()) {
        return Err(
            "sorting by size needs a size column: put a size with a unit before or after each path \
             (e.g. `4.0K  src/app.js` or `src/app.js  1.2MB`). A bare byte count works too when a \
             TAB separates it from the path"
                .to_string(),
        );
    }

    let desc = order == Order::Desc;
    entries.sort_by(|a, b| {
        // Folders stay on top regardless of direction — the file-manager convention.
        if dirs_first && a.is_dir != b.is_dir {
            return if a.is_dir {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        // Entries with no size column always sink to the bottom.
        if sort_by == SortBy::Size && a.size.is_some() != b.size.is_some() {
            return if a.size.is_some() {
                Ordering::Less
            } else {
                Ordering::Greater
            };
        }
        let mut c = Ordering::Equal;
        if group_by_dir {
            c = natural_cmp(&a.dir, &b.dir, ignore_case);
        }
        if c == Ordering::Equal {
            c = match sort_by {
                SortBy::Natural => natural_cmp(&a.norm, &b.norm, ignore_case),
                SortBy::Alpha => alpha_cmp(&a.norm, &b.norm, ignore_case),
                SortBy::Basename => natural_cmp(&a.name, &b.name, ignore_case),
                SortBy::Extension => alpha_cmp(&a.ext, &b.ext, ignore_case),
                SortBy::Depth => a.depth.cmp(&b.depth),
                SortBy::Size => a.size.unwrap_or(0).cmp(&b.size.unwrap_or(0)),
            };
        }
        if c == Ordering::Equal {
            c = natural_cmp(&a.norm, &b.norm, ignore_case).then_with(|| a.path.cmp(&b.path));
        }
        if desc {
            c.reverse()
        } else {
            c
        }
    });

    Ok(render(&entries, sort_by, order, dirs_first, group_by_dir, format))
}

fn render(
    entries: &[Entry],
    sort_by: SortBy,
    order: Order,
    dirs_first: bool,
    group_by_dir: bool,
    format: Format,
) -> String {
    match format {
        Format::List => entries
            .iter()
            .map(|e| e.path.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        Format::Numbered => {
            let w = entries.len().to_string().len();
            entries
                .iter()
                .enumerate()
                .map(|(i, e)| format!("{:>w$}. {}", i + 1, e.path, w = w))
                .collect::<Vec<_>>()
                .join("\n")
        }
        Format::Table => {
            let folders = entries.iter().filter(|e| e.is_dir).count();
            let mut head = format!(
                "{} entries · {} folders · {} files · sorted by {} ({})",
                entries.len(),
                folders,
                entries.len() - folders,
                sort_by.label(),
                if order == Order::Asc { "asc" } else { "desc" },
            );
            if dirs_first && folders > 0 {
                head.push_str(" · folders first");
            }
            if group_by_dir {
                head.push_str(" · grouped by folder");
            }

            let cells: Vec<[String; 6]> = entries
                .iter()
                .enumerate()
                .map(|(i, e)| {
                    [
                        format!("{}", i + 1),
                        e.path.clone(),
                        if e.is_dir { "folder" } else { "file" }.to_string(),
                        if e.ext.is_empty() {
                            "-".to_string()
                        } else {
                            e.ext.clone()
                        },
                        e.depth.to_string(),
                        match e.size {
                            Some(b) if !e.size_text.is_empty() => {
                                format!("{} ({})", e.size_text, human_size(b))
                            }
                            _ => "-".to_string(),
                        },
                    ]
                })
                .collect();
            let header = [
                "#".to_string(),
                "path".to_string(),
                "type".to_string(),
                "ext".to_string(),
                "depth".to_string(),
                "size".to_string(),
            ];
            let mut widths = [0usize; 6];
            for (k, w) in widths.iter_mut().enumerate() {
                *w = header[k].chars().count();
                for row in &cells {
                    *w = (*w).max(row[k].chars().count());
                }
            }
            let line = |row: &[String; 6]| {
                let mut s = String::new();
                for k in 0..6 {
                    if k > 0 {
                        s.push_str("  ");
                    }
                    let pad = widths[k] - row[k].chars().count();
                    s.push_str(&row[k]);
                    if k < 5 {
                        s.push_str(&" ".repeat(pad));
                    }
                }
                s.trim_end().to_string()
            };
            let mut out = vec![head, String::new(), line(&header)];
            for row in &cells {
                out.push(line(row));
            }
            out.join("\n")
        }
        Format::Json => {
            let items: Vec<serde_json::Value> = entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "path": e.path,
                        "name": e.name,
                        "dir": e.dir,
                        "extension": e.ext,
                        "depth": e.depth,
                        "is_dir": e.is_dir,
                        "size_bytes": e.size,
                        "size_text": e.size_text,
                    })
                })
                .collect();
            serde_json::to_string_pretty(&serde_json::json!({
                "count": entries.len(),
                "folders": entries.iter().filter(|e| e.is_dir).count(),
                "sort_by": match sort_by {
                    SortBy::Natural => "natural",
                    SortBy::Alpha => "alpha",
                    SortBy::Basename => "basename",
                    SortBy::Extension => "extension",
                    SortBy::Depth => "depth",
                    SortBy::Size => "size",
                },
                "order": if order == Order::Asc { "asc" } else { "desc" },
                "entries": items,
            }))
            .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "img10.png\nimg2.png\nimg1.png";

    fn sorted(input: &str, sort_by: &str, order: &str) -> String {
        run(input, sort_by, order, true, true, false, false, true, "list").unwrap()
    }

    #[test]
    fn natural_order_puts_img2_before_img10() {
        assert_eq!(sorted(LIST, "natural", "asc"), "img1.png\nimg2.png\nimg10.png");
    }

    #[test]
    fn alpha_order_is_the_machine_order() {
        assert_eq!(sorted(LIST, "alpha", "asc"), "img1.png\nimg10.png\nimg2.png");
    }

    #[test]
    fn desc_reverses_the_key_but_keeps_folders_on_top() {
        let out = sorted("src/\nb.txt\na.txt", "natural", "desc");
        assert_eq!(out, "src/\nb.txt\na.txt");
    }

    #[test]
    fn extension_sort_groups_by_type() {
        let out = sorted("a.zip\nb.md\nc.rs\nREADME", "extension", "asc");
        assert_eq!(out, "README\nb.md\nc.rs\na.zip");
    }

    #[test]
    fn depth_sort_is_shallowest_first() {
        let out = sorted("a/b/c/deep.txt\ntop.txt\na/mid.txt", "depth", "asc");
        assert_eq!(out, "top.txt\na/mid.txt\na/b/c/deep.txt");
    }

    #[test]
    fn basename_sort_ignores_the_folders() {
        let out = sorted("zzz/apple.txt\naaa/banana.txt", "basename", "asc");
        assert_eq!(out, "zzz/apple.txt\naaa/banana.txt");
    }

    #[test]
    fn parent_entries_are_inferred_as_folders() {
        let out = sorted("src/main.rs\nzeta.txt\nsrc", "natural", "asc");
        assert_eq!(out, "src\nsrc/main.rs\nzeta.txt");
    }

    #[test]
    fn dirs_first_can_be_turned_off() {
        let out = run(
            "src/\nb.txt\na.txt",
            "natural",
            "asc",
            true,
            false,
            false,
            false,
            true,
            "list",
        )
        .unwrap();
        assert_eq!(out, "a.txt\nb.txt\nsrc/");
    }

    #[test]
    fn size_sort_reads_a_unit_column_and_sinks_unsized_entries() {
        let out = sorted("4.0K\tsmall.bin\n2M\tbig.bin\nunknown.bin\n512B\ttiny.bin", "size", "desc");
        assert_eq!(out, "big.bin\nsmall.bin\ntiny.bin\nunknown.bin");
    }

    #[test]
    fn bare_byte_counts_need_a_tab_so_years_stay_in_names() {
        let out = sorted("2024 report.txt\nnotes.txt\t900", "size", "asc");
        assert_eq!(out, "notes.txt\n2024 report.txt");
    }

    #[test]
    fn size_sort_without_any_size_column_is_an_error() {
        let err = run("a.txt\nb.txt", "size", "asc", true, true, false, false, true, "list")
            .unwrap_err();
        assert!(err.contains("size column"), "{err}");
    }

    #[test]
    fn windows_separators_are_understood() {
        let out = sorted("docs\\b.txt\ndocs\\a.txt\nz.txt", "natural", "asc");
        assert_eq!(out, "docs\\a.txt\ndocs\\b.txt\nz.txt");
    }

    #[test]
    fn case_sensitivity_is_switchable() {
        let ci = sorted("beta.txt\nAlpha.txt", "natural", "asc");
        assert_eq!(ci, "Alpha.txt\nbeta.txt");
        let cs = run(
            "beta.txt\nAlpha.txt",
            "alpha",
            "asc",
            false,
            true,
            false,
            false,
            true,
            "list",
        )
        .unwrap();
        assert_eq!(cs, "Alpha.txt\nbeta.txt");
        let cs2 = run(
            "Beta.txt\nalpha.txt",
            "alpha",
            "asc",
            false,
            true,
            false,
            false,
            true,
            "list",
        )
        .unwrap();
        assert_eq!(cs2, "Beta.txt\nalpha.txt");
    }

    #[test]
    fn group_by_dir_keeps_folders_together() {
        let out = run(
            "b/2.txt\na/z.txt\nb/1.txt\na/a.txt",
            "basename",
            "asc",
            true,
            true,
            true,
            false,
            true,
            "list",
        )
        .unwrap();
        assert_eq!(out, "a/a.txt\na/z.txt\nb/1.txt\nb/2.txt");
    }

    #[test]
    fn unique_drops_repeated_paths() {
        let out = run(
            "a.txt\nA.TXT\nb.txt\na.txt",
            "natural",
            "asc",
            true,
            true,
            false,
            true,
            true,
            "list",
        )
        .unwrap();
        assert_eq!(out, "a.txt\nb.txt");
    }

    #[test]
    fn numbered_and_table_and_json_render() {
        let numbered = run(
            "b.txt\na.txt",
            "natural",
            "asc",
            true,
            true,
            false,
            false,
            true,
            "numbered",
        )
        .unwrap();
        assert_eq!(numbered, "1. a.txt\n2. b.txt");

        let table = run(
            "src/main.rs\nnotes.md",
            "natural",
            "asc",
            true,
            true,
            false,
            false,
            true,
            "table",
        )
        .unwrap();
        assert!(table.starts_with("2 entries · 0 folders · 2 files · sorted by natural order (asc)"));
        assert!(table.contains("1  notes.md     file  md   0      -"), "{table}");
        assert!(table.contains("2  src/main.rs  file  rs   1      -"), "{table}");

        let json = run(
            "src/main.rs",
            "natural",
            "asc",
            true,
            true,
            false,
            false,
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["count"], 1);
        assert_eq!(v["entries"][0]["extension"], "rs");
        assert_eq!(v["entries"][0]["dir"], "src");
        assert_eq!(v["entries"][0]["depth"], 1);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n\n", "natural", "asc", true, true, false, false, true, "list")
            .unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn unknown_options_are_rejected() {
        assert!(run("a", "colour", "asc", true, true, false, false, true, "list")
            .unwrap_err()
            .contains("unknown sort_by"));
        assert!(run("a", "natural", "sideways", true, true, false, false, true, "list")
            .unwrap_err()
            .contains("unknown order"));
        assert!(run("a", "natural", "asc", true, true, false, false, true, "yaml")
            .unwrap_err()
            .contains("unknown format"));
    }

    #[test]
    fn over_the_cap_is_rejected() {
        let big = (0..MAX_ENTRIES + 1)
            .map(|i| format!("f{i}.txt"))
            .collect::<Vec<_>>()
            .join("\n");
        let err = run(&big, "natural", "asc", true, true, false, false, true, "list").unwrap_err();
        assert!(err.contains("capped at"), "{err}");
    }

    #[test]
    fn dotfiles_have_no_extension_and_double_extensions_use_the_last() {
        let json = run(
            ".gitignore\narchive.tar.gz",
            "natural",
            "asc",
            true,
            true,
            false,
            false,
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["entries"][0]["extension"], "");
        assert_eq!(v["entries"][1]["extension"], "gz");
    }
}
