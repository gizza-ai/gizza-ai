//! gizza-ai/video-aspect-pad core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Letterboxes/pillarboxes a video onto a target-aspect canvas: the frame is
//! scaled to FIT the canvas (`force_original_aspect_ratio=decrease`, never
//! cropped or stretched) and centered with `pad`, the rest filled with a chosen
//! bar color. The output is always EXACTLY the canvas size — either the
//! platform-standard canvas for the chosen aspect (e.g. 1080×1920 for 9:16) or
//! `width` × the ratio-derived height. `force_divisible_by=2` keeps the scaled
//! content even so a rounding edge can never overflow the pad area, and
//! `setsar=1` forces square pixels. Audio is stream-copied untouched.

/// Supported aspect presets: `(name, aspect_w, aspect_h, default_canvas_w,
/// default_canvas_h)`. Every default canvas is exactly the stated ratio and
/// even on both axes (H.264/yuv420p requirement).
pub const ASPECTS: &[(&str, u32, u32, u32, u32)] = &[
    ("9:16", 9, 16, 1080, 1920), // Reels / Shorts / TikTok
    ("1:1", 1, 1, 1080, 1080),   // square feed
    ("16:9", 16, 9, 1920, 1080), // YouTube / landscape
    ("4:5", 4, 5, 1080, 1350),   // Instagram portrait
    ("3:4", 3, 4, 1080, 1440),
    ("4:3", 4, 3, 1440, 1080),
    ("2:3", 2, 3, 1080, 1620),   // Pinterest
    ("21:9", 21, 9, 2520, 1080), // cinematic letterbox
];

/// Smallest / largest accepted `width` override, in pixels.
pub const MIN_WIDTH: u32 = 16;
pub const MAX_WIDTH: u32 = 4096;

/// The color names ffmpeg's own parser accepts (`ffmpeg -colors`, the standard
/// CSS/X11 table), lower-cased. Validating against the same table means a bad
/// color fails HERE with a guiding message instead of deep inside ffmpeg — and
/// the strict charset keeps the filtergraph string injection-free.
const COLOR_NAMES: &[&str] = &[
    "aliceblue", "antiquewhite", "aqua", "aquamarine", "azure", "beige", "bisque", "black",
    "blanchedalmond", "blue", "blueviolet", "brown", "burlywood", "cadetblue", "chartreuse",
    "chocolate", "coral", "cornflowerblue", "cornsilk", "crimson", "cyan", "darkblue", "darkcyan",
    "darkgoldenrod", "darkgray", "darkgreen", "darkkhaki", "darkmagenta", "darkolivegreen",
    "darkorange", "darkorchid", "darkred", "darksalmon", "darkseagreen", "darkslateblue",
    "darkslategray", "darkturquoise", "darkviolet", "deeppink", "deepskyblue", "dimgray",
    "dodgerblue", "firebrick", "floralwhite", "forestgreen", "fuchsia", "gainsboro", "ghostwhite",
    "gold", "goldenrod", "gray", "green", "greenyellow", "honeydew", "hotpink", "indianred",
    "indigo", "ivory", "khaki", "lavender", "lavenderblush", "lawngreen", "lemonchiffon",
    "lightblue", "lightcoral", "lightcyan", "lightgoldenrodyellow", "lightgreen", "lightgrey",
    "lightpink", "lightsalmon", "lightseagreen", "lightskyblue", "lightslategray",
    "lightsteelblue", "lightyellow", "lime", "limegreen", "linen", "magenta", "maroon",
    "mediumaquamarine", "mediumblue", "mediumorchid", "mediumpurple", "mediumseagreen",
    "mediumslateblue", "mediumspringgreen", "mediumturquoise", "mediumvioletred", "midnightblue",
    "mintcream", "mistyrose", "moccasin", "navajowhite", "navy", "oldlace", "olive", "olivedrab",
    "orange", "orangered", "orchid", "palegoldenrod", "palegreen", "paleturquoise",
    "palevioletred", "papayawhip", "peachpuff", "peru", "pink", "plum", "powderblue", "purple",
    "red", "rosybrown", "royalblue", "saddlebrown", "salmon", "sandybrown", "seagreen",
    "seashell", "sienna", "silver", "skyblue", "slateblue", "slategray", "snow", "springgreen",
    "steelblue", "tan", "teal", "thistle", "tomato", "turquoise", "violet", "wheat", "white",
    "whitesmoke", "yellow", "yellowgreen",
];

fn aspect_list() -> String {
    ASPECTS.iter().map(|a| a.0).collect::<Vec<_>>().join("|")
}

fn out_ext(in_name: &str) -> &str {
    in_name
        .rsplit_once('.')
        .map(|(_, e)| e)
        .filter(|e| !e.is_empty())
        .unwrap_or("mp4")
}

/// Resolve the output canvas `(width, height)` for an aspect preset and an
/// optional width override. No override → the platform-standard canvas. With
/// an override the height follows the ratio, rounded to the nearest even
/// number. The override must be even (H.264/yuv420p) and within
/// [`MIN_WIDTH`, `MAX_WIDTH`]; out-of-spec values are rejected, not clamped.
pub fn canvas(aspect: &str, width: Option<u32>) -> Result<(u32, u32), String> {
    let &(_, aw, ah, dw, dh) = ASPECTS
        .iter()
        .find(|a| a.0 == aspect.trim())
        .ok_or_else(|| format!("aspect {:?} not supported ({})", aspect.trim(), aspect_list()))?;
    let Some(w) = width else {
        return Ok((dw, dh));
    };
    if !(MIN_WIDTH..=MAX_WIDTH).contains(&w) {
        return Err(format!(
            "width must be between {MIN_WIDTH} and {MAX_WIDTH} pixels, got {w}"
        ));
    }
    if w % 2 != 0 {
        return Err(format!(
            "width must be an even number (H.264/yuv420p requires even dimensions), got {w}"
        ));
    }
    let mut h = ((w as f64) * (ah as f64) / (aw as f64)).round() as u32;
    if h % 2 == 1 {
        h += 1;
    }
    Ok((w, h))
}

/// Normalize the user-facing bar color into a form ffmpeg accepts verbatim:
/// a lower-cased name from ffmpeg's own color table, or `#RRGGBB` / `#RGB` /
/// bare 6-digit hex → `0xRRGGBB`. Empty means the default black.
pub fn normalize_color(color: &str) -> Result<String, String> {
    let t = color.trim();
    if t.is_empty() {
        return Ok("black".to_string());
    }
    let hex = t.strip_prefix('#').unwrap_or(t);
    if hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(format!("0x{}", hex.to_ascii_uppercase()));
    }
    if t.starts_with('#') && hex.len() == 3 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        let doubled: String = hex.chars().flat_map(|c| [c, c]).collect();
        return Ok(format!("0x{}", doubled.to_ascii_uppercase()));
    }
    let lower = t.to_ascii_lowercase();
    if COLOR_NAMES.contains(&lower.as_str()) {
        return Ok(lower);
    }
    Err(format!(
        "color {t:?} not recognized — use a CSS color name (black, white, navy, …) or hex like #1A2B3C"
    ))
}

/// The `-vf` chain: fit inside the canvas (never crop/stretch), center, fill
/// the rest with `color`, square pixels.
pub fn build_filter(w: u32, h: u32, color: &str) -> String {
    format!(
        "scale=w={w}:h={h}:force_original_aspect_ratio=decrease:force_divisible_by=2,\
         pad=w={w}:h={h}:x=(ow-iw)/2:y=(oh-ih)/2:color={color},setsar=1"
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`). Video is re-encoded H.264
/// (medium/CRF 23, the family default); audio is stream-copied untouched.
pub fn build_argv(in_name: &str, out_name: &str, filter: &str) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        filter.into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-crf".into(),
        "23".into(),
        "-c:a".into(),
        "copy".into(),
        out_name.into(),
    ]
}

/// Validate everything and return `(argv, out_name)` — the single entry point
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
/// `out_name` keeps the input container extension.
pub fn plan(
    in_name: &str,
    aspect: &str,
    width: Option<u32>,
    color: &str,
) -> Result<(Vec<String>, String), String> {
    let (w, h) = canvas(aspect, width)?;
    let color = normalize_color(color)?;
    let filter = build_filter(w, h, &color);
    let out_name = format!("out.{}", out_ext(in_name));
    Ok((build_argv(in_name, &out_name, &filter), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-vf").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn default_canvas_full_argv() {
        let (argv, out) = plan("in.mp4", "9:16", None, "black").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-vf",
                "scale=w=1080:h=1920:force_original_aspect_ratio=decrease:force_divisible_by=2,\
                 pad=w=1080:h=1920:x=(ow-iw)/2:y=(oh-ih)/2:color=black,setsar=1",
                "-c:v",
                "libx264",
                "-preset",
                "medium",
                "-crf",
                "23",
                "-c:a",
                "copy",
                "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn every_preset_canvas_is_exact_ratio_and_even() {
        for &(name, aw, ah, dw, dh) in ASPECTS {
            assert_eq!(dw * ah, dh * aw, "{name}: default canvas must be exactly {aw}:{ah}");
            assert_eq!(dw % 2, 0, "{name}: even width");
            assert_eq!(dh % 2, 0, "{name}: even height");
            assert_eq!(canvas(name, None).unwrap(), (dw, dh));
        }
    }

    #[test]
    fn width_override_derives_even_height() {
        assert_eq!(canvas("9:16", Some(90)).unwrap(), (90, 160));
        assert_eq!(canvas("16:9", Some(128)).unwrap(), (128, 72));
        // 100 × 16/9 = 177.78 → rounds to 178 (already even).
        assert_eq!(canvas("9:16", Some(100)).unwrap(), (100, 178));
        // 3:4: 122 × 4/3 = 162.67 → rounds to 163 → bumped to even 164.
        assert_eq!(canvas("3:4", Some(122)).unwrap(), (122, 164));
    }

    #[test]
    fn bad_aspect_odd_or_out_of_range_width_rejected() {
        let err = plan("in.mp4", "5:7", None, "black").unwrap_err();
        assert!(err.contains("not supported") && err.contains("9:16"), "{err}");
        let err = canvas("9:16", Some(91)).unwrap_err();
        assert!(err.contains("even"), "{err}");
        assert!(canvas("9:16", Some(14)).is_err());
        assert!(canvas("9:16", Some(4098)).is_err());
        // Boundaries are valid.
        assert!(canvas("9:16", Some(16)).is_ok());
        assert!(canvas("9:16", Some(4096)).is_ok());
    }

    #[test]
    fn color_names_and_hex_normalize() {
        assert_eq!(normalize_color("black").unwrap(), "black");
        assert_eq!(normalize_color(" White ").unwrap(), "white");
        assert_eq!(normalize_color("DarkSlateGray").unwrap(), "darkslategray");
        assert_eq!(normalize_color("#ff0000").unwrap(), "0xFF0000");
        assert_eq!(normalize_color("1a2b3c").unwrap(), "0x1A2B3C");
        assert_eq!(normalize_color("#abc").unwrap(), "0xAABBCC");
        assert_eq!(normalize_color("").unwrap(), "black"); // empty → default
    }

    #[test]
    fn bad_colors_rejected_with_guidance() {
        for bad in ["blurple", "#12345", "#gggggg", "rgb(0,0,0)", "0x00FF00FF; rm"] {
            let err = normalize_color(bad).unwrap_err();
            assert!(err.contains("CSS color name"), "{bad}: {err}");
        }
    }

    #[test]
    fn filter_fits_centers_and_squares_pixels() {
        let (argv, _) = plan("clip.webm", "1:1", Some(320), "white").unwrap();
        let f = vf(&argv);
        assert!(f.contains("force_original_aspect_ratio=decrease"), "{f}");
        assert!(f.contains("force_divisible_by=2"), "{f}");
        assert!(f.contains("pad=w=320:h=320:x=(ow-iw)/2:y=(oh-ih)/2:color=white"), "{f}");
        assert!(f.ends_with("setsar=1"), "{f}");
    }

    #[test]
    fn keeps_container_extension_and_copies_audio() {
        let (argv, out) = plan("clip.webm", "16:9", None, "black").unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"));
    }
}
