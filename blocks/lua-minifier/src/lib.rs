//! gizza-ai/lua-minifier — chat skill block on the shared tool abstraction.
//! Shrinks Lua source by stripping comments and whitespace and, optionally,
//! renaming locals and parameters to short aliases.
//! Chat schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_true")]
    remove_comments: bool,
    #[serde(default = "default_true")]
    keep_license: bool,
    #[serde(default)]
    rename_locals: bool,
    #[serde(default = "default_line_breaks")]
    line_breaks: String,
}

fn default_true() -> bool {
    true
}
fn default_line_breaks() -> String {
    "strip".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The Lua source to minify, pasted exactly as written — a module, a script, a config chunk, any Lua 5.1 through 5.4 dialect including LuaJIT. Scanning is token-aware, so string and long-string interiors, long-bracket levels (`[=[ … ]=]`), numeric literal forms and a leading `#!` shebang line all survive untouched. No code is reordered, folded or dropped; the only thing removed is comments and the whitespace between tokens. Malformed input is minified as far as it can be read rather than rejected."),
        )
        .param(
            Param::boolean("remove_comments")
                .default(true)
                .describe("Strip `--` line comments and `--[[ … ]]` long comments (any bracket level). On by default — comments are usually the largest share of what a minifier removes. Turn it off to collapse only the whitespace, which is what you want when the comments are documentation you intend to ship. Text that merely looks like a comment inside a string is never touched. See keep_license for exempting a banner."),
        )
        .param(
            Param::boolean("keep_license")
                .default(true)
                .describe("Keep license and attribution banners even while remove_comments is stripping everything else. On by default, so an MIT or copyright header survives minification by accident-proof default. A comment is treated as a banner when it contains `@license`, `@preserve` or `@copyright`, or when it starts with a `!` right after the delimiter (`--!` or `--[[!`) — the same marker convention JS and CSS minifiers use. Set it to false to strip every comment without exception. It has no effect when remove_comments is false, since nothing is being stripped."),
        )
        .param(
            Param::boolean("rename_locals")
                .default(false)
                .describe("Also rename `local` variables, `local function` names and function parameters to short aliases (a, b, … z, A … Z, aa, …), which is where the bulk of the remaining bytes go. Off by default, because it is the one transform that can change behavior. What it will never rename: globals, table fields (`t.x`), method names (`t:m()`), table-constructor keys, `goto` labels, and anything inside a string — so `require`, `_ENV`, an exported module table and any name reached by reflection all keep working. Aliases are unique across the whole file, so an inner block can never shadow an outer local that is still referenced, and they avoid every global name the file uses. If the block structure does not balance (a missing or extra `end`), the rename refuses with an error instead of emitting plausible-but-wrong code — minifying without it still works. Do not use it on code that looks its own locals up by name (`debug.getlocal`, `load` over a string built from local names)."),
        )
        .param(
            Param::enumv("line_breaks", ["strip", "keep"])
                .default("strip")
                .describe("What happens to the source's line structure. \"strip\" (default) joins the whole script onto a single line for the smallest possible output — safe in Lua, which has no automatic-semicolon-insertion rule, so a line break never carries meaning. \"keep\" emits one output line per non-empty source line with the indentation removed, which stays diff-able and keeps runtime error line numbers roughly aligned with the original. Either way a kept `--` line comment still ends its line (otherwise it would swallow the rest of the script), and a `#!` shebang keeps its own first line."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct LuaMinifier;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/lua-minifier",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Minify Lua source by stripping comments and whitespace, optionally renaming locals to short names",
    skill(
        description = "Shrink Lua source code. Paste the script into `code` and get it back with the comments and the whitespace between tokens removed. The scan is token-aware rather than a find-and-replace, so string and long-string interiors, long-bracket levels, numeric literals and a `#!` shebang survive exactly as written, and any two tokens that would fuse into a different token when written adjacently (`a - -b`, `1 .. 2`, `t[ [[x]] ]`) keep the one space that stops them. Nothing is reordered, folded or dropped. `remove_comments` (on by default) strips `--` line comments and `--[[ … ]]` long comments, while `keep_license` (also on) exempts license and attribution banners — a comment containing `@license`, `@preserve` or `@copyright`, or starting with `--!` / `--[[!`. `line_breaks` chooses between \"strip\" (default — the whole script on one line, which is safe because Lua line breaks carry no meaning) and \"keep\" (one line per non-empty source line, minus the indentation, so the output stays diff-able and error line numbers roughly hold). `rename_locals` is off by default and adds a scope-aware pass that renames `local` variables, `local function` names and parameters to short unique aliases; it never touches globals, table fields, method names, table keys, `goto` labels or string contents, never lets an inner alias shadow an outer local that is still referenced, avoids every global name the file uses, and refuses with an explicit error rather than guessing if the block structure does not balance. This is a minifier, not an obfuscator and not a full Lua parser: there is no constant folding, dead-code removal or string encoding, and the output is always valid Lua that behaves as the input did. Errors come back when the input is empty or when it is nothing but comments and whitespace.",
        parameters = schema_json()
    ),
)]
impl LuaMinifier {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "lua-minifier", |a: Args| {
            gizza_ai_lua_minifier_core::run(
                &a.code,
                a.remove_comments,
                a.keep_license,
                a.rename_locals,
                &a.line_breaks,
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
            r##"{
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "The Lua source to minify, pasted exactly as written — a module, a script, a config chunk, any Lua 5.1 through 5.4 dialect including LuaJIT. Scanning is token-aware, so string and long-string interiors, long-bracket levels (`[=[ … ]=]`), numeric literal forms and a leading `#!` shebang line all survive untouched. No code is reordered, folded or dropped; the only thing removed is comments and the whitespace between tokens. Malformed input is minified as far as it can be read rather than rejected." },
                    "remove_comments": { "type": "boolean", "default": true, "description": "Strip `--` line comments and `--[[ … ]]` long comments (any bracket level). On by default — comments are usually the largest share of what a minifier removes. Turn it off to collapse only the whitespace, which is what you want when the comments are documentation you intend to ship. Text that merely looks like a comment inside a string is never touched. See keep_license for exempting a banner." },
                    "keep_license": { "type": "boolean", "default": true, "description": "Keep license and attribution banners even while remove_comments is stripping everything else. On by default, so an MIT or copyright header survives minification by accident-proof default. A comment is treated as a banner when it contains `@license`, `@preserve` or `@copyright`, or when it starts with a `!` right after the delimiter (`--!` or `--[[!`) — the same marker convention JS and CSS minifiers use. Set it to false to strip every comment without exception. It has no effect when remove_comments is false, since nothing is being stripped." },
                    "rename_locals": { "type": "boolean", "default": false, "description": "Also rename `local` variables, `local function` names and function parameters to short aliases (a, b, … z, A … Z, aa, …), which is where the bulk of the remaining bytes go. Off by default, because it is the one transform that can change behavior. What it will never rename: globals, table fields (`t.x`), method names (`t:m()`), table-constructor keys, `goto` labels, and anything inside a string — so `require`, `_ENV`, an exported module table and any name reached by reflection all keep working. Aliases are unique across the whole file, so an inner block can never shadow an outer local that is still referenced, and they avoid every global name the file uses. If the block structure does not balance (a missing or extra `end`), the rename refuses with an error instead of emitting plausible-but-wrong code — minifying without it still works. Do not use it on code that looks its own locals up by name (`debug.getlocal`, `load` over a string built from local names)." },
                    "line_breaks": { "type": "string", "enum": ["strip", "keep"], "default": "strip", "description": "What happens to the source's line structure. \"strip\" (default) joins the whole script onto a single line for the smallest possible output — safe in Lua, which has no automatic-semicolon-insertion rule, so a line break never carries meaning. \"keep\" emits one output line per non-empty source line with the indentation removed, which stays diff-able and keeps runtime error line numbers roughly aligned with the original. Either way a kept `--` line comment still ends its line (otherwise it would swallow the rest of the script), and a `#!` shebang keeps its own first line." }
                },
                "required": ["code"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn every_param_is_documented_for_an_llm() {
        for p in descriptor().params {
            assert!(
                p.description.len() > 80,
                "param `{}` needs a description an LLM can act on",
                p.name
            );
        }
    }
}
