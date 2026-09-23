//! apk-permission-explainer core — read an APK's permission list and explain
//! every entry in plain English, flagged by risk.
//!
//! An APK is a ZIP whose `AndroidManifest.xml` is Android's binary XML (AXML)
//! format, so the pipeline is: Base64 → ZIP → `AndroidManifest.xml` → AXML →
//! `<uses-permission>` / `<uses-permission-sdk-23>` / `<permission>` →
//! description table. Everything is pure Rust (`zip`, `base64`, `quick-xml`,
//! plus the local AXML decoder), so it runs entirely in the browser/wasm
//! sandbox — the APK never leaves the device.
//!
//! The input is deliberately forgiving: a Base64 APK, a Base64 standalone
//! `AndroidManifest.xml` (binary or text), or a plain-text manifest pasted
//! directly all reach the same extraction code.

mod axml;
mod perms;

use std::io::{Cursor, Read};

use axml::Element;
use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use perms::Risk;
use zip::ZipArchive;

/// Section order in the report, and the order `sort=risk` uses.
const RISK_ORDER: [Risk; 5] = [
    Risk::Dangerous,
    Risk::PrivacySensitive,
    Risk::Signature,
    Risk::Normal,
    Risk::Unknown,
];

/// One requested permission, after de-duplication.
#[derive(Debug, Clone)]
struct Permission {
    /// Fully-qualified name as written in the manifest.
    full: String,
    /// Display name — the `android.permission.` prefix dropped when present.
    short: String,
    risk: Risk,
    description: String,
    /// Declared via `<uses-permission-sdk-23>` (requested only on API 23+).
    sdk23_only: bool,
    /// `android:maxSdkVersion` — the permission stops being requested above it.
    max_sdk: Option<String>,
}

/// Manifest-level facts worth showing next to the permission list.
#[derive(Debug, Default, Clone)]
struct AppInfo {
    package: Option<String>,
    version_name: Option<String>,
    version_code: Option<String>,
    min_sdk: Option<String>,
    target_sdk: Option<String>,
    compile_sdk: Option<String>,
}

/// Explain the permissions an APK requests.
///
/// * `data` — Base64 APK bytes, Base64 `AndroidManifest.xml` bytes, or a
///   plain-text manifest pasted as-is.
/// * `mode` — `report` | `list` | `csv` | `json`.
/// * `risk` — `all` | `risky` | `dangerous` | `privacy-sensitive` | `signature`
///   | `normal` | `unknown`.
/// * `sort` — `risk` (most severe first) | `name` (A→Z).
pub fn run(data: &str, mode: &str, risk: &str, sort: &str) -> Result<String, String> {
    let mode = normalize(mode, "report");
    let risk_filter = normalize(risk, "all");
    let sort = normalize(sort, "risk");
    if !matches!(mode.as_str(), "report" | "list" | "csv" | "json") {
        return Err(format!(
            "unknown mode '{mode}' — use report, list, csv or json"
        ));
    }
    if !RISK_ORDER.iter().any(|r| r.slug() == risk_filter)
        && !matches!(risk_filter.as_str(), "all" | "risky")
    {
        return Err(format!(
            "unknown risk filter '{risk_filter}' — use all, risky, dangerous, \
             privacy-sensitive, signature, normal or unknown"
        ));
    }
    if !matches!(sort.as_str(), "risk" | "name") {
        return Err(format!("unknown sort '{sort}' — use risk or name"));
    }

    let elements = load_manifest(data)?;
    if !elements.iter().any(|e| e.name == "manifest") {
        return Err(
            "the decoded XML has no <manifest> element — this does not look like an \
             AndroidManifest.xml"
                .into(),
        );
    }

    let app = app_info(&elements);
    let all = collect_permissions(&elements);
    let declared = declared_permissions(&elements);
    let counts = RISK_ORDER.map(|r| all.iter().filter(|p| p.risk == r).count());

    let mut shown: Vec<&Permission> = all.iter().filter(|p| keep(p, &risk_filter)).collect();
    match sort.as_str() {
        "name" => shown.sort_by(|a, b| short_sort_key(&a.short).cmp(short_sort_key(&b.short))),
        _ => shown.sort_by(|a, b| (a.risk, &a.short).cmp(&(b.risk, &b.short))),
    }

    Ok(match mode.as_str() {
        "list" => render_list(&shown),
        "csv" => render_csv(&shown),
        "json" => render_json(&app, &all, &shown, &counts, &declared),
        _ => render_report(&app, &all, &shown, &counts, &declared, &risk_filter),
    })
}

fn normalize(s: &str, default: &str) -> String {
    let t = s.trim().to_ascii_lowercase();
    if t.is_empty() {
        default.to_string()
    } else {
        t
    }
}

fn keep(p: &Permission, filter: &str) -> bool {
    match filter {
        "all" => true,
        "risky" => matches!(
            p.risk,
            Risk::Dangerous | Risk::PrivacySensitive | Risk::Signature
        ),
        other => p.risk.slug() == other,
    }
}

fn short_sort_key(name: &str) -> &str {
    name.rsplit('.').next().unwrap_or(name)
}

// ---------------------------------------------------------------------------
// Input handling
// ---------------------------------------------------------------------------

/// Turn whatever the caller pasted into a flat element list.
fn load_manifest(data: &str) -> Result<Vec<Element>, String> {
    let trimmed = data.trim();
    if trimmed.is_empty() {
        return Err("no input: paste an APK as Base64, or paste an AndroidManifest.xml".into());
    }
    // A pasted text manifest needs no decoding at all.
    if trimmed.starts_with('<') {
        return axml::parse_text_xml(trimmed);
    }
    let bytes = decode_base64(trimmed)?;
    manifest_from_bytes(&bytes)
}

fn manifest_from_bytes(bytes: &[u8]) -> Result<Vec<Element>, String> {
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        let raw = read_apk_manifest(bytes)?;
        return manifest_from_bytes(&raw);
    }
    if axml::looks_like_axml(bytes) {
        return axml::decode(bytes);
    }
    let text = String::from_utf8_lossy(bytes);
    if text.trim_start().starts_with('<') {
        return axml::parse_text_xml(text.trim());
    }
    Err(
        "the decoded bytes are neither a ZIP/APK, a binary AndroidManifest.xml, nor XML text \
         — check that the Base64 covers the whole file"
            .into(),
    )
}

fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    // Tolerate a data: URL wrapper and any wrapping the paste picked up.
    let body = match s.find("base64,") {
        Some(i) if s.starts_with("data:") => &s[i + 7..],
        _ => s,
    };
    let cleaned: String = body.chars().filter(|c| !c.is_whitespace()).collect();
    for engine in [&STANDARD, &STANDARD_NO_PAD, &URL_SAFE, &URL_SAFE_NO_PAD] {
        if let Ok(v) = engine.decode(cleaned.as_bytes()) {
            if !v.is_empty() {
                return Ok(v);
            }
        }
    }
    Err(
        "could not decode the input as Base64 — paste the APK's Base64 text, or paste a \
         plain-text AndroidManifest.xml starting with '<'"
            .into(),
    )
}

/// Pull `AndroidManifest.xml` out of the APK's ZIP central directory.
fn read_apk_manifest(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut zip = ZipArchive::new(Cursor::new(bytes))
        .map_err(|e| format!("the input is not a readable ZIP/APK archive: {e}"))?;
    let index = (0..zip.len()).find(|i| {
        zip.by_index_raw(*i)
            .map(|f| f.name().eq_ignore_ascii_case("AndroidManifest.xml"))
            .unwrap_or(false)
    });
    let index = index.ok_or_else(|| {
        "the archive has no AndroidManifest.xml entry — an APK always has one at its root \
         (an .aab app bundle stores it under base/manifest/ instead)"
            .to_string()
    })?;
    let mut entry = zip
        .by_index(index)
        .map_err(|e| format!("could not read AndroidManifest.xml from the APK: {e}"))?;
    let mut out = Vec::new();
    entry
        .read_to_end(&mut out)
        .map_err(|e| format!("could not decompress AndroidManifest.xml: {e}"))?;
    Ok(out)
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

fn app_info(elements: &[Element]) -> AppInfo {
    let mut info = AppInfo::default();
    for e in elements {
        match e.name.as_str() {
            "manifest" => {
                info.package = e
                    .plain_attr("package")
                    .map(str::to_string)
                    .filter(|s| !s.is_empty());
                info.version_name = nonempty(e.attr("versionName"));
                info.version_code = nonempty(e.attr("versionCode"));
                info.compile_sdk = nonempty(e.attr("compileSdkVersion"));
            }
            "uses-sdk" => {
                info.min_sdk = nonempty(e.attr("minSdkVersion"));
                info.target_sdk = nonempty(e.attr("targetSdkVersion"));
            }
            _ => {}
        }
    }
    info
}

fn nonempty(v: Option<&str>) -> Option<String> {
    v.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Every requested permission, de-duplicated by name (first declaration wins,
/// later ones only contribute their flags).
fn collect_permissions(elements: &[Element]) -> Vec<Permission> {
    let mut out: Vec<Permission> = Vec::new();
    for e in elements {
        let sdk23_only = match e.name.as_str() {
            "uses-permission" => false,
            "uses-permission-sdk-23" => true,
            _ => continue,
        };
        let Some(full) = nonempty(e.attr("name")) else {
            continue;
        };
        let max_sdk = nonempty(e.attr("maxSdkVersion"));
        if let Some(existing) = out.iter_mut().find(|p| p.full == full) {
            existing.sdk23_only &= sdk23_only;
            if existing.max_sdk.is_none() {
                existing.max_sdk = max_sdk;
            }
            continue;
        }
        let info = perms::lookup(&full);
        out.push(Permission {
            short: full
                .strip_prefix("android.permission.")
                .unwrap_or(&full)
                .to_string(),
            full,
            risk: info.risk,
            description: info.description,
            sdk23_only,
            max_sdk,
        });
    }
    out
}

/// Custom permissions the APK *defines* for other apps to hold.
fn declared_permissions(elements: &[Element]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for e in elements.iter().filter(|e| e.name == "permission") {
        if let Some(n) = nonempty(e.attr("name")) {
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out.sort();
    out
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn notes(p: &Permission) -> Vec<String> {
    let mut n = Vec::new();
    if p.sdk23_only {
        n.push("requested only on Android 6.0 (API 23) and newer".into());
    }
    if let Some(max) = &p.max_sdk {
        n.push(format!("not requested above API {max}"));
    }
    n
}

fn render_report(
    app: &AppInfo,
    all: &[Permission],
    shown: &[&Permission],
    counts: &[usize; 5],
    declared: &[String],
    filter: &str,
) -> String {
    let mut s = String::new();
    let title = app
        .package
        .clone()
        .unwrap_or_else(|| "this APK".to_string());
    s.push_str(&format!("# Permissions requested by {title}\n\n"));

    let mut facts: Vec<String> = Vec::new();
    if let Some(p) = &app.package {
        facts.push(format!("- Package: `{p}`"));
    }
    match (&app.version_name, &app.version_code) {
        (Some(n), Some(c)) => facts.push(format!("- Version: {n} (version code {c})")),
        (Some(n), None) => facts.push(format!("- Version: {n}")),
        (None, Some(c)) => facts.push(format!("- Version code: {c}")),
        (None, None) => {}
    }
    let sdks: Vec<String> = [
        app.min_sdk
            .as_ref()
            .map(|v| format!("min SDK {v}{}", android_release(v))),
        app.target_sdk
            .as_ref()
            .map(|v| format!("target SDK {v}{}", android_release(v))),
        app.compile_sdk
            .as_ref()
            .map(|v| format!("compiled against SDK {v}")),
    ]
    .into_iter()
    .flatten()
    .collect();
    if !sdks.is_empty() {
        facts.push(format!("- SDK levels: {}", sdks.join(" · ")));
    }
    facts.push(format!("- Permissions requested: {}", all.len()));
    s.push_str(&facts.join("\n"));
    s.push_str("\n\n");

    if all.is_empty() {
        s.push_str(
            "This manifest requests no permissions at all. That is unusual but legitimate for a \
             fully offline app — it cannot reach the network, the camera, storage or your \
             contacts.\n",
        );
        return s;
    }

    s.push_str("## Risk summary\n\n| Risk | Count | What it means |\n| --- | --- | --- |\n");
    for (r, c) in RISK_ORDER.iter().zip(counts.iter()) {
        s.push_str(&format!("| {} | {} | {} |\n", r.label(), c, r.meaning()));
    }
    s.push('\n');

    let headline = counts[0] + counts[1];
    s.push_str(&format!(
        "{}\n\n",
        if headline == 0 {
            "No dangerous or privacy-sensitive permissions are requested — nothing here triggers a \
             runtime consent prompt."
                .to_string()
        } else {
            format!(
                "{headline} permission(s) are worth reading closely: {} runtime prompt(s) and {} \
                 privacy-sensitive grant(s) that appear without a prompt.",
                counts[0], counts[1]
            )
        }
    ));

    if shown.is_empty() {
        s.push_str(&format!("No permissions match the `{filter}` filter.\n"));
        return s;
    }

    for r in RISK_ORDER {
        let group: Vec<&&Permission> = shown.iter().filter(|p| p.risk == r).collect();
        if group.is_empty() {
            continue;
        }
        s.push_str(&format!("## {} ({})\n\n", r.label(), group.len()));
        s.push_str("| Permission | What the app can do | Notes |\n| --- | --- | --- |\n");
        for p in group {
            let note = notes(p);
            s.push_str(&format!(
                "| `{}` | {} | {} |\n",
                p.short,
                p.description,
                if note.is_empty() {
                    "—".to_string()
                } else {
                    note.join("; ")
                }
            ));
        }
        s.push('\n');
    }

    if !declared.is_empty() {
        s.push_str(&format!(
            "## Permissions this app defines ({})\n\nThese are declared by the app for other apps \
             to request; they do not grant the app anything by themselves.\n\n",
            declared.len()
        ));
        for d in declared {
            s.push_str(&format!("- `{d}`\n"));
        }
        s.push('\n');
    }

    s.push_str(
        "A permission in the manifest means the app *can ask*. Dangerous ones still need your \
         consent at runtime on Android 6.0 and newer, and you can revoke them later in Settings → \
         Apps → Permissions.\n",
    );
    s
}

/// Map an API level to its marketing release, for the levels users recognise.
fn android_release(level: &str) -> String {
    let name = match level.trim().parse::<u32>().ok() {
        Some(21) => "Android 5.0",
        Some(22) => "Android 5.1",
        Some(23) => "Android 6.0",
        Some(24) => "Android 7.0",
        Some(25) => "Android 7.1",
        Some(26) => "Android 8.0",
        Some(27) => "Android 8.1",
        Some(28) => "Android 9",
        Some(29) => "Android 10",
        Some(30) => "Android 11",
        Some(31) => "Android 12",
        Some(32) => "Android 12L",
        Some(33) => "Android 13",
        Some(34) => "Android 14",
        Some(35) => "Android 15",
        Some(36) => "Android 16",
        _ => return String::new(),
    };
    format!(" ({name})")
}

fn render_list(shown: &[&Permission]) -> String {
    if shown.is_empty() {
        return "No permissions match the filter.\n".to_string();
    }
    let mut s = String::new();
    for p in shown {
        let note = notes(p);
        s.push_str(&format!(
            "[{}] {} — {}{}\n",
            p.risk.slug(),
            p.full,
            p.description,
            if note.is_empty() {
                String::new()
            } else {
                format!(" ({})", note.join("; "))
            }
        ));
    }
    s
}

fn csv_field(v: &str) -> String {
    if v.contains([',', '"', '\n']) {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn render_csv(shown: &[&Permission]) -> String {
    let mut s = String::from("permission,risk,description,notes\n");
    for p in shown {
        s.push_str(&format!(
            "{},{},{},{}\n",
            csv_field(&p.full),
            csv_field(p.risk.slug()),
            csv_field(&p.description),
            csv_field(&notes(p).join("; "))
        ));
    }
    s
}

fn render_json(
    app: &AppInfo,
    all: &[Permission],
    shown: &[&Permission],
    counts: &[usize; 5],
    declared: &[String],
) -> String {
    use serde_json::{json, Value};
    let summary: Value = RISK_ORDER
        .iter()
        .zip(counts.iter())
        .map(|(r, c)| (r.slug().to_string(), json!(c)))
        .collect::<serde_json::Map<_, _>>()
        .into();
    let value = json!({
        "package": app.package,
        "version_name": app.version_name,
        "version_code": app.version_code,
        "min_sdk": app.min_sdk,
        "target_sdk": app.target_sdk,
        "compile_sdk": app.compile_sdk,
        "total_permissions": all.len(),
        "summary": summary,
        "declares_permissions": declared,
        "permissions": shown.iter().map(|p| json!({
            "name": p.full,
            "short_name": p.short,
            "risk": p.risk.slug(),
            "description": p.description,
            "sdk23_only": p.sdk23_only,
            "max_sdk_version": p.max_sdk,
        })).collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use zip::write::{SimpleFileOptions, ZipWriter};
    use zip::CompressionMethod;

    const ANDROID_NS: &str = "http://schemas.android.com/apk/res/android";

    /// One attribute of a synthetic AXML element: (namespace string index,
    /// attribute-name string index, string-value index).
    type FixtureAttr = (Option<usize>, usize, usize);

    /// Build a binary AndroidManifest.xml: file header, a UTF-8 string pool,
    /// then one START_ELEMENT chunk per element. This is the same layout aapt2
    /// emits, minus the chunks our decoder skips anyway.
    fn build_axml(strings: &[&str], elements: &[(usize, Vec<FixtureAttr>)]) -> Vec<u8> {
        // --- string pool ---
        let mut data = Vec::new();
        let mut offsets = Vec::new();
        for s in strings {
            assert!(s.len() < 128, "fixture strings use the 1-byte length form");
            offsets.push(data.len() as u32);
            data.push(s.chars().count() as u8);
            data.push(s.len() as u8);
            data.extend_from_slice(s.as_bytes());
            data.push(0);
        }
        while data.len() % 4 != 0 {
            data.push(0);
        }
        let strings_start = 28 + 4 * strings.len();
        let pool_size = strings_start + data.len();
        let mut pool = Vec::new();
        pool.extend_from_slice(&1u16.to_le_bytes()); // RES_STRING_POOL_TYPE
        pool.extend_from_slice(&28u16.to_le_bytes()); // headerSize
        pool.extend_from_slice(&(pool_size as u32).to_le_bytes());
        pool.extend_from_slice(&(strings.len() as u32).to_le_bytes());
        pool.extend_from_slice(&0u32.to_le_bytes()); // styleCount
        pool.extend_from_slice(&0x100u32.to_le_bytes()); // UTF8_FLAG
        pool.extend_from_slice(&(strings_start as u32).to_le_bytes());
        pool.extend_from_slice(&0u32.to_le_bytes()); // stylesStart
        for o in &offsets {
            pool.extend_from_slice(&o.to_le_bytes());
        }
        pool.extend_from_slice(&data);

        // --- start elements ---
        let mut body = Vec::new();
        for (name_idx, attrs) in elements {
            let size = 16 + 20 + 20 * attrs.len();
            body.extend_from_slice(&0x0102u16.to_le_bytes()); // START_ELEMENT
            body.extend_from_slice(&16u16.to_le_bytes()); // headerSize
            body.extend_from_slice(&(size as u32).to_le_bytes());
            body.extend_from_slice(&1u32.to_le_bytes()); // lineNumber
            body.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // comment
            body.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // ns
            body.extend_from_slice(&(*name_idx as u32).to_le_bytes());
            body.extend_from_slice(&20u16.to_le_bytes()); // attributeStart
            body.extend_from_slice(&20u16.to_le_bytes()); // attributeSize
            body.extend_from_slice(&(attrs.len() as u16).to_le_bytes());
            body.extend_from_slice(&0u16.to_le_bytes()); // idIndex
            body.extend_from_slice(&0u16.to_le_bytes()); // classIndex
            body.extend_from_slice(&0u16.to_le_bytes()); // styleIndex
            for (ns, name, value) in attrs {
                let ns = ns.map(|i| i as u32).unwrap_or(0xFFFF_FFFF);
                body.extend_from_slice(&ns.to_le_bytes());
                body.extend_from_slice(&(*name as u32).to_le_bytes());
                body.extend_from_slice(&(*value as u32).to_le_bytes()); // rawValue
                body.extend_from_slice(&8u16.to_le_bytes()); // typed size
                body.push(0); // res0
                body.push(0x03); // TYPE_STRING
                body.extend_from_slice(&(*value as u32).to_le_bytes());
            }
        }

        let total = 8 + pool.len() + body.len();
        let mut out = Vec::with_capacity(total);
        out.extend_from_slice(&3u16.to_le_bytes()); // RES_XML_TYPE
        out.extend_from_slice(&8u16.to_le_bytes()); // headerSize
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&pool);
        out.extend_from_slice(&body);
        out
    }

    /// A binary manifest for `com.example.demo` requesting a spread of risks.
    fn demo_axml() -> Vec<u8> {
        let strings = [
            ANDROID_NS,                                // 0
            "package",                                 // 1
            "name",                                    // 2
            "manifest",                                // 3
            "uses-permission",                         // 4
            "uses-sdk",                                // 5
            "com.example.demo",                        // 6
            "android.permission.CAMERA",               // 7
            "android.permission.INTERNET",             // 8
            "android.permission.ACCESS_FINE_LOCATION", // 9
            "minSdkVersion",                           // 10
            "targetSdkVersion",                        // 11
            "24",                                      // 12
            "34",                                      // 13
            "versionName",                             // 14
            "1.4.2",                                   // 15
            "uses-permission-sdk-23",                  // 16
            "com.google.android.gms.permission.AD_ID", // 17
        ];
        build_axml(
            &strings,
            &[
                (3, vec![(None, 1, 6), (Some(0), 14, 15)]),
                (5, vec![(Some(0), 10, 12), (Some(0), 11, 13)]),
                (4, vec![(Some(0), 2, 7)]),
                (4, vec![(Some(0), 2, 8)]),
                (4, vec![(Some(0), 2, 9)]),
                (16, vec![(Some(0), 2, 17)]),
            ],
        )
    }

    /// Wrap bytes in a one-entry ZIP the way an APK stores its manifest.
    fn apk_with(manifest: &[u8]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zw = ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            zw.start_file("AndroidManifest.xml", opts).unwrap();
            std::io::Write::write_all(&mut zw, manifest).unwrap();
            zw.start_file("classes.dex", opts).unwrap();
            std::io::Write::write_all(&mut zw, b"dex\n035\0not-a-real-dex").unwrap();
            zw.finish().unwrap();
        }
        buf
    }

    fn b64(bytes: &[u8]) -> String {
        STANDARD.encode(bytes)
    }

    const TEXT_MANIFEST: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<manifest xmlns:android="http://schemas.android.com/apk/res/android"
    package="com.example.text" android:versionName="2.0" android:versionCode="7">
  <uses-sdk android:minSdkVersion="26" android:targetSdkVersion="33" />
  <permission android:name="com.example.text.permission.SYNC" />
  <uses-permission android:name="android.permission.READ_CONTACTS" />
  <uses-permission android:name="android.permission.WRITE_EXTERNAL_STORAGE" android:maxSdkVersion="28" />
  <uses-permission android:name="android.permission.INTERNET" />
  <uses-permission android:name="com.example.text.permission.SYNC" />
  <application android:label="Text" />
</manifest>
"#;

    // ---- happy path: binary manifest inside an APK -------------------------

    #[test]
    fn report_from_base64_apk_lists_every_permission_with_risk() {
        let out = run(&b64(&apk_with(&demo_axml())), "report", "all", "risk").unwrap();
        assert!(
            out.contains("# Permissions requested by com.example.demo"),
            "{out}"
        );
        assert!(out.contains("- Package: `com.example.demo`"), "{out}");
        assert!(out.contains("Version: 1.4.2"), "{out}");
        assert!(out.contains("min SDK 24 (Android 7.0)"), "{out}");
        assert!(out.contains("target SDK 34 (Android 14)"), "{out}");
        assert!(out.contains("- Permissions requested: 4"), "{out}");
        assert!(out.contains("## Dangerous (2)"), "{out}");
        assert!(out.contains("| `CAMERA` |"), "{out}");
        assert!(out.contains("| `ACCESS_FINE_LOCATION` |"), "{out}");
        assert!(out.contains("## Privacy-sensitive (1)"), "{out}");
        assert!(out.contains("Google Advertising ID"), "{out}");
        assert!(out.contains("## Normal (1)"), "{out}");
        assert!(out.contains("| `INTERNET` |"), "{out}");
        assert!(
            out.contains("requested only on Android 6.0 (API 23) and newer"),
            "{out}"
        );
    }

    #[test]
    fn bare_binary_manifest_is_accepted_without_the_zip() {
        let out = run(&b64(&demo_axml()), "report", "all", "risk").unwrap();
        assert!(out.contains("| `CAMERA` |"), "{out}");
    }

    #[test]
    fn output_is_deterministic() {
        let input = b64(&apk_with(&demo_axml()));
        assert_eq!(
            run(&input, "report", "all", "risk").unwrap(),
            run(&input, "report", "all", "risk").unwrap()
        );
    }

    // ---- plain-text manifests ---------------------------------------------

    #[test]
    fn plain_text_manifest_is_parsed_directly() {
        let out = run(TEXT_MANIFEST, "report", "all", "risk").unwrap();
        assert!(
            out.contains("# Permissions requested by com.example.text"),
            "{out}"
        );
        assert!(out.contains("Version: 2.0 (version code 7)"), "{out}");
        assert!(out.contains("| `READ_CONTACTS` |"), "{out}");
        assert!(out.contains("not requested above API 28"), "{out}");
        assert!(out.contains("## Unknown / app-defined (1)"), "{out}");
        assert!(out.contains("## Permissions this app defines (1)"), "{out}");
        assert!(out.contains("`com.example.text.permission.SYNC`"), "{out}");
    }

    #[test]
    fn base64_wrapped_text_manifest_is_accepted() {
        let out = run(&b64(TEXT_MANIFEST.as_bytes()), "list", "all", "name").unwrap();
        assert!(out.contains("android.permission.READ_CONTACTS"), "{out}");
    }

    // ---- filters, sorting and output modes ---------------------------------

    #[test]
    fn risky_filter_drops_normal_permissions() {
        let out = run(&b64(&apk_with(&demo_axml())), "list", "risky", "risk").unwrap();
        assert!(out.contains("android.permission.CAMERA"), "{out}");
        assert!(!out.contains("android.permission.INTERNET"), "{out}");
    }

    #[test]
    fn risk_sort_puts_dangerous_first_and_name_sort_is_alphabetical() {
        let input = b64(&apk_with(&demo_axml()));
        let by_risk = run(&input, "list", "all", "risk").unwrap();
        let first = by_risk.lines().next().unwrap();
        assert!(
            first.starts_with("[dangerous] android.permission.ACCESS_FINE_LOCATION"),
            "{by_risk}"
        );

        let by_name = run(&input, "list", "all", "name").unwrap();
        let names: Vec<&str> = by_name
            .lines()
            .map(|l| l.split(' ').nth(1).unwrap())
            .collect();
        let mut sorted = names.clone();
        sorted.sort_by_key(|n| n.rsplit('.').next().unwrap());
        assert_eq!(names, sorted, "{by_name}");
    }

    #[test]
    fn csv_mode_quotes_descriptions_containing_commas() {
        let out = run(&b64(&apk_with(&demo_axml())), "csv", "dangerous", "name").unwrap();
        assert!(
            out.starts_with("permission,risk,description,notes\n"),
            "{out}"
        );
        assert_eq!(out.lines().count(), 3, "{out}");
        assert!(
            out.contains("android.permission.CAMERA,dangerous,Take photos"),
            "{out}"
        );
    }

    #[test]
    fn json_mode_reports_counts_and_metadata() {
        let out = run(&b64(&apk_with(&demo_axml())), "json", "all", "risk").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["package"], "com.example.demo");
        assert_eq!(v["min_sdk"], "24");
        assert_eq!(v["total_permissions"], 4);
        assert_eq!(v["summary"]["dangerous"], 2);
        assert_eq!(v["summary"]["privacy-sensitive"], 1);
        assert_eq!(v["permissions"][0]["risk"], "dangerous");
    }

    #[test]
    fn filter_that_matches_nothing_says_so_instead_of_failing() {
        let out = run(&b64(&apk_with(&demo_axml())), "report", "signature", "risk").unwrap();
        assert!(
            out.contains("No permissions match the `signature` filter."),
            "{out}"
        );
    }

    #[test]
    fn a_manifest_with_no_permissions_is_called_out() {
        let xml = r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="com.example.offline"/>"#;
        let out = run(xml, "report", "all", "risk").unwrap();
        assert!(out.contains("- Permissions requested: 0"), "{out}");
        assert!(out.contains("requests no permissions at all"), "{out}");
    }

    #[test]
    fn duplicate_declarations_are_collapsed() {
        let xml = r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="p">
          <uses-permission android:name="android.permission.CAMERA"/>
          <uses-permission android:name="android.permission.CAMERA"/>
        </manifest>"#;
        let out = run(xml, "report", "all", "risk").unwrap();
        assert!(out.contains("- Permissions requested: 1"), "{out}");
    }

    #[test]
    fn unknown_platform_permission_still_gets_a_verdict() {
        let xml = r#"<manifest xmlns:android="http://schemas.android.com/apk/res/android" package="p">
          <uses-permission android:name="android.permission.SOME_FUTURE_THING"/>
        </manifest>"#;
        let out = run(xml, "list", "all", "risk").unwrap();
        assert!(
            out.starts_with("[unknown] android.permission.SOME_FUTURE_THING"),
            "{out}"
        );
    }

    // ---- error cases -------------------------------------------------------

    #[test]
    fn empty_input_is_rejected() {
        let err = run("   ", "report", "all", "risk").unwrap_err();
        assert!(err.contains("no input"), "{err}");
    }

    #[test]
    fn non_base64_input_is_rejected() {
        let err = run("this is not base64 !!!", "report", "all", "risk").unwrap_err();
        assert!(
            err.contains("could not decode the input as Base64"),
            "{err}"
        );
    }

    #[test]
    fn a_zip_without_a_manifest_is_rejected() {
        let mut buf = Vec::new();
        {
            let mut zw = ZipWriter::new(Cursor::new(&mut buf));
            zw.start_file("readme.txt", SimpleFileOptions::default())
                .unwrap();
            std::io::Write::write_all(&mut zw, b"hello").unwrap();
            zw.finish().unwrap();
        }
        let err = run(&b64(&buf), "report", "all", "risk").unwrap_err();
        assert!(err.contains("no AndroidManifest.xml entry"), "{err}");
    }

    #[test]
    fn random_bytes_are_rejected_with_a_useful_message() {
        let err = run(
            &b64(&[0u8, 1, 2, 3, 4, 5, 6, 7, 8]),
            "report",
            "all",
            "risk",
        )
        .unwrap_err();
        assert!(err.contains("neither a ZIP/APK"), "{err}");
    }

    #[test]
    fn xml_that_is_not_a_manifest_is_rejected() {
        let err = run("<rss><channel/></rss>", "report", "all", "risk").unwrap_err();
        assert!(err.contains("no <manifest> element"), "{err}");
    }

    #[test]
    fn a_truncated_binary_manifest_is_rejected_not_panicked_on() {
        let full = demo_axml();
        let err = run(&b64(&full[..full.len() / 2]), "report", "all", "risk").unwrap_err();
        assert!(!err.is_empty(), "expected an error message");
    }

    #[test]
    fn bad_mode_risk_and_sort_values_are_rejected() {
        let input = b64(&apk_with(&demo_axml()));
        assert!(run(&input, "html", "all", "risk")
            .unwrap_err()
            .contains("unknown mode"));
        assert!(run(&input, "report", "scary", "risk")
            .unwrap_err()
            .contains("unknown risk filter"));
        assert!(run(&input, "report", "all", "size")
            .unwrap_err()
            .contains("unknown sort"));
    }

    #[test]
    fn empty_option_strings_fall_back_to_defaults() {
        let input = b64(&apk_with(&demo_axml()));
        assert_eq!(
            run(&input, "", "", "").unwrap(),
            run(&input, "report", "all", "risk").unwrap()
        );
    }
}
