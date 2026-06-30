//! gizza-ai/photo-filter-presets core — pure ffmpeg argv construction shared by
//! the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Each named preset maps to a fixed ffmpeg `-vf` filter chain built from
//! standard, browser-ffmpeg-available filters (colorchannelmixer, colorbalance,
//! curves, hue, eq, negate). The mapping is deterministic so the chat block, CLI
//! and page all produce identical output for a given preset. Every chain is a
//! single token (no spaces) so it passes cleanly as one argv element.

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Preset {
    Sepia,
    Vintage,
    Warm,
    Cool,
    Noir,
    Grayscale,
    Vivid,
    Invert,
    Fade,
}

/// The canonical preset names, in display order. Used for the schema enum and
/// the page `<select>` options. KEEP IN SYNC with `parse_preset` / `filter_chain`.
pub const PRESETS: [&str; 9] = [
    "sepia", "vintage", "warm", "cool", "noir", "grayscale", "vivid", "invert", "fade",
];

/// The default preset applied when no `preset` is supplied.
pub const DEFAULT_PRESET: &str = "sepia";

/// Parse a preset name (case-insensitive). `None` / `""` default to `sepia`.
pub fn parse_preset(s: Option<&str>) -> Result<Preset, String> {
    let v = s.unwrap_or(DEFAULT_PRESET).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "sepia" => Ok(Preset::Sepia),
        "vintage" => Ok(Preset::Vintage),
        "warm" => Ok(Preset::Warm),
        "cool" => Ok(Preset::Cool),
        "noir" => Ok(Preset::Noir),
        "grayscale" | "greyscale" => Ok(Preset::Grayscale),
        "vivid" => Ok(Preset::Vivid),
        "invert" | "negative" => Ok(Preset::Invert),
        "fade" => Ok(Preset::Fade),
        other => Err(format!(
            "invalid preset {other:?}; expected one of {}",
            PRESETS.join("|")
        )),
    }
}

/// The lower-cased canonical name of a preset (matches a `PRESETS` entry).
pub fn preset_name(preset: Preset) -> &'static str {
    match preset {
        Preset::Sepia => "sepia",
        Preset::Vintage => "vintage",
        Preset::Warm => "warm",
        Preset::Cool => "cool",
        Preset::Noir => "noir",
        Preset::Grayscale => "grayscale",
        Preset::Vivid => "vivid",
        Preset::Invert => "invert",
        Preset::Fade => "fade",
    }
}

/// The ffmpeg `-vf` filter chain that realises a preset.
///
/// - **sepia** — classic sepia tone matrix (the standard 0.393/0.769/0.189… weights).
/// - **vintage** — ffmpeg's built-in `curves=preset=vintage` faded-film look.
/// - **warm** — push midtones/highlights toward red, away from blue.
/// - **cool** — push midtones/highlights toward blue, away from red.
/// - **noir** — desaturate to black & white, then boost contrast for a film-noir look.
/// - **grayscale** — plain black & white (desaturate only).
/// - **vivid** — punchy boost to saturation and a touch of contrast.
/// - **invert** — photographic negative.
/// - **fade** — soft, matte faded look with lifted blacks.
pub fn filter_chain(preset: Preset) -> &'static str {
    match preset {
        Preset::Sepia => {
            "colorchannelmixer=0.393:0.769:0.189:0:0.349:0.686:0.168:0:0.272:0.534:0.131"
        }
        Preset::Vintage => "curves=preset=vintage",
        Preset::Warm => "colorbalance=rm=0.08:gm=0.02:bm=-0.08:rh=0.06:bh=-0.06",
        Preset::Cool => "colorbalance=rm=-0.06:bm=0.08:rh=-0.05:bh=0.07",
        Preset::Noir => "hue=s=0,eq=contrast=1.5:brightness=-0.04",
        Preset::Grayscale => "hue=s=0",
        Preset::Vivid => "eq=saturation=1.4:contrast=1.08",
        Preset::Invert => "negate",
        Preset::Fade => "curves=preset=lighter",
    }
}

/// Build the ffmpeg argv (no leading "ffmpeg") and the output filename for
/// applying `preset` to `in_name`. `out_name` keeps the input extension.
pub fn plan(in_name: &str, preset: Preset) -> Result<(Vec<String>, String), String> {
    let ext = in_name
        .rsplit('.')
        .next()
        .filter(|e| !e.is_empty())
        .unwrap_or("png");
    let out_name = format!("out.{ext}");
    let argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        filter_chain(preset).to_string(),
        out_name.clone(),
    ];
    Ok((argv, out_name))
}

/// Parse + plan in one step from a raw preset string (used by the web page + CLI).
pub fn plan_named(in_name: &str, preset: Option<&str>) -> Result<(Vec<String>, String), String> {
    let preset = parse_preset(preset)?;
    plan(in_name, preset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_preset_default_is_sepia() {
        assert_eq!(parse_preset(None).unwrap(), Preset::Sepia);
        assert_eq!(parse_preset(Some("")).unwrap(), Preset::Sepia);
        assert_eq!(DEFAULT_PRESET, "sepia");
    }

    #[test]
    fn parse_preset_is_case_insensitive() {
        assert_eq!(parse_preset(Some("NOIR")).unwrap(), Preset::Noir);
        assert_eq!(parse_preset(Some(" Warm ")).unwrap(), Preset::Warm);
        assert_eq!(parse_preset(Some("Vivid")).unwrap(), Preset::Vivid);
    }

    #[test]
    fn parse_preset_all_names() {
        for (name, want) in [
            ("sepia", Preset::Sepia),
            ("vintage", Preset::Vintage),
            ("warm", Preset::Warm),
            ("cool", Preset::Cool),
            ("noir", Preset::Noir),
            ("grayscale", Preset::Grayscale),
            ("vivid", Preset::Vivid),
            ("invert", Preset::Invert),
            ("fade", Preset::Fade),
        ] {
            assert_eq!(parse_preset(Some(name)).unwrap(), want);
        }
    }

    #[test]
    fn parse_preset_accepts_aliases() {
        assert_eq!(parse_preset(Some("greyscale")).unwrap(), Preset::Grayscale);
        assert_eq!(parse_preset(Some("negative")).unwrap(), Preset::Invert);
    }

    #[test]
    fn parse_preset_rejects_unknown() {
        assert!(parse_preset(Some("instagram")).is_err());
        // The error lists the valid names so the LLM/CLI user can recover.
        let err = parse_preset(Some("instagram")).unwrap_err();
        assert!(err.contains("sepia") && err.contains("fade"));
    }

    #[test]
    fn presets_const_matches_parser_and_names() {
        for name in PRESETS {
            let p = parse_preset(Some(name)).expect("PRESETS entry must parse");
            assert_eq!(preset_name(p), name, "preset_name round-trips PRESETS");
        }
    }

    #[test]
    fn every_preset_has_a_nonempty_distinct_filter_chain() {
        let mut chains = std::collections::HashSet::new();
        for name in PRESETS {
            let p = parse_preset(Some(name)).unwrap();
            let chain = filter_chain(p);
            assert!(!chain.is_empty(), "{name} has an empty chain");
            assert!(!chain.contains(' '), "{name} chain must be a single argv token");
            assert!(chains.insert(chain), "{name} chain duplicates another preset");
        }
    }

    #[test]
    fn sepia_uses_colorchannelmixer_matrix() {
        let (argv, out) = plan("in.png", Preset::Sepia).unwrap();
        assert_eq!(out, "out.png");
        let vf = argv.iter().position(|a| a == "-vf").unwrap();
        assert!(argv[vf + 1].starts_with("colorchannelmixer=0.393:0.769:0.189"));
    }

    #[test]
    fn noir_desaturates_then_boosts_contrast() {
        let chain = filter_chain(Preset::Noir);
        assert!(chain.contains("hue=s=0"));
        assert!(chain.contains("contrast=1.5"));
    }

    #[test]
    fn vintage_uses_builtin_curves_preset() {
        assert_eq!(filter_chain(Preset::Vintage), "curves=preset=vintage");
    }

    #[test]
    fn invert_uses_negate() {
        assert_eq!(filter_chain(Preset::Invert), "negate");
    }

    #[test]
    fn warm_and_cool_are_distinct() {
        assert_ne!(filter_chain(Preset::Warm), filter_chain(Preset::Cool));
    }

    #[test]
    fn plan_argv_structure_and_extension() {
        let (argv, out) = plan("photo.jpg", Preset::Warm).unwrap();
        assert_eq!(out, "out.jpg");
        assert_eq!(&argv[0], "-i");
        assert_eq!(&argv[1], "photo.jpg");
        assert_eq!(&argv[2], "-vf");
        assert_eq!(&argv[3], filter_chain(Preset::Warm));
        assert_eq!(&argv[4], "out.jpg");
    }

    #[test]
    fn plan_no_dot_falls_back_to_png() {
        let (_argv, out) = plan("imagefile", Preset::Cool).unwrap();
        assert_eq!(out, "out.imagefile");
    }

    #[test]
    fn plan_named_parses_and_plans() {
        let (argv, _out) = plan_named("in.webp", Some("noir")).unwrap();
        assert!(argv.iter().any(|a| a.contains("hue=s=0")));
        assert!(plan_named("in.png", Some("nope")).is_err());
    }
}
