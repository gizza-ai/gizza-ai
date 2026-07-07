//! gizza-tool-pages — renders standalone static pages for every tool that has
//! a `blocks/<tool>/page/meta.toml`, into `pkg/tools/<tool>/`.
//!
//! Usage: `gizza-tool-pages <repo_root>` (defaults to current dir).
//! Assumes each tool's wasm-pack output already exists at
//! `blocks/<tool>/web/pkg/`.

mod categories;
mod control;
mod descriptor;
mod faq;
mod index;
mod markdown;
mod meta;
mod og;
mod related;
mod template;
mod vocab;

use std::fs;
use std::path::{Path, PathBuf};

use meta::ToolMeta;

fn main() {
    if let Err(e) = run() {
        eprintln!("gizza-tool-pages: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let root = std::env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let root = PathBuf::from(root);
    let blocks = root.join("blocks");
    let pkg_tools = root.join("pkg").join("tools");

    let metas = collect_tool_metas(&blocks)?;
    if metas.is_empty() {
        eprintln!("no tool pages found (no blocks/*/page/meta.toml)");
    }

    // Category hub pages share the /tools/<slug>/ namespace with tool pages —
    // fail the build loudly if a block slug ever collides with a category.
    let block_slugs = collect_block_slugs(&blocks)?;
    categories::check_slug_collisions(block_slugs.iter().map(String::as_str))?;

    let og_renderer = og::OgRenderer::new();
    let metas_only: Vec<ToolMeta> = metas.iter().map(|(_, m)| m.clone()).collect();

    for (tool_dir, m) in &metas {
        let out = pkg_tools.join(&m.slug);
        fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;

        let content_md = fs::read_to_string(tool_dir.join("page/content.md"))
            .map_err(|e| format!("read content.md for {}: {e}", m.slug))?;
        let content_html = render_markdown(&content_md);
        // Resolve each field's control (select/checkbox/number/text) from the
        // tool's declared schema so the page form mirrors the descriptor.
        let schema = control::ParamSchema::load(tool_dir);
        // Optional per-tool escape hatch: page/custom.js (+ custom.css) are copied
        // next to the page; the shared driver imports custom.js via cfg.custom.
        let custom_js = tool_dir.join("page/custom.js");
        let custom_css = tool_dir.join("page/custom.css");
        let has_custom_js = custom_js.is_file();
        let has_custom_css = custom_css.is_file();
        if has_custom_js {
            copy_file(&custom_js, &out.join("custom.js"))?;
        }
        if has_custom_css {
            copy_file(&custom_css, &out.join("custom.css"))?;
        }
        // Top-5 related tools by shared tags — internal links on the page and
        // its markdown twin.
        let related = related::related_tools(m, &metas_only);
        let html = template::render_page(
            m,
            &content_html,
            &schema,
            has_custom_js,
            has_custom_css,
            &related,
        );
        fs::write(out.join("index.html"), html)
            .map_err(|e| format!("write index.html: {e}"))?;
        fs::write(
            out.join("index.md"),
            markdown::tool_markdown(m, &content_md, &schema, &related),
        )
        .map_err(|e| format!("write index.md: {e}"))?;
        // Machine-readable descriptor (identity, URLs, CLI example and the
        // manifest's tool schema) — the agent-facing twin of the page.
        fs::write(
            out.join("tool.json"),
            descriptor::tool_descriptor(m, &schema, descriptor::load_manifest(tool_dir).as_ref()),
        )
        .map_err(|e| format!("write tool.json: {e}"))?;
        // Per-tool Open Graph card — the page's og:image/twitter:image target.
        fs::write(out.join("og.png"), og_renderer.tool_card(m)?)
            .map_err(|e| format!("write og.png: {e}"))?;

        let web_pkg = tool_dir.join("web/pkg");
        let js_path = web_pkg.join(format!("{}.js", m.wasm));
        let wasm_path = web_pkg.join(format!("{}_bg.wasm", m.wasm));
        if js_path.is_file() && wasm_path.is_file() {
            copy_file(&js_path, &out.join(format!("{}.js", m.wasm)))?;
            copy_file(&wasm_path, &out.join(format!("{}_bg.wasm", m.wasm)))?;
        } else {
            eprintln!("warning: web/pkg not found for {} (skipping WASM copy)", m.slug);
        }

        copy_file(&root.join("site/tool.js"), &out.join("tool.js"))?;
        copy_file(&root.join("js/query-prefill.js"), &out.join("query-prefill.js"))?;
        copy_file(&root.join("site/tool.css"), &out.join("tool.css"))?;
        copy_file(&root.join("site/header.css"), &out.join("header.css"))?;
        copy_file(&root.join("site/header.js"), &out.join("header.js"))?;
        copy_file(&root.join("site/tools-index.js"), &out.join("tools-index.js"))?;
        if m.runtime == "ffmpeg" {
            copy_file(&root.join("js/ffmpeg.js"), &out.join("ffmpeg.js"))?;
            copy_file(&root.join("site/tool-ffmpeg.js"), &out.join("tool-ffmpeg.js"))?;
            copy_file(&root.join("site/tool-audio.js"), &out.join("tool-audio.js"))?;
        }
        eprintln!("rendered tools/{}/", m.slug);
    }

    // Static index for the in-app tools modal (fetched client-side; lives under
    // /tools/ so it is covered by the runtime SW's /tools/ bypass).
    fs::write(
        pkg_tools.join("_index.json"),
        index::tools_index_json(&metas_only),
    )
    .map_err(|e| format!("write tools/_index.json: {e}"))?;

    // Category hubs: group the same metas by the fixed taxonomy. Rendered
    // below as /tools/<category>/ pages and linked from the landing nav.
    let hubs = categories::build_hubs(&metas_only);

    // `/tools/` landing page — a build-time card grid of every tool, rendered
    // from the same `metas` as the per-tool pages + `_index.json` (one source of
    // truth, no drift). Its chrome assets are copied alongside so `./header.css`
    // etc. resolve when the page is served at `/tools/`.
    fs::write(
        pkg_tools.join("index.html"),
        template::render_tools_index(&metas_only, &hubs),
    )
    .map_err(|e| format!("write tools/index.html: {e}"))?;
    // Markdown twin of the landing page (the "tools .md page") for LLMs/agents.
    fs::write(
        pkg_tools.join("index.md"),
        index::tools_catalog_md(&metas_only),
    )
    .map_err(|e| format!("write tools/index.md: {e}"))?;
    copy_file(&root.join("site/tool.css"), &pkg_tools.join("tool.css"))?;
    copy_file(&root.join("site/header.css"), &pkg_tools.join("header.css"))?;
    copy_file(&root.join("site/header.js"), &pkg_tools.join("header.js"))?;
    copy_file(&root.join("site/tools-index.js"), &pkg_tools.join("tools-index.js"))?;
    fs::write(pkg_tools.join("og.png"), og_renderer.index_card(metas_only.len())?)
        .map_err(|e| format!("write tools/og.png: {e}"))?;
    eprintln!("rendered tools/ (landing page, {} tools)", metas_only.len());

    // Category hub pages at /tools/<category>/ — a member-card grid with its
    // own SEO head, OG card and chrome assets (same relative-asset pattern as
    // the landing page).
    for hub in &hubs {
        let out = pkg_tools.join(hub.category.slug);
        fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
        fs::write(
            out.join("index.html"),
            template::render_category_hub(hub, &hubs),
        )
        .map_err(|e| format!("write tools/{}/index.html: {e}", hub.category.slug))?;
        fs::write(out.join("og.png"), og_renderer.hub_card(hub.category)?)
            .map_err(|e| format!("write tools/{}/og.png: {e}", hub.category.slug))?;
        for asset in ["tool.css", "header.css", "header.js", "tools-index.js"] {
            copy_file(&root.join("site").join(asset), &out.join(asset))?;
        }
        eprintln!(
            "rendered tools/{}/ (hub, {} tools)",
            hub.category.slug,
            hub.members.len()
        );
    }
    // Machine-readable hub index — consumed by scripts/gen-seo.sh to add the
    // hub URLs to the sitemap.
    fs::write(pkg_tools.join("_hubs.json"), index::hubs_json(&hubs))
        .map_err(|e| format!("write tools/_hubs.json: {e}"))?;

    Ok(())
}

/// Every direct subdirectory name of `blocks/` — the full block-slug
/// namespace, including blocks without a page (a future page would collide
/// with a hub just the same).
fn collect_block_slugs(blocks: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    if !blocks.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(blocks).map_err(|e| format!("read blocks/: {e}"))? {
        let entry = entry.map_err(|e| format!("blocks entry: {e}"))?;
        if entry.path().is_dir() {
            out.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    out.sort();
    Ok(out)
}

/// Find every `blocks/<tool>/page/meta.toml`, parse it, sorted by slug.
fn collect_tool_metas(blocks: &Path) -> Result<Vec<(PathBuf, ToolMeta)>, String> {
    let mut out = Vec::new();
    if !blocks.is_dir() {
        return Ok(out);
    }
    for entry in fs::read_dir(blocks).map_err(|e| format!("read blocks/: {e}"))? {
        let entry = entry.map_err(|e| format!("blocks entry: {e}"))?;
        let meta_path = entry.path().join("page/meta.toml");
        if !meta_path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&meta_path)
            .map_err(|e| format!("read {}: {e}", meta_path.display()))?;
        let m = ToolMeta::from_toml(&text)?;
        out.push((entry.path(), m));
    }
    out.sort_by(|a, b| a.1.slug.cmp(&b.1.slug));
    Ok(out)
}

/// Render markdown to an HTML fragment. GFM tables are enabled so tool prose can
/// lay examples/options out as readable tables.
fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Options, Parser};
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(md, opts);
    let mut buf = String::new();
    html::push_html(&mut buf, parser);
    buf
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))
}
