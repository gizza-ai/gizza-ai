//! itunes-library-parser core — read an iTunes/Music `Library.xml` (an Apple XML
//! property list) and export its playlists and track metadata as CSV, TSV, JSON,
//! M3U/M3U8, a playlist index, or a library summary.
//!
//! Pure compute, shared by the chat skill block and the web page: the whole
//! library is parsed in memory, nothing is fetched, and no file is ever touched.
//! `Location` values in the XML are `file://` URLs, so they are percent-decoded
//! back to real filesystem paths and can optionally be re-rooted onto another
//! music folder (the classic "my drive letter changed" M3U problem).

use std::collections::BTreeMap;
use std::io::Cursor;

use plist::{Dictionary, Value};
use serde_json::{Map, Value as Json};

/// Hard cap on the pasted library text. A 20 MB `Library.xml` is roughly a
/// 40k-track library; past that the browser tab, not the parser, is the limit.
pub const MAX_INPUT_BYTES: usize = 20 * 1024 * 1024;

/// Largest accepted `limit`. 0 means "no limit".
pub const MAX_LIMIT: i64 = 100_000;

/// Columns emitted when `fields` is left empty.
pub const DEFAULT_FIELDS: &str = "name,artist,album,genre,year,duration,location";

/// How a track field is rendered once it has been pulled out of the plist.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    /// Plain string value.
    Text,
    /// Integer value.
    Int,
    /// Boolean value.
    Bool,
    /// Plist `<date>` rendered as an ISO-8601 UTC timestamp.
    Date,
    /// `Total Time` milliseconds rendered as `m:ss`.
    Mmss,
    /// `Total Time` milliseconds rendered as whole seconds.
    Secs,
    /// `Rating` 0-100 rendered as 0-5 stars (half stars allowed).
    Stars,
    /// `Location` `file://` URL decoded to a filesystem path.
    Path,
}

/// Every selectable track field: our canonical name, the plist key it reads, and
/// how it renders. Order is the order they appear in the docs/help text.
const FIELDS: &[(&str, &str, Kind)] = &[
    ("track_id", "Track ID", Kind::Int),
    ("name", "Name", Kind::Text),
    ("artist", "Artist", Kind::Text),
    ("album_artist", "Album Artist", Kind::Text),
    ("composer", "Composer", Kind::Text),
    ("album", "Album", Kind::Text),
    ("grouping", "Grouping", Kind::Text),
    ("work", "Work", Kind::Text),
    ("genre", "Genre", Kind::Text),
    ("kind", "Kind", Kind::Text),
    ("size", "Size", Kind::Int),
    ("duration", "Total Time", Kind::Mmss),
    ("duration_seconds", "Total Time", Kind::Secs),
    ("disc_number", "Disc Number", Kind::Int),
    ("disc_count", "Disc Count", Kind::Int),
    ("track_number", "Track Number", Kind::Int),
    ("track_count", "Track Count", Kind::Int),
    ("year", "Year", Kind::Int),
    ("bpm", "BPM", Kind::Int),
    ("date_added", "Date Added", Kind::Date),
    ("date_modified", "Date Modified", Kind::Date),
    ("release_date", "Release Date", Kind::Date),
    ("bit_rate", "Bit Rate", Kind::Int),
    ("sample_rate", "Sample Rate", Kind::Int),
    ("comments", "Comments", Kind::Text),
    ("play_count", "Play Count", Kind::Int),
    ("play_date", "Play Date UTC", Kind::Date),
    ("skip_count", "Skip Count", Kind::Int),
    ("rating", "Rating", Kind::Stars),
    ("rating_raw", "Rating", Kind::Int),
    ("album_rating", "Album Rating", Kind::Int),
    ("loved", "Loved", Kind::Bool),
    ("compilation", "Compilation", Kind::Bool),
    ("podcast", "Podcast", Kind::Bool),
    ("sort_name", "Sort Name", Kind::Text),
    ("sort_artist", "Sort Artist", Kind::Text),
    ("sort_album", "Sort Album", Kind::Text),
    ("persistent_id", "Persistent ID", Kind::Text),
    ("location", "Location", Kind::Path),
];

/// Playlists iTunes/Music creates itself. `Distinguished Kind` and `Master` mark
/// most of them, but libraries written by older versions (and some third-party
/// exporters) omit both, so the well-known names are a fallback.
const BUILTIN_NAMES: &[&str] = &[
    "library",
    "music",
    "movies",
    "tv shows",
    "podcasts",
    "audiobooks",
    "books",
    "purchased",
    "downloaded",
    "voice memos",
    "genius",
    "recently added",
    "recently played",
    "top 25 most played",
    "90's music",
    "my top rated",
    "music videos",
    "home videos",
    "classical music",
    "applications",
    "tones",
    "ringtones",
];

/// One playlist as it appears in the `Playlists` array.
struct Playlist {
    name: String,
    folder: bool,
    smart: bool,
    builtin: bool,
    persistent_id: String,
    parent_id: String,
    item_ids: Vec<i64>,
}

/// A parsed `Library.xml`.
struct Library {
    /// Decoded `Music Folder` path, with a trailing separator when present.
    music_folder: String,
    application_version: String,
    tracks: BTreeMap<i64, Dictionary>,
    playlists: Vec<Playlist>,
}

/// Parse an iTunes/Music `Library.xml` and render the requested export.
///
/// * `library` — the XML property list text.
/// * `output` — `csv` | `tsv` | `json` | `m3u` | `m3u8` | `playlists` | `summary`.
/// * `playlist` — playlist name to export (case-insensitive); empty = every track.
/// * `fields` — comma-separated track columns; empty = [`DEFAULT_FIELDS`].
/// * `path_prefix` — replaces the library's music-folder prefix on each path.
/// * `path_style` — `original` | `unix` | `windows` | `filename`.
/// * `include_builtin` — list Apple's own playlists in the `playlists` index.
/// * `sort_by` — `original` | `name` | `artist` | `album` | `year` | `duration` |
///   `play_count` | `date_added`.
/// * `limit` — max rows, 0 = all, up to [`MAX_LIMIT`].
#[allow(clippy::too_many_arguments)]
pub fn run(
    library: &str,
    output: &str,
    playlist: &str,
    fields: &str,
    path_prefix: &str,
    path_style: &str,
    include_builtin: bool,
    sort_by: &str,
    limit: i64,
) -> Result<String, String> {
    if library.trim().is_empty() {
        return Err("library is empty: paste the contents of your iTunes/Music Library.xml \
                    (File > Library > Export Library... in Music, or the Library.xml in your \
                    iTunes folder)"
            .into());
    }
    if library.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "library is {} MB, over the {} MB limit for a single run",
            library.len() / (1024 * 1024),
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }
    if !(0..=MAX_LIMIT).contains(&limit) {
        return Err(format!(
            "limit must be between 0 (no limit) and {MAX_LIMIT}, got {limit}"
        ));
    }
    let style = parse_path_style(path_style)?;
    let sort = parse_sort(sort_by)?;
    let lib = parse_library(library)?;

    match output {
        "summary" => Ok(render_summary(&lib)),
        "playlists" => Ok(render_playlist_index(&lib, include_builtin)),
        "csv" | "tsv" | "json" | "m3u" | "m3u8" => {
            let selected = select_tracks(&lib, playlist, sort, limit)?;
            match output {
                "m3u" | "m3u8" => Ok(render_m3u(
                    &lib,
                    &selected,
                    output == "m3u8",
                    path_prefix,
                    style,
                )),
                "json" => {
                    let cols = parse_fields(fields)?;
                    Ok(render_json(&lib, &selected, &cols, path_prefix, style))
                }
                _ => {
                    let cols = parse_fields(fields)?;
                    let sep = if output == "tsv" { '\t' } else { ',' };
                    Ok(render_delimited(
                        &lib,
                        &selected,
                        &cols,
                        sep,
                        path_prefix,
                        style,
                    ))
                }
            }
        }
        other => Err(format!(
            "unknown output \"{other}\": expected csv, tsv, json, m3u, m3u8, playlists or summary"
        )),
    }
}

/// The list of selectable track field names, for help text and error messages.
pub fn field_names() -> Vec<&'static str> {
    FIELDS.iter().map(|(n, _, _)| *n).collect()
}

// ---------------------------------------------------------------- parsing ---

fn parse_library(text: &str) -> Result<Library, String> {
    let value = Value::from_reader(Cursor::new(text.as_bytes())).map_err(|e| {
        format!(
            "could not parse the library as an XML property list: {e}. Expected the \
             <?xml ...?><plist> document that Music/iTunes writes as Library.xml"
        )
    })?;
    let root = value.as_dictionary().ok_or_else(|| {
        "expected the property list root to be a <dict>, got another plist value: this does \
         not look like an iTunes/Music Library.xml"
            .to_string()
    })?;
    if !root.contains_key("Tracks") && !root.contains_key("Playlists") {
        let mut keys: Vec<&str> = root.keys().map(|k| k.as_str()).take(6).collect();
        if root.len() > keys.len() {
            keys.push("...");
        }
        return Err(format!(
            "expected a \"Tracks\" or \"Playlists\" key at the top level of the property list, \
             found [{}]: this is a plist, but not an iTunes/Music library export",
            keys.join(", ")
        ));
    }

    let music_folder = root
        .get("Music Folder")
        .and_then(|v| v.as_string())
        .map(url_to_path)
        .unwrap_or_default();
    let application_version = root
        .get("Application Version")
        .and_then(|v| v.as_string())
        .unwrap_or_default()
        .to_string();

    let mut tracks = BTreeMap::new();
    if let Some(dict) = root.get("Tracks").and_then(|v| v.as_dictionary()) {
        for (key, value) in dict {
            let Some(track) = value.as_dictionary() else {
                continue;
            };
            let id = track
                .get("Track ID")
                .and_then(int_of)
                .or_else(|| key.parse::<i64>().ok());
            if let Some(id) = id {
                tracks.insert(id, track.clone());
            }
        }
    }

    let mut playlists = Vec::new();
    if let Some(list) = root.get("Playlists").and_then(|v| v.as_array()) {
        for entry in list {
            let Some(p) = entry.as_dictionary() else {
                continue;
            };
            let name = p
                .get("Name")
                .and_then(|v| v.as_string())
                .unwrap_or_default()
                .to_string();
            let folder = p.get("Folder").and_then(|v| v.as_boolean()).unwrap_or(false);
            let smart = p.contains_key("Smart Criteria") || p.contains_key("Smart Info");
            let master = p.get("Master").and_then(|v| v.as_boolean()).unwrap_or(false);
            let builtin = master
                || p.contains_key("Distinguished Kind")
                || BUILTIN_NAMES.contains(&name.to_lowercase().as_str());
            let item_ids = p
                .get("Playlist Items")
                .and_then(|v| v.as_array())
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|i| i.as_dictionary()?.get("Track ID").and_then(int_of))
                        .collect()
                })
                .unwrap_or_default();
            playlists.push(Playlist {
                name,
                folder,
                smart,
                builtin,
                persistent_id: p
                    .get("Playlist Persistent ID")
                    .and_then(|v| v.as_string())
                    .unwrap_or_default()
                    .to_string(),
                parent_id: p
                    .get("Parent Persistent ID")
                    .and_then(|v| v.as_string())
                    .unwrap_or_default()
                    .to_string(),
                item_ids,
            });
        }
    }

    Ok(Library {
        music_folder,
        application_version,
        tracks,
        playlists,
    })
}

/// Plist integers are signed or unsigned depending on how they were written.
fn int_of(v: &Value) -> Option<i64> {
    v.as_signed_integer()
        .or_else(|| v.as_unsigned_integer().and_then(|u| i64::try_from(u).ok()))
}

fn parse_fields(spec: &str) -> Result<Vec<&'static (&'static str, &'static str, Kind)>, String> {
    let spec = if spec.trim().is_empty() {
        DEFAULT_FIELDS
    } else {
        spec
    };
    let mut cols = Vec::new();
    for raw in spec.split(',') {
        let want = raw.trim().to_lowercase().replace([' ', '-'], "_");
        if want.is_empty() {
            continue;
        }
        match FIELDS.iter().find(|(name, _, _)| *name == want) {
            Some(f) => cols.push(f),
            None => {
                return Err(format!(
                    "unknown field \"{}\": expected one of {}",
                    raw.trim(),
                    field_names().join(", ")
                ))
            }
        }
    }
    if cols.is_empty() {
        return Err(format!(
            "fields selected no columns: expected a comma-separated list such as \"{DEFAULT_FIELDS}\""
        ));
    }
    Ok(cols)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PathStyle {
    Original,
    Unix,
    Windows,
    Filename,
}

fn parse_path_style(s: &str) -> Result<PathStyle, String> {
    match s.trim() {
        "" | "original" => Ok(PathStyle::Original),
        "unix" => Ok(PathStyle::Unix),
        "windows" => Ok(PathStyle::Windows),
        "filename" => Ok(PathStyle::Filename),
        other => Err(format!(
            "unknown path_style \"{other}\": expected original, unix, windows or filename"
        )),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sort {
    Original,
    Name,
    Artist,
    Album,
    Year,
    Duration,
    PlayCount,
    DateAdded,
}

fn parse_sort(s: &str) -> Result<Sort, String> {
    match s.trim() {
        "" | "original" => Ok(Sort::Original),
        "name" => Ok(Sort::Name),
        "artist" => Ok(Sort::Artist),
        "album" => Ok(Sort::Album),
        "year" => Ok(Sort::Year),
        "duration" => Ok(Sort::Duration),
        "play_count" => Ok(Sort::PlayCount),
        "date_added" => Ok(Sort::DateAdded),
        other => Err(format!(
            "unknown sort_by \"{other}\": expected original, name, artist, album, year, \
             duration, play_count or date_added"
        )),
    }
}

// -------------------------------------------------------------- selection ---

/// Resolve the requested playlist (or the whole library), apply `sort_by`, then
/// `limit`. Returns the track IDs to render, in output order.
fn select_tracks(
    lib: &Library,
    playlist: &str,
    sort: Sort,
    limit: i64,
) -> Result<Vec<i64>, String> {
    let wanted = playlist.trim();
    let mut ids: Vec<i64> = if wanted.is_empty() {
        // BTreeMap iteration = ascending Track ID, the library's own order.
        lib.tracks.keys().copied().collect()
    } else {
        let found = lib
            .playlists
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(wanted))
            .ok_or_else(|| {
                let mut names: Vec<&str> = lib
                    .playlists
                    .iter()
                    .filter(|p| !p.folder)
                    .map(|p| p.name.as_str())
                    .take(20)
                    .collect();
                if names.is_empty() {
                    names.push("(none in this library)");
                }
                format!(
                    "playlist \"{wanted}\" not found: expected one of {}. Leave playlist empty \
                     to export every track, or use output=playlists to list them all.",
                    names.join(", ")
                )
            })?;
        if found.folder {
            return Err(format!(
                "\"{}\" is a playlist folder, not a playlist: it holds other playlists and has \
                 no tracks of its own",
                found.name
            ));
        }
        found
            .item_ids
            .iter()
            .copied()
            .filter(|id| lib.tracks.contains_key(id))
            .collect()
    };

    if sort != Sort::Original {
        // Stable sort, so ties keep playlist/library order; tracks missing the
        // sort field sort last instead of clustering at the top as empty values.
        ids.sort_by(|a, b| {
            let ka = sort_key(lib.tracks.get(a).unwrap(), sort);
            let kb = sort_key(lib.tracks.get(b).unwrap(), sort);
            match (ka, kb) {
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (Some(_), None) => std::cmp::Ordering::Less,
                (Some(x), Some(y)) => x.cmp(&y),
            }
        });
    }

    if limit > 0 && ids.len() > limit as usize {
        ids.truncate(limit as usize);
    }
    Ok(ids)
}

/// Sort keys are strings so text and numbers share one comparison: numbers are
/// zero-padded so `9` sorts before `10`.
fn sort_key(t: &Dictionary, sort: Sort) -> Option<String> {
    let text = |key: &str| {
        t.get(key)
            .and_then(|v| v.as_string())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_lowercase())
    };
    let num = |key: &str| t.get(key).and_then(int_of).map(|n| format!("{n:020}"));
    match sort {
        Sort::Original => None,
        Sort::Name => text("Sort Name").or_else(|| text("Name")),
        Sort::Artist => text("Sort Artist")
            .or_else(|| text("Artist"))
            .or_else(|| text("Album Artist")),
        Sort::Album => text("Sort Album").or_else(|| text("Album")),
        Sort::Year => num("Year"),
        Sort::Duration => num("Total Time"),
        Sort::PlayCount => num("Play Count"),
        Sort::DateAdded => t
            .get("Date Added")
            .and_then(|v| v.as_date())
            // ISO-8601 UTC sorts lexicographically.
            .map(|d| d.to_xml_format()),
    }
}

// ----------------------------------------------------------------- paths ----

fn hex(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// `%20` → space, and so on. Invalid escapes are left verbatim.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// `file://localhost/Users/me/My%20Song.mp3` → `/Users/me/My Song.mp3`, and the
/// Windows form `file://localhost/C:/Music/x.mp3` → `C:/Music/x.mp3`.
fn url_to_path(url: &str) -> String {
    let rest = url
        .strip_prefix("file://localhost")
        .or_else(|| url.strip_prefix("file://"))
        .unwrap_or(url);
    let decoded = percent_decode(rest);
    let b = decoded.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[2] == b':' && b[1].is_ascii_alphabetic() {
        decoded[1..].to_string()
    } else {
        decoded
    }
}

/// Re-root a track path onto `prefix`, replacing the library's music folder.
/// Paths that live outside the music folder are returned unchanged.
fn apply_prefix(path: &str, music_folder: &str, prefix: &str) -> String {
    if prefix.is_empty() || path.is_empty() {
        return path.to_string();
    }
    let folder = music_folder.trim_end_matches(['/', '\\']);
    let rest = if !folder.is_empty() && path.len() > folder.len() && path.starts_with(folder) {
        &path[folder.len()..]
    } else {
        return path.to_string();
    };
    let base = prefix.trim_end_matches(['/', '\\']);
    let sep = if base.contains('\\') { '\\' } else { '/' };
    format!("{base}{sep}{}", rest.trim_start_matches(['/', '\\']))
}

fn style_path(path: &str, style: PathStyle) -> String {
    match style {
        PathStyle::Original => path.to_string(),
        PathStyle::Unix => path.replace('\\', "/"),
        PathStyle::Windows => path.replace('/', "\\"),
        PathStyle::Filename => path
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(path)
            .to_string(),
    }
}

fn track_path(lib: &Library, t: &Dictionary, prefix: &str, style: PathStyle) -> String {
    let Some(raw) = t.get("Location").and_then(|v| v.as_string()) else {
        return String::new();
    };
    let path = apply_prefix(&url_to_path(raw), &lib.music_folder, prefix);
    style_path(&path, style)
}

// ---------------------------------------------------------------- fields ----

fn cell(
    lib: &Library,
    t: &Dictionary,
    field: &(&str, &str, Kind),
    prefix: &str,
    style: PathStyle,
) -> Json {
    let (_, key, kind) = field;
    let Some(v) = t.get(key) else {
        return Json::Null;
    };
    match kind {
        Kind::Text => v.as_string().map(Json::from).unwrap_or(Json::Null),
        Kind::Int => int_of(v).map(Json::from).unwrap_or(Json::Null),
        Kind::Bool => v.as_boolean().map(Json::from).unwrap_or(Json::Null),
        Kind::Date => v
            .as_date()
            .map(|d| Json::from(d.to_xml_format()))
            .unwrap_or(Json::Null),
        Kind::Mmss => int_of(v)
            .map(|ms| Json::from(mmss(ms)))
            .unwrap_or(Json::Null),
        Kind::Secs => int_of(v)
            .map(|ms| Json::from((ms + 500) / 1000))
            .unwrap_or(Json::Null),
        // iTunes stores star ratings as 0-100 in steps of 20 (10 for half stars).
        Kind::Stars => int_of(v)
            .map(|r| Json::from(r as f64 / 20.0))
            .unwrap_or(Json::Null),
        Kind::Path => {
            let p = track_path(lib, t, prefix, style);
            if p.is_empty() {
                Json::Null
            } else {
                Json::from(p)
            }
        }
    }
}

/// JSON cell → the flat text a CSV/TSV/table wants. Null becomes an empty cell,
/// and whole-number floats (`4.0` stars) drop the trailing `.0`.
fn cell_text(v: &Json) -> String {
    match v {
        Json::Null => String::new(),
        Json::String(s) => s.clone(),
        Json::Bool(b) => b.to_string(),
        Json::Number(n) => match n.as_f64() {
            Some(f) if f.fract() == 0.0 && n.as_i64().is_none() => format!("{}", f as i64),
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

fn mmss(ms: i64) -> String {
    let total = (ms + 500) / 1000;
    format!("{}:{:02}", total / 60, total % 60)
}

fn hms(ms: i64) -> String {
    let total = ms / 1000;
    format!("{}:{:02}:{:02}", total / 3600, (total / 60) % 60, total % 60)
}

fn human_size(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    if u == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[u])
    }
}

// -------------------------------------------------------------- rendering ---

fn csv_escape(s: &str, sep: char) -> String {
    if sep == '\t' {
        // TSV has no quoting convention; make the cell single-line and tab-free.
        return s.replace(['\t', '\n', '\r'], " ");
    }
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_delimited(
    lib: &Library,
    ids: &[i64],
    cols: &[&(&str, &str, Kind)],
    sep: char,
    prefix: &str,
    style: PathStyle,
) -> String {
    let mut out = String::new();
    let header: Vec<String> = cols
        .iter()
        .map(|(n, _, _)| csv_escape(n, sep))
        .collect();
    out.push_str(&header.join(&sep.to_string()));
    out.push('\n');
    for id in ids {
        let Some(t) = lib.tracks.get(id) else { continue };
        let row: Vec<String> = cols
            .iter()
            .map(|f| csv_escape(&cell_text(&cell(lib, t, f, prefix, style)), sep))
            .collect();
        out.push_str(&row.join(&sep.to_string()));
        out.push('\n');
    }
    out
}

fn render_json(
    lib: &Library,
    ids: &[i64],
    cols: &[&(&str, &str, Kind)],
    prefix: &str,
    style: PathStyle,
) -> String {
    let rows: Vec<Json> = ids
        .iter()
        .filter_map(|id| lib.tracks.get(id))
        .map(|t| {
            let mut obj = Map::new();
            for f in cols {
                obj.insert(f.0.to_string(), cell(lib, t, f, prefix, style));
            }
            Json::Object(obj)
        })
        .collect();
    serde_json::to_string_pretty(&Json::Array(rows)).unwrap_or_else(|_| "[]".into())
}

fn render_m3u(
    lib: &Library,
    ids: &[i64],
    extended: bool,
    prefix: &str,
    style: PathStyle,
) -> String {
    let mut out = String::new();
    if extended {
        out.push_str("#EXTM3U\n");
    }
    for id in ids {
        let Some(t) = lib.tracks.get(id) else { continue };
        let path = track_path(lib, t, prefix, style);
        // A blank line is not a valid M3U entry, so tracks with no file (cloud
        // or missing) are skipped rather than emitted as an empty path.
        if path.is_empty() {
            continue;
        }
        if extended {
            let secs = t
                .get("Total Time")
                .and_then(int_of)
                .map(|ms| (ms + 500) / 1000)
                .unwrap_or(-1);
            let name = t.get("Name").and_then(|v| v.as_string()).unwrap_or("");
            let artist = t
                .get("Artist")
                .and_then(|v| v.as_string())
                .or_else(|| t.get("Album Artist").and_then(|v| v.as_string()))
                .unwrap_or("");
            let title = if artist.is_empty() {
                name.to_string()
            } else {
                format!("{artist} - {name}")
            };
            out.push_str(&format!("#EXTINF:{secs},{title}\n"));
        }
        out.push_str(&path);
        out.push('\n');
    }
    out
}

fn playlist_kind(p: &Playlist) -> &'static str {
    if p.folder {
        "folder"
    } else if p.builtin {
        "built-in"
    } else if p.smart {
        "smart"
    } else {
        "playlist"
    }
}

fn render_playlist_index(lib: &Library, include_builtin: bool) -> String {
    let by_id: BTreeMap<&str, &str> = lib
        .playlists
        .iter()
        .filter(|p| !p.persistent_id.is_empty())
        .map(|p| (p.persistent_id.as_str(), p.name.as_str()))
        .collect();
    let mut out = String::from("name,kind,tracks,parent,persistent_id\n");
    for p in &lib.playlists {
        if p.builtin && !include_builtin {
            continue;
        }
        let parent = by_id.get(p.parent_id.as_str()).copied().unwrap_or("");
        let row = [
            csv_escape(&p.name, ','),
            playlist_kind(p).to_string(),
            p.item_ids.len().to_string(),
            csv_escape(parent, ','),
            p.persistent_id.clone(),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out
}

/// Count values into a "N  value" leaderboard, highest first then alphabetical.
fn top_counts(lib: &Library, key: &str, take: usize) -> Vec<(String, usize)> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for t in lib.tracks.values() {
        if let Some(v) = t.get(key).and_then(|v| v.as_string()) {
            if !v.is_empty() {
                *counts.entry(v.to_string()).or_default() += 1;
            }
        }
    }
    let mut list: Vec<(String, usize)> = counts.into_iter().collect();
    list.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    list.truncate(take);
    list
}

fn render_summary(lib: &Library) -> String {
    let total_size: i64 = lib
        .tracks
        .values()
        .filter_map(|t| t.get("Size").and_then(int_of))
        .sum();
    let total_time: i64 = lib
        .tracks
        .values()
        .filter_map(|t| t.get("Total Time").and_then(int_of))
        .sum();
    let missing = lib
        .tracks
        .values()
        .filter(|t| t.get("Location").and_then(|v| v.as_string()).is_none())
        .count();
    let builtin = lib.playlists.iter().filter(|p| p.builtin).count();
    let folders = lib.playlists.iter().filter(|p| p.folder).count();
    let smart = lib
        .playlists
        .iter()
        .filter(|p| p.smart && !p.builtin && !p.folder)
        .count();
    let user = lib.playlists.len() - builtin - folders;

    let mut out = String::new();
    out.push_str(&format!("Tracks: {}\n", lib.tracks.len()));
    out.push_str(&format!("Tracks with no file path: {missing}\n"));
    out.push_str(&format!("Total size: {}\n", human_size(total_size)));
    out.push_str(&format!("Total playing time: {}\n", hms(total_time)));
    out.push_str(&format!(
        "Playlists: {} ({user} user, {smart} smart, {folders} folder, {builtin} built-in)\n",
        lib.playlists.len()
    ));
    if !lib.music_folder.is_empty() {
        out.push_str(&format!("Music folder: {}\n", lib.music_folder));
    }
    if !lib.application_version.is_empty() {
        out.push_str(&format!(
            "Application version: {}\n",
            lib.application_version
        ));
    }
    for (label, key) in [("Top artists", "Artist"), ("Top genres", "Genre")] {
        let list = top_counts(lib, key, 10);
        if list.is_empty() {
            continue;
        }
        out.push_str(&format!("\n{label}:\n"));
        for (value, count) in list {
            out.push_str(&format!("  {count}  {value}\n"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A miniature but structurally faithful Library.xml: two tracks, one user
    /// playlist, one built-in playlist, percent-encoded locations.
    const LIB: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Major Version</key><integer>1</integer>
  <key>Application Version</key><string>12.12.4.1</string>
  <key>Music Folder</key><string>file://localhost/Users/me/Music/iTunes/iTunes%20Media/</string>
  <key>Tracks</key>
  <dict>
    <key>101</key>
    <dict>
      <key>Track ID</key><integer>101</integer>
      <key>Name</key><string>Let It Happen</string>
      <key>Artist</key><string>Tame Impala</string>
      <key>Album</key><string>Currents</string>
      <key>Genre</key><string>Electronic</string>
      <key>Year</key><integer>2015</integer>
      <key>Total Time</key><integer>467000</integer>
      <key>Size</key><integer>11200000</integer>
      <key>Play Count</key><integer>12</integer>
      <key>Rating</key><integer>100</integer>
      <key>Date Added</key><date>2016-02-03T10:00:00Z</date>
      <key>Location</key><string>file://localhost/Users/me/Music/iTunes/iTunes%20Media/Music/Tame%20Impala/Currents/01%20Let%20It%20Happen.m4a</string>
    </dict>
    <key>102</key>
    <dict>
      <key>Track ID</key><integer>102</integer>
      <key>Name</key><string>All I Need</string>
      <key>Artist</key><string>Air</string>
      <key>Album</key><string>Moon Safari</string>
      <key>Genre</key><string>Electronic</string>
      <key>Year</key><integer>1998</integer>
      <key>Total Time</key><integer>268000</integer>
      <key>Size</key><integer>6400000</integer>
      <key>Play Count</key><integer>30</integer>
      <key>Date Added</key><date>2014-07-01T09:30:00Z</date>
      <key>Location</key><string>file://localhost/Users/me/Music/iTunes/iTunes%20Media/Music/Air/Moon%20Safari/03%20All%20I%20Need.m4a</string>
    </dict>
  </dict>
  <key>Playlists</key>
  <array>
    <dict>
      <key>Name</key><string>Library</string>
      <key>Master</key><true/>
      <key>Playlist Persistent ID</key><string>AAAA0001</string>
      <key>Playlist Items</key>
      <array><dict><key>Track ID</key><integer>101</integer></dict></array>
    </dict>
    <dict>
      <key>Name</key><string>Late Night</string>
      <key>Playlist Persistent ID</key><string>AAAA0002</string>
      <key>Playlist Items</key>
      <array>
        <dict><key>Track ID</key><integer>102</integer></dict>
        <dict><key>Track ID</key><integer>101</integer></dict>
      </array>
    </dict>
  </array>
</dict>
</plist>
"#;

    fn csv(playlist: &str, fields: &str) -> String {
        run(LIB, "csv", playlist, fields, "", "original", false, "original", 0).unwrap()
    }

    #[test]
    fn exports_the_whole_library_as_csv_in_track_id_order() {
        assert_eq!(
            csv("", "name,artist,year,duration"),
            "name,artist,year,duration\n\
             Let It Happen,Tame Impala,2015,7:47\n\
             All I Need,Air,1998,4:28\n"
        );
    }

    #[test]
    fn exports_a_named_playlist_in_playlist_order() {
        assert_eq!(
            csv("Late Night", "name,album"),
            "name,album\nAll I Need,Moon Safari\nLet It Happen,Currents\n"
        );
    }

    #[test]
    fn playlist_lookup_is_case_insensitive() {
        assert_eq!(csv("late night", "name"), csv("Late Night", "name"));
    }

    #[test]
    fn decodes_percent_escaped_file_urls_into_real_paths() {
        assert_eq!(
            csv("", "location"),
            "location\n\
             /Users/me/Music/iTunes/iTunes Media/Music/Tame Impala/Currents/01 Let It Happen.m4a\n\
             /Users/me/Music/iTunes/iTunes Media/Music/Air/Moon Safari/03 All I Need.m4a\n"
        );
    }

    #[test]
    fn re_roots_paths_onto_a_new_music_folder() {
        let out = run(
            LIB, "csv", "", "location", "D:\\Music", "windows", false, "original", 0,
        )
        .unwrap();
        assert!(
            out.contains("D:\\Music\\Music\\Air\\Moon Safari\\03 All I Need.m4a"),
            "{out}"
        );
    }

    #[test]
    fn filename_path_style_keeps_only_the_basename() {
        let out = run(LIB, "csv", "", "location", "", "filename", false, "name", 0).unwrap();
        assert_eq!(out, "location\n03 All I Need.m4a\n01 Let It Happen.m4a\n");
    }

    #[test]
    fn renders_extended_m3u_with_extinf_lines() {
        let out = run(LIB, "m3u8", "Late Night", "", "", "unix", false, "original", 0).unwrap();
        assert_eq!(
            out,
            "#EXTM3U\n\
             #EXTINF:268,Air - All I Need\n\
             /Users/me/Music/iTunes/iTunes Media/Music/Air/Moon Safari/03 All I Need.m4a\n\
             #EXTINF:467,Tame Impala - Let It Happen\n\
             /Users/me/Music/iTunes/iTunes Media/Music/Tame Impala/Currents/01 Let It Happen.m4a\n"
        );
    }

    #[test]
    fn plain_m3u_is_paths_only() {
        let out = run(LIB, "m3u", "Late Night", "", "", "filename", false, "original", 0).unwrap();
        assert_eq!(out, "03 All I Need.m4a\n01 Let It Happen.m4a\n");
    }

    #[test]
    fn playlist_index_hides_built_ins_by_default_and_shows_them_on_request() {
        let hidden = run(LIB, "playlists", "", "", "", "original", false, "original", 0).unwrap();
        assert_eq!(
            hidden,
            "name,kind,tracks,parent,persistent_id\nLate Night,playlist,2,,AAAA0002\n"
        );
        let shown = run(LIB, "playlists", "", "", "", "original", true, "original", 0).unwrap();
        assert!(shown.contains("Library,built-in,1,,AAAA0001\n"), "{shown}");
    }

    #[test]
    fn json_output_keeps_native_types_and_null_for_missing_fields() {
        let out = run(
            LIB, "json", "", "name,year,play_count,rating,loved", "", "original", false, "name", 1,
        )
        .unwrap();
        let rows: Json = serde_json::from_str(&out).unwrap();
        assert_eq!(rows[0]["name"], Json::from("All I Need"));
        assert_eq!(rows[0]["year"], Json::from(1998));
        assert_eq!(rows[0]["play_count"], Json::from(30));
        assert_eq!(rows[0]["rating"], Json::Null);
        assert_eq!(rows[0]["loved"], Json::Null);
        assert_eq!(rows.as_array().unwrap().len(), 1, "limit=1 kept one row");
    }

    #[test]
    fn ratings_render_as_stars() {
        assert_eq!(csv("", "name,rating,rating_raw"), "name,rating,rating_raw\nLet It Happen,5,100\nAll I Need,,\n");
    }

    #[test]
    fn sorts_by_play_count_and_by_artist() {
        assert_eq!(
            run(LIB, "csv", "", "artist", "", "original", false, "play_count", 0).unwrap(),
            "artist\nTame Impala\nAir\n"
        );
        assert_eq!(
            run(LIB, "csv", "", "artist", "", "original", false, "artist", 0).unwrap(),
            "artist\nAir\nTame Impala\n"
        );
    }

    #[test]
    fn tsv_uses_tabs_and_csv_quotes_commas() {
        let tsv = run(LIB, "tsv", "", "name,album", "", "original", false, "name", 1).unwrap();
        assert_eq!(tsv, "name\talbum\nAll I Need\tMoon Safari\n");
        let quoted = run(
            &LIB.replace("All I Need", "All, I Need"),
            "csv",
            "",
            "name",
            "",
            "original",
            false,
            "name",
            1,
        )
        .unwrap();
        assert_eq!(quoted, "name\n\"All, I Need\"\n");
    }

    #[test]
    fn summary_reports_counts_sizes_and_leaderboards() {
        let out = run(LIB, "summary", "", "", "", "original", false, "original", 0).unwrap();
        assert!(out.contains("Tracks: 2\n"), "{out}");
        assert!(out.contains("Total playing time: 0:12:15\n"), "{out}");
        assert!(out.contains("Playlists: 2 (1 user, 0 smart, 0 folder, 1 built-in)\n"), "{out}");
        assert!(out.contains("Music folder: /Users/me/Music/iTunes/iTunes Media/\n"), "{out}");
        assert!(out.contains("  2  Electronic\n"), "{out}");
    }

    #[test]
    fn empty_library_is_rejected_with_guidance() {
        let e = run("  ", "csv", "", "", "", "original", false, "original", 0).unwrap_err();
        assert!(e.contains("library is empty"), "{e}");
    }

    #[test]
    fn a_plist_that_is_not_an_itunes_library_is_rejected() {
        let other = r#"<?xml version="1.0"?><plist version="1.0"><dict><key>Foo</key><string>bar</string></dict></plist>"#;
        let e = run(other, "csv", "", "", "", "original", false, "original", 0).unwrap_err();
        assert!(e.contains("expected a \"Tracks\" or \"Playlists\" key"), "{e}");
        assert!(e.contains("Foo"), "{e}");
    }

    #[test]
    fn malformed_xml_is_rejected() {
        let e = run("not xml at all", "csv", "", "", "", "original", false, "original", 0)
            .unwrap_err();
        assert!(e.contains("could not parse the library"), "{e}");
    }

    #[test]
    fn an_unknown_playlist_lists_the_real_ones() {
        let e = csv_err("Road Trip");
        assert!(e.contains("playlist \"Road Trip\" not found"), "{e}");
        assert!(e.contains("Late Night"), "{e}");
    }

    #[test]
    fn an_unknown_field_lists_the_valid_ones() {
        let e = run(LIB, "csv", "", "nmae", "", "original", false, "original", 0).unwrap_err();
        assert!(e.contains("unknown field \"nmae\""), "{e}");
        assert!(e.contains("album_artist"), "{e}");
    }

    #[test]
    fn unknown_enums_and_out_of_range_limits_are_rejected() {
        for (out, want) in [
            (
                run(LIB, "xlsx", "", "", "", "original", false, "original", 0),
                "unknown output \"xlsx\"",
            ),
            (
                run(LIB, "csv", "", "", "", "dos", false, "original", 0),
                "unknown path_style \"dos\"",
            ),
            (
                run(LIB, "csv", "", "", "", "original", false, "shuffle", 0),
                "unknown sort_by \"shuffle\"",
            ),
            (
                run(LIB, "csv", "", "", "", "original", false, "original", -1),
                "limit must be between 0",
            ),
        ] {
            let e = out.unwrap_err();
            assert!(e.contains(want), "expected {want:?}, got {e:?}");
        }
    }

    #[test]
    fn a_playlist_folder_is_not_exportable_as_tracks() {
        let with_folder = LIB.replace(
            "<key>Name</key><string>Late Night</string>",
            "<key>Name</key><string>Late Night</string><key>Folder</key><true/>",
        );
        let e = run(
            &with_folder, "csv", "Late Night", "", "", "original", false, "original", 0,
        )
        .unwrap_err();
        assert!(e.contains("is a playlist folder"), "{e}");
    }

    #[test]
    fn field_names_are_documented_and_unique() {
        let names = field_names();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), names.len(), "duplicate field name");
        for f in DEFAULT_FIELDS.split(',') {
            assert!(names.contains(&f), "default field {f} is not selectable");
        }
    }

    fn csv_err(playlist: &str) -> String {
        run(LIB, "csv", playlist, "", "", "original", false, "original", 0).unwrap_err()
    }
}
