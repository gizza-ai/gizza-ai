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
mod formats;
mod index;
mod markdown;
mod meta;
mod og;
mod pairs;
mod related;
mod site;
mod template;
mod vocab;

use std::fs;
use std::path::{Path, PathBuf};

use meta::ToolMeta;

const TRANSFORMERS_RUNTIME_FILES: &[&str] = &["transformers.min.js"];
const ONNX_RUNTIME_FILES: &[&str] = &[
    "ort-wasm-simd-threaded.jsep.mjs",
    "ort-wasm-simd-threaded.jsep.wasm",
    "ort-wasm-simd-threaded.asyncify.mjs",
    "ort-wasm-simd-threaded.asyncify.wasm",
    "ort-wasm-simd-threaded.mjs",
    "ort-wasm-simd-threaded.wasm",
];

/// SD-Turbo's own ONNX Runtime (onnxruntime-web 1.31.1, npm alias
/// `onnxruntime-web-sd`), published under `/tools/_model-runtime/ort-sd/`.
/// Its WebGPU build loads only the asyncify factory + binary.
const SD_ORT_FILES: &[&str] = &["ort.webgpu.min.mjs", "ort-wasm-simd-threaded.asyncify.mjs"];
/// The asyncify binary is 27.4 MiB, over Cloudflare Pages' 25 MiB per-file
/// limit, so it ships gzipped (~7 MB) as `<name>.gz` and never raw: sd-worker.js
/// inflates it with DecompressionStream and hands ORT the bytes as
/// `env.wasm.wasmBinary`, so ORT never fetches a `.wasm` itself.
const SD_ORT_GZIPPED: &[&str] = &["ort-wasm-simd-threaded.asyncify.wasm"];
/// Worker-side modules published next to model-worker.js.
const MODEL_RUNTIME_MODULES: &[&str] = &[
    "model-worker.js",
    "model-download-progress.js",
    "janus-builds.js",
    "sd-worker.js",
    "sd-core.js",
];
/// Cloudflare Pages rejects any single file larger than this.
const PAGES_MAX_FILE_BYTES: u64 = 25 * 1024 * 1024;

/// Runtime modules published once under `/tools/_runtime/` so pages other than
/// a tool's own page (e.g. an image editor) can drive any published tool by its
/// `tool.json` `page` spec with the exact code the tool page runs. Model pages
/// resolve their worker at `../_model-runtime/` from here as from a tool dir.
const SHARED_RUNTIME_FILES: &[&str] = &[
    "tool-run.js",
    "tool-fields.js",
    "tool-ffmpeg.js",
    "tool-model.js",
    "custom-model.js",
    // custom-model.js's Janus builds, the same module model-worker.js loads.
    "janus-builds.js",
    "image-models.js",
    "tool-crop-box.js",
    "tool-crop-box.css",
];

fn main() {
    if let Err(e) = run() {
        eprintln!("gizza-tool-pages: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut root = String::from(".");
    let mut cfg_path: Option<String> = None;
    let mut out_dir: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--site-config" => cfg_path = args.next(),
            "--out" => out_dir = args.next(),
            _ => root = a,
        }
    }
    let root = PathBuf::from(root);
    let cfg = match cfg_path {
        Some(p) => site::SiteConfig::load(Path::new(&p))?,
        None => site::SiteConfig::default(),
    };
    let blocks = root.join("blocks");
    let pkg_tools = out_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("pkg").join("tools"));
    let runtime = root.join("tools/generator/assets/runtime");

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

    // Model pages share one self-hosted Transformers.js + ONNX Runtime bundle
    // under /tools/_model-runtime/. Model weights themselves remain lazy and
    // browser-cached, so ordinary tool pages never pay this cost and multiple
    // AI tools do not duplicate a ~22 MB WASM runtime per page.
    if metas.iter().any(|(_, m)| m.runtime == "model") {
        let model_runtime = pkg_tools.join("_model-runtime");
        fs::create_dir_all(&model_runtime)
            .map_err(|e| format!("mkdir {}: {e}", model_runtime.display()))?;
        let transformers_dist = root.join("node_modules/@huggingface/transformers/dist");
        let onnx_dist = root.join("node_modules/onnxruntime-web/dist");
        let dependency_hint =
            "run `npm install --no-audit --no-fund` before generating model pages";
        for name in TRANSFORMERS_RUNTIME_FILES {
            let source = transformers_dist.join(name);
            if !source.is_file() {
                return Err(format!("missing {} ({dependency_hint})", source.display()));
            }
            copy_file(&source, &model_runtime.join(name))?;
        }
        // ONNX Runtime selects the JSEP build for WebGPU, the asyncify build
        // for most WASM browsers, and the regular threaded build for Safari.
        // Keep all three local so inference never falls back to a third-party
        // CDN and every advertised backend has the matching factory + binary.
        for name in ONNX_RUNTIME_FILES {
            let source = onnx_dist.join(name);
            if !source.is_file() {
                return Err(format!("missing {} ({dependency_hint})", source.display()));
            }
            copy_file(&source, &model_runtime.join(name))?;
        }
        copy_file(
            &root.join("node_modules/@huggingface/transformers/LICENSE"),
            &model_runtime.join("LICENSE.transformers.txt"),
        )?;
        copy_file(
            &runtime.join("LICENSE.onnxruntime.txt"),
            &model_runtime.join("LICENSE.onnxruntime.txt"),
        )?;
        for name in MODEL_RUNTIME_MODULES {
            copy_file(&runtime.join(name), &model_runtime.join(name))?;
        }
        publish_sd_ort(
            &root.join("node_modules/onnxruntime-web-sd/dist"),
            &model_runtime.join("ort-sd"),
            dependency_hint,
        )?;
        check_pages_file_sizes(&model_runtime)?;
    }

    let shared_runtime = pkg_tools.join("_runtime");
    fs::create_dir_all(&shared_runtime)
        .map_err(|e| format!("mkdir {}: {e}", shared_runtime.display()))?;
    for name in SHARED_RUNTIME_FILES {
        copy_file(&runtime.join(name), &shared_runtime.join(name))?;
    }

    // "X to Y" conversion pair pages nested under the converter tools —
    // enumerated once, linked from each parent's "Popular conversions"
    // section and rendered below after the parent pages exist.
    let pair_specs = pairs::all_pairs();

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
        // Converter tools get a crawlable "Popular conversions" section
        // linking every pair page nested under them (empty for other tools).
        let pair_links = pairs::pair_links_for_parent(&pair_specs, &m.slug);
        let html = template::render_page(
            &cfg,
            m,
            &content_html,
            &schema,
            has_custom_js,
            has_custom_css,
            &related,
            &pair_links,
        );
        fs::write(out.join("index.html"), html)
            .map_err(|e| format!("write index.html: {e}"))?;
        fs::write(
            out.join("index.md"),
            markdown::tool_markdown(&cfg, m, &content_md, &schema, &related),
        )
        .map_err(|e| format!("write index.md: {e}"))?;
        // Machine-readable descriptor (identity, URLs, CLI example and the
        // manifest's tool schema) — the agent-facing twin of the page.
        fs::write(
            out.join("tool.json"),
            descriptor::tool_descriptor(&cfg, m, &schema, descriptor::load_manifest(tool_dir).as_ref()),
        )
        .map_err(|e| format!("write tool.json: {e}"))?;
        // Per-tool Open Graph card — the page's og:image/twitter:image target.
        fs::write(out.join("og.png"), og_renderer.tool_card(&cfg, m)?)
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

        copy_file(&runtime.join("tool.js"), &out.join("tool.js"))?;
        // tool.js has a static top-level `import … from "./tool-svg.js"` used by
        // every page (not just format="svg") — a missing file 404s the import
        // and aborts the whole module, so this copy is unconditional too.
        copy_file(&runtime.join("tool-svg.js"), &out.join("tool-svg.js"))?;
        // tool.js statically imports tool-run.js (which imports tool-fields.js)
        // on every page; the runtime-specific modules it imports lazily are
        // copied per runtime below.
        copy_file(&runtime.join("tool-run.js"), &out.join("tool-run.js"))?;
        copy_file(&runtime.join("tool-fields.js"), &out.join("tool-fields.js"))?;
        copy_file(&root.join("js/query-prefill.js"), &out.join("query-prefill.js"))?;
        copy_file(&runtime.join("tool.css"), &out.join("tool.css"))?;
        copy_file(&runtime.join("header.css"), &out.join("header.css"))?;
        copy_file(&runtime.join("header.js"), &out.join("header.js"))?;
        copy_file(&runtime.join("tools-index.js"), &out.join("tools-index.js"))?;
        if m.runtime == "ffmpeg" {
            copy_file(&root.join("js/ffmpeg.js"), &out.join("ffmpeg.js"))?;
            copy_file(&runtime.join("tool-ffmpeg.js"), &out.join("tool-ffmpeg.js"))?;
            copy_file(&runtime.join("tool-audio.js"), &out.join("tool-audio.js"))?;
        } else if m.runtime == "model" {
            copy_file(&runtime.join("tool-model.js"), &out.join("tool-model.js"))?;
        }
        eprintln!("rendered tools/{}/", m.slug);
    }

    // "X to Y" pair pages: one landing page per (source, target) format pair,
    // nested under its parent converter (pkg/tools/<parent>/<src>-to-<tgt>/).
    // Rendered after the parent loop so the collision check below sees every
    // file the parent page ships. Assets (tool.css, header.js, …) are linked
    // from the parent directory — nothing is copied per pair.
    let mut rendered_pairs: Vec<&pairs::PairSpec> = Vec::new();
    for pair in &pair_specs {
        let Some((_, parent)) = metas.iter().find(|(_, m)| m.slug == pair.parent) else {
            eprintln!(
                "warning: pair {} skipped (parent {} has no page)",
                pair.slug(),
                pair.parent
            );
            continue;
        };
        let out = pkg_tools.join(pair.parent).join(pair.slug());
        // A pair directory must never shadow a file the parent ships
        // (index.html, og.png, the wasm bundle, …) — fail loudly, not subtly.
        if out.exists() && !out.is_dir() {
            return Err(format!(
                "pair page {} collides with an existing file in tools/{}/",
                out.display(),
                pair.parent
            ));
        }
        fs::create_dir_all(&out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
        fs::write(
            out.join("index.html"),
            pairs::render_pair_page(&cfg, parent, pair, &pair_specs),
        )
        .map_err(|e| format!("write {}/index.html: {e}", pair.path()))?;
        fs::write(out.join("index.md"), pairs::pair_markdown(&cfg, parent, pair))
            .map_err(|e| format!("write {}/index.md: {e}", pair.path()))?;
        let card = og_renderer.pair_card(
            &cfg,
            &pair.h1(),
            &pair.og_tagline(),
            &cfg.og_label(&format!("tools/{}", pair.path())),
        )?;
        fs::write(out.join("og.png"), card)
            .map_err(|e| format!("write {}/og.png: {e}", pair.path()))?;
        rendered_pairs.push(pair);
    }
    // Machine-readable pair index — consumed by scripts/gen-seo.sh to add the
    // pair URLs to the sitemap (same pattern as _hubs.json).
    fs::write(
        pkg_tools.join("_pairs.json"),
        pairs::pairs_json(&cfg, &rendered_pairs),
    )
    .map_err(|e| format!("write tools/_pairs.json: {e}"))?;
    eprintln!("rendered {} conversion pair pages", rendered_pairs.len());

    // Static index for the in-app tools modal (fetched client-side; lives under
    // /tools/ so it is covered by the runtime SW's /tools/ bypass).
    fs::write(
        pkg_tools.join("_index.json"),
        index::tools_index_json(&cfg, &metas_only),
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
        template::render_tools_index(&cfg, &metas_only, &hubs),
    )
    .map_err(|e| format!("write tools/index.html: {e}"))?;
    // Markdown twin of the landing page (the "tools .md page") for LLMs/agents.
    fs::write(
        pkg_tools.join("index.md"),
        index::tools_catalog_md(&cfg, &metas_only),
    )
    .map_err(|e| format!("write tools/index.md: {e}"))?;
    copy_file(&runtime.join("tool.css"), &pkg_tools.join("tool.css"))?;
    copy_file(&runtime.join("header.css"), &pkg_tools.join("header.css"))?;
    copy_file(&runtime.join("header.js"), &pkg_tools.join("header.js"))?;
    copy_file(&runtime.join("tools-index.js"), &pkg_tools.join("tools-index.js"))?;
    fs::write(pkg_tools.join("og.png"), og_renderer.index_card(&cfg, metas_only.len())?)
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
            template::render_category_hub(&cfg, hub, &hubs),
        )
        .map_err(|e| format!("write tools/{}/index.html: {e}", hub.category.slug))?;
        fs::write(out.join("og.png"), og_renderer.hub_card(&cfg, hub.category)?)
            .map_err(|e| format!("write tools/{}/og.png: {e}", hub.category.slug))?;
        for asset in ["tool.css", "header.css", "header.js", "tools-index.js"] {
            copy_file(&runtime.join(asset), &out.join(asset))?;
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

/// Publishes SD-Turbo's ONNX Runtime into `out` (see SD_ORT_FILES).
fn publish_sd_ort(dist: &Path, out: &Path, dependency_hint: &str) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|e| format!("mkdir {}: {e}", out.display()))?;
    for name in SD_ORT_FILES.iter().chain(SD_ORT_GZIPPED) {
        let source = dist.join(name);
        if !source.is_file() {
            return Err(format!("missing {} ({dependency_hint})", source.display()));
        }
    }
    for name in SD_ORT_FILES {
        copy_file(&dist.join(name), &out.join(name))?;
    }
    for name in SD_ORT_GZIPPED {
        gzip_file(&dist.join(name), &out.join(format!("{name}.gz")))?;
    }
    Ok(())
}

fn gzip_file(from: &Path, to: &Path) -> Result<(), String> {
    use flate2::{write::GzEncoder, Compression};
    use std::io::Write;
    let bytes = fs::read(from).map_err(|e| format!("read {}: {e}", from.display()))?;
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&bytes)
        .and_then(|()| encoder.finish())
        .and_then(|gz| fs::write(to, gz))
        .map_err(|e| format!("gzip {} -> {}: {e}", from.display(), to.display()))
}

/// Fails the build if any file under `dir` is too large for Cloudflare Pages,
/// so the limit is caught here, not at deploy upload.
fn check_pages_file_sizes(dir: &Path) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry
            .map_err(|e| format!("read {}: {e}", dir.display()))?
            .path();
        if path.is_dir() {
            check_pages_file_sizes(&path)?;
            continue;
        }
        let size = fs::metadata(&path)
            .map_err(|e| format!("stat {}: {e}", path.display()))?
            .len();
        if size > PAGES_MAX_FILE_BYTES {
            return Err(format!(
                "{} is {:.1} MiB, over Cloudflare Pages' 25 MiB per-file limit",
                path.display(),
                size as f64 / 1048576.0
            ));
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path) -> Result<(), String> {
    fs::copy(from, to)
        .map(|_| ())
        .map_err(|e| format!("copy {} -> {}: {e}", from.display(), to.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_runtime_self_hosts_every_backend_variant() {
        for suffix in ["jsep", "asyncify"] {
            assert!(ONNX_RUNTIME_FILES
                .iter()
                .any(|name| name.ends_with(&format!("{suffix}.mjs"))));
            assert!(ONNX_RUNTIME_FILES
                .iter()
                .any(|name| name.ends_with(&format!("{suffix}.wasm"))));
        }
        assert!(ONNX_RUNTIME_FILES.contains(&"ort-wasm-simd-threaded.mjs"));
        assert!(ONNX_RUNTIME_FILES.contains(&"ort-wasm-simd-threaded.wasm"));
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("gizza-gen-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn model_runtime_publishes_the_sd_worker_and_its_math() {
        for f in [
            "sd-worker.js",
            "sd-core.js",
            "model-worker.js",
            "model-download-progress.js",
            "janus-builds.js",
        ] {
            assert!(MODEL_RUNTIME_MODULES.contains(&f), "{f} not published");
            assert!(
                Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("assets/runtime")
                    .join(f)
                    .is_file(),
                "{f} missing from assets/runtime"
            );
        }
    }

    #[test]
    fn sd_ort_ships_its_webgpu_build_with_the_binary_only_gzipped() {
        assert!(SD_ORT_FILES.contains(&"ort.webgpu.min.mjs"));
        assert!(SD_ORT_FILES.contains(&"ort-wasm-simd-threaded.asyncify.mjs"));
        assert!(
            SD_ORT_FILES.iter().all(|f| !f.ends_with(".wasm")),
            "raw wasm is never published"
        );
        assert_eq!(SD_ORT_GZIPPED, &["ort-wasm-simd-threaded.asyncify.wasm"]);
        // sd-worker.js names the same files.
        let worker = fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/runtime/sd-worker.js"),
        )
        .unwrap();
        for f in [
            "./ort-sd/ort.webgpu.min.mjs",
            "ort-wasm-simd-threaded.asyncify.mjs",
            "ort-wasm-simd-threaded.asyncify.wasm.gz",
        ] {
            assert!(worker.contains(f), "sd-worker.js does not load {f}");
        }
    }

    #[test]
    fn publish_sd_ort_gzips_the_binary_and_keeps_it_under_the_pages_limit() {
        let dist = scratch_dir("sd-dist");
        let out = scratch_dir("sd-out").join("ort-sd");
        for f in SD_ORT_FILES {
            fs::write(dist.join(f), "export default 1;").unwrap();
        }
        // Compressible like the real 27.4 MiB binary (7.1 MB gzipped), and
        // over the limit raw.
        let wasm: Vec<u8> = (0..PAGES_MAX_FILE_BYTES + 1024)
            .map(|i| (i % 7) as u8)
            .collect();
        fs::write(dist.join(SD_ORT_GZIPPED[0]), &wasm).unwrap();
        publish_sd_ort(&dist, &out, "hint").unwrap();

        assert!(
            !out.join(SD_ORT_GZIPPED[0]).exists(),
            "raw wasm is not published"
        );
        let gz = fs::read(out.join(format!("{}.gz", SD_ORT_GZIPPED[0]))).unwrap();
        let mut inflated = Vec::new();
        std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&gz[..]), &mut inflated)
            .unwrap();
        assert_eq!(inflated, wasm);
        for f in SD_ORT_FILES {
            assert!(out.join(f).is_file());
        }
        check_pages_file_sizes(out.parent().unwrap()).unwrap();

        fs::remove_file(dist.join(SD_ORT_FILES[0])).unwrap();
        let err = publish_sd_ort(&dist, &out, "run npm install").unwrap_err();
        assert!(err.contains("run npm install"), "{err}");
    }

    #[test]
    fn pages_size_check_rejects_a_file_over_25_mib() {
        let dir = scratch_dir("pages-limit");
        let nested = dir.join("ort-sd");
        fs::create_dir_all(&nested).unwrap();
        let at_limit = fs::File::create(nested.join("ok.wasm")).unwrap();
        at_limit.set_len(PAGES_MAX_FILE_BYTES).unwrap();
        check_pages_file_sizes(&dir).unwrap();
        let over = fs::File::create(nested.join("big.wasm")).unwrap();
        over.set_len(PAGES_MAX_FILE_BYTES + 1).unwrap();
        let err = check_pages_file_sizes(&dir).unwrap_err();
        assert!(err.contains("big.wasm") && err.contains("25 MiB"), "{err}");
    }

    #[test]
    fn shared_runtime_publishes_model_registry_and_custom_check() {
        for f in ["image-models.js", "custom-model.js", "janus-builds.js"] {
            assert!(SHARED_RUNTIME_FILES.contains(&f), "{f} not published");
        }
    }
}
