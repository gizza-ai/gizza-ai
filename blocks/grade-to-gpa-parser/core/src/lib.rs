//! grade-to-gpa-parser core — turn a pasted list of letter grades into GPA
//! points on a configurable scale and average them, credit-weighted.
//! Pure compute, no I/O: shared by the chat/CLI skill block and the web page.

/// Everything the calculation can be tuned with. Kept as a struct so every
/// surface (chat, CLI, page) passes the same knobs in the same order.
#[derive(Debug, Clone)]
pub struct Options {
    /// Base scale name: "4.0", "4.3" or "5.0".
    pub scale: String,
    /// `LETTER=POINTS` overrides merged onto the base scale.
    pub custom_scale: String,
    /// How to read a numeric grade: "auto", "letter", "percent" or "points".
    pub grade_format: String,
    /// Credit hours used for an entry that does not state its own.
    pub default_credits: f64,
    /// Extra points added to a non-failing honours course.
    pub honors_bonus: f64,
    /// Extra points added to a non-failing AP / IB / dual-enrollment course.
    pub ap_bonus: f64,
    /// GPA already earned before this list (0 = none).
    pub prior_gpa: f64,
    /// Credits behind `prior_gpa` (0 = none).
    pub prior_credits: f64,
    /// Drop pass/fail and withdrawal marks instead of erroring on them.
    pub skip_nongraded: bool,
    /// Decimal places in the output, 0-6.
    pub decimals: u32,
    /// "report" or "json".
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            scale: "4.0".to_string(),
            custom_scale: String::new(),
            grade_format: "auto".to_string(),
            default_credits: 1.0,
            honors_bonus: 0.5,
            ap_bonus: 1.0,
            prior_gpa: 0.0,
            prior_credits: 0.0,
            skip_nongraded: true,
            decimals: 2,
            output: "report".to_string(),
        }
    }
}

/// Hard cap on how many courses one run will parse.
pub const MAX_ENTRIES: usize = 2000;

const SCALE_4_0: &[(&str, f64)] = &[
    ("A+", 4.0),
    ("A", 4.0),
    ("A-", 3.7),
    ("B+", 3.3),
    ("B", 3.0),
    ("B-", 2.7),
    ("C+", 2.3),
    ("C", 2.0),
    ("C-", 1.7),
    ("D+", 1.3),
    ("D", 1.0),
    ("D-", 0.7),
    ("F", 0.0),
    ("E", 0.0),
];

const SCALE_4_3: &[(&str, f64)] = &[
    ("A+", 4.3),
    ("A", 4.0),
    ("A-", 3.7),
    ("B+", 3.3),
    ("B", 3.0),
    ("B-", 2.7),
    ("C+", 2.3),
    ("C", 2.0),
    ("C-", 1.7),
    ("D+", 1.3),
    ("D", 1.0),
    ("D-", 0.7),
    ("F", 0.0),
    ("E", 0.0),
];

const SCALE_5_0: &[(&str, f64)] = &[
    ("A+", 5.0),
    ("A", 5.0),
    ("A-", 4.7),
    ("B+", 4.3),
    ("B", 4.0),
    ("B-", 3.7),
    ("C+", 3.3),
    ("C", 3.0),
    ("C-", 2.7),
    ("D+", 2.3),
    ("D", 2.0),
    ("D-", 1.7),
    ("F", 0.0),
    ("E", 0.0),
];

/// Percentage bands of the common US plus/minus scheme, high to low.
const PERCENT_BANDS: &[(f64, &str)] = &[
    (97.0, "A+"),
    (93.0, "A"),
    (90.0, "A-"),
    (87.0, "B+"),
    (83.0, "B"),
    (80.0, "B-"),
    (77.0, "C+"),
    (73.0, "C"),
    (70.0, "C-"),
    (67.0, "D+"),
    (63.0, "D"),
    (60.0, "D-"),
];

/// Marks that carry no grade points on a US transcript.
const NON_GRADED: &[(&str, &str)] = &[
    ("P", "pass/fail"),
    ("NP", "pass/fail"),
    ("S", "satisfactory/unsatisfactory"),
    ("U", "satisfactory/unsatisfactory"),
    ("CR", "credit/no-credit"),
    ("NC", "credit/no-credit"),
    ("W", "withdrawn"),
    ("WD", "withdrawn"),
    ("I", "incomplete"),
    ("INC", "incomplete"),
    ("IP", "in progress"),
    ("AU", "audited"),
    ("NG", "not graded"),
    ("TR", "transfer credit"),
];

/// Tokens that mark a course as honours-weighted or AP-weighted.
const HONORS_TAGS: &[&str] = &["HONORS", "HONOR", "HON", "HNRS"];
const AP_TAGS: &[&str] = &["AP", "IB", "DE", "DUAL", "COLLEGE", "ADV"];

/// Filler words that never belong to a course name or a grade.
const NOISE: &[&str] = &[
    "CREDIT", "CREDITS", "CR.", "HR", "HRS", "HOUR", "HOURS", "UNIT", "UNITS", "X", "@",
];

#[derive(Debug, Clone)]
struct Course {
    n: usize,
    name: String,
    label: String,
    points: f64,
    bonus: f64,
    bonus_tag: &'static str,
    credits: f64,
}

#[derive(Debug, Clone)]
struct Skipped {
    name: String,
    mark: String,
    reason: &'static str,
}

/// Format a number with the requested decimals, never emitting `-0.00`.
fn fmt(v: f64, decimals: u32) -> String {
    let v = if v == 0.0 { 0.0 } else { v };
    format!("{:.*}", decimals as usize, v)
}

/// Render a number without trailing noise, for labels that quote what the user
/// typed (`92%`, `3.7 pts`) rather than a computed figure.
fn fmt_compact(v: f64) -> String {
    if v.fract() == 0.0 && v.abs() < 1e15 {
        return format!("{v:.0}");
    }
    let s = format!("{v:.6}");
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

/// A 1-2 character token like `Z` or `Q+` is someone attempting a grade, not a
/// course name — worth an error rather than a silent re-read of the credits.
fn looks_like_grade(up: &str) -> bool {
    let mut chars = up.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !first.is_ascii_alphabetic() {
        return false;
    }
    match chars.next() {
        None => true,
        Some(second) => matches!(second, '+' | '-') && chars.next().is_none(),
    }
}

/// Uppercase a token and fold the unicode dashes that word processors insert
/// into the ASCII hyphen a minus grade is written with.
fn normalize(tok: &str) -> String {
    tok.chars()
        .map(|c| match c {
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2212}' => '-',
            other => other,
        })
        .collect::<String>()
        .to_uppercase()
}

fn lookup(scale: &[(String, f64)], key: &str) -> Option<f64> {
    scale
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| *v)
}

fn non_graded_reason(key: &str) -> Option<&'static str> {
    NON_GRADED
        .iter()
        .find(|(k, _)| *k == key)
        .map(|(_, r)| *r)
}

/// Build the working scale: the named base table with any `LETTER=POINTS`
/// overrides merged on top (an unknown letter is appended, a known one replaced).
fn build_scale(name: &str, custom: &str) -> Result<(Vec<(String, f64)>, bool), String> {
    let base: &[(&str, f64)] = match name.trim() {
        "" | "4.0" | "4" => SCALE_4_0,
        "4.3" => SCALE_4_3,
        "5.0" | "5" => SCALE_5_0,
        other => {
            return Err(format!(
                "scale must be one of 4.0, 4.3 or 5.0 (got '{other}') — use custom_scale for anything else"
            ))
        }
    };
    let mut scale: Vec<(String, f64)> =
        base.iter().map(|(k, v)| (k.to_string(), *v)).collect();

    let mut customized = false;
    for pair in custom
        .split(|c: char| c == ',' || c == ';' || c == '\n' || c == '\r')
        .map(str::trim)
        .filter(|p| !p.is_empty())
    {
        let (k, v) = pair.split_once('=').or_else(|| pair.split_once(':')).ok_or_else(|| {
            format!("custom_scale entry '{pair}' must look like LETTER=POINTS, e.g. A+=4.5")
        })?;
        let key = normalize(k.trim());
        if key.is_empty() {
            return Err(format!("custom_scale entry '{pair}' is missing its letter grade"));
        }
        let points: f64 = v.trim().parse().map_err(|_| {
            format!("custom_scale entry '{pair}' must end in a number, e.g. A+=4.5")
        })?;
        if !points.is_finite() {
            return Err(format!("custom_scale entry '{pair}' must be a finite number"));
        }
        customized = true;
        match scale.iter_mut().find(|(existing, _)| *existing == key) {
            Some(slot) => slot.1 = points,
            None => scale.push((key, points)),
        }
    }
    Ok((scale, customized))
}

/// Split the pasted list into entries on newlines, semicolons and commas.
fn split_entries(grades: &str) -> Vec<&str> {
    grades
        .split(|c: char| c == '\n' || c == '\r' || c == ';' || c == ',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .collect()
}

/// Break one entry into comparable tokens: separators and brackets become
/// spaces, filler words are dropped, and everything is upper-cased for matching
/// while the original spelling is kept for the course name.
fn tokenize(entry: &str) -> Vec<(String, String)> {
    let cleaned: String = entry
        .chars()
        .map(|c| match c {
            '(' | ')' | '[' | ']' | '{' | '}' | ':' | '|' | '*' | '\u{00d7}' | '\t'
            | '\u{2013}' | '\u{2014}' | '\u{2192}' => ' ',
            other => other,
        })
        .collect();
    cleaned
        .split_whitespace()
        .map(|t| (t.to_string(), normalize(t)))
        .filter(|(_, up)| up != "-" && !NOISE.contains(&up.as_str()))
        .collect()
}

fn parse_number(tok: &str) -> Option<f64> {
    let t = tok.trim_end_matches('%');
    if t.is_empty() {
        return None;
    }
    t.parse::<f64>().ok().filter(|v| v.is_finite())
}

/// Map a percentage onto a letter of the plus/minus scheme.
fn percent_letter(pct: f64) -> &'static str {
    for (floor, letter) in PERCENT_BANDS {
        if pct >= *floor {
            return letter;
        }
    }
    "F"
}

struct Parsed {
    courses: Vec<Course>,
    skipped: Vec<Skipped>,
}

#[allow(clippy::too_many_lines)]
fn parse(grades: &str, scale: &[(String, f64)], o: &Options) -> Result<Parsed, String> {
    let entries = split_entries(grades);
    if entries.is_empty() {
        return Err(
            "no grades found — paste one course per line, e.g. 'Biology: A- 4' or just 'A, B+, C-'"
                .to_string(),
        );
    }
    if entries.len() > MAX_ENTRIES {
        return Err(format!(
            "too many entries: {} (maximum {MAX_ENTRIES})",
            entries.len()
        ));
    }

    let format = o.grade_format.trim();
    let format = if format.is_empty() { "auto" } else { format };
    if !matches!(format, "auto" | "letter" | "percent" | "points") {
        return Err(format!(
            "grade_format must be auto, letter, percent or points (got '{format}')"
        ));
    }
    let scale_max = scale.iter().map(|(_, v)| *v).fold(0.0_f64, f64::max);

    let mut courses: Vec<Course> = Vec::new();
    let mut skipped: Vec<Skipped> = Vec::new();

    for (idx, entry) in entries.iter().enumerate() {
        let row = idx + 1;
        let tokens = tokenize(entry);
        if tokens.is_empty() {
            continue;
        }

        // Level tags may sit anywhere in the entry ("AP Biology: A" and "A (AP)").
        let mut bonus = 0.0;
        let mut bonus_tag = "";
        for (_, up) in &tokens {
            if HONORS_TAGS.contains(&up.as_str()) && bonus_tag != "AP" {
                bonus = o.honors_bonus;
                bonus_tag = "honors";
            } else if AP_TAGS.contains(&up.as_str()) {
                bonus = o.ap_bonus;
                bonus_tag = "AP";
            }
        }
        let is_tag = |up: &str| HONORS_TAGS.contains(&up) || AP_TAGS.contains(&up);

        // The grade is the LAST letter token the scale (or the non-graded list)
        // knows about, so "AP Physics C: B" still reads as a B.
        let letter_at = tokens.iter().rposition(|(_, up)| {
            !is_tag(up) && (lookup(scale, up).is_some() || non_graded_reason(up).is_some())
        });
        let number_positions: Vec<usize> = tokens
            .iter()
            .enumerate()
            .filter(|(_, (raw, _))| parse_number(raw).is_some())
            .map(|(i, _)| i)
            .collect();

        let (grade_at, credits_at) = match letter_at {
            Some(i) => (
                i,
                number_positions
                    .iter()
                    .copied()
                    .find(|p| *p > i)
                    .or_else(|| number_positions.first().copied()),
            ),
            // No known grade: a short unknown token is a misspelled grade (it
            // falls through to the "not a grade on this scale" error below),
            // anything else means the entry is numeric.
            None => match tokens
                .iter()
                .rposition(|(_, up)| !is_tag(up) && looks_like_grade(up))
            {
                Some(i) => (i, number_positions.first().copied()),
                None => match number_positions.first().copied() {
                    Some(first) => (first, number_positions.get(1).copied()),
                    None => {
                        return Err(format!(
                            "entry {row} ('{entry}') has no grade — expected a letter grade such as A, B+ or C-, or a number"
                        ))
                    }
                },
            },
        };

        let name: String = tokens
            .iter()
            .enumerate()
            .filter(|(i, (_, up))| {
                *i != grade_at && Some(*i) != credits_at && !is_tag(up)
            })
            .map(|(_, (raw, _))| raw.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let name = if name.trim().is_empty() {
            format!("Course {row}")
        } else {
            name.trim().to_string()
        };

        let credits = match credits_at {
            Some(i) => parse_number(&tokens[i].0).unwrap_or(o.default_credits),
            None => o.default_credits,
        };
        if !credits.is_finite() || credits < 0.0 {
            return Err(format!(
                "entry {row} ('{entry}') has negative credits ({credits}) — credits must be 0 or more"
            ));
        }

        let (raw_tok, up_tok) = &tokens[grade_at];
        let (label, points) = if let Some(num) = parse_number(raw_tok) {
            // A numeric grade: percentage or raw grade points, per grade_format.
            let as_percent = match format {
                "percent" => true,
                "points" => false,
                "letter" => {
                    return Err(format!(
                        "entry {row} ('{entry}') is the number {num} but grade_format is 'letter' — set grade_format to percent, points or auto to accept numbers"
                    ))
                }
                // auto: anything above the top of the scale can only be a percentage.
                _ => num > scale_max,
            };
            if as_percent {
                if !(0.0..=100.0).contains(&num) {
                    return Err(format!(
                        "entry {row} ('{entry}'): a percentage grade must be between 0 and 100 (got {num})"
                    ));
                }
                let letter = percent_letter(num);
                let pts = lookup(scale, letter).ok_or_else(|| {
                    format!(
                        "entry {row} ('{entry}'): {num}% maps to grade {letter}, which is not in the scale — add {letter}=<points> to custom_scale"
                    )
                })?;
                (format!("{}% ({letter})", fmt_compact(num)), pts)
            } else {
                if num < 0.0 {
                    return Err(format!(
                        "entry {row} ('{entry}'): grade points must be 0 or more (got {num})"
                    ));
                }
                (format!("{} pts", fmt_compact(num)), num)
            }
        } else if let Some(pts) = lookup(scale, up_tok) {
            (up_tok.clone(), pts)
        } else if let Some(reason) = non_graded_reason(up_tok) {
            if !o.skip_nongraded {
                return Err(format!(
                    "entry {row} ('{entry}') is the non-graded mark {up_tok} ({reason}) — turn on skip_nongraded to leave such rows out of the GPA"
                ));
            }
            skipped.push(Skipped {
                name,
                mark: up_tok.clone(),
                reason,
            });
            continue;
        } else {
            return Err(format!(
                "entry {row} ('{entry}'): '{raw_tok}' is not a grade on this scale — expected one of {}",
                scale
                    .iter()
                    .map(|(k, _)| k.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        };

        // An F stays an F: the weighted bonus only lifts a passing grade.
        let bonus = if points > 0.0 { bonus } else { 0.0 };
        courses.push(Course {
            n: courses.len() + 1,
            name,
            label,
            points,
            bonus,
            bonus_tag: if bonus > 0.0 { bonus_tag } else { "" },
            credits,
        });
    }

    Ok(Parsed { courses, skipped })
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
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Parse a list of grades, convert each to points and return the credit-weighted
/// GPA as a human report or as JSON.
pub fn summary(grades: &str, o: &Options) -> Result<String, String> {
    if o.decimals > 6 {
        return Err(format!(
            "decimals must be between 0 and 6 (got {})",
            o.decimals
        ));
    }
    for (label, v) in [
        ("default_credits", o.default_credits),
        ("honors_bonus", o.honors_bonus),
        ("ap_bonus", o.ap_bonus),
        ("prior_gpa", o.prior_gpa),
        ("prior_credits", o.prior_credits),
    ] {
        if !v.is_finite() {
            return Err(format!("{label} must be a finite number (got {v})"));
        }
    }
    if o.default_credits <= 0.0 {
        return Err(format!(
            "default_credits must be greater than 0 (got {}) — it is the credit weight used for an entry that does not state one",
            o.default_credits
        ));
    }
    if o.honors_bonus < 0.0 || o.ap_bonus < 0.0 {
        return Err("honors_bonus and ap_bonus must be 0 or more".to_string());
    }
    if o.prior_credits < 0.0 || o.prior_gpa < 0.0 {
        return Err("prior_gpa and prior_credits must be 0 or more".to_string());
    }
    let output = o.output.trim();
    let output = if output.is_empty() { "report" } else { output };
    if !matches!(output, "report" | "json") {
        return Err(format!("output must be report or json (got '{output}')"));
    }

    let (scale, customized) = build_scale(&o.scale, &o.custom_scale)?;
    let Parsed { courses, skipped } = parse(grades, &scale, o)?;

    if courses.is_empty() {
        return Err(format!(
            "no graded courses to average — all {} entries were non-graded marks such as P or W",
            skipped.len()
        ));
    }
    let total_credits: f64 = courses.iter().map(|c| c.credits).sum();
    if total_credits <= 0.0 {
        return Err(
            "total credits add up to 0, so there is nothing to average — give at least one course a credit value above 0"
                .to_string(),
        );
    }
    let total_points: f64 = courses
        .iter()
        .map(|c| (c.points + c.bonus) * c.credits)
        .sum();
    let gpa = total_points / total_credits;

    let cumulative = if o.prior_credits > 0.0 {
        Some((
            (total_points + o.prior_gpa * o.prior_credits) / (total_credits + o.prior_credits),
            total_credits + o.prior_credits,
        ))
    } else {
        None
    };

    let d = o.decimals;
    let scale_name = if o.scale.trim().is_empty() {
        "4.0"
    } else {
        o.scale.trim()
    };

    if output == "json" {
        let mut s = String::new();
        s.push_str(&format!("{{\n  \"gpa\": {},\n", fmt(gpa, d)));
        s.push_str(&format!("  \"grade_points\": {},\n", fmt(total_points, d)));
        s.push_str(&format!("  \"credits\": {},\n", fmt(total_credits, d)));
        s.push_str(&format!("  \"courses_counted\": {},\n", courses.len()));
        s.push_str(&format!(
            "  \"scale\": \"{}\",\n  \"custom_scale_applied\": {},\n",
            json_escape(scale_name),
            customized
        ));
        s.push_str("  \"courses\": [");
        for (i, c) in courses.iter().enumerate() {
            s.push_str(if i == 0 { "\n" } else { ",\n" });
            s.push_str(&format!(
                "    {{ \"n\": {}, \"course\": \"{}\", \"grade\": \"{}\", \"points\": {}, \"bonus\": {}, \"credits\": {}, \"quality_points\": {} }}",
                c.n,
                json_escape(&c.name),
                json_escape(&c.label),
                fmt(c.points, d),
                fmt(c.bonus, d),
                fmt(c.credits, d),
                fmt((c.points + c.bonus) * c.credits, d)
            ));
        }
        s.push_str(if courses.is_empty() { "],\n" } else { "\n  ],\n" });
        s.push_str("  \"not_counted\": [");
        for (i, k) in skipped.iter().enumerate() {
            s.push_str(if i == 0 { "\n" } else { ",\n" });
            s.push_str(&format!(
                "    {{ \"course\": \"{}\", \"mark\": \"{}\", \"reason\": \"{}\" }}",
                json_escape(&k.name),
                json_escape(&k.mark),
                json_escape(k.reason)
            ));
        }
        s.push_str(if skipped.is_empty() { "]" } else { "\n  ]" });
        if let Some((cgpa, ccred)) = cumulative {
            s.push_str(&format!(
                ",\n  \"cumulative\": {{ \"gpa\": {}, \"credits\": {} }}",
                fmt(cgpa, d),
                fmt(ccred, d)
            ));
        }
        s.push_str("\n}");
        return Ok(s);
    }

    let mut out = String::new();
    out.push_str(&format!("GPA: {}\n", fmt(gpa, d)));
    out.push_str(&format!("Grade points: {}\n", fmt(total_points, d)));
    out.push_str(&format!("Credits counted: {}\n", fmt(total_credits, d)));
    out.push_str(&format!("Courses counted: {}\n", courses.len()));

    out.push_str("\nCourse breakdown\n");
    for c in &courses {
        let bonus_part = if c.bonus > 0.0 {
            format!(" +{} {}", fmt(c.bonus, d), c.bonus_tag)
        } else {
            String::new()
        };
        out.push_str(&format!(
            "{}. {} — {}{} → {} × {} credits = {}\n",
            c.n,
            c.name,
            c.label,
            bonus_part,
            fmt(c.points + c.bonus, d),
            fmt(c.credits, d),
            fmt((c.points + c.bonus) * c.credits, d)
        ));
    }

    if !skipped.is_empty() {
        out.push_str("\nNot counted\n");
        for k in &skipped {
            out.push_str(&format!("- {} — {} ({})\n", k.name, k.mark, k.reason));
        }
    }

    if let Some((cgpa, ccred)) = cumulative {
        out.push_str(&format!(
            "\nCumulative GPA: {} over {} credits (this list plus a prior {} over {} credits)\n",
            fmt(cgpa, d),
            fmt(ccred, d),
            fmt(o.prior_gpa, d),
            fmt(o.prior_credits, d)
        ));
    }

    out.push_str(&format!(
        "\nScale: {}{} — {}",
        scale_name,
        if customized { " with custom overrides" } else { "" },
        scale
            .iter()
            .map(|(k, v)| format!("{k} {}", fmt(*v, d)))
            .collect::<Vec<_>>()
            .join(", ")
    ));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn plain_letter_list_is_a_simple_average() {
        let out = summary("A, B+, C-", &opts()).unwrap();
        // (4.0 + 3.3 + 1.7) / 3 = 3.0
        assert!(out.starts_with("GPA: 3.00\n"), "{out}");
        assert!(out.contains("Courses counted: 3"), "{out}");
        assert!(out.contains("1. Course 1 — A → 4.00 × 1.00 credits = 4.00"), "{out}");
    }

    #[test]
    fn credits_weight_the_average() {
        let out = summary("Biology: A- 4\nMath: B 3\nArt: C 1", &opts()).unwrap();
        // (3.7*4 + 3.0*3 + 2.0*1) / 8 = 25.8 / 8 = 3.225 -> 3.23
        assert!(out.starts_with("GPA: 3.23\n"), "{out}");
        assert!(out.contains("Credits counted: 8.00"), "{out}");
        assert!(out.contains("Biology — A- → 3.70 × 4.00 credits = 14.80"), "{out}");
    }

    #[test]
    fn honors_and_ap_tags_add_their_bonus_but_never_to_an_f() {
        let out = summary("AP History: A 3\nHonors Chem: B 3\nAP Art: F 3", &opts()).unwrap();
        // (5.0*3 + 3.5*3 + 0*3) / 9 = 25.5 / 9 = 2.8333
        assert!(out.starts_with("GPA: 2.83\n"), "{out}");
        assert!(out.contains("A +1.00 AP → 5.00"), "{out}");
        assert!(out.contains("B +0.50 honors → 3.50"), "{out}");
        assert!(out.contains("Art — F → 0.00 × 3.00 credits = 0.00"), "{out}");
    }

    #[test]
    fn a_plus_follows_the_selected_scale() {
        let mut o = opts();
        assert!(summary("A+", &o).unwrap().starts_with("GPA: 4.00\n"));
        o.scale = "4.3".to_string();
        assert!(summary("A+", &o).unwrap().starts_with("GPA: 4.30\n"));
        o.scale = "5.0".to_string();
        assert!(summary("B", &o).unwrap().starts_with("GPA: 4.00\n"));
    }

    #[test]
    fn custom_scale_overrides_and_extends_the_base() {
        let mut o = opts();
        o.custom_scale = "A+=4.5, HD=4.0".to_string();
        let out = summary("A+, HD", &o).unwrap();
        assert!(out.starts_with("GPA: 4.25\n"), "{out}");
        assert!(out.contains("with custom overrides"), "{out}");
    }

    #[test]
    fn percentages_convert_through_the_letter_bands() {
        let out = summary("Biology 92 4\nMath 85 3", &opts()).unwrap();
        // 92 -> A- (3.7), 85 -> B (3.0); (14.8 + 9.0) / 7 = 3.4
        assert!(out.starts_with("GPA: 3.40\n"), "{out}");
        assert!(out.contains("92% (A-)"), "{out}");
    }

    #[test]
    fn small_numbers_are_read_as_grade_points_in_auto_mode() {
        let out = summary("3.7, 4", &opts()).unwrap();
        assert!(out.starts_with("GPA: 3.85\n"), "{out}");
        assert!(out.contains("3.7 pts"), "{out}");
    }

    #[test]
    fn pass_fail_marks_are_listed_but_left_out_of_the_average() {
        let out = summary("Biology: A 4\nYoga: P 2", &opts()).unwrap();
        assert!(out.starts_with("GPA: 4.00\n"), "{out}");
        assert!(out.contains("Credits counted: 4.00"), "{out}");
        assert!(out.contains("- Yoga — P (pass/fail)"), "{out}");
    }

    #[test]
    fn prior_credits_produce_a_cumulative_gpa() {
        let mut o = opts();
        o.prior_gpa = 3.0;
        o.prior_credits = 30.0;
        let out = summary("A 10", &o).unwrap();
        // (40 + 90) / 40 = 3.25
        assert!(out.contains("Cumulative GPA: 3.25 over 40.00 credits"), "{out}");
    }

    #[test]
    fn json_output_is_parseable_shaped_data() {
        let mut o = opts();
        o.output = "json".to_string();
        let out = summary("Biology: A- 4", &o).unwrap();
        assert!(out.contains("\"gpa\": 3.70"), "{out}");
        assert!(out.contains("\"course\": \"Biology\""), "{out}");
        assert!(out.contains("\"quality_points\": 14.80"), "{out}");
    }

    #[test]
    fn unknown_grade_is_rejected_with_the_accepted_values() {
        let e = summary("Biology: Z 4", &opts()).unwrap_err();
        assert!(e.contains("'Z' is not a grade on this scale"), "{e}");
        assert!(e.contains("A+, A, A-"), "{e}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let e = summary("   \n  ", &opts()).unwrap_err();
        assert!(e.contains("no grades found"), "{e}");
    }

    #[test]
    fn out_of_range_decimals_is_rejected() {
        let mut o = opts();
        o.decimals = 9;
        let e = summary("A", &o).unwrap_err();
        assert!(e.contains("decimals must be between 0 and 6"), "{e}");
    }

    #[test]
    fn a_letter_only_run_rejects_a_numeric_grade() {
        let mut o = opts();
        o.grade_format = "letter".to_string();
        let e = summary("Biology 92", &o).unwrap_err();
        assert!(e.contains("grade_format is 'letter'"), "{e}");
    }

    #[test]
    fn nongraded_rows_error_when_skipping_is_turned_off() {
        let mut o = opts();
        o.skip_nongraded = false;
        let e = summary("Biology: A 4\nYoga: W 2", &o).unwrap_err();
        assert!(e.contains("non-graded mark W"), "{e}");
    }

    #[test]
    fn all_nongraded_input_has_nothing_to_average() {
        let e = summary("P, W, I", &opts()).unwrap_err();
        assert!(e.contains("no graded courses to average"), "{e}");
    }

    #[test]
    fn a_bad_custom_scale_entry_is_rejected() {
        let mut o = opts();
        o.custom_scale = "A+ 4.5".to_string();
        let e = summary("A+", &o).unwrap_err();
        assert!(e.contains("must look like LETTER=POINTS"), "{e}");
    }

    #[test]
    fn too_many_entries_is_capped() {
        let big = vec!["A"; MAX_ENTRIES + 1].join("\n");
        let e = summary(&big, &opts()).unwrap_err();
        assert!(e.contains("too many entries"), "{e}");
        let ok = vec!["A"; MAX_ENTRIES].join("\n");
        assert!(summary(&ok, &opts()).unwrap().starts_with("GPA: 4.00\n"));
    }

    #[test]
    fn course_names_survive_a_trailing_grade_letter() {
        let out = summary("AP Physics C: B 4", &opts()).unwrap();
        assert!(out.contains("1. Physics C — B +1.00 AP"), "{out}");
    }

    #[test]
    fn unicode_minus_and_bracketed_credits_parse() {
        let out = summary("Biology: A\u{2212} (4)", &opts()).unwrap();
        assert!(out.starts_with("GPA: 3.70\n"), "{out}");
        assert!(out.contains("× 4.00 credits"), "{out}");
    }
}
