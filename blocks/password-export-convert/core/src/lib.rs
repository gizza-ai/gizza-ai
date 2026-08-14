//! password-export-convert core — pure compute, shared by the chat skill block and the web page.
//!
//! Converts a password-manager export between Bitwarden JSON, Bitwarden CSV, KeePass/KeePassXC
//! CSV, LastPass CSV, Chrome CSV and a neutral generic CSV, in any direction. Everything runs on
//! the text that is handed in — no I/O, no clock, no randomness — so the same export always
//! converts to the same bytes.
//!
//! The pipeline is always parse → `Vec<Entry>` → write, so adding a format means adding one
//! reader and one writer, never an N×N matrix of converters.

use csv::{QuoteStyle, ReaderBuilder, WriterBuilder};
use serde_json::{json, Map, Value};

/// Hard cap on entries per run, so a pasted mega-vault can't wedge a browser tab.
pub const MAX_ENTRIES: usize = 5000;

/// The vault-export file formats this tool reads and writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    BitwardenJson,
    BitwardenCsv,
    KeepassCsv,
    LastpassCsv,
    ChromeCsv,
    GenericCsv,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "bitwarden-json" => Ok(Format::BitwardenJson),
            "bitwarden-csv" => Ok(Format::BitwardenCsv),
            "keepass-csv" => Ok(Format::KeepassCsv),
            "lastpass-csv" => Ok(Format::LastpassCsv),
            "chrome-csv" => Ok(Format::ChromeCsv),
            "generic-csv" => Ok(Format::GenericCsv),
            other => Err(format!(
                "unknown format \"{other}\"; expected one of bitwarden-json, bitwarden-csv, \
                 keepass-csv, lastpass-csv, chrome-csv, generic-csv"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::BitwardenJson => "bitwarden-json",
            Format::BitwardenCsv => "bitwarden-csv",
            Format::KeepassCsv => "keepass-csv",
            Format::LastpassCsv => "lastpass-csv",
            Format::ChromeCsv => "chrome-csv",
            Format::GenericCsv => "generic-csv",
        }
    }
}

/// What an entry is. Only Bitwarden distinguishes all four; every other format collapses the
/// non-login kinds into "a row with no password", so they round-trip as notes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Kind {
    #[default]
    Login,
    Note,
    Card,
    Identity,
}

impl Kind {
    fn from_bitwarden_type(n: i64) -> Kind {
        match n {
            2 => Kind::Note,
            3 => Kind::Card,
            4 => Kind::Identity,
            _ => Kind::Login,
        }
    }

    fn bitwarden_type(self) -> i64 {
        match self {
            Kind::Login => 1,
            // Cards and identities carry their detail in the note once converted, so they are
            // written back as secure notes rather than half-filled card objects.
            _ => 2,
        }
    }

    /// The word written to a `type` column, and the word `from_text` reads back.
    fn label(self) -> &'static str {
        match self {
            Kind::Login => "login",
            Kind::Note => "note",
            Kind::Card => "card",
            Kind::Identity => "identity",
        }
    }

    fn from_text(s: &str) -> Kind {
        match norm(s).as_str() {
            "note" | "securenote" | "2" => Kind::Note,
            "card" | "3" => Kind::Card,
            "identity" | "4" => Kind::Identity,
            _ => Kind::Login,
        }
    }
}

/// One vault entry, in the shape every supported format can be projected onto.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Entry {
    pub folder: String,
    pub name: String,
    pub username: String,
    pub password: String,
    pub url: String,
    pub notes: String,
    pub totp: String,
    pub favorite: bool,
    pub kind: Kind,
}

/// Normalise a header cell or type word: lowercase, letters and digits only, so `Login Name`,
/// `login_name` and `LOGINNAME` are the same column.
fn norm(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "x" | "t"
    )
}

/// Convert an export. `from` is a format label or `"auto"`; `to` is a format label.
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    from: &str,
    to: &str,
    include_totp: bool,
    include_extra_fields: bool,
    skip_empty_passwords: bool,
    default_folder: &str,
) -> Result<String, String> {
    let data = data.trim_start_matches('\u{feff}');
    if data.trim().is_empty() {
        return Err("paste a password-manager export first — this box is empty".to_string());
    }

    let target = Format::parse(to)?;
    let source = match from.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => detect(data)?,
        explicit => Format::parse(explicit)?,
    };

    let mut entries = match source {
        Format::BitwardenJson => read_bitwarden_json(data, include_extra_fields)?,
        _ => read_csv(data, include_extra_fields)?,
    };

    if entries.len() > MAX_ENTRIES {
        return Err(format!(
            "up to {MAX_ENTRIES} entries per run, this export has {}; split it and convert in \
             batches",
            entries.len()
        ));
    }

    if skip_empty_passwords {
        entries.retain(|e| !e.password.trim().is_empty());
    }
    for e in entries.iter_mut() {
        if !include_totp {
            e.totp.clear();
        }
        if e.folder.trim().is_empty() && !default_folder.trim().is_empty() {
            e.folder = default_folder.trim().to_string();
        }
    }

    if entries.is_empty() {
        return Err(format!(
            "no entries found — the input parsed as {} but produced no rows{}",
            source.label(),
            if skip_empty_passwords {
                " (every entry was dropped by \"skip entries with no password\")"
            } else {
                ""
            }
        ));
    }

    match target {
        Format::BitwardenJson => write_bitwarden_json(&entries),
        Format::BitwardenCsv => write_bitwarden_csv(&entries),
        Format::KeepassCsv => write_keepass_csv(&entries),
        Format::LastpassCsv => write_lastpass_csv(&entries),
        Format::ChromeCsv => write_chrome_csv(&entries, include_totp),
        Format::GenericCsv => write_generic_csv(&entries),
    }
}

// ---------------------------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------------------------

/// Sniff the source format from the bytes: JSON by its opening brace, CSV by its header shape.
pub fn detect(data: &str) -> Result<Format, String> {
    let trimmed = data.trim_start();
    if trimmed.starts_with('{') || trimmed.starts_with('[') {
        return Ok(Format::BitwardenJson);
    }

    let headers = csv_headers(data)?;
    let set: Vec<String> = headers.iter().map(|h| norm(h)).collect();
    let has = |n: &str| set.iter().any(|h| h.as_str() == n);

    if has("loginpassword") || has("loginuri") || has("logintotp") {
        return Ok(Format::BitwardenCsv);
    }
    if has("grouping") && has("extra") {
        return Ok(Format::LastpassCsv);
    }
    if has("title") && (has("group") || has("icon")) {
        return Ok(Format::KeepassCsv);
    }
    if has("note") && has("name") && has("url") && !has("notes") {
        return Ok(Format::ChromeCsv);
    }
    if has("password") || has("username") {
        return Ok(Format::GenericCsv);
    }

    Err(format!(
        "could not tell which export format this is. The first line is \"{}\" — a CSV export needs \
         a header row with at least a password or username column, and a Bitwarden JSON export \
         starts with {{. Pick the format by hand if the header was edited.",
        headers.join(",")
    ))
}

fn csv_headers(data: &str) -> Result<Vec<String>, String> {
    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(data.as_bytes());
    let headers = rdr
        .headers()
        .map_err(|e| format!("could not read the CSV header row: {e}"))?;
    Ok(headers.iter().map(|h| h.trim().to_string()).collect())
}

// ---------------------------------------------------------------------------------------------
// CSV reading (one alias-driven reader for every CSV dialect)
// ---------------------------------------------------------------------------------------------

const NAME_ALIASES: &[&str] = &[
    "name",
    "title",
    "account",
    "itemname",
    "entryname",
    "displayname",
    "site",
];
const USERNAME_ALIASES: &[&str] = &[
    "username",
    "loginusername",
    "loginname",
    "login",
    "user",
    "email",
    "emailaddress",
];
const PASSWORD_ALIASES: &[&str] = &["password", "loginpassword", "pass", "passwd", "secret"];
const URL_ALIASES: &[&str] = &[
    "url",
    "loginuri",
    "uri",
    "website",
    "weburl",
    "websiteurl",
    "loginurl",
    "link",
    "urls",
];
const NOTES_ALIASES: &[&str] = &[
    "notes",
    "note",
    "extra",
    "comments",
    "comment",
    "description",
    "memo",
];
const TOTP_ALIASES: &[&str] = &[
    "totp",
    "logintotp",
    "totpsecret",
    "otpauth",
    "otp",
    "authenticatorkey",
    "twofactorsecret",
];
const FOLDER_ALIASES: &[&str] = &[
    "folder",
    "group",
    "grouping",
    "category",
    "collection",
    "groupname",
    "path",
];
const FAVORITE_ALIASES: &[&str] = &["favorite", "favourite", "fav", "starred"];
const TYPE_ALIASES: &[&str] = &["type", "itemtype"];
/// Bookkeeping columns that exist in some dialect but mean nothing after a migration — they are
/// dropped rather than pasted into the note, so a KeePass `Icon`/`Created` pair doesn't end up as
/// text in every imported entry.
const IGNORED_COLUMNS: &[&str] = &[
    "icon",
    "created",
    "createdtime",
    "lastmodified",
    "modified",
    "lastused",
    "reprompt",
    "fields",
    "guid",
    "id",
    "uuid",
    "timecreated",
    "timelastused",
    "timepasswordchanged",
    "httprealm",
    "formactionorigin",
    "organizationid",
    "collectionids",
    "passwordhistory",
];

/// Resolve one logical field to a column index, preferring earlier aliases and never claiming a
/// column another field already took (so `Account`/`Login Name` land on name/username, not both
/// on the same cell).
fn column_for(headers: &[String], aliases: &[&str], used: &mut Vec<usize>) -> Option<usize> {
    for alias in aliases {
        for (i, h) in headers.iter().enumerate() {
            if h == alias && !used.contains(&i) {
                used.push(i);
                return Some(i);
            }
        }
    }
    None
}

fn read_csv(data: &str, include_extra_fields: bool) -> Result<Vec<Entry>, String> {
    let raw_headers = csv_headers(data)?;
    let headers: Vec<String> = raw_headers.iter().map(|h| norm(h)).collect();

    let mut used: Vec<usize> = Vec::new();
    // Resolution order matters: `name` claims "account" before `username` can.
    let c_name = column_for(&headers, NAME_ALIASES, &mut used);
    let c_username = column_for(&headers, USERNAME_ALIASES, &mut used);
    let c_password = column_for(&headers, PASSWORD_ALIASES, &mut used);
    let c_url = column_for(&headers, URL_ALIASES, &mut used);
    let c_notes = column_for(&headers, NOTES_ALIASES, &mut used);
    let c_totp = column_for(&headers, TOTP_ALIASES, &mut used);
    let c_folder = column_for(&headers, FOLDER_ALIASES, &mut used);
    let c_favorite = column_for(&headers, FAVORITE_ALIASES, &mut used);
    let c_type = column_for(&headers, TYPE_ALIASES, &mut used);

    if c_password.is_none() && c_username.is_none() {
        return Err(format!(
            "this CSV has no password or username column. Its header row is \"{}\"; a password \
             export needs a column named password (or login_password) and usually username too.",
            raw_headers.join(",")
        ));
    }

    let mut rdr = ReaderBuilder::new()
        .flexible(true)
        .has_headers(true)
        .from_reader(data.as_bytes());

    let mut entries = Vec::new();
    for (i, record) in rdr.records().enumerate() {
        let line = i + 2; // header is line 1
        let record = record.map_err(|e| format!("row {line}: could not read this CSV row: {e}"))?;
        let cell = |c: Option<usize>| -> String {
            c.and_then(|idx| record.get(idx))
                .unwrap_or("")
                .trim()
                .to_string()
        };

        let mut entry = Entry {
            folder: cell(c_folder),
            name: cell(c_name),
            username: cell(c_username),
            password: cell(c_password),
            url: cell(c_url),
            notes: c_notes
                .and_then(|idx| record.get(idx))
                .unwrap_or("")
                .to_string(),
            totp: cell(c_totp),
            favorite: truthy(&cell(c_favorite)),
            kind: match c_type {
                Some(_) => Kind::from_text(&cell(c_type)),
                None => Kind::Login,
            },
        };

        // LastPass writes secure notes with the placeholder address http://sn — that is a marker,
        // not a site, so it becomes a note rather than a login pointing at a dead host.
        if entry.url.eq_ignore_ascii_case("http://sn") {
            entry.url.clear();
            if entry.password.is_empty() {
                entry.kind = Kind::Note;
            }
        }

        if entry.name.is_empty()
            && entry.username.is_empty()
            && entry.password.is_empty()
            && entry.url.is_empty()
            && entry.notes.trim().is_empty()
        {
            continue; // blank row
        }
        if entry.name.is_empty() {
            entry.name = if !entry.url.is_empty() {
                entry.url.clone()
            } else {
                format!("Untitled entry {}", entries.len() + 1)
            };
        }

        if include_extra_fields {
            // Anything the alias table didn't claim is real user data in some dialect; keep it in
            // the note instead of dropping it silently.
            let mut extras = Vec::new();
            for (idx, header) in raw_headers.iter().enumerate() {
                if used.contains(&idx)
                    || header.trim().is_empty()
                    || IGNORED_COLUMNS.contains(&headers[idx].as_str())
                {
                    continue;
                }
                let value = record.get(idx).unwrap_or("").trim();
                if !value.is_empty() {
                    extras.push(format!("{}: {}", header.trim(), value));
                }
            }
            append_notes(&mut entry.notes, &extras);
        }

        entries.push(entry);
        if entries.len() > MAX_ENTRIES {
            return Err(format!(
                "up to {MAX_ENTRIES} entries per run; this CSV has more than that — split it and \
                 convert in batches"
            ));
        }
    }

    Ok(entries)
}

fn append_notes(notes: &mut String, lines: &[String]) {
    if lines.is_empty() {
        return;
    }
    if !notes.trim().is_empty() {
        notes.push('\n');
    } else {
        notes.clear();
    }
    notes.push_str(&lines.join("\n"));
}

// ---------------------------------------------------------------------------------------------
// Bitwarden JSON reading
// ---------------------------------------------------------------------------------------------

fn str_at(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.trim().to_string(),
        Some(Value::Number(n)) => n.to_string(),
        _ => String::new(),
    }
}

fn read_bitwarden_json(data: &str, include_extra_fields: bool) -> Result<Vec<Entry>, String> {
    let root: Value = serde_json::from_str(data).map_err(|e| {
        format!(
            "could not read this as Bitwarden JSON: {e}. A Bitwarden export is one object with an \
             \"items\" array — export with File Format .json, not .csv or the encrypted option."
        )
    })?;

    if root.get("encrypted") == Some(&Value::Bool(true)) {
        return Err("this Bitwarden export is encrypted (\"encrypted\": true), so its entries are \
                    ciphertext. Re-export with \"Password protect my export\" turned off, then \
                    convert the plain .json."
            .to_string());
    }

    let items = match &root {
        Value::Array(a) => a.clone(),
        Value::Object(_) => match root.get("items") {
            Some(Value::Array(a)) => a.clone(),
            _ => {
                return Err("this JSON has no \"items\" array. A Bitwarden export looks like \
                            {\"folders\": [...], \"items\": [...]} — check you exported the vault \
                            rather than a single item."
                    .to_string())
            }
        },
        _ => return Err("expected a Bitwarden export object, got a bare JSON value".to_string()),
    };

    let names_by_id = |key: &str| -> Map<String, Value> {
        let mut map = Map::new();
        if let Some(Value::Array(list)) = root.get(key) {
            for f in list {
                let id = str_at(f, "id");
                let name = str_at(f, "name");
                if !id.is_empty() && !name.is_empty() {
                    map.insert(id, Value::String(name));
                }
            }
        }
        map
    };
    let folders = names_by_id("folders");
    let collections = names_by_id("collections");

    let mut entries = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let n = i + 1;
        if !item.is_object() {
            return Err(format!("item {n}: expected an object, got {item}"));
        }

        let kind = match item.get("type") {
            Some(Value::Number(num)) => Kind::from_bitwarden_type(num.as_i64().unwrap_or(1)),
            Some(Value::String(s)) => Kind::from_text(s),
            _ => Kind::Login,
        };

        let mut folder = String::new();
        if let Some(Value::String(id)) = item.get("folderId") {
            if let Some(Value::String(name)) = folders.get(id) {
                folder = name.clone();
            }
        }
        if folder.is_empty() {
            if let Some(Value::Array(ids)) = item.get("collectionIds") {
                if let Some(Value::String(id)) = ids.first() {
                    if let Some(Value::String(name)) = collections.get(id) {
                        folder = name.clone();
                    }
                }
            }
        }

        let login = item.get("login");
        let mut url = String::new();
        let mut extra_urls = Vec::new();
        if let Some(Value::Array(uris)) = login.and_then(|l| l.get("uris")) {
            for (u_i, u) in uris.iter().enumerate() {
                let uri = match u {
                    Value::String(s) => s.trim().to_string(),
                    _ => str_at(u, "uri"),
                };
                if uri.is_empty() {
                    continue;
                }
                if u_i == 0 || url.is_empty() {
                    url = uri;
                } else {
                    extra_urls.push(format!("URL {}: {}", extra_urls.len() + 2, uri));
                }
            }
        }

        let mut entry = Entry {
            folder,
            name: str_at(item, "name"),
            username: login.map(|l| str_at(l, "username")).unwrap_or_default(),
            password: login.map(|l| str_at(l, "password")).unwrap_or_default(),
            url,
            notes: match item.get("notes") {
                Some(Value::String(s)) => s.clone(),
                _ => String::new(),
            },
            totp: login.map(|l| str_at(l, "totp")).unwrap_or_default(),
            favorite: matches!(item.get("favorite"), Some(Value::Bool(true)))
                || truthy(&str_at(item, "favorite")),
            kind,
        };
        if entry.name.is_empty() {
            entry.name = if entry.url.is_empty() {
                format!("Untitled entry {n}")
            } else {
                entry.url.clone()
            };
        }

        if include_extra_fields {
            let mut extras = extra_urls;
            if let Some(Value::Array(fields)) = item.get("fields") {
                for f in fields {
                    let name = str_at(f, "name");
                    let value = str_at(f, "value");
                    if name.is_empty() && value.is_empty() {
                        continue;
                    }
                    // Hidden custom fields (type 1) carry secrets; they are kept, because losing
                    // them silently on a migration is worse than writing them to the note.
                    extras.push(format!(
                        "{}: {value}",
                        if name.is_empty() { "field" } else { &name }
                    ));
                }
            }
            for (obj_key, labels) in [
                (
                    "card",
                    &[
                        ("cardholderName", "Cardholder"),
                        ("brand", "Brand"),
                        ("number", "Number"),
                        ("expMonth", "Expiry month"),
                        ("expYear", "Expiry year"),
                        ("code", "Security code"),
                    ][..],
                ),
                (
                    "identity",
                    &[
                        ("title", "Title"),
                        ("firstName", "First name"),
                        ("middleName", "Middle name"),
                        ("lastName", "Last name"),
                        ("company", "Company"),
                        ("email", "Email"),
                        ("phone", "Phone"),
                        ("address1", "Address"),
                        ("address2", "Address 2"),
                        ("city", "City"),
                        ("state", "State"),
                        ("postalCode", "Postal code"),
                        ("country", "Country"),
                        ("ssn", "SSN"),
                        ("passportNumber", "Passport number"),
                        ("licenseNumber", "Licence number"),
                    ][..],
                ),
            ] {
                if let Some(obj) = item.get(obj_key) {
                    for (key, label) in labels {
                        let value = str_at(obj, key);
                        if !value.is_empty() {
                            extras.push(format!("{label}: {value}"));
                        }
                    }
                }
            }
            append_notes(&mut entry.notes, &extras);
        }

        entries.push(entry);
        if entries.len() > MAX_ENTRIES {
            return Err(format!(
                "up to {MAX_ENTRIES} entries per run; this export has more than that — split it \
                 and convert in batches"
            ));
        }
    }

    Ok(entries)
}

// ---------------------------------------------------------------------------------------------
// Writers
// ---------------------------------------------------------------------------------------------

fn csv_out(
    header: &[&str],
    rows: Vec<Vec<String>>,
    quote_all: bool,
) -> Result<String, String> {
    let mut wtr = WriterBuilder::new()
        .quote_style(if quote_all {
            QuoteStyle::Always
        } else {
            QuoteStyle::Necessary
        })
        .from_writer(Vec::new());
    wtr.write_record(header)
        .map_err(|e| format!("could not write the CSV header: {e}"))?;
    for row in rows {
        wtr.write_record(&row)
            .map_err(|e| format!("could not write a CSV row: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("could not finish the CSV: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("the CSV was not valid UTF-8: {e}"))
}

/// A stable id derived from the entry's own text, so re-running the conversion produces the same
/// file — no clock, no randomness, clean diffs, and re-imports update instead of duplicating.
fn stable_id(prefix: &str, key: &str) -> String {
    let h1 = fnv1a(&format!("{prefix}\u{1}{key}"));
    let h2 = fnv1a(&format!("{key}\u{1}{prefix}\u{2}"));
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (h1 >> 32) as u32,
        (h1 >> 16) as u16,
        h1 as u16,
        (h2 >> 48) as u16,
        h2 & 0xffff_ffff_ffff
    )
}

fn fnv1a(s: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}

fn opt_str(s: &str) -> Value {
    if s.trim().is_empty() {
        Value::Null
    } else {
        Value::String(s.to_string())
    }
}

fn write_bitwarden_json(entries: &[Entry]) -> Result<String, String> {
    let mut folder_names: Vec<String> = Vec::new();
    for e in entries {
        let f = e.folder.trim();
        if !f.is_empty() && !folder_names.iter().any(|n| n == f) {
            folder_names.push(f.to_string());
        }
    }
    let folders: Vec<Value> = folder_names
        .iter()
        .map(|name| json!({ "id": stable_id("folder", name), "name": name }))
        .collect();

    let items: Vec<Value> = entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let folder_id = if e.folder.trim().is_empty() {
                Value::Null
            } else {
                Value::String(stable_id("folder", e.folder.trim()))
            };
            let mut item = json!({
                "id": stable_id("item", &format!("{}\u{1}{}\u{1}{i}", e.name, e.username)),
                "organizationId": Value::Null,
                "folderId": folder_id,
                "type": e.kind.bitwarden_type(),
                "reprompt": 0,
                "name": e.name,
                "notes": opt_str(&e.notes),
                "favorite": e.favorite,
                "fields": Value::Null,
                "collectionIds": Value::Null,
            });
            if e.kind == Kind::Login {
                let uris = if e.url.trim().is_empty() {
                    Value::Null
                } else {
                    json!([{ "match": Value::Null, "uri": e.url }])
                };
                item["login"] = json!({
                    "uris": uris,
                    "username": opt_str(&e.username),
                    "password": opt_str(&e.password),
                    "totp": opt_str(&e.totp),
                });
            } else {
                item["secureNote"] = json!({ "type": 0 });
            }
            item
        })
        .collect();

    serde_json::to_string_pretty(&json!({
        "encrypted": false,
        "folders": folders,
        "items": items,
    }))
    .map(|s| format!("{s}\n"))
    .map_err(|e| format!("could not write the Bitwarden JSON: {e}"))
}

fn write_bitwarden_csv(entries: &[Entry]) -> Result<String, String> {
    let rows = entries
        .iter()
        .map(|e| {
            let login = e.kind == Kind::Login;
            vec![
                e.folder.clone(),
                if e.favorite { "1" } else { "0" }.to_string(),
                if login { "login" } else { "note" }.to_string(),
                e.name.clone(),
                e.notes.clone(),
                String::new(), // fields — custom fields are folded into notes
                "0".to_string(),
                if login { e.url.clone() } else { String::new() },
                if login {
                    e.username.clone()
                } else {
                    String::new()
                },
                if login {
                    e.password.clone()
                } else {
                    String::new()
                },
                if login { e.totp.clone() } else { String::new() },
            ]
        })
        .collect();
    csv_out(
        &[
            "folder",
            "favorite",
            "type",
            "name",
            "notes",
            "fields",
            "reprompt",
            "login_uri",
            "login_username",
            "login_password",
            "login_totp",
        ],
        rows,
        false,
    )
}

fn write_keepass_csv(entries: &[Entry]) -> Result<String, String> {
    let rows = entries
        .iter()
        .map(|e| {
            vec![
                e.folder.clone(),
                e.name.clone(),
                e.username.clone(),
                e.password.clone(),
                e.url.clone(),
                e.notes.clone(),
                e.totp.clone(),
                "0".to_string(), // Icon — the default key icon
                String::new(),   // Last Modified — unknown, left for KeePass to stamp
                String::new(),   // Created
            ]
        })
        .collect();
    csv_out(
        &[
            "Group",
            "Title",
            "Username",
            "Password",
            "URL",
            "Notes",
            "TOTP",
            "Icon",
            "Last Modified",
            "Created",
        ],
        rows,
        true,
    )
}

fn write_lastpass_csv(entries: &[Entry]) -> Result<String, String> {
    let rows = entries
        .iter()
        .map(|e| {
            // http://sn is the marker LastPass uses for a secure note rather than a site login.
            let url = if e.kind == Kind::Login || !e.url.trim().is_empty() {
                e.url.clone()
            } else {
                "http://sn".to_string()
            };
            vec![
                url,
                e.username.clone(),
                e.password.clone(),
                e.totp.clone(),
                e.notes.clone(),
                e.name.clone(),
                e.folder.clone(),
                if e.favorite { "1" } else { "0" }.to_string(),
            ]
        })
        .collect();
    csv_out(
        &[
            "url", "username", "password", "totp", "extra", "name", "grouping", "fav",
        ],
        rows,
        false,
    )
}

fn write_chrome_csv(entries: &[Entry], include_totp: bool) -> Result<String, String> {
    let rows = entries
        .iter()
        .map(|e| {
            // Chrome's CSV has no TOTP column, so the secret rides in the note rather than being
            // dropped on the floor during a migration.
            let mut note = e.notes.clone();
            if include_totp && !e.totp.trim().is_empty() {
                append_notes(&mut note, &[format!("TOTP: {}", e.totp)]);
            }
            if !e.folder.trim().is_empty() {
                append_notes(&mut note, &[format!("Folder: {}", e.folder)]);
            }
            vec![
                e.name.clone(),
                e.url.clone(),
                e.username.clone(),
                e.password.clone(),
                note,
            ]
        })
        .collect();
    csv_out(&["name", "url", "username", "password", "note"], rows, false)
}

/// The neutral sheet. It carries every field of `Entry`, `type` included, so a generic CSV is the
/// one target that loses nothing — notes, cards and identities come back as themselves.
fn write_generic_csv(entries: &[Entry]) -> Result<String, String> {
    let rows = entries
        .iter()
        .map(|e| {
            vec![
                e.folder.clone(),
                e.name.clone(),
                e.url.clone(),
                e.username.clone(),
                e.password.clone(),
                e.notes.clone(),
                e.totp.clone(),
                if e.favorite { "1" } else { "0" }.to_string(),
                e.kind.label().to_string(),
            ]
        })
        .collect();
    csv_out(
        &[
            "folder", "name", "url", "username", "password", "notes", "totp", "favorite", "type",
        ],
        rows,
        false,
    )
}

// ---------------------------------------------------------------------------------------------
// Tests — every credential below is invented and belongs to no real account.
// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const BITWARDEN_JSON: &str = r#"{
      "encrypted": false,
      "folders": [{ "id": "f1", "name": "Work" }],
      "items": [
        {
          "id": "i1",
          "folderId": "f1",
          "type": 1,
          "name": "Example Mail",
          "notes": "recovery codes in the safe",
          "favorite": true,
          "fields": [{ "name": "PIN", "value": "0000", "type": 0 }],
          "login": {
            "uris": [{ "match": null, "uri": "https://mail.example.com" }],
            "username": "demo-user@example.com",
            "password": "sample-passphrase-1",
            "totp": "JBSWY3DPEHPK3PXP"
          }
        },
        {
          "id": "i2",
          "folderId": null,
          "type": 2,
          "name": "Router note",
          "notes": "admin panel at 192.0.2.1",
          "favorite": false
        }
      ]
    }"#;

    #[test]
    fn bitwarden_json_to_keepass_csv_is_exact() {
        let out = run(
            BITWARDEN_JSON,
            "auto",
            "keepass-csv",
            true,
            true,
            false,
            "",
        )
        .unwrap();
        assert_eq!(
            out,
            "\"Group\",\"Title\",\"Username\",\"Password\",\"URL\",\"Notes\",\"TOTP\",\"Icon\",\"Last Modified\",\"Created\"\n\
             \"Work\",\"Example Mail\",\"demo-user@example.com\",\"sample-passphrase-1\",\"https://mail.example.com\",\"recovery codes in the safe\nPIN: 0000\",\"JBSWY3DPEHPK3PXP\",\"0\",\"\",\"\"\n\
             \"\",\"Router note\",\"\",\"\",\"\",\"admin panel at 192.0.2.1\",\"\",\"0\",\"\",\"\"\n"
        );
    }

    #[test]
    fn detects_each_csv_dialect() {
        assert_eq!(detect(BITWARDEN_JSON).unwrap(), Format::BitwardenJson);
        assert_eq!(
            detect("folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n").unwrap(),
            Format::BitwardenCsv
        );
        assert_eq!(
            detect("url,username,password,totp,extra,name,grouping,fav\n").unwrap(),
            Format::LastpassCsv
        );
        assert_eq!(
            detect("Group,Title,Username,Password,URL,Notes,TOTP,Icon,Last Modified,Created\n")
                .unwrap(),
            Format::KeepassCsv
        );
        assert_eq!(
            detect("name,url,username,password,note\n").unwrap(),
            Format::ChromeCsv
        );
        assert_eq!(
            detect("Login Name,Password,Web Site\n").unwrap(),
            Format::GenericCsv
        );
    }

    #[test]
    fn keepass_csv_round_trips_back_to_bitwarden_json() {
        let keepass = run(BITWARDEN_JSON, "auto", "keepass-csv", true, true, false, "").unwrap();
        let json = run(&keepass, "auto", "bitwarden-json", true, true, false, "").unwrap();
        let v: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["folders"][0]["name"], "Work");
        assert_eq!(v["items"][0]["login"]["username"], "demo-user@example.com");
        assert_eq!(v["items"][0]["login"]["totp"], "JBSWY3DPEHPK3PXP");
        assert_eq!(v["items"][0]["favorite"], false); // KeePass CSV has no favourite column
        assert_eq!(
            v["items"][0]["login"]["uris"][0]["uri"],
            "https://mail.example.com"
        );
        // Stable ids: converting the same input twice gives byte-identical output.
        assert_eq!(
            json,
            run(&keepass, "auto", "bitwarden-json", true, true, false, "").unwrap()
        );
    }

    #[test]
    fn chrome_csv_to_bitwarden_csv() {
        let chrome = "name,url,username,password,note\n\
                      Example Shop,https://shop.example.com,demo-user@example.com,sample-passphrase-2,gift card inside\n";
        let out = run(chrome, "auto", "bitwarden-csv", true, true, false, "Imported").unwrap();
        assert_eq!(
            out,
            "folder,favorite,type,name,notes,fields,reprompt,login_uri,login_username,login_password,login_totp\n\
             Imported,0,login,Example Shop,gift card inside,,0,https://shop.example.com,demo-user@example.com,sample-passphrase-2,\n"
        );
    }

    #[test]
    fn lastpass_secure_note_marker_becomes_a_note() {
        let lastpass = "url,username,password,totp,extra,name,grouping,fav\n\
                        http://sn,,,,wifi key is on the router,Home wifi,Home,1\n";
        let out = run(lastpass, "lastpass-csv", "bitwarden-json", true, true, false, "").unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["items"][0]["type"], 2);
        assert_eq!(v["items"][0]["secureNote"]["type"], 0);
        assert_eq!(v["items"][0]["favorite"], true);
        assert!(v["items"][0].get("login").is_none());
    }

    #[test]
    fn passwords_with_commas_quotes_and_newlines_survive_a_round_trip() {
        let nasty = "a,b\"c\nd";
        let generic = format!(
            "name,url,username,password,notes\nTricky,https://example.org,demo-user,\"{}\",\n",
            nasty.replace('"', "\"\"")
        );
        let keepass = run(&generic, "auto", "keepass-csv", true, true, false, "").unwrap();
        let back = run(&keepass, "auto", "generic-csv", true, true, false, "").unwrap();
        let mut rdr = ReaderBuilder::new().from_reader(back.as_bytes());
        let row = rdr.records().next().unwrap().unwrap();
        // generic-csv column 4 is `password` — see write_generic_csv.
        assert_eq!(row.get(4).unwrap(), nasty);
    }

    #[test]
    fn generic_csv_is_exact_and_round_trips_the_entry_kind() {
        let out = run(BITWARDEN_JSON, "auto", "generic-csv", true, true, false, "").unwrap();
        assert_eq!(
            out,
            "folder,name,url,username,password,notes,totp,favorite,type\n\
             Work,Example Mail,https://mail.example.com,demo-user@example.com,sample-passphrase-1,\"recovery codes in the safe\nPIN: 0000\",JBSWY3DPEHPK3PXP,1,login\n\
             ,Router note,,,,admin panel at 192.0.2.1,,0,note\n"
        );
        // The `type` column is read back, so the secure note stays a secure note — the one target
        // that survives a full round trip.
        let back = run(&out, "auto", "bitwarden-json", true, true, false, "").unwrap();
        let v: Value = serde_json::from_str(&back).unwrap();
        assert_eq!(v["items"][0]["type"], 1);
        assert_eq!(v["items"][1]["type"], 2);
        assert_eq!(v["items"][0]["favorite"], true);
        assert_eq!(v["items"][0]["login"]["totp"], "JBSWY3DPEHPK3PXP");
    }

    #[test]
    fn totp_and_extra_fields_can_be_dropped() {
        let out = run(BITWARDEN_JSON, "auto", "generic-csv", false, false, false, "").unwrap();
        assert!(!out.contains("JBSWY3DPEHPK3PXP"), "totp should be dropped");
        assert!(!out.contains("PIN: 0000"), "custom fields should be dropped");
    }

    #[test]
    fn chrome_output_folds_totp_and_folder_into_the_note() {
        let out = run(BITWARDEN_JSON, "auto", "chrome-csv", true, true, false, "").unwrap();
        assert!(out.contains("TOTP: JBSWY3DPEHPK3PXP"));
        assert!(out.contains("Folder: Work"));
    }

    #[test]
    fn skip_empty_passwords_drops_the_note() {
        let out = run(BITWARDEN_JSON, "auto", "generic-csv", true, true, true, "").unwrap();
        assert!(out.contains("Example Mail"));
        assert!(!out.contains("Router note"));
    }

    #[test]
    fn card_details_are_carried_into_the_note() {
        let src = r#"{"items":[{"type":3,"name":"Sample card","card":{"cardholderName":"A Person","brand":"Visa","number":"4111111111111111","expMonth":"12","expYear":"2030","code":"123"}}]}"#;
        let out = run(src, "bitwarden-json", "keepass-csv", true, true, false, "").unwrap();
        assert!(out.contains("Cardholder: A Person"));
        assert!(out.contains("Expiry month: 12"));
    }

    #[test]
    fn encrypted_export_is_rejected_with_a_useful_message() {
        let err = run(
            r#"{"encrypted": true, "data": "2.abc|def"}"#,
            "auto",
            "keepass-csv",
            true,
            true,
            false,
            "",
        )
        .unwrap_err();
        assert!(err.contains("encrypted"), "got: {err}");
        assert!(err.contains("Password protect my export"), "got: {err}");
    }

    #[test]
    fn a_csv_without_credentials_columns_is_rejected() {
        let err = run(
            "first,last,city\nA,Person,Berlin\n",
            "auto",
            "keepass-csv",
            true,
            true,
            false,
            "",
        )
        .unwrap_err();
        assert!(err.contains("could not tell which export format"), "got: {err}");
    }

    #[test]
    fn unknown_target_format_is_rejected() {
        let err = run(BITWARDEN_JSON, "auto", "1password-csv", true, true, false, "").unwrap_err();
        assert!(err.contains("unknown format"), "got: {err}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = run("   ", "auto", "keepass-csv", true, true, false, "").unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn json_without_items_is_rejected() {
        let err = run(r#"{"folders": []}"#, "auto", "keepass-csv", true, true, false, "")
            .unwrap_err();
        assert!(err.contains("items"), "got: {err}");
    }

    #[test]
    fn too_many_entries_is_rejected() {
        let mut csv = String::from("name,username,password\n");
        for i in 0..(MAX_ENTRIES + 1) {
            csv.push_str(&format!("Site {i},demo-user,sample-passphrase-{i}\n"));
        }
        let err = run(&csv, "auto", "keepass-csv", true, true, false, "").unwrap_err();
        assert!(err.contains("5000 entries"), "got: {err}");
    }

    #[test]
    fn unclaimed_csv_columns_are_kept_in_the_note() {
        let csv = "name,username,password,created_at\nExample,demo-user,sample-passphrase-3,2026-01-02\n";
        let out = run(csv, "generic-csv", "keepass-csv", true, true, false, "").unwrap();
        assert!(out.contains("created_at: 2026-01-02"), "got: {out}");
        let dropped = run(csv, "generic-csv", "keepass-csv", true, false, false, "").unwrap();
        assert!(!dropped.contains("created_at"), "got: {dropped}");
    }

    #[test]
    fn bookkeeping_columns_never_leak_into_the_notes() {
        // KeePass writes Icon/Last Modified/Created; they mean nothing after a migration, so a
        // keepass → generic → keepass trip must not grow an "Icon: 0" line on every entry.
        let keepass = run(BITWARDEN_JSON, "auto", "keepass-csv", true, true, false, "").unwrap();
        let generic = run(&keepass, "auto", "generic-csv", true, true, false, "").unwrap();
        assert!(!generic.contains("Icon"), "got: {generic}");
        assert!(!generic.contains("Last Modified"), "got: {generic}");
        // Real user data in an unrecognised column is still rescued.
        assert!(generic.contains("PIN: 0000"), "got: {generic}");
    }
}
