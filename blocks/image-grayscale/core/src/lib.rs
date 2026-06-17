//! gizza-ai/image-grayscale core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.

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
        "format=gray".to_string(),
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_png_contains_format_gray() {
        let (argv, out_name) = plan("in.png").unwrap();
        assert!(argv.iter().any(|a| a == "format=gray"), "argv should contain format=gray: {argv:?}");
        assert_eq!(out_name, "out.png");
        assert!(argv.contains(&"-i".to_string()));
        assert!(argv.contains(&"in.png".to_string()));
        assert!(argv.contains(&"out.png".to_string()));
    }

    #[test]
    fn plan_jpg_keeps_extension() {
        let (argv, out_name) = plan("photo.jpg").unwrap();
        assert_eq!(out_name, "out.jpg");
        assert!(argv.contains(&"out.jpg".to_string()));
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
        // Should be exactly: ["-i", "in.webp", "-vf", "format=gray", "out.webp"]
        assert_eq!(&argv[0], "-i");
        assert_eq!(&argv[1], "in.webp");
        assert_eq!(&argv[2], "-vf");
        assert_eq!(&argv[3], "format=gray");
        assert_eq!(&argv[4], out_name.as_str());
    }
}
