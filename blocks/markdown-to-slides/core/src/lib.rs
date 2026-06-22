//! gizza-ai/markdown-to-slides core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Converts a Markdown document into a single self-contained, navigable HTML
//! slide deck. Slides are separated by a thematic break (`---`, `***` or `___`)
//! on its own line — the standard Markdown horizontal rule, which is also the
//! de-facto slide separator used by reveal.js / Marp / remark. Each slide's
//! Markdown is rendered with `pulldown-cmark` (CommonMark + GitHub-flavored
//! extensions) and sanitized with `ammonia`, then wrapped in one HTML file that
//! embeds its own CSS and a tiny vanilla-JS navigator (arrow keys / click /
//! swipe / slide counter / progress bar). The result is a string you can save
//! as `deck.html` and open in any browser — no network, no build step.

use ammonia::Builder;
use pulldown_cmark::{html, Options, Parser};

/// A parsed deck: the rendered (sanitized) HTML body of each slide.
struct Deck {
    slides: Vec<String>,
}

/// Convert `markdown` into a complete self-contained HTML slide deck.
///
/// * `markdown` — the source document. Split into slides on lines that are a
///   thematic break (`---`, `***`, `___`, optionally longer).
/// * `theme` — `"light"` or `"dark"`; selects the embedded CSS palette.
/// * `title` — `<title>` of the generated document (blank → "Slides").
///
/// Returns the full `<!doctype html>…</html>` string, or an error if the input
/// is empty / produces no slides.
pub fn build_slides(markdown: &str, theme: &str, title: &str) -> Result<String, String> {
    if markdown.trim().is_empty() {
        return Err("input is empty".into());
    }
    let theme = normalize_theme(theme)?;
    let deck = parse_deck(markdown);
    if deck.slides.is_empty() {
        return Err("no slides produced from the input".into());
    }
    let title = {
        let t = title.trim();
        if t.is_empty() {
            "Slides".to_string()
        } else {
            t.to_string()
        }
    };
    Ok(render_document(&deck, theme, &title))
}

/// Default-theme convenience wrapper.
pub fn build_slides_default(markdown: &str) -> Result<String, String> {
    build_slides(markdown, "light", "Slides")
}

fn normalize_theme(theme: &str) -> Result<&'static str, String> {
    match theme.trim().to_ascii_lowercase().as_str() {
        "" | "light" => Ok("light"),
        "dark" => Ok("dark"),
        other => Err(format!(
            "unknown theme {other:?} (expected \"light\" or \"dark\")"
        )),
    }
}

/// Is this line a Markdown thematic break (a slide separator)?
/// A thematic break is a line of 3+ of the same `-`, `*` or `_`, optionally
/// separated by spaces, with nothing else on the line.
fn is_separator(line: &str) -> bool {
    let t = line.trim();
    if t.len() < 3 {
        return false;
    }
    for marker in ['-', '*', '_'] {
        if t.chars().all(|c| c == marker || c == ' ')
            && t.chars().filter(|&c| c == marker).count() >= 3
        {
            return true;
        }
    }
    false
}

/// Split the document into slide chunks on separator lines, render + sanitize
/// each chunk. Empty chunks (e.g. leading/trailing/consecutive separators) are
/// dropped so a stray `---` never yields a blank slide.
fn parse_deck(markdown: &str) -> Deck {
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for line in markdown.lines() {
        if is_separator(line) {
            chunks.push(std::mem::take(&mut current));
            continue;
        }
        current.push_str(line);
        current.push('\n');
    }
    chunks.push(current);

    let slides: Vec<String> = chunks
        .into_iter()
        .filter(|c| !c.trim().is_empty())
        .map(|c| render_markdown(&c))
        .collect();
    Deck { slides }
}

/// Render one slide's Markdown to sanitized HTML.
fn render_markdown(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_SMART_PUNCTUATION);

    let parser = Parser::new_ext(md, options);
    let mut unsafe_html = String::new();
    html::push_html(&mut unsafe_html, parser);
    Builder::default().clean(&unsafe_html).to_string()
}

/// HTML-escape for the document chrome (title text + attribute).
fn escape_html(s: &str) -> String {
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

fn render_document(deck: &Deck, theme: &'static str, title: &str) -> String {
    // --bg --fg --muted --accent --code-bg
    let (bg, fg, muted, accent, code_bg) = if theme == "dark" {
        ("#1e1e2e", "#e6e6f0", "#9a9ab0", "#89b4fa", "#2b2b3c")
    } else {
        ("#ffffff", "#1a1a1a", "#6a6a6a", "#2563eb", "#f3f4f6")
    };

    let mut sections = String::new();
    for (i, slide) in deck.slides.iter().enumerate() {
        sections.push_str(&format!(
            "<section class=\"slide\"{}>\n{}\n</section>\n",
            if i == 0 { " data-active" } else { "" },
            slide
        ));
    }
    let total = deck.slides.len();
    let esc_title = escape_html(title);

    format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="generator" content="gizza.ai markdown-to-slides">
<title>{title}</title>
<style>
:root {{
  --bg: {bg}; --fg: {fg}; --muted: {muted}; --accent: {accent}; --code-bg: {code_bg};
}}
* {{ box-sizing: border-box; }}
html, body {{ margin: 0; height: 100%; }}
body {{
  background: var(--bg); color: var(--fg);
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
  -webkit-font-smoothing: antialiased;
}}
#deck {{ position: relative; width: 100%; height: 100vh; overflow: hidden; }}
.slide {{
  position: absolute; inset: 0; display: none;
  flex-direction: column; justify-content: center;
  padding: 6vh 10vw; overflow: auto;
}}
.slide[data-active] {{ display: flex; }}
.slide > :first-child {{ margin-top: 0; }}
.slide h1 {{ font-size: clamp(2rem, 6vw, 4rem); line-height: 1.1; margin: 0 0 .4em; }}
.slide h2 {{ font-size: clamp(1.5rem, 4vw, 2.6rem); margin: 0 0 .4em; }}
.slide h3 {{ font-size: clamp(1.2rem, 3vw, 1.8rem); }}
.slide p, .slide li {{ font-size: clamp(1rem, 2.2vw, 1.5rem); line-height: 1.5; }}
.slide a {{ color: var(--accent); }}
.slide code {{
  background: var(--code-bg); padding: .12em .4em; border-radius: 4px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; font-size: .9em;
}}
.slide pre {{
  background: var(--code-bg); padding: 1em 1.2em; border-radius: 8px; overflow: auto;
}}
.slide pre code {{ background: none; padding: 0; }}
.slide blockquote {{
  border-left: 4px solid var(--accent); margin: 0 0 1em; padding: .2em 1em; color: var(--muted);
}}
.slide table {{ border-collapse: collapse; }}
.slide th, .slide td {{ border: 1px solid var(--muted); padding: .4em .8em; }}
.slide img {{ max-width: 100%; height: auto; }}
#counter {{
  position: fixed; bottom: 1.2vh; right: 1.5vw; color: var(--muted);
  font-size: .9rem; font-variant-numeric: tabular-nums; user-select: none;
}}
#progress {{
  position: fixed; top: 0; left: 0; height: 4px; background: var(--accent);
  transition: width .15s ease; width: 0;
}}
.nav-btn {{
  position: fixed; top: 0; height: 100%; width: 12vw; max-width: 120px;
  border: none; background: transparent; cursor: pointer; opacity: 0;
}}
#prev {{ left: 0; }} #next {{ right: 0; }}
@media print {{
  .slide {{ display: flex !important; position: relative; height: 100vh; page-break-after: always; }}
  #counter, #progress, .nav-btn {{ display: none; }}
}}
</style>
</head>
<body>
<div id="progress"></div>
<div id="deck">
{sections}</div>
<div id="counter"><span id="cur">1</span> / {total}</div>
<button class="nav-btn" id="prev" aria-label="Previous slide"></button>
<button class="nav-btn" id="next" aria-label="Next slide"></button>
<script>
(function () {{
  var slides = Array.prototype.slice.call(document.querySelectorAll('.slide'));
  var total = slides.length;
  var i = 0;
  var cur = document.getElementById('cur');
  var progress = document.getElementById('progress');
  function show(n) {{
    i = Math.max(0, Math.min(total - 1, n));
    slides.forEach(function (s, idx) {{
      if (idx === i) {{ s.setAttribute('data-active', ''); }}
      else {{ s.removeAttribute('data-active'); }}
    }});
    cur.textContent = (i + 1);
    progress.style.width = (total > 1 ? (i / (total - 1)) * 100 : 100) + '%';
    if (location.hash !== '#' + (i + 1)) {{ history.replaceState(null, '', '#' + (i + 1)); }}
  }}
  function next() {{ show(i + 1); }}
  function prev() {{ show(i - 1); }}
  document.addEventListener('keydown', function (e) {{
    if (e.key === 'ArrowRight' || e.key === 'PageDown' || e.key === ' ') {{ next(); e.preventDefault(); }}
    else if (e.key === 'ArrowLeft' || e.key === 'PageUp') {{ prev(); e.preventDefault(); }}
    else if (e.key === 'Home') {{ show(0); }}
    else if (e.key === 'End') {{ show(total - 1); }}
  }});
  document.getElementById('next').addEventListener('click', next);
  document.getElementById('prev').addEventListener('click', prev);
  var sx = null;
  document.addEventListener('touchstart', function (e) {{ sx = e.changedTouches[0].clientX; }}, {{ passive: true }});
  document.addEventListener('touchend', function (e) {{
    if (sx === null) {{ return; }}
    var dx = e.changedTouches[0].clientX - sx; sx = null;
    if (dx < -40) {{ next(); }} else if (dx > 40) {{ prev(); }}
  }}, {{ passive: true }});
  var fromHash = parseInt((location.hash || '').replace('#', ''), 10);
  show(isNaN(fromHash) ? 0 : fromHash - 1);
}})();
</script>
</body>
</html>
"##,
        title = esc_title,
        bg = bg,
        fg = fg,
        muted = muted,
        accent = accent,
        code_bg = code_bg,
        sections = sections,
        total = total,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_on_separator_and_counts_slides() {
        let md = "# One\n\nintro\n\n---\n\n# Two\n\n---\n\n# Three";
        let html = build_slides(md, "light", "Deck").unwrap();
        assert_eq!(html.matches("<section class=\"slide\"").count(), 3);
        assert!(html.contains("/ 3</div>"));
        assert!(html.contains("<title>Deck</title>"));
        assert!(html.contains("<h1>One</h1>"));
        assert!(html.contains("<h1>Three</h1>"));
        assert!(html.contains("<section class=\"slide\" data-active>"));
    }

    #[test]
    fn single_slide_when_no_separators() {
        let html = build_slides("# Just one\n\nbody text", "light", "").unwrap();
        assert_eq!(html.matches("<section class=\"slide\"").count(), 1);
        assert!(html.contains("/ 1</div>"));
        assert!(html.contains("<title>Slides</title>"));
    }

    #[test]
    fn dark_theme_palette_applied() {
        let html = build_slides("# Hi", "dark", "T").unwrap();
        assert!(html.contains("--bg: #1e1e2e"));
        let light = build_slides("# Hi", "light", "T").unwrap();
        assert!(light.contains("--bg: #ffffff"));
    }

    #[test]
    fn sanitizes_embedded_script() {
        let html = build_slides("# Safe\n\n<script>alert(1)</script>", "light", "").unwrap();
        assert!(!html.contains("alert(1)"));
    }

    #[test]
    fn dropped_blank_slides_from_consecutive_separators() {
        let md = "---\n\n# A\n\n---\n\n---\n\n# B\n\n---";
        let html = build_slides(md, "light", "").unwrap();
        assert_eq!(html.matches("<section class=\"slide\"").count(), 2);
    }

    #[test]
    fn star_and_underscore_separators() {
        let html = build_slides("# A\n\n***\n\n# B\n\n___\n\n# C", "light", "").unwrap();
        assert_eq!(html.matches("<section class=\"slide\"").count(), 3);
    }

    #[test]
    fn empty_input_errors() {
        assert!(build_slides("   \n  ", "light", "").is_err());
    }

    #[test]
    fn unknown_theme_errors() {
        assert!(build_slides("# A", "neon", "").is_err());
    }

    #[test]
    fn default_wrapper_works() {
        let html = build_slides_default("# Hello").unwrap();
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("--bg: #ffffff"));
    }

    #[test]
    fn gfm_table_renders() {
        let md = "# T\n\n| a | b |\n|---|---|\n| 1 | 2 |";
        let html = build_slides(md, "light", "").unwrap();
        assert!(html.contains("<table>"));
        assert_eq!(html.matches("<section class=\"slide\"").count(), 1);
    }

    #[test]
    fn title_is_escaped() {
        let html = build_slides("# A", "light", "Q&A <talk>").unwrap();
        assert!(html.contains("<title>Q&amp;A &lt;talk&gt;</title>"));
    }
}
