# glob-filter — competitor analysis (2026-07-26)

Tool: **glob-filter** — "Filters a path list by glob and gitignore-style include/exclude
patterns and previews exactly which paths match." Pure, browser-local, no server.

## Scan

One WebSearch ("glob pattern tester gitignore filter tool online"). Skimmed the top real
tools/docs (paraphrased below — no copy/branding reproduced):

1. **globster.xyz** — interactive glob tester built on minimatch. You type a glob and it
   highlights which of a file list match, in real time. Supports `*`, `**`, `?`, `[...]`
   character classes, `{a,b}` brace expansion, and negation. Positioned around validating
   `.gitignore` / Docker ignore rules.
2. **Shell Glob Pattern Tester (iotools.cloud)** — tests a glob against a pasted list of
   paths and lets you switch matching *semantics*: Bash, Zsh, `.gitignore`, Python
   `fnmatch`, Go `path.Match`. Real-time list of matches.
3. **Glob Pattern Tester (aidevhub.io / alienfusiongenerator / litedevtools / toolsfyi)** —
   several near-identical "paste paths + one pattern → see matches highlighted" tools.
   All emphasise *runs in your browser / private / no signup*, live matching, and
   gitignore/tsconfig/shell syntax support.

## Table-stakes (each → descriptor param or the out-of-model list, never dropped)

| Capability | In model? | Where it lands |
| --- | --- | --- |
| Glob wildcards `*`, `**`, `?` | yes | core matcher (`glob` + `gitignore` syntax) |
| Character classes `[abc]`, `[a-z]`, `[!x]` | yes | core matcher |
| Brace expansion `{a,b,c}` | yes | core matcher (recursive, nestable) |
| Negation / re-include | yes | `!`-prefixed pattern lines (last-match-wins) in `include`/`exclude` |
| gitignore semantics (anchoring, dir, any-depth) | yes | `syntax = "gitignore"` |
| Plain whole-path glob semantics | yes | `syntax = "glob"` |
| Test a **list** of paths at once | yes | `paths` (newline list) — this is the core UX |
| Include + exclude in one pass | yes | separate `include` / `exclude` fields |
| Show which paths match / don't | yes | `output = matched \| unmatched \| annotated` |
| Case-insensitive matching | yes | `case_sensitive` boolean |
| Match count / summary | yes | JSON `total` + `matched`, page shows both |
| Runs locally / private / no signup | yes | inherent (wasm, no server) — stated on page |
| Real-time highlight as you type | partial | page re-runs on input; ✓/✗ via `annotated` output |
| Preset syntax dialects (Bash/Zsh/fnmatch/Go) | rejected | two dialects (`glob`, `gitignore`) cover the practical split; per-shell nuances (Zsh extended globs, Go `path.Match` no-`**`) are niche and would bloat the enum. Listed, not built. |

## Controls / defaults / examples designed in

- Params: `paths` (required, multiline), `include`, `exclude` (both multiline, one pattern
  per line, `#` comments + `!` negation honoured in gitignore mode), `syntax`
  (`glob`\|`gitignore`, default `gitignore`), `case_sensitive` (bool, default true),
  `output` (`matched`\|`unmatched`\|`annotated`, default `matched`).
- Preset `[[example]]` chips: "Keep only Rust sources", "Apply a .gitignore",
  "Annotate every path".
- Worked examples + a syntax cheat-sheet table on the page; ≥3 FAQ accordions covering the
  glob-vs-gitignore distinction, negation ordering, and depth matching.

## Out-of-model / not built (listed, per skill rule)

- Per-shell dialects (Zsh extended glob, Go `path.Match`, Python `fnmatch` exact quirks) —
  niche; the `glob`/`gitignore` split is the 95% case.
- Walking a real filesystem / uploading a folder — gizza is text-in/text-out; the user
  pastes the path list (which a user gets from `git ls-files`, `find`, etc.).
- Live per-keystroke highlight *inside a shared editor widget* — the page re-runs on input
  and the `annotated` mode marks every path, which covers the intent declaratively.
