//! gizza-ai/images-to-animated-webp core — combine a set of images into a single
//! animated WebP.
//!
//! Pure Rust: `image` decodes each source frame, `image-webp` encodes each frame
//! as a lossless VP8L still, and this module assembles the extended WebP
//! container (`VP8X` + `ANIM` + one `ANMF` per frame) by hand. No ffmpeg and no
//! libwebp C bindings, so it runs on every backend (incl. the chat Service
//! Worker).
//!
//! Every frame is written as a full-canvas independent keyframe with blending
//! disabled, so there is no frame-stacking/ghosting to configure.

use std::collections::BTreeMap;
use std::io::Cursor;

use color_quant::NeuQuant;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use image_webp::{ColorType, WebPEncoder};

/// Hard ceiling on frames — keeps memory + encode time bounded.
pub const MAX_FRAMES: usize = 300;
/// Hard ceiling on canvas pixels (w × h). 4 MP ≈ 16 MB of RGBA per frame, which
/// fits the 64 MiB wasm sandbox alongside the encoder's own buffers.
pub const MAX_CANVAS_PIXELS: u64 = 4_000_000;
/// VP8L bitstream limit (14-bit dimensions).
pub const MAX_DIMENSION: u32 = 16_383;
/// Ceiling on the total encoded frame data, checked while encoding.
pub const MAX_ENCODED_BYTES: usize = 48_000_000;

/// How a frame that does not match the canvas aspect ratio is placed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fit {
    /// Scale to fit inside the canvas, pad with the background color.
    Contain,
    /// Scale to fill the canvas, center-crop the overflow.
    Cover,
    /// Stretch to the exact canvas size, ignoring aspect ratio.
    Stretch,
}

impl Fit {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "contain" | "" => Ok(Fit::Contain),
            "cover" => Ok(Fit::Cover),
            "stretch" => Ok(Fit::Stretch),
            other => Err(format!(
                "invalid fit '{other}' (use 'contain', 'cover', or 'stretch')"
            )),
        }
    }
}

/// Playback order of the supplied images.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order {
    /// First image → last image.
    Forward,
    /// Last image → first image.
    Reverse,
    /// Forward, then back again without repeating the end frames (ping-pong).
    Boomerang,
}

impl Order {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "forward" | "" => Ok(Order::Forward),
            "reverse" => Ok(Order::Reverse),
            "boomerang" | "pingpong" | "ping-pong" => Ok(Order::Boomerang),
            other => Err(format!(
                "invalid order '{other}' (use 'forward', 'reverse', or 'boomerang')"
            )),
        }
    }

    /// Source-image indices, in playback order, for `n` images.
    fn sequence(self, n: usize) -> Vec<usize> {
        match self {
            Order::Forward => (0..n).collect(),
            Order::Reverse => (0..n).rev().collect(),
            Order::Boomerang => {
                let mut v: Vec<usize> = (0..n).collect();
                if n > 2 {
                    v.extend((1..n - 1).rev());
                }
                v
            }
        }
    }
}

/// Encoding options. See the block descriptor for user-facing units/defaults.
#[derive(Clone, Debug)]
pub struct Options {
    /// Delay applied to every frame, in milliseconds (10–60000).
    pub delay_ms: u32,
    /// Optional per-image delays (one per SOURCE image, before ordering).
    /// Empty = use `delay_ms` for all frames.
    pub frame_delays_ms: Vec<u32>,
    /// 0 = loop forever, otherwise the number of plays (1–65535).
    pub loop_count: u16,
    pub order: Order,
    /// 0 = keep the natural canvas size, otherwise scale the canvas down to
    /// this width (never up).
    pub max_width: u32,
    pub fit: Fit,
    /// Fills padding (`contain`) and any transparent area of the canvas.
    pub background: Rgba<u8>,
    /// 0 = full color (no quantization); 2–256 = quantize each frame to that
    /// many colors before the lossless encode, which shrinks the file a lot.
    pub colors: u16,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            delay_ms: 200,
            frame_delays_ms: Vec::new(),
            loop_count: 0,
            order: Order::Forward,
            max_width: 0,
            fit: Fit::Contain,
            background: Rgba([255, 255, 255, 255]),
            colors: 0,
        }
    }
}

/// The encoded animation plus the facts a caller wants to report back.
#[derive(Clone, Debug)]
pub struct Animation {
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    /// Total duration of ONE loop, in milliseconds.
    pub duration_ms: u64,
}

/// Parse `#rgb`, `#rrggbb`, `#rrggbbaa`, or `transparent` into RGBA.
pub fn parse_color(s: &str) -> Result<Rgba<u8>, String> {
    let t = s.trim();
    if t.eq_ignore_ascii_case("transparent") || t.eq_ignore_ascii_case("none") {
        return Ok(Rgba([0, 0, 0, 0]));
    }
    let h = t.trim_start_matches('#');
    let v = |a: &str| u8::from_str_radix(a, 16).map_err(|_| format!("invalid color '{s}'"));
    let (r, g, b, a) = match h.len() {
        3 => {
            let cs: Vec<char> = h.chars().collect();
            let d = |c: char| -> Result<u8, String> {
                u8::from_str_radix(&c.to_string().repeat(2), 16)
                    .map_err(|_| format!("invalid color '{s}'"))
            };
            (d(cs[0])?, d(cs[1])?, d(cs[2])?, 255)
        }
        6 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, 255),
        8 => (v(&h[0..2])?, v(&h[2..4])?, v(&h[4..6])?, v(&h[6..8])?),
        _ => {
            return Err(format!(
                "invalid color '{s}' (use #rgb, #rrggbb, #rrggbbaa, or 'transparent')"
            ))
        }
    };
    Ok(Rgba([r, g, b, a]))
}

/// Parse a comma/space separated list of per-frame delays.
pub fn parse_delays(s: &str) -> Result<Vec<u32>, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(Vec::new());
    }
    t.split(|c| c == ',' || c == ' ' || c == ';')
        .filter(|p| !p.trim().is_empty())
        .map(|p| {
            p.trim()
                .parse::<u32>()
                .map_err(|_| format!("invalid frame delay '{}' (whole milliseconds)", p.trim()))
                .map(|ms| ms.clamp(10, 60_000))
        })
        .collect()
}

/// Build an animated WebP from encoded `images` (one frame per image).
pub fn animated_webp_from_images(images: &[Vec<u8>], opts: &Options) -> Result<Animation, String> {
    if images.is_empty() {
        return Err("provide at least one image".into());
    }
    if images.len() > MAX_FRAMES {
        return Err(format!(
            "too many images: {} (max {MAX_FRAMES} frames)",
            images.len()
        ));
    }
    if !opts.frame_delays_ms.is_empty() && opts.frame_delays_ms.len() != images.len() {
        return Err(format!(
            "frame_delays_ms has {} value(s) but {} image(s) were supplied — give one delay per image, or leave it empty to use delay_ms",
            opts.frame_delays_ms.len(),
            images.len()
        ));
    }
    if opts.colors != 0 && !(2..=256).contains(&opts.colors) {
        return Err(format!(
            "invalid colors '{}' (use 0 for full color, or 2-256)",
            opts.colors
        ));
    }

    // Read dimensions from the headers first (no raster allocated yet) so the
    // canvas can be sized without holding every decoded source in memory.
    let dims: Vec<(u32, u32)> = images
        .iter()
        .enumerate()
        .map(|(i, b)| {
            image::ImageReader::new(Cursor::new(b.as_slice()))
                .with_guessed_format()
                .map_err(|e| format!("image #{}: could not read: {e}", i + 1))?
                .into_dimensions()
                .map_err(|e| format!("image #{}: could not decode: {e}", i + 1))
        })
        .collect::<Result<_, _>>()?;

    // Canvas = the largest source dimensions, optionally scaled down to max_width.
    let mut canvas_w = dims.iter().map(|d| d.0).max().unwrap_or(1).max(1);
    let mut canvas_h = dims.iter().map(|d| d.1).max().unwrap_or(1).max(1);
    if opts.max_width > 0 && canvas_w > opts.max_width {
        let scale = opts.max_width as f64 / canvas_w as f64;
        canvas_h = ((canvas_h as f64 * scale).round() as u32).max(1);
        canvas_w = opts.max_width;
    }
    if canvas_w > MAX_DIMENSION || canvas_h > MAX_DIMENSION {
        return Err(format!(
            "canvas {canvas_w}x{canvas_h} exceeds the WebP limit of {MAX_DIMENSION}px per side — set max_width"
        ));
    }
    if u64::from(canvas_w) * u64::from(canvas_h) > MAX_CANVAS_PIXELS {
        return Err(format!(
            "canvas {canvas_w}x{canvas_h} is too large ({} MP, max {} MP) — set max_width to scale it down",
            (u64::from(canvas_w) * u64::from(canvas_h)) / 1_000_000,
            MAX_CANVAS_PIXELS / 1_000_000
        ));
    }

    let sequence = opts.order.sequence(images.len());
    let uniform_delay = opts.delay_ms.clamp(10, 60_000);

    let mut chunks: Vec<(Vec<u8>, u32)> = Vec::with_capacity(sequence.len());
    let mut any_alpha = false;
    let mut duration_ms: u64 = 0;
    // A boomerang replays source frames — encode each source image once.
    let mut cache: BTreeMap<usize, (Vec<u8>, bool)> = BTreeMap::new();
    let mut encoded_bytes: usize = 0;

    for &idx in &sequence {
        if !cache.contains_key(&idx) {
            let src = image::load_from_memory(&images[idx])
                .map_err(|e| format!("image #{}: could not decode: {e}", idx + 1))?;
            let canvas = compose_frame(&src, canvas_w, canvas_h, opts);
            drop(src);
            let frame_alpha = canvas.pixels().any(|p| p.0[3] != 255);
            let payload = encode_vp8l(&canvas, frame_alpha)?;
            encoded_bytes += payload.len();
            if encoded_bytes > MAX_ENCODED_BYTES {
                return Err(format!(
                    "the animation exceeds {} MB — use fewer/smaller images, set max_width, or reduce colors",
                    MAX_ENCODED_BYTES / 1_000_000
                ));
            }
            cache.insert(idx, (payload, frame_alpha));
        }
        let (payload, frame_alpha) = &cache[&idx];
        any_alpha |= *frame_alpha;
        let delay = if opts.frame_delays_ms.is_empty() {
            uniform_delay
        } else {
            opts.frame_delays_ms[idx].clamp(10, 60_000)
        };
        duration_ms += u64::from(delay);
        chunks.push((payload.clone(), delay));
    }

    let bytes = assemble_container(canvas_w, canvas_h, any_alpha, opts, &chunks);
    Ok(Animation {
        bytes,
        width: canvas_w,
        height: canvas_h,
        frames: sequence.len(),
        duration_ms,
    })
}

/// Scale + place one source image onto the shared canvas.
fn compose_frame(src: &DynamicImage, canvas_w: u32, canvas_h: u32, opts: &Options) -> RgbaImage {
    let mut canvas = RgbaImage::from_pixel(canvas_w, canvas_h, opts.background);
    let (iw, ih) = src.dimensions();
    let (iw, ih) = (iw.max(1), ih.max(1));

    match opts.fit {
        Fit::Stretch => {
            let resized = src.resize_exact(canvas_w, canvas_h, FilterType::Lanczos3).to_rgba8();
            image::imageops::overlay(&mut canvas, &resized, 0, 0);
        }
        Fit::Contain => {
            let scale = (canvas_w as f64 / iw as f64).min(canvas_h as f64 / ih as f64);
            let nw = ((iw as f64 * scale).round() as u32).clamp(1, canvas_w);
            let nh = ((ih as f64 * scale).round() as u32).clamp(1, canvas_h);
            let resized = src.resize_exact(nw, nh, FilterType::Lanczos3).to_rgba8();
            let ox = (canvas_w - nw) / 2;
            let oy = (canvas_h - nh) / 2;
            image::imageops::overlay(&mut canvas, &resized, ox as i64, oy as i64);
        }
        Fit::Cover => {
            let scale = (canvas_w as f64 / iw as f64).max(canvas_h as f64 / ih as f64);
            let nw = ((iw as f64 * scale).round() as u32).max(canvas_w);
            let nh = ((ih as f64 * scale).round() as u32).max(canvas_h);
            let resized = src.resize_exact(nw, nh, FilterType::Lanczos3).to_rgba8();
            let ox = ((nw - canvas_w) / 2) as i64;
            let oy = ((nh - canvas_h) / 2) as i64;
            let cropped =
                image::imageops::crop_imm(&resized, ox as u32, oy as u32, canvas_w, canvas_h)
                    .to_image();
            image::imageops::overlay(&mut canvas, &cropped, 0, 0);
        }
    }

    if opts.colors >= 2 {
        quantize(&mut canvas, opts.colors as usize);
    }
    canvas
}

/// Reduce a frame to `colors` colors (NeuQuant) — the WebP lossless coder packs
/// a small palette far more tightly, which is where the GIF-beating sizes come
/// from for flat-color/graphic frames.
fn quantize(canvas: &mut RgbaImage, colors: usize) {
    let opaque = canvas.pixels().all(|p| p.0[3] == 255);
    let nq = NeuQuant::new(10, colors, canvas.as_raw());
    for px in canvas.pixels_mut() {
        let idx = nq.index_of(&px.0);
        if let Some(c) = nq.lookup(idx) {
            px.0 = c;
            if opaque {
                px.0[3] = 255;
            }
        }
    }
}

/// Encode one canvas as a lossless still WebP and return just its `VP8L`
/// bitstream payload (the bytes that go inside an `ANMF` chunk).
fn encode_vp8l(canvas: &RgbaImage, keep_alpha: bool) -> Result<Vec<u8>, String> {
    let (w, h) = canvas.dimensions();
    let mut still = Vec::new();
    if keep_alpha {
        WebPEncoder::new(Cursor::new(&mut still))
            .encode(canvas.as_raw(), w, h, ColorType::Rgba8)
            .map_err(|e| format!("webp encode: {e}"))?;
    } else {
        let rgb: Vec<u8> = canvas.pixels().flat_map(|p| [p.0[0], p.0[1], p.0[2]]).collect();
        WebPEncoder::new(Cursor::new(&mut still))
            .encode(&rgb, w, h, ColorType::Rgb8)
            .map_err(|e| format!("webp encode: {e}"))?;
    }
    // Simple container: "RIFF" size "WEBP" "VP8L" size <payload>.
    if still.len() < 20 || &still[0..4] != b"RIFF" || &still[8..12] != b"WEBP" {
        return Err("webp encode: unexpected container".into());
    }
    if &still[12..16] != b"VP8L" {
        return Err("webp encode: expected a VP8L chunk".into());
    }
    let size = u32::from_le_bytes([still[16], still[17], still[18], still[19]]) as usize;
    let end = 20usize
        .checked_add(size)
        .filter(|e| *e <= still.len())
        .ok_or_else(|| "webp encode: truncated VP8L chunk".to_string())?;
    Ok(still[20..end].to_vec())
}

fn u24(v: u32) -> [u8; 3] {
    let b = v.to_le_bytes();
    [b[0], b[1], b[2]]
}

fn write_chunk(out: &mut Vec<u8>, name: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(name);
    out.extend_from_slice(&(data.len() as u32).to_le_bytes());
    out.extend_from_slice(data);
    if data.len() % 2 == 1 {
        out.push(0); // chunks are padded to an even size
    }
}

/// Assemble the extended (animated) WebP container around the per-frame VP8L
/// payloads: `RIFF/WEBP` → `VP8X` → `ANIM` → one `ANMF` per frame.
fn assemble_container(
    width: u32,
    height: u32,
    has_alpha: bool,
    opts: &Options,
    frames: &[(Vec<u8>, u32)],
) -> Vec<u8> {
    let mut body = Vec::new();

    let mut vp8x = Vec::with_capacity(10);
    let mut flags = 0b0000_0010u8; // animation
    if has_alpha {
        flags |= 1 << 4;
    }
    vp8x.push(flags);
    vp8x.extend_from_slice(&[0, 0, 0]); // reserved
    vp8x.extend_from_slice(&u24(width - 1));
    vp8x.extend_from_slice(&u24(height - 1));
    write_chunk(&mut body, b"VP8X", &vp8x);

    let bg = opts.background.0;
    let mut anim = Vec::with_capacity(6);
    anim.extend_from_slice(&[bg[2], bg[1], bg[0], bg[3]]); // background hint, BGRA
    anim.extend_from_slice(&opts.loop_count.to_le_bytes());
    write_chunk(&mut body, b"ANIM", &anim);

    for (payload, delay) in frames {
        let mut anmf = Vec::with_capacity(16 + payload.len() + 8);
        anmf.extend_from_slice(&u24(0)); // frame x / 2
        anmf.extend_from_slice(&u24(0)); // frame y / 2
        anmf.extend_from_slice(&u24(width - 1));
        anmf.extend_from_slice(&u24(height - 1));
        anmf.extend_from_slice(&u24(*delay & 0x00ff_ffff));
        anmf.push(0b0000_0010); // no blending, no disposal — full independent keyframe
        write_chunk(&mut anmf, b"VP8L", payload);
        write_chunk(&mut body, b"ANMF", &anmf);
    }

    let mut out = Vec::with_capacity(body.len() + 12);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&((body.len() + 4) as u32).to_le_bytes());
    out.extend_from_slice(b"WEBP");
    out.extend_from_slice(&body);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::webp::WebPDecoder;
    use image::{AnimationDecoder, ImageFormat};

    fn solid_png(w: u32, h: u32, color: Rgba<u8>) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, color);
        let mut out = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut out, ImageFormat::Png)
            .unwrap();
        out.into_inner()
    }

    fn frames_of(webp: &[u8]) -> Vec<image::Frame> {
        let dec = WebPDecoder::new(Cursor::new(webp.to_vec())).unwrap();
        dec.into_frames().collect::<Result<_, _>>().unwrap()
    }

    #[test]
    fn builds_multiframe_animation() {
        let imgs = vec![
            solid_png(20, 20, Rgba([255, 0, 0, 255])),
            solid_png(20, 20, Rgba([0, 255, 0, 255])),
            solid_png(10, 30, Rgba([0, 0, 255, 255])),
        ];
        let opts = Options {
            delay_ms: 150,
            ..Options::default()
        };
        let anim = animated_webp_from_images(&imgs, &opts).unwrap();
        assert_eq!(&anim.bytes[0..4], b"RIFF");
        assert_eq!(&anim.bytes[8..12], b"WEBP");
        assert_eq!((anim.width, anim.height), (20, 30));
        assert_eq!(anim.frames, 3);
        assert_eq!(anim.duration_ms, 450);

        let frames = frames_of(&anim.bytes);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0].buffer().dimensions(), (20, 30));
        // 150 ms per frame, exactly.
        assert_eq!(frames[1].delay().numer_denom_ms(), (150, 1));
        // Frame 1 is the red source, letterboxed onto a white canvas.
        assert_eq!(frames[0].buffer().get_pixel(10, 15).0, [255, 0, 0, 255]);
        assert_eq!(frames[0].buffer().get_pixel(1, 1).0, [255, 255, 255, 255]);
        assert_eq!(frames[1].buffer().get_pixel(10, 15).0, [0, 255, 0, 255]);
    }

    #[test]
    fn single_image_is_a_valid_still_animation() {
        let anim = animated_webp_from_images(
            &[solid_png(8, 8, Rgba([1, 2, 3, 255]))],
            &Options::default(),
        )
        .unwrap();
        let frames = frames_of(&anim.bytes);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].buffer().get_pixel(4, 4).0, [1, 2, 3, 255]);
    }

    #[test]
    fn alpha_survives_a_transparent_background() {
        let opts = Options {
            background: parse_color("transparent").unwrap(),
            ..Options::default()
        };
        // A 10x30 frame on a 30x30 canvas leaves transparent padding.
        let imgs = vec![
            solid_png(30, 30, Rgba([0, 0, 0, 255])),
            solid_png(10, 30, Rgba([9, 9, 9, 255])),
        ];
        let anim = animated_webp_from_images(&imgs, &opts).unwrap();
        let frames = frames_of(&anim.bytes);
        assert_eq!(frames[1].buffer().get_pixel(0, 0).0[3], 0);
        assert_eq!(frames[1].buffer().get_pixel(15, 15).0, [9, 9, 9, 255]);
    }

    #[test]
    fn order_and_per_frame_delays() {
        let imgs = vec![
            solid_png(10, 10, Rgba([255, 0, 0, 255])),
            solid_png(10, 10, Rgba([0, 255, 0, 255])),
            solid_png(10, 10, Rgba([0, 0, 255, 255])),
        ];
        let boomerang = Options {
            order: Order::Boomerang,
            frame_delays_ms: vec![100, 200, 300],
            ..Options::default()
        };
        let anim = animated_webp_from_images(&imgs, &boomerang).unwrap();
        assert_eq!(anim.frames, 4); // 0,1,2,1
        assert_eq!(anim.duration_ms, 100 + 200 + 300 + 200);
        let frames = frames_of(&anim.bytes);
        assert_eq!(frames[3].buffer().get_pixel(5, 5).0, [0, 255, 0, 255]);
        assert_eq!(frames[3].delay().numer_denom_ms(), (200, 1));

        let reverse = Options {
            order: Order::Reverse,
            ..Options::default()
        };
        let anim = animated_webp_from_images(&imgs, &reverse).unwrap();
        let frames = frames_of(&anim.bytes);
        assert_eq!(frames[0].buffer().get_pixel(5, 5).0, [0, 0, 255, 255]);
    }

    #[test]
    fn fit_modes_and_max_width() {
        let imgs = vec![solid_png(40, 20, Rgba([255, 0, 0, 255]))];
        let cover = Options {
            fit: Fit::Cover,
            ..Options::default()
        };
        // Single 40x20 source → 40x20 canvas; cover fills it edge to edge.
        let anim = animated_webp_from_images(&imgs, &cover).unwrap();
        let frames = frames_of(&anim.bytes);
        assert_eq!(frames[0].buffer().get_pixel(0, 0).0, [255, 0, 0, 255]);

        let scaled = Options {
            max_width: 20,
            fit: Fit::Stretch,
            ..Options::default()
        };
        let anim = animated_webp_from_images(&imgs, &scaled).unwrap();
        assert_eq!((anim.width, anim.height), (20, 10));
    }

    #[test]
    fn quantized_output_is_smaller_and_still_decodes() {
        // Photographic-style noise: incompressible at full color, cheap at 8
        // colors. (A SMOOTH gradient is the opposite case — the lossless
        // predictor already codes it in a few hundred bytes, so quantizing it
        // can make the file bigger. That trade-off is documented for users.)
        let mut img = RgbaImage::new(120, 120);
        let mut seed: u32 = 12345;
        for p in img.pixels_mut() {
            let mut next = || {
                seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                (seed >> 24) as u8
            };
            *p = Rgba([next(), next(), next(), 255]);
        }
        let mut buf = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(img)
            .write_to(&mut buf, ImageFormat::Png)
            .unwrap();
        let src = vec![buf.into_inner()];

        let full = animated_webp_from_images(&src, &Options::default()).unwrap();
        let few = animated_webp_from_images(
            &src,
            &Options {
                colors: 8,
                ..Options::default()
            },
        )
        .unwrap();
        assert!(
            few.bytes.len() < full.bytes.len(),
            "8-color {} should be smaller than full-color {}",
            few.bytes.len(),
            full.bytes.len()
        );
        assert_eq!(frames_of(&few.bytes).len(), 1);
    }

    #[test]
    fn loop_count_is_written() {
        let imgs = vec![solid_png(8, 8, Rgba([0, 0, 0, 255]))];
        let anim = animated_webp_from_images(
            &imgs,
            &Options {
                loop_count: 3,
                ..Options::default()
            },
        )
        .unwrap();
        let dec = WebPDecoder::new(Cursor::new(anim.bytes.clone())).unwrap();
        // image 0.25 exposes the loop count via the decoder's ANIM parse.
        assert!(dec.has_animation());
        // ANIM chunk sits right after the 18-byte VP8X chunk; loop count is its
        // last 2 bytes.
        let anim_start = 12 + 18 + 8;
        let lc = u16::from_le_bytes([anim.bytes[anim_start + 4], anim.bytes[anim_start + 5]]);
        assert_eq!(lc, 3);
    }

    #[test]
    fn colors_and_helpers_parse() {
        assert_eq!(parse_color("#fff").unwrap(), Rgba([255, 255, 255, 255]));
        assert_eq!(parse_color("#ff0000").unwrap(), Rgba([255, 0, 0, 255]));
        assert_eq!(parse_color("#00000080").unwrap(), Rgba([0, 0, 0, 128]));
        assert_eq!(parse_color("transparent").unwrap(), Rgba([0, 0, 0, 0]));
        assert!(parse_color("nope").is_err());
        assert_eq!(parse_delays("100, 250,300").unwrap(), vec![100, 250, 300]);
        assert_eq!(parse_delays("  ").unwrap(), Vec::<u32>::new());
        assert_eq!(parse_delays("1").unwrap(), vec![10]); // clamped to the 10 ms floor
        assert!(parse_delays("fast").is_err());
        assert_eq!(Fit::parse("cover").unwrap(), Fit::Cover);
        assert!(Fit::parse("squish").is_err());
        assert_eq!(Order::parse("boomerang").unwrap(), Order::Boomerang);
        assert!(Order::parse("shuffle").is_err());
    }

    #[test]
    fn errors() {
        assert!(animated_webp_from_images(&[], &Options::default()).is_err());
        let one = vec![solid_png(4, 4, Rgba([0, 0, 0, 255]))];
        // Wrong number of per-frame delays.
        let err = animated_webp_from_images(
            &one,
            &Options {
                frame_delays_ms: vec![100, 200],
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("one delay per image"), "{err}");
        // Not an image.
        let err = animated_webp_from_images(&[b"not an image".to_vec()], &Options::default())
            .unwrap_err();
        assert!(err.contains("could not decode"), "{err}");
        // Out-of-range palette size.
        let err = animated_webp_from_images(
            &one,
            &Options {
                colors: 300,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("2-256"), "{err}");
    }
}
