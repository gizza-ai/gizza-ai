//! keep-to-markdown core — pure compute, shared by the chat skill block and the web page.
//! Converts a Google Takeout Keep export (per-note JSON, a JSON array of notes, or the
//! Keep HTML export) into a bundle of Markdown notes, preserving labels and checkboxes.
//!
//! No wafer/wasm-bindgen deps, no clock, no I/O — the whole conversion is deterministic.

use serde_json::Value;

/// Takeout Keep exports are per-note files; even a concatenated array stays small.
/// The block runs in a 64 MiB wasm sandbox, so cap the input well below that.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;

/// Longest slug (in chars) taken from a note title when building a filename.
const MAX_SLUG_CHARS: usize = 60;

// ---------------------------------------------------------------------------
// Parsed note model
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Clone)]
struct ListItem {
    text: String,
    checked: bool,
}

#[derive(Debug, Default, Clone)]
struct Attachment {
    name: String,
    is_image: bool,
}

#[derive(Debug, Default, Clone)]
struct Note {
    title: String,
    text: String,
    items: Vec<ListItem>,
    labels: Vec<String>,
    /// ISO-8601 UTC instant, e.g. `2026-01-15T09:30:00Z`.
    created: Option<String>,
    updated: Option<String>,
    pinned: bool,
    archived: bool,
    trashed: bool,
    color: Option<String>,
    attachments: Vec<Attachment>,
    /// Weblink annotations (Keep's link chips): (title, url).
    links: Vec<(String, String)>,
}

impl Note {
    fn is_empty(&self) -> bool {
        self.title.trim().is_empty()
            && self.text.trim().is_empty()
            && self.items.is_empty()
            && self.attachments.is_empty()
            && self.links.is_empty()
    }

    /// The date part (YYYY-MM-DD) of the creation instant, for `date-title` filenames.
    fn created_date(&self) -> Option<&str> {
        self.created.as_deref().and_then(|s| s.get(..10))
    }
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Metadata {
    Frontmatter,
    Inline,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum FilenameStyle {
    DateTitle,
    Title,
    LabelTitle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CheckboxStyle {
    TaskList,
    Bullet,
    Plain,
}

fn pick<T: Copy>(value: &str, default: T, choices: &[(&str, T)], param: &str) -> Result<T, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default);
    }
    for (name, out) in choices {
        if v.eq_ignore_ascii_case(name) {
            return Ok(*out);
        }
    }
    let names: Vec<&str> = choices.iter().map(|(n, _)| *n).collect();
    Err(format!(
        "{param} must be one of {} (got '{v}')",
        names.join(", ")
    ))
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Convert a Google Keep Takeout export into a Markdown bundle.
///
/// `input` is one note's `.json`, a JSON array of such notes, or the Keep `.html` export.
/// Each note becomes one `==== filename.md ====` section so the bundle can be split into files.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    metadata: &str,
    filename_style: &str,
    checkbox_style: &str,
    include_archived: bool,
    include_trashed: bool,
    link_attachments: bool,
) -> Result<String, String> {
    let metadata = pick(
        metadata,
        Metadata::Frontmatter,
        &[
            ("frontmatter", Metadata::Frontmatter),
            ("inline", Metadata::Inline),
            ("none", Metadata::None),
        ],
        "metadata",
    )?;
    let filename_style = pick(
        filename_style,
        FilenameStyle::DateTitle,
        &[
            ("date-title", FilenameStyle::DateTitle),
            ("title", FilenameStyle::Title),
            ("label-title", FilenameStyle::LabelTitle),
        ],
        "filename_style",
    )?;
    let checkbox_style = pick(
        checkbox_style,
        CheckboxStyle::TaskList,
        &[
            ("task-list", CheckboxStyle::TaskList),
            ("bullet", CheckboxStyle::Bullet),
            ("plain", CheckboxStyle::Plain),
        ],
        "checkbox_style",
    )?;

    let trimmed = input.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err("input is empty — paste a Google Keep Takeout note (its .json), a JSON array of notes, or the Keep .html export".into());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes (~4 MB) — convert your Takeout notes in smaller batches",
            input.len(),
            MAX_INPUT_BYTES
        ));
    }

    let notes = if trimmed.starts_with('{') || trimmed.starts_with('[') {
        parse_json_export(trimmed)?
    } else if trimmed.starts_with('<') || trimmed.to_ascii_lowercase().contains("<html") {
        parse_html_export(trimmed)?
    } else {
        return Err("unrecognised input — expected a Keep Takeout note in JSON (starting with '{' or '[') or the Keep HTML export (starting with '<')".into());
    };

    if notes.is_empty() {
        return Err("no Keep notes found in the input — a Takeout Keep note is a JSON object with textContent or listContent, and the HTML export wraps each note in a note block".into());
    }

    let kept: Vec<&Note> = notes
        .iter()
        .filter(|n| (include_archived || !n.archived) && (include_trashed || !n.trashed))
        .collect();
    if kept.is_empty() {
        return Err(format!(
            "all {} note(s) were filtered out — turn on 'Include archived notes' or 'Include trashed notes' to export them",
            notes.len()
        ));
    }

    let mut used: Vec<String> = Vec::new();
    let mut sections: Vec<String> = Vec::new();
    for note in kept {
        let name = unique_name(&filename_for(note, filename_style), &mut used);
        let body = render_note(note, metadata, checkbox_style, link_attachments);
        sections.push(format!("==== {name} ====\n{body}"));
    }
    Ok(sections.join("\n\n"))
}

// ---------------------------------------------------------------------------
// JSON export
// ---------------------------------------------------------------------------

fn parse_json_export(input: &str) -> Result<Vec<Note>, String> {
    let value: Value = serde_json::from_str(input)
        .map_err(|e| format!("input looks like JSON but could not be parsed: {e}"))?;
    match value {
        Value::Object(_) => Ok(vec![note_from_json(&value)?]),
        Value::Array(items) => {
            let mut notes = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                if !item.is_object() {
                    return Err(format!(
                        "JSON array entry {} is not a note object — expected an array of Takeout Keep notes",
                        i + 1
                    ));
                }
                notes.push(note_from_json(item)?);
            }
            Ok(notes)
        }
        _ => Err(
            "expected a Takeout Keep note object, or a JSON array of note objects".to_string(),
        ),
    }
}

fn note_from_json(v: &Value) -> Result<Note, String> {
    let mut note = Note {
        title: v
            .get("title")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim()
            .to_string(),
        text: v
            .get("textContent")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .replace("\r\n", "\n")
            .trim_end()
            .to_string(),
        pinned: v.get("isPinned").and_then(Value::as_bool).unwrap_or(false),
        archived: v.get("isArchived").and_then(Value::as_bool).unwrap_or(false),
        trashed: v.get("isTrashed").and_then(Value::as_bool).unwrap_or(false),
        created: usec_field(v, "createdTimestampUsec"),
        updated: usec_field(v, "userEditedTimestampUsec"),
        ..Note::default()
    };

    if let Some(color) = v.get("color").and_then(Value::as_str) {
        let color = color.trim();
        if !color.is_empty() && !color.eq_ignore_ascii_case("DEFAULT") {
            note.color = Some(color.to_string());
        }
    }

    if let Some(items) = v.get("listContent").and_then(Value::as_array) {
        for item in items {
            let text = item
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            let checked = item
                .get("isChecked")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !text.is_empty() {
                note.items.push(ListItem { text, checked });
            }
        }
    }

    if let Some(labels) = v.get("labels").and_then(Value::as_array) {
        for label in labels {
            let name = label
                .get("name")
                .and_then(Value::as_str)
                .or_else(|| label.as_str())
                .unwrap_or_default()
                .trim();
            if !name.is_empty() {
                note.labels.push(name.to_string());
            }
        }
    }

    if let Some(atts) = v.get("attachments").and_then(Value::as_array) {
        for att in atts {
            let path = att
                .get("filePath")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if path.is_empty() {
                continue;
            }
            let mime = att
                .get("mimetype")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let name = file_name(path);
            let is_image = mime.starts_with("image/") || looks_like_image(&name);
            note.attachments.push(Attachment { name, is_image });
        }
    }

    if let Some(anns) = v.get("annotations").and_then(Value::as_array) {
        for ann in anns {
            let url = ann
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            if url.is_empty() {
                continue;
            }
            let title = ann
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .trim();
            let title = if title.is_empty() { url } else { title };
            note.links.push((title.to_string(), url.to_string()));
        }
    }

    Ok(note)
}

/// Keep stores timestamps as microseconds since the Unix epoch, sometimes JSON-stringified.
fn usec_field(v: &Value, key: &str) -> Option<String> {
    let raw = v.get(key)?;
    let usec = match raw {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64))?,
        Value::String(s) => s.trim().parse::<i64>().ok()?,
        _ => return None,
    };
    if usec == 0 {
        return None;
    }
    Some(iso_from_secs(usec.div_euclid(1_000_000)))
}

// ---------------------------------------------------------------------------
// Time formatting (no chrono: the conversion must stay clock-free and wasm-safe)
// ---------------------------------------------------------------------------

/// Howard Hinnant's civil-from-days algorithm: days since 1970-01-01 → (year, month, day).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn iso_from_secs(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

// ---------------------------------------------------------------------------
// HTML export — a forgiving tag scanner (Keep's export is not well-formed XML)
// ---------------------------------------------------------------------------

#[derive(Debug)]
enum Node {
    Text(String),
    El(El),
}

#[derive(Debug, Default)]
struct El {
    tag: String,
    class: String,
    src: String,
    href: String,
    children: Vec<Node>,
}

impl El {
    fn has_class(&self, want: &str) -> bool {
        self.class.split_whitespace().any(|c| c == want)
    }
}

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];

fn parse_html_export(input: &str) -> Result<Vec<Note>, String> {
    let dom = parse_nodes(input);
    let mut note_els: Vec<&El> = Vec::new();
    collect_class(&dom, "note", &mut note_els);
    if note_els.is_empty() {
        // A single-note export sometimes lacks the wrapper div; fall back to the body.
        let mut body: Vec<&El> = Vec::new();
        collect_tag(&dom, "body", &mut body);
        note_els.extend(body);
    }
    let mut notes: Vec<Note> = Vec::new();
    for el in note_els {
        let note = note_from_html(el);
        if !note.is_empty() {
            notes.push(note);
        }
    }
    if notes.is_empty() {
        return Err("no Keep notes found in the HTML export — each note lives in a note block containing a title and a content block".into());
    }
    Ok(notes)
}

/// Collect every descendant of `el` carrying `class`.
fn find_class<'a>(el: &'a El, class: &str) -> Vec<&'a El> {
    let mut out = Vec::new();
    collect_class(&el.children, class, &mut out);
    out
}

fn find_tag<'a>(el: &'a El, tag: &str) -> Vec<&'a El> {
    let mut out = Vec::new();
    collect_tag(&el.children, tag, &mut out);
    out
}

fn note_from_html(el: &El) -> Note {
    let mut note = Note::default();

    if let Some(t) = find_class(el, "title").first() {
        note.title = text_of_el(t).trim().to_string();
    }
    if let Some(h) = find_class(el, "heading").first() {
        note.created = parse_us_datetime(&text_of_el(h));
    }

    if let Some(c) = find_class(el, "content").first() {
        let items = find_class(c, "listitem");
        if items.is_empty() {
            note.text = text_of_el(c).trim().to_string();
        } else {
            for item in items {
                let raw = match find_class(item, "text").first() {
                    Some(t) => text_of_el(t),
                    None => text_of_el(item),
                };
                let text = strip_bullets(&raw);
                if text.is_empty() {
                    continue;
                }
                note.items.push(ListItem {
                    checked: item.has_class("checked") || raw.contains('\u{2611}'),
                    text,
                });
            }
        }
    }

    let mut label_els = find_class(el, "label-name");
    if label_els.is_empty() {
        label_els = find_class(el, "label");
    }
    for l in label_els {
        let name = text_of_el(l).trim().to_string();
        if !name.is_empty() && !note.labels.contains(&name) {
            note.labels.push(name);
        }
    }

    note.archived = !find_class(el, "archived").is_empty();
    note.pinned = !find_class(el, "pinned").is_empty();

    for m in find_tag(el, "img") {
        if m.src.is_empty() {
            continue;
        }
        let name = file_name(&m.src);
        note.attachments.push(Attachment {
            is_image: looks_like_image(&name),
            name,
        });
    }

    for a in find_tag(el, "a") {
        if !a.href.starts_with("http") {
            continue;
        }
        let title = text_of_el(a).trim().to_string();
        let title = if title.is_empty() {
            a.href.clone()
        } else {
            title
        };
        note.links.push((title, a.href.clone()));
    }

    note
}

fn parse_nodes(input: &str) -> Vec<Node> {
    let bytes: Vec<char> = input.chars().collect();
    let mut i = 0usize;
    let mut stack: Vec<El> = vec![El {
        tag: "#root".into(),
        ..El::default()
    }];
    let mut text = String::new();

    macro_rules! flush_text {
        () => {
            if !text.is_empty() {
                let t = std::mem::take(&mut text);
                stack.last_mut().unwrap().children.push(Node::Text(t));
            }
        };
    }

    while i < bytes.len() {
        if bytes[i] != '<' {
            text.push(bytes[i]);
            i += 1;
            continue;
        }
        // Comment / doctype.
        if input_slice_starts(&bytes, i, "<!--") {
            flush_text!();
            i = find_seq(&bytes, i + 4, "-->").map(|p| p + 3).unwrap_or(bytes.len());
            continue;
        }
        if input_slice_starts(&bytes, i, "<!") || input_slice_starts(&bytes, i, "<?") {
            flush_text!();
            i = find_seq(&bytes, i + 2, ">").map(|p| p + 1).unwrap_or(bytes.len());
            continue;
        }
        let Some(end) = find_tag_end(&bytes, i) else {
            text.push(bytes[i]);
            i += 1;
            continue;
        };
        let raw: String = bytes[i + 1..end].iter().collect();
        i = end + 1;
        flush_text!();

        if let Some(name) = raw.strip_prefix('/') {
            let name = name.trim().to_ascii_lowercase();
            // Pop to the matching open tag; unbalanced markup just closes nothing.
            if let Some(pos) = stack.iter().rposition(|e| e.tag == name) {
                if pos > 0 {
                    while stack.len() > pos {
                        let el = stack.pop().unwrap();
                        stack.last_mut().unwrap().children.push(Node::El(el));
                    }
                }
            }
            continue;
        }

        let self_closing = raw.trim_end().ends_with('/');
        let raw = raw.trim_end().trim_end_matches('/');
        let mut parts = raw.splitn(2, |c: char| c.is_whitespace());
        let tag = parts.next().unwrap_or_default().to_ascii_lowercase();
        let attrs = parts.next().unwrap_or_default();
        let el = El {
            class: attr_value(attrs, "class"),
            src: attr_value(attrs, "src"),
            href: attr_value(attrs, "href"),
            tag: tag.clone(),
            children: Vec::new(),
        };

        if tag == "script" || tag == "style" {
            // Skip raw-text element contents entirely.
            let close = format!("</{tag}");
            i = find_seq(&bytes, i, &close)
                .and_then(|p| find_seq(&bytes, p, ">").map(|q| q + 1))
                .unwrap_or(bytes.len());
            continue;
        }
        if self_closing || VOID_TAGS.contains(&tag.as_str()) {
            stack.last_mut().unwrap().children.push(Node::El(el));
            continue;
        }
        stack.push(el);
    }
    flush_text!();
    while stack.len() > 1 {
        let el = stack.pop().unwrap();
        stack.last_mut().unwrap().children.push(Node::El(el));
    }
    stack.pop().unwrap().children
}

fn input_slice_starts(chars: &[char], at: usize, needle: &str) -> bool {
    needle
        .chars()
        .enumerate()
        .all(|(k, c)| chars.get(at + k).copied() == Some(c))
}

fn find_seq(chars: &[char], from: usize, needle: &str) -> Option<usize> {
    let n: Vec<char> = needle.chars().collect();
    if n.is_empty() || from >= chars.len() {
        return None;
    }
    (from..=chars.len().saturating_sub(n.len())).find(|&p| chars[p..p + n.len()] == n[..])
}

/// Find the `>` that closes the tag opened at `start`, skipping quoted attribute values.
fn find_tag_end(chars: &[char], start: usize) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (p, &c) in chars.iter().enumerate().skip(start + 1) {
        match quote {
            Some(q) if c == q => quote = None,
            Some(_) => {}
            None if c == '"' || c == '\'' => quote = Some(c),
            None if c == '>' => return Some(p),
            None => {}
        }
    }
    None
}

fn attr_value(attrs: &str, name: &str) -> String {
    let lower = attrs.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(pos) = lower[from..].find(name) {
        let at = from + pos;
        let before_ok = at == 0
            || lower[..at]
                .chars()
                .next_back()
                .map(|c| c.is_whitespace())
                .unwrap_or(false);
        let rest = &attrs[at + name.len()..];
        let trimmed = rest.trim_start();
        if before_ok && trimmed.starts_with('=') {
            let val = trimmed[1..].trim_start();
            let out = if let Some(stripped) = val.strip_prefix('"') {
                stripped.split('"').next().unwrap_or_default()
            } else if let Some(stripped) = val.strip_prefix('\'') {
                stripped.split('\'').next().unwrap_or_default()
            } else {
                val.split_whitespace().next().unwrap_or_default()
            };
            return decode_entities(out);
        }
        from = at + name.len();
    }
    String::new()
}

fn collect_class<'a>(nodes: &'a [Node], class: &str, out: &mut Vec<&'a El>) {
    for node in nodes {
        if let Node::El(el) = node {
            if el.has_class(class) {
                out.push(el);
                // Nested notes/labels don't occur in Keep exports; don't descend into a match.
                continue;
            }
            collect_class(&el.children, class, out);
        }
    }
}

fn collect_tag<'a>(nodes: &'a [Node], tag: &str, out: &mut Vec<&'a El>) {
    for node in nodes {
        if let Node::El(el) = node {
            if el.tag == tag {
                out.push(el);
            }
            collect_tag(&el.children, tag, out);
        }
    }
}

fn text_of_el(el: &El) -> String {
    let mut out = String::new();
    push_text(&el.children, &mut out);
    out
}

fn push_text(nodes: &[Node], out: &mut String) {
    for node in nodes {
        match node {
            Node::Text(t) => out.push_str(&decode_entities(t)),
            Node::El(el) => {
                if el.tag == "br" {
                    out.push('\n');
                } else {
                    push_text(&el.children, out);
                }
            }
        }
    }
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] != '&' {
            out.push(chars[i]);
            i += 1;
            continue;
        }
        let end = (i + 1..chars.len().min(i + 12)).find(|&p| chars[p] == ';');
        let Some(end) = end else {
            out.push('&');
            i += 1;
            continue;
        };
        let name: String = chars[i + 1..end].iter().collect();
        let decoded = match name.as_str() {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" | "#39" => Some('\''),
            "nbsp" => Some(' '),
            _ => {
                if let Some(hex) = name.strip_prefix("#x").or_else(|| name.strip_prefix("#X")) {
                    u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
                } else if let Some(dec) = name.strip_prefix('#') {
                    dec.parse::<u32>().ok().and_then(char::from_u32)
                } else {
                    None
                }
            }
        };
        match decoded {
            Some(c) => {
                out.push(c);
                i = end + 1;
            }
            None => {
                out.push('&');
                i += 1;
            }
        }
    }
    out
}

fn strip_bullets(s: &str) -> String {
    s.trim()
        .trim_start_matches(['\u{2610}', '\u{2611}', '\u{2612}', '\u{2022}'])
        .trim()
        .to_string()
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// Parse the Keep HTML heading date, e.g. `Jan 15, 2026, 9:30:00 AM` → `2026-01-15T09:30:00Z`.
/// Keep writes local times without a zone; they are emitted verbatim with a `Z` suffix.
fn parse_us_datetime(s: &str) -> Option<String> {
    let cleaned: String = s
        .chars()
        .map(|c| if c == ',' { ' ' } else { c })
        .collect::<String>();
    let toks: Vec<&str> = cleaned.split_whitespace().collect();
    let mut month = None;
    let mut idx = 0;
    for (i, t) in toks.iter().enumerate() {
        let lower = t.to_ascii_lowercase();
        if lower.len() >= 3 {
            if let Some(m) = MONTHS.iter().position(|m| lower.starts_with(m)) {
                month = Some(m as u32 + 1);
                idx = i;
                break;
            }
        }
    }
    let month = month?;
    let day: u32 = toks.get(idx + 1)?.trim_matches('.').parse().ok()?;
    let year: i64 = toks.get(idx + 2)?.parse().ok()?;
    let (mut hh, mut mm, mut ss) = (0u32, 0u32, 0u32);
    if let Some(time) = toks.get(idx + 3) {
        let mut parts = time.split(':');
        hh = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        mm = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        ss = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        match toks.get(idx + 4).map(|m| m.to_ascii_uppercase()) {
            Some(m) if m.starts_with("PM") && hh < 12 => hh += 12,
            Some(m) if m.starts_with("AM") && hh == 12 => hh = 0,
            _ => {}
        }
    }
    if !(1..=31).contains(&day) || hh > 23 || mm > 59 || ss > 59 {
        return None;
    }
    Some(format!("{year:04}-{month:02}-{day:02}T{hh:02}:{mm:02}:{ss:02}Z"))
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_note(
    note: &Note,
    metadata: Metadata,
    checkbox_style: CheckboxStyle,
    link_attachments: bool,
) -> String {
    let mut out = String::new();

    if metadata == Metadata::Frontmatter {
        out.push_str("---\n");
        if !note.title.is_empty() {
            out.push_str(&format!("title: {}\n", yaml_string(&note.title)));
        }
        if let Some(c) = &note.created {
            out.push_str(&format!("created: {c}\n"));
        }
        if let Some(u) = &note.updated {
            out.push_str(&format!("updated: {u}\n"));
        }
        if !note.labels.is_empty() {
            let list: Vec<String> = note.labels.iter().map(|l| yaml_string(l)).collect();
            out.push_str(&format!("labels: [{}]\n", list.join(", ")));
        }
        if note.pinned {
            out.push_str("pinned: true\n");
        }
        if note.archived {
            out.push_str("archived: true\n");
        }
        if note.trashed {
            out.push_str("trashed: true\n");
        }
        if let Some(color) = &note.color {
            out.push_str(&format!("color: {color}\n"));
        }
        out.push_str("---\n\n");
    }

    if !note.title.is_empty() {
        out.push_str(&format!("# {}\n\n", note.title));
    }

    let mut body = String::new();
    if !note.text.trim().is_empty() {
        body.push_str(note.text.trim_end());
    }
    if !note.items.is_empty() {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        for item in &note.items {
            let line = match checkbox_style {
                CheckboxStyle::TaskList => {
                    format!("- [{}] {}", if item.checked { 'x' } else { ' ' }, item.text)
                }
                CheckboxStyle::Bullet => format!("- {}", item.text),
                CheckboxStyle::Plain => item.text.clone(),
            };
            body.push_str(&line);
            body.push('\n');
        }
        body = body.trim_end().to_string();
    }
    if link_attachments && !note.attachments.is_empty() {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        for att in &note.attachments {
            let bang = if att.is_image { "!" } else { "" };
            body.push_str(&format!("{bang}[{0}]({0})\n", att.name));
        }
        body = body.trim_end().to_string();
    }
    if !note.links.is_empty() {
        if !body.is_empty() {
            body.push_str("\n\n");
        }
        for (title, url) in &note.links {
            body.push_str(&format!("- [{title}]({url})\n"));
        }
        body = body.trim_end().to_string();
    }
    out.push_str(&body);

    if metadata == Metadata::Inline && !note.labels.is_empty() {
        if !out.trim_end().is_empty() {
            out = out.trim_end().to_string();
            out.push_str("\n\n");
        }
        let tags: Vec<String> = note.labels.iter().map(|l| format!("#{}", slug(l))).collect();
        out.push_str(&tags.join(" "));
    }

    out.trim_end().to_string()
}

fn yaml_string(s: &str) -> String {
    format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\""))
}

// ---------------------------------------------------------------------------
// Filenames
// ---------------------------------------------------------------------------

fn slug(s: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if c.is_alphanumeric() {
            // Keep non-ASCII letters/digits (accents, CJK) rather than mangling the title.
            out.push(c);
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    let mut capped: String = out.chars().take(MAX_SLUG_CHARS).collect();
    while capped.ends_with('-') {
        capped.pop();
    }
    capped
}

fn title_slug(note: &Note) -> String {
    let base = if !note.title.trim().is_empty() {
        note.title.clone()
    } else if let Some(line) = note.text.lines().find(|l| !l.trim().is_empty()) {
        line.to_string()
    } else if let Some(item) = note.items.first() {
        item.text.clone()
    } else {
        String::new()
    };
    let s = slug(&base);
    if s.is_empty() {
        "untitled".to_string()
    } else {
        s
    }
}

fn filename_for(note: &Note, style: FilenameStyle) -> String {
    let base = title_slug(note);
    match style {
        FilenameStyle::Title => format!("{base}.md"),
        FilenameStyle::DateTitle => match note.created_date() {
            Some(date) => format!("{date}-{base}.md"),
            None => format!("{base}.md"),
        },
        FilenameStyle::LabelTitle => {
            let folder = note
                .labels
                .first()
                .map(|l| slug(l))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "unlabeled".to_string());
            format!("{folder}/{base}.md")
        }
    }
}

fn unique_name(name: &str, used: &mut Vec<String>) -> String {
    if !used.iter().any(|u| u == name) {
        used.push(name.to_string());
        return name.to_string();
    }
    let (stem, ext) = match name.rsplit_once(".md") {
        Some((stem, _)) => (stem, ".md"),
        None => (name, ""),
    };
    for n in 1..10_000 {
        let candidate = format!("{stem}-{n}{ext}");
        if !used.iter().any(|u| *u == candidate) {
            used.push(candidate.clone());
            return candidate;
        }
    }
    name.to_string()
}

fn file_name(path: &str) -> String {
    path.rsplit(['/', '\\']).next().unwrap_or(path).to_string()
}

fn looks_like_image(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    [".jpg", ".jpeg", ".png", ".gif", ".webp", ".heic", ".bmp"]
        .iter()
        .any(|ext| lower.ends_with(ext))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE_JSON: &str = r#"{
        "color": "BLUE",
        "isTrashed": false,
        "isPinned": true,
        "isArchived": false,
        "title": "Grocery List",
        "labels": [{ "name": "Shopping" }, { "name": "Home" }],
        "createdTimestampUsec": 1768469400000000,
        "userEditedTimestampUsec": 1768557600000000,
        "listContent": [
            { "text": "Milk", "isChecked": false },
            { "text": "Eggs", "isChecked": true }
        ]
    }"#;

    fn convert_default(input: &str) -> Result<String, String> {
        convert(input, "", "", "", true, false, true)
    }

    #[test]
    fn happy_path_json_note_with_frontmatter_and_checkboxes() {
        let out = convert_default(NOTE_JSON).unwrap();
        assert_eq!(
            out,
            "==== 2026-01-15-grocery-list.md ====\n\
             ---\n\
             title: \"Grocery List\"\n\
             created: 2026-01-15T09:30:00Z\n\
             updated: 2026-01-16T10:00:00Z\n\
             labels: [\"Shopping\", \"Home\"]\n\
             pinned: true\n\
             color: BLUE\n\
             ---\n\n\
             # Grocery List\n\n\
             - [ ] Milk\n\
             - [x] Eggs"
        );
    }

    #[test]
    fn error_on_empty_input() {
        let err = convert_default("   ").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn error_on_broken_json() {
        let err = convert_default("{ \"title\": ").unwrap_err();
        assert!(err.contains("could not be parsed"), "{err}");
    }

    #[test]
    fn error_on_unknown_enum_value() {
        let err = convert(NOTE_JSON, "yaml", "", "", true, false, true).unwrap_err();
        assert_eq!(
            err,
            "metadata must be one of frontmatter, inline, none (got 'yaml')"
        );
    }

    #[test]
    fn error_when_every_note_is_filtered_out() {
        let input = r#"[{ "title": "Old", "textContent": "gone", "isTrashed": true }]"#;
        let err = convert(input, "", "", "", true, false, true).unwrap_err();
        assert!(err.contains("filtered out"), "{err}");
    }

    #[test]
    fn inline_metadata_writes_hashtags_and_no_frontmatter() {
        let out = convert(NOTE_JSON, "inline", "title", "bullet", true, false, true).unwrap();
        assert_eq!(
            out,
            "==== grocery-list.md ====\n# Grocery List\n\n- Milk\n- Eggs\n\n#shopping #home"
        );
    }

    #[test]
    fn metadata_none_and_plain_checkboxes() {
        let out = convert(NOTE_JSON, "none", "title", "plain", true, false, true).unwrap();
        assert_eq!(out, "==== grocery-list.md ====\n# Grocery List\n\nMilk\nEggs");
    }

    #[test]
    fn label_title_filenames_use_the_first_label_as_a_folder() {
        let out = convert(NOTE_JSON, "none", "label-title", "", true, false, true).unwrap();
        assert!(out.starts_with("==== shopping/grocery-list.md ===="), "{out}");
    }

    #[test]
    fn unlabeled_notes_get_an_unlabeled_folder() {
        let input = r#"{ "title": "Solo", "textContent": "body" }"#;
        let out = convert(input, "none", "label-title", "", true, false, true).unwrap();
        assert_eq!(out, "==== unlabeled/solo.md ====\n# Solo\n\nbody");
    }

    #[test]
    fn archived_notes_are_included_by_default_and_excludable() {
        let input = r#"[
            { "title": "Live", "textContent": "a" },
            { "title": "Old", "textContent": "b", "isArchived": true }
        ]"#;
        let all = convert(input, "none", "title", "", true, false, true).unwrap();
        assert!(all.contains("==== old.md ===="), "{all}");
        let live = convert(input, "none", "title", "", false, false, true).unwrap();
        assert!(!live.contains("==== old.md ===="), "{live}");
        assert!(live.contains("==== live.md ===="), "{live}");
    }

    #[test]
    fn trashed_notes_are_excluded_unless_requested() {
        let input = r#"[
            { "title": "Live", "textContent": "a" },
            { "title": "Bin", "textContent": "b", "isTrashed": true }
        ]"#;
        let out = convert(input, "none", "title", "", true, false, true).unwrap();
        assert!(!out.contains("==== bin.md ===="), "{out}");
        let with_trash = convert(input, "frontmatter", "title", "", true, true, true).unwrap();
        assert!(with_trash.contains("trashed: true"), "{with_trash}");
    }

    #[test]
    fn attachments_render_as_markdown_links_and_can_be_turned_off() {
        let input = r#"{
            "title": "Trip",
            "textContent": "photos",
            "attachments": [
                { "filePath": "1650000000.jpg", "mimetype": "image/jpeg" },
                { "filePath": "voice.3gp", "mimetype": "audio/3gpp" }
            ]
        }"#;
        let on = convert(input, "none", "title", "", true, false, true).unwrap();
        assert!(on.contains("![1650000000.jpg](1650000000.jpg)"), "{on}");
        assert!(on.contains("[voice.3gp](voice.3gp)"), "{on}");
        assert!(!on.contains("![voice.3gp]"), "{on}");
        let off = convert(input, "none", "title", "", true, false, false).unwrap();
        assert!(!off.contains("1650000000.jpg"), "{off}");
    }

    #[test]
    fn weblink_annotations_become_markdown_links() {
        let input = r#"{
            "title": "Reading",
            "textContent": "later",
            "annotations": [{ "source": "WEBLINK", "title": "Rust book", "url": "https://doc.rust-lang.org/book/" }]
        }"#;
        let out = convert(input, "none", "title", "", true, false, true).unwrap();
        assert!(
            out.contains("- [Rust book](https://doc.rust-lang.org/book/)"),
            "{out}"
        );
    }

    #[test]
    fn duplicate_titles_get_numeric_suffixes() {
        let input = r#"[
            { "title": "Ideas", "textContent": "one" },
            { "title": "Ideas", "textContent": "two" }
        ]"#;
        let out = convert(input, "none", "title", "", true, false, true).unwrap();
        assert!(out.contains("==== ideas.md ===="), "{out}");
        assert!(out.contains("==== ideas-1.md ===="), "{out}");
    }

    #[test]
    fn untitled_notes_fall_back_to_the_first_body_line() {
        let input = r#"{ "textContent": "Call the plumber\nnumber is on the fridge" }"#;
        let out = convert(input, "none", "title", "", true, false, true).unwrap();
        assert_eq!(
            out,
            "==== call-the-plumber.md ====\nCall the plumber\nnumber is on the fridge"
        );
    }

    #[test]
    fn html_export_is_detected_and_converted() {
        let html = r#"<!DOCTYPE html><html><head><title>Grocery List</title></head><body>
            <div class="note">
              <div class="heading">Jan 15, 2026, 9:30:00 AM</div>
              <div class="title">Grocery List</div>
              <div class="content">
                <div class="listitem"><span class="bullet">&#9744;</span><span class="text">Milk</span></div>
                <div class="listitem checked"><span class="bullet">&#9745;</span><span class="text">Eggs &amp; bread</span></div>
              </div>
              <div class="chips"><span class="label"><span class="label-name">Shopping</span></span></div>
            </div></body></html>"#;
        let out = convert(html, "frontmatter", "date-title", "task-list", true, false, true).unwrap();
        assert_eq!(
            out,
            "==== 2026-01-15-grocery-list.md ====\n\
             ---\n\
             title: \"Grocery List\"\n\
             created: 2026-01-15T09:30:00Z\n\
             labels: [\"Shopping\"]\n\
             ---\n\n\
             # Grocery List\n\n\
             - [ ] Milk\n\
             - [x] Eggs & bread"
        );
    }

    #[test]
    fn html_plain_note_keeps_line_breaks_and_archived_chip() {
        let html = r#"<html><body><div class="note">
            <div class="heading">Feb 2, 2026, 8:00:00 PM</div>
            <div class="title">Packing</div>
            <div class="content">socks<br>charger</div>
            <div class="chips"><span class="archived">Archived</span></div>
        </div></body></html>"#;
        let out = convert(html, "frontmatter", "title", "", true, false, true).unwrap();
        assert!(out.contains("created: 2026-02-02T20:00:00Z"), "{out}");
        assert!(out.contains("archived: true"), "{out}");
        assert!(out.contains("socks\ncharger"), "{out}");
        let hidden = convert(html, "none", "title", "", false, false, true).unwrap_err();
        assert!(hidden.contains("filtered out"), "{hidden}");
    }

    #[test]
    fn error_on_html_without_notes() {
        let err = convert_default("<html><body></body></html>").unwrap_err();
        assert!(err.contains("no Keep notes found"), "{err}");
    }

    #[test]
    fn error_on_non_json_non_html_input() {
        let err = convert_default("just some prose").unwrap_err();
        assert!(err.contains("unrecognised input"), "{err}");
    }

    #[test]
    fn error_on_oversized_input() {
        let big = format!("{{\"textContent\":\"{}\"}}", "x".repeat(MAX_INPUT_BYTES));
        let err = convert_default(&big).unwrap_err();
        assert!(err.contains("the limit is"), "{err}");
    }

    #[test]
    fn epoch_conversion_matches_known_instants() {
        assert_eq!(iso_from_secs(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_from_secs(1_000_000_000), "2001-09-09T01:46:40Z");
        assert_eq!(iso_from_secs(1_768_469_400), "2026-01-15T09:30:00Z");
    }
}
