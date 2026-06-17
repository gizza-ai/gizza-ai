//! gizza-tool-pages — renders standalone static pages for every tool that has
//! a `blocks/<tool>/page/meta.toml`, into `pkg/tools/<tool>/`.
//!
//! Usage: `gizza-tool-pages <repo_root>` (defaults to current dir).
//! Assumes each tool's wasm-pack output already exists at
//! `blocks/<tool>/web/pkg/`.

mod index;
mod markdown;
mod meta;
mod seo;
mod template;

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

    for (tool_dir, m) in &metas {
        let out = pkg_tools.join(&m.slug);
        fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;

        let content_md = fs::read_to_string(tool_dir.join("page/content.md"))
            .map_err(|e| format!("read content.md for {}: {e}", m.slug))?;
        let content_html = render_markdown(&content_md);
        let html = template::render_page(m, &content_html);
        fs::write(out.join("index.html"), html)
            .map_err(|e| format!("write index.html: {e}"))?;
        fs::write(out.join("index.md"), markdown::tool_markdown(m, &content_md))
            .map_err(|e| format!("write index.md: {e}"))?;

        let web_pkg = tool_dir.join("web/pkg");
        copy_file(&web_pkg.join(format!("{}.js", m.wasm)), &out.join(format!("{}.js", m.wasm)))?;
        copy_file(
            &web_pkg.join(format!("{}_bg.wasm", m.wasm)),
            &out.join(format!("{}_bg.wasm", m.wasm)),
        )?;

        copy_file(&root.join("site/tool.js"), &out.join("tool.js"))?;
        copy_file(&root.join("site/tool.css"), &out.join("tool.css"))?;
        if m.runtime == "ffmpeg" {
            copy_file(&root.join("js/ffmpeg.js"), &out.join("ffmpeg.js"))?;
            copy_file(&root.join("site/tool-ffmpeg.js"), &out.join("tool-ffmpeg.js"))?;
        }
        eprintln!("rendered tools/{}/", m.slug);
    }

    // Static index for the in-app tools modal (fetched client-side; lives under
    // /tools/ so it is covered by the runtime SW's /tools/ bypass).
    let metas_only: Vec<ToolMeta> = metas.iter().map(|(_, m)| m.clone()).collect();
    fs::write(
        pkg_tools.join("_index.json"),
        index::tools_index_json(&metas_only),
    )
    .map_err(|e| format!("write tools/_index.json: {e}"))?;

    let slugs: Vec<String> = metas.iter().map(|(_, m)| m.slug.clone()).collect();
    let pkg = root.join("pkg");
    fs::write(pkg.join("sitemap.xml"), seo::sitemap(&slugs))
        .map_err(|e| format!("write sitemap.xml: {e}"))?;
    fs::write(pkg.join("robots.txt"), seo::robots())
        .map_err(|e| format!("write robots.txt: {e}"))?;

    Ok(())
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

/// Render markdown to an HTML fragment.
fn render_markdown(md: &str) -> String {
    use pulldown_cmark::{html, Parser};
    let parser = Parser::new(md);
    let mut buf = String::new();
    html::push_html(&mut buf, parser);
    buf
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))
}
