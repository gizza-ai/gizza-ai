//! google-takeout-photo-merger core — merge Google Takeout JSON sidecar
//! metadata (date taken, GPS position, caption) back into each photo's EXIF.
//!
//! Input is ONE ZIP (a Takeout album folder); output is ONE ZIP with the same
//! layout, the photos rewritten, and the now-redundant `.json` sidecars
//! dropped. Pure Rust: `zip` (deflate) + `serde_json` + the shipped
//! `gizza-ai-exif-edit-core` write engine + `kamadak-exif` on the read side.
//!
//! Why a ZIP in and a ZIP out: the job is inherently per-album ("merge the
//! metadata back into EACH photo"), and a single archive is the only batch
//! in/batch out shape the block model can carry.
//!
//! Deliberate limits (see the competitor analysis doc):
//!   - EXIF can only be written to JPEG and PNG. HEIC/MP4/MOV/AVI/GIF are
//!     copied through untouched and reported, never silently dropped.
//!   - Sidecar timestamps are Unix epochs, i.e. UTC, and are written as UTC.
//!     Deriving the camera's local time would need a GPS→timezone database.
//!   - A Takeout export split across several ZIPs must be recombined first;
//!     a photo whose sidecar lives in another archive is reported unmatched.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use exif::{In, Tag};
use gizza_ai_exif_edit_core::{edit, parse_date, Edits};
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Hard ceiling on entries in the input archive (a Takeout album is far below).
const MAX_ENTRIES: usize = 20_000;
/// Hard ceiling on the uncompressed contents (zip-bomb guard).
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
/// Shortest prefix accepted when matching a sidecar whose name Google clipped.
/// Below this a prefix match stops being evidence and starts being a guess.
const MIN_TRUNCATED_PREFIX: usize = 12;

/// The metadata groups that can be merged. Order is the documented order.
pub const FIELD_GROUPS: [&str; 3] = ["date", "gps", "description"];

/// Where to take the GPS position from when a sidecar carries both blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpsSource {
    /// Prefer a non-zero `geoData` (what Google Photos shows, including a
    /// location the user corrected by hand), else fall back to `geoDataExif`.
    Auto,
    /// Only ever read `geoData`.
    GeoData,
    /// Only ever read `geoDataExif` (the position the camera originally wrote).
    GeoDataExif,
}

/// Where to take the capture timestamp from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateSource {
    /// `photoTakenTime`, falling back to `creationTime` when it is absent.
    Auto,
    /// Only ever read `photoTakenTime` (when the photo was shot).
    PhotoTaken,
    /// Only ever read `creationTime` (when it was uploaded to Google Photos).
    Creation,
}

/// Everything the caller can tune. `Default` is the recommended configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Options {
    /// Validated subset of [`FIELD_GROUPS`], in the caller's order.
    pub fields: Vec<String>,
    /// Replace EXIF fields the photo already has. Default `false`: a real
    /// camera timestamp always beats Google's, so only gaps are filled.
    pub overwrite: bool,
    pub gps_source: GpsSource,
    pub date_source: DateSource,
    /// Rename entries whose magic bytes disagree with their extension
    /// (Takeout exports PNGs named `.jpg`, which breaks other importers).
    pub fix_extension: bool,
    /// Stamp each output entry's ZIP timestamp with the capture time, so the
    /// files sort correctly after extraction even in apps that ignore EXIF.
    pub set_file_times: bool,
    /// Carry the `.json` sidecars into the output ZIP instead of dropping them.
    pub keep_sidecars: bool,
    /// Produce the report only — no output ZIP is built.
    pub dry_run: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            fields: FIELD_GROUPS.iter().map(|s| s.to_string()).collect(),
            overwrite: false,
            gps_source: GpsSource::Auto,
            date_source: DateSource::Auto,
            fix_extension: true,
            set_file_times: true,
            keep_sidecars: false,
            dry_run: false,
        }
    }
}

/// What happened to one input file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// EXIF was written (or, in a dry run, would be written).
    Merged,
    /// A sidecar was found but every requested field was already present and
    /// `overwrite` was off — the photo is copied through unchanged.
    AlreadyComplete,
    /// No sidecar could be paired with this photo.
    NoSidecar,
    /// A media file whose container cannot carry EXIF (HEIC/MP4/MOV/GIF/…).
    SkippedUnsupported,
    /// The sidecar or the image itself could not be processed; `note` says why.
    /// The original bytes are copied through untouched.
    Failed,
    /// A `.json` sidecar consumed by the merge.
    Sidecar,
    /// Anything else in the archive (album metadata, print-subscriptions, …).
    Other,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Merged => "merged",
            Status::AlreadyComplete => "already-complete",
            Status::NoSidecar => "no-sidecar",
            Status::SkippedUnsupported => "skipped-unsupported",
            Status::Failed => "failed",
            Status::Sidecar => "sidecar",
            Status::Other => "other",
        }
    }
}

/// One row of the per-file report.
#[derive(Debug, Clone, PartialEq)]
pub struct FileOutcome {
    /// Path as it appeared in the input archive.
    pub path: String,
    /// Path in the output archive (differs when `fix_extension` renamed it).
    pub out_path: String,
    pub status: Status,
    /// Sidecar that was paired with this photo, if any.
    pub sidecar: Option<String>,
    /// EXIF tag groups actually written ("date", "gps", "description").
    pub fields_written: Vec<String>,
    /// Human-readable detail (why it failed, what was renamed, …).
    pub note: Option<String>,
}

/// Aggregate result of a merge.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MergeReport {
    pub media_total: usize,
    pub merged: usize,
    pub already_complete: usize,
    pub no_sidecar: usize,
    pub skipped_unsupported: usize,
    pub failed: usize,
    pub sidecars_found: usize,
    pub sidecars_dropped: usize,
    pub extensions_fixed: usize,
    pub dry_run: bool,
    pub files: Vec<FileOutcome>,
}

/// The subset of a Takeout sidecar this tool understands.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Sidecar {
    pub title: Option<String>,
    pub description: Option<String>,
    /// `photoTakenTime.timestamp`, seconds since the Unix epoch (UTC).
    pub photo_taken: Option<i64>,
    /// `creationTime.timestamp`, seconds since the Unix epoch (UTC).
    pub creation: Option<i64>,
    /// `geoData` — `None` when absent or all-zero (Google's "unknown").
    pub geo: Option<Geo>,
    /// `geoDataExif` — same convention.
    pub geo_exif: Option<Geo>,
}

/// A decoded GPS position.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Geo {
    pub latitude: f64,
    pub longitude: f64,
    pub altitude: Option<f64>,
}

// ---------------------------------------------------------------------------
// Option parsing / validation
// ---------------------------------------------------------------------------

/// Parse a `fields` list ("date, gps") into validated group names.
pub fn parse_fields(list: &str) -> Result<Vec<String>, String> {
    let mut out: Vec<String> = Vec::new();
    for raw in list.split(',') {
        let g = raw.trim().to_ascii_lowercase();
        if g.is_empty() {
            continue;
        }
        if g == "all" {
            return Ok(FIELD_GROUPS.iter().map(|s| s.to_string()).collect());
        }
        if !FIELD_GROUPS.contains(&g.as_str()) {
            return Err(format!(
                "unknown field group '{g}': valid groups are {} (or 'all')",
                FIELD_GROUPS.join(", ")
            ));
        }
        if !out.contains(&g) {
            out.push(g);
        }
    }
    if out.is_empty() {
        return Err(format!(
            "fields is empty: pass at least one of {} (or 'all')",
            FIELD_GROUPS.join(", ")
        ));
    }
    Ok(out)
}

pub fn parse_gps_source(s: &str) -> Result<GpsSource, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(GpsSource::Auto),
        "geo_data" => Ok(GpsSource::GeoData),
        "geo_data_exif" => Ok(GpsSource::GeoDataExif),
        other => Err(format!(
            "unknown gps_source '{other}': valid values are auto, geo_data, geo_data_exif"
        )),
    }
}

pub fn parse_date_source(s: &str) -> Result<DateSource, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "auto" => Ok(DateSource::Auto),
        "photo_taken" => Ok(DateSource::PhotoTaken),
        "creation" => Ok(DateSource::Creation),
        other => Err(format!(
            "unknown date_source '{other}': valid values are auto, photo_taken, creation"
        )),
    }
}

// ---------------------------------------------------------------------------
// Time
// ---------------------------------------------------------------------------

/// Unix epoch seconds → civil UTC (year, month, day, hour, minute, second).
/// Howard Hinnant's `civil_from_days`, valid for the whole EXIF year range.
pub fn civil_from_epoch(secs: i64) -> Result<(i32, u32, u32, u32, u32, u32), String> {
    // EXIF's ASCII date has a 4-digit year; clamp to what it can express.
    if !(-62_135_596_800..=253_402_300_799).contains(&secs) {
        return Err(format!(
            "timestamp {secs} is outside the range EXIF can store (years 1..9999)"
        ));
    }
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);

    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };

    Ok((
        year as i32,
        m as u32,
        d as u32,
        hour as u32,
        minute as u32,
        second as u32,
    ))
}

/// Unix epoch seconds → EXIF's `"YYYY:MM:DD HH:MM:SS"` (UTC).
pub fn exif_datetime_from_epoch(secs: i64) -> Result<String, String> {
    let (y, mo, d, h, mi, s) = civil_from_epoch(secs)?;
    Ok(format!("{y:04}:{mo:02}:{d:02} {h:02}:{mi:02}:{s:02}"))
}

// ---------------------------------------------------------------------------
// Sidecar parsing
// ---------------------------------------------------------------------------

/// A Takeout timestamp field is `{ "timestamp": "1698765432", "formatted": … }`
/// — the epoch arrives as a STRING in every export seen, but tolerate a number.
fn json_timestamp(v: &serde_json::Value, key: &str) -> Option<i64> {
    let t = v.get(key)?.get("timestamp")?;
    match t {
        serde_json::Value::String(s) => s.trim().parse::<i64>().ok(),
        serde_json::Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

/// Google writes `0.0` for "no location known", not a missing key — a literal
/// null island reading is indistinguishable from that and is treated as absent.
fn json_geo(v: &serde_json::Value, key: &str) -> Option<Geo> {
    let g = v.get(key)?;
    let lat = g.get("latitude")?.as_f64()?;
    let lon = g.get("longitude")?.as_f64()?;
    if !lat.is_finite() || !lon.is_finite() {
        return None;
    }
    if lat == 0.0 && lon == 0.0 {
        return None;
    }
    if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
        return None;
    }
    let altitude = g
        .get("altitude")
        .and_then(|a| a.as_f64())
        .filter(|a| a.is_finite() && *a != 0.0 && (-11_000.0..=20_000.0).contains(a));
    Some(Geo {
        latitude: lat,
        longitude: lon,
        altitude,
    })
}

fn non_empty(v: &serde_json::Value, key: &str) -> Option<String> {
    let s = v.get(key)?.as_str()?.trim();
    if s.is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

/// Parse one sidecar's JSON bytes.
pub fn parse_sidecar(bytes: &[u8]) -> Result<Sidecar, String> {
    let v: serde_json::Value = serde_json::from_slice(bytes)
        .map_err(|e| format!("sidecar is not valid JSON: {e}"))?;
    if !v.is_object() {
        return Err("sidecar JSON is not an object".into());
    }
    Ok(Sidecar {
        title: non_empty(&v, "title"),
        description: non_empty(&v, "description"),
        photo_taken: json_timestamp(&v, "photoTakenTime"),
        creation: json_timestamp(&v, "creationTime"),
        geo: json_geo(&v, "geoData"),
        geo_exif: json_geo(&v, "geoDataExif"),
    })
}

impl Sidecar {
    /// Capture timestamp under the caller's `date_source` policy.
    pub fn timestamp(&self, source: DateSource) -> Option<i64> {
        match source {
            DateSource::Auto => self.photo_taken.or(self.creation),
            DateSource::PhotoTaken => self.photo_taken,
            DateSource::Creation => self.creation,
        }
    }

    /// GPS position under the caller's `gps_source` policy.
    pub fn position(&self, source: GpsSource) -> Option<Geo> {
        match source {
            GpsSource::Auto => self.geo.or(self.geo_exif),
            GpsSource::GeoData => self.geo,
            GpsSource::GeoDataExif => self.geo_exif,
        }
    }
}

// ---------------------------------------------------------------------------
// Sidecar ↔ photo pairing
// ---------------------------------------------------------------------------

/// Split `dir/name` into its directory prefix (with the slash) and file name.
fn split_dir(path: &str) -> (&str, &str) {
    match path.rfind('/') {
        Some(i) => (&path[..=i], &path[i + 1..]),
        None => ("", path),
    }
}

/// `photo.jpg` → `("photo", Some("jpg"))`; a dotfile keeps its whole name.
fn split_ext(name: &str) -> (&str, Option<&str>) {
    match name.rfind('.') {
        Some(i) if i > 0 => (&name[..i], Some(&name[i + 1..])),
        _ => (name, None),
    }
}

/// `IMG_0001(1).jpg` → `("IMG_0001", "1", "jpg")`. Google moves the counter
/// when it names the sidecar: the pair is `IMG_0001(1).jpg` ↔
/// `IMG_0001.jpg(1).json`, which no naive `name + ".json"` rule finds.
fn split_counter(name: &str) -> Option<(&str, &str, Option<&str>)> {
    let (stem, ext) = split_ext(name);
    let open = stem.rfind('(')?;
    if !stem.ends_with(')') {
        return None;
    }
    let digits = &stem[open + 1..stem.len() - 1];
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some((&stem[..open], digits, ext))
}

/// Google Photos exports an edited copy alongside the original and gives BOTH
/// the original's sidecar. Localized exports use a translated word, so match
/// the shape (`-<word>` before the extension) for the known set only —
/// guessing any trailing `-word` would eat legitimate file names.
const EDITED_SUFFIXES: [&str; 6] = ["-edited", "-bearbeitet", "-bewerkt", "-editado", "-modifié", "-redigerad"];

/// Strip an "edited copy" suffix from a file name, keeping the extension.
fn strip_edited(name: &str) -> Option<String> {
    let (stem, ext) = split_ext(name);
    let lower = stem.to_lowercase();
    for suf in EDITED_SUFFIXES {
        if lower.ends_with(suf) && lower.len() > suf.len() {
            let base = &stem[..stem.len() - suf.len()];
            return Some(match ext {
                Some(e) => format!("{base}.{e}"),
                None => base.to_string(),
            });
        }
    }
    None
}

/// Every base name a sidecar could have been keyed on, most specific first.
fn base_candidates(name: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut push = |s: String| {
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    };
    push(name.to_string());
    // `IMG(1).jpg` → `IMG.jpg(1)`, Google's counter placement.
    if let Some((stem, n, ext)) = split_counter(name) {
        match ext {
            Some(e) => push(format!("{stem}.{e}({n})")),
            None => push(format!("{stem}({n})")),
        }
    }
    // Bare stem: the legacy `IMG_0001.json` form.
    let (stem, _) = split_ext(name);
    push(stem.to_string());
    out
}

/// Sidecar names Google may have written for `base`, most specific first.
/// (`base` is already one of [`base_candidates`]'s entries.)
fn sidecar_names(base: &str) -> [String; 2] {
    [
        format!("{base}.supplemental-metadata.json"),
        format!("{base}.json"),
    ]
}

/// Find the sidecar for `file_name` among the `.json` entries in its directory.
///
/// Tries, in order: the exact modern name, the exact legacy name, then a
/// truncated modern name — Google clips the whole sidecar file name at roughly
/// 46–51 characters, producing `….supplemental-metad.json`, `….suppleme.json`
/// and friends, so any `.json` entry whose stem is a proper prefix of the
/// untruncated name is accepted. Each base form (as-is, counter-swapped, bare
/// stem) is tried before falling back to the `-edited`-stripped forms.
pub fn find_sidecar(file_name: &str, jsons_in_dir: &[String]) -> Option<String> {
    let mut roots = vec![file_name.to_string()];
    if let Some(orig) = strip_edited(file_name) {
        roots.push(orig);
    }

    // Pass 1+2: exact names, most specific base first.
    for root in &roots {
        for base in base_candidates(root) {
            for want in sidecar_names(&base) {
                if let Some(hit) = jsons_in_dir.iter().find(|j| j.eq_ignore_ascii_case(&want)) {
                    return Some(hit.clone());
                }
            }
        }
    }

    // Pass 3: Google truncated the sidecar name. Prefer the longest prefix so a
    // clipped `IMG_0001.jpg.supplemental-me.json` never loses to a shorter,
    // vaguer match.
    let mut best: Option<(usize, &String)> = None;
    for root in &roots {
        for base in base_candidates(root) {
            let full = format!("{base}.supplemental-metadata");
            let full_lower = full.to_lowercase();
            for j in jsons_in_dir {
                let stem = match j.strip_suffix(".json") {
                    Some(s) => s,
                    None => continue,
                };
                if stem.len() < MIN_TRUNCATED_PREFIX || stem.len() >= full.len() {
                    continue;
                }
                if !full_lower.starts_with(&stem.to_lowercase()) {
                    continue;
                }
                if best.is_none_or(|(len, _)| stem.len() > len) {
                    best = Some((stem.len(), j));
                }
            }
        }
        if let Some((_, hit)) = best {
            return Some(hit.clone());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Container sniffing
// ---------------------------------------------------------------------------

/// Detect the real container from magic bytes. Returns the canonical extension
/// and whether this block can write EXIF into it.
fn sniff(bytes: &[u8]) -> Option<(&'static str, bool)> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some(("jpg", true));
    }
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some(("png", true));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Some(("gif", false));
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(("webp", false));
    }
    if bytes.len() > 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"AVI " {
        return Some(("avi", false));
    }
    if bytes.len() > 12 && &bytes[4..8] == b"ftyp" {
        let brand = &bytes[8..12];
        if brand.starts_with(b"heic") || brand.starts_with(b"heix") || brand.starts_with(b"mif1") {
            return Some(("heic", false));
        }
        if brand.starts_with(b"qt") {
            return Some(("mov", false));
        }
        return Some(("mp4", false));
    }
    if bytes.starts_with(b"II*\0") || bytes.starts_with(b"MM\0*") {
        return Some(("tif", false));
    }
    None
}

/// Extensions that mean "this is media", used only to classify leftovers in
/// the report (the merge itself always trusts the magic bytes).
fn looks_like_media(name: &str) -> bool {
    const EXTS: [&str; 14] = [
        "jpg", "jpeg", "png", "gif", "webp", "heic", "heif", "tif", "tiff", "mp4", "mov", "avi",
        "m4v", "mkv",
    ];
    let (_, ext) = split_ext(name);
    ext.is_some_and(|e| EXTS.contains(&e.to_ascii_lowercase().as_str()))
}

// ---------------------------------------------------------------------------
// EXIF presence probe
// ---------------------------------------------------------------------------

/// Which of the three groups the photo ALREADY carries.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Present {
    date: bool,
    gps: bool,
    description: bool,
}

/// Probe the existing EXIF. An unreadable/absent block simply means "nothing
/// is present" — the write path rebuilds it from scratch anyway.
fn probe(bytes: &[u8]) -> Present {
    let exif = match exif::Reader::new().read_from_container(&mut Cursor::new(bytes)) {
        Ok(e) => e,
        Err(_) => return Present::default(),
    };
    let has = |t: Tag| exif.get_field(t, In::PRIMARY).is_some();
    Present {
        date: has(Tag::DateTimeOriginal) || has(Tag::DateTime),
        gps: has(Tag::GPSLatitude) && has(Tag::GPSLongitude),
        description: has(Tag::ImageDescription),
    }
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

/// Build the `Edits` for one photo: the requested groups, minus anything the
/// photo already has (unless `overwrite`), minus anything the sidecar lacks.
fn plan_edits(sc: &Sidecar, present: Present, opts: &Options) -> Result<(Edits, Vec<String>), String> {
    let mut edits = Edits::default();
    let mut written: Vec<String> = Vec::new();

    for group in &opts.fields {
        match group.as_str() {
            "date" => {
                if present.date && !opts.overwrite {
                    continue;
                }
                if let Some(ts) = sc.timestamp(opts.date_source) {
                    edits.date_taken = Some(parse_date(&exif_datetime_from_epoch(ts)?)?);
                    written.push("date".into());
                }
            }
            "gps" => {
                if present.gps && !opts.overwrite {
                    continue;
                }
                if let Some(g) = sc.position(opts.gps_source) {
                    edits.latitude = Some(g.latitude);
                    edits.longitude = Some(g.longitude);
                    edits.altitude = g.altitude;
                    written.push("gps".into());
                }
            }
            "description" => {
                if present.description && !opts.overwrite {
                    continue;
                }
                if let Some(d) = &sc.description {
                    edits.description = Some(d.clone());
                    written.push("description".into());
                }
            }
            _ => {}
        }
    }
    Ok((edits, written))
}

/// Strip absolute paths and `..` traversal from an archive member name.
fn safe_path(raw: &str) -> Option<String> {
    let normalized = raw.replace('\\', "/");
    let cleaned: Vec<&str> = normalized
        .split('/')
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .collect();
    if cleaned.is_empty() {
        return None;
    }
    Some(cleaned.join("/"))
}

/// Rename `name` so its extension matches `real_ext`, if it does not already.
fn retitle(name: &str, real_ext: &str) -> Option<String> {
    let (stem, ext) = split_ext(name);
    let cur = ext?.to_ascii_lowercase();
    // `.jpeg` and `.jpg` are the same container; leave the user's spelling be.
    let same = cur == real_ext || (real_ext == "jpg" && cur == "jpeg") || (real_ext == "tif" && cur == "tiff");
    if same {
        return None;
    }
    Some(format!("{stem}.{real_ext}"))
}

/// Merge one Takeout ZIP. Returns the output ZIP (`None` for a dry run) plus
/// the report.
pub fn merge_zip(input: &[u8], opts: &Options) -> Result<(Option<Vec<u8>>, MergeReport), String> {
    if opts.fields.is_empty() {
        return Err(format!(
            "fields is empty: pass at least one of {}",
            FIELD_GROUPS.join(", ")
        ));
    }

    // --- read every member -------------------------------------------------
    let mut archive = zip::ZipArchive::new(Cursor::new(input))
        .map_err(|e| format!("not a valid ZIP archive: {e} — export the album from Google Takeout and upload that .zip"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!("archive has too many entries (> {MAX_ENTRIES})"));
    }

    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    let mut total: u64 = 0;
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| format!("failed to read zip entry {i}: {e}"))?;
        if entry.is_dir() {
            continue;
        }
        total = total.saturating_add(entry.size());
        if total > MAX_TOTAL_BYTES {
            return Err("archive contents are too large to expand".into());
        }
        let raw = entry.name().to_string();
        let Some(path) = safe_path(&raw) else { continue };
        let mut buf = Vec::with_capacity(entry.size().min(64 * 1024 * 1024) as usize);
        entry
            .read_to_end(&mut buf)
            .map_err(|e| format!("failed to extract '{raw}': {e}"))?;
        files.push((path, buf));
    }
    if files.is_empty() {
        return Err("the ZIP contains no files".into());
    }

    // --- index the sidecars per directory ----------------------------------
    let mut jsons_by_dir: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (path, _) in &files {
        if path.to_ascii_lowercase().ends_with(".json") {
            let (dir, name) = split_dir(path);
            jsons_by_dir.entry(dir.to_string()).or_default().push(name.to_string());
        }
    }

    let mut report = MergeReport {
        dry_run: opts.dry_run,
        ..Default::default()
    };
    let mut used_sidecars: Vec<String> = Vec::new();
    // (out_path, bytes, mtime epoch) — collected first so the report is
    // complete before any ZIP is built.
    let mut out_files: Vec<(String, Vec<u8>, Option<i64>)> = Vec::new();

    for (path, bytes) in &files {
        let (dir, name) = split_dir(path);
        let is_json = path.to_ascii_lowercase().ends_with(".json");
        if is_json {
            report.sidecars_found += 1;
            continue; // classified in a second pass, once pairing is known
        }

        let sniffed = sniff(bytes);
        let media_like = sniffed.is_some() || looks_like_media(name);
        if !media_like {
            report.files.push(FileOutcome {
                path: path.clone(),
                out_path: path.clone(),
                status: Status::Other,
                sidecar: None,
                fields_written: Vec::new(),
                note: None,
            });
            out_files.push((path.clone(), bytes.clone(), None));
            continue;
        }
        report.media_total += 1;

        // Extension fix is independent of whether EXIF can be written.
        let mut out_path = path.clone();
        let mut note: Option<String> = None;
        if opts.fix_extension {
            if let Some((real_ext, _)) = sniffed {
                if let Some(fixed) = retitle(name, real_ext) {
                    out_path = format!("{dir}{fixed}");
                    report.extensions_fixed += 1;
                    note = Some(format!("renamed to .{real_ext} (contents are {real_ext}, not {})",
                        split_ext(name).1.unwrap_or("?")));
                }
            }
        }

        let empty: Vec<String> = Vec::new();
        let jsons = jsons_by_dir.get(dir).unwrap_or(&empty);
        let sidecar_name = find_sidecar(name, jsons);
        let sidecar_path = sidecar_name.as_ref().map(|s| format!("{dir}{s}"));

        let Some(sc_path) = sidecar_path.clone() else {
            report.no_sidecar += 1;
            report.files.push(FileOutcome {
                path: path.clone(),
                out_path: out_path.clone(),
                status: Status::NoSidecar,
                sidecar: None,
                fields_written: Vec::new(),
                note: note.or(Some(
                    "no matching .json in this folder — if the export is split across several \
                     ZIPs, extract them into one folder and re-zip"
                        .into(),
                )),
            });
            out_files.push((out_path, bytes.clone(), None));
            continue;
        };
        if !used_sidecars.contains(&sc_path) {
            used_sidecars.push(sc_path.clone());
        }

        let sc_bytes = files
            .iter()
            .find(|(p, _)| *p == sc_path)
            .map(|(_, b)| b)
            .expect("sidecar path came from the file list");
        let sc = match parse_sidecar(sc_bytes) {
            Ok(sc) => sc,
            Err(e) => {
                report.failed += 1;
                report.files.push(FileOutcome {
                    path: path.clone(),
                    out_path: out_path.clone(),
                    status: Status::Failed,
                    sidecar: Some(sc_path),
                    fields_written: Vec::new(),
                    note: Some(e),
                });
                out_files.push((out_path, bytes.clone(), None));
                continue;
            }
        };

        // The capture time is used for the file timestamp even when EXIF
        // cannot be written — that alone fixes sorting for HEIC/MP4.
        let mtime = if opts.set_file_times {
            sc.timestamp(opts.date_source)
        } else {
            None
        };

        let writable = sniffed.map(|(_, w)| w).unwrap_or(false);
        if !writable {
            report.skipped_unsupported += 1;
            let kind = sniffed.map(|(e, _)| e).unwrap_or("unknown");
            report.files.push(FileOutcome {
                path: path.clone(),
                out_path: out_path.clone(),
                status: Status::SkippedUnsupported,
                sidecar: Some(sc_path),
                fields_written: Vec::new(),
                note: Some(format!(
                    "{kind} cannot carry EXIF — copied through unchanged{}",
                    if mtime.is_some() { ", file timestamp set" } else { "" }
                )),
            });
            out_files.push((out_path, bytes.clone(), mtime));
            continue;
        }

        let present = probe(bytes);
        let (edits, written) = match plan_edits(&sc, present, opts) {
            Ok(v) => v,
            Err(e) => {
                report.failed += 1;
                report.files.push(FileOutcome {
                    path: path.clone(),
                    out_path: out_path.clone(),
                    status: Status::Failed,
                    sidecar: Some(sc_path),
                    fields_written: Vec::new(),
                    note: Some(e),
                });
                out_files.push((out_path, bytes.clone(), mtime));
                continue;
            }
        };

        if written.is_empty() {
            report.already_complete += 1;
            report.files.push(FileOutcome {
                path: path.clone(),
                out_path: out_path.clone(),
                status: Status::AlreadyComplete,
                sidecar: Some(sc_path),
                fields_written: Vec::new(),
                note: note.or(Some(
                    "nothing to add: the photo already has these fields (pass overwrite=true to \
                     replace them) or the sidecar has none"
                        .into(),
                )),
            });
            out_files.push((out_path, bytes.clone(), mtime));
            continue;
        }

        if opts.dry_run {
            report.merged += 1;
            report.files.push(FileOutcome {
                path: path.clone(),
                out_path: out_path.clone(),
                status: Status::Merged,
                sidecar: Some(sc_path),
                fields_written: written,
                note,
            });
            continue;
        }

        match edit(bytes, &edits) {
            Ok((out_bytes, _)) => {
                report.merged += 1;
                report.files.push(FileOutcome {
                    path: path.clone(),
                    out_path: out_path.clone(),
                    status: Status::Merged,
                    sidecar: Some(sc_path),
                    fields_written: written,
                    note,
                });
                out_files.push((out_path, out_bytes, mtime));
            }
            Err(e) => {
                report.failed += 1;
                report.files.push(FileOutcome {
                    path: path.clone(),
                    out_path: out_path.clone(),
                    status: Status::Failed,
                    sidecar: Some(sc_path),
                    fields_written: Vec::new(),
                    note: Some(e),
                });
                out_files.push((out_path, bytes.clone(), mtime));
            }
        }
    }

    // --- second pass: the sidecars themselves ------------------------------
    for (path, bytes) in &files {
        if !path.to_ascii_lowercase().ends_with(".json") {
            continue;
        }
        let used = used_sidecars.contains(path);
        if opts.keep_sidecars {
            out_files.push((path.clone(), bytes.clone(), None));
        } else if used {
            report.sidecars_dropped += 1;
        } else {
            // An unpaired .json is not necessarily a sidecar (album metadata,
            // print-subscriptions.json …) — never delete what we did not use.
            out_files.push((path.clone(), bytes.clone(), None));
        }
        report.files.push(FileOutcome {
            path: path.clone(),
            out_path: path.clone(),
            status: if used { Status::Sidecar } else { Status::Other },
            sidecar: None,
            fields_written: Vec::new(),
            note: if used && !opts.keep_sidecars {
                Some("merged into its photo and dropped from the result".into())
            } else {
                None
            },
        });
    }

    report.files.sort_by(|a, b| a.path.cmp(&b.path));

    if opts.dry_run {
        return Ok((None, report));
    }
    let zip = write_zip(&out_files, opts.set_file_times)?;
    Ok((Some(zip), report))
}

/// Repack the merged files into one deflate ZIP, stamping capture times.
fn write_zip(
    out_files: &[(String, Vec<u8>, Option<i64>)],
    set_times: bool,
) -> Result<Vec<u8>, String> {
    let mut buf: Vec<u8> = Vec::new();
    {
        let mut zw = ZipWriter::new(Cursor::new(&mut buf));
        let base = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, bytes, mtime) in out_files {
            let mut opts = base;
            if set_times {
                if let Some(ts) = mtime {
                    // The ZIP DOS timestamp cannot represent anything before
                    // 1980; older photos just keep the default stamp.
                    if let Ok((y, mo, d, h, mi, s)) = civil_from_epoch(*ts) {
                        if (1980..=2107).contains(&y) {
                            if let Ok(dt) = zip::DateTime::from_date_and_time(
                                y as u16, mo as u8, d as u8, h as u8, mi as u8, s as u8,
                            ) {
                                opts = opts.last_modified_time(dt);
                            }
                        }
                    }
                }
            }
            zw.start_file(name.as_str(), opts)
                .map_err(|e| format!("zip start_file '{name}': {e}"))?;
            zw.write_all(bytes)
                .map_err(|e| format!("zip write '{name}': {e}"))?;
        }
        zw.finish().map_err(|e| format!("zip finish: {e}"))?;
    }
    Ok(buf)
}

/// Render the report as the plain-text summary shown in chat and on the CLI.
pub fn render_report(report: &MergeReport, max_rows: usize) -> String {
    let mut out = String::new();
    let verb = if report.dry_run { "would merge" } else { "merged" };
    out.push_str(&format!(
        "{} {} of {} media file(s); {} already complete, {} without a sidecar, \
         {} unsupported container(s), {} failed",
        verb,
        report.merged,
        report.media_total,
        report.already_complete,
        report.no_sidecar,
        report.skipped_unsupported,
        report.failed,
    ));
    if report.extensions_fixed > 0 {
        out.push_str(&format!("; {} extension(s) corrected", report.extensions_fixed));
    }
    if report.sidecars_dropped > 0 {
        out.push_str(&format!("; {} sidecar(s) dropped", report.sidecars_dropped));
    }
    out.push('\n');

    let rows: Vec<&FileOutcome> = report
        .files
        .iter()
        .filter(|f| !matches!(f.status, Status::Other | Status::Sidecar))
        .collect();
    for f in rows.iter().take(max_rows) {
        out.push_str(&format!("  {} [{}]", f.path, f.status.label()));
        if !f.fields_written.is_empty() {
            out.push_str(&format!(" {}", f.fields_written.join("+")));
        }
        if let Some(n) = &f.note {
            out.push_str(&format!(" — {n}"));
        }
        out.push('\n');
    }
    if rows.len() > max_rows {
        out.push_str(&format!("  … and {} more\n", rows.len() - max_rows));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- time ------------------------------------------------------------

    #[test]
    fn epoch_to_exif_datetime_is_utc() {
        assert_eq!(exif_datetime_from_epoch(0).unwrap(), "1970:01:01 00:00:00");
        // 2024-06-01T14:30:00Z
        assert_eq!(
            exif_datetime_from_epoch(1_717_252_200).unwrap(),
            "2024:06:01 14:30:00"
        );
        // A leap day, to exercise the civil-from-days branch.
        assert_eq!(
            exif_datetime_from_epoch(1_709_208_000).unwrap(),
            "2024:02:29 12:00:00"
        );
    }

    #[test]
    fn epoch_outside_exif_range_is_rejected() {
        let err = exif_datetime_from_epoch(300_000_000_000).unwrap_err();
        assert!(err.contains("outside the range"), "{err}");
    }

    // -- option parsing --------------------------------------------------

    #[test]
    fn parse_fields_accepts_lists_and_all() {
        assert_eq!(parse_fields("date, gps").unwrap(), vec!["date", "gps"]);
        assert_eq!(parse_fields("all").unwrap(), FIELD_GROUPS.to_vec());
        assert_eq!(parse_fields("GPS,gps").unwrap(), vec!["gps"]);
    }

    #[test]
    fn parse_fields_rejects_unknown_group() {
        let err = parse_fields("date,people").unwrap_err();
        assert!(err.contains("unknown field group 'people'"), "{err}");
    }

    #[test]
    fn parse_sources_round_trip_and_reject() {
        assert_eq!(parse_gps_source("geo_data_exif").unwrap(), GpsSource::GeoDataExif);
        assert_eq!(parse_date_source("creation").unwrap(), DateSource::Creation);
        assert!(parse_gps_source("gps").is_err());
        assert!(parse_date_source("modified").is_err());
    }

    // -- sidecar parsing -------------------------------------------------

    fn sidecar_json() -> Vec<u8> {
        br#"{
          "title": "IMG_0001.jpg",
          "description": "Sunset over the harbor",
          "photoTakenTime": { "timestamp": "1717252200", "formatted": "Jun 1, 2024" },
          "creationTime":   { "timestamp": "1717500000", "formatted": "Jun 4, 2024" },
          "geoData":     { "latitude": 48.8584, "longitude": 2.2945, "altitude": 35.0 },
          "geoDataExif": { "latitude": 0.0, "longitude": 0.0, "altitude": 0.0 }
        }"#
        .to_vec()
    }

    #[test]
    fn parses_a_real_shaped_sidecar() {
        let sc = parse_sidecar(&sidecar_json()).unwrap();
        assert_eq!(sc.photo_taken, Some(1_717_252_200));
        assert_eq!(sc.creation, Some(1_717_500_000));
        assert_eq!(sc.description.as_deref(), Some("Sunset over the harbor"));
        let g = sc.geo.unwrap();
        assert_eq!((g.latitude, g.longitude, g.altitude), (48.8584, 2.2945, Some(35.0)));
        // all-zero geoDataExif means "unknown", not the null island.
        assert_eq!(sc.geo_exif, None);
    }

    #[test]
    fn sidecar_sources_follow_the_policy() {
        let sc = parse_sidecar(&sidecar_json()).unwrap();
        assert_eq!(sc.timestamp(DateSource::Auto), Some(1_717_252_200));
        assert_eq!(sc.timestamp(DateSource::Creation), Some(1_717_500_000));
        assert!(sc.position(GpsSource::Auto).is_some());
        assert_eq!(sc.position(GpsSource::GeoDataExif), None);
    }

    #[test]
    fn malformed_sidecar_is_an_error() {
        let err = parse_sidecar(b"{not json").unwrap_err();
        assert!(err.contains("not valid JSON"), "{err}");
    }

    // -- pairing ---------------------------------------------------------

    fn jsons(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn pairs_the_modern_supplemental_metadata_name() {
        let js = jsons(&["IMG_0001.jpg.supplemental-metadata.json"]);
        assert_eq!(
            find_sidecar("IMG_0001.jpg", &js).as_deref(),
            Some("IMG_0001.jpg.supplemental-metadata.json")
        );
    }

    #[test]
    fn pairs_the_legacy_and_bare_stem_names() {
        assert_eq!(
            find_sidecar("IMG_0002.jpg", &jsons(&["IMG_0002.jpg.json"])).as_deref(),
            Some("IMG_0002.jpg.json")
        );
        assert_eq!(
            find_sidecar("IMG_0003.jpg", &jsons(&["IMG_0003.json"])).as_deref(),
            Some("IMG_0003.json")
        );
    }

    #[test]
    fn pairs_a_truncated_sidecar_name() {
        let js = jsons(&["a-very-long-holiday-photo-name.jpg.supplemental-me.json"]);
        assert_eq!(
            find_sidecar("a-very-long-holiday-photo-name.jpg", &js).as_deref(),
            Some("a-very-long-holiday-photo-name.jpg.supplemental-me.json")
        );
    }

    #[test]
    fn pairs_the_swapped_duplicate_counter() {
        // Google names the pair IMG_0004(1).jpg ↔ IMG_0004.jpg(1).json.
        let js = jsons(&["IMG_0004.jpg(1).json", "IMG_0004.jpg.json"]);
        assert_eq!(
            find_sidecar("IMG_0004(1).jpg", &js).as_deref(),
            Some("IMG_0004.jpg(1).json")
        );
        assert_eq!(
            find_sidecar("IMG_0004.jpg", &js).as_deref(),
            Some("IMG_0004.jpg.json")
        );
    }

    #[test]
    fn edited_copies_fall_back_to_the_originals_sidecar() {
        let js = jsons(&["IMG_0005.jpg.supplemental-metadata.json"]);
        assert_eq!(
            find_sidecar("IMG_0005-edited.jpg", &js).as_deref(),
            Some("IMG_0005.jpg.supplemental-metadata.json")
        );
    }

    #[test]
    fn unmatched_photo_gets_no_sidecar() {
        assert_eq!(find_sidecar("IMG_9999.jpg", &jsons(&["IMG_0001.jpg.json"])), None);
    }

    // -- container sniffing ----------------------------------------------

    #[test]
    fn sniff_detects_writable_and_unwritable_containers() {
        assert_eq!(sniff(&[0xFF, 0xD8, 0xFF, 0xE0]), Some(("jpg", true)));
        assert_eq!(sniff(b"\x89PNG\r\n\x1a\n....").map(|(e, _)| e), Some("png"));
        assert_eq!(sniff(b"GIF89a...."), Some(("gif", false)));
        assert_eq!(sniff(b"\0\0\0\x18ftypheic...."), Some(("heic", false)));
        assert_eq!(sniff(b"not an image at all"), None);
    }

    #[test]
    fn retitle_only_renames_a_real_mismatch() {
        assert_eq!(retitle("photo.jpg", "png").as_deref(), Some("photo.png"));
        assert_eq!(retitle("photo.jpeg", "jpg"), None);
        assert_eq!(retitle("photo.JPG", "jpg"), None);
    }

    // -- end-to-end over a real ZIP --------------------------------------

    /// SOI + APP0 (JFIF) + SOS + entropy data + EOI — the same minimal framing
    /// exif-edit's own tests use. Enough for img-parts to round-trip a splice
    /// repeatedly; the metadata path never decodes pixels.
    fn tiny_jpeg() -> Vec<u8> {
        vec![
            0xFF, 0xD8, // SOI
            0xFF, 0xE0, 0x00, 0x10, // APP0, len 16
            b'J', b'F', b'I', b'F', 0x00, 0x01, 0x01, 0x00, 0x00, 0x01, 0x00, 0x01, 0x00, 0x00,
            0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, // SOS
            0x12, 0x34, // entropy-coded scan data
            0xFF, 0xD9, // EOI
        ]
    }

    /// A 1x1 PNG with a real IDAT, so the container survives an EXIF splice.
    fn tiny_png() -> Vec<u8> {
        vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // signature
            0x00, 0x00, 0x00, 0x0D, // IHDR len
            b'I', b'H', b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06,
            0x00, 0x00, 0x00, 0x1F, 0x15, 0xC4, 0x89, // IHDR data + crc
            0x00, 0x00, 0x00, 0x0A, // IDAT len
            b'I', b'D', b'A', b'T', 0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01,
            0x0D, 0x0A, 0x2D, 0xB4, // IDAT data + crc
            0x00, 0x00, 0x00, 0x00, // IEND len
            b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82, // IEND + crc
        ]
    }

    fn build_zip(entries: &[(&str, Vec<u8>)]) -> Vec<u8> {
        let mut buf = Vec::new();
        {
            let mut zw = ZipWriter::new(Cursor::new(&mut buf));
            let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
            for (name, bytes) in entries {
                zw.start_file(*name, opts).unwrap();
                zw.write_all(bytes).unwrap();
            }
            zw.finish().unwrap();
        }
        buf
    }

    fn read_zip(bytes: &[u8]) -> BTreeMap<String, Vec<u8>> {
        let mut a = zip::ZipArchive::new(Cursor::new(bytes)).expect("valid zip");
        let mut out = BTreeMap::new();
        for i in 0..a.len() {
            let mut f = a.by_index(i).unwrap();
            if f.is_dir() {
                continue;
            }
            let name = f.name().to_string();
            let mut b = Vec::new();
            f.read_to_end(&mut b).unwrap();
            out.insert(name, b);
        }
        out
    }

    fn album() -> Vec<u8> {
        build_zip(&[
            ("Album/IMG_0001.jpg", tiny_jpeg()),
            ("Album/IMG_0001.jpg.supplemental-metadata.json", sidecar_json()),
            ("Album/clip.mp4", b"\0\0\0\x18ftypmp42 padding".to_vec()),
            ("Album/clip.mp4.json", sidecar_json()),
            ("Album/orphan.jpg", tiny_jpeg()),
            ("Album/print-subscriptions.json", b"{\"x\":1}".to_vec()),
        ])
    }

    #[test]
    fn merges_writes_exif_and_drops_used_sidecars() {
        let (zip, report) = merge_zip(&album(), &Options::default()).unwrap();
        let zip = zip.expect("not a dry run");

        assert_eq!(report.media_total, 3);
        assert_eq!(report.merged, 1);
        assert_eq!(report.skipped_unsupported, 1, "mp4 cannot carry EXIF");
        assert_eq!(report.no_sidecar, 1, "orphan.jpg has no sidecar");
        assert_eq!(report.failed, 0);
        assert_eq!(report.sidecars_dropped, 2);

        let merged = report
            .files
            .iter()
            .find(|f| f.path == "Album/IMG_0001.jpg")
            .unwrap();
        assert_eq!(merged.fields_written, vec!["date", "gps", "description"]);

        let out = read_zip(&zip);
        // The two consumed sidecars are gone; the unrelated JSON survives.
        assert!(!out.contains_key("Album/IMG_0001.jpg.supplemental-metadata.json"));
        assert!(!out.contains_key("Album/clip.mp4.json"));
        assert!(out.contains_key("Album/print-subscriptions.json"));
        assert!(out.contains_key("Album/clip.mp4"));
        assert!(out.contains_key("Album/orphan.jpg"));

        // The merged photo really carries the sidecar's EXIF now.
        let photo = &out["Album/IMG_0001.jpg"];
        let exif = exif::Reader::new()
            .read_from_container(&mut Cursor::new(photo))
            .expect("output has a readable EXIF block");
        let date = exif.get_field(Tag::DateTimeOriginal, In::PRIMARY).unwrap();
        assert_eq!(date.display_value().to_string(), "2024-06-01 14:30:00");
        assert!(exif.get_field(Tag::GPSLatitude, In::PRIMARY).is_some());
        let desc = exif.get_field(Tag::ImageDescription, In::PRIMARY).unwrap();
        assert!(
            desc.display_value().to_string().contains("Sunset over the harbor"),
            "{}",
            desc.display_value()
        );
    }

    #[test]
    fn dry_run_reports_without_producing_a_zip() {
        let opts = Options {
            dry_run: true,
            ..Default::default()
        };
        let (zip, report) = merge_zip(&album(), &opts).unwrap();
        assert!(zip.is_none());
        assert_eq!(report.merged, 1);
        assert!(report.dry_run);
        assert!(render_report(&report, 10).starts_with("would merge 1 of 3 media file(s)"));
    }

    #[test]
    fn fields_selection_limits_what_is_written() {
        let opts = Options {
            fields: vec!["gps".into()],
            ..Default::default()
        };
        let (zip, report) = merge_zip(&album(), &opts).unwrap();
        let out = read_zip(&zip.unwrap());
        let exif = exif::Reader::new()
            .read_from_container(&mut Cursor::new(&out["Album/IMG_0001.jpg"]))
            .unwrap();
        assert!(exif.get_field(Tag::GPSLatitude, In::PRIMARY).is_some());
        assert!(exif.get_field(Tag::DateTimeOriginal, In::PRIMARY).is_none());
        assert_eq!(report.files.iter().filter(|f| f.status == Status::Merged).count(), 1);
    }

    #[test]
    fn keep_sidecars_carries_the_json_through() {
        let opts = Options {
            keep_sidecars: true,
            ..Default::default()
        };
        let (zip, report) = merge_zip(&album(), &opts).unwrap();
        let out = read_zip(&zip.unwrap());
        assert!(out.contains_key("Album/IMG_0001.jpg.supplemental-metadata.json"));
        assert_eq!(report.sidecars_dropped, 0);
    }

    #[test]
    fn wrong_extension_is_corrected() {
        let zip_in = build_zip(&[
            ("Album/shot.jpg", tiny_png()),
            ("Album/shot.jpg.json", sidecar_json()),
        ]);
        let (zip, report) = merge_zip(&zip_in, &Options::default()).unwrap();
        assert_eq!(report.extensions_fixed, 1);
        let out = read_zip(&zip.unwrap());
        assert!(out.contains_key("Album/shot.png"));
        assert!(!out.contains_key("Album/shot.jpg"));
    }

    #[test]
    fn overwrite_off_keeps_an_existing_camera_date() {
        // Pre-stamp the photo with a real camera date, then merge.
        let mut pre = Edits::default();
        pre.date_taken = Some(parse_date("2001-09-09 01:46:40").unwrap());
        let (stamped, _) = edit(&tiny_jpeg(), &pre).unwrap();

        let zip_in = build_zip(&[
            ("Album/IMG_0001.jpg", stamped),
            ("Album/IMG_0001.jpg.json", sidecar_json()),
        ]);

        let (zip, report) = merge_zip(&zip_in, &Options::default()).unwrap();
        let row = &report.files.iter().find(|f| f.path == "Album/IMG_0001.jpg").unwrap();
        assert_eq!(row.fields_written, vec!["gps", "description"], "date left alone");
        let out = read_zip(&zip.unwrap());
        let exif = exif::Reader::new()
            .read_from_container(&mut Cursor::new(&out["Album/IMG_0001.jpg"]))
            .unwrap();
        assert_eq!(
            exif.get_field(Tag::DateTimeOriginal, In::PRIMARY)
                .unwrap()
                .display_value()
                .to_string(),
            "2001-09-09 01:46:40"
        );

        // …and overwrite=true replaces it with the sidecar's.
        let opts = Options { overwrite: true, ..Default::default() };
        let (zip, _) = merge_zip(&zip_in, &opts).unwrap();
        let out = read_zip(&zip.unwrap());
        let exif = exif::Reader::new()
            .read_from_container(&mut Cursor::new(&out["Album/IMG_0001.jpg"]))
            .unwrap();
        assert_eq!(
            exif.get_field(Tag::DateTimeOriginal, In::PRIMARY)
                .unwrap()
                .display_value()
                .to_string(),
            "2024-06-01 14:30:00"
        );
    }

    #[test]
    fn a_corrupt_sidecar_fails_only_that_photo() {
        let zip_in = build_zip(&[
            ("Album/IMG_0001.jpg", tiny_jpeg()),
            ("Album/IMG_0001.jpg.json", b"{oops".to_vec()),
            ("Album/IMG_0002.jpg", tiny_jpeg()),
            ("Album/IMG_0002.jpg.json", sidecar_json()),
        ]);
        let (zip, report) = merge_zip(&zip_in, &Options::default()).unwrap();
        assert_eq!(report.failed, 1);
        assert_eq!(report.merged, 1);
        // The failed photo is still in the output, byte-for-byte.
        let out = read_zip(&zip.unwrap());
        assert_eq!(out["Album/IMG_0001.jpg"], tiny_jpeg());
    }

    // -- errors ----------------------------------------------------------

    #[test]
    fn non_zip_input_is_a_clear_error() {
        let err = merge_zip(b"this is not a zip file", &Options::default()).unwrap_err();
        assert!(err.contains("not a valid ZIP archive"), "{err}");
    }

    #[test]
    fn empty_zip_is_an_error() {
        let err = merge_zip(&build_zip(&[]), &Options::default()).unwrap_err();
        assert!(err.contains("no files"), "{err}");
    }
}
