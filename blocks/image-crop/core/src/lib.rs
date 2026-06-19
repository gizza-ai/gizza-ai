//! gizza-ai/image-crop core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.

/// Build the ffmpeg argv for a crop (no leading "ffmpeg").
///
/// ffmpeg's `crop` video filter takes `crop=w:h:x:y` — width and height first,
/// then the top-left x/y offset.
pub fn build_argv(in_name: &str, out_name: &str, x: u32, y: u32, w: u32, h: u32) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        format!("crop={w}:{h}:{x}:{y}"),
        out_name.into(),
    ]
}

/// Validate the crop rectangle and return `(argv, out_name)` for an input file.
/// `out_name` keeps the input extension. Used by the web page (file in → file out).
pub fn plan_crop(
    in_name: &str,
    x: u32,
    y: u32,
    w: u32,
    h: u32,
) -> Result<(Vec<String>, String), String> {
    if w == 0 || h == 0 {
        return Err("width and height must be > 0".into());
    }
    let ext = in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty())
        .unwrap_or("png");
    let out_name = format!("out.{ext}");
    Ok((build_argv(in_name, &out_name, x, y, w, h), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_crop_positional_order() {
        // ffmpeg crop filter is w:h:x:y.
        let argv = build_argv("in.png", "out.png", 10, 20, 200, 300);
        assert_eq!(
            argv,
            vec![
                "-i".to_string(),
                "in.png".to_string(),
                "-vf".to_string(),
                "crop=200:300:10:20".to_string(),
                "out.png".to_string(),
            ]
        );
    }

    #[test]
    fn plan_crop_keeps_extension_and_validates() {
        let (argv, out) = plan_crop("in.jpg", 0, 0, 320, 240).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.iter().any(|a| a == "crop=320:240:0:0"));
        assert!(plan_crop("in.png", 0, 0, 0, 10).is_err());
        assert!(plan_crop("in.png", 0, 0, 10, 0).is_err());
    }

    #[test]
    fn plan_crop_defaults_extension_when_trailing_dot() {
        // A trailing dot leaves an empty last segment → fall back to "png".
        let (_, out) = plan_crop("file.", 0, 0, 10, 10).unwrap();
        assert_eq!(out, "out.png");
    }
}
