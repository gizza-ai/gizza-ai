## What this tool does

Paste a **Jupyter notebook** — the full contents of an `.ipynb` file, which is
just JSON — and get back a clean, runnable **Python script**. It runs entirely in
your browser with WebAssembly: nothing is uploaded, it works offline, and there's
no sign-up.

The converter:

- extracts every **code cell in order**, verbatim,
- **drops all outputs and execution counts** (`In[3]:`, stored `stdout`, plots,
  tracebacks) so you get source, not a run log,
- turns **markdown cells into `#` comments** (or keeps them verbatim in Markdown
  mode) so the narrative survives,
- **skips empty cells** so the script stays tidy,
- understands both **nbformat v4** (top-level `cells`) and older **v3**
  (`worksheets`), and
- reads the notebook's own **language** metadata for Markdown code fences
  (defaults to `python`).

## Options

| Option | What it does |
| --- | --- |
| **Output format** | `script` (default) — a `.py` file with code verbatim and markdown cells as `#` comments. `markdown` — a `.md` document with markdown cells verbatim and code in fenced blocks. |
| **Keep markdown cells** | On by default. Markdown/raw cells become `#` comments (script) or stay verbatim (markdown). Turn it off for **code only**. |
| **Include stored cell outputs** | Off by default — the whole point is dropping outputs. Turn it on to append each cell's stored text output under a `# Output:` comment (or a fenced block in markdown mode). |
| **Add # %% cell markers** | Off by default. Emits a `# %%` marker before each cell so VS Code and Jupytext see cell boundaries in the `.py`. Ignored in Markdown mode. |

## Worked example

Given this notebook (two cells — one markdown, one code with a stored output):

```json
{
  "cells": [
    {"cell_type": "markdown", "source": ["# Demo notebook\n", "\n", "Load and greet."]},
    {"cell_type": "code", "execution_count": 1,
     "outputs": [{"output_type": "stream", "name": "stdout", "text": ["hi\n"]}],
     "source": ["import sys\n", "print('hi')"]}
  ],
  "metadata": {"language_info": {"name": "python"}},
  "nbformat": 4, "nbformat_minor": 5
}
```

the default **script** output is:

```python
# # Demo notebook
#
# Load and greet.

import sys
print('hi')
```

The `execution_count` and the stored `hi` output are gone; the markdown cell
became `#` comments. Turn on **Add # %% cell markers** and each cell is prefixed
with `# %%` (`# %% [markdown]` for prose) so VS Code shows cell boundaries. Turn
on **Include stored cell outputs** and a `# Output:` / `# hi` comment is appended
after the code.

## Limits & edge cases

- **Paste the JSON, not a file.** A `.ipynb` *is* a JSON text file — open it and
  paste its contents. There's no binary upload; if the text isn't valid JSON you
  get a clear "input is not valid JSON" error.
- **It never executes the notebook.** There's no Python kernel here — it only
  reads outputs already stored in the file, and by default drops them. It can't
  produce *fresh* outputs.
- **Empty cells are skipped**, and a notebook with no convertible cells returns a
  clear error rather than an empty file.
- **Blank line between cells:** cells are joined with one blank line. Cell-internal
  blank lines are preserved as written.
- The output has **no trailing newline** — copy or download it as-is.
- **`# In[N]:` markers and the shebang are intentionally omitted** (nbconvert adds
  them) to keep the script clean and diff-friendly.

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes. The conversion runs locally in your browser with WebAssembly — your notebook
never leaves your device, and the page keeps working offline once it has loaded.
There's no account and nothing is uploaded.

</details>

<details>
<summary>How is this different from <code>jupyter nbconvert --to python</code>?</summary>

Same core idea — code cells become Python, markdown cells become `#` comments —
but the output is deliberately leaner. This tool **omits** nbconvert's
`#!/usr/bin/env python` shebang, `# coding: utf-8` line, and per-cell `# In[N]:`
execution-count markers, so you get a clean, diff-friendly script. You can opt
into `# %%` VS Code / Jupytext markers instead.

</details>

<details>
<summary>Why is my <code>print()</code> output missing?</summary>

That's the point of the tool — outputs and execution counts are dropped so you get
source code, not a run log. If you *do* want the stored text outputs kept, turn on
**Include stored cell outputs** and they're appended under a `# Output:` comment.

</details>

<details>
<summary>Can I get Markdown instead of a Python file?</summary>

Yes. Set **Output format** to `markdown`. Markdown cells are kept verbatim and
code cells are wrapped in fenced code blocks (using the notebook's language, e.g.
` ```python `). It's handy for turning a notebook into a readable document.

</details>

<details>
<summary>Does it work with R, Julia, or old notebooks?</summary>

Yes. Any language works — code cells are copied verbatim, and Markdown mode fences
them with the notebook's own language metadata. Both nbformat **v4** (top-level
`cells`) and older **v3** notebooks (`worksheets`) are supported.

</details>

<details>
<summary>How do I keep cell boundaries for VS Code or Jupytext?</summary>

Turn on **Add # %% cell markers**. Each cell is prefixed with `# %%`
(`# %% [markdown]` for prose cells), which VS Code's Python extension and Jupytext
recognize as cell boundaries — so you can still run the `.py` cell-by-cell.

</details>
