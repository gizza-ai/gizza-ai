//! gizza-ai/list-converter core — reformat a list between comma/newline/bulleted/
//! numbered/quoted/space forms, with optional sort and dedupe. No deps; pure
//! string processing. Items are trimmed and empties dropped.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InSep {
    Auto,
    Comma,
    Newline,
    Semicolon,
    Space,
    Tab,
    Pipe,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Comma,
    Newline,
    Bulleted,
    Numbered,
    Quoted,
    Space,
    Tab,
    Pipe,
    JSON,
    XML,
    SQL,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaseTransform {
    None,
    Lowercase,
    Uppercase,
    Titlecase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    None,
    AlphabeticalAsc,
    AlphabeticalDesc,
    LengthAsc,
    LengthDesc,
    Shuffle,
}

pub fn parse_in_sep(s: &str) -> Result<InSep, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(InSep::Auto),
        "comma" => Ok(InSep::Comma),
        "newline" | "lines" | "line" => Ok(InSep::Newline),
        "semicolon" => Ok(InSep::Semicolon),
        "space" | "spaces" => Ok(InSep::Space),
        "tab" | "tabs" => Ok(InSep::Tab),
        "pipe" | "pipes" => Ok(InSep::Pipe),
        "custom" => Ok(InSep::Custom),
        other => Err(format!(
            "input_separator {other:?} not supported (auto|comma|newline|semicolon|space|tab|pipe|custom)"
        )),
    }
}

pub fn parse_out_format(s: &str) -> Result<OutFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "comma" | "csv" => Ok(OutFormat::Comma),
        "" | "newline" | "lines" => Ok(OutFormat::Newline),
        "bulleted" | "bullets" | "bullet" => Ok(OutFormat::Bulleted),
        "numbered" | "number" => Ok(OutFormat::Numbered),
        "quoted" | "quote" => Ok(OutFormat::Quoted),
        "space" | "spaces" => Ok(OutFormat::Space),
        "tab" | "tabs" => Ok(OutFormat::Tab),
        "pipe" | "pipes" => Ok(OutFormat::Pipe),
        "json" | "array" => Ok(OutFormat::JSON),
        "xml" => Ok(OutFormat::XML),
        "sql" | "in" => Ok(OutFormat::SQL),
        "custom" => Ok(OutFormat::Custom),
        other => Err(format!(
            "output_format {other:?} not supported (comma|newline|bulleted|numbered|quoted|space|tab|pipe|json|xml|sql|custom)"
        )),
    }
}

pub fn parse_case_transform(s: &str) -> Result<CaseTransform, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(CaseTransform::None),
        "lowercase" | "lower" => Ok(CaseTransform::Lowercase),
        "uppercase" | "upper" => Ok(CaseTransform::Uppercase),
        "titlecase" | "title" => Ok(CaseTransform::Titlecase),
        other => Err(format!(
            "case_transform {other:?} not supported (none|lowercase|uppercase|titlecase)"
        )),
    }
}

pub fn parse_sort_mode(s: &str) -> Result<SortMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(SortMode::None),
        "asc" | "alphabetical" | "alphabetical_asc" => Ok(SortMode::AlphabeticalAsc),
        "desc" | "alphabetical_desc" => Ok(SortMode::AlphabeticalDesc),
        "length_asc" | "length" => Ok(SortMode::LengthAsc),
        "length_desc" => Ok(SortMode::LengthDesc),
        "shuffle" | "randomize" | "random" => Ok(SortMode::Shuffle),
        other => Err(format!(
            "sort_mode {other:?} not supported (none|asc|desc|length_asc|length_desc|shuffle)"
        )),
    }
}

fn to_title_case(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().collect::<String>() + &chars.as_str().to_lowercase(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn shuffle_items<T>(items: &mut [T], seed: u64) {
    let mut state = seed;
    let mut next_u32 = || {
        state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (state >> 32) as u32
    };
    let n = items.len();
    for i in (1..n).rev() {
        let j = (next_u32() as usize) % (i + 1);
        items.swap(i, j);
    }
}

fn split(input: &str, sep: InSep, custom_sep: &str) -> Vec<String> {
    let effective = match sep {
        InSep::Auto => {
            if input.contains('\n') {
                InSep::Newline
            } else if input.contains(',') {
                InSep::Comma
            } else if input.contains(';') {
                InSep::Semicolon
            } else if input.contains('|') {
                InSep::Pipe
            } else if input.contains('\t') {
                InSep::Tab
            } else {
                InSep::Newline
            }
        }
        other => other,
    };
    let raw: Vec<String> = match effective {
        InSep::Comma => input.split(',').map(|s| s.to_string()).collect(),
        InSep::Newline => input.split('\n').map(|s| s.to_string()).collect(),
        InSep::Semicolon => input.split(';').map(|s| s.to_string()).collect(),
        InSep::Space => input.split_whitespace().map(|s| s.to_string()).collect(),
        InSep::Tab => input.split('\t').map(|s| s.to_string()).collect(),
        InSep::Pipe => input.split('|').map(|s| s.to_string()).collect(),
        InSep::Custom => {
            if custom_sep.is_empty() {
                input.split('\n').map(|s| s.to_string()).collect()
            } else {
                input.split(custom_sep).map(|s| s.to_string()).collect()
            }
        }
        InSep::Auto => unreachable!(),
    };
    raw.into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn convert(
    input: &str,
    in_sep: InSep,
    custom_in_sep: &str,
    out_format: OutFormat,
    custom_out_sep: &str,
    sort_mode: SortMode,
    dedupe: bool,
    case_transform: CaseTransform,
    prefix: &str,
    suffix: &str,
    seed: u64,
) -> Result<String, String> {
    let mut items = split(input, in_sep, custom_in_sep);
    if items.is_empty() {
        return Err("no list items found".into());
    }

    if dedupe {
        let mut seen = std::collections::HashSet::new();
        items.retain(|i| seen.insert(i.clone()));
    }

    for item in &mut items {
        *item = match case_transform {
            CaseTransform::None => item.clone(),
            CaseTransform::Lowercase => item.to_lowercase(),
            CaseTransform::Uppercase => item.to_uppercase(),
            CaseTransform::Titlecase => to_title_case(item),
        };
    }

    if !prefix.is_empty() || !suffix.is_empty() {
        for item in &mut items {
            *item = format!("{}{}{}", prefix, item, suffix);
        }
    }

    match sort_mode {
        SortMode::None => {}
        SortMode::AlphabeticalAsc => {
            items.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
        }
        SortMode::AlphabeticalDesc => {
            items.sort_by(|a, b| b.to_lowercase().cmp(&a.to_lowercase()));
        }
        SortMode::LengthAsc => {
            items.sort_by(|a, b| a.len().cmp(&b.len()).then_with(|| a.to_lowercase().cmp(&b.to_lowercase())));
        }
        SortMode::LengthDesc => {
            items.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| b.to_lowercase().cmp(&a.to_lowercase())));
        }
        SortMode::Shuffle => {
            shuffle_items(&mut items, seed);
        }
    }

    let out = match out_format {
        OutFormat::Comma => items.join(", "),
        OutFormat::Newline => items.join("\n"),
        OutFormat::Space => items.join(" "),
        OutFormat::Tab => items.join("\t"),
        OutFormat::Pipe => items.join("|"),
        OutFormat::Bulleted => items.iter().map(|i| format!("- {i}")).collect::<Vec<_>>().join("\n"),
        OutFormat::Numbered => items
            .iter()
            .enumerate()
            .map(|(n, i)| format!("{}. {i}", n + 1))
            .collect::<Vec<_>>()
            .join("\n"),
        OutFormat::Quoted => items
            .iter()
            .map(|i| format!("\"{}\"", i.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(", "),
        OutFormat::JSON => {
            let escaped: Vec<String> = items
                .iter()
                .map(|i| format!("\"{}\"", i.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n").replace('\r', "\\r").replace('\t', "\\t")))
                .collect();
            format!("[{}]", escaped.join(", "))
        }
        OutFormat::XML => {
            items.iter().map(|i| format!("<item>{}</item>", html_escape(i))).collect::<Vec<_>>().join("\n")
        }
        OutFormat::SQL => {
            let escaped: Vec<String> = items
                .iter()
                .map(|i| format!("'{}'", i.replace('\'', "''")))
                .collect();
            format!("({})", escaped.join(", "))
        }
        OutFormat::Custom => {
            items.join(custom_out_sep)
        }
    };

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_conversions() {
        let out = convert("a, b, c", InSep::Auto, "", OutFormat::Newline, "", SortMode::None, false, CaseTransform::None, "", "", 0).unwrap();
        assert_eq!(out, "a\nb\nc");

        let out = convert("a\nb\nc", InSep::Auto, "", OutFormat::Comma, "", SortMode::None, false, CaseTransform::None, "", "", 0).unwrap();
        assert_eq!(out, "a, b, c");
    }

    #[test]
    fn test_custom_delimiters() {
        let out = convert("a||b||c", InSep::Custom, "||", OutFormat::Custom, " - ", SortMode::None, false, CaseTransform::None, "", "", 0).unwrap();
        assert_eq!(out, "a - b - c");
    }

    #[test]
    fn test_case_transform() {
        let out = convert("aPPlE, baNaNa", InSep::Comma, "", OutFormat::Comma, "", SortMode::None, false, CaseTransform::Titlecase, "", "", 0).unwrap();
        assert_eq!(out, "Apple, Banana");

        let out = convert("aPPlE, baNaNa", InSep::Comma, "", OutFormat::Comma, "", SortMode::None, false, CaseTransform::Lowercase, "", "", 0).unwrap();
        assert_eq!(out, "apple, banana");
    }

    #[test]
    fn test_prefix_suffix() {
        let out = convert("a, b", InSep::Comma, "", OutFormat::Comma, "", SortMode::None, false, CaseTransform::None, "pre_", "_post", 0).unwrap();
        assert_eq!(out, "pre_a_post, pre_b_post");
    }

    #[test]
    fn test_advanced_sorting() {
        // Length sorting
        let out = convert("apple, ox, banana", InSep::Comma, "", OutFormat::Comma, "", SortMode::LengthAsc, false, CaseTransform::None, "", "", 0).unwrap();
        assert_eq!(out, "ox, apple, banana");

        // Reverse alphabetical
        let out = convert("a, c, b", InSep::Comma, "", OutFormat::Comma, "", SortMode::AlphabeticalDesc, false, CaseTransform::None, "", "", 0).unwrap();
        assert_eq!(out, "c, b, a");
    }

    #[test]
    fn test_xml_sql_json() {
        let out = convert("a, b", InSep::Comma, "", OutFormat::JSON, "", SortMode::None, false, CaseTransform::None, "", "", 0).unwrap();
        assert_eq!(out, "[\"a\", \"b\"]");

        let out = convert("a'b, c", InSep::Comma, "", OutFormat::SQL, "", SortMode::None, false, CaseTransform::None, "", "", 0).unwrap();
        assert_eq!(out, "('a''b', 'c')");

        let out = convert("a<b, c", InSep::Comma, "", OutFormat::XML, "", SortMode::None, false, CaseTransform::None, "", "", 0).unwrap();
        assert_eq!(out, "<item>a&lt;b</item>\n<item>c</item>");
    }
}
