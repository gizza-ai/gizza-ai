//! gizza-ai/image-resize-to-filesize core — pure, surface-agnostic logic shared
//! by the chat/CLI block (`src/lib.rs`) and the standalone page (`web/src/lib.rs`
//! + `page/custom.js`).
//!
//! Unlike a single-pass ffmpeg tool, hitting a *target file size* needs a search:
//! re-encode at a candidate quality, measure the output bytes, and adjust. This
//! module owns the two pieces that MUST stay identical across every surface:
//!
//!   1. [`plan_attempt`] — the ffmpeg argv for ONE encode attempt at a given
//!      quality (and optional width cap). Both the Rust loop and the JS loop
//!      call it so the encoder flags never drift.
//!   2. [`search_quality`] — the binary search over quality `5..=95` for the
//!      HIGHEST quality whose output is at or under the byte budget. It takes a
//!      `probe` closure so it can be unit-tested here (synthetic sizes) and
//!      reused in the wasm block (probe = one `dispatch_ffmpeg` call). The JS
//!      page mirrors this exact search with the same [`Q_MIN`]/[`Q_MAX`] bounds.
//!
//! PNG is intentionally excluded: it has no lossy quality knob to search, so the
//! output is always JPEG or WebP (the page states this).

/// Output format for the lossy quality search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fmt {
    Jpeg,
    Webp,
}

impl Fmt {
    /// Output file extension.
    pub fn ext(self) -> &'static str {
        match self {
            Fmt::Jpeg => "jpg",
            Fmt::Webp => "webp",
        }
    }
    /// Output MIME type (for the chat/CLI envelope + the page media element).
    pub fn mime(self) -> &'static str {
        match self {
            Fmt::Jpeg => "image/jpeg",
            Fmt::Webp => "image/webp",
        }
    }
    /// Parse the `format` param value. Accepts `jpg`/`jpeg` and `webp`.
    pub fn from_arg(s: &str) -> Result<Fmt, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Ok(Fmt::Jpeg),
            "webp" => Ok(Fmt::Webp),
            other => Err(format!(
                "unsupported format {other:?}; use \"jpg\" or \"webp\" (PNG has no quality knob to search)"
            )),
        }
    }
}

/// Lowest / highest quality the search probes. Mirrored verbatim in
/// `page/custom.js` (`Q_MIN`/`Q_MAX`) — keep the three in sync.
pub const Q_MIN: u8 = 5;
pub const Q_MAX: u8 = 95;

/// Map web-conventional quality 1-100 to ffmpeg JPEG `-q:v` range 31 (worst) – 2
/// (best). Mirrors `gizza-ai/image-compress`'s `quality_to_qv` so the two tools
/// agree on what "quality N" means for a JPEG.
fn quality_to_qv(q: u8) -> u8 {
    let q = q.clamp(1, 100) as f32;
    let qv = 31.0 - (q - 1.0) * (29.0 / 99.0);
    qv.round().clamp(2.0, 31.0) as u8
}

/// Build the ffmpeg argv (no leading `ffmpeg`) for ONE encode attempt of
/// `in_name` at `quality` (1-100, higher = better/larger), targeting `fmt`, with
/// an optional `max_width` cap (0 = keep original; shrinks only, never upscales).
/// Returns `(argv, out_name)`. Shared by the block loop and the page loop.
pub fn plan_attempt(fmt: Fmt, quality: u8, max_width: u32, in_name: &str) -> (Vec<String>, String) {
    let out_name = format!("out.{}", fmt.ext());
    let mut argv = vec!["-i".to_string(), in_name.to_string()];
    if max_width > 0 {
        // `min(W,iw)` caps the width at W but never upscales a smaller image;
        // `-2` derives an even height that keeps the aspect ratio. The comma
        // inside `min(...)` is escaped so the filtergraph parser doesn't read it
        // as a filter separator.
        argv.push("-vf".to_string());
        argv.push(format!("scale=min({max_width}\\,iw):-2"));
    }
    match fmt {
        Fmt::Jpeg => {
            argv.push("-q:v".to_string());
            argv.push(quality_to_qv(quality).to_string());
        }
        Fmt::Webp => {
            argv.push("-quality".to_string());
            argv.push(quality.clamp(1, 100).to_string());
        }
    }
    argv.push(out_name.clone());
    (argv, out_name)
}

/// Result of the quality search: the chosen quality and whether its output
/// actually fit under the target (`false` = even the lowest quality was still
/// over budget, so this is the smallest achievable output — a best effort).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SearchOutcome {
    pub quality: u8,
    pub fit: bool,
}

/// Binary-search quality `Q_MIN..=Q_MAX` for the HIGHEST quality whose encoded
/// size (from `probe`) is `<= target_bytes`. `probe(q)` returns the byte length
/// of encoding at quality `q` (and may fail with `E`). If nothing fits, returns
/// the quality that produced the SMALLEST output with `fit: false`.
///
/// Assumes size is monotonic non-decreasing in quality (true for JPEG `-q:v` and
/// WebP `-quality`). At most ~7 probes (⌈log2(91)⌉).
pub fn search_quality<F, E>(target_bytes: usize, mut probe: F) -> Result<SearchOutcome, E>
where
    F: FnMut(u8) -> Result<usize, E>,
{
    let (mut lo, mut hi) = (Q_MIN, Q_MAX);
    let mut best: Option<u8> = None; // highest quality that fits so far
    let mut smallest_q = Q_MIN;
    let mut smallest_size = usize::MAX;
    while lo <= hi {
        let mid = lo + (hi - lo) / 2;
        let size = probe(mid)?;
        if size < smallest_size {
            smallest_size = size;
            smallest_q = mid;
        }
        if size <= target_bytes {
            best = Some(mid);
            lo = mid + 1; // try to spend the budget on higher quality
        } else if mid == Q_MIN {
            break; // can't go lower; avoid u8 underflow
        } else {
            hi = mid - 1;
        }
    }
    Ok(match best {
        Some(quality) => SearchOutcome { quality, fit: true },
        None => SearchOutcome {
            quality: smallest_q,
            fit: false,
        },
    })
}

/// Validate a `target_kb` value and convert it to a byte budget. Rejects
/// non-finite and sub-1-KB targets (the smallest useful budget).
pub fn target_kb_to_bytes(target_kb: f64) -> Result<usize, String> {
    if !target_kb.is_finite() || target_kb < 1.0 {
        return Err(format!(
            "target_kb must be a number >= 1 (KB), got {target_kb}"
        ));
    }
    Ok((target_kb * 1024.0).round() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_attempt_uses_qv_and_writes_jpg() {
        let (argv, out) = plan_attempt(Fmt::Jpeg, 80, 0, "in.png");
        assert_eq!(out, "out.jpg");
        let i = argv.iter().position(|a| a == "-q:v").expect("jpeg sets -q:v");
        let qv: u8 = argv[i + 1].parse().unwrap();
        assert!((2..=31).contains(&qv), "q:v in range, got {qv}");
        assert!(
            !argv.iter().any(|a| a == "-vf"),
            "no scale filter when max_width=0"
        );
    }

    #[test]
    fn webp_attempt_uses_quality_and_writes_webp() {
        let (argv, out) = plan_attempt(Fmt::Webp, 40, 0, "in.jpg");
        assert_eq!(out, "out.webp");
        let i = argv
            .iter()
            .position(|a| a == "-quality")
            .expect("webp sets -quality");
        assert_eq!(argv[i + 1], "40");
    }

    #[test]
    fn max_width_adds_shrink_only_scale_filter() {
        let (argv, _) = plan_attempt(Fmt::Jpeg, 50, 320, "in.png");
        let i = argv
            .iter()
            .position(|a| a == "-vf")
            .expect("scale filter present");
        assert_eq!(argv[i + 1], r"scale=min(320\,iw):-2");
    }

    #[test]
    fn fmt_parses_aliases_and_rejects_png() {
        assert_eq!(Fmt::from_arg("JPG").unwrap(), Fmt::Jpeg);
        assert_eq!(Fmt::from_arg("jpeg").unwrap(), Fmt::Jpeg);
        assert_eq!(Fmt::from_arg("webp").unwrap(), Fmt::Webp);
        assert!(Fmt::from_arg("png").is_err());
    }

    #[test]
    fn target_kb_validation() {
        assert_eq!(target_kb_to_bytes(100.0).unwrap(), 102_400);
        assert!(target_kb_to_bytes(0.5).is_err());
        assert!(target_kb_to_bytes(f64::NAN).is_err());
    }

    // A synthetic encoder whose output size grows with quality — models JPEG.
    // size(q) = q * 1000 bytes.
    fn synthetic(q: u8) -> Result<usize, std::convert::Infallible> {
        Ok(q as usize * 1000)
    }

    #[test]
    fn search_finds_highest_quality_under_budget() {
        // target 60_000 → highest q with q*1000 <= 60000 is q=60.
        let out = search_quality(60_000, synthetic).unwrap();
        assert!(out.fit);
        assert_eq!(out.quality, 60);
    }

    #[test]
    fn search_returns_smallest_when_nothing_fits() {
        // target 1_000 but even Q_MIN (5) → 5000 bytes > target.
        let out = search_quality(1_000, synthetic).unwrap();
        assert!(!out.fit, "no quality fits the tiny budget");
        assert_eq!(out.quality, Q_MIN, "falls back to the smallest output");
    }

    #[test]
    fn search_caps_quality_at_q_max() {
        // Huge budget → best is Q_MAX (95), never above.
        let out = search_quality(10_000_000, synthetic).unwrap();
        assert!(out.fit);
        assert_eq!(out.quality, Q_MAX);
    }

    #[test]
    fn search_propagates_probe_errors() {
        let r: Result<SearchOutcome, &str> = search_quality(50_000, |_q| Err("boom"));
        assert_eq!(r, Err("boom"));
    }
}
