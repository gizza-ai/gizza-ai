## About this tool

LaTeX is excellent for typesetting, but it is noisy when you need plain prose for review, indexing, search, summaries, accessibility checks, or copy editing. This converter turns a `.tex` source into readable text without running a TeX engine: commands are removed, visible argument text is kept, comments and preamble material are dropped by default, and common accents and symbols become Unicode.

The default output is intentionally close to classic `detex`: math is removed, citation and reference commands disappear, and table/code/picture environments are skipped. The controls let you keep math source, replace formulas with `[math]`, keep citation keys, preserve source line breaks, include comments, include preamble metadata, or override the list of environments whose contents should be discarded.

### Worked example

Input:

```tex
\documentclass{article}
\title{On Ducks}
\begin{document}
\section{Introduction}
The \textbf{mallard} is a common duck~\cite{smith2024}.
It swims at $v = 3$ m/s.
\end{document}
```

Default output:

```text
Introduction

The mallard is a common duck. It swims at m/s.
```

With `citations = keys` and `math = placeholder`, the same fragment keeps more structure:

```text
Introduction

The mallard is a common duck smith2024. It swims at [math] m/s.
```

### Limits and edge cases

- This is **not a TeX engine**. It does not expand user-defined `\newcommand`/`\def` macros or evaluate package logic.
- `\input` and `\include` are not followed. Paste a concatenated source if your document is split across files.
- Bibliography resolution is out of scope. `citations = keys` keeps raw keys; it does not look up `.bib` titles or authors.
- The environment drop list is for non-prose blocks. Math environments (`equation`, `align`, `gather`, and friends) follow the Math handling setting instead.
- Input is capped at 1,000,000 characters. Convert a thesis one chapter at a time if needed.

## FAQ

<details>
<summary>Does this compile my LaTeX?</summary>

No. It tokenizes the source directly and recovers readable text. That makes it fast and safe in the browser, CLI, and chat block, but it also means package macros and user-defined commands are not expanded like a real TeX run.

</details>

<details>
<summary>Why did my formula disappear?</summary>

The default math mode is `remove`, matching the common "detex for prose" workflow. Choose `placeholder` to leave `[math]` markers in the sentence, or `keep` to preserve the original math source with its delimiters.

</details>

<details>
<summary>Can I keep citation or reference keys?</summary>

Yes. Set `citations` to `keys` and commands such as `\cite{knuth1984}` or `\ref{fig:one}` emit the raw key. The converter does not read `.bib` files, so it cannot replace keys with formatted references.

</details>

<details>
<summary>What happens to tables, code listings, and TikZ pictures?</summary>

Their contents are dropped by default because they are usually not prose. Edit the Dropped environments tag list if you want to keep text from a specific environment, for example removing `tabular` from the list to recover cell text.

</details>

<details>
<summary>Is my source uploaded?</summary>

No. The page runs the converter in WebAssembly in your browser. The CLI and chat tool use the same pure Rust core, so the conversion logic is identical across surfaces.

</details>
