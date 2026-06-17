//! gizza-ai/image-resize core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Fit { Contain, Cover, Stretch }

pub fn parse_fit(s: Option<&str>) -> Result<Fit, String> {
    match s.unwrap_or("contain") {
        "" | "contain" => Ok(Fit::Contain),
        "cover"        => Ok(Fit::Cover),
        "stretch"      => Ok(Fit::Stretch),
        other          => Err(format!("invalid fit {other:?}; expected contain|cover|stretch")),
    }
}

/// Build the ffmpeg argv for a resize (no leading "ffmpeg").
pub fn build_argv(in_name: &str, out_name: &str, w: Option<u32>, h: Option<u32>, fit: Fit) -> Vec<String> {
    let (sw, sh) = (
        w.map(|v| v.to_string()).unwrap_or_else(|| "-1".to_string()),
        h.map(|v| v.to_string()).unwrap_or_else(|| "-1".to_string()),
    );
    let vf = match fit {
        Fit::Stretch => format!("scale={sw}:{sh}"),
        Fit::Contain => format!("scale={sw}:{sh}:force_original_aspect_ratio=decrease"),
        Fit::Cover   => format!("scale={sw}:{sh}:force_original_aspect_ratio=increase,crop={sw}:{sh}"),
    };
    vec!["-i".into(), in_name.into(), "-vf".into(), vf, out_name.into()]
}

/// Validate dimensions + fit and return `(argv, out_name)` for an input file.
/// `out_name` keeps the input extension. Used by the web page (file in → file out).
pub fn plan_resize(in_name: &str, w: Option<u32>, h: Option<u32>, fit: Fit) -> Result<(Vec<String>, String), String> {
    if w.is_none() && h.is_none() {
        return Err("at least one of width/height is required".into());
    }
    if w == Some(0) || h == Some(0) {
        return Err("width/height must be > 0".into());
    }
    if fit == Fit::Cover && (w.is_none() || h.is_none()) {
        return Err("fit=cover requires both width and height".into());
    }
    let ext = in_name.rsplit('.').next().filter(|e| !e.is_empty()).unwrap_or("png");
    let out_name = format!("out.{ext}");
    Ok((build_argv(in_name, &out_name, w, h, fit), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_contain_both_dims() {
        let argv = build_argv("in.png", "out.png", Some(640), Some(480), Fit::Contain);
        assert_eq!(argv, vec![
            "-i".to_string(), "in.png".to_string(), "-vf".to_string(),
            "scale=640:480:force_original_aspect_ratio=decrease".to_string(), "out.png".to_string(),
        ]);
    }

    #[test]
    fn argv_contain_width_only_uses_minus_one_height() {
        let argv = build_argv("in.png", "out.png", Some(640), None, Fit::Contain);
        assert!(argv.iter().any(|a| a == "scale=640:-1:force_original_aspect_ratio=decrease"));
    }

    #[test]
    fn argv_contain_height_only_uses_minus_one_width() {
        let argv = build_argv("in.png", "out.png", None, Some(480), Fit::Contain);
        assert!(argv.iter().any(|a| a == "scale=-1:480:force_original_aspect_ratio=decrease"));
    }

    #[test]
    fn argv_stretch_no_aspect_ratio_flag() {
        let argv = build_argv("in.png", "out.png", Some(640), Some(480), Fit::Stretch);
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(vf, "scale=640:480");
    }

    #[test]
    fn argv_cover_uses_two_stage_filter() {
        let argv = build_argv("in.png", "out.png", Some(640), Some(480), Fit::Cover);
        let vf = argv.iter().find(|a| a.starts_with("scale=")).unwrap();
        assert_eq!(vf, "scale=640:480:force_original_aspect_ratio=increase,crop=640:480");
    }

    #[test]
    fn parse_fit_default_is_contain() { assert_eq!(parse_fit(None).unwrap(), Fit::Contain); }

    #[test]
    fn parse_fit_empty_is_contain() { assert_eq!(parse_fit(Some("")).unwrap(), Fit::Contain); }

    #[test]
    fn parse_fit_rejects_unknown() { assert!(parse_fit(Some("squish")).is_err()); }

    #[test]
    fn plan_resize_keeps_extension_and_validates() {
        let (argv, out) = plan_resize("in.jpg", Some(320), None, Fit::Contain).unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv.iter().any(|a| a == "scale=320:-1:force_original_aspect_ratio=decrease"));
        assert!(plan_resize("in.png", None, None, Fit::Contain).is_err());
        assert!(plan_resize("in.png", Some(0), Some(10), Fit::Stretch).is_err());
        assert!(plan_resize("in.png", Some(10), None, Fit::Cover).is_err());
    }
}
