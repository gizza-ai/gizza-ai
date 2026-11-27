//! gizza-ai/image-grayscale core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.

use gizza_ai_block_utils::ffmpeg::{finish_image_alpha, image_alpha, ImageAlpha, FLATTEN_ON_WHITE};

/// The grayscale filter for an output with an alpha channel (PNG, WebP) or a
/// GIF: gray + alpha (`ya8`), so every pixel keeps its exact opacity.
///
/// It is forced, not offered as a `ya8|gray` format LIST: ffmpeg's negotiation
/// does not count an indexed PNG's palette transparency (`pal8` + tRNS) as
/// alpha and would pick `gray`, flattening the image. The cost is that an
/// opaque PNG also comes back as gray + (fully opaque) alpha.
pub const GRAY_ALPHA_FILTER: &str = "format=ya8";

/// The grayscale filter chain for `out.<ext>`. JPEG has no alpha, so the image
/// is flattened onto white first (what the transparent result looks like on a
/// page) and then converted to plain `gray`; GIF gets a transparent palette.
/// Both gray formats carry the same BT.601 luma.
pub fn filter(out_ext: &str) -> String {
    match image_alpha(out_ext) {
        ImageAlpha::None => format!("{FLATTEN_ON_WHITE},format=gray"),
        ImageAlpha::Channel | ImageAlpha::Palette => finish_image_alpha(GRAY_ALPHA_FILTER, out_ext),
    }
}

/// Build the ffmpeg argv and output filename for a grayscale conversion.
///
/// Returns `(argv, out_name)` where `argv` has no leading "ffmpeg" and
/// `out_name` keeps the same extension as `in_name`.
pub fn plan(in_name: &str) -> Result<(Vec<String>, String), String> {
    let ext = in_name.rsplit('.').next().filter(|e| !e.is_empty()).unwrap_or("png");
    let out_name = format!("out.{ext}");
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        filter(ext),
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gizza_ai_block_utils::ffmpeg::GIF_ALPHA_PALETTE;

    #[test]
    fn plan_png_keeps_alpha_as_gray_plus_alpha() {
        let (argv, out_name) = plan("in.png").unwrap();
        assert_eq!(argv[3], "format=ya8", "{argv:?}");
        assert_eq!(out_name, "out.png");
        assert!(argv.contains(&"-i".to_string()));
        assert!(argv.contains(&"in.png".to_string()));
        assert!(argv.contains(&"out.png".to_string()));
    }

    #[test]
    fn alpha_format_is_forced_not_negotiated() {
        // A `ya8|gray` list negotiates to `gray` for a palette PNG with tRNS
        // (ffmpeg does not count pal8 as alpha) and drops its transparency.
        for ext in ["png", "webp", "gif"] {
            let f = filter(ext);
            assert!(f.starts_with("format=ya8"), "{ext}: {f}");
            assert!(!f.contains('|'), "{ext}: {f}");
        }
    }

    #[test]
    fn plan_jpg_keeps_extension_and_flattens_onto_white() {
        let (argv, out_name) = plan("photo.jpg").unwrap();
        assert_eq!(out_name, "out.jpg");
        assert!(argv.contains(&"out.jpg".to_string()));
        assert_eq!(argv[3], format!("{FLATTEN_ON_WHITE},format=gray"));
        assert_eq!(filter("JPEG"), argv[3]);
    }

    #[test]
    fn plan_gif_maps_alpha_to_a_transparent_palette() {
        let (argv, out_name) = plan("anim.gif").unwrap();
        assert_eq!(out_name, "out.gif");
        assert_eq!(argv[3], format!("format=ya8,{GIF_ALPHA_PALETTE}"));
    }

    #[test]
    fn plan_no_dot_uses_whole_name_as_ext() {
        // rsplit('.') on a name with no dot returns the whole name;
        // behaviour mirrors image-resize: wraps it as "out.{whole_name}".
        let (_argv, out_name) = plan("imagefile").unwrap();
        assert_eq!(out_name, "out.imagefile");
    }

    #[test]
    fn plan_argv_has_correct_structure() {
        let (argv, out_name) = plan("in.webp").unwrap();
        // Should be exactly: ["-i", "in.webp", "-vf", "format=ya8", "out.webp"]
        assert_eq!(&argv[0], "-i");
        assert_eq!(&argv[1], "in.webp");
        assert_eq!(&argv[2], "-vf");
        assert_eq!(&argv[3], "format=ya8");
        assert_eq!(&argv[4], out_name.as_str());
        assert_eq!(argv.len(), 5);
    }
}
