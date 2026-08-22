//! music-file-renamer core — turn a batch of music tag records into a
//! deterministic rename/move PLAN. Pure compute, shared by the chat skill block,
//! the CLI and the web page. No wafer/wasm-bindgen deps, no filesystem access.
//!
//! Input is a tag dump — whatever a tagger, `ffprobe`, `exiftool` or a library
//! manager already exported: delimited text with a header row, a JSON array of
//! objects, or `key=value`/`key: value` blocks separated by blank lines. Each
//! record must carry the file's current path plus whatever tags it has.
//!
//! Output is `current path -> new path` for every record. The tool NEVER touches
//! files: it computes names only, so the plan is safe to review, share as a URL,
//! or emit as a shell script you run yourself.
//!
//! Not to be confused with `blocks/bulk-file-renamer`, which transforms filename
//! STRINGS (find/replace, regex, numbering) and never looks at metadata.

use std::collections::HashMap;

/// Hard cap on records per run — a plan past this is unreadable and a pasted
/// library dump that large belongs in a script, not a preview.
pub const MAX_TRACKS: usize = 5000;

/// How the pasted tag dump is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Sniff the shape from the text itself.
    Auto,
    /// Comma-separated with a header row (RFC 4180 quoting).
    Csv,
    /// Tab-separated with a header row.
    Tsv,
    /// A JSON array of objects (or one object, or `{"tracks": [...]}`).
    Json,
    /// `key=value` / `key: value` lines, one blank-line-separated block per file.
    KeyValue,
}

impl InputFormat {
    pub fn parse(s: &str) -> Result<InputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" | "detect" => Ok(InputFormat::Auto),
            "csv" => Ok(InputFormat::Csv),
            "tsv" | "tab" => Ok(InputFormat::Tsv),
            "json" => Ok(InputFormat::Json),
            "keyvalue" | "key_value" | "key-value" | "kv" => Ok(InputFormat::KeyValue),
            other => Err(format!(
                "unknown input_format '{other}' (use auto, csv, tsv, json or keyvalue)"
            )),
        }
    }
}

/// What to do with a record whose pattern references a tag it does not have.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnMissing {
    /// Substitute `unknown_text` and keep the record in the plan.
    Unknown,
    /// Leave the record out of the plan and list it under "skipped".
    Skip,
    /// Keep the file exactly where it is (new path == current path).
    KeepOriginal,
}

impl OnMissing {
    pub fn parse(s: &str) -> Result<OnMissing, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "unknown" | "placeholder" => Ok(OnMissing::Unknown),
            "skip" => Ok(OnMissing::Skip),
            "keep_original" | "keep-original" | "keep" => Ok(OnMissing::KeepOriginal),
            other => Err(format!(
                "unknown on_missing '{other}' (use unknown, skip or keep_original)"
            )),
        }
    }
}

/// Which characters the target filesystem tolerates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Charset {
    /// Windows/NTFS/exFAT-safe: no `< > : " / \ | ? *`, no control characters, no
    /// trailing dots or spaces, reserved device names defused. Safe everywhere.
    Windows,
    /// Unix/APFS/ext4: only the path separator and NUL are illegal.
    Unix,
    /// Windows rules plus accent folding — the result is plain 7-bit ASCII.
    Ascii,
}

impl Charset {
    pub fn parse(s: &str) -> Result<Charset, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "windows" | "win" => Ok(Charset::Windows),
            "unix" | "linux" | "mac" | "macos" | "posix" => Ok(Charset::Unix),
            "ascii" => Ok(Charset::Ascii),
            other => Err(format!(
                "unknown charset '{other}' (use windows, unix or ascii)"
            )),
        }
    }
}

/// What happens to spaces in the generated names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpaceStyle {
    Keep,
    Underscore,
    Hyphen,
}

impl SpaceStyle {
    pub fn parse(s: &str) -> Result<SpaceStyle, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" | "space" => Ok(SpaceStyle::Keep),
            "underscore" | "underscores" | "_" => Ok(SpaceStyle::Underscore),
            "hyphen" | "dash" | "-" => Ok(SpaceStyle::Hyphen),
            other => Err(format!(
                "unknown space_style '{other}' (use keep, underscore or hyphen)"
            )),
        }
    }
}

/// Letter case applied to every generated path component.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseStyle {
    Keep,
    Lower,
    Upper,
    Title,
}

impl CaseStyle {
    pub fn parse(s: &str) -> Result<CaseStyle, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "keep" | "asis" | "as-is" => Ok(CaseStyle::Keep),
            "lower" | "lowercase" => Ok(CaseStyle::Lower),
            "upper" | "uppercase" => Ok(CaseStyle::Upper),
            "title" | "titlecase" => Ok(CaseStyle::Title),
            other => Err(format!(
                "unknown case_style '{other}' (use keep, lower, upper or title)"
            )),
        }
    }
}

/// Shape of the rendered plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Summary line plus aligned `current  ->  new` rows.
    Table,
    /// Bare `current -> new`, one per line.
    List,
    /// `current_path,new_path,status` with a header row.
    Csv,
    /// Structured plan with per-entry status and counts.
    Json,
    /// A reviewable `/bin/sh` script of `mkdir -p` + `mv -n`.
    Sh,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<OutputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "table" => Ok(OutputFormat::Table),
            "list" | "plain" => Ok(OutputFormat::List),
            "csv" => Ok(OutputFormat::Csv),
            "json" => Ok(OutputFormat::Json),
            "sh" | "shell" | "bash" | "script" => Ok(OutputFormat::Sh),
            other => Err(format!(
                "unknown format '{other}' (use table, list, csv, json or sh)"
            )),
        }
    }
}

/// Everything the planner needs beyond the tag records themselves.
#[derive(Debug, Clone)]
pub struct Options {
    pub input_format: InputFormat,
    pub pattern: String,
    pub base_dir: String,
    pub track_padding: usize,
    pub on_missing: OnMissing,
    pub unknown_text: String,
    pub charset: Charset,
    pub replace_char: String,
    pub space_style: SpaceStyle,
    pub case_style: CaseStyle,
    pub max_component: usize,
    pub keep_extension: bool,
    pub format: OutputFormat,
}

/// The stock library layout: one folder per artist, one per album, zero-padded
/// track number in front of the title.
pub const DEFAULT_PATTERN: &str = "{artist}/{album}/{track} {title}";

impl Default for Options {
    fn default() -> Self {
        Options {
            input_format: InputFormat::Auto,
            pattern: DEFAULT_PATTERN.to_string(),
            base_dir: String::new(),
            track_padding: 2,
            on_missing: OnMissing::Unknown,
            unknown_text: "Unknown".to_string(),
            charset: Charset::Windows,
            replace_char: "_".to_string(),
            space_style: SpaceStyle::Keep,
            case_style: CaseStyle::Keep,
            max_component: 100,
            keep_extension: true,
            format: OutputFormat::Table,
        }
    }
}

/// What the plan says about one record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// The file moves.
    Rename,
    /// The file is already where the pattern wants it.
    Unchanged,
    /// Another record in this plan lands on the same target path.
    Collision,
}

impl Status {
    pub fn as_str(self) -> &'static str {
        match self {
            Status::Rename => "rename",
            Status::Unchanged => "unchanged",
            Status::Collision => "collision",
        }
    }
}

/// One planned move.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub current: String,
    pub new: String,
    pub status: Status,
}

/// One record left out of the plan (only produced by `on_missing = skip`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub current: String,
    pub reason: String,
}

/// The full plan plus its tallies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    pub entries: Vec<Entry>,
    pub skipped: Vec<Skipped>,
    pub renames: usize,
    pub unchanged: usize,
    pub collisions: usize,
}

impl Plan {
    /// Records seen, planned + skipped.
    pub fn total(&self) -> usize {
        self.entries.len() + self.skipped.len()
    }
}

// ---------------------------------------------------------------------------
// Tag-field canonicalisation
// ---------------------------------------------------------------------------

/// Strip a field name down to its comparable core: lowercase, letters + digits
/// only. `Album Artist`, `album_artist`, `ALBUMARTIST` and `TAG:albumartist` all
/// collapse to `albumartist`.
fn compact_key(raw: &str) -> String {
    let mut s = raw.trim();
    for prefix in ["tag:", "TAG:", "format.tags.", "tags."] {
        if let Some(rest) = s.strip_prefix(prefix) {
            s = rest.trim();
        }
    }
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Map a compacted field name onto the canonical token name, when it is one of
/// the tags every tagger writes under a different spelling.
fn canonical_field(compact: &str) -> Option<&'static str> {
    Some(match compact {
        "path" | "file" | "filename" | "filepath" | "fullpath" | "sourcefile" | "currentpath"
        | "currentfile" | "currentname" | "oldpath" | "oldname" | "location" | "directory"
        | "source" => "path",
        "artist" | "artists" | "performer" | "tpe1" | "trackartist" | "leadperformer" => "artist",
        "albumartist" | "band" | "tpe2" | "ensemble" | "albumperformer" => "albumartist",
        "album" | "talb" | "albumtitle" => "album",
        "title" | "tit2" | "tracktitle" | "songtitle" | "song" => "title",
        "track" | "tracknumber" | "trackno" | "tracknum" | "trck" | "trackn" => "track",
        "disc" | "disk" | "discnumber" | "disknumber" | "discno" | "diskno" | "tpos"
        | "partofset" => "disc",
        "year" | "date" | "originaldate" | "originalyear" | "releasedate" | "tyer" | "tdrc" => {
            "year"
        }
        "genre" | "tcon" => "genre",
        "composer" | "tcom" => "composer",
        "comment" | "comm" | "description" => "comment",
        "ext" | "extension" | "fileextension" => "ext",
        _ => return None,
    })
}

/// One parsed record: canonical tags plus every other field it carried, keyed by
/// its compacted name so `{bitrate}` or `{isrc}` work without being enumerated.
type Record = HashMap<String, String>;

fn record_insert(rec: &mut Record, raw_key: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    let compact = compact_key(raw_key);
    if compact.is_empty() {
        return;
    }
    let canon = canonical_field(&compact);
    // The literal spelling always wins over an alias, and the first non-empty
    // value wins over later ones (so `date` never clobbers an explicit `year`).
    let key = canon.unwrap_or(compact.as_str()).to_string();
    let exact = canon == Some(compact.as_str());
    match rec.get(&key) {
        Some(existing) if !existing.is_empty() && !exact => {}
        _ => {
            rec.insert(key, value.to_string());
        }
    }
    if canon.is_some() && canon != Some(compact.as_str()) {
        // Also expose the original spelling, so `{tpe1}` resolves too.
        rec.entry(compact).or_insert_with(|| value.to_string());
    }
}

// ---------------------------------------------------------------------------
// Input parsing
// ---------------------------------------------------------------------------

/// Split a delimited document into rows of fields, honouring RFC 4180 quoting
/// (doubled quotes escape, quoted fields may contain the delimiter and newlines).
fn parse_delimited(text: &str, delim: char) -> Vec<Vec<String>> {
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = text.chars().peekable();
    let mut started = false;
    while let Some(c) = chars.next() {
        started = true;
        if in_quotes {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    in_quotes = false;
                }
            } else {
                field.push(c);
            }
        } else if c == '"' && field.trim().is_empty() {
            field.clear();
            in_quotes = true;
        } else if c == delim {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(c);
        }
    }
    if started && (!field.is_empty() || !row.is_empty()) {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    rows
}

/// Count delimiter candidates outside quotes on the first physical line.
fn sniff_delimiter(text: &str) -> char {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut in_quotes = false;
    let (mut tab, mut comma, mut semi, mut pipe) = (0usize, 0usize, 0usize, 0usize);
    for c in line.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            _ if in_quotes => {}
            '\t' => tab += 1,
            ',' => comma += 1,
            ';' => semi += 1,
            '|' => pipe += 1,
            _ => {}
        }
    }
    let best = [(tab, '\t'), (comma, ','), (semi, ';'), (pipe, '|')]
        .into_iter()
        .filter(|(n, _)| *n > 0)
        .max_by_key(|(n, _)| *n);
    best.map(|(_, c)| c).unwrap_or(',')
}

/// Does this line read as `key = value` / `key: value` rather than a delimited row?
fn looks_key_value(line: &str) -> bool {
    let kv = line.find(['=', ':']);
    let sep = line.find(['\t', ',', ';']);
    match (kv, sep) {
        (Some(k), Some(s)) => k > 0 && k < s,
        (Some(k), None) => k > 0,
        _ => false,
    }
}

fn detect_format(text: &str) -> InputFormat {
    let t = text.trim_start();
    if t.starts_with('[') || t.starts_with('{') {
        return InputFormat::Json;
    }
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    if first.trim_start().starts_with("========") || looks_key_value(first) {
        return InputFormat::KeyValue;
    }
    if sniff_delimiter(text) == '\t' {
        InputFormat::Tsv
    } else {
        InputFormat::Csv
    }
}

fn parse_key_value(text: &str) -> Vec<Record> {
    let mut out: Vec<Record> = Vec::new();
    let mut cur = Record::new();
    let flush = |cur: &mut Record, out: &mut Vec<Record>| {
        if !cur.is_empty() {
            out.push(std::mem::take(cur));
        }
    };
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            flush(&mut cur, &mut out);
            continue;
        }
        // `exiftool -a` separates files with `======== path/to/file.mp3`.
        if let Some(rest) = line.strip_prefix("========") {
            flush(&mut cur, &mut out);
            let name = rest.trim().trim_start_matches('=').trim();
            if !name.is_empty() {
                record_insert(&mut cur, "path", name);
            }
            continue;
        }
        // `ffprobe`'s section markers start a new file block.
        if line.starts_with('[') {
            let upper = line.to_ascii_uppercase();
            if upper.starts_with("[FORMAT]") || upper.starts_with("[STREAM]") {
                flush(&mut cur, &mut out);
            }
            continue;
        }
        if line.starts_with('#') || line.starts_with(';') {
            continue;
        }
        let eq = line.find('=');
        let colon = line.find(':');
        let idx = match (eq, colon) {
            // ffprobe/exiftool tag keys commonly contain a namespacing colon
            // (`TAG:artist=...`); prefer `=` when present so the whole key is
            // canonicalised instead of treating `artist=...` as the value.
            (Some(e), _) => e,
            (None, Some(c)) => c,
            (None, None) => continue,
        };
        let (key, value) = line.split_at(idx);
        let value = &value[1..];
        record_insert(&mut cur, key, value);
    }
    flush(&mut cur, &mut out);
    out
}

fn json_scalar(v: &serde_json::Value) -> Option<String> {
    match v {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(b) => Some(b.to_string()),
        serde_json::Value::Array(a) => {
            let parts: Vec<String> = a.iter().filter_map(json_scalar).collect();
            (!parts.is_empty()).then(|| parts.join("; "))
        }
        _ => None,
    }
}

/// Flatten one JSON object into a record, descending into the wrapper objects
/// that real tag dumps use (`format`, `tags`, `common`, `metadata`).
fn json_object_into(obj: &serde_json::Map<String, serde_json::Value>, rec: &mut Record) {
    for (k, v) in obj {
        if let Some(nested) = v.as_object() {
            let compact = compact_key(k);
            if matches!(
                compact.as_str(),
                "format" | "tags" | "tag" | "common" | "metadata" | "meta" | "native"
            ) {
                json_object_into(nested, rec);
            }
            continue;
        }
        if let Some(s) = json_scalar(v) {
            record_insert(rec, k, &s);
        }
    }
}

fn parse_json(text: &str) -> Result<Vec<Record>, String> {
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|e| format!("input is not valid JSON: {e}. Pick a different input_format if the paste is CSV or key=value text."))?;
    let items: Vec<&serde_json::Value> = match &value {
        serde_json::Value::Array(a) => a.iter().collect(),
        serde_json::Value::Object(o) => {
            let wrapped = ["tracks", "files", "items", "songs", "entries", "results"]
                .iter()
                .find_map(|k| o.get(*k).and_then(|v| v.as_array()));
            match wrapped {
                Some(a) => a.iter().collect(),
                None => vec![&value],
            }
        }
        _ => return Err("JSON input must be an array of track objects".to_string()),
    };
    let mut out = Vec::with_capacity(items.len());
    for item in items {
        let obj = item
            .as_object()
            .ok_or_else(|| "every JSON entry must be an object of tag fields".to_string())?;
        let mut rec = Record::new();
        json_object_into(obj, &mut rec);
        if !rec.is_empty() {
            out.push(rec);
        }
    }
    Ok(out)
}

fn parse_records(text: &str, format: InputFormat) -> Result<Vec<Record>, String> {
    let format = match format {
        InputFormat::Auto => detect_format(text),
        other => other,
    };
    match format {
        InputFormat::Json => parse_json(text),
        InputFormat::KeyValue => Ok(parse_key_value(text)),
        InputFormat::Csv | InputFormat::Tsv | InputFormat::Auto => {
            let delim = match format {
                InputFormat::Tsv => '\t',
                _ => {
                    let d = sniff_delimiter(text);
                    if d == '\t' {
                        ','
                    } else {
                        d
                    }
                }
            };
            let rows = parse_delimited(text, delim);
            let mut iter = rows.into_iter();
            let header = iter
                .next()
                .ok_or_else(|| "no header row found in the delimited input".to_string())?;
            let mut out = Vec::new();
            for row in iter {
                let mut rec = Record::new();
                for (i, cell) in row.iter().enumerate() {
                    if let Some(name) = header.get(i) {
                        record_insert(&mut rec, name, cell);
                    }
                }
                if !rec.is_empty() {
                    out.push(rec);
                }
            }
            Ok(out)
        }
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn basename(path: &str) -> &str {
    let cut = path.rfind(['/', '\\']).map(|i| i + 1).unwrap_or(0);
    &path[cut..]
}

fn dirname(path: &str) -> &str {
    match path.rfind(['/', '\\']) {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Extension of a filename, without the dot. A leading dot (`.hidden`) is not an
/// extension, and anything longer than 10 characters is treated as part of the name.
fn extension(name: &str) -> &str {
    match name.rfind('.') {
        Some(i) if i > 0 && i + 1 < name.len() && name.len() - i - 1 <= 10 => &name[i + 1..],
        _ => "",
    }
}

fn stem(name: &str) -> &str {
    let ext = extension(name);
    if ext.is_empty() {
        name
    } else {
        &name[..name.len() - ext.len() - 1]
    }
}

// ---------------------------------------------------------------------------
// Token resolution
// ---------------------------------------------------------------------------

/// Take the numeric part of `3`, `03`, `3/12` or `03 of 12` and zero-pad it.
/// Anything non-numeric is returned untouched.
fn normalize_number(raw: &str, padding: usize) -> String {
    let head = raw.split(['/', '\\']).next().unwrap_or(raw).trim();
    let head = head.split_whitespace().next().unwrap_or(head);
    let digits: String = head.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() || digits.len() != head.chars().count() {
        return raw.trim().to_string();
    }
    let n: u64 = digits.parse().unwrap_or(0);
    format!("{n:0padding$}")
}

/// `2015-06-11`, `2015/06`, `2015` → `2015`. Anything with no 4-digit run is
/// returned untouched.
fn normalize_year(raw: &str) -> String {
    let chars: Vec<char> = raw.chars().collect();
    let mut run = 0usize;
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            run += 1;
            if run == 4 {
                let start = i - 3;
                return chars[start..=i].iter().collect();
            }
        } else {
            run = 0;
        }
    }
    raw.trim().to_string()
}

/// Resolve one token name against a record. Returns `None` when the tag is absent
/// or empty, which is what drives `on_missing`.
fn resolve_token(name: &str, rec: &Record, current: &str, opts: &Options) -> Option<String> {
    let key = compact_key(name);
    if key.is_empty() {
        return None;
    }
    let base = basename(current);
    match key.as_str() {
        "ext" | "extension" => {
            let e = rec
                .get("ext")
                .map(|s| s.trim_start_matches('.').to_string())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| extension(base).to_string());
            return (!e.is_empty()).then_some(e);
        }
        "filename" => return (!base.is_empty()).then(|| base.to_string()),
        "stem" | "basename" => {
            let s = stem(base);
            return (!s.is_empty()).then(|| s.to_string());
        }
        "dir" | "folder" => {
            let d = dirname(current);
            return (!d.is_empty()).then(|| d.replace('\\', "/"));
        }
        _ => {}
    }
    let canon = canonical_field(&key).unwrap_or(key.as_str());
    let raw = rec
        .get(canon)
        .or_else(|| rec.get(&key))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())?;
    Some(match canon {
        "track" => normalize_number(raw, opts.track_padding),
        "disc" => normalize_number(raw, 1),
        "year" => normalize_year(raw),
        _ => raw.to_string(),
    })
}

/// Expand `{token}` / `{a|b|c}` placeholders. Returns the rendered string and the
/// names of the tokens that had no value anywhere in their fallback chain.
fn render_pattern(
    pattern: &str,
    rec: &Record,
    current: &str,
    opts: &Options,
) -> Result<(String, Vec<String>), String> {
    let mut out = String::with_capacity(pattern.len() + 32);
    let mut missing: Vec<String> = Vec::new();
    let mut chars = pattern.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                let mut token = String::new();
                let mut closed = false;
                for c in chars.by_ref() {
                    if c == '}' {
                        closed = true;
                        break;
                    }
                    token.push(c);
                }
                if !closed {
                    return Err(format!(
                        "unclosed {{ in pattern '{pattern}' — every {{token}} needs a closing brace"
                    ));
                }
                let alts: Vec<&str> = token.split('|').map(|a| a.trim()).collect();
                let mut hit = None;
                for alt in &alts {
                    if alt.is_empty() {
                        continue;
                    }
                    if let Some(v) = resolve_token(alt, rec, current, opts) {
                        hit = Some(v);
                        break;
                    }
                }
                match hit {
                    Some(v) => out.push_str(&v),
                    None => {
                        let label = alts.first().copied().unwrap_or("").trim().to_string();
                        if !label.is_empty() && !missing.contains(&label) {
                            missing.push(label);
                        }
                        out.push_str(&opts.unknown_text);
                    }
                }
            }
            '}' => {
                return Err(format!(
                "stray }} in pattern '{pattern}' — write {{token}} with a matching opening brace"
            ))
            }
            _ => out.push(c),
        }
    }
    Ok((out, missing))
}

// ---------------------------------------------------------------------------
// Sanitising
// ---------------------------------------------------------------------------

/// Latin-1 Supplement / Latin Extended-A folding, plus the punctuation taggers
/// pick up from web sources. `None` means "no ASCII equivalent".
fn fold_ascii(c: char) -> Option<&'static str> {
    Some(match c {
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => "A",
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'Æ' => "AE",
        'æ' => "ae",
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'Ð' | 'Ď' | 'Đ' => "D",
        'ð' | 'ď' | 'đ' => "d",
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'Ĝ' | 'Ğ' | 'Ġ' | 'Ģ' => "G",
        'ĝ' | 'ğ' | 'ġ' | 'ģ' => "g",
        'Ĥ' | 'Ħ' => "H",
        'ĥ' | 'ħ' => "h",
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ĩ' | 'Ī' | 'Ĭ' | 'Į' | 'İ' => "I",
        'ì' | 'í' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'Ĵ' => "J",
        'ĵ' => "j",
        'Ķ' => "K",
        'ķ' => "k",
        'Ĺ' | 'Ļ' | 'Ľ' | 'Ŀ' | 'Ł' => "L",
        'ĺ' | 'ļ' | 'ľ' | 'ŀ' | 'ł' => "l",
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "N",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => "O",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'Œ' => "OE",
        'œ' => "oe",
        'Ŕ' | 'Ŗ' | 'Ř' => "R",
        'ŕ' | 'ŗ' | 'ř' => "r",
        'Ś' | 'Ŝ' | 'Ş' | 'Š' => "S",
        'ś' | 'ŝ' | 'ş' | 'š' => "s",
        'ß' => "ss",
        'Ţ' | 'Ť' | 'Ŧ' => "T",
        'ţ' | 'ť' | 'ŧ' => "t",
        'Þ' => "TH",
        'þ' => "th",
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ũ' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'ù' | 'ú' | 'û' | 'ü' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'Ŵ' => "W",
        'ŵ' => "w",
        'Ý' | 'Ŷ' | 'Ÿ' => "Y",
        'ý' | 'ŷ' | 'ÿ' => "y",
        'Ź' | 'Ż' | 'Ž' => "Z",
        'ź' | 'ż' | 'ž' => "z",
        '\u{2018}' | '\u{2019}' | '\u{02bc}' => "'",
        '\u{201c}' | '\u{201d}' => "\"",
        '\u{2013}' | '\u{2014}' | '\u{2212}' => "-",
        '\u{2026}' => "...",
        '\u{00d7}' => "x",
        '\u{00a0}' | '\u{2007}' | '\u{202f}' => " ",
        // Combining diacritics left over from NFD text: drop them.
        c if ('\u{0300}'..='\u{036f}').contains(&c) => "",
        _ => return None,
    })
}

const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

fn apply_case(s: &str, style: CaseStyle) -> String {
    match style {
        CaseStyle::Keep => s.to_string(),
        CaseStyle::Lower => s.to_lowercase(),
        CaseStyle::Upper => s.to_uppercase(),
        CaseStyle::Title => {
            let mut out = String::with_capacity(s.len());
            let mut fresh = true;
            for c in s.chars() {
                if fresh && c.is_alphabetic() {
                    out.extend(c.to_uppercase());
                    fresh = false;
                } else {
                    out.extend(c.to_lowercase());
                }
                if !c.is_alphanumeric() && c != '\'' {
                    fresh = true;
                }
            }
            out
        }
    }
}

/// Truncate to `max` characters, then drop the trailing spaces/dots the cut may
/// have exposed (Windows silently strips those and then two names can collide).
fn truncate_component(s: &str, max: usize) -> String {
    let mut out: String = if s.chars().count() > max {
        s.chars().take(max).collect()
    } else {
        s.to_string()
    };
    while out.ends_with(' ') || out.ends_with('.') {
        out.pop();
    }
    out
}

/// Make one path component legal for the chosen filesystem.
fn sanitize_component(raw: &str, opts: &Options) -> String {
    let spaced = match opts.space_style {
        SpaceStyle::Keep => raw.to_string(),
        SpaceStyle::Underscore => raw.replace(' ', "_"),
        SpaceStyle::Hyphen => raw.replace(' ', "-"),
    };
    let cased = apply_case(&spaced, opts.case_style);

    let mut out = String::with_capacity(cased.len());
    for c in cased.chars() {
        let mapped: Option<String> = if opts.charset == Charset::Ascii && !c.is_ascii() {
            match fold_ascii(c) {
                Some(rep) => Some(rep.to_string()),
                None => None, // no ASCII equivalent → replacement char
            }
        } else if c.is_control() {
            None
        } else if c == '/' || c == '\\' {
            None
        } else if matches!(opts.charset, Charset::Windows | Charset::Ascii)
            && matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')
        {
            None
        } else {
            Some(c.to_string())
        };
        match mapped {
            Some(s) => out.push_str(&s),
            None => out.push_str(&opts.replace_char),
        }
    }

    let mut out = out.trim().to_string();
    if matches!(opts.charset, Charset::Windows | Charset::Ascii) {
        while out.ends_with('.') || out.ends_with(' ') {
            out.pop();
        }
        let head = stem(&out).to_ascii_uppercase();
        if WINDOWS_RESERVED.contains(&head.as_str()) {
            out.push('_');
        }
    }
    out = truncate_component(&out, opts.max_component);
    if out.is_empty() {
        out = truncate_component(opts.unknown_text.trim(), opts.max_component);
    }
    if out.is_empty() {
        out = "_".to_string();
    }
    out
}

// ---------------------------------------------------------------------------
// Planning
// ---------------------------------------------------------------------------

/// Build the rename/move plan. Never touches the filesystem.
pub fn plan(tracks: &str, opts: &Options) -> Result<Plan, String> {
    if opts.pattern.trim().is_empty() {
        return Err("pattern must not be empty — try {artist}/{album}/{track} {title}".to_string());
    }
    if tracks.trim().is_empty() {
        return Err(
            "no tracks to rename — paste a tag dump (CSV/TSV with a header row, JSON, or key=value blocks)"
                .to_string(),
        );
    }
    let records = parse_records(tracks, opts.input_format)?;
    if records.is_empty() {
        return Err(
            "no track records found — the input needs one row (or block) per file, each with its current file name"
                .to_string(),
        );
    }
    if records.len() > MAX_TRACKS {
        return Err(format!(
            "too many tracks: {} (max {MAX_TRACKS} per run)",
            records.len()
        ));
    }

    let base = opts
        .base_dir
        .trim()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string();

    let mut entries: Vec<Entry> = Vec::with_capacity(records.len());
    let mut skipped: Vec<Skipped> = Vec::new();

    for (i, rec) in records.iter().enumerate() {
        let current = rec
            .get("path")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                format!(
                    "record {} has no current file name — add a file, filename or path column/field",
                    i + 1
                )
            })?;

        let (rendered, missing) = render_pattern(&opts.pattern, rec, &current, opts)?;

        if !missing.is_empty() {
            match opts.on_missing {
                OnMissing::Skip => {
                    skipped.push(Skipped {
                        current,
                        reason: format!("missing tag: {}", missing.join(", ")),
                    });
                    continue;
                }
                OnMissing::KeepOriginal => {
                    entries.push(Entry {
                        new: current.clone(),
                        current,
                        status: Status::Unchanged,
                    });
                    continue;
                }
                OnMissing::Unknown => {}
            }
        }

        let normalized = rendered.replace('\\', "/");
        let mut comps: Vec<String> = normalized
            .split('/')
            .filter(|c| !c.trim().is_empty())
            .map(|c| sanitize_component(c, opts))
            .collect();
        if comps.is_empty() {
            comps.push(sanitize_component(opts.unknown_text.trim(), opts));
        }

        if opts.keep_extension {
            let ext = resolve_token("ext", rec, &current, opts).unwrap_or_default();
            if !ext.is_empty() {
                let last = comps.last_mut().expect("at least one component");
                let suffix = format!(".{ext}");
                if !last.to_lowercase().ends_with(&suffix.to_lowercase()) {
                    let room = opts.max_component.saturating_sub(suffix.chars().count());
                    *last = truncate_component(last, room.max(1));
                    last.push_str(&suffix);
                }
            }
        }

        let mut new = comps.join("/");
        if !base.is_empty() {
            new = format!("{base}/{new}");
        }

        entries.push(Entry {
            status: if new == current {
                Status::Unchanged
            } else {
                Status::Rename
            },
            current,
            new,
        });
    }

    // Collisions are detected case-insensitively: on Windows and macOS two names
    // that differ only in case are the same file.
    let mut seen: HashMap<String, usize> = HashMap::new();
    for e in &entries {
        *seen.entry(e.new.to_lowercase()).or_insert(0) += 1;
    }
    for e in entries.iter_mut() {
        if seen.get(&e.new.to_lowercase()).copied().unwrap_or(0) > 1 {
            e.status = Status::Collision;
        }
    }

    let renames = entries
        .iter()
        .filter(|e| e.status == Status::Rename)
        .count();
    let unchanged = entries
        .iter()
        .filter(|e| e.status == Status::Unchanged)
        .count();
    let collisions = entries
        .iter()
        .filter(|e| e.status == Status::Collision)
        .count();

    Ok(Plan {
        entries,
        skipped,
        renames,
        unchanged,
        collisions,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("{n} {one}")
    } else {
        format!("{n} {many}")
    }
}

fn summary_line(p: &Plan) -> String {
    format!(
        "{}, {}, {} unchanged, {}, {} skipped",
        plural(p.total(), "track", "tracks"),
        plural(p.renames, "rename", "renames"),
        p.unchanged,
        plural(p.collisions, "collision", "collisions"),
        p.skipped.len()
    )
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Render a plan in the requested shape.
pub fn render(p: &Plan, opts: &Options) -> String {
    match opts.format {
        OutputFormat::Table => {
            let width = p
                .entries
                .iter()
                .map(|e| e.current.chars().count())
                .max()
                .unwrap_or(0);
            let mut out = String::new();
            out.push_str(&summary_line(p));
            out.push_str("\n\n");
            for e in &p.entries {
                let pad = width.saturating_sub(e.current.chars().count());
                out.push_str(&e.current);
                out.push_str(&" ".repeat(pad));
                out.push_str("  ->  ");
                out.push_str(&e.new);
                match e.status {
                    Status::Rename => {}
                    Status::Unchanged => out.push_str("  [unchanged]"),
                    Status::Collision => out.push_str("  [collision]"),
                }
                out.push('\n');
            }
            if !p.skipped.is_empty() {
                out.push_str("\nSkipped:\n");
                for s in &p.skipped {
                    out.push_str(&format!("{}  ({})\n", s.current, s.reason));
                }
            }
            out
        }
        OutputFormat::List => {
            let mut out = String::new();
            for e in &p.entries {
                out.push_str(&format!("{} -> {}\n", e.current, e.new));
            }
            out
        }
        OutputFormat::Csv => {
            let mut out = String::from("current_path,new_path,status\n");
            for e in &p.entries {
                out.push_str(&format!(
                    "{},{},{}\n",
                    csv_cell(&e.current),
                    csv_cell(&e.new),
                    e.status.as_str()
                ));
            }
            for s in &p.skipped {
                out.push_str(&format!(
                    "{},,{}\n",
                    csv_cell(&s.current),
                    csv_cell(&format!("skipped: {}", s.reason))
                ));
            }
            out
        }
        OutputFormat::Json => {
            let entries: Vec<serde_json::Value> = p
                .entries
                .iter()
                .map(|e| {
                    serde_json::json!({
                        "current": e.current,
                        "new": e.new,
                        "status": e.status.as_str(),
                    })
                })
                .collect();
            let skipped: Vec<serde_json::Value> = p
                .skipped
                .iter()
                .map(|s| serde_json::json!({ "current": s.current, "reason": s.reason }))
                .collect();
            let doc = serde_json::json!({
                "pattern": opts.pattern,
                "count": p.total(),
                "renames": p.renames,
                "unchanged": p.unchanged,
                "collisions": p.collisions,
                "skipped_count": p.skipped.len(),
                "entries": entries,
                "skipped": skipped,
            });
            serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        }
        OutputFormat::Sh => {
            let mut out = String::from("#!/bin/sh\n");
            out.push_str(&format!("# music-file-renamer plan: {}\n", summary_line(p)));
            out.push_str("# Nothing has been executed. Review, then run from the folder holding the current paths.\n");
            out.push_str("set -eu\n\n");
            let mut dirs: Vec<String> = Vec::new();
            for e in &p.entries {
                if e.status != Status::Rename {
                    continue;
                }
                let d = dirname(&e.new);
                if !d.is_empty() && !dirs.contains(&d.to_string()) {
                    dirs.push(d.to_string());
                }
            }
            for d in &dirs {
                out.push_str(&format!("mkdir -p {}\n", sh_quote(d)));
            }
            if !dirs.is_empty() {
                out.push('\n');
            }
            for e in &p.entries {
                match e.status {
                    Status::Rename => {
                        out.push_str(&format!(
                            "mv -n {} {}\n",
                            sh_quote(&e.current),
                            sh_quote(&e.new)
                        ));
                    }
                    Status::Unchanged => {
                        out.push_str(&format!("# unchanged: {}\n", sh_quote(&e.current)));
                    }
                    Status::Collision => {
                        out.push_str(&format!(
                            "# COLLISION, not moved: {} would become {}\n",
                            sh_quote(&e.current),
                            sh_quote(&e.new)
                        ));
                    }
                }
            }
            for s in &p.skipped {
                out.push_str(&format!(
                    "# skipped ({}): {}\n",
                    s.reason,
                    sh_quote(&s.current)
                ));
            }
            out
        }
    }
}

/// String façade shared by the CLI/chat block and the browser page: parse every
/// option out of its textual form, plan, render.
#[allow(clippy::too_many_arguments)]
pub fn run(
    tracks: &str,
    input_format: &str,
    pattern: &str,
    base_dir: &str,
    track_padding: i64,
    on_missing: &str,
    unknown_text: &str,
    charset: &str,
    replace_char: &str,
    space_style: &str,
    case_style: &str,
    max_component: i64,
    keep_extension: bool,
    format: &str,
) -> Result<String, String> {
    if !(0..=6).contains(&track_padding) {
        return Err(format!(
            "track_padding must be between 0 and 6 (got {track_padding})"
        ));
    }
    if !(8..=255).contains(&max_component) {
        return Err(format!(
            "max_component must be between 8 and 255 (got {max_component})"
        ));
    }
    let pattern = if pattern.trim().is_empty() {
        DEFAULT_PATTERN.to_string()
    } else {
        pattern.to_string()
    };
    let unknown_text = if unknown_text.is_empty() {
        "Unknown".to_string()
    } else {
        unknown_text.to_string()
    };
    let opts = Options {
        input_format: InputFormat::parse(input_format)?,
        pattern,
        base_dir: base_dir.to_string(),
        track_padding: track_padding as usize,
        on_missing: OnMissing::parse(on_missing)?,
        unknown_text,
        charset: Charset::parse(charset)?,
        replace_char: replace_char.to_string(),
        space_style: SpaceStyle::parse(space_style)?,
        case_style: CaseStyle::parse(case_style)?,
        max_component: max_component as usize,
        keep_extension,
        format: OutputFormat::parse(format)?,
    };
    let p = plan(tracks, &opts)?;
    Ok(render(&p, &opts))
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: &str = "file,artist,album,track,title\n\
                       track01.mp3,Tame Impala,Currents,1,Let It Happen\n\
                       track02.mp3,Tame Impala,Currents,2,Nangs\n";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn csv_happy_path_builds_the_default_library_layout() {
        let mut o = opts();
        o.format = OutputFormat::List;
        let out = run(
            CSV,
            "auto",
            DEFAULT_PATTERN,
            "",
            2,
            "unknown",
            "Unknown",
            "windows",
            "_",
            "keep",
            "keep",
            100,
            true,
            "list",
        )
        .unwrap();
        assert_eq!(
            out,
            "track01.mp3 -> Tame Impala/Currents/01 Let It Happen.mp3\n\
             track02.mp3 -> Tame Impala/Currents/02 Nangs.mp3\n"
        );
        let p = plan(CSV, &o).unwrap();
        assert_eq!(p.renames, 2);
        assert_eq!(p.collisions, 0);
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = plan("   ", &opts()).unwrap_err();
        assert!(err.contains("no tracks to rename"), "{err}");
    }

    #[test]
    fn unclosed_brace_is_an_error() {
        let mut o = opts();
        o.pattern = "{artist}/{album".to_string();
        let err = plan(CSV, &o).unwrap_err();
        assert!(err.contains("unclosed"), "{err}");
    }

    #[test]
    fn missing_current_path_column_is_an_error() {
        let err = plan("artist,title\nTame Impala,Nangs\n", &opts()).unwrap_err();
        assert!(err.contains("no current file name"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        assert!(Charset::parse("ntfs").is_err());
        assert!(OutputFormat::parse("yaml").is_err());
        assert!(OnMissing::parse("nope").is_err());
        assert!(InputFormat::parse("xml").is_err());
        assert!(SpaceStyle::parse("dot").is_err());
        assert!(CaseStyle::parse("camel").is_err());
    }

    #[test]
    fn numeric_bounds_are_enforced() {
        let bad = run(
            CSV,
            "auto",
            DEFAULT_PATTERN,
            "",
            9,
            "unknown",
            "Unknown",
            "windows",
            "_",
            "keep",
            "keep",
            100,
            true,
            "list",
        )
        .unwrap_err();
        assert!(bad.contains("track_padding"), "{bad}");
        let bad = run(
            CSV,
            "auto",
            DEFAULT_PATTERN,
            "",
            2,
            "unknown",
            "Unknown",
            "windows",
            "_",
            "keep",
            "keep",
            4,
            true,
            "list",
        )
        .unwrap_err();
        assert!(bad.contains("max_component"), "{bad}");
    }

    #[test]
    fn tsv_and_semicolon_delimiters_are_sniffed() {
        let tsv = "file\tartist\ttitle\na.flac\tBoards of Canada\tOlson\n";
        let mut o = opts();
        o.pattern = "{artist} - {title}".into();
        o.format = OutputFormat::List;
        let out = render(&plan(tsv, &o).unwrap(), &o);
        assert_eq!(out, "a.flac -> Boards of Canada - Olson.flac\n");

        let semi = "file;artist;title\na.flac;Boards of Canada;Olson\n";
        let out = render(&plan(semi, &o).unwrap(), &o);
        assert_eq!(out, "a.flac -> Boards of Canada - Olson.flac\n");
    }

    #[test]
    fn json_array_and_nested_ffprobe_shape_both_parse() {
        let json = r#"[{"filename":"x.m4a","artist":"Air","album":"Moon Safari","track":"3","title":"All I Need"}]"#;
        let mut o = opts();
        o.format = OutputFormat::List;
        let out = render(&plan(json, &o).unwrap(), &o);
        assert_eq!(out, "x.m4a -> Air/Moon Safari/03 All I Need.m4a\n");

        let nested = r#"{"format":{"filename":"y.mp3","tags":{"artist":"Air","album":"Moon Safari","track":"3/10","title":"All I Need"}}}"#;
        let out = render(&plan(nested, &o).unwrap(), &o);
        assert_eq!(out, "y.mp3 -> Air/Moon Safari/03 All I Need.mp3\n");
    }

    #[test]
    fn key_value_blocks_and_exiftool_separators_parse() {
        let kv = "filename=a.mp3\nTAG:artist=Portishead\nTAG:title=Roads\n\nfilename=b.mp3\nTAG:artist=Portishead\nTAG:title=Glory Box\n";
        let mut o = opts();
        o.pattern = "{artist}/{title}".into();
        o.format = OutputFormat::List;
        let out = render(&plan(kv, &o).unwrap(), &o);
        assert_eq!(
            out,
            "a.mp3 -> Portishead/Roads.mp3\nb.mp3 -> Portishead/Glory Box.mp3\n"
        );

        let exif = "======== a.mp3\nArtist                          : Portishead\nTitle                           : Roads\n";
        let out = render(&plan(exif, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> Portishead/Roads.mp3\n");
    }

    #[test]
    fn fallback_chain_prefers_the_album_artist() {
        let csv = "file,artist,albumartist,album,title\n\
                   a.mp3,Guest Singer,Various Artists,Comp 1,Track A\n";
        let mut o = opts();
        o.pattern = "{albumartist|artist}/{album}/{title}".into();
        o.format = OutputFormat::List;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> Various Artists/Comp 1/Track A.mp3\n");
    }

    #[test]
    fn illegal_characters_are_replaced_per_charset() {
        let csv = "file,artist,title\na.mp3,AC/DC,Who Made Who?\n";
        let mut o = opts();
        o.pattern = "{artist}/{title}".into();
        o.format = OutputFormat::List;
        // The `/` inside the tag became a folder boundary, `?` became `_`.
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> AC/DC/Who Made Who_.mp3\n");

        o.charset = Charset::Unix;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> AC/DC/Who Made Who?.mp3\n");

        o.replace_char = String::new();
        o.charset = Charset::Windows;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> AC/DC/Who Made Who.mp3\n");
    }

    #[test]
    fn ascii_charset_folds_accents() {
        let csv = "file,artist,title\na.mp3,Sigur Rós,Ágætis byrjun\n";
        let mut o = opts();
        o.pattern = "{artist}/{title}".into();
        o.charset = Charset::Ascii;
        o.format = OutputFormat::List;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> Sigur Ros/Agaetis byrjun.mp3\n");
    }

    #[test]
    fn windows_reserved_names_and_trailing_dots_are_defused() {
        let csv = "file,artist,title\na.mp3,CON,Intro...\n";
        let mut o = opts();
        o.pattern = "{artist}/{title}".into();
        o.format = OutputFormat::List;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> CON_/Intro.mp3\n");
    }

    #[test]
    fn space_and_case_styles_apply_per_component() {
        let csv = "file,artist,title\na.mp3,Boards of Canada,ROYGBIV\n";
        let mut o = opts();
        o.pattern = "{artist}/{title}".into();
        o.space_style = SpaceStyle::Underscore;
        o.case_style = CaseStyle::Title;
        o.format = OutputFormat::List;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> Boards_Of_Canada/Roygbiv.mp3\n");
    }

    #[test]
    fn max_component_truncates_and_keeps_the_extension() {
        let csv = "file,artist,title\na.mp3,X,This Title Is Definitely Far Too Long For The Cap\n";
        let mut o = opts();
        o.pattern = "{title}".into();
        o.max_component = 20;
        o.format = OutputFormat::List;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> This Title Is De.mp3\n");
        assert_eq!("This Title Is De.mp3".len(), 20);
    }

    #[test]
    fn missing_tags_follow_the_on_missing_setting() {
        let csv = "file,artist,title\na.mp3,,Roads\n";
        let mut o = opts();
        o.pattern = "{artist}/{title}".into();
        o.format = OutputFormat::List;

        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> Unknown/Roads.mp3\n");

        o.unknown_text = "Unknown Artist".into();
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> Unknown Artist/Roads.mp3\n");

        o.on_missing = OnMissing::KeepOriginal;
        let p = plan(csv, &o).unwrap();
        assert_eq!(p.entries[0].new, "a.mp3");
        assert_eq!(p.unchanged, 1);

        o.on_missing = OnMissing::Skip;
        let p = plan(csv, &o).unwrap();
        assert!(p.entries.is_empty());
        assert_eq!(p.skipped.len(), 1);
        assert_eq!(p.skipped[0].reason, "missing tag: artist");
    }

    #[test]
    fn collisions_are_flagged_case_insensitively() {
        let csv = "file,artist,title\na.mp3,X,Same\nb.mp3,X,SAME\n";
        let mut o = opts();
        o.pattern = "{artist}/{title}".into();
        let p = plan(csv, &o).unwrap();
        assert_eq!(p.collisions, 2);
        assert_eq!(p.renames, 0);
        o.format = OutputFormat::Table;
        assert!(render(&p, &o).contains("[collision]"));
    }

    #[test]
    fn base_dir_prefixes_every_target() {
        let mut o = opts();
        o.base_dir = "/srv/music/".into();
        o.format = OutputFormat::List;
        let out = render(&plan(CSV, &o).unwrap(), &o);
        assert!(out
            .starts_with("track01.mp3 -> /srv/music/Tame Impala/Currents/01 Let It Happen.mp3\n"));
    }

    #[test]
    fn track_padding_and_disc_and_year_normalise() {
        let csv = "file,artist,album,track,disc,date,title\n\
                   a.mp3,X,Y,3/12,1/2,2015-06-11,T\n";
        let mut o = opts();
        o.pattern = "{year} {album}/{disc}-{track} {title}".into();
        o.track_padding = 3;
        o.format = OutputFormat::List;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "a.mp3 -> 2015 Y/1-003 T.mp3\n");
    }

    #[test]
    fn keep_extension_off_needs_the_ext_token() {
        let mut o = opts();
        o.pattern = "{title}".into();
        o.keep_extension = false;
        o.format = OutputFormat::List;
        let out = render(&plan(CSV, &o).unwrap(), &o);
        assert!(out.starts_with("track01.mp3 -> Let It Happen\n"), "{out}");

        o.pattern = "{title}.{ext}".into();
        let out = render(&plan(CSV, &o).unwrap(), &o);
        assert!(
            out.starts_with("track01.mp3 -> Let It Happen.mp3\n"),
            "{out}"
        );
    }

    #[test]
    fn path_tokens_come_from_the_current_file() {
        let csv = "file,artist,title\nInbox/2019/track01.mp3,X,T\n";
        let mut o = opts();
        o.pattern = "{dir}/{stem}-{title}".into();
        o.format = OutputFormat::List;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert_eq!(out, "Inbox/2019/track01.mp3 -> Inbox/2019/track01-T.mp3\n");
    }

    #[test]
    fn every_output_format_renders() {
        let mut o = opts();
        for f in [
            OutputFormat::Table,
            OutputFormat::List,
            OutputFormat::Csv,
            OutputFormat::Json,
            OutputFormat::Sh,
        ] {
            o.format = f;
            let out = render(&plan(CSV, &o).unwrap(), &o);
            assert!(out.contains("Let It Happen"), "{f:?} -> {out}");
        }
        o.format = OutputFormat::Table;
        assert!(render(&plan(CSV, &o).unwrap(), &o)
            .starts_with("2 tracks, 2 renames, 0 unchanged, 0 collisions, 0 skipped\n\n"));
        o.format = OutputFormat::Csv;
        assert!(render(&plan(CSV, &o).unwrap(), &o).starts_with("current_path,new_path,status\n"));
        o.format = OutputFormat::Sh;
        let sh = render(&plan(CSV, &o).unwrap(), &o);
        assert!(sh.starts_with("#!/bin/sh\n"));
        assert!(sh.contains("mkdir -p 'Tame Impala/Currents'"));
        assert!(sh.contains("mv -n 'track01.mp3' 'Tame Impala/Currents/01 Let It Happen.mp3'"));
    }

    #[test]
    fn json_output_is_structured() {
        let mut o = opts();
        o.format = OutputFormat::Json;
        let out = render(&plan(CSV, &o).unwrap(), &o);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["renames"], 2);
        assert_eq!(v["entries"][0]["status"], "rename");
        assert_eq!(
            v["entries"][0]["new"],
            "Tame Impala/Currents/01 Let It Happen.mp3"
        );
    }

    #[test]
    fn quoted_csv_cells_survive_the_round_trip() {
        let csv = "file,artist,title\n\"a, b.mp3\",\"Earth, Wind & Fire\",\"Say \"\"Yes\"\"\"\n";
        let mut o = opts();
        o.pattern = "{artist}/{title}".into();
        o.format = OutputFormat::Csv;
        let out = render(&plan(csv, &o).unwrap(), &o);
        assert!(out.contains("\"a, b.mp3\""), "{out}");
        assert!(
            out.contains("\"Earth, Wind & Fire/Say _Yes_.mp3\""),
            "{out}"
        );
    }

    #[test]
    fn max_tracks_boundary_is_exact() {
        let mut ok = String::from("file,artist,title\n");
        for i in 0..MAX_TRACKS {
            ok.push_str(&format!("f{i}.mp3,X,T{i}\n"));
        }
        let mut o = opts();
        o.pattern = "{title}".into();
        assert_eq!(plan(&ok, &o).unwrap().entries.len(), MAX_TRACKS);

        ok.push_str("one-too-many.mp3,X,T\n");
        let err = plan(&ok, &o).unwrap_err();
        assert!(err.contains("too many tracks: 5001"), "{err}");
    }
}
