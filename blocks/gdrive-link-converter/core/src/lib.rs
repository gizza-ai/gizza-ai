//! gdrive-link-converter core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps, no network. Given any Google Drive
//! link shape (or a bare file ID), it extracts the file ID and rebuilds it as a
//! direct-download link, an inline-embed link, a share/view link, a preview
//! iframe URL, a thumbnail URL, or just the raw ID — all by string manipulation,
//! so it never contacts Google.

/// Which link to emit for an extracted file ID.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    /// `uc?export=download` — the classic direct-download link (small files).
    Direct,
    /// `drive.usercontent.google.com/download…&confirm=t` — skips the virus-scan
    /// interstitial so large files download straight away.
    DirectConfirm,
    /// `uc?export=view` — serves the bytes inline, for embedding an image in HTML/Markdown.
    View,
    /// `file/d/ID/view?usp=sharing` — the human share/view link (the "back" conversion).
    Share,
    /// `file/d/ID/preview` — the iframe-embeddable preview URL.
    Preview,
    /// `thumbnail?id=ID&sz=…` — a resizable thumbnail image URL.
    Thumbnail,
    /// The bare file ID, for scripts.
    Id,
}

impl Output {
    fn parse(s: &str) -> Result<Output, String> {
        Ok(match s {
            "direct" => Output::Direct,
            "direct_confirm" => Output::DirectConfirm,
            "view" => Output::View,
            "share" => Output::Share,
            "preview" => Output::Preview,
            "thumbnail" => Output::Thumbnail,
            "id" => Output::Id,
            other => {
                return Err(format!(
                    "unknown output '{other}' — expected one of: direct, direct_confirm, view, share, preview, thumbnail, id"
                ))
            }
        })
    }
}

/// True for the characters that make up a Google Drive file ID (base64url-ish).
fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Take the leading run of ID characters from `s`.
fn take_id(s: &str) -> String {
    s.chars().take_while(|&c| is_id_char(c)).collect()
}

/// A plausible Drive file ID: only ID chars, and long enough not to match a stray
/// path segment like "d" or "edit". Real IDs are 28–44 chars; we accept ≥ 12 to
/// stay tolerant of future formats without matching short words.
fn is_valid_id(s: &str) -> bool {
    s.len() >= 12 && s.chars().all(is_id_char)
}

/// Read the `id=` query parameter's value (still-encoded) from a URL, if present.
fn query_id(s: &str) -> Option<String> {
    let q = s.split_once('?')?.1;
    let q = q.split('#').next().unwrap_or(q);
    for pair in q.split('&') {
        if let Some(v) = pair.strip_prefix("id=") {
            return Some(take_id(v));
        }
    }
    None
}

/// Pull a Google Drive file (or folder) ID out of any supported link shape, or a
/// bare ID. Returns `None` when nothing ID-shaped is found.
///
/// Recognised shapes:
/// - `drive.google.com/file/d/ID/view`, `…/d/ID/edit` (Docs/Sheets/Slides)
/// - `drive.google.com/open?id=ID`, `uc?export=download&id=ID`
/// - `drive.usercontent.google.com/download?id=ID&export=download`
/// - `drive.google.com/thumbnail?id=ID&sz=…`
/// - `drive.google.com/drive/folders/ID`, `…/folders/ID`
/// - a bare `ID`
pub fn extract_id(input: &str) -> Option<String> {
    let s = input.trim();
    if s.is_empty() {
        return None;
    }
    // Path markers, most specific first so `/file/d/` wins over the `/d/` inside it.
    for marker in ["/file/d/", "/folders/", "/d/"] {
        if let Some(idx) = s.find(marker) {
            let id = take_id(&s[idx + marker.len()..]);
            if is_valid_id(&id) {
                return Some(id);
            }
        }
    }
    // `id=` query parameter (open?id=, uc?id=, download?id=, thumbnail?id=).
    if let Some(id) = query_id(s) {
        if is_valid_id(&id) {
            return Some(id);
        }
    }
    // Bare ID (the whole trimmed input).
    if is_valid_id(s) {
        return Some(s.to_string());
    }
    None
}

/// Sanitise a thumbnail size token to Drive's `sz` syntax (`w<pixels>`,
/// `h<pixels>`, `w<W>-h<H>`, or `s<pixels>`). Falls back to the default on junk
/// so the URL is always well-formed.
fn thumb_size(size: &str) -> String {
    let s = size.trim();
    let ok = !s.is_empty()
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        && s.starts_with(['w', 'h', 's']);
    if ok {
        s.to_string()
    } else {
        "w1000".to_string()
    }
}

/// Build the requested link from an already-extracted `id`.
fn build(id: &str, output: Output, size: &str) -> String {
    match output {
        Output::Direct => format!("https://drive.google.com/uc?export=download&id={id}"),
        Output::DirectConfirm => {
            format!("https://drive.usercontent.google.com/download?id={id}&export=download&confirm=t")
        }
        Output::View => format!("https://drive.google.com/uc?export=view&id={id}"),
        Output::Share => format!("https://drive.google.com/file/d/{id}/view?usp=sharing"),
        Output::Preview => format!("https://drive.google.com/file/d/{id}/preview"),
        Output::Thumbnail => {
            format!("https://drive.google.com/thumbnail?id={id}&sz={}", thumb_size(size))
        }
        Output::Id => id.to_string(),
    }
}

/// Convert one line: extract the ID and emit the requested link, or an error
/// explaining that no Drive ID was found.
fn convert_one(line: &str, output: Output, size: &str) -> Result<String, String> {
    match extract_id(line) {
        Some(id) => Ok(build(&id, output, size)),
        None => Err(format!(
            "no Google Drive file ID found in '{}' — paste a share link like https://drive.google.com/file/d/FILE_ID/view or a bare ID",
            line.trim()
        )),
    }
}

/// Convert `input`. With `per_line`, each non-empty line is converted
/// independently and rejoined with newlines (blank lines preserved); otherwise
/// the whole input is treated as one link. `output` selects the link form and
/// `size` is the thumbnail `sz` token (only used for output="thumbnail").
pub fn convert(input: &str, output: &str, size: &str, per_line: bool) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste a Google Drive link or file ID".into());
    }
    let output = Output::parse(output)?;
    if per_line {
        let mut out = Vec::new();
        for line in input.lines() {
            if line.trim().is_empty() {
                out.push(String::new());
            } else {
                out.push(convert_one(line, output, size)?);
            }
        }
        Ok(out.join("\n"))
    } else {
        convert_one(input.trim(), output, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW";

    #[test]
    fn extract_from_file_view_link() {
        let u = format!("https://drive.google.com/file/d/{ID}/view?usp=sharing");
        assert_eq!(extract_id(&u).as_deref(), Some(ID));
    }

    #[test]
    fn extract_from_open_id_link() {
        let u = format!("https://drive.google.com/open?id={ID}");
        assert_eq!(extract_id(&u).as_deref(), Some(ID));
    }

    #[test]
    fn extract_from_uc_download_link() {
        let u = format!("https://drive.google.com/uc?export=download&id={ID}");
        assert_eq!(extract_id(&u).as_deref(), Some(ID));
    }

    #[test]
    fn extract_from_usercontent_download_link() {
        let u = format!("https://drive.usercontent.google.com/download?id={ID}&export=download");
        assert_eq!(extract_id(&u).as_deref(), Some(ID));
    }

    #[test]
    fn extract_from_docs_link() {
        let u = format!("https://docs.google.com/document/d/{ID}/edit?usp=sharing");
        assert_eq!(extract_id(&u).as_deref(), Some(ID));
    }

    #[test]
    fn extract_from_folder_link() {
        let u = format!("https://drive.google.com/drive/folders/{ID}?usp=sharing");
        assert_eq!(extract_id(&u).as_deref(), Some(ID));
    }

    #[test]
    fn extract_from_thumbnail_link() {
        let u = format!("https://drive.google.com/thumbnail?id={ID}&sz=w400");
        assert_eq!(extract_id(&u).as_deref(), Some(ID));
    }

    #[test]
    fn extract_from_bare_id() {
        assert_eq!(extract_id(ID).as_deref(), Some(ID));
        assert_eq!(extract_id(&format!("  {ID}  ")).as_deref(), Some(ID));
    }

    #[test]
    fn no_id_returns_none() {
        assert_eq!(extract_id("https://example.com/hello"), None);
        assert_eq!(extract_id("not a link"), None);
        assert_eq!(extract_id(""), None);
    }

    #[test]
    fn direct_download_from_share_link() {
        let u = format!("https://drive.google.com/file/d/{ID}/view?usp=sharing");
        assert_eq!(
            convert(&u, "direct", "", false).unwrap(),
            format!("https://drive.google.com/uc?export=download&id={ID}")
        );
    }

    #[test]
    fn direct_confirm_for_large_files() {
        assert_eq!(
            convert(ID, "direct_confirm", "", false).unwrap(),
            format!("https://drive.usercontent.google.com/download?id={ID}&export=download&confirm=t")
        );
    }

    #[test]
    fn view_embed_link() {
        assert_eq!(
            convert(ID, "view", "", false).unwrap(),
            format!("https://drive.google.com/uc?export=view&id={ID}")
        );
    }

    #[test]
    fn back_to_share_link_from_direct() {
        let u = format!("https://drive.google.com/uc?export=download&id={ID}");
        assert_eq!(
            convert(&u, "share", "", false).unwrap(),
            format!("https://drive.google.com/file/d/{ID}/view?usp=sharing")
        );
    }

    #[test]
    fn preview_iframe_link() {
        assert_eq!(
            convert(ID, "preview", "", false).unwrap(),
            format!("https://drive.google.com/file/d/{ID}/preview")
        );
    }

    #[test]
    fn thumbnail_with_default_and_custom_size() {
        assert_eq!(
            convert(ID, "thumbnail", "", false).unwrap(),
            format!("https://drive.google.com/thumbnail?id={ID}&sz=w1000")
        );
        assert_eq!(
            convert(ID, "thumbnail", "w320-h240", false).unwrap(),
            format!("https://drive.google.com/thumbnail?id={ID}&sz=w320-h240")
        );
        // Junk size falls back to the default.
        assert_eq!(
            convert(ID, "thumbnail", "garbage!!", false).unwrap(),
            format!("https://drive.google.com/thumbnail?id={ID}&sz=w1000")
        );
    }

    #[test]
    fn id_only_output() {
        let u = format!("https://drive.google.com/file/d/{ID}/view");
        assert_eq!(convert(&u, "id", "", false).unwrap(), ID);
    }

    #[test]
    fn per_line_batch_preserves_blank_lines() {
        let id2 = "ABCDEFGHIJKL_MNOPQRSTUVWX";
        let input = format!(
            "https://drive.google.com/file/d/{ID}/view\n\nhttps://drive.google.com/open?id={id2}"
        );
        let got = convert(&input, "direct", "", true).unwrap();
        assert_eq!(
            got,
            format!(
                "https://drive.google.com/uc?export=download&id={ID}\n\nhttps://drive.google.com/uc?export=download&id={id2}"
            )
        );
    }

    #[test]
    fn unknown_output_errors() {
        assert!(convert(ID, "bogus", "", false).is_err());
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", "direct", "", false).is_err());
    }

    #[test]
    fn missing_id_errors() {
        assert!(convert("https://example.com/nope", "direct", "", false).is_err());
    }
}
