//! dot-to-svg core — render Graphviz DOT diagram source into a standalone SVG.
//! Pure-Rust layout (the `layout-rs` engine), so it runs on every backend
//! (including the chat Service Worker) with no Graphviz install. No
//! wafer/wasm-bindgen deps here.

use layout::backends::svg::SVGWriter;
use layout::gv::{DotParser, GraphBuilder};

/// Options for rendering a DOT graph.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    /// Recolor the SVG for a dark background: light text/edges/node strokes on
    /// a transparent canvas. Off by default (light theme).
    pub dark_mode: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { dark_mode: false }
    }
}

/// Render Graphviz DOT source into an SVG string.
///
/// Accepts `digraph`/`graph` source. Returns the SVG markup, or an error
/// describing the parse failure.
pub fn render(dot: &str, opts: &Options) -> Result<String, String> {
    let dot = dot.trim();
    if dot.is_empty() {
        return Err("DOT source is empty.".into());
    }

    let mut parser = DotParser::new(dot);
    let graph = parser
        .process()
        .map_err(|e| format!("Could not parse DOT source: {e}"))?;

    let mut builder = GraphBuilder::new();
    builder.visit_graph(&graph);
    let mut visual = builder.get();

    let mut writer = SVGWriter::new();
    // do_it(debug, disable_opt, disable_layout, writer)
    visual.do_it(false, false, false, &mut writer);
    let svg = writer.finalize();

    if !svg.contains("<svg") {
        return Err("Renderer produced no SVG output.".into());
    }

    let svg = if opts.dark_mode { to_dark(&svg) } else { svg };
    Ok(svg)
}

/// Recolor a light-theme SVG for a dark background. The `layout-rs` backend
/// emits black strokes (`#000000ff`) and white node fills (`#ffffffff`) as
/// 8-digit RGBA hex, and renders text via a `.aNN { font-size … }` CSS class
/// with no explicit fill (so it defaults to black). Swap the dark strokes for a
/// light grey, the white node fills for a dark grey, and inject a light text
/// `fill` into the font style block so labels read on a dark page.
const DARK_STROKE: &str = "#e6e6e6";
const DARK_NODE_FILL: &str = "#1e1e1e";

fn to_dark(svg: &str) -> String {
    let svg = svg
        // stroke colors (node outlines + edges)
        .replace("stroke=\"#000000ff\"", &format!("stroke=\"{DARK_STROKE}\""))
        .replace("stroke=\"#000000\"", &format!("stroke=\"{DARK_STROKE}\""))
        .replace("stroke=\"black\"", &format!("stroke=\"{DARK_STROKE}\""))
        // node fills
        .replace("fill=\"#ffffffff\"", &format!("fill=\"{DARK_NODE_FILL}\""))
        .replace("fill=\"#ffffff\"", &format!("fill=\"{DARK_NODE_FILL}\""))
        .replace("fill=\"white\"", &format!("fill=\"{DARK_NODE_FILL}\""));
    // text uses a `.aNN { font-... }` class with no fill (defaults to black) —
    // append a fill rule to the generated font-family style rules so labels are
    // light. The backend emits rules like `.a14 { font-size: 14px; font-family: … }`.
    svg.replace(
        "font-family: Times, serif; }",
        &format!("font-family: Times, serif; fill: {DARK_STROKE}; }}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_digraph_to_svg() {
        let svg = render("digraph { a -> b; b -> c; a -> c; }", &Options::default()).unwrap();
        assert!(svg.contains("<svg"), "missing <svg>: {svg}");
        assert!(svg.contains("</svg>"), "missing </svg>");
    }

    #[test]
    fn renders_undirected_graph() {
        let svg = render("graph { x -- y; y -- z; }", &Options::default()).unwrap();
        assert!(svg.contains("<svg"));
    }

    #[test]
    fn dark_mode_recolors() {
        let light = render("digraph { a -> b; }", &Options::default()).unwrap();
        let dark = render("digraph { a -> b; }", &Options { dark_mode: true }).unwrap();
        assert_ne!(light, dark, "dark mode should change the output");
        assert!(dark.contains("#e6e6e6"), "dark mode should introduce light strokes/text");
    }

    #[test]
    fn empty_input_errors() {
        let err = render("   ", &Options::default()).unwrap_err();
        assert!(err.to_lowercase().contains("empty"));
    }

    #[test]
    fn invalid_dot_errors() {
        let err = render("this is not dot @@@ {{{", &Options::default()).unwrap_err();
        assert!(err.to_lowercase().contains("parse"), "got: {err}");
    }
}
