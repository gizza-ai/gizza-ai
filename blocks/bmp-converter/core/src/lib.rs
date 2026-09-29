//! gizza-ai/bmp-converter core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Converts an image **to** an uncompressed Windows BMP at a chosen bit depth,
//! or **from** BMP (or anything else ffmpeg decodes) back out to PNG/JPEG.
//!
//! The BMP side is ffmpeg's native `bmp` encoder driven by `-pix_fmt`:
//!
//! | `bit_depth` | pixel format | `biBitCount` | notes                        |
//! |-------------|--------------|--------------|------------------------------|
//! | `32`        | `bgra`       | 32           | keeps the alpha channel      |
//! | `24`        | `bgr24`      | 24           | classic true colour, default |
//! | `16-565`    | `rgb565le`   | 16           | BI_BITFIELDS masks           |
//! | `16-555`    | `rgb555le`   | 16           | BI_RGB, widest compatibility |
//! | `8`         | `pal8`/`gray`| 8            | indexed palette, or greyscale|
//! | `1`         | `monob`      | 1            | black & white                |
//!
//! Indexed (8-bit palette) output needs an explicit palette — swscale cannot
//! convert RGB to `pal8` on its own — so those runs go through the standard
//! `palettegen`/`paletteuse` pair, which is also where the `colors` (palette
//! size) and `dither` controls take effect. 1-bit output is a greyscale
//! threshold in swscale, so `dither` maps onto `-sws_dither` there instead.
//!
//! Every depth below 32 drops alpha, so transparent inputs are flattened onto
//! `background` first via `split`/`drawbox`/`overlay` (a self-sized solid layer
//! — no second input file, which the single-input dispatch contract forbids).
//!
//! The whole `-vf` graph is a single argv token (no spaces).

/// The canonical output-format names, in display order. KEEP IN SYNC with
/// `parse_format`.
pub const FORMATS: [&str; 3] = ["bmp", "png", "jpeg"];

/// The canonical bit-depth names, in display order. KEEP IN SYNC with
/// `parse_depth`.
pub const BIT_DEPTHS: [&str; 6] = ["1", "8", "16-555", "16-565", "24", "32"];

/// The canonical dither names, in display order. KEEP IN SYNC with
/// `parse_dither`.
pub const DITHERS: [&str; 4] = ["none", "bayer", "floyd_steinberg", "sierra2_4a"];

/// Default output format: write a BMP.
pub const DEFAULT_FORMAT: &str = "bmp";
/// Default bit depth: 24-bit true colour, the most widely readable BMP.
pub const DEFAULT_BIT_DEPTH: &str = "24";
/// Default palette size for 8-bit indexed output: the full 256 entries.
pub const DEFAULT_COLORS: i64 = 256;
/// Default dithering for reduced-colour output.
pub const DEFAULT_DITHER: &str = "floyd_steinberg";
/// Default flatten colour for transparent inputs: white.
pub const DEFAULT_BACKGROUND: &str = "#ffffff";
/// Default greyscale toggle: off (keep colour).
pub const DEFAULT_GRAYSCALE: bool = false;

/// Smallest / largest palette used by 8-bit indexed output.
pub const MIN_COLORS: i64 = 2;
pub const MAX_COLORS: i64 = 256;

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OutFormat {
    Bmp,
    Png,
    Jpeg,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum BitDepth {
    /// 1-bit black & white (`monob`).
    Mono,
    /// 8-bit — indexed palette, or greyscale when `grayscale` is on.
    Eight,
    /// 16-bit 5:5:5 high colour (`rgb555le`, BI_RGB).
    Sixteen555,
    /// 16-bit 5:6:5 high colour (`rgb565le`, BI_BITFIELDS).
    Sixteen565,
    /// 24-bit true colour (`bgr24`).
    TwentyFour,
    /// 32-bit true colour with alpha (`bgra`).
    ThirtyTwo,
}

impl BitDepth {
    /// The canonical name, as accepted by `parse_depth` and listed in the schema.
    pub fn name(self) -> &'static str {
        match self {
            BitDepth::Mono => "1",
            BitDepth::Eight => "8",
            BitDepth::Sixteen555 => "16-555",
            BitDepth::Sixteen565 => "16-565",
            BitDepth::TwentyFour => "24",
            BitDepth::ThirtyTwo => "32",
        }
    }

    /// `biBitCount` the BMP encoder writes for this depth.
    pub fn bits(self) -> u32 {
        match self {
            BitDepth::Mono => 1,
            BitDepth::Eight => 8,
            BitDepth::Sixteen555 | BitDepth::Sixteen565 => 16,
            BitDepth::TwentyFour => 24,
            BitDepth::ThirtyTwo => 32,
        }
    }

    /// Whether the written BMP keeps an alpha channel. Only 32-bit does.
    pub fn keeps_alpha(self) -> bool {
        self == BitDepth::ThirtyTwo
    }

    /// The ffmpeg `-pix_fmt` for this depth. 8-bit picks `gray` (a true 8-bit
    /// greyscale BMP) over `pal8` when greyscale is requested.
    pub fn pix_fmt(self, grayscale: bool) -> &'static str {
        match self {
            BitDepth::Mono => "monob",
            BitDepth::Eight if grayscale => "gray",
            BitDepth::Eight => "pal8",
            BitDepth::Sixteen555 => "rgb555le",
            BitDepth::Sixteen565 => "rgb565le",
            BitDepth::TwentyFour => "bgr24",
            BitDepth::ThirtyTwo => "bgra",
        }
    }

    /// True when the run needs an explicit generated palette (`palettegen` +
    /// `paletteuse`) — i.e. 8-bit colour output. Greyscale 8-bit goes through
    /// swscale's `gray` instead and needs no palette.
    pub fn needs_palette(self, grayscale: bool) -> bool {
        self == BitDepth::Eight && !grayscale
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Dither {
    None,
    Bayer,
    FloydSteinberg,
    Sierra2_4a,
}

impl Dither {
    /// The `paletteuse=dither=` value for indexed output.
    pub fn paletteuse(self) -> &'static str {
        match self {
            Dither::None => "none",
            Dither::Bayer => "bayer",
            Dither::FloydSteinberg => "floyd_steinberg",
            Dither::Sierra2_4a => "sierra2_4a",
        }
    }

    /// The `-sws_dither` value used for the 1-bit threshold, where there is no
    /// palette to dither against. Both error-diffusion choices map onto `ed`,
    /// which is swscale's only error-diffusion mode.
    pub fn sws(self) -> &'static str {
        match self {
            Dither::None => "none",
            Dither::Bayer => "bayer",
            Dither::FloydSteinberg | Dither::Sierra2_4a => "ed",
        }
    }
}

/// Parse an output format name (case-insensitive; `jpg` is an alias for
/// `jpeg`). `None` / `""` default to `bmp`.
pub fn parse_format(s: Option<&str>) -> Result<OutFormat, String> {
    let v = s.unwrap_or(DEFAULT_FORMAT).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "bmp" => Ok(OutFormat::Bmp),
        "png" => Ok(OutFormat::Png),
        "jpeg" | "jpg" => Ok(OutFormat::Jpeg),
        other => Err(format!(
            "invalid format {other:?}; expected one of {}",
            FORMATS.join("|")
        )),
    }
}

/// Parse a bit-depth name (case-insensitive). `16` alone is accepted as a
/// friendly alias for the BI_BITFIELDS 5:6:5 layout. `None` / `""` default
/// to `24`.
pub fn parse_depth(s: Option<&str>) -> Result<BitDepth, String> {
    let v = s.unwrap_or(DEFAULT_BIT_DEPTH).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "24" => Ok(BitDepth::TwentyFour),
        "1" => Ok(BitDepth::Mono),
        "8" => Ok(BitDepth::Eight),
        "16-555" | "16555" | "555" => Ok(BitDepth::Sixteen555),
        "16" | "16-565" | "16565" | "565" => Ok(BitDepth::Sixteen565),
        "32" => Ok(BitDepth::ThirtyTwo),
        other => Err(format!(
            "invalid bit_depth {other:?}; expected one of {}",
            BIT_DEPTHS.join("|")
        )),
    }
}

/// Parse a dither name (case-insensitive; `fs` and `floyd-steinberg` are
/// accepted aliases). `None` / `""` default to `floyd_steinberg`.
pub fn parse_dither(s: Option<&str>) -> Result<Dither, String> {
    let v = s.unwrap_or(DEFAULT_DITHER).trim().to_ascii_lowercase();
    match v.as_str() {
        "" | "floyd_steinberg" | "floyd-steinberg" | "fs" => Ok(Dither::FloydSteinberg),
        "none" | "off" => Ok(Dither::None),
        "bayer" | "ordered" => Ok(Dither::Bayer),
        "sierra2_4a" | "sierra2-4a" | "sierra" => Ok(Dither::Sierra2_4a),
        other => Err(format!(
            "invalid dither {other:?}; expected one of {}",
            DITHERS.join("|")
        )),
    }
}

/// Parse a boolean toggle from a page/CLI string. `None`/`""` default to
/// `default_val`; positive-truthy (`true/1/on/yes`) → true, the negative forms
/// (`false/0/off/no`) → false; anything else is a guided error.
pub fn parse_bool(s: Option<&str>, default_val: bool) -> Result<bool, String> {
    match s.map(|v| v.trim().to_ascii_lowercase()) {
        None => Ok(default_val),
        Some(v) if v.is_empty() => Ok(default_val),
        Some(v) => match v.as_str() {
            "true" | "1" | "on" | "yes" => Ok(true),
            "false" | "0" | "off" | "no" => Ok(false),
            other => Err(format!("invalid boolean {other:?}; expected true or false")),
        },
    }
}

/// Validate the palette size for 8-bit indexed output (2–256). A cleared page
/// field arrives as `0`, which falls back to the default 256.
pub fn parse_colors(colors: f64) -> Result<u32, String> {
    if !colors.is_finite() {
        return Err(format!(
            "colors must be a whole number between {MIN_COLORS} and {MAX_COLORS}, got {colors}"
        ));
    }
    let n = colors.round() as i64;
    if n == 0 {
        return Ok(DEFAULT_COLORS as u32);
    }
    if !(MIN_COLORS..=MAX_COLORS).contains(&n) {
        return Err(format!(
            "colors must be between {MIN_COLORS} and {MAX_COLORS}, got {n}"
        ));
    }
    Ok(n as u32)
}

/// Normalise a colour for ffmpeg's `drawbox=c=`.
///
/// `#f00`, `f00`, `#ff0000`, `FF0000` and `0xff0000` all normalise to
/// `0xff0000`; anything else (e.g. `white`, `navy`) is passed through as an
/// ffmpeg named colour after a character check. Short hex is expanded rather
/// than forwarded, because ffmpeg reads a bare 3-digit hex as a *name* and
/// falls back to white.
pub fn normalize_color(s: Option<&str>) -> Result<String, String> {
    let raw = s.unwrap_or(DEFAULT_BACKGROUND).trim();
    let raw = if raw.is_empty() {
        DEFAULT_BACKGROUND
    } else {
        raw
    };
    let hex = raw
        .strip_prefix('#')
        .or_else(|| raw.strip_prefix("0x"))
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);
    let is_hex = !hex.is_empty() && hex.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex && (hex.len() == 3 || hex.len() == 6) {
        let full = if hex.len() == 3 {
            hex.chars().flat_map(|c| [c, c]).collect::<String>()
        } else {
            hex.to_string()
        };
        return Ok(format!("0x{}", full.to_ascii_lowercase()));
    }
    // A named ffmpeg colour: letters only (plus an optional @alpha suffix is
    // NOT accepted here — the filter string appends its own opacity).
    if raw.chars().all(|c| c.is_ascii_alphabetic()) && !raw.is_empty() {
        return Ok(raw.to_ascii_lowercase());
    }
    Err(format!(
        "invalid background {raw:?}; expected a hex colour like #ffffff or #f00, \
         or an ffmpeg colour name like white"
    ))
}

/// The `-vf` filtergraph for one conversion. A single argv token (no spaces).
///
/// Ordering matters: flatten transparency first (so the palette is generated
/// from what the viewer will actually see), then greyscale, then quantize.
pub fn filter(
    format: OutFormat,
    depth: BitDepth,
    colors: u32,
    dither: Dither,
    grayscale: bool,
    background: &str,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    // Does the OUTPUT keep alpha? Only 32-bit BMP and PNG do.
    let keeps_alpha = match format {
        OutFormat::Bmp => depth.keeps_alpha(),
        OutFormat::Png => true,
        OutFormat::Jpeg => false,
    };
    if !keeps_alpha {
        // Composite over a self-sized solid layer: split the frame, paint one
        // copy fully with the background colour, overlay the original on top.
        // No second input file — the dispatch contract allows only one.
        parts.push(format!(
            "format=rgba,split[bgsrc][fg];[bgsrc]drawbox=c={background}@1:t=fill[bg];\
             [bg][fg]overlay=format=auto"
        ));
    }
    if grayscale {
        parts.push("format=gray".to_string());
    }
    if format == OutFormat::Bmp && depth.needs_palette(grayscale) {
        parts.push(format!(
            "split[pqsrc][pquse];[pqsrc]palettegen=max_colors={colors}[pal];\
             [pquse][pal]paletteuse=dither={}",
            dither.paletteuse()
        ));
    }
    parts.join(",").replace(['\n', ' '], "")
}

/// The output filename for `format`.
fn out_name_for(format: OutFormat) -> &'static str {
    match format {
        OutFormat::Bmp => "out.bmp",
        OutFormat::Png => "out.png",
        OutFormat::Jpeg => "out.jpg",
    }
}

/// Build the ffmpeg argv (no leading "ffmpeg") and the output filename.
///
/// `in_name` is the virtual-FS name of the uploaded/fetched image. The output
/// always has the input's exact pixel dimensions — this tool never resizes.
#[allow(clippy::too_many_arguments)]
pub fn plan(
    in_name: &str,
    format: OutFormat,
    depth: BitDepth,
    colors: u32,
    dither: Dither,
    grayscale: bool,
    background: &str,
) -> Result<(Vec<String>, String), String> {
    if in_name.trim().is_empty() {
        return Err("input filename must not be empty".to_string());
    }
    if !(MIN_COLORS as u32..=MAX_COLORS as u32).contains(&colors) {
        return Err(format!(
            "colors must be between {MIN_COLORS} and {MAX_COLORS}, got {colors}"
        ));
    }
    let out_name = out_name_for(format).to_string();
    let mut argv: Vec<String> = Vec::new();
    if format == OutFormat::Bmp && depth == BitDepth::Mono {
        // The 1-bit path is a swscale threshold, so dithering is selected with
        // the scaler option rather than paletteuse. It must precede -i.
        argv.push("-sws_dither".to_string());
        argv.push(dither.sws().to_string());
    }
    argv.push("-i".to_string());
    argv.push(in_name.to_string());
    let vf = filter(format, depth, colors, dither, grayscale, background);
    if !vf.is_empty() {
        argv.push("-vf".to_string());
        argv.push(vf);
    }
    match format {
        OutFormat::Bmp => {
            argv.push("-pix_fmt".to_string());
            argv.push(depth.pix_fmt(grayscale).to_string());
            argv.push("-c:v".to_string());
            argv.push("bmp".to_string());
        }
        OutFormat::Png => {
            argv.push("-pix_fmt".to_string());
            argv.push(if grayscale { "gray" } else { "rgba" }.to_string());
            argv.push("-c:v".to_string());
            argv.push("png".to_string());
        }
        OutFormat::Jpeg => {
            // mjpeg's default quality is visibly lossy; pin a high-quality encode.
            argv.push("-q:v".to_string());
            argv.push("2".to_string());
        }
    }
    // Animated inputs (GIF/WebP) would otherwise fail in the image muxer.
    argv.push("-frames:v".to_string());
    argv.push("1".to_string());
    argv.push("-f".to_string());
    argv.push("image2".to_string());
    argv.push(out_name.clone());
    Ok((argv, out_name))
}

/// Parse + plan in one step from raw strings (used by the web page and the CLI
/// paths, where every field arrives as text).
pub fn plan_named(
    in_name: &str,
    format: Option<&str>,
    bit_depth: Option<&str>,
    colors: f64,
    dither: Option<&str>,
    grayscale: Option<&str>,
    background: Option<&str>,
) -> Result<(Vec<String>, String), String> {
    let format = parse_format(format)?;
    let depth = parse_depth(bit_depth)?;
    let colors = parse_colors(colors)?;
    let dither = parse_dither(dither)?;
    let grayscale = parse_bool(grayscale, DEFAULT_GRAYSCALE)?;
    let background = normalize_color(background)?;
    plan(
        in_name, format, depth, colors, dither, grayscale, &background,
    )
}

/// A one-line human/LLM summary of what a run produced.
pub fn summary(format: OutFormat, depth: BitDepth, grayscale: bool) -> String {
    match format {
        OutFormat::Bmp => {
            let kind = if grayscale && depth == BitDepth::Eight {
                "greyscale"
            } else if depth == BitDepth::Mono {
                "black & white"
            } else if depth == BitDepth::Eight {
                "indexed"
            } else if depth.keeps_alpha() {
                "true colour + alpha"
            } else {
                "true colour"
            };
            format!(
                "uncompressed {}-bit {kind} BMP",
                depth.bits()
            )
        }
        OutFormat::Png => "PNG".to_string(),
        OutFormat::Jpeg => "JPEG".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_default_is_24bit_uncompressed_bmp() {
        let (argv, out) = plan(
            "in.png",
            OutFormat::Bmp,
            BitDepth::TwentyFour,
            256,
            Dither::FloydSteinberg,
            false,
            "0xffffff",
        )
        .unwrap();
        assert_eq!(out, "out.bmp");
        assert_eq!(&argv[0], "-i");
        assert_eq!(&argv[1], "in.png");
        assert_eq!(&argv[2], "-vf");
        // 24-bit drops alpha → the flatten graph runs, no palette graph.
        assert!(argv[3].contains("drawbox=c=0xffffff@1:t=fill"), "{}", argv[3]);
        assert!(!argv[3].contains("palettegen"), "{}", argv[3]);
        assert!(argv.windows(2).any(|w| w == ["-pix_fmt", "bgr24"]), "{argv:?}");
        assert!(argv.windows(2).any(|w| w == ["-c:v", "bmp"]), "{argv:?}");
        assert_eq!(argv.last().unwrap(), "out.bmp");
    }

    #[test]
    fn plan_rejects_out_of_range_palette() {
        let err = plan(
            "in.png",
            OutFormat::Bmp,
            BitDepth::Eight,
            1000,
            Dither::None,
            false,
            "0xffffff",
        )
        .unwrap_err();
        assert!(err.contains("256") && err.contains("1000"), "{err}");
        assert!(plan(
            "",
            OutFormat::Bmp,
            BitDepth::TwentyFour,
            256,
            Dither::None,
            false,
            "0xffffff"
        )
        .is_err());
    }

    #[test]
    fn every_depth_maps_to_the_right_pix_fmt_and_bit_count() {
        let expected = [
            (BitDepth::Mono, "monob", 1u32),
            (BitDepth::Eight, "pal8", 8),
            (BitDepth::Sixteen555, "rgb555le", 16),
            (BitDepth::Sixteen565, "rgb565le", 16),
            (BitDepth::TwentyFour, "bgr24", 24),
            (BitDepth::ThirtyTwo, "bgra", 32),
        ];
        for (depth, pix, bits) in expected {
            assert_eq!(depth.pix_fmt(false), pix, "{depth:?}");
            assert_eq!(depth.bits(), bits, "{depth:?}");
            let (argv, _) = plan(
                "in.png",
                OutFormat::Bmp,
                depth,
                256,
                Dither::None,
                false,
                "0xffffff",
            )
            .unwrap();
            assert!(
                argv.windows(2).any(|w| w == ["-pix_fmt", pix]),
                "{depth:?} → {argv:?}"
            );
        }
        // Greyscale 8-bit writes a real greyscale BMP, not an indexed one.
        assert_eq!(BitDepth::Eight.pix_fmt(true), "gray");
    }

    #[test]
    fn indexed_output_generates_a_palette_and_honours_colors_and_dither() {
        let (argv, _) = plan(
            "in.png",
            OutFormat::Bmp,
            BitDepth::Eight,
            16,
            Dither::Bayer,
            false,
            "0xffffff",
        )
        .unwrap();
        let vf = &argv[3];
        assert!(vf.contains("palettegen=max_colors=16"), "{vf}");
        assert!(vf.contains("paletteuse=dither=bayer"), "{vf}");
        // Greyscale 8-bit skips the palette entirely.
        let (argv, _) = plan(
            "in.png",
            OutFormat::Bmp,
            BitDepth::Eight,
            16,
            Dither::Bayer,
            true,
            "0xffffff",
        )
        .unwrap();
        assert!(!argv[3].contains("palettegen"), "{}", argv[3]);
        assert!(argv[3].contains("format=gray"), "{}", argv[3]);
        assert!(argv.windows(2).any(|w| w == ["-pix_fmt", "gray"]), "{argv:?}");
    }

    #[test]
    fn mono_uses_the_scaler_dither_option_before_the_input() {
        let (argv, _) = plan(
            "in.png",
            OutFormat::Bmp,
            BitDepth::Mono,
            256,
            Dither::None,
            false,
            "0xffffff",
        )
        .unwrap();
        assert_eq!(&argv[0], "-sws_dither");
        assert_eq!(&argv[1], "none");
        assert_eq!(&argv[2], "-i");
        assert!(argv.windows(2).any(|w| w == ["-pix_fmt", "monob"]), "{argv:?}");
        // Error diffusion maps onto swscale's single `ed` mode.
        let (argv, _) = plan(
            "in.png",
            OutFormat::Bmp,
            BitDepth::Mono,
            256,
            Dither::Sierra2_4a,
            false,
            "0xffffff",
        )
        .unwrap();
        assert_eq!(&argv[1], "ed");
        // Non-mono depths never pass -sws_dither.
        let (argv, _) = plan(
            "in.png",
            OutFormat::Bmp,
            BitDepth::TwentyFour,
            256,
            Dither::None,
            false,
            "0xffffff",
        )
        .unwrap();
        assert!(!argv.contains(&"-sws_dither".to_string()), "{argv:?}");
    }

    #[test]
    fn thirty_two_bit_and_png_keep_alpha_so_no_flatten_runs() {
        let (argv, _) = plan(
            "in.png",
            OutFormat::Bmp,
            BitDepth::ThirtyTwo,
            256,
            Dither::None,
            false,
            "0xff0000",
        )
        .unwrap();
        assert!(!argv[3].contains("drawbox"), "{}", argv[3]);
        assert!(argv.windows(2).any(|w| w == ["-pix_fmt", "bgra"]), "{argv:?}");
        let (argv, out) = plan(
            "in.bmp",
            OutFormat::Png,
            BitDepth::TwentyFour,
            256,
            Dither::None,
            false,
            "0xff0000",
        )
        .unwrap();
        assert_eq!(out, "out.png");
        assert!(!argv.contains(&"-vf".to_string()), "{argv:?}");
        assert!(argv.windows(2).any(|w| w == ["-pix_fmt", "rgba"]), "{argv:?}");
        assert!(argv.windows(2).any(|w| w == ["-c:v", "png"]), "{argv:?}");
    }

    #[test]
    fn jpeg_output_flattens_and_pins_quality() {
        let (argv, out) = plan(
            "in.bmp",
            OutFormat::Jpeg,
            BitDepth::TwentyFour,
            256,
            Dither::None,
            false,
            "0x000000",
        )
        .unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv[3].contains("drawbox=c=0x000000@1"), "{}", argv[3]);
        assert!(argv.windows(2).any(|w| w == ["-q:v", "2"]), "{argv:?}");
        assert!(!argv.contains(&"-c:v".to_string()), "{argv:?}");
    }

    #[test]
    fn filter_is_a_single_argv_token() {
        for depth in [
            BitDepth::Mono,
            BitDepth::Eight,
            BitDepth::Sixteen555,
            BitDepth::Sixteen565,
            BitDepth::TwentyFour,
            BitDepth::ThirtyTwo,
        ] {
            for gray in [false, true] {
                let f = filter(
                    OutFormat::Bmp,
                    depth,
                    64,
                    Dither::FloydSteinberg,
                    gray,
                    "0xffffff",
                );
                assert!(!f.contains(' '), "{depth:?}/{gray}: {f}");
                assert!(!f.is_empty() || depth == BitDepth::ThirtyTwo);
            }
        }
    }

    #[test]
    fn normalize_color_expands_short_hex_and_accepts_names() {
        assert_eq!(normalize_color(Some("#f00")).unwrap(), "0xff0000");
        assert_eq!(normalize_color(Some("f00")).unwrap(), "0xff0000");
        assert_eq!(normalize_color(Some("#FF0000")).unwrap(), "0xff0000");
        assert_eq!(normalize_color(Some("0xFF0000")).unwrap(), "0xff0000");
        assert_eq!(normalize_color(Some(" #3366ff ")).unwrap(), "0x3366ff");
        assert_eq!(normalize_color(Some("white")).unwrap(), "white");
        assert_eq!(normalize_color(None).unwrap(), "0xffffff");
        assert_eq!(normalize_color(Some("")).unwrap(), "0xffffff");
    }

    #[test]
    fn normalize_color_rejects_junk() {
        let err = normalize_color(Some("#12345")).unwrap_err();
        assert!(err.contains("#ffffff"), "{err}");
        assert!(normalize_color(Some("rgb(1,2,3)")).is_err());
        assert!(normalize_color(Some("#ggg")).is_err());
    }

    #[test]
    fn parse_format_default_and_aliases() {
        assert_eq!(parse_format(None).unwrap(), OutFormat::Bmp);
        assert_eq!(parse_format(Some("")).unwrap(), OutFormat::Bmp);
        assert_eq!(parse_format(Some("BMP")).unwrap(), OutFormat::Bmp);
        assert_eq!(parse_format(Some("jpg")).unwrap(), OutFormat::Jpeg);
        assert_eq!(parse_format(Some("jpeg")).unwrap(), OutFormat::Jpeg);
        assert_eq!(parse_format(Some("png")).unwrap(), OutFormat::Png);
        let err = parse_format(Some("tiff")).unwrap_err();
        assert!(err.contains("bmp") && err.contains("png"), "{err}");
    }

    #[test]
    fn parse_depth_default_and_aliases() {
        assert_eq!(parse_depth(None).unwrap(), BitDepth::TwentyFour);
        assert_eq!(parse_depth(Some("")).unwrap(), BitDepth::TwentyFour);
        assert_eq!(parse_depth(Some("16")).unwrap(), BitDepth::Sixteen565);
        assert_eq!(parse_depth(Some("16-555")).unwrap(), BitDepth::Sixteen555);
        assert_eq!(parse_depth(Some("16-565")).unwrap(), BitDepth::Sixteen565);
        assert_eq!(parse_depth(Some("1")).unwrap(), BitDepth::Mono);
        assert_eq!(parse_depth(Some("32")).unwrap(), BitDepth::ThirtyTwo);
        let err = parse_depth(Some("4")).unwrap_err();
        assert!(err.contains("16-555") && err.contains("24"), "{err}");
    }

    #[test]
    fn parse_dither_default_and_aliases() {
        assert_eq!(parse_dither(None).unwrap(), Dither::FloydSteinberg);
        assert_eq!(parse_dither(Some("ordered")).unwrap(), Dither::Bayer);
        assert_eq!(parse_dither(Some("NONE")).unwrap(), Dither::None);
        assert_eq!(parse_dither(Some("sierra2_4a")).unwrap(), Dither::Sierra2_4a);
        let err = parse_dither(Some("riemersma")).unwrap_err();
        assert!(err.contains("bayer"), "{err}");
    }

    #[test]
    fn enum_consts_round_trip_their_parsers() {
        for name in FORMATS {
            assert!(parse_format(Some(name)).is_ok(), "FORMATS entry {name}");
        }
        for name in BIT_DEPTHS {
            let d = parse_depth(Some(name)).expect("BIT_DEPTHS entry");
            assert_eq!(d.name(), name, "name() must round-trip {name}");
        }
        for name in DITHERS {
            assert!(parse_dither(Some(name)).is_ok(), "DITHERS entry {name}");
        }
    }

    #[test]
    fn parse_colors_clamps_cleared_field_and_rejects_bad_values() {
        assert_eq!(parse_colors(0.0).unwrap(), 256); // cleared page field
        assert_eq!(parse_colors(2.0).unwrap(), 2);
        assert_eq!(parse_colors(256.0).unwrap(), 256);
        assert_eq!(parse_colors(15.6).unwrap(), 16);
        let err = parse_colors(257.0).unwrap_err();
        assert!(err.contains("256"), "{err}");
        assert!(parse_colors(1.0).is_err());
        assert!(parse_colors(f64::NAN).is_err());
    }

    #[test]
    fn parse_bool_forms_and_errors() {
        assert!(!parse_bool(None, false).unwrap());
        assert!(parse_bool(Some("true"), false).unwrap());
        assert!(parse_bool(Some(" ON "), false).unwrap());
        assert!(!parse_bool(Some("0"), true).unwrap());
        let err = parse_bool(Some("maybe"), false).unwrap_err();
        assert!(err.contains("true") && err.contains("false"), "{err}");
    }

    #[test]
    fn plan_named_parses_every_field_from_text() {
        let (argv, out) =
            plan_named("in.png", Some("bmp"), Some("8"), 16.0, Some("none"), Some("false"), Some("#f00"))
                .unwrap();
        assert_eq!(out, "out.bmp");
        assert!(argv[3].contains("drawbox=c=0xff0000@1"), "{}", argv[3]);
        assert!(argv[3].contains("palettegen=max_colors=16"), "{}", argv[3]);
        assert!(argv[3].contains("paletteuse=dither=none"), "{}", argv[3]);
        // Empty strings everywhere = the descriptor defaults.
        let (argv, out) =
            plan_named("in.bmp", Some(""), Some(""), 0.0, Some(""), Some(""), Some("")).unwrap();
        assert_eq!(out, "out.bmp");
        assert!(argv.windows(2).any(|w| w == ["-pix_fmt", "bgr24"]), "{argv:?}");
        // Bad values are rejected with a guided message.
        assert!(plan_named("in.png", Some("gif"), None, 0.0, None, None, None).is_err());
        assert!(plan_named("in.png", None, Some("4"), 0.0, None, None, None).is_err());
        assert!(plan_named("in.png", None, None, 300.0, None, None, None).is_err());
        assert!(plan_named("in.png", None, None, 0.0, Some("nope"), None, None).is_err());
        assert!(plan_named("in.png", None, None, 0.0, None, Some("maybe"), None).is_err());
        assert!(plan_named("in.png", None, None, 0.0, None, None, Some("#12345")).is_err());
    }

    #[test]
    fn summary_describes_the_output() {
        assert_eq!(
            summary(OutFormat::Bmp, BitDepth::TwentyFour, false),
            "uncompressed 24-bit true colour BMP"
        );
        assert_eq!(
            summary(OutFormat::Bmp, BitDepth::ThirtyTwo, false),
            "uncompressed 32-bit true colour + alpha BMP"
        );
        assert_eq!(
            summary(OutFormat::Bmp, BitDepth::Eight, false),
            "uncompressed 8-bit indexed BMP"
        );
        assert_eq!(
            summary(OutFormat::Bmp, BitDepth::Eight, true),
            "uncompressed 8-bit greyscale BMP"
        );
        assert_eq!(
            summary(OutFormat::Bmp, BitDepth::Mono, false),
            "uncompressed 1-bit black & white BMP"
        );
        assert_eq!(summary(OutFormat::Png, BitDepth::TwentyFour, false), "PNG");
    }
}
