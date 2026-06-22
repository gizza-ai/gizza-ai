//! gizza-ai/citation-generator — format a reference into APA, MLA, or Chicago
//! style from structured bibliographic fields.
//!
//! Thin chat-skill wrapper around `gizza-ai-citation-generator-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_citation_generator_core::Fields;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    style: String,
    #[serde(default)]
    authors: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    year: String,
    #[serde(default)]
    container: String,
    #[serde(default)]
    publisher: String,
    #[serde(default)]
    volume: String,
    #[serde(default)]
    issue: String,
    #[serde(default)]
    pages: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    doi: String,
    #[serde(default)]
    accessed: String,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("style", ["apa", "mla", "chicago", "harvard"])
                .default("apa")
                .describe("Citation style: 'apa' (APA 7th, default), 'mla' (MLA 9th works-cited), 'chicago' (Chicago 17th notes-bibliography entry), or 'harvard' (Harvard author-date reference)."),
        )
        .param(
            Param::string("title")
                .describe("Title of the work (article, chapter, page, or book). Required unless 'authors' is given."),
        )
        .param(
            Param::string("authors")
                .describe("Author(s). Separate multiple authors with ';' (recommended) or ' and '. Each author may be written 'Family, Given' or 'Given Family'. A single token with no space (e.g. 'UNESCO') is treated as an organization."),
        )
        .param(
            Param::string("year")
                .describe("Publication year (e.g. '2021'). Omit for an undated source; APA then prints '(n.d.)'."),
        )
        .param(
            Param::string("container")
                .describe("The larger work the source appears in: journal name, website name, or book/anthology title. Omitting it formats the source as a standalone work (e.g. a whole book)."),
        )
        .param(
            Param::string("publisher")
                .describe("Publisher name (for books / standalone works without a container)."),
        )
        .param(
            Param::string("volume")
                .describe("Volume number of the journal/series."),
        )
        .param(
            Param::string("issue")
                .describe("Issue (number) within the volume."),
        )
        .param(
            Param::string("pages")
                .describe("Page range, e.g. '45-67' or '12'."),
        )
        .param(
            Param::string("url")
                .describe("URL of the online source (used when no DOI is given)."),
        )
        .param(
            Param::string("doi")
                .describe("DOI of the source, with or without the leading 'https://doi.org/'. Preferred over 'url' when present."),
        )
        .param(
            Param::string("accessed")
                .describe("Date the online source was accessed, free-form (e.g. '12 Mar. 2024'). Used by MLA for web pages."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CitationGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/citation-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Citation generator skill",
    skill(
        description = "Format a single reference into APA 7th, MLA 9th, Chicago 17th, or Harvard style from structured fields. Set style='apa' (default), 'mla', 'chicago', or 'harvard'. Provide whatever fields you have: 'title', 'authors' (separate several with ';' or ' and '; write each 'Family, Given' or 'Given Family'; a single token like 'UNESCO' is treated as an organization), 'year', 'container' (journal/website/book name), 'publisher', 'volume', 'issue', 'pages', 'url', 'doi', and 'accessed'. Blank fields are omitted. With a 'container' the title is formatted as a part of a larger work (quoted in MLA/Chicago); without one it is treated as a standalone work. A DOI is preferred over a URL. Returns one ready-to-paste reference-list / works-cited / bibliography entry as plain text (italics that the styles call for cannot be rendered in plain text). At least a title or an author is required.",
        parameters = schema_json()
    ),
)]
impl CitationGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "citation-generator", |a: Args| {
            let fields = Fields {
                authors: a.authors,
                title: a.title,
                year: a.year,
                container: a.container,
                publisher: a.publisher,
                volume: a.volume,
                issue: a.issue,
                pages: a.pages,
                url: a.url,
                doi: a.doi,
                accessed: a.accessed,
            };
            gizza_ai_citation_generator_core::cite(&a.style, &fields)
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
                    "style": { "type": "string", "enum": ["apa", "mla", "chicago", "harvard"], "default": "apa", "description": "Citation style: 'apa' (APA 7th, default), 'mla' (MLA 9th works-cited), 'chicago' (Chicago 17th notes-bibliography entry), or 'harvard' (Harvard author-date reference)." },
                    "title": { "type": "string", "description": "Title of the work (article, chapter, page, or book). Required unless 'authors' is given." },
                    "authors": { "type": "string", "description": "Author(s). Separate multiple authors with ';' (recommended) or ' and '. Each author may be written 'Family, Given' or 'Given Family'. A single token with no space (e.g. 'UNESCO') is treated as an organization." },
                    "year": { "type": "string", "description": "Publication year (e.g. '2021'). Omit for an undated source; APA then prints '(n.d.)'." },
                    "container": { "type": "string", "description": "The larger work the source appears in: journal name, website name, or book/anthology title. Omitting it formats the source as a standalone work (e.g. a whole book)." },
                    "publisher": { "type": "string", "description": "Publisher name (for books / standalone works without a container)." },
                    "volume": { "type": "string", "description": "Volume number of the journal/series." },
                    "issue": { "type": "string", "description": "Issue (number) within the volume." },
                    "pages": { "type": "string", "description": "Page range, e.g. '45-67' or '12'." },
                    "url": { "type": "string", "description": "URL of the online source (used when no DOI is given)." },
                    "doi": { "type": "string", "description": "DOI of the source, with or without the leading 'https://doi.org/'. Preferred over 'url' when present." },
                    "accessed": { "type": "string", "description": "Date the online source was accessed, free-form (e.g. '12 Mar. 2024'). Used by MLA for web pages." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
