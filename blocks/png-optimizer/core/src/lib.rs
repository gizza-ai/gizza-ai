//! gizza-ai/png-optimizer core — losslessly shrink a PNG by re-encoding it with
//! better filters and a more compact color type, WITHOUT changing a single pixel.
//!
//! Pure Rust (`png` for encode, `image` for decode/analysis — no ffmpeg, no C
//! deps) so it runs on every backend including the chat Service Worker.
//!
//! What "lossless" means here: we decode the PNG to exact pixels and re-encode
//! them. The displayed pixels are bit-for-bit identical (verified in tests).
//! Optimizations applied:
//!   * **Filter search** — PNG scanline filters are re-chosen (adaptive per-row,
//!     or, at `max` effort, a brute-force sweep of every filter, keeping the
//!     smallest result).
//!   * **Compression** — re-deflated at a higher zlib effort.
//!   * **Color-type reduction** (when `reduce`, lossless only): RGB→indexed
//!     palette when the image has ≤256 distinct colors; drop a fully-opaque
//!     alpha channel; RGB→grayscale when every pixel has R==G==B.
//!   * **Metadata stripped** — all ancillary chunks (EXIF/text/gamma/etc.) are
//!     dropped; the output is never interlaced.
//! The result is never larger than the input: if we can't beat it, the original
//! bytes are returned unchanged.

use std::collections::HashMap;
use std::io::Cursor;

use serde::Serialize;

/// The 8-byte PNG file signature.
const PNG_MAGIC: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/// Reject absurd rasters before decoding (guards memory).
const MAX_PIXELS: u64 = 40_000_000;

/// How hard to work for a smaller file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Effort {
    /// Fast zlib, non-adaptive filter, single pass.
    Fast,
    /// Best zlib, adaptive per-row filter, single pass.
    Default,
    /// Best zlib + brute-force filter sweep (adaptive and each fixed filter),
    /// keeping the smallest output.
    Max,
}

impl Effort {
    pub fn parse(s: &str) -> Result<Effort, String> {
        match s {
            "fast" => Ok(Effort::Fast),
            "default" => Ok(Effort::Default),
            "max" => Ok(Effort::Max),
            other => Err(format!("unknown effort {other:?}; use fast, default, or max")),
        }
    }
}

/// Result of an optimization pass.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Optimized {
    /// The output PNG bytes (the original bytes if we couldn't shrink them).
    pub bytes: Vec<u8>,
    pub input_len: usize,
    pub output_len: usize,
    pub width: u32,
    pub height: u32,
    /// Human label of the chosen output color type (or "unchanged").
    pub color_type: String,
    /// True when the optimizer could not beat the input and returned it as-is.
    pub reused_original: bool,
}

impl Optimized {
    /// Percent of bytes saved, 0..=100 (0 when the original was returned).
    pub fn percent_saved(&self) -> f64 {
        if self.input_len == 0 || self.output_len >= self.input_len {
            return 0.0;
        }
        (self.input_len - self.output_len) as f64 / self.input_len as f64 * 100.0
    }
}

/// Losslessly optimize a PNG. Returns an error if `bytes` is not a PNG.
pub fn optimize(bytes: &[u8], effort: Effort, reduce: bool) -> Result<Optimized, String> {
    if bytes.len() < 8 || bytes[..8] != PNG_MAGIC {
        return Err("input is not a PNG file (missing the PNG signature)".into());
    }
    let (w0, h0) = png_dimensions(bytes)?;
    if (w0 as u64) * (h0 as u64) > MAX_PIXELS {
        return Err(format!(
            "image too large: {w0}×{h0} exceeds the {MAX_PIXELS}-pixel limit"
        ));
    }

    let img = image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
        .map_err(|e| format!("could not decode PNG: {e}"))?;
    let (w, h) = (img.width(), img.height());

    let (candidate, label) = encode_optimized(&img, effort, reduce)?;

    let input_len = bytes.len();
    if candidate.len() >= input_len {
        // Couldn't beat the original — return it untouched (never enlarge).
        return Ok(Optimized {
            bytes: bytes.to_vec(),
            input_len,
            output_len: input_len,
            width: w,
            height: h,
            color_type: "unchanged".into(),
            reused_original: true,
        });
    }
    let output_len = candidate.len();
    Ok(Optimized {
        bytes: candidate,
        input_len,
        output_len,
        width: w,
        height: h,
        color_type: label,
        reused_original: false,
    })
}

/// Cheap IHDR-only dimension read.
fn png_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    let reader = png::Decoder::new(Cursor::new(bytes))
        .read_info()
        .map_err(|e| format!("invalid PNG header: {e}"))?;
    let info = reader.info();
    Ok((info.width, info.height))
}

/// Encode `img` to the smallest lossless PNG we can, returning (bytes, label).
fn encode_optimized(
    img: &image::DynamicImage,
    effort: Effort,
    reduce: bool,
) -> Result<(Vec<u8>, String), String> {
    let (w, h) = (img.width(), img.height());
    match img.color() {
        // 16-bit sources stay 16-bit (down-sampling would be lossy).
        image::ColorType::Rgba16 => {
            enc16(w, h, png::ColorType::Rgba, &img.to_rgba16().into_raw(), effort)
                .map(|b| (b, "16-bit RGBA".into()))
        }
        image::ColorType::Rgb16 => enc16(w, h, png::ColorType::Rgb, &img.to_rgb16().into_raw(), effort)
            .map(|b| (b, "16-bit RGB".into())),
        image::ColorType::La16 => enc16(
            w,
            h,
            png::ColorType::GrayscaleAlpha,
            &img.to_luma_alpha16().into_raw(),
            effort,
        )
        .map(|b| (b, "16-bit gray+alpha".into())),
        image::ColorType::L16 => {
            enc16(w, h, png::ColorType::Grayscale, &img.to_luma16().into_raw(), effort)
                .map(|b| (b, "16-bit grayscale".into()))
        }
        // Everything else is treated as 8-bit.
        _ => encode_8bit(img, effort, reduce),
    }
}

/// Encode a 16-bit buffer (converting native u16 samples to big-endian bytes).
fn enc16(
    w: u32,
    h: u32,
    color: png::ColorType,
    samples: &[u16],
    effort: Effort,
) -> Result<Vec<u8>, String> {
    let mut be = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        be.extend_from_slice(&s.to_be_bytes());
    }
    encode_direct(w, h, color, png::BitDepth::Sixteen, &be, effort)
}

/// Choose the most compact lossless 8-bit representation and encode it.
fn encode_8bit(
    img: &image::DynamicImage,
    effort: Effort,
    reduce: bool,
) -> Result<(Vec<u8>, String), String> {
    let (w, h) = (img.width(), img.height());
    let rgba = img.to_rgba8();

    // Single pass over the pixels for the reduction decisions.
    let mut has_alpha = false;
    let mut is_gray = true;
    for p in rgba.pixels() {
        let [r, g, b, a] = p.0;
        if a != 255 {
            has_alpha = true;
        }
        if r != g || g != b {
            is_gray = false;
        }
    }

    if reduce {
        // 1. Indexed palette when the image has ≤256 distinct colors.
        if let Some((palette, trns, indices)) = build_palette(&rgba) {
            let colors = palette.len() / 3;
            let bytes = encode_indexed(w, h, &indices, &palette, trns.as_deref(), effort)?;
            return Ok((bytes, format!("indexed ({colors} colors)")));
        }
        // 2. Grayscale when every pixel has R==G==B.
        if is_gray && !has_alpha {
            return encode_direct(
                w,
                h,
                png::ColorType::Grayscale,
                png::BitDepth::Eight,
                img.to_luma8().as_raw(),
                effort,
            )
            .map(|b| (b, "grayscale".into()));
        }
        if is_gray && has_alpha {
            return encode_direct(
                w,
                h,
                png::ColorType::GrayscaleAlpha,
                png::BitDepth::Eight,
                img.to_luma_alpha8().as_raw(),
                effort,
            )
            .map(|b| (b, "gray+alpha".into()));
        }
        // 3. Drop a fully-opaque alpha channel.
        if !has_alpha {
            return encode_direct(
                w,
                h,
                png::ColorType::Rgb,
                png::BitDepth::Eight,
                img.to_rgb8().as_raw(),
                effort,
            )
            .map(|b| (b, "RGB".into()));
        }
        // 4. True-color with transparency.
        return encode_direct(w, h, png::ColorType::Rgba, png::BitDepth::Eight, rgba.as_raw(), effort)
            .map(|b| (b, "RGBA".into()));
    }

    // reduce disabled: preserve the source color type (still re-filter/re-deflate).
    match img.color() {
        image::ColorType::L8 => encode_direct(
            w,
            h,
            png::ColorType::Grayscale,
            png::BitDepth::Eight,
            img.to_luma8().as_raw(),
            effort,
        )
        .map(|b| (b, "grayscale".into())),
        image::ColorType::La8 => encode_direct(
            w,
            h,
            png::ColorType::GrayscaleAlpha,
            png::BitDepth::Eight,
            img.to_luma_alpha8().as_raw(),
            effort,
        )
        .map(|b| (b, "gray+alpha".into())),
        image::ColorType::Rgb8 => encode_direct(
            w,
            h,
            png::ColorType::Rgb,
            png::BitDepth::Eight,
            img.to_rgb8().as_raw(),
            effort,
        )
        .map(|b| (b, "RGB".into())),
        _ => encode_direct(w, h, png::ColorType::Rgba, png::BitDepth::Eight, rgba.as_raw(), effort)
            .map(|b| (b, "RGBA".into())),
    }
}

/// Build an 8-bit indexed palette + tRNS + index buffer, or `None` if the image
/// has more than 256 distinct colors. Transparent entries are sorted first so
/// the tRNS chunk can be truncated to the shortest run covering them.
fn build_palette(rgba: &image::RgbaImage) -> Option<(Vec<u8>, Option<Vec<u8>>, Vec<u8>)> {
    let mut lookup: HashMap<[u8; 4], u8> = HashMap::new();
    let mut colors: Vec<[u8; 4]> = Vec::new();
    let mut indices: Vec<u8> = Vec::with_capacity((rgba.width() * rgba.height()) as usize);
    for p in rgba.pixels() {
        let key = p.0;
        let idx = if let Some(&i) = lookup.get(&key) {
            i
        } else {
            if colors.len() >= 256 {
                return None; // too many colors to index losslessly
            }
            let i = colors.len() as u8;
            colors.push(key);
            lookup.insert(key, i);
            i
        };
        indices.push(idx);
    }

    // Sort palette by alpha ascending (transparent first) and remap indices.
    let mut order: Vec<usize> = (0..colors.len()).collect();
    order.sort_by_key(|&i| colors[i][3]);
    let mut remap = vec![0u8; colors.len()];
    for (new_i, &old_i) in order.iter().enumerate() {
        remap[old_i] = new_i as u8;
    }
    for idx in indices.iter_mut() {
        *idx = remap[*idx as usize];
    }
    let sorted: Vec<[u8; 4]> = order.iter().map(|&i| colors[i]).collect();

    let mut palette = Vec::with_capacity(sorted.len() * 3);
    for c in &sorted {
        palette.extend_from_slice(&[c[0], c[1], c[2]]);
    }
    let trns = sorted
        .iter()
        .rposition(|c| c[3] != 255)
        .map(|last| sorted[..=last].iter().map(|c| c[3]).collect::<Vec<u8>>());

    Some((palette, trns, indices))
}

/// Filter/compression strategies to try for a given effort.
fn strategies(effort: Effort) -> Vec<(png::Compression, png::AdaptiveFilterType, png::FilterType)> {
    use png::{AdaptiveFilterType as A, Compression as C, FilterType as F};
    match effort {
        Effort::Fast => vec![(C::Fast, A::NonAdaptive, F::Sub)],
        Effort::Default => vec![(C::Best, A::Adaptive, F::Sub)],
        Effort::Max => {
            let mut v = vec![(C::Best, A::Adaptive, F::Sub)];
            for f in [F::NoFilter, F::Sub, F::Up, F::Avg, F::Paeth] {
                v.push((C::Best, A::NonAdaptive, f));
            }
            v
        }
    }
}

/// Encode a non-indexed buffer, trying each strategy and keeping the smallest.
fn encode_direct(
    w: u32,
    h: u32,
    color: png::ColorType,
    depth: png::BitDepth,
    data: &[u8],
    effort: Effort,
) -> Result<Vec<u8>, String> {
    let mut best: Option<Vec<u8>> = None;
    for (comp, adaptive, filter) in strategies(effort) {
        let mut out: Vec<u8> = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(color);
            enc.set_depth(depth);
            enc.set_compression(comp);
            enc.set_adaptive_filter(adaptive);
            enc.set_filter(filter);
            let mut writer = enc.write_header().map_err(|e| format!("png header: {e}"))?;
            writer.write_image_data(data).map_err(|e| format!("png write: {e}"))?;
            writer.finish().map_err(|e| format!("png finish: {e}"))?;
        }
        best = Some(pick_smaller(best, out));
    }
    best.ok_or_else(|| "no encoder strategy produced output".into())
}

/// Encode an indexed buffer with palette + optional tRNS.
fn encode_indexed(
    w: u32,
    h: u32,
    indices: &[u8],
    palette: &[u8],
    trns: Option<&[u8]>,
    effort: Effort,
) -> Result<Vec<u8>, String> {
    let mut best: Option<Vec<u8>> = None;
    for (comp, adaptive, filter) in strategies(effort) {
        let mut out: Vec<u8> = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(png::ColorType::Indexed);
            enc.set_depth(png::BitDepth::Eight);
            enc.set_palette(palette);
            if let Some(a) = trns {
                enc.set_trns(a);
            }
            enc.set_compression(comp);
            enc.set_adaptive_filter(adaptive);
            enc.set_filter(filter);
            let mut writer = enc.write_header().map_err(|e| format!("png header: {e}"))?;
            writer.write_image_data(indices).map_err(|e| format!("png write: {e}"))?;
            writer.finish().map_err(|e| format!("png finish: {e}"))?;
        }
        best = Some(pick_smaller(best, out));
    }
    best.ok_or_else(|| "no encoder strategy produced output".into())
}

fn pick_smaller(best: Option<Vec<u8>>, candidate: Vec<u8>) -> Vec<u8> {
    match best {
        Some(b) if b.len() <= candidate.len() => b,
        _ => candidate,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode a deliberately un-optimized PNG (fast zlib, no filter) so our
    /// optimizer has room to beat it.
    fn poor_png(color: png::ColorType, depth: png::BitDepth, w: u32, h: u32, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut out, w, h);
            enc.set_color(color);
            enc.set_depth(depth);
            enc.set_compression(png::Compression::Fast);
            enc.set_filter(png::FilterType::NoFilter);
            let mut wr = enc.write_header().unwrap();
            wr.write_image_data(data).unwrap();
            wr.finish().unwrap();
        }
        out
    }

    fn decode_rgba(bytes: &[u8]) -> image::RgbaImage {
        image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
            .unwrap()
            .to_rgba8()
    }

    fn output_color_type(bytes: &[u8]) -> png::ColorType {
        png::Decoder::new(Cursor::new(bytes))
            .read_info()
            .unwrap()
            .info()
            .color_type
    }

    /// A 64×64 opaque RGBA image whose pixels are all gray (R==G==B).
    fn gray_rgba() -> (u32, u32, Vec<u8>) {
        let (w, h) = (64u32, 64u32);
        let mut data = Vec::with_capacity((w * h * 4) as usize);
        for y in 0..h {
            for x in 0..w {
                let v = ((x + y) % 256) as u8;
                data.extend_from_slice(&[v, v, v, 255]);
            }
        }
        (w, h, data)
    }

    #[test]
    fn rejects_non_png() {
        assert!(optimize(b"not a png at all", Effort::Default, true).is_err());
        assert!(optimize(&[], Effort::Default, true).is_err());
    }

    #[test]
    fn lossless_roundtrip_true_color() {
        // A gradient with varying alpha and color → stays RGBA, must be bit-exact.
        let (w, h) = (48u32, 48u32);
        let mut data = Vec::new();
        for y in 0..h {
            for x in 0..w {
                data.extend_from_slice(&[x as u8, y as u8, (x ^ y) as u8, (x.wrapping_add(1)) as u8]);
            }
        }
        let input = poor_png(png::ColorType::Rgba, png::BitDepth::Eight, w, h, &data);
        let before = decode_rgba(&input);
        let opt = optimize(&input, Effort::Max, true).unwrap();
        let after = decode_rgba(&opt.bytes);
        assert_eq!(before, after, "pixels must be unchanged");
    }

    #[test]
    fn reduces_gray_rgba_and_shrinks() {
        let (w, h, data) = gray_rgba();
        let input = poor_png(png::ColorType::Rgba, png::BitDepth::Eight, w, h, &data);
        let opt = optimize(&input, Effort::Max, true).unwrap();
        assert!(!opt.reused_original, "should have found a smaller encoding");
        assert!(opt.output_len < opt.input_len, "output must be smaller");
        assert!(opt.percent_saved() > 0.0);
        // Lossless: decoded pixels identical.
        assert_eq!(decode_rgba(&input), decode_rgba(&opt.bytes));
        // The opaque grayscale RGBA collapsed to a compact type (indexed here,
        // since only ~128 distinct gray values appear).
        let ct = output_color_type(&opt.bytes);
        assert!(
            matches!(ct, png::ColorType::Indexed | png::ColorType::Grayscale),
            "expected indexed or grayscale, got {ct:?}"
        );
    }

    #[test]
    fn reduce_false_preserves_color_type() {
        let (w, h, data) = gray_rgba();
        let input = poor_png(png::ColorType::Rgba, png::BitDepth::Eight, w, h, &data);
        let opt = optimize(&input, Effort::Default, false).unwrap();
        assert_eq!(decode_rgba(&input), decode_rgba(&opt.bytes), "still lossless");
        assert_eq!(
            output_color_type(&opt.bytes),
            png::ColorType::Rgba,
            "reduce=false must keep the RGBA color type"
        );
    }

    #[test]
    fn effort_parse() {
        assert_eq!(Effort::parse("fast").unwrap(), Effort::Fast);
        assert_eq!(Effort::parse("default").unwrap(), Effort::Default);
        assert_eq!(Effort::parse("max").unwrap(), Effort::Max);
        assert!(Effort::parse("turbo").is_err());
    }
}
