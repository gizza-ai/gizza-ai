# Markdown tool-page twins Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Emit a clean markdown twin (`index.md`) of every tool page, single-sourced from the `meta.toml` + `content.md` the generator already reads, so web-browsing LLMs can read tools without parsing HTML.

**Architecture:** Add a `markdown` module to the existing `tools/generator` static pass. For each tool it writes `pkg/tools/<slug>/index.md` — a generated header (description, run-it, inputs, output) followed by the tool's `content.md` prose. The HTML head gains a `<link rel="alternate" type="text/markdown">`. No runtime/block/SW changes, and explicitly **no** `/llms.txt`/`sitemap`/`robots` work — that belongs to the separate `feat/seo-discoverability-and-chrome` effort. See the design: `docs/superpowers/specs/2026-06-17-markdown-tool-pages-design.md`.

**Tech Stack:** Rust (`tools/generator`, a static binary `gizza-tool-pages`), `toml`/`serde` (already used by `meta.rs`).

---

## Commands (run from `gizza-ai/`)

- Generator unit tests (CI gate): `cargo test --manifest-path tools/generator/Cargo.toml`
- Just the markdown/template tests: `cargo test --manifest-path tools/generator/Cargo.toml markdown` / `… --lib`

## Reference: `ToolMeta` (from `tools/generator/src/meta.rs`)

`slug, title, description, tags, h1, hero_subtitle, wasm, export, live,
interval_ms, output_label, format, runtime, inputs: Vec<Input>` where
`Input { name, source, label, placeholder, accept }`, `source ∈
{"field","clock","file"}`.

## Coordination note

This branch (`feat/md-tool-pages`) currently carries the SEO effort's design-doc
commit `bc18942` in its base (an artifact of the shared-tree branch tangle). That
is harmless — it's the same commit `feat/seo-discoverability-and-chrome` will
merge — and resolves itself on merge to `main`. Do **not** rebase it off
`bc18942` while other sessions are live (that would disturb the shared working
tree). Before each commit, run `git branch --show-current` and confirm it reads
`feat/md-tool-pages` (the shared tree can be switched by another session).

---

### Task 1: `markdown::tool_markdown` (per-tool `index.md`)

**Files:**
- Create: `tools/generator/src/markdown.rs`
- Modify: `tools/generator/src/main.rs:8` (add `mod markdown;`)

- [ ] **Step 1: Create the module with its tests**

Create `tools/generator/src/markdown.rs`:

```rust
//! Markdown twin of each tool page (`index.md`), single-sourced from the same
//! `ToolMeta` (meta.toml) + content.md that drive the HTML page, so web-browsing
//! LLMs can read a tool without parsing HTML.

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
    fn live_tool_takes_no_manual_arguments() {
        let md = tool_markdown(&live_tool(), "prose");
        assert!(md.contains("`gizza tool clock`"), "no-arg CLI example");
        assert!(md.contains("_no manual inputs — runs automatically_"), "no manual inputs note");
    }
}
```

- [ ] **Step 2: Declare the module**

In `tools/generator/src/main.rs`, after `mod index;` (line 8), add:

```rust
mod markdown;
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml markdown`
Expected: PASS — `field_tool_has_header_runit_inputs_output_prose`,
`file_tool_uses_path_example_and_accept`, `live_tool_takes_no_manual_arguments`.

- [ ] **Step 4: Commit** (first confirm the branch — shared tree)

```bash
test "$(git branch --show-current)" = "feat/md-tool-pages" || { echo "WRONG BRANCH"; exit 1; }
git add tools/generator/src/markdown.rs tools/generator/src/main.rs
git commit -m "feat(md-tool-pages): markdown::tool_markdown renders per-tool index.md"
```

---

### Task 2: Write `index.md` per tool in the generator pass

**Files:**
- Modify: `tools/generator/src/main.rs:44-45` (after the `index.html` write)

- [ ] **Step 1: Add the `index.md` write**

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

(`out`, `m`, and `content_md` are all in scope in this loop.)

- [ ] **Step 2: Verify the generator compiles + all generator tests pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS — existing `index`/`meta`/`seo`/`template` tests plus the new
`markdown` tests; no compile errors.

- [ ] **Step 3: Commit**

```bash
test "$(git branch --show-current)" = "feat/md-tool-pages" || { echo "WRONG BRANCH"; exit 1; }
git add tools/generator/src/main.rs
git commit -m "feat(md-tool-pages): generator writes index.md next to index.html per tool"
```

---

### Task 3: HTML discovery link (`<link rel="alternate">`)

**Files:**
- Modify: `tools/generator/src/template.rs` (head near the canonical link ~line 38; existing head test ~line 147)

- [ ] **Step 1: Add the failing assertion to the existing head test**

In `tools/generator/src/template.rs`, inside the test that already renders a page
and asserts the canonical link (right after the line
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
test "$(git branch --show-current)" = "feat/md-tool-pages" || { echo "WRONG BRANCH"; exit 1; }
git add tools/generator/src/template.rs
git commit -m "feat(md-tool-pages): link rel=alternate text/markdown in each page head"
```

---

### Task 4: Final verification

- [ ] **Step 1: Run the generator CI-gate suite**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS — all generator tests (markdown + template + index + seo + meta).

- [ ] **Step 2: (Optional, deploy pipeline) full generation**

When per-tool wasms exist (`blocks/<slug>/web/pkg/` built — the deploy pipeline
does this), `gizza-tool-pages` (`cargo run --manifest-path
tools/generator/Cargo.toml -- .`) writes `pkg/tools/<slug>/index.md` for every
tool. Confirm a sample `pkg/tools/calculator/index.md` reads well as markdown
(header, Run it, Inputs, Output, `---`, prose) and that `index.html` links it via
the `<link rel="alternate">`. No new pipeline step is required.

- [ ] **Step 3: Record completion in the spec**

In `docs/superpowers/specs/2026-06-17-markdown-tool-pages-design.md`, update the
**Status** line to note the feature is implemented + unit-tested.

- [ ] **Step 4: Commit**

```bash
test "$(git branch --show-current)" = "feat/md-tool-pages" || { echo "WRONG BRANCH"; exit 1; }
git add docs/superpowers/specs/2026-06-17-markdown-tool-pages-design.md
git commit -m "docs(md-tool-pages): mark design implemented"
```

---

## Self-review

**Spec coverage:**
- Per-tool `index.md` (header: description, run-it, inputs, output + prose) → Task 1 (`tool_markdown`) + Task 2 (wired). ✓
- Schema from `meta.toml` → Task 1 reads only `ToolMeta`; no runtime/CLI call. ✓
- Field / file / auto-only variants → Task 1 tests all three. ✓
- `<link rel="alternate">` discovery → Task 3. ✓
- **No** `/llms.txt`/`sitemap`/`robots`/`seo.rs` changes → nothing in any task touches them (owned by `feat/seo-discoverability-and-chrome`). ✓
- Single-source / no-drift → derived entirely from `ToolMeta` + `content_md`. ✓

**Placeholder scan:** No TBD/TODO/"add error handling"/"similar to" — every code and command step is concrete. ✓

**Type/name consistency:** `tool_markdown(meta: &ToolMeta, content_md: &str)`,
`cli_example(meta)`, `SITE`, and the `Input` fields
(`name`/`source`/`label`/`placeholder`/`accept`) match `meta.rs`. The generator
variables `m`, `content_md`, `out` match `main.rs`. ✓

**Collision check:** This plan touches only `tools/generator/src/{markdown.rs,main.rs,template.rs}`. The SEO effort touches `main.rs` too (it *removes* the sitemap/robots writes + `mod seo`); the only shared file is `main.rs`, where this plan adds one `index.md` write line and one `mod markdown;` — a trivial merge against that effort's removals. No shared function or struct.
