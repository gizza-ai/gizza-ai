//! video-aspect-ratio-fix core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page.
//!
//! Fixes the "anamorphic / squeezed / stretched" class of file: the encoded
//! pixels are perfectly fine, but the container tells the player the wrong
//! shape, so a 16:9 recording plays back squashed into 4:3 (or a phone clip
//! shows up letterboxed sideways). The repair is to rewrite the **display
//! aspect ratio (DAR) tag** with `ffmpeg -map 0 -c copy -aspect W:H`, which
//! remuxes the file and stamps the new ratio into the fresh container header.
//! Because it is stream copy, nothing is decoded and nothing is re-encoded —
//! the audio/video packets are copied bit-for-bit and the output is typically
//! the same size as the input, byte for byte.
//!
//! ffmpeg derives the sample (pixel) aspect ratio from the requested DAR and
//! the stored frame size, so `-aspect 16:9` on a 640×480 file leaves the frame
//! at 640×480 and sets SAR to 4:3. To reset a file to SQUARE pixels, pass the
//! stored pixel size as the ratio (`640x480` → SAR 1:1); there is no
//! dimension-independent stream-copy form (`-aspect 0` is rejected by ffmpeg).
//!
//! Ratios are normalized here rather than handed to ffmpeg verbatim, so every
//! accepted spelling (`16:9`, `16/9`, `1.7777`, `1920x1080`) ends up as the
//! same exact integer pair.

/// Ratio presets offered by the `aspect` enum, in page/select order, plus the
/// `custom` sentinel that switches to the free-form `custom_aspect` field.
pub const ASPECTS: [&str; 13] = [
    "16:9", "9:16", "4:3", "3:4", "1:1", "21:9", "2.39:1", "1.85:1", "5:4", "4:5", "3:2", "2:3",
    CUSTOM,
];

/// Sentinel `aspect` value that defers to `custom_aspect`.
pub const CUSTOM: &str = "custom";

/// Output container policy — `keep` reuses the input's extension.
pub const CONTAINER_KEEP: &str = "keep";
pub const CONTAINERS: [&str; 5] = ["keep", "mp4", "mkv", "mov", "webm"];

/// Smallest / largest display aspect ratio accepted, as a plain multiplier
/// (width ÷ height). Anything outside is a typo far more often than an intent
/// (`1920x0`, `100:1`), so it is rejected with the bounds named.
pub const MIN_RATIO: f64 = 0.05;
pub const MAX_RATIO: f64 = 20.0;

/// Largest numerator/denominator kept after reduction. Real ratios reduce far
/// below this; a longer decimal (`1.7777777`) is rounded to fit rather than
/// emitted as a giant fraction ffmpeg would have to carry into the header.
const MAX_TERM: u64 = 100_000;

fn aspect_list() -> String {
    ASPECTS.join(", ")
}

fn extension(in_name: &str) -> &str {
    in_name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| !ext.is_empty())
        .unwrap_or("mp4")
}

/// True for MP4-family containers, where `-movflags +faststart` applies.
fn is_mp4_family(ext: &str) -> bool {
    matches!(ext, "mp4" | "mov" | "m4v")
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a
    } else {
        gcd(b, a % b)
    }
}

/// Parse one side of a ratio (or a whole bare-decimal ratio) into an exact
/// `numerator / denominator` pair, straight from the decimal TEXT so `2.39`
/// becomes exactly 239/100 rather than an f64 approximation.
fn parse_decimal(text: &str) -> Result<(u64, u64), String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("empty number".to_string());
    }
    let (int_part, frac_part) = match t.split_once('.') {
        Some((i, f)) => (i, f),
        None => (t, ""),
    };
    // Digits only: no sign, no exponent, no separators — those are typos here.
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(format!("{t:?} is not a number"));
    }
    if !int_part.chars().all(|c| c.is_ascii_digit()) || !frac_part.chars().all(|c| c.is_ascii_digit())
    {
        return Err(format!("{t:?} is not a positive decimal number"));
    }
    if frac_part.len() > 9 || int_part.len() > 9 {
        return Err(format!("{t:?} has too many digits"));
    }
    let digits = format!("{int_part}{frac_part}");
    let num: u64 = digits
        .parse()
        .map_err(|_| format!("{t:?} is not a number"))?;
    let den = 10u64.pow(frac_part.len() as u32);
    Ok((num, den))
}

/// Parse an aspect-ratio spec into an exact, reduced `(width, height)` integer
/// pair suitable for ffmpeg's `-aspect`.
///
/// Accepted forms (all equivalent to 16:9): `16:9`, `16/9`, `1920x1080`,
/// `1.7778`. A single decimal is treated as width ÷ height. Whitespace and a
/// `×` multiplication sign are tolerated.
pub fn parse_ratio(spec: &str) -> Result<(u64, u64), String> {
    let s = spec.trim().replace('×', "x").replace('X', "x");
    if s.is_empty() {
        return Err(
            "aspect ratio is empty; use a form like 16:9, 16/9, 1.85 or 1920x1080".to_string(),
        );
    }
    // Byte index (not char index) so a stray multi-byte char can never split
    // mid-codepoint; the separators themselves are all single-byte ASCII.
    let sep = s.find([':', '/', 'x']);
    let ((wn, wd), (hn, hd)) = match sep {
        Some(i) => {
            let (left, right) = s.split_at(i);
            let right = &right[1..];
            let bad = |e: String| {
                format!("aspect ratio {spec:?} is invalid ({e}); use a form like 16:9, 16/9, 1.85 or 1920x1080")
            };
            (
                parse_decimal(left).map_err(bad)?,
                parse_decimal(right).map_err(bad)?,
            )
        }
        None => (
            parse_decimal(&s).map_err(|e| {
                format!("aspect ratio {spec:?} is invalid ({e}); use a form like 16:9, 16/9, 1.85 or 1920x1080")
            })?,
            (1, 1),
        ),
    };

    // (wn/wd) / (hn/hd) = (wn*hd) / (wd*hn)
    let mut num = wn.saturating_mul(hd);
    let mut den = wd.saturating_mul(hn);
    if num == 0 || den == 0 {
        return Err(format!(
            "aspect ratio {spec:?} must have a non-zero width and height"
        ));
    }
    let g = gcd(num, den);
    num /= g;
    den /= g;

    let value = num as f64 / den as f64;
    if !(MIN_RATIO..=MAX_RATIO).contains(&value) {
        return Err(format!(
            "aspect ratio {spec:?} works out to {value:.4}:1, outside the supported {MIN_RATIO}–{MAX_RATIO} range"
        ));
    }

    // Long decimals (1.7777777) reduce to huge coprime terms; scale them down
    // to a compact equivalent so the container stores a tidy ratio.
    while num > MAX_TERM || den > MAX_TERM {
        num = (num + 1) / 2;
        den = (den + 1) / 2;
        let g = gcd(num, den);
        num /= g;
        den /= g;
    }
    Ok((num, den))
}

/// Resolve the effective ratio from the enum choice + the custom field.
/// `custom_aspect` is only consulted when `aspect` is `custom`.
pub fn resolve_aspect(aspect: &str, custom_aspect: &str) -> Result<(u64, u64), String> {
    let a = aspect.trim();
    if a.is_empty() {
        return Err(format!("aspect is empty; use one of {}", aspect_list()));
    }
    if a.eq_ignore_ascii_case(CUSTOM) {
        if custom_aspect.trim().is_empty() {
            return Err(
                "aspect=custom needs custom_aspect, e.g. 1.85:1, 16/9, 2.35 or 1920x1080"
                    .to_string(),
            );
        }
        return parse_ratio(custom_aspect);
    }
    if !ASPECTS.contains(&a) {
        return Err(format!(
            "aspect {a:?} is not a preset; use one of {} (or aspect=custom with custom_aspect)",
            aspect_list()
        ));
    }
    parse_ratio(a)
}

/// Build the ffmpeg argv (without the leading `ffmpeg`) and the output filename.
///
/// - `aspect` — a preset from [`ASPECTS`], or `custom` to use `custom_aspect`.
/// - `custom_aspect` — free-form ratio (`16:9`, `16/9`, `1.85`, `1920x1080`);
///   ignored unless `aspect` is `custom`.
/// - `container` — `keep` reuses the input extension; otherwise remux into the
///   named container. Stream copy only, so the codecs must fit the container.
/// - `faststart` — MP4/MOV output only: move the `moov` atom to the front so
///   players read the new ratio immediately and the file streams progressively.
pub fn plan(
    aspect: &str,
    custom_aspect: &str,
    container: &str,
    faststart: bool,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let (num, den) = resolve_aspect(aspect, custom_aspect)?;

    let out_ext = match container.trim() {
        CONTAINER_KEEP => extension(in_name).to_ascii_lowercase(),
        c if CONTAINERS.contains(&c) => c.to_string(),
        other => {
            return Err(format!(
                "unknown container {other:?}; use one of {}",
                CONTAINERS.join(", ")
            ))
        }
    };
    let out_name = format!("out.{out_ext}");

    let mut argv: Vec<String> = vec!["-i".to_string(), in_name.to_string()];
    // Keep every stream (video/audio/subtitle) in the rebuilt container.
    argv.extend(["-map".to_string(), "0".to_string()]);
    // Stream copy: no decode, no re-encode — only the container header changes.
    argv.extend(["-c".to_string(), "copy".to_string()]);
    argv.extend(["-aspect".to_string(), format!("{num}:{den}")]);
    if faststart && is_mp4_family(&out_ext) {
        argv.extend(["-movflags".to_string(), "+faststart".to_string()]);
    }
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preset_tags_dar_with_stream_copy_and_keeps_container() {
        let (argv, out) = plan("16:9", "", "keep", true, "in.mkv").unwrap();
        assert_eq!(out, "out.mkv");
        assert_eq!(
            argv,
            vec![
                "-i", "in.mkv", "-map", "0", "-c", "copy", "-aspect", "16:9", "out.mkv"
            ]
            .into_iter()
            .map(String::from)
            .collect::<Vec<_>>()
        );
        // No re-encode flags may ever appear — the whole point of the tool.
        assert!(!argv.iter().any(|a| a == "-c:v" || a == "libx264" || a == "-crf"));
        // faststart is a no-op on a matroska output.
        assert!(!argv.iter().any(|a| a == "-movflags"));
    }

    #[test]
    fn mp4_output_gets_faststart_and_can_be_switched_off() {
        let (argv, out) = plan("9:16", "", "mp4", true, "clip.webm").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.windows(2).any(|w| w[0] == "-movflags" && w[1] == "+faststart"));
        assert!(argv.windows(2).any(|w| w[0] == "-aspect" && w[1] == "9:16"));
        let (argv, _) = plan("9:16", "", "mp4", false, "clip.webm").unwrap();
        assert!(!argv.iter().any(|a| a == "-movflags"));
    }

    #[test]
    fn cinema_presets_normalize_to_exact_integer_ratios() {
        let (argv, _) = plan("2.39:1", "", "keep", false, "in.mp4").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-aspect" && w[1] == "239:100"));
        let (argv, _) = plan("1.85:1", "", "keep", false, "in.mp4").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-aspect" && w[1] == "37:20"));
    }

    #[test]
    fn custom_accepts_colon_slash_decimal_and_dimension_forms() {
        for spec in ["16:9", "16/9", "1920x1080", "1920X1080", "1920×1080"] {
            assert_eq!(parse_ratio(spec).unwrap(), (16, 9), "form {spec}");
        }
        assert_eq!(parse_ratio("1.85").unwrap(), (37, 20));
        assert_eq!(parse_ratio(" 2.35 : 1 ").unwrap(), (47, 20));
        // Square-pixel reset recipe: the stored pixel size reduces to its ratio.
        assert_eq!(parse_ratio("640x480").unwrap(), (4, 3));
    }

    #[test]
    fn custom_field_is_used_only_when_aspect_is_custom() {
        let (argv, _) = plan(CUSTOM, "1920x800", "keep", false, "in.mp4").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-aspect" && w[1] == "12:5"));
        // Ignored for a preset — the preset still wins.
        let (argv, _) = plan("4:3", "1920x800", "keep", false, "in.mp4").unwrap();
        assert!(argv.windows(2).any(|w| w[0] == "-aspect" && w[1] == "4:3"));
    }

    #[test]
    fn long_decimal_reduces_to_a_compact_ratio() {
        let (num, den) = parse_ratio("1.777777777").unwrap();
        assert!(num <= MAX_TERM && den <= MAX_TERM, "got {num}:{den}");
        assert!((num as f64 / den as f64 - 16.0 / 9.0).abs() < 1e-3);
    }

    #[test]
    fn rejects_custom_without_a_value() {
        let err = plan(CUSTOM, "  ", "keep", true, "in.mp4").unwrap_err();
        assert!(err.contains("needs custom_aspect"), "got: {err}");
    }

    #[test]
    fn rejects_zero_negative_and_nonsense_ratios() {
        for spec in ["0:9", "16:0", "-16:9", "wide", "16:9:4", ""] {
            assert!(parse_ratio(spec).is_err(), "{spec:?} should be rejected");
        }
        let err = parse_ratio("100:1").unwrap_err();
        assert!(err.contains("outside the supported"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_aspect_and_container() {
        let err = plan("16:10.5", "", "keep", true, "in.mp4").unwrap_err();
        assert!(err.contains("is not a preset"), "got: {err}");
        let err = plan("16:9", "", "avi", true, "in.mp4").unwrap_err();
        assert!(err.contains("unknown container"), "got: {err}");
    }

    #[test]
    fn unknown_extension_defaults_to_mp4() {
        let (_, out) = plan("16:9", "", "keep", true, "noext").unwrap();
        assert_eq!(out, "out.mp4");
    }
}
