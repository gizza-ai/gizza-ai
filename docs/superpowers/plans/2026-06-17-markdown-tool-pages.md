# Markdown-for-LLMs tool pages Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Emit a clean markdown twin (`index.md`) of every tool page plus a root `/llms.txt` index, single-sourced from the `meta.toml` + `content.md` the generator already reads, so web-browsing LLMs can read tools without parsing HTML.

**Architecture:** Add a `markdown` module to the existing `tools/generator` static pass. For each tool it writes `pkg/tools/<slug>/index.md` (a generated header — description, run-it, inputs, output — followed by the tool's `content.md` prose). Once per run it writes `pkg/llms.txt` (a title + summary + link list to each `index.md`). The HTML head gains a `<link rel="alternate" type="text/markdown">`, and `/llms.txt` is added to the SW fetch-bypass. No runtime/block/SW-template changes. See the design: `docs/superpowers/specs/2026-06-17-markdown-tool-pages-design.md`.

**Tech Stack:** Rust (`tools/generator`, a static binary `gizza-tool-pages`), `toml`/`serde` (already used by `meta.rs`), `node:test` for the SW-bypass assertion.

---

## Commands (run from `gizza-ai/`)

- Generator unit tests (CI gate): `cargo test --manifest-path tools/generator/Cargo.toml`
- Generator compiles: `cargo build --manifest-path tools/generator/Cargo.toml`
- App build (regenerates `pkg/sw.js`): `solobase build`
- SW-bypass JS test: `node --test js/sw-bypass.test.js`

Note: producing the *actual* `index.md`/`llms.txt` files requires running the
`gizza-tool-pages` binary, which first needs each tool's `blocks/<slug>/web/pkg/`
wasm built (the deploy pipeline does this). The **unit tests** prove
`tool_markdown`/`llms_txt` correctness deterministically without that heavy step.

## Reference: `ToolMeta` fields (from `tools/generator/src/meta.rs`)

`slug, title, description, tags, h1, hero_subtitle, wasm, export, live,
interval_ms, output_label, format, runtime, inputs: Vec<Input>` where
`Input { name, source, label, placeholder, accept }` and `source ∈
{"field","clock","file"}`.

---

### Task 1: `markdown::tool_markdown` (per-tool `index.md` body)

**Files:**
- Create: `tools/generator/src/markdown.rs`

- [ ] **Step 1: Write the module with failing tests**

Create `tools/generator/src/markdown.rs`:

```rust
//! Markdown twin of each tool page + the root /llms.txt index.
//!
//! Single-sourced from the same `ToolMeta` (meta.toml) + content.md that drive
//! the HTML page, so web-browsing LLMs can read tools without parsing HTML.

use crate::meta::{Input, ToolMeta};

/// Public site origin — matches the literal used in `seo.rs` + `template.rs`.
const SITE: &str = "https://gizza.ai";

/// Render a tool's `index.md`: a generated header (description, run-it, inputs,
/// output) followed by the tool's prose `content.md`, verbatim.
pub fn tool_markdown(meta: &ToolMeta, content_md: &str) -> String {
    let mut s = String::new();
    s.push_str(&format!("# {}\n\n", meta.h1));
    s.push_str(&format!("{}\n\n", meta.description));

    s.push_str("## Run it\n\n");
    s.push_str(&format!("- **CLI:** `{}`\n", cli_example(meta)));
    s.push_str(&format!("- **Web:** {}/tools/{}/\n\n", SITE, meta.slug));

    s.push_str("## Inputs\n\n");
    let manual: Vec<&Input> = meta
        .inputs
        .iter()
        .filter(|i| i.source == "field" || i.source == "file")
        .collect();
    if manual.is_empty() {
        s.push_str("- _no manual inputs — runs automatically_\n\n");
    } else {
        for i in &manual {
            let label = if i.label.is_empty() { i.name.as_str() } else { i.label.as_str() };
            let mut note = i.source.clone();
            if !i.accept.is_empty() {
                note.push_str(&format!("; accept: {}", i.accept));
            }
            s.push_str(&format!("- `{}` — {} _({})_\n", i.name, label, note));
        }
        s.push('\n');
    }

    s.push_str("## Output\n\n");
    s.push_str(&format!("- {} ({})\n\n", meta.output_label, meta.format));

    s.push_str("---\n\n");
    s.push_str(content_md.trim_end());
    s.push('\n');
    s
}

/// The example CLI invocation: field tools use the first field input's
/// placeholder as the example arg; file tools take a path; auto-only tools
/// (e.g. clock) take no args.
fn cli_example(meta: &ToolMeta) -> String {
    if let Some(field) = meta.inputs.iter().find(|i| i.source == "field") {
        let arg = if field.placeholder.is_empty() { "..." } else { field.placeholder.as_str() };
        format!("gizza tool {} \"{}\"", meta.slug, arg)
    } else if meta.inputs.iter().any(|i| i.source == "file") {
        format!("gizza tool {} <path>", meta.slug)
    } else {
        format!("gizza tool {}", meta.slug)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator — gizza.ai"
description   = "Evaluate expressions instantly."
tags          = ["math"]
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

    fn file_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "image-grayscale"
title         = "Grayscale an Image — gizza.ai"
description   = "Convert an image to grayscale in your browser."
h1            = "Grayscale an Image"
hero_subtitle = "Upload an image."
wasm          = "gizza_ai_image_grayscale_web"
export        = "build_argv"
output_label  = "Image"
format        = "image"
runtime       = "ffmpeg"

[[input]]
name   = "file"
label  = "Image"
source = "file"
accept = "image/*"
"#,
        )
        .unwrap()
    }

    fn live_tool() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "clock"
title         = "Clock — gizza.ai"
description   = "The current time, live."
h1            = "Clock"
hero_subtitle = "Live time."
wasm          = "gizza_ai_clock_web"
export        = "now"
live          = true
output_label  = "Time"
format        = "text"

[[input]]
name   = "now"
label  = "Now"
source = "clock"
"#,
        )
        .unwrap()
    }

    #[test]
    fn field_tool_has_header_runit_inputs_output_prose() {
        let md = tool_markdown(&field_tool(), "Some **prose** about the calculator.");
        assert!(md.contains("# Free Online Calculator"));
        assert!(md.contains("`gizza tool calculator \"2 + 2 * 3\"`"), "CLI example");
        assert!(md.contains("https://gizza.ai/tools/calculator/"), "web URL");
        assert!(md.contains("`expr` — Expression _(field)_"), "input listed");
        assert!(md.contains("Result (number)"), "output listed");
        assert!(md.contains("Some **prose** about the calculator."), "prose appended");
    }

    #[test]
    fn file_tool_uses_path_example_and_accept() {
        let md = tool_markdown(&file_tool(), "prose");
        assert!(md.contains("`gizza tool image-grayscale <path>`"), "path CLI example");
        assert!(md.contains("`file` — Image _(file; accept: image/*)_"), "accept shown");
    }

    #[test]
    fn live_tool_takes_no_arguments() {
        let md = tool_markdown(&live_tool(), "prose");
        assert!(md.contains("`gizza tool clock`"), "no-arg CLI example");
        assert!(md.contains("_no manual inputs — runs automatically_"), "no manual inputs note");
    }
}
```

- [ ] **Step 2: Add the module declaration**

In `tools/generator/src/main.rs`, after `mod index;` (line 8), add:

```rust
mod markdown;
```

- [ ] **Step 3: Run tests to verify they pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml markdown`
Expected: PASS — `field_tool_has_header_runit_inputs_output_prose`,
`file_tool_uses_path_example_and_accept`, `live_tool_takes_no_arguments`.

(If `mod markdown;` is missing the module won't compile — that's the red state.)

- [ ] **Step 4: Commit**

```bash
git add tools/generator/src/markdown.rs tools/generator/src/main.rs
git commit -m "feat(md-tool-pages): markdown::tool_markdown renders per-tool index.md"
```

---

### Task 2: `markdown::llms_txt` (root `/llms.txt` index)

**Files:**
- Modify: `tools/generator/src/markdown.rs`

- [ ] **Step 1: Write the failing test**

In `tools/generator/src/markdown.rs`, inside `mod tests`, add:

```rust
    #[test]
    fn llms_txt_lists_each_tool_with_absolute_md_link() {
        let out = llms_txt(&[field_tool(), live_tool()]);
        assert!(out.contains("# gizza.ai"), "title");
        assert!(out.contains("## Tools"), "tools section");
        assert!(
            out.contains("- [Free Online Calculator — gizza.ai](https://gizza.ai/tools/calculator/index.md): Evaluate expressions instantly."),
            "calculator entry with absolute .md link",
        );
        assert!(
            out.contains("- [Clock — gizza.ai](https://gizza.ai/tools/clock/index.md): The current time, live."),
            "clock entry",
        );
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path tools/generator/Cargo.toml llms_txt`
Expected: FAIL — `cannot find function llms_txt in this scope`.

- [ ] **Step 3: Implement `llms_txt`**

In `tools/generator/src/markdown.rs`, after `tool_markdown` (before `fn cli_example`), add:

```rust
/// Render the root `/llms.txt`: a title, a summary blockquote, and a link list
/// to each tool's markdown twin (an index, not a full dump).
pub fn llms_txt(metas: &[ToolMeta]) -> String {
    let mut s = String::new();
    s.push_str("# gizza.ai — browser-native tools\n\n");
    s.push_str(
        "> Free, single-purpose tools that run entirely in your browser. Many also \
run headlessly via `gizza tool <name>` (see the CLI + SKILL.md in the repo).\n\n",
    );
    s.push_str("## Tools\n\n");
    for m in metas {
        s.push_str(&format!(
            "- [{}]({}/tools/{}/index.md): {}\n",
            m.title, SITE, m.slug, m.description
        ));
    }
    s
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path tools/generator/Cargo.toml llms_txt`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/generator/src/markdown.rs
git commit -m "feat(md-tool-pages): markdown::llms_txt renders the root /llms.txt index"
```

---

### Task 3: Wire `markdown` into the generator pass

**Files:**
- Modify: `tools/generator/src/main.rs:44-45` (write `index.md`), `:76-77` (write `llms.txt`)

- [ ] **Step 1: Write `index.md` per tool**

In `tools/generator/src/main.rs`, immediately after the `index.html` write
(currently lines 44-45):

```rust
        fs::write(out.join("index.html"), html)
            .map_err(|e| format!("write index.html: {e}"))?;
```

add:

```rust
        fs::write(out.join("index.md"), markdown::tool_markdown(m, &content_md))
            .map_err(|e| format!("write index.md: {e}"))?;
```

- [ ] **Step 2: Write `llms.txt` once at pkg root**

In `tools/generator/src/main.rs`, immediately after the `robots.txt` write
(currently lines 76-77):

```rust
    fs::write(pkg.join("robots.txt"), seo::robots())
        .map_err(|e| format!("write robots.txt: {e}"))?;
```

add:

```rust
    fs::write(pkg.join("llms.txt"), markdown::llms_txt(&metas_only))
        .map_err(|e| format!("write llms.txt: {e}"))?;
```

(`metas_only` is already built at line 65; `pkg` at line 73 — both in scope here.)

- [ ] **Step 3: Verify the generator still compiles + all generator tests pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS — all existing tests plus the new markdown tests; no compile errors.

- [ ] **Step 4: Commit**

```bash
git add tools/generator/src/main.rs
git commit -m "feat(md-tool-pages): generator writes index.md per tool + pkg/llms.txt"
```

---

### Task 4: HTML discovery link (`<link rel="alternate">`)

**Files:**
- Modify: `tools/generator/src/template.rs` (head, near the canonical link ~line 38; test ~line 147)

- [ ] **Step 1: Add the failing assertion to the existing head test**

In `tools/generator/src/template.rs`, inside the test that already renders a page
and asserts the canonical link (after the line
`assert!(html.contains(r#"<link rel="canonical" href="https://gizza.ai/tools/calculator/">"#));`),
add:

```rust
        assert!(
            html.contains(r#"<link rel="alternate" type="text/markdown" href="index.md">"#),
            "markdown twin discovery link present",
        );
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path tools/generator/Cargo.toml --lib`
Expected: FAIL on the head test — the alternate link isn't rendered yet.

- [ ] **Step 3: Render the alternate link**

In `tools/generator/src/template.rs`, in the `head { … }` maud block, immediately
after the canonical link line (`link rel="canonical" href=(canonical);`), add:

```rust
                link rel="alternate" type="text/markdown" href="index.md";
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path tools/generator/Cargo.toml --lib`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tools/generator/src/template.rs
git commit -m "feat(md-tool-pages): link rel=alternate text/markdown in each page head"
```

---

### Task 5: Serve + SW-bypass `/llms.txt`

**Files:**
- Modify: `js/sw-bypass.test.js` (add assertion), `solobase.toml` (`extra_bypass_prefix`)

- [ ] **Step 1: Write the failing bypass assertion**

In `js/sw-bypass.test.js`, after the existing `/tools/` test, append:

```js
test('sw.js bypasses /llms.txt so the agent index serves statically in-browser', () => {
  assert.ok(existsSync(swPath), 'pkg/sw.js missing — run `solobase build` first');
  const src = readFileSync(swPath, 'utf8');
  assert.match(
    src,
    /startsWith\(['"]\/llms\.txt['"]\)/,
    'sw.js is missing the /llms.txt bypass — check extra_bypass_prefix in solobase.toml',
  );
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `node --test js/sw-bypass.test.js`
Expected: FAIL — current `pkg/sw.js` has no `/llms.txt` bypass.

- [ ] **Step 3: Add `/llms.txt` to the bypass list**

In `solobase.toml`, append `"/llms.txt"` to the `[assets].extra_bypass_prefix`
array (keep all existing entries):

```toml
extra_bypass_prefix = ["/gizza-app.js", "/gizza.css", "/render.js", "/pending.js", "/gis.png", "/gis_no_eyes.png", "/gis_a_job_no_eyes.png", "/eye.png", "/gis_video_idle.mp4", "/gis_video_typing_loop.mp4", "/gis_video_typing_finish.mp4", "/favicon.ico", "/favicon-32.png", "/apple-touch-icon.png", "/logo.webp", "/model-picker.js", "/model-picker.css", "/tool.js", "/tool.css", "/tools/", "/tools-modal.js", "/tools-modal.css", "/llms.txt"]
```

- [ ] **Step 4: Rebuild so `pkg/sw.js` regenerates**

Run: `solobase build`
Expected: build succeeds; `pkg/sw.js` now contains the `/llms.txt` bypass clause.

- [ ] **Step 5: Run the bypass test to verify it passes**

Run: `node --test js/sw-bypass.test.js`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add solobase.toml js/sw-bypass.test.js
git commit -m "build(md-tool-pages): SW-bypass /llms.txt so it serves statically"
```

---

### Task 6: Final verification

- [ ] **Step 1: Run the generator CI-gate suite**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS — all generator tests (markdown + template + index + seo + meta).

- [ ] **Step 2: Sanity-check the rendered output shape (no per-tool wasm needed)**

Add a throwaway check by running the markdown unit assertions only (already
covered in Tasks 1-2); confirm by eye that a sample `tool_markdown` output reads
well as markdown (header, Run it, Inputs, Output, `---`, prose). No code change.

- [ ] **Step 3: (Optional, deploy-pipeline) full generation**

When per-tool wasms exist (`blocks/<slug>/web/pkg/` built — the deploy pipeline
does this), running `gizza-tool-pages` (`cargo run --manifest-path
tools/generator/Cargo.toml -- .`) writes `pkg/tools/<slug>/index.md` for every
tool and `pkg/llms.txt`. Confirm `pkg/llms.txt` lists every tool and a sample
`pkg/tools/calculator/index.md` looks right. This is exercised by the existing
deploy build; no new pipeline step is required.

- [ ] **Step 4: Record completion in the spec**

In `docs/superpowers/specs/2026-06-17-markdown-tool-pages-design.md`, update the
**Status** line to note the feature is implemented + unit-tested.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/specs/2026-06-17-markdown-tool-pages-design.md
git commit -m "docs(md-tool-pages): mark design implemented"
```

---

## Self-review

**Spec coverage:**
- Per-tool `index.md` (header: description, run-it, inputs, output + prose) → Task 1 (`tool_markdown`) + Task 3 (wired). ✓
- Root `/llms.txt` index (title + summary + link list, not full dump) → Task 2 (`llms_txt`) + Task 3 (wired). ✓
- Schema from `meta.toml` → Tasks 1-2 read only `ToolMeta`; no runtime/CLI call. ✓
- Field / file / live (auto) variants → Task 1 tests all three. ✓
- `<link rel="alternate">` discovery → Task 4. ✓
- `/llms.txt` SW-bypass → Task 5. ✓
- Single-source / no-drift → derived entirely from `ToolMeta` + `content_md`; adding a tool auto-produces both. ✓

**Placeholder scan:** No TBD/TODO/"add error handling"/"similar to" — every code and command step is concrete. ✓

**Type/name consistency:** `tool_markdown(meta: &ToolMeta, content_md: &str)`,
`llms_txt(metas: &[ToolMeta])`, `cli_example(meta)`, `SITE`, and the `Input`
fields (`name`/`source`/`label`/`placeholder`/`accept`) match `meta.rs`. The
generator variables `m`, `content_md`, `metas_only`, `pkg`, `out` match
`main.rs`. Absolute `https://gizza.ai/...index.md` links are used consistently in
`llms_txt` and tested as such. ✓

**Note (spec refinement):** `llms_txt` uses **absolute** `https://gizza.ai/tools/<slug>/index.md` links (not the root-relative form sketched in the spec) — absolute is correct for an `llms.txt` fetched standalone via WebFetch. Spec example updated to match.
