//! gizza-ai/video-hdr-to-sdr core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! video-hdr-to-sdr **tone-maps** an HDR video (PQ / `smpte2084` or HLG /
//! `arib-std-b67`, BT.2020 wide-gamut) down to a standard-dynamic-range,
//! BT.709 / yuv420p file — so it doesn't render gray, dim, or blown-out on an
//! ordinary SDR screen. The heavy lifting is a fixed, order-sensitive `zscale`
//! + `tonemap` filter chain (libzimg is present in both system ffmpeg AND the
//! browser `@ffmpeg/core` build):
//!
//! ```text
//! zscale=t=linear:npl=<peak>,format=gbrpf32le,zscale=p=bt709,
//! tonemap=tonemap=<algo>:desat=<desat>,zscale=t=bt709:m=bt709:r=tv,format=yuv420p
//! ```
//!
//! Order matters: linearize (reads the source PQ/HLG transfer automatically) →
//! float RGB → gamut-map to BT.709 → tone-map luminance → re-encode the BT.709
//! transfer at limited (`tv`) range → 8-bit 4:2:0. Dropping `format=gbrpf32le`
//! or swapping steps causes banding / wash-out.

/// Tone-mapping operator passed to ffmpeg's `tonemap=tonemap=<algo>` filter.
/// Each maps HDR luminance into the SDR range with a different curve.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Algorithm {
    /// `hable` — filmic S-curve, the near-universal default. Preserves highlight
    /// and shadow detail with natural contrast.
    Hable,
    /// `mobius` — keeps more midtone saturation; punchier, slightly less highlight roll-off.
    Mobius,
    /// `reinhard` — brighter, flatter global operator. Simple and fast; can look washed vs hable.
    Reinhard,
    /// `linear` — straight luminance scaling. Fast, but clips or dims without a real curve.
    Linear,
    /// `clip` — no roll-off; hard-clips out-of-range highlights. Sharpest but blows out bright areas.
    Clip,
}

impl Algorithm {
    /// The exact string passed to ffmpeg's `tonemap=tonemap=<...>`.
    pub fn as_arg(self) -> &'static str {
        match self {
            Algorithm::Hable => "hable",
            Algorithm::Mobius => "mobius",
            Algorithm::Reinhard => "reinhard",
            Algorithm::Linear => "linear",
            Algorithm::Clip => "clip",
        }
    }
}

/// Parse the user-facing tone-map string (the values the chat schema + page accept).
pub fn parse_algorithm(s: &str) -> Result<Algorithm, String> {
    match s {
        "hable" => Ok(Algorithm::Hable),
        "mobius" => Ok(Algorithm::Mobius),
        "reinhard" => Ok(Algorithm::Reinhard),
        "linear" => Ok(Algorithm::Linear),
        "clip" => Ok(Algorithm::Clip),
        other => Err(format!(
            "tonemap {other:?} not supported (hable|mobius|reinhard|linear|clip)"
        )),
    }
}

/// Default tone-map operator — `hable`, the filmic curve every HDR→SDR guide
/// recommends as the safe starting point.
pub const DEFAULT_ALGORITHM: Algorithm = Algorithm::Hable;

/// Output container video-hdr-to-sdr can write. The video is always re-encoded
/// SDR H.264 (mp4) or VP9 (webm); the tone-map is identical either way.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Mp4,
    Webm,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Mp4 => "mp4",
            Format::Webm => "webm",
        }
    }
}

/// Parse the user-facing output format string.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s {
        "mp4" => Ok(Format::Mp4),
        "webm" => Ok(Format::Webm),
        other => Err(format!("format {other:?} not supported (mp4|webm)")),
    }
}

/// Default output format.
pub const DEFAULT_FORMAT: Format = Format::Mp4;

/// Default quality when none is supplied (≈ CRF 13 — high-quality SDR delivery).
pub const DEFAULT_QUALITY: u8 = 75;

/// Default nominal peak luminance (`npl`, nits) of the target SDR display. 100
/// nits is the reference SDR white point every tone-map guide uses.
pub const DEFAULT_PEAK: u32 = 100;

/// Default highlight desaturation strength. `0` keeps highlight color (the
/// recommended default — a non-zero desat grays bright areas).
pub const DEFAULT_DESAT: f64 = 0.0;

/// Map web-conventional quality 1-100 to ffmpeg's CRF range 0 (best) – 51 (worst).
pub fn quality_to_crf(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let crf = 51.0 - (q - 1.0) * (51.0 / 99.0);
    crf.round().clamp(0.0, 51.0) as u8
}

/// Build the order-sensitive `zscale`+`tonemap` filtergraph string (the value
/// passed to `-vf`). `peak` is the target nominal peak luminance (npl, nits);
/// `desat` is the highlight desaturation strength.
pub fn build_filter(algo: Algorithm, peak: u32, desat: f64) -> String {
    format!(
        "zscale=t=linear:npl={peak},format=gbrpf32le,zscale=p=bt709,\
tonemap=tonemap={algo}:desat={desat},zscale=t=bt709:m=bt709:r=tv,format=yuv420p",
        peak = peak,
        algo = algo.as_arg(),
        desat = fmt_num(desat),
    )
}

/// Format an f64 without a trailing `.0` (`0.0` → `"0"`, `0.5` → `"0.5"`) so the
/// ffmpeg filter string stays clean and stable.
fn fmt_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that tone-maps `in_name` to
/// `out_name`. mp4 → libx264/aac (+faststart), webm → libvpx-vp9/libopus
/// (`-b:v 0` for constant-quality VP9). Audio is copied through untouched.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    filter: &str,
    format: Format,
    crf: u8,
) -> Vec<String> {
    let mut argv = vec!["-i".into(), in_name.into(), "-vf".into(), filter.into()];
    match format {
        Format::Mp4 => argv.extend([
            "-c:v".into(),
            "libx264".into(),
            "-crf".into(),
            crf.to_string(),
            "-c:a".into(),
            "aac".into(),
            "-movflags".into(),
            "+faststart".into(),
        ]),
        Format::Webm => argv.extend([
            "-c:v".into(),
            "libvpx-vp9".into(),
            "-crf".into(),
            crf.to_string(),
            "-b:v".into(),
            "0".into(),
            "-c:a".into(),
            "libopus".into(),
        ]),
    }
    argv.push(out_name.into());
    argv
}

/// Validate params, parse the tone-map operator + output format, and build
/// `(argv, out_name)` for an input file. Single source shared by the chat block
/// (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_hdr_to_sdr(
    tonemap: &str,
    peak: u32,
    desat: f64,
    format: &str,
    quality: u8,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    if !(1..=100).contains(&quality) {
        return Err(format!("quality must be 1-100, got {quality}"));
    }
    if !(100..=10000).contains(&peak) {
        return Err(format!("peak must be 100-10000 nits, got {peak}"));
    }
    if !(0.0..=4.0).contains(&desat) {
        return Err(format!("desat must be 0-4, got {desat}"));
    }
    let algo = parse_algorithm(tonemap)?;
    let fmt = parse_format(format)?;
    let crf = quality_to_crf(quality);
    let filter = build_filter(algo, peak, desat);
    let out_name = format!("out.{}", fmt.ext());
    Ok((build_argv(in_name, &out_name, &filter, fmt, crf), out_name))
}

/// Convenience wrapper using every default except the required `in_name` — used
/// where a caller supplies no knobs (defaults tone-map to SDR mp4 at quality 75).
pub fn plan(in_name: &str) -> Result<(Vec<String>, String), String> {
    plan_hdr_to_sdr(
        DEFAULT_ALGORITHM.as_arg(),
        DEFAULT_PEAK,
        DEFAULT_DESAT,
        DEFAULT_FORMAT.ext(),
        DEFAULT_QUALITY,
        in_name,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_algorithm_known() {
        assert_eq!(parse_algorithm("hable").unwrap(), Algorithm::Hable);
        assert_eq!(parse_algorithm("reinhard").unwrap(), Algorithm::Reinhard);
        assert_eq!(parse_algorithm("clip").unwrap(), Algorithm::Clip);
    }

    #[test]
    fn parse_algorithm_rejects_unknown() {
        assert!(parse_algorithm("aces").is_err());
        assert!(parse_algorithm("bt2390").is_err());
    }

    #[test]
    fn parse_format_known_and_unknown() {
        assert_eq!(parse_format("mp4").unwrap(), Format::Mp4);
        assert_eq!(parse_format("webm").unwrap(), Format::Webm);
        assert!(parse_format("mkv").is_err());
    }

    #[test]
    fn quality_to_crf_endpoints() {
        assert_eq!(quality_to_crf(100), 0);
        assert_eq!(quality_to_crf(1), 51);
    }

    #[test]
    fn filter_has_ordered_zscale_tonemap_pipeline() {
        let f = build_filter(Algorithm::Hable, 100, 0.0);
        // linearize BEFORE tonemap, gbrpf32le float RGB present, closes on yuv420p.
        let lin = f.find("zscale=t=linear:npl=100").unwrap();
        let flt = f.find("format=gbrpf32le").unwrap();
        let tm = f.find("tonemap=tonemap=hable:desat=0").unwrap();
        let out = f.find("zscale=t=bt709:m=bt709:r=tv").unwrap();
        assert!(lin < flt && flt < tm && tm < out, "pipeline order: {f}");
        assert!(f.ends_with("format=yuv420p"));
    }

    #[test]
    fn filter_desat_and_peak_are_interpolated() {
        let f = build_filter(Algorithm::Mobius, 1000, 0.5);
        assert!(f.contains("npl=1000"));
        assert!(f.contains("tonemap=mobius:desat=0.5"), "{f}");
    }

    #[test]
    fn argv_mp4_uses_h264_faststart_and_crf() {
        let filter = build_filter(Algorithm::Hable, 100, 0.0);
        let argv = build_argv("in.mp4", "out.mp4", &filter, Format::Mp4, 13);
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-vf" && w[1] == filter));
        assert!(argv.iter().any(|a| a == "libx264"));
        assert!(argv.windows(2).any(|w| w[0] == "-crf" && w[1] == "13"));
        assert!(argv.iter().any(|a| a == "+faststart"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }

    #[test]
    fn argv_webm_uses_vp9_opus_constant_quality() {
        let filter = build_filter(Algorithm::Reinhard, 100, 0.0);
        let argv = build_argv("in.mp4", "out.webm", &filter, Format::Webm, 20);
        assert!(argv.iter().any(|a| a == "libvpx-vp9"));
        assert!(argv.iter().any(|a| a == "libopus"));
        assert!(argv.windows(2).any(|w| w[0] == "-b:v" && w[1] == "0"));
        assert_eq!(argv.last().map(String::as_str), Some("out.webm"));
    }

    #[test]
    fn plan_hdr_to_sdr_uses_target_extension() {
        let (_argv, out) = plan_hdr_to_sdr("hable", 100, 0.0, "mp4", 75, "clip.mov").unwrap();
        assert_eq!(out, "out.mp4");
        let (_argv, out) = plan_hdr_to_sdr("hable", 100, 0.0, "webm", 75, "clip.mov").unwrap();
        assert_eq!(out, "out.webm");
    }

    #[test]
    fn plan_hdr_to_sdr_rejects_bad_params() {
        assert!(plan_hdr_to_sdr("hable", 100, 0.0, "mp4", 0, "a.mp4").is_err());
        assert!(plan_hdr_to_sdr("hable", 100, 0.0, "mp4", 101, "a.mp4").is_err());
        assert!(plan_hdr_to_sdr("hable", 50, 0.0, "mp4", 75, "a.mp4").is_err());
        assert!(plan_hdr_to_sdr("hable", 100, 9.0, "mp4", 75, "a.mp4").is_err());
        assert!(plan_hdr_to_sdr("nope", 100, 0.0, "mp4", 75, "a.mp4").is_err());
        assert!(plan_hdr_to_sdr("hable", 100, 0.0, "avi", 75, "a.mp4").is_err());
    }

    #[test]
    fn plan_defaults_to_hable_mp4() {
        let (argv, out) = plan("in.mov").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.iter().any(|a| a.contains("tonemap=hable")));
        assert!(argv.iter().any(|a| a == "libx264"));
    }
}
