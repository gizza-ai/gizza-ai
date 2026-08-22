//! Browser-facing wasm-bindgen wrapper for /tools/music-file-renamer/.
//! The page hands every field over as a string in `page/meta.toml` order, so the
//! numeric and boolean options are parsed here and empty fields fall back to the
//! descriptor defaults.
use wasm_bindgen::prelude::*;

/// Parse a page number field, falling back to the descriptor default when the
/// field is blank. Out-of-range values are left to the core's own validation so
/// the user gets the real error message instead of a silent clamp.
fn num(raw: &str, default: i64) -> i64 {
    let t = raw.trim();
    if t.is_empty() {
        default
    } else {
        t.parse::<i64>().unwrap_or(default)
    }
}

/// A checkbox marshals as "true"/"false"; a blank value means the field was
/// never rendered, so fall back to the descriptor default.
fn flag(raw: &str, default: bool) -> bool {
    match raw.trim().to_ascii_lowercase().as_str() {
        "" => default,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    tracks: &str,
    input_format: &str,
    pattern: &str,
    base_dir: &str,
    track_padding: &str,
    on_missing: &str,
    unknown_text: &str,
    charset: &str,
    replace_char: &str,
    space_style: &str,
    case_style: &str,
    max_component: &str,
    keep_extension: &str,
    format: &str,
) -> Result<String, JsValue> {
    gizza_ai_music_file_renamer_core::run(
        tracks,
        input_format,
        pattern,
        base_dir,
        num(track_padding, 2),
        on_missing,
        unknown_text,
        charset,
        replace_char,
        space_style,
        case_style,
        num(max_component, 100),
        flag(keep_extension, true),
        format,
    )
    .map_err(|e| JsValue::from_str(&e))
}
