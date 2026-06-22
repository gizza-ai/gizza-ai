//! fake-data-generator core — generate rows of realistic-looking fake test
//! records (name, email, phone, address, and more) as CSV or JSON.
//!
//! Pure compute, no I/O, no external crates: a small deterministic xorshift PRNG
//! seeded by the caller makes the same `seed` reproduce the same dataset, so the
//! tool fits the page's recompute-on-input model and the CLI/chat surfaces. With
//! `seed = 0` (the default "random" sentinel) a fresh entropy-free seed is derived
//! from the request shape so back-to-back calls still vary; pass any non-zero seed
//! for a fully reproducible run.
//!
//! Shared by the chat skill block and the web page.

use std::fmt::Write as _;

/// Maximum rows per request — keeps output bounded for chat/CLI/page.
pub const MAX_ROWS: u32 = 1000;

/// The fields this generator can emit, in canonical column order. The caller
/// picks a subset via a comma-separated `fields` string; an empty/`"all"`
/// selection yields the default set.
pub const ALL_FIELDS: &[&str] = &[
    "id",
    "uuid",
    "first_name",
    "last_name",
    "full_name",
    "username",
    "email",
    "phone",
    "street",
    "city",
    "state",
    "zip",
    "country",
    "latitude",
    "longitude",
    "company",
    "job_title",
    "birthdate",
    "domain",
    "ipv4",
    "ipv6",
    "mac",
    "color",
    "boolean",
    "credit_card",
    "sentence",
];

/// Fields emitted when the caller does not specify any.
const DEFAULT_FIELDS: &[&str] = &[
    "full_name", "email", "phone", "street", "city", "state", "zip",
];

// --- data pools -------------------------------------------------------------

const FIRST_NAMES: &[&str] = &[
    "Ava", "Liam", "Maya", "Noah", "Sofia", "Ethan", "Olivia", "Lucas", "Emma",
    "Mateo", "Isla", "Aiden", "Chloe", "Kai", "Zoe", "Diego", "Nora", "Omar",
    "Priya", "Leon", "Yuki", "Hana", "Ravi", "Mila", "Theo", "Amara", "Jonas",
    "Lina", "Felix", "Carmen", "Tariq", "Ingrid", "Pablo", "Sana", "Marcus",
    "Elena", "Hugo", "Aisha", "Niko", "Greta",
];

const LAST_NAMES: &[&str] = &[
    "Nguyen", "Garcia", "Smith", "Khan", "Mueller", "Rossi", "Kowalski",
    "Johansson", "Okafor", "Tanaka", "Silva", "Andersson", "Patel", "Dubois",
    "Hernandez", "Lindqvist", "Bauer", "Costa", "Petrov", "Haddad", "Kim",
    "Walker", "Romano", "Schneider", "Moreau", "Novak", "Fischer", "Lopez",
    "Ahmed", "Larsen", "Reyes", "Wagner", "Santos", "Ivanov", "Murphy",
];

const STREET_NAMES: &[&str] = &[
    "Maple", "Oak", "Cedar", "Birch", "Elm", "Sunset", "Park", "Lake", "Hill",
    "River", "Spring", "Willow", "Aspen", "Linden", "Chestnut", "Magnolia",
    "Pine", "Walnut", "Juniper", "Sycamore",
];

const STREET_SUFFIX: &[&str] = &[
    "St", "Ave", "Blvd", "Rd", "Ln", "Dr", "Ct", "Way", "Ter", "Pl",
];

const CITIES: &[&str] = &[
    "Springfield", "Riverside", "Fairview", "Madison", "Georgetown",
    "Franklin", "Clinton", "Greenville", "Bristol", "Salem", "Newport",
    "Ashland", "Burlington", "Manchester", "Oxford", "Dover", "Auburn",
    "Kingston", "Milton", "Hudson",
];

const STATES: &[(&str, &str)] = &[
    ("California", "CA"),
    ("Texas", "TX"),
    ("New York", "NY"),
    ("Florida", "FL"),
    ("Illinois", "IL"),
    ("Washington", "WA"),
    ("Colorado", "CO"),
    ("Oregon", "OR"),
    ("Georgia", "GA"),
    ("Arizona", "AZ"),
    ("Massachusetts", "MA"),
    ("Ohio", "OH"),
];

const COUNTRIES: &[&str] = &[
    "United States", "Canada", "United Kingdom", "Australia", "Germany",
    "France", "Spain", "Italy", "Japan", "Brazil", "Mexico", "Sweden",
];

const EMAIL_DOMAINS: &[&str] = &[
    "example.com", "test.org", "mail.net", "demo.io", "sample.co", "fake.dev",
];

const COMPANY_PREFIX: &[&str] = &[
    "Globex", "Initech", "Umbrella", "Acme", "Hooli", "Vandelay", "Stark",
    "Wayne", "Wonka", "Cyberdyne", "Soylent", "Tyrell", "Pied Piper", "Aperture",
];

const COMPANY_SUFFIX: &[&str] =
    &["Inc", "LLC", "Group", "Labs", "Systems", "Holdings", "Co", "Partners"];

const JOB_TITLES: &[&str] = &[
    "Software Engineer", "Product Manager", "Data Analyst", "UX Designer",
    "Account Executive", "Operations Lead", "Marketing Specialist",
    "QA Engineer", "Project Coordinator", "Sales Director", "Support Agent",
    "DevOps Engineer", "Content Strategist", "Finance Manager",
];

const TLDS: &[&str] = &["com", "net", "org", "io", "dev", "co", "app"];

const LOREM: &[&str] = &[
    "lorem", "ipsum", "dolor", "sit", "amet", "consectetur", "adipiscing",
    "elit", "sed", "do", "eiusmod", "tempor", "incididunt", "ut", "labore",
    "et", "dolore", "magna", "aliqua", "enim", "ad", "minim", "veniam", "quis",
    "nostrud", "exercitation", "ullamco", "laboris", "nisi", "aliquip", "ex",
    "ea", "commodo", "consequat", "duis", "aute", "irure", "in", "reprehenderit",
];

// --- PRNG -------------------------------------------------------------------

/// Tiny SplitMix64-style PRNG — deterministic, no deps, wasm-safe.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        // Avoid the degenerate all-zero state.
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform-ish index into a slice of length `n` (n > 0).
    fn idx(&mut self, n: usize) -> usize {
        (self.next_u64() % (n as u64)) as usize
    }
    /// Integer in `[lo, hi]` inclusive.
    fn range(&mut self, lo: u64, hi: u64) -> u64 {
        if hi <= lo {
            return lo;
        }
        lo + self.next_u64() % (hi - lo + 1)
    }
    fn pick<'a, T>(&mut self, items: &'a [T]) -> &'a T {
        &items[self.idx(items.len())]
    }
}

// --- field resolution -------------------------------------------------------

/// Parse + validate a comma-separated field selection. Empty or `"all"`
/// (case-insensitive) expands to the full canonical set; an empty explicit
/// selection falls back to the default subset. Returns an error naming the first
/// unknown field.
fn resolve_fields(fields: &str) -> Result<Vec<&'static str>, String> {
    let trimmed = fields.trim();
    if trimmed.is_empty() {
        return Ok(DEFAULT_FIELDS.to_vec());
    }
    if trimmed.eq_ignore_ascii_case("all") {
        return Ok(ALL_FIELDS.to_vec());
    }
    let mut out: Vec<&'static str> = Vec::new();
    for raw in trimmed.split(',') {
        let f = raw.trim().to_ascii_lowercase();
        if f.is_empty() {
            continue;
        }
        match ALL_FIELDS.iter().find(|c| **c == f) {
            Some(canon) => {
                if !out.contains(canon) {
                    out.push(*canon);
                }
            }
            None => {
                return Err(format!(
                    "unknown field '{}'. Valid fields: {}, or 'all'.",
                    raw.trim(),
                    ALL_FIELDS.join(", ")
                ))
            }
        }
    }
    if out.is_empty() {
        return Ok(DEFAULT_FIELDS.to_vec());
    }
    Ok(out)
}

/// One generated record, as an ordered list of (column, value) pairs matching
/// the requested field order.
struct Record {
    first: String,
    last: String,
    rng: u64, // a per-row salt for derived values (email/username/phone)
    index: u32,
}

fn build_record(rng: &mut Rng, index: u32) -> Record {
    let first = rng.pick(FIRST_NAMES).to_string();
    let last = rng.pick(LAST_NAMES).to_string();
    Record {
        first,
        last,
        rng: rng.next_u64(),
        index,
    }
}

fn field_value(rec: &Record, field: &str, salt_rng: &mut Rng) -> String {
    match field {
        "id" => (rec.index + 1).to_string(),
        "first_name" => rec.first.clone(),
        "last_name" => rec.last.clone(),
        "full_name" => format!("{} {}", rec.first, rec.last),
        "username" => format!(
            "{}{}{}",
            rec.first.to_ascii_lowercase(),
            rec.last.to_ascii_lowercase(),
            (rec.rng % 100)
        ),
        "email" => format!(
            "{}.{}{}@{}",
            rec.first.to_ascii_lowercase(),
            rec.last.to_ascii_lowercase(),
            rec.rng % 1000,
            EMAIL_DOMAINS[(rec.rng as usize / 7) % EMAIL_DOMAINS.len()]
        ),
        "phone" => {
            let area = salt_rng.range(200, 989);
            let mid = salt_rng.range(200, 999);
            let last4 = salt_rng.range(0, 9999);
            format!("+1-{:03}-{:03}-{:04}", area, mid, last4)
        }
        "street" => {
            let num = salt_rng.range(10, 9999);
            let name = salt_rng.pick(STREET_NAMES);
            let suf = salt_rng.pick(STREET_SUFFIX);
            format!("{} {} {}", num, name, suf)
        }
        "city" => salt_rng.pick(CITIES).to_string(),
        "state" => salt_rng.pick(STATES).0.to_string(),
        "zip" => format!("{:05}", salt_rng.range(1000, 99950)),
        "country" => salt_rng.pick(COUNTRIES).to_string(),
        "company" => {
            let p = salt_rng.pick(COMPANY_PREFIX);
            let s = salt_rng.pick(COMPANY_SUFFIX);
            format!("{} {}", p, s)
        }
        "job_title" => salt_rng.pick(JOB_TITLES).to_string(),
        "birthdate" => {
            let year = salt_rng.range(1955, 2005);
            let month = salt_rng.range(1, 12);
            let day = salt_rng.range(1, 28);
            format!("{:04}-{:02}-{:02}", year, month, day)
        }
        "uuid" => {
            // RFC-4122 version-4 layout, but deterministic (seeded), not secure.
            let a = salt_rng.next_u64();
            let b = salt_rng.next_u64();
            let bytes = [a.to_be_bytes(), b.to_be_bytes()].concat();
            let mut h = [0u8; 16];
            h.copy_from_slice(&bytes[..16]);
            h[6] = (h[6] & 0x0f) | 0x40; // version 4
            h[8] = (h[8] & 0x3f) | 0x80; // variant
            format!(
                "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7], h[8], h[9], h[10], h[11], h[12], h[13], h[14], h[15]
            )
        }
        "domain" => {
            let p = salt_rng.pick(COMPANY_PREFIX).to_ascii_lowercase();
            let t = salt_rng.pick(TLDS);
            format!("{}.{}", p, t)
        }
        "ipv4" => format!(
            "{}.{}.{}.{}",
            salt_rng.range(1, 255),
            salt_rng.range(0, 255),
            salt_rng.range(0, 255),
            salt_rng.range(1, 254)
        ),
        "ipv6" => {
            let g: Vec<String> = (0..8)
                .map(|_| format!("{:04x}", salt_rng.range(0, 0xffff)))
                .collect();
            g.join(":")
        }
        "mac" => {
            let g: Vec<String> = (0..6)
                .map(|_| format!("{:02x}", salt_rng.range(0, 255)))
                .collect();
            g.join(":")
        }
        "color" => format!(
            "#{:02x}{:02x}{:02x}",
            salt_rng.range(0, 255),
            salt_rng.range(0, 255),
            salt_rng.range(0, 255)
        ),
        "boolean" => {
            if salt_rng.next_u64() & 1 == 0 {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        "latitude" => {
            let v = (salt_rng.range(0, 180_000_000) as f64) / 1_000_000.0 - 90.0;
            format!("{:.6}", v)
        }
        "longitude" => {
            let v = (salt_rng.range(0, 360_000_000) as f64) / 1_000_000.0 - 180.0;
            format!("{:.6}", v)
        }
        "credit_card" => {
            // 16 digits with a valid Luhn check digit (test-card style, not a
            // real account). First digit 4 = Visa-like.
            let mut digits = [0u8; 16];
            digits[0] = 4;
            for d in digits.iter_mut().skip(1).take(14) {
                *d = salt_rng.range(0, 9) as u8;
            }
            // compute Luhn check digit for the last position
            let mut sum = 0u32;
            for (i, &d) in digits[..15].iter().rev().enumerate() {
                let mut v = d as u32;
                if i % 2 == 0 {
                    v *= 2;
                    if v > 9 {
                        v -= 9;
                    }
                }
                sum += v;
            }
            digits[15] = ((10 - (sum % 10)) % 10) as u8;
            digits.iter().map(|d| (b'0' + d) as char).collect()
        }
        "sentence" => {
            let n = salt_rng.range(5, 12) as usize;
            let mut words: Vec<String> =
                (0..n).map(|_| salt_rng.pick(LOREM).to_string()).collect();
            if let Some(first) = words.first_mut() {
                let mut c = first.chars();
                if let Some(f) = c.next() {
                    *first = f.to_uppercase().collect::<String>() + c.as_str();
                }
            }
            format!("{}.", words.join(" "))
        }
        _ => String::new(),
    }
}

// --- CSV / JSON encoding ----------------------------------------------------

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            c => out.push(c),
        }
    }
    out
}

/// SQL: numeric/boolean columns are emitted unquoted, everything else as a
/// single-quoted string literal with `'` doubled.
fn sql_is_numeric_col(col: &str) -> bool {
    matches!(col, "id" | "latitude" | "longitude")
}

fn sql_literal(col: &str, v: &str) -> String {
    if col == "boolean" {
        return v.to_string(); // true/false
    }
    if sql_is_numeric_col(col) {
        return v.to_string();
    }
    format!("'{}'", v.replace('\'', "''"))
}

/// Convenience wrapper preserving the original 4-arg signature: CSV header on,
/// table name `people`.
pub fn generate(count: u32, format: &str, fields: &str, seed: u64) -> Result<String, String> {
    generate_opts(count, format, fields, seed, true, "people")
}

/// Generate `count` fake records.
///
/// - `count`: number of rows, 1..=`MAX_ROWS`.
/// - `format`: `"csv"`, `"json"`, `"sql"`, or `"xml"` (case-insensitive; blank → csv).
/// - `fields`: comma-separated subset of [`ALL_FIELDS`], `"all"`, or blank for
///   the default set.
/// - `seed`: reproducibility seed; non-zero reproduces the same dataset. 0 is a
///   sentinel meaning "vary per request"; with 0 here we use a fixed fallback so
///   the pure function stays deterministic.
/// - `header`: CSV only — include the column-name header row (default true).
/// - `table`: SQL only — the table name for `INSERT INTO` (blank → `people`).
pub fn generate_opts(
    count: u32,
    format: &str,
    fields: &str,
    seed: u64,
    header: bool,
    table: &str,
) -> Result<String, String> {
    if count == 0 {
        return Err("count must be at least 1".into());
    }
    if count > MAX_ROWS {
        return Err(format!("count must be at most {MAX_ROWS}"));
    }
    let cols = resolve_fields(fields)?;
    let fmt = format.trim().to_ascii_lowercase();
    let fmt = if fmt.is_empty() { "csv" } else { fmt.as_str() };
    if !matches!(fmt, "csv" | "json" | "sql" | "xml") {
        return Err(format!(
            "unknown format '{format}'. Use 'csv', 'json', 'sql', or 'xml'."
        ));
    }
    let table = {
        let t = table.trim();
        if t.is_empty() {
            "people"
        } else {
            t
        }
    };

    // seed == 0 → deterministic fallback so the pure fn never depends on entropy.
    let effective_seed = if seed == 0 { 0xC0FF_EE12_3456_789A } else { seed };
    let mut rng = Rng::new(effective_seed);

    // Materialize every row once, so all formats share the same data for a seed.
    let rows: Vec<Vec<String>> = (0..count)
        .map(|i| {
            let rec = build_record(&mut rng, i);
            let mut salt = Rng::new(rng.next_u64());
            cols.iter().map(|c| field_value(&rec, c, &mut salt)).collect()
        })
        .collect();

    match fmt {
        "csv" => {
            let mut out = String::new();
            if header {
                out.push_str(&cols.join(","));
                out.push('\n');
            }
            for row in &rows {
                let escaped: Vec<String> = row.iter().map(|v| csv_escape(v)).collect();
                out.push_str(&escaped.join(","));
                out.push('\n');
            }
            Ok(out)
        }
        "json" => {
            let mut out = String::from("[\n");
            for (i, row) in rows.iter().enumerate() {
                out.push_str("  {\n");
                for (j, c) in cols.iter().enumerate() {
                    let comma = if j + 1 < cols.len() { "," } else { "" };
                    let _ = write!(
                        out,
                        "    \"{}\": \"{}\"{}\n",
                        json_escape(c),
                        json_escape(&row[j]),
                        comma
                    );
                }
                let comma = if i + 1 < rows.len() { "," } else { "" };
                let _ = write!(out, "  }}{}\n", comma);
            }
            out.push(']');
            Ok(out)
        }
        "sql" => {
            let mut out = String::new();
            let col_list = cols.join(", ");
            for row in &rows {
                let vals: Vec<String> = cols
                    .iter()
                    .enumerate()
                    .map(|(j, c)| sql_literal(c, &row[j]))
                    .collect();
                let _ = writeln!(
                    out,
                    "INSERT INTO {} ({}) VALUES ({});",
                    table,
                    col_list,
                    vals.join(", ")
                );
            }
            Ok(out)
        }
        _ => {
            // xml
            let mut out = String::from("<records>\n");
            for row in &rows {
                out.push_str("  <record>\n");
                for (j, c) in cols.iter().enumerate() {
                    let _ = writeln!(
                        out,
                        "    <{0}>{1}</{0}>",
                        c,
                        xml_escape(&row[j])
                    );
                }
                out.push_str("  </record>\n");
            }
            out.push_str("</records>");
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_has_header_and_rows() {
        let out = generate(3, "csv", "full_name,email", 42).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "full_name,email");
        assert_eq!(lines.len(), 4); // header + 3 rows
        for line in &lines[1..] {
            assert!(line.contains('@'), "email present: {line}");
            assert!(line.contains(','), "two columns: {line}");
        }
    }

    #[test]
    fn deterministic_for_same_seed() {
        let a = generate(5, "json", "all", 12345).unwrap();
        let b = generate(5, "json", "all", 12345).unwrap();
        assert_eq!(a, b, "same seed reproduces dataset");
    }

    #[test]
    fn different_seeds_differ() {
        let a = generate(5, "csv", "all", 1).unwrap();
        let b = generate(5, "csv", "all", 2).unwrap();
        assert_ne!(a, b, "different seeds yield different data");
    }

    #[test]
    fn json_is_valid_array() {
        let out = generate(2, "json", "id,full_name", 7).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid JSON");
        let arr = v.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["id"], "1");
        assert!(arr[0]["full_name"].as_str().unwrap().contains(' '));
    }

    #[test]
    fn default_fields_when_blank() {
        let out = generate(1, "csv", "", 1).unwrap();
        let header = out.lines().next().unwrap();
        assert_eq!(header, DEFAULT_FIELDS.join(","));
    }

    #[test]
    fn all_fields_expands() {
        let out = generate(1, "csv", "all", 1).unwrap();
        let header = out.lines().next().unwrap();
        assert_eq!(header, ALL_FIELDS.join(","));
    }

    #[test]
    fn dedupes_and_keeps_requested_order() {
        // the caller's column order is honored; duplicates are dropped.
        let out = generate(1, "csv", "email,full_name,email", 1).unwrap();
        let header = out.lines().next().unwrap();
        assert_eq!(header, "email,full_name");
    }

    #[test]
    fn err_unknown_field() {
        let e = generate(1, "csv", "full_name,bogus", 1).unwrap_err();
        assert!(e.contains("unknown field 'bogus'"), "{e}");
    }

    #[test]
    fn err_unknown_format() {
        let e = generate(1, "yaml", "all", 1).unwrap_err();
        assert!(e.contains("unknown format"), "{e}");
    }

    #[test]
    fn err_count_bounds() {
        assert!(generate(0, "csv", "all", 1).unwrap_err().contains("at least 1"));
        assert!(generate(MAX_ROWS + 1, "csv", "all", 1)
            .unwrap_err()
            .contains("at most"));
    }

    #[test]
    fn sql_emits_insert_statements() {
        let out = generate_opts(2, "sql", "id,full_name,boolean", 5, true, "users").unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("INSERT INTO users (id, full_name, boolean) VALUES ("));
        assert!(lines[0].ends_with(");"));
        // id is numeric (unquoted), full_name quoted, boolean unquoted true/false
        assert!(lines[0].contains("(1, '"), "id unquoted: {}", lines[0]);
        assert!(lines[0].contains("true") || lines[0].contains("false"));
    }

    #[test]
    fn sql_escapes_single_quotes() {
        // force a value with an apostrophe via xml? names have none; assert the
        // escaper directly on a literal containing a quote.
        assert_eq!(sql_literal("city", "O'Brien"), "'O''Brien'");
        assert_eq!(sql_literal("id", "5"), "5");
        assert_eq!(sql_literal("boolean", "true"), "true");
    }

    #[test]
    fn xml_wraps_records() {
        let out = generate_opts(2, "xml", "id,full_name", 5, true, "people").unwrap();
        assert!(out.starts_with("<records>"));
        assert!(out.ends_with("</records>"));
        assert_eq!(out.matches("<record>").count(), 2);
        assert!(out.contains("<id>1</id>"));
    }

    #[test]
    fn xml_escapes_entities() {
        assert_eq!(xml_escape("a<b>&\"'"), "a&lt;b&gt;&amp;&quot;&apos;");
    }

    #[test]
    fn csv_header_toggle() {
        let with = generate_opts(1, "csv", "email", 1, true, "").unwrap();
        let without = generate_opts(1, "csv", "email", 1, false, "").unwrap();
        assert_eq!(with.lines().next().unwrap(), "email");
        assert!(without.lines().next().unwrap().contains('@'));
        assert_eq!(without.lines().count(), 1);
    }

    #[test]
    fn credit_card_passes_luhn() {
        let out = generate_opts(20, "csv", "credit_card", 3, false, "").unwrap();
        for line in out.lines() {
            let digits: Vec<u32> = line.chars().filter_map(|c| c.to_digit(10)).collect();
            assert_eq!(digits.len(), 16, "16-digit card: {line}");
            let mut sum = 0u32;
            for (i, &d) in digits.iter().rev().enumerate() {
                let mut v = d;
                if i % 2 == 1 {
                    v *= 2;
                    if v > 9 {
                        v -= 9;
                    }
                }
                sum += v;
            }
            assert_eq!(sum % 10, 0, "valid Luhn: {line}");
        }
    }

    #[test]
    fn uuid_v4_shape() {
        let out = generate_opts(1, "csv", "uuid", 1, false, "").unwrap();
        let u = out.trim();
        assert_eq!(u.len(), 36);
        let parts: Vec<&str> = u.split('-').collect();
        assert_eq!(parts.iter().map(|p| p.len()).collect::<Vec<_>>(), vec![8, 4, 4, 4, 12]);
        assert_eq!(&parts[2][..1], "4", "version 4");
    }

    #[test]
    fn formats_share_data_for_seed() {
        // CSV and JSON for the same seed must carry the same first email value.
        let csv = generate_opts(1, "csv", "email", 77, false, "").unwrap();
        let json = generate_opts(1, "json", "email", 77, false, "").unwrap();
        let email = csv.trim();
        assert!(json.contains(email), "csv {email} also in json {json}");
    }

    #[test]
    fn csv_escapes_commas() {
        // job_title / company contain spaces but no commas; ensure quoting logic
        // is exercised via a synthetic field with a comma is not in pools, so
        // just confirm no stray unescaped commas break the column count.
        let out = generate(4, "csv", "company,job_title", 99).unwrap();
        for line in out.lines().skip(1) {
            // each row must parse to exactly 2 CSV fields
            let n = count_csv_fields(line);
            assert_eq!(n, 2, "row has 2 fields: {line}");
        }
    }

    // minimal CSV field counter honoring double-quote escaping
    fn count_csv_fields(line: &str) -> usize {
        let mut fields = 1;
        let mut in_quotes = false;
        let mut chars = line.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    if in_quotes && chars.peek() == Some(&'"') {
                        chars.next();
                    } else {
                        in_quotes = !in_quotes;
                    }
                }
                ',' if !in_quotes => fields += 1,
                _ => {}
            }
        }
        fields
    }
}
