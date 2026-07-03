//! gizza-ai/video-aspect-pad core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Letterboxes/pillarboxes a video onto a target-aspect canvas: the frame is
//! scaled to FIT the canvas (`force_original_aspect_ratio=decrease`, never
//! cropped or stretched) and centered, the rest filled with either a chosen
//! solid bar color (`pad`) or a blurred, scaled-to-cover copy of the video
//! itself (`split`/`boxblur`/`overlay` — the "blur background" look). The
//! output is always EXACTLY the canvas size — either the platform-standard
//! canvas for the chosen aspect (e.g. 1080×1920 for 9:16) or `width` × the
//! ratio-derived height. `force_divisible_by=2` keeps the scaled content even
//! so a rounding edge can never overflow the pad area, and `setsar=1` forces
//! square pixels. The input container is kept (audio stream-copied untouched)
//! when it can hold H.264 + AAC (mp4/mov/m4v/mkv); anything else (webm, …)
//! switches to mp4 and the audio is re-encoded to AAC — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`. MP4/MOV outputs get
//! `+faststart` so social players can start streaming before the download ends.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

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
    ("3:2", 3, 2, 1620, 1080),   // classic photo / camera
    ("2:3", 2, 3, 1080, 1620),   // Pinterest
    ("21:9", 21, 9, 2520, 1080), // cinematic letterbox
];

/// Encoding-quality tiers: `(name, x264 CRF)`. The default is the family-wide
/// `medium`/CRF 23; `high` trades a bigger file for near-transparent quality,
/// `low` the reverse. Naming the exact CRF on the page is deliberate — it's a
/// trust signal, not marketing tiers.
pub const QUALITIES: &[(&str, &str)] = &[("high", "18"), ("medium", "23"), ("low", "28")];

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

/// Resolve a quality tier name to its x264 CRF. Empty means the default
/// `medium`; anything else must be a known tier (rejected with guidance).
pub fn crf_for(quality: &str) -> Result<&'static str, String> {
    let q = quality.trim().to_ascii_lowercase();
    if q.is_empty() {
        return Ok("23");
    }
    QUALITIES
        .iter()
        .find(|(name, _)| *name == q)
        .map(|(_, crf)| *crf)
        .ok_or_else(|| {
            let names = QUALITIES.iter().map(|q| q.0).collect::<Vec<_>>().join("|");
            format!("quality {quality:?} not supported ({names})")
        })
}

/// Blur radius for the blurred-background fill, derived from the canvas so the
/// look scales with resolution (1080-wide → 67). Clamped to ≥2 so tiny
/// canvases still visibly blur; `min(w,h)/16` always satisfies boxblur's
/// radius ≤ dimension/2 rule, including on half-size chroma planes.
pub fn blur_radius(w: u32, h: u32) -> u32 {
    (w.min(h) / 16).max(2)
}

/// The `-vf` chain for solid bars: fit inside the canvas (never crop/stretch),
/// center, fill the rest with `color`, square pixels.
pub fn build_filter(w: u32, h: u32, color: &str) -> String {
    format!(
        "scale=w={w}:h={h}:force_original_aspect_ratio=decrease:force_divisible_by=2,\
         pad=w={w}:h={h}:x=(ow-iw)/2:y=(oh-ih)/2:color={color},setsar=1"
    )
}

/// The `-filter_complex` graph for the blurred-background fill: one copy of
/// the frame is scaled to COVER the canvas, center-cropped and box-blurred
/// (the backdrop); the other is scaled to FIT (never cropped/stretched) and
/// centered on top. Output is labeled `[v]` for `-map`.
pub fn build_blur_graph(w: u32, h: u32) -> String {
    let r = blur_radius(w, h);
    format!(
        "[0:v]split=2[bg][fg];\
         [bg]scale=w={w}:h={h}:force_original_aspect_ratio=increase:force_divisible_by=2,\
         crop=w={w}:h={h},boxblur={r}:2[bgb];\
         [fg]scale=w={w}:h={h}:force_original_aspect_ratio=decrease:force_divisible_by=2[fgs];\
         [bgb][fgs]overlay=x=(W-w)/2:y=(H-h)/2,setsar=1[v]"
    )
}

/// Shared encoder tail: H.264 at the tier's CRF (`medium` preset, the family
/// default), audio stream-copied untouched — or re-encoded to AAC when
/// `transcode_audio` is set (the output container differs from the input's,
/// so a copied Opus/Vorbis track would be invalid or unplayable in mp4) —
/// and `+faststart` on MP4/MOV so the moov atom leads and social players can
/// stream immediately.
fn encoder_args(out_name: &str, crf: &str, transcode_audio: bool) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-crf".into(),
        crf.into(),
        "-c:a".into(),
        if transcode_audio { "aac".into() } else { "copy".into() },
    ];
    if out_name.ends_with(".mp4") || out_name.ends_with(".mov") {
        args.push("-movflags".into());
        args.push("+faststart".into());
    }
    args.push(out_name.into());
    args
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for the solid-bars path.
pub fn build_argv(in_name: &str, out_name: &str, filter: &str, crf: &str, transcode_audio: bool) -> Vec<String> {
    let mut argv: Vec<String> = vec!["-i".into(), in_name.into(), "-vf".into(), filter.into()];
    argv.extend(encoder_args(out_name, crf, transcode_audio));
    argv
}

/// Build the ffmpeg argv for the blurred-background path: `-filter_complex`
/// with the video mapped from the graph's `[v]` and audio (if any) copied —
/// or AAC-encoded when `transcode_audio` is set.
pub fn build_argv_blur(in_name: &str, out_name: &str, graph: &str, crf: &str, transcode_audio: bool) -> Vec<String> {
    let mut argv: Vec<String> = vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        graph.into(),
        "-map".into(),
        "[v]".into(),
        "-map".into(),
        "0:a?".into(),
    ];
    argv.extend(encoder_args(out_name, crf, transcode_audio));
    argv
}

/// Validate everything and return `(argv, out_name)` — the single entry point
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
/// `out_name` keeps the input container when it can hold H.264 + AAC;
/// otherwise it is `out.mp4` and the audio is re-encoded to AAC. `blur` fills
/// the bars with a blurred cover-scaled copy of the video instead of the solid
/// `color` (which is still validated, then unused).
pub fn plan(
    in_name: &str,
    aspect: &str,
    width: Option<u32>,
    color: &str,
    blur: bool,
    quality: &str,
) -> Result<(Vec<String>, String), String> {
    let (w, h) = canvas(aspect, width)?;
    let color = normalize_color(color)?;
    let crf = crf_for(quality)?;
    let (ext, transcode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let argv = if blur {
        build_argv_blur(in_name, &out_name, &build_blur_graph(w, h), crf, transcode_audio)
    } else {
        build_argv(in_name, &out_name, &build_filter(w, h, &color), crf, transcode_audio)
    };
    Ok((argv, out_name))
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
        let (argv, out) = plan("in.mp4", "9:16", None, "black", false, "").unwrap();
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
                "-movflags",
                "+faststart",
                "out.mp4",
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn quality_tiers_map_to_crf_and_bad_tier_rejected() {
        assert_eq!(crf_for("").unwrap(), "23"); // empty → family default
        assert_eq!(crf_for("high").unwrap(), "18");
        assert_eq!(crf_for(" Medium ").unwrap(), "23");
        assert_eq!(crf_for("low").unwrap(), "28");
        let err = crf_for("ultra").unwrap_err();
        assert!(err.contains("high|medium|low"), "{err}");
        let (argv, _) = plan("in.mp4", "9:16", None, "black", false, "high").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-crf" && w[1] == "18"));
    }

    #[test]
    fn blur_background_builds_cover_blur_overlay_graph() {
        let (argv, out) = plan("in.mp4", "9:16", Some(90), "black", true, "").unwrap();
        assert_eq!(out, "out.mp4");
        let i = argv.iter().position(|a| a == "-filter_complex").unwrap();
        let g = &argv[i + 1];
        // backdrop: cover-scale, center-crop, blur (radius min(90,160)/16 → 5)
        assert!(g.contains("force_original_aspect_ratio=increase"), "{g}");
        assert!(g.contains("crop=w=90:h=160"), "{g}");
        assert!(g.contains("boxblur=5:2"), "{g}");
        // foreground: fit-scale, centered overlay, square pixels, labeled [v]
        assert!(g.contains("force_original_aspect_ratio=decrease"), "{g}");
        assert!(g.contains("overlay=x=(W-w)/2:y=(H-h)/2,setsar=1[v]"), "{g}");
        // the graph's video is mapped; audio (if any) is copied
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "[v]"), "{argv:?}");
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "0:a?"), "{argv:?}");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"), "{argv:?}");
        // no -vf in blur mode
        assert!(!argv.iter().any(|a| a == "-vf"), "{argv:?}");
    }

    #[test]
    fn blur_radius_scales_with_canvas_and_never_underflows() {
        assert_eq!(blur_radius(1080, 1920), 67);
        assert_eq!(blur_radius(90, 160), 5);
        assert_eq!(blur_radius(16, 30), 2); // 16/16 = 1 → clamped to 2
        // boxblur needs radius ≤ dimension/2, also on half-size chroma planes.
        for &(_, _, _, w, h) in ASPECTS {
            assert!(blur_radius(w, h) <= w.min(h) / 4);
        }
    }

    #[test]
    fn faststart_on_mp4_and_mov_only() {
        let fast = |argv: &[String]| {
            argv.windows(2).any(|w| w[0] == "-movflags" && w[1] == "+faststart")
        };
        let (argv, _) = plan("in.mp4", "1:1", None, "black", false, "").unwrap();
        assert!(fast(&argv));
        let (argv, _) = plan("in.mov", "1:1", None, "black", true, "").unwrap();
        assert!(fast(&argv));
        // webm input switches the container to mp4 → faststart applies there too.
        let (argv, _) = plan("in.webm", "1:1", None, "black", false, "").unwrap();
        assert!(fast(&argv));
        let (argv, _) = plan("in.mkv", "1:1", None, "black", false, "").unwrap();
        assert!(!fast(&argv));
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
        // 3:2 (new preset): default canvas and a width override.
        assert_eq!(canvas("3:2", None).unwrap(), (1620, 1080));
        assert_eq!(canvas("3:2", Some(300)).unwrap(), (300, 200));
    }

    #[test]
    fn bad_aspect_odd_or_out_of_range_width_rejected() {
        let err = plan("in.mp4", "5:7", None, "black", false, "").unwrap_err();
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
        let (argv, _) = plan("clip.webm", "1:1", Some(320), "white", false, "").unwrap();
        let f = vf(&argv);
        assert!(f.contains("force_original_aspect_ratio=decrease"), "{f}");
        assert!(f.contains("force_divisible_by=2"), "{f}");
        assert!(f.contains("pad=w=320:h=320:x=(ow-iw)/2:y=(oh-ih)/2:color=white"), "{f}");
        assert!(f.ends_with("setsar=1"), "{f}");
    }

    #[test]
    fn keeps_h264_capable_containers_and_copies_audio() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (argv, out) = plan(&format!("clip.{ext}"), "16:9", None, "black", false, "").unwrap();
            assert_eq!(out, format!("out.{ext}"));
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "copy"), "{ext}");
        }
    }

    #[test]
    fn webm_input_switches_to_mp4_and_reencodes_audio() {
        // WebM can't hold H.264, and its Opus/Vorbis audio can't be copied
        // into mp4 — so: out.mp4 + AAC re-encode (both paths).
        let (argv, out) = plan("clip.webm", "16:9", None, "black", false, "").unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
        let (argv, out) = plan("clip.webm", "9:16", None, "black", true, "").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
    }

    #[test]
    fn blur_still_validates_color_and_quality() {
        // A bad color is rejected even though blur ignores it — silently
        // accepting garbage would teach users a value that breaks later.
        assert!(plan("in.mp4", "9:16", None, "blurple", true, "").is_err());
        assert!(plan("in.mp4", "9:16", None, "black", true, "ultra").is_err());
    }
}
