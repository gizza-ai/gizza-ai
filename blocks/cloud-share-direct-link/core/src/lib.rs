//! cloud-share-direct-link core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps, no network: every conversion is
//! string surgery on the share URL itself, so the tool never contacts Google,
//! Dropbox, Microsoft, or your Nextcloud server.
//!
//! Supported providers: Google Drive (incl. Docs/Sheets/Slides editor links),
//! Dropbox (both the legacy `/s/` and the current `/scl/` share shapes), OneDrive
//! (`1drv.ms`, `onedrive.live.com`, SharePoint) and Nextcloud/ownCloud public
//! shares on any host.

/// Which cloud the link belongs to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Provider {
    GoogleDrive,
    Dropbox,
    OneDrive,
    Nextcloud,
}

impl Provider {
    fn parse(s: &str) -> Result<Option<Provider>, String> {
        Ok(match s.trim() {
            "" | "auto" => None,
            "google_drive" => Some(Provider::GoogleDrive),
            "dropbox" => Some(Provider::Dropbox),
            "onedrive" => Some(Provider::OneDrive),
            "nextcloud" => Some(Provider::Nextcloud),
            other => {
                return Err(format!(
                    "unknown provider '{other}' — expected one of: auto, google_drive, dropbox, onedrive, nextcloud"
                ))
            }
        })
    }

    fn label(self) -> &'static str {
        match self {
            Provider::GoogleDrive => "Google Drive",
            Provider::Dropbox => "Dropbox",
            Provider::OneDrive => "OneDrive",
            Provider::Nextcloud => "Nextcloud/ownCloud",
        }
    }
}

/// Download (force the bytes as an attachment) vs inline (hotlink/embed).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Download,
    Inline,
}

impl Mode {
    fn parse(s: &str) -> Result<Mode, String> {
        Ok(match s.trim() {
            "" | "download" => Mode::Download,
            "inline" => Mode::Inline,
            other => {
                return Err(format!(
                    "unknown mode '{other}' — expected one of: download, inline"
                ))
            }
        })
    }
}

/// How to render each converted link.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Output {
    Url,
    Markdown,
    Html,
    Curl,
    Wget,
}

impl Output {
    fn parse(s: &str) -> Result<Output, String> {
        Ok(match s.trim() {
            "" | "url" => Output::Url,
            "markdown" => Output::Markdown,
            "html" => Output::Html,
            "curl" => Output::Curl,
            "wget" => Output::Wget,
            other => {
                return Err(format!(
                    "unknown output '{other}' — expected one of: url, markdown, html, curl, wget"
                ))
            }
        })
    }
}

/// Which of the two known OneDrive rewrites to use for `mode=download`.
#[derive(Clone, Copy, PartialEq, Eq)]
enum OneDriveStyle {
    /// `api.onedrive.com/v1.0/shares/u!<base64url>/root/content`.
    Api,
    /// The original URL with a `download=1` flag.
    DownloadParam,
}

impl OneDriveStyle {
    fn parse(s: &str) -> Result<OneDriveStyle, String> {
        Ok(match s.trim() {
            "" | "api" => OneDriveStyle::Api,
            "download_param" => OneDriveStyle::DownloadParam,
            other => {
                return Err(format!(
                    "unknown onedrive_style '{other}' — expected one of: api, download_param"
                ))
            }
        })
    }
}

/// Google Docs editor export format family, mapped per document kind so the
/// emitted `format=` token is always one Google actually accepts.
#[derive(Clone, Copy, PartialEq, Eq)]
enum DocsExport {
    Pdf,
    Office,
    Txt,
}

impl DocsExport {
    fn parse(s: &str) -> Result<DocsExport, String> {
        Ok(match s.trim() {
            "" | "pdf" => DocsExport::Pdf,
            "office" => DocsExport::Office,
            "txt" => DocsExport::Txt,
            other => {
                return Err(format!(
                    "unknown docs_export '{other}' — expected one of: pdf, office, txt"
                ))
            }
        })
    }

    /// `kind` is the Docs URL segment: `document`, `spreadsheets`, `presentation`.
    fn token(self, kind: &str) -> &'static str {
        match (self, kind) {
            (DocsExport::Pdf, _) => "pdf",
            (DocsExport::Office, "spreadsheets") => "xlsx",
            (DocsExport::Office, "presentation") => "pptx",
            (DocsExport::Office, _) => "docx",
            (DocsExport::Txt, "spreadsheets") => "csv",
            (DocsExport::Txt, _) => "txt",
        }
    }
}

/// Every knob the conversion takes, parsed once for a whole batch.
struct Opts {
    provider: Option<Provider>,
    mode: Mode,
    output: Output,
    onedrive_style: OneDriveStyle,
    docs_export: DocsExport,
    file: String,
}

// ---------------------------------------------------------------------------
// URL helpers (deliberately tiny — a full URL crate is overkill for this and
// would drag percent-encoding tables into two wasm targets).
// ---------------------------------------------------------------------------

/// Split a URL into (before-query, query, fragment-with-#). The query excludes
/// both delimiters; an absent query is an empty string.
fn split_url(url: &str) -> (&str, &str, &str) {
    let (head, frag) = match url.find('#') {
        Some(i) => (&url[..i], &url[i..]),
        None => (url, ""),
    };
    match head.find('?') {
        Some(i) => (&head[..i], &head[i + 1..], frag),
        None => (head, "", frag),
    }
}

/// Lowercased host of an absolute http(s) URL, or `""` when there isn't one.
fn host_of(url: &str) -> String {
    let rest = match url.split_once("://") {
        Some((scheme, rest)) if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") => rest,
        _ => return String::new(),
    };
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..end];
    // Drop userinfo, keep the port (a self-hosted Nextcloud often runs on one).
    let hostport = authority.rsplit('@').next().unwrap_or(authority);
    hostport.to_ascii_lowercase()
}

/// True when `host` is `domain` or any subdomain of it.
fn host_is(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

/// Rebuild a URL with `key` forced to `value`, dropping any of `drop_keys` that
/// were already present. Preserves the rest of the query and the fragment.
fn set_query_param(url: &str, key: &str, value: &str, drop_keys: &[&str]) -> String {
    let (base, query, frag) = split_url(url);
    let mut kept: Vec<&str> = Vec::new();
    for pair in query.split('&').filter(|p| !p.is_empty()) {
        let name = pair.split('=').next().unwrap_or(pair);
        if name == key || drop_keys.contains(&name) {
            continue;
        }
        kept.push(pair);
    }
    let mut q = kept.join("&");
    if !q.is_empty() {
        q.push('&');
    }
    q.push_str(key);
    q.push('=');
    q.push_str(value);
    format!("{base}?{q}{frag}")
}

/// Percent-encode everything outside the unreserved set, so a file name with
/// spaces or `&` survives a query string intact.
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => out.push(*b as char),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// Decode `%XX` escapes (used only to recover a display file name from a path).
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// Standard base64 with the URL-safe alphabet and no padding — exactly the
/// encoding the OneDrive shares API expects after its `u!` prefix.
fn base64url_nopad(data: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        if chunk.len() > 1 {
            out.push(ALPHABET[(n >> 6) as usize & 63] as char);
        }
        if chunk.len() > 2 {
            out.push(ALPHABET[n as usize & 63] as char);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Provider detection
// ---------------------------------------------------------------------------

/// The `/s/<token>` (or `/index.php/s/<token>`) shape every Nextcloud and
/// ownCloud public share uses, on whatever host the server happens to run.
fn nextcloud_parts(url: &str) -> Option<(String, String)> {
    let (base, _, _) = split_url(url);
    let idx = base.find("/s/")?;
    let prefix = &base[..idx];
    let token: String = base[idx + 3..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_'))
        .collect();
    if token.is_empty() {
        return None;
    }
    Some((prefix.to_string(), token))
}

/// Work out which cloud a URL belongs to purely from its host and path.
pub fn detect_provider(url: &str) -> Option<Provider> {
    let host = host_of(url);
    if host.is_empty() {
        return None;
    }
    if host_is(&host, "google.com") && (host.starts_with("drive") || host.starts_with("docs")) {
        return Some(Provider::GoogleDrive);
    }
    if host_is(&host, "dropbox.com") || host_is(&host, "dropboxusercontent.com") {
        return Some(Provider::Dropbox);
    }
    if host_is(&host, "1drv.ms")
        || host_is(&host, "onedrive.live.com")
        || host_is(&host, "sharepoint.com")
        || host_is(&host, "onedrive.com")
    {
        return Some(Provider::OneDrive);
    }
    // Anything else carrying a Nextcloud/ownCloud public-share path.
    if nextcloud_parts(url).is_some() {
        return Some(Provider::Nextcloud);
    }
    None
}

// ---------------------------------------------------------------------------
// Google Drive
// ---------------------------------------------------------------------------

fn is_id_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn take_id(s: &str) -> String {
    s.chars().take_while(|&c| is_id_char(c)).collect()
}

/// Real Drive IDs are 28–44 chars; accept ≥ 12 so future shapes still parse
/// while short path words like `edit` never match.
fn is_valid_id(s: &str) -> bool {
    s.len() >= 12 && s.chars().all(is_id_char)
}

fn query_value(url: &str, key: &str) -> Option<String> {
    let (_, query, _) = split_url(url);
    for pair in query.split('&') {
        if let Some(v) = pair.strip_prefix(&format!("{key}=")) {
            return Some(v.to_string());
        }
    }
    None
}

/// `document` / `spreadsheets` / `presentation` when the URL is a Google Docs
/// editor link (`docs.google.com/<kind>/d/<id>/edit`).
fn docs_kind(url: &str) -> Option<&'static str> {
    let (base, _, _) = split_url(url);
    for kind in ["document", "spreadsheets", "presentation"] {
        if base.contains(&format!("/{kind}/d/")) {
            return Some(kind);
        }
    }
    None
}

fn google_file_id(url: &str) -> Option<String> {
    let (base, _, _) = split_url(url);
    for marker in ["/file/d/", "/folders/", "/d/"] {
        if let Some(idx) = base.find(marker) {
            let id = take_id(&base[idx + marker.len()..]);
            if is_valid_id(&id) {
                return Some(id);
            }
        }
    }
    if let Some(v) = query_value(url, "id") {
        let id = take_id(&v);
        if is_valid_id(&id) {
            return Some(id);
        }
    }
    None
}

fn convert_google(url: &str, o: &Opts) -> Result<(String, Option<String>), String> {
    let (base, _, _) = split_url(url);
    if base.contains("/folders/") || base.contains("/drive/folders") {
        return Err(format!(
            "'{url}' is a Google Drive FOLDER link — a folder has no single direct-download URL (Drive only zips folders from its own web UI). Share the individual file and paste that link instead."
        ));
    }
    let id = google_file_id(url).ok_or_else(|| {
        format!(
            "no Google Drive file ID found in '{url}' — expected a link like https://drive.google.com/file/d/FILE_ID/view or https://drive.google.com/open?id=FILE_ID"
        )
    })?;

    // Docs/Sheets/Slides editor links export to a real file format instead.
    if let Some(kind) = docs_kind(url) {
        let fmt = o.docs_export.token(kind);
        return Ok((
            format!("https://docs.google.com/{kind}/d/{id}/export?format={fmt}"),
            Some(format!("{id}.{fmt}")),
        ));
    }

    let link = match o.mode {
        // The usercontent download host with the confirm token serves large
        // files straight through instead of the virus-scan interstitial.
        Mode::Download => format!(
            "https://drive.usercontent.google.com/download?id={id}&export=download&confirm=t"
        ),
        Mode::Inline => format!("https://drive.google.com/uc?export=view&id={id}"),
    };
    Ok((link, None))
}

// ---------------------------------------------------------------------------
// Dropbox
// ---------------------------------------------------------------------------

/// Last PATH segment, when it looks like a file name (has an extension). The
/// host is skipped first so a bare `https://1drv.ms` never reads as "1drv.ms".
fn path_file_name(url: &str) -> Option<String> {
    let (base, _, _) = split_url(url);
    let rest = base.split_once("://").map(|(_, r)| r).unwrap_or(base);
    let path = rest.split_once('/')?.1;
    let last = path.rsplit('/').next()?;
    if last.is_empty() || !last.contains('.') {
        return None;
    }
    Some(percent_decode(last))
}

fn convert_dropbox(url: &str, o: &Opts) -> Result<(String, Option<String>), String> {
    let (base, _, _) = split_url(url);
    let host = host_of(url);
    // Both share shapes: the legacy `/s/<id>/<name>` and the current
    // `/scl/fi/<id>/<name>` (files) / `/scl/fo/<id>` (folders, served as a zip).
    if !(base.contains("/s/") || base.contains("/scl/")) {
        return Err(format!(
            "'{url}' is not a Dropbox share link — expected https://www.dropbox.com/scl/fi/FILE_ID/name.ext?rlkey=… or https://www.dropbox.com/s/FILE_ID/name.ext"
        ));
    }
    // Content-host links (dl.dropboxusercontent.com) are normalised back to the
    // share host so the flag below is the one Dropbox actually honours.
    let normalised = if host_is(&host, "dropboxusercontent.com") {
        url.replacen(&host, "www.dropbox.com", 1)
    } else {
        url.to_string()
    };
    let link = match o.mode {
        Mode::Download => set_query_param(&normalised, "dl", "1", &["raw"]),
        Mode::Inline => set_query_param(&normalised, "raw", "1", &["dl"]),
    };
    Ok((link, path_file_name(url)))
}

// ---------------------------------------------------------------------------
// OneDrive
// ---------------------------------------------------------------------------

fn onedrive_api_url(url: &str) -> String {
    format!(
        "https://api.onedrive.com/v1.0/shares/u!{}/root/content",
        base64url_nopad(url.trim().as_bytes())
    )
}

fn convert_onedrive(url: &str, o: &Opts) -> Result<(String, Option<String>), String> {
    if host_of(url).is_empty() {
        return Err(format!(
            "'{url}' is not a OneDrive share link — expected an absolute https:// URL such as https://1drv.ms/u/s!TOKEN or https://onedrive.live.com/…"
        ));
    }
    let link = match (o.mode, o.onedrive_style) {
        // The shares/content endpoint returns the raw bytes, so it is also the
        // only OneDrive form that works for embedding.
        (Mode::Inline, _) | (Mode::Download, OneDriveStyle::Api) => onedrive_api_url(url),
        (Mode::Download, OneDriveStyle::DownloadParam) => set_query_param(url, "download", "1", &[]),
    };
    Ok((link, path_file_name(url)))
}

// ---------------------------------------------------------------------------
// Nextcloud / ownCloud
// ---------------------------------------------------------------------------

fn convert_nextcloud(url: &str, o: &Opts) -> Result<(String, Option<String>), String> {
    let (prefix, token) = nextcloud_parts(url).ok_or_else(|| {
        format!(
            "no Nextcloud/ownCloud share token found in '{url}' — expected a public share link like https://cloud.example.com/s/TOKEN or https://cloud.example.com/index.php/s/TOKEN"
        )
    })?;
    if host_of(url).is_empty() {
        return Err(format!(
            "'{url}' is missing a scheme and host — paste the whole share link, e.g. https://cloud.example.com/s/{token}"
        ));
    }
    let endpoint = match o.mode {
        Mode::Download => "download",
        // Nextcloud's preview endpoint renders the file inline (images/PDF
        // thumbnails); it is the embed counterpart of /download.
        Mode::Inline => "preview",
    };
    let mut link = format!("{prefix}/s/{token}/{endpoint}");
    let file = o.file.trim().trim_start_matches('/');
    let mut name = None;
    if !file.is_empty() {
        // "sub/dir/report.pdf" → path=/sub/dir & files=report.pdf
        let (dir, base) = match file.rsplit_once('/') {
            Some((d, b)) => (format!("/{d}"), b.to_string()),
            None => ("/".to_string(), file.to_string()),
        };
        link.push_str(&format!(
            "?path={}&files={}",
            percent_encode(&dir),
            percent_encode(&base)
        ));
        name = Some(base);
    }
    Ok((link, name))
}

// ---------------------------------------------------------------------------
// Rendering + entry point
// ---------------------------------------------------------------------------

fn render(link: &str, name: Option<&str>, output: Output) -> String {
    let label = name.unwrap_or("download");
    match output {
        Output::Url => link.to_string(),
        Output::Markdown => format!("[{label}]({link})"),
        Output::Html => format!("<a href=\"{link}\">{label}</a>"),
        Output::Curl => match name {
            Some(n) => format!("curl -L -o \"{n}\" \"{link}\""),
            // No name in the URL: let the server's Content-Disposition name it.
            None => format!("curl -L -OJ \"{link}\""),
        },
        Output::Wget => match name {
            Some(n) => format!("wget -O \"{n}\" \"{link}\""),
            None => format!("wget --content-disposition \"{link}\""),
        },
    }
}

fn convert_one(raw: &str, o: &Opts) -> Result<String, String> {
    let url = raw.trim();
    let provider = match o.provider {
        Some(p) => {
            // An explicit override still has to be plausible, or the user gets a
            // confidently wrong URL instead of an error.
            if let Some(detected) = detect_provider(url) {
                if detected != p {
                    return Err(format!(
                        "'{url}' looks like a {} link, but provider was forced to {} — set provider=auto or paste a {} link",
                        detected.label(),
                        p.label(),
                        p.label()
                    ));
                }
            }
            p
        }
        None => detect_provider(url).ok_or_else(|| {
            format!(
                "could not detect a cloud provider in '{url}' — supported: Google Drive (drive.google.com / docs.google.com), Dropbox (dropbox.com), OneDrive (1drv.ms / onedrive.live.com / *.sharepoint.com) and Nextcloud/ownCloud public shares (any host with a /s/TOKEN path)"
            )
        })?,
    };
    let (link, name) = match provider {
        Provider::GoogleDrive => convert_google(url, o)?,
        Provider::Dropbox => convert_dropbox(url, o)?,
        Provider::OneDrive => convert_onedrive(url, o)?,
        Provider::Nextcloud => convert_nextcloud(url, o)?,
    };
    Ok(render(&link, name.as_deref(), o.output))
}

/// Rewrite one share URL (or a whole batch, one per line) into its direct link.
///
/// `provider` is `auto` or a forced provider; `mode` is `download`|`inline`;
/// `output` is `url`|`markdown`|`html`|`curl`|`wget`; `onedrive_style` is
/// `api`|`download_param`; `docs_export` is `pdf`|`office`|`txt`; `file` names a
/// file inside a Nextcloud folder share; `per_line` converts each line
/// independently (blank lines preserved).
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    provider: &str,
    mode: &str,
    output: &str,
    onedrive_style: &str,
    docs_export: &str,
    file: &str,
    per_line: bool,
) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err(
            "input is empty — paste a share link, e.g. https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=k&dl=0"
                .into(),
        );
    }
    let o = Opts {
        provider: Provider::parse(provider)?,
        mode: Mode::parse(mode)?,
        output: Output::parse(output)?,
        onedrive_style: OneDriveStyle::parse(onedrive_style)?,
        docs_export: DocsExport::parse(docs_export)?,
        file: file.to_string(),
    };
    if !per_line {
        return convert_one(input, &o);
    }
    let mut out = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            out.push(String::new());
        } else {
            out.push(convert_one(line, &o)?);
        }
    }
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GID: &str = "1A2b3C4d5E6f7G8h9I0jKlMnOpQrStUvW";

    /// Defaults: auto-detect, download, plain URL, api style, pdf export, no file.
    fn conv(input: &str) -> Result<String, String> {
        convert(input, "auto", "download", "url", "api", "pdf", "", false)
    }

    #[test]
    fn detects_each_provider() {
        assert_eq!(
            detect_provider(&format!("https://drive.google.com/file/d/{GID}/view")),
            Some(Provider::GoogleDrive)
        );
        assert_eq!(
            detect_provider("https://www.dropbox.com/scl/fi/abc/report.pdf?rlkey=k&dl=0"),
            Some(Provider::Dropbox)
        );
        assert_eq!(
            detect_provider("https://1drv.ms/u/s!AhJrpDQRn5d"),
            Some(Provider::OneDrive)
        );
        assert_eq!(
            detect_provider("https://cloud.example.com/s/yxcFKRWBJqYYzp4"),
            Some(Provider::Nextcloud)
        );
        assert_eq!(detect_provider("https://example.com/files/report.pdf"), None);
        assert_eq!(detect_provider("not a url"), None);
    }

    // --- Google Drive ------------------------------------------------------

    #[test]
    fn google_share_link_to_direct_download() {
        let u = format!("https://drive.google.com/file/d/{GID}/view?usp=sharing");
        assert_eq!(
            conv(&u).unwrap(),
            format!("https://drive.usercontent.google.com/download?id={GID}&export=download&confirm=t")
        );
    }

    #[test]
    fn google_open_id_and_inline_embed() {
        let u = format!("https://drive.google.com/open?id={GID}");
        assert_eq!(
            convert(&u, "auto", "inline", "url", "api", "pdf", "", false).unwrap(),
            format!("https://drive.google.com/uc?export=view&id={GID}")
        );
    }

    #[test]
    fn google_docs_export_formats() {
        let doc = format!("https://docs.google.com/document/d/{GID}/edit?usp=sharing");
        assert_eq!(
            conv(&doc).unwrap(),
            format!("https://docs.google.com/document/d/{GID}/export?format=pdf")
        );
        assert_eq!(
            convert(&doc, "auto", "download", "url", "api", "office", "", false).unwrap(),
            format!("https://docs.google.com/document/d/{GID}/export?format=docx")
        );
        let sheet = format!("https://docs.google.com/spreadsheets/d/{GID}/edit#gid=0");
        assert_eq!(
            convert(&sheet, "auto", "download", "url", "api", "office", "", false).unwrap(),
            format!("https://docs.google.com/spreadsheets/d/{GID}/export?format=xlsx")
        );
        assert_eq!(
            convert(&sheet, "auto", "download", "url", "api", "txt", "", false).unwrap(),
            format!("https://docs.google.com/spreadsheets/d/{GID}/export?format=csv")
        );
        let slides = format!("https://docs.google.com/presentation/d/{GID}/edit");
        assert_eq!(
            convert(&slides, "auto", "download", "url", "api", "office", "", false).unwrap(),
            format!("https://docs.google.com/presentation/d/{GID}/export?format=pptx")
        );
    }

    #[test]
    fn google_folder_link_errors() {
        let u = format!("https://drive.google.com/drive/folders/{GID}?usp=sharing");
        let e = conv(&u).unwrap_err();
        assert!(e.contains("FOLDER"), "{e}");
    }

    // --- Dropbox -----------------------------------------------------------

    #[test]
    fn dropbox_scl_link_flips_dl_flag() {
        let u = "https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&st=aa&dl=0";
        assert_eq!(
            conv(u).unwrap(),
            "https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&st=aa&dl=1"
        );
    }

    #[test]
    fn dropbox_legacy_link_and_inline_raw() {
        let u = "https://www.dropbox.com/s/snmff8j8dapsnmm/Getting%20Started.pdf?dl=0";
        assert_eq!(
            conv(u).unwrap(),
            "https://www.dropbox.com/s/snmff8j8dapsnmm/Getting%20Started.pdf?dl=1"
        );
        assert_eq!(
            convert(u, "auto", "inline", "url", "api", "pdf", "", false).unwrap(),
            "https://www.dropbox.com/s/snmff8j8dapsnmm/Getting%20Started.pdf?raw=1"
        );
    }

    #[test]
    fn dropbox_content_host_is_normalised() {
        let u = "https://dl.dropboxusercontent.com/s/abc/report.pdf";
        assert_eq!(
            conv(u).unwrap(),
            "https://www.dropbox.com/s/abc/report.pdf?dl=1"
        );
    }

    #[test]
    fn dropbox_non_share_url_errors() {
        let e = conv("https://www.dropbox.com/home").unwrap_err();
        assert!(e.contains("not a Dropbox share link"), "{e}");
    }

    // --- OneDrive ----------------------------------------------------------

    #[test]
    fn onedrive_api_style_base64url() {
        let u = "https://1drv.ms/u/s!AhJrpDQRn5d";
        // base64url of the share URL, no padding, prefixed with `u!`.
        let expected = format!(
            "https://api.onedrive.com/v1.0/shares/u!{}/root/content",
            base64url_nopad(u.as_bytes())
        );
        assert_eq!(conv(u).unwrap(), expected);
        assert!(!expected.contains('='), "no base64 padding: {expected}");
    }

    #[test]
    fn onedrive_download_param_style() {
        let u = "https://1drv.ms/t/s!AhJrpDQRn5d?e=YFQlMA";
        assert_eq!(
            convert(u, "auto", "download", "url", "download_param", "pdf", "", false).unwrap(),
            "https://1drv.ms/t/s!AhJrpDQRn5d?e=YFQlMA&download=1"
        );
    }

    #[test]
    fn base64url_matches_known_vectors() {
        assert_eq!(base64url_nopad(b""), "");
        assert_eq!(base64url_nopad(b"f"), "Zg");
        assert_eq!(base64url_nopad(b"fo"), "Zm8");
        assert_eq!(base64url_nopad(b"foo"), "Zm9v");
        assert_eq!(base64url_nopad(b"foobar"), "Zm9vYmFy");
        // URL-safe alphabet: `+`/`/` become `-`/`_`.
        assert_eq!(base64url_nopad(&[0xfb, 0xff, 0xfe]), "-__-");
    }

    // --- Nextcloud ---------------------------------------------------------

    #[test]
    fn nextcloud_share_to_download() {
        assert_eq!(
            conv("https://cloud.example.com/s/yxcFKRWBJqYYzp4").unwrap(),
            "https://cloud.example.com/s/yxcFKRWBJqYYzp4/download"
        );
        assert_eq!(
            conv("https://cloud.example.com/index.php/s/yxcFKRWBJqYYzp4").unwrap(),
            "https://cloud.example.com/index.php/s/yxcFKRWBJqYYzp4/download"
        );
    }

    #[test]
    fn nextcloud_file_inside_a_folder_share() {
        assert_eq!(
            convert(
                "https://cloud.example.com/s/TOKEN123",
                "auto",
                "download",
                "url",
                "api",
                "pdf",
                "reports/Q3 report.pdf",
                false
            )
            .unwrap(),
            "https://cloud.example.com/s/TOKEN123/download?path=%2Freports&files=Q3%20report.pdf"
        );
    }

    #[test]
    fn nextcloud_inline_uses_preview_endpoint() {
        assert_eq!(
            convert(
                "https://cloud.example.com/s/TOKEN123",
                "auto",
                "inline",
                "url",
                "api",
                "pdf",
                "",
                false
            )
            .unwrap(),
            "https://cloud.example.com/s/TOKEN123/preview"
        );
    }

    #[test]
    fn nextcloud_on_a_custom_port() {
        assert_eq!(
            conv("https://files.example.org:8443/index.php/s/AbC-1_2").unwrap(),
            "https://files.example.org:8443/index.php/s/AbC-1_2/download"
        );
    }

    // --- Output forms ------------------------------------------------------

    #[test]
    fn output_forms_render_commands_and_links() {
        let u = "https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&dl=0";
        let direct = "https://www.dropbox.com/scl/fi/abc123/report.pdf?rlkey=xyz&dl=1";
        assert_eq!(
            convert(u, "auto", "download", "markdown", "api", "pdf", "", false).unwrap(),
            format!("[report.pdf]({direct})")
        );
        assert_eq!(
            convert(u, "auto", "download", "html", "api", "pdf", "", false).unwrap(),
            format!("<a href=\"{direct}\">report.pdf</a>")
        );
        assert_eq!(
            convert(u, "auto", "download", "curl", "api", "pdf", "", false).unwrap(),
            format!("curl -L -o \"report.pdf\" \"{direct}\"")
        );
        assert_eq!(
            convert(u, "auto", "download", "wget", "api", "pdf", "", false).unwrap(),
            format!("wget -O \"report.pdf\" \"{direct}\"")
        );
    }

    #[test]
    fn output_commands_fall_back_when_the_name_is_unknown() {
        let u = format!("https://drive.google.com/file/d/{GID}/view");
        let got = convert(&u, "auto", "download", "curl", "api", "pdf", "", false).unwrap();
        assert!(got.starts_with("curl -L -OJ \""), "{got}");
        let got = convert(&u, "auto", "download", "wget", "api", "pdf", "", false).unwrap();
        assert!(got.starts_with("wget --content-disposition \""), "{got}");
    }

    // --- Batch + errors ----------------------------------------------------

    #[test]
    fn per_line_batch_preserves_blank_lines() {
        let input = format!(
            "https://drive.google.com/file/d/{GID}/view\n\nhttps://cloud.example.com/s/TOKEN123"
        );
        let got = convert(&input, "auto", "download", "url", "api", "pdf", "", true).unwrap();
        assert_eq!(
            got,
            format!(
                "https://drive.usercontent.google.com/download?id={GID}&export=download&confirm=t\n\nhttps://cloud.example.com/s/TOKEN123/download"
            )
        );
    }

    #[test]
    fn forced_provider_mismatch_errors() {
        let u = "https://www.dropbox.com/scl/fi/abc123/report.pdf?dl=0";
        let e = convert(u, "onedrive", "download", "url", "api", "pdf", "", false).unwrap_err();
        assert!(e.contains("looks like a Dropbox link"), "{e}");
    }

    #[test]
    fn unknown_provider_errors() {
        let e = conv("https://example.com/files/report.pdf").unwrap_err();
        assert!(e.contains("could not detect a cloud provider"), "{e}");
    }

    #[test]
    fn empty_input_errors() {
        assert!(conv("   ").is_err());
    }

    #[test]
    fn unknown_enum_values_error() {
        let u = "https://cloud.example.com/s/TOKEN123";
        assert!(convert(u, "icloud", "download", "url", "api", "pdf", "", false).is_err());
        assert!(convert(u, "auto", "sideways", "url", "api", "pdf", "", false).is_err());
        assert!(convert(u, "auto", "download", "yaml", "api", "pdf", "", false).is_err());
        assert!(convert(u, "auto", "download", "url", "magic", "pdf", "", false).is_err());
        assert!(convert(u, "auto", "download", "url", "api", "rtf", "", false).is_err());
    }
}
