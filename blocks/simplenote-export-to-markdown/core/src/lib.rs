//! simplenote-export-to-markdown core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps. Converts a Simplenote (or Evernote-style) JSON export into a
//! labeled bundle of clean Markdown files with titles, tags, and dates.

use serde_json::Value;
use std::collections::HashSet;

/// Max characters kept from a slugged title before truncating at a word boundary.
pub const MAX_SLUG_LEN: usize = 60;

/// Convert a JSON note export into a bundle of Markdown files.
///
/// - `input`: the pasted JSON export. Three shapes are auto-detected:
///   modern Simplenote (`{ "activeNotes": [...], "trashedNotes": [...] }`),
///   legacy Simplenote / generic array of note objects, and Evernote-style
///   arrays that carry an explicit `title` field.
/// - `filename_style`: `date-title` (default), `title`, or `id`.
/// - `metadata`: `frontmatter` (YAML block, default) or `inline` (#hashtags).
/// - `include_trashed`: include trashed/deleted notes (default false).
pub fn convert(
    input: &str,
    filename_style: &str,
    metadata: &str,
    include_trashed: bool,
) -> Result<String, String> {
    let style = match filename_style.trim() {
        "" | "date-title" => FilenameStyle::DateTitle,
        "title" => FilenameStyle::Title,
        "id" => FilenameStyle::Id,
        other => {
            return Err(format!(
                "invalid filename_style '{other}': expected one of date-title, title, id"
            ))
        }
    };
    let meta = match metadata.trim() {
        "" | "frontmatter" => Metadata::Frontmatter,
        "inline" => Metadata::Inline,
        other => {
            return Err(format!(
                "invalid metadata '{other}': expected one of frontmatter, inline"
            ))
        }
    };

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("no input: paste the JSON from your Simplenote export".into());
    }
    let root: Value = serde_json::from_str(trimmed)
        .map_err(|e| format!("could not parse JSON export: {e}"))?;

    // Collect (note, is_trashed) pairs across the supported shapes.
    let mut notes: Vec<(&Value, bool)> = Vec::new();
    match &root {
        Value::Object(obj) if obj.contains_key("activeNotes") || obj.contains_key("trashedNotes") => {
            // Modern Simplenote export.
            if let Some(Value::Array(a)) = obj.get("activeNotes") {
                notes.extend(a.iter().map(|n| (n, false)));
            }
            if let Some(Value::Array(a)) = obj.get("trashedNotes") {
                notes.extend(a.iter().map(|n| (n, true)));
            }
        }
        Value::Array(arr) => {
            // Legacy Simplenote / Evernote-style array of note objects. Trash is
            // signalled per-note via a truthy deleted/trashed flag.
            for n in arr {
                let trashed = is_truthy(n.get("deleted"))
                    || is_truthy(n.get("trashed"))
                    || is_truthy(n.get("inTrash"));
                notes.push((n, trashed));
            }
        }
        Value::Object(obj) if obj.contains_key("notes") => {
            if let Some(Value::Array(a)) = obj.get("notes") {
                for n in a {
                    let trashed = is_truthy(n.get("deleted")) || is_truthy(n.get("trashed"));
                    notes.push((n, trashed));
                }
            }
        }
        _ => {
            return Err("unrecognized export shape: expected a Simplenote export object with \
                        an 'activeNotes' array, or a JSON array of note objects"
                .into())
        }
    }

    let mut used: HashSet<String> = HashSet::new();
    let mut files: Vec<String> = Vec::new();
    for (note, trashed) in notes {
        if trashed && !include_trashed {
            continue;
        }
        if !note.is_object() {
            continue;
        }
        let doc = render_note(note, meta);
        let filename = unique_name(dedupe_name(&doc, style), &mut used);
        files.push(format!("==== {filename} ====\n{}", doc.body));
    }

    if files.is_empty() {
        return Err("no notes found in the export (nothing to convert)".into());
    }
    Ok(files.join("\n\n"))
}

#[derive(Clone, Copy)]
enum FilenameStyle {
    DateTitle,
    Title,
    Id,
}

#[derive(Clone, Copy)]
enum Metadata {
    Frontmatter,
    Inline,
}

struct RenderedNote {
    /// The full Markdown file contents.
    body: String,
    /// Slugged title (may be empty → "untitled").
    slug: String,
    /// `YYYY-MM-DD` creation date, if one was found.
    date: Option<String>,
    /// Note id/key, if one was found.
    id: Option<String>,
}

fn render_note(note: &Value, meta: Metadata) -> RenderedNote {
    let content = first_str(note, &["content", "text", "body", "note"]).unwrap_or_default();
    let explicit_title = first_str(note, &["title", "name"]).filter(|s| !s.trim().is_empty());

    let (title, note_body) = match explicit_title {
        Some(t) => (t.trim().to_string(), content.clone()),
        None => split_title(&content),
    };

    let created = get_date(note, CREATED_KEYS);
    let modified = get_date(note, MODIFIED_KEYS);
    let tags = get_tags(note);
    let pinned = is_truthy(note.get("pinned"));
    let markdown = is_truthy(note.get("markdown"));

    let mut out = String::new();
    if let Metadata::Frontmatter = meta {
        out.push_str("---\n");
        out.push_str(&format!("title: {}\n", yaml_quote(&title)));
        if let Some(c) = &created {
            out.push_str(&format!("created: {}\n", c.iso));
        }
        if let Some(m) = &modified {
            out.push_str(&format!("updated: {}\n", m.iso));
        }
        if !tags.is_empty() {
            let joined = tags
                .iter()
                .map(|t| yaml_quote(t))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("tags: [{joined}]\n"));
        }
        if pinned {
            out.push_str("pinned: true\n");
        }
        if markdown {
            out.push_str("markdown: true\n");
        }
        out.push_str("---\n\n");
    }

    let heading = if title.is_empty() { "Untitled" } else { &title };
    out.push_str(&format!("# {heading}\n"));
    let trimmed_body = note_body.trim();
    if !trimmed_body.is_empty() {
        out.push_str("\n");
        out.push_str(trimmed_body);
        out.push('\n');
    }

    if let Metadata::Inline = meta {
        if !tags.is_empty() {
            let hashtags = tags
                .iter()
                .map(|t| format!("#{}", hashtagify(t)))
                .collect::<Vec<_>>()
                .join(" ");
            out.push_str(&format!("\n{hashtags}\n"));
        }
    }

    RenderedNote {
        body: out,
        slug: slugify(&title),
        date: created.map(|d| d.date),
        id: first_str(note, &["id", "key", "uuid", "guid"]).filter(|s| !s.trim().is_empty()),
    }
}

fn dedupe_name(doc: &RenderedNote, style: FilenameStyle) -> String {
    let slug = if doc.slug.is_empty() {
        "untitled".to_string()
    } else {
        doc.slug.clone()
    };
    match style {
        FilenameStyle::Title => format!("{slug}.md"),
        FilenameStyle::DateTitle => match &doc.date {
            Some(d) => format!("{d}-{slug}.md"),
            None => format!("{slug}.md"),
        },
        FilenameStyle::Id => match &doc.id {
            Some(id) => format!("{}.md", slugify(id)),
            None => format!("{slug}.md"),
        },
    }
}

fn unique_name(name: String, used: &mut HashSet<String>) -> String {
    if used.insert(name.clone()) {
        return name;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((s, e)) => (s.to_string(), format!(".{e}")),
        None => (name.clone(), String::new()),
    };
    let mut n = 1;
    loop {
        let candidate = format!("{stem}-{n}{ext}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}

/// First non-empty line becomes the title; the remainder is the body.
fn split_title(content: &str) -> (String, String) {
    let mut title = String::new();
    let mut consumed = 0usize;
    for (i, line) in content.lines().enumerate() {
        if !line.trim().is_empty() {
            title = line.trim().to_string();
            consumed = i + 1;
            break;
        }
    }
    let body: String = content
        .lines()
        .skip(consumed)
        .collect::<Vec<_>>()
        .join("\n");
    (title, body)
}

fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.len() > MAX_SLUG_LEN {
        out.truncate(MAX_SLUG_LEN);
        while out.ends_with('-') {
            out.pop();
        }
    }
    out
}

/// Tag as a `#hashtag`: runs of non-word characters collapse to a single `-`.
fn hashtagify(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_alphanumeric() || c == '_' {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash && !out.is_empty() {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

fn yaml_quote(s: &str) -> String {
    let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
    format!("\"{escaped}\"")
}

fn first_str(note: &Value, keys: &[&str]) -> Option<String> {
    for k in keys {
        if let Some(Value::String(s)) = note.get(*k) {
            return Some(s.clone());
        }
    }
    None
}

fn get_tags(note: &Value) -> Vec<String> {
    match note.get("tags") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
        Some(Value::String(s)) => s
            .split(',')
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .collect(),
        _ => Vec::new(),
    }
}

fn is_truthy(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().map(|f| f != 0.0).unwrap_or(false),
        Some(Value::String(s)) => {
            let s = s.trim().to_ascii_lowercase();
            matches!(s.as_str(), "true" | "1" | "yes")
        }
        _ => false,
    }
}

const CREATED_KEYS: &[&str] = &[
    "creationDate",
    "creation_date",
    "createdate",
    "created",
    "created_at",
    "dateCreated",
];
const MODIFIED_KEYS: &[&str] = &[
    "lastModified",
    "last_modified",
    "modifydate",
    "modificationDate",
    "updated",
    "updated_at",
    "modified",
    "dateModified",
];

struct NoteDate {
    date: String,
    iso: String,
}

fn get_date(note: &Value, keys: &[&str]) -> Option<NoteDate> {
    for k in keys {
        match note.get(*k) {
            Some(Value::String(s)) => {
                let s = s.trim();
                if s.is_empty() {
                    continue;
                }
                if looks_iso(s) {
                    let date = s.chars().take(10).collect::<String>();
                    return Some(NoteDate {
                        date,
                        iso: s.to_string(),
                    });
                }
                if let Ok(secs) = s.parse::<f64>() {
                    return Some(epoch_to_date(secs as i64));
                }
            }
            Some(Value::Number(n)) => {
                if let Some(secs) = n.as_f64() {
                    return Some(epoch_to_date(secs as i64));
                }
            }
            _ => {}
        }
    }
    None
}

fn looks_iso(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() >= 10
        && b[0].is_ascii_digit()
        && b[1].is_ascii_digit()
        && b[2].is_ascii_digit()
        && b[3].is_ascii_digit()
        && b[4] == b'-'
        && b[7] == b'-'
}

fn epoch_to_date(secs: i64) -> NoteDate {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let hh = rem / 3600;
    let mm = (rem % 3600) / 60;
    let ss = rem % 60;
    NoteDate {
        date: format!("{y:04}-{m:02}-{d:02}"),
        iso: format!("{y:04}-{m:02}-{d:02}T{hh:02}:{mm:02}:{ss:02}Z"),
    }
}

/// Howard Hinnant's days-from-civil, inverted: civil date from a day count
/// relative to the Unix epoch (1970-01-01). Pure integer math, no deps.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    (y + if m <= 2 { 1 } else { 0 }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MODERN: &str = r#"{
      "activeNotes": [
        {
          "id": "abc-123",
          "content": "Grocery List\nMilk\nEggs",
          "tags": ["home", "shopping list"],
          "creationDate": "2026-01-15T09:30:00.000Z",
          "lastModified": "2026-01-16T10:00:00.000Z",
          "pinned": true,
          "markdown": false
        }
      ],
      "trashedNotes": [
        { "id": "old-9", "content": "Deleted idea", "tags": [], "creationDate": "2025-12-01T00:00:00.000Z" }
      ]
    }"#;

    #[test]
    fn modern_frontmatter_date_title() {
        let out = convert(MODERN, "date-title", "frontmatter", false).unwrap();
        let expected = "==== 2026-01-15-grocery-list.md ====\n\
---\n\
title: \"Grocery List\"\n\
created: 2026-01-15T09:30:00.000Z\n\
updated: 2026-01-16T10:00:00.000Z\n\
tags: [\"home\", \"shopping list\"]\n\
pinned: true\n\
---\n\
\n\
# Grocery List\n\
\n\
Milk\nEggs\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn trashed_excluded_by_default() {
        let out = convert(MODERN, "date-title", "frontmatter", false).unwrap();
        assert!(!out.contains("Deleted idea"));
        let with = convert(MODERN, "date-title", "frontmatter", true).unwrap();
        assert!(with.contains("Deleted idea"));
        assert!(with.contains("==== 2025-12-01-deleted-idea.md ===="));
    }

    #[test]
    fn inline_hashtags_and_title_style() {
        let out = convert(MODERN, "title", "inline", false).unwrap();
        assert!(out.starts_with("==== grocery-list.md ====\n# Grocery List\n"));
        assert!(out.contains("#home #shopping-list"));
        assert!(!out.contains("---"));
    }

    #[test]
    fn id_filename_style() {
        let out = convert(MODERN, "id", "frontmatter", false).unwrap();
        assert!(out.starts_with("==== abc-123.md ===="));
    }

    #[test]
    fn legacy_epoch_and_explicit_title() {
        let legacy = r#"[
          { "key": "k1", "content": "First line body\nmore", "tags": ["a"], "createdate": 1451606400, "modifydate": 1451692800 }
        ]"#;
        let out = convert(legacy, "date-title", "frontmatter", false).unwrap();
        // 1451606400 = 2016-01-01T00:00:00Z
        assert!(out.starts_with("==== 2016-01-01-first-line-body.md ===="));
        assert!(out.contains("created: 2016-01-01T00:00:00Z"));
    }

    #[test]
    fn evernote_explicit_title_kept_in_body() {
        let ever = r#"[
          { "title": "Meeting Notes", "content": "Discussed roadmap", "tags": ["work"], "created": "2026-03-04T12:00:00Z" }
        ]"#;
        let out = convert(ever, "title", "frontmatter", false).unwrap();
        assert!(out.contains("# Meeting Notes\n\nDiscussed roadmap\n"));
        assert!(out.contains("==== meeting-notes.md ===="));
    }

    #[test]
    fn collision_suffix() {
        let dup = r#"{ "activeNotes": [
          { "id": "1", "content": "Same" },
          { "id": "2", "content": "Same" }
        ] }"#;
        let out = convert(dup, "title", "inline", false).unwrap();
        assert!(out.contains("==== same.md ===="));
        assert!(out.contains("==== same-1.md ===="));
    }

    #[test]
    fn error_on_bad_json() {
        assert!(convert("not json", "date-title", "frontmatter", false).is_err());
    }

    #[test]
    fn error_on_empty() {
        assert!(convert("   ", "date-title", "frontmatter", false).is_err());
    }

    #[test]
    fn error_on_no_notes() {
        assert!(convert(r#"{"activeNotes": []}"#, "date-title", "frontmatter", false).is_err());
    }

    #[test]
    fn error_on_bad_style() {
        assert!(convert(MODERN, "bogus", "frontmatter", false).is_err());
    }
}
