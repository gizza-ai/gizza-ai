//! gizza-ai/markdown-notes-index — chat skill block. Turns a pasted bundle of
//! Markdown notes into a linked index: titles, tags, heading outlines and counts.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    notes: String,
    #[serde(default = "default_split")]
    split: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_heading_depth")]
    heading_depth: f64,
    #[serde(default = "default_group_by")]
    group_by: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default = "default_link_style")]
    link_style: String,
    #[serde(default = "default_true")]
    include_toc: bool,
    #[serde(default = "default_true")]
    include_stats: bool,
    #[serde(default = "default_true")]
    inline_tags: bool,
}

fn default_split() -> String {
    "heading".to_string()
}
fn default_format() -> String {
    "markdown".to_string()
}
fn default_heading_depth() -> f64 {
    2.0
}
fn default_group_by() -> String {
    "none".to_string()
}
fn default_sort() -> String {
    "input".to_string()
}
fn default_link_style() -> String {
    "anchor".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("notes")
                .required()
                .describe("The Markdown notes to index, pasted one after another in a single document. Each note may start with a `---` YAML front-matter block (only `title`, `tags`/`tag` and `keywords` are read) and is otherwise plain Markdown. Headings must be ATX style (`#` … `######`); setext underlines are not treated as headings, and headings inside ``` or ~~~ code fences are ignored. Up to 500 notes per run. Note bodies are never reproduced in the output — only the index metadata."),
        )
        .param(
            Param::enumv("split", ["heading", "hr", "file-marker"])
                .default("heading")
                .describe("Where one note ends and the next begins. \"heading\" (default) starts a new note at every top-level `# ` heading. \"hr\" splits on a thematic break (`---`, `***` or `___` on its own line); a `---` that opens a note's front matter is not treated as a break. \"file-marker\" splits on file banner lines — `=== notes/todo.md ===` or `==> notes/todo.md <==`, the form `head`/`tail` print — and remembers each path, so titles and links point at the real files. A file-marker run with no markers in the input is an error, not one giant note."),
        )
        .param(
            Param::enumv("format", ["markdown", "json", "csv"])
                .default("markdown")
                .describe("Output shape. \"markdown\" (default) is a ready-to-paste index note: a summary line, an optional linked table of contents, then one section per note with its tags, source file, counts and heading outline. \"json\" returns {notes, words, tags[], index[], groups?} with title, anchor, file, tags, headings and counts per note, for feeding another tool. \"csv\" returns one row per note with title, file, tags (semicolon-joined) and, with stats on, heading and word counts — for a spreadsheet."),
        )
        .param(
            Param::integer("heading_depth")
                .default(2)
                .min(0.0)
                .max(6.0)
                .describe("Deepest heading level kept in each note's outline, 1–6; 2 is the default, so `#` and `##` headings are listed and deeper ones are left out. Set 0 for no outline at all — just titles, tags and counts. The heading used as a note's title is never repeated in its own outline, and heading counts in the stats always cover every heading, not just the listed ones."),
        )
        .param(
            Param::enumv("group_by", ["none", "tag"])
                .default("none")
                .describe("How the table of contents is organised. \"none\" (default) is one numbered list in the chosen sort order. \"tag\" gives each tag its own subsection listing the notes carrying it — a note with three tags appears under all three — followed by an \"Untagged\" subsection when some notes have no tags. Tags are matched case-insensitively. In JSON output this adds a `groups` array instead."),
        )
        .param(
            Param::enumv("sort", ["input", "title", "words"])
                .default("input")
                .describe("Order of the notes in the index. \"input\" (default) keeps the order they were pasted in. \"title\" sorts A→Z case-insensitively by the resolved title. \"words\" puts the longest notes first, which is a quick way to spot stubs at the bottom of a vault. Notes that tie keep their input order."),
        )
        .param(
            Param::enumv("link_style", ["anchor", "wiki", "none"])
                .default("anchor")
                .describe("How index entries link to notes. \"anchor\" (default) links to the note's own section in this index using a GitHub-style slug, or to the source file when the note came from a `=== path ===` marker (outline entries then become `file.md#heading` links). \"wiki\" emits `[[Note title]]` and `[[Note title#Heading]]` for Obsidian-style vaults. \"none\" leaves plain text, for pasting somewhere that has no link targets."),
        )
        .param(
            Param::boolean("include_toc")
                .default(true)
                .describe("Put a linked \"Contents\" list at the top of the Markdown index. On by default. Turn it off when you only want the per-note sections — for example when the index is being appended under a table of contents that already exists. Ignored for CSV output."),
        )
        .param(
            Param::boolean("include_stats")
                .default(true)
                .describe("Include word and heading counts — a total on the summary line and per-note counts on each section. On by default. Words are whitespace-separated tokens containing at least one letter or digit, so bare Markdown punctuation (`-`, `>`, `|`) is not counted. Turn it off for a clean index with no numbers; in CSV this drops the headings and words columns."),
        )
        .param(
            Param::boolean("inline_tags")
                .default(true)
                .describe("Collect `#tag` mentions from the note body as tags, on top of any front-matter tags. On by default. A tag must start at a boundary (line start, whitespace, `(` or `[`) and contain at least one letter, so issue references like `#1234` are ignored and `#build/ci` is kept whole. Turn it off to use only the front-matter `tags` list."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownNotesIndex;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-notes-index",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a linked index of pasted Markdown notes with titles, tags, outlines and counts",
    skill(
        description = "Turn a pile of pasted Markdown notes into one linked index — the map-of-content note a vault or docs folder is missing. Paste the notes into `notes` one after another; `split` says where each note ends: heading (default, a new note at every `# ` heading), hr (a `---`/`***`/`___` thematic break, front matter excluded), or file-marker (`=== notes/todo.md ===` or `==> notes/todo.md <==` banners, which also give each note its source path). Each note's title is taken from its front-matter `title:`, else its first heading, else its file name, else `Untitled note N`. Tags come from front-matter `tags`/`tag`/`keywords` in inline, flow or block-list YAML, plus inline `#tags` in the body unless inline_tags is off. The Markdown output is a summary line, an optional linked table of contents (include_toc), and one section per note with tags, source file, counts (include_stats) and a heading outline down to heading_depth (0–6, default 2). group_by=tag lists the notes under each tag instead of one flat list; sort orders by input, title or word count; link_style picks anchor links, Obsidian `[[wiki]]` links or plain text. format=json returns the same index as structured data and format=csv as one row per note. ATX headings only, headings inside code fences are skipped, and up to 500 notes per run. Pure text in, index text out — nothing is read from disk and note bodies are never reproduced.",
        parameters = schema_json()
    ),
)]
impl MarkdownNotesIndex {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-notes-index", |a: Args| {
            gizza_ai_markdown_notes_index_core::run(
                &a.notes,
                &a.split,
                &a.format,
                a.heading_depth.round().max(0.0) as u32,
                &a.group_by,
                &a.sort,
                &a.link_style,
                a.include_toc,
                a.include_stats,
                a.inline_tags,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "notes": { "type": "string", "description": "The Markdown notes to index, pasted one after another in a single document. Each note may start with a `---` YAML front-matter block (only `title`, `tags`/`tag` and `keywords` are read) and is otherwise plain Markdown. Headings must be ATX style (`#` … `######`); setext underlines are not treated as headings, and headings inside ``` or ~~~ code fences are ignored. Up to 500 notes per run. Note bodies are never reproduced in the output — only the index metadata." },
                    "split": { "type": "string", "enum": ["heading", "hr", "file-marker"], "default": "heading", "description": "Where one note ends and the next begins. \"heading\" (default) starts a new note at every top-level `# ` heading. \"hr\" splits on a thematic break (`---`, `***` or `___` on its own line); a `---` that opens a note's front matter is not treated as a break. \"file-marker\" splits on file banner lines — `=== notes/todo.md ===` or `==> notes/todo.md <==`, the form `head`/`tail` print — and remembers each path, so titles and links point at the real files. A file-marker run with no markers in the input is an error, not one giant note." },
                    "format": { "type": "string", "enum": ["markdown", "json", "csv"], "default": "markdown", "description": "Output shape. \"markdown\" (default) is a ready-to-paste index note: a summary line, an optional linked table of contents, then one section per note with its tags, source file, counts and heading outline. \"json\" returns {notes, words, tags[], index[], groups?} with title, anchor, file, tags, headings and counts per note, for feeding another tool. \"csv\" returns one row per note with title, file, tags (semicolon-joined) and, with stats on, heading and word counts — for a spreadsheet." },
                    "heading_depth": { "type": "integer", "default": 2, "minimum": 0, "maximum": 6, "description": "Deepest heading level kept in each note's outline, 1–6; 2 is the default, so `#` and `##` headings are listed and deeper ones are left out. Set 0 for no outline at all — just titles, tags and counts. The heading used as a note's title is never repeated in its own outline, and heading counts in the stats always cover every heading, not just the listed ones." },
                    "group_by": { "type": "string", "enum": ["none", "tag"], "default": "none", "description": "How the table of contents is organised. \"none\" (default) is one numbered list in the chosen sort order. \"tag\" gives each tag its own subsection listing the notes carrying it — a note with three tags appears under all three — followed by an \"Untagged\" subsection when some notes have no tags. Tags are matched case-insensitively. In JSON output this adds a `groups` array instead." },
                    "sort": { "type": "string", "enum": ["input", "title", "words"], "default": "input", "description": "Order of the notes in the index. \"input\" (default) keeps the order they were pasted in. \"title\" sorts A→Z case-insensitively by the resolved title. \"words\" puts the longest notes first, which is a quick way to spot stubs at the bottom of a vault. Notes that tie keep their input order." },
                    "link_style": { "type": "string", "enum": ["anchor", "wiki", "none"], "default": "anchor", "description": "How index entries link to notes. \"anchor\" (default) links to the note's own section in this index using a GitHub-style slug, or to the source file when the note came from a `=== path ===` marker (outline entries then become `file.md#heading` links). \"wiki\" emits `[[Note title]]` and `[[Note title#Heading]]` for Obsidian-style vaults. \"none\" leaves plain text, for pasting somewhere that has no link targets." },
                    "include_toc": { "type": "boolean", "default": true, "description": "Put a linked \"Contents\" list at the top of the Markdown index. On by default. Turn it off when you only want the per-note sections — for example when the index is being appended under a table of contents that already exists. Ignored for CSV output." },
                    "include_stats": { "type": "boolean", "default": true, "description": "Include word and heading counts — a total on the summary line and per-note counts on each section. On by default. Words are whitespace-separated tokens containing at least one letter or digit, so bare Markdown punctuation (`-`, `>`, `|`) is not counted. Turn it off for a clean index with no numbers; in CSV this drops the headings and words columns." },
                    "inline_tags": { "type": "boolean", "default": true, "description": "Collect `#tag` mentions from the note body as tags, on top of any front-matter tags. On by default. A tag must start at a boundary (line start, whitespace, `(` or `[`) and contain at least one letter, so issue references like `#1234` are ignored and `#build/ci` is kept whole. Turn it off to use only the front-matter `tags` list." }
                },
                "required": ["notes"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
