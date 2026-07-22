//! before-after-slider core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Given two image sources (URL or `data:` URI) plus a few options, emit a
//! self-contained interactive before/after comparison slider: a single HTML
//! blob with inline CSS + JS, no external libraries, no build step. The user
//! drags (or hovers) a divider to wipe between the two overlaid images. The
//! generated widget supports pointer + touch drag, keyboard arrows, a start
//! position, optional side labels, horizontal or vertical wipe, and works for
//! several sliders on one page (the inline script initializes every widget).

/// Wipe direction of the reveal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Vertical divider that moves left↔right; the "before" image is revealed
    /// on the left, the "after" image on the right.
    Horizontal,
    /// Horizontal divider that moves up↕down; the "before" image is revealed on
    /// top, the "after" image on the bottom.
    Vertical,
}

pub fn parse_orientation(s: &str) -> Result<Orientation, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "horizontal" | "h" | "vertical-divider" => Ok(Orientation::Horizontal),
        "vertical" | "v" | "horizontal-divider" => Ok(Orientation::Vertical),
        other => Err(format!(
            "orientation {other:?} not supported (expected 'horizontal' or 'vertical')"
        )),
    }
}

/// Whole-document HTML page vs. an embeddable widget snippet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// A complete `<!DOCTYPE html>` page you can save as `.html` and open.
    Document,
    /// Just the `<style>` + `<div>` + `<script>` you paste into an existing page.
    Embed,
}

pub fn parse_output(s: &str) -> Result<OutputMode, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "document" | "page" | "html" => Ok(OutputMode::Document),
        "embed" | "snippet" | "widget" => Ok(OutputMode::Embed),
        other => Err(format!(
            "output {other:?} not supported (expected 'document' or 'embed')"
        )),
    }
}

pub struct Options {
    /// `before` image source: an `http(s)://` URL, a relative path, or a
    /// `data:image/...;base64,...` URI. Rejected if empty or a `javascript:` URI.
    pub before: String,
    /// `after` image source (same accepted forms as `before`).
    pub after: String,
    /// Caption badge over the "before" side. Empty = no badge.
    pub before_label: String,
    /// Caption badge over the "after" side. Empty = no badge.
    pub after_label: String,
    pub orientation: Orientation,
    /// Initial divider position as a percent, clamped to 0–100.
    pub start: f64,
    /// Max widget width in CSS px; 0 = fluid (fills its container, responsive).
    pub width: u32,
    /// Move the divider on hover instead of requiring a drag/click.
    pub move_on_hover: bool,
    /// Divider + handle color: any CSS color (`#rgb`, `#rrggbb`, named).
    pub handle_color: String,
    pub output: OutputMode,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            before: String::new(),
            after: String::new(),
            before_label: "Before".into(),
            after_label: "After".into(),
            orientation: Orientation::Horizontal,
            start: 50.0,
            width: 0,
            move_on_hover: false,
            handle_color: "#ffffff".into(),
            output: OutputMode::Document,
        }
    }
}

/// HTML-escape for use inside an attribute value (double-quoted) or text node.
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(c),
        }
    }
    out
}

/// Reject obviously unsafe/empty image sources. We only allow sources that can
/// actually resolve to an image so the generated HTML can't smuggle a
/// `javascript:` handler.
fn validate_src(name: &str, raw: &str) -> Result<String, String> {
    let src = raw.trim();
    if src.is_empty() {
        return Err(format!("{name} image source is required (expected an http(s):// URL, a relative path, or a data:image/... URI)"));
    }
    let lower = src.to_ascii_lowercase();
    // Block script/other active schemes that could execute if the src is reused
    // outside an <img>. data: is allowed only for images.
    for bad in ["javascript:", "vbscript:", "file:"] {
        if lower.starts_with(bad) {
            return Err(format!(
                "{name} image source uses a disallowed scheme ({bad}); pass an http(s):// URL, a relative path, or a data:image/... URI"
            ));
        }
    }
    if lower.starts_with("data:") && !lower.starts_with("data:image/") {
        return Err(format!(
            "{name} data: URI must be an image (data:image/...); got {src:?}"
        ));
    }
    Ok(src.to_string())
}

/// Build the complete widget (styles + markup + script) or a full HTML page.
pub fn render(opts: &Options) -> Result<String, String> {
    let before = validate_src("before", &opts.before)?;
    let after = validate_src("after", &opts.after)?;
    let start = opts.start.clamp(0.0, 100.0);
    let horizontal = opts.orientation == Orientation::Horizontal;

    let color = {
        let c = opts.handle_color.trim();
        if c.is_empty() { "#ffffff".to_string() } else { c.to_string() }
    };

    // A container-width rule: fluid by default, capped when width > 0.
    let width_rule = if opts.width > 0 {
        format!("max-width:{}px;", opts.width)
    } else {
        String::new()
    };

    // clip-path that reveals the "before" layer up to `--pos`%. Horizontal keeps
    // the left slice; vertical keeps the top slice.
    let clip = if horizontal {
        "inset(0 calc(100% - var(--pos)) 0 0)"
    } else {
        "inset(0 0 calc(100% - var(--pos)) 0)"
    };
    // Divider geometry differs per axis.
    let divider_css = if horizontal {
        "left:var(--pos);top:0;width:3px;height:100%;transform:translateX(-50%);cursor:ew-resize;"
    } else {
        "top:var(--pos);left:0;height:3px;width:100%;transform:translateY(-50%);cursor:ns-resize;"
    };
    let handle_rotate = if horizontal { "" } else { "transform:translate(-50%,-50%) rotate(90deg);" };

    let before_badge = if opts.before_label.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<span class="bas-tag bas-tag-before">{}</span>"#,
            esc(opts.before_label.trim())
        )
    };
    let after_badge = if opts.after_label.trim().is_empty() {
        String::new()
    } else {
        format!(
            r#"<span class="bas-tag bas-tag-after">{}</span>"#,
            esc(opts.after_label.trim())
        )
    };

    let hover_attr = if opts.move_on_hover { r#" data-hover="1""# } else { "" };
    let axis = if horizontal { "x" } else { "y" };

    // Scoped CSS — every selector lives under .bas-container so an embedded
    // snippet never leaks styles into the host page.
    let css = format!(
        r#"<style>
.bas-container{{position:relative;width:100%;{width_rule}margin:0 auto;line-height:0;overflow:hidden;border-radius:8px;user-select:none;touch-action:none;-webkit-user-select:none}}
.bas-container:focus-visible{{outline:3px solid {color};outline-offset:2px}}
.bas-container img{{display:block;width:100%;height:auto;pointer-events:none}}
.bas-before{{position:absolute;inset:0;width:100%;height:100%;object-fit:cover;clip-path:{clip}}}
.bas-divider{{position:absolute;{divider_css}background:{color};box-shadow:0 0 3px rgba(0,0,0,.45)}}
.bas-handle{{position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);width:40px;height:40px;border-radius:50%;background:{color};box-shadow:0 0 4px rgba(0,0,0,.45);display:flex;align-items:center;justify-content:center;{handle_rotate}}}
.bas-handle::before{{content:"";position:absolute;left:9px;border:6px solid transparent;border-right-color:rgba(0,0,0,.55)}}
.bas-handle::after{{content:"";position:absolute;right:9px;border:6px solid transparent;border-left-color:rgba(0,0,0,.55)}}
.bas-tag{{position:absolute;top:10px;padding:3px 9px;font:600 13px/1.4 system-ui,-apple-system,Segoe UI,Roboto,sans-serif;color:#fff;background:rgba(0,0,0,.55);border-radius:4px;pointer-events:none}}
.bas-tag-before{{left:10px}}
.bas-tag-after{{right:10px}}
</style>"#
    );

    let before_src = esc(&before);
    let after_src = esc(&after);

    let widget = format!(
        r#"<div class="bas-container" role="slider" tabindex="0" aria-label="Before and after image comparison slider" aria-valuemin="0" aria-valuemax="100" aria-valuenow="{start_round}" data-axis="{axis}"{hover_attr} style="--pos:{start}%">
<img class="bas-after" src="{after_src}" alt="After">
<img class="bas-before" src="{before_src}" alt="Before">
{before_badge}{after_badge}<div class="bas-divider"><div class="bas-handle"></div></div>
</div>"#,
        start_round = start.round() as i64,
    );

    let script = format!(
        r#"<script>
(function(){{
  function initAll(){{
    document.querySelectorAll('.bas-container:not([data-bas-ready])').forEach(function(el){{
      el.setAttribute('data-bas-ready','1');
      var axis = el.getAttribute('data-axis') || 'x';
      var hover = el.getAttribute('data-hover') === '1';
      var dragging = false;
      function clamp(n){{ return Math.max(0, Math.min(100, n)); }}
      function setPos(p){{ p = clamp(p); el.style.setProperty('--pos', p + '%'); el.setAttribute('aria-valuenow', Math.round(p)); }}
      function fromEvent(e){{
        var r = el.getBoundingClientRect();
        var p = axis === 'y' ? (e.clientY - r.top) / r.height : (e.clientX - r.left) / r.width;
        setPos(p * 100);
      }}
      el.addEventListener('pointerdown', function(e){{ dragging = true; el.setPointerCapture && el.setPointerCapture(e.pointerId); fromEvent(e); e.preventDefault(); }});
      el.addEventListener('pointermove', function(e){{ if (dragging || hover) fromEvent(e); }});
      window.addEventListener('pointerup', function(){{ dragging = false; }});
      el.addEventListener('keydown', function(e){{
        var cur = parseFloat(el.style.getPropertyValue('--pos')) || 0;
        var step = e.shiftKey ? 10 : 2;
        if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') {{ setPos(cur - step); e.preventDefault(); }}
        else if (e.key === 'ArrowRight' || e.key === 'ArrowDown') {{ setPos(cur + step); e.preventDefault(); }}
        else if (e.key === 'Home') {{ setPos(0); e.preventDefault(); }}
        else if (e.key === 'End') {{ setPos(100); e.preventDefault(); }}
      }});
    }});
  }}
  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', initAll); else initAll();
}})();
</script>"#
    );

    let body = format!("{css}\n{widget}\n{script}");

    match opts.output {
        OutputMode::Embed => Ok(body),
        OutputMode::Document => Ok(format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>Before / After Slider</title>
<style>body{{margin:0;min-height:100vh;display:flex;align-items:center;justify-content:center;background:#111;padding:24px;box-sizing:border-box}}</style>
</head>
<body>
{body}
</body>
</html>
"#
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options {
            before: "https://example.com/before.jpg".into(),
            after: "https://example.com/after.jpg".into(),
            ..Default::default()
        }
    }

    #[test]
    fn happy_path_document() {
        let html = render(&opts()).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("bas-container"));
        assert!(html.contains("src=\"https://example.com/before.jpg\""));
        assert!(html.contains("src=\"https://example.com/after.jpg\""));
        assert!(html.contains("Before"));
        assert!(html.contains("After"));
        // default start = 50%
        assert!(html.contains("--pos:50%"));
        assert!(html.contains("data-axis=\"x\""));
    }

    #[test]
    fn embed_mode_is_snippet_not_page() {
        let mut o = opts();
        o.output = OutputMode::Embed;
        let html = render(&o).unwrap();
        assert!(!html.contains("<!DOCTYPE"));
        assert!(!html.contains("<html"));
        assert!(html.contains("bas-container"));
        assert!(html.contains("<script>"));
    }

    #[test]
    fn vertical_orientation_clips_bottom_and_sets_axis() {
        let mut o = opts();
        o.orientation = Orientation::Vertical;
        let html = render(&o).unwrap();
        assert!(html.contains("data-axis=\"y\""));
        assert!(html.contains("inset(0 0 calc(100% - var(--pos)) 0)"));
    }

    #[test]
    fn horizontal_clips_right() {
        let html = render(&opts()).unwrap();
        assert!(html.contains("inset(0 calc(100% - var(--pos)) 0 0)"));
    }

    #[test]
    fn start_is_clamped() {
        let mut o = opts();
        o.start = 250.0;
        assert!(render(&o).unwrap().contains("--pos:100%"));
        o.start = -30.0;
        assert!(render(&o).unwrap().contains("--pos:0%"));
    }

    #[test]
    fn width_caps_container() {
        let mut o = opts();
        o.width = 640;
        assert!(render(&o).unwrap().contains("max-width:640px;"));
        o.width = 0;
        assert!(!render(&o).unwrap().contains("max-width:"));
    }

    #[test]
    fn empty_labels_omit_badges() {
        let mut o = opts();
        o.before_label = "".into();
        o.after_label = "  ".into();
        let html = render(&o).unwrap();
        assert!(!html.contains("<span class=\"bas-tag bas-tag-before\""));
        assert!(!html.contains("<span class=\"bas-tag bas-tag-after\""));
    }

    #[test]
    fn labels_are_escaped() {
        let mut o = opts();
        o.before_label = "<b>2019</b>".into();
        let html = render(&o).unwrap();
        assert!(html.contains("&lt;b&gt;2019&lt;/b&gt;"));
        assert!(!html.contains("<b>2019</b>"));
    }

    #[test]
    fn src_quotes_are_escaped_no_breakout() {
        let mut o = opts();
        o.before = "https://x/a.jpg\"><script>alert(1)</script>".into();
        let html = render(&o).unwrap();
        assert!(!html.contains("<script>alert(1)"));
        assert!(html.contains("&quot;&gt;&lt;script&gt;"));
    }

    #[test]
    fn rejects_javascript_scheme() {
        let mut o = opts();
        o.before = "javascript:alert(1)".into();
        assert!(render(&o).is_err());
    }

    #[test]
    fn rejects_non_image_data_uri() {
        let mut o = opts();
        o.after = "data:text/html,<b>hi</b>".into();
        assert!(render(&o).is_err());
    }

    #[test]
    fn accepts_image_data_uri() {
        let mut o = opts();
        o.before = "data:image/png;base64,iVBORw0KGgo=".into();
        assert!(render(&o).is_ok());
    }

    #[test]
    fn empty_source_errors() {
        let mut o = opts();
        o.before = "   ".into();
        assert!(render(&o).is_err());
    }

    #[test]
    fn move_on_hover_sets_attr() {
        let mut o = opts();
        o.move_on_hover = true;
        assert!(render(&o).unwrap().contains("data-hover=\"1\""));
        o.move_on_hover = false;
        assert!(!render(&o).unwrap().contains(" data-hover=\"1\""));
    }

    #[test]
    fn handle_color_applied() {
        let mut o = opts();
        o.handle_color = "#ff3366".into();
        assert!(render(&o).unwrap().contains("#ff3366"));
    }

    #[test]
    fn orientation_parse() {
        assert_eq!(parse_orientation("horizontal").unwrap(), Orientation::Horizontal);
        assert_eq!(parse_orientation("V").unwrap(), Orientation::Vertical);
        assert_eq!(parse_orientation("").unwrap(), Orientation::Horizontal);
        assert!(parse_orientation("diagonal").is_err());
    }

    #[test]
    fn output_parse() {
        assert_eq!(parse_output("embed").unwrap(), OutputMode::Embed);
        assert_eq!(parse_output("").unwrap(), OutputMode::Document);
        assert!(parse_output("pdf").is_err());
    }
}
