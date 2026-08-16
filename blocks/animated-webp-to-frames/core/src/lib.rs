//! gizza-ai/animated-webp-to-frames core — split an animated WebP into its
//! individual frame images. Pure Rust (`image`'s WebP decoder + the PNG/JPEG/WebP
//! encoders, plus `zip`), so it runs on every backend including the chat Service
//! Worker — no ffmpeg, no `webpmux`/`anim_dump`.
//!
//! The `image` WebP frame iterator already COALESCES the animation: an animated
//! WebP stores each frame as an ANMF sub-rectangle with its own blend and
//! disposal method, and the iterator applies both to a persistent canvas, so
//! every yielded frame is a full canvas-sized RGBA picture rather than the
//! sub-tile the file actually holds.
//!
//! Output is a ZIP holding `frame-0001.png`, `frame-0002.png`, … plus a
//! `manifest.json` recording the canvas size, frame order and each frame's
//! delay in milliseconds — the timing you need to rebuild or re-time the
//! animation after editing the stills.

use std::io::{Cursor, Write};

use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::{WebPDecoder, WebPEncoder};
use image::{AnimationDecoder, ExtendedColorType, ImageEncoder, RgbaImage};
use serde::Serialize;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Hard ceiling on frames written, whatever `max_frames` asks for.
pub const FRAME_CAP: u32 = 500;
/// Canvas-pixel ceiling. Each coalesced frame is w*h*4 bytes of RGBA and the
/// wasm sandbox has 64 MiB total, so refuse absurd canvases up front with a
/// readable message instead of trapping mid-decode.
const MAX_PIXELS: u64 = 16_000_000;
/// JPEG quality used for `format = jpg`. High enough that the frames stay
/// edit-grade; JPEG is offered for size, PNG stays the lossless default.
const JPEG_QUALITY: u8 = 92;

/// Image format each extracted frame is written in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FrameFormat {
    /// Lossless, keeps the alpha channel. The default.
    #[default]
    Png,
    /// Lossy and much smaller, but JPEG has no alpha — transparent pixels are
    /// flattened onto white.
    Jpg,
    /// Lossless WebP — keeps alpha and is smaller than PNG.
    Webp,
}

impl FrameFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "png" | "" => Ok(FrameFormat::Png),
            "jpg" | "jpeg" => Ok(FrameFormat::Jpg),
            "webp" => Ok(FrameFormat::Webp),
            other => Err(format!(
                "invalid format '{other}' (use 'png', 'jpg' or 'webp')"
            )),
        }
    }

    /// Filename extension for this format.
    pub fn ext(self) -> &'static str {
        match self {
            FrameFormat::Png => "png",
            FrameFormat::Jpg => "jpg",
            FrameFormat::Webp => "webp",
        }
    }

    /// True when the format cannot carry an alpha channel.
    fn flattens_alpha(self) -> bool {
        matches!(self, FrameFormat::Jpg)
    }
}

/// One frame's entry in the ZIP's `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameInfo {
    /// 1-based playback position in the source WebP.
    pub index: u32,
    /// Name of this frame's image inside the ZIP.
    pub filename: String,
    /// How long this frame is shown, in milliseconds.
    pub delay_ms: u32,
    /// Milliseconds from the start of playback to this frame's first showing.
    pub start_ms: u64,
}

/// The `manifest.json` written into the ZIP alongside the frame images.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Manifest {
    /// Filename of the WebP the frames came from.
    pub source: String,
    /// Canvas width in pixels (every extracted frame has this width).
    pub width: u32,
    /// Canvas height in pixels (every extracted frame has this height).
    pub height: u32,
    /// True when the source carried an animation chunk (vs a still WebP).
    pub animated: bool,
    /// Image format the frames were written in.
    pub format: FrameFormat,
    /// Number of frames actually written to the ZIP.
    pub frame_count: u32,
    /// Frames present in the source WebP (may exceed `frame_count` when capped).
    pub total_frames: u32,
    /// True when `max_frames`/the hard cap stopped extraction early.
    pub truncated: bool,
    /// Sum of the written frames' delays, in milliseconds.
    pub total_duration_ms: u64,
    /// Frames in playback order.
    pub frames: Vec<FrameInfo>,
}

/// Knobs for [`extract_frames`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractParams {
    /// Filename base for each frame image (`frame` → `frame-0001.png`).
    pub prefix: String,
    /// Stop after this many frames; clamped to 1..=[`FRAME_CAP`].
    pub max_frames: u32,
    /// Image format each frame is written in.
    pub format: FrameFormat,
    /// Recorded in the manifest as the source filename.
    pub source_name: String,
}

impl Default for ExtractParams {
    fn default() -> Self {
        Self {
            prefix: "frame".to_string(),
            max_frames: FRAME_CAP,
            format: FrameFormat::Png,
            source_name: "input.webp".to_string(),
        }
    }
}

/// Sanitize a user-supplied filename base: keep it a single path-free segment so
/// a prefix can never write outside the archive or produce an unopenable name.
fn clean_prefix(prefix: &str) -> String {
    let cleaned: String = prefix
        .trim()
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' => c,
            _ => '-',
        })
        .collect();
    let cleaned = cleaned.trim_matches(['-', '.']).to_string();
    if cleaned.is_empty() {
        "frame".to_string()
    } else {
        cleaned
    }
}

/// Composite RGBA onto an opaque white background — JPEG has no alpha channel,
/// and dropping it outright would turn transparent pixels into black.
fn flatten_onto_white(buf: &RgbaImage) -> Vec<u8> {
    let mut rgb = Vec::with_capacity((buf.width() as usize) * (buf.height() as usize) * 3);
    for px in buf.pixels() {
        let a = px[3] as u32;
        for c in 0..3 {
            let over = (px[c] as u32 * a + 255 * (255 - a) + 127) / 255;
            rgb.push(over.min(255) as u8);
        }
    }
    rgb
}

/// Encode one coalesced frame in `format`.
fn encode_frame(buf: &RgbaImage, format: FrameFormat, index: u32) -> Result<Vec<u8>, String> {
    let (w, h) = (buf.width(), buf.height());
    let mut out: Vec<u8> = Vec::new();
    let res = match format {
        FrameFormat::Png => PngEncoder::new(&mut out).write_image(
            buf.as_raw(),
            w,
            h,
            ExtendedColorType::Rgba8,
        ),
        FrameFormat::Jpg => {
            let rgb = flatten_onto_white(buf);
            JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY).write_image(
                &rgb,
                w,
                h,
                ExtendedColorType::Rgb8,
            )
        }
        FrameFormat::Webp => WebPEncoder::new_lossless(&mut out).encode(
            buf.as_raw(),
            w,
            h,
            ExtendedColorType::Rgba8,
        ),
    };
    res.map_err(|e| format!("{} encode failed on frame {index}: {e}", format.ext()))?;
    Ok(out)
}

/// Split an animated WebP into per-frame images bundled in a ZIP.
///
/// Returns `(zip bytes, manifest)`. Frames are fully composited (coalesced), so
/// each image is a standalone picture of the canvas at that point in playback,
/// not the WebP's internal ANMF sub-rectangle. A still (non-animated) WebP
/// yields exactly one frame.
pub fn extract_frames(webp: &[u8], params: &ExtractParams) -> Result<(Vec<u8>, Manifest), String> {
    if webp.is_empty() {
        return Err("input is empty — provide an animated WebP".into());
    }

    let decoder =
        WebPDecoder::new(Cursor::new(webp)).map_err(|e| format!("not a valid WebP: {e}"))?;
    let (width, height) = {
        use image::ImageDecoder;
        decoder.dimensions()
    };
    if width == 0 || height == 0 {
        return Err("WebP canvas has a zero dimension".into());
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!(
            "WebP canvas is too large to extract ({width}x{height}); the limit is {MAX_PIXELS} pixels — resize the WebP first"
        ));
    }
    let animated = decoder.has_animation();

    let limit = params.max_frames.clamp(1, FRAME_CAP);
    let prefix = clean_prefix(&params.prefix);
    let format = params.format;

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    // Frame images are already compressed (PNG/JPEG/WebP all deflate or better
    // internally), so store them and spend the deflate pass only on the text
    // manifest — recompressing them would cost CPU for ~0 bytes.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut frames: Vec<FrameInfo> = Vec::new();
    let mut total_duration_ms: u64 = 0;
    let mut total_frames: u32 = 0;
    let mut truncated = false;

    // Stream: encode + write each coalesced frame, then drop its buffer. Holding
    // every frame at once would be frames * w * h * 4 bytes of RGBA.
    let write_frame = |buf: RgbaImage,
                           delay_ms: u32,
                           frames: &mut Vec<FrameInfo>,
                           total_duration_ms: &mut u64,
                           zip: &mut ZipWriter<Cursor<Vec<u8>>>|
     -> Result<(), String> {
        let index = frames.len() as u32 + 1;
        let filename = format!("{prefix}-{index:04}.{}", format.ext());
        let bytes = encode_frame(&buf, format, index)?;

        zip.start_file(&filename, stored)
            .map_err(|e| format!("zip error: {e}"))?;
        zip.write_all(&bytes)
            .map_err(|e| format!("zip write error: {e}"))?;

        frames.push(FrameInfo {
            index,
            filename,
            delay_ms,
            start_ms: *total_duration_ms,
        });
        *total_duration_ms += u64::from(delay_ms);
        Ok(())
    };

    if animated {
        for frame in decoder.into_frames() {
            let frame = frame.map_err(|e| format!("could not decode WebP frames: {e}"))?;
            total_frames = total_frames.saturating_add(1);
            if frames.len() as u32 >= limit {
                truncated = true;
                continue; // keep counting the source frames for the manifest
            }
            let (num, denom) = frame.delay().numer_denom_ms();
            let delay_ms = if denom == 0 { 0 } else { num / denom };
            write_frame(
                frame.into_buffer(),
                delay_ms,
                &mut frames,
                &mut total_duration_ms,
                &mut zip,
            )?;
        }
    } else {
        // A still WebP has no animation chunk, so the frame iterator yields
        // nothing — decode the single image and emit it as frame 1.
        let img = image::DynamicImage::from_decoder(decoder)
            .map_err(|e| format!("could not decode WebP image: {e}"))?
            .to_rgba8();
        total_frames = 1;
        write_frame(img, 0, &mut frames, &mut total_duration_ms, &mut zip)?;
    }

    if frames.is_empty() {
        return Err("WebP has no frames to extract".into());
    }

    let manifest = Manifest {
        source: params.source_name.clone(),
        width,
        height,
        animated,
        format,
        frame_count: frames.len() as u32,
        total_frames,
        truncated,
        total_duration_ms,
        frames,
    };

    let manifest_json =
        serde_json::to_vec_pretty(&manifest).map_err(|e| format!("manifest serialize error: {e}"))?;
    zip.start_file("manifest.json", deflated)
        .map_err(|e| format!("zip error: {e}"))?;
    zip.write_all(&manifest_json)
        .map_err(|e| format!("zip write error: {e}"))?;

    let zip_bytes = zip
        .finish()
        .map_err(|e| format!("zip finalize error: {e}"))?
        .into_inner();

    Ok((zip_bytes, manifest))
}

/// True when `format` loses the source's alpha channel (used for the caller's
/// summary line so the lossy/flattening trade-off is stated, not hidden).
pub fn format_flattens_alpha(format: FrameFormat) -> bool {
    format.flattens_alpha()
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::Rgba;

    // ---- animated-WebP fixture builder -------------------------------------
    //
    // `image` can encode a STILL lossless WebP but not an ANIMATED one, so the
    // tests assemble the RIFF container by hand: VP8X (canvas + ANIMATION flag)
    // + ANIM (background/loop) + one ANMF per frame, each wrapping that frame's
    // VP8L chunk taken from a still encode. This is the container layout the
    // decoder under test has to walk, so building it explicitly is the point.

    fn u24le(v: u32) -> [u8; 3] {
        [(v & 0xff) as u8, ((v >> 8) & 0xff) as u8, ((v >> 16) & 0xff) as u8]
    }

    /// Wrap `payload` in a RIFF chunk (fourcc + LE size + payload + pad byte).
    fn chunk(fourcc: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 + payload.len() + 1);
        out.extend_from_slice(fourcc);
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        if payload.len() % 2 == 1 {
            out.push(0);
        }
        out
    }

    /// A solid-colour still lossless WebP.
    fn still_webp(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
        still_webp_img(&RgbaImage::from_pixel(w, h, Rgba(rgba)))
    }

    fn still_webp_img(img: &RgbaImage) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        WebPEncoder::new_lossless(&mut out)
            .encode(img.as_raw(), img.width(), img.height(), ExtendedColorType::Rgba8)
            .unwrap();
        out.into_inner()
    }

    /// Pull the whole `VP8L` (or `VP8 `) chunk — fourcc, size and data — out of a
    /// still WebP so it can be re-embedded verbatim inside an ANMF frame. The
    /// RIFF pad byte is kept: the decoder checks the enclosing ANMF payload
    /// against the sub-chunk's ROUNDED size, so an unpadded odd chunk is
    /// rejected as a malformed header.
    fn image_chunk(still: &[u8]) -> Vec<u8> {
        let mut i = 12; // skip "RIFF" + size + "WEBP"
        while i + 8 <= still.len() {
            let fourcc = &still[i..i + 4];
            let size = u32::from_le_bytes(still[i + 4..i + 8].try_into().unwrap()) as usize;
            let padded = size + (size % 2);
            if fourcc == b"VP8L" || fourcc == b"VP8 " {
                let mut out = still[i..i + 8 + size].to_vec();
                if size % 2 == 1 {
                    out.push(0);
                }
                return out;
            }
            i += 8 + padded;
        }
        panic!("no VP8L/VP8 chunk in the still WebP");
    }

    struct FrameSpec {
        img: RgbaImage,
        x: u32,
        y: u32,
        duration_ms: u32,
        /// True → dispose this frame to the background colour before the next.
        dispose_to_background: bool,
    }

    /// Assemble an animated WebP with canvas `w`x`h` from `specs`.
    fn animated_webp(w: u32, h: u32, specs: &[FrameSpec]) -> Vec<u8> {
        let mut body = Vec::new();

        // VP8X: flags bit1 = ANIMATION, bit4 = ALPHA; canvas dims are minus-one.
        let mut vp8x = Vec::new();
        vp8x.push(0b0001_0010);
        vp8x.extend_from_slice(&[0, 0, 0]);
        vp8x.extend_from_slice(&u24le(w - 1));
        vp8x.extend_from_slice(&u24le(h - 1));
        body.extend_from_slice(&chunk(b"VP8X", &vp8x));

        // ANIM: background colour (BGRA, transparent) + loop forever.
        let mut anim = Vec::new();
        anim.extend_from_slice(&[0, 0, 0, 0]);
        anim.extend_from_slice(&0u16.to_le_bytes());
        body.extend_from_slice(&chunk(b"ANIM", &anim));

        for spec in specs {
            let mut anmf = Vec::new();
            // Frame offsets are stored in units of 2 pixels.
            anmf.extend_from_slice(&u24le(spec.x / 2));
            anmf.extend_from_slice(&u24le(spec.y / 2));
            anmf.extend_from_slice(&u24le(spec.img.width() - 1));
            anmf.extend_from_slice(&u24le(spec.img.height() - 1));
            anmf.extend_from_slice(&u24le(spec.duration_ms));
            // bit0 = disposal (1 = to background), bit1 = blending (1 = none).
            anmf.push(if spec.dispose_to_background { 0b01 } else { 0b10 });
            anmf.extend_from_slice(&image_chunk(&still_webp_img(&spec.img)));
            body.extend_from_slice(&chunk(b"ANMF", &anmf));
        }

        let mut out = Vec::with_capacity(12 + body.len());
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
        out.extend_from_slice(b"WEBP");
        out.extend_from_slice(&body);
        out
    }

    /// N full-canvas frames, frame i a solid colour derived from i.
    fn simple_anim(w: u32, h: u32, delays_ms: &[u32]) -> Vec<u8> {
        let specs: Vec<FrameSpec> = delays_ms
            .iter()
            .enumerate()
            .map(|(i, ms)| FrameSpec {
                img: RgbaImage::from_pixel(
                    w,
                    h,
                    Rgba([(i as u8) * 60, 0, 255 - (i as u8) * 60, 255]),
                ),
                x: 0,
                y: 0,
                duration_ms: *ms,
                dispose_to_background: false,
            })
            .collect();
        animated_webp(w, h, &specs)
    }

    fn zip_names(zip: &[u8]) -> Vec<String> {
        let mut ar = zip::ZipArchive::new(Cursor::new(zip.to_vec())).unwrap();
        (0..ar.len())
            .map(|i| ar.by_index(i).unwrap().name().to_string())
            .collect()
    }

    fn zip_entry(zip: &[u8], name: &str) -> Vec<u8> {
        use std::io::Read;
        let mut ar = zip::ZipArchive::new(Cursor::new(zip.to_vec())).unwrap();
        let mut f = ar.by_name(name).unwrap();
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).unwrap();
        buf
    }

    fn zip_image(zip: &[u8], name: &str, format: image::ImageFormat) -> RgbaImage {
        image::load_from_memory_with_format(&zip_entry(zip, name), format)
            .unwrap()
            .to_rgba8()
    }

    // ---- tests --------------------------------------------------------------

    #[test]
    fn splits_an_animated_webp_into_png_frames_and_a_manifest() {
        let webp = simple_anim(8, 6, &[100, 200, 50]);
        let (zip, m) = extract_frames(&webp, &ExtractParams::default()).unwrap();

        assert_eq!(
            zip_names(&zip),
            vec![
                "frame-0001.png",
                "frame-0002.png",
                "frame-0003.png",
                "manifest.json"
            ]
        );

        assert_eq!((m.width, m.height), (8, 6));
        assert!(m.animated);
        assert_eq!(m.format, FrameFormat::Png);
        assert_eq!(m.frame_count, 3);
        assert_eq!(m.total_frames, 3);
        assert!(!m.truncated);
        assert_eq!(m.total_duration_ms, 350);
        assert_eq!(
            m.frames.iter().map(|f| f.delay_ms).collect::<Vec<_>>(),
            vec![100, 200, 50]
        );
        assert_eq!(
            m.frames.iter().map(|f| f.start_ms).collect::<Vec<_>>(),
            vec![0, 100, 300]
        );
        assert_eq!(
            m.frames.iter().map(|f| f.index).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );

        // Frames keep the canvas size, playback order and pixel content.
        for (i, f) in m.frames.iter().enumerate() {
            let img = zip_image(&zip, &f.filename, image::ImageFormat::Png);
            assert_eq!(img.dimensions(), (8, 6), "frame {} size", f.index);
            let px = img.get_pixel(3, 3);
            assert_eq!(px[0], (i as u8) * 60, "frame {} red", f.index);
            assert_eq!(px[2], 255 - (i as u8) * 60, "frame {} blue", f.index);
            assert_eq!(px[3], 255, "frame {} opaque", f.index);
        }

        // The manifest inside the ZIP matches the returned summary.
        let parsed: serde_json::Value =
            serde_json::from_slice(&zip_entry(&zip, "manifest.json")).unwrap();
        assert_eq!(parsed["frame_count"], 3);
        assert_eq!(parsed["format"], "png");
        assert_eq!(parsed["frames"][1]["filename"], "frame-0002.png");
        assert_eq!(parsed["frames"][1]["delay_ms"], 200);
        assert_eq!(parsed["frames"][1]["start_ms"], 100);
    }

    /// An animated WebP stores frame 2+ as a small ANMF sub-rectangle. Extraction
    /// must still yield full-canvas composited pictures — the "coalesce" every
    /// competitor either does silently or exposes as an option.
    #[test]
    fn partial_frames_are_coalesced_to_full_canvas_pictures() {
        let base = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 255, 255]));
        let patch = RgbaImage::from_pixel(2, 2, Rgba([255, 0, 0, 255]));
        let webp = animated_webp(
            10,
            10,
            &[
                FrameSpec {
                    img: base,
                    x: 0,
                    y: 0,
                    duration_ms: 100,
                    dispose_to_background: false,
                },
                FrameSpec {
                    img: patch,
                    x: 4,
                    y: 4,
                    duration_ms: 100,
                    dispose_to_background: false,
                },
            ],
        );

        let (zip, m) = extract_frames(&webp, &ExtractParams::default()).unwrap();
        assert_eq!(m.frame_count, 2);

        let f2 = zip_image(&zip, "frame-0002.png", image::ImageFormat::Png);
        assert_eq!(f2.dimensions(), (10, 10));
        // The 2x2 patch landed at its offset AND the untouched background is
        // still blue — a raw sub-rectangle decode would give a 2x2 image.
        assert_eq!(f2.get_pixel(4, 4), &Rgba([255, 0, 0, 255]));
        assert_eq!(f2.get_pixel(0, 0), &Rgba([0, 0, 255, 255]));
        assert_eq!(f2.get_pixel(9, 9), &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn a_still_webp_yields_one_frame() {
        let webp = still_webp(4, 4, [10, 20, 30, 255]);
        let (zip, m) = extract_frames(&webp, &ExtractParams::default()).unwrap();
        assert!(!m.animated);
        assert_eq!(m.frame_count, 1);
        assert_eq!(m.total_frames, 1);
        assert_eq!(m.total_duration_ms, 0);
        assert_eq!(zip_names(&zip), vec!["frame-0001.png", "manifest.json"]);
        let img = zip_image(&zip, "frame-0001.png", image::ImageFormat::Png);
        assert_eq!(img.dimensions(), (4, 4));
        assert_eq!(img.get_pixel(1, 1), &Rgba([10, 20, 30, 255]));
    }

    #[test]
    fn transparency_survives_in_the_png_alpha_channel() {
        let mut img = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 0]));
        img.put_pixel(1, 1, Rgba([255, 255, 255, 255]));
        let webp = animated_webp(
            4,
            4,
            &[FrameSpec {
                img,
                x: 0,
                y: 0,
                duration_ms: 100,
                dispose_to_background: false,
            }],
        );
        let (zip, _) = extract_frames(&webp, &ExtractParams::default()).unwrap();
        let f1 = zip_image(&zip, "frame-0001.png", image::ImageFormat::Png);
        assert_eq!(f1.get_pixel(0, 0)[3], 0, "background stays transparent");
        assert_eq!(f1.get_pixel(1, 1), &Rgba([255, 255, 255, 255]));
    }

    #[test]
    fn jpg_frames_flatten_transparency_onto_white() {
        let mut img = RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 0]));
        for y in 0..8 {
            for x in 0..4 {
                img.put_pixel(x, y, Rgba([255, 0, 0, 255]));
            }
        }
        let webp = animated_webp(
            8,
            8,
            &[FrameSpec {
                img,
                x: 0,
                y: 0,
                duration_ms: 100,
                dispose_to_background: false,
            }],
        );
        let params = ExtractParams {
            format: FrameFormat::Jpg,
            ..Default::default()
        };
        let (zip, m) = extract_frames(&webp, &params).unwrap();
        assert_eq!(zip_names(&zip), vec!["frame-0001.jpg", "manifest.json"]);
        assert_eq!(m.format, FrameFormat::Jpg);

        let f1 = zip_image(&zip, "frame-0001.jpg", image::ImageFormat::Jpeg);
        assert_eq!(f1.dimensions(), (8, 8));
        // Transparent half → white (not black), opaque half → still red. JPEG is
        // lossy, so allow a small tolerance.
        let clear = f1.get_pixel(6, 4);
        assert!(clear[0] > 240 && clear[1] > 240 && clear[2] > 240, "{clear:?}");
        let solid = f1.get_pixel(1, 4);
        assert!(solid[0] > 200 && solid[1] < 60 && solid[2] < 60, "{solid:?}");
        assert!(format_flattens_alpha(FrameFormat::Jpg));
        assert!(!format_flattens_alpha(FrameFormat::Png));
    }

    #[test]
    fn webp_frames_keep_alpha_and_decode_back() {
        let mut img = RgbaImage::from_pixel(6, 6, Rgba([0, 0, 0, 0]));
        img.put_pixel(2, 2, Rgba([0, 200, 40, 255]));
        let webp = animated_webp(
            6,
            6,
            &[FrameSpec {
                img,
                x: 0,
                y: 0,
                duration_ms: 40,
                dispose_to_background: false,
            }],
        );
        let params = ExtractParams {
            format: FrameFormat::Webp,
            ..Default::default()
        };
        let (zip, _) = extract_frames(&webp, &params).unwrap();
        assert_eq!(zip_names(&zip), vec!["frame-0001.webp", "manifest.json"]);

        let f1 = zip_image(&zip, "frame-0001.webp", image::ImageFormat::WebP);
        assert_eq!(f1.dimensions(), (6, 6));
        assert_eq!(f1.get_pixel(2, 2), &Rgba([0, 200, 40, 255]));
        assert_eq!(f1.get_pixel(0, 0)[3], 0);
    }

    #[test]
    fn format_parse_accepts_the_advertised_values() {
        assert_eq!(FrameFormat::parse("png").unwrap(), FrameFormat::Png);
        assert_eq!(FrameFormat::parse("").unwrap(), FrameFormat::Png);
        assert_eq!(FrameFormat::parse("JPG").unwrap(), FrameFormat::Jpg);
        assert_eq!(FrameFormat::parse("jpeg").unwrap(), FrameFormat::Jpg);
        assert_eq!(FrameFormat::parse(" webp ").unwrap(), FrameFormat::Webp);
        let err = FrameFormat::parse("tiff").unwrap_err();
        assert!(err.contains("invalid format 'tiff'"), "{err}");
    }

    #[test]
    fn prefix_names_the_frames() {
        let webp = simple_anim(4, 4, &[100, 100]);
        let params = ExtractParams {
            prefix: "shot".into(),
            ..Default::default()
        };
        let (zip, _) = extract_frames(&webp, &params).unwrap();
        assert_eq!(
            zip_names(&zip),
            vec!["shot-0001.png", "shot-0002.png", "manifest.json"]
        );
    }

    #[test]
    fn a_path_like_prefix_is_sanitized_to_one_segment() {
        assert_eq!(clean_prefix("../../etc/passwd"), "etc-passwd");
        assert_eq!(clean_prefix("  "), "frame");
        assert_eq!(clean_prefix("my frame"), "my-frame");

        let webp = simple_anim(4, 4, &[100]);
        let params = ExtractParams {
            prefix: "a/b".into(),
            ..Default::default()
        };
        let (zip, _) = extract_frames(&webp, &params).unwrap();
        assert_eq!(zip_names(&zip), vec!["a-b-0001.png", "manifest.json"]);
    }

    #[test]
    fn max_frames_truncates_and_records_the_source_total() {
        let webp = simple_anim(4, 4, &[100, 100, 100, 100, 100]);
        let params = ExtractParams {
            max_frames: 2,
            ..Default::default()
        };
        let (zip, m) = extract_frames(&webp, &params).unwrap();
        assert_eq!(m.frame_count, 2);
        assert_eq!(m.total_frames, 5);
        assert!(m.truncated);
        assert_eq!(m.total_duration_ms, 200);
        assert_eq!(
            zip_names(&zip),
            vec!["frame-0001.png", "frame-0002.png", "manifest.json"]
        );
    }

    #[test]
    fn max_frames_at_and_over_the_boundary() {
        let webp = simple_anim(4, 4, &[100, 100, 100]);

        // Exactly the frame count → nothing truncated.
        let (_, at) = extract_frames(
            &webp,
            &ExtractParams {
                max_frames: 3,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(at.frame_count, 3);
        assert!(!at.truncated);

        // One over → still all 3 frames, still not truncated.
        let (_, over) = extract_frames(
            &webp,
            &ExtractParams {
                max_frames: 4,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(over.frame_count, 3);
        assert!(!over.truncated);

        // Above the hard cap clamps to the cap (and 0 clamps up to 1).
        let (_, huge) = extract_frames(
            &webp,
            &ExtractParams {
                max_frames: 10_000,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(huge.frame_count, 3);

        let (_, zero) = extract_frames(
            &webp,
            &ExtractParams {
                max_frames: 0,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(zero.frame_count, 1);
        assert!(zero.truncated);
    }

    #[test]
    fn rejects_empty_input() {
        let err = extract_frames(&[], &ExtractParams::default()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn rejects_a_non_webp() {
        // A valid PNG is not a WebP.
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([1, 2, 3, 255])))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let err = extract_frames(&png.into_inner(), &ExtractParams::default()).unwrap_err();
        assert!(err.contains("not a valid WebP"), "{err}");
    }

    /// A WebP header followed by a truncated stream: the container parses far
    /// enough to start, the frames don't — the error must name a step, not panic.
    #[test]
    fn rejects_a_truncated_webp_stream() {
        let mut bytes = simple_anim(8, 8, &[100, 100]);
        bytes.truncate(bytes.len() / 2);
        let err = extract_frames(&bytes, &ExtractParams::default()).unwrap_err();
        assert!(
            err.contains("not a valid WebP")
                || err.contains("could not decode WebP frames")
                || err.contains("no frames"),
            "{err}"
        );
    }
}
