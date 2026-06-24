//! Renders the option-C tool page: top nav + hero tool + SEO content + footer,
//! with SEO `<head>` tags and JSON-LD.

use crate::control::{fmt_num, Control, ParamSchema};
use crate::markdown::example_deeplink;
use crate::meta::ToolMeta;
use gizza_chrome::{header as chrome_header, footer as chrome_footer, Active};
use maud::{html, PreEscaped, DOCTYPE};

/// Render the full HTML document for a tool page. `content_html` is the
/// markdown-rendered SEO section.
pub fn render_page(meta: &ToolMeta, content_html: &str, schema: &ParamSchema) -> String {
    let canonical = format!("https://gizza.ai/tools/{}/", meta.slug);
    // Both JSON blobs below are emitted raw inside <script> via PreEscaped, so a
    // literal "</script>" in any value would break out of the element. serde_json
    // does not escape '/', so we neutralize the closing-tag sequence. Values are
    // repo-authored today; this is defense-in-depth against a future meta.toml.
    let client_cfg = meta.client_config().to_string().replace("</", "<\\/");
    let json_ld = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebApplication",
        "name": meta.h1,
        "description": meta.description,
        "url": canonical,
        "applicationCategory": "UtilitiesApplication",
        "operatingSystem": "Any",
        "offers": { "@type": "Offer", "price": "0", "priceCurrency": "USD" },
        "publisher": { "@type": "Organization", "name": "gizza.ai", "url": "https://gizza.ai" }
    })
    .to_string()
    .replace("</", "<\\/");

    // How to run THIS tool headlessly via the gizza CLI — derived from the
    // tool's own inputs (first field's placeholder, or a url= for file tools).
    let cli_example = match meta.inputs.first() {
        Some(inp) if inp.source == "file" => {
            format!("gizza tool {} 'url=https://example.com/input'", meta.slug)
        }
        Some(inp) if !inp.placeholder.is_empty() => {
            format!("gizza tool {} '{}'", meta.slug, inp.placeholder)
        }
        _ => format!("gizza tool {}", meta.slug),
    };

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (meta.title) }
                meta name="description" content=(meta.description);
                link rel="canonical" href=(canonical);
                link rel="alternate" type="text/markdown" href="index.md";
                meta property="og:type" content="website";
                meta property="og:title" content=(meta.title);
                meta property="og:description" content=(meta.description);
                meta property="og:url" content=(canonical);
                meta property="og:image" content="https://gizza.ai/gis.png";
                meta name="twitter:card" content="summary";
                meta name="twitter:title" content=(meta.title);
                meta name="twitter:description" content=(meta.description);
                link rel="stylesheet" href="https://site-kit.suppers.ai/dist/design-system.css";
                link rel="stylesheet" href="./tool.css";
                link rel="stylesheet" href="./header.css";
                link rel="icon" href="https://gizza.ai/favicon-32.png" sizes="32x32";
                style { (PreEscaped(TOOL_CLI_CSS)) }
                script type="application/ld+json" { (PreEscaped(json_ld)) }
                script type="module" src="./header.js" {}
            }
            body {
                ({
                    let brand = html! {
                        a.tool-brand href="https://gizza.ai" {
                            img src="/logo.webp" alt="gizza.ai logo";
                            span { "gizza.ai" }
                        }
                    };
                    chrome_header(brand, Active::Tool)
                })
                main class="tool-main" {
                    section class="tool-hero" {
                        h1 { (meta.h1) }
                        p class="tool-hero-sub" { (meta.hero_subtitle) }
                        div class="tool-widget" {
                            @for input in &meta.inputs {
                                @if input.source == "field" {
                                    @let id = format!("in-{}", input.name);
                                    label class="tool-field-label" for=(id) { (input.label) }
                                    @match schema.control_for(&input.name, input.multiline) {
                                        Control::Select { options, default } => {
                                            select id=(id) class="tool-input tool-select" {
                                                @for opt in &options {
                                                    option value=(opt) selected[Some(opt) == default.as_ref()] { (opt) }
                                                }
                                            }
                                        }
                                        Control::Checkbox { default } => {
                                            input id=(id) class="tool-checkbox" type="checkbox" checked[default];
                                        }
                                        Control::Number { min, max, default } => {
                                            @let min_s = min.map(fmt_num);
                                            @let max_s = max.map(fmt_num);
                                            @let val_s = default.map(fmt_num);
                                            input id=(id) class="tool-input" type="number"
                                                  placeholder=(input.placeholder)
                                                  min=[min_s.as_deref()] max=[max_s.as_deref()] value=[val_s.as_deref()]
                                                  autocomplete="off" autocapitalize="off" spellcheck="false";
                                        }
                                        Control::Textarea => {
                                            textarea id=(id) class="tool-input" rows="4"
                                                  placeholder=(input.placeholder)
                                                  autocomplete="off" autocapitalize="off" spellcheck="false" {}
                                        }
                                        Control::Text => {
                                            input id=(id) class="tool-input"
                                                  type="text" placeholder=(input.placeholder)
                                                  autocomplete="off" autocapitalize="off" spellcheck="false";
                                        }
                                    }
                                } @else if input.source == "file" {
                                    label class="tool-field-label" for=(format!("in-{}", input.name)) { (input.label) }
                                    input id=(format!("in-{}", input.name)) class="tool-file"
                                          type="file" accept=(input.accept);
                                }
                            }
                            div class="tool-output-label" { (meta.output_label) }
                            @if meta.format == "image" || meta.format == "video" || meta.format == "audio" {
                                @if meta.format == "image" {
                                    img id="tool-output-media" class="tool-output-media" alt="" hidden;
                                } @else if meta.format == "video" {
                                    video id="tool-output-media" class="tool-output-media" controls hidden {}
                                } @else {
                                    audio id="tool-output-media" class="tool-output-media" controls hidden {}
                                }
                                a id="tool-output-download" class="tool-output-download" download hidden { "Download" }
                                output id="tool-output" class="tool-output" { "" }
                            } @else {
                                output id="tool-output" class="tool-output" { "" }
                            }
                        }
                    }
                    section class="tool-content" {
                        (PreEscaped(content_html))
                    }
                    section class="tool-dev-group" {
                        h2 { "Developer & Automation Access" }
                        div class="tool-dev-grid" {
                            div class="tool-cli-card" {
                                h3 { "Run it from the terminal" }
                                p { "Same engine as this page, headless — via the gizza CLI:" }
                                pre class="tool-cli-code" { code { (cli_example) } }
                                p class="tool-cli-note" {
                                    "New to the CLI? "
                                    a href="https://github.com/gizza-ai/gizza-ai/blob/main/cli/README.md"
                                        target="_blank" rel="noopener noreferrer" { "Get gizza →" }
                                }
                            }
                            @if meta.inputs.iter().any(|i| i.source == "field" || i.source == "file") {
                                div class="tool-cli-card" {
                                    h3 { "Open it by URL" }
                                    p { "Pre-fill and auto-run this tool with query parameters — the names match the API/CLI:" }
                                    pre class="tool-cli-code" { code { (example_deeplink(meta, schema)) } }
                                }
                            }
                        }
                    }
                }
                (chrome_footer())
                script { (PreEscaped(format!("window.GIZZA_TOOL = {client_cfg};"))) }
                script type="module" src="./tool.js" {}
            }
        }
    };
    markup.into_string()
}

/// Inline styles for the per-tool "Run it from the terminal" CLI block.
const TOOL_CLI_CSS: &str = r#"
.tool-dev-group { max-width: 720px; margin: 48px auto 64px; }
.tool-dev-group h2 { font-size: 1.3rem; margin: 0 0 16px; color: var(--tool-ink, #0f172a); border-bottom: 1px solid #e2e8f0; padding-bottom: 8px; }
.tool-dev-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 20px; }
.tool-cli-card { background: #fafafa; border: 1px solid #e5e7eb; border-radius: 12px; padding: 18px; display: flex; flex-direction: column; }
.tool-cli-card h3 { font-size: 1.05rem; margin: 0 0 8px; color: var(--tool-ink, #0f172a); }
.tool-cli-card > p { color: var(--tool-muted, #6b7280); font-size: 0.9rem; margin: 0 0 12px; }
.tool-cli-code { background: #0f172a; color: #e2e8f0; padding: 14px 16px; border-radius: 10px; overflow-x: auto; font-size: .9rem; line-height: 1.4; margin: 0; }
.tool-cli-code code { color: inherit; background: none; padding: 0; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
.tool-cli-note { font-size: .85rem; margin: 12px 0 0; color: var(--tool-muted, #6b7280); margin-top: auto; padding-top: 10px; }
"#;

/// Inline styles for the `/tools/` landing grid (uses the `--tool-*` tokens
/// from `tool.css`, with literal fallbacks matching them).
const TOOLS_INDEX_CSS: &str = r#"
.tools-index { max-width: 1040px; margin: 0 auto; padding: 32px 20px 72px; }
.tools-index__hero { text-align: center; margin: 24px auto 36px; max-width: 600px; }
.tools-index__hero h1 { font-size: 2rem; margin: 0 0 10px; color: var(--tool-ink, #0f172a); }
.tools-index__sub { color: var(--tool-muted, #6b7280); margin: 0; line-height: 1.55; }
.tools-grid { list-style: none; margin: 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fill, minmax(258px, 1fr)); gap: 16px; }
.tools-card { display: flex; flex-direction: column; gap: 7px; height: 100%; padding: 18px; border: 1px solid #e5e7eb; border-radius: 14px; background: #fff; text-decoration: none; color: inherit; transition: border-color .15s, box-shadow .15s, transform .15s; }
.tools-card:hover, .tools-card:focus-visible { border-color: var(--tool-accent, #4f46e5); box-shadow: 0 6px 20px rgba(15,23,42,.09); transform: translateY(-2px); }
.tools-card__title { font-size: 1.05rem; font-weight: 650; margin: 0; color: var(--tool-ink, #0f172a); }
.tools-card__desc { font-size: .9rem; color: var(--tool-muted, #6b7280); margin: 0; line-height: 1.45; }
.tools-card__tags { display: flex; flex-wrap: wrap; gap: 6px; margin-top: auto; padding-top: 10px; }
.tools-card__tag { font-size: .72rem; color: var(--tool-accent, #4f46e5); background: rgba(79,70,229,.10); padding: 2px 9px; border-radius: 999px; }
.tools-index__search { margin-top: 20px; }
.tools-index__search input { width: 100%; max-width: 440px; padding: 11px 16px; border: 1px solid #e5e7eb; border-radius: 999px; font-size: .95rem; outline: none; }
.tools-index__search input:focus { border-color: var(--tool-accent, #4f46e5); box-shadow: 0 0 0 3px rgba(79,70,229,.12); }
.tools-index__empty { text-align: center; color: var(--tool-muted, #6b7280); margin: 24px 0 8px; }
.tools-grid li[hidden] { display: none; }
"#;

/// Client-side filter for the `/tools/` grid — reuses `filterTools` from
/// `tools-index.js` (same matcher as the header Explore search) and just
/// shows/hides the server-rendered cards, so they stay crawlable.
const TOOLS_FILTER_JS: &str = r#"
import { filterTools } from './tools-index.js';
const input = document.getElementById('tools-filter');
const grid = document.getElementById('tools-grid');
const empty = document.getElementById('tools-empty');
let tools = null;
async function ensureData() {
  if (tools) return;
  try { tools = await (await fetch('_index.json')).json(); } catch { tools = []; }
}
async function apply() {
  const q = input.value.trim();
  if (q) await ensureData();
  const matched = q ? new Set(filterTools(tools, q).map(t => t.slug)) : null;
  let shown = 0;
  for (const li of grid.querySelectorAll('li[data-slug]')) {
    const on = !matched || matched.has(li.dataset.slug);
    li.hidden = !on;
    if (on) shown++;
  }
  empty.hidden = shown !== 0;
}
input.addEventListener('input', apply);
"#;

/// Render the `/tools/` landing page: shared chrome + a responsive card grid of
/// every tool that has a page. Built from the same `ToolMeta` slice as the
/// per-tool pages and `_index.json` — one source of truth, no drift.
pub fn render_tools_index(metas: &[ToolMeta]) -> String {
    let canonical = "https://gizza.ai/tools/";
    let title = "All Tools — gizza.ai";
    let description = "Browse every gizza.ai tool — free, private, browser-local utilities. \
        No sign-up, nothing leaves your device, works offline.";
    // JSON-LD ItemList of the tools (SEO); `</`-neutralized like the other pages.
    let item_list = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "ItemList",
        "name": "gizza.ai tools",
        "itemListElement": metas.iter().enumerate().map(|(i, m)| serde_json::json!({
            "@type": "ListItem",
            "position": i + 1,
            "url": format!("https://gizza.ai/tools/{}/", m.slug),
            "name": m.h1,
        })).collect::<Vec<_>>(),
    })
    .to_string()
    .replace("</", "<\\/");

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                meta name="description" content=(description);
                link rel="canonical" href=(canonical);
                meta property="og:type" content="website";
                meta property="og:title" content=(title);
                meta property="og:description" content=(description);
                meta property="og:url" content=(canonical);
                meta property="og:image" content="https://gizza.ai/gis.png";
                meta name="twitter:card" content="summary";
                meta name="twitter:title" content=(title);
                meta name="twitter:description" content=(description);
                link rel="stylesheet" href="https://site-kit.suppers.ai/dist/design-system.css";
                link rel="stylesheet" href="./tool.css";
                link rel="stylesheet" href="./header.css";
                link rel="icon" href="https://gizza.ai/favicon-32.png" sizes="32x32";
                style { (PreEscaped(TOOLS_INDEX_CSS)) }
                script type="application/ld+json" { (PreEscaped(item_list)) }
                script type="module" src="./header.js" {}
            }
            body {
                ({
                    let brand = html! {
                        a.tool-brand href="https://gizza.ai" {
                            img src="/logo.webp" alt="gizza.ai logo";
                            span { "gizza.ai" }
                        }
                    };
                    chrome_header(brand, Active::Tool)
                })
                main class="tools-index" {
                    section class="tools-index__hero" {
                        h1 { "All tools" }
                        p class="tools-index__sub" {
                            "Free, private, browser-local tools. Everything runs in your browser — \
                             nothing leaves your device, no sign-up, works offline."
                        }
                        div class="tools-index__search" {
                            input #tools-filter type="search" placeholder="Search tools…"
                                  autocomplete="off" autocapitalize="off" spellcheck="false"
                                  aria-label="Search tools";
                        }
                    }
                    // Cards are server-rendered (crawlable for SEO); the filter
                    // below just shows/hides them client-side via filterTools.
                    ul #tools-grid class="tools-grid" {
                        @for m in metas {
                            li data-slug=(m.slug) {
                                a class="tools-card" href=(format!("/tools/{}/", m.slug)) {
                                    h2 class="tools-card__title" { (m.h1) }
                                    p class="tools-card__desc" { (m.description) }
                                    @if !m.tags.is_empty() {
                                        div class="tools-card__tags" {
                                            @for t in m.tags.iter().take(3) {
                                                span class="tools-card__tag" { (t) }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    p #tools-empty class="tools-index__empty" hidden { "No tools match your search." }
                }
                (chrome_footer())
                script type="module" { (PreEscaped(TOOLS_FILTER_JS)) }
            }
        }
    };
    markup.into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::ToolMeta;

    fn sample() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator — gizza.ai"
description   = "Evaluate expressions instantly."
h1            = "Free Online Calculator"
hero_subtitle = "Type a math expression."
wasm          = "gizza_ai_calculator_web"
export        = "evaluate"
live          = false
output_label  = "Result"
format        = "number"

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
source      = "field"
"#,
        )
        .unwrap()
    }

    #[test]
    fn includes_seo_head_and_widget() {
        let html = render_page(&sample(), "<h2>About</h2>", &ParamSchema::empty());
        assert!(html.contains("<title>Free Online Calculator — gizza.ai</title>"));
        assert!(html.contains(r#"<link rel="canonical" href="https://gizza.ai/tools/calculator/">"#));
        assert!(
            html.contains(r#"<link rel="alternate" type="text/markdown" href="index.md">"#),
            "markdown twin discovery link present",
        );
        assert!(html.contains("application/ld+json"));
        assert!(html.contains(r#"id="in-expr""#));
        assert!(html.contains(r#"id="tool-output""#));
        assert!(html.contains("window.GIZZA_TOOL"));
        assert!(html.contains("<h2>About</h2>"));
    }

    #[test]
    fn page_documents_query_param_deep_link() {
        let html = render_page(&sample(), "<h2>About</h2>", &ParamSchema::empty());
        assert!(html.contains("Open it by URL"), "deep-link section present");
        assert!(
            html.contains("https://gizza.ai/tools/calculator/?expr="),
            "example deep-link rendered on the page",
        );
    }

    #[test]
    fn includes_shared_chrome_header_and_footer() {
        let html = render_page(&sample(), "<h2>About</h2>", &ParamSchema::empty());
        // Shared header markers from gizza-chrome
        assert!(
            html.contains(r#"id="explore-search""#),
            "shared header explore-search input present",
        );
        assert!(html.contains("Explore"), "shared header Explore mega-menu trigger present");
        // Shared footer columns from gizza-chrome
        assert!(html.contains("Product"), "footer Product column present");
        assert!(html.contains("Resources"), "footer Resources column present");
        assert!(html.contains("For AI"), "footer For AI & devs column present");
        // #85 rel=alternate link must survive the chrome migration
        assert!(
            html.contains(r#"<link rel="alternate" type="text/markdown" href="index.md">"#),
            "#85 markdown twin discovery link preserved after chrome integration",
        );
        // Chrome asset links present (tools-index.js is copied alongside but not
        // referenced directly — header.js imports it as a relative module)
        assert!(html.contains("header.css"), "header.css link present");
        assert!(html.contains("header.js"), "header.js script present");
    }

    #[test]
    fn tools_index_lists_all_tools_with_chrome() {
        let html = render_tools_index(&[sample()]);
        // landing-page SEO + shared chrome
        assert!(html.contains("<title>All Tools — gizza.ai</title>"));
        assert!(html.contains(r#"<link rel="canonical" href="https://gizza.ai/tools/">"#));
        assert!(html.contains(r#"id="explore-search""#), "shared header present");
        assert!(html.contains("Resources"), "shared footer present");
        assert!(html.contains("ItemList"), "JSON-LD ItemList for SEO");
        // a card per tool, rendered from the same ToolMeta (one source of truth)
        assert!(html.contains(r#"href="/tools/calculator/""#), "card links to the tool page");
        assert!(html.contains("Free Online Calculator"), "card shows the tool h1");
        assert!(html.contains("Evaluate expressions instantly."), "card shows the description");
        assert!(html.contains("tools-card"), "rendered as a card grid");
        // live filter: a search input + per-card data-slug the JS keys on
        assert!(html.contains(r#"id="tools-filter""#), "search filter input present");
        assert!(html.contains(r#"data-slug="calculator""#), "cards carry data-slug for filtering");
    }

    fn ffmpeg_sample() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "image-resize"
title         = "Resize"
description   = "d"
h1            = "Resize an image"
hero_subtitle = "s"
wasm          = "gizza_ai_image_resize_web"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "Resized image"
format        = "image"

[[input]]
name   = "image"
source = "file"
accept = "image/*"
label  = "Image"

[[input]]
name   = "width"
source = "field"
label  = "Width (px)"
placeholder = "640"
"#,
        )
        .unwrap()
    }

    #[test]
    fn renders_file_input_and_media_output() {
        let html = render_page(&ffmpeg_sample(), "<h2>About</h2>", &ParamSchema::empty());
        assert!(html.contains(r#"type="file""#), "file input present");
        assert!(html.contains(r#"id="in-image""#), "file input id");
        assert!(html.contains(r#"accept="image/*""#), "accept attr");
        assert!(html.contains(r#"id="in-width""#), "field input still present");
        assert!(html.contains(r#"id="tool-output-media""#), "media output element");
        assert!(html.contains(r#"id="tool-output-download""#), "download link");
        assert!(html.contains(r#"id="tool-output""#), "status output for errors");
    }
}
