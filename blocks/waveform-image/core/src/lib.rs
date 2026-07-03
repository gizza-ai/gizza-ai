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
//! validated to strict hex (`#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA`) — input
//! hygiene that doubles as filtergraph hardening (a stray `,`/`;`/`:` in a
//! color would rewrite the graph). Short forms are EXPANDED to the 6/8-digit
//! form before interpolation: ffmpeg's own color parser only understands
//! `#RRGGBB[AA]`, and silently falls back to white on a 3-digit hex.
//!
//! `color2` turns the wave into a horizontal left→right gradient: the wave is
//! drawn white, its alpha plane is extracted as a mask, and the mask is
//! `alphamerge`d onto a `gradients` source (`color` → `color2` across the
//! width). A background still wraps the result in the overlay recipe.

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

/// Per-column sampling modes showwavespic accepts. `average` draws the mean
/// amplitude; `peak` draws the loudest sample — a fuller wave that keeps
/// short hits visible.
pub const SAMPLINGS: [&str; 2] = ["average", "peak"];

/// Most channels a `color` list may address. ffmpeg itself defaults to a
/// 9-color cycle; 8 coves 7.1 audio and keeps the filtergraph bounded.
pub const MAX_COLORS: usize = 8;

/// Validate a strict hex color (`#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA`,
/// case-insensitive) and NORMALIZE it to the 6/8-digit form ffmpeg's color
/// parser understands (a 3-digit hex is silently dropped → white wave).
/// Strictness doubles as filtergraph hardening.
fn parse_hex_color(value: &str, field: &str) -> Result<String, String> {
    let v = value.trim();
    let digits = v.strip_prefix('#').unwrap_or("");
    let ok = matches!(digits.len(), 3 | 4 | 6 | 8)
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_hexdigit())
        && v.starts_with('#');
    if !ok {
        return Err(format!(
            "{field} must be a hex color like #4f46e5, #f00 or #00000080, got {value:?}"
        ));
    }
    if digits.len() <= 4 {
        // #RGB(A) → #RRGGBB(AA): double each digit.
        let mut out = String::with_capacity(1 + digits.len() * 2);
        out.push('#');
        for c in digits.chars() {
            out.push(c);
            out.push(c);
        }
        Ok(out)
    } else {
        Ok(v.to_string())
    }
}

/// Parse a comma-separated list of hex colors (for per-channel coloring with
/// `split_channels`) into showwavespic's pipe-joined `colors=` value. Empty →
/// the default accent. Each entry is validated + normalized.
fn parse_color_list(value: &str, field: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(DEFAULT_COLOR.to_string());
    }
    let parts: Vec<&str> = v.split(',').map(str::trim).collect();
    if parts.len() > MAX_COLORS {
        return Err(format!(
            "{field} lists {} colors — at most {MAX_COLORS} (one per channel) are supported",
            parts.len()
        ));
    }
    let mut colors = Vec::with_capacity(parts.len());
    for part in parts {
        colors.push(parse_hex_color(part, field)?);
    }
    Ok(colors.join("|"))
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

/// Parse the per-column sampling mode. Empty defaults to `average`.
pub fn parse_sampling(s: &str) -> Result<&'static str, String> {
    let v = s.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok("average");
    }
    SAMPLINGS
        .iter()
        .find(|k| **k == v)
        .copied()
        .ok_or_else(|| format!("sampling {s:?} not supported (average|peak)"))
}

/// Build the `-filter_complex` graph string from validated parts. `colors` is
/// the pipe-joined showwavespic value; `gradient` is `Some((from, to))` for a
/// horizontal gradient-filled wave; `background` wraps everything in the
/// color-source + overlay recipe.
fn build_graph(
    width: u32,
    height: u32,
    colors: &str,
    gradient: Option<(&str, &str)>,
    background: Option<&str>,
    split_channels: bool,
    scale: &str,
    sampling: &str,
) -> String {
    // `filter=peak` only when asked, so default graphs stay byte-identical
    // to the pre-sampling ones (argv stability across surfaces + tests).
    let peak = if sampling == "peak" { ":filter=peak" } else { "" };
    let wave_colors = if gradient.is_some() { "#ffffff" } else { colors };
    let wave = if split_channels {
        format!(
            "[0:a]showwavespic=s={width}x{height}:colors={wave_colors}:scale={scale}{peak}:split_channels=1"
        )
    } else {
        format!(
            "[0:a]aformat=channel_layouts=mono,showwavespic=s={width}x{height}:colors={wave_colors}:scale={scale}{peak}"
        )
    };
    // Gradient fill: draw the wave white, lift its alpha plane out as a mask,
    // and merge the mask onto a left→right two-color `gradients` source. The
    // gradient axis is pinned to the horizontal midline so it is purely
    // horizontal at any size.
    let (wave, wave_label) = match gradient {
        None => (wave, "[wave]"),
        Some((from, to)) => {
            let mid = height / 2;
            (
                format!(
                    "gradients=s={width}x{height}:c0={from}:c1={to}:x0=0:y0={mid}:x1={width}:y1={mid}[grad];\
                     {wave}[w];[w]alphaextract[mask];[grad][mask]alphamerge"
                ),
                "[gwave]",
            )
        }
    };
    match background {
        None => wave,
        Some(bg) => format!(
            "color=c={bg}:s={width}x{height}[bg];{wave}{wave_label};[bg]{wave_label}overlay=format=auto"
        ),
    }
}

/// Validate params and return `(argv, out_name)` for an input file. Width and
/// height arrive as f64 (the page sends 0 for an empty field → default);
/// `color` is one hex or a comma-separated per-channel list (empty → default);
/// `color2` is an optional gradient end color (empty → solid); `background`
/// is a hex (empty → transparent); `scale` is lin|sqrt|cbrt|log (empty → lin);
/// `sampling` is average|peak (empty → average). All hex colors accept
/// `#RGB`/`#RGBA`/`#RRGGBB`/`#RRGGBBAA` and are normalized for ffmpeg.
/// Single source shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`). The output is always `out.png`.
#[allow(clippy::too_many_arguments)] // mirrors the page field order 1:1
pub fn plan_waveform_image(
    in_name: &str,
    width: f64,
    height: f64,
    color: &str,
    color2: &str,
    background: &str,
    split_channels: bool,
    scale: &str,
    sampling: &str,
) -> Result<(Vec<String>, String), String> {
    let w = parse_dim(width, DEFAULT_WIDTH, MAX_WIDTH, "width")?;
    let h = parse_dim(height, DEFAULT_HEIGHT, MAX_HEIGHT, "height")?;
    let colors = parse_color_list(color, "color")?;
    let gradient_to = if color2.trim().is_empty() {
        None
    } else {
        Some(parse_hex_color(color2, "color2")?)
    };
    if gradient_to.is_some() && colors.contains('|') {
        return Err(
            "color2 draws a gradient from a single color — use one color, not a list, with color2"
                .to_string(),
        );
    }
    let background = if background.trim().is_empty() {
        None
    } else {
        Some(parse_hex_color(background, "background")?)
    };
    let scale = parse_scale(scale)?;
    let sampling = parse_sampling(sampling)?;
    let gradient = gradient_to.as_deref().map(|to| (colors.as_str(), to));
    let graph = build_graph(
        w,
        h,
        &colors,
        gradient,
        background.as_deref(),
        split_channels,
        scale,
        sampling,
    );
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

    /// `plan_waveform_image` with the two new params defaulted — keeps the
    /// pre-existing behavior tests readable.
    fn plan(
        in_name: &str,
        width: f64,
        height: f64,
        color: &str,
        background: &str,
        split_channels: bool,
        scale: &str,
    ) -> Result<(Vec<String>, String), String> {
        plan_waveform_image(
            in_name,
            width,
            height,
            color,
            "",
            background,
            split_channels,
            scale,
            "",
        )
    }

    #[test]
    fn default_argv_is_mono_transparent_1200x300_accent() {
        let (argv, out) = plan("in.mp3", 0.0, 0.0, "", "", false, "").unwrap();
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
        let (argv, _) = plan("in.wav", 640.0, 200.0, "#FF0000", "#000000", false, "lin").unwrap();
        assert_eq!(
            argv[3],
            "color=c=#000000:s=640x200[bg];\
             [0:a]aformat=channel_layouts=mono,showwavespic=s=640x200:colors=#FF0000:scale=lin[wave];\
             [bg][wave]overlay=format=auto"
        );
    }

    #[test]
    fn split_channels_skips_downmix_and_sets_the_option() {
        let (argv, _) = plan("in.mp3", 0.0, 0.0, "", "", true, "sqrt").unwrap();
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
    fn sampling_variants_accepted_and_defaulted() {
        assert_eq!(parse_sampling("").unwrap(), "average");
        assert_eq!(parse_sampling("PEAK").unwrap(), "peak");
        assert_eq!(parse_sampling(" average ").unwrap(), "average");
        assert!(parse_sampling("rms").is_err());
    }

    #[test]
    fn sampling_peak_adds_the_filter_option_average_stays_bare() {
        let (argv, _) =
            plan_waveform_image("a.mp3", 0.0, 0.0, "", "", "", false, "", "peak").unwrap();
        assert_eq!(
            argv[3],
            "[0:a]aformat=channel_layouts=mono,showwavespic=s=1200x300:colors=#4f46e5:scale=lin:filter=peak"
        );
        // Explicit "average" keeps the graph byte-identical to the default.
        let (avg, _) =
            plan_waveform_image("a.mp3", 0.0, 0.0, "", "", "", false, "", "average").unwrap();
        assert!(!avg[3].contains("filter="), "{}", avg[3]);
    }

    /// REGRESSION (found in the improve pass): ffmpeg's color parser only
    /// understands `#RRGGBB[AA]` — a raw `#f00` was warned about and silently
    /// replaced with WHITE. Short forms must be expanded before interpolation.
    #[test]
    fn three_and_four_digit_hex_expand_to_ffmpeg_form() {
        let (argv, _) = plan("a.mp3", 0.0, 0.0, "#f00", "#ABC", false, "").unwrap();
        assert!(argv[3].contains("colors=#ff0000"), "{}", argv[3]);
        assert!(argv[3].contains("color=c=#AABBCC"), "{}", argv[3]);
        let (argv, _) = plan("a.mp3", 0.0, 0.0, "#f008", "", false, "").unwrap();
        assert!(argv[3].contains("colors=#ff000088"), "{}", argv[3]);
    }

    #[test]
    fn alpha_hex_passes_through_for_translucent_scrims() {
        let (argv, _) = plan("a.mp3", 0.0, 0.0, "#4f46e5", "#00000080", false, "").unwrap();
        assert!(argv[3].contains("color=c=#00000080"), "{}", argv[3]);
    }

    #[test]
    fn six_and_eight_digit_hex_keep_their_case_and_value() {
        let (argv, _) = plan("a.mp3", 0.0, 0.0, "#ABCdef", "#0b1220", false, "").unwrap();
        assert!(argv[3].contains("colors=#ABCdef"), "{}", argv[3]);
        assert!(argv[3].contains("color=c=#0b1220"), "{}", argv[3]);
    }

    #[test]
    fn color_list_joins_with_pipe_for_per_channel_lanes() {
        let (argv, _) = plan("a.mp3", 0.0, 0.0, "#ff0000, #00f", "", true, "").unwrap();
        assert_eq!(
            argv[3],
            "[0:a]showwavespic=s=1200x300:colors=#ff0000|#0000ff:scale=lin:split_channels=1"
        );
    }

    #[test]
    fn color_list_rejects_bad_entries_and_oversized_lists() {
        let err = plan("a.mp3", 0.0, 0.0, "#ff0000,red", "", true, "").unwrap_err();
        assert!(err.contains("color"), "{err}");
        let nine = vec!["#ff0000"; 9].join(",");
        let err = plan("a.mp3", 0.0, 0.0, &nine, "", true, "").unwrap_err();
        assert!(err.contains("8"), "{err}");
    }

    #[test]
    fn gradient_wave_uses_the_alphamerge_recipe() {
        let (argv, _) =
            plan_waveform_image("a.mp3", 320.0, 100.0, "#ff0000", "#0000ff", "", false, "", "")
                .unwrap();
        assert_eq!(
            argv[3],
            "gradients=s=320x100:c0=#ff0000:c1=#0000ff:x0=0:y0=50:x1=320:y1=50[grad];\
             [0:a]aformat=channel_layouts=mono,showwavespic=s=320x100:colors=#ffffff:scale=lin[w];\
             [w]alphaextract[mask];[grad][mask]alphamerge"
        );
    }

    #[test]
    fn gradient_with_background_overlays_the_merged_wave() {
        let (argv, _) = plan_waveform_image(
            "a.mp3", 320.0, 100.0, "#f00", "#00f", "#000000", false, "", "",
        )
        .unwrap();
        assert_eq!(
            argv[3],
            "color=c=#000000:s=320x100[bg];\
             gradients=s=320x100:c0=#ff0000:c1=#0000ff:x0=0:y0=50:x1=320:y1=50[grad];\
             [0:a]aformat=channel_layouts=mono,showwavespic=s=320x100:colors=#ffffff:scale=lin[w];\
             [w]alphaextract[mask];[grad][mask]alphamerge[gwave];\
             [bg][gwave]overlay=format=auto"
        );
    }

    #[test]
    fn gradient_composes_with_split_channels_and_peak() {
        let (argv, _) = plan_waveform_image(
            "a.mp3", 0.0, 0.0, "#f00", "#00f", "", true, "sqrt", "peak",
        )
        .unwrap();
        assert!(argv[3].contains("split_channels=1"), "{}", argv[3]);
        assert!(argv[3].contains("filter=peak"), "{}", argv[3]);
        assert!(argv[3].contains("alphamerge"), "{}", argv[3]);
        assert!(argv[3].contains("colors=#ffffff"), "{}", argv[3]);
    }

    #[test]
    fn gradient_rejects_a_color_list_start() {
        let err = plan_waveform_image(
            "a.mp3", 0.0, 0.0, "#f00,#0f0", "#00f", "", false, "", "",
        )
        .unwrap_err();
        assert!(err.contains("single"), "{err}");
        // …and a bad color2 names its field.
        let err =
            plan_waveform_image("a.mp3", 0.0, 0.0, "#f00", "blue", "", false, "", "").unwrap_err();
        assert!(err.contains("color2"), "{err}");
    }

    #[test]
    fn rejects_bad_hex_colors_with_field_name() {
        let err = plan("a.mp3", 0.0, 0.0, "red", "", false, "").unwrap_err();
        assert!(err.contains("color"), "{err}");
        assert!(err.contains("#4f46e5"), "{err}");
        let err = plan("a.mp3", 0.0, 0.0, "", "#12345", false, "").unwrap_err();
        assert!(err.contains("background"), "{err}");
    }

    #[test]
    fn hex_validation_blocks_filtergraph_injection() {
        // A crafted color must not be able to smuggle graph syntax through —
        // in any color-bearing field (color splits on ',', each part strict).
        for evil in ["#fff;movie=x", "#fff:s=9x9", "white", "#fff|#000"] {
            assert!(
                plan("a.mp3", 0.0, 0.0, evil, "", false, "").is_err(),
                "{evil} should be rejected as color"
            );
            assert!(
                plan("a.mp3", 0.0, 0.0, "", evil, false, "").is_err(),
                "{evil} should be rejected as background"
            );
            assert!(
                plan_waveform_image("a.mp3", 0.0, 0.0, "#fff", evil, "", false, "", "").is_err(),
                "{evil} should be rejected as color2"
            );
        }
        // The comma is a list separator for color, but strict hex elsewhere.
        assert!(plan("a.mp3", 0.0, 0.0, "#fff,anullsink", "", false, "").is_err());
        assert!(plan("a.mp3", 0.0, 0.0, "", "#fff,#000", false, "").is_err());
    }

    #[test]
    fn zero_or_empty_dimensions_take_the_defaults() {
        let (argv, _) = plan("a.mp3", 0.0, 150.0, "", "", false, "").unwrap();
        assert!(argv[3].contains("s=1200x150"), "{}", argv[3]);
        let (argv, _) = plan("a.mp3", 800.0, 0.0, "", "", false, "").unwrap();
        assert!(argv[3].contains("s=800x300"), "{}", argv[3]);
    }

    #[test]
    fn fractional_dimensions_are_rounded() {
        let (argv, _) = plan("a.mp3", 799.6, 200.4, "", "", false, "").unwrap();
        assert!(argv[3].contains("s=800x200"), "{}", argv[3]);
    }

    #[test]
    fn rejects_out_of_range_dimensions_with_bounds_in_message() {
        let err = plan("a.mp3", 4097.0, 300.0, "", "", false, "").unwrap_err();
        assert!(err.contains("width"), "{err}");
        assert!(err.contains("4096"), "{err}");
        let err = plan("a.mp3", 1200.0, 2049.0, "", "", false, "").unwrap_err();
        assert!(err.contains("height"), "{err}");
        assert!(err.contains("2048"), "{err}");
        let err = plan("a.mp3", 8.0, 300.0, "", "", false, "").unwrap_err();
        assert!(err.contains("16"), "{err}");
        assert!(plan("a.mp3", f64::NAN, 0.0, "", "", false, "").is_err());
        assert!(plan("a.mp3", -100.0, 0.0, "", "", false, "").is_err());
    }

    #[test]
    fn bounds_are_inclusive() {
        assert!(plan("a.mp3", 16.0, 16.0, "", "", false, "").is_ok());
        assert!(plan("a.mp3", 4096.0, 2048.0, "", "", false, "").is_ok());
    }

    #[test]
    fn out_name_is_always_png_regardless_of_input_ext() {
        for in_name in ["in.flac", "in.wav", "in.m4a"] {
            let (_, out) = plan(in_name, 0.0, 0.0, "", "", false, "").unwrap();
            assert_eq!(out, "out.png");
        }
    }
}
