//! Renders the option-C tool page: top nav + hero tool + SEO content + footer,
//! with SEO `<head>` tags and JSON-LD.

use crate::categories::Hub;
use crate::control::{fmt_num, Control, ParamSchema};
use crate::markdown::{cli_example as shared_cli_example, example_deeplink};
use crate::meta::ToolMeta;
use crate::pairs::PairLink;
use crate::site::SiteConfig;
use maud::{html, Markup, PreEscaped, DOCTYPE};

/// Render the full HTML document for a tool page. `content_html` is the
/// markdown-rendered SEO section. `custom_js`/`custom_css` say whether the tool
/// ships a `page/custom.js`/`page/custom.css` (copied next to the page by
/// main.rs) — the escape hatch for bespoke UI that the declarative meta.toml
/// controls can't express. `related` is the tag-ranked top-5 from
/// `related::related_tools`, rendered as internal links. `pairs` is the
/// tool's "X to Y" conversion pages (`pairs::pair_links_for_parent`) —
/// non-empty only for the converter tools, rendered as a crawlable "Popular
/// conversions" section right after the content.
pub fn render_page(
    cfg: &SiteConfig,
    meta: &ToolMeta,
    content_html: &str,
    schema: &ParamSchema,
    custom_js: bool,
    custom_css: bool,
    related: &[&ToolMeta],
    pairs: &[PairLink],
) -> String {
    let canonical = cfg.abs(&format!("/tools/{}/", meta.slug));
    let og_image = cfg.abs(&format!("/tools/{}/og.png", meta.slug));
    // `meta.title` is the bare per-tool title (Task 3: the literal site suffix
    // moved out of `blocks/*/page/meta.toml` and into `cfg.title_suffix`).
    let page_title = cfg.title(&meta.title);
    // Both JSON blobs below are emitted raw inside <script> via PreEscaped, so a
    // literal "</script>" in any value would break out of the element. serde_json
    // does not escape '/', so we neutralize the closing-tag sequence. Values are
    // repo-authored today; this is defense-in-depth against a future meta.toml.
    let mut client_cfg_json = meta.client_config(schema);
    client_cfg_json["custom"] = serde_json::json!(custom_js);
    let client_cfg = client_cfg_json.to_string().replace("</", "<\\/");
    let mut json_ld_obj = serde_json::json!({
        "@context": "https://schema.org",
        "@type": "WebApplication",
        "name": meta.h1,
        "description": meta.description,
        "applicationCategory": "UtilitiesApplication",
        "operatingSystem": "Any",
        "offers": { "@type": "Offer", "price": "0", "priceCurrency": "USD" },
    });
    if let Some(c) = &canonical {
        json_ld_obj["url"] = serde_json::json!(c);
    }
    if let Some(img) = &og_image {
        json_ld_obj["image"] = serde_json::json!(img);
    }
    if !cfg.base_url.is_empty() {
        json_ld_obj["publisher"] =
            serde_json::json!({ "@type": "Organization", "name": cfg.brand_name, "url": cfg.base_url });
    }
    let json_ld = json_ld_obj.to_string().replace("</", "<\\/");
    // BreadcrumbList JSON-LD only makes sense tied to a canonical identity —
    // omitted entirely for the unbranded/generic render.
    let breadcrumbs_ld = (!cfg.base_url.is_empty()).then(|| {
        breadcrumbs_json_ld(&[
            ("Home", &cfg.abs("/").unwrap()),
            ("Tools", &cfg.abs("/tools/").unwrap()),
            (&meta.h1, canonical.as_deref().unwrap()),
        ])
    });
    // FAQPage JSON-LD mirrors the page's visible `## FAQ` <details> blocks
    // (extracted from the rendered HTML — one source of truth, no drift).
    // Pages without a FAQ section simply don't emit the block.
    let faqs = crate::faq::extract_faqs(content_html);
    let faq_ld = (!faqs.is_empty()).then(|| {
        serde_json::json!({
            "@context": "https://schema.org",
            "@type": "FAQPage",
            "mainEntity": faqs.iter().map(|f| serde_json::json!({
                "@type": "Question",
                "name": f.question,
                "acceptedAnswer": { "@type": "Answer", "text": f.answer_html },
            })).collect::<Vec<_>>(),
        })
        .to_string()
        .replace("</", "<\\/")
    });

    // How to run THIS tool headlessly via the gizza CLI — the same
    // copy-paste-runnable example as the markdown twin (file tools include a
    // sample for every field, since those are often required).
    let cli_example = shared_cli_example(meta, schema);

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (page_title) }
                meta name="description" content=(meta.description);
                @if let Some(c) = &canonical {
                    link rel="canonical" href=(c);
                }
                link rel="alternate" type="text/markdown" href="index.md";
                link rel="alternate" type="application/json" href="tool.json";
                meta property="og:type" content="website";
                meta property="og:title" content=(page_title);
                meta property="og:description" content=(meta.description);
                @if let Some(c) = &canonical {
                    meta property="og:url" content=(c);
                }
                @if let Some(img) = &og_image {
                    meta property="og:image" content=(img);
                    meta property="og:image:width" content="1200";
                    meta property="og:image:height" content="630";
                    meta property="og:image:alt" content=(page_title);
                }
                meta name="twitter:card" content="summary_large_image";
                meta name="twitter:title" content=(page_title);
                meta name="twitter:description" content=(meta.description);
                @if let Some(img) = &og_image {
                    meta name="twitter:image" content=(img);
                }
                link rel="stylesheet" href="https://site-kit.suppers.ai/dist/design-system.css";
                link rel="stylesheet" href="./tool.css";
                link rel="stylesheet" href="./header.css";
                @if custom_css {
                    link rel="stylesheet" href="./custom.css";
                }
                (PreEscaped(cfg.head_extras.clone()))
                style { (PreEscaped(TOOL_CLI_CSS)) }
                @if !related.is_empty() {
                    style { (PreEscaped(TOOL_RELATED_CSS)) }
                }
                @if !pairs.is_empty() {
                    style { (PreEscaped(TOOL_PAIRS_CSS)) }
                }
                script type="application/ld+json" { (PreEscaped(json_ld)) }
                @if let Some(bld) = &breadcrumbs_ld {
                    script type="application/ld+json" { (PreEscaped(bld)) }
                }
                @if let Some(faq_ld) = &faq_ld {
                    script type="application/ld+json" { (PreEscaped(faq_ld)) }
                }
                script type="module" src="./header.js" {}
            }
            body {
                (PreEscaped(cfg.header_html_fragment().to_string()))
                main class="tool-main" {
                    section class="tool-hero" {
                        h1 { (meta.h1) }
                        p class="tool-hero-sub" { (meta.hero_subtitle) }
                        // Every other tool on the site is local-only, so a tool that
                        // actually reaches the network has to say so before it runs.
                        @if meta.network {
                            p class="tool-network-note" {
                                "Heads up: this tool makes a real network request. "
                                "The address you enter is fetched from your browser, so \
                                 the request leaves your device and the target site must \
                                 allow cross-origin access — many sites do not."
                            }
                        }
                        @let widget_class = if meta.wide { "tool-widget tool-widget--wide" } else { "tool-widget" };
                        div class=(widget_class) {
                            @for input in &meta.inputs {
                                @if input.source == "field" {
                                    @let id = format!("in-{}", input.name);
                                    label class="tool-field-label" for=(id) { (input.label) }
                                    @match schema.control_for_input(input) {
                                        Control::Select { options, default } => {
                                            select id=(id) class="tool-input tool-select" {
                                                @for opt in &options {
                                                    // Value stays canonical (deep-links/chips/CLI);
                                                    // meta `labels` only enriches the visible text.
                                                    option value=(opt) selected[Some(opt) == default.as_ref()] {
                                                        (input.labels.get(opt).unwrap_or(opt))
                                                    }
                                                }
                                            }
                                        }
                                        Control::Checkbox { default } => {
                                            input id=(id) class="tool-checkbox" type="checkbox" checked[default];
                                        }
                                        Control::Number { min, max, default, step_any } => {
                                            @let min_s = min.map(fmt_num);
                                            @let max_s = max.map(fmt_num);
                                            @let val_s = default.map(fmt_num);
                                            input id=(id) class="tool-input" type="number"
                                                  placeholder=(input.placeholder)
                                                  min=[min_s.as_deref()] max=[max_s.as_deref()] value=[val_s.as_deref()]
                                                  step=[step_any.then_some("any")]
                                                  autocomplete="off" autocapitalize="off" spellcheck="false";
                                        }
                                        Control::Slider { min, max, default, step } => {
                                            // Range + number pair. The number input keeps the
                                            // canonical in-<name> id so deep-links, reset, chips
                                            // and gatherArgs are untouched; tool.js mirrors the
                                            // slider generically via data-for (no slug branches).
                                            @let min_s = fmt_num(min);
                                            @let max_s = fmt_num(max);
                                            @let val_s = default.map(fmt_num);
                                            div class="tool-slider-row" {
                                                input id=(format!("{id}-slider")) class="tool-slider" type="range"
                                                      data-for=(id) min=(min_s) max=(max_s) step=(step)
                                                      value=[val_s.as_deref()] aria-label=(input.label);
                                                input id=(id) class="tool-input tool-slider-value" type="number"
                                                      placeholder=(input.placeholder)
                                                      min=(min_s) max=(max_s) value=[val_s.as_deref()] step="any"
                                                      autocomplete="off" autocapitalize="off" spellcheck="false";
                                            }
                                        }
                                        Control::Picker { input_type } => {
                                            input id=(id) class="tool-input" type=(input_type)
                                                  placeholder=(input.placeholder) autocomplete="off";
                                        }
                                        Control::Color { default } => {
                                            // Swatch + text pair. The TEXT input keeps the
                                            // canonical in-<name> id (deep-links, reset, chips,
                                            // gatherArgs untouched) and stays the source of
                                            // truth — empty ("transparent"/default), alpha hex
                                            // and color lists all live there; tool.js mirrors
                                            // the native swatch generically via data-for.
                                            @let swatch = default.as_deref().and_then(crate::control::expand_hex);
                                            div class="tool-color-row" {
                                                input id=(format!("{id}-swatch")) class="tool-color-swatch" type="color"
                                                      data-for=(id) value=[swatch.as_deref()] aria-label=(input.label);
                                                input id=(id) class="tool-input tool-color-value" type="text"
                                                      placeholder=(input.placeholder)
                                                      autocomplete="off" autocapitalize="off" spellcheck="false";
                                            }
                                        }
                                        Control::Datalist { options } => {
                                            @let list_id = format!("dl-{}", input.name);
                                            input id=(id) class="tool-input" type="text" list=(list_id)
                                                  placeholder=(input.placeholder)
                                                  autocomplete="off" autocapitalize="off" spellcheck="false";
                                            datalist id=(list_id) {
                                                @for opt in &options { option value=(opt) {} }
                                            }
                                        }
                                        Control::TagList { options } => {
                                            @let list_id = format!("dl-{}", input.name);
                                            @let has_opts = !options.is_empty();
                                            input id=(id) type="text" hidden;
                                            div class="tool-tags" data-input=(id) {
                                                div class="tool-tags-list" {}
                                                div class="tool-tags-add" {
                                                    input type="text" class="tool-input tool-tags-search"
                                                          list=[has_opts.then(|| list_id.clone())]
                                                          placeholder=(input.placeholder)
                                                          autocomplete="off" autocapitalize="off" spellcheck="false";
                                                    button type="button" class="tool-tags-add-btn" { "Add" }
                                                }
                                            }
                                            @if has_opts {
                                                datalist id=(list_id) {
                                                    @for opt in &options { option value=(opt) {} }
                                                }
                                            }
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
                            @if !meta.examples.is_empty() {
                                div class="tool-examples" {
                                    span class="tool-examples-label" { "Try:" }
                                    @for (i, ex) in meta.examples.iter().enumerate() {
                                        button type="button" class="tool-example-chip" data-example=(i) { (ex.label) }
                                    }
                                }
                            }
                            div class="tool-output-label" { (meta.output_label) }
                            // Media display covers svg too: the SVG is rendered as
                            // <img src="data:image/svg+xml;base64,…">, which cannot
                            // execute script the way innerHTML would, so no sanitizer
                            // dependency and no reliance on each block escaping user
                            // text correctly.
                            @let is_media = meta.format == "image" || meta.format == "video"
                                || meta.format == "audio" || meta.format == "svg";
                            // Binary media = the formats whose result is bytes, not text.
                            // svg is media for DISPLAY but still text for copy/paste.
                            @let is_binary_media = meta.format == "image" || meta.format == "video"
                                || meta.format == "audio";
                            @if is_media {
                                @if meta.format == "image" || meta.format == "svg" {
                                    img id="tool-output-media" class="tool-output-media" alt="" hidden;
                                } @else if meta.format == "video" {
                                    video id="tool-output-media" class="tool-output-media" controls hidden {}
                                } @else {
                                    audio id="tool-output-media" class="tool-output-media" controls hidden {}
                                }
                                // Media-output actions row. Download is offered for
                                // every media format; "Copy image" only for raster
                                // images — video/audio have no reliable ClipboardItem
                                // path, and for svg the clipboard is reliably PNG-only
                                // while canvas-drawing an SVG varies by browser, so svg
                                // uses the text "Copy result" button instead. All start
                                // hidden; tool.js reveals them once a result renders.
                                div class="tool-media-actions" {
                                    a id="tool-output-download" class="tool-output-download" download hidden { "Download" }
                                    @if meta.format == "image" {
                                        button id="tool-copy-image" class="tool-widget-btn" type="button" hidden
                                               title="Copy the image to the clipboard" { "Copy image" }
                                    }
                                }
                                output id="tool-output" class="tool-output" { "" }
                            } @else {
                                output id="tool-output" class="tool-output" { "" }
                            }
                            @let has_fields = meta.inputs.iter().any(|i| i.source == "field");
                            @if has_fields || !is_binary_media {
                                div class="tool-widget-actions" {
                                    @if has_fields {
                                        button id="tool-reset" class="tool-widget-btn" type="button"
                                               title="Restore the default inputs" { "Reset" }
                                    }
                                    @if !is_binary_media {
                                        button id="tool-copy-output" class="tool-widget-btn" type="button"
                                               title="Copy the result to the clipboard" { "Copy result" }
                                    }
                                    // Text tools: download the current output as a file (every
                                    // list/CSV/JSON competitor offers an export). tool.js keeps the
                                    // blob URL in sync with the output; data-text-download marks it
                                    // apart from the media download link (same id, one per page).
                                    @if meta.format == "text" {
                                        a id="tool-output-download" class="tool-widget-btn" data-text-download
                                          download=(format!("{}-output.txt", meta.slug)) hidden
                                          title="Download the result as a text file" { "Download" }
                                    }
                                }
                            }
                        }
                    }
                    section class="tool-content" {
                        (PreEscaped(content_html))
                    }
                    // Crawlable "X to Y" landing-page links for the converter
                    // tools — each chip is a pair page nested under this tool.
                    @if !pairs.is_empty() {
                        section class="tool-pairs" {
                            h2 { "Popular conversions" }
                            ul class="tool-pairs-list" {
                                @for p in pairs {
                                    li { a class="tool-pairs-link" href=(p.href) { (p.label) } }
                                }
                            }
                        }
                    }
                    // Internal links to the tag-nearest tools — crawlable
                    // `<a href>`s that spread link equity across the catalog
                    // and give readers a next step.
                    @if !related.is_empty() {
                        section class="tool-related" {
                            h2 { "Related tools" }
                            ul class="tool-related-list" {
                                @for r in related {
                                    li class="tool-related-item" {
                                        a class="tool-related-link" href=(format!("/tools/{}/", r.slug)) { (r.h1) }
                                        p class="tool-related-desc" { (r.description) }
                                    }
                                }
                            }
                        }
                    }
                    section class="tool-dev-group" {
                        h2 { "Developer & Automation Access" }
                        div class="tool-dev-grid" {
                            div class="tool-cli-card" {
                                h3 { "Run it from the terminal" }
                                p { "Same engine as this page, headless — via the gizza CLI:" }
                                div class="tool-cli-code-wrapper" {
                                    pre class="tool-cli-code" { code { (cli_example) } }
                                    button class="tool-cli-copy-btn" title="Copy to clipboard"
                                        onclick="navigator.clipboard.writeText(this.previousElementSibling.textContent.trim()).then(()=>{this.classList.add('copied');setTimeout(()=>this.classList.remove('copied'),2000)})" {
                                        svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" {
                                            rect width="14" height="14" x="8" y="8" rx="2" ry="2" {}
                                            path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" {}
                                        }
                                        svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" {
                                            polyline points="20 6 9 17 4 12" {}
                                        }
                                    }
                                }
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
                                    div class="tool-cli-code-wrapper" {
                                        pre class="tool-cli-code" { code { (example_deeplink(cfg, meta, schema)) } }
                                        button class="tool-cli-copy-btn" title="Copy to clipboard"
                                            onclick="navigator.clipboard.writeText(this.previousElementSibling.textContent.trim()).then(()=>{this.classList.add('copied');setTimeout(()=>this.classList.remove('copied'),2000)})" {
                                            svg class="copy-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" {
                                                rect width="14" height="14" x="8" y="8" rx="2" ry="2" {}
                                                path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2" {}
                                            }
                                            svg class="check-icon" xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round" {
                                                polyline points="20 6 9 17 4 12" {}
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        // Full-width footer line under the grid — the third
                        // automation surface next to the CLI and deep-link cards.
                        p class="tool-dev-note" {
                            "Machine-readable descriptor: "
                            a href="tool.json" { "tool.json" }
                            " — title + parameters JSON Schema for agents."
                        }
                    }
                }
                (PreEscaped(cfg.footer_html_fragment().to_string()))
                script { (PreEscaped(format!("window.GIZZA_TOOL = {client_cfg};"))) }
                script type="module" src="./tool.js" {}
            }
        }
    };
    markup.into_string()
}

/// BreadcrumbList JSON-LD for an ordered (name, url) trail, `</`-neutralized
/// like the other JSON-LD blobs (`pub(crate)` — the pair pages emit the same
/// trail with one extra level).
pub(crate) fn breadcrumbs_json_ld(trail: &[(&str, &str)]) -> String {
    serde_json::json!({
        "@context": "https://schema.org",
        "@type": "BreadcrumbList",
        "itemListElement": trail.iter().enumerate().map(|(i, (name, url))| {
            serde_json::json!({
                "@type": "ListItem",
                "position": i + 1,
                "name": name,
                "item": url,
            })
        }).collect::<Vec<_>>(),
    })
    .to_string()
    .replace("</", "<\\/")
}

/// Inline styles for the per-tool "Run it from the terminal" CLI block.
const TOOL_CLI_CSS: &str = r#"
.tool-dev-group { max-width: 720px; margin: 48px auto 64px; }
.tool-dev-group h2 { font-size: 1.3rem; margin: 0 0 8px; color: var(--tool-ink, #0f172a); border-bottom: 1px solid #e2e8f0; padding-bottom: 8px; }
.tool-dev-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(320px, 1fr)); gap: 20px; }
.tool-cli-card { background: #ffffff; border: 1px solid #e2e8f0; border-radius: 12px; padding: 20px; display: flex; flex-direction: column; box-shadow: 0 1px 3px rgba(0,0,0,0.05); }
.tool-cli-card h3 { font-size: 1.1rem; font-weight: 650; margin: 0 0 8px; color: var(--tool-ink, #0f172a); }
.tool-cli-card > p { color: var(--tool-muted, #6b7280); font-size: 0.9rem; margin: 0 0 12px; }
.tool-cli-code-wrapper { position: relative; width: 100%; display: flex; align-items: stretch; }
.tool-cli-code { flex: 1; background: #0f172a; color: #e2e8f0; padding: 14px 44px 14px 16px; border-radius: 10px; font-size: 0.85rem; line-height: 1.5; margin: 0; border: 1px solid #1e293b; box-shadow: inset 0 2px 4px rgba(0,0,0,0.35); white-space: pre-wrap; word-break: break-all; overflow-wrap: break-word; }
.tool-cli-code code { color: inherit; background: none; padding: 0; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
.tool-cli-copy-btn { position: absolute; top: 10px; right: 10px; background: transparent; border: none; color: #64748b; padding: 6px; border-radius: 6px; cursor: pointer; display: flex; align-items: center; justify-content: center; transition: background-color 0.15s, color 0.15s; }
.tool-cli-copy-btn:hover { background: rgba(255, 255, 255, 0.1); color: #e2e8f0; }
.tool-cli-copy-btn.copied { color: #22c55e; background: rgba(34, 197, 94, 0.1); }
.tool-cli-copy-btn .check-icon { display: none; }
.tool-cli-copy-btn.copied .copy-icon { display: none; }
.tool-cli-copy-btn.copied .check-icon { display: block; }
.tool-cli-note { font-size: .85rem; margin: 12px 0 0; color: var(--tool-muted, #6b7280); margin-top: auto; padding-top: 10px; }
.tool-dev-note { font-size: .85rem; color: var(--tool-muted, #6b7280); margin: 14px 2px 0; }
.tool-dev-note a { color: var(--tool-accent, #4f46e5); }
"#;

/// Inline styles for the per-tool "Related tools" section (same card idiom as
/// the CLI block above; only emitted when the section renders).
const TOOL_RELATED_CSS: &str = r#"
.tool-related { max-width: 720px; margin: 48px auto 0; }
.tool-related h2 { font-size: 1.3rem; margin: 0 0 12px; color: var(--tool-ink, #0f172a); border-bottom: 1px solid #e2e8f0; padding-bottom: 8px; }
.tool-related-list { list-style: none; margin: 0; padding: 0; display: grid; gap: 10px; }
.tool-related-item { background: #ffffff; border: 1px solid #e2e8f0; border-radius: 12px; padding: 14px 16px; box-shadow: 0 1px 3px rgba(0,0,0,0.05); }
.tool-related-link { font-weight: 650; color: var(--tool-accent, #4f46e5); text-decoration: none; }
.tool-related-link:hover, .tool-related-link:focus-visible { text-decoration: underline; }
.tool-related-desc { margin: 4px 0 0; font-size: .9rem; color: var(--tool-muted, #6b7280); line-height: 1.45; }
"#;

/// Inline styles for the "Popular conversions" chip list on the converter
/// tools' pages (same section idiom as the related-tools block; only emitted
/// when the tool has pair pages).
const TOOL_PAIRS_CSS: &str = r#"
.tool-pairs { max-width: 720px; margin: 48px auto 0; }
.tool-pairs h2 { font-size: 1.3rem; margin: 0 0 12px; color: var(--tool-ink, #0f172a); border-bottom: 1px solid #e2e8f0; padding-bottom: 8px; }
.tool-pairs-list { list-style: none; margin: 0; padding: 0; display: flex; flex-wrap: wrap; gap: 8px; }
.tool-pairs-link { display: inline-block; padding: 7px 14px; border: 1px solid #e2e8f0; border-radius: 999px; background: #fff; color: var(--tool-accent, #4f46e5); font-size: .9rem; text-decoration: none; }
.tool-pairs-link:hover, .tool-pairs-link:focus-visible { border-color: var(--tool-accent, #4f46e5); text-decoration: underline; }
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
.tools-cats { display: flex; flex-wrap: wrap; gap: 8px; justify-content: center; margin: 0 0 28px; }
.tools-cats__link { display: inline-flex; align-items: center; gap: 6px; padding: 6px 14px; border: 1px solid #e5e7eb; border-radius: 999px; background: #fff; text-decoration: none; color: var(--tool-ink, #0f172a); font-size: .88rem; transition: border-color .15s, color .15s; }
.tools-cats__link:hover, .tools-cats__link:focus-visible { border-color: var(--tool-accent, #4f46e5); color: var(--tool-accent, #4f46e5); }
.tools-cats__link[aria-current="page"] { border-color: var(--tool-accent, #4f46e5); background: rgba(79,70,229,.08); color: var(--tool-accent, #4f46e5); }
.tools-cats__count { font-size: .75rem; color: var(--tool-muted, #6b7280); }
.tools-breadcrumb { font-size: .85rem; color: var(--tool-muted, #6b7280); margin: 0 0 10px; }
.tools-breadcrumb a { color: inherit; text-decoration: none; }
.tools-breadcrumb a:hover { text-decoration: underline; }
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

/// The shared `<head>` SEO cluster for the `/tools/` landing and category hub
/// pages: title/description/canonical, OG + twitter card tags and relative
/// stylesheet/icon links (both pages get their chrome assets copied alongside).
/// `canonical`/`og_image` are `None` when unbranded — the corresponding tags
/// are omitted entirely rather than pointing at a bogus relative URL.
fn index_head_meta(
    cfg: &SiteConfig,
    title: &str,
    description: &str,
    canonical: Option<&str>,
    og_image: Option<&str>,
) -> Markup {
    html! {
        meta charset="utf-8";
        meta name="viewport" content="width=device-width, initial-scale=1";
        title { (title) }
        meta name="description" content=(description);
        @if let Some(c) = canonical {
            link rel="canonical" href=(c);
        }
        meta property="og:type" content="website";
        meta property="og:title" content=(title);
        meta property="og:description" content=(description);
        @if let Some(c) = canonical {
            meta property="og:url" content=(c);
        }
        @if let Some(img) = og_image {
            meta property="og:image" content=(img);
            meta property="og:image:width" content="1200";
            meta property="og:image:height" content="630";
            meta property="og:image:alt" content=(title);
        }
        meta name="twitter:card" content="summary_large_image";
        meta name="twitter:title" content=(title);
        meta name="twitter:description" content=(description);
        @if let Some(img) = og_image {
            meta name="twitter:image" content=(img);
        }
        link rel="stylesheet" href="https://site-kit.suppers.ai/dist/design-system.css";
        link rel="stylesheet" href="./tool.css";
        link rel="stylesheet" href="./header.css";
        (PreEscaped(cfg.head_extras.clone()))
        style { (PreEscaped(TOOLS_INDEX_CSS)) }
    }
}

/// ItemList JSON-LD for a set of tool cards, `</`-neutralized like the other
/// JSON-LD blobs. Shared by the landing page and the category hubs.
fn item_list_json_ld(cfg: &SiteConfig, name: &str, metas: &[&ToolMeta]) -> String {
    serde_json::json!({
        "@context": "https://schema.org",
        "@type": "ItemList",
        "name": name,
        "itemListElement": metas.iter().enumerate().map(|(i, m)| serde_json::json!({
            "@type": "ListItem",
            "position": i + 1,
            "url": cfg.url_or_rel(&format!("/tools/{}/", m.slug)),
            "name": m.h1,
        })).collect::<Vec<_>>(),
    })
    .to_string()
    .replace("</", "<\\/")
}

/// The server-rendered tool-card grid (crawlable `<a href>` per tool) —
/// shared by the `/tools/` landing and the category hub pages. `data-slug`
/// is what the landing's client-side filter keys on.
fn tools_grid(metas: &[&ToolMeta]) -> Markup {
    html! {
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
    }
}

/// Crawlable category-hub nav (title + tool count per hub), shared by the
/// landing page and the hubs themselves; `current` marks the open hub.
fn category_nav(hubs: &[Hub], current: Option<&str>) -> Markup {
    html! {
        nav class="tools-cats" aria-label="Tool categories" {
            @for hub in hubs {
                @let is_current = current == Some(hub.category.slug);
                a class="tools-cats__link" href=(format!("/tools/{}/", hub.category.slug))
                  aria-current=[is_current.then_some("page")] {
                    (hub.category.title)
                    span class="tools-cats__count" { (hub.members.len()) }
                }
            }
        }
    }
}

/// Render the `/tools/` landing page: shared chrome + a category nav + a
/// responsive card grid of every tool that has a page. Built from the same
/// `ToolMeta` slice as the per-tool pages and `_index.json` — one source of
/// truth, no drift.
pub fn render_tools_index(cfg: &SiteConfig, metas: &[ToolMeta], hubs: &[Hub]) -> String {
    let canonical = cfg.abs("/tools/");
    let title = cfg.brand_title("All Tools");
    let description = format!(
        "Browse every {}tool — free, private, browser-local utilities. \
         No sign-up, nothing leaves your device, works offline.",
        if cfg.brand_name.is_empty() { String::new() } else { format!("{} ", cfg.brand_name) }
    );
    let og_image = cfg.abs("/tools/og.png");
    let refs: Vec<&ToolMeta> = metas.iter().collect();
    let item_list_name = if cfg.brand_name.is_empty() {
        "Tools".to_string()
    } else {
        format!("{} tools", cfg.brand_name)
    };
    let item_list = item_list_json_ld(cfg, &item_list_name, &refs);
    // BreadcrumbList JSON-LD only makes sense tied to a canonical identity —
    // omitted entirely for the unbranded/generic render.
    let breadcrumbs_ld = (!cfg.base_url.is_empty()).then(|| {
        breadcrumbs_json_ld(&[("Home", &cfg.abs("/").unwrap()), ("Tools", &cfg.abs("/tools/").unwrap())])
    });

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                (index_head_meta(cfg, &title, &description, canonical.as_deref(), og_image.as_deref()))
                // Atom feed autodiscovery — feed readers and crawlers find
                // the 50-newest-tools feed from the landing page. Only
                // discoverable when the site has a canonical feed to point at.
                @if let Some(feed_url) = cfg.abs("/feed.xml") {
                    link rel="alternate" type="application/atom+xml"
                         title=(cfg.brand_title("New tools")) href=(feed_url);
                }
                script type="application/ld+json" { (PreEscaped(item_list)) }
                @if let Some(bld) = &breadcrumbs_ld {
                    script type="application/ld+json" { (PreEscaped(bld)) }
                }
                script type="module" src="./header.js" {}
            }
            body {
                (PreEscaped(cfg.header_html_fragment().to_string()))
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
                    // Category hubs — crawlable links with per-hub tool counts.
                    (category_nav(hubs, None))
                    // Cards are server-rendered (crawlable for SEO); the filter
                    // below just shows/hides them client-side via filterTools.
                    (tools_grid(&refs))
                    p #tools-empty class="tools-index__empty" hidden { "No tools match your search." }
                }
                (PreEscaped(cfg.footer_html_fragment().to_string()))
                script type="module" { (PreEscaped(TOOLS_FILTER_JS)) }
            }
        }
    };
    markup.into_string()
}

/// Render a category hub page at `/tools/<category>/`: the category's title,
/// blurb and member-card grid, with its own canonical/OG/ItemList/breadcrumb
/// SEO head, a visible breadcrumb back to `/tools/`, and the sibling-category
/// nav. No client-side filter here — that stays a landing-page affordance
/// (its `_index.json` fetch is relative to `/tools/`).
pub fn render_category_hub(cfg: &SiteConfig, hub: &Hub, all_hubs: &[Hub]) -> String {
    let cat = hub.category;
    let canonical = cfg.abs(&format!("/tools/{}/", cat.slug));
    let title = cfg.brand_title(cat.title);
    let og_image = cfg.abs(&format!("/tools/{}/og.png", cat.slug));
    let item_list = item_list_json_ld(cfg, cat.title, &hub.members);
    // BreadcrumbList JSON-LD only makes sense tied to a canonical identity —
    // omitted entirely for the unbranded/generic render.
    let breadcrumbs = (!cfg.base_url.is_empty()).then(|| {
        breadcrumbs_json_ld(&[
            ("Home", &cfg.abs("/").unwrap()),
            ("Tools", &cfg.abs("/tools/").unwrap()),
            (cat.title, canonical.as_deref().unwrap()),
        ])
    });

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                (index_head_meta(cfg, &title, cat.blurb, canonical.as_deref(), og_image.as_deref()))
                script type="application/ld+json" { (PreEscaped(item_list)) }
                @if let Some(bcr) = &breadcrumbs {
                    script type="application/ld+json" { (PreEscaped(bcr)) }
                }
                script type="module" src="./header.js" {}
            }
            body {
                (PreEscaped(cfg.header_html_fragment().to_string()))
                main class="tools-index" {
                    section class="tools-index__hero" {
                        p class="tools-breadcrumb" {
                            a href="/" { "Home" }
                            " › "
                            a href="/tools/" { "All tools" }
                            " › "
                            span { (cat.title) }
                        }
                        h1 { (cat.title) }
                        p class="tools-index__sub" { (cat.blurb) }
                    }
                    (category_nav(all_hubs, Some(cat.slug)))
                    (tools_grid(&hub.members))
                }
                (PreEscaped(cfg.footer_html_fragment().to_string()))
            }
        }
    };
    markup.into_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::meta::ToolMeta;

    /// A branded config matching today's real `site/site-config.toml`
    /// (`base_url`/`brand_name`/`title_suffix` set) — most existing assertions
    /// here check the historical absolute `https://gizza.ai/...` links, chrome,
    /// and suffixed `<title>` text.
    fn branded() -> SiteConfig {
        SiteConfig {
            base_url: "https://gizza.ai".into(),
            brand_name: "gizza.ai".into(),
            title_suffix: " — gizza.ai".into(),
            ..Default::default()
        }
    }

    fn sample() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator"
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
        let html = render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
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
    fn emits_per_tool_og_image_and_breadcrumbs() {
        let html = render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            html.contains(r#"content="https://gizza.ai/tools/calculator/og.png""#),
            "og:image + twitter:image point at the per-tool card"
        );
        assert!(html.contains(r#"property="og:image:width" content="1200""#));
        assert!(html.contains(r#"property="og:image:height" content="630""#));
        assert!(html.contains(r#"name="twitter:card" content="summary_large_image""#));
        assert!(html.contains(r#""@type":"BreadcrumbList""#), "breadcrumbs JSON-LD present");
        assert!(
            html.contains(r#""image":"https://gizza.ai/tools/calculator/og.png""#),
            "WebApplication JSON-LD carries the card as its image"
        );
    }

    #[test]
    fn faq_json_ld_mirrors_content_faq_section() {
        let with_faq = "<h2>FAQ</h2>\
            <details><summary>Is it private?</summary><p>Yes — runs locally.</p></details>";
        let html = render_page(&branded(), &sample(), with_faq, &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains(r#""@type":"FAQPage""#), "FAQPage JSON-LD present");
        assert!(html.contains(r#""name":"Is it private?""#));

        let without = render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            !without.contains("FAQPage"),
            "no FAQPage block when the page has no FAQ section"
        );
    }

    #[test]
    fn links_machine_readable_descriptor_in_head_and_dev_section() {
        let html = render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            html.contains(r#"<link rel="alternate" type="application/json" href="tool.json">"#),
            "tool.json discovery link in the head",
        );
        assert!(
            html.contains(r#"<p class="tool-dev-note">Machine-readable descriptor: <a href="tool.json">tool.json</a>"#),
            "dev section links the descriptor",
        );
        // placement: the note sits inside the dev group, after the grid
        let grid = html.find(r#"class="tool-dev-grid""#).unwrap();
        let note = html.find(r#"class="tool-dev-note""#).unwrap();
        assert!(grid < note, "note renders under the dev-access grid");
        assert!(html.contains(".tool-dev-note"), "note CSS emitted");
    }

    #[test]
    fn page_documents_query_param_deep_link() {
        let html = render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains("Open it by URL"), "deep-link section present");
        assert!(
            html.contains("https://gizza.ai/tools/calculator/?expr="),
            "example deep-link rendered on the page",
        );
    }

    #[test]
    fn default_config_renders_generic_header_footer_no_brand() {
        // `sample()`'s title is bare (Task 3: the site suffix moved out of
        // `blocks/*/page/meta.toml` and into `cfg.title_suffix`), so the
        // default (empty-suffix) config renders it completely unsuffixed.
        let html = render_page(
            &SiteConfig::default(),
            &sample(),
            "<h2>About</h2>",
            &ParamSchema::empty(),
            false,
            false,
            &[],
            &[],
        );
        // Generic (unbranded) chrome — the built-in fallback markup.
        assert!(html.contains(r#"<header class="tool-nav">"#), "generic header rendered");
        assert!(html.contains(r#"<footer class="tool-footer">"#), "generic footer rendered");
        // "gizza" alone still appears in the unrelated `gizza` CLI tool name and
        // its GitHub repo link (product naming, not site branding/theming) — the
        // acceptance bar is no *site domain* branding leaking through.
        assert!(!html.contains("gizza.ai"), "no brand leaks into a fully generic render");
        // #85 rel=alternate link must survive with no chrome fragment configured
        assert!(
            html.contains(r#"<link rel="alternate" type="text/markdown" href="index.md">"#),
            "#85 markdown twin discovery link preserved",
        );
        assert!(html.contains("header.css"), "header.css link present");
        assert!(html.contains("header.js"), "header.js script present");
    }

    #[test]
    fn branded_config_injects_header_and_footer_fragments_verbatim() {
        let cfg = SiteConfig {
            header: r#"<header id="brand-header">BRAND HEADER</header>"#.to_string(),
            footer: r#"<footer id="brand-footer">BRAND FOOTER</footer>"#.to_string(),
            ..Default::default()
        };
        let html = render_page(&cfg, &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            html.contains(r#"<header id="brand-header">BRAND HEADER</header>"#),
            "configured header fragment injected verbatim"
        );
        assert!(
            html.contains(r#"<footer id="brand-footer">BRAND FOOTER</footer>"#),
            "configured footer fragment injected verbatim"
        );
        assert!(!html.contains(r#"class="tool-nav""#), "generic header fallback not used when configured");
        assert!(!html.contains(r#"class="tool-footer""#), "generic footer fallback not used when configured");
    }

    #[test]
    fn tools_index_lists_all_tools_with_chrome() {
        let metas = [sample()];
        let hubs = crate::categories::build_hubs(&metas);
        let html = render_tools_index(&branded(), &metas, &hubs);
        // landing-page SEO + chrome (branded() sets identity only, so chrome
        // falls back to the generic fragments here — header/footer fragment
        // wiring is covered separately by the render_page chrome tests).
        assert!(html.contains("<title>All Tools — gizza.ai</title>"));
        assert!(html.contains(r#"<link rel="canonical" href="https://gizza.ai/tools/">"#));
        assert!(html.contains(r#"<header class="tool-nav">"#), "chrome header present");
        assert!(html.contains(r#"<footer class="tool-footer">"#), "chrome footer present");
        assert!(html.contains("ItemList"), "JSON-LD ItemList for SEO");
        // a card per tool, rendered from the same ToolMeta (one source of truth)
        assert!(html.contains(r#"href="/tools/calculator/""#), "card links to the tool page");
        assert!(html.contains("Free Online Calculator"), "card shows the tool h1");
        assert!(html.contains("Evaluate expressions instantly."), "card shows the description");
        assert!(html.contains("tools-card"), "rendered as a card grid");
        // live filter: a search input + per-card data-slug the JS keys on
        assert!(html.contains(r#"id="tools-filter""#), "search filter input present");
        assert!(html.contains(r#"data-slug="calculator""#), "cards carry data-slug for filtering");
        // landing page shares the generated /tools/og.png card
        assert!(html.contains(r#"content="https://gizza.ai/tools/og.png""#));
        assert!(html.contains(r#"name="twitter:card" content="summary_large_image""#));
        assert!(html.contains(r#""@type":"BreadcrumbList""#));
        // Atom feed autodiscovery link in the head
        assert!(
            html.contains(
                r#"<link rel="alternate" type="application/atom+xml" title="New tools — gizza.ai" href="https://gizza.ai/feed.xml">"#
            ),
            "Atom feed autodiscovery link in the head"
        );
    }

    #[test]
    fn default_config_renders_a_generic_unbranded_landing_page() {
        let metas = [sample()];
        let hubs = crate::categories::build_hubs(&metas);
        let html = render_tools_index(&SiteConfig::default(), &metas, &hubs);
        assert!(html.contains("<title>All Tools</title>"), "unsuffixed generic title");
        assert!(!html.contains(r#"<link rel="canonical""#), "no canonical when unbranded");
        assert!(!html.contains("og:image"), "no og:image when unbranded");
        assert!(!html.contains("atom+xml"), "no feed autodiscovery when unbranded");
        assert!(!html.contains("BreadcrumbList"), "no BreadcrumbList JSON-LD when unbranded");
        assert!(html.contains(r#"<header class="tool-nav">"#), "generic header rendered");
        assert!(html.contains(r#"<footer class="tool-footer">"#), "generic footer rendered");
        assert!(!html.contains("gizza"), "fully generic — no brand leaks");
    }

    #[test]
    fn tools_index_renders_category_nav_with_counts() {
        // sample() ("calculator", no tags) lands in math via its slug word.
        let metas = [sample()];
        let hubs = crate::categories::build_hubs(&metas);
        let html = render_tools_index(&branded(), &metas, &hubs);
        assert!(
            html.contains(r#"<nav class="tools-cats" aria-label="Tool categories">"#),
            "category nav present above the grid"
        );
        assert!(
            html.contains(r#"<a class="tools-cats__link" href="/tools/math/""#),
            "crawlable hub link"
        );
        assert!(
            html.contains(r#"Math &amp; statistics tools<span class="tools-cats__count">1</span>"#),
            "hub link shows the title and tool count"
        );
        // the nav renders BEFORE the card grid
        let nav = html.find(r#"<nav class="tools-cats""#).unwrap();
        let grid = html.find(r#"data-slug="calculator""#).unwrap();
        assert!(nav < grid, "category nav sits above the grid");
    }

    #[test]
    fn category_hub_page_has_seo_head_grid_and_nav() {
        let metas = [sample()];
        let hubs = crate::categories::build_hubs(&metas);
        let hub = &hubs[0];
        assert_eq!(hub.category.slug, "math");
        let html = render_category_hub(&branded(), hub, &hubs);
        // SEO head: canonical, description, per-hub og card
        assert!(html.contains("<title>Math &amp; statistics tools — gizza.ai</title>"));
        assert!(html.contains(r#"<link rel="canonical" href="https://gizza.ai/tools/math/">"#));
        assert!(html.contains(r#"content="https://gizza.ai/tools/math/og.png""#), "per-hub og.png");
        assert!(html.contains(r#"name="description" content="Calculators"#), "blurb as meta description");
        // JSON-LD: member ItemList + Home › Tools › Category breadcrumbs
        assert!(html.contains(r#""@type":"ItemList""#));
        assert!(html.contains(r#""url":"https://gizza.ai/tools/calculator/""#), "member in ItemList");
        assert!(html.contains(r#""@type":"BreadcrumbList""#));
        // serde_json orders keys alphabetically: item precedes name
        assert!(html.contains(r#""item":"https://gizza.ai/tools/math/","name":"Math & statistics tools""#));
        // hero: h1 = category title + blurb, visible breadcrumb back to /tools/
        assert!(html.contains("<h1>Math &amp; statistics tools</h1>"));
        assert!(html.contains(r#"<a href="/tools/">All tools</a>"#), "links back to the landing");
        // member card grid, crawlable
        assert!(html.contains(r#"<a class="tools-card" href="/tools/calculator/""#));
        assert!(html.contains("Free Online Calculator"));
        // sibling nav marks the current hub
        assert!(html.contains(r#"href="/tools/math/" aria-current="page""#));
        // no client-side filter here — its _index.json fetch is landing-relative
        assert!(!html.contains(r#"id="tools-filter""#), "no search filter on hub pages");
        // chrome (branded() sets identity only, so this falls back to generic)
        assert!(html.contains(r#"<header class="tool-nav">"#), "chrome header present");
        assert!(html.contains(r#"<footer class="tool-footer">"#), "chrome footer present");
    }

    #[test]
    fn default_config_renders_a_generic_unbranded_hub_page() {
        let metas = [sample()];
        let hubs = crate::categories::build_hubs(&metas);
        let html = render_category_hub(&SiteConfig::default(), &hubs[0], &hubs);
        assert!(html.contains("<title>Math &amp; statistics tools</title>"), "unsuffixed generic title");
        assert!(!html.contains(r#"<link rel="canonical""#), "no canonical when unbranded");
        assert!(!html.contains("og:image"), "no og:image when unbranded");
        assert!(!html.contains("BreadcrumbList"), "no BreadcrumbList JSON-LD when unbranded");
        assert!(html.contains(r#""@type":"ItemList""#), "ItemList JSON-LD still present");
        assert!(html.contains(r#""url":"/tools/calculator/""#), "ItemList member URL is relative when unbranded");
        assert!(!html.contains("gizza"), "fully generic — no brand leaks");
    }

    #[test]
    fn related_tools_section_renders_between_content_and_dev_access() {
        let related_meta = ToolMeta::from_toml(
            r#"
slug          = "percentage-calculator"
title         = "t"
description   = "Work out percentages instantly."
h1            = "Percentage Calculator"
hero_subtitle = "s"
wasm          = "w"
export        = "run"
output_label  = "o"
format        = "text"
"#,
        )
        .unwrap();
        let related = vec![&related_meta];
        let html =
            render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &related, &[]);
        assert!(html.contains(r#"<section class="tool-related">"#), "related section present");
        assert!(html.contains("<h2>Related tools</h2>"));
        assert!(
            html.contains(r#"<a class="tool-related-link" href="/tools/percentage-calculator/">Percentage Calculator</a>"#),
            "linked title"
        );
        assert!(
            html.contains(r#"<p class="tool-related-desc">Work out percentages instantly.</p>"#),
            "one-line description"
        );
        assert!(html.contains(".tool-related-list"), "related CSS emitted");
        // placement: content → related → Developer & Automation Access
        let content = html.find(r#"class="tool-content""#).unwrap();
        let related_at = html.find(r#"class="tool-related""#).unwrap();
        let dev = html.find("Developer &amp; Automation Access").unwrap();
        assert!(content < related_at && related_at < dev, "related sits between content and dev access");

        // no related tools → neither the section nor its CSS
        let html =
            render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(!html.contains("tool-related"), "no empty related section");
    }

    #[test]
    fn popular_conversions_section_renders_after_content() {
        let pairs = vec![
            PairLink { label: "WAV to MP3".to_string(), href: "/tools/audio-convert/wav-to-mp3/".to_string() },
            PairLink { label: "WAV to FLAC".to_string(), href: "/tools/audio-convert/wav-to-flac/".to_string() },
        ];
        let html =
            render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &pairs);
        assert!(html.contains(r#"<section class="tool-pairs">"#), "pairs section present");
        assert!(html.contains("<h2>Popular conversions</h2>"));
        assert!(
            html.contains(r#"<a class="tool-pairs-link" href="/tools/audio-convert/wav-to-mp3/">WAV to MP3</a>"#),
            "crawlable pair link"
        );
        assert!(html.contains(".tool-pairs-list"), "pairs CSS emitted");
        // placement: content → popular conversions → Developer & Automation Access
        let content = html.find(r#"class="tool-content""#).unwrap();
        let pairs_at = html.find(r#"class="tool-pairs""#).unwrap();
        let dev = html.find("Developer &amp; Automation Access").unwrap();
        assert!(content < pairs_at && pairs_at < dev, "pairs sit between content and dev access");

        // no pairs → neither the section nor its CSS
        let html =
            render_page(&branded(), &sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(!html.contains("tool-pairs"), "no empty pairs section");
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

    fn declarative_sample() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "age-calculator"
title         = "Age Calculator"
description   = "d"
h1            = "Age Calculator"
hero_subtitle = "s"
wasm          = "gizza_ai_age_calculator_web"
export        = "run"
output_label  = "Age"
format        = "text"
wide          = true

[[input]]
name        = "birthdate"
label       = "Date of birth"
source      = "field"
kind        = "date"

[[input]]
name        = "as_of"
label       = "As of"
source      = "field"
kind        = "date"
default     = "today"

[[input]]
name        = "from"
label       = "From zone"
source      = "field"
options     = "timezones"

[[example]]
label  = "Born 1990-04-15"
params = { birthdate = "1990-04-15" }
"#,
        )
        .unwrap()
    }

    #[test]
    fn renders_declarative_controls_examples_and_widget_chrome() {
        let html = render_page(&branded(), &declarative_sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        // meta kind="date" → native picker, no per-tool JS type-swapping
        assert!(html.contains(r#"id="in-birthdate" class="tool-input" type="date""#), "date picker rendered");
        // meta options="timezones" → datalist autocomplete
        assert!(html.contains(r#"list="dl-from""#), "input references its datalist");
        assert!(html.contains(r#"<datalist id="dl-from">"#), "datalist rendered");
        assert!(html.contains(r#"<option value="Europe/Amsterdam">"#), "vocab options present");
        // [[example]] → one-click chip wired by data-example index
        assert!(html.contains("tool-example-chip"), "example chip rendered");
        assert!(html.contains(r#"data-example="0""#), "chip carries its example index");
        assert!(html.contains("Born 1990-04-15"), "chip shows the example label");
        // wide widgets are a class, not a JS style override
        assert!(html.contains("tool-widget tool-widget--wide"), "wide widget class");
        // standard chrome: reset + copy-result on every field/text tool
        assert!(html.contains(r#"id="tool-reset""#), "reset button rendered");
        assert!(html.contains(r#"id="tool-copy-output""#), "copy-result button rendered");
        // text tools also get a Download link for the current output, hidden
        // until tool.js fills it (competitor-standard export affordance)
        assert!(
            html.contains(r#"data-text-download download="age-calculator-output.txt""#),
            "text-output download link rendered with slug-derived filename"
        );
        // client config carries the declarative default + examples for tool.js
        assert!(html.contains(r#""default":"today""#), "input default in client config");
        assert!(html.contains(r#""examples":[{"label":"Born 1990-04-15""#), "examples in client config");
    }

    #[test]
    fn renders_slider_kind_and_step_any_number() {
        let meta = ToolMeta::from_toml(
            r#"
slug          = "audio-pitch-shift"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "o"
format        = "audio"

[[input]]
name   = "audio"
source = "file"
accept = "audio/*"

[[input]]
name        = "semitones"
source      = "field"
label       = "Semitones"
placeholder = "3"
kind        = "slider"

[[input]]
name        = "gain"
source      = "field"
label       = "Gain"
"#,
        )
        .unwrap();
        let schema = ParamSchema::from_props_for_tests(serde_json::json!({
            "semitones": { "type": "number", "minimum": -24, "maximum": 24 },
            "gain": { "type": "number", "minimum": -60, "maximum": 60 }
        }));
        let html = render_page(&branded(), &meta, "<h2>About</h2>", &schema, false, false, &[], &[]);
        // kind="slider" → a range input mirroring the canonical number box
        assert!(html.contains(r#"id="in-semitones-slider""#), "slider rendered");
        assert!(html.contains(r#"data-for="in-semitones""#), "slider targets its number box");
        assert!(
            html.contains(r#"min="-24" max="24" step="1""#),
            "slider carries schema bounds and default step"
        );
        assert!(html.contains(r#"id="in-semitones""#), "canonical number input kept");
        // number-typed params accept fractions without step-mismatch warnings
        assert!(html.contains(r#"step="any""#), "fractional step on number inputs");
        // the plain number field (no kind) has no slider
        assert!(!html.contains(r#"id="in-gain-slider""#), "no slider without kind=slider");
        // copy-paste-runnable CLI example includes the field samples
        assert!(
            html.contains("gizza tool audio-pitch-shift 'url=https://example.com/input' 'semitones=3'"),
            "CLI example includes required field sample"
        );
    }

    #[test]
    fn renders_color_kind_as_swatch_plus_canonical_text() {
        let meta = ToolMeta::from_toml(
            r##"
slug          = "waveform-image"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "o"
format        = "image"

[[input]]
name   = "audio"
source = "file"
accept = "audio/*"

[[input]]
name        = "color"
source      = "field"
label       = "Wave color"
placeholder = "#4f46e5"
kind        = "color"

[[input]]
name        = "background"
source      = "field"
label       = "Background"
placeholder = "transparent — or a hex like #0b1220"
kind        = "color"
"##,
        )
        .unwrap();
        let schema = ParamSchema::from_props_for_tests(serde_json::json!({
            "color": { "type": "string", "default": "#4f46e5" },
            "background": { "type": "string" }
        }));
        let html = render_page(&branded(), &meta, "<h2>About</h2>", &schema, false, false, &[], &[]);
        // kind="color" → a native swatch mirroring the canonical TEXT input
        assert!(html.contains(r##"id="in-color-swatch""##), "swatch rendered");
        assert!(html.contains(r##"data-for="in-color""##), "swatch targets its text input");
        assert!(
            html.contains(r##"id="in-color" class="tool-input tool-color-value" type="text""##),
            "canonical text input kept (empty/alpha/lists stay expressible)"
        );
        // schema default seeds the swatch; a default-less color has no value attr
        assert!(html.contains(r##"id="in-color-swatch" class="tool-color-swatch" type="color" data-for="in-color" value="#4f46e5""##), "swatch seeded with schema default");
        assert!(!html.contains(r##"data-for="in-background" value="##), "no swatch value without a schema default");
        // the CLI example uses the hex sample for color and OMITS background
        // (its non-hex placeholder means "empty = transparent" is the example)
        assert!(
            html.contains("gizza tool waveform-image 'url=https://example.com/input' 'color=#4f46e5'"),
            "CLI example is runnable: hex sample, placeholder-leak omitted"
        );
    }

    #[test]
    fn select_labels_enrich_display_text_but_keep_canonical_values() {
        let meta = ToolMeta::from_toml(
            r#"
slug          = "video-aspect-pad"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "o"
format        = "video"

[[input]]
name   = "video"
source = "file"
accept = "video/*"

[[input]]
name   = "aspect"
source = "field"
label  = "Target aspect ratio"
labels = { "9:16" = "9:16 — Reels / Shorts / TikTok (1080×1920)", "1:1" = "1:1 — square feed (1080×1080)" }
"#,
        )
        .unwrap();
        let schema = ParamSchema::from_props_for_tests(serde_json::json!({
            "aspect": { "type": "string", "enum": ["9:16", "1:1", "16:9"], "default": "9:16" }
        }));
        let html = render_page(&branded(), &meta, "<h2>About</h2>", &schema, false, false, &[], &[]);
        // labeled options: canonical value attr + enriched visible text
        assert!(
            html.contains(r#"<option value="9:16" selected>9:16 — Reels / Shorts / TikTok (1080×1920)</option>"#),
            "labeled default option keeps canonical value"
        );
        assert!(
            html.contains(r#"<option value="1:1">1:1 — square feed (1080×1080)</option>"#),
            "labeled non-default option"
        );
        // an unlabeled option falls back to its value as the text
        assert!(html.contains(r#"<option value="16:9">16:9</option>"#), "unlabeled option unchanged");
    }

    #[test]
    fn media_tools_get_reset_but_not_copy_result() {
        let html = render_page(&branded(), &ffmpeg_sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains(r#"id="tool-reset""#), "reset present (has fields)");
        assert!(!html.contains(r#"id="tool-copy-output""#), "no copy-result on media output");
    }

    #[test]
    fn copy_image_button_only_on_image_output() {
        // ffmpeg_sample() is format="image" → the Copy-image button is rendered
        // (hidden, next to the download link), so a result can be copied to the
        // clipboard as PNG by tool.js.
        let image_html = render_page(&branded(), &ffmpeg_sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            image_html.contains(r#"id="tool-copy-image""#),
            "image-output page renders the Copy-image button"
        );
        assert!(
            image_html.contains(r#"button id="tool-copy-image" class="tool-widget-btn" type="button" hidden"#),
            "Copy-image button is a hidden widget button until a result renders"
        );

        // Video output (select_labels_… sample is format="video") must NOT get it:
        // there is no reliable ClipboardItem path for video.
        let video_meta = ToolMeta::from_toml(
            r#"
slug          = "video-mute"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "o"
format        = "video"

[[input]]
name   = "video"
source = "file"
accept = "video/*"
"#,
        )
        .unwrap();
        let video_html = render_page(&branded(), &video_meta, "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            !video_html.contains(r#"id="tool-copy-image""#),
            "video-output page must not render the Copy-image button"
        );

        // Audio output must NOT get it either.
        let audio_meta = ToolMeta::from_toml(
            r#"
slug          = "audio-normalize"
title         = "t"
description   = "d"
h1            = "h"
hero_subtitle = "s"
wasm          = "w"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "o"
format        = "audio"

[[input]]
name   = "audio"
source = "file"
accept = "audio/*"
"#,
        )
        .unwrap();
        let audio_html = render_page(&branded(), &audio_meta, "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            !audio_html.contains(r#"id="tool-copy-image""#),
            "audio-output page must not render the Copy-image button"
        );

        // Text output (declarative_sample is format="text") must NOT get it.
        let text_html = render_page(&branded(), &declarative_sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            !text_html.contains(r#"id="tool-copy-image""#),
            "text-output page must not render the Copy-image button"
        );
    }

    #[test]
    fn renders_file_input_and_media_output() {
        let html = render_page(&branded(), &ffmpeg_sample(), "<h2>About</h2>", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains(r#"type="file""#), "file input present");
        assert!(html.contains(r#"id="in-image""#), "file input id");
        assert!(html.contains(r#"accept="image/*""#), "accept attr");
        assert!(html.contains(r#"id="in-width""#), "field input still present");
        assert!(html.contains(r#"id="tool-output-media""#), "media output element");
        assert!(html.contains(r#"id="tool-output-download""#), "download link");
        assert!(html.contains(r#"id="tool-output""#), "status output for errors");
    }

    #[test]
    fn svg_format_renders_an_img_and_download() {
        let mut meta = sample();
        meta.format = "svg".to_string();
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            html.contains(r#"<img id="tool-output-media""#),
            "svg output renders the <img> media element"
        );
        assert!(
            html.contains(r#"id="tool-output-download""#),
            "svg output offers a download link"
        );
    }

    #[test]
    fn svg_format_keeps_copy_result_and_omits_copy_image() {
        // The SVG source is what an SVG user wants on the clipboard. "Copy image"
        // is deliberately NOT offered: ClipboardItem is reliably PNG-only and
        // canvas-drawing an SVG varies by browser.
        let mut meta = sample();
        meta.format = "svg".to_string();
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains(r#"id="tool-copy-output""#), "Copy result is offered for svg");
        assert!(!html.contains(r#"id="tool-copy-image""#), "Copy image is not offered for svg");
    }

    #[test]
    fn image_format_still_offers_copy_image() {
        let mut meta = sample();
        meta.format = "image".to_string();
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains(r#"id="tool-copy-image""#), "raster image output keeps Copy image");
        assert!(!html.contains(r#"id="tool-copy-output""#), "raster image output has no Copy result");
    }

    #[test]
    fn svg_format_with_no_field_inputs_still_offers_copy_output() {
        // svg is is_media == true but is_binary_media == false, so the outer
        // widget-actions gate must not be keyed on is_media alone — an svg tool
        // with zero field inputs (e.g. no `source = "field"` input at all) must
        // still render #tool-copy-output per the interface contract.
        let mut meta = sample();
        meta.format = "svg".to_string();
        meta.inputs.clear();
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(
            html.contains(r#"id="tool-copy-output""#),
            "Copy result is offered for svg even with no field inputs"
        );
    }

    #[test]
    fn network_flag_renders_the_disclosure_badge() {
        let mut meta = sample();
        meta.network = true;
        let html = render_page(&branded(), &meta, "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(html.contains("tool-network-note"), "badge element is rendered");
        assert!(
            html.contains("makes a real network request"),
            "badge states that the request is real"
        );
        assert!(
            html.contains("must allow cross-origin access"),
            "badge sets the CORS expectation before the user runs the tool"
        );
    }

    #[test]
    fn pages_without_the_network_flag_have_no_badge() {
        let html = render_page(&branded(), &sample(), "", &ParamSchema::empty(), false, false, &[], &[]);
        assert!(!html.contains("tool-network-note"), "no badge on a local-only tool");
    }
}
