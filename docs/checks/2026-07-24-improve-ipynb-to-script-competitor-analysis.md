# ipynb-to-script — competitor analysis (2026-07-24)

Function: extract the code cells of a Jupyter `.ipynb` notebook into a clean `.py`
script (or Markdown), dropping outputs and execution counts.

## Competitors scanned (paraphrased notes only — no copy/branding reused)

1. **`jupyter nbconvert --to python`** (the reference CLI). Turns code cells into
   Python source, markdown cells into `#`-comment lines, adds an
   `#!/usr/bin/env python` + `# coding: utf-8` header and per-cell `# In[N]:`
   markers, and drops all outputs. The de-facto behavior every online tool imitates.
2. **ipynb2py / code-format.com / tool-convert.com style uploaders.** Upload an
   `.ipynb`, download a ready-to-run `.py`. Code cells only by default; outputs and
   execution counts stripped. Purely client-side in the browser variants (no upload).
3. **ipynbtools.com "to-py".** Extract code cells, optionally keep markdown as
   comments, and optionally emit `# %%` cell markers so VS Code / Jupytext-style
   editors still see cell boundaries.
4. **jupytools.com "ipynb-to-python".** Code cells become real Python, markdown
   cells become `#` comment lines, cell boundaries use `# %%` markers, and there's
   an option to include cell text outputs under `# Output:` comments.
5. **runcell.dev "jupyter-to-python".** Drag-and-drop, in-browser, no upload;
   preserves cell order and converts markdown cells to helpful comments.

## Table-stakes → decision (every one lands in the descriptor or is listed out-of-model)

| Capability | Decision | Where |
|---|---|---|
| Extract code cells → Python source, in order | in-model | core (always) |
| Drop outputs + execution counts by default | in-model | core (always) |
| Choose output format: Python script vs Markdown | in-model | `output` enum (script/markdown) |
| Markdown cells → `#` comments (script) / kept verbatim (markdown) | in-model | `include_markdown` bool |
| `# %%` cell markers (VS Code / Jupytext percent format) | in-model | `cell_markers` bool |
| Optionally include cell text outputs as comments | in-model | `include_outputs` bool |
| Preserve notebook cell order | in-model | core (always) |
| Client-side / private, nothing uploaded | in-model | runs in-browser on the page |

## UX / control patterns matched
- Output-format choice rendered as a `<select>` (friendly labels via `[input.labels]`).
- Booleans as checkboxes with sensible defaults (include markdown on; outputs off; markers off).
- `[[example]]` preset chips for the common conversions (clean `.py`, VS Code `# %%`, Markdown export).

## Out-of-model / intentionally excluded
- **File upload of a real `.ipynb` binary**: this repo's page model is text-in/text-out;
  the user pastes the notebook JSON (a `.ipynb` *is* JSON), so no binary upload is needed.
- **Executing the notebook / capturing fresh outputs** (needs a Python kernel) — out of a
  pure-Rust model. We only read outputs already stored in the file.
- **Syntax highlighting / running the produced script** — out of scope for a converter.
- Header shebang / `# In[N]:` execution-count markers are intentionally *omitted* to keep
  the output a clean, diff-friendly script (nbconvert emits them; the online tools mostly don't).
