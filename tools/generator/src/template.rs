//! Renders the option-C tool page: top nav + hero tool + SEO content + footer,
//! with SEO `<head>` tags and JSON-LD.

use crate::meta::ToolMeta;
use maud::{html, PreEscaped, DOCTYPE};

/// Render the full HTML document for a tool page. `content_html` is the
/// markdown-rendered SEO section.
pub fn render_page(meta: &ToolMeta, content_html: &str) -> String {
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

    let markup = html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (meta.title) }
                meta name="description" content=(meta.description);
                link rel="canonical" href=(canonical);
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
                link rel="icon" href="https://gizza.ai/favicon-32.png" sizes="32x32";
                script type="application/ld+json" { (PreEscaped(json_ld)) }
            }
            body {
                header class="tool-nav" {
                    a class="tool-brand" href="https://gizza.ai" {
                        img src="https://gizza.ai/logo.webp" alt="gizza.ai logo";
                        span { "gizza.ai" }
                    }
                    a class="tool-chat-link" href="https://gizza.ai" { "Open AI chat →" }
                }
                main class="tool-main" {
                    section class="tool-hero" {
                        h1 { (meta.h1) }
                        p class="tool-hero-sub" { (meta.hero_subtitle) }
                        div class="tool-widget" {
                            @for input in &meta.inputs {
                                @if input.source == "field" {
                                    label class="tool-field-label" for=(format!("in-{}", input.name)) { (input.label) }
                                    input id=(format!("in-{}", input.name)) class="tool-input"
                                          type="text" placeholder=(input.placeholder)
                                          autocomplete="off" autocapitalize="off" spellcheck="false";
                                }
                            }
                            div class="tool-output-label" { (meta.output_label) }
                            output id="tool-output" class="tool-output" { "" }
                        }
                    }
                    section class="tool-content" {
                        (PreEscaped(content_html))
                    }
                }
                footer class="tool-footer" {
                    div class="tool-footer-brand" {
                        img src="https://gizza.ai/logo.webp" alt="";
                        span { "⚡ Powered by gizza.ai" }
                    }
                    p {
                        strong { "gizza.ai" }
                        " is a free, private AI assistant that runs entirely in your browser — no server, no sign-up, your data never leaves your device. It can chat, run tools like this one, and work with images and video. "
                        a href="https://gizza.ai" { "Visit gizza.ai →" }
                    }
                }
                script { (PreEscaped(format!("window.GIZZA_TOOL = {client_cfg};"))) }
                script type="module" src="./tool.js" {}
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
        let html = render_page(&sample(), "<h2>About</h2>");
        assert!(html.contains("<title>Free Online Calculator — gizza.ai</title>"));
        assert!(html.contains(r#"<link rel="canonical" href="https://gizza.ai/tools/calculator/">"#));
        assert!(html.contains("application/ld+json"));
        assert!(html.contains(r#"id="in-expr""#));
        assert!(html.contains(r#"id="tool-output""#));
        assert!(html.contains("window.GIZZA_TOOL"));
        assert!(html.contains("Powered by gizza.ai"));
        assert!(html.contains("<h2>About</h2>"));
    }
}
