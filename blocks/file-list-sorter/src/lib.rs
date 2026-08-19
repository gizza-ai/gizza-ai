//! gizza-ai/file-list-sorter — chat skill block. Sorts a pasted list of file
//! names or paths by path-aware keys (natural, extension, depth, size, …).
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    paths: String,
    #[serde(default = "default_sort_by")]
    sort_by: String,
    #[serde(default = "default_order")]
    order: String,
    #[serde(default = "default_true")]
    ignore_case: bool,
    #[serde(default = "default_true")]
    dirs_first: bool,
    #[serde(default)]
    group_by_dir: bool,
    #[serde(default)]
    unique: bool,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default = "default_format")]
    format: String,
}

fn default_sort_by() -> String {
    "natural".to_string()
}
fn default_order() -> String {
    "asc".to_string()
}
fn default_format() -> String {
    "list".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("paths")
                .required()
                .describe("The file names or paths to sort, one per line. Anything a listing produces works: bare names (`img10.png`), relative paths (`src/app.js`), Windows paths (`docs\\report.docx`), or `ls`/`find`/`git ls-files`/`du` output. Both `/` and `\\` count as folder separators, a leading `./` is ignored, and blank lines are dropped. Entries are returned in the same spelling you pasted. Up to 20000 paths per run."),
        )
        .param(
            Param::enumv(
                "sort_by",
                ["natural", "alpha", "basename", "extension", "depth", "size"],
            )
            .default("natural")
            .describe("Which key to sort on. \"natural\" (default) is human order over the whole path — digit runs compare as numbers, so img2.png comes before img10.png. \"alpha\" is the classic codepoint order a machine sort gives (img10.png before img2.png). \"basename\" sorts naturally by file name only, ignoring the folders above it. \"extension\" groups by file type (extension-less entries first), then naturally by path. \"depth\" sorts shallowest first by how many folders are above the entry. \"size\" reads a size column off each line (see the `paths` note) and sorts by bytes, with entries that carry no size always last."),
        )
        .param(
            Param::enumv("order", ["asc", "desc"])
                .default("asc")
                .describe("Direction of the sort key: \"asc\" (default) is A→Z, shallowest, smallest first; \"desc\" reverses it. The folders-first rule is not reversed — folders stay on top in both directions, the way a file manager behaves."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(true)
                .describe("Compare case-insensitively, so README.md sits next to readme.txt instead of ahead of every lowercase name. On by default because that is what Explorer, Finder and most file managers do. Turn it off for the case-sensitive order of `sort`, `ls` or a Git tree, where all uppercase names come first."),
        )
        .param(
            Param::boolean("dirs_first")
                .default(true)
                .describe("Put folders above files. On by default. An entry counts as a folder when it ends in a slash (`src/`) or when another pasted entry lives underneath it (`src` is a folder if `src/main.rs` is also in the list). Turn it off to sort every entry purely by the chosen key."),
        )
        .param(
            Param::boolean("group_by_dir")
                .default(false)
                .describe("Keep each folder's contents together: sort by parent folder first, then apply the chosen key inside each folder. Off by default. Useful when sorting a deep `find` dump by file name or size but you still want one folder's files listed side by side."),
        )
        .param(
            Param::boolean("unique")
                .default(false)
                .describe("Drop duplicate paths, keeping the first spelling of each. Off by default. Comparison follows ignore_case and the normalised path, so `./src/app.js`, `src/app.js` and `SRC/APP.JS` count as one entry when ignore_case is on."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("Strip leading and trailing whitespace from every line before sorting. On by default, because indented `tree`/`ls` output otherwise sorts by its indentation. Turn it off when a file name genuinely begins or ends with a space."),
        )
        .param(
            Param::enumv("format", ["list", "numbered", "table", "json"])
                .default("list")
                .describe("Output shape. \"list\" (default) is the sorted paths, one per line, ready to paste back into a script. \"numbered\" prefixes each line with its rank. \"table\" adds a summary line plus aligned path/type/extension/depth/size columns for checking the sort. \"json\" returns {count, folders, sort_by, order, entries[]} with path, name, dir, extension, depth, is_dir, size_bytes and size_text per entry."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct FileListSorter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/file-list-sorter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sort a list of file names or paths by natural order, extension, depth or size",
    skill(
        description = "Sort a pasted list of file names or paths the way a file manager would, instead of the way a plain text sort does. Paste one path per line — bare names, relative paths, Windows paths, or the output of ls, find, git ls-files or du. sort_by picks the key: natural (default) compares digit runs as numbers so img2.png precedes img10.png; alpha is the classic codepoint order; basename ignores the folders above the file; extension groups by file type; depth sorts shallowest first; size reads a size column off each line. A size column is recognised when it carries a unit (`4.0K  src/app.js`, `src/app.js  1.2MB`, K/M/G/T are 1024-based) or when a bare byte count is TAB-separated from the path, so a name like `2024 report.txt` keeps its year; entries with no size sort last. order flips the key direction, ignore_case (on) matches file-manager casing, dirs_first (on) keeps folders above files — an entry is a folder if it ends in a slash or another entry sits underneath it — group_by_dir keeps each folder's contents together, unique drops repeated paths and trim strips indentation from pasted listings. format=list returns the sorted paths one per line, numbered adds ranks, table adds path/type/extension/depth/size columns, json returns structured entries. Both / and \\ are treated as separators. Up to 20000 paths per run. Pure text in, sorted text out — nothing is read from disk.",
        parameters = schema_json()
    ),
)]
impl FileListSorter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "file-list-sorter", |a: Args| {
            gizza_ai_file_list_sorter_core::run(
                &a.paths,
                &a.sort_by,
                &a.order,
                a.ignore_case,
                a.dirs_first,
                a.group_by_dir,
                a.unique,
                a.trim,
                &a.format,
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
                    "paths": { "type": "string", "description": "The file names or paths to sort, one per line. Anything a listing produces works: bare names (`img10.png`), relative paths (`src/app.js`), Windows paths (`docs\\report.docx`), or `ls`/`find`/`git ls-files`/`du` output. Both `/` and `\\` count as folder separators, a leading `./` is ignored, and blank lines are dropped. Entries are returned in the same spelling you pasted. Up to 20000 paths per run." },
                    "sort_by": { "type": "string", "enum": ["natural", "alpha", "basename", "extension", "depth", "size"], "default": "natural", "description": "Which key to sort on. \"natural\" (default) is human order over the whole path — digit runs compare as numbers, so img2.png comes before img10.png. \"alpha\" is the classic codepoint order a machine sort gives (img10.png before img2.png). \"basename\" sorts naturally by file name only, ignoring the folders above it. \"extension\" groups by file type (extension-less entries first), then naturally by path. \"depth\" sorts shallowest first by how many folders are above the entry. \"size\" reads a size column off each line (see the `paths` note) and sorts by bytes, with entries that carry no size always last." },
                    "order": { "type": "string", "enum": ["asc", "desc"], "default": "asc", "description": "Direction of the sort key: \"asc\" (default) is A→Z, shallowest, smallest first; \"desc\" reverses it. The folders-first rule is not reversed — folders stay on top in both directions, the way a file manager behaves." },
                    "ignore_case": { "type": "boolean", "default": true, "description": "Compare case-insensitively, so README.md sits next to readme.txt instead of ahead of every lowercase name. On by default because that is what Explorer, Finder and most file managers do. Turn it off for the case-sensitive order of `sort`, `ls` or a Git tree, where all uppercase names come first." },
                    "dirs_first": { "type": "boolean", "default": true, "description": "Put folders above files. On by default. An entry counts as a folder when it ends in a slash (`src/`) or when another pasted entry lives underneath it (`src` is a folder if `src/main.rs` is also in the list). Turn it off to sort every entry purely by the chosen key." },
                    "group_by_dir": { "type": "boolean", "default": false, "description": "Keep each folder's contents together: sort by parent folder first, then apply the chosen key inside each folder. Off by default. Useful when sorting a deep `find` dump by file name or size but you still want one folder's files listed side by side." },
                    "unique": { "type": "boolean", "default": false, "description": "Drop duplicate paths, keeping the first spelling of each. Off by default. Comparison follows ignore_case and the normalised path, so `./src/app.js`, `src/app.js` and `SRC/APP.JS` count as one entry when ignore_case is on." },
                    "trim": { "type": "boolean", "default": true, "description": "Strip leading and trailing whitespace from every line before sorting. On by default, because indented `tree`/`ls` output otherwise sorts by its indentation. Turn it off when a file name genuinely begins or ends with a space." },
                    "format": { "type": "string", "enum": ["list", "numbered", "table", "json"], "default": "list", "description": "Output shape. \"list\" (default) is the sorted paths, one per line, ready to paste back into a script. \"numbered\" prefixes each line with its rank. \"table\" adds a summary line plus aligned path/type/extension/depth/size columns for checking the sort. \"json\" returns {count, folders, sort_by, order, entries[]} with path, name, dir, extension, depth, is_dir, size_bytes and size_text per entry." }
                },
                "required": ["paths"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
