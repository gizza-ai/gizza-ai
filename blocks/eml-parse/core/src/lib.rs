//! gizza-ai/eml-parse core — parse a raw `.eml` / RFC 5322 message into
//! structured headers, decoded text and HTML bodies, and an attachment list.
//! Pure-Rust (`mail-parser`), no wafer/wasm-bindgen deps.

use mail_parser::{Address, MessageParser, MimeHeaders};
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Mailbox {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Attachment {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,
    pub content_type: String,
    pub size: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Email {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_id: Option<String>,
    pub from: Vec<Mailbox>,
    pub to: Vec<Mailbox>,
    pub cc: Vec<Mailbox>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_body: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_body: Option<String>,
    pub attachments: Vec<Attachment>,
}

fn addresses(addr: Option<&Address>) -> Vec<Mailbox> {
    match addr {
        None => Vec::new(),
        Some(a) => a
            .iter()
            .map(|m| Mailbox {
                name: m.name().map(|s| s.to_string()),
                address: m.address().map(|s| s.to_string()),
            })
            .filter(|m| m.name.is_some() || m.address.is_some())
            .collect(),
    }
}

/// Parse a raw email message.
pub fn parse(raw: &str) -> Result<Email, String> {
    if raw.trim().is_empty() {
        return Err("input is empty — paste a raw .eml message".into());
    }
    let msg = MessageParser::default()
        .parse(raw.as_bytes())
        .ok_or_else(|| "could not parse the message as RFC 5322 / MIME".to_string())?;

    let attachments = msg
        .attachments()
        .map(|p| {
            let content_type = p
                .content_type()
                .map(|ct| match ct.subtype() {
                    Some(sub) => format!("{}/{}", ct.ctype(), sub),
                    None => ct.ctype().to_string(),
                })
                .unwrap_or_else(|| "application/octet-stream".to_string());
            Attachment {
                filename: p.attachment_name().map(|s| s.to_string()),
                content_type,
                size: p.len(),
            }
        })
        .collect();

    Ok(Email {
        subject: msg.subject().map(|s| s.to_string()),
        date: msg.date().map(|d| d.to_rfc3339()),
        message_id: msg.message_id().map(|s| s.to_string()),
        from: addresses(msg.from()),
        to: addresses(msg.to()),
        cc: addresses(msg.cc()),
        text_body: msg.body_text(0).map(|s| s.into_owned()),
        html_body: msg.body_html(0).map(|s| s.into_owned()),
        attachments,
    })
}

/// Human-readable rendering (used by the page).
pub fn render(raw: &str) -> Result<String, String> {
    let e = parse(raw)?;
    let fmt_list = |v: &[Mailbox]| {
        if v.is_empty() {
            "(none)".to_string()
        } else {
            v.iter()
                .map(|m| match (&m.name, &m.address) {
                    (Some(n), Some(a)) => format!("{n} <{a}>"),
                    (None, Some(a)) => a.clone(),
                    (Some(n), None) => n.clone(),
                    (None, None) => String::new(),
                })
                .collect::<Vec<_>>()
                .join(", ")
        }
    };
    let mut out = String::new();
    out.push_str(&format!("Subject: {}\n", e.subject.as_deref().unwrap_or("(none)")));
    out.push_str(&format!("Date:    {}\n", e.date.as_deref().unwrap_or("(none)")));
    out.push_str(&format!("From:    {}\n", fmt_list(&e.from)));
    out.push_str(&format!("To:      {}\n", fmt_list(&e.to)));
    if !e.cc.is_empty() {
        out.push_str(&format!("Cc:      {}\n", fmt_list(&e.cc)));
    }
    if let Some(id) = &e.message_id {
        out.push_str(&format!("Message-ID: {id}\n"));
    }
    out.push_str(&format!("\nAttachments ({}):\n", e.attachments.len()));
    for a in &e.attachments {
        out.push_str(&format!(
            "  - {} [{}] {} bytes\n",
            a.filename.as_deref().unwrap_or("(unnamed)"),
            a.content_type,
            a.size
        ));
    }
    if let Some(t) = &e.text_body {
        out.push_str("\n--- Text body ---\n");
        out.push_str(t);
        out.push('\n');
    }
    if e.text_body.is_none() {
        if let Some(h) = &e.html_body {
            out.push_str("\n--- HTML body ---\n");
            out.push_str(h);
            out.push('\n');
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "From: Alice Example <alice@example.com>\r\nTo: Bob <bob@example.org>\r\nCc: carol@example.net\r\nSubject: Hello there\r\nDate: Mon, 1 Jan 2024 10:00:00 +0000\r\nMessage-ID: <abc123@example.com>\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nThis is the body of the email.\r\n";

    const MULTIPART: &str = "From: a@x.com\r\nTo: b@y.com\r\nSubject: With attachment\r\nMIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=\"BB\"\r\n\r\n--BB\r\nContent-Type: text/plain\r\n\r\nbody text\r\n--BB\r\nContent-Type: text/csv; name=\"data.csv\"\r\nContent-Disposition: attachment; filename=\"data.csv\"\r\n\r\na,b\r\n1,2\r\n--BB--\r\n";

    #[test]
    fn parses_basic_headers_and_body() {
        let e = parse(SAMPLE).unwrap();
        assert_eq!(e.subject.as_deref(), Some("Hello there"));
        assert_eq!(e.from[0].address.as_deref(), Some("alice@example.com"));
        assert_eq!(e.from[0].name.as_deref(), Some("Alice Example"));
        assert_eq!(e.to[0].address.as_deref(), Some("bob@example.org"));
        assert_eq!(e.cc[0].address.as_deref(), Some("carol@example.net"));
        assert_eq!(e.message_id.as_deref(), Some("abc123@example.com"));
        assert!(e.text_body.unwrap().contains("body of the email"));
        assert!(e.date.unwrap().starts_with("2024-01-01"));
        assert!(e.attachments.is_empty());
    }

    #[test]
    fn parses_attachment() {
        let e = parse(MULTIPART).unwrap();
        assert_eq!(e.text_body.as_deref(), Some("body text"));
        assert_eq!(e.attachments.len(), 1);
        let a = &e.attachments[0];
        assert_eq!(a.filename.as_deref(), Some("data.csv"));
        assert_eq!(a.content_type, "text/csv");
        assert!(a.size > 0);
    }

    #[test]
    fn render_includes_subject_and_attachment() {
        let r = render(MULTIPART).unwrap();
        assert!(r.contains("Subject: With attachment"));
        assert!(r.contains("data.csv"));
        assert!(r.contains("text/csv"));
    }

    #[test]
    fn errors() {
        assert!(parse("").is_err());
    }
}
