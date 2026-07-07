## About this tool

**Files to Prompt** bundles several files into a single, LLM-ready block of text —
the kind you paste into Claude, ChatGPT, or any other model when you want it to see a
whole set of files at once. You get three things in one digest:

- a **directory tree** so the model sees the layout at a glance,
- each file's **contents**, rendered as Markdown fenced code blocks, a Claude-style
  `<documents>` XML wrapper, or plain `path` + `---` rules, and
- a rough **token estimate** so you can judge how much of the context window it uses.

Everything runs **in your browser** via WebAssembly — your files are never uploaded.
The same tool is available from the [gizza CLI](/) and in chat.

### How to paste your files

This tool takes text, not a folder, so each file is introduced by a **header line**:
the separator (default `===`) followed by the file's path. Everything up to the next
header is that file's content:

```
=== src/greet.py
def greet(name):
    return f"Hello, {name}!"

=== README.md
# Demo
```

With **Markdown** output and the directory tree on, that becomes:

````
Directory structure:
├── README.md
└── src
    └── greet.py

## src/greet.py
```python
def greet(name):
    return f"Hello, {name}!"
```

## README.md
```markdown
# Demo
```

2 files, 173 characters, ~44 tokens (estimate)
````

The tree is sorted; the files stay in the order you pasted them; and the fence
language (`python`, `markdown`, …) is picked from each file's extension.

### Output formats

- **Markdown** — a `## path` heading and a language-detected code fence per file.
  If a file already contains a ``` fence, the wrapper automatically uses more
  backticks so it isn't closed early.
- **XML** — one `<documents>` block containing an indexed `<document>` per file
  (`<source>` + `<document_contents>`). This is the long-context layout Anthropic
  suggests for Claude.
- **Plain** — the `path`, a `---` rule, the contents, and another `---`, per file.

### Limits and edge cases

- **Text only, no crawling.** This runs in your browser, so it can't read a folder,
  follow a repo, or apply `.gitignore` — you paste exactly the files you want.
- The token figure is an **estimate** (≈ characters ÷ 4), not an exact model
  tokenizer, so treat it as a ballpark.
- A run of blank lines directly above or below a file's content is trimmed.
- If your file contents contain lines that start with `===`, change the separator
  (e.g. to `>>>`) so those lines aren't mistaken for new file headers.

## FAQ

<details>
<summary>How do I separate one file from the next?</summary>

Start each file with a **header line**: the separator followed by the path, like
`=== src/main.rs`. Everything after that line, up to the next header, is that file's
content. A trailing separator is fine too (`=== src/main.rs ===`). The default
separator is `===`, but you can change it to anything (for example `>>>`) if your
files contain lines that begin with `===`.

</details>

<details>
<summary>Is the token count exact?</summary>

No — it's a quick estimate based on roughly **4 characters per token**, the common
rule of thumb. Real tokenizers (Claude's, GPT's) split text with model-specific
tables that would add megabytes to a browser tool, so we don't ship one. The estimate
is close enough to judge whether a bundle fits a context window, but for exact
accounting use your model provider's tokenizer.

</details>

<details>
<summary>Which output format should I use?</summary>

**Markdown** is the safe default and reads well everywhere. **XML** wraps the files
in a `<documents>` structure that Anthropic recommends for long-context prompts to
Claude. **Plain** is the classic `path` + `---` style if you want the least markup.
All three include the same directory tree and token estimate.

</details>

<details>
<summary>Can it read a whole repository or folder?</summary>

No. It runs entirely in your browser with no filesystem or network access, so it
can't walk a directory, clone a repo, or honor `.gitignore`. Copy in the files you
want to include (each under its own `=== path` header). For crawling an actual repo,
a command-line packer is the right tool; this one keeps everything local and private.

</details>

<details>
<summary>Does it upload my files anywhere?</summary>

No. All processing happens locally via WebAssembly — the text you paste never leaves
your device. You can confirm it works offline once the page has loaded.

</details>
