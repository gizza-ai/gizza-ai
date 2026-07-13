//! vcard-deduplicate core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps. Finds duplicate contacts in a vCard
//! (.vcf) file by matching on names, emails, and/or phone numbers, then either
//! merges each duplicate group into one card (unioning phones/emails/etc.) or
//! removes the extra copies. Preserves each card's original property lines and
//! handles vCard 2.1, 3.0 (RFC 2426), and 4.0 (RFC 6350), including folded
//! continuation lines and multiple cards in one file.

use std::collections::{HashMap, HashSet};

/// Which signals count when deciding two cards are the same contact.
pub enum MatchBy {
    /// Cards are duplicates if they share a normalized name OR any email OR any phone.
    Any,
    /// Match only on the normalized full name (FN, else the joined N components).
    Name,
    /// Match only on a shared email address (case-insensitive).
    Email,
    /// Match only on a shared phone number (compared by its digits).
    Phone,
}

impl MatchBy {
    pub fn parse(s: &str) -> Result<MatchBy, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "any" => Ok(MatchBy::Any),
            "name" => Ok(MatchBy::Name),
            "email" => Ok(MatchBy::Email),
            "phone" => Ok(MatchBy::Phone),
            other => Err(format!(
                "unknown match_by '{other}' (use 'any', 'name', 'email', or 'phone')"
            )),
        }
    }
    fn uses_name(&self) -> bool {
        matches!(self, MatchBy::Any | MatchBy::Name)
    }
    fn uses_email(&self) -> bool {
        matches!(self, MatchBy::Any | MatchBy::Email)
    }
    fn uses_phone(&self) -> bool {
        matches!(self, MatchBy::Any | MatchBy::Phone)
    }
}

/// One content line kept verbatim (`raw`) plus the parsed pieces we key on.
struct Line {
    /// Upper-cased property name, group prefix stripped (e.g. "EMAIL", "TEL").
    name: String,
    /// The raw, still-escaped value after the first unquoted ':'.
    value: String,
    /// The full original line `head:value`, re-emitted as-is.
    raw: String,
}

/// Property names kept only once per merged card (the survivor's value wins);
/// everything else (EMAIL, TEL, URL, ADR, IMPP, CATEGORIES, X-*, …) is additive.
fn is_singular(name: &str) -> bool {
    matches!(
        name,
        "VERSION"
            | "FN"
            | "N"
            | "NICKNAME"
            | "BDAY"
            | "ANNIVERSARY"
            | "GENDER"
            | "KIND"
            | "PRODID"
            | "REV"
            | "UID"
            | "ORG"
            | "TITLE"
            | "ROLE"
            | "NOTE"
    )
}

/// Deduplicate the vCard document.
///
/// - `match_by`: which identifiers make two cards the same contact.
/// - `merge`: true → merge each duplicate group into one card, unioning repeatable
///   properties and filling empty singular fields from later copies; false → keep
///   only the first card of each group and drop the rest.
pub fn run(input: &str, match_by: MatchBy, merge: bool) -> Result<String, String> {
    let logical = unfold(input);
    let cards = split_cards(&logical)?;
    let n = cards.len();

    // Union-find over card indices; unite any two cards that share a match key.
    let mut parent: Vec<usize> = (0..n).collect();

    // Map each key (kind, value) to the first card index that presented it; a later
    // hit unions the two cards.
    let mut seen: HashMap<(u8, String), usize> = HashMap::new();
    for (i, card) in cards.iter().enumerate() {
        for (kind, key) in card_keys(card, &match_by) {
            match seen.entry((kind, key)) {
                std::collections::hash_map::Entry::Occupied(e) => union(&mut parent, i, *e.get()),
                std::collections::hash_map::Entry::Vacant(e) => {
                    e.insert(i);
                }
            }
        }
    }

    // Gather groups in first-appearance order of their root card.
    let mut order: Vec<usize> = Vec::new();
    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        groups.entry(r).or_default().push(i);
        if r == i {
            order.push(i);
        }
    }

    let mut out = String::new();
    for root in order {
        let members = &groups[&root];
        let card_lines: Vec<String> = if merge {
            merge_group(members, &cards)
        } else {
            // Keep only the first (lowest-index) member verbatim.
            let first = *members.iter().min().unwrap();
            cards[first].iter().map(|l| l.raw.clone()).collect()
        };
        out.push_str("BEGIN:VCARD\n");
        for raw in &card_lines {
            out.push_str(raw);
            out.push('\n');
        }
        out.push_str("END:VCARD\n");
    }
    Ok(out)
}

fn find(parent: &mut [usize], mut x: usize) -> usize {
    while parent[x] != x {
        parent[x] = parent[parent[x]];
        x = parent[x];
    }
    x
}

fn union(parent: &mut [usize], a: usize, b: usize) {
    let ra = find(parent, a);
    let rb = find(parent, b);
    if ra != rb {
        // Keep the lower index as root so groups follow first-appearance order.
        if ra < rb {
            parent[rb] = ra;
        } else {
            parent[ra] = rb;
        }
    }
}

/// Merge a group of cards (given by index) into one list of raw property lines: the
/// survivor's lines, then each other card's repeatable values not already present
/// and any singular fields the survivor lacked.
fn merge_group(members: &[usize], cards: &[Vec<Line>]) -> Vec<String> {
    let mut idxs: Vec<usize> = members.to_vec();
    idxs.sort_unstable();
    let base = idxs[0];

    let mut lines: Vec<String> = Vec::new();
    let mut singular_present: HashSet<String> = HashSet::new();
    let mut value_present: HashSet<(String, String)> = HashSet::new();

    for &i in &idxs {
        let is_base = i == base;
        for l in &cards[i] {
            if is_singular(&l.name) {
                // The survivor keeps its own singular lines; later cards only fill gaps.
                let is_new = singular_present.insert(l.name.clone());
                if is_base || is_new {
                    lines.push(l.raw.clone());
                }
            } else {
                let key = (l.name.clone(), value_key(&l.name, &l.value));
                let is_new = value_present.insert(key);
                // The survivor keeps every one of its own lines (even repeats); later
                // cards contribute only values not already present.
                if is_base || is_new {
                    lines.push(l.raw.clone());
                }
            }
        }
    }
    lines
}

/// The match keys a card contributes, tagged by kind (0=name, 1=email, 2=phone).
fn card_keys(card: &[Line], match_by: &MatchBy) -> Vec<(u8, String)> {
    let mut keys = Vec::new();
    if match_by.uses_name() {
        if let Some(k) = name_key(card) {
            keys.push((0u8, k));
        }
    }
    if match_by.uses_email() {
        for l in card.iter().filter(|l| l.name == "EMAIL") {
            let k = email_key(&l.value);
            if !k.is_empty() {
                keys.push((1u8, k));
            }
        }
    }
    if match_by.uses_phone() {
        for l in card.iter().filter(|l| l.name == "TEL") {
            let k = phone_key(&l.value);
            if !k.is_empty() {
                keys.push((2u8, k));
            }
        }
    }
    keys
}

/// Normalized full-name key: the FN value if present, else the N components joined
/// by spaces. Lower-cased with collapsed whitespace. None if there is no name.
fn name_key(card: &[Line]) -> Option<String> {
    let raw = if let Some(l) = card.iter().find(|l| l.name == "FN") {
        unescape(&l.value)
    } else if let Some(l) = card.iter().find(|l| l.name == "N") {
        unescape(&l.value).replace(';', " ")
    } else {
        return None;
    };
    let key = normalize_ws(&raw).to_ascii_lowercase();
    if key.is_empty() {
        None
    } else {
        Some(key)
    }
}

/// Collapse runs of whitespace to single spaces and trim.
fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Case-insensitive, trimmed email key.
fn email_key(value: &str) -> String {
    unescape(value).trim().to_ascii_lowercase()
}

/// Phone key = just the digits, so `+1 (555) 123-4567` and `5551234567` collide
/// only when their full digit runs match.
fn phone_key(value: &str) -> String {
    unescape(value)
        .chars()
        .filter(|c| c.is_ascii_digit())
        .collect()
}

/// Value-dedup key used when unioning repeatable properties inside a merged card.
fn value_key(name: &str, value: &str) -> String {
    match name {
        "EMAIL" => email_key(value),
        "TEL" => phone_key(value),
        _ => normalize_ws(&unescape(value)).to_ascii_lowercase(),
    }
}

/// RFC 6350 §3.2 line unfolding: normalize newlines, then a line beginning with a
/// single space or tab continues the previous line (that whitespace char is dropped).
fn unfold(input: &str) -> Vec<String> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<String> = Vec::new();
    for raw in normalized.split('\n') {
        if let Some(rest) = raw.strip_prefix(' ').or_else(|| raw.strip_prefix('\t')) {
            if let Some(last) = out.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        out.push(raw.to_string());
    }
    out
}

/// Split logical lines into cards delimited by BEGIN:VCARD / END:VCARD
/// (case-insensitive). Errors if no card is found.
fn split_cards(lines: &[String]) -> Result<Vec<Vec<Line>>, String> {
    let mut cards: Vec<Vec<Line>> = Vec::new();
    let mut current: Option<Vec<Line>> = None;
    for raw in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_ascii_uppercase();
        if upper == "BEGIN:VCARD" {
            current = Some(Vec::new());
            continue;
        }
        if upper == "END:VCARD" {
            if let Some(card) = current.take() {
                cards.push(card);
            }
            continue;
        }
        if let Some(card) = current.as_mut() {
            if let Some(line) = parse_line(trimmed) {
                card.push(line);
            }
        }
    }
    if cards.is_empty() {
        return Err(
            "no vCard found: expected at least one 'BEGIN:VCARD ... END:VCARD' block".into(),
        );
    }
    Ok(cards)
}

/// Parse one content line into its property name + value while keeping the raw text.
fn parse_line(line: &str) -> Option<Line> {
    let colon = find_unquoted_colon(line)?;
    let (head, rest) = line.split_at(colon);
    let value = rest[1..].to_string();

    let name_tok = head.split(';').next().unwrap_or(head);
    let name = name_tok
        .rsplit('.')
        .next()
        .unwrap_or(name_tok)
        .trim()
        .to_ascii_uppercase();
    if name.is_empty() {
        return None;
    }
    Some(Line {
        name,
        value,
        raw: line.to_string(),
    })
}

/// Find the first ':' not inside a double-quoted param value.
fn find_unquoted_colon(s: &str) -> Option<usize> {
    let mut in_quote = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            ':' if !in_quote => return Some(i),
            _ => {}
        }
    }
    None
}

/// Unescape a vCard TEXT value: `\n`/`\N` → newline, `\,` → `,`, `\;` → `;`,
/// `\\` → `\`. Any other `\x` becomes `x`.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dedup(input: &str, match_by: &str, merge: bool) -> String {
        run(input, MatchBy::parse(match_by).unwrap(), merge).unwrap()
    }

    fn count_cards(s: &str) -> usize {
        s.matches("BEGIN:VCARD").count()
    }

    #[test]
    fn merge_same_name_unions_phones_and_emails() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:John Doe\nEMAIL:john@work.com\nTEL:+1-555-111-2222\nEND:VCARD\n\
                   BEGIN:VCARD\nVERSION:3.0\nFN:John Doe\nEMAIL:john@home.com\nTEL:1 (555) 111-2222\nEND:VCARD\n";
        let out = dedup(vcf, "any", true);
        assert_eq!(count_cards(&out), 1, "two John Doe cards should merge:\n{out}");
        // Both distinct emails survive.
        assert!(out.contains("john@work.com"), "{out}");
        assert!(out.contains("john@home.com"), "{out}");
        // The two phones share the same digits, so only the survivor's remains.
        assert_eq!(
            out.matches("TEL").count(),
            1,
            "duplicate phone should collapse:\n{out}"
        );
        assert!(out.contains("+1-555-111-2222"));
    }

    #[test]
    fn shared_email_links_different_names() {
        // Different display names, same email → one contact under match_by=any.
        let vcf = "BEGIN:VCARD\nVERSION:4.0\nFN:Ada L.\nEMAIL:ada@ex.com\nEND:VCARD\n\
                   BEGIN:VCARD\nVERSION:4.0\nFN:Ada Lovelace\nEMAIL:ADA@EX.COM\nEND:VCARD\n";
        let out = dedup(vcf, "any", true);
        assert_eq!(count_cards(&out), 1, "shared email should link them:\n{out}");
        // Survivor keeps the first card's FN; the second FN is a singular dup, dropped.
        assert!(out.contains("FN:Ada L."), "{out}");
        assert_eq!(out.matches("FN:").count(), 1, "second FN should drop:\n{out}");
    }

    #[test]
    fn match_by_name_ignores_shared_email() {
        let vcf = "BEGIN:VCARD\nVERSION:4.0\nFN:Ada L.\nEMAIL:ada@ex.com\nEND:VCARD\n\
                   BEGIN:VCARD\nVERSION:4.0\nFN:Grace H.\nEMAIL:ada@ex.com\nEND:VCARD\n";
        // Same email but match_by=name → distinct names stay separate.
        let out = dedup(vcf, "name", true);
        assert_eq!(count_cards(&out), 2, "name-only match must not merge:\n{out}");
    }

    #[test]
    fn merge_false_drops_extra_copies() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:John Doe\nEMAIL:john@work.com\nEND:VCARD\n\
                   BEGIN:VCARD\nVERSION:3.0\nFN:John Doe\nEMAIL:john@home.com\nEND:VCARD\n";
        let out = dedup(vcf, "any", false);
        assert_eq!(count_cards(&out), 1, "{out}");
        // Only the first card is kept; the second card's unique email is NOT merged in.
        assert!(out.contains("john@work.com"), "{out}");
        assert!(
            !out.contains("john@home.com"),
            "merge=false should drop the extra copy:\n{out}"
        );
    }

    #[test]
    fn distinct_contacts_are_untouched() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:Alice\nEMAIL:a@ex.com\nTEL:111\nEND:VCARD\n\
                   BEGIN:VCARD\nVERSION:3.0\nFN:Bob\nEMAIL:b@ex.com\nTEL:222\nEND:VCARD\n";
        let out = dedup(vcf, "any", true);
        assert_eq!(count_cards(&out), 2, "{out}");
        assert!(out.contains("FN:Alice") && out.contains("FN:Bob"));
    }

    #[test]
    fn merge_fills_missing_singular_and_folds_input() {
        // First card lacks ORG; folded ORG line on the second fills the gap.
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:Jane Roe\nTEL:+1 555 000\nEND:VCARD\n\
                   BEGIN:VCARD\nVERSION:3.0\nFN:Jane Roe\nORG:Acme \n Corp\nEND:VCARD\n";
        let out = dedup(vcf, "name", true);
        assert_eq!(count_cards(&out), 1, "{out}");
        assert!(out.contains("ORG:Acme Corp"), "folded ORG should carry over:\n{out}");
    }

    #[test]
    fn error_no_vcard() {
        let err = run("just some text", MatchBy::Any, true).unwrap_err();
        assert!(err.contains("no vCard found"), "got: {err}");
    }
}
