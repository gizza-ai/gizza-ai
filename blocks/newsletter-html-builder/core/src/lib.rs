//! newsletter-html-builder core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Turns a plain-text list of newsletter sections into ONE self-contained,
//! email-client-safe HTML document: XHTML transitional doctype, nested
//! `role="presentation"` tables, every style INLINE (so Gmail's `<style>`
//! stripping can't break the layout), an Outlook ghost-table wrapper in a
//! conditional comment, a hidden preheader line, a mobile media query, and an
//! optional `prefers-color-scheme: dark` block.
//!
//! ## Section syntax
//!
//! One section per non-empty line, pipe-separated into up to four parts:
//! `type | content | extra | extra2`. Lines whose first non-space character is
//! `#` are comments.
//!
//! - `heading | Big title`
//! - `subheading | Smaller title`
//! - `text | A paragraph. Supports **bold**, *italic*, [links](https://x.com)
//!   and `\n` for a line break.`
//! - `button | Read more | https://example.com`
//! - `image | https://example.com/a.png | Alt text | https://example.com` (the
//!   4th part is an optional click-through link)
//! - `columns | Left column text | Right column text` (stacks on mobile)
//! - `divider`
//! - `spacer | 24` (height in px, default 24)
//! - `footer | Small print. [Unsubscribe](https://example.com/u)`
//! - `html | <p>raw markup passed through verbatim</p>`

/// Maximum number of sections in one newsletter. Keeps output bounded for the
/// in-browser wasm sandbox. At the cap it succeeds; one over errors.
pub const MAX_SECTIONS: usize = 200;

/// Narrowest / widest content width (px) the builder accepts.
pub const MIN_WIDTH: u32 = 320;
pub const MAX_WIDTH: u32 = 900;

const DEFAULT_WIDTH: u32 = 600;
const DEFAULT_BACKGROUND: &str = "#f4f4f5";
const DEFAULT_CONTENT_BACKGROUND: &str = "#ffffff";
const DEFAULT_TEXT_COLOR: &str = "#1f2937";
const DEFAULT_ACCENT: &str = "#2563eb";
const MUTED: &str = "#6b7280";
const RULE: &str = "#e5e7eb";

/// Dark-mode swaps applied by the `prefers-color-scheme` block.
const DARK_PAGE: &str = "#0b1220";
const DARK_CARD: &str = "#111827";
const DARK_TEXT: &str = "#e5e7eb";
const DARK_MUTED: &str = "#9ca3af";
const DARK_RULE: &str = "#374151";

/// Email-safe font stack for a `font` choice.
fn font_stack(font: &str) -> Result<&'static str, String> {
    Ok(match font.trim() {
        "" | "system" => {
            "-apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Helvetica, Arial, sans-serif"
        }
        "arial" => "Arial, Helvetica, sans-serif",
        "helvetica" => "'Helvetica Neue', Helvetica, Arial, sans-serif",
        "verdana" => "Verdana, Geneva, sans-serif",
        "tahoma" => "Tahoma, Verdana, Segoe, sans-serif",
        "trebuchet" => "'Trebuchet MS', Tahoma, Arial, sans-serif",
        "georgia" => "Georgia, 'Times New Roman', Times, serif",
        "times" => "'Times New Roman', Times, serif",
        "courier" => "'Courier New', Courier, monospace",
        other => {
            return Err(format!(
                "invalid font {other:?}: expected one of system, arial, helvetica, verdana, \
                 tahoma, trebuchet, georgia, times, courier"
            ))
        }
    })
}

/// Escape text for an HTML text node / attribute value.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Validate a colour so it can be interpolated into an inline `style="…"`
/// safely: `#rgb`/`#rrggbb`/`#rrggbbaa` hex, a bare CSS colour keyword, or
/// `transparent`. Anything else (a `url(...)`, a quote, a semicolon) is
/// rejected rather than escaped, because a broken colour silently ruins the
/// rendered email.
fn color(value: &str, field: &str, default: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(default.to_string());
    }
    let ok = if let Some(hex) = v.strip_prefix('#') {
        matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|c| c.is_ascii_hexdigit())
    } else {
        !v.is_empty()
            && v.len() <= 24
            && v.chars().all(|c| c.is_ascii_alphabetic())
    };
    if !ok {
        return Err(format!(
            "invalid {field} colour {v:?}: expected a hex colour like \"#2563eb\" (3, 4, 6 or 8 \
             hex digits) or a CSS colour name like \"white\""
        ));
    }
    Ok(v.to_string())
}

/// Validate a link/image URL: only `http(s)`, `mailto:`, `tel:`, a `#anchor`,
/// or a merge tag (`{{unsubscribe_url}}`) may reach an `href`/`src`. This is
/// what keeps `javascript:` out of generated markup.
fn url(value: &str, what: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(format!("{what} is missing a URL"));
    }
    let ok = v.starts_with("https://")
        || v.starts_with("http://")
        || v.starts_with("mailto:")
        || v.starts_with("tel:")
        || v.starts_with('#')
        || v.starts_with("{{")
        || v.starts_with("*|");
    if !ok {
        return Err(format!(
            "invalid URL {v:?} in {what}: expected it to start with https://, http://, mailto:, \
             tel:, # or a merge tag such as {{{{unsubscribe_url}}}}"
        ));
    }
    Ok(esc(v))
}

/// Render the mini inline syntax (`**bold**`, `*italic*`, `[text](url)`, `\n`)
/// into escaped, email-safe HTML. Unmatched markers stay literal.
fn inline(raw: &str, accent: &str) -> Result<String, String> {
    let b = raw.as_bytes();
    let mut out = String::with_capacity(raw.len() + 16);
    let mut i = 0usize;
    while i < b.len() {
        if b[i..].starts_with(b"\\n") {
            out.push_str("<br />");
            i += 2;
            continue;
        }
        if b[i..].starts_with(b"**") {
            if let Some(end) = find(b, i + 2, b"**") {
                out.push_str("<strong>");
                out.push_str(&inline(&raw[i + 2..end], accent)?);
                out.push_str("</strong>");
                i = end + 2;
                continue;
            }
        }
        if b[i] == b'*' {
            if let Some(end) = find(b, i + 1, b"*") {
                out.push_str("<em>");
                out.push_str(&inline(&raw[i + 1..end], accent)?);
                out.push_str("</em>");
                i = end + 1;
                continue;
            }
        }
        if b[i] == b'[' {
            if let Some(close) = find(b, i + 1, b"](") {
                if let Some(paren) = find(b, close + 2, b")") {
                    let label = &raw[i + 1..close];
                    let href = url(&raw[close + 2..paren], "a [text](url) link")?;
                    out.push_str(&format!(
                        "<a href=\"{href}\" style=\"color:{accent};text-decoration:underline;\">{}</a>",
                        inline(label, accent)?
                    ));
                    i = paren + 1;
                    continue;
                }
            }
        }
        // Not a marker: escape one character.
        let ch = raw[i..].chars().next().expect("char boundary");
        out.push_str(&esc(&ch.to_string()));
        i += ch.len_utf8();
    }
    Ok(out)
}

/// Byte index of `needle` in `hay` at or after `from`.
fn find(hay: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    if from > hay.len() {
        return None;
    }
    hay[from..]
        .windows(needle.len())
        .position(|w| w == needle)
        .map(|p| p + from)
}

/// One parsed section line.
struct Section<'a> {
    kind: &'a str,
    /// Everything after the FIRST pipe, verbatim (trimmed). Single-content
    /// types (text, heading, footer, raw html) use this so a paragraph may
    /// contain pipes.
    rest: &'a str,
    /// `rest` split on pipes, for the multi-field types (button, image,
    /// columns).
    parts: Vec<&'a str>,
    line_no: usize,
}

fn parse_sections(input: &str) -> Result<Vec<Section<'_>>, String> {
    let mut out = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let (kind, rest) = match trimmed.split_once('|') {
            Some((k, r)) => (k.trim(), r.trim()),
            None => (trimmed, ""),
        };
        out.push(Section {
            kind,
            rest,
            parts: rest.split('|').map(|p| p.trim()).collect(),
            line_no: idx + 1,
        });
    }
    if out.is_empty() {
        return Err("no sections: add at least one line such as \"heading | Hello\" \
                    (types: heading, subheading, text, button, image, columns, divider, \
                    spacer, footer, html)"
            .into());
    }
    if out.len() > MAX_SECTIONS {
        return Err(format!(
            "too many sections: {} (max {MAX_SECTIONS})",
            out.len()
        ));
    }
    Ok(out)
}

/// Style shared by body copy cells.
fn body_style(stack: &str, size: u32, line: u32, colour: &str) -> String {
    format!(
        "font-family:{stack};font-size:{size}px;line-height:{line}px;color:{colour};"
    )
}

#[allow(clippy::too_many_arguments)]
fn render_section(
    s: &Section<'_>,
    stack: &str,
    text_color: &str,
    accent: &str,
    dark: bool,
    content_width: u32,
) -> Result<String, String> {
    let text_cls = if dark { " class=\"dm-text\"" } else { "" };
    let muted_cls = if dark { " class=\"dm-muted\"" } else { "" };
    let part = |n: usize| s.parts.get(n).copied().unwrap_or("").trim();

    let row = match s.kind {
        "heading" | "h1" => {
            let t = s.rest;
            if t.is_empty() {
                return Err(err_at(s, "heading needs text, e.g. \"heading | Monthly update\""));
            }
            format!(
                "    <tr>\n      <td{text_cls} align=\"left\" style=\"padding:32px 32px 8px 32px;{} \
                 font-weight:700;\">{}</td>\n    </tr>\n",
                body_style(stack, 28, 36, text_color),
                inline(t, accent)?
            )
        }
        "subheading" | "h2" => {
            let t = s.rest;
            if t.is_empty() {
                return Err(err_at(s, "subheading needs text"));
            }
            format!(
                "    <tr>\n      <td{text_cls} align=\"left\" style=\"padding:20px 32px 4px 32px;{} \
                 font-weight:600;\">{}</td>\n    </tr>\n",
                body_style(stack, 20, 28, text_color),
                inline(t, accent)?
            )
        }
        "text" | "paragraph" | "p" => {
            let t = s.rest;
            if t.is_empty() {
                return Err(err_at(s, "text needs some words, e.g. \"text | Hello there\""));
            }
            format!(
                "    <tr>\n      <td{text_cls} align=\"left\" style=\"padding:8px 32px;{}\">{}</td>\n    </tr>\n",
                body_style(stack, 16, 26, text_color),
                inline(t, accent)?
            )
        }
        "button" | "cta" => {
            let label = part(0);
            if label.is_empty() {
                return Err(err_at(
                    s,
                    "button needs a label and a URL, e.g. \"button | Read more | https://example.com\"",
                ));
            }
            let href = url(part(1), "the button").map_err(|e| err_at(s, &e))?;
            format!(
                "    <tr>\n      <td align=\"center\" style=\"padding:24px 32px;\">\n\
                 \x20       <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\">\n\
                 \x20         <tr>\n\
                 \x20           <td align=\"center\" bgcolor=\"{accent}\" style=\"border-radius:6px;background-color:{accent};\">\n\
                 \x20             <a href=\"{href}\" style=\"display:inline-block;padding:14px 28px;{} \
                 font-weight:600;text-decoration:none;border-radius:6px;\">{}</a>\n\
                 \x20           </td>\n\
                 \x20         </tr>\n\
                 \x20       </table>\n      </td>\n    </tr>\n",
                body_style(stack, 16, 20, "#ffffff"),
                inline(label, accent)?
            )
        }
        "image" | "img" => {
            let src = url(part(0), "the image").map_err(|e| err_at(s, &e))?;
            let alt = esc(part(1));
            let img = format!(
                "<img src=\"{src}\" alt=\"{alt}\" width=\"{content_width}\" \
                 style=\"display:block;width:100%;max-width:{content_width}px;height:auto;border:0;\" />"
            );
            let inner = if part(2).is_empty() {
                img
            } else {
                let href = url(part(2), "the image link").map_err(|e| err_at(s, &e))?;
                format!("<a href=\"{href}\" style=\"text-decoration:none;\">{img}</a>")
            };
            format!(
                "    <tr>\n      <td align=\"center\" style=\"padding:0;font-size:0;line-height:0;\">{inner}</td>\n    </tr>\n"
            )
        }
        "columns" | "two-column" => {
            let left = part(0);
            let right = part(1);
            if left.is_empty() || right.is_empty() {
                return Err(err_at(
                    s,
                    "columns needs both column texts, e.g. \"columns | Left copy | Right copy\"",
                ));
            }
            format!(
                "    <tr>\n      <td style=\"padding:12px 32px;\">\n\
                 \x20       <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"100%\" style=\"width:100%;\">\n\
                 \x20         <tr>\n\
                 \x20           <td{col_cls_l} width=\"50%\" valign=\"top\" style=\"padding:0 10px 0 0;{style}\">{l}</td>\n\
                 \x20           <td{col_cls_r} width=\"50%\" valign=\"top\" style=\"padding:0 0 0 10px;{style}\">{r}</td>\n\
                 \x20         </tr>\n\
                 \x20       </table>\n      </td>\n    </tr>\n",
                col_cls_l = if dark { " class=\"sm-stack dm-text\"" } else { " class=\"sm-stack\"" },
                col_cls_r = if dark { " class=\"sm-stack dm-text\"" } else { " class=\"sm-stack\"" },
                style = body_style(stack, 16, 26, text_color),
                l = inline(left, accent)?,
                r = inline(right, accent)?,
            )
        }
        "divider" | "hr" => format!(
            "    <tr>\n      <td style=\"padding:16px 32px;\">\n\
             \x20       <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"100%\" style=\"width:100%;\">\n\
             \x20         <tr>\n\
             \x20           <td{rule_cls} style=\"border-top:1px solid {RULE};font-size:0;line-height:0;\">&nbsp;</td>\n\
             \x20         </tr>\n\
             \x20       </table>\n      </td>\n    </tr>\n",
            rule_cls = if dark { " class=\"dm-rule\"" } else { "" },
        ),
        "spacer" | "space" => {
            let raw = part(0);
            let h: u32 = if raw.is_empty() {
                24
            } else {
                raw.parse().map_err(|_| {
                    err_at(s, &format!("spacer height {raw:?} is not a whole number of pixels"))
                })?
            };
            if h > 200 {
                return Err(err_at(s, "spacer height must be 200px or less"));
            }
            format!(
                "    <tr>\n      <td style=\"height:{h}px;font-size:0;line-height:0;\">&nbsp;</td>\n    </tr>\n"
            )
        }
        "footer" => {
            let t = s.rest;
            if t.is_empty() {
                return Err(err_at(s, "footer needs text"));
            }
            format!(
                "    <tr>\n      <td{muted_cls} align=\"center\" style=\"padding:8px 32px 28px 32px;{}\">{}</td>\n    </tr>\n",
                body_style(stack, 12, 20, MUTED),
                inline(t, accent)?
            )
        }
        "html" | "raw" => {
            let raw = s.rest;
            if raw.trim().is_empty() {
                return Err(err_at(s, "html needs markup after the pipe"));
            }
            format!("    <tr>\n      <td style=\"padding:0 32px;\">{}</td>\n    </tr>\n", raw.trim())
        }
        other => {
            return Err(err_at(
                s,
                &format!(
                    "unknown section type {other:?}: expected heading, subheading, text, button, \
                     image, columns, divider, spacer, footer or html"
                ),
            ))
        }
    };
    Ok(row)
}

fn err_at(s: &Section<'_>, msg: &str) -> String {
    format!("line {}: {msg}", s.line_no)
}

/// Build the complete newsletter document.
#[allow(clippy::too_many_arguments)]
pub fn build(
    sections: &str,
    subject: &str,
    preheader: &str,
    width: f64,
    background: &str,
    content_background: &str,
    text_color: &str,
    accent: &str,
    font: &str,
    dark_mode: bool,
) -> Result<String, String> {
    let stack = font_stack(font)?;
    let width_px: u32 = if width == 0.0 {
        DEFAULT_WIDTH
    } else {
        if !width.is_finite() || width.fract() != 0.0 {
            return Err(format!("invalid width {width}: expected a whole number of pixels"));
        }
        let w = width as i64;
        if !(MIN_WIDTH as i64..=MAX_WIDTH as i64).contains(&w) {
            return Err(format!(
                "invalid width {w}: expected {MIN_WIDTH}–{MAX_WIDTH} pixels (most newsletters use 600)"
            ));
        }
        w as u32
    };
    let background = color(background, "background", DEFAULT_BACKGROUND)?;
    let card = color(content_background, "content background", DEFAULT_CONTENT_BACKGROUND)?;
    let text_color = color(text_color, "text", DEFAULT_TEXT_COLOR)?;
    let accent = color(accent, "accent", DEFAULT_ACCENT)?;

    let parsed = parse_sections(sections)?;
    let mut rows = String::new();
    for s in &parsed {
        rows.push_str(&render_section(
            s,
            stack,
            &text_color,
            &accent,
            dark_mode,
            width_px,
        )?);
    }

    let title = if subject.trim().is_empty() {
        "Newsletter".to_string()
    } else {
        esc(subject.trim())
    };

    // Hidden preview line + zero-width padding so the body copy doesn't bleed
    // into the inbox preview after it.
    let preheader_block = if preheader.trim().is_empty() {
        String::new()
    } else {
        format!(
            "  <div style=\"display:none;font-size:1px;line-height:1px;max-height:0;max-width:0;\
             opacity:0;overflow:hidden;mso-hide:all;color:transparent;\">{}{}</div>\n",
            esc(preheader.trim()),
            "&#847;&zwnj;&nbsp;&#8199;&#65279;".repeat(12)
        )
    };

    let dark_meta = if dark_mode {
        "  <meta name=\"color-scheme\" content=\"light dark\" />\n  \
         <meta name=\"supported-color-schemes\" content=\"light dark\" />\n"
    } else {
        ""
    };
    let dark_css = if dark_mode {
        format!(
            "      @media (prefers-color-scheme: dark) {{\n\
             \x20       .dm-page, .dm-page td {{ background-color: {DARK_PAGE} !important; }}\n\
             \x20       .dm-card {{ background-color: {DARK_CARD} !important; }}\n\
             \x20       .dm-text, .dm-text td, .dm-text a {{ color: {DARK_TEXT} !important; }}\n\
             \x20       .dm-muted, .dm-muted td {{ color: {DARK_MUTED} !important; }}\n\
             \x20       .dm-rule {{ border-color: {DARK_RULE} !important; }}\n\
             \x20     }}\n"
        )
    } else {
        String::new()
    };
    let page_cls = if dark_mode { " class=\"dm-page\"" } else { "" };
    let card_cls = if dark_mode { "sm-full dm-card" } else { "sm-full" };

    Ok(format!(
        "<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\" \"http://www.w3.org/TR/xhtml1/DTD/xhtml1-transitional.dtd\">\n\
<html xmlns=\"http://www.w3.org/1999/xhtml\" xmlns:v=\"urn:schemas-microsoft-com:vml\" xmlns:o=\"urn:schemas-microsoft-com:office:office\" lang=\"en\">\n\
<head>\n\
\x20 <meta http-equiv=\"Content-Type\" content=\"text/html; charset=utf-8\" />\n\
\x20 <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\" />\n\
\x20 <meta http-equiv=\"X-UA-Compatible\" content=\"IE=edge\" />\n\
\x20 <meta name=\"x-apple-disable-message-reformatting\" />\n\
{dark_meta}\
\x20 <title>{title}</title>\n\
\x20 <!--[if mso]>\n\
\x20 <noscript><xml><o:OfficeDocumentSettings><o:PixelsPerInch>96</o:PixelsPerInch></o:OfficeDocumentSettings></xml></noscript>\n\
\x20 <![endif]-->\n\
\x20 <style type=\"text/css\">\n\
\x20     html, body {{ margin:0 !important; padding:0 !important; width:100% !important; }}\n\
\x20     body, table, td {{ -webkit-text-size-adjust:100%; -ms-text-size-adjust:100%; }}\n\
\x20     table, td {{ mso-table-lspace:0pt; mso-table-rspace:0pt; border-collapse:collapse; }}\n\
\x20     img {{ -ms-interpolation-mode:bicubic; border:0; height:auto; line-height:100%; outline:none; text-decoration:none; }}\n\
\x20     @media screen and (max-width: {width_px}px) {{\n\
\x20       .sm-full {{ width:100% !important; max-width:100% !important; }}\n\
\x20       .sm-stack {{ display:block !important; width:100% !important; max-width:100% !important; padding:8px 0 !important; }}\n\
\x20     }}\n\
{dark_css}\
\x20 </style>\n\
</head>\n\
<body{page_cls} style=\"margin:0;padding:0;width:100%;background-color:{background};\">\n\
{preheader_block}\
\x20 <div role=\"article\" aria-roledescription=\"email\" aria-label=\"{title}\" lang=\"en\">\n\
\x20 <table role=\"presentation\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"100%\"{page_cls} style=\"width:100%;background-color:{background};\">\n\
\x20   <tr>\n\
\x20     <td align=\"center\" style=\"padding:24px 12px;\">\n\
\x20       <!--[if mso]><table role=\"presentation\" align=\"center\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"{width_px}\"><tr><td><![endif]-->\n\
\x20       <table role=\"presentation\" class=\"{card_cls}\" cellpadding=\"0\" cellspacing=\"0\" border=\"0\" width=\"{width_px}\" style=\"width:{width_px}px;max-width:{width_px}px;background-color:{card};border-radius:8px;overflow:hidden;\">\n\
{rows}\
\x20       </table>\n\
\x20       <!--[if mso]></td></tr></table><![endif]-->\n\
\x20     </td>\n\
\x20   </tr>\n\
\x20 </table>\n\
\x20 </div>\n\
</body>\n\
</html>\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple(sections: &str) -> Result<String, String> {
        build(sections, "", "", 0.0, "", "", "", "", "", true)
    }

    #[test]
    fn builds_a_full_document_with_every_section_type() {
        let html = simple(
            "# a comment\n\
             heading | Monthly **update**\n\
             text | Hello [there](https://example.com), welcome.\n\
             button | Read more | https://example.com/post\n\
             image | https://example.com/a.png | Cover\n\
             columns | Left | Right\n\
             divider\n\
             spacer | 32\n\
             footer | You get this because you subscribed.",
        )
        .expect("builds");
        assert!(html.starts_with("<!DOCTYPE html PUBLIC \"-//W3C//DTD XHTML 1.0 Transitional//EN\""));
        assert!(html.contains("<strong>update</strong>"));
        assert!(html.contains("href=\"https://example.com\""));
        assert!(html.contains("href=\"https://example.com/post\""));
        assert!(html.contains("<img src=\"https://example.com/a.png\" alt=\"Cover\""));
        assert!(html.contains("class=\"sm-stack dm-text\""));
        assert!(html.contains("border-top:1px solid #e5e7eb"));
        assert!(html.contains("height:32px"));
        assert!(html.contains("role=\"presentation\""));
        // Outlook ghost table + pixel-density fix.
        assert!(html.contains("<!--[if mso]><table role=\"presentation\" align=\"center\""));
        assert!(html.contains("o:PixelsPerInch"));
        // Default width everywhere.
        assert!(html.contains("width:600px;max-width:600px"));
        // The comment line produced no row.
        assert!(!html.contains("a comment"));
    }

    #[test]
    fn preheader_is_hidden_and_padded() {
        let html = build("text | Hi", "Subject", "The short preview line", 0.0, "", "", "", "", "", true)
            .expect("builds");
        assert!(html.contains("<title>Subject</title>"));
        assert!(html.contains("mso-hide:all"));
        assert!(html.contains("The short preview line&#847;&zwnj;"));
    }

    #[test]
    fn dark_mode_off_drops_the_media_query_and_classes() {
        let html = build("text | Hi", "", "", 0.0, "", "", "", "", "", false).expect("builds");
        assert!(!html.contains("prefers-color-scheme"));
        assert!(!html.contains("dm-card"));
        assert!(html.contains("class=\"sm-full\""));
    }

    #[test]
    fn custom_theme_is_applied_inline() {
        let html = build(
            "heading | Hi\nbutton | Go | https://example.com",
            "",
            "",
            480.0,
            "#000000",
            "#111111",
            "#eeeeee",
            "#ff0055",
            "georgia",
            false,
        )
        .expect("builds");
        assert!(html.contains("width:480px;max-width:480px"));
        assert!(html.contains("background-color:#000000"));
        assert!(html.contains("background-color:#111111"));
        assert!(html.contains("color:#eeeeee"));
        assert!(html.contains("bgcolor=\"#ff0055\""));
        assert!(html.contains("font-family:Georgia, 'Times New Roman', Times, serif"));
        assert!(html.contains("@media screen and (max-width: 480px)"));
    }

    #[test]
    fn inline_syntax_escapes_and_breaks() {
        let html = simple("text | 5 > 3 & \"safe\"\\nsecond *line*").expect("builds");
        assert!(html.contains("5 &gt; 3 &amp; &quot;safe&quot;<br />second <em>line</em>"));
    }

    #[test]
    fn image_can_link_and_alt_defaults_to_empty() {
        let html = simple("image | https://example.com/a.png |  | https://example.com").expect("builds");
        assert!(html.contains("<a href=\"https://example.com\" style=\"text-decoration:none;\"><img"));
        assert!(html.contains("alt=\"\""));
    }

    #[test]
    fn raw_html_passes_through() {
        let html = simple("html | <p style=\"color:red\">custom | block</p>").expect("builds");
        assert!(html.contains("<p style=\"color:red\">custom | block</p>"));
    }

    #[test]
    fn merge_tags_survive_in_text_and_links() {
        let html = simple("footer | Hi {{first_name}} — [Unsubscribe]({{unsubscribe_url}})").expect("builds");
        assert!(html.contains("Hi {{first_name}}"));
        assert!(html.contains("href=\"{{unsubscribe_url}}\""));
    }

    #[test]
    fn empty_sections_error() {
        let err = simple("\n  \n# only comments\n").unwrap_err();
        assert!(err.contains("no sections"), "got {err}");
    }

    #[test]
    fn unknown_section_type_errors_with_the_line_number() {
        let err = simple("heading | Hi\nvideo | https://example.com").unwrap_err();
        assert!(err.contains("line 2"), "got {err}");
        assert!(err.contains("unknown section type \"video\""), "got {err}");
    }

    #[test]
    fn javascript_urls_are_rejected() {
        let err = simple("button | Click | javascript:alert(1)").unwrap_err();
        assert!(err.contains("invalid URL"), "got {err}");
        let err = simple("text | [x](javascript:alert(1))").unwrap_err();
        assert!(err.contains("invalid URL"), "got {err}");
    }

    #[test]
    fn button_without_a_url_errors() {
        let err = simple("button | Read more").unwrap_err();
        assert!(err.contains("line 1"), "got {err}");
        assert!(err.contains("missing a URL"), "got {err}");
    }

    #[test]
    fn bad_colour_and_font_and_width_error() {
        let err = build("text | Hi", "", "", 0.0, "red;}", "", "", "", "", true).unwrap_err();
        assert!(err.contains("invalid background colour"), "got {err}");
        let err = build("text | Hi", "", "", 0.0, "", "", "", "", "comic", true).unwrap_err();
        assert!(err.contains("invalid font"), "got {err}");
        let err = build("text | Hi", "", "", 200.0, "", "", "", "", "", true).unwrap_err();
        assert!(err.contains("expected 320–900 pixels"), "got {err}");
    }

    #[test]
    fn spacer_height_must_be_a_number() {
        let err = simple("spacer | tall").unwrap_err();
        assert!(err.contains("not a whole number of pixels"), "got {err}");
    }

    #[test]
    fn section_cap_is_inclusive() {
        let ok = "text | line\n".repeat(MAX_SECTIONS);
        assert!(simple(&ok).is_ok());
        let over = "text | line\n".repeat(MAX_SECTIONS + 1);
        let err = simple(&over).unwrap_err();
        assert!(err.contains("too many sections"), "got {err}");
    }

    #[test]
    fn named_colours_are_accepted() {
        let html = build("text | Hi", "", "", 0.0, "white", "", "", "", "", false).expect("builds");
        assert!(html.contains("background-color:white"));
    }
}
