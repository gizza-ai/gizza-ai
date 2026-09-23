# latex-to-text — competitor analysis (2026-09-23)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are paraphrased
behaviour notes taken from public documentation; no competitor copy, branding, or trademark text
is reused anywhere in the tool, its page, or its tests.

Backlog row: `latex-to-text,text,Strips LaTeX commands and environments to recover readable plain
text from a .tex source.,…,pure` (tools-to-build.csv:1727).

## Competitors reviewed

| # | Tool | Kind | Reachable | Notes |
|---|------|------|-----------|-------|
| 1 | **detex / OpenDetex** | CLI, shipped in TeX distributions since the 1980s | yes (man page) | The canonical reference implementation; its flag set is the de-facto feature checklist |
| 2 | **pylatexenc `latex2text`** | Python library + CLI | yes (readthedocs) | The most configurable of the three; richest math + unicode handling |
| 3 | **FormaTeX "LaTeX to TXT"** | Browser tool (paste or upload, copy/download result) | yes | The closest UX analogue to a gizza tool page |
| 4 | **PyDetex** | Python GUI/CLI | partially (PyPI 503; GitHub README thin) | "simple" vs "strict" pipelines, repeated-word check, unicode symbol replacement |

Supplementary context: Vertopal and reformatfile.com offer the same paste-or-upload → plain-text
conversion with no exposed options, so they set no additional table stakes.

## Table-stakes extracted, and where each one landed

| Capability | Seen in | Fit | Where it landed |
|---|---|---|---|
| Strip commands/macros but keep the visible argument text (`\textbf{x}` → `x`, `\section{T}` → `T`) | all | in-model | Default behaviour of the core tokenizer |
| Drop `%` comments (respecting `\%`) | detex, pylatexenc (`keep_comments=False`) | in-model | `keep_comments` (default `false`) |
| Remove math content by default | detex (default), pylatexenc (`math_mode`) | in-model | `math = "remove"` (default) |
| Keep math source instead of dropping it | pylatexenc `math_mode='verbatim'/'with-delimiters'` | in-model | `math = "keep"` |
| Substitute a placeholder for math so sentences stay readable | detex `-r` | in-model | `math = "placeholder"` (emits `[math]`) |
| Configurable list of environments whose *content* is discarded (detex default: array, eqnarray, equation, longtable, picture, tabular, verbatim) | detex `-e` | in-model | `drop_environments`, a comma list rendered as a `tag-list` control; default covers detex's list plus modern math/code environments |
| Echo `\cite` / `\ref` / `\pageref` arguments instead of dropping them | detex `-c` | in-model | `citations = "drop"` (default) / `"keys"` |
| Convert LaTeX accents and symbol macros to Unicode (`\^e` → `ê`, `\ldots` → `…`, `---` → `—`) | pylatexenc, PyDetex | in-model | `unicode` (default `true`); `false` emits ASCII fallbacks |
| Reflow into readable paragraphs vs preserve source line breaks | pylatexenc `fill_text` | in-model | `line_breaks = "paragraphs"` (default) / `"source"` |
| Skip the preamble and convert only the document body | implied by detex's `\begin{document}` LaTeX-mode detection | in-model | `body_only` (default `true`) |
| Paste **or** upload, then copy/download the result, no account | FormaTeX, Vertopal | in-model | The generic tool page already provides paste, Copy, Download, and Reset for `format = "text"` |
| One-click worked examples | FormaTeX-style "try it" affordance | in-model | Four `[[example]]` preset chips in `page/meta.toml` |

## Deliberately NOT built (out of model / out of scope) — listed, not implemented

- **Following `\input` / `\include` into other files** (detex default; `-n` disables). A gizza block
  is a sandboxed single-input function with no filesystem, so multi-file projects can't be
  resolved. The page and the descriptor state that these lines are dropped, and the FAQ tells users
  to concatenate their chapters first.
- **Expanding user-defined `\newcommand` / `\def` macros.** Real expansion needs a TeX engine
  (this is the same reason `latex-compile` is already skiplisted). Definitions are dropped; the
  uses fall back to the generic "keep the braced argument text" rule, which is documented on the
  page as a known limit.
- **Resolving citations against a `.bib` file.** Needs a second input plus BibTeX resolution;
  `citations = "keys"` emits the raw keys instead. (`blocks/bibtex-to-csv` and
  `blocks/ris-bibtex-converter` already cover the bibliography side.)
- **`detex -w` one-word-per-line output and word counts.** Deliberately left to the existing
  `blocks/text-statistics` / `blocks/word-frequency` tools rather than duplicated here.
- **PyDetex's repeated-word detection, dictionary/synonyms, and language detection.** A different
  product (a writing assistant), not a converter; several need per-language lexicons.
- **`detex -t` plain-TeX mode and `-s` control-sequences-to-spaces.** Legacy compatibility modes —
  the man page itself notes `-s` can damage accented words, which is exactly what `unicode = true`
  is for. Skipped as anti-features rather than gaps.
- **File upload of a whole project / REST API** (FormaTeX). Out of scope for this repo: the page
  takes a paste, and the CLI plus the chat block are the automation surfaces.

## Verification performed

- `cargo test --workspace` in `blocks/latex-to-text` (core happy-path + error tests, descriptor
  drift guard).
- `scripts/build-block-wasm.sh latex-to-text`, `wasm-pack build … --target web --release`,
  `cargo install --path cli`, `python3 scripts/sync-tool-manifest.py latex-to-text`, page
  generation, `python3 scripts/check-tool-hygiene.py latex-to-text`.
- CLI runs including one exact-output case and the page's generated CLI example verbatim.
- Playwright `tests/tool-page-latex-to-text.spec.ts`: default run, one real run per `math` choice,
  `citations=keys`, a non-default checkbox state (`unicode` off), `line_breaks=source`, the
  1,000,000-character cap boundary, and a `?param=` deep link.
