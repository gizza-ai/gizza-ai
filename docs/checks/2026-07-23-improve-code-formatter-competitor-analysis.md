# code-formatter — competitor analysis (2026-07-23)

Tool: `code-formatter` — beautify and re-indent an HTML, CSS, JavaScript, or JSON
snippet. Auto-detect the language or pick one; choose spaces (1–8) or tabs.
Whitespace-only for HTML/CSS/JS; JSON is validated and re-serialized with key
order preserved.

## Competitors scanned

1. **Prettier** (`prettier/prettier`, prettier.io/playground) — the dominant
   opinionated code formatter. Reflows and rewrites (quote style, trailing
   commas, print width, semicolons) across many languages. Node/CLI + web
   playground.
2. **js-beautify** (`beautify-web/js-beautify`, beautifier.io) — classic
   whitespace-only beautifier for JS/HTML/CSS with configurable indent size and
   char. Web UI + npm. Closest analogue to this tool's philosophy.
3. **CodeBeautify — Code Beautifier** (codebeautify.org) — a multi-language web
   beautifier (JSON, HTML, CSS, JS, XML, SQL, …) with a language picker and
   indent options. Broad language menu.
4. **FreeFormatter** (freeformatter.com) — separate per-language formatter pages
   (HTML/XML/CSS/JS/JSON) with indent controls.
5. **JSONLint / JSON Formatter** (jsonlint.com, jsonformatter.org) — validate and
   pretty-print JSON with indent selection; reports parse errors.

## Table-stakes → decision

| Capability | Competitor(s) | In our model? | Where it lands |
|---|---|---|---|
| Pretty-print / re-indent JSON | all, JSONLint | yes | `language=json` or `auto` |
| Beautify HTML | js-beautify, CodeBeautify, FreeFormatter | yes | `language=html` or `auto` |
| Beautify CSS | js-beautify, CodeBeautify, FreeFormatter | yes | `language=css` or `auto` |
| Beautify JavaScript | Prettier, js-beautify, CodeBeautify | yes | `language=javascript` or `auto` |
| Auto-detect language from snippet | CodeBeautify (partial) | yes (differentiator) | `language=auto` (default) |
| Configurable indent size | all | yes | `indent` (1–8 spaces) |
| Tabs vs spaces | js-beautify, Prettier | yes | `indent_char=space\|tab` |
| Preserve JSON key order | JSONLint, js-beautify | yes | serde_json `preserve_order` |
| Validate JSON, report parse errors | JSONLint, JSON Formatter | yes | `invalid JSON: …` error |
| Opinionated rewrites (quotes, trailing commas, print width, semicolons) | Prettier | **out-of-model** | this is a whitespace-only beautifier; it never rewrites tokens |
| Minify / compact mode | CodeBeautify, FreeFormatter | **out-of-model (this pass)** | gizza has separate minify-oriented tools; this one only expands |
| Extra languages (XML, SQL, YAML, Markdown, Java, …) | Prettier, CodeBeautify | **out-of-model** | scope is the four web languages; other formats are separate tools |
| Line-wrap to a max print width | Prettier | **out-of-model** | only indentation/line-breaks by structure, no width reflow |
| Sort object keys / lint | some JSON tools | **out-of-model** | never reorders keys; not a linter |

## Design chosen

- `code` (string, required) — the snippet to beautify.
- `language` (enum, default `auto`): `auto | html | css | javascript | json`.
  `auto` recognizes HTML (leading `<`) and valid JSON reliably; CSS vs JS is a
  heuristic that falls back to JavaScript.
- `indent` (integer, default `2`, range 1–8) — spaces per nesting level; ignored
  when `indent_char=tab`.
- `indent_char` (enum, default `space`): `space | tab`.
- HTML/CSS/JS change whitespace only (nothing renamed, reordered, or dropped);
  JSON is parsed, validated, and re-serialized with key order preserved. Raw HTML
  regions (`pre`/`textarea`/`script`/`style`) and CSS strings/comments are kept
  verbatim.

## Not a duplicate

Existing gizza JSON blocks target conversion or querying (`json-to-sql-insert`,
`csv-to-sql`, `format-validator`), not multi-language beautification. No existing
block re-indents HTML, CSS, and JavaScript behind one auto-detecting interface,
and the JS pretty-printer core (`js-beautify`) had no standalone formatter tool
exposing all four languages. The unified, whitespace-only, key-order-preserving
formatter is the distinct capability here.

Copy/branding note: no competitor copy, wording, or trademarks were reused — only
the capability set was analysed.
