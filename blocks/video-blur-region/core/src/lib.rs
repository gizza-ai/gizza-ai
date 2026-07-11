//! gizza-ai/video-blur-region core — pure ffmpeg argv construction shared by the
//! chat skill block and the standalone web page. No wasm-bindgen deps.
//!
//! Blurs OR pixelates a fixed `width`x`height` rectangle at top-left offset
//! `x`/`y`, applied to every frame of the video. The region is cropped out,
//! processed, and overlaid back onto the original frame so only that rectangle
//! changes. Re-encodes video via libx264 and (re)encodes audio via AAC; the
//! input container is kept when it can hold H.264 + AAC (mp4/mov/m4v/mkv),
//! anything else (webm, …) switches to mp4 — see
//! `gizza_ai_block_utils::ffmpeg::h264_out_ext`.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Redaction mode for the region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Soft Gaussian blur (sigma = strength).
    Blur,
    /// Coarse mosaic: block size in pixels = strength.
    Pixelate,
}

impl Mode {
    /// Parse the descriptor/CLI/page string value.
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "blur" => Ok(Mode::Blur),
            "pixelate" | "pixellate" | "mosaic" => Ok(Mode::Pixelate),
            other => Err(format!("mode must be \"blur\" or \"pixelate\", got \"{other}\"")),
        }
    }
}

/// Build the `-filter_complex` graph string that blurs/pixelates the region and
/// overlays it back onto the source frame.
fn filter_complex(x: u32, y: u32, w: u32, h: u32, mode: Mode, strength: u32) -> String {
    match mode {
        Mode::Blur => {
            // gblur sigma is unbounded, so a large strength won't crash on a
            // small region (boxblur radius must stay < the region dimension).
            format!("[0:v]crop={w}:{h}:{x}:{y},gblur=sigma={strength}[fg];[0:v][fg]overlay={x}:{y}")
        }
        Mode::Pixelate => {
            // Downscale the region to (w/block)x(h/block), then nearest-neighbour
            // scale it back up so each source block becomes one flat pixel.
            let dw = (w / strength).max(1);
            let dh = (h / strength).max(1);
            format!(
                "[0:v]crop={w}:{h}:{x}:{y},scale={dw}:{dh}:flags=neighbor,scale={w}:{h}:flags=neighbor[fg];[0:v][fg]overlay={x}:{y}"
            )
        }
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for the given region + mode.
pub fn build_argv(in_name: &str, out_name: &str, x: u32, y: u32, w: u32, h: u32, mode: Mode, strength: u32) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        filter_complex(x, y, w, h, mode, strength),
        "-c:v".into(),
        "libx264".into(),
        "-preset".into(),
        "medium".into(),
        "-c:a".into(),
        "aac".into(),
        out_name.into(),
    ]
}

/// Validate the region + strength and return `(argv, out_name)`. `out_name`
/// keeps the input container when it can hold H.264 + AAC; otherwise `out.mp4`.
pub fn plan(in_name: &str, x: u32, y: u32, w: u32, h: u32, mode: Mode, strength: u32) -> Result<(Vec<String>, String), String> {
    if w == 0 || h == 0 {
        return Err("region width and height must be > 0".into());
    }
    if strength == 0 {
        return Err("strength must be > 0".into());
    }
    let out_name = format!("out.{}", h264_out_ext(in_name).0);
    Ok((build_argv(in_name, &out_name, x, y, w, h, mode, strength), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_builds_crop_gblur_overlay() {
        let (argv, out) = plan("in.mp4", 10, 20, 320, 240, Mode::Blur, 25).unwrap();
        assert_eq!(out, "out.mp4");
        let fc = argv.iter().position(|a| a == "-filter_complex").unwrap();
        assert_eq!(
            argv[fc + 1],
            "[0:v]crop=320:240:10:20,gblur=sigma=25[fg];[0:v][fg]overlay=10:20"
        );
        assert!(argv.windows(2).any(|w| w[0] == "-c:a" && w[1] == "aac"));
    }

    #[test]
    fn pixelate_downscales_by_block_size() {
        let (argv, _) = plan("in.mp4", 0, 0, 320, 160, Mode::Pixelate, 16).unwrap();
        let fc = argv.iter().position(|a| a == "-filter_complex").unwrap();
        // 320/16 = 20, 160/16 = 10.
        assert_eq!(
            argv[fc + 1],
            "[0:v]crop=320:160:0:0,scale=20:10:flags=neighbor,scale=320:160:flags=neighbor[fg];[0:v][fg]overlay=0:0"
        );
    }

    #[test]
    fn pixelate_block_larger_than_region_clamps_to_one() {
        let (argv, _) = plan("in.mp4", 0, 0, 40, 30, Mode::Pixelate, 100).unwrap();
        let fc = argv.iter().position(|a| a == "-filter_complex").unwrap();
        assert!(argv[fc + 1].contains("scale=1:1:flags=neighbor"));
    }

    #[test]
    fn mode_parse_accepts_aliases_and_rejects_junk() {
        assert_eq!(Mode::parse("Blur").unwrap(), Mode::Blur);
        assert_eq!(Mode::parse("pixelate").unwrap(), Mode::Pixelate);
        assert_eq!(Mode::parse("mosaic").unwrap(), Mode::Pixelate);
        assert!(Mode::parse("swirl").is_err());
    }

    #[test]
    fn plan_keeps_h264_capable_containers_and_switches_webm() {
        for ext in ["mp4", "mov", "m4v", "mkv"] {
            let (_, out) = plan(&format!("clip.{ext}"), 0, 0, 64, 64, Mode::Blur, 10).unwrap();
            assert_eq!(out, format!("out.{ext}"));
        }
        assert_eq!(plan("clip.webm", 0, 0, 64, 64, Mode::Blur, 10).unwrap().1, "out.mp4");
        assert_eq!(plan("CLIP.MP4", 0, 0, 64, 64, Mode::Blur, 10).unwrap().1, "out.mp4");
    }

    #[test]
    fn plan_rejects_zero_region_and_zero_strength() {
        assert!(plan("in.mp4", 0, 0, 0, 100, Mode::Blur, 10).is_err());
        assert!(plan("in.mp4", 0, 0, 100, 0, Mode::Blur, 10).is_err());
        assert!(plan("in.mp4", 0, 0, 100, 100, Mode::Blur, 0).is_err());
    }
}
