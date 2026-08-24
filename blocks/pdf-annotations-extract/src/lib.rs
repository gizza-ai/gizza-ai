//! gizza-ai/pdf-annotations-extract — collect every comment, highlight, sticky
//! note, drawing and stamp annotation out of a PDF, with page numbers, authors,
//! dates, colours, and the marked-up text under each highlight.
//!
//! No-page block (chat + CLI surface only, like `blocks/pdf-form-data-extract`
//! and `blocks/pdf-extract-text`): a PDF is a binary file input and the output is
//! delimited text / JSON / Markdown, which fits neither the pure-text page shape
//! nor the ffmpeg file→media page shape — so there is no standalone page.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI).
//!
//! Pipeline: parse `{url|ref}` + filters → resolve bytes via
//! `block_utils::resolve_source` (URL fetch or attachment lookup, validated to
//! the `application/*` `AssetKind::Document` class) → `core::extract_annotations`
//! (lopdf `/Annots` walk + a positioned text-layer walk that maps `/QuadPoints`
//! back onto the characters a highlight covers) → `core::serialize` → emit a text
//! `Envelope`. The LLM sees the annotation list (head-truncated if large); the UI
//! gets a downloadable `data:` URL + a `*.json`/`*.csv`/`*.md`/`*.txt` filename.

// The #[wafer_block] macro emits wasm-only registration; supporting imports and
// the Args type are only used inside that impl. `descriptor()` / `schema_json()`
// remain native-compilable so the drift-guard + unit tests below can run.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
// The source resolver calls the wasm-gated network/attachment host imports; the
// descriptor and arg parsing are host-testable.
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, AssetKind, Envelope, ForUi, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
use gizza_ai_pdf_annotations_extract_core::{
    extract_annotations, serialize, Annotation, Options, FORMAT_CHOICES, SORT_CHOICES,
    TYPE_CHOICES,
};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the PDF input we accept (matches the other PDF blocks; lopdf holds the
/// whole document in memory inside a 64 MiB wasm sandbox).
const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

/// Cap on the annotation text fed back to the LLM (`for_llm`). Larger results are
/// head-truncated with a note; the full output is always in the download.
const MAX_LLM_CHARS: usize = 16 * 1024; // ~16 KiB

#[derive(Debug, Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// Output format: `json` (default), `csv`, `markdown`, or `text`.
    #[serde(default)]
    format: Option<String>,
    /// Annotation kind filter: `all` (default) or one kind / the `markup` group.
    #[serde(default)]
    types: Option<String>,
    /// 1-based page spec, e.g. `"1,3-5"`. Empty/absent = every page.
    #[serde(default)]
    pages: Option<String>,
    /// Case-insensitive substring match on the annotation author.
    #[serde(default)]
    author: Option<String>,
    /// Recover the page text each highlight/underline covers (default true).
    #[serde(default)]
    include_marked_text: Option<bool>,
    /// Keep annotations with no comment and no marked text (default false).
    #[serde(default)]
    include_empty: Option<bool>,
    /// Result order: `page` (default), `author`, `type`, or `date`.
    #[serde(default)]
    sort: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the `url`⊕`ref` `oneOf`. The drift-guard test below proves the derived
/// schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::enumv("format", FORMAT_CHOICES).default("json").describe(
                "Output format: \"json\" (array of {page, type, subtype, author, date, color, comment, marked_text} objects), \"csv\" (one row per annotation with a header line), \"markdown\" (bullets grouped under a `## Page N` heading), or \"text\" (one flat line per annotation). Default \"json\".",
            ),
        )
        .param(
            Param::enumv("types", TYPE_CHOICES).default("all").describe(
                "Which annotations to keep: \"all\" (default, every kind), \"markup\" (highlight + underline + strikeout + squiggly together), or a single kind — \"highlight\", \"underline\", \"strikeout\", \"squiggly\", \"note\" (sticky note), \"freetext\" (callout box), \"drawing\" (ink/square/circle/line/polygon), \"stamp\", or \"link\". Form fields and popup windows are always skipped.",
            ),
        )
        .param(
            Param::string("pages").default("").describe(
                "1-based page spec limiting which pages are scanned, e.g. \"3\" or \"1,4-6\". Leave empty (the default) for the whole document.",
            ),
        )
        .param(
            Param::string("author").default("").describe(
                "Keep only annotations whose author contains this text, case-insensitively (e.g. \"ada\" matches \"Ada Lovelace\"). Empty (the default) keeps every author.",
            ),
        )
        .param(
            Param::boolean("include_marked_text").default(true).describe(
                "Recover the page text that each highlight/underline/strikeout/squiggly covers, by mapping the annotation's QuadPoints onto the PDF's text layer. Set false to skip that work and return only the typed comments. Default true.",
            ),
        )
        .param(
            Param::boolean("include_empty").default(false).describe(
                "Include annotations that carry neither a comment nor marked-up text — bare links, empty stamps, and unlabelled drawings. Text markup (highlights etc.) is always kept regardless. Default false.",
            ),
        )
        .param(
            Param::enumv("sort", SORT_CHOICES).default("page").describe(
                "Result order: \"page\" (default, document order), \"author\", \"type\", or \"date\". Ties always fall back to page order.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Map a format name to its `(mime, extension)` pair. Errors on an unknown name.
fn format_target(format: &str) -> Result<(&'static str, &'static str), SkillError> {
    match format {
        "json" => Ok(("application/json", "json")),
        "csv" => Ok(("text/csv", "csv")),
        "markdown" => Ok(("text/markdown", "md")),
        "text" => Ok(("text/plain", "txt")),
        other => Err(SkillError::InvalidArgs(format!(
            "unknown format {other:?}; use one of: {}",
            FORMAT_CHOICES.join(", ")
        ))),
    }
}

/// Build the `Options` the core takes from the parsed `Args`.
fn options_from(args: &Args) -> Options {
    Options {
        types: args.types.clone().unwrap_or_else(|| "all".to_string()),
        pages: args.pages.clone().unwrap_or_default(),
        author: args.author.clone().unwrap_or_default(),
        include_marked_text: args.include_marked_text.unwrap_or(true),
        include_empty: args.include_empty.unwrap_or(false),
        sort: args.sort.clone().unwrap_or_else(|| "page".to_string()),
    }
}

/// One-line summary of what came back, so the LLM has context above the payload.
fn summarize(list: &[Annotation], filename: &str, opts: &Options) -> String {
    if list.is_empty() {
        let mut why = String::new();
        if opts.types != "all" {
            why.push_str(&format!(" matching types \"{}\"", opts.types));
        }
        if !opts.author.trim().is_empty() {
            why.push_str(&format!(" by an author containing \"{}\"", opts.author.trim()));
        }
        if !opts.pages.trim().is_empty() {
            why.push_str(&format!(" on pages {}", opts.pages.trim()));
        }
        format!(
            "No annotations{why} in {filename}. (Comments a PDF viewer draws itself — form fields and popup windows — are never reported; a flattened PDF has no annotation layer left.)"
        )
    } else {
        let pages: std::collections::BTreeSet<u32> = list.iter().map(|a| a.page).collect();
        format!(
            "Found {} annotation(s) across {} page(s) in {filename}:",
            list.len(),
            pages.len()
        )
    }
}

/// Head-truncate `text` to `MAX_LLM_CHARS` for the LLM view, with a note.
fn clip_for_llm(text: &str) -> String {
    if text.chars().count() <= MAX_LLM_CHARS {
        text.to_string()
    } else {
        let head: String = text.chars().take(MAX_LLM_CHARS).collect();
        format!(
            "{head}\n… (truncated to {MAX_LLM_CHARS} of {} chars; full output in the download)",
            text.chars().count()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct PdfAnnotationsExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-annotations-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract PDF comments, highlights and sticky notes with pages and authors",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Collect every comment, highlight, sticky note, free-text callout, drawing, stamp and link annotation in a PDF, each with its page number, author, timestamp, colour, typed comment, and — for text markup — the page text the annotation covers. Provide the file via `url` (a public http/https link) or `ref` (an uploaded attachment id). Choose `format` (json/csv/markdown/text), narrow with `types` (all/markup/highlight/underline/strikeout/squiggly/note/freetext/drawing/stamp/link), `pages` (\"1,4-6\"), or `author` (case-insensitive substring), order with `sort` (page/author/type/date), and toggle `include_marked_text` and `include_empty`. Example: format=markdown, types=highlight returns `## Page 1` bullets like `- **highlight** — \u{201c}brown fox\u{201d} — check this animal _(Ada Lovelace, 2026-01-15T10:30:00+01:00)_`. Marked-up text is reconstructed from the text layer by position, so it can gain or lose a character at the edge of a highlight and returns nothing for scanned/image-only PDFs; form-field (Widget) and popup annotations are always skipped; input is capped at 16 MiB.",
        parameters = schema_json()
    ),
)]
impl PdfAnnotationsExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-annotations-extract")?;

    let format = args.format.clone().unwrap_or_else(|| "json".to_string());
    let (mime, ext) = format_target(&format)?;
    let opts = options_from(&args);

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let list = extract_annotations(&bytes, &opts).map_err(SkillError::InvalidArgs)?;
    let text = serialize(&list, &format).map_err(SkillError::InvalidArgs)?;

    let summary = summarize(&list, &filename, &opts);
    let for_llm = if list.is_empty() {
        summary
    } else {
        format!("{summary}\n{}", clip_for_llm(&text))
    };

    let out_filename = replace_extension(&filename, ext);
    let data_url = format!("data:{mime};base64,{}", B64.encode(text.as_bytes()));

    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: mime.to_string(),
            filename: out_filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety / no LLM-facing chat-schema drift: the descriptor-derived
    /// schema must match this authored one. `Input::Document` supplies the
    /// centralized `url`/`ref` wording + the `oneOf`; the tool params carry their
    /// `.describe()` text, enum variants, and defaults.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["json", "csv", "markdown", "text"], "default": "json", "description": "Output format: \"json\" (array of {page, type, subtype, author, date, color, comment, marked_text} objects), \"csv\" (one row per annotation with a header line), \"markdown\" (bullets grouped under a `## Page N` heading), or \"text\" (one flat line per annotation). Default \"json\"." },
                    "types": { "type": "string", "enum": ["all", "markup", "highlight", "underline", "strikeout", "squiggly", "note", "freetext", "drawing", "stamp", "link"], "default": "all", "description": "Which annotations to keep: \"all\" (default, every kind), \"markup\" (highlight + underline + strikeout + squiggly together), or a single kind — \"highlight\", \"underline\", \"strikeout\", \"squiggly\", \"note\" (sticky note), \"freetext\" (callout box), \"drawing\" (ink/square/circle/line/polygon), \"stamp\", or \"link\". Form fields and popup windows are always skipped." },
                    "pages": { "type": "string", "default": "", "description": "1-based page spec limiting which pages are scanned, e.g. \"3\" or \"1,4-6\". Leave empty (the default) for the whole document." },
                    "author": { "type": "string", "default": "", "description": "Keep only annotations whose author contains this text, case-insensitively (e.g. \"ada\" matches \"Ada Lovelace\"). Empty (the default) keeps every author." },
                    "include_marked_text": { "type": "boolean", "default": true, "description": "Recover the page text that each highlight/underline/strikeout/squiggly covers, by mapping the annotation's QuadPoints onto the PDF's text layer. Set false to skip that work and return only the typed comments. Default true." },
                    "include_empty": { "type": "boolean", "default": false, "description": "Include annotations that carry neither a comment nor marked-up text — bare links, empty stamps, and unlabelled drawings. Text markup (highlights etc.) is always kept regardless. Default false." },
                    "sort": { "type": "string", "enum": ["page", "author", "type", "date"], "default": "page", "description": "Result order: \"page\" (default, document order), \"author\", \"type\", or \"date\". Ties always fall back to page order." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_url_with_options() {
        use gizza_ai_block_utils::Source;
        let a: Args = serde_json::from_str(
            r#"{"url":"https://x/y.pdf","format":"markdown","types":"highlight","pages":"1,3-4","author":"ada","include_marked_text":false,"include_empty":true,"sort":"author"}"#,
        )
        .unwrap();
        assert!(matches!(a.source.into_inner(), Source::Url(ref u) if u == "https://x/y.pdf"));
        assert_eq!(a.format.as_deref(), Some("markdown"));
        assert_eq!(a.types.as_deref(), Some("highlight"));
        assert_eq!(a.pages.as_deref(), Some("1,3-4"));
        assert_eq!(a.author.as_deref(), Some("ada"));
        assert_eq!(a.include_marked_text, Some(false));
        assert_eq!(a.include_empty, Some(true));
        assert_eq!(a.sort.as_deref(), Some("author"));
    }

    #[test]
    fn args_parse_ref_defaults_to_the_documented_option_set() {
        use gizza_ai_block_utils::Source;
        let a: Args = serde_json::from_str(r#"{"ref":"call_9"}"#).unwrap();
        let opts = options_from(&a);
        assert!(matches!(a.source.into_inner(), Source::Ref(ref r) if r == "call_9"));
        assert_eq!(opts.types, "all");
        assert_eq!(opts.sort, "page");
        assert!(opts.include_marked_text);
        assert!(!opts.include_empty);
        assert!(opts.pages.is_empty());
        assert!(opts.author.is_empty());
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }

    #[test]
    fn format_target_maps_every_advertised_format() {
        assert_eq!(format_target("json").unwrap(), ("application/json", "json"));
        assert_eq!(format_target("csv").unwrap(), ("text/csv", "csv"));
        assert_eq!(format_target("markdown").unwrap(), ("text/markdown", "md"));
        assert_eq!(format_target("text").unwrap(), ("text/plain", "txt"));
    }

    #[test]
    fn format_target_rejects_unknown_format() {
        let err = format_target("xml").unwrap_err();
        assert!(err.to_string().contains("unknown format"), "{err}");
        assert!(err.to_string().contains("markdown"), "error lists valid values: {err}");
    }

    #[test]
    fn summary_explains_an_empty_result_with_the_active_filters() {
        let opts = Options {
            types: "highlight".to_string(),
            author: "ada".to_string(),
            pages: "2-3".to_string(),
            ..Options::default()
        };
        let s = summarize(&[], "paper.pdf", &opts);
        assert!(s.contains("No annotations"), "{s}");
        assert!(s.contains("types \"highlight\""), "{s}");
        assert!(s.contains("containing \"ada\""), "{s}");
        assert!(s.contains("pages 2-3"), "{s}");
    }

    #[test]
    fn summary_counts_annotations_and_pages() {
        let list = vec![
            Annotation {
                page: 1,
                kind: "highlight".into(),
                subtype: "Highlight".into(),
                author: "Ada".into(),
                date: String::new(),
                color: String::new(),
                comment: String::new(),
                marked_text: "brown fox".into(),
            },
            Annotation {
                page: 3,
                kind: "note".into(),
                subtype: "Text".into(),
                author: "Grace".into(),
                date: String::new(),
                color: String::new(),
                comment: "check".into(),
                marked_text: String::new(),
            },
        ];
        let s = summarize(&list, "paper.pdf", &Options::default());
        assert!(s.contains("Found 2 annotation(s) across 2 page(s) in paper.pdf"), "{s}");
    }

    #[test]
    fn clip_for_llm_truncates_with_a_note() {
        let long = "x".repeat(MAX_LLM_CHARS + 10);
        let clipped = clip_for_llm(&long);
        assert!(clipped.contains("truncated to"), "{}", &clipped[clipped.len() - 80..]);
        assert!(clip_for_llm("short").contains("short"));
    }
}
