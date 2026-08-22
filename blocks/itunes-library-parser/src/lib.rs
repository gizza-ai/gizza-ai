//! gizza-ai/itunes-library-parser — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill, which calls the
//! pure core. Nothing is fetched and no file is touched: the library XML arrives
//! as text and the export comes back as text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_itunes_library_parser_core::{run, DEFAULT_FIELDS};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    library: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    playlist: String,
    #[serde(default = "default_fields")]
    fields: String,
    #[serde(default)]
    path_prefix: String,
    #[serde(default = "default_path_style")]
    path_style: String,
    #[serde(default)]
    include_builtin: bool,
    #[serde(default = "default_sort_by")]
    sort_by: String,
    #[serde(default)]
    limit: i64,
}

fn default_output() -> String {
    "csv".into()
}
fn default_fields() -> String {
    DEFAULT_FIELDS.into()
}
fn default_path_style() -> String {
    "original".into()
}
fn default_sort_by() -> String {
    "original".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("library").required().describe(
            "The full text of an iTunes/Music Library.xml — the Apple XML property list you get \
             from Music via File > Library > Export Library..., or the Library.xml/iTunes Music \
             Library.xml file in your iTunes folder. Must be the XML form (starts with <?xml and \
             a <plist> element); binary .itl libraries are not readable. Max 20 MB per run.",
        ))
        .param(
            Param::enumv(
                "output",
                ["csv", "tsv", "json", "m3u", "m3u8", "playlists", "summary"],
            )
            .default("csv")
            .describe(
                "What to produce. csv (default) and tsv emit one row per track with the columns \
                 named in fields; json emits an array of track objects with native numbers, \
                 booleans and nulls; m3u is a paths-only playlist file and m3u8 is the extended \
                 form with an #EXTM3U header and #EXTINF duration/artist lines; playlists emits a \
                 CSV index of every playlist (name, kind, tracks, parent, persistent_id); summary \
                 emits library totals plus top artists and genres.",
            ),
        )
        .param(Param::string("playlist").default("").describe(
            "Name of a single playlist to export, matched case-insensitively (for example \
             \"Late Night\"). Tracks come out in the playlist's own order unless sort_by says \
             otherwise. Leave empty (default) to export every track in the library. Run with \
             output=playlists first if you are not sure of the exact name.",
        ))
        .param(Param::string("fields").default(DEFAULT_FIELDS).describe(
            "Comma-separated track columns for the csv, tsv and json outputs, in the order you \
             want them; ignored by m3u, m3u8, playlists and summary. Default: \
             name,artist,album,genre,year,duration,location. Available: track_id, name, artist, \
             album_artist, composer, album, grouping, work, genre, kind, size, duration (m:ss), \
             duration_seconds, disc_number, disc_count, track_number, track_count, year, bpm, \
             date_added, date_modified, release_date, bit_rate, sample_rate, comments, play_count, \
             play_date, skip_count, rating (0-5 stars), rating_raw (0-100), album_rating, loved, \
             compilation, podcast, sort_name, sort_artist, sort_album, persistent_id, location.",
        ))
        .param(Param::string("path_prefix").default("").describe(
            "Re-root every file path: the library's own Music Folder prefix is replaced with this \
             value, so a playlist exported on a Mac can point at a Windows drive or a NAS share \
             (for example D:\\Music or /volume1/music). Tracks stored outside the library's music \
             folder keep their original path. Empty (default) leaves paths exactly as the library \
             records them.",
        ))
        .param(
            Param::enumv("path_style", ["original", "unix", "windows", "filename"])
                .default("original")
                .describe(
                    "Slash direction for file paths: original (default) keeps whatever the library \
                     recorded, unix rewrites backslashes to forward slashes, windows rewrites \
                     forward slashes to backslashes, and filename drops every folder and keeps only \
                     the file name — useful for a flat folder of copied music.",
                ),
        )
        .param(Param::boolean("include_builtin").default(false).describe(
            "Include the playlists Music/iTunes creates itself (Library, Music, Movies, Podcasts, \
             Downloaded, Recently Added, Genius, ...) in the output=playlists index. Off by \
             default so you see only your own playlists. Does not affect exporting a built-in \
             playlist by name, which always works.",
        ))
        .param(
            Param::enumv(
                "sort_by",
                [
                    "original",
                    "name",
                    "artist",
                    "album",
                    "year",
                    "duration",
                    "play_count",
                    "date_added",
                ],
            )
            .default("original")
            .describe(
                "Row order. original (default) keeps the playlist's own running order, or Track ID \
                 order when exporting the whole library. The others sort ascending by that field \
                 (using Sort Name/Sort Artist/Sort Album when the library provides them); tracks \
                 missing the field sort last.",
            ),
        )
        .param(
            Param::integer("limit")
                .min(0.0)
                .max(100_000.0)
                .default(0)
                .describe(
                    "Maximum number of track rows to emit, applied after sorting. 0 (default) means \
                     no limit; the ceiling is 100000. Use a small value such as 20 to preview a \
                     large library before exporting all of it.",
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
    name = "gizza-ai/itunes-library-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Export iTunes/Music Library.xml playlists and track metadata as CSV, TSV, JSON or M3U",
    skill(
        description = "Parse an iTunes/Music Library.xml (Apple XML property list) and export playlists and track metadata. Outputs CSV or TSV with any of 39 selectable columns (name, artist, album artist, album, genre, year, duration, bit rate, play count, star rating, date added, file path, ...), JSON with native types, a paths-only M3U or an extended M3U8 with #EXTINF lines, a CSV index of every playlist (name, kind, track count, parent folder, persistent ID), or a library summary with totals and top artists/genres. Export one playlist by name or the whole library, re-root file paths onto another drive or NAS with path_prefix, switch slash direction or reduce paths to bare file names, sort by name/artist/album/year/duration/play count/date added, and cap the row count. Percent-encoded file:// locations are decoded back to real filesystem paths. Runs entirely on the pasted text: nothing is uploaded, fetched or written. Max 20 MB of library XML per run.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "itunes-library-parser", |a: Args| {
            run(
                &a.library,
                &a.output,
                &a.playlist,
                &a.fields,
                &a.path_prefix,
                &a.path_style,
                a.include_builtin,
                &a.sort_by,
                a.limit,
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
            serde_json::json!(["library"]),
            "library is the only required param"
        );

        assert_eq!(
            props["output"]["enum"],
            serde_json::json!(["csv", "tsv", "json", "m3u", "m3u8", "playlists", "summary"])
        );
        assert_eq!(
            props["path_style"]["enum"],
            serde_json::json!(["original", "unix", "windows", "filename"])
        );
        assert_eq!(
            props["sort_by"]["enum"],
            serde_json::json!([
                "original",
                "name",
                "artist",
                "album",
                "year",
                "duration",
                "play_count",
                "date_added"
            ])
        );

        assert_eq!(props["output"]["default"], serde_json::json!("csv"));
        assert_eq!(props["path_style"]["default"], serde_json::json!("original"));
        assert_eq!(props["sort_by"]["default"], serde_json::json!("original"));
        assert_eq!(props["playlist"]["default"], serde_json::json!(""));
        assert_eq!(props["path_prefix"]["default"], serde_json::json!(""));
        assert_eq!(props["fields"]["default"], serde_json::json!(DEFAULT_FIELDS));

        assert_eq!(props["include_builtin"]["type"], serde_json::json!("boolean"));
        assert_eq!(props["include_builtin"]["default"], serde_json::json!(false));
        assert_eq!(props["limit"]["type"], serde_json::json!("integer"));
        assert_eq!(props["limit"]["minimum"], serde_json::json!(0));
        assert_eq!(props["limit"]["maximum"], serde_json::json!(100000));
        assert_eq!(props["limit"]["default"], serde_json::json!(0));

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
            "library",
            "output",
            "playlist",
            "fields",
            "path_prefix",
            "path_style",
            "include_builtin",
            "sort_by",
            "limit",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        expected.sort();
        assert_eq!(names, expected);

        let args: Args = serde_json::from_str(r#"{"library":"<plist/>"}"#).unwrap();
        assert_eq!(args.output, "csv");
        assert_eq!(args.fields, DEFAULT_FIELDS);
        assert_eq!(args.path_style, "original");
        assert_eq!(args.sort_by, "original");
        assert!(args.playlist.is_empty());
        assert!(!args.include_builtin);
        assert_eq!(args.limit, 0);
    }

    /// Every field name advertised in the `fields` description must actually be
    /// selectable, or the schema documents columns the core would reject.
    #[test]
    fn advertised_field_names_all_exist_in_the_core() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let doc = schema["properties"]["fields"]["description"]
            .as_str()
            .unwrap();
        for name in gizza_ai_itunes_library_parser_core::field_names() {
            assert!(
                doc.contains(name),
                "field {name} is selectable but undocumented in the fields describe()"
            );
        }
    }
}
