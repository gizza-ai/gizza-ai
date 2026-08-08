//! gizza-ai/mv-script-generator — a before→after filename mapping → a reviewable
//! rename script (bash `mv` or PowerShell `Rename-Item`/`Move-Item`) + its undo.
//!
//! Thin chat-skill wrapper around `gizza-ai-mv-script-generator-core`. Chat schema
//! single-sourced from `descriptor()`; handler delegates to `run_skill`. Pure — no
//! file is ever touched, the output is script text. Pairs with the sibling
//! `bulk-file-renamer`, which *computes* the old → new mapping this tool consumes.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mv_script_generator_core::generate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    mapping: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_shell")]
    shell: String,
    #[serde(default)]
    base_dir: String,
    #[serde(default)]
    dry_run: bool,
    #[serde(default)]
    overwrite: bool,
    #[serde(default = "default_true")]
    mkdir_parents: bool,
    #[serde(default = "default_true")]
    undo_script: bool,
    #[serde(default = "default_true")]
    comments: bool,
}

fn default_format() -> String {
    "auto".into()
}
fn default_shell() -> String {
    "bash".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("mapping").required().describe(
                "The before→after filename mapping, one pair per line, e.g. 'IMG_001.JPG,photo-001.jpg'. Accepts comma, tab, pipe (' | ') or arrow (' -> ') between the old and the new name. Blank lines and lines starting with # are ignored, and a leading header row (old,new / from,to / source,destination) is skipped. Paths may be relative or absolute; quoting is handled for you. Maximum 1000 pairs.",
            ),
        )
        .param(
            Param::enumv("format", ["auto", "csv", "tsv", "arrow", "pipe"])
                .default("auto")
                .describe(
                    "How to split each mapping line. 'auto' (default) picks the one separator that works on every line. 'csv' = comma, honouring \"quoted,fields\" so a filename may contain commas; 'tsv' = tab; 'arrow' = ' -> '; 'pipe' = '|'. Set it explicitly when a filename contains the separator of another format.",
                ),
        )
        .param(
            Param::enumv("shell", ["bash", "powershell"])
                .default("bash")
                .describe(
                    "Script dialect to emit. 'bash' (default) writes a POSIX script with 'set -euo pipefail' and an 'mv_safe' helper. 'powershell' writes a PowerShell 5.1+ script with $ErrorActionPreference = 'Stop' and a 'Move-Safe' helper using Rename-Item -LiteralPath for same-directory renames and Move-Item otherwise.",
                ),
        )
        .param(
            Param::string("base_dir")
                .default("")
                .describe(
                    "Optional directory the script should run in. When set, the script starts with 'cd -- <dir>' (bash) or 'Set-Location -LiteralPath <dir>' (PowerShell) so the mapping can use plain filenames. Blank (default) means the script runs in the current directory.",
                ),
        )
        .param(
            Param::boolean("dry_run")
                .default(false)
                .describe(
                    "When true, the script reports each move instead of performing it — bash prints 'would move: old -> new', PowerShell passes -WhatIf. Default false (the script really renames). Use it to preview the plan on the real directory first.",
                ),
        )
        .param(
            Param::boolean("overwrite")
                .default(false)
                .describe(
                    "When true, an existing destination is replaced ('mv -f' / -Force). Default false, which makes the script abort before the first move if any destination already exists, so nothing is destroyed.",
                ),
        )
        .param(
            Param::boolean("mkdir_parents")
                .default(true)
                .describe(
                    "When true (default), the script creates missing parent directories of each destination ('mkdir -p' / New-Item -ItemType Directory). Turn it off to make a move into a non-existent folder fail loudly.",
                ),
        )
        .param(
            Param::boolean("undo_script")
                .default(true)
                .describe(
                    "When true (default), the output holds two scripts: the rename script, then an undo script that reverses every emitted move in reverse order (including the temp names used to break rename cycles). Set false to get only the rename script.",
                ),
        )
        .param(
            Param::boolean("comments")
                .default(true)
                .describe(
                    "When true (default), each script carries a header comment summarising the run: how many renames, how many unchanged pairs were skipped, whether moves were reordered, and how many temp names broke a cycle. Set false for a bare script.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn render(a: Args) -> Result<String, String> {
    generate(
        &a.mapping,
        &a.format,
        &a.shell,
        &a.base_dir,
        a.dry_run,
        a.overwrite,
        a.mkdir_parents,
        a.undo_script,
        a.comments,
    )
}

#[cfg(target_arch = "wasm32")]
struct MvScriptGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mv-script-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn an old,new filename mapping into a safe bash or PowerShell rename script plus its undo",
    skill(
        description = "Turn an already-decided before→after filename mapping into a reviewable rename script — bash (mv) or PowerShell (Rename-Item/Move-Item) — plus the matching undo script. Nothing is moved here: the output is script text you review and run yourself. `mapping` takes one 'old,new' pair per line (comma, tab, pipe or ' -> ' separated; `format` forces one, `auto` detects it), and the generator applies the safety passes a hand-written script usually misses: every path is single-quoted with dialect-correct escaping and passed after '--' / via -LiteralPath so spaces, quotes, $ and leading dashes are inert; duplicate sources and duplicate destinations are hard errors naming both line numbers; chained renames (a→b, b→c) are ordered so no move clobbers a file a later move still needs; and true swaps (a→b, b→a) are staged through a temp name that the undo script unwinds the same way. Options: shell (bash|powershell), base_dir to cd into first, dry_run for a report-only script (-WhatIf on PowerShell), overwrite to allow replacing an existing destination (off by default, so the script aborts instead of destroying a file), mkdir_parents to create missing destination folders, undo_script, and comments for the summary header. Maximum 1000 pairs. To derive the new names in the first place (find/replace, regex, numbering, case), use bulk-file-renamer and feed its mapping in here.",
        parameters = schema_json()
    )
)]
impl MvScriptGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mv-script-generator", |a: Args| {
            render(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(mapping: &str) -> Args {
        Args {
            mapping: mapping.into(),
            format: default_format(),
            shell: default_shell(),
            base_dir: String::new(),
            dry_run: false,
            overwrite: false,
            mkdir_parents: true,
            undo_script: true,
            comments: true,
        }
    }

    #[test]
    fn render_applies_defaults_and_emits_both_scripts() {
        let out = render(args("IMG_001.JPG,photo-001.jpg")).unwrap();
        assert!(out.starts_with("# ===== rename.sh ====="), "{out}");
        assert!(out.contains("mv_safe 'IMG_001.JPG' 'photo-001.jpg'"), "{out}");
        assert!(out.contains("# ===== undo-rename.sh ====="), "{out}");
        assert!(out.contains("mv_safe 'photo-001.jpg' 'IMG_001.JPG'"), "{out}");
        assert!(out.contains("mkdir -p -- "), "{out}");
    }

    #[test]
    fn render_honours_the_full_option_surface() {
        let mut a = args("a.txt | sub/b.txt");
        a.format = "pipe".into();
        a.shell = "powershell".into();
        a.base_dir = "C:\\photos".into();
        a.dry_run = true;
        a.overwrite = true;
        a.mkdir_parents = false;
        a.undo_script = false;
        a.comments = false;
        let out = render(a).unwrap();
        assert!(out.starts_with("#Requires -Version 5.1\n"), "{out}");
        assert!(out.contains("Set-Location -LiteralPath 'C:\\photos'"), "{out}");
        assert!(out.contains("-WhatIf"), "{out}");
        assert!(out.contains("-Force"), "{out}");
        assert!(!out.contains("New-Item -ItemType Directory"), "{out}");
        assert!(!out.contains("undo-rename"), "{out}");
        assert!(!out.contains("# Rename script"), "{out}");
    }

    #[test]
    fn render_rejects_bad_values_with_actionable_errors() {
        let mut a = args("a.txt,b.txt");
        a.shell = "fish".into();
        assert!(render(a).unwrap_err().contains("unknown shell"));

        let mut a = args("a.txt,b.txt");
        a.format = "yaml".into();
        assert!(render(a).unwrap_err().contains("unknown format"));

        assert!(render(args("   ")).unwrap_err().contains("mapping is empty"));
        assert!(render(args("a.txt,c.txt\nb.txt,c.txt"))
            .unwrap_err()
            .contains("would both become 'c.txt'"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "mapping": { "type": "string", "description": "The before→after filename mapping, one pair per line, e.g. 'IMG_001.JPG,photo-001.jpg'. Accepts comma, tab, pipe (' | ') or arrow (' -> ') between the old and the new name. Blank lines and lines starting with # are ignored, and a leading header row (old,new / from,to / source,destination) is skipped. Paths may be relative or absolute; quoting is handled for you. Maximum 1000 pairs." },
                    "format": { "type": "string", "enum": ["auto", "csv", "tsv", "arrow", "pipe"], "default": "auto", "description": "How to split each mapping line. 'auto' (default) picks the one separator that works on every line. 'csv' = comma, honouring \"quoted,fields\" so a filename may contain commas; 'tsv' = tab; 'arrow' = ' -> '; 'pipe' = '|'. Set it explicitly when a filename contains the separator of another format." },
                    "shell": { "type": "string", "enum": ["bash", "powershell"], "default": "bash", "description": "Script dialect to emit. 'bash' (default) writes a POSIX script with 'set -euo pipefail' and an 'mv_safe' helper. 'powershell' writes a PowerShell 5.1+ script with $ErrorActionPreference = 'Stop' and a 'Move-Safe' helper using Rename-Item -LiteralPath for same-directory renames and Move-Item otherwise." },
                    "base_dir": { "type": "string", "default": "", "description": "Optional directory the script should run in. When set, the script starts with 'cd -- <dir>' (bash) or 'Set-Location -LiteralPath <dir>' (PowerShell) so the mapping can use plain filenames. Blank (default) means the script runs in the current directory." },
                    "dry_run": { "type": "boolean", "default": false, "description": "When true, the script reports each move instead of performing it — bash prints 'would move: old -> new', PowerShell passes -WhatIf. Default false (the script really renames). Use it to preview the plan on the real directory first." },
                    "overwrite": { "type": "boolean", "default": false, "description": "When true, an existing destination is replaced ('mv -f' / -Force). Default false, which makes the script abort before the first move if any destination already exists, so nothing is destroyed." },
                    "mkdir_parents": { "type": "boolean", "default": true, "description": "When true (default), the script creates missing parent directories of each destination ('mkdir -p' / New-Item -ItemType Directory). Turn it off to make a move into a non-existent folder fail loudly." },
                    "undo_script": { "type": "boolean", "default": true, "description": "When true (default), the output holds two scripts: the rename script, then an undo script that reverses every emitted move in reverse order (including the temp names used to break rename cycles). Set false to get only the rename script." },
                    "comments": { "type": "boolean", "default": true, "description": "When true (default), each script carries a header comment summarising the run: how many renames, how many unchanged pairs were skipped, whether moves were reordered, and how many temp names broke a cycle. Set false for a bare script." }
                },
                "required": ["mapping"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
