# lua-formatter competitor analysis — 2026-07-28

## Sources scanned

| Tool | Visible table-stakes | In-model decisions |
| --- | --- | --- |
| OnlineDevTools Lua Formatter | Paste/code textarea, sample input, indentation choices such as 2 spaces / 4 spaces / tabs, instant browser-local formatting. | Browser-local textarea, preset examples, indent width, and tabs are in-model and implemented. |
| SwapCode Lua Formatter | Online Lua beautifier positioned around readable formatting, client-side execution, handling messy/minified Lua. | Local WebAssembly execution, multiline input, beautified output, and messy/minified examples are in-model and implemented. |
| DecodeIt Lua Beautifier | Formatting Lua scripts with proper indentation/structure for Lua 5.4/game scripts; direct copy/paste workflow. | Dialect-agnostic tokenizer, Lua 5.1-5.4/LuaJIT/Luau copy, and direct copy/paste page workflow are in-model and implemented. |

## Table-stakes matrix

| Capability / UX pattern | Fit | Decision |
| --- | --- | --- |
| Multiline source textarea | In model | `input` is required, multiline, with Lua placeholder and examples. |
| 2-space / 4-space / tab presets | In model | `indent` integer supports 1-8 spaces; `indent_char` enum supports `space` and `tab`; examples cover 2-space, 4-space, and tabs. |
| Client-side formatting / privacy | In model | Web wrapper uses wasm-bindgen over pure Rust core; page copy states local execution. |
| Preserve comments and strings safely | In model | Tokenizer preserves long strings, line comments, and long comments; content documents this limit. |
| Quote normalization | In model | `quote_style` enum supports `preserve`, `double`, and `single` with escape handling. |
| Syntax highlighting editor | Out of model for this repo page | The generic page renderer exposes textarea/select/number controls, not a Monaco-style code editor. Documented as not required for correctness. |
| Full AST pretty-printing / line wrapping | Out of model for current implementation | Would require a dialect-specific Lua parser/printer and policy choices. Tool is explicitly a conservative re-indenter that does not wrap lines. |
| Syntax error detection | Out of model by design | A forgiving formatter should not reject LuaJIT/Luau/extensions; FAQ tells users to validate syntax with a Lua interpreter. |

## Defaults chosen

- `indent = 2`: common Lua style and table-stake default among online formatters.
- `indent_char = space`: safest display across browsers and terminals.
- `quote_style = preserve`: avoids surprising source changes unless the user opts in.

## Verification requirements derived from scan

- Page and CLI must show exact re-indented output for a basic `if` block.
- Non-default 4-space and tab indentation must be exercised.
- Quote-style enum choices (`preserve`, `double`, `single`) must be exercised.
- Boundary indent value `8` must be exercised.
- Page spec must include a query-param deep link and assert real output.
