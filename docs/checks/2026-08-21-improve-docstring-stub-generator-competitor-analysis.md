# docstring-stub-generator — competitor analysis (2026-08-21)

Scan run before implementing. All findings are paraphrased from public documentation; no
competitor copy, branding or trademark is reproduced here or in the tool.

## Competitors reviewed

| # | Tool | Surface | Why it matters |
|---|------|---------|----------------|
| 1 | autoDocstring (VS Code extension, `njpwerner.autodocstring`) | Editor | The reference implementation for signature → Python docstring. Deterministic, no model. |
| 2 | VS DocBlockr (VS Code extension, `jeremyljackson.vs-docblock`) | Editor | Same idea for the tag-block family (JS/TS, PHP, Java, C, SCSS, Vue). |
| 3 | WebStorm "insert documentation comment stub" | IDE | Built-in JSDoc stub generation; sets the baseline expectation for JS/TS output. |
| 4 | ToolWard "Code Comment Generator" | Web page | The closest *paste-in-a-box* web competitor; multi-language, AI-backed. |

(The fourth slot originally targeted `jsdoc-online-generator.com`, which does not resolve
(`ENOTFOUND`) — replaced with ToolWard rather than running with three.)

## Table-stakes extracted

| Capability | Seen in | Fit | Where it landed |
|---|---|---|---|
| Multiple Python docstring conventions (Google, NumPy, Sphinx/reST, Epytext, PEP 257) | 1 | in-model | `style` enum |
| "no types" variant of every convention | 1 (`-notypes` formats) | in-model | `types = none` |
| Type inference from annotations, from default values, and a placeholder otherwise | 1 (`guessTypes`) | in-model | `types = guess` / `annotated` |
| Configurable description placeholder text | 1 (mustache templates, `_description_`) | in-model | `placeholder` |
| Quote style for Python (`"""` vs `'''`) | 1 (`quoteStyle`) | in-model | `quote_style` |
| Extended-summary paragraph slot | 1 (`includeExtendedSummary`) | in-model | `extended_summary` |
| `*args` / `**kwargs`, decorators, return annotations, yields | 1 | in-model | parser handles `*`/`**`/`/` markers, decorator lines, `-> T` |
| Tag-block languages: JS/TS (JSDoc), PHP (PHPDoc), Java (Javadoc), C# (XML doc) | 2, 3, 4 | in-model | `language` enum |
| Go (godoc) and Ruby (YARD) conventions | 4 | in-model | `language` enum |
| Rust (rustdoc `# Arguments` / `# Errors`) | — (our own ecosystem) | in-model | `language` enum |
| Align `@param` type/name columns | 2 ("Align Tags") | in-model | `align_tags` |
| Include / exclude the return tag | 2 ("Default Return Tag") | in-model | falls out of `types`/parse — a void/`None` return emits no return tag |
| `@throws` / `Raises:` / `@raise` section | 3, 4 | in-model | `raises` (comma-separated names) |
| `@example` / `Examples:` section | 4 (ecosystem conventions) | in-model | `examples` |
| Configurable indentation | 2 ("Column Spacing"), editor tab settings | in-model | `indent_size` |
| Emit the stub merged back into the pasted signature, ready to paste | 2, 3, 4 | in-model | `output = annotated` (default) |
| Machine-readable parse of the signature | — | in-model bonus | `output = json` |
| Deep-linkable / shareable settings | 4 (share + embed) | in-model | every param is a `?query=` param on the page |
| **AI-written prose** descriptions instead of placeholders | 4, and every "AI comment generator" | **out-of-model** | not built — gizza blocks are deterministic pure Rust with no model. The tool generates *stubs*; the prose is the author's job. Stated on the page. |
| Reading a whole file / repository and documenting every function in place | 1, 2, 3 (editor integration) | **out-of-model** | not built — a browser/CLI block takes one pasted input. Stated on the page. |
| Full language parsers (bodies, `raise` statements auto-detected, yields) | 1 (parses the function body for `raise`) | **partly out-of-model** | we only get a *signature*, so raised errors are user-declared via `raises`. Stated on the page + in the `raises` description. |
| CSV / PDF export, login, social share | 4 | **out-of-model** | not built — the page is brand-free and has a plain copy/download affordance from the generic runtime. |
| Custom user templates (mustache) | 1 (`customTemplatePath`) | **out-of-model** | not built — would need a template engine and a file input; the six built-in conventions cover the documented formats. |

## Feasibility spikes

* **Parsing without a language grammar** — tree-sitter/ANTLR grammars are C libraries that do not
  instantiate in the wasm sandbox (same constraint recorded for `code-comment-extractor`). Verified
  the target is reachable with a hand-written, string-literal-aware, depth-tracking splitter: a
  signature is a name token followed by a balanced paren group, and per-language parameter shapes
  (`name: T = d`, `T $name`, `T name`, `a, b string`, `...rest`) are regular enough to parse
  deterministically. Built that way — no dependency beyond `serde_json`.
* **Angle-bracket generics** (`Map<String, Integer> m`, `Result<T, E>`) break a naive comma split.
  Tracked as an extra depth level for the languages that use them.
* **Go grouped parameters** (`func f(a, b string)`) share one type — handled by back-filling the
  type from the next typed chunk.

## Decisions

* Default `output = annotated` (signature + stub, ready to paste) because every editor competitor
  inserts in place; `docstring` returns the bare stub and `json` the parsed structure.
* `style` covers the five Python conventions plus `auto`; tag-based languages have exactly one
  native convention each, so `style` is documented as Python-only rather than exposing invalid
  language/style combinations.
* Presets ship as `[[example]]` chips (Python Google, NumPy no-types, JSDoc aligned, PHPDoc)
  because the editor competitors ship format presets.
