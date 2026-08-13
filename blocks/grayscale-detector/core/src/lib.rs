//! grayscale-detector core — detect whether decoded image pixels are effectively grayscale.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and web page.

use base64::{engine::general_purpose, Engine as _};

pub const MAX_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_SAMPLES_CAP: u32 = 200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum InputFormat {
    Base64,
    Hex,
}

impl InputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "base64" => Ok(Self::Base64),
            "hex" => Ok(Self::Hex),
            other => Err(format!(
                "unknown input_format {other:?} — expected base64 or hex"
            )),
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Metric {
    ChannelDelta,
    Saturation,
}

impl Metric {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "channel_delta" => Ok(Self::ChannelDelta),
            "saturation" => Ok(Self::Saturation),
            other => Err(format!(
                "unknown metric {other:?} — expected channel_delta or saturation"
            )),
        }
    }

    /// The per-pixel colorfulness score, always on a 0-255 scale.
    fn score(self, r: u8, g: u8, b: u8) -> u8 {
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let delta = max - min;
        match self {
            Self::ChannelDelta => delta,
            // HSV saturation, scaled to 0-255: a dark tinted pixel scores much
            // higher here than its raw channel delta.
            Self::Saturation => {
                if max == 0 {
                    0
                } else {
                    ((delta as u32 * 255) / max as u32) as u8
                }
            }
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::ChannelDelta => "channel_delta",
            Self::Saturation => "saturation",
        }
    }

    /// Header line describing how the score is computed.
    fn headline(self) -> &'static str {
        match self {
            Self::ChannelDelta => "RGB channel delta (max - min of R, G, B)",
            Self::Saturation => "HSV saturation on a 0-255 scale ((max - min) / max)",
        }
    }

    /// Short label reused for the max/mean rows and the per-sample score.
    fn label(self) -> &'static str {
        match self {
            Self::ChannelDelta => "RGB channel delta",
            Self::Saturation => "HSV saturation",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum OutputFormat {
    Report,
    Json,
}

impl OutputFormat {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "report" => Ok(Self::Report),
            "json" => Ok(Self::Json),
            other => Err(format!("unknown output {other:?} — expected report or json")),
        }
    }
}

#[derive(Debug, Clone)]
struct Sample {
    x: u32,
    y: u32,
    r: u8,
    g: u8,
    b: u8,
    score: u8,
}

impl Sample {
    fn hex(&self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

#[derive(Debug)]
struct Analysis {
    width: u32,
    height: u32,
    pixels: u64,
    metric: Metric,
    tolerance: u8,
    ignore_alpha: bool,
    scanned: u64,
    transparent_skipped: u64,
    colorish_pixels: u64,
    max_score: u8,
    sum_score: u64,
    samples: Vec<Sample>,
}

impl Analysis {
    fn effective_grayscale(&self) -> bool {
        self.colorish_pixels == 0
    }

    fn gray_pixels(&self) -> u64 {
        self.scanned - self.colorish_pixels
    }

    fn percent(&self, count: u64) -> f64 {
        if self.scanned == 0 {
            0.0
        } else {
            count as f64 * 100.0 / self.scanned as f64
        }
    }

    fn mean_score(&self) -> f64 {
        if self.scanned == 0 {
            0.0
        } else {
            self.sum_score as f64 / self.scanned as f64
        }
    }
}

fn decode_input(input: &str, format: InputFormat) -> Result<Vec<u8>, String> {
    let compact: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.is_empty() {
        return Err("input is empty — paste image bytes encoded as base64 or hex".to_string());
    }
    let bytes = match format {
        InputFormat::Base64 => general_purpose::STANDARD
            .decode(compact.as_bytes())
            .or_else(|_| general_purpose::URL_SAFE.decode(compact.as_bytes()))
            .map_err(|e| format!("could not decode base64 input: {e}"))?,
        InputFormat::Hex => {
            let clean: String = compact
                .chars()
                .filter(|c| *c != ':' && *c != '-' && *c != '_')
                .collect();
            if clean.len() % 2 != 0 {
                return Err("hex input has an odd number of digits".to_string());
            }
            let mut out = Vec::with_capacity(clean.len() / 2);
            for i in (0..clean.len()).step_by(2) {
                let byte = u8::from_str_radix(&clean[i..i + 2], 16)
                    .map_err(|_| format!("invalid hex byte {:?}", &clean[i..i + 2]))?;
                out.push(byte);
            }
            out
        }
    };
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes, which exceeds the maximum of {MAX_BYTES}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn analyze(
    bytes: &[u8],
    metric: Metric,
    tolerance: u8,
    ignore_alpha: bool,
    max_samples: u32,
) -> Result<Analysis, String> {
    let img = image::load_from_memory(bytes).map_err(|e| format!("could not decode image: {e}"))?;
    let rgba = img.to_rgba8();
    let (width, height) = rgba.dimensions();
    if width == 0 || height == 0 {
        return Err("image has zero size".to_string());
    }
    let pixels = width as u64 * height as u64;
    let mut scanned = 0u64;
    let mut transparent_skipped = 0u64;
    let mut colorish_pixels = 0u64;
    let mut max_score = 0u8;
    let mut sum_score = 0u64;
    let mut samples = Vec::new();

    for (x, y, p) in rgba.enumerate_pixels() {
        let [r, g, b, a] = p.0;
        // With ignore_alpha off, a fully transparent pixel carries no visible
        // colour, so it is excluded from the verdict rather than counted gray.
        if !ignore_alpha && a == 0 {
            transparent_skipped += 1;
            continue;
        }
        scanned += 1;
        let score = metric.score(r, g, b);
        max_score = max_score.max(score);
        sum_score += score as u64;
        if score > tolerance {
            colorish_pixels += 1;
            if (samples.len() as u32) < max_samples {
                samples.push(Sample { x, y, r, g, b, score });
            }
        }
    }

    Ok(Analysis {
        width,
        height,
        pixels,
        metric,
        tolerance,
        ignore_alpha,
        scanned,
        transparent_skipped,
        colorish_pixels,
        max_score,
        sum_score,
        samples,
    })
}

fn render_report(a: &Analysis) -> String {
    let status = if a.effective_grayscale() {
        "effectively grayscale"
    } else {
        "contains color pixels"
    };
    let mut lines = vec![
        format!("Status: {status}"),
        format!(
            "Dimensions: {}×{} ({} pixels)",
            a.width, a.height, a.pixels
        ),
        format!("Metric: {}", a.metric.headline()),
        format!("Tolerance: {}", a.tolerance),
        format!("Scanned pixels: {}", a.scanned),
    ];
    if !a.ignore_alpha {
        lines.push(format!(
            "Transparent pixels skipped: {}",
            a.transparent_skipped
        ));
    }
    lines.push(format!(
        "Gray pixels: {} ({:.4}%)",
        a.gray_pixels(),
        a.percent(a.gray_pixels())
    ));
    lines.push(format!(
        "Color pixels: {} ({:.4}%)",
        a.colorish_pixels,
        a.percent(a.colorish_pixels)
    ));
    lines.push(format!("Max {}: {}", a.metric.label(), a.max_score));
    lines.push(format!(
        "Mean {}: {:.4}",
        a.metric.label(),
        a.mean_score()
    ));
    if a.scanned == 0 {
        lines.push(
            "Suggestion: every pixel is fully transparent — there is nothing visible to judge."
                .to_string(),
        );
    } else if a.effective_grayscale() {
        lines.push(
            "Suggestion: safe to store as a grayscale (single-channel) image at this tolerance."
                .to_string(),
        );
    } else {
        if !a.samples.is_empty() {
            let samples: Vec<String> = a
                .samples
                .iter()
                .map(|s| format!("({},{}) {} rgb({},{},{}) score {}", s.x, s.y, s.hex(), s.r, s.g, s.b, s.score))
                .collect();
            lines.push(format!("Sample color pixels: {}", samples.join(", ")));
        }
        lines.push(
            "Suggestion: keep RGB/color storage, or convert deliberately before saving as grayscale."
                .to_string(),
        );
    }
    lines.join("\n")
}

fn render_json(a: &Analysis) -> String {
    let samples: Vec<String> = a
        .samples
        .iter()
        .map(|s| {
            format!(
                "    {{ \"x\": {}, \"y\": {}, \"hex\": \"{}\", \"r\": {}, \"g\": {}, \"b\": {}, \"score\": {} }}",
                s.x,
                s.y,
                s.hex(),
                s.r,
                s.g,
                s.b,
                s.score
            )
        })
        .collect();
    let status = if a.effective_grayscale() {
        "effectively_grayscale"
    } else {
        "contains_color_pixels"
    };
    format!(
        "{{\n  \"status\": \"{}\",\n  \"effective_grayscale\": {},\n  \"width\": {},\n  \"height\": {},\n  \"pixels\": {},\n  \"metric\": \"{}\",\n  \"tolerance\": {},\n  \"ignore_alpha\": {},\n  \"scanned_pixels\": {},\n  \"transparent_skipped\": {},\n  \"gray_pixels\": {},\n  \"gray_percent\": {:.6},\n  \"color_pixels\": {},\n  \"color_percent\": {:.6},\n  \"max_score\": {},\n  \"mean_score\": {:.6},\n  \"samples\": {}\n}}",
        status,
        a.effective_grayscale(),
        a.width,
        a.height,
        a.pixels,
        a.metric.id(),
        a.tolerance,
        a.ignore_alpha,
        a.scanned,
        a.transparent_skipped,
        a.gray_pixels(),
        a.percent(a.gray_pixels()),
        a.colorish_pixels,
        a.percent(a.colorish_pixels),
        a.max_score,
        a.mean_score(),
        if samples.is_empty() {
            "[]".to_string()
        } else {
            format!("[\n{}\n  ]", samples.join(",\n"))
        }
    )
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    input: &str,
    input_format: &str,
    tolerance: u8,
    metric: &str,
    ignore_alpha: bool,
    max_samples: u32,
    output: &str,
) -> Result<String, String> {
    let format = InputFormat::parse(input_format)?;
    let metric = Metric::parse(metric)?;
    let output = OutputFormat::parse(output)?;
    if max_samples > MAX_SAMPLES_CAP {
        return Err(format!(
            "max_samples must be 0-{MAX_SAMPLES_CAP} (got {max_samples})"
        ));
    }
    let bytes = decode_input(input, format)?;
    let analysis = analyze(&bytes, metric, tolerance, ignore_alpha, max_samples)?;
    Ok(match output {
        OutputFormat::Report => render_report(&analysis),
        OutputFormat::Json => render_json(&analysis),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageFormat, Rgba, RgbaImage};

    fn png_base64(pixels: &[[u8; 4]], width: u32, height: u32) -> String {
        let mut img = RgbaImage::new(width, height);
        for (i, px) in pixels.iter().enumerate() {
            img.put_pixel((i as u32) % width, (i as u32) / width, Rgba(*px));
        }
        let mut buf = std::io::Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        general_purpose::STANDARD.encode(buf.into_inner())
    }

    fn report(input: &str, tolerance: u8) -> String {
        run(input, "base64", tolerance, "channel_delta", true, 20, "report").unwrap()
    }

    #[test]
    fn detects_true_grayscale_image() {
        let input = png_base64(
            &[
                [0, 0, 0, 255],
                [128, 128, 128, 255],
                [255, 255, 255, 255],
                [9, 9, 9, 0],
            ],
            2,
            2,
        );
        let out = report(&input, 0);
        assert!(out.contains("Status: effectively grayscale"), "{out}");
        assert!(out.contains("Gray pixels: 4 (100.0000%)"), "{out}");
        assert!(out.contains("Color pixels: 0 (0.0000%)"), "{out}");
    }

    #[test]
    fn detects_color_pixels_and_reports_samples() {
        let input = png_base64(
            &[
                [10, 10, 10, 255],
                [30, 40, 30, 255],
                [7, 8, 9, 255],
                [0, 0, 0, 255],
            ],
            2,
            2,
        );
        let out = report(&input, 2);
        assert!(out.contains("Status: contains color pixels"), "{out}");
        assert!(out.contains("Color pixels: 1 (25.0000%)"), "{out}");
        assert!(out.contains("Gray pixels: 3 (75.0000%)"), "{out}");
        assert!(
            out.contains("Sample color pixels: (1,0) #1e281e rgb(30,40,30) score 10"),
            "{out}"
        );
    }

    #[test]
    fn tolerance_can_classify_near_gray_as_ok() {
        let input = png_base64(&[[7, 8, 9, 255]], 1, 1);
        assert!(report(&input, 2).contains("Status: effectively grayscale"));
        assert!(report(&input, 1).contains("Status: contains color pixels"));
    }

    #[test]
    fn tolerance_cap_of_255_accepts_every_pixel() {
        let input = png_base64(&[[255, 0, 0, 255]], 1, 1);
        assert!(report(&input, 255).contains("Status: effectively grayscale"));
        assert!(report(&input, 254).contains("Status: contains color pixels"));
    }

    #[test]
    fn saturation_metric_flags_dark_tints_channel_delta_misses() {
        // A dark red pixel: channel delta is only 12, saturation is full 255.
        let input = png_base64(&[[12, 0, 0, 255]], 1, 1);
        let delta = run(&input, "base64", 20, "channel_delta", true, 20, "report").unwrap();
        assert!(delta.contains("Status: effectively grayscale"), "{delta}");
        let sat = run(&input, "base64", 20, "saturation", true, 20, "report").unwrap();
        assert!(sat.contains("Status: contains color pixels"), "{sat}");
        assert!(sat.contains("Max HSV saturation: 255"), "{sat}");
    }

    #[test]
    fn ignore_alpha_false_skips_fully_transparent_pixels() {
        let input = png_base64(&[[255, 0, 0, 0], [8, 8, 8, 255]], 2, 1);
        let counted = run(&input, "base64", 0, "channel_delta", true, 20, "report").unwrap();
        assert!(counted.contains("Status: contains color pixels"), "{counted}");
        let skipped = run(&input, "base64", 0, "channel_delta", false, 20, "report").unwrap();
        assert!(skipped.contains("Status: effectively grayscale"), "{skipped}");
        assert!(skipped.contains("Transparent pixels skipped: 1"), "{skipped}");
        assert!(skipped.contains("Scanned pixels: 1"), "{skipped}");
    }

    #[test]
    fn fully_transparent_image_reports_nothing_visible() {
        let input = png_base64(&[[255, 0, 0, 0]], 1, 1);
        let out = run(&input, "base64", 0, "channel_delta", false, 20, "report").unwrap();
        assert!(out.contains("Scanned pixels: 0"), "{out}");
        assert!(out.contains("every pixel is fully transparent"), "{out}");
    }

    #[test]
    fn max_samples_limits_the_listed_pixels() {
        let input = png_base64(&[[9, 0, 0, 255], [0, 9, 0, 255], [0, 0, 9, 255]], 3, 1);
        let one = run(&input, "base64", 0, "channel_delta", true, 1, "report").unwrap();
        assert_eq!(one.matches("rgb(").count(), 1, "{one}");
        let none = run(&input, "base64", 0, "channel_delta", true, 0, "report").unwrap();
        assert!(!none.contains("Sample color pixels"), "{none}");
        assert!(run(&input, "base64", 0, "channel_delta", true, 201, "report")
            .unwrap_err()
            .contains("max_samples must be 0-200"));
    }

    #[test]
    fn json_output_reports_counts() {
        let input = png_base64(&[[1, 2, 3, 255]], 1, 1);
        let out = run(&input, "base64", 0, "channel_delta", true, 20, "json").unwrap();
        assert!(out.contains("\"effective_grayscale\": false"), "{out}");
        assert!(out.contains("\"color_pixels\": 1"), "{out}");
        assert!(out.contains("\"gray_pixels\": 0"), "{out}");
        assert!(out.contains("\"max_score\": 2"), "{out}");
        assert!(out.contains("\"hex\": \"#010203\""), "{out}");
    }

    #[test]
    fn hex_input_is_supported() {
        let b64 = png_base64(&[[5, 5, 5, 255]], 1, 1);
        let bytes = general_purpose::STANDARD.decode(b64).unwrap();
        let hex = bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
        let out = run(&hex, "hex", 0, "channel_delta", true, 20, "report").unwrap();
        assert!(out.contains("effectively grayscale"), "{out}");
    }

    #[test]
    fn errors_are_actionable() {
        let ok = png_base64(&[[0, 0, 0, 255]], 1, 1);
        assert!(run("", "base64", 0, "channel_delta", true, 20, "report")
            .unwrap_err()
            .contains("input is empty"));
        assert!(
            run("not-base64", "base64", 0, "channel_delta", true, 20, "report")
                .unwrap_err()
                .contains("base64")
        );
        assert!(run("00f", "hex", 0, "channel_delta", true, 20, "report")
            .unwrap_err()
            .contains("odd number"));
        assert!(run("00", "hex", 0, "channel_delta", true, 20, "report")
            .unwrap_err()
            .contains("could not decode image"));
        assert!(run("00", "binary", 0, "channel_delta", true, 20, "report")
            .unwrap_err()
            .contains("unknown input_format"));
        assert!(run(&ok, "base64", 0, "hsv", true, 20, "report")
            .unwrap_err()
            .contains("unknown metric"));
        assert!(run(&ok, "base64", 0, "channel_delta", true, 20, "yaml")
            .unwrap_err()
            .contains("unknown output"));
    }
}
