//! markdown-notes-index core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Takes a pasted bundle of Markdown notes, splits it into individual notes, reads each
//! note's title, tags, heading outline and word count, and renders a linked index of the
//! whole set as Markdown, JSON or CSV.

use serde_json::json;

/// Maximum number of notes accepted in one run. Past this the input is rejected with a
/// clear error rather than silently truncated.
pub const MAX_NOTES: usize = 500;

/// One heading found inside a note's body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Heading {
    /// ATX level as written, 1-6.
    pub level: u32,
    pub text: String,
    /// GitHub-style anchor slug for the heading text.
    pub anchor: String,
}

/// One parsed note.
#[derive(Debug, Clone)]
pub struct Note {
    pub title: String,
    /// Path from a `=== path ===` / `==> path <==` file marker, when the bundle had one.
    pub file: Option<String>,
    pub tags: Vec<String>,
    pub headings: Vec<Heading>,
    pub words: usize,
    /// De-duplicated anchor for this note's section in the generated index.
    pub anchor: String,
}

/// A note segment as produced by the splitter, before parsing.
struct RawNote {
    file: Option<String>,
    text: String,
}

/// Build the index. `heading_depth` is the deepest heading level kept in each note's
/// outline (0 = no outline); every other argument is a lowercase keyword.
#[allow(clippy::too_many_arguments)]
pub fn run(
    notes: &str,
    split: &str,
    format: &str,
    heading_depth: u32,
    group_by: &str,
    sort: &str,
    link_style: &str,
    include_toc: bool,
    include_stats: bool,
    inline_tags: bool,
) -> Result<String, String> {
    let split = keyword("split", split, &["heading", "hr", "file-marker"])?;
    let format = keyword("format", format, &["markdown", "json", "csv"])?;
    let group_by = keyword("group_by", group_by, &["none", "tag"])?;
    let sort = keyword("sort", sort, &["input", "title", "words"])?;
    let link_style = keyword("link_style", link_style, &["anchor", "wiki", "none"])?;
    if heading_depth > 6 {
        return Err(format!(
            "heading_depth must be between 0 and 6, got {heading_depth}"
        ));
    }
    if notes.trim().is_empty() {
        return Err("expected Markdown notes, got an empty document".to_string());
    }

    let raw = split_notes(notes, &split)?;
    if raw.is_empty() {
        return Err(format!(
            "no notes found: nothing in the input matched the \"{split}\" boundary"
        ));
    }
    if raw.len() > MAX_NOTES {
        return Err(format!(
            "too many notes: {} found, the limit is {MAX_NOTES} per run — split the bundle and index it in batches",
            raw.len()
        ));
    }

    let mut used: Vec<(String, usize)> = Vec::new();
    let mut parsed: Vec<Note> = Vec::new();
    for (i, r) in raw.iter().enumerate() {
        let mut note = parse_note(&r.text, r.file.clone(), inline_tags, i + 1);
        note.anchor = unique_anchor(&slugify(&note.title), &mut used);
        parsed.push(note);
    }

    let order = sort_order(&parsed, &sort);

    match format.as_str() {
        "markdown" => Ok(render_markdown(
            &parsed,
            &order,
            heading_depth,
            &group_by,
            &link_style,
            include_toc,
            include_stats,
        )),
        "json" => Ok(render_json(
            &parsed,
            &order,
            heading_depth,
            &group_by,
            include_stats,
        )),
        _ => Ok(render_csv(&parsed, &order, include_stats)),
    }
}

/// `run` with every option at its documented default.
pub fn run_default(notes: &str) -> Result<String, String> {
    run(
        notes, "heading", "markdown", 2, "none", "input", "anchor", true, true, true,
    )
}

fn keyword(name: &str, value: &str, allowed: &[&str]) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok(allowed[0].to_string());
    }
    if allowed.contains(&v.as_str()) {
        return Ok(v);
    }
    Err(format!(
        "unknown {name} \"{value}\": expected one of {}",
        allowed.join(", ")
    ))
}

// ---------------------------------------------------------------------------
// splitting
// ---------------------------------------------------------------------------

fn is_fence(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("```") || t.starts_with("~~~")
}

fn is_atx_h1(line: &str) -> bool {
    let t = line.trim_start();
    t == "#" || t.starts_with("# ")
}

/// A thematic break: three or more `-`, `*` or `_`, spaces allowed between them.
fn is_thematic_break(line: &str) -> bool {
    let t = line.trim();
    let first = match t.chars().next() {
        Some(c) if c == '-' || c == '*' || c == '_' => c,
        _ => return false,
    };
    let mut count = 0;
    for c in t.chars() {
        if c == first {
            count += 1;
        } else if !c.is_whitespace() {
            return false;
        }
    }
    count >= 3
}

/// `=== path/note.md ===` or `==> path/note.md <==` (the `head`/`tail` multi-file banner).
fn file_marker(line: &str) -> Option<String> {
    let t = line.trim();
    if let Some(rest) = t.strip_prefix("==>") {
        if let Some(name) = rest.strip_suffix("<==") {
            let name = name.trim();
            if !name.is_empty() {
                return Some(name.to_string());
            }
        }
        return None;
    }
    if t.starts_with("==") && t.ends_with("==") && t.len() > 4 {
        let inner = t.trim_matches('=').trim();
        if !inner.is_empty() && !inner.contains('=') {
            return Some(inner.to_string());
        }
    }
    None
}

fn split_notes(input: &str, split: &str) -> Result<Vec<RawNote>, String> {
    let mut out: Vec<RawNote> = Vec::new();
    let mut cur: Vec<&str> = Vec::new();
    let mut cur_file: Option<String> = None;
    let mut in_fence = false;
    let mut in_frontmatter = false;
    // Content other than blank lines and a leading front-matter block. A boundary that
    // fires before any real content would only produce an empty note, so it is skipped —
    // this is what keeps `---\ntitle: x\n---\n# Title` a single note.
    let mut seen_content = false;
    let mut saw_marker = false;

    fn flush(cur: &mut Vec<&str>, file: &mut Option<String>, out: &mut Vec<RawNote>) {
        let text = cur.join("\n");
        if !text.trim().is_empty() {
            out.push(RawNote {
                file: file.clone(),
                text,
            });
        }
        cur.clear();
        *file = None;
    }

    for line in input.lines() {
        if in_fence {
            cur.push(line);
            if is_fence(line) {
                in_fence = false;
            }
            continue;
        }
        if is_fence(line) {
            in_fence = true;
            seen_content = true;
            cur.push(line);
            continue;
        }
        if in_frontmatter {
            cur.push(line);
            if line.trim() == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        // A `---` opening a note's front matter is not a thematic break.
        if !seen_content && line.trim() == "---" {
            in_frontmatter = true;
            cur.push(line);
            continue;
        }
        if let Some(name) = file_marker(line) {
            saw_marker = true;
            if split == "file-marker" {
                flush(&mut cur, &mut cur_file, &mut out);
                cur_file = Some(name);
                seen_content = false;
            } else if !seen_content {
                // Keep the path as the current note's source even when splitting elsewhere.
                cur_file = Some(name);
            }
            continue;
        }
        match split {
            "heading" if is_atx_h1(line) && seen_content => {
                flush(&mut cur, &mut cur_file, &mut out);
                seen_content = true;
                cur.push(line);
            }
            "hr" if is_thematic_break(line) => {
                flush(&mut cur, &mut cur_file, &mut out);
                seen_content = false;
            }
            _ => {
                if !line.trim().is_empty() {
                    seen_content = true;
                }
                cur.push(line);
            }
        }
    }
    flush(&mut cur, &mut cur_file, &mut out);

    if split == "file-marker" && !saw_marker {
        return Err(
            "no file markers found: expected lines like `=== notes/todo.md ===` or `==> notes/todo.md <==` between notes"
                .to_string(),
        );
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// parsing one note
// ---------------------------------------------------------------------------

fn parse_note(text: &str, file: Option<String>, inline_tags: bool, index: usize) -> Note {
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    let mut fm: Vec<&str> = Vec::new();
    if i < lines.len() && lines[i].trim() == "---" {
        let mut j = i + 1;
        while j < lines.len() && lines[j].trim() != "---" {
            j += 1;
        }
        if j < lines.len() {
            fm = lines[i + 1..j].to_vec();
            i = j + 1;
        }
    }
    let body: Vec<&str> = lines[i.min(lines.len())..].to_vec();

    let (fm_title, mut tags) = parse_frontmatter(&fm);

    let mut headings: Vec<Heading> = Vec::new();
    let mut in_fence = false;
    for line in &body {
        if is_fence(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(h) = atx_heading(line) {
            headings.push(h);
        }
        if inline_tags {
            for t in scan_inline_tags(line) {
                push_tag(&mut tags, t);
            }
        }
    }

    let mut title_from_heading = false;
    let title = if let Some(t) = fm_title {
        t
    } else if let Some(h) = headings.first() {
        title_from_heading = true;
        h.text.clone()
    } else if let Some(f) = &file {
        file_stem(f)
    } else {
        format!("Untitled note {index}")
    };
    if title_from_heading && !headings.is_empty() {
        headings.remove(0);
    }

    let words = body.iter().map(|l| count_words(l)).sum::<usize>();

    tags.sort_by_key(|t| t.to_lowercase());

    Note {
        title,
        file,
        tags,
        headings,
        words,
        anchor: String::new(),
    }
}

/// Words are whitespace-separated tokens that carry at least one letter or digit, so bare
/// Markdown punctuation (`#`, `-`, `>`, `|`) is not counted as a word.
fn count_words(line: &str) -> usize {
    line.split_whitespace()
        .filter(|t| t.chars().any(|c| c.is_alphanumeric()))
        .count()
}

fn file_stem(path: &str) -> String {
    let name = path.rsplit(['/', '\\']).next().unwrap_or(path);
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => stem.to_string(),
        _ => name.to_string(),
    }
}

fn atx_heading(line: &str) -> Option<Heading> {
    let t = line.trim_start();
    let hashes = t.chars().take_while(|c| *c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &t[hashes..];
    if !rest.is_empty() && !rest.starts_with(' ') && !rest.starts_with('\t') {
        return None;
    }
    let text = rest.trim().trim_end_matches('#').trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(Heading {
        anchor: slugify(&text),
        level: hashes as u32,
        text,
    })
}

fn is_tag_char(c: char) -> bool {
    c.is_alphanumeric() || c == '-' || c == '_' || c == '/'
}

/// Inline `#tag` mentions. A tag must follow a boundary (start of line, whitespace, or an
/// opening bracket), must not be a heading marker, and must contain at least one letter so
/// issue references like `#1234` are left alone.
fn scan_inline_tags(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    for (i, c) in chars.iter().enumerate() {
        if *c != '#' {
            continue;
        }
        let prev = if i == 0 { None } else { Some(chars[i - 1]) };
        let boundary = matches!(prev, None | Some(' ') | Some('\t') | Some('(') | Some('['));
        if !boundary {
            continue;
        }
        let mut j = i + 1;
        while j < chars.len() && is_tag_char(chars[j]) {
            j += 1;
        }
        let tag: String = chars[i + 1..j].iter().collect();
        if tag.chars().any(|c| c.is_alphabetic()) {
            out.push(tag);
        }
    }
    out
}

fn push_tag(tags: &mut Vec<String>, tag: String) {
    let tag = tag.trim().trim_start_matches('#').trim().to_string();
    let tag = tag.trim_matches(['"', '\'']).to_string();
    if tag.is_empty() {
        return;
    }
    if tags.iter().any(|t| t.eq_ignore_ascii_case(&tag)) {
        return;
    }
    tags.push(tag);
}

/// Reads `title:`, `tags:` and `tag:` out of a YAML front-matter block. Handles inline
/// (`tags: a, b`), flow (`tags: [a, b]`) and block-list (`- a` on following lines) forms.
fn parse_frontmatter(fm: &[&str]) -> (Option<String>, Vec<String>) {
    let mut title = None;
    let mut tags: Vec<String> = Vec::new();
    let mut in_tag_list = false;
    for line in fm {
        let trimmed = line.trim();
        if in_tag_list {
            if let Some(item) = trimmed.strip_prefix("- ") {
                push_tag(&mut tags, item.to_string());
                continue;
            }
            if trimmed == "-" {
                continue;
            }
            in_tag_list = false;
        }
        let Some((key, value)) = trimmed.split_once(':') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let value = value.trim();
        match key.as_str() {
            "title" => {
                let v = value.trim_matches(['"', '\'']).trim();
                if !v.is_empty() {
                    title = Some(v.to_string());
                }
            }
            "tags" | "tag" | "keywords" => {
                if value.is_empty() {
                    in_tag_list = true;
                    continue;
                }
                let inner = value.trim_start_matches('[').trim_end_matches(']');
                for part in inner.split([',', ';']) {
                    push_tag(&mut tags, part.to_string());
                }
            }
            _ => {}
        }
    }
    (title, tags)
}

// ---------------------------------------------------------------------------
// anchors + ordering
// ---------------------------------------------------------------------------

fn slugify(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            out.push(if c == ' ' { '-' } else { c });
        }
    }
    let out = out.trim_matches('-').to_string();
    if out.is_empty() {
        "note".to_string()
    } else {
        out
    }
}

fn unique_anchor(base: &str, used: &mut Vec<(String, usize)>) -> String {
    if let Some(entry) = used.iter_mut().find(|(a, _)| a == base) {
        entry.1 += 1;
        return format!("{base}-{}", entry.1);
    }
    used.push((base.to_string(), 0));
    base.to_string()
}

fn sort_order(notes: &[Note], sort: &str) -> Vec<usize> {
    let mut order: Vec<usize> = (0..notes.len()).collect();
    match sort {
        "title" => order.sort_by(|a, b| {
            notes[*a]
                .title
                .to_lowercase()
                .cmp(&notes[*b].title.to_lowercase())
                .then(a.cmp(b))
        }),
        "words" => order.sort_by(|a, b| notes[*b].words.cmp(&notes[*a].words).then(a.cmp(b))),
        _ => {}
    }
    order
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

/// Link to a note title, honouring the link style. A note that came from a file marker
/// links to that file; otherwise it links to its own section in this index.
fn note_link(note: &Note, link_style: &str) -> String {
    match link_style {
        "wiki" => format!("[[{}]]", note.title),
        "none" => note.title.clone(),
        _ => match &note.file {
            Some(f) => format!("[{}]({})", note.title, f),
            None => format!("[{}](#{})", note.title, note.anchor),
        },
    }
}

/// Outline entries only become links when there is somewhere real to point at: a source
/// file (`file#anchor`) or a wiki target. Otherwise they stay plain text.
fn heading_link(note: &Note, h: &Heading, link_style: &str) -> String {
    match link_style {
        "wiki" => format!("[[{}#{}]]", note.title, h.text),
        "none" => h.text.clone(),
        _ => match &note.file {
            Some(f) => format!("[{}]({}#{})", h.text, f, h.anchor),
            None => h.text.clone(),
        },
    }
}

fn visible_headings(note: &Note, heading_depth: u32) -> Vec<&Heading> {
    if heading_depth == 0 {
        return Vec::new();
    }
    note.headings
        .iter()
        .filter(|h| h.level <= heading_depth)
        .collect()
}

fn tag_totals(notes: &[Note]) -> Vec<(String, usize)> {
    let mut totals: Vec<(String, usize)> = Vec::new();
    for n in notes {
        for t in &n.tags {
            match totals.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case(t)) {
                Some(e) => e.1 += 1,
                None => totals.push((t.clone(), 1)),
            }
        }
    }
    totals.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
    totals
}

fn render_markdown(
    notes: &[Note],
    order: &[usize],
    heading_depth: u32,
    group_by: &str,
    link_style: &str,
    include_toc: bool,
    include_stats: bool,
) -> String {
    let totals = tag_totals(notes);
    let words: usize = notes.iter().map(|n| n.words).sum();
    let mut out = String::from("# Notes index\n\n");

    let mut summary = vec![
        plural(notes.len(), "note", "notes"),
        plural(totals.len(), "tag", "tags"),
    ];
    if include_stats {
        summary.push(plural(words, "word", "words"));
    }
    out.push_str(&summary.join(" · "));
    out.push_str("\n\n");

    if include_toc {
        out.push_str("## Contents\n\n");
        if group_by == "tag" {
            for (tag, _) in &totals {
                out.push_str(&format!("### #{tag}\n\n"));
                for &i in order {
                    if notes[i].tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                        out.push_str(&format!("- {}\n", note_link(&notes[i], link_style)));
                    }
                }
                out.push('\n');
            }
            let untagged: Vec<usize> = order
                .iter()
                .copied()
                .filter(|i| notes[*i].tags.is_empty())
                .collect();
            if !untagged.is_empty() {
                out.push_str("### Untagged\n\n");
                for i in untagged {
                    out.push_str(&format!("- {}\n", note_link(&notes[i], link_style)));
                }
                out.push('\n');
            }
        } else {
            for (n, &i) in order.iter().enumerate() {
                out.push_str(&format!("{}. {}\n", n + 1, note_link(&notes[i], link_style)));
            }
            out.push('\n');
        }
    }

    for &i in order {
        let note = &notes[i];
        out.push_str(&format!("## {}\n\n", note.title));
        let mut meta: Vec<String> = Vec::new();
        if !note.tags.is_empty() {
            let tags: Vec<String> = note.tags.iter().map(|t| format!("#{t}")).collect();
            meta.push(format!("Tags: {}", tags.join(", ")));
        }
        if let Some(f) = &note.file {
            meta.push(format!("File: {f}"));
        }
        let shown = visible_headings(note, heading_depth);
        if include_stats {
            meta.push(plural(note.words, "word", "words"));
            meta.push(plural(note.headings.len(), "heading", "headings"));
        }
        if !meta.is_empty() {
            out.push_str(&meta.join(" · "));
            out.push_str("\n\n");
        }
        if !shown.is_empty() {
            let base = shown.iter().map(|h| h.level).min().unwrap_or(1);
            for h in shown {
                let indent = "  ".repeat((h.level - base) as usize);
                out.push_str(&format!("{indent}- {}\n", heading_link(note, h, link_style)));
            }
            out.push('\n');
        }
    }

    out.trim_end().to_string() + "\n"
}

fn render_json(
    notes: &[Note],
    order: &[usize],
    heading_depth: u32,
    group_by: &str,
    include_stats: bool,
) -> String {
    let totals = tag_totals(notes);
    let mut index = Vec::new();
    for &i in order {
        let note = &notes[i];
        let headings: Vec<serde_json::Value> = visible_headings(note, heading_depth)
            .iter()
            .map(|h| json!({ "level": h.level, "text": h.text, "anchor": h.anchor }))
            .collect();
        let mut entry = json!({
            "title": note.title,
            "anchor": note.anchor,
            "file": note.file,
            "tags": note.tags,
            "headings": headings,
        });
        if include_stats {
            entry["words"] = json!(note.words);
            entry["heading_count"] = json!(note.headings.len());
        }
        index.push(entry);
    }

    let mut doc = json!({
        "notes": notes.len(),
        "tags": totals
            .iter()
            .map(|(t, c)| json!({ "tag": t, "notes": c }))
            .collect::<Vec<_>>(),
        "index": index,
    });
    if include_stats {
        doc["words"] = json!(notes.iter().map(|n| n.words).sum::<usize>());
    }
    if group_by == "tag" {
        let mut groups = Vec::new();
        for (tag, _) in &totals {
            let titles: Vec<String> = order
                .iter()
                .filter(|i| notes[**i].tags.iter().any(|t| t.eq_ignore_ascii_case(tag)))
                .map(|i| notes[*i].title.clone())
                .collect();
            groups.push(json!({ "tag": tag, "notes": titles }));
        }
        let untagged: Vec<String> = order
            .iter()
            .filter(|i| notes[**i].tags.is_empty())
            .map(|i| notes[*i].title.clone())
            .collect();
        if !untagged.is_empty() {
            groups.push(json!({ "tag": null, "notes": untagged }));
        }
        doc["groups"] = json!(groups);
    }
    serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(notes: &[Note], order: &[usize], include_stats: bool) -> String {
    let mut out = String::from("title,file,tags");
    if include_stats {
        out.push_str(",headings,words");
    }
    out.push('\n');
    for &i in order {
        let note = &notes[i];
        let row = [
            csv_field(&note.title),
            csv_field(note.file.as_deref().unwrap_or("")),
            csv_field(&note.tags.join(";")),
        ];
        out.push_str(&row.join(","));
        if include_stats {
            out.push_str(&format!(",{},{}", note.headings.len(), note.words));
        }
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUNDLE: &str = "# Getting started\n\nSome intro text.\n\n## Install\n\nRun it.\n\n# Weekly review\n\nNotes about the week.\n";

    #[test]
    fn indexes_two_notes_split_by_heading() {
        let got = run_default(BUNDLE).unwrap();
        let expected = "\
# Notes index

2 notes · 0 tags · 14 words

## Contents

1. [Getting started](#getting-started)
2. [Weekly review](#weekly-review)

## Getting started

8 words · 1 heading

- Install

## Weekly review

6 words · 0 headings
";
        assert_eq!(got, expected);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run_default("   \n  ").unwrap_err();
        assert_eq!(err, "expected Markdown notes, got an empty document");
    }

    #[test]
    fn unknown_format_names_the_allowed_values() {
        let err = run(
            BUNDLE, "heading", "yaml", 2, "none", "input", "anchor", true, true, true,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "unknown format \"yaml\": expected one of markdown, json, csv"
        );
    }

    #[test]
    fn heading_depth_above_six_is_rejected() {
        let err = run(
            BUNDLE, "heading", "markdown", 7, "none", "input", "anchor", true, true, true,
        )
        .unwrap_err();
        assert_eq!(err, "heading_depth must be between 0 and 6, got 7");
    }

    #[test]
    fn frontmatter_title_and_tags_win_over_the_first_heading() {
        let input = "---\ntitle: Project brief\ntags: [planning, q3]\n---\n\n# Ignored heading\n\nBody.\n";
        let got = run_default(input).unwrap();
        assert!(got.contains("## Project brief"), "{got}");
        assert!(got.contains("Tags: #planning, #q3"), "{got}");
        // The heading is not the title, so it stays in the outline.
        assert!(got.contains("- Ignored heading"), "{got}");
    }

    #[test]
    fn frontmatter_block_list_tags_are_read() {
        let input = "---\ntags:\n  - alpha\n  - beta\n---\n# Note\n\nBody.\n";
        let got = run_default(input).unwrap();
        assert!(got.contains("Tags: #alpha, #beta"), "{got}");
    }

    #[test]
    fn inline_hashtags_are_collected_and_issue_refs_are_not() {
        let input = "# Standup\n\nBlocked on #infra and #build/ci, see #1234.\n";
        let got = run_default(input).unwrap();
        assert!(got.contains("Tags: #build/ci, #infra"), "{got}");
        assert!(!got.contains("#1234"), "{got}");
    }

    #[test]
    fn inline_tags_can_be_switched_off() {
        let input = "# Standup\n\nBlocked on #infra.\n";
        let got = run(
            input, "heading", "markdown", 2, "none", "input", "anchor", true, true, false,
        )
        .unwrap();
        assert!(!got.contains("#infra"), "{got}");
    }

    #[test]
    fn headings_inside_code_fences_are_ignored() {
        let input = "# Shell notes\n\n```sh\n# not a heading\necho hi\n```\n\n## Real heading\n";
        let got = run_default(input).unwrap();
        assert!(got.contains("- Real heading"), "{got}");
        assert!(!got.contains("not a heading"), "{got}");
    }

    #[test]
    fn heading_depth_limits_the_outline() {
        let input = "# Note\n\n## Two\n\n### Three\n";
        let deep = run(
            input, "heading", "markdown", 3, "none", "input", "anchor", true, true, true,
        )
        .unwrap();
        assert!(deep.contains("  - Three"), "{deep}");
        let shallow = run_default(input).unwrap();
        assert!(!shallow.contains("Three"), "{shallow}");
        let none = run(
            input, "heading", "markdown", 0, "none", "input", "anchor", true, true, true,
        )
        .unwrap();
        assert!(!none.contains("- Two"), "{none}");
    }

    #[test]
    fn thematic_break_split_keeps_frontmatter_with_its_note() {
        let input = "---\ntitle: First\n---\nBody one.\n\n---\n\n---\ntitle: Second\n---\nBody two.\n";
        let got = run(
            input, "hr", "markdown", 2, "none", "input", "anchor", true, true, true,
        )
        .unwrap();
        assert!(got.contains("## First"), "{got}");
        assert!(got.contains("## Second"), "{got}");
        assert!(got.starts_with("# Notes index\n\n2 notes"), "{got}");
    }

    #[test]
    fn file_markers_split_and_supply_links() {
        let input = "=== notes/todo.md ===\n# Todo\n\n## Today\n\n=== notes/ideas.md ===\nIdeas without a heading.\n";
        let got = run(
            input,
            "file-marker",
            "markdown",
            2,
            "none",
            "input",
            "anchor",
            true,
            true,
            true,
        )
        .unwrap();
        assert!(got.contains("1. [Todo](notes/todo.md)"), "{got}");
        assert!(got.contains("- [Today](notes/todo.md#today)"), "{got}");
        // No heading and no front matter: the file name becomes the title.
        assert!(got.contains("## ideas"), "{got}");
    }

    #[test]
    fn head_style_file_markers_are_recognised() {
        let input = "==> a.md <==\n# Alpha\n\n==> b.md <==\n# Beta\n";
        let got = run(
            input,
            "file-marker",
            "markdown",
            2,
            "none",
            "input",
            "anchor",
            true,
            true,
            true,
        )
        .unwrap();
        assert!(got.contains("[Alpha](a.md)"), "{got}");
        assert!(got.contains("[Beta](b.md)"), "{got}");
    }

    #[test]
    fn file_marker_split_without_markers_is_an_error() {
        let err = run(
            BUNDLE,
            "file-marker",
            "markdown",
            2,
            "none",
            "input",
            "anchor",
            true,
            true,
            true,
        )
        .unwrap_err();
        assert!(err.starts_with("no file markers found"), "{err}");
    }

    #[test]
    fn tag_grouping_lists_each_note_under_every_tag() {
        let input = "# One\n\n#alpha #beta\n\n# Two\n\n#beta\n\n# Three\n\nNo tags here.\n";
        let got = run(
            input, "heading", "markdown", 2, "tag", "input", "anchor", true, true, true,
        )
        .unwrap();
        assert!(got.contains("### #alpha\n\n- [One](#one)\n"), "{got}");
        assert!(
            got.contains("### #beta\n\n- [One](#one)\n- [Two](#two)\n"),
            "{got}"
        );
        assert!(got.contains("### Untagged\n\n- [Three](#three)\n"), "{got}");
    }

    #[test]
    fn wiki_links_target_the_vault_note() {
        let input = "# Meeting notes\n\n## Actions\n";
        let got = run(
            input, "heading", "markdown", 2, "none", "input", "wiki", true, true, true,
        )
        .unwrap();
        assert!(got.contains("1. [[Meeting notes]]"), "{got}");
        assert!(got.contains("- [[Meeting notes#Actions]]"), "{got}");
    }

    #[test]
    fn link_style_none_leaves_plain_titles() {
        let got = run(
            BUNDLE, "heading", "markdown", 2, "none", "input", "none", true, true, true,
        )
        .unwrap();
        assert!(got.contains("1. Getting started"), "{got}");
        assert!(!got.contains("](#"), "{got}");
    }

    #[test]
    fn duplicate_titles_get_distinct_anchors() {
        let input = "# Log\n\nOne.\n\n# Log\n\nTwo.\n";
        let got = run_default(input).unwrap();
        assert!(got.contains("1. [Log](#log)\n2. [Log](#log-1)"), "{got}");
    }

    #[test]
    fn sort_by_title_and_by_words() {
        let input = "# Zebra\n\none two three four\n\n# Apple\n\none\n";
        let by_title = run(
            input, "heading", "markdown", 2, "none", "title", "anchor", true, true, true,
        )
        .unwrap();
        assert!(
            by_title.find("[Apple]").unwrap() < by_title.find("[Zebra]").unwrap(),
            "{by_title}"
        );
        let by_words = run(
            input, "heading", "markdown", 2, "none", "words", "anchor", true, true, true,
        )
        .unwrap();
        assert!(
            by_words.find("[Zebra]").unwrap() < by_words.find("[Apple]").unwrap(),
            "{by_words}"
        );
    }

    #[test]
    fn toc_can_be_switched_off_and_stats_hidden() {
        let got = run(
            BUNDLE, "heading", "markdown", 2, "none", "input", "anchor", false, false, true,
        )
        .unwrap();
        assert!(!got.contains("## Contents"), "{got}");
        assert!(!got.contains("words"), "{got}");
    }

    #[test]
    fn json_output_has_the_documented_shape() {
        let input = "---\ntags: [ops]\n---\n# Runbook\n\n## Restart\n\nSteps.\n";
        let got = run(
            input, "heading", "json", 2, "none", "input", "anchor", true, true, true,
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(v["notes"], 1);
        assert_eq!(v["tags"][0]["tag"], "ops");
        assert_eq!(v["tags"][0]["notes"], 1);
        assert_eq!(v["index"][0]["title"], "Runbook");
        assert_eq!(v["index"][0]["anchor"], "runbook");
        assert_eq!(v["index"][0]["headings"][0]["text"], "Restart");
        assert_eq!(v["index"][0]["headings"][0]["level"], 2);
        assert_eq!(v["index"][0]["words"], 3);
    }

    #[test]
    fn csv_output_quotes_commas() {
        let input = "# Plans, ideas\n\n#a #b\n\n# Second\n\nText.\n";
        let got = run(
            input, "heading", "csv", 2, "none", "input", "anchor", true, true, true,
        )
        .unwrap();
        assert_eq!(
            got,
            "title,file,tags,headings,words\n\"Plans, ideas\",,a;b,0,4\nSecond,,,0,2\n"
        );
    }

    #[test]
    fn note_cap_is_enforced_at_the_boundary() {
        let ok: String = (1..=MAX_NOTES).map(|i| format!("# Note {i}\n\nx\n\n")).collect();
        let got = run_default(&ok).unwrap();
        assert!(got.starts_with("# Notes index\n\n500 notes"), "{got}");

        let over: String = (1..=MAX_NOTES + 1)
            .map(|i| format!("# Note {i}\n\nx\n\n"))
            .collect();
        let err = run_default(&over).unwrap_err();
        assert_eq!(
            err,
            "too many notes: 501 found, the limit is 500 per run — split the bundle and index it in batches"
        );
    }
}
