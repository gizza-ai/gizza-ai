# ansi-log-renderer — competitor analysis (2026-07-25)

Tool function: parse raw terminal output containing ANSI escape codes and render it to
clean colored HTML, or strip the codes to plain text.

One WebSearch performed; top real tools skimmed (paraphrased notes only, no copied copy):

## Competitors skimmed

1. **drudru/ansi_up** (JS lib, widely embedded in web log viewers)
   - Interprets SGR codes → colored HTML `<span>`s. Zero deps.
   - Supports 16 basic colors, 256 indexed colors, and 24-bit RGB truecolor.
   - Styles: bold, italic, underline, blink, inverse/reverse, dim, strikethrough.
   - Option `use_classes` — emit `class="ansi-..."` (needs an external stylesheet) instead of
     inline `style="color:.."`. Escapes HTML entities in the text.
   - Also decodes OSC-8 hyperlinks into `<a>` (opt-in).

2. **ansi-to-html** (npm, port of bcat's converter)
   - Options: default `fg`/`bg` colors, `newline` (convert `\n`→`<br>`), `escapeXML`,
     custom `colors` palette override, `stream` mode.
   - Emits inline-styled spans; 16 + 256 color support.

3. **ansi2html** (Python, `pycontribs`)
   - `--inline` (inline styles) vs default stylesheet/class output; `--partial` (fragment)
     vs a full standalone HTML **document**; dark/light **background scheme**; `--escape`.
   - Full 16 / 256 / 24-bit RGB support; renders inside a `<pre>`.

4. **buildkite/terminal-to-html** (Go, used in CI log rendering)
   - Renders arbitrary shell output (incl. cursor movement / carriage-return overwrites)
     to HTML; 8 + 256 colors; wraps runs in styled spans; emits self-contained markup.

## Table-stakes params / defaults / decisions

| Capability | Competitors | gizza decision |
| --- | --- | --- |
| Output: colored HTML vs strip to plain text | all (converters); strip is our sibling tool | **in-model** — `output` enum `html`\|`text`, default `html` |
| 16 basic + bright colors | all | **in-model** — full xterm 16-color palette |
| 256 indexed colors | ansi_up, ansi2html, terminal-to-html | **in-model** — 6×6×6 cube + grayscale ramp |
| 24-bit RGB truecolor (`38;2;r;g;b`) | ansi_up, ansi2html | **in-model** |
| Styles: bold/dim/italic/underline/inverse/strike/conceal | all | **in-model** |
| Inline styles vs CSS classes | ansi_up `use_classes`, ansi2html | **in-model** — `styles` enum `inline`\|`classes` |
| Dark/light background theme | ansi2html | **in-model** — `theme` enum `dark`\|`light` (default fg/bg + `<pre>` bg) |
| HTML entity escaping | all | **in-model** — always escape `& < >` in text content |
| `<pre>`-wrapped, self-contained output | all | **in-model** — wrap in a themed `<pre>`; classes mode emits a `<style>` block |
| OSC-8 hyperlink → `<a>` | ansi_up (opt-in), ansi-to-pre | **considered, not built (v1)** — non-SGR OSC stripped; noted as a limit |
| Full standalone `<!DOCTYPE html>` document | ansi2html `--partial` | **considered, rejected** — the `<pre>` fragment already renders standalone when pasted; a full-doc toggle adds surface with little value in a paste-and-copy tool |
| Cursor-move / carriage-return overwrite emulation | terminal-to-html | **considered, not built** — non-SGR control sequences are stripped (not replayed); noted as a limit |
| Custom palette override, streaming | ansi-to-html | **out-of-model** — niche/library-only |

## UX controls competitors ship

- Paste-in textarea for raw output; live preview of the rendered HTML.
- A toggle between rendered preview and the raw HTML source (we output HTML text with a
  Copy button + the page shows it as text — user copies the markup).
- Theme switch (dark/light). Preset examples.

No competitor copy, branding, or trademarks reused — original implementation and copy only.
