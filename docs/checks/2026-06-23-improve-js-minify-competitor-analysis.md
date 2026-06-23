# js-minify — competitor analysis (2026-06-23)

Tool: **JavaScript Minifier** (`blocks/js-minify`). Pure-Rust, dependency-free,
token-aware minifier. Removes unnecessary whitespace, line breaks and comments
while preserving behavior (semantically identical output). Surfaces: chat skill,
CLI, standalone page (all verified).

## Top competitors surveyed

1. **Terser** (`terser.org`, the de-facto modern minifier; UglifyJS fork, ES6+).
2. **UglifyJS 3** (`github.com/mishoo/UglifyJS`, classic parser/mangler/compressor).
3. **minify-js.com** — browser-based online minifier (uses Terser, "runs locally").
4. **FWD Tools — JavaScript Minifier** (`fwdtools.com/javascript-minifier/`).
5. **esbuild** minifier (benchmark reference) / babel-minify.

## Capability diff

| Capability | Competitors | gizza js-minify | Notes |
|---|---|---|---|
| Strip whitespace / line breaks | yes | **yes** | token-aware, never welds tokens |
| Remove comments | yes | **yes** (`remove_comments`, default on) | |
| Preserve license/banner comments | Terser `comments:/^!/`, online "keep license comments" | **yes** (`keep_license`, default on) | `/*! … */`, `@license`, `@preserve` survive — **gap closed this run** |
| Strings/regex/template kept verbatim | yes | **yes** | regex-vs-division disambiguation; `$\{…\}` interpolation handled |
| ASI-safe (newline after `return`, postfix `++`) | yes (via real parser) | **yes** (conservative newline preservation) | keeps a real `\n` where ASI could change meaning |
| Runs locally / privacy | minify-js.com, FWD claim local | **yes** (WebAssembly, nothing uploaded) | matches the strongest privacy claim |
| Identifier / variable **mangling** | Terser/UglifyJS `--mangle` | **no** — out of model | needs a full scope-aware parser (rename without collisions); not feasible as a whitespace-token minifier. Documented as a non-goal. |
| Dead-code elimination / constant folding (`--compress`) | Terser/UglifyJS | **no** — out of model | requires full AST + control-flow analysis |
| `drop_console`, `keep_classnames`, etc. | Terser options | **no** — out of model | depend on the AST compressor |
| Concatenate multiple files / IIFE wrap | online tools | **no** — out of model | the page/chat take a single source input; multi-file is a build-tool concern |
| Remove optional semicolons | some online tools | **no** — out of model | needs statement-boundary understanding (ASI rules) to be safe; the conservative minifier leaves semicolons in place |

## Gaps closed this run

- **Preserve license/banner comments** (`keep_license`, default true): even when
  `remove_comments` strips comments, `/*! … */`, `@license` and `@preserve`
  banner comments are kept — matching Terser's default and online tools' "keep
  license comments" option. Added as a core flag + chat/CLI param + page
  checkbox, with unit, CLI and Playwright coverage.

## Out-of-model features (intentionally NOT built)

These all require a full JavaScript parser/AST + scope analysis, which is far
beyond a whitespace/comment minifier and would be a different tool:

- Identifier/variable **mangling** (shortening names).
- **Compression**: dead-code elimination, constant folding, `drop_console`,
  inlining, `keep_classnames`/`keep_fnames`, etc.
- Removing "optional" semicolons (unsafe without full ASI modelling).
- Multi-file concatenation / IIFE wrapping (single-input surfaces; a build-tool
  concern).

The tool is deliberately a **safe, behavior-preserving** minifier: identifiers
are never renamed and code is never reordered or dropped, so output is always
semantically identical to input — the property the AST-rewriting competitors
trade away for extra size at the risk of subtle breakage.

## Verification (all surfaces)

- `cargo test --workspace`: 20 core tests + 1 chat-schema drift test — pass.
- `wafer build`: chat `block.wasm` validates (306.8 KiB).
- CLI (`gizza tool js-minify …`): whitespace strip, comment removal, regex
  safety, `remove_comments=false`, `keep_license` on/off — all correct.
- Page Playwright (`tool-page-js-minify.spec.ts`): default minify, keep-comments,
  and license-banner preservation — 3/3 pass.

No competitor copy, branding or trademarks were used.
