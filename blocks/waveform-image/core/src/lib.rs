//! gizza-ai/waveform-image core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Renders a static waveform PNG from an audio file via ffmpeg's
//! `showwavespic` filter (`-frames:v 1` — the filter emits a single frame).
//! With no `background` the PNG keeps showwavespic's native transparent RGBA
//! canvas; with a background hex the wave chain is wrapped in the classic
//! `color=c=…:s=WxH[bg];…[wave];[bg][wave]overlay` recipe (the size is known
//! up front, so no `scale2ref` is needed).
//!
//! When `split_channels` is off the audio is downmixed to mono first
//! (`aformat=channel_layouts=mono`) so a stereo file renders ONE clean wave
//! instead of two channels alpha-blended on top of each other; when on, the
//! downmix is skipped and `split_channels=1` stacks one lane per channel.
//!
//! Color strings are interpolated into `-filter_complex`, so they are
//! validated to strict `#RGB`/`#RRGGBB` hex — input hygiene that doubles as
//! filtergraph hardening (a stray `,`/`;`/`:` in a color would rewrite the
//! graph).

/// Default output width in pixels — the 1200×300 social-banner shape the
/// popular waveform generators standardized on.
pub const DEFAULT_WIDTH: u32 = 1200;
/// Default output height in pixels.
pub const DEFAULT_HEIGHT: u32 = 300;
/// Default wave color (the site accent).
pub const DEFAULT_COLOR: &str = "#4f46e5";

/// Inclusive dimension bounds. ffmpeg needs a few pixels to draw anything;
/// the caps keep the canvas (and the browser tab's memory) sane.
pub const MIN_DIM: u32 = 16;
pub const MAX_WIDTH: u32 = 4096;
pub const MAX_HEIGHT: u32 = 2048;

/// Amplitude scales showwavespic accepts. `lin` is the true waveform;
/// sqrt/cbrt/log progressively boost quiet material so it stays visible.
pub const SCALES: [&str; 4] = ["lin", "sqrt", "cbrt", "log"];

/// Validate a strict hex color: `#RGB` or `#RRGGBB`, case-insensitive.
/// Returns the trimmed value. Strictness doubles as filtergraph hardening.
fn parse_hex_color<'a>(value: &'a str, field: &str) -> Result<&'a str, String> {
    let v = value.trim();
    let ok = matches!(v.len(), 4 | 7)
        && v.starts_with('#')
        && v[1..].bytes().all(|b| b.is_ascii_hexdigit());
    if ok {
        Ok(v)
    } else {
        Err(format!(
            "{field} must be a hex color like #4f46e5 or #f00, got {value:?}"
        ))
    }
}

/// Round + range-check a dimension. `0` (an empty page field) selects the
/// default; anything else must land in `[MIN_DIM, max]` after rounding.
fn parse_dim(value: f64, default: u32, max: u32, field: &str) -> Result<u32, String> {
    if value == 0.0 {
        return Ok(default);
    }
    if !value.is_finite() {
        return Err(format!("{field} must be a number of pixels, got {value}"));
    }
    let rounded = value.round();
    if rounded < MIN_DIM as f64 || rounded > max as f64 {
        return Err(format!(
            "{field} must be between {MIN_DIM} and {max} pixels (0 or empty = {default}), got {value}"
        ));
    }
    Ok(rounded as u32)
}

/// Parse the amplitude scale. Empty defaults to `lin`.
pub fn parse_scale(s: &str) -> Result<&'static str, String> {
    let v = s.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok("lin");
    }
    SCALES
        .iter()
        .find(|k| **k == v)
        .copied()
        .ok_or_else(|| format!("scale {s:?} not supported (lin|sqrt|cbrt|log)"))
}

/// Build the `-filter_complex` graph string from validated parts.
fn build_graph(
    width: u32,
    height: u32,
    color: &str,
    background: Option<&str>,
    split_channels: bool,
    scale: &str,
) -> String {
    let wave = if split_channels {
        format!(
            "[0:a]showwavespic=s={width}x{height}:colors={color}:scale={scale}:split_channels=1"
        )
    } else {
        format!(
            "[0:a]aformat=channel_layouts=mono,showwavespic=s={width}x{height}:colors={color}:scale={scale}"
        )
    };
    match background {
        None => wave,
        Some(bg) => format!(
            "color=c={bg}:s={width}x{height}[bg];{wave}[wave];[bg][wave]overlay=format=auto"
        ),
    }
}

/// Validate params and return `(argv, out_name)` for an input file. Width and
/// height arrive as f64 (the page sends 0 for an empty field → default);
/// `color`/`background` are hex strings (empty color → default, empty
/// background → transparent); `scale` is lin|sqrt|cbrt|log (empty → lin).
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`). The output is always `out.png`.
pub fn plan_waveform_image(
    in_name: &str,
    width: f64,
    height: f64,
    color: &str,
    background: &str,
    split_channels: bool,
    scale: &str,
) -> Result<(Vec<String>, String), String> {
    let w = parse_dim(width, DEFAULT_WIDTH, MAX_WIDTH, "width")?;
    let h = parse_dim(height, DEFAULT_HEIGHT, MAX_HEIGHT, "height")?;
    let color = if color.trim().is_empty() {
        DEFAULT_COLOR
    } else {
        parse_hex_color(color, "color")?
    };
    let background = if background.trim().is_empty() {
        None
    } else {
        Some(parse_hex_color(background, "background")?)
    };
    let scale = parse_scale(scale)?;
    let graph = build_graph(w, h, color, background, split_channels, scale);
    let out_name = "out.png".to_string();
    // showwavespic emits exactly one frame; -frames:v 1 stops after it and
    // -update 1 tells the image2 muxer this is a single image, not a sequence.
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-filter_complex".to_string(),
        graph,
        "-frames:v".to_string(),
        "1".to_string(),
        "-update".to_string(),
        "1".to_string(),
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_argv_is_mono_transparent_1200x300_accent() {
        let (argv, out) = plan_waveform_image("in.mp3", 0.0, 0.0, "", "", false, "").unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp3",
                "-filter_complex",
                "[0:a]aformat=channel_layouts=mono,showwavespic=s=1200x300:colors=#4f46e5:scale=lin",
                "-frames:v",
                "1",
                "-update",
                "1",
                "out.png",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn background_wraps_wave_in_color_overlay_recipe() {
        let (argv, _) =
            plan_waveform_image("in.wav", 640.0, 200.0, "#FF0000", "#000000", false, "lin")
                .unwrap();
        assert_eq!(
            argv[3],
            "color=c=#000000:s=640x200[bg];\
             [0:a]aformat=channel_layouts=mono,showwavespic=s=640x200:colors=#FF0000:scale=lin[wave];\
             [bg][wave]overlay=format=auto"
        );
    }

    #[test]
    fn split_channels_skips_downmix_and_sets_the_option() {
        let (argv, _) = plan_waveform_image("in.mp3", 0.0, 0.0, "", "", true, "sqrt").unwrap();
        assert_eq!(
            argv[3],
            "[0:a]showwavespic=s=1200x300:colors=#4f46e5:scale=sqrt:split_channels=1"
        );
        assert!(!argv[3].contains("aformat"));
    }

    #[test]
    fn scale_variants_accepted_and_defaulted() {
        assert_eq!(parse_scale("").unwrap(), "lin");
        assert_eq!(parse_scale("LOG").unwrap(), "log");
        assert_eq!(parse_scale(" cbrt ").unwrap(), "cbrt");
        assert!(parse_scale("linear").is_err());
    }

    #[test]
    fn short_hex_and_mixed_case_are_accepted() {
        let (argv, _) =
            plan_waveform_image("a.mp3", 0.0, 0.0, "#F00", "#ABCdef", false, "").unwrap();
        assert!(argv[3].contains("colors=#F00"), "{}", argv[3]);
        assert!(argv[3].contains("color=c=#ABCdef"), "{}", argv[3]);
    }

    #[test]
    fn rejects_bad_hex_colors_with_field_name() {
        let err = plan_waveform_image("a.mp3", 0.0, 0.0, "red", "", false, "").unwrap_err();
        assert!(err.contains("color"), "{err}");
        assert!(err.contains("#4f46e5"), "{err}");
        let err = plan_waveform_image("a.mp3", 0.0, 0.0, "", "#12345", false, "").unwrap_err();
        assert!(err.contains("background"), "{err}");
    }

    #[test]
    fn hex_validation_blocks_filtergraph_injection() {
        // A crafted "color" must not be able to smuggle graph syntax through.
        for evil in ["#fff,anullsink", "#fff;movie=x", "#fff:s=9x9", "white"] {
            assert!(
                plan_waveform_image("a.mp3", 0.0, 0.0, evil, "", false, "").is_err(),
                "{evil} should be rejected as color"
            );
            assert!(
                plan_waveform_image("a.mp3", 0.0, 0.0, "", evil, false, "").is_err(),
                "{evil} should be rejected as background"
            );
        }
    }

    #[test]
    fn zero_or_empty_dimensions_take_the_defaults() {
        let (argv, _) = plan_waveform_image("a.mp3", 0.0, 150.0, "", "", false, "").unwrap();
        assert!(argv[3].contains("s=1200x150"), "{}", argv[3]);
        let (argv, _) = plan_waveform_image("a.mp3", 800.0, 0.0, "", "", false, "").unwrap();
        assert!(argv[3].contains("s=800x300"), "{}", argv[3]);
    }

    #[test]
    fn fractional_dimensions_are_rounded() {
        let (argv, _) = plan_waveform_image("a.mp3", 799.6, 200.4, "", "", false, "").unwrap();
        assert!(argv[3].contains("s=800x200"), "{}", argv[3]);
    }

    #[test]
    fn rejects_out_of_range_dimensions_with_bounds_in_message() {
        let err = plan_waveform_image("a.mp3", 4097.0, 300.0, "", "", false, "").unwrap_err();
        assert!(err.contains("width"), "{err}");
        assert!(err.contains("4096"), "{err}");
        let err = plan_waveform_image("a.mp3", 1200.0, 2049.0, "", "", false, "").unwrap_err();
        assert!(err.contains("height"), "{err}");
        assert!(err.contains("2048"), "{err}");
        let err = plan_waveform_image("a.mp3", 8.0, 300.0, "", "", false, "").unwrap_err();
        assert!(err.contains("16"), "{err}");
        assert!(plan_waveform_image("a.mp3", f64::NAN, 0.0, "", "", false, "").is_err());
        assert!(plan_waveform_image("a.mp3", -100.0, 0.0, "", "", false, "").is_err());
    }

    #[test]
    fn bounds_are_inclusive() {
        assert!(plan_waveform_image("a.mp3", 16.0, 16.0, "", "", false, "").is_ok());
        assert!(plan_waveform_image("a.mp3", 4096.0, 2048.0, "", "", false, "").is_ok());
    }

    #[test]
    fn out_name_is_always_png_regardless_of_input_ext() {
        for in_name in ["in.flac", "in.wav", "in.m4a"] {
            let (_, out) = plan_waveform_image(in_name, 0.0, 0.0, "", "", false, "").unwrap();
            assert_eq!(out, "out.png");
        }
    }
}
