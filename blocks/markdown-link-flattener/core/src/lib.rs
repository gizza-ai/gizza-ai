//! markdown-link-flattener core — pure compute, shared by the chat skill block and the web page.
//!
//! Removes Markdown inline-link syntax while preserving the prose around it. The
//! parser is intentionally small and deterministic: it handles inline links,
//! images and reference definitions, while leaving code spans and fenced code
//! blocks byte-for-byte alone.

pub const MAX_BYTES: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LinkMode {
    Text,
    TextUrl,
    Url,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ImageMode {
    AltText,
    AltUrl,
    Drop,
    KeepMarkdown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReferenceMode {
    Drop,
    Keep,
}

fn parse_link_mode(s: &str) -> Result<LinkMode, String> {
    Ok(match s.trim() {
        "" | "text" => LinkMode::Text,
        "text_url" => LinkMode::TextUrl,
        "url" => LinkMode::Url,
        other => {
            return Err(format!(
                "link_mode must be text, text_url or url — got '{other}'"
            ))
        }
    })
}

fn parse_image_mode(s: &str) -> Result<ImageMode, String> {
    Ok(match s.trim() {
        "" | "alt_text" => ImageMode::AltText,
        "alt_url" => ImageMode::AltUrl,
        "drop" => ImageMode::Drop,
        "keep_markdown" => ImageMode::KeepMarkdown,
        other => {
            return Err(format!(
                "image_mode must be alt_text, alt_url, drop or keep_markdown — got '{other}'"
            ))
        }
    })
}

fn parse_reference_mode(s: &str) -> Result<ReferenceMode, String> {
    Ok(match s.trim() {
        "" | "drop" => ReferenceMode::Drop,
        "keep" => ReferenceMode::Keep,
        other => {
            return Err(format!(
                "reference_definitions must be drop or keep — got '{other}'"
            ))
        }
    })
}

fn find_matching(bytes: &[u8], mut i: usize, open: u8, close: u8) -> Option<usize> {
    let mut depth = 1usize;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b if b == open => {
                depth += 1;
                i += 1;
            }
            b if b == close => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    None
}

fn split_destination_and_title(raw: &str) -> &str {
    let t = raw.trim();
    if let Some(rest) = t.strip_prefix('<') {
        if let Some(end) = rest.find('>') {
            return &rest[..end];
        }
    }
    t.split_whitespace().next().unwrap_or("")
}

fn starts_reference_definition(line: &str) -> bool {
    let t = line.trim_start();
    if !t.starts_with('[') || t.starts_with("[]") {
        return false;
    }
    let Some(end) = t.find("]:") else {
        return false;
    };
    end > 1 && t[end + 2..].trim_start().len() > 0
}

fn render_link(label: &str, url: &str, mode: LinkMode) -> String {
    match mode {
        LinkMode::Text => label.to_string(),
        LinkMode::TextUrl => {
            if label.trim().is_empty() {
                url.to_string()
            } else if url.trim().is_empty() {
                label.to_string()
            } else {
                format!("{label} ({url})")
            }
        }
        LinkMode::Url => url.to_string(),
    }
}

fn render_image(alt: &str, url: &str, original: &str, mode: ImageMode) -> String {
    match mode {
        ImageMode::AltText => alt.to_string(),
        ImageMode::AltUrl => {
            if alt.trim().is_empty() {
                url.to_string()
            } else if url.trim().is_empty() {
                alt.to_string()
            } else {
                format!("{alt} ({url})")
            }
        }
        ImageMode::Drop => String::new(),
        ImageMode::KeepMarkdown => original.to_string(),
    }
}

fn flatten_inline(
    line: &str,
    link_mode: LinkMode,
    image_mode: ImageMode,
    preserve_code: bool,
) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    let mut in_code = false;
    while i < bytes.len() {
        if preserve_code && bytes[i] == b'`' {
            in_code = !in_code;
            out.push('`');
            i += 1;
            continue;
        }
        if in_code {
            out.push(bytes[i] as char);
            i += 1;
            continue;
        }
        let is_image = bytes[i] == b'!' && i + 1 < bytes.len() && bytes[i + 1] == b'[';
        let label_start = if is_image {
            i + 2
        } else if bytes[i] == b'[' {
            i + 1
        } else {
            usize::MAX
        };
        if label_start != usize::MAX {
            if let Some(label_end) = find_matching(bytes, label_start, b'[', b']') {
                if label_end + 1 < bytes.len() && bytes[label_end + 1] == b'(' {
                    if let Some(dest_end) = find_matching(bytes, label_end + 2, b'(', b')') {
                        let label = &line[label_start..label_end];
                        let dest = &line[label_end + 2..dest_end];
                        let url = split_destination_and_title(dest);
                        if is_image {
                            out.push_str(&render_image(
                                label,
                                url,
                                &line[i..=dest_end],
                                image_mode,
                            ));
                        } else {
                            out.push_str(&render_link(label, url, link_mode));
                        }
                        i = dest_end + 1;
                        continue;
                    }
                }
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}

pub fn run(
    markdown: &str,
    link_mode: &str,
    image_mode: &str,
    reference_definitions: &str,
    preserve_code: bool,
) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("markdown is empty — paste Markdown that contains links to flatten".into());
    }
    if markdown.len() > MAX_BYTES {
        return Err(format!(
            "markdown is {} bytes, over the {MAX_BYTES} byte limit — split it and run the parts separately",
            markdown.len()
        ));
    }
    let link_mode = parse_link_mode(link_mode)?;
    let image_mode = parse_image_mode(image_mode)?;
    let reference_mode = parse_reference_mode(reference_definitions)?;

    let mut out = String::with_capacity(markdown.len());
    let mut in_fence = false;
    for chunk in markdown.split_inclusive('\n') {
        let line = chunk.strip_suffix('\n').unwrap_or(chunk);
        let newline = if chunk.ends_with('\n') { "\n" } else { "" };
        let trimmed = line.trim_start();
        if preserve_code && (trimmed.starts_with("```") || trimmed.starts_with("~~~")) {
            in_fence = !in_fence;
            out.push_str(line);
            out.push_str(newline);
            continue;
        }
        if in_fence {
            out.push_str(line);
            out.push_str(newline);
            continue;
        }
        if reference_mode == ReferenceMode::Drop && starts_reference_definition(line) {
            continue;
        }
        out.push_str(&flatten_inline(line, link_mode, image_mode, preserve_code));
        out.push_str(newline);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flattens_inline_links_to_text() {
        let got = run(
            "Read [the docs](https://example.com/docs) today.",
            "text",
            "alt_text",
            "drop",
            true,
        )
        .unwrap();
        assert_eq!(got, "Read the docs today.");
    }

    #[test]
    fn text_url_mode_keeps_destination() {
        let got = run(
            "Read [the docs](https://example.com/docs \"title\").",
            "text_url",
            "alt_text",
            "drop",
            true,
        )
        .unwrap();
        assert_eq!(got, "Read the docs (https://example.com/docs).");
    }

    #[test]
    fn images_are_configurable() {
        assert_eq!(
            run("Logo ![Acme](logo.png)", "text", "alt_text", "drop", true).unwrap(),
            "Logo Acme"
        );
        assert_eq!(
            run("Logo ![Acme](logo.png)", "text", "alt_url", "drop", true).unwrap(),
            "Logo Acme (logo.png)"
        );
        assert_eq!(
            run("Logo ![Acme](logo.png)", "text", "drop", "drop", true).unwrap(),
            "Logo "
        );
    }

    #[test]
    fn drops_reference_definitions_but_keeps_reference_uses() {
        let md = "See [the docs][docs].\n\n[docs]: https://example.com\n";
        assert_eq!(
            run(md, "text", "alt_text", "drop", true).unwrap(),
            "See [the docs][docs].\n\n"
        );
        assert_eq!(run(md, "text", "alt_text", "keep", true).unwrap(), md);
    }

    #[test]
    fn preserves_code_when_asked() {
        let md = "`[x](y)` and [real](url)\n```\n[a](b)\n```";
        assert_eq!(
            run(md, "text", "alt_text", "drop", true).unwrap(),
            "`[x](y)` and real\n```\n[a](b)\n```"
        );
        assert_eq!(
            run(md, "text", "alt_text", "drop", false).unwrap(),
            "`x` and real\n```\na\n```"
        );
    }

    #[test]
    fn validates_options_and_cap() {
        assert!(run(" ", "text", "alt_text", "drop", true)
            .unwrap_err()
            .contains("empty"));
        assert!(run("[x](y)", "bad", "alt_text", "drop", true)
            .unwrap_err()
            .contains("link_mode"));
        let over = "x".repeat(MAX_BYTES + 1);
        assert!(run(&over, "text", "alt_text", "drop", true)
            .unwrap_err()
            .contains("over"));
    }
}
