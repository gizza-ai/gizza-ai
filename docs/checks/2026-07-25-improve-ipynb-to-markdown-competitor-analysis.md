# ipynb-to-markdown — competitor analysis (2026-07-25)

One WebSearch ("ipynb to markdown converter tool nbconvert options outputs images"); skimmed the
top real tools. All findings paraphrased — no competitor copy, branding, or trademarks reproduced.

## Competitors skimmed

1. **Jupyter `nbconvert --to markdown`** (the reference implementation). Converts an `.ipynb`
   to a `.md` file. Table-stakes behaviour:
   - Markdown cells emitted verbatim.
   - Code cells emitted as fenced blocks tagged with the notebook's kernel language.
   - Cell **outputs are INCLUDED by default** (stream text, `execute_result`/`display_data`,
     errors). This is the key contrast with a code-extractor.
   - **Image outputs** (`image/png`, `image/jpeg`, `image/svg+xml`) are written out as separate
     files into a `<notebook>_files/` directory and linked from the Markdown.
   - Flags: `--no-input` / `TemplateExporter.exclude_input=True` (drop code, keep outputs),
     `--no-prompt` (drop `In[]/Out[]` execution-count prompts), `ClearOutputPreprocessor`
     (strip outputs). Templates/branding are extensible.

2. **VS Code / Jupytext "export to Markdown"** and single-file web converters (e.g. browser
   "ipynb → md" pages, nbviewer-style rendering). Because a single-file Markdown export cannot
   carry a sidecar `_files/` directory, these **embed images inline as base64 `data:` URIs**, and
   render rich outputs (HTML tables from DataFrames, Markdown display output) in place. Outputs on
   by default; option to strip code or outputs.

3. **MyST / documentation-oriented exporters** (custom nbconvert templates). Add MyST-flavoured
   directives, prompt labels, and metadata front-matter. Mostly out-of-scope niche syntax.

## Table-stakes → decision (each lands in the descriptor or is explicitly out-of-model)

| Feature | Decision |
| --- | --- |
| Markdown cells verbatim | in-model — always |
| Code cells as fenced blocks with kernel language | in-model — always (auto-detected from `metadata.language_info`/`kernelspec`) |
| Include cell outputs (default ON) | in-model — `include_outputs` (default true) |
| Stream stdout/stderr, execute_result/display_data, error tracebacks | in-model — rendered, ANSI stripped |
| Image outputs | in-model — `image_mode` enum **embed** (inline base64 `data:` URI, since we emit a single string — no sidecar dir), **placeholder** (a `*[image output]*` note), or **omit** |
| Rich reps: text/markdown, text/html, image/svg+xml, text/latex, text/plain | in-model — richest rep chosen by a documented priority |
| Drop code, keep outputs (`--no-input`) | in-model — `include_code` (default true) |
| Drop markdown cells (code only) | in-model — `include_markdown` (default true) |
| `In[]/Out[]` execution-count prompts | in-model — `show_prompts` (default false) |
| Markdown-cell `attachment:` embedded images | in-model — resolved to inline `data:` URIs in embed mode |
| Sidecar `<nb>_files/` image directory | **out-of-model** — the tool returns one Markdown string; inline base64 is the single-file equivalent |
| Custom templates / MyST directives / front-matter | **out-of-model** — niche templating; not a general converter concern |
| Export to HTML / PDF / reST / LaTeX docs | **out-of-model** — separate tools/formats |

## UX controls (page)

- Large multiline textarea for the pasted `.ipynb` JSON (placeholder shows a minimal notebook).
- `image_mode` renders as a `<select>` (enum); `include_code` / `include_outputs` /
  `include_markdown` / `show_prompts` render as checkboxes from the schema.
- `[[example]]` preset chips: a full render (outputs + embedded image), a code-only export, a
  prose-only export (no code), and a prompts-labelled export — these double as worked examples.

## Differentiation vs the existing `blocks/ipynb-to-script`

`ipynb-to-script` is a code-EXTRACTOR (its purpose is dropping outputs to get a runnable `.py`); its
`output=markdown` mode is a minimal secondary path: outputs OFF by default and, when on, only
`text/plain` rendered as bare fences — **no image embedding, no execution-count prompts, no rich
output reps (HTML/Markdown/SVG), no attachment resolution**. `ipynb-to-markdown` is the
document-EXPORTER counterpart (nbconvert `--to markdown` role): outputs ON by default, inline
base64 images, rich-rep selection, prompt labels, and attachment resolution. Distinct primary
purpose and capability set — not a semantic duplicate.
