//! gizza-ai/docx-comment-extractor — pull the tracked review comments out of a
//! `.docx` (Microsoft Word) file and return them as a spreadsheet-ready table.
//!
//! Pipeline: parse `{url|ref, format, columns, authors, status, include_replies,
//! flatten_newlines}` → fetch the document bytes via `block-utils`
//! `resolve_source` (URL fetch through `wafer-run/network`, or an uploaded
//! attachment ref) → delegate to the pure `core::extract` (which reads
//! `word/comments.xml`, the comment anchors in the body parts, and the
//! `commentsExtended` thread/status part) → return a flat JSON response the LLM
//! reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). The handler stays thin (parse `Args`, build `Options`,
//! run the extraction, emit the flat `Resp` JSON) rather than going through
//! `run_skill`, because the success shape is the flat `Resp` JSON with the row
//! counts alongside the table, not the `{ "result": … }` wrapper.
//!
//! No page surface: a document is a binary file input and the output is text,
//! which fits neither the pure-text nor the ffmpeg file→media page shapes — this
//! is a chat + CLI block (the "no-page file-input" pattern, like
//! `docx-text-extract` / `pdf-extract-text`).

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting imports
// + the Args type are only used inside that impl, so they look "unused" when
// running native unit tests. Block-local helpers stay native-compilable so the
// unit tests below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_docx_comment_extractor_core::{
    extract, parse_authors, parse_columns, Extraction, Format, Options, StatusFilter, ALL_COLUMNS,
    DEFAULT_COLUMNS,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB
const MAX_OUTPUT_CHARS: usize = 1_000_000; // 1M chars of rendered table

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Output table format. Defaults to `csv`.
    #[serde(default)]
    format: Option<String>,
    /// Comma-separated column selection. Defaults to `core::DEFAULT_COLUMNS`.
    #[serde(default)]
    columns: Option<String>,
    /// Comma-separated author filter terms. Blank/absent keeps every author.
    #[serde(default)]
    authors: Option<String>,
    /// Resolved/open filter. Defaults to `all`.
    #[serde(default)]
    status: Option<String>,
    /// Keep replies within a thread. Defaults to true.
    #[serde(default)]
    include_replies: Option<bool>,
    /// Collapse newlines inside a cell so one comment is one row. Defaults to true.
    #[serde(default)]
    flatten_newlines: Option<bool>,
}

#[derive(Serialize)]
struct Resp {
    /// The rendered comment table in the requested format.
    output: String,
    /// Comments found in the document, before any filter.
    total: usize,
    /// Rows actually emitted (after the author/status/reply filters).
    returned: usize,
    /// Emitted rows that are replies to another comment.
    replies: usize,
    /// Emitted rows whose thread is marked resolved.
    resolved: usize,
    /// Every distinct comment author in the document, before any filter.
    authors: Vec<String>,
    /// True when the table was clipped to the output cap.
    truncated: bool,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf`; the rest select the output shape and the
/// author/status/reply filters.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("format", ["csv", "tsv", "json", "markdown"])
                .default("csv")
                .describe(
                    "Output table format: `csv` (default, RFC 4180 quoting), `tsv` \
                     (tab-separated), `json` (an array of row objects), or `markdown` \
                     (a pipe table).",
                ),
        )
        .param(
            Param::string("columns")
                .default(DEFAULT_COLUMNS)
                .describe(&format!(
                    "Comma-separated columns to emit, in the order given. Available: {}. \
                     Defaults to `{}` (`initials` and the raw ISO `timestamp` are opt-in, \
                     being redundant with `author` and `date`+`time`).",
                    ALL_COLUMNS.join(", "),
                    DEFAULT_COLUMNS
                )),
        )
        .param(Param::string("authors").describe(
            "Comma-separated author filter, matched case-insensitively as a substring of \
             the comment author's name (e.g. `jane` matches \"Jane Doe\"). Leave empty to \
             keep every author.",
        ))
        .param(
            Param::enumv("status", ["all", "open", "resolved"])
                .default("all")
                .describe(
                    "Which comments to keep by thread state: `all` (default), `open` (not \
                     yet marked resolved in Word), or `resolved`.",
                ),
        )
        .param(Param::boolean("include_replies").default(true).describe(
            "Include replies within a comment thread (true, default). Set false to return \
             only top-level comments.",
        ))
        .param(Param::boolean("flatten_newlines").default(true).describe(
            "Collapse newlines and repeated whitespace inside each cell so one comment is \
             one row (true, default). Set false to keep multi-paragraph comments' line \
             breaks inside the quoted cell.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Parse + validate the non-source args into the core [`Options`]. Every
/// unset param falls back to the core default, so chat, CLI and the core unit
/// tests all agree on the defaults.
fn build_options(args: &Args) -> Result<Options, SkillError> {
    let d = Options::default();
    Ok(Options {
        format: match args.format.as_deref() {
            Some(s) => Format::parse(s).map_err(SkillError::InvalidArgs)?,
            None => d.format,
        },
        columns: match args.columns.as_deref() {
            Some(s) => parse_columns(s).map_err(SkillError::InvalidArgs)?,
            None => d.columns,
        },
        authors: args
            .authors
            .as_deref()
            .map(parse_authors)
            .unwrap_or_default(),
        status: match args.status.as_deref() {
            Some(s) => StatusFilter::parse(s).map_err(SkillError::InvalidArgs)?,
            None => d.status,
        },
        include_replies: args.include_replies.unwrap_or(d.include_replies),
        flatten_newlines: args.flatten_newlines.unwrap_or(d.flatten_newlines),
    })
}

/// Build the flat response from an extraction, applying the output-cap clip.
fn build_resp(e: Extraction) -> Resp {
    let truncated = e.output.chars().count() > MAX_OUTPUT_CHARS;
    let output = if truncated {
        e.output.chars().take(MAX_OUTPUT_CHARS).collect()
    } else {
        e.output
    };
    Resp {
        output,
        total: e.total,
        returned: e.returned,
        replies: e.replies,
        resolved: e.resolved,
        authors: e.authors,
        truncated,
    }
}

#[cfg(target_arch = "wasm32")]
struct DocxCommentExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/docx-comment-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract Word (DOCX) review comments to a CSV/TSV/JSON/Markdown table",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract the tracked review comments from a Microsoft Word .docx file and return them as a spreadsheet-ready table. Each row carries the comment id, its thread parent, the author (and initials), the date/time, the open/resolved status, the document text the comment is anchored to, and the comment text itself. Provide url (HTTP/HTTPS) or ref from a prior tool call. Choose the output with format (csv, tsv, json, markdown) and columns; narrow the rows with authors (case-insensitive substring), status (all, open, resolved), and include_replies. Returns the table plus the total/returned/replies/resolved counts and the document's full author roster. Reads only the review layer, not the document prose (use docx-text-extract for that); .docx only, not legacy .doc.",
        parameters = schema_json()
    ),
)]
impl DocxCommentExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // Returns the flat Resp JSON directly (no `{ "result": … }` wrapper), so
        // it keeps a thin handle rather than using run_skill.
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("docx-comment-extractor")?;
    let opts = build_options(&args)?;

    // 2. Resolve source — URL fetch or attachment lookup. `AssetKind::Any`: a
    //    server may label a DOCX as octet-stream, so we don't gate on MIME and
    //    instead verify the ZIP magic bytes in the pure core.
    let (input_bytes, _mime, _filename) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;

    // 3. Extract via the pure core. Maps parse errors to InvalidArgs.
    let extraction = extract(&input_bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let resp = build_resp(extraction);
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize docx-comment-extractor response: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use gizza_ai_block_utils::Source;
    use gizza_ai_docx_comment_extractor_core::Column;

    use super::*;

    fn args(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn args_parse_url_and_leave_every_option_unset() {
        let a = args(r#"{"url":"https://x/y.docx"}"#);
        assert!(a.format.is_none() && a.columns.is_none() && a.authors.is_none());
        assert!(a.status.is_none() && a.include_replies.is_none() && a.flatten_newlines.is_none());
        assert!(matches!(a.source.into_inner(), Source::Url(u) if u == "https://x/y.docx"));
    }

    #[test]
    fn args_parse_ref_with_options() {
        let a = args(r#"{"ref":"call_7","format":"json","include_replies":false}"#);
        assert_eq!(a.format.as_deref(), Some("json"));
        assert_eq!(a.include_replies, Some(false));
        assert!(matches!(a.source.into_inner(), Source::Ref(r) if r == "call_7"));
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn build_options_defaults_match_the_core_defaults() {
        let o = build_options(&args(r#"{"url":"https://x/y.docx"}"#)).unwrap();
        let d = Options::default();
        assert_eq!(o.format, d.format);
        assert_eq!(o.columns, d.columns);
        assert_eq!(o.status, d.status);
        assert!(o.authors.is_empty());
        assert!(o.include_replies && o.flatten_newlines);
    }

    #[test]
    fn build_options_applies_every_param() {
        let o = build_options(&args(
            r#"{"ref":"r","format":"markdown","columns":"author, comment",
                "authors":"Jane, sam","status":"open",
                "include_replies":false,"flatten_newlines":false}"#,
        ))
        .unwrap();
        assert_eq!(o.format, Format::Markdown);
        assert_eq!(o.columns, vec![Column::Author, Column::Comment]);
        assert_eq!(o.authors, vec!["jane".to_string(), "sam".to_string()]);
        assert_eq!(o.status, StatusFilter::Open);
        assert!(!o.include_replies && !o.flatten_newlines);
    }

    #[test]
    fn build_options_rejects_bad_enum_and_column_values() {
        for bad in [
            r#"{"ref":"r","format":"xlsx"}"#,
            r#"{"ref":"r","status":"done"}"#,
            r#"{"ref":"r","columns":"author,page"}"#,
        ] {
            assert!(build_options(&args(bad)).is_err(), "should reject: {bad}");
        }
    }

    #[test]
    fn build_resp_carries_the_counts_through() {
        let r = build_resp(Extraction {
            output: "id,author\n1,Jane Doe".into(),
            total: 3,
            returned: 1,
            replies: 0,
            resolved: 1,
            authors: vec!["Jane Doe".into()],
        });
        assert_eq!(r.output, "id,author\n1,Jane Doe");
        assert_eq!((r.total, r.returned, r.replies, r.resolved), (3, 1, 0, 1));
        assert_eq!(r.authors, vec!["Jane Doe".to_string()]);
        assert!(!r.truncated);
    }
}
