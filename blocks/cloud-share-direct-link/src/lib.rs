//! gizza-ai/cloud-share-direct-link — rewrite a Nextcloud, Dropbox, Google Drive
//! or OneDrive share-page URL into the direct-download URL for the file itself.
//!
//! Thin chat-skill wrapper around `gizza-ai-cloud-share-direct-link-core`. The
//! chat schema is derived from `descriptor()` (single source — shared across chat
//! + CLI + page query-params); the handler delegates to `block_utils::run_skill`.
//! No host calls — every rewrite is string surgery inside the WASM sandbox, so
//! the tool never contacts Google, Dropbox, Microsoft, or your Nextcloud server.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_cloud_share_direct_link_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default = "default_provider")]
    provider: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_onedrive_style")]
    onedrive_style: String,
    #[serde(default = "default_docs_export")]
    docs_export: String,
    #[serde(default)]
    file: String,
    #[serde(default = "default_per_line")]
    per_line: bool,
}

fn default_provider() -> String {
    "auto".to_string()
}

fn default_mode() -> String {
    "download".to_string()
}

fn default_output() -> String {
    "url".to_string()
}

fn default_onedrive_style() -> String {
    "api".to_string()
}

fn default_docs_export() -> String {
    "pdf".to_string()
}

fn default_per_line() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("url").required().describe(
                "The share-page URL to rewrite. Google Drive (https://drive.google.com/file/d/FILE_ID/view, open?id=FILE_ID, or a docs.google.com editor link), Dropbox (https://www.dropbox.com/scl/fi/ID/name.ext?rlkey=... or the older /s/ID/name.ext), OneDrive (https://1drv.ms/u/s!TOKEN, onedrive.live.com, *.sharepoint.com), or a Nextcloud/ownCloud public share on any host (https://cloud.example.com/s/TOKEN or /index.php/s/TOKEN). With per_line=true (the default) pass a batch, one URL per line.",
            ),
        )
        .param(
            Param::enumv("provider", ["auto", "google_drive", "dropbox", "onedrive", "nextcloud"])
                .default("auto")
                .describe(
                    "Which cloud the link belongs to. 'auto' (default) detects it from the URL's host and path; the explicit values force a provider and error if the URL clearly belongs to a different one. Force 'nextcloud' for a self-hosted server whose URL shape is unusual.",
                ),
        )
        .param(
            Param::enumv("mode", ["download", "inline"]).default("download").describe(
                "What the rewritten link should do. 'download' (default) forces the file to download; 'inline' produces a hotlink/embed URL that serves the bytes in the page (Drive uc?export=view, Dropbox raw=1, Nextcloud /preview, OneDrive's shares content endpoint).",
            ),
        )
        .param(
            Param::enumv("output", ["url", "markdown", "html", "curl", "wget"])
                .default("url")
                .describe(
                    "How to render each result. 'url' (default) = the bare link; 'markdown' = [name](link); 'html' = an <a href> anchor; 'curl' / 'wget' = a ready-to-paste command line. When the share URL contains the file name it is used for the link text and the -o/-O output file; otherwise the command falls back to the server-provided name (curl -OJ, wget --content-disposition).",
                ),
        )
        .param(
            Param::enumv("onedrive_style", ["api", "download_param"]).default("api").describe(
                "Which OneDrive rewrite to use for mode=download. 'api' (default) base64url-encodes the share URL into https://api.onedrive.com/v1.0/shares/u!ENCODED/root/content, the form that works most consistently; 'download_param' just adds download=1 to the share URL, which is simpler but often ignored. Ignored for other providers and for mode=inline.",
            ),
        )
        .param(
            Param::enumv("docs_export", ["pdf", "office", "txt"]).default("pdf").describe(
                "Export format for Google Docs/Sheets/Slides EDITOR links (docs.google.com/document|spreadsheets|presentation/d/ID). 'pdf' (default); 'office' = docx/xlsx/pptx to match the document kind; 'txt' = txt for Docs and Slides, csv for Sheets. Ignored for ordinary Drive files and other providers.",
            ),
        )
        .param(
            Param::string("file").default("").describe(
                "Optional: the name of ONE file inside a Nextcloud/ownCloud FOLDER share, e.g. 'report.pdf' or 'reports/Q3 report.pdf'. It is split into the path= and files= query pair Nextcloud expects. Leave empty for a single-file share. Ignored for the other providers.",
            ),
        )
        .param(
            Param::boolean("per_line").default(true).describe(
                "When true (the default), convert each line of the input independently and rejoin with newlines, preserving blank lines — paste a whole column of share links. Set false to treat the entire input as one URL.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CloudShareDirectLink;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cloud-share-direct-link",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rewrite a Nextcloud, Dropbox, Google Drive or OneDrive share link into its direct-download URL",
    skill(
        description = "Turn a cloud share-page URL into the direct link to the file itself. Supports Google Drive (file, open?id and Docs/Sheets/Slides editor links), Dropbox (both the /scl/ and legacy /s/ share shapes), OneDrive (1drv.ms, onedrive.live.com, SharePoint) and Nextcloud/ownCloud public shares on any host, with the provider auto-detected from the URL. mode=download (default) forces a download; mode=inline gives a hotlink/embed URL. output renders the result as a bare url, markdown, html, or a curl/wget command. onedrive_style picks between the shares-API form and a download=1 flag; docs_export picks pdf/office/txt for Google editor links; file addresses one file inside a Nextcloud folder share; per_line (default true) converts a whole batch, one link per line. Pure string rewriting — it never contacts any cloud provider, and a folder link or an unrecognised host returns an explicit error rather than a link that 404s.",
        parameters = schema_json()
    ),
)]
impl CloudShareDirectLink {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cloud-share-direct-link", |a: Args| {
            convert(
                &a.url,
                &a.provider,
                &a.mode,
                &a.output,
                &a.onedrive_style,
                &a.docs_export,
                &a.file,
                a.per_line,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "The share-page URL to rewrite. Google Drive (https://drive.google.com/file/d/FILE_ID/view, open?id=FILE_ID, or a docs.google.com editor link), Dropbox (https://www.dropbox.com/scl/fi/ID/name.ext?rlkey=... or the older /s/ID/name.ext), OneDrive (https://1drv.ms/u/s!TOKEN, onedrive.live.com, *.sharepoint.com), or a Nextcloud/ownCloud public share on any host (https://cloud.example.com/s/TOKEN or /index.php/s/TOKEN). With per_line=true (the default) pass a batch, one URL per line." },
                    "provider": { "type": "string", "enum": ["auto", "google_drive", "dropbox", "onedrive", "nextcloud"], "default": "auto", "description": "Which cloud the link belongs to. 'auto' (default) detects it from the URL's host and path; the explicit values force a provider and error if the URL clearly belongs to a different one. Force 'nextcloud' for a self-hosted server whose URL shape is unusual." },
                    "mode": { "type": "string", "enum": ["download", "inline"], "default": "download", "description": "What the rewritten link should do. 'download' (default) forces the file to download; 'inline' produces a hotlink/embed URL that serves the bytes in the page (Drive uc?export=view, Dropbox raw=1, Nextcloud /preview, OneDrive's shares content endpoint)." },
                    "output": { "type": "string", "enum": ["url", "markdown", "html", "curl", "wget"], "default": "url", "description": "How to render each result. 'url' (default) = the bare link; 'markdown' = [name](link); 'html' = an <a href> anchor; 'curl' / 'wget' = a ready-to-paste command line. When the share URL contains the file name it is used for the link text and the -o/-O output file; otherwise the command falls back to the server-provided name (curl -OJ, wget --content-disposition)." },
                    "onedrive_style": { "type": "string", "enum": ["api", "download_param"], "default": "api", "description": "Which OneDrive rewrite to use for mode=download. 'api' (default) base64url-encodes the share URL into https://api.onedrive.com/v1.0/shares/u!ENCODED/root/content, the form that works most consistently; 'download_param' just adds download=1 to the share URL, which is simpler but often ignored. Ignored for other providers and for mode=inline." },
                    "docs_export": { "type": "string", "enum": ["pdf", "office", "txt"], "default": "pdf", "description": "Export format for Google Docs/Sheets/Slides EDITOR links (docs.google.com/document|spreadsheets|presentation/d/ID). 'pdf' (default); 'office' = docx/xlsx/pptx to match the document kind; 'txt' = txt for Docs and Slides, csv for Sheets. Ignored for ordinary Drive files and other providers." },
                    "file": { "type": "string", "default": "", "description": "Optional: the name of ONE file inside a Nextcloud/ownCloud FOLDER share, e.g. 'report.pdf' or 'reports/Q3 report.pdf'. It is split into the path= and files= query pair Nextcloud expects. Leave empty for a single-file share. Ignored for the other providers." },
                    "per_line": { "type": "boolean", "default": true, "description": "When true (the default), convert each line of the input independently and rejoin with newlines, preserving blank lines — paste a whole column of share links. Set false to treat the entire input as one URL." }
                },
                "required": ["url"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
