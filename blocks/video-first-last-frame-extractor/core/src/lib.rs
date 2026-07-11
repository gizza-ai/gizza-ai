//! gizza-ai/video-first-last-frame-extractor core — pure ffmpeg argv construction
//! shared by the chat skill block and the standalone web page. No wafer/wasm-bindgen deps.
//!
//! Grabs the **first** frame and the **last** frame of a video and joins them into a
//! single comparison image — side by side (`horizontal`) or stacked (`vertical`).
//! Both frames come from ONE decode pass: the graph splits the stream, `select`s
//! frame 0 on one branch, and `reverse`s the other branch so its frame 0 is the
//! original last frame, then `hstack`/`vstack` glues the two together. Because the
//! page renders one output file, the pair is delivered as this single stitched
//! image rather than two separate downloads. Output is always an image (PNG or
//! JPG), so the page renders it as an `<img>` even though the input is `video/*`.

/// Join layouts: the two frames placed side by side, or stacked top/bottom.
pub const LAYOUTS: &[&str] = &["horizontal", "vertical"];

/// Output image formats. PNG is lossless (crisp UI/text frames); JPG is smaller
/// for photographic content.
pub const FORMATS: &[&str] = &["png", "jpg"];

/// Map an output format name to its file extension (PNG or JPG). Rejects anything
/// outside the advertised enum with a guiding message.
pub fn format_ext(format: &str) -> Result<&'static str, String> {
    match format {
        "png" => Ok("png"),
        "jpg" | "jpeg" => Ok("jpg"),
        other => Err(format!("format must be one of png, jpg (got {other:?})")),
    }
}

/// Map a join layout to the stacking filter that combines the two frames.
/// `horizontal` places first|last left-to-right (`hstack`, equal heights);
/// `vertical` stacks first over last (`vstack`, equal widths). The first and
/// last frames share the source dimensions, so both stacks always align.
fn layout_filter(layout: &str) -> Result<&'static str, String> {
    match layout {
        "horizontal" => Ok("hstack"),
        "vertical" => Ok("vstack"),
        other => Err(format!(
            "layout must be one of horizontal, vertical (got {other:?})"
        )),
    }
}

/// Build the ffmpeg argv (no leading `ffmpeg`) that extracts the first + last
/// frame of `in_name` and stitches them with `stack` (`hstack`/`vstack`), writing
/// `out_name`.
///
/// The filtergraph reads the input once: `split` duplicates the decoded stream;
/// one branch keeps frame 0 (the first frame), the other `reverse`s and keeps its
/// frame 0 (the original last frame); `setpts=PTS-STARTPTS` re-bases both so the
/// stack filter accepts them; `hstack`/`vstack` joins them. `-frames:v 1 -update 1`
/// writes exactly one image. NOTE: `reverse` buffers the decoded video in memory,
/// so the handler caps the input size.
pub fn build_argv(in_name: &str, out_name: &str, stack: &str) -> Vec<String> {
    // The comma inside `eq(n,0)` is escaped (`eq(n\,0)`) so ffmpeg's filtergraph
    // parser reads it as one expression argument, not a filterchain separator.
    let graph = format!(
        "[0:v]split=2[s0][s1];\
         [s0]select=eq(n\\,0),setpts=PTS-STARTPTS[a];\
         [s1]reverse,select=eq(n\\,0),setpts=PTS-STARTPTS[b];\
         [a][b]{stack}=inputs=2[out]"
    );
    vec![
        "-i".into(),
        in_name.into(),
        "-filter_complex".into(),
        graph,
        "-map".into(),
        "[out]".into(),
        "-frames:v".into(),
        "1".into(),
        "-update".into(),
        "1".into(),
        "-y".into(),
        out_name.into(),
    ]
}

/// Validate the layout + format and return `(argv, out_name)` for an input file.
/// The output is `out.png` or `out.jpg` (the stitched first+last frame image).
/// Shared by the chat block and the web page (video file in → image file out).
pub fn plan(in_name: &str, layout: &str, format: &str) -> Result<(Vec<String>, String), String> {
    let stack = layout_filter(layout)?;
    let ext = format_ext(format)?;
    let out_name = format!("out.{ext}");
    Ok((build_argv(in_name, &out_name, stack), out_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_horizontal_png_uses_hstack_and_png_out() {
        let (argv, out) = plan("in.mp4", "horizontal", "png").unwrap();
        assert_eq!(out, "out.png");
        assert_eq!(argv[0], "-i");
        assert_eq!(argv[1], "in.mp4");
        let graph = &argv[3];
        assert!(graph.contains("hstack=inputs=2"), "graph: {graph}");
        assert!(graph.contains("reverse"), "graph must grab the last frame");
        assert!(graph.contains("select=eq(n\\,0)"), "escaped comma: {graph}");
        assert!(argv.windows(2).any(|w| w[0] == "-frames:v" && w[1] == "1"));
        assert!(argv.windows(2).any(|w| w[0] == "-update" && w[1] == "1"));
        assert!(argv.windows(2).any(|w| w[0] == "-map" && w[1] == "[out]"));
        assert_eq!(argv.last().map(String::as_str), Some("out.png"));
    }

    #[test]
    fn plan_vertical_jpg_uses_vstack_and_jpg_out() {
        let (argv, out) = plan("clip.webm", "vertical", "jpg").unwrap();
        assert_eq!(out, "out.jpg");
        assert!(argv[3].contains("vstack=inputs=2"), "graph: {}", argv[3]);
        assert_eq!(argv.last().map(String::as_str), Some("out.jpg"));
    }

    #[test]
    fn format_ext_accepts_jpeg_alias() {
        assert_eq!(format_ext("jpeg").unwrap(), "jpg");
        assert_eq!(format_ext("png").unwrap(), "png");
    }

    #[test]
    fn plan_rejects_unknown_layout() {
        let err = plan("in.mp4", "diagonal", "png").unwrap_err();
        assert!(err.contains("layout"), "got: {err}");
    }

    #[test]
    fn plan_rejects_unknown_format() {
        let err = plan("in.mp4", "horizontal", "gif").unwrap_err();
        assert!(err.contains("format"), "got: {err}");
    }
}
