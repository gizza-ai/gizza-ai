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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutFormat {
    Comma,
    Newline,
    Bulleted,
    Numbered,
    Quoted,
    Space,
}

pub fn parse_in_sep(s: &str) -> Result<InSep, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(InSep::Auto),
        "comma" => Ok(InSep::Comma),
        "newline" | "lines" | "line" => Ok(InSep::Newline),
        "semicolon" => Ok(InSep::Semicolon),
        "space" | "spaces" => Ok(InSep::Space),
        other => Err(format!("input_separator {other:?} not supported (auto|comma|newline|semicolon|space)")),
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
        other => Err(format!("output_format {other:?} not supported (comma|newline|bulleted|numbered|quoted|space)")),
    }
}

fn split(input: &str, sep: InSep) -> Vec<String> {
    let effective = match sep {
        InSep::Auto => {
            if input.contains('\n') {
                InSep::Newline
            } else if input.contains(',') {
                InSep::Comma
            } else if input.contains(';') {
                InSep::Semicolon
            } else {
                InSep::Newline
            }
        }
        other => other,
    };
    let raw: Vec<&str> = match effective {
        InSep::Comma => input.split(',').collect(),
        InSep::Newline => input.split('\n').collect(),
        InSep::Semicolon => input.split(';').collect(),
        InSep::Space => input.split_whitespace().collect(),
        InSep::Auto => unreachable!(),
    };
    raw.into_iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// Reformat `input`. Splits per `in_sep`, optionally sorts (case-insensitive) and
/// dedupes (preserving first occurrence), then renders as `out_format`.
pub fn convert(
    input: &str,
    in_sep: InSep,
    out_format: OutFormat,
    sort: bool,
    dedupe: bool,
) -> Result<String, String> {
    let mut items = split(input, in_sep);
    if items.is_empty() {
        return Err("no list items found".into());
    }
    if dedupe {
        let mut seen = std::collections::HashSet::new();
        items.retain(|i| seen.insert(i.clone()));
    }
    if sort {
        items.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    }

    let out = match out_format {
        OutFormat::Comma => items.join(", "),
        OutFormat::Newline => items.join("\n"),
        OutFormat::Space => items.join(" "),
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
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comma_to_newline() {
        let out = convert("a, b, c", InSep::Auto, OutFormat::Newline, false, false).unwrap();
        assert_eq!(out, "a\nb\nc");
    }

    #[test]
    fn newline_to_comma() {
        let out = convert("a\nb\nc", InSep::Auto, OutFormat::Comma, false, false).unwrap();
        assert_eq!(out, "a, b, c");
    }

    #[test]
    fn bulleted_and_numbered() {
        assert_eq!(
            convert("a,b", InSep::Comma, OutFormat::Bulleted, false, false).unwrap(),
            "- a\n- b"
        );
        assert_eq!(
            convert("a,b", InSep::Comma, OutFormat::Numbered, false, false).unwrap(),
            "1. a\n2. b"
        );
    }

    #[test]
    fn quoted_escapes() {
        let out = convert(r#"a,he said "hi""#, InSep::Comma, OutFormat::Quoted, false, false).unwrap();
        assert_eq!(out, r#""a", "he said \"hi\"""#);
    }

    #[test]
    fn sort_and_dedupe() {
        let out = convert("b, a, B, c, a", InSep::Comma, OutFormat::Comma, true, true).unwrap();
        // dedupe removes the exact dup 'a' → [b, a, B, c]; stable case-insensitive
        // sort keeps b before B (equal keys retain original order) → a, b, B, c.
        assert_eq!(out, "a, b, B, c");
    }

    #[test]
    fn trims_and_drops_empties() {
        let out = convert("  a , , b ,", InSep::Comma, OutFormat::Comma, false, false).unwrap();
        assert_eq!(out, "a, b");
    }

    #[test]
    fn space_split() {
        let out = convert("one two   three", InSep::Space, OutFormat::Numbered, false, false).unwrap();
        assert_eq!(out, "1. one\n2. two\n3. three");
    }

    #[test]
    fn errors() {
        assert!(convert("   ", InSep::Auto, OutFormat::Comma, false, false).is_err());
        assert!(parse_in_sep("pipe").is_err());
        assert!(parse_out_format("xml").is_err());
    }
}
