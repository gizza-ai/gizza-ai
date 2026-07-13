//! gizza-ai/weak-password-detector core — check a password against a bundled,
//! ranked list of the most common and breached passwords, entirely offline.
//!
//! This is a dictionary/blocklist check, NOT a live breach-database lookup: it
//! matches against a fixed, bundled list (no network, no API). A "not found"
//! result is therefore not proof of strength — it only means the password is
//! not one of the well-known common passwords shipped here.

use serde::Serialize;

/// A single detection result.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Detection {
    /// True when the password (or a case/leetspeak variant of it) is on the list.
    pub found: bool,
    /// 1-based rank of the matched entry (1 = most common). `None` when not found.
    pub rank: Option<usize>,
    /// The list entry that matched (its canonical spelling). `None` when not found.
    pub matched_entry: Option<String>,
    /// How it matched: "none", "exact", "case-insensitive", or "leetspeak".
    pub match_kind: String,
    /// Risk band: "critical", "high", "common", or "safe".
    pub severity: String,
    /// Total number of passwords in the bundled list.
    pub list_size: usize,
    /// A human-readable one-line verdict.
    pub message: String,
}

/// The bundled list of common / breached passwords, ordered most-common first.
/// Index 0 is rank 1. Curated from the perennial top entries of public breach
/// compilations (all entries are widely published as "worst passwords").
const COMMON: &[&str] = &[
    "123456", "password", "123456789", "12345678", "12345", "qwerty", "1234567",
    "111111", "1234567890", "123123", "abc123", "1234", "password1", "iloveyou",
    "1q2w3e4r", "000000", "qwerty123", "zaq12wsx", "dragon", "sunshine", "654321",
    "master", "1qaz2wsx", "123321", "666666", "121212", "monkey", "letmein",
    "welcome", "admin", "aa123456", "flower", "qwertyuiop", "123qwe", "loveme",
    "hello", "shadow", "michael", "football", "baseball", "superman", "batman",
    "trustno1", "hottie", "loveyou", "princess", "starwars", "whatever", "passw0rd",
    "222222", "asdfghjkl", "555555", "qazwsx", "7777777", "888888", "999999",
    "112233", "google", "zaq1zaq1", "computer", "michelle", "jessica", "pepper",
    "1111", "131313", "freedom", "777777", "pass", "maggie", "159753", "aaaaaa",
    "ginger", "princess1", "joshua", "cheese", "amanda", "summer", "love", "ashley",
    "nicole", "chelsea", "biteme", "matthew", "access", "yankees", "987654321",
    "dallas", "austin", "thunder", "taylor", "matrix", "mustang", "hunter",
    "buster", "soccer", "harley", "batman1", "andrew", "tigger", "sunshine1",
    "iloveyou1", "fuckyou", "2000", "charlie", "robert", "thomas", "hockey",
    "ranger", "daniel", "starwars1", "klaster", "112358", "george", "computer1",
    "michelle1", "jordan", "jordan23", "pokemon", "abcd1234", "abcdef", "111222",
    "qwer1234", "qwerty1", "password123", "1q2w3e", "1q2w3e4r5t", "q1w2e3r4",
    "1234qwer", "123abc", "asdf1234", "asdfgh", "a1b2c3", "zxcvbn", "zxcvbnm",
    "1qazxsw2", "qazxsw", "poiuytrewq", "asd123", "1233211234567", "789456123",
    "147258369", "159357", "753951", "chocolate", "12341234", "temp", "temp123",
    "test", "test123", "root", "toor", "changeme", "secret", "internet",
    "service", "guest", "oracle", "admin123", "administrator", "passw0rd1",
    "welcome1", "welcome123", "login", "adminadmin", "server", "system", "default",
    "money", "love123", "iloveu", "forever", "hello123", "michael1", "ncc1701",
    "december", "november", "october", "september", "january", "spring", "winter",
    "autumn", "orange", "purple", "yellow", "silver", "banana", "melissa",
    "jennifer", "hannah", "samantha", "jasmine", "tiffany", "kimberly",
    "nathan", "jackson", "anthony", "william", "elizabeth", "brandon", "richard",
    "cookie", "coffee", "phoenix", "gandalf", "master1", "diamond",
    "eagles", "steelers", "rangers", "cowboys", "arsenal", "liverpool", "chelsea1",
    "ferrari", "porsche", "corvette", "mercedes", "harley1", "yamaha", "honda",
    "555666", "00000", "0000000", "11111", "123123123", "121314",
    "abc12345", "letmein1", "p@ssword", "qwe123", "qweasd", "asdasd",
    "1111111", "qqqqqq", "zzzzzz", "asdfasdf", "iloveyou2", "princess123",
    "monkey1", "shadow1", "dragon1", "football1", "baseball1", "superman1",
];

/// Map a character to its leetspeak canonical form (lowercased letters).
fn deleet_char(c: char) -> char {
    match c {
        '0' => 'o',
        '1' | '!' | '|' => 'i',
        '3' => 'e',
        '4' | '@' => 'a',
        '5' | '$' => 's',
        '7' | '+' => 't',
        '8' => 'b',
        '9' => 'g',
        _ => c.to_ascii_lowercase(),
    }
}

/// Normalize a string for leetspeak comparison: lowercase + collapse common
/// digit/symbol substitutions back to letters.
fn normalize(s: &str) -> String {
    s.chars().map(deleet_char).collect()
}

fn severity_for(rank: usize) -> &'static str {
    if rank <= 25 {
        "critical"
    } else if rank <= 100 {
        "high"
    } else {
        "common"
    }
}

/// Check `password` against the bundled common-password list.
///
/// - `case_sensitive`: when false (default), `PASSWORD` matches `password`.
/// - `normalize_leet`: when true (default), leetspeak variants like `P@ssw0rd`
///   match `password`.
pub fn detect(password: &str, case_sensitive: bool, normalize_leet: bool) -> Result<Detection, String> {
    if password.is_empty() {
        return Err("password is empty".into());
    }

    let lower = password.to_lowercase();
    let norm = normalize(password);

    // Priority of a match kind (higher = stronger evidence).
    let priority = |kind: &str| match kind {
        "exact" => 3,
        "case-insensitive" => 2,
        "leetspeak" => 1,
        _ => 0,
    };

    let mut best: Option<(usize, &str, &str)> = None; // (rank, entry, kind)
    for (i, &entry) in COMMON.iter().enumerate() {
        let rank = i + 1;
        let kind = if password == entry {
            "exact"
        } else if !case_sensitive && lower == entry.to_lowercase() {
            "case-insensitive"
        } else if normalize_leet && norm == normalize(entry) {
            "leetspeak"
        } else {
            continue;
        };
        let better = match best {
            None => true,
            Some((brank, _, bkind)) => {
                priority(kind) > priority(bkind)
                    || (priority(kind) == priority(bkind) && rank < brank)
            }
        };
        if better {
            best = Some((rank, entry, kind));
        }
    }

    let list_size = COMMON.len();
    match best {
        Some((rank, entry, kind)) => {
            let severity = severity_for(rank).to_string();
            let message = match kind {
                "exact" => format!(
                    "\u{201c}{entry}\u{201d} is the #{rank} most common password on the bundled list — attackers try these first. Replace it now."
                ),
                "case-insensitive" => format!(
                    "This is just a capitalization variant of \u{201c}{entry}\u{201d} (#{rank} most common). Changing letter case does not help — attackers ignore case. Choose a different password."
                ),
                _ => format!(
                    "This is a leetspeak variant of the very common password \u{201c}{entry}\u{201d} (#{rank} most common). Attackers apply these digit/symbol substitutions automatically, so it offers no real protection."
                ),
            };
            Ok(Detection {
                found: true,
                rank: Some(rank),
                matched_entry: Some(entry.to_string()),
                match_kind: kind.to_string(),
                severity,
                list_size,
                message,
            })
        }
        None => Ok(Detection {
            found: false,
            rank: None,
            matched_entry: None,
            match_kind: "none".to_string(),
            severity: "safe".to_string(),
            list_size,
            message: format!(
                "Not found in the bundled list of {list_size} common/breached passwords. This is not proof of strength — it only rules out the best-known weak passwords. Pair it with a strength/entropy check."
            ),
        }),
    }
}

/// Human-readable, multi-line rendering of a [`detect`] result (used by the
/// page and any surface that wants plain text instead of the JSON struct).
pub fn render(password: &str, case_sensitive: bool, normalize_leet: bool) -> Result<String, String> {
    let d = detect(password, case_sensitive, normalize_leet)?;
    let mut out = String::new();
    if d.found {
        let rank = d.rank.unwrap_or(0);
        let entry = d.matched_entry.as_deref().unwrap_or("");
        out.push_str(&format!("Verdict : WEAK — {}\n", d.severity.to_uppercase()));
        out.push_str(&format!("Match   : {} (\u{201c}{}\u{201d}, rank #{} of {})\n", d.match_kind, entry, rank, d.list_size));
        out.push_str(&format!("\n{}", d.message));
    } else {
        out.push_str("Verdict : not on the common-password list\n");
        out.push_str(&format!("Checked : {} bundled common/breached passwords\n", d.list_size));
        out.push_str(&format!("\n{}", d.message));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_flags_common_password() {
        let out = render("123456", false, true).unwrap();
        assert!(out.contains("WEAK"));
        assert!(out.contains("rank #1"));
        assert!(out.contains("exact"));
    }

    #[test]
    fn render_reports_not_found() {
        let out = render("cor6rect$horse!Battery9Staple", false, true).unwrap();
        assert!(out.contains("not on the common-password list"));
    }

    #[test]
    fn render_empty_is_error() {
        assert!(render("", false, true).is_err());
    }

    #[test]
    fn exact_top_password_flagged_rank_one() {
        let d = detect("123456", false, true).unwrap();
        assert!(d.found);
        assert_eq!(d.rank, Some(1));
        assert_eq!(d.match_kind, "exact");
        assert_eq!(d.severity, "critical");
        assert_eq!(d.matched_entry.as_deref(), Some("123456"));
    }

    #[test]
    fn case_insensitive_match() {
        let d = detect("PASSWORD", false, true).unwrap();
        assert!(d.found);
        assert_eq!(d.match_kind, "case-insensitive");
        assert_eq!(d.matched_entry.as_deref(), Some("password"));
    }

    #[test]
    fn case_sensitive_mode_misses_uppercase() {
        // With case sensitivity on and no leet, PASSWORD is not literally listed.
        let d = detect("PASSWORD", true, false).unwrap();
        assert!(!d.found);
        assert_eq!(d.match_kind, "none");
        assert_eq!(d.severity, "safe");
    }

    #[test]
    fn leetspeak_variant_detected() {
        let d = detect("P@ssw0rd", false, true).unwrap();
        assert!(d.found);
        // Reduces to "password" via @->a, 0->o.
        assert_eq!(d.matched_entry.as_deref(), Some("password"));
        assert!(matches!(d.match_kind.as_str(), "leetspeak" | "exact" | "case-insensitive"));
    }

    #[test]
    fn leetspeak_disabled_misses_variant() {
        let d = detect("l3tm31n", false, false).unwrap();
        assert!(!d.found);
    }

    #[test]
    fn strong_password_not_found() {
        let d = detect("cor6rect$horse!Battery9Staple", false, true).unwrap();
        assert!(!d.found);
        assert_eq!(d.rank, None);
        assert_eq!(d.severity, "safe");
        assert!(d.list_size >= 200);
    }

    #[test]
    fn empty_is_error() {
        assert!(detect("", false, true).is_err());
    }

    #[test]
    fn no_duplicate_entries() {
        let mut seen = std::collections::HashSet::new();
        for &e in COMMON {
            assert!(seen.insert(e), "duplicate entry in COMMON list: {e}");
        }
    }
}
