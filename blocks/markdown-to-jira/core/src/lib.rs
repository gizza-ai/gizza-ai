//! markdown-to-jira core — pure Markdown ↔ Jira wiki markup conversion.
//!
//! This is intentionally dependency-free and deterministic so it runs in the
//! wafer/wasm sandbox. It covers the common Jira wiki markup subset used by the
//! public converter tools: headings, emphasis, code, links/images, lists, tables,
//! quotes, panels, and horizontal rules. It is not an Atlassian Document Format
//! (ADF) renderer and does not try to preserve every Jira-only macro.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    MarkdownToJira,
    JiraToMarkdown,
}

impl Direction {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "md-to-jira" | "markdown-to-jira" | "markdown_to_jira" => Ok(Self::MarkdownToJira),
            "jira-to-md" | "jira-to-markdown" | "jira_to_markdown" => Ok(Self::JiraToMarkdown),
            other => Err(format!(
                "direction must be \"md-to-jira\" or \"jira-to-md\" (got {other:?})"
            )),
        }
    }
}

pub fn convert(
    input: &str,
    direction: &str,
    heading_offset: i64,
    panel_blockquotes: bool,
) -> Result<String, String> {
    if heading_offset < 0 || heading_offset > 5 {
        return Err("heading_offset must be between 0 and 5".into());
    }
    match Direction::parse(direction)? {
        Direction::MarkdownToJira => Ok(markdown_to_jira(
            input,
            heading_offset as usize,
            panel_blockquotes,
        )),
        Direction::JiraToMarkdown => Ok(jira_to_markdown(input, panel_blockquotes)),
    }
}

/// Back-compat wrapper used by the scaffold before the real descriptor exists.
pub fn run(input: &str) -> Result<String, String> {
    convert(input, "md-to-jira", 0, true)
}

fn markdown_to_jira(input: &str, heading_offset: usize, panel_blockquotes: bool) -> String {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();
    let mut out = Vec::new();
    let mut i = 0usize;
    let mut in_code = false;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_end();
        let t = trimmed.trim_start();

        if let Some(lang) = t.strip_prefix("```") {
            if in_code {
                out.push("{code}".to_string());
                in_code = false;
            } else {
                let lang = lang.trim();
                if lang.is_empty() {
                    out.push("{code}".to_string());
                } else {
                    out.push(format!("{{code:{}}}", lang));
                }
                in_code = true;
            }
            i += 1;
            continue;
        }
        if in_code {
            out.push(line.to_string());
            i += 1;
            continue;
        }

        if is_table_header_at(&lines, i) {
            let headers = split_md_table(lines[i]);
            out.push(format!(
                "||{}||",
                headers
                    .iter()
                    .map(|c| inline_md_to_jira(c.trim()))
                    .collect::<Vec<_>>()
                    .join("||")
            ));
            i += 2; // skip delimiter
            while i < lines.len() && looks_like_md_table_row(lines[i]) {
                let cells = split_md_table(lines[i]);
                out.push(format!(
                    "|{}|",
                    cells
                        .iter()
                        .map(|c| inline_md_to_jira(c.trim()))
                        .collect::<Vec<_>>()
                        .join("|")
                ));
                i += 1;
            }
            continue;
        }

        if panel_blockquotes {
            if let Some((macro_name, body)) = markdown_panel_quote(t) {
                out.push(format!("{{{macro_name}}}"));
                if !body.is_empty() {
                    out.push(inline_md_to_jira(body));
                }
                out.push(format!("{{{macro_name}}}"));
                i += 1;
                continue;
            }
        }

        if let Some(h) = markdown_heading(t, heading_offset) {
            out.push(h);
        } else if t == "---" || t == "***" || t == "___" {
            out.push("----".to_string());
        } else if let Some(rest) = t.strip_prefix("> ").or_else(|| t.strip_prefix('>')) {
            out.push(format!("bq. {}", inline_md_to_jira(rest.trim_start())));
        } else if let Some((marker, body)) = markdown_list_item(line) {
            out.push(format!(
                "{} {}",
                marker,
                inline_md_to_jira(body.trim_start())
            ));
        } else {
            out.push(inline_md_to_jira(trimmed));
        }
        i += 1;
    }
    out.join("\n").trim_end().to_string()
}

fn jira_to_markdown(input: &str, panel_blockquotes: bool) -> String {
    let mut out = Vec::new();
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut in_quote = false;
    let mut in_panel: Option<&'static str> = None;

    for raw in input.replace("\r\n", "\n").replace('\r', "\n").lines() {
        let t = raw.trim_end();

        if let Some(lang) = jira_code_open(t) {
            if in_code {
                out.push("```".to_string());
                in_code = false;
                code_lang.clear();
            } else {
                code_lang = lang.unwrap_or("").to_string();
                out.push(format!("```{}", code_lang));
                in_code = true;
            }
            continue;
        }
        if in_code {
            out.push(raw.to_string());
            continue;
        }

        if t == "{quote}" {
            in_quote = !in_quote;
            continue;
        }
        if let Some(kind) = panel_macro(t) {
            if panel_blockquotes {
                if in_panel == Some(kind) {
                    in_panel = None;
                } else {
                    in_panel = Some(kind);
                }
                continue;
            }
        }

        if let Some(rest) = t.strip_prefix("bq. ") {
            out.push(format!("> {}", inline_jira_to_md(rest)));
        } else if let Some(h) = jira_heading(t) {
            out.push(h);
        } else if t == "----" {
            out.push("---".to_string());
        } else if let Some(row) = jira_table_header(t) {
            out.push(row.clone());
            let cells = row.matches('|').count().saturating_sub(1);
            if cells > 0 {
                out.push(format!(
                    "|{}|",
                    (0..cells).map(|_| " --- ").collect::<Vec<_>>().join("|")
                ));
            }
        } else if let Some(row) = jira_table_row(t) {
            out.push(row);
        } else if let Some(item) = jira_list_item(t) {
            out.push(item);
        } else if in_quote {
            out.push(format!("> {}", inline_jira_to_md(t)));
        } else if let Some(kind) = in_panel {
            out.push(format!("> {}: {}", title_case(kind), inline_jira_to_md(t)));
        } else {
            out.push(inline_jira_to_md(t));
        }
    }
    out.join("\n").trim_end().to_string()
}

fn markdown_heading(t: &str, offset: usize) -> Option<String> {
    let hashes = t.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && t.chars().nth(hashes) == Some(' ') {
        let level = (hashes + offset).min(6);
        Some(format!(
            "h{}. {}",
            level,
            inline_md_to_jira(t[hashes + 1..].trim())
        ))
    } else {
        None
    }
}

fn jira_heading(t: &str) -> Option<String> {
    let bytes = t.as_bytes();
    if bytes.len() > 4
        && bytes[0] == b'h'
        && bytes[1].is_ascii_digit()
        && bytes[2] == b'.'
        && bytes[3] == b' '
    {
        let level = (bytes[1] - b'0').clamp(1, 6) as usize;
        Some(format!(
            "{} {}",
            "#".repeat(level),
            inline_jira_to_md(&t[4..])
        ))
    } else {
        None
    }
}

fn markdown_list_item(line: &str) -> Option<(String, &str)> {
    let indent = line
        .chars()
        .take_while(|c| *c == ' ' || *c == '\t')
        .fold(0usize, |n, c| n + if c == '\t' { 4 } else { 1 });
    let depth = indent / 2 + 1;
    let t = line.trim_start();
    for marker in ["- ", "* ", "+ "] {
        if let Some(rest) = t.strip_prefix(marker) {
            return Some(("*".repeat(depth), rest));
        }
    }
    if let Some(dot) = t.find(". ") {
        if dot > 0 && t[..dot].chars().all(|c| c.is_ascii_digit()) {
            return Some(("#".repeat(depth), &t[dot + 2..]));
        }
    }
    None
}

fn jira_list_item(t: &str) -> Option<String> {
    let markers: String = t.chars().take_while(|c| *c == '*' || *c == '#').collect();
    if markers.is_empty() || !t[markers.len()..].starts_with(' ') {
        return None;
    }
    let depth = markers.len().saturating_sub(1);
    let last = markers.chars().last().unwrap_or('*');
    let bullet = if last == '#' { "1." } else { "-" };
    Some(format!(
        "{}{} {}",
        "  ".repeat(depth),
        bullet,
        inline_jira_to_md(t[markers.len() + 1..].trim_start())
    ))
}

fn markdown_panel_quote(t: &str) -> Option<(&'static str, &str)> {
    let rest = t
        .strip_prefix("> ")
        .or_else(|| t.strip_prefix('>'))?
        .trim_start();
    for (prefix, macro_name) in [
        ("Note:", "note"),
        ("Info:", "info"),
        ("Warning:", "warning"),
        ("Warn:", "warning"),
        ("Tip:", "tip"),
    ] {
        if let Some(body) = rest.strip_prefix(prefix) {
            return Some((macro_name, body.trim_start()));
        }
    }
    None
}

fn panel_macro(t: &str) -> Option<&'static str> {
    match t {
        "{note}" => Some("note"),
        "{info}" => Some("info"),
        "{warning}" => Some("warning"),
        "{tip}" => Some("tip"),
        _ => None,
    }
}

fn title_case(kind: &str) -> &'static str {
    match kind {
        "note" => "Note",
        "info" => "Info",
        "warning" => "Warning",
        "tip" => "Tip",
        _ => "Note",
    }
}

fn jira_code_open(t: &str) -> Option<Option<&str>> {
    if t == "{code}" {
        Some(None)
    } else if t.starts_with("{code:") && t.ends_with('}') {
        Some(Some(&t[6..t.len() - 1]))
    } else {
        None
    }
}

fn looks_like_md_table_row(line: &str) -> bool {
    let t = line.trim();
    t.starts_with('|') && t.ends_with('|') && t.matches('|').count() >= 2
}

fn is_table_header_at(lines: &[&str], i: usize) -> bool {
    i + 1 < lines.len() && looks_like_md_table_row(lines[i]) && is_md_table_delim(lines[i + 1])
}

fn is_md_table_delim(line: &str) -> bool {
    let cells = split_md_table(line);
    !cells.is_empty()
        && cells.iter().all(|c| {
            let c = c.trim();
            c.len() >= 3 && c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
        })
}

fn split_md_table(line: &str) -> Vec<&str> {
    line.trim().trim_matches('|').split('|').collect()
}

fn jira_table_header(t: &str) -> Option<String> {
    if t.starts_with("||") && t.ends_with("||") {
        let cells: Vec<String> = t
            .trim_matches('|')
            .split("||")
            .map(|c| inline_jira_to_md(c.trim()))
            .collect();
        Some(format!("| {} |", cells.join(" | ")))
    } else {
        None
    }
}

fn jira_table_row(t: &str) -> Option<String> {
    if t.starts_with('|') && t.ends_with('|') && !t.starts_with("||") {
        let cells: Vec<String> = t
            .trim_matches('|')
            .split('|')
            .map(|c| inline_jira_to_md(c.trim()))
            .collect();
        Some(format!("| {} |", cells.join(" | ")))
    } else {
        None
    }
}

fn inline_md_to_jira(s: &str) -> String {
    let s = replace_images(s, true);
    let s = replace_links(&s, true);
    let s = replace_pair(&s, "`", "{{", "}}");
    let s = replace_pair(&s, "~~", "-", "-");
    let s = replace_pair(&s, "**", "*", "*");
    replace_underscore_italics(&s, true)
}

fn inline_jira_to_md(s: &str) -> String {
    let s = replace_images(s, false);
    let s = replace_links(&s, false);
    let s = replace_braces_code(&s);
    let s = replace_pair(&s, "*", "**", "**");
    let s = replace_pair(&s, "_", "*", "*");
    replace_pair(&s, "-", "~~", "~~")
}

fn replace_pair(s: &str, marker: &str, open: &str, close: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    let mut opening = true;
    while let Some(pos) = rest.find(marker) {
        out.push_str(&rest[..pos]);
        out.push_str(if opening { open } else { close });
        opening = !opening;
        rest = &rest[pos + marker.len()..];
    }
    out.push_str(rest);
    out
}

fn replace_underscore_italics(s: &str, _md_to_jira: bool) -> String {
    replace_pair(s, "_", "_", "_")
}

fn replace_braces_code(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        if let Some(end) = rest[start + 2..].find("}}") {
            out.push('`');
            out.push_str(&rest[start + 2..start + 2 + end]);
            out.push('`');
            rest = &rest[start + 2 + end + 2..];
        } else {
            out.push_str(&rest[start..]);
            return out;
        }
    }
    out.push_str(rest);
    out
}

fn replace_links(s: &str, md_to_jira: bool) -> String {
    if md_to_jira {
        replace_md_links(s)
    } else {
        replace_jira_links(s)
    }
}

fn replace_md_links(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        if open > 0 && rest.as_bytes()[open - 1] == b'!' {
            out.push_str(&rest[open..open + 1]);
            rest = &rest[open + 1..];
            continue;
        }
        if let Some(mid) = rest[open..].find("](") {
            let mid = open + mid;
            if let Some(close) = rest[mid + 2..].find(')') {
                let text = &rest[open + 1..mid];
                let url = &rest[mid + 2..mid + 2 + close];
                out.push_str(&format!("[{text}|{url}]"));
                rest = &rest[mid + 2 + close + 1..];
                continue;
            }
        }
        out.push('[');
        rest = &rest[open + 1..];
    }
    out.push_str(rest);
    out
}

fn replace_jira_links(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(open) = rest.find('[') {
        out.push_str(&rest[..open]);
        if let Some(close) = rest[open + 1..].find(']') {
            let body = &rest[open + 1..open + 1 + close];
            if let Some(pipe) = body.find('|') {
                out.push_str(&format!("[{}]({})", &body[..pipe], &body[pipe + 1..]));
            } else {
                out.push_str(&format!("[{}]({})", body, body));
            }
            rest = &rest[open + 1 + close + 1..];
        } else {
            out.push('[');
            rest = &rest[open + 1..];
        }
    }
    out.push_str(rest);
    out
}

fn replace_images(s: &str, md_to_jira: bool) -> String {
    if md_to_jira {
        let mut out = String::new();
        let mut rest = s;
        while let Some(open) = rest.find("![") {
            out.push_str(&rest[..open]);
            if let Some(mid) = rest[open..].find("](") {
                let mid = open + mid;
                if let Some(close) = rest[mid + 2..].find(')') {
                    let url = &rest[mid + 2..mid + 2 + close];
                    out.push_str(&format!("!{url}!"));
                    rest = &rest[mid + 2 + close + 1..];
                    continue;
                }
            }
            out.push_str("![");
            rest = &rest[open + 2..];
        }
        out.push_str(rest);
        out
    } else {
        let mut out = String::new();
        let mut rest = s;
        while let Some(start) = rest.find('!') {
            out.push_str(&rest[..start]);
            if let Some(end) = rest[start + 1..].find('!') {
                let url = &rest[start + 1..start + 1 + end];
                if !url.is_empty() && !url.contains(' ') {
                    out.push_str(&format!("![]({url})"));
                    rest = &rest[start + 1 + end + 1..];
                    continue;
                }
            }
            out.push('!');
            rest = &rest[start + 1..];
        }
        out.push_str(rest);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_to_jira_happy_path() {
        let input = "# Title\n\n**Bold** and `code`\n\n- one\n- two\n\n[site](https://example.com)";
        let out = convert(input, "md-to-jira", 0, true).unwrap();
        assert_eq!(
            out,
            "h1. Title\n\n*Bold* and {{code}}\n\n* one\n* two\n\n[site|https://example.com]"
        );
    }

    #[test]
    fn jira_to_markdown_happy_path() {
        let input = "h2. Title\n\n*Bold* and {{code}}\n\n# one\n# two";
        let out = convert(input, "jira-to-md", 0, true).unwrap();
        assert_eq!(out, "## Title\n\n**Bold** and `code`\n\n1. one\n1. two");
    }

    #[test]
    fn code_fences_and_language() {
        let input = "```rust\nfn main() {}\n```";
        assert_eq!(
            convert(input, "md-to-jira", 0, true).unwrap(),
            "{code:rust}\nfn main() {}\n{code}"
        );
        assert_eq!(
            convert("{code:rust}\nfn main() {}\n{code}", "jira-to-md", 0, true).unwrap(),
            "```rust\nfn main() {}\n```"
        );
    }

    #[test]
    fn table_conversion() {
        let md = "| Name | Value |\n| --- | --- |\n| a | b |";
        assert_eq!(
            convert(md, "md-to-jira", 0, true).unwrap(),
            "||Name||Value||\n|a|b|"
        );
    }

    #[test]
    fn panels_and_heading_offset() {
        let md = "> Warning: check this\n## Section";
        assert_eq!(
            convert(md, "md-to-jira", 1, true).unwrap(),
            "{warning}\ncheck this\n{warning}\nh3. Section"
        );
    }

    #[test]
    fn invalid_direction_errors() {
        let err = convert("x", "sideways", 0, true).unwrap_err();
        assert!(err.contains("direction must be"));
    }

    #[test]
    fn invalid_heading_offset_errors() {
        assert_eq!(
            convert("# x", "md-to-jira", 6, true).unwrap_err(),
            "heading_offset must be between 0 and 5"
        );
    }
}
