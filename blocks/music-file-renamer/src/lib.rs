//! gizza-ai/music-file-renamer — turn a pasted music tag dump into a
//! current-path → new-path rename/move PLAN. Chat/CLI schema is single-sourced
//! from descriptor(); the handler delegates to the pure core, which never
//! touches a filesystem.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_music_file_renamer_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    tracks: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_pattern")]
    pattern: String,
    #[serde(default)]
    base_dir: String,
    #[serde(default = "default_track_padding")]
    track_padding: i64,
    #[serde(default = "default_on_missing")]
    on_missing: String,
    #[serde(default = "default_unknown_text")]
    unknown_text: String,
    #[serde(default = "default_charset")]
    charset: String,
    #[serde(default = "default_replace_char")]
    replace_char: String,
    #[serde(default = "default_keep")]
    space_style: String,
    #[serde(default = "default_keep")]
    case_style: String,
    #[serde(default = "default_max_component")]
    max_component: i64,
    #[serde(default = "default_true")]
    keep_extension: bool,
    #[serde(default = "default_format")]
    format: String,
}

fn default_input_format() -> String {
    "auto".into()
}
fn default_pattern() -> String {
    gizza_ai_music_file_renamer_core::DEFAULT_PATTERN.into()
}
fn default_track_padding() -> i64 {
    2
}
fn default_on_missing() -> String {
    "unknown".into()
}
fn default_unknown_text() -> String {
    "Unknown".into()
}
fn default_charset() -> String {
    "windows".into()
}
fn default_replace_char() -> String {
    "_".into()
}
fn default_keep() -> String {
    "keep".into()
}
fn default_max_component() -> i64 {
    100
}
fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "table".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("tracks").required().describe(
            "The tag dump, one record per file. Accepts CSV/TSV (or ;/| delimited) with a header row, \
             a JSON array of tag objects (ffprobe -print_format json and music-metadata shapes included), \
             or key=value / key: value blocks separated by blank lines (ffprobe -show_format, exiftool). \
             Every record needs its current file name in a file/filename/path/SourceFile field, plus \
             whatever tags it has (artist, albumartist, album, title, track, disc, year, genre, composer, \
             comment, ...). Max 5000 records per run. Example: \
             'file,artist,album,track,title' then 'track01.mp3,Tame Impala,Currents,1,Let It Happen'.",
        ))
        .param(
            Param::enumv("input_format", ["auto", "csv", "tsv", "json", "keyvalue"])
                .default("auto")
                .describe(
                    "How to read the tracks text. auto (default) sniffs JSON vs delimited vs key=value \
                     and picks the delimiter; force csv, tsv, json or keyvalue when the sniffer guesses wrong.",
                ),
        )
        .param(
            Param::string("pattern")
                .default(gizza_ai_music_file_renamer_core::DEFAULT_PATTERN)
                .describe(
                    "Target path template. {token} inserts a tag; / (or \\) starts a new folder, so the \
                     plan is a move, not just a rename. Canonical tokens: artist, albumartist, album, title, \
                     track, disc, year, genre, composer, comment, ext — plus any other column/field in your \
                     dump by its own name (e.g. {bitrate}, {isrc}) and the current-file tokens {filename}, \
                     {stem}, {dir}. Write {a|b|c} for a fallback chain: {albumartist|artist} keeps \
                     compilations in one folder. Default: {artist}/{album}/{track} {title}.",
                ),
        )
        .param(Param::string("base_dir").default("").describe(
            "Optional destination root prefixed to every target path, e.g. /srv/music or D:/Library. \
             Empty (default) leaves the plan relative to wherever the current paths are.",
        ))
        .param(
            Param::integer("track_padding")
                .min(0.0)
                .max(6.0)
                .default(2)
                .describe(
                    "Digits to zero-pad {track} to, 0-6 (default 2, so 3 and '3/12' both render as 03). \
                     Use 0 to leave the number unpadded.",
                ),
        )
        .param(
            Param::enumv("on_missing", ["unknown", "skip", "keep_original"])
                .default("unknown")
                .describe(
                    "What to do with a file whose dump lacks a tag the pattern needs: unknown (default) \
                     substitutes unknown_text and still renames it, skip leaves it out of the plan and \
                     lists it under Skipped, keep_original plans no move for it.",
                ),
        )
        .param(Param::string("unknown_text").default("Unknown").describe(
            "Filler written in place of a missing tag when on_missing=unknown, and used when a whole path \
             component sanitises away to nothing. Default: Unknown.",
        ))
        .param(
            Param::enumv("charset", ["windows", "unix", "ascii"])
                .default("windows")
                .describe(
                    "Which characters the destination filesystem tolerates. windows (default, also safe on \
                     macOS/Linux) strips < > : \" | ? * and control characters, trims trailing dots/spaces \
                     and defuses reserved names like CON; unix only forbids the separator and NUL; ascii \
                     applies the windows rules and folds accents so the result is plain 7-bit ASCII \
                     (Sigur Rós → Sigur Ros).",
                ),
        )
        .param(Param::string("replace_char").default("_").describe(
            "Text substituted for each character that is illegal on the chosen charset (default _). \
             Set it to an empty string to delete illegal characters instead of replacing them.",
        ))
        .param(
            Param::enumv("space_style", ["keep", "underscore", "hyphen"])
                .default("keep")
                .describe(
                    "What happens to spaces in generated names: keep them (default), or convert every \
                     space to an underscore or a hyphen.",
                ),
        )
        .param(
            Param::enumv("case_style", ["keep", "lower", "upper", "title"])
                .default("keep")
                .describe(
                    "Letter case applied to every generated path component: keep the tag's own case \
                     (default), lower, upper, or title case.",
                ),
        )
        .param(
            Param::integer("max_component")
                .min(8.0)
                .max(255.0)
                .default(100)
                .describe(
                    "Maximum characters per folder or file name, 8-255 (default 100). Longer components are \
                     truncated; the file extension is re-appended after the cut so it is never lost.",
                ),
        )
        .param(Param::boolean("keep_extension").default(true).describe(
            "When true (default) the current file's extension is appended to the last path component \
             unless the pattern already ends with it. Turn it off to control the extension yourself with \
             an explicit {ext} token.",
        ))
        .param(
            Param::enumv("format", ["table", "list", "csv", "json", "sh"])
                .default("table")
                .describe(
                    "Shape of the plan: table (default) is a summary line plus aligned 'current -> new' rows \
                     with [unchanged]/[collision] flags, list is bare 'current -> new' lines, csv is \
                     current_path,new_path,status, json is a structured plan with counts, and sh emits a \
                     reviewable /bin/sh script of mkdir -p plus mv -n commands you can run yourself.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/music-file-renamer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Plan tag-driven music renames and folder moves from a pasted tag dump",
    skill(
        description = "Turn a music tag dump into a deterministic current-path -> new-path rename and move plan. Reads CSV/TSV, a JSON array of tag objects (ffprobe/music-metadata shapes included) or key=value blocks (ffprobe -show_format, exiftool), then builds each target from a {token} path template over the tags: artist, albumartist, album, title, track, disc, year, genre, composer, comment, ext and any other field in the dump, with {a|b} fallback chains, zero-padded track numbers, year normalisation, a destination root, per-filesystem character sanitising (windows/unix/ascii accent folding), space and case styles, a per-component length cap and case-insensitive collision detection. Output as a table, a plain list, CSV, JSON, or a runnable mkdir -p + mv -n shell script. Preview only: it computes names and never touches, uploads or moves any file. Max 5000 records per run.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "music-file-renamer", |a: Args| {
            run(
                &a.tracks,
                &a.input_format,
                &a.pattern,
                &a.base_dir,
                a.track_padding,
                &a.on_missing,
                &a.unknown_text,
                &a.charset,
                &a.replace_char,
                &a.space_style,
                &a.case_style,
                a.max_component,
                a.keep_extension,
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

    /// Drift guard: the page form reads manifest.json, which is generated from
    /// this schema — so the enum variant lists, bounds and defaults asserted here
    /// are exactly what decides whether a field renders as a <select>, a bounded
    /// number box or a checkbox.
    #[test]
    fn schema_json_matches_authored_parameters() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(
            schema["required"],
            serde_json::json!(["tracks"]),
            "tracks is the only required param"
        );

        assert_eq!(
            props["input_format"]["enum"],
            serde_json::json!(["auto", "csv", "tsv", "json", "keyvalue"])
        );
        assert_eq!(
            props["on_missing"]["enum"],
            serde_json::json!(["unknown", "skip", "keep_original"])
        );
        assert_eq!(
            props["charset"]["enum"],
            serde_json::json!(["windows", "unix", "ascii"])
        );
        assert_eq!(
            props["space_style"]["enum"],
            serde_json::json!(["keep", "underscore", "hyphen"])
        );
        assert_eq!(
            props["case_style"]["enum"],
            serde_json::json!(["keep", "lower", "upper", "title"])
        );
        assert_eq!(
            props["format"]["enum"],
            serde_json::json!(["table", "list", "csv", "json", "sh"])
        );

        assert_eq!(props["input_format"]["default"], serde_json::json!("auto"));
        assert_eq!(props["on_missing"]["default"], serde_json::json!("unknown"));
        assert_eq!(props["charset"]["default"], serde_json::json!("windows"));
        assert_eq!(props["space_style"]["default"], serde_json::json!("keep"));
        assert_eq!(props["case_style"]["default"], serde_json::json!("keep"));
        assert_eq!(props["format"]["default"], serde_json::json!("table"));

        assert_eq!(props["track_padding"]["minimum"], serde_json::json!(0));
        assert_eq!(props["track_padding"]["maximum"], serde_json::json!(6));
        assert_eq!(props["track_padding"]["default"], serde_json::json!(2));
        assert_eq!(props["max_component"]["minimum"], serde_json::json!(8));
        assert_eq!(props["max_component"]["maximum"], serde_json::json!(255));
        assert_eq!(props["max_component"]["default"], serde_json::json!(100));
        assert_eq!(
            props["keep_extension"]["type"],
            serde_json::json!("boolean")
        );
        assert_eq!(props["keep_extension"]["default"], serde_json::json!(true));
        assert_eq!(
            props["pattern"]["default"],
            serde_json::json!(gizza_ai_music_file_renamer_core::DEFAULT_PATTERN)
        );
        assert_eq!(props["replace_char"]["default"], serde_json::json!("_"));
        assert_eq!(
            props["unknown_text"]["default"],
            serde_json::json!("Unknown")
        );

        // Every param must carry a description an LLM/CLI user can act on.
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 30, "param {name} needs a real describe(): {d:?}");
        }
    }

    /// The descriptor's param set must line up with the Args struct the handler
    /// deserialises, or a chat call silently drops an option.
    #[test]
    fn every_descriptor_param_is_wired_through_to_the_core() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let mut names: Vec<String> = schema["properties"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        names.sort();
        let mut expected: Vec<String> = [
            "tracks",
            "input_format",
            "pattern",
            "base_dir",
            "track_padding",
            "on_missing",
            "unknown_text",
            "charset",
            "replace_char",
            "space_style",
            "case_style",
            "max_component",
            "keep_extension",
            "format",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        expected.sort();
        assert_eq!(names, expected);

        let args: Args = serde_json::from_str(r#"{"tracks":"file,title\na.mp3,T\n"}"#).unwrap();
        assert_eq!(args.pattern, "{artist}/{album}/{track} {title}");
        assert_eq!(args.track_padding, 2);
        assert_eq!(args.max_component, 100);
        assert!(args.keep_extension);
        assert_eq!(args.format, "table");
    }
}
