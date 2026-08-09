//! gizza-ai/video-grayscale core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Converts a color video to grayscale (or a toned monochrome) in a single
//! `colorchannelmixer` pass, then re-encodes to H.264 video. The input container
//! is kept when it can hold H.264 + AAC (mp4/mov/m4v/mkv); anything else (webm,
//! …) switches to mp4 — see `gizza_ai_block_utils::ffmpeg::h264_out_ext`.
//!
//! # The matrix
//!
//! Grayscale is a weighted sum of the RGB channels: `luma = Wr·R + Wg·G + Wb·B`.
//! `method` picks the weights, `tint` scales the resulting gray per output
//! channel, and `intensity` linearly blends the toned gray back toward the
//! original. All three collapse into ONE 3×3 matrix, so a tinted 60 % blend
//! costs exactly what a plain grayscale costs:
//!
//! ```text
//! out_R = (1-w)·R + w·Tr·(Wr·R + Wg·G + Wb·B)
//! out_G = (1-w)·G + w·Tg·(Wr·R + Wg·G + Wb·B)
//! out_B = (1-w)·B + w·Tb·(Wr·R + Wg·G + Wb·B)
//! ```
//!
//! with `w = intensity / 100`. At `intensity = 0` the matrix is the identity;
//! at `intensity = 100, tint = none` it is the classic luma matrix.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

pub const INTENSITY_MIN: f64 = 0.0;
pub const INTENSITY_MAX: f64 = 100.0;
pub const CONTRAST_MIN: f64 = 0.5;
pub const CONTRAST_MAX: f64 = 2.0;

/// Luma weights per `method`, as `(name, (Wr, Wg, Wb))`.
///
/// `bt709` is the HD/sRGB standard and the default; `bt601` is the older
/// SD weighting (greener, and what many legacy editors call "grayscale");
/// `average` weights all three equally (flat, high-key); `red`/`green`/`blue`
/// take a single channel, the darkroom color-filter technique (a red filter
/// darkens a blue sky, a green filter lightens foliage).
pub const METHODS: &[(&str, (f64, f64, f64))] = &[
    ("bt709", (0.2126, 0.7152, 0.0722)),
    ("bt601", (0.299, 0.587, 0.114)),
    ("average", (0.3333, 0.3334, 0.3333)),
    ("red", (1.0, 0.0, 0.0)),
    ("green", (0.0, 1.0, 0.0)),
    ("blue", (0.0, 0.0, 1.0)),
];

/// Per-channel multipliers applied to the gray value, as `(name, (Tr, Tg, Tb))`.
/// `none` is neutral; the rest are mild darkroom-style tones chosen to keep
/// overall brightness roughly unchanged.
pub const TINTS: &[(&str, (f64, f64, f64))] = &[
    ("none", (1.0, 1.0, 1.0)),
    ("sepia", (1.07, 0.94, 0.74)),
    ("warm", (1.06, 1.0, 0.90)),
    ("cool", (0.90, 1.0, 1.08)),
    ("cyanotype", (0.70, 0.90, 1.20)),
];

/// Quality tier → x264 CRF (lower = better quality, bigger file) and preset.
pub const QUALITIES: &[(&str, (&str, &str))] = &[
    ("fast", ("28", "veryfast")),
    ("balanced", ("23", "medium")),
    ("best", ("20", "slow")),
];

fn lookup<T: Copy>(table: &[(&str, T)], value: &str, label: &str, default: &str) -> Result<T, String> {
    let v = if value.trim().is_empty() { default } else { value.trim() };
    table
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(v))
        .map(|(_, t)| *t)
        .ok_or_else(|| {
            let names = table.iter().map(|t| t.0).collect::<Vec<_>>().join("|");
            format!("{label} {value:?} not supported ({names})")
        })
}

/// Luma weights for a `method` name (empty string → the `bt709` default).
pub fn method_weights(method: &str) -> Result<(f64, f64, f64), String> {
    lookup(METHODS, method, "method", "bt709")
}

/// Per-channel gray multipliers for a `tint` name (empty string → `none`).
pub fn tint_factors(tint: &str) -> Result<(f64, f64, f64), String> {
    lookup(TINTS, tint, "tint", "none")
}

/// `(crf, preset)` for a `quality` tier (empty string → `balanced`).
pub fn quality_settings(quality: &str) -> Result<(&'static str, &'static str), String> {
    lookup(QUALITIES, quality, "quality", "balanced")
}

fn check(name: &str, v: f64, min: f64, max: f64) -> Result<(), String> {
    if !v.is_finite() {
        return Err(format!("{name} must be a finite number, got {v}"));
    }
    if v < min || v > max {
        return Err(format!("{name} must be between {min} and {max}, got {v}"));
    }
    Ok(())
}

/// Format a matrix coefficient for ffmpeg: 4 decimals, trailing zeros trimmed,
/// never a signed zero.
fn fmt(v: f64) -> String {
    let v = (v * 10_000.0).round() / 10_000.0;
    let v = if v == 0.0 { 0.0 } else { v };
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() { "0".to_string() } else { s.to_string() }
}

/// Build the `colorchannelmixer` expression that applies `method`, `tint`, and
/// `intensity` (0–100) in one pass. See the module docs for the derivation.
pub fn build_mixer(method: &str, intensity: f64, tint: &str) -> Result<String, String> {
    let (wr, wg, wb) = method_weights(method)?;
    let (tr, tg, tb) = tint_factors(tint)?;
    let w = intensity / 100.0;
    let keep = 1.0 - w;
    // Row for output channel `c` with tint factor `t`: the luma row scaled by
    // `w·t`, plus `keep` on the channel's own diagonal entry.
    let row = |t: f64, diag: usize| -> (f64, f64, f64) {
        let mut r = (w * t * wr, w * t * wg, w * t * wb);
        match diag {
            0 => r.0 += keep,
            1 => r.1 += keep,
            _ => r.2 += keep,
        }
        r
    };
    let (rr, rg, rb) = row(tr, 0);
    let (gr, gg, gb) = row(tg, 1);
    let (br, bg, bb) = row(tb, 2);
    Ok(format!(
        "colorchannelmixer=rr={}:rg={}:rb={}:gr={}:gg={}:gb={}:br={}:bg={}:bb={}",
        fmt(rr),
        fmt(rg),
        fmt(rb),
        fmt(gr),
        fmt(gg),
        fmt(gb),
        fmt(br),
        fmt(bg),
        fmt(bb),
    ))
}

/// The full `-vf` chain: the mixer, plus `eq=contrast=` when contrast ≠ 1.
pub fn build_filter(method: &str, intensity: f64, tint: &str, contrast: f64) -> Result<String, String> {
    let mixer = build_mixer(method, intensity, tint)?;
    if (contrast - 1.0).abs() < f64::EPSILON {
        Ok(mixer)
    } else {
        Ok(format!("{mixer},eq=contrast={}", fmt(contrast)))
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for a grayscale conversion.
///
/// Video is re-encoded to H.264 at the tier's CRF. Audio is stream-copied when
/// the container is unchanged, re-encoded to AAC when it changed (a copied
/// Opus/Vorbis track would be invalid in mp4), or dropped entirely with `-an`
/// when `keep_audio` is false. MP4/MOV outputs get `+faststart` so the moov
/// atom leads and players can start before the whole file is buffered.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    filter: &str,
    crf: &str,
    preset: &str,
    transcode_audio: bool,
    keep_audio: bool,
) -> Vec<String> {
    let mut argv: Vec<String> = vec![
        "-i".into(),
        in_name.into(),
        "-vf".into(),
        filter.into(),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        preset.into(),
        "-crf".into(),
        crf.into(),
    ];
    if keep_audio {
        argv.push("-c:a".into());
        argv.push(if transcode_audio { "aac".into() } else { "copy".into() });
    } else {
        argv.push("-an".into());
    }
    if out_name.ends_with(".mp4") || out_name.ends_with(".mov") {
        argv.push("-movflags".into());
        argv.push("+faststart".into());
    }
    argv.push(out_name.into());
    argv
}

/// Validate every parameter and return `(argv, out_name)` — the single entry
/// point shared by the chat block (`src/lib.rs`) and the web page
/// (`web/src/lib.rs`). `out_name` keeps the input container when it can hold
/// H.264 + AAC; otherwise it is `out.mp4`.
pub fn plan(
    in_name: &str,
    method: &str,
    intensity: f64,
    tint: &str,
    contrast: f64,
    quality: &str,
    keep_audio: bool,
) -> Result<(Vec<String>, String), String> {
    check("intensity", intensity, INTENSITY_MIN, INTENSITY_MAX)?;
    check("contrast", contrast, CONTRAST_MIN, CONTRAST_MAX)?;
    let filter = build_filter(method, intensity, tint, contrast)?;
    let (crf, preset) = quality_settings(quality)?;
    let (ext, transcode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let argv = build_argv(
        in_name,
        &out_name,
        &filter,
        crf,
        preset,
        transcode_audio,
        keep_audio,
    );
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
    fn default_grayscale_full_argv() {
        let (argv, out) = plan("in.mp4", "bt709", 100.0, "none", 1.0, "balanced", true).unwrap();
        assert_eq!(out, "out.mp4");
        assert_eq!(
            argv,
            vec![
                "-i",
                "in.mp4",
                "-vf",
                "colorchannelmixer=rr=0.2126:rg=0.7152:rb=0.0722:\
                 gr=0.2126:gg=0.7152:gb=0.0722:br=0.2126:bg=0.7152:bb=0.0722",
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
    fn empty_strings_fall_back_to_defaults() {
        let (a, _) = plan("in.mp4", "", 100.0, "", 1.0, "", true).unwrap();
        let (b, _) = plan("in.mp4", "bt709", 100.0, "none", 1.0, "balanced", true).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn every_method_is_a_valid_row_summing_to_one() {
        for (name, (wr, wg, wb)) in METHODS {
            let sum = wr + wg + wb;
            assert!((sum - 1.0).abs() < 1e-3, "{name} weights sum to {sum}");
            let mixer = build_mixer(name, 100.0, "none").unwrap();
            assert!(mixer.starts_with("colorchannelmixer=rr="), "{name}: {mixer}");
        }
    }

    #[test]
    fn full_intensity_no_tint_makes_all_three_rows_identical() {
        // A pure grayscale matrix has R == G == B for every input pixel, which
        // means all three output rows carry the same coefficients.
        let m = build_mixer("bt601", 100.0, "none").unwrap();
        assert_eq!(
            m,
            "colorchannelmixer=rr=0.299:rg=0.587:rb=0.114:\
             gr=0.299:gg=0.587:gb=0.114:br=0.299:bg=0.587:bb=0.114"
        );
    }

    #[test]
    fn zero_intensity_is_the_identity_matrix() {
        assert_eq!(
            build_mixer("bt709", 0.0, "sepia").unwrap(),
            "colorchannelmixer=rr=1:rg=0:rb=0:gr=0:gg=1:gb=0:br=0:bg=0:bb=1"
        );
    }

    #[test]
    fn half_intensity_blends_halfway_between_identity_and_gray() {
        let m = build_mixer("average", 50.0, "none").unwrap();
        // 0.5 kept on the diagonal + 0.5 × 1/3 spread across the row.
        assert!(m.contains("rr=0.6667"), "{m}");
        assert!(m.contains("rg=0.1667"), "{m}");
        assert!(m.contains("gg=0.6667"), "{m}");
    }

    #[test]
    fn sepia_tint_biases_red_over_blue() {
        let m = build_mixer("bt709", 100.0, "sepia").unwrap();
        // Red row scaled by 1.07, blue row by 0.74 → red coefficients dominate.
        assert!(m.contains("rr=0.2275"), "{m}");
        assert!(m.contains("br=0.1573"), "{m}");
    }

    #[test]
    fn red_channel_method_passes_red_straight_through() {
        let m = build_mixer("red", 100.0, "none").unwrap();
        assert_eq!(
            m,
            "colorchannelmixer=rr=1:rg=0:rb=0:gr=1:gg=0:gb=0:br=1:bg=0:bb=0"
        );
    }

    #[test]
    fn contrast_appends_an_eq_stage_only_when_not_one() {
        let (argv, _) = plan("in.mp4", "bt709", 100.0, "none", 1.4, "balanced", true).unwrap();
        assert!(vf(&argv).ends_with(",eq=contrast=1.4"), "{}", vf(&argv));
        let (argv, _) = plan("in.mp4", "bt709", 100.0, "none", 1.0, "balanced", true).unwrap();
        assert!(!vf(&argv).contains("eq="), "{}", vf(&argv));
    }

    #[test]
    fn quality_tiers_map_to_crf_and_preset() {
        for (tier, (crf, preset)) in QUALITIES {
            let (argv, _) = plan("in.mp4", "bt709", 100.0, "none", 1.0, tier, true).unwrap();
            let i = argv.iter().position(|a| a == "-crf").unwrap();
            assert_eq!(&argv[i + 1], crf, "{tier}");
            let p = argv.iter().position(|a| a == "-preset").unwrap();
            assert_eq!(&argv[p + 1], preset, "{tier}");
        }
    }

    #[test]
    fn dropping_audio_uses_an_instead_of_a_codec() {
        let (argv, _) = plan("in.mp4", "bt709", 100.0, "none", 1.0, "balanced", false).unwrap();
        assert!(argv.contains(&"-an".to_string()), "{argv:?}");
        assert!(!argv.contains(&"-c:a".to_string()), "{argv:?}");
    }

    #[test]
    fn webm_input_becomes_mp4_and_re_encodes_audio() {
        let (argv, out) = plan("clip.webm", "bt709", 100.0, "none", 1.0, "balanced", true).unwrap();
        assert_eq!(out, "out.mp4");
        let i = argv.iter().position(|a| a == "-c:a").unwrap();
        assert_eq!(&argv[i + 1], "aac");
    }

    #[test]
    fn h264_containers_are_kept_and_audio_is_copied() {
        for ext in ["mov", "mkv", "m4v"] {
            let (argv, out) = plan(&format!("clip.{ext}"), "bt709", 100.0, "none", 1.0, "best", true).unwrap();
            assert_eq!(out, format!("out.{ext}"));
            let i = argv.iter().position(|a| a == "-c:a").unwrap();
            assert_eq!(&argv[i + 1], "copy", "{ext}");
        }
        // faststart only applies to the containers that have a moov atom.
        let (argv, _) = plan("clip.mkv", "bt709", 100.0, "none", 1.0, "best", true).unwrap();
        assert!(!argv.contains(&"-movflags".to_string()));
    }

    #[test]
    fn rejects_out_of_range_intensity() {
        let err = plan("in.mp4", "bt709", 140.0, "none", 1.0, "balanced", true).unwrap_err();
        assert!(err.contains("intensity must be between 0 and 100"), "{err}");
        assert!(plan("in.mp4", "bt709", -1.0, "none", 1.0, "balanced", true).is_err());
        assert!(plan("in.mp4", "bt709", f64::NAN, "none", 1.0, "balanced", true).is_err());
    }

    #[test]
    fn rejects_out_of_range_contrast() {
        let err = plan("in.mp4", "bt709", 100.0, "none", 3.0, "balanced", true).unwrap_err();
        assert!(err.contains("contrast must be between 0.5 and 2"), "{err}");
        assert!(plan("in.mp4", "bt709", 100.0, "none", 0.4, "balanced", true).is_err());
    }

    #[test]
    fn rejects_unknown_enum_values_with_the_allowed_list() {
        let err = plan("in.mp4", "hdr", 100.0, "none", 1.0, "balanced", true).unwrap_err();
        assert!(err.contains("method \"hdr\" not supported"), "{err}");
        assert!(err.contains("bt709|bt601|average|red|green|blue"), "{err}");

        let err = plan("in.mp4", "bt709", 100.0, "vintage", 1.0, "balanced", true).unwrap_err();
        assert!(err.contains("tint \"vintage\" not supported"), "{err}");

        let err = plan("in.mp4", "bt709", 100.0, "none", 1.0, "ultra", true).unwrap_err();
        assert!(err.contains("quality \"ultra\" not supported"), "{err}");
    }

    #[test]
    fn boundary_values_are_accepted() {
        assert!(plan("in.mp4", "bt709", 0.0, "none", 0.5, "fast", true).is_ok());
        assert!(plan("in.mp4", "bt709", 100.0, "cyanotype", 2.0, "best", false).is_ok());
    }
}
