//! gizza-ai/audio-bit-depth-converter core — pure ffmpeg argv construction
//! shared by the chat skill block and the standalone web page. No wafer /
//! wasm-bindgen deps.
//!
//! Changes an audio file's PCM **bit depth** (word length) — 24-bit → 16-bit for
//! CD/streaming delivery, up to 32-bit float for a DAW, down to 8-bit for retro
//! or tiny-payload uses. This is a different job from audio-resampler (which
//! changes the SAMPLE RATE in Hz) and from audio-convert (which changes the
//! container/codec): here the word length is the point.
//!
//! Requantizing DOWN throws away low-order bits, and doing that by plain
//! truncation adds correlated quantization distortion that is audible in quiet
//! passages and fade-outs. The fix is **dither**: a tiny amount of shaped noise
//! added before the truncation so the error becomes uncorrelated noise instead
//! of distortion. ffmpeg's swresample implements the standard family, selected
//! through the `aresample` filter's `dither_method` option — this core builds
//! that filter string.
//!
//! `-vn` drops any embedded album-art (attached-picture video) stream so
//! audio-only muxers like wav don't choke on it.

/// Target PCM word length. `S24` and `F32` both travel through ffmpeg's 32-bit
/// sample formats; the encoder (and `output_sample_bits` for 24-bit) is what
/// makes the written words the right width.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Depth {
    /// 8-bit unsigned PCM (`pcm_u8`) — 48 dB of dynamic range.
    U8,
    /// 16-bit signed PCM (`pcm_s16le`) — CD standard, 96 dB.
    S16,
    /// 24-bit signed PCM (`pcm_s24le`) — studio standard, 144 dB.
    S24,
    /// 32-bit IEEE float (`pcm_f32le`) — DAW interchange, effectively no
    /// clipping headroom limit.
    F32,
}

impl Depth {
    /// The value accepted by the chat schema / CLI / page for this depth.
    pub fn key(self) -> &'static str {
        match self {
            Depth::U8 => "8",
            Depth::S16 => "16",
            Depth::S24 => "24",
            Depth::F32 => "32f",
        }
    }

    /// Human label used in the LLM summary and the output filename copy.
    pub fn label(self) -> &'static str {
        match self {
            Depth::U8 => "8-bit",
            Depth::S16 => "16-bit",
            Depth::S24 => "24-bit",
            Depth::F32 => "32-bit float",
        }
    }

    /// Filename suffix appended to the original stem (`song` → `song-16bit`).
    pub fn suffix(self) -> &'static str {
        match self {
            Depth::U8 => "-8bit",
            Depth::S16 => "-16bit",
            Depth::S24 => "-24bit",
            Depth::F32 => "-32float",
        }
    }

    /// swresample output sample format (`aresample=osf=…`). There is no `s24`
    /// sample format in ffmpeg — 24-bit rides inside `s32` and the encoder
    /// writes 3-byte words.
    fn osf(self) -> &'static str {
        match self {
            Depth::U8 => "u8",
            Depth::S16 => "s16",
            Depth::S24 => "s32",
            Depth::F32 => "flt",
        }
    }

    /// Bits swresample should quantize (and dither) to, when that differs from
    /// the full width of `osf()`. Only 24-in-32 needs it.
    fn output_sample_bits(self) -> Option<u32> {
        match self {
            Depth::S24 => Some(24),
            _ => None,
        }
    }

    /// True for integer PCM targets, i.e. the ones where dither is meaningful.
    /// Dithering into 32-bit float is a no-op — floats keep far more precision
    /// than any real source, so nothing is being truncated.
    pub fn is_integer(self) -> bool {
        !matches!(self, Depth::F32)
    }
}

/// Parse the user-facing bit-depth string. Accepts the canonical keys plus the
/// obvious spellings people type (`16bit`, `32 float`, `32-bit float`).
pub fn parse_depth(s: &str) -> Result<Depth, String> {
    let norm: String = s
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '_'))
        .collect();
    match norm.as_str() {
        "8" | "8bit" | "u8" => Ok(Depth::U8),
        "16" | "16bit" | "s16" => Ok(Depth::S16),
        "24" | "24bit" | "s24" => Ok(Depth::S24),
        "32f" | "32float" | "32bitfloat" | "float" | "float32" | "flt" | "f32" => Ok(Depth::F32),
        other => Err(format!(
            "bit depth {other:?} not supported (expected 8, 16, 24 or 32f)"
        )),
    }
}

/// The default target depth: 16-bit, the CD / streaming delivery standard and
/// by far the most common reason to change bit depth at all.
pub const DEFAULT_DEPTH: &str = "16";

/// Dither algorithm applied while requantizing down. Mirrors swresample's
/// `dither_method`; `None` means plain truncation (no noise added).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Dither {
    None,
    Rectangular,
    Triangular,
    TriangularHp,
    Lipshitz,
    FWeighted,
    ModifiedEWeighted,
    ImprovedEWeighted,
    Shibata,
    LowShibata,
    HighShibata,
}

impl Dither {
    /// The value accepted by the chat schema / CLI / page.
    pub fn key(self) -> &'static str {
        match self {
            Dither::None => "none",
            Dither::Rectangular => "rectangular",
            Dither::Triangular => "triangular",
            Dither::TriangularHp => "triangular_hp",
            Dither::Lipshitz => "lipshitz",
            Dither::FWeighted => "f_weighted",
            Dither::ModifiedEWeighted => "modified_e_weighted",
            Dither::ImprovedEWeighted => "improved_e_weighted",
            Dither::Shibata => "shibata",
            Dither::LowShibata => "low_shibata",
            Dither::HighShibata => "high_shibata",
        }
    }

    /// swresample `dither_method` value, or `None` for plain truncation (the
    /// filter option is simply omitted, which is swresample's own default).
    fn ffmpeg_method(self) -> Option<&'static str> {
        match self {
            Dither::None => None,
            other => Some(other.key()),
        }
    }

    /// True for the noise-shaping members (psychoacoustically weighted noise,
    /// pushed where the ear is least sensitive) as opposed to flat dither.
    pub fn is_noise_shaped(self) -> bool {
        matches!(
            self,
            Dither::Lipshitz
                | Dither::FWeighted
                | Dither::ModifiedEWeighted
                | Dither::ImprovedEWeighted
                | Dither::Shibata
                | Dither::LowShibata
                | Dither::HighShibata
        )
    }
}

/// Every dither value the tool advertises, in the order they appear in the
/// schema enum and the page `<select>`.
pub const DITHER_KEYS: [&str; 11] = [
    "none",
    "rectangular",
    "triangular",
    "triangular_hp",
    "lipshitz",
    "f_weighted",
    "modified_e_weighted",
    "improved_e_weighted",
    "shibata",
    "low_shibata",
    "high_shibata",
];

/// Parse the user-facing dither string. Hyphens/spaces are normalized to the
/// underscore spelling so `triangular-hp` and `low shibata` both work.
pub fn parse_dither(s: &str) -> Result<Dither, String> {
    let norm = s
        .trim()
        .to_ascii_lowercase()
        .replace([' ', '-'], "_")
        .replace("__", "_");
    match norm.as_str() {
        "none" | "off" | "truncate" => Ok(Dither::None),
        "rectangular" | "rpdf" => Ok(Dither::Rectangular),
        "triangular" | "tpdf" => Ok(Dither::Triangular),
        "triangular_hp" | "tpdf_hp" => Ok(Dither::TriangularHp),
        "lipshitz" => Ok(Dither::Lipshitz),
        "f_weighted" => Ok(Dither::FWeighted),
        "modified_e_weighted" => Ok(Dither::ModifiedEWeighted),
        "improved_e_weighted" => Ok(Dither::ImprovedEWeighted),
        "shibata" => Ok(Dither::Shibata),
        "low_shibata" => Ok(Dither::LowShibata),
        "high_shibata" => Ok(Dither::HighShibata),
        other => Err(format!(
            "dither {other:?} not supported (expected one of: {})",
            DITHER_KEYS.join(", ")
        )),
    }
}

/// The default dither: plain TPDF (triangular probability density function),
/// the textbook-standard choice and what every serious mastering chain falls
/// back to when noise shaping isn't wanted.
pub const DEFAULT_DITHER: &str = "triangular";

/// Output container. Both are lossless — requantizing into a lossy codec would
/// defeat the point of picking a bit depth at all.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Format {
    Wav,
    Flac,
}

impl Format {
    /// Lower-cased file extension this format writes (used for `out.<ext>`).
    pub fn ext(self) -> &'static str {
        match self {
            Format::Wav => "wav",
            Format::Flac => "flac",
        }
    }

    /// IANA media type for the produced file.
    pub fn mime(self) -> &'static str {
        match self {
            Format::Wav => "audio/wav",
            Format::Flac => "audio/flac",
        }
    }

    /// Encoder argv fragment for a given depth, or an error when the codec
    /// simply cannot represent it (FLAC has no 8-bit and no float mode).
    fn codec_args(self, depth: Depth) -> Result<Vec<String>, String> {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<String>>();
        match (self, depth) {
            (Format::Wav, Depth::U8) => Ok(a(&["-c:a", "pcm_u8"])),
            (Format::Wav, Depth::S16) => Ok(a(&["-c:a", "pcm_s16le"])),
            (Format::Wav, Depth::S24) => Ok(a(&["-c:a", "pcm_s24le"])),
            (Format::Wav, Depth::F32) => Ok(a(&["-c:a", "pcm_f32le"])),
            (Format::Flac, Depth::S16) => Ok(a(&["-c:a", "flac", "-sample_fmt", "s16"])),
            (Format::Flac, Depth::S24) => Ok(a(&[
                "-c:a",
                "flac",
                "-sample_fmt",
                "s32",
                "-bits_per_raw_sample",
                "24",
            ])),
            (Format::Flac, d) => Err(format!(
                "flac cannot store {} audio (the FLAC codec supports 16-bit and 24-bit only) — \
                 choose format=wav for {}, or bit_depth=16 or 24 for flac",
                d.label(),
                d.label()
            )),
        }
    }
}

/// Parse the user-facing output format. Empty is handled by the caller, which
/// substitutes [`DEFAULT_FORMAT`] first.
pub fn parse_format(s: &str) -> Result<Format, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "wav" | "wave" => Ok(Format::Wav),
        "flac" => Ok(Format::Flac),
        other => Err(format!("format {other:?} not supported (wav|flac)")),
    }
}

/// The default output container: WAV, which can carry every depth this tool
/// offers and previews natively in every browser.
pub const DEFAULT_FORMAT: &str = "wav";

/// Build the `aresample=…` filter string that does the actual requantization.
/// Dither is only attached for integer targets — swresample would ignore it for
/// a float output, and advertising it there would be misleading.
pub fn filter_string(depth: Depth, dither: Dither) -> String {
    let mut opts = vec![format!("osf={}", depth.osf())];
    if let Some(bits) = depth.output_sample_bits() {
        opts.push(format!("output_sample_bits={bits}"));
    }
    if depth.is_integer() {
        if let Some(method) = dither.ffmpeg_method() {
            opts.push(format!("dither_method={method}"));
        }
    }
    format!("aresample={}", opts.join(":"))
}

/// Build the ffmpeg argv (no leading `ffmpeg`) converting `in_name` to
/// `out_name` at `depth`, dithering with `dither`, into `format`.
///
/// `-map_metadata -1` is added only when `keep_metadata` is false; ffmpeg copies
/// tags by default, so the common case needs no flag.
pub fn build_argv(
    in_name: &str,
    out_name: &str,
    depth: Depth,
    dither: Dither,
    format: Format,
    keep_metadata: bool,
) -> Result<Vec<String>, String> {
    let mut argv = vec!["-i".to_string(), in_name.to_string(), "-vn".to_string()];
    if !keep_metadata {
        argv.push("-map_metadata".to_string());
        argv.push("-1".to_string());
    }
    argv.push("-af".to_string());
    argv.push(filter_string(depth, dither));
    argv.extend(format.codec_args(depth)?);
    argv.push(out_name.to_string());
    Ok(argv)
}

/// Parse + validate every param and return `(argv, out_name)`. Single source
/// shared by the chat block (`src/lib.rs`) and the web page (`web/src/lib.rs`).
pub fn plan_convert(
    in_name: &str,
    bit_depth: &str,
    dither: &str,
    format: &str,
    keep_metadata: bool,
) -> Result<(Vec<String>, String), String> {
    let depth = parse_depth(bit_depth)?;
    let dith = parse_dither(dither)?;
    let fmt = parse_format(format)?;
    let out_name = format!("out.{}", fmt.ext());
    let argv = build_argv(in_name, &out_name, depth, dith, fmt, keep_metadata)?;
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strs(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn default_16_bit_wav_argv_is_exact() {
        let (argv, out) = plan_convert("in.wav", "16", "triangular", "wav", true).unwrap();
        assert_eq!(out, "out.wav");
        assert_eq!(
            argv,
            strs(&[
                "-i",
                "in.wav",
                "-vn",
                "-af",
                "aresample=osf=s16:dither_method=triangular",
                "-c:a",
                "pcm_s16le",
                "out.wav",
            ])
        );
    }

    #[test]
    fn twenty_four_bit_sets_output_sample_bits_and_the_s24_encoder() {
        // ffmpeg has no `s24` sample format: 24-bit rides in s32 and
        // output_sample_bits is what makes swresample quantize (and dither) to
        // 24 bits rather than 32.
        let (argv, _) = plan_convert("in.flac", "24", "shibata", "wav", true).unwrap();
        assert!(argv.contains(&"aresample=osf=s32:output_sample_bits=24:dither_method=shibata".to_string()));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "pcm_s24le"));
    }

    #[test]
    fn eight_bit_uses_unsigned_pcm() {
        let (argv, _) = plan_convert("in.mp3", "8", "triangular_hp", "wav", true).unwrap();
        assert!(argv.contains(&"aresample=osf=u8:dither_method=triangular_hp".to_string()));
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "pcm_u8"));
    }

    #[test]
    fn float_target_never_carries_a_dither_method() {
        // Dithering into float truncates nothing; attaching a method would be
        // an empty promise, so it is dropped even when explicitly requested.
        for d in DITHER_KEYS {
            let (argv, _) = plan_convert("in.wav", "32f", d, "wav", true).unwrap();
            assert!(
                argv.contains(&"aresample=osf=flt".to_string()),
                "dither {d}: float filter must be bare"
            );
            assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "pcm_f32le"));
        }
    }

    #[test]
    fn dither_none_omits_the_option_entirely() {
        let (argv, _) = plan_convert("in.wav", "16", "none", "wav", true).unwrap();
        assert!(argv.contains(&"aresample=osf=s16".to_string()));
        assert!(
            !argv.iter().any(|a| a.contains("dither_method")),
            "none means plain truncation"
        );
    }

    #[test]
    fn every_dither_value_maps_to_its_ffmpeg_method() {
        for key in DITHER_KEYS.iter().filter(|k| **k != "none") {
            let (argv, _) = plan_convert("in.wav", "16", key, "wav", true).unwrap();
            assert!(
                argv.contains(&format!("aresample=osf=s16:dither_method={key}")),
                "dither {key} must reach the filter"
            );
        }
    }

    #[test]
    fn flac_targets_carry_the_right_sample_fmt() {
        let (argv, out) = plan_convert("in.wav", "16", "triangular", "flac", true).unwrap();
        assert_eq!(out, "out.flac");
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "flac"));
        assert!(argv.windows(2).any(|w| w[0] == "-sample_fmt" && w[1] == "s16"));

        let (argv, _) = plan_convert("in.wav", "24", "triangular", "flac", true).unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-sample_fmt" && w[1] == "s32"));
        assert!(argv
            .windows(2)
            .any(|w| w[0] == "-bits_per_raw_sample" && w[1] == "24"));
    }

    #[test]
    fn flac_rejects_depths_the_codec_cannot_store() {
        for d in ["8", "32f"] {
            let err = plan_convert("in.wav", d, "triangular", "flac", true).unwrap_err();
            assert!(err.contains("flac"), "{d}: {err}");
            assert!(err.contains("wav"), "{d} error must suggest a way out: {err}");
        }
    }

    #[test]
    fn keep_metadata_false_strips_tags_and_true_adds_no_flag() {
        let (kept, _) = plan_convert("in.flac", "16", "triangular", "wav", true).unwrap();
        assert!(!kept.iter().any(|a| a == "-map_metadata"));
        let (stripped, _) = plan_convert("in.flac", "16", "triangular", "wav", false).unwrap();
        assert!(stripped
            .windows(2)
            .any(|w| w[0] == "-map_metadata" && w[1] == "-1"));
    }

    #[test]
    fn argv_always_drops_video_streams() {
        // Album-art files carry an attached-picture video stream; -vn keeps the
        // wav muxer from failing on it.
        for depth in ["8", "16", "24", "32f"] {
            let (argv, _) = plan_convert("in.mp3", depth, "triangular", "wav", true).unwrap();
            assert!(argv.iter().any(|a| a == "-vn"), "depth {depth} missing -vn");
        }
    }

    #[test]
    fn filter_precedes_the_encoder() {
        // -af must land before -c:a so the encoder receives requantized samples.
        let (argv, _) = plan_convert("in.wav", "24", "low_shibata", "flac", true).unwrap();
        let af = argv.iter().position(|a| a == "-af").expect("has -af");
        let ca = argv.iter().position(|a| a == "-c:a").expect("has -c:a");
        assert!(af < ca);
    }

    #[test]
    fn depth_parsing_accepts_the_spellings_people_type() {
        assert_eq!(parse_depth("16").unwrap(), Depth::S16);
        assert_eq!(parse_depth(" 24-bit ").unwrap(), Depth::S24);
        assert_eq!(parse_depth("8bit").unwrap(), Depth::U8);
        assert_eq!(parse_depth("32 float").unwrap(), Depth::F32);
        assert_eq!(parse_depth("32F").unwrap(), Depth::F32);
    }

    #[test]
    fn unknown_values_are_rejected_with_actionable_messages() {
        let e = parse_depth("12").unwrap_err();
        assert!(e.contains("8, 16, 24 or 32f"), "{e}");
        let e = parse_dither("magic").unwrap_err();
        assert!(e.contains("triangular"), "{e}");
        let e = parse_format("mp3").unwrap_err();
        assert!(e.contains("wav|flac"), "{e}");
        assert!(plan_convert("in.wav", "", "triangular", "wav", true).is_err());
    }

    #[test]
    fn dither_parsing_normalizes_separators_and_aliases() {
        assert_eq!(parse_dither("TPDF").unwrap(), Dither::Triangular);
        assert_eq!(parse_dither("triangular-hp").unwrap(), Dither::TriangularHp);
        assert_eq!(parse_dither("low shibata").unwrap(), Dither::LowShibata);
        assert_eq!(parse_dither("  none ").unwrap(), Dither::None);
    }

    #[test]
    fn metadata_helpers_are_consistent() {
        assert_eq!(Format::Wav.mime(), "audio/wav");
        assert_eq!(Format::Flac.mime(), "audio/flac");
        assert_eq!(Depth::S16.suffix(), "-16bit");
        assert_eq!(Depth::F32.suffix(), "-32float");
        assert_eq!(Depth::F32.label(), "32-bit float");
        assert!(!Depth::F32.is_integer());
        assert!(Depth::U8.is_integer());
        assert!(Dither::Shibata.is_noise_shaped());
        assert!(!Dither::Triangular.is_noise_shaped());
        for k in DITHER_KEYS {
            assert_eq!(parse_dither(k).unwrap().key(), k);
        }
        for k in ["8", "16", "24", "32f"] {
            assert_eq!(parse_depth(k).unwrap().key(), k);
        }
    }
}
