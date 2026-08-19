//! gizza-ai/gif-extract-frames core — split an animated GIF into its individual
//! frame images. Pure Rust (`image`'s GIF decoder + PNG encoder, plus `zip`), so
//! it runs on every backend including the chat Service Worker — no ffmpeg, no
//! gifsicle.
//!
//! The `image` GIF frame iterator already COALESCES (unoptimizes) the animation:
//! every yielded frame is a full canvas-sized RGBA buffer with the GIF's
//! disposal method applied, so partial/optimized frames come out as complete
//! pictures rather than the sliver-on-transparent artifacts a raw block decode
//! would give.
//!
//! Output is a ZIP holding `frame-0001.png`, `frame-0002.png`, … plus a
//! `manifest.json` recording the canvas size, frame order and each frame's
//! delay in milliseconds.

use std::io::{Cursor, Write};

use image::codecs::gif::GifDecoder;
use image::codecs::png::PngEncoder;
use image::{AnimationDecoder, ExtendedColorType, ImageEncoder};
use serde::Serialize;
use zip::write::{SimpleFileOptions, ZipWriter};
use zip::CompressionMethod;

/// Hard ceiling on frames written, whatever `max_frames` asks for.
pub const FRAME_CAP: u32 = 500;
/// Canvas-pixel ceiling. Each coalesced frame is w*h*4 bytes of RGBA and the
/// wasm sandbox has 64 MiB total, so refuse absurd canvases up front with a
/// readable message instead of trapping mid-decode.
const MAX_PIXELS: u64 = 16_000_000;

/// One frame's entry in the ZIP's `manifest.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameInfo {
    /// 1-based playback position in the source GIF.
    pub index: u32,
    /// Name of this frame's PNG inside the ZIP.
    pub filename: String,
    /// How long this frame is shown, in milliseconds.
    pub delay_ms: u32,
}

/// The `manifest.json` written into the ZIP alongside the PNGs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Manifest {
    /// Filename of the GIF the frames came from.
    pub source: String,
    /// Canvas width in pixels (every extracted frame has this width).
    pub width: u32,
    /// Canvas height in pixels (every extracted frame has this height).
    pub height: u32,
    /// Number of frames actually written to the ZIP.
    pub frame_count: u32,
    /// Frames present in the source GIF (may exceed `frame_count` when capped).
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
    /// Filename base for each PNG (`frame` → `frame-0001.png`).
    pub prefix: String,
    /// Stop after this many frames; clamped to 1..=[`FRAME_CAP`].
    pub max_frames: u32,
    /// Recorded in the manifest as the source filename.
    pub source_name: String,
}

impl Default for ExtractParams {
    fn default() -> Self {
        Self {
            prefix: "frame".to_string(),
            max_frames: FRAME_CAP,
            source_name: "input.gif".to_string(),
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

/// Split an animated GIF into per-frame PNGs bundled in a ZIP.
///
/// Returns `(zip bytes, manifest)`. Frames are fully composited (coalesced), so
/// each PNG is a standalone picture of the canvas at that point in playback, not
/// the GIF's internal partial-update tile.
pub fn extract_frames(gif: &[u8], params: &ExtractParams) -> Result<(Vec<u8>, Manifest), String> {
    if gif.is_empty() {
        return Err("input is empty — provide an animated GIF".into());
    }

    let decoder =
        GifDecoder::new(Cursor::new(gif)).map_err(|e| format!("not a valid GIF: {e}"))?;
    let (width, height) = {
        use image::ImageDecoder;
        decoder.dimensions()
    };
    if width == 0 || height == 0 {
        return Err("GIF canvas has a zero dimension".into());
    }
    if u64::from(width) * u64::from(height) > MAX_PIXELS {
        return Err(format!(
            "GIF canvas is too large to extract ({width}x{height}); the limit is {MAX_PIXELS} pixels — resize the GIF first"
        ));
    }

    let limit = params.max_frames.clamp(1, FRAME_CAP);
    let prefix = clean_prefix(&params.prefix);

    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    let mut frames: Vec<FrameInfo> = Vec::new();
    let mut total_duration_ms: u64 = 0;
    let mut total_frames: u32 = 0;
    let mut truncated = false;

    // Stream: encode + write each coalesced frame, then drop its buffer. Holding
    // every frame at once would be frames * w * h * 4 bytes of RGBA.
    for frame in decoder.into_frames() {
        let frame = frame.map_err(|e| format!("could not decode GIF frames: {e}"))?;
        total_frames = total_frames.saturating_add(1);
        if frames.len() as u32 >= limit {
            truncated = true;
            continue; // keep counting the source frames for the manifest
        }

        let (num, denom) = frame.delay().numer_denom_ms();
        let delay_ms = if denom == 0 { 0 } else { num / denom };
        let buf = frame.into_buffer();

        let index = frames.len() as u32 + 1;
        let filename = format!("{prefix}-{index:04}.png");

        let mut png: Vec<u8> = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(buf.as_raw(), buf.width(), buf.height(), ExtendedColorType::Rgba8)
            .map_err(|e| format!("png encode failed on frame {index}: {e}"))?;

        zip.start_file(&filename, opts)
            .map_err(|e| format!("zip error: {e}"))?;
        zip.write_all(&png)
            .map_err(|e| format!("zip write error: {e}"))?;

        total_duration_ms += u64::from(delay_ms);
        frames.push(FrameInfo {
            index,
            filename,
            delay_ms,
        });
    }

    if frames.is_empty() {
        return Err("GIF has no frames to extract".into());
    }

    let manifest = Manifest {
        source: params.source_name.clone(),
        width,
        height,
        frame_count: frames.len() as u32,
        total_frames,
        truncated,
        total_duration_ms,
        frames,
    };

    let manifest_json = serde_json::to_vec_pretty(&manifest)
        .map_err(|e| format!("manifest serialize error: {e}"))?;
    zip.start_file("manifest.json", opts)
        .map_err(|e| format!("zip error: {e}"))?;
    zip.write_all(&manifest_json)
        .map_err(|e| format!("zip write error: {e}"))?;

    let zip_bytes = zip
        .finish()
        .map_err(|e| format!("zip finalize error: {e}"))?
        .into_inner();

    Ok((zip_bytes, manifest))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::gif::{GifEncoder, Repeat};
    use image::{Delay, Frame, Rgba, RgbaImage};

    /// An N-frame w x h GIF whose frame i is a solid colour derived from i, each
    /// shown for `delays_ms[i]` milliseconds.
    fn make_gif(w: u32, h: u32, delays_ms: &[u32]) -> Vec<u8> {
        let mut out = Cursor::new(Vec::new());
        {
            let mut enc = GifEncoder::new(&mut out);
            enc.set_repeat(Repeat::Infinite).unwrap();
            for (i, ms) in delays_ms.iter().enumerate() {
                let c = Rgba([(i as u8) * 60, 0, 255 - (i as u8) * 60, 255]);
                let img = RgbaImage::from_pixel(w, h, c);
                enc.encode_frame(Frame::from_parts(
                    img,
                    0,
                    0,
                    Delay::from_numer_denom_ms(*ms, 1),
                ))
                .unwrap();
            }
        }
        out.into_inner()
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

    fn zip_png(zip: &[u8], name: &str) -> RgbaImage {
        image::load_from_memory_with_format(&zip_entry(zip, name), image::ImageFormat::Png)
            .unwrap()
            .to_rgba8()
    }

    #[test]
    fn splits_an_animated_gif_into_png_frames_and_a_manifest() {
        let gif = make_gif(8, 6, &[100, 200, 50]);
        let (zip, m) = extract_frames(&gif, &ExtractParams::default()).unwrap();

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
        assert_eq!(m.frame_count, 3);
        assert_eq!(m.total_frames, 3);
        assert!(!m.truncated);
        assert_eq!(m.total_duration_ms, 350);
        assert_eq!(
            m.frames.iter().map(|f| f.delay_ms).collect::<Vec<_>>(),
            vec![100, 200, 50]
        );
        assert_eq!(
            m.frames.iter().map(|f| f.index).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );

        // Frames keep the canvas size, playback order and pixel content.
        for (i, f) in m.frames.iter().enumerate() {
            let img = zip_png(&zip, &f.filename);
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
        assert_eq!(parsed["frames"][1]["filename"], "frame-0002.png");
        assert_eq!(parsed["frames"][1]["delay_ms"], 200);
    }

    /// A GIF that only ever draws a small tile after the first frame must still
    /// come out as full-canvas composited pictures (the "unoptimize"/coalesce
    /// behaviour every competitor exposes as an option).
    #[test]
    fn partial_frames_are_coalesced_to_full_canvas_pictures() {
        // Frame 1: solid blue canvas. Frame 2: same canvas with a red 2x2 patch.
        let mut out = Cursor::new(Vec::new());
        {
            let mut enc = GifEncoder::new(&mut out);
            enc.set_repeat(Repeat::Infinite).unwrap();
            let base = RgbaImage::from_pixel(10, 10, Rgba([0, 0, 255, 255]));
            enc.encode_frame(Frame::from_parts(
                base.clone(),
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .unwrap();
            let mut second = base;
            for y in 4..6 {
                for x in 4..6 {
                    second.put_pixel(x, y, Rgba([255, 0, 0, 255]));
                }
            }
            enc.encode_frame(Frame::from_parts(
                second,
                0,
                0,
                Delay::from_numer_denom_ms(100, 1),
            ))
            .unwrap();
        }
        let gif = out.into_inner();

        let (zip, m) = extract_frames(&gif, &ExtractParams::default()).unwrap();
        assert_eq!(m.frame_count, 2);

        let f2 = zip_png(&zip, "frame-0002.png");
        assert_eq!(f2.dimensions(), (10, 10));
        // The patch is red AND the untouched background is still blue — a
        // non-coalesced decode would leave the background transparent.
        assert_eq!(f2.get_pixel(4, 4), &Rgba([255, 0, 0, 255]));
        assert_eq!(f2.get_pixel(0, 0), &Rgba([0, 0, 255, 255]));
        assert_eq!(f2.get_pixel(9, 9), &Rgba([0, 0, 255, 255]));
    }

    #[test]
    fn a_single_frame_gif_yields_one_png() {
        let gif = make_gif(4, 4, &[0]);
        let (zip, m) = extract_frames(&gif, &ExtractParams::default()).unwrap();
        assert_eq!(m.frame_count, 1);
        assert_eq!(m.total_frames, 1);
        assert_eq!(zip_names(&zip), vec!["frame-0001.png", "manifest.json"]);
    }

    #[test]
    fn prefix_names_the_frames() {
        let gif = make_gif(4, 4, &[100, 100]);
        let params = ExtractParams {
            prefix: "shot".into(),
            ..Default::default()
        };
        let (zip, _) = extract_frames(&gif, &params).unwrap();
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

        let gif = make_gif(4, 4, &[100]);
        let params = ExtractParams {
            prefix: "a/b".into(),
            ..Default::default()
        };
        let (zip, _) = extract_frames(&gif, &params).unwrap();
        assert_eq!(zip_names(&zip), vec!["a-b-0001.png", "manifest.json"]);
    }

    #[test]
    fn max_frames_truncates_and_records_the_source_total() {
        let gif = make_gif(4, 4, &[100, 100, 100, 100, 100]);
        let params = ExtractParams {
            max_frames: 2,
            ..Default::default()
        };
        let (zip, m) = extract_frames(&gif, &params).unwrap();
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
        let gif = make_gif(4, 4, &[100, 100, 100]);

        // Exactly the frame count → nothing truncated.
        let (_, at) = extract_frames(
            &gif,
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
            &gif,
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
            &gif,
            &ExtractParams {
                max_frames: 10_000,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(huge.frame_count, 3);

        let (_, zero) = extract_frames(
            &gif,
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
    fn rejects_a_non_gif() {
        // A valid PNG is not a GIF.
        let mut png = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 4, Rgba([1, 2, 3, 255])))
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let err = extract_frames(&png.into_inner(), &ExtractParams::default()).unwrap_err();
        assert!(err.contains("not a valid GIF"), "{err}");
    }

    /// A GIF magic followed by a truncated stream: the header parses, the frames
    /// don't — the error must name the decode step, not panic.
    #[test]
    fn rejects_a_truncated_gif_stream() {
        let mut bytes = make_gif(8, 8, &[100, 100]);
        bytes.truncate(bytes.len() / 2);
        let err = extract_frames(&bytes, &ExtractParams::default()).unwrap_err();
        assert!(
            err.contains("could not decode GIF frames") || err.contains("no frames"),
            "{err}"
        );
    }

    /// Garbage behind a GIF magic reads as an absurd canvas — reject it with the
    /// size guard's actionable message rather than OOM-trapping the sandbox.
    #[test]
    fn rejects_an_oversized_canvas() {
        let err = extract_frames(b"GIF89a not really", &ExtractParams::default()).unwrap_err();
        assert!(err.contains("too large to extract"), "{err}");
    }
}
