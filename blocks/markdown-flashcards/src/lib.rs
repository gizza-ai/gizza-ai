//! gizza-ai/markdown-flashcards — turn Q/A-shaped Markdown notes into an Anki-importable
//! deck file (TSV/CSV with `#` header directives), a preview, or JSON. Thin wrapper; the
//! chat schema is single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_flashcards_core::{generate, FieldFormat, FieldSep, Mode, Options, OutputKind};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    heading_level: f64,
    #[serde(default)]
    field_separator: String,
    #[serde(default)]
    field_format: String,
    #[serde(default)]
    notetype: String,
    #[serde(default)]
    deck: String,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    tags_from_headings: bool,
    #[serde(default = "default_true")]
    include_headers: bool,
    #[serde(default = "default_true")]
    dedupe: bool,
    #[serde(default)]
    output: String,
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown notes to turn into flashcards, e.g. `## What is mitosis?` with the answer underneath, `Q:`/`A:` lines, a `| question | answer |` table, or `term :: definition` lines. Up to 1,000,000 characters and 5,000 cards."),
        )
        .param(
            Param::enumv("mode", ["auto", "heading", "separator", "qa", "table", "cloze"])
                .default("auto")
                .describe("How the notes are cut into cards: 'auto' (default) detects the shape; 'heading' uses each heading as the question and the text under it as the answer; 'separator' splits one card per line on a delimiter; 'qa' reads `Q:`/`A:` blocks; 'table' reads Markdown table rows (3rd column = tags); 'cloze' turns each `**bold**` or `==highlighted==` span into a `{{c1::…}}` deletion."),
        )
        .param(
            Param::string("separator")
                .default("auto")
                .describe("Separator mode only: what splits the question from the answer on a line. 'auto' (default) picks the most-used of `::`, `=>`, tab, `|`, `;`, ` - `, `:`; or pass a name ('tab', 'colon', 'double-colon', 'semicolon', 'pipe', 'comma', 'dash', 'arrow') or any literal string."),
        )
        .param(
            Param::integer("heading_level")
                .default(0)
                .min(0.0)
                .max(6.0)
                .describe("Heading mode only: which heading level becomes the question. 0 (default) auto-picks the level with the most answers; 1-6 pins `#`…`######`."),
        )
        .param(
            Param::enumv("field_separator", ["tab", "comma", "semicolon", "pipe"])
                .default("tab")
                .describe("Field separator in the exported file: 'tab' (default, what Anki recommends), 'comma' (CSV for Quizlet-style importers), 'semicolon' or 'pipe'. Fields containing the separator, a quote or a newline are quoted RFC-4180 style."),
        )
        .param(
            Param::enumv("field_format", ["html", "markdown", "plain"])
                .default("html")
                .describe("How each field is rendered: 'html' (default) converts bold/italic/code/links/lists/code blocks to HTML and newlines to `<br>` (import with 'Allow HTML in fields'); 'markdown' keeps the raw Markdown; 'plain' strips all markup."),
        )
        .param(
            Param::enumv(
                "notetype",
                [
                    "Basic",
                    "Basic (and reversed card)",
                    "Basic (type in the answer)",
                    "Cloze",
                ],
            )
            .default("Basic")
            .describe("Anki note type written to the `#notetype:` header. Default 'Basic'; cloze mode upgrades 'Basic' to 'Cloze' automatically."),
        )
        .param(
            Param::string("deck")
                .default("")
                .describe("Optional deck name for the `#deck:` header, e.g. 'Biology::Cells' (`::` makes a subdeck). Empty (default) imports into whatever deck is selected in Anki."),
        )
        .param(
            Param::string("tags")
                .default("")
                .describe("Optional tags applied to every card, space- or comma-separated, e.g. 'exam week1'. Written to the `#tags:` header, or folded into each row's Tags column when include_headers is false."),
        )
        .param(
            Param::boolean("tags_from_headings")
                .default(false)
                .describe("Heading mode only: tag each card with its parent-heading path as one hierarchical Anki tag, e.g. `Biology::Cell_Parts`. Default false."),
        )
        .param(
            Param::boolean("include_headers")
                .default(true)
                .describe("Write Anki's `#separator:`/`#html:`/`#notetype:`/`#deck:`/`#tags:`/`#columns:` header lines at the top of the file (default true). Set false for a bare CSV/TSV for other flashcard apps."),
        )
        .param(
            Param::boolean("dedupe")
                .default(true)
                .describe("Drop later cards whose question repeats an earlier one, case-insensitively (default true). Set false to keep every card."),
        )
        .param(
            Param::enumv("output", ["anki", "preview", "json"])
                .default("anki")
                .describe("What to return: 'anki' (default) the importable text file; 'preview' a numbered human-readable card list with the detected mode; 'json' the parsed cards as `{mode, notetype, count, cards[]}`."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn to_options(a: &Args) -> Result<Options, String> {
    if a.heading_level.fract() != 0.0 {
        return Err(format!(
            "heading_level must be a whole number 0-6 (0 = auto-detect), got {}",
            a.heading_level
        ));
    }
    if !(0.0..=6.0).contains(&a.heading_level) {
        return Err(format!(
            "heading_level must be 0-6 (0 = auto-detect), got {}",
            a.heading_level
        ));
    }
    Ok(Options {
        mode: Mode::parse(&a.mode)?,
        separator: if a.separator.is_empty() {
            "auto".to_string()
        } else {
            a.separator.clone()
        },
        heading_level: a.heading_level as u8,
        field_separator: FieldSep::parse(&a.field_separator)?,
        field_format: FieldFormat::parse(&a.field_format)?,
        notetype: a.notetype.clone(),
        deck: a.deck.clone(),
        tags: a.tags.clone(),
        tags_from_headings: a.tags_from_headings,
        include_headers: a.include_headers,
        dedupe: a.dedupe,
        output: OutputKind::parse(&a.output)?,
    })
}

#[cfg(target_arch = "wasm32")]
struct MarkdownFlashcards;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-flashcards",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn Markdown notes into an Anki-importable flashcard deck",
    skill(
        description = "Parse Q/A-shaped Markdown notes into a flashcard deck and export an Anki-importable text file. mode=auto (default) detects the shape: heading (`## Question` + the text under it), qa (`Q:` / `A:` blocks), table (`| question | answer |` rows, optional 3rd tags column) or separator (one card per line split on `::`, `=>`, tab, `|`, `;`, ` - ` or `:`); mode=cloze turns every `**bold**`/`==highlighted==` span into a `{{c1::…}}` deletion. separator pins the line delimiter, heading_level (0=auto, 1-6) pins which heading is the question. field_separator=tab (default)|comma|semicolon|pipe sets the export delimiter with RFC-4180 quoting; field_format=html (default) converts bold/italic/code/links/lists/code blocks to HTML and newlines to <br>, or markdown/plain. deck, tags, notetype and include_headers control Anki's `#deck:`/`#tags:`/`#notetype:`/`#columns:` header directives; tags_from_headings tags each card with its heading path; dedupe drops repeated questions. output=anki (default)|preview|json. Limits: 1,000,000 characters, 5,000 cards. Runs locally.",
        parameters = schema_json()
    ),
)]
impl MarkdownFlashcards {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-flashcards", |a: Args| {
            let opts = to_options(&a).map_err(SkillError::InvalidArgs)?;
            generate(&a.markdown, &opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "markdown": { "type": "string", "description": "The Markdown notes to turn into flashcards, e.g. `## What is mitosis?` with the answer underneath, `Q:`/`A:` lines, a `| question | answer |` table, or `term :: definition` lines. Up to 1,000,000 characters and 5,000 cards." },
                    "mode": { "type": "string", "enum": ["auto", "heading", "separator", "qa", "table", "cloze"], "default": "auto", "description": "How the notes are cut into cards: 'auto' (default) detects the shape; 'heading' uses each heading as the question and the text under it as the answer; 'separator' splits one card per line on a delimiter; 'qa' reads `Q:`/`A:` blocks; 'table' reads Markdown table rows (3rd column = tags); 'cloze' turns each `**bold**` or `==highlighted==` span into a `{{c1::…}}` deletion." },
                    "separator": { "type": "string", "default": "auto", "description": "Separator mode only: what splits the question from the answer on a line. 'auto' (default) picks the most-used of `::`, `=>`, tab, `|`, `;`, ` - `, `:`; or pass a name ('tab', 'colon', 'double-colon', 'semicolon', 'pipe', 'comma', 'dash', 'arrow') or any literal string." },
                    "heading_level": { "type": "integer", "default": 0, "minimum": 0, "maximum": 6, "description": "Heading mode only: which heading level becomes the question. 0 (default) auto-picks the level with the most answers; 1-6 pins `#`…`######`." },
                    "field_separator": { "type": "string", "enum": ["tab", "comma", "semicolon", "pipe"], "default": "tab", "description": "Field separator in the exported file: 'tab' (default, what Anki recommends), 'comma' (CSV for Quizlet-style importers), 'semicolon' or 'pipe'. Fields containing the separator, a quote or a newline are quoted RFC-4180 style." },
                    "field_format": { "type": "string", "enum": ["html", "markdown", "plain"], "default": "html", "description": "How each field is rendered: 'html' (default) converts bold/italic/code/links/lists/code blocks to HTML and newlines to `<br>` (import with 'Allow HTML in fields'); 'markdown' keeps the raw Markdown; 'plain' strips all markup." },
                    "notetype": { "type": "string", "enum": ["Basic", "Basic (and reversed card)", "Basic (type in the answer)", "Cloze"], "default": "Basic", "description": "Anki note type written to the `#notetype:` header. Default 'Basic'; cloze mode upgrades 'Basic' to 'Cloze' automatically." },
                    "deck": { "type": "string", "default": "", "description": "Optional deck name for the `#deck:` header, e.g. 'Biology::Cells' (`::` makes a subdeck). Empty (default) imports into whatever deck is selected in Anki." },
                    "tags": { "type": "string", "default": "", "description": "Optional tags applied to every card, space- or comma-separated, e.g. 'exam week1'. Written to the `#tags:` header, or folded into each row's Tags column when include_headers is false." },
                    "tags_from_headings": { "type": "boolean", "default": false, "description": "Heading mode only: tag each card with its parent-heading path as one hierarchical Anki tag, e.g. `Biology::Cell_Parts`. Default false." },
                    "include_headers": { "type": "boolean", "default": true, "description": "Write Anki's `#separator:`/`#html:`/`#notetype:`/`#deck:`/`#tags:`/`#columns:` header lines at the top of the file (default true). Set false for a bare CSV/TSV for other flashcard apps." },
                    "dedupe": { "type": "boolean", "default": true, "description": "Drop later cards whose question repeats an earlier one, case-insensitively (default true). Set false to keep every card." },
                    "output": { "type": "string", "enum": ["anki", "preview", "json"], "default": "anki", "description": "What to return: 'anki' (default) the importable text file; 'preview' a numbered human-readable card list with the detected mode; 'json' the parsed cards as `{mode, notetype, count, cards[]}`." }
                },
                "required": ["markdown"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn heading_level_must_be_a_whole_number_in_range() {
        let mut a = Args {
            markdown: "## a\nb".into(),
            mode: String::new(),
            separator: String::new(),
            heading_level: 2.5,
            field_separator: String::new(),
            field_format: String::new(),
            notetype: String::new(),
            deck: String::new(),
            tags: String::new(),
            tags_from_headings: false,
            include_headers: true,
            dedupe: true,
            output: String::new(),
        };
        assert!(to_options(&a).unwrap_err().contains("whole number"));
        a.heading_level = 9.0;
        assert!(to_options(&a).unwrap_err().contains("must be 0-6"));
        a.heading_level = 2.0;
        let opts = to_options(&a).unwrap();
        assert_eq!(opts.heading_level, 2);
        assert_eq!(
            generate("## a\nb", &opts).unwrap(),
            "#separator:Tab\n#html:true\n#notetype:Basic\n#columns:Front\tBack\na\tb"
        );
    }
}
