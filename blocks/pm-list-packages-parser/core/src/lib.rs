//! pm-list-packages-parser core — turn raw `adb shell pm list packages` output
//! into a clean, filterable table.
//!
//! The parser is deliberately forgiving about which flag combination produced
//! the paste. Every documented output shape is accepted:
//!
//! ```text
//! package:com.example.app                                   (no flags)
//! package:/data/app/~~a==/com.example.app-b==/base.apk=com.example.app   (-f)
//! package:com.example.app  installer=com.android.vending    (-i)
//! package:com.example.app uid:10123                         (-U)
//! package:com.example.app versionCode:1234                  (--show-versioncode)
//! ```
//!
//! and any combination of them, because the trailing `key=value` / `key:value`
//! decorations are consumed from the end of the line before the
//! `<apk path>=<package>` head is split.
//!
//! System-vs-user classification comes from the APK partition: anything under
//! `/data/...` was installed (or updated) by the user, anything under
//! `/system`, `/system_ext`, `/product`, `/vendor`, `/apex`, `/oem` or `/odm`
//! ships with the ROM. A preinstalled app that has since been updated lives in
//! `/data/app` too, so an optional `pm list packages -s` paste is used to
//! promote those rows to `system-updated` instead of mislabelling them.
//!
//! A single `pm list packages` run carries no enabled/disabled bit at all, so
//! the status column stays `unknown` unless an optional `pm list packages -d`
//! paste is supplied.

use std::collections::BTreeMap;

/// Maximum number of package lines accepted per text field. A real device
/// lists a few hundred; the cap keeps a pasted log from wedging the browser.
pub const MAX_LINES: usize = 5000;

/// How a package got onto the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Installed by the user / sideloaded — lives under `/data`.
    User,
    /// Ships with the ROM — lives on a read-only system partition.
    System,
    /// Preinstalled, but the running copy is an update under `/data/app`.
    SystemUpdated,
    /// No APK path and no `-s` list, so origin cannot be determined.
    Unknown,
}

impl Kind {
    fn label(self) -> &'static str {
        match self {
            Kind::User => "user",
            Kind::System => "system",
            Kind::SystemUpdated => "system-updated",
            Kind::Unknown => "unknown",
        }
    }

    /// Section heading used when `group` is on.
    fn heading(self) -> &'static str {
        match self {
            Kind::User => "User apps",
            Kind::SystemUpdated => "Updated system apps",
            Kind::System => "System apps",
            Kind::Unknown => "Unclassified",
        }
    }

    /// Display order for grouping and for `sort=type`.
    fn rank(self) -> u8 {
        match self {
            Kind::User => 0,
            Kind::SystemUpdated => 1,
            Kind::System => 2,
            Kind::Unknown => 3,
        }
    }
}

/// Enabled/disabled state, which only a `pm list packages -d` paste can supply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Status {
    Enabled,
    Disabled,
    Unknown,
}

impl Status {
    fn label(self) -> &'static str {
        match self {
            Status::Enabled => "enabled",
            Status::Disabled => "disabled",
            Status::Unknown => "unknown",
        }
    }
}

/// One parsed line, before classification.
#[derive(Clone, Debug, Default)]
struct Entry {
    name: String,
    path: String,
    installer: String,
    version_code: String,
    uid: String,
}

/// A fully classified row, ready to render.
#[derive(Clone, Debug)]
struct Row {
    name: String,
    kind: Kind,
    status: Status,
    partition: String,
    path: String,
    installer: String,
    version_code: String,
    uid: String,
}

/// Entry point shared by the chat block, the CLI and the web page.
pub fn run(
    input: &str,
    filter: &str,
    format: &str,
    sort: &str,
    group: bool,
    disabled_list: &str,
    system_list: &str,
) -> Result<String, String> {
    let filter = norm(filter, "all");
    let format = norm(format, "table");
    let sort = norm(sort, "package");

    if !matches!(
        filter.as_str(),
        "all" | "user" | "system" | "system-updated" | "enabled" | "disabled"
    ) {
        return Err(format!(
            "unknown filter \"{filter}\" — use all, user, system, system-updated, enabled or disabled"
        ));
    }
    if !matches!(
        format.as_str(),
        "table" | "list" | "csv" | "json" | "markdown"
    ) {
        return Err(format!(
            "unknown format \"{format}\" — use table, list, csv, json or markdown"
        ));
    }
    if !matches!(sort.as_str(), "package" | "type" | "path") {
        return Err(format!(
            "unknown sort \"{sort}\" — use package, type or path"
        ));
    }

    let entries = parse_field(input, "package list")?;
    if entries.is_empty() {
        return Err("no packages found — paste the output of `adb shell pm list packages -f`, one `package:` line per app".into());
    }
    let disabled = name_set(disabled_list, "disabled list")?;
    let systems = name_set(system_list, "system list")?;

    let mut rows: Vec<Row> = entries
        .into_iter()
        .map(|e| classify(e, &disabled, &systems))
        .collect();

    let total = rows.len();
    let status_known = !disabled.is_empty();

    rows.retain(|r| keep(r, &filter));
    sort_rows(&mut rows, &sort);

    let summary = summarize(&rows, total, &filter, status_known);

    Ok(match format.as_str() {
        "list" => render_list(&rows),
        "csv" => render_csv(&rows),
        "json" => render_json(&rows, total, &filter, status_known),
        "markdown" => render_markdown(&rows, &summary, group, status_known, &filter, &disabled),
        _ => render_table(&rows, &summary, group, status_known, &filter, &disabled),
    })
}

fn norm(v: &str, default: &str) -> String {
    let v = v.trim();
    if v.is_empty() {
        default.to_string()
    } else {
        v.to_ascii_lowercase()
    }
}

// ---------------------------------------------------------------- parsing ---

/// True for a trailing decoration token such as `installer=com.android.vending`,
/// `uid:10123` or `versionCode:1234`. An APK path never matches (it starts with
/// `/`) and neither does a bare package name (no `:` or `=`).
fn is_decoration(token: &str) -> bool {
    let idx = token.find([':', '=']);
    let Some(idx) = idx else { return false };
    if idx == 0 {
        return false;
    }
    let key = &token[..idx];
    key.starts_with(|c: char| c.is_ascii_alphabetic())
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Lines that are shell noise rather than package data (a copied prompt, the
/// echoed command itself, or a `#` comment the user added).
fn is_noise(line: &str) -> bool {
    line.starts_with('#')
        || line.starts_with('$')
        || line.starts_with('>')
        || line.starts_with("adb ")
        || line.starts_with("pm ")
        || line.starts_with("cmd package ")
}

fn valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_')
}

/// Parse one pasted field into de-duplicated entries (later lines fill in
/// fields the first sighting left empty, so two runs can be pasted together).
fn parse_field(text: &str, field: &str) -> Result<Vec<Entry>, String> {
    let mut out: Vec<Entry> = Vec::new();
    let mut index: BTreeMap<String, usize> = BTreeMap::new();
    let mut seen = 0usize;

    for (n, raw) in text.lines().enumerate() {
        let line = raw.trim_end_matches('\r').trim();
        if line.is_empty() || is_noise(line) {
            continue;
        }
        seen += 1;
        if seen > MAX_LINES {
            return Err(format!(
                "{field} has more than {MAX_LINES} package lines — split the paste and run it in batches"
            ));
        }
        let entry = parse_line(line, n + 1, field)?;
        match index.get(&entry.name) {
            Some(&i) => merge(&mut out[i], entry),
            None => {
                index.insert(entry.name.clone(), out.len());
                out.push(entry);
            }
        }
    }
    Ok(out)
}

fn parse_line(line: &str, lineno: usize, field: &str) -> Result<Entry, String> {
    let rest = line.strip_prefix("package:").unwrap_or(line).trim();
    let mut tokens: Vec<&str> = rest.split_whitespace().collect();
    let mut entry = Entry::default();

    // Peel the trailing `-i` / `-U` / `--show-versioncode` decorations off the
    // end, never consuming the head token that carries the package name.
    let mut decorations: Vec<&str> = Vec::new();
    while tokens.len() > 1 && is_decoration(tokens[tokens.len() - 1]) {
        decorations.push(tokens.pop().unwrap());
    }
    for token in decorations {
        let (key, value) = split_decoration(token);
        match key.to_ascii_lowercase().as_str() {
            "installer" => entry.installer = value.to_string(),
            "uid" => entry.uid = value.to_string(),
            "versioncode" => entry.version_code = value.to_string(),
            // Unknown vendor decorations are ignored rather than fatal.
            _ => {}
        }
    }

    let head = tokens.first().copied().unwrap_or("");
    let (path, name) = match head.rsplit_once('=') {
        Some((p, n)) => (p, n),
        None => ("", head),
    };
    if !path.is_empty() && !path.starts_with('/') {
        return Err(bad_line(field, lineno, line));
    }
    if !valid_package_name(name) {
        return Err(bad_line(field, lineno, line));
    }
    entry.name = name.to_string();
    entry.path = path.to_string();
    Ok(entry)
}

fn bad_line(field: &str, lineno: usize, line: &str) -> String {
    format!(
        "{field} line {lineno}: could not read a package name from \"{line}\" — expected \
         `package:com.example.app` or `package:/data/app/.../base.apk=com.example.app`"
    )
}

fn split_decoration(token: &str) -> (&str, &str) {
    match token.find([':', '=']) {
        Some(i) => (&token[..i], &token[i + 1..]),
        None => (token, ""),
    }
}

fn merge(into: &mut Entry, from: Entry) {
    if into.path.is_empty() {
        into.path = from.path;
    }
    if into.installer.is_empty() {
        into.installer = from.installer;
    }
    if into.version_code.is_empty() {
        into.version_code = from.version_code;
    }
    if into.uid.is_empty() {
        into.uid = from.uid;
    }
}

fn name_set(text: &str, field: &str) -> Result<Vec<String>, String> {
    Ok(parse_field(text, field)?
        .into_iter()
        .map(|e| e.name)
        .collect())
}

// ----------------------------------------------------------- classification ---

fn partition_of(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let mut parts = path.trim_start_matches('/').split('/');
    let first = parts.next().unwrap_or("");
    let second = parts.next().unwrap_or("");
    match first {
        "system" | "system_ext" | "product" | "vendor" | "odm" | "oem" => {
            if second.starts_with("priv-app") {
                format!("/{first}/priv-app")
            } else {
                format!("/{first}")
            }
        }
        "data" => {
            if second == "app" {
                "/data/app".to_string()
            } else {
                "/data".to_string()
            }
        }
        "apex" => "/apex".to_string(),
        other => format!("/{other}"),
    }
}

fn classify(e: Entry, disabled: &[String], systems: &[String]) -> Row {
    let on_data = e.path.starts_with("/data/");
    let kind = if systems.iter().any(|s| *s == e.name) {
        if on_data {
            Kind::SystemUpdated
        } else {
            Kind::System
        }
    } else if !systems.is_empty() {
        Kind::User
    } else if e.path.is_empty() {
        Kind::Unknown
    } else if on_data {
        Kind::User
    } else {
        Kind::System
    };

    let status = if disabled.iter().any(|d| *d == e.name) {
        Status::Disabled
    } else if disabled.is_empty() {
        Status::Unknown
    } else {
        Status::Enabled
    };

    Row {
        partition: partition_of(&e.path),
        name: e.name,
        kind,
        status,
        path: e.path,
        installer: e.installer,
        version_code: e.version_code,
        uid: e.uid,
    }
}

fn keep(r: &Row, filter: &str) -> bool {
    match filter {
        "user" => r.kind == Kind::User,
        "system" => matches!(r.kind, Kind::System | Kind::SystemUpdated),
        "system-updated" => r.kind == Kind::SystemUpdated,
        "enabled" => r.status == Status::Enabled,
        "disabled" => r.status == Status::Disabled,
        _ => true,
    }
}

fn sort_rows(rows: &mut [Row], sort: &str) {
    match sort {
        "type" => rows.sort_by(|a, b| {
            a.kind
                .rank()
                .cmp(&b.kind.rank())
                .then_with(|| a.name.cmp(&b.name))
        }),
        "path" => rows.sort_by(|a, b| a.path.cmp(&b.path).then_with(|| a.name.cmp(&b.name))),
        _ => rows.sort_by(|a, b| a.name.cmp(&b.name)),
    }
}

// ------------------------------------------------------------- rendering ---

fn summarize(rows: &[Row], total: usize, filter: &str, status_known: bool) -> String {
    let count = |k: Kind| rows.iter().filter(|r| r.kind == k).count();
    let user = count(Kind::User);
    let system = count(Kind::System);
    let updated = count(Kind::SystemUpdated);
    let unknown = count(Kind::Unknown);

    let head = if filter == "all" {
        format!("{} {}", rows.len(), plural(rows.len()))
    } else {
        format!(
            "{} of {} {} shown (filter: {filter})",
            rows.len(),
            total,
            plural(total)
        )
    };
    let mut s = format!("{head}: {user} user, {system} system, {updated} updated system");
    if unknown > 0 {
        s.push_str(&format!(", {unknown} unclassified"));
    }
    if status_known {
        let disabled = rows.iter().filter(|r| r.status == Status::Disabled).count();
        let enabled = rows.iter().filter(|r| r.status == Status::Enabled).count();
        s.push_str(&format!(" | {disabled} disabled, {enabled} enabled"));
    }
    s
}

fn plural(n: usize) -> &'static str {
    if n == 1 {
        "package"
    } else {
        "packages"
    }
}

/// Reminder shown when a status filter was asked for but no `-d` paste exists.
fn status_hint(filter: &str, disabled_empty: bool) -> Option<&'static str> {
    if disabled_empty && matches!(filter, "enabled" | "disabled") {
        Some("No enabled/disabled information: paste the output of `adb shell pm list packages -d` into the disabled-packages field.")
    } else {
        None
    }
}

/// Which optional columns carry data. Package and type are always rendered.
struct Cols {
    status: bool,
    partition: bool,
    path: bool,
    installer: bool,
    version_code: bool,
    uid: bool,
}

fn columns(rows: &[Row], status_known: bool) -> Cols {
    Cols {
        status: status_known,
        partition: rows.iter().any(|r| !r.partition.is_empty()),
        path: rows.iter().any(|r| !r.path.is_empty()),
        installer: rows.iter().any(|r| !r.installer.is_empty()),
        version_code: rows.iter().any(|r| !r.version_code.is_empty()),
        uid: rows.iter().any(|r| !r.uid.is_empty()),
    }
}

fn headers(c: &Cols) -> Vec<&'static str> {
    let mut h = vec!["PACKAGE", "TYPE"];
    if c.status {
        h.push("STATUS");
    }
    if c.partition {
        h.push("PARTITION");
    }
    if c.path {
        h.push("APK PATH");
    }
    if c.installer {
        h.push("INSTALLER");
    }
    if c.version_code {
        h.push("VERSIONCODE");
    }
    if c.uid {
        h.push("UID");
    }
    h
}

fn cells(r: &Row, c: &Cols) -> Vec<String> {
    let mut v = vec![r.name.clone(), r.kind.label().to_string()];
    if c.status {
        v.push(r.status.label().to_string());
    }
    if c.partition {
        v.push(r.partition.clone());
    }
    if c.path {
        v.push(r.path.clone());
    }
    if c.installer {
        v.push(r.installer.clone());
    }
    if c.version_code {
        v.push(r.version_code.clone());
    }
    if c.uid {
        v.push(r.uid.clone());
    }
    v
}

fn render_list(rows: &[Row]) -> String {
    rows.iter()
        .map(|r| r.name.as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn grouped(rows: &[Row]) -> Vec<(Kind, Vec<&Row>)> {
    let mut out = Vec::new();
    for kind in [Kind::User, Kind::SystemUpdated, Kind::System, Kind::Unknown] {
        let group: Vec<&Row> = rows.iter().filter(|r| r.kind == kind).collect();
        if !group.is_empty() {
            out.push((kind, group));
        }
    }
    out
}

fn fixed_table(rows: &[&Row], c: &Cols) -> String {
    let head = headers(c);
    let mut grid: Vec<Vec<String>> = vec![head.iter().map(|h| h.to_string()).collect()];
    grid.extend(rows.iter().map(|r| cells(r, c)));

    let cols = head.len();
    let mut widths = vec![0usize; cols];
    for row in &grid {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }

    grid.iter()
        .map(|row| {
            let mut line = String::new();
            for (i, cell) in row.iter().enumerate() {
                if i + 1 == cols {
                    line.push_str(cell);
                } else {
                    line.push_str(cell);
                    let pad = widths[i] - cell.chars().count() + 2;
                    line.push_str(&" ".repeat(pad));
                }
            }
            line.trim_end().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_table(
    rows: &[Row],
    summary: &str,
    group: bool,
    status_known: bool,
    filter: &str,
    disabled: &[String],
) -> String {
    let c = columns(rows, status_known);
    let mut out = String::new();
    if rows.is_empty() {
        out.push_str("No packages matched the filter.\n\n");
    } else if group {
        for (kind, members) in grouped(rows) {
            out.push_str(&format!("{} ({})\n", kind.heading(), members.len()));
            out.push_str(&fixed_table(&members, &c));
            out.push_str("\n\n");
        }
    } else {
        let refs: Vec<&Row> = rows.iter().collect();
        out.push_str(&fixed_table(&refs, &c));
        out.push_str("\n\n");
    }
    out.push_str(summary);
    if let Some(hint) = status_hint(filter, disabled.is_empty()) {
        out.push('\n');
        out.push_str(hint);
    }
    out
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
}

fn md_table(rows: &[&Row], c: &Cols) -> String {
    let head = headers(c);
    let mut out = format!("| {} |\n", head.join(" | "));
    out.push_str(&format!(
        "| {} |\n",
        head.iter().map(|_| "---").collect::<Vec<_>>().join(" | ")
    ));
    for r in rows {
        let row: Vec<String> = cells(r, c)
            .iter()
            .map(|cell| {
                if cell.is_empty() {
                    String::new()
                } else {
                    md_escape(cell)
                }
            })
            .collect();
        out.push_str(&format!("| {} |\n", row.join(" | ")));
    }
    out
}

fn render_markdown(
    rows: &[Row],
    summary: &str,
    group: bool,
    status_known: bool,
    filter: &str,
    disabled: &[String],
) -> String {
    let c = columns(rows, status_known);
    let mut out = String::new();
    if rows.is_empty() {
        out.push_str("No packages matched the filter.\n\n");
    } else if group {
        for (kind, members) in grouped(rows) {
            out.push_str(&format!("### {} ({})\n\n", kind.heading(), members.len()));
            out.push_str(&md_table(&members, &c));
            out.push('\n');
        }
    } else {
        let refs: Vec<&Row> = rows.iter().collect();
        out.push_str(&md_table(&refs, &c));
        out.push('\n');
    }
    out.push_str(summary);
    if let Some(hint) = status_hint(filter, disabled.is_empty()) {
        out.push('\n');
        out.push_str(hint);
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// CSV and JSON keep the full fixed schema even when a column is empty, so
/// downstream scripts see a stable shape.
fn render_csv(rows: &[Row]) -> String {
    let mut out =
        String::from("package,type,status,partition,apk_path,installer,version_code,uid\n");
    for r in rows {
        let line = [
            r.name.as_str(),
            r.kind.label(),
            r.status.label(),
            r.partition.as_str(),
            r.path.as_str(),
            r.installer.as_str(),
            r.version_code.as_str(),
            r.uid.as_str(),
        ]
        .iter()
        .map(|c| csv_cell(c))
        .collect::<Vec<_>>()
        .join(",");
        out.push_str(&line);
        out.push('\n');
    }
    out
}

fn render_json(rows: &[Row], total: usize, filter: &str, status_known: bool) -> String {
    let count = |k: Kind| rows.iter().filter(|r| r.kind == k).count();
    let packages: Vec<serde_json::Value> = rows
        .iter()
        .map(|r| {
            serde_json::json!({
                "package": r.name,
                "type": r.kind.label(),
                "status": r.status.label(),
                "partition": r.partition,
                "apk_path": r.path,
                "installer": r.installer,
                "version_code": r.version_code,
                "uid": r.uid,
            })
        })
        .collect();
    let value = serde_json::json!({
        "total": total,
        "shown": rows.len(),
        "filter": filter,
        "status_available": status_known,
        "counts": {
            "user": count(Kind::User),
            "system": count(Kind::System),
            "system_updated": count(Kind::SystemUpdated),
            "unclassified": count(Kind::Unknown),
            "enabled": rows.iter().filter(|r| r.status == Status::Enabled).count(),
            "disabled": rows.iter().filter(|r| r.status == Status::Disabled).count(),
        },
        "packages": packages,
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "package:/data/app/~~kJ2v==/com.example.notes-9Q==/base.apk=com.example.notes\npackage:/system/priv-app/Settings/Settings.apk=com.android.settings\npackage:/product/app/Chrome/Chrome.apk=com.android.chrome\n";

    #[test]
    fn parses_dash_f_output_into_a_grouped_table() {
        let out = run(SAMPLE, "all", "table", "package", true, "", "").unwrap();
        assert!(out.starts_with("User apps (1)\n"), "{out}");
        assert!(out.contains("com.example.notes  user  /data/app"), "{out}");
        assert!(out.contains("System apps (2)"), "{out}");
        assert!(
            out.ends_with("3 packages: 1 user, 2 system, 0 updated system"),
            "{out}"
        );
    }

    #[test]
    fn rejects_a_line_that_has_no_package_name() {
        let err = run("package:/data/app/base.apk=", "all", "table", "package", true, "", "")
            .unwrap_err();
        assert!(err.contains("package list line 1"), "{err}");
        assert!(err.contains("could not read a package name"), "{err}");
    }

    #[test]
    fn rejects_unknown_enum_values() {
        assert!(run(SAMPLE, "nope", "table", "package", true, "", "")
            .unwrap_err()
            .contains("unknown filter"));
        assert!(run(SAMPLE, "all", "nope", "package", true, "", "")
            .unwrap_err()
            .contains("unknown format"));
        assert!(run(SAMPLE, "all", "table", "nope", true, "", "")
            .unwrap_err()
            .contains("unknown sort"));
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n\n", "all", "table", "package", true, "", "").unwrap_err();
        assert!(err.contains("no packages found"), "{err}");
    }

    #[test]
    fn plain_output_without_paths_is_unclassified() {
        let out = run(
            "package:com.example.one\npackage:com.example.two\n",
            "all",
            "list",
            "package",
            true,
            "",
            "",
        )
        .unwrap();
        assert_eq!(out, "com.example.one\ncom.example.two");
    }

    #[test]
    fn installer_uid_and_versioncode_decorations_are_split_out() {
        let out = run(
            "package:/data/app/x==/com.example.a-y==/base.apk=com.example.a  installer=com.android.vending uid:10123 versionCode:4417",
            "all",
            "csv",
            "package",
            true,
            "",
            "",
        )
        .unwrap();
        assert_eq!(
            out,
            "package,type,status,partition,apk_path,installer,version_code,uid\n\
             com.example.a,user,unknown,/data/app,/data/app/x==/com.example.a-y==/base.apk,com.android.vending,4417,10123\n"
        );
    }

    #[test]
    fn disabled_paste_supplies_status_and_enables_the_disabled_filter() {
        let out = run(
            SAMPLE,
            "disabled",
            "list",
            "package",
            true,
            "package:com.android.chrome\n",
            "",
        )
        .unwrap();
        assert_eq!(out, "com.android.chrome");

        let table = run(SAMPLE, "all", "table", "package", false, "package:com.android.chrome\n", "")
            .unwrap();
        let line = table
            .lines()
            .find(|l| l.starts_with("com.android.chrome"))
            .unwrap_or_else(|| panic!("{table}"));
        assert!(line.contains("system") && line.contains("disabled"), "{line}");
        assert!(table.ends_with("| 1 disabled, 2 enabled"), "{table}");
    }

    #[test]
    fn system_paste_promotes_an_updated_preinstalled_app() {
        let input = "package:/data/app/~~a==/com.android.chrome-b==/base.apk=com.android.chrome\npackage:/data/app/~~c==/com.example.notes-d==/base.apk=com.example.notes\n";
        let out = run(input, "all", "table", "type", false, "", "package:com.android.chrome\n")
            .unwrap();
        assert!(out.contains("com.android.chrome  system-updated"), "{out}");
        assert!(out.contains("com.example.notes   user"), "{out}");
        assert!(
            out.ends_with("2 packages: 1 user, 0 system, 1 updated system"),
            "{out}"
        );
    }

    #[test]
    fn filter_counts_report_against_the_full_list() {
        let out = run(SAMPLE, "user", "table", "package", false, "", "").unwrap();
        assert!(
            out.ends_with("1 of 3 packages shown (filter: user): 1 user, 0 system, 0 updated system"),
            "{out}"
        );
    }

    #[test]
    fn status_filter_without_a_disabled_paste_explains_itself() {
        let out = run(SAMPLE, "disabled", "table", "package", true, "", "").unwrap();
        assert!(out.starts_with("No packages matched the filter."), "{out}");
        assert!(out.contains("pm list packages -d"), "{out}");
    }

    #[test]
    fn duplicate_lines_from_two_runs_merge() {
        let input = "package:com.example.a\npackage:/data/app/a==/com.example.a-b==/base.apk=com.example.a\n";
        let out = run(input, "all", "csv", "package", true, "", "").unwrap();
        assert_eq!(out.lines().count(), 2, "{out}");
        assert!(out.contains("/data/app/a==/com.example.a-b==/base.apk"), "{out}");
    }

    #[test]
    fn shell_noise_and_comments_are_skipped() {
        let input = "$ adb shell pm list packages -f\n# my notes\npackage:com.example.a\n";
        let out = run(input, "all", "list", "package", true, "", "").unwrap();
        assert_eq!(out, "com.example.a");
    }

    #[test]
    fn markdown_and_json_render() {
        let md = run(SAMPLE, "all", "markdown", "package", false, "", "").unwrap();
        assert!(md.contains("| PACKAGE | TYPE | PARTITION | APK PATH |"), "{md}");
        let json = run(SAMPLE, "all", "json", "package", false, "", "").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["total"], 3);
        assert_eq!(v["counts"]["user"], 1);
        assert_eq!(v["packages"][0]["package"], "com.android.chrome");
        assert_eq!(v["packages"][0]["partition"], "/product");
    }

    #[test]
    fn line_cap_is_enforced_at_the_boundary() {
        let at_cap: String = (0..MAX_LINES)
            .map(|i| format!("package:com.example.a{i}\n"))
            .collect();
        assert!(run(&at_cap, "all", "list", "package", true, "", "").is_ok());
        let over = format!("{at_cap}package:com.example.over\n");
        let err = run(&over, "all", "list", "package", true, "", "").unwrap_err();
        assert!(err.contains("more than 5000 package lines"), "{err}");
    }
}
