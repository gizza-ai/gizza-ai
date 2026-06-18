// gizza-chrome — shared header/footer/icon chrome for gizza.ai.
//
// Both consumers:
//   - the chat app (`src/blocks/ui.rs`, wasm32 via maud default-features=false)
//   - the static tool-page generator (`tools/generator/src/template.rs`, native)
// add this crate as a path dep and call `header(brand, active)` / `footer()`.

use maud::{html, Markup, PreEscaped};

/// Wrap a `&'static str` SVG literal into a `Markup` (`PreEscaped<String>`).
/// maud 0.26 with `default-features = false` uses `Markup = PreEscaped<String>`,
/// so we must convert from `&str`.
fn svg(s: &str) -> Markup {
    PreEscaped(s.to_string())
}

// ── Active section marker ─────────────────────────────────────────────────────

/// Which section of the site is currently active. Used to highlight the
/// appropriate nav item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Active {
    /// The apex `/` chat application.
    Chat,
    /// A `/tools/<slug>/` standalone tool page.
    Tool,
    /// No specific section (e.g. other pages).
    None,
}

// ── Icon helpers ──────────────────────────────────────────────────────────────
//
// Functional/category icons use Lucide SVG paths (MIT licensed).
// GitHub and Discord use their official brand-mark SVGs — Lucide has
// deprecated brand icons, so we embed the authoritative paths directly.

/// GitHub brand-mark SVG (official mark, not Lucide).
pub fn icon_github() -> Markup {
    svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 0C5.374 0 0 5.373 0 12c0 5.302 3.438 9.8 8.207 11.387.599.111.793-.261.793-.577v-2.234c-3.338.726-4.033-1.416-4.033-1.416-.546-1.387-1.333-1.756-1.333-1.756-1.089-.745.083-.729.083-.729 1.205.084 1.839 1.237 1.839 1.237 1.07 1.834 2.807 1.304 3.492.997.107-.775.418-1.305.762-1.604-2.665-.305-5.467-1.334-5.467-5.931 0-1.311.469-2.381 1.236-3.221-.124-.303-.535-1.524.117-3.176 0 0 1.008-.322 3.301 1.23A11.509 11.509 0 0 1 12 5.803c1.02.005 2.047.138 3.006.404 2.291-1.552 3.297-1.23 3.297-1.23.653 1.653.242 2.874.118 3.176.77.84 1.235 1.911 1.235 3.221 0 4.609-2.807 5.624-5.479 5.921.43.372.823 1.102.823 2.222v3.293c0 .319.192.694.801.576C20.566 21.797 24 17.3 24 12c0-6.627-5.373-12-12-12z"/></svg>"#)
}

/// Discord brand-mark SVG (official mark, not Lucide).
pub fn icon_discord() -> Markup {
    svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M20.317 4.37a19.791 19.791 0 0 0-4.885-1.515.074.074 0 0 0-.079.037c-.21.375-.444.864-.608 1.25a18.27 18.27 0 0 0-5.487 0 12.64 12.64 0 0 0-.617-1.25.077.077 0 0 0-.079-.037A19.736 19.736 0 0 0 3.677 4.37a.07.07 0 0 0-.032.027C.533 9.046-.32 13.58.099 18.057a.082.082 0 0 0 .031.057 19.9 19.9 0 0 0 5.993 3.03.078.078 0 0 0 .084-.028 14.09 14.09 0 0 0 1.226-1.994.076.076 0 0 0-.041-.106 13.107 13.107 0 0 1-1.872-.892.077.077 0 0 1-.008-.128 10.2 10.2 0 0 0 .372-.292.074.074 0 0 1 .077-.01c3.928 1.793 8.18 1.793 12.062 0a.074.074 0 0 1 .078.01c.12.098.246.198.373.292a.077.077 0 0 1-.006.127 12.299 12.299 0 0 1-1.873.892.077.077 0 0 0-.041.107c.36.698.772 1.362 1.225 1.993a.076.076 0 0 0 .084.028 19.839 19.839 0 0 0 6.002-3.03.077.077 0 0 0 .032-.054c.5-5.177-.838-9.674-3.549-13.66a.061.061 0 0 0-.031-.03zM8.02 15.33c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.956-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.956 2.418-2.157 2.418zm7.975 0c-1.183 0-2.157-1.085-2.157-2.419 0-1.333.955-2.419 2.157-2.419 1.21 0 2.176 1.096 2.157 2.42 0 1.333-.946 2.418-2.157 2.418z"/></svg>"#)
}

/// Lucide Search icon (functional icon).
pub fn icon_search() -> Markup {
    svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="11" cy="11" r="8"/><path d="m21 21-4.35-4.35"/></svg>"#)
}

/// Lucide ChevronDown icon (mega-menu trigger indicator).
pub fn icon_chevron_down() -> Markup {
    svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="m6 9 6 6 6-6"/></svg>"#)
}

/// Lucide Info icon (About link in mega-menu Resources column).
pub fn icon_info() -> Markup {
    svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><circle cx="12" cy="12" r="10"/><path d="M12 16v-4"/><path d="M12 8h.01"/></svg>"#)
}

/// Lucide Terminal icon (CLI link in mega-menu Resources column).
pub fn icon_terminal() -> Markup {
    svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>"#)
}

/// Lucide Bot icon (SKILL.md / agents link in mega-menu Resources column).
pub fn icon_bot() -> Markup {
    svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="M12 8V4H8"/><rect width="16" height="12" x="4" y="8" rx="2"/><path d="M2 14h2"/><path d="M20 14h2"/><path d="M15 13v2"/><path d="M9 13v2"/></svg>"#)
}

// ── Header ────────────────────────────────────────────────────────────────────

/// Shared sticky site header.
///
/// `brand` — caller-supplied brand block (the chat app passes its animated
/// mascot; tool pages pass a static logo link). This keeps the right-hand nav,
/// mega-menu, and icon links identical across both surfaces.
///
/// `active` — marks the current section for nav highlighting.
pub fn header(brand: Markup, active: Active) -> Markup {
    let chat_class = if active == Active::Chat { "nav-link nav-link--active" } else { "nav-link" };
    let tool_class = if active == Active::Tool { "nav-link nav-link--active" } else { "nav-link" };

    html! {
        header.site-header {
            // ── Left: caller-supplied brand ────────────────────────────────
            .site-header__brand {
                (brand)
            }

            // ── Right: nav + mega-menu + standalone icon links ─────────────
            nav.site-header__nav aria-label="Main navigation" {
                // Active-section text links
                a.(chat_class) href="/" { "Chat" }
                a.(tool_class) href="/tools/" { "Tools" }

                // ── Explore mega-menu trigger ──────────────────────────────
                .mega-menu-wrapper {
                    button #explore-trigger
                        .mega-menu__trigger
                        type="button"
                        aria-haspopup="true"
                        aria-expanded="false"
                        aria-controls="explore-panel"
                    {
                        "Explore"
                        (icon_chevron_down())
                    }

                    // ── Mega-menu panel ────────────────────────────────────
                    .mega-menu #explore-panel role="dialog" aria-label="Explore panel" {
                        // Column 1: Tools search
                        //   header.js fetches /tools/_index.json on first open,
                        //   runs filterTools() on input, renders ≤8 windowed rows
                        //   into #tools-results.
                        .mega-menu__col.mega-menu__col--tools {
                            h3.mega-menu__col-title { "Tools" }
                            .mega-menu__search-wrap {
                                (icon_search())
                                input
                                    #tools-search
                                    type="search"
                                    placeholder="Search tools…"
                                    autocomplete="off"
                                    aria-label="Search tools"
                                    {}
                            }
                            ul #tools-results .mega-menu__results aria-live="polite" aria-label="Tool results" {}
                        }

                        // Column 2: Resources
                        .mega-menu__col.mega-menu__col--resources {
                            h3.mega-menu__col-title { "Resources" }
                            ul.mega-menu__resource-list {
                                li {
                                    a.mega-menu__resource-link
                                        href="https://github.com/suppers-ai/gizza-ai"
                                        target="_blank"
                                        rel="noopener noreferrer"
                                    {
                                        .mega-menu__resource-icon { (icon_github()) }
                                        .mega-menu__resource-text {
                                            span.mega-menu__resource-title { "GitHub" }
                                            span.mega-menu__resource-subtitle { "Source & issues" }
                                        }
                                    }
                                }
                                li {
                                    a.mega-menu__resource-link
                                        href="https://discord.gg/gizza"
                                        target="_blank"
                                        rel="noopener noreferrer"
                                    {
                                        .mega-menu__resource-icon { (icon_discord()) }
                                        .mega-menu__resource-text {
                                            span.mega-menu__resource-title { "Discord" }
                                            span.mega-menu__resource-subtitle { "Community" }
                                        }
                                    }
                                }
                                li {
                                    a.mega-menu__resource-link
                                        href="https://github.com/suppers-ai/gizza-ai/blob/main/cli/README.md"
                                        target="_blank"
                                        rel="noopener noreferrer"
                                    {
                                        .mega-menu__resource-icon { (icon_terminal()) }
                                        .mega-menu__resource-text {
                                            span.mega-menu__resource-title { "CLI" }
                                            span.mega-menu__resource-subtitle { "Run tools locally" }
                                        }
                                    }
                                }
                                li {
                                    a.mega-menu__resource-link
                                        href="https://github.com/suppers-ai/gizza-ai/blob/main/SKILL.md"
                                        target="_blank"
                                        rel="noopener noreferrer"
                                    {
                                        .mega-menu__resource-icon { (icon_bot()) }
                                        .mega-menu__resource-text {
                                            span.mega-menu__resource-title { "SKILL.md" }
                                            span.mega-menu__resource-subtitle { "For AI agents" }
                                        }
                                    }
                                }
                                li {
                                    a.mega-menu__resource-link href="/about" {
                                        .mega-menu__resource-icon { (icon_info()) }
                                        .mega-menu__resource-text {
                                            span.mega-menu__resource-title { "About" }
                                            span.mega-menu__resource-subtitle { "What is gizza?" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // ── Standalone GitHub + Discord icon links (always visible) ──
                a.site-header__icon-link
                    href="https://github.com/suppers-ai/gizza-ai"
                    target="_blank"
                    rel="noopener noreferrer"
                    aria-label="GitHub repository"
                {
                    (icon_github())
                }
                a.site-header__icon-link
                    href="https://discord.gg/gizza"
                    target="_blank"
                    rel="noopener noreferrer"
                    aria-label="Discord community"
                {
                    (icon_discord())
                }
            }
        }
    }
}

// ── Footer ────────────────────────────────────────────────────────────────────

/// Shared site footer.
///
/// Renders on standalone tool pages. The chat app does NOT render a footer
/// (it is a viewport-filling app; all links are reachable via the header's
/// Explore dropdown).
pub fn footer() -> Markup {
    html! {
        footer.site-footer {
            .site-footer__brand {
                a.site-footer__logo href="/" aria-label="gizza.ai home" {
                    "gizza.ai"
                }
                p.site-footer__blurb {
                    "A free, private AI assistant and tool platform. "
                    "Everything runs in your browser — your data never leaves your device."
                }
            }

            .site-footer__cols {
                // Tools column
                nav.site-footer__col aria-label="Tools links" {
                    h4.site-footer__col-title { "Tools" }
                    ul {
                        li { a href="/tools/" { "All tools" } }
                        li { a href="/tools/calculator/" { "Calculator" } }
                        li { a href="/tools/image-resize/" { "Image resize" } }
                        li { a href="/tools/word-count/" { "Word count" } }
                    }
                }

                // Resources column
                nav.site-footer__col aria-label="Resources links" {
                    h4.site-footer__col-title { "Resources" }
                    ul {
                        li {
                            a href="https://github.com/suppers-ai/gizza-ai"
                                target="_blank" rel="noopener noreferrer"
                            { "GitHub" }
                        }
                        li {
                            a href="https://discord.gg/gizza"
                                target="_blank" rel="noopener noreferrer"
                            { "Discord" }
                        }
                        li {
                            a href="https://github.com/suppers-ai/gizza-ai/blob/main/cli/README.md"
                                target="_blank" rel="noopener noreferrer"
                            { "CLI" }
                        }
                        li {
                            a href="https://github.com/suppers-ai/gizza-ai/blob/main/SKILL.md"
                                target="_blank" rel="noopener noreferrer"
                            { "SKILL.md" }
                        }
                    }
                }
            }
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── Required brief tests (verbatim) ──────────────────────────────────────

    #[test]
    fn header_has_brand_passthrough_and_nav() {
        let h = header(maud::html! { span.brand-test { "BRANDX" } }, Active::Chat).into_string();
        assert!(h.contains("BRANDX"));                     // caller brand passed through
        assert!(h.contains("github.com"));                 // GitHub link
        assert!(h.contains("discord"));                    // Discord link
        assert!(h.contains("id=\"tools-search\""));        // Tools search input
        assert!(h.contains("id=\"tools-results\""));       // results container header.js fills
        assert!(h.contains("Explore"));                    // mega-menu trigger label
    }

    #[test]
    fn footer_has_blurb_and_columns() {
        let f = footer().into_string();
        assert!(f.contains("free") && f.contains("private")); // the existing blurb words
        assert!(f.contains("Tools") && f.contains("Resources"));
    }

    #[test]
    fn icons_return_svg() {
        for s in [icon_github(), icon_discord(), icon_search()] {
            assert!(s.into_string().contains("<svg"));
        }
    }

    // ── Extra coverage ────────────────────────────────────────────────────────

    #[test]
    fn header_active_chat_sets_active_class() {
        let h = header(maud::html! { "brand" }, Active::Chat).into_string();
        assert!(h.contains("nav-link--active"));
    }

    #[test]
    fn header_active_none_no_active_class() {
        let h = header(maud::html! { "brand" }, Active::None).into_string();
        assert!(!h.contains("nav-link--active"));
    }

    #[test]
    fn header_has_explore_trigger_id() {
        let h = header(maud::html! { "brand" }, Active::None).into_string();
        assert!(h.contains("id=\"explore-trigger\""));
    }

    #[test]
    fn all_icon_helpers_return_svg() {
        let icons = [
            icon_github(),
            icon_discord(),
            icon_search(),
            icon_chevron_down(),
            icon_info(),
            icon_terminal(),
            icon_bot(),
        ];
        for icon in icons {
            let s = icon.into_string();
            assert!(s.contains("<svg"), "icon did not contain <svg: {s}");
            assert!(s.contains("</svg>"), "icon did not contain </svg>: {s}");
        }
    }

    #[test]
    fn footer_has_github_and_discord_links() {
        let f = footer().into_string();
        assert!(f.contains("github.com"));
        assert!(f.contains("discord"));
    }
}
