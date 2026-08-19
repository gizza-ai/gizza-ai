//! gizza-ai/video-deinterlace core — pure ffmpeg argv construction shared by the
//! chat block and the standalone web page. No wasm-bindgen deps.
//!
//! Removes interlacing "combing" (the horizontal comb teeth on motion that come
//! from two half-height fields captured a half-frame apart being woven into one
//! frame) and writes clean progressive frames, using ffmpeg's motion-adaptive
//! deinterlacers:
//!
//! * `bwdif` (default) — "Bob Weaver Deinterlacing Filter": yadif's motion
//!   adaptivity plus the w3fdif interpolator, so it keeps more vertical detail
//!   on the interpolated lines. Same cost class as yadif in practice.
//! * `yadif` — "yet another deinterlacing filter", the long-standing ffmpeg
//!   default. Slightly softer on fine detail, but it is the most widely
//!   documented/compatible choice.
//!
//! Both take the same three options, which this crate maps 1:1 from the
//! user-facing params:
//!
//! * `mode` — [`Mode::Frame`] (`send_frame`) emits one progressive frame per
//!   input frame and KEEPS the frame rate (50i → 25p); [`Mode::Field`]
//!   (`send_field`) emits one frame per FIELD and therefore DOUBLES the frame
//!   rate (50i → 50p), which restores the original 50-motion-steps-per-second
//!   fluidity of broadcast footage at the cost of ~2× the frames.
//! * `field_order` — which field is temporally first. [`FieldOrder::Auto`]
//!   trusts the flags the container/codec carries (right for almost every real
//!   capture); `tff`/`bff` force it, which is what fixes the classic "motion
//!   stutters / jitters back and forth" symptom of a mis-flagged file.
//! * `apply_to` — [`Apply::All`] deinterlaces every frame (`deint=all`, the
//!   ffmpeg default) and is the safe choice because most captures are NOT
//!   flagged; [`Apply::Flagged`] (`deint=interlaced`) only touches frames the
//!   decoder marked interlaced, so progressive frames in mixed footage pass
//!   through untouched.
//!
//! Because the picture is rewritten, the video stream is re-encoded to H.264
//! (`-crf 20`, visually near-transparent). Audio is stream-copied when the
//! container is kept (mp4/mov/m4v/mkv) and only re-encoded to AAC when the
//! input container can't hold H.264/AAC (e.g. webm), in which case the output
//! switches to MP4 — see `gizza_ai_block_utils::ffmpeg::h264_out_ext`.
//!
//! NOTE: inverse telecine (`fieldmatch`+`decimate`, for 29.97i film-sourced
//! footage that should return to 23.976p) is deliberately NOT part of this
//! tool — it is a different operation with its own failure modes, and running
//! it by accident on true video-sourced footage drops real frames.

use gizza_ai_block_utils::ffmpeg::h264_out_ext;

/// Which ffmpeg deinterlacer to run.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Filter {
    /// `bwdif` — Bob Weaver, motion-adaptive with a w3fdif interpolator.
    Bwdif,
    /// `yadif` — the classic ffmpeg deinterlacer.
    Yadif,
}

impl Filter {
    /// The ffmpeg filter name.
    pub fn name(self) -> &'static str {
        match self {
            Filter::Bwdif => "bwdif",
            Filter::Yadif => "yadif",
        }
    }
}

/// One output frame per input frame (keep fps) vs one per field (double fps).
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Mode {
    /// `send_frame` — 50i → 25p, frame rate unchanged.
    Frame,
    /// `send_field` — 50i → 50p, frame rate doubled (smoother motion).
    Field,
}

impl Mode {
    /// The `mode=` value for yadif/bwdif.
    pub fn ffmpeg_value(self) -> &'static str {
        match self {
            Mode::Frame => "send_frame",
            Mode::Field => "send_field",
        }
    }

    /// Whether this mode doubles the output frame rate.
    pub fn doubles_frame_rate(self) -> bool {
        matches!(self, Mode::Field)
    }
}

/// Which field of an interlaced frame is temporally first.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FieldOrder {
    /// Trust the flags in the file (yadif/bwdif `parity=auto`).
    Auto,
    /// Top field first — DV/HDV, most 1080i broadcast.
    Tff,
    /// Bottom field first — most DVD/analogue-capture SD (NTSC 480i, PAL DV).
    Bff,
}

impl FieldOrder {
    /// The `parity=` value for yadif/bwdif.
    pub fn ffmpeg_value(self) -> &'static str {
        match self {
            FieldOrder::Auto => "auto",
            FieldOrder::Tff => "tff",
            FieldOrder::Bff => "bff",
        }
    }
}

/// Which frames the deinterlacer touches.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Apply {
    /// `deint=all` — deinterlace every frame (the ffmpeg default).
    All,
    /// `deint=interlaced` — only frames the decoder flagged as interlaced.
    Flagged,
}

impl Apply {
    /// The `deint=` value for yadif/bwdif.
    pub fn ffmpeg_value(self) -> &'static str {
        match self {
            Apply::All => "all",
            Apply::Flagged => "interlaced",
        }
    }
}

/// Parse the user-facing filter string. Empty defaults to bwdif.
pub fn parse_filter(s: &str) -> Result<Filter, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "bwdif" => Ok(Filter::Bwdif),
        "yadif" => Ok(Filter::Yadif),
        other => Err(format!("filter {other:?} not supported (bwdif|yadif)")),
    }
}

/// Parse the user-facing mode string. Empty defaults to frame (keep fps).
pub fn parse_mode(s: &str) -> Result<Mode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "frame" => Ok(Mode::Frame),
        "field" => Ok(Mode::Field),
        other => Err(format!("mode {other:?} not supported (frame|field)")),
    }
}

/// Parse the user-facing field-order string. Empty defaults to auto.
pub fn parse_field_order(s: &str) -> Result<FieldOrder, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => Ok(FieldOrder::Auto),
        "tff" => Ok(FieldOrder::Tff),
        "bff" => Ok(FieldOrder::Bff),
        other => Err(format!(
            "field_order {other:?} not supported (auto|tff|bff)"
        )),
    }
}

/// Parse the user-facing apply-to string. Empty defaults to all.
pub fn parse_apply(s: &str) -> Result<Apply, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "all" => Ok(Apply::All),
        "flagged" => Ok(Apply::Flagged),
        other => Err(format!("apply_to {other:?} not supported (all|flagged)")),
    }
}

/// Build the ffmpeg `-vf` filter string.
///
/// yadif and bwdif accept the identical `mode`/`parity`/`deint` options, so the
/// only difference is the filter name. Every option is written explicitly
/// (rather than relying on the filter defaults) so the argv is self-documenting
/// and a future ffmpeg default change can't silently alter output.
pub fn build_filter(filter: Filter, mode: Mode, order: FieldOrder, apply: Apply) -> String {
    format!(
        "{}=mode={}:parity={}:deint={}",
        filter.name(),
        mode.ffmpeg_value(),
        order.ffmpeg_value(),
        apply.ffmpeg_value()
    )
}

/// Build the ffmpeg argv (no leading `ffmpeg`) and the output filename for a
/// deinterlace pass. Shared verbatim by the web page (`build_argv`) and the
/// chat block.
///
/// The deinterlacer rewrites the picture, so the video is re-encoded to H.264
/// (`libx264`, `-preset medium`, `-crf 20`). The input container is kept when it
/// can hold H.264/AAC, otherwise the output is `out.mp4`; audio is
/// stream-copied when the container is kept and re-encoded to AAC otherwise.
///
/// `-flags -ildct-ilme` and `-field_order progressive` make the *output*
/// explicitly progressive: without them libx264 can inherit the source's
/// interlaced coding flags and hand players a file that still claims to be
/// interlaced, which makes some players re-apply their own deinterlacer to
/// already-clean frames.
pub fn build_argv(
    filter: Filter,
    mode: Mode,
    order: FieldOrder,
    apply: Apply,
    in_name: &str,
) -> (Vec<String>, String) {
    let vf = build_filter(filter, mode, order, apply);
    let (ext, reencode_audio) = h264_out_ext(in_name);
    let out_name = format!("out.{ext}");
    let mut argv = vec![
        "-i".to_string(),
        in_name.to_string(),
        "-vf".to_string(),
        vf,
        "-c:v".to_string(),
        "libx264".to_string(),
        "-preset".to_string(),
        "medium".to_string(),
        "-crf".to_string(),
        "20".to_string(),
        "-flags".to_string(),
        "-ildct-ilme".to_string(),
        "-field_order".to_string(),
        "progressive".to_string(),
        "-c:a".to_string(),
    ];
    argv.push(if reencode_audio { "aac" } else { "copy" }.to_string());
    argv.push(out_name.clone());
    (argv, out_name)
}

/// Validate the request and return `(argv, out_name)`. Every param is parsed
/// from its user string (empty → that param's default: bwdif / frame / auto /
/// all); an unknown value is a clear user error.
pub fn plan(
    filter: &str,
    mode: &str,
    field_order: &str,
    apply_to: &str,
    in_name: &str,
) -> Result<(Vec<String>, String), String> {
    let filter = parse_filter(filter)?;
    let mode = parse_mode(mode)?;
    let order = parse_field_order(field_order)?;
    let apply = parse_apply(apply_to)?;
    Ok(build_argv(filter, mode, order, apply, in_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filter_variants() {
        assert_eq!(parse_filter("").unwrap(), Filter::Bwdif);
        assert_eq!(parse_filter("bwdif").unwrap(), Filter::Bwdif);
        assert_eq!(parse_filter(" YADIF ").unwrap(), Filter::Yadif);
        let err = parse_filter("nnedi").unwrap_err();
        assert!(err.contains("bwdif|yadif"), "{err}");
    }

    #[test]
    fn parse_mode_variants() {
        assert_eq!(parse_mode("").unwrap(), Mode::Frame);
        assert_eq!(parse_mode("frame").unwrap(), Mode::Frame);
        assert_eq!(parse_mode("Field").unwrap(), Mode::Field);
        assert!(parse_mode("send_field").is_err());
        assert!(!Mode::Frame.doubles_frame_rate());
        assert!(Mode::Field.doubles_frame_rate());
    }

    #[test]
    fn parse_field_order_variants() {
        assert_eq!(parse_field_order("").unwrap(), FieldOrder::Auto);
        assert_eq!(parse_field_order("auto").unwrap(), FieldOrder::Auto);
        assert_eq!(parse_field_order("TFF").unwrap(), FieldOrder::Tff);
        assert_eq!(parse_field_order(" bff ").unwrap(), FieldOrder::Bff);
        let err = parse_field_order("top").unwrap_err();
        assert!(err.contains("auto|tff|bff"), "{err}");
    }

    #[test]
    fn parse_apply_variants() {
        assert_eq!(parse_apply("").unwrap(), Apply::All);
        assert_eq!(parse_apply("all").unwrap(), Apply::All);
        assert_eq!(parse_apply("Flagged").unwrap(), Apply::Flagged);
        // `interlaced` is ffmpeg's spelling, not the tool's — reject it loudly
        // rather than silently accepting two names for one value.
        assert!(parse_apply("interlaced").is_err());
    }

    #[test]
    fn defaults_build_the_documented_bwdif_filter() {
        let (argv, out) = plan("", "", "", "", "in.mp4").unwrap();
        assert_eq!(out, "out.mp4");
        let vf = &argv[argv.iter().position(|a| a == "-vf").unwrap() + 1];
        assert_eq!(vf, "bwdif=mode=send_frame:parity=auto:deint=all");
        assert!(argv.contains(&"libx264".to_string()));
        // mp4 in → mp4 out, so the audio is stream-copied untouched.
        assert!(argv.contains(&"copy".to_string()));
        // The output is flagged progressive, not interlaced.
        assert_eq!(
            argv[argv.iter().position(|a| a == "-field_order").unwrap() + 1],
            "progressive"
        );
    }

    #[test]
    fn double_rate_yadif_with_forced_bottom_field_first() {
        let (argv, _) = plan("yadif", "field", "bff", "flagged", "in.mkv").unwrap();
        let vf = &argv[argv.iter().position(|a| a == "-vf").unwrap() + 1];
        assert_eq!(vf, "yadif=mode=send_field:parity=bff:deint=interlaced");
    }

    #[test]
    fn every_enum_combination_builds_a_well_formed_filter() {
        for f in [Filter::Bwdif, Filter::Yadif] {
            for m in [Mode::Frame, Mode::Field] {
                for o in [FieldOrder::Auto, FieldOrder::Tff, FieldOrder::Bff] {
                    for a in [Apply::All, Apply::Flagged] {
                        let vf = build_filter(f, m, o, a);
                        assert!(vf.starts_with(f.name()), "{vf}");
                        assert_eq!(vf.matches(':').count(), 2, "{vf}");
                        assert!(vf.contains(m.ffmpeg_value()), "{vf}");
                        assert!(vf.contains(&format!("parity={}", o.ffmpeg_value())), "{vf}");
                        assert!(vf.contains(&format!("deint={}", a.ffmpeg_value())), "{vf}");
                    }
                }
            }
        }
    }

    #[test]
    fn webm_input_switches_container_and_reencodes_audio() {
        let (argv, out) = plan("bwdif", "frame", "auto", "all", "clip.webm").unwrap();
        assert_eq!(out, "out.mp4");
        assert!(argv.contains(&"aac".to_string()));
        assert!(!argv.contains(&"copy".to_string()));
    }

    #[test]
    fn mov_and_mkv_keep_their_container() {
        assert_eq!(plan("", "", "", "", "tape.MOV").unwrap().1, "out.mov");
        assert_eq!(plan("", "", "", "", "cap.mkv").unwrap().1, "out.mkv");
    }

    #[test]
    fn unknown_values_are_rejected_per_param() {
        assert!(plan("nnedi", "", "", "", "in.mp4").is_err());
        assert!(plan("", "double", "", "", "in.mp4").is_err());
        assert!(plan("", "", "top", "", "in.mp4").is_err());
        assert!(plan("", "", "", "only", "in.mp4").is_err());
    }
}
