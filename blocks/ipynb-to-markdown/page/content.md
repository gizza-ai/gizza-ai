## Export a Jupyter notebook as Markdown

Paste the JSON contents of an `.ipynb` file and this tool turns the notebook into a
single Markdown document. Markdown cells are emitted as Markdown, code cells become
fenced code blocks using the notebook language, and stored outputs can be rendered
below the cell just like a notebook export.

Use it when you want a clean README, blog draft, lab note, or documentation page
from a notebook without installing Python or running `nbconvert` locally.

### What gets converted

- **Markdown/raw cells** are copied verbatim, including headings, lists, and links.
- **Code cells** become fenced blocks such as ```` ```python ````.
- **Stream outputs** (`stdout` / `stderr`) and plain results are rendered in text
  fences.
- **Rich results** prefer Markdown, then images, then HTML tables, then LaTeX, then
  plain text.
- **Errors** are rendered as stripped tracebacks so ANSI colors do not leak into
  the Markdown.
- **Image outputs and attachments** can be embedded as inline `data:` URIs,
  replaced with a short placeholder, or omitted.

### Worked example

A notebook with one markdown cell and one code cell:

```json
{"cells":[
  {"cell_type":"markdown","source":["# Demo\n","Some notes"]},
  {"cell_type":"code","execution_count":1,
   "source":["print(2 + 2)"],
   "outputs":[{"output_type":"stream","name":"stdout","text":["4\n"]}]}
],"metadata":{"language_info":{"name":"python"}},"nbformat":4,"nbformat_minor":5}
```

becomes:

````markdown
# Demo

Some notes

```python
print(2 + 2)
```

```
4
```
````

Turn off **Include code cells** to get a no-input export that keeps outputs, or turn
off **Include stored outputs** for a code-only notebook-to-Markdown conversion.

## FAQ

<details>
<summary>Is this the same as converting a notebook to a Python script?</summary>

No. A script export focuses on runnable code and usually drops notebook outputs.
This tool is a document export: it preserves markdown cells and, by default,
renders stored outputs such as stdout, rich results, errors, and images. Use
`ipynb-to-script` when you want a `.py`; use this when you want a readable `.md`.

</details>

<details>
<summary>What happens to plots and image outputs?</summary>

By default, image outputs are embedded as inline `data:` URIs so the Markdown is a
single self-contained string. If you do not want large base64 blocks, set **Image
handling** to **placeholder** for `*[image output]*` notes or **omit** to drop image
outputs entirely. A sidecar `_files/` image directory is not produced because this
tool returns one Markdown value.

</details>

<details>
<summary>Can it keep outputs but hide the code?</summary>

Yes. Turn off **Include code cells** and leave **Include stored outputs** on. That
matches the common no-input export shape: prose plus outputs, without the source
code that produced them.

</details>

<details>
<summary>Does it execute the notebook?</summary>

No. It reads the JSON already stored in the `.ipynb` file. Only outputs that are
present in the notebook are rendered. If a notebook was saved after clearing its
outputs, this converter cannot recreate them.

</details>

<details>
<summary>Why are some rich outputs rendered as HTML?</summary>

Notebook outputs can carry many MIME representations. Markdown is chosen first,
then images, then HTML, then LaTeX, then plain text. Keeping HTML is useful for
DataFrame tables and other rich displays because raw HTML is valid inside Markdown.

</details>

### Limits & edge cases

- Input must be valid Jupyter notebook JSON with a `cells` array (nbformat v4 is
  the primary target; legacy v3 markdown cells are tolerated).
- The tool does not run code, fetch external files, or evaluate widgets.
- Very large embedded images can produce large Markdown because base64 image data
  is included inline when `image_mode=embed`.
- Binary sidecar files from `nbconvert` are represented as inline data URIs,
  placeholders, or omitted; no ZIP or `_files/` directory is generated.
- Unknown cell types are ignored, and empty cells are skipped.
