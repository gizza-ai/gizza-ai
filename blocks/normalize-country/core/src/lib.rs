//! normalize-country core — resolves messy country names and codes to canonical
//! ISO 3166-1 forms (English short name, alpha-2, alpha-3, numeric).
//!
//! Pure compute, shared by the chat/CLI skill block and the browser page. No I/O,
//! no clock, no allocation of anything but the result — the whole ISO 3166-1 table
//! and the alias list are compiled in (see [`data`]).
//!
//! Resolution is layered and each layer is reported back to the caller as a
//! [`Status`], so a batch run tells you *how confident* each row is instead of
//! silently guessing:
//!
//! 1. `exact`     — an ISO alpha-2 / alpha-3 / numeric code, or an ISO short name.
//! 2. `alias`     — a curated variant: former name, endonym, abbreviation, demonym,
//!                  or a comma-inverted form such as `Korea, Republic of`.
//! 3. `fuzzy`     — a single near match within an edit-distance budget (typos).
//! 4. `ambiguous` — several equally-good near matches; nothing is picked.
//! 5. `unmatched` — no candidate at all.

pub mod data;

pub use data::{Country, ALIASES, COUNTRIES};

/// Maximum number of items accepted in one run.
pub const MAX_ITEMS: usize = 1000;

/// How an input item was resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    /// Matched an ISO code or the ISO short name outright.
    Exact,
    /// Matched a curated alias (former name, endonym, demonym, inverted form).
    Alias,
    /// Matched one candidate within the edit-distance budget.
    Fuzzy,
    /// Several candidates tied; the alpha-2 codes of the tied countries.
    Ambiguous(Vec<&'static str>),
    /// No candidate found.
    Unmatched,
}

impl Status {
    /// The word shown in the `Match` column / `match` field.
    pub fn label(&self) -> String {
        match self {
            Status::Exact => "exact".into(),
            Status::Alias => "alias".into(),
            Status::Fuzzy => "fuzzy".into(),
            Status::Ambiguous(c) => format!("ambiguous ({})", c.join("/")),
            Status::Unmatched => "unmatched".into(),
        }
    }

    /// True when no single country was resolved.
    pub fn is_unresolved(&self) -> bool {
        matches!(self, Status::Ambiguous(_) | Status::Unmatched)
    }
}

/// One resolved input item.
#[derive(Debug, Clone)]
pub struct Resolved {
    /// The item exactly as the caller wrote it (trimmed).
    pub input: String,
    /// The matched country, if exactly one was resolved.
    pub country: Option<&'static Country>,
    /// How it was resolved.
    pub status: Status,
}

/// Fold one character towards its ASCII base so `Türkiye`, `TURKIYE` and
/// `turkiye` all reach the same lookup key. Covers every non-ASCII character in
/// the ISO short names plus the accented forms people actually paste.
fn fold_char(c: char) -> &'static str {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'ç' | 'ć' | 'č' => "c",
        'ď' | 'đ' | 'ð' => "d",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ė' | 'ę' | 'ě' => "e",
        'ğ' => "g",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => "i",
        'ł' => "l",
        'ñ' | 'ń' | 'ň' => "n",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ő' => "o",
        'ř' => "r",
        'ś' | 'š' | 'ş' => "s",
        'ť' | 'ţ' => "t",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
        'ý' | 'ÿ' => "y",
        'ź' | 'ż' | 'ž' => "z",
        'æ' => "ae",
        'œ' => "oe",
        'ß' => "ss",
        'þ' => "th",
        '&' => "and",
        _ => "",
    }
}

/// Collapse a written country name/code to its lookup key: lowercase, accents
/// folded, `&` spelled out, every separator and punctuation mark dropped, and a
/// leading `the` removed. `"Côte d'Ivoire"`, `"COTE D IVOIRE"` and
/// `"cote-d'ivoire"` all become `cotedivoire`.
pub fn normalize_key(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !c.is_ascii() || c == '&' {
            for lc in c.to_lowercase() {
                let folded = fold_char(lc);
                if folded.is_empty() {
                    // Unknown non-ASCII (e.g. CJK): keep it out of the key rather
                    // than inventing a transliteration.
                    if lc.is_alphanumeric() {
                        out.push(lc);
                    }
                } else {
                    out.push_str(folded);
                }
            }
        }
    }
    if out.len() > 3 && out.starts_with("the") {
        out.drain(..3);
    }
    out
}

/// The flag emoji for an alpha-2 code (two Unicode regional indicator symbols).
pub fn flag_emoji(alpha2: &str) -> String {
    alpha2
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .filter_map(|c| char::from_u32(0x1F1E6 + (c.to_ascii_uppercase() as u32 - 'A' as u32)))
        .collect()
}

/// Levenshtein distance, abandoned early once every cell in a row exceeds `budget`.
fn edit_distance(a: &str, b: &str, budget: usize) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        let mut row_min = cur[0];
        for (j, &cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1);
            row_min = row_min.min(cur[j + 1]);
        }
        if row_min > budget {
            return budget + 1;
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// How many single-character edits a key of this length may absorb.
fn fuzzy_budget(len: usize) -> usize {
    match len {
        0..=3 => 0, // too short to guess safely
        4..=6 => 1,
        7..=12 => 2,
        _ => 3,
    }
}

fn by_alpha2(code: &str) -> Option<&'static Country> {
    COUNTRIES.iter().find(|c| c.alpha2.eq_ignore_ascii_case(code))
}

/// Every lookup key the matcher knows, paired with its alpha-2 code.
fn all_keys() -> Vec<(String, &'static str)> {
    let mut keys: Vec<(String, &'static str)> = Vec::with_capacity(COUNTRIES.len() * 2 + ALIASES.len());
    for c in COUNTRIES {
        keys.push((normalize_key(c.name), c.alpha2));
        let common = normalize_key(c.common);
        if common != normalize_key(c.name) {
            keys.push((common, c.alpha2));
        }
    }
    for (k, a2) in ALIASES {
        keys.push(((*k).to_string(), a2));
    }
    keys
}

/// Look a single already-normalized key up through the exact layers only.
fn lookup_key(key: &str) -> Option<(&'static Country, Status)> {
    if key.is_empty() {
        return None;
    }
    // Numeric code (with or without the conventional zero padding).
    if key.chars().all(|c| c.is_ascii_digit()) {
        if let Ok(n) = key.parse::<u16>() {
            return COUNTRIES
                .iter()
                .find(|c| c.numeric == n)
                .map(|c| (c, Status::Exact));
        }
        return None;
    }
    if key.len() == 2 {
        if let Some(c) = COUNTRIES.iter().find(|c| c.alpha2.eq_ignore_ascii_case(key)) {
            return Some((c, Status::Exact));
        }
    }
    if key.len() == 3 {
        if let Some(c) = COUNTRIES.iter().find(|c| c.alpha3.eq_ignore_ascii_case(key)) {
            return Some((c, Status::Exact));
        }
    }
    if let Some(c) = COUNTRIES.iter().find(|c| normalize_key(c.name) == key) {
        return Some((c, Status::Exact));
    }
    if let Some(c) = COUNTRIES.iter().find(|c| normalize_key(c.common) == key) {
        return Some((c, Status::Exact));
    }
    if let Some((_, a2)) = ALIASES.iter().find(|(k, _)| *k == key) {
        return by_alpha2(a2).map(|c| (c, Status::Alias));
    }
    None
}

/// Resolve one written country to an ISO 3166-1 entry.
///
/// `fuzzy` enables the edit-distance layer; with it off, a typo is reported
/// `unmatched` rather than guessed at.
pub fn resolve(raw: &str, fuzzy: bool) -> Resolved {
    let input = raw.trim().to_string();
    let primary = normalize_key(&input);

    if let Some((c, status)) = lookup_key(&primary) {
        return Resolved { input, country: Some(c), status };
    }

    // Comma-inverted register forms: "Korea, Republic of" -> "Republic of Korea",
    // and the plain head "Bolivia, Plurinational State of" -> "Bolivia".
    let head_key = if let Some((head, tail)) = input.split_once(',') {
        let rotated = normalize_key(&format!("{tail} {head}"));
        if let Some((c, _)) = lookup_key(&rotated) {
            return Resolved { input, country: Some(c), status: Status::Alias };
        }
        let head_key = normalize_key(head);
        if head_key != primary {
            if let Some((c, _)) = lookup_key(&head_key) {
                return Resolved { input, country: Some(c), status: Status::Alias };
            }
            Some(head_key)
        } else {
            None
        }
    } else {
        None
    };

    if fuzzy {
        for key in [Some(primary.clone()), head_key].into_iter().flatten() {
            let budget = fuzzy_budget(key.chars().count());
            if budget == 0 {
                continue;
            }
            let key_len = key.chars().count();
            let mut best = budget + 1;
            let mut hits: Vec<&'static str> = Vec::new();
            for (cand, a2) in all_keys() {
                let cand_len = cand.chars().count();
                if cand_len.abs_diff(key_len) > budget {
                    continue;
                }
                let d = edit_distance(&key, &cand, budget);
                if d > budget {
                    continue;
                }
                if d < best {
                    best = d;
                    hits.clear();
                }
                if d == best && !hits.contains(&a2) {
                    hits.push(a2);
                }
            }
            match hits.len() {
                0 => {}
                1 => {
                    return Resolved {
                        input,
                        country: by_alpha2(hits[0]),
                        status: Status::Fuzzy,
                    }
                }
                _ => {
                    hits.sort_unstable();
                    return Resolved { input, country: None, status: Status::Ambiguous(hits) };
                }
            }
        }
    }

    Resolved { input, country: None, status: Status::Unmatched }
}

/// Split a batch input into items. `auto` keeps commas intact when the input is
/// already one-per-line, so `Korea, Republic of` survives a pasted column.
fn split_items(input: &str, delimiter: &str) -> Result<Vec<String>, String> {
    let seps: &[char] = match delimiter {
        "auto" => {
            if input.contains('\n') {
                &['\n', '\r']
            } else {
                &[',', ';', '|', '\t']
            }
        }
        "newline" => &['\n', '\r'],
        "comma" => &[','],
        "semicolon" => &[';'],
        "pipe" => &['|'],
        "tab" => &['\t'],
        other => {
            return Err(format!(
                "unknown delimiter '{other}': expected one of auto, newline, comma, semicolon, pipe, tab"
            ))
        }
    };
    Ok(input
        .split(|c| seps.contains(&c))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect())
}

fn csv_field(s: &str) -> String {
    if s.contains([',', '"', '\n']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The display name for a row under the chosen `name_style`.
fn display_name(c: &Country, name_style: &str) -> &'static str {
    if name_style == "common" {
        c.common
    } else {
        c.name
    }
}

/// The single value a one-column output emits for a row.
fn single_value(r: &Resolved, output: &str, name_style: &str, on_unmatched: &str) -> String {
    match r.country {
        Some(c) => match output {
            "name" => display_name(c, name_style).to_string(),
            "alpha2" => c.alpha2.to_string(),
            "alpha3" => c.alpha3.to_string(),
            "numeric" => format!("{:03}", c.numeric),
            "flag" => flag_emoji(c.alpha2),
            _ => String::new(),
        },
        // "keep" echoes the original text so a converted column stays aligned with
        // the source column; "blank" leaves a hole.
        None if on_unmatched == "blank" => String::new(),
        None => r.input.clone(),
    }
}

/// Normalize a batch of written countries.
///
/// * `input` — one or more countries, split by `delimiter`.
/// * `output` — `table` | `name` | `alpha2` | `alpha3` | `numeric` | `flag` | `csv` | `json`.
/// * `name_style` — `iso` (ISO short name) or `common` (everyday name).
/// * `delimiter` — `auto` | `newline` | `comma` | `semicolon` | `pipe` | `tab`.
/// * `on_unmatched` — `keep` | `blank` | `omit` | `only`.
/// * `dedupe` — collapse repeats of the same country.
/// * `sort` — `input` | `asc` | `desc`.
/// * `fuzzy` — allow the edit-distance layer.
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    input: &str,
    output: &str,
    name_style: &str,
    delimiter: &str,
    on_unmatched: &str,
    dedupe: bool,
    sort: &str,
    fuzzy: bool,
) -> Result<String, String> {
    let output = if output.is_empty() { "table" } else { output };
    let name_style = if name_style.is_empty() { "iso" } else { name_style };
    let delimiter = if delimiter.is_empty() { "auto" } else { delimiter };
    let on_unmatched = if on_unmatched.is_empty() { "keep" } else { on_unmatched };
    let sort = if sort.is_empty() { "input" } else { sort };

    if !matches!(
        output,
        "table" | "name" | "alpha2" | "alpha3" | "numeric" | "flag" | "csv" | "json"
    ) {
        return Err(format!(
            "unknown output '{output}': expected one of table, name, alpha2, alpha3, numeric, flag, csv, json"
        ));
    }
    if !matches!(name_style, "iso" | "common") {
        return Err(format!(
            "unknown name_style '{name_style}': expected one of iso, common"
        ));
    }
    if !matches!(on_unmatched, "keep" | "blank" | "omit" | "only") {
        return Err(format!(
            "unknown on_unmatched '{on_unmatched}': expected one of keep, blank, omit, only"
        ));
    }
    if !matches!(sort, "input" | "asc" | "desc") {
        return Err(format!("unknown sort '{sort}': expected one of input, asc, desc"));
    }

    let items = split_items(input, delimiter)?;
    if items.is_empty() {
        return Err(
            "no input: expected at least one country name or ISO 3166 code, e.g. 'USA, Deutschland, 826'"
                .into(),
        );
    }
    if items.len() > MAX_ITEMS {
        return Err(format!(
            "too many items: expected at most {MAX_ITEMS} per run, got {} — split the list and run it again",
            items.len()
        ));
    }

    let mut rows: Vec<Resolved> = items.iter().map(|i| resolve(i, fuzzy)).collect();

    if dedupe {
        let mut seen: Vec<String> = Vec::new();
        rows.retain(|r| {
            let key = match r.country {
                Some(c) => c.alpha2.to_string(),
                None => format!("?{}", normalize_key(&r.input)),
            };
            if seen.contains(&key) {
                false
            } else {
                seen.push(key);
                true
            }
        });
    }

    if on_unmatched == "omit" {
        rows.retain(|r| r.country.is_some());
    } else if on_unmatched == "only" {
        rows.retain(|r| r.country.is_none());
    }

    if rows.is_empty() {
        return Ok(String::new());
    }

    if sort != "input" {
        rows.sort_by_key(|r| {
            let key = match (&r.country, output) {
                (Some(c), "alpha2") => c.alpha2.to_string(),
                (Some(c), "alpha3") => c.alpha3.to_string(),
                (Some(c), "numeric") => format!("{:03}", c.numeric),
                (Some(c), _) => display_name(c, name_style).to_string(),
                (None, _) => r.input.clone(),
            };
            key.to_lowercase()
        });
        if sort == "desc" {
            rows.reverse();
        }
    }

    Ok(match output {
        "name" | "alpha2" | "alpha3" | "numeric" | "flag" => rows
            .iter()
            .map(|r| single_value(r, output, name_style, on_unmatched))
            .collect::<Vec<_>>()
            .join("\n"),
        "csv" => {
            let mut out = String::from("input,name,alpha2,alpha3,numeric,flag,match\n");
            for r in &rows {
                let (name, a2, a3, num, flag) = match r.country {
                    Some(c) => (
                        display_name(c, name_style).to_string(),
                        c.alpha2.to_string(),
                        c.alpha3.to_string(),
                        format!("{:03}", c.numeric),
                        flag_emoji(c.alpha2),
                    ),
                    None => Default::default(),
                };
                out.push_str(&format!(
                    "{},{},{},{},{},{},{}\n",
                    csv_field(&r.input),
                    csv_field(&name),
                    a2,
                    a3,
                    num,
                    flag,
                    csv_field(&r.status.label())
                ));
            }
            out.trim_end().to_string()
        }
        "json" => {
            let mut out = String::from("[\n");
            for (i, r) in rows.iter().enumerate() {
                let body = match r.country {
                    Some(c) => format!(
                        "\"name\": {}, \"alpha2\": {}, \"alpha3\": {}, \"numeric\": {}, \"flag\": {}",
                        json_string(display_name(c, name_style)),
                        json_string(c.alpha2),
                        json_string(c.alpha3),
                        json_string(&format!("{:03}", c.numeric)),
                        json_string(&flag_emoji(c.alpha2)),
                    ),
                    None => "\"name\": null, \"alpha2\": null, \"alpha3\": null, \"numeric\": null, \"flag\": null".into(),
                };
                out.push_str(&format!(
                    "  {{\"input\": {}, {}, \"match\": {}}}{}\n",
                    json_string(&r.input),
                    body,
                    json_string(&r.status.label()),
                    if i + 1 == rows.len() { "" } else { "," }
                ));
            }
            out.push(']');
            out
        }
        _ => {
            let header = ["Input", "Name", "Alpha-2", "Alpha-3", "Numeric", "Match", "Flag"];
            let cells: Vec<[String; 7]> = rows
                .iter()
                .map(|r| match r.country {
                    Some(c) => [
                        r.input.clone(),
                        display_name(c, name_style).to_string(),
                        c.alpha2.to_string(),
                        c.alpha3.to_string(),
                        format!("{:03}", c.numeric),
                        r.status.label(),
                        flag_emoji(c.alpha2),
                    ],
                    None => [
                        r.input.clone(),
                        "—".into(),
                        "—".into(),
                        "—".into(),
                        "—".into(),
                        r.status.label(),
                        "—".into(),
                    ],
                })
                .collect();
            // Flag is last so its double-width emoji can't skew any other column.
            let mut widths = [0usize; 7];
            for (i, h) in header.iter().enumerate() {
                widths[i] = h.chars().count();
            }
            for row in &cells {
                for (i, c) in row.iter().enumerate() {
                    widths[i] = widths[i].max(c.chars().count());
                }
            }
            let mut out = String::new();
            let render = |cols: &[String], out: &mut String| {
                let line: Vec<String> = cols
                    .iter()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == cols.len() - 1 {
                            c.clone()
                        } else {
                            format!("{c:<width$}", width = widths[i])
                        }
                    })
                    .collect();
                out.push_str(line.join("  ").trim_end());
                out.push('\n');
            };
            render(&header.iter().map(|s| s.to_string()).collect::<Vec<_>>(), &mut out);
            for row in &cells {
                render(row, &mut out);
            }
            out.trim_end().to_string()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(raw: &str) -> Resolved {
        resolve(raw, true)
    }

    #[test]
    fn data_table_is_wellformed() {
        assert_eq!(COUNTRIES.len(), 249, "ISO 3166-1 officially assigned entries");
        for c in COUNTRIES {
            assert_eq!(c.alpha2.len(), 2, "{} alpha-2", c.name);
            assert_eq!(c.alpha3.len(), 3, "{} alpha-3", c.name);
            assert!(c.numeric > 0 && c.numeric < 1000, "{} numeric", c.name);
        }
        // Alias keys are stored pre-normalized; a hand-edit that breaks that would
        // make the entry unreachable, so guard it here.
        for (k, a2) in ALIASES {
            assert_eq!(&normalize_key(k), k, "alias key {k} is not normalized");
            assert!(by_alpha2(a2).is_some(), "alias {k} -> unknown code {a2}");
        }
    }

    #[test]
    fn happy_path_resolves_every_input_form() {
        // alpha-2, alpha-3, numeric (padded and bare), ISO name, common name.
        for raw in ["US", "us", "USA", "840", "0840", "United States of America", "united states"] {
            let r = one(raw);
            assert_eq!(r.country.unwrap().alpha2, "US", "{raw}");
            assert_eq!(r.status, Status::Exact, "{raw}");
        }
        // Alias layers: former name, endonym, abbreviation, demonym, UK nation.
        for (raw, want) in [
            ("Burma", "MM"),
            ("Deutschland", "DE"),
            ("UK", "GB"),
            ("Scotland", "GB"),
            ("Holland", "NL"),
            ("Czech Republic", "CZ"),
            ("swiss", "CH"),
            ("Zaire", "CD"),
        ] {
            let r = one(raw);
            assert_eq!(r.country.unwrap().alpha2, want, "{raw}");
            assert_eq!(r.status, Status::Alias, "{raw}");
        }
        // Punctuation, case and accents are all folded away.
        assert_eq!(one("côte d'IVOIRE").country.unwrap().alpha2, "CI");
        assert_eq!(one("  St. Kitts & Nevis ").country.unwrap().alpha2, "KN");
        // Comma-inverted register form, then the plain head.
        assert_eq!(one("Korea, Republic of").country.unwrap().alpha2, "KR");
        assert_eq!(one("Bolivia, Plurinational State of").country.unwrap().alpha2, "BO");
        // Typo, via the fuzzy layer only.
        let r = one("Swizerland");
        assert_eq!(r.country.unwrap().alpha2, "CH");
        assert_eq!(r.status, Status::Fuzzy);
        assert_eq!(resolve("Swizerland", false).status, Status::Unmatched);
    }

    #[test]
    fn unresolvable_inputs_are_reported_not_guessed() {
        // Never assigned by ISO 3166-1.
        for raw in ["Kosovo", "Soviet Union", "Atlantis", "EU"] {
            assert!(one(raw).country.is_none(), "{raw} should not resolve");
        }
        assert_eq!(one("Atlantis").status, Status::Unmatched);
        // Austria and Australia are both one edit away, so nothing is picked.
        let r = one("Austrlia");
        assert!(r.country.is_none());
        match r.status {
            Status::Ambiguous(c) => assert_eq!(c, vec!["AT", "AU"]),
            other => panic!("expected ambiguous, got {other:?}"),
        }
    }

    #[test]
    fn normalize_rejects_bad_arguments() {
        assert!(normalize("", "table", "iso", "auto", "keep", false, "input", true)
            .unwrap_err()
            .starts_with("no input:"));
        assert!(normalize("US", "xml", "iso", "auto", "keep", false, "input", true)
            .unwrap_err()
            .starts_with("unknown output 'xml':"));
        assert!(normalize("US", "table", "iso", "colon", "keep", false, "input", true)
            .unwrap_err()
            .starts_with("unknown delimiter 'colon':"));
        assert!(normalize("US", "table", "latin", "auto", "keep", false, "input", true)
            .unwrap_err()
            .starts_with("unknown name_style 'latin':"));
        assert!(normalize("US", "table", "iso", "auto", "drop", false, "input", true)
            .unwrap_err()
            .starts_with("unknown on_unmatched 'drop':"));
        assert!(normalize("US", "table", "iso", "auto", "keep", false, "random", true)
            .unwrap_err()
            .starts_with("unknown sort 'random':"));
        // Cap boundary: MAX_ITEMS is fine, one more is refused.
        let at_cap = vec!["US"; MAX_ITEMS].join("\n");
        assert!(normalize(&at_cap, "alpha2", "iso", "auto", "keep", false, "input", false).is_ok());
        let over_cap = vec!["US"; MAX_ITEMS + 1].join("\n");
        let err = normalize(&over_cap, "alpha2", "iso", "auto", "keep", false, "input", false)
            .unwrap_err();
        assert!(err.starts_with("too many items:"), "{err}");
        assert!(err.contains("1001"), "{err}");
    }

    #[test]
    fn output_formats_render_as_documented() {
        let got = normalize("usa, uk", "alpha3", "iso", "auto", "keep", false, "input", true).unwrap();
        assert_eq!(got, "USA\nGBR");
        let got = normalize("nz\nvn", "name", "common", "auto", "keep", false, "input", true).unwrap();
        assert_eq!(got, "New Zealand\nVietnam");
        let got = normalize("nz\nvn", "name", "iso", "auto", "keep", false, "input", true).unwrap();
        assert_eq!(got, "New Zealand\nViet Nam");
        // Numeric always carries the conventional three digits.
        let got = normalize("Afghanistan", "numeric", "iso", "auto", "keep", false, "input", true).unwrap();
        assert_eq!(got, "004");
        let got = normalize("de", "flag", "iso", "auto", "keep", false, "input", true).unwrap();
        assert_eq!(got, "🇩🇪");
        let got = normalize("Japan", "csv", "iso", "auto", "keep", false, "input", true).unwrap();
        assert_eq!(
            got,
            "input,name,alpha2,alpha3,numeric,flag,match\nJapan,Japan,JP,JPN,392,🇯🇵,exact"
        );
        let got = normalize("Nihon", "json", "common", "auto", "keep", false, "input", true).unwrap();
        assert_eq!(
            got,
            "[\n  {\"input\": \"Nihon\", \"name\": \"Japan\", \"alpha2\": \"JP\", \"alpha3\": \"JPN\", \"numeric\": \"392\", \"flag\": \"🇯🇵\", \"match\": \"alias\"}\n]"
        );
        let got = normalize("fr", "table", "iso", "auto", "keep", false, "input", true).unwrap();
        assert_eq!(
            got,
            "Input  Name    Alpha-2  Alpha-3  Numeric  Match  Flag\nfr     France  FR       FRA      250      exact  🇫🇷"
        );
    }

    #[test]
    fn batch_options_control_rows() {
        // auto keeps commas intact once the input is one-per-line.
        let listy = "Korea, Republic of\nMexico";
        assert_eq!(
            normalize(listy, "alpha2", "iso", "auto", "keep", false, "input", true).unwrap(),
            "KR\nMX"
        );
        // ...but splits a single line on commas.
        assert_eq!(
            normalize("France, Japan", "alpha2", "iso", "auto", "keep", false, "input", true).unwrap(),
            "FR\nJP"
        );
        let messy = "USA\nAtlantis\nunited states\nJapan";
        // keep echoes the unresolved text; blank leaves a hole; omit drops it.
        assert_eq!(
            normalize(messy, "alpha2", "iso", "auto", "keep", false, "input", true).unwrap(),
            "US\nAtlantis\nUS\nJP"
        );
        assert_eq!(
            normalize(messy, "alpha2", "iso", "auto", "blank", false, "input", true).unwrap(),
            "US\n\nUS\nJP"
        );
        assert_eq!(
            normalize(messy, "alpha2", "iso", "auto", "omit", false, "input", true).unwrap(),
            "US\nUS\nJP"
        );
        assert_eq!(
            normalize(messy, "alpha2", "iso", "auto", "only", false, "input", true).unwrap(),
            "Atlantis"
        );
        // dedupe collapses different spellings of the same country.
        assert_eq!(
            normalize(messy, "alpha2", "iso", "auto", "omit", true, "input", true).unwrap(),
            "US\nJP"
        );
        // sort orders by the rendered value, not by input order.
        assert_eq!(
            normalize("uk|japan|france", "alpha2", "iso", "pipe", "keep", false, "asc", true).unwrap(),
            "FR\nGB\nJP"
        );
        assert_eq!(
            normalize("uk|japan|france", "alpha2", "iso", "pipe", "keep", false, "desc", true).unwrap(),
            "JP\nGB\nFR"
        );
        // Explicit delimiters.
        assert_eq!(
            normalize("de;fr", "alpha2", "iso", "semicolon", "keep", false, "input", true).unwrap(),
            "DE\nFR"
        );
        assert_eq!(
            normalize("de\tfr", "alpha2", "iso", "tab", "keep", false, "input", true).unwrap(),
            "DE\nFR"
        );
        assert_eq!(
            normalize("de,fr", "alpha2", "iso", "comma", "keep", false, "input", true).unwrap(),
            "DE\nFR"
        );
    }
}
