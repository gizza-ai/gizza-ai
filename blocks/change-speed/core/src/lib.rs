//! gizza-ai/change-speed core — pure ffmpeg argv construction shared by the chat
//! skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Speeds up / slows down a video by `factor` (e.g. 2.0 = twice as fast, 0.5 =
//! half speed) while keeping the audio in sync: video PTS is divided by the
//! factor (`setpts=PTS/factor`) and the audio tempo is multiplied by it
//! (`atempo`). `atempo` only accepts 0.5..=2.0 per instance, so larger/smaller
//! factors are chained. The `-filter:a` is a per-stream filter, so it is simply
//! skipped when the input has no audio track.

pub const MIN_FACTOR: f64 = 0.25;
pub const MAX_FACTOR: f64 = 4.0;

fn out_ext(in_name: &str) -> &str {
    in_name.rsplit_once('.').map(|(_, e)| e).filter(|e| !e.is_empty()).unwrap_or("mp4")
}

/// Decompose `factor` into a chain of `atempo=` filters each within 0.5..=2.0.
fn atempo_chain(factor: f64) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut f = factor;
    while f > 2.0 {
        parts.push("atempo=2.0".to_string());
        f /= 2.0;
    }
    while f < 0.5 {
        parts.push("atempo=0.5".to_string());
        f /= 0.5;
    }
    parts.push(format!("atempo={f}"));
    parts.join(",")
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for a speed change.
pub fn build_argv(in_name: &str, out_name: &str, factor: f64) -> Vec<String> {
    vec![
        "-i".into(),
        in_name.into(),
        "-filter:v".into(),
        format!("setpts=PTS/{factor}"),
        "-filter:a".into(),
        atempo_chain(factor),
        out_name.into(),
    ]
}

/// Validate the factor and return `(argv, out_name)`. `out_name` keeps the input
/// extension.
pub fn plan(in_name: &str, factor: f64) -> Result<(Vec<String>, String), String> {
    if !factor.is_finite() || factor <= 0.0 {
        return Err("factor must be a positive number".into());
    }
    if factor < MIN_FACTOR || factor > MAX_FACTOR {
        return Err(format!("factor must be between {MIN_FACTOR} and {MAX_FACTOR}"));
    }
    let out_name = format!("out.{}", out_ext(in_name));
    Ok((build_argv(in_name, &out_name, factor), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vf(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-filter:v").unwrap();
        argv[i + 1].clone()
    }
    fn af(argv: &[String]) -> String {
        let i = argv.iter().position(|a| a == "-filter:a").unwrap();
        argv[i + 1].clone()
    }

    #[test]
    fn double_speed_setpts_and_atempo() {
        let argv = build_argv("in.mp4", "out.mp4", 2.0);
        assert_eq!(vf(&argv), "setpts=PTS/2");
        assert_eq!(af(&argv), "atempo=2");
    }

    #[test]
    fn half_speed() {
        let argv = build_argv("in.mp4", "out.mp4", 0.5);
        assert_eq!(vf(&argv), "setpts=PTS/0.5");
        assert_eq!(af(&argv), "atempo=0.5");
    }

    #[test]
    fn large_factor_chains_atempo() {
        // 4x → atempo=2.0,atempo=2.0
        assert_eq!(af(&build_argv("i.mp4", "o.mp4", 4.0)), "atempo=2.0,atempo=2");
        // 3x → atempo=2.0,atempo=1.5
        assert_eq!(af(&build_argv("i.mp4", "o.mp4", 3.0)), "atempo=2.0,atempo=1.5");
    }

    #[test]
    fn small_factor_chains_atempo() {
        // 0.25x → atempo=0.5,atempo=0.5
        assert_eq!(af(&build_argv("i.mp4", "o.mp4", 0.25)), "atempo=0.5,atempo=0.5");
    }

    #[test]
    fn plan_validates_and_keeps_extension() {
        let (argv, out) = plan("clip.webm", 1.5).unwrap();
        assert_eq!(out, "out.webm");
        assert!(argv.iter().any(|a| a == "setpts=PTS/1.5"));
        assert!(plan("i.mp4", 0.0).is_err());
        assert!(plan("i.mp4", 10.0).is_err());
        assert!(plan("i.mp4", -2.0).is_err());
    }
}
