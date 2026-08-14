//! gizza-ai/image-dither core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Dithering trades colour depth for spatial detail: instead of snapping every
//! pixel to the nearest palette entry (which produces flat banding), the
//! quantization error is spread over neighbouring pixels so that a small palette
//! still reads as a continuous tone. That is what gives the retro / pixel-art /
//! e-ink look and what makes a 2-colour image of a photo legible at all.
//!
//! ffmpeg implements eight diffusion/ordered kernels inside `paletteuse`, so the
//! whole tool is one `-filter_complex` graph:
//!
//! ```text
//! [0:v] <contrast> , <downscale> , <gray?>            [b]
//! <palette source>                                    [p]
//! [b][p] paletteuse=dither=<algorithm>[:bayer_scale=N] , format=rgb24 , <upscale>
//! ```
//!
//! The palette branch is either derived from the image (`palettegen`) or built
//! from `color=` sources stacked into the 16x16 = 256-pixel image `paletteuse`
//! demands. Building fixed palettes in-graph (rather than as a second `-i`
//! input) is what keeps this working unchanged on the browser ffmpeg build,
//! which only ever gets the one uploaded file.

/// Diffusion / ordered kernels exposed by `paletteuse`, in page order.
pub const ALGORITHMS: [&str; 9] = [
    "floyd_steinberg",
    "bayer",
    "atkinson",
    "burkes",
    "sierra2",
    "sierra3",
    "sierra2_4a",
    "heckbert",
    "none",
];

/// Palette choices. `auto` derives one from the image; `custom` reads
/// `palette_colors`; everything else is a fixed ramp defined below.
pub const PALETTES: [&str; 8] = [
    "auto", "mono", "gray4", "gray16", "green4", "amber2", "cga4", "custom",
];

/// Output containers. `same` keeps the upload's own format.
pub const FORMATS: [&str; 5] = ["same", "png", "jpeg", "webp", "gif"];

/// Palette entries are RGB hex without the leading `#`.
/// A 4-shade green reflective-LCD ramp (dark → light).
const GREEN4: [&str; 4] = ["0f380f", "306230", "8bac0f", "9bbc0f"];
/// Amber-on-black terminal: two entries only.
const AMBER2: [&str; 2] = ["000000", "ffb000"];
/// The classic 4-colour CGA display palette (black, cyan, magenta, white).
const CGA4: [&str; 4] = ["000000", "55ffff", "ff55ff", "ffffff"];

/// Largest number of entries a fixed/custom palette may have. The palette image
/// is 16 rows tall and every entry needs at least one row.
pub const MAX_FIXED_PALETTE: usize = 16;
/// `paletteuse` requires exactly 256 palette pixels — 16x16.
const PALETTE_W: u32 = 16;
const PALETTE_ROWS: u32 = 16;

/// Palettes that describe a single-hue ramp. For these the image is converted to
/// luma first (`format=gray`) so the nearest-entry match follows brightness
/// instead of RGB distance — otherwise a saturated red would land on whichever
/// palette entry happens to be closest in the RGB cube, not the one that matches
/// how bright it looks.
fn is_monochromatic(palette: &str) -> bool {
    matches!(palette, "mono" | "gray4" | "gray16" | "green4" | "amber2")
}

/// The fixed entry list for a palette name, or `None` for `auto`/`custom`.
fn fixed_palette(palette: &str) -> Option<Vec<String>> {
    let v: Vec<String> = match palette {
        "mono" => vec!["000000".into(), "ffffff".into()],
        "gray4" => gray_ramp(4),
        "gray16" => gray_ramp(16),
        "green4" => GREEN4.iter().map(|s| s.to_string()).collect(),
        "amber2" => AMBER2.iter().map(|s| s.to_string()).collect(),
        "cga4" => CGA4.iter().map(|s| s.to_string()).collect(),
        _ => return None,
    };
    Some(v)
}

/// An evenly spaced grayscale ramp of `n` levels, black through white.
fn gray_ramp(n: usize) -> Vec<String> {
    (0..n)
        .map(|i| {
            let v = (i as f64 * 255.0 / (n - 1) as f64).round() as u8;
            format!("{v:02x}{v:02x}{v:02x}")
        })
        .collect()
}

/// Parse a comma/space separated custom palette into normalised 6-digit hex.
///
/// Accepts `#rgb`, `#rrggbb`, and the same without the `#`, so a list pasted
/// from anywhere works. Returns an actionable error rather than letting ffmpeg
/// fail with a filtergraph message the user can't map back to a field.
pub fn parse_palette_colors(spec: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in spec.split([',', ' ', '\n', '\t']) {
        let t = raw.trim().trim_start_matches('#');
        if t.is_empty() {
            continue;
        }
        let hex = match t.len() {
            3 => {
                let mut s = String::with_capacity(6);
                for c in t.chars() {
                    s.push(c);
                    s.push(c);
                }
                s
            }
            6 => t.to_string(),
            _ => {
                return Err(format!(
                    "palette_colors entry {raw:?} is not a hex colour — use #rgb or #rrggbb (e.g. #000000,#ffffff)"
                ))
            }
        };
        if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "palette_colors entry {raw:?} is not a hex colour — use #rgb or #rrggbb (e.g. #000000,#ffffff)"
            ));
        }
        out.push(hex.to_ascii_lowercase());
    }
    if out.len() < 2 {
        return Err(
            "palette_colors needs at least 2 colours, e.g. \"#1b1b1b,#e8e8e8\"".to_string(),
        );
    }
    if out.len() > MAX_FIXED_PALETTE {
        return Err(format!(
            "palette_colors accepts at most {MAX_FIXED_PALETTE} colours (got {})",
            out.len()
        ));
    }
    Ok(out)
}

/// Spread `PALETTE_ROWS` rows over `n` palette entries as evenly as possible.
/// The first `PALETTE_ROWS % n` entries get one extra row, so the total is
/// always exactly 16 rows => 16x16 => the 256 pixels `paletteuse` requires.
fn row_split(n: usize) -> Vec<u32> {
    let n = n as u32;
    let base = PALETTE_ROWS / n;
    let rem = PALETTE_ROWS % n;
    (0..n).map(|i| base + u32::from(i < rem)).collect()
}

/// Build the `color=`/`vstack` chain that materialises a fixed palette as a
/// 16x16 image on the `[p]` label.
fn fixed_palette_graph(colors: &[String]) -> String {
    let rows = row_split(colors.len());
    let mut g = String::new();
    for (i, (c, h)) in colors.iter().zip(rows.iter()).enumerate() {
        g.push_str(&format!(
            "color=c=0x{c}:s={PALETTE_W}x{h}[pc{i}];"
        ));
    }
    if colors.len() == 1 {
        // Unreachable via the public API (parse rejects <2), but keep the graph
        // total rather than emitting a 1-input vstack, which ffmpeg rejects.
        g.push_str("[pc0]null[p]");
        return g;
    }
    for i in 0..colors.len() {
        g.push_str(&format!("[pc{i}]"));
    }
    g.push_str(&format!("vstack=inputs={}[p]", colors.len()));
    g
}

/// Format an f64 without a trailing `.0` for whole numbers, so the filter string
/// and the unit tests stay readable (`1.4` stays `1.4`, `2.0` becomes `2`).
fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Everything the graph builder needs, validated.
struct Settings {
    algorithm: String,
    bayer_scale: u32,
    pixel_scale: u32,
    contrast: f64,
    palette: String,
    palette_colors: Vec<String>,
}

fn validate(
    algorithm: &str,
    palette: &str,
    colors: u32,
    palette_colors: &str,
    bayer_scale: u32,
    pixel_scale: u32,
    contrast: f64,
) -> Result<(Settings, u32), String> {
    let algorithm = algorithm.trim().to_ascii_lowercase();
    if !ALGORITHMS.contains(&algorithm.as_str()) {
        return Err(format!(
            "unknown algorithm {algorithm:?} — use one of: {}",
            ALGORITHMS.join(", ")
        ));
    }
    let palette = palette.trim().to_ascii_lowercase();
    if !PALETTES.contains(&palette.as_str()) {
        return Err(format!(
            "unknown palette {palette:?} — use one of: {}",
            PALETTES.join(", ")
        ));
    }
    if !(2..=256).contains(&colors) {
        return Err(format!("colors must be between 2 and 256 (got {colors})"));
    }
    if bayer_scale > 5 {
        return Err(format!(
            "bayer_scale must be between 0 and 5 (got {bayer_scale})"
        ));
    }
    if !(1..=16).contains(&pixel_scale) {
        return Err(format!(
            "pixel_scale must be between 1 and 16 (got {pixel_scale})"
        ));
    }
    if !(0.5..=3.0).contains(&contrast) {
        return Err(format!(
            "contrast must be between 0.5 and 3.0 (got {})",
            trim_num(contrast)
        ));
    }
    let palette_colors = match palette.as_str() {
        "custom" => parse_palette_colors(palette_colors)?,
        _ => fixed_palette(&palette).unwrap_or_default(),
    };
    Ok((
        Settings {
            algorithm,
            bayer_scale,
            pixel_scale,
            contrast,
            palette,
            palette_colors,
        },
        colors,
    ))
}

/// Build the `-filter_complex` graph string.
pub fn build_filter(
    algorithm: &str,
    palette: &str,
    colors: u32,
    palette_colors: &str,
    bayer_scale: u32,
    pixel_scale: u32,
    contrast: f64,
) -> Result<String, String> {
    let (s, colors) = validate(
        algorithm,
        palette,
        colors,
        palette_colors,
        bayer_scale,
        pixel_scale,
        contrast,
    )?;

    // Pre-dither chain on the image branch.
    let mut pre = String::new();
    if s.contrast != 1.0 {
        pre.push_str(&format!(",eq=contrast={}", trim_num(s.contrast)));
    }
    if s.pixel_scale > 1 {
        // Nearest-neighbour so the down-scale itself doesn't blur; dithering a
        // smaller image and blowing it back up is what produces chunky pixels.
        pre.push_str(&format!(
            ",scale=iw/{ps}:ih/{ps}:flags=neighbor",
            ps = s.pixel_scale
        ));
    }
    if is_monochromatic(&s.palette) {
        pre.push_str(",format=gray");
    }

    // paletteuse options.
    let mut use_opts = format!("dither={}", s.algorithm);
    if s.algorithm == "bayer" {
        use_opts.push_str(&format!(":bayer_scale={}", s.bayer_scale));
    }

    // Post chain. `format=rgb24` before any scaling is load-bearing: scaling the
    // pal8 output directly negotiates through subsampled chroma and silently
    // turns an exact 4-colour result into ~28 near-colours.
    let mut post = String::new();
    if s.pixel_scale > 1 {
        post.push_str(&format!(
            ",format=rgb24,scale=iw*{ps}:ih*{ps}:flags=neighbor",
            ps = s.pixel_scale
        ));
    }

    Ok(if s.palette == "auto" {
        format!(
            "[0:v]null{pre},split[b][s];\
             [s]palettegen=max_colors={colors}:stats_mode=single[p];\
             [b][p]paletteuse={use_opts}{post}"
        )
    } else {
        let pal = fixed_palette_graph(&s.palette_colors);
        format!(
            "{pal};[0:v]null{pre}[b];[b][p]paletteuse={use_opts}{post}"
        )
    })
}

/// Resolve the output file name for a chosen `format` given the input extension.
pub fn out_name(format: &str, in_ext: &str) -> Result<String, String> {
    let format = format.trim().to_ascii_lowercase();
    if !FORMATS.contains(&format.as_str()) {
        return Err(format!(
            "unknown format {format:?} — use one of: {}",
            FORMATS.join(", ")
        ));
    }
    let ext = match format.as_str() {
        "same" => in_ext,
        "jpeg" => "jpg",
        other => other,
    };
    Ok(format!("out.{ext}"))
}

/// Build the full ffmpeg argv (no leading `ffmpeg`) plus the output file name.
///
/// `-i <in>` → `-filter_complex <graph>` → `-frames:v 1` (the palette `color=`
/// sources are infinite, so the output is pinned to a single frame) → encoder
/// quality → `<out>`.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    algorithm: &str,
    palette: &str,
    colors: u32,
    palette_colors: &str,
    bayer_scale: u32,
    pixel_scale: u32,
    contrast: f64,
    format: &str,
) -> Result<(Vec<String>, String), String> {
    if in_name.trim().is_empty() {
        return Err("input file name is empty".into());
    }
    let filter = build_filter(
        algorithm,
        palette,
        colors,
        palette_colors,
        bayer_scale,
        pixel_scale,
        contrast,
    )?;
    let in_ext = in_name.rsplit('.').next().unwrap_or("png");
    let out = out_name(format, in_ext)?;

    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-filter_complex".to_string(),
        filter,
        "-frames:v".to_string(),
        "1".to_string(),
    ];
    // Dither patterns are single-pixel detail, so lossy encoders are pushed to
    // their highest practical quality. PNG/GIF are lossless and need nothing.
    if out.ends_with(".jpg") {
        argv.push("-q:v".to_string());
        argv.push("2".to_string());
    } else if out.ends_with(".webp") {
        argv.push("-lossless".to_string());
        argv.push("1".to_string());
    }
    argv.push(out.clone());
    Ok((argv, out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_default_auto_palette() {
        let (argv, out) = plan(
            "in.png",
            "floyd_steinberg",
            "auto",
            16,
            "",
            2,
            1,
            1.0,
            "png",
        )
        .unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.png");
        assert_eq!(argv[2], "-filter_complex");
        assert_eq!(
            argv[3],
            "[0:v]null,split[b][s];[s]palettegen=max_colors=16:stats_mode=single[p];[b][p]paletteuse=dither=floyd_steinberg"
        );
        assert_eq!(argv[argv.len() - 1], "out.png");
        // No lossy-encoder flags for PNG.
        assert!(!argv.contains(&"-q:v".to_string()));
    }

    #[test]
    fn mono_palette_is_pure_black_and_white_and_converts_to_luma() {
        let f = build_filter("atkinson", "mono", 16, "", 2, 1, 1.0).unwrap();
        assert_eq!(
            f,
            "color=c=0x000000:s=16x8[pc0];color=c=0xffffff:s=16x8[pc1];[pc0][pc1]vstack=inputs=2[p];\
             [0:v]null,format=gray[b];[b][p]paletteuse=dither=atkinson"
        );
    }

    #[test]
    fn gray4_ramp_is_evenly_spaced_over_16_rows() {
        assert_eq!(gray_ramp(4), ["000000", "555555", "aaaaaa", "ffffff"]);
        let f = build_filter("none", "gray4", 16, "", 2, 1, 1.0).unwrap();
        assert!(f.contains("color=c=0x000000:s=16x4[pc0]"));
        assert!(f.contains("color=c=0xffffff:s=16x4[pc3]"));
        assert!(f.contains("vstack=inputs=4[p]"));
    }

    #[test]
    fn row_split_always_totals_sixteen() {
        for n in 1..=MAX_FIXED_PALETTE {
            let rows = row_split(n);
            assert_eq!(rows.len(), n);
            assert_eq!(rows.iter().sum::<u32>(), 16, "n={n}");
            assert!(rows.iter().all(|&r| r >= 1), "n={n}");
        }
        assert_eq!(row_split(3), vec![6, 5, 5]);
    }

    #[test]
    fn bayer_scale_only_applies_to_the_bayer_kernel() {
        let b = build_filter("bayer", "auto", 8, "", 4, 1, 1.0).unwrap();
        assert!(b.contains("paletteuse=dither=bayer:bayer_scale=4"), "{b}");
        let fs = build_filter("floyd_steinberg", "auto", 8, "", 4, 1, 1.0).unwrap();
        assert!(!fs.contains("bayer_scale"), "{fs}");
    }

    #[test]
    fn pixel_scale_wraps_the_dither_and_forces_rgb24_before_the_upscale() {
        let f = build_filter("floyd_steinberg", "auto", 16, "", 2, 4, 1.0).unwrap();
        assert!(f.contains("[0:v]null,scale=iw/4:ih/4:flags=neighbor,split"), "{f}");
        // The rgb24 hop must sit between paletteuse and the upscale.
        assert!(
            f.ends_with("paletteuse=dither=floyd_steinberg,format=rgb24,scale=iw*4:ih*4:flags=neighbor"),
            "{f}"
        );
    }

    #[test]
    fn contrast_is_prepended_only_when_it_is_not_neutral() {
        let neutral = build_filter("bayer", "auto", 16, "", 2, 1, 1.0).unwrap();
        assert!(!neutral.contains("eq=contrast"), "{neutral}");
        let boosted = build_filter("bayer", "auto", 16, "", 2, 1, 1.5).unwrap();
        assert!(boosted.contains("[0:v]null,eq=contrast=1.5,split"), "{boosted}");
    }

    #[test]
    fn custom_palette_accepts_short_and_long_hex_with_or_without_hash() {
        assert_eq!(
            parse_palette_colors("#000, ffffff, #FF0000").unwrap(),
            ["000000", "ffffff", "ff0000"]
        );
        let f = build_filter("burkes", "custom", 16, "#000,#fff", 2, 1, 1.0).unwrap();
        assert!(f.starts_with("color=c=0x000000:s=16x8[pc0];color=c=0xffffff:s=16x8[pc1];"), "{f}");
        // A chromatic custom palette must NOT be forced through format=gray.
        let c = build_filter("burkes", "custom", 16, "#f00,#0f0,#00f", 2, 1, 1.0).unwrap();
        assert!(!c.contains("format=gray"), "{c}");
    }

    #[test]
    fn format_maps_to_the_right_output_extension() {
        assert_eq!(out_name("same", "jpg").unwrap(), "out.jpg");
        assert_eq!(out_name("png", "jpg").unwrap(), "out.png");
        assert_eq!(out_name("jpeg", "png").unwrap(), "out.jpg");
        assert_eq!(out_name("webp", "png").unwrap(), "out.webp");
        assert_eq!(out_name("gif", "png").unwrap(), "out.gif");
    }

    #[test]
    fn lossy_outputs_get_max_quality_flags() {
        let (argv, _) = plan("in.png", "bayer", "auto", 16, "", 2, 1, 1.0, "jpeg").unwrap();
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]), "{argv:?}");
        let (argv, _) = plan("in.png", "bayer", "auto", 16, "", 2, 1, 1.0, "webp").unwrap();
        assert!(argv.windows(2).any(|w| w == ["-lossless", "1"]), "{argv:?}");
    }

    // ---- error cases ----

    #[test]
    fn unknown_algorithm_is_rejected() {
        let e = plan("in.png", "stucki", "auto", 16, "", 2, 1, 1.0, "png").unwrap_err();
        assert!(e.contains("unknown algorithm"), "{e}");
        assert!(e.contains("floyd_steinberg"), "{e}");
    }

    #[test]
    fn out_of_range_params_are_rejected_with_the_bound_in_the_message() {
        assert!(build_filter("bayer", "auto", 1, "", 2, 1, 1.0)
            .unwrap_err()
            .contains("colors must be between 2 and 256"));
        assert!(build_filter("bayer", "auto", 16, "", 9, 1, 1.0)
            .unwrap_err()
            .contains("bayer_scale must be between 0 and 5"));
        assert!(build_filter("bayer", "auto", 16, "", 2, 0, 1.0)
            .unwrap_err()
            .contains("pixel_scale must be between 1 and 16"));
        assert!(build_filter("bayer", "auto", 16, "", 2, 1, 9.0)
            .unwrap_err()
            .contains("contrast must be between 0.5 and 3.0"));
        assert!(out_name("tiff", "png").unwrap_err().contains("unknown format"));
    }

    #[test]
    fn custom_palette_errors_are_actionable() {
        assert!(parse_palette_colors("#00ff")
            .unwrap_err()
            .contains("not a hex colour"));
        assert!(parse_palette_colors("#zzzzzz")
            .unwrap_err()
            .contains("not a hex colour"));
        assert!(parse_palette_colors("#000")
            .unwrap_err()
            .contains("at least 2 colours"));
        let seventeen = (0..17).map(|_| "#000000").collect::<Vec<_>>().join(",");
        assert!(parse_palette_colors(&seventeen)
            .unwrap_err()
            .contains("at most 16 colours"));
        // custom without a list surfaces the same guidance through plan().
        assert!(plan("in.png", "bayer", "custom", 16, "", 2, 1, 1.0, "png")
            .unwrap_err()
            .contains("at least 2 colours"));
    }

    #[test]
    fn empty_input_name_is_rejected() {
        assert!(plan("", "bayer", "auto", 16, "", 2, 1, 1.0, "png")
            .unwrap_err()
            .contains("input file name is empty"));
    }
}
