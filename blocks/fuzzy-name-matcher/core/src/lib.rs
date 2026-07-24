//! fuzzy-name-matcher core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Matches and deduplicates person / organization names that refer to the same
//! entity but are spelled differently — typos, casing, honorifics, initials, and
//! *phonetic* variants (Smith/Smyth, Jon/John). Three name-tuned similarity
//! algorithms are selectable:
//!
//! * **jaro_winkler** — the record-linkage standard for short strings; boosts a
//!   shared prefix, so it's forgiving of typos while still ranking well.
//! * **levenshtein** — a normalized edit-distance ratio (insert/delete/substitute).
//! * **soundex** — phonetic: each token is reduced to its Soundex code, so names
//!   that *sound* alike match even when spelled very differently.
//!
//! Two output shapes: `groups` greedily clusters the list into match groups with a
//! canonical (most frequent) name per group; `pairs` lists every name pair scoring
//! at or above the threshold, best first. This is the name/entity-resolution
//! counterpart to the generic Levenshtein-only `cluster-similar-values` and
//! `fuzzy-dedupe` tools.

use std::collections::HashMap;

use serde::Serialize;

/// Honorific prefixes stripped from the head of a name when `ignore_titles` is on.
const TITLES: &[&str] = &[
    "mr", "mrs", "ms", "miss", "mx", "dr", "prof", "professor", "sir", "madam",
    "madame", "rev", "hon", "capt", "sgt", "lt", "col", "gen",
];
/// Generational / credential suffixes stripped from the tail when `ignore_titles` is on.
const SUFFIXES: &[&str] = &[
    "jr", "sr", "ii", "iii", "iv", "phd", "md", "esq", "dds", "cpa", "dvm",
];

/// Strip surrounding punctuation from a token for title matching ("Dr." → "dr").
fn depunct(tok: &str) -> String {
    tok.trim_matches(|c: char| !c.is_alphanumeric()).to_ascii_lowercase()
}

/// Normalize a name for *comparison only* (never for display): collapse whitespace,
/// optionally lowercase, and optionally drop leading honorifics / trailing suffixes.
fn normalize(name: &str, case: bool, titles: bool) -> String {
    let mut toks: Vec<&str> = name.split_whitespace().collect();
    if titles {
        while let Some(first) = toks.first() {
            if TITLES.contains(&depunct(first).as_str()) {
                toks.remove(0);
            } else {
                break;
            }
        }
        while let Some(last) = toks.last() {
            if SUFFIXES.contains(&depunct(last).as_str()) {
                toks.pop();
            } else {
                break;
            }
        }
    }
    let mut r = toks.join(" ");
    if r.trim().is_empty() {
        // A name that was ALL titles (e.g. "Dr.") keeps its original text so it
        // still participates rather than collapsing to empty.
        r = name.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if case {
        r = r.to_lowercase();
    }
    r
}

/// Full Levenshtein edit distance (names are short, no bounding needed).
fn levenshtein(a: &[char], b: &[char]) -> usize {
    if a.is_empty() {
        return b.len();
    }
    if b.is_empty() {
        return a.len();
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut curr = vec![0usize; b.len() + 1];
    for (i, &ca) in a.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &cb) in b.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }
    prev[b.len()]
}

/// Normalized Levenshtein similarity of two strings in 0..=100 (100 = identical).
fn levenshtein_ratio(a: &str, b: &str) -> f64 {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let m = ca.len().max(cb.len());
    if m == 0 {
        return 100.0;
    }
    let d = levenshtein(&ca, &cb);
    (1.0 - d as f64 / m as f64) * 100.0
}

/// Jaro similarity of two strings in 0..=1.
fn jaro(a: &[char], b: &[char]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let match_dist = (a.len().max(b.len()) / 2).saturating_sub(1);
    let mut a_match = vec![false; a.len()];
    let mut b_match = vec![false; b.len()];
    let mut matches = 0usize;
    for (i, &ca) in a.iter().enumerate() {
        let lo = i.saturating_sub(match_dist);
        let hi = (i + match_dist + 1).min(b.len());
        for j in lo..hi {
            if !b_match[j] && b[j] == ca {
                a_match[i] = true;
                b_match[j] = true;
                matches += 1;
                break;
            }
        }
    }
    if matches == 0 {
        return 0.0;
    }
    // Count transpositions.
    let mut transpositions = 0usize;
    let mut k = 0usize;
    for i in 0..a.len() {
        if a_match[i] {
            while !b_match[k] {
                k += 1;
            }
            if a[i] != b[k] {
                transpositions += 1;
            }
            k += 1;
        }
    }
    let m = matches as f64;
    let t = transpositions as f64 / 2.0;
    (m / a.len() as f64 + m / b.len() as f64 + (m - t) / m) / 3.0
}

/// Jaro-Winkler similarity in 0..=100 (adds a bonus for a shared prefix up to 4).
fn jaro_winkler_ratio(a: &str, b: &str) -> f64 {
    let ca: Vec<char> = a.chars().collect();
    let cb: Vec<char> = b.chars().collect();
    let j = jaro(&ca, &cb);
    let prefix = ca
        .iter()
        .zip(cb.iter())
        .take(4)
        .take_while(|(x, y)| x == y)
        .count();
    let jw = j + prefix as f64 * 0.1 * (1.0 - j);
    jw * 100.0
}

/// Soundex code (American Soundex): a retained first letter + three digits.
fn soundex_token(token: &str) -> String {
    let letters: Vec<char> = token
        .chars()
        .filter(|c| c.is_ascii_alphabetic())
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if letters.is_empty() {
        return String::new();
    }
    fn code(c: char) -> char {
        match c {
            'B' | 'F' | 'P' | 'V' => '1',
            'C' | 'G' | 'J' | 'K' | 'Q' | 'S' | 'X' | 'Z' => '2',
            'D' | 'T' => '3',
            'L' => '4',
            'M' | 'N' => '5',
            'R' => '6',
            _ => '0', // vowels + H, W, Y
        }
    }
    let first = letters[0];
    let mut out = String::new();
    out.push(first);
    let mut last = code(first);
    for &c in &letters[1..] {
        let d = code(c);
        // H and W don't reset the "previous code" (they're transparent); vowels do.
        if d != '0' && d != last {
            out.push(d);
            if out.len() == 4 {
                break;
            }
        }
        if c != 'H' && c != 'W' {
            last = d;
        }
    }
    while out.len() < 4 {
        out.push('0');
    }
    out
}

/// Phonetic key for a whole name: Soundex each whitespace token, join with spaces.
fn phonetic_key(name: &str) -> String {
    name.split_whitespace()
        .map(soundex_token)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// Similarity of two ALREADY-NORMALIZED names in 0..=100 under the chosen algorithm.
fn similarity(a: &str, b: &str, algorithm: &str) -> f64 {
    match algorithm {
        "levenshtein" => levenshtein_ratio(a, b),
        "soundex" => levenshtein_ratio(&phonetic_key(a), &phonetic_key(b)),
        // "jaro_winkler" (default / validated by caller)
        _ => jaro_winkler_ratio(a, b),
    }
}

/// Round a 0–100 score to one decimal place for stable display / tests.
fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

struct Unique {
    value: String, // original text, kept exactly
    norm: String,  // comparison form
    count: usize,
    first: usize, // first-seen order
}

#[derive(Serialize)]
struct OutGroupMember {
    name: String,
    count: usize,
}

#[derive(Serialize)]
struct OutGroup {
    canonical: String,
    size: usize,
    count: usize,
    members: Vec<OutGroupMember>,
}

#[derive(Serialize)]
struct OutPair {
    name_a: String,
    name_b: String,
    score: f64,
}

#[derive(Serialize)]
struct GroupsOut {
    mode: &'static str,
    algorithm: String,
    threshold: f64,
    total_names: usize,
    unique_names: usize,
    group_count: usize,
    match_groups: usize,
    groups: Vec<OutGroup>,
}

#[derive(Serialize)]
struct PairsOut {
    mode: &'static str,
    algorithm: String,
    threshold: f64,
    total_names: usize,
    unique_names: usize,
    match_count: usize,
    pairs: Vec<OutPair>,
}

/// Collect the input into distinct original names with counts, preserving
/// first-seen order and skipping blank lines. Accepts a newline list or a CSV-ish
/// list (only the first field of each row is taken as the name).
fn collect_names(data: &str, case: bool, titles: bool) -> Vec<Unique> {
    // Take the first comma/tab field of each line so single-column CSVs "just work".
    fn name_of(line: &str) -> &str {
        line.split(['\t', ','])
            .next()
            .unwrap_or("")
            .trim_matches(|c: char| c == '"' || c.is_whitespace())
    }
    let mut order: Vec<String> = Vec::new();
    let mut counts: HashMap<String, usize> = HashMap::new();
    for line in data.lines() {
        let raw = name_of(line);
        if raw.is_empty() {
            continue;
        }
        if !counts.contains_key(raw) {
            order.push(raw.to_string());
        }
        *counts.entry(raw.to_string()).or_insert(0) += 1;
    }
    order
        .iter()
        .enumerate()
        .map(|(first, value)| Unique {
            norm: normalize(value, case, titles),
            count: counts[value],
            value: value.clone(),
            first,
        })
        .collect()
}

/// Match / deduplicate names. See the module docs for the algorithm + mode menu.
///
/// * `names` — one name per line (or a single-column CSV; the first field is used).
/// * `algorithm` — `jaro_winkler` (default), `levenshtein`, or `soundex`.
/// * `mode` — `groups` (cluster into match groups) or `pairs` (scored matched pairs).
/// * `threshold` — 0..=100; names at or above this similarity are the "same".
/// * `output` — `table` (markdown), `csv`, or `json`.
#[allow(clippy::too_many_arguments)]
pub fn match_names(
    names: &str,
    algorithm: &str,
    mode: &str,
    threshold: f64,
    normalize_case: bool,
    ignore_titles: bool,
    output: &str,
) -> Result<String, String> {
    if names.trim().is_empty() {
        return Err("input is empty — paste one name per line".into());
    }
    if !(0.0..=100.0).contains(&threshold) {
        return Err(format!("threshold must be between 0 and 100, got {threshold}"));
    }
    if !matches!(algorithm, "jaro_winkler" | "levenshtein" | "soundex") {
        return Err(format!(
            "algorithm must be one of jaro_winkler, levenshtein, soundex — got '{algorithm}'"
        ));
    }
    if !matches!(mode, "groups" | "pairs") {
        return Err(format!("mode must be one of groups, pairs — got '{mode}'"));
    }
    let fmt = match output.trim().to_ascii_lowercase().as_str() {
        "" | "table" | "markdown" | "md" => "table",
        "csv" => "csv",
        "json" => "json",
        other => return Err(format!("output must be table, csv, or json — got '{other}'")),
    };

    let uniques = collect_names(names, normalize_case, ignore_titles);
    if uniques.is_empty() {
        return Err("no names found — every line was blank".into());
    }
    let total_names: usize = uniques.iter().map(|u| u.count).sum();
    let unique_names = uniques.len();

    if mode == "pairs" {
        let mut pairs: Vec<OutPair> = Vec::new();
        for i in 0..uniques.len() {
            for j in (i + 1)..uniques.len() {
                let s = similarity(&uniques[i].norm, &uniques[j].norm, algorithm);
                if s >= threshold {
                    pairs.push(OutPair {
                        name_a: uniques[i].value.clone(),
                        name_b: uniques[j].value.clone(),
                        score: round1(s),
                    });
                }
            }
        }
        // Best score first; stable tie-break by name for deterministic output.
        pairs.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.name_a.cmp(&b.name_a))
                .then_with(|| a.name_b.cmp(&b.name_b))
        });
        let match_count = pairs.len();
        return Ok(match fmt {
            "json" => serde_json::to_string_pretty(&PairsOut {
                mode: "pairs",
                algorithm: algorithm.to_string(),
                threshold,
                total_names,
                unique_names,
                match_count,
                pairs,
            })
            .map_err(|e| format!("json error: {e}"))?,
            "csv" => render_pairs_csv(&pairs),
            _ => render_pairs_table(algorithm, threshold, unique_names, &pairs),
        });
    }

    // groups mode: process most frequent first so the seed / canonical is the most
    // common spelling; ties break by first-seen order.
    let mut idxs: Vec<usize> = (0..uniques.len()).collect();
    idxs.sort_by(|&a, &b| {
        uniques[b]
            .count
            .cmp(&uniques[a].count)
            .then_with(|| uniques[a].first.cmp(&uniques[b].first))
    });

    struct Group {
        seed_norm: String,
        members: Vec<usize>, // indices into `uniques`
        total: usize,
    }
    let mut groups: Vec<Group> = Vec::new();
    for &u in &idxs {
        let norm = &uniques[u].norm;
        let mut best: Option<(usize, f64)> = None;
        for (gi, g) in groups.iter().enumerate() {
            let s = similarity(norm, &g.seed_norm, algorithm);
            if s >= threshold && best.map(|(_, bs)| s > bs).unwrap_or(true) {
                best = Some((gi, s));
            }
        }
        match best {
            Some((gi, _)) => {
                groups[gi].members.push(u);
                groups[gi].total += uniques[u].count;
            }
            None => groups.push(Group {
                seed_norm: norm.clone(),
                members: vec![u],
                total: uniques[u].count,
            }),
        }
    }
    // Surface merged groups first, then by total count.
    groups.sort_by(|a, b| {
        b.members
            .len()
            .cmp(&a.members.len())
            .then_with(|| b.total.cmp(&a.total))
            .then_with(|| uniques[a.members[0]].first.cmp(&uniques[b.members[0]].first))
    });

    let group_count = groups.len();
    let match_groups = groups.iter().filter(|g| g.members.len() > 1).count();
    let out_groups: Vec<OutGroup> = groups
        .iter()
        .map(|g| OutGroup {
            canonical: uniques[g.members[0]].value.clone(),
            size: g.members.len(),
            count: g.total,
            members: g
                .members
                .iter()
                .map(|&m| OutGroupMember {
                    name: uniques[m].value.clone(),
                    count: uniques[m].count,
                })
                .collect(),
        })
        .collect();

    Ok(match fmt {
        "json" => serde_json::to_string_pretty(&GroupsOut {
            mode: "groups",
            algorithm: algorithm.to_string(),
            threshold,
            total_names,
            unique_names,
            group_count,
            match_groups,
            groups: out_groups,
        })
        .map_err(|e| format!("json error: {e}"))?,
        "csv" => render_groups_csv(&out_groups),
        _ => render_groups_table(
            algorithm,
            threshold,
            total_names,
            unique_names,
            group_count,
            match_groups,
            &out_groups,
        ),
    })
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|")
}

fn thr_str(threshold: f64) -> String {
    if threshold.fract() == 0.0 {
        format!("{}", threshold as i64)
    } else {
        format!("{threshold}")
    }
}

fn render_groups_table(
    algorithm: &str,
    threshold: f64,
    total: usize,
    unique: usize,
    group_count: usize,
    match_groups: usize,
    groups: &[OutGroup],
) -> String {
    let mut s = String::new();
    s.push_str("# Name match groups\n\n");
    s.push_str(&format!(
        "{total} names, {unique} unique → {group_count} groups ({match_groups} with matches) using {algorithm} at threshold {}%.\n\n",
        thr_str(threshold)
    ));
    s.push_str("## Match groups\n\n");
    if match_groups == 0 {
        s.push_str("No matching names found at this threshold.\n\n");
    } else {
        for g in groups.iter().filter(|g| g.size > 1) {
            s.push_str(&format!("### {}\n\n", md_escape(&g.canonical)));
            s.push_str("| Name | Count |\n| --- | --- |\n");
            for (i, m) in g.members.iter().enumerate() {
                let name = if i == 0 {
                    format!("**{}**", md_escape(&m.name))
                } else {
                    md_escape(&m.name)
                };
                s.push_str(&format!("| {} | {} |\n", name, m.count));
            }
            s.push('\n');
        }
    }
    s.push_str("## Mapping\n\n");
    s.push_str("| Name | Canonical | Count |\n| --- | --- | --- |\n");
    for g in groups {
        for m in &g.members {
            s.push_str(&format!(
                "| {} | {} | {} |\n",
                md_escape(&m.name),
                md_escape(&g.canonical),
                m.count
            ));
        }
    }
    s
}

fn render_groups_csv(groups: &[OutGroup]) -> String {
    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
    let _ = wtr.write_record(["group", "name", "canonical", "count"]);
    for (gi, g) in groups.iter().enumerate() {
        for m in &g.members {
            let _ = wtr.write_record([
                &(gi + 1).to_string(),
                &m.name,
                &g.canonical,
                &m.count.to_string(),
            ]);
        }
    }
    let bytes = wtr.into_inner().unwrap_or_default();
    String::from_utf8_lossy(&bytes).trim_end_matches(['\n', '\r']).to_string()
}

fn render_pairs_table(
    algorithm: &str,
    threshold: f64,
    unique: usize,
    pairs: &[OutPair],
) -> String {
    let mut s = String::new();
    s.push_str("# Matched name pairs\n\n");
    s.push_str(&format!(
        "{} matching pairs among {unique} unique names using {algorithm} at threshold {}%.\n\n",
        pairs.len(),
        thr_str(threshold)
    ));
    if pairs.is_empty() {
        s.push_str("No matching pairs found at this threshold.\n");
        return s;
    }
    s.push_str("| Name A | Name B | Score |\n| --- | --- | --- |\n");
    for p in pairs {
        s.push_str(&format!(
            "| {} | {} | {} |\n",
            md_escape(&p.name_a),
            md_escape(&p.name_b),
            score_str(p.score)
        ));
    }
    s
}

fn render_pairs_csv(pairs: &[OutPair]) -> String {
    let mut wtr = csv::WriterBuilder::new().from_writer(vec![]);
    let _ = wtr.write_record(["name_a", "name_b", "score"]);
    for p in pairs {
        let _ = wtr.write_record([&p.name_a, &p.name_b, &score_str(p.score)]);
    }
    let bytes = wtr.into_inner().unwrap_or_default();
    String::from_utf8_lossy(&bytes).trim_end_matches(['\n', '\r']).to_string()
}

/// Render a score without a trailing `.0` (so 100.0 → "100", 92.3 → "92.3").
fn score_str(x: f64) -> String {
    if x.fract() == 0.0 {
        format!("{}", x as i64)
    } else {
        format!("{x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn soundex_codes_known_vectors() {
        assert_eq!(soundex_token("Robert"), "R163");
        assert_eq!(soundex_token("Rupert"), "R163");
        assert_eq!(soundex_token("Smith"), "S530");
        assert_eq!(soundex_token("Smyth"), "S530");
        assert_eq!(soundex_token("Tymczak"), "T522");
        assert_eq!(soundex_token("Pfister"), "P236");
    }

    #[test]
    fn jaro_winkler_prefix_boost() {
        // Classic Winkler example ≈ 0.961.
        let s = jaro_winkler_ratio("martha", "marhta");
        assert!((s - 96.1).abs() < 0.5, "got {s}");
        // Identical → 100.
        assert_eq!(jaro_winkler_ratio("jones", "jones"), 100.0);
    }

    #[test]
    fn groups_jaro_winkler_merges_typo_variants() {
        // Three spellings of one person + a distinct name.
        let data = "Jonathan Smith\nJonathon Smith\nJonathan Smyth\nMaria Garcia";
        let out = match_names(data, "jaro_winkler", "groups", 88.0, true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["unique_names"], 4);
        assert_eq!(v["match_groups"], 1);
        let groups = v["groups"].as_array().unwrap();
        assert_eq!(groups[0]["size"], 3);
        assert_eq!(groups[0]["canonical"], "Jonathan Smith");
        assert_eq!(groups[1]["canonical"], "Maria Garcia");
    }

    #[test]
    fn soundex_matches_phonetic_spellings() {
        // Different letters, same sound → soundex groups them; edit distance wouldn't.
        let data = "Catherine\nKatherine\nKathryn";
        // Catherine=C365, Katherine=K365, Kathryn=K365 → Katherine/Kathryn merge.
        let out = match_names(data, "soundex", "groups", 100.0, true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["match_groups"], 1);
    }

    #[test]
    fn ignore_titles_strips_honorifics_and_suffixes() {
        let data = "Dr. John Adams\nJohn Adams Jr\nMr John Adams";
        let out = match_names(data, "jaro_winkler", "groups", 95.0, true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // All three collapse to "john adams" and match tightly.
        assert_eq!(v["group_count"], 1);
        assert_eq!(v["groups"][0]["size"], 3);
    }

    #[test]
    fn titles_kept_when_flag_off() {
        let data = "Dr. John Adams\nJohn Adams";
        let out = match_names(data, "levenshtein", "groups", 95.0, true, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        // "dr. john adams" vs "john adams" is < 95% by edit distance → 2 groups.
        assert_eq!(v["group_count"], 2);
    }

    #[test]
    fn pairs_mode_lists_scored_matches_best_first() {
        let data = "Bill Gates\nWilliam Gates\nBil Gates";
        let out = match_names(data, "jaro_winkler", "pairs", 80.0, true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        let pairs = v["pairs"].as_array().unwrap();
        assert!(!pairs.is_empty());
        // Bill Gates / Bil Gates is the closest pair.
        assert_eq!(pairs[0]["name_a"], "Bill Gates");
        assert_eq!(pairs[0]["name_b"], "Bil Gates");
        let s0 = pairs[0]["score"].as_f64().unwrap();
        let s1 = pairs[1]["score"].as_f64().unwrap();
        assert!(s0 >= s1);
    }

    #[test]
    fn csv_groups_first_field_of_each_row() {
        // CSV-ish input: only the first field is the name.
        let data = "Acme Corp,100\nAcme Corporation,90\nGlobex,50";
        let out = match_names(data, "jaro_winkler", "groups", 85.0, true, true, "csv").unwrap();
        assert!(out.starts_with("group,name,canonical,count"));
        assert!(out.contains("Acme Corp"));
        assert!(out.contains("Globex"));
    }

    #[test]
    fn counts_duplicates_and_picks_frequent_canonical() {
        let data = "Jon Doe\nJohn Doe\nJohn Doe\nJohn Doe";
        let out = match_names(data, "jaro_winkler", "groups", 85.0, true, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total_names"], 4);
        assert_eq!(v["unique_names"], 2);
        // "John Doe" (3×) is the canonical over "Jon Doe" (1×).
        assert_eq!(v["groups"][0]["canonical"], "John Doe");
    }

    #[test]
    fn table_output_has_headings_and_mapping() {
        let data = "Sara Lee\nSarah Lee";
        let out = match_names(data, "jaro_winkler", "groups", 85.0, true, true, "table").unwrap();
        assert!(out.contains("# Name match groups"));
        assert!(out.contains("## Mapping"));
        assert!(out.contains("jaro_winkler"));
    }

    #[test]
    fn err_on_empty_input() {
        assert!(match_names("   ", "jaro_winkler", "groups", 85.0, true, true, "table").is_err());
    }

    #[test]
    fn err_on_bad_threshold() {
        assert!(match_names("a\nb", "jaro_winkler", "groups", 150.0, true, true, "json").is_err());
    }

    #[test]
    fn err_on_bad_algorithm() {
        let e = match_names("a\nb", "cosine", "groups", 85.0, true, true, "json").unwrap_err();
        assert!(e.contains("algorithm must be"), "got: {e}");
    }

    #[test]
    fn err_on_bad_mode() {
        assert!(match_names("a\nb", "jaro_winkler", "chart", 85.0, true, true, "json").is_err());
    }

    #[test]
    fn err_on_bad_output() {
        assert!(match_names("a\nb", "jaro_winkler", "groups", 85.0, true, true, "xml").is_err());
    }
}
