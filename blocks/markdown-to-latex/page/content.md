## Markdown to LaTeX, right in your browser

Paste a Markdown document and get clean, ready-to-compile **LaTeX** source. The
conversion runs entirely on your device with WebAssembly — your text is never
uploaded, there is no account, and there are no limits. Copy the result straight
into Overleaf, a local TeX editor, or an existing `.tex` file.

## What gets converted

- **Headings** — both ATX (`#` … `######`) and setext (a line underlined with
  `===` or `---`) map to `\section`, `\subsection`, `\subsubsection`,
  `\paragraph`, and `\subparagraph`. Use the **heading offset** option to demote
  every heading by 1–5 levels when you are pasting into an existing chapter.
- **Lists** — bullet, numbered, and nested lists become `itemize` / `enumerate`
  environments. GitHub **task lists** (`- [x]` / `- [ ]`) render with check and
  empty-box markers.
- **Tables** — GitHub pipe tables become a `tabular` with `booktabs` rules
  (`\toprule` / `\midrule` / `\bottomrule`) and per-column `l`/`c`/`r` alignment
  taken from the `:---:` delimiter row.
- **Code** — fenced blocks become `lstlisting` (with the language passed through
  when you tag the fence), and indented blocks become `verbatim`. Code is emitted
  literally, so the LaTeX special characters inside it are left untouched.
- **Inline formatting** — `**bold**`, `*italic*`, and `` `code` `` become
  `\textbf`, `\textit`, and `\texttt`, and `~~strikethrough~~` becomes `\sout`.
- **Footnotes** — a `[^id]` reference plus a `[^id]: text` definition line
  becomes an inline `\footnote{…}`.
- **Links and images** — `[text](url)` becomes `\href`, `![alt](src)` becomes
  `\includegraphics`, and `<https://…>` autolinks become `\url`.
- **Blockquotes** become a `quote` environment, and `---` rules become a
  full-width horizontal line.

## Math and special characters

Inline `$…$` and display `$$…$$` math is passed through **verbatim**, so your
equations land in the output exactly as written. Everywhere else, the ten LaTeX
special characters (`& % $ # _ { } ~ ^ \`) in your prose are escaped
automatically — so a `100%` discount or a `file_name.txt` won't break the build.

## Fragment or full document

Leave **Wrap in a full document** off to get just the body, ideal for pasting
into a file you already have. Turn it on to get a complete, compilable file with
a `\documentclass{article}` preamble that already loads `hyperref`, `graphicx`,
`listings`, `booktabs`, `array`, and `ulem` — everything the converted
constructs need.

## Private by design

Like every gizza tool, the conversion happens locally in your browser. Nothing
is sent to a server, so it works offline and your documents stay yours.
