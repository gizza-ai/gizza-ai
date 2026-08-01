# ast-diff — competitor analysis (2026-07-31)

Tool: structural diff for Rust source. It parses two inputs, canonicalizes their ASTs, and diffs the canonical forms so formatting, whitespace, and ordinary comments do not appear as changes.

## Competitors scanned (paraphrased)

1. **SemanticDiff-style hosted code review tools**. These emphasize language-aware diffs, moved-code detection, syntax-aware views, and repository/PR workflows. Table-stakes: unified and side-by-side views, ignore formatting, syntax awareness, language selection, and clear parse errors. Repository integration and multi-file navigation are outside this local block's model.

2. **Difftastic (`difft`)**. A CLI syntax-aware diff tool that parses many languages and compares syntax trees. It highlights changed syntax, ignores some formatting churn, supports many file types through tree-sitter grammars, and is used from terminals/Git. Table-stakes: language-aware parsing, readable hunks, fallback/error behavior, and configurable context. Multi-language support and Git integration are broader than this tool.

3. **GumTree / AST differencing research tools**. AST matching engines used for source-code edit scripts, move detection, and refactoring-aware analysis. Table-stakes: parse both versions, compare syntax nodes rather than bytes, summarize structural changes, and report when parsing fails. Fine-grained edit scripts and move detection are powerful but too complex for a small deterministic browser utility.

4. **IDE compare views (JetBrains / VS Code extensions)**. IDEs can ignore whitespace and sometimes provide language-aware navigation. Table-stakes: ignore whitespace, show changed code, work locally, and provide a quick answer for small snippets. IDE project indexing, navigation, and refactoring awareness are out of scope.

## Table-stakes and decisions

| Capability | In this tool? | Decision |
| --- | --- | --- |
| Ignore whitespace / formatting-only churn | yes | Parse Rust with `syn`, pretty-print with `prettyplease`, then compare canonical lines. |
| Ignore ordinary comments | yes | Parser discards trivia; documented clearly. |
| Show unified diff | yes | Default `mode=unified`, with configurable context lines. |
| Quick verdict / summary | yes | `mode=summary` distinguishes identical, formatting-only, and changed line counts. |
| Canonicalized source preview | yes | `canonical-a` and `canonical-b` expose the normalized form being diffed. |
| Clear parse errors | yes | Errors identify source A/B and include line/column when available. |
| Multi-language AST diff | no | Out-of-model for this block; would require bundling and maintaining multiple parsers/canonical printers. |
| Move detection / AST edit script | no | Out-of-model for a compact deterministic web tool; canonical line diff is easier to explain and verify. |
| Git/PR/repository workflow | no | Out-of-model; this is a paste-in browser/CLI utility. |
| Side-by-side visual diff | no | The page surface is single-output text; unified diff is portable and copyable. |

## In-model feature set shipped

- Rust-only structural comparison using wasm-safe crates (`syn`, `prettyplease`).
- Modes: `unified`, `summary`, `canonical-a`, `canonical-b`.
- Context line control (`0..=100`) for unified diffs.
- Formatting/comment-only changes collapse to a no-diff or formatting-only verdict.
- Parse failures name the side (`source A` / `source B`) and include location when available.

## Out-of-model / not built

- Multi-language parser bundle (large, parser-specific output differences, and not currently part of the gizza model for this pure block).
- Moved-code detection or GumTree-style edit scripts.
- Repository/Git integration, side-by-side UI, syntax highlighting, or live IDE navigation.
