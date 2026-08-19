# shell-script-linter — competitor analysis (2026-08-14)

Scan run **before** implementation. One WebSearch (`online shell script linter bash shellcheck
tool`), then the top three reachable competitor tools were skimmed. All notes are paraphrased;
no competitor copy, branding, or trademarks are reproduced in the tool, its page, or its tests.

## Competitors skimmed

| # | Tool | URL | What it is |
|---|------|-----|------------|
| 1 | ShellCheck (official web playground) | https://www.shellcheck.net/ | The reference implementation: a Haskell static analyser exposed through an Ace code editor with a live results panel. |
| 2 | DevToolbox "Shell Script Linter & Bash Checker" | https://www.dev-toolbox.tech/tools/shell-script-linter | Browser-local linter implementing a subset of the well-known rule set, with a strict-mode/best-practice section. |
| 3 | utils.com "Shellcheck Paste Analyzer" | https://shellcheck.utils.com/ | Paste box + Bash/Zsh toggle, four one-click sample snippets, real-time local analysis. |

## Table-stakes observed

| Capability | Seen on | Fit | Decision |
|---|---|---|---|
| Paste-a-script textarea, analysis runs locally in the browser | 1, 2, 3 | in-model | Built — `script` field is `multiline = true`; the page runs the wasm module client-side. |
| Findings carry **line number + rule code + severity + message** | 1, 2, 3 | in-model | Built — text report is `L<n> [severity] CODE: message` plus the offending source line. |
| **Severity levels** (error / warning / info) | 1, 2 | in-model | Built — three levels, plus a `min_severity` filter (`all` / `warning` / `error`). |
| **Shell dialect selector** (bash vs sh/POSIX vs zsh) | 3 (Bash/Zsh toggle); 1 via `# shellcheck shell=` directive | in-model | Built — `shell` = `auto` \| `bash` \| `sh` \| `dash` \| `zsh`; `auto` reads the shebang. `sh`/`dash` additionally enable the bashism rule. |
| **Unquoted variable** detection | 1, 2, 3 | in-model | Built — `UNQUOTED-VAR`. |
| **Useless `cat`** detection | 2, 3 | in-model | Built — `USELESS-CAT`. |
| **Deprecated backtick** substitution | 2, 3 | in-model | Built — `BACKTICKS`. |
| **Missing shebang** | 2, 3 | in-model | Built — `MISSING-SHEBANG`. |
| **Strict mode** audit (`set -e`, `set -u`, `set -o pipefail`) | 2 | in-model | Built — `STRICT-MODE`, and the message names exactly which of the three options are absent. |
| **Unsafe `rm -rf`** on an interpolated path | 2 | in-model | Built — `RM-RISK` (error severity). |
| Bracket / block **syntax mismatch** (`if`…`fi`, `do`…`done`, unbalanced quotes) | 1, 2 | in-model | Built — `SYNTAX` (error severity). |
| **Non-POSIX bash constructs under a `sh` shebang** | 2, 3 | in-model | Built — `SH-BASHISM`. |
| **One-click sample scripts / presets** | 1 ("load random example"), 3 (four named samples) | in-model | Built — three `[[example]]` preset chips (unquoted-variable script, strict-mode audit, JSON for CI). |
| Clear/reset control, copy results | 1, 3 | in-model | Free from the generator: every field page gets Reset + Copy result buttons. |
| "Not a full replacement — keep the real linter in CI" honesty note | 2, 3 | in-model | Built — stated in *Limits and edge cases* on the page and in the FAQ. |
| Machine-readable output for CI | none of the three (all are HTML-only) | in-model | Built as a **differentiator** — `format = json` returns summary counts + a findings array, and the CLI surface makes it pipeable. |
| Suppressing a known-intentional finding | 1 (`# shellcheck disable=` directives) | in-model | Built as a parameter — `ignore` takes comma/space-separated rule codes. Inline source directives are out of model (see below). |

## Additional pitfalls we detect that the three competitors' visible rule lists did not advertise

These come from the backlog description ("subshell scope traps") and from the common-pitfall
literature; they are in-model and shipped:

- `SUBSHELL-SCOPE` — `… | while read …` runs the loop body in a subshell, so variables assigned
  inside it are lost after the pipeline.
- `UNCHECKED-CD` — a bare `cd` whose failure is not handled, so the rest of the script runs in the
  wrong directory.
- `PARSE-LS` — iterating over `$(ls …)`, which breaks on whitespace in filenames.
- `ASSIGN-SPACES` — `VAR = value`, which runs `VAR` as a command instead of assigning (error).
- `LEGACY-TEST` — single-bracket `[ … ]` in a bash script, where `[[ … ]]` avoids word-splitting
  and glob surprises.

## Out of model — listed, deliberately not built

| Feature | Seen on | Why it is out of model here |
|---|---|---|
| Full grammar-accurate parser with ~400 diagnostics | 1 | Requires porting a large Haskell analyser; this block is a heuristic, token-masking linter by design. |
| **"Apply fixes"** / auto-rewrite of the script | 1 | Needs the exact AST + source ranges from a real parser to rewrite safely; a heuristic rewrite would corrupt scripts. |
| Ace/CodeMirror editor with inline squiggles and gutter markers | 1 | The generator renders declarative controls; an embedded code editor would need bespoke page JS and is not a linting capability. |
| Per-finding deep links into an external rule wiki | 1 | Out of scope for an offline, unbranded page — we print a self-describing code and message instead of linking to a third-party site. |
| Inline `# shellcheck disable=SCxxxx` source directives | 1 | Uses another project's identifier namespace; we expose suppression through the `ignore` parameter with our own rule codes. |
| Auto-lint-as-you-type toggle | 2 | The page already re-runs on field change; a separate debounce toggle adds a control without adding capability. |
| Keyboard shortcuts (Ctrl+Enter to lint, Ctrl+Shift+C to copy) | 2 | Shared page-runtime concern, not a per-tool feature; would mean per-tool JS. |
| Zsh-specific diagnostics beyond shared-shell rules | 3 | `zsh` is accepted as a shell value and suppresses POSIX-only rules, but zsh-unique grammar (globbing flags, `setopt`) is not modelled. |

## Rule-code naming decision

Competitor tools surface ShellCheck's `SCxxxx` identifiers. This block uses its own descriptive
codes (`UNQUOTED-VAR`, `USELESS-CAT`, …) rather than reusing another project's numbering scheme —
consistent with the existing `sql-linter` block, and it keeps the `ignore` parameter readable.

## Verification performed

- `cargo test --workspace` in `blocks/shell-script-linter/` (core unit tests incl. one error case,
  plus the descriptor drift-guard).
- `scripts/build-block-wasm.sh shell-script-linter`; `wasm-pack build … --target web --release`.
- `cargo install --path cli`; `python3 scripts/sync-tool-manifest.py shell-script-linter`;
  page generator run.
- CLI advertised-values matrix: every `shell`, `min_severity` and `format` enum value exercised on
  a real script, an `ignore` suppression run, the exact 200 000-byte input cap and one byte over,
  and an exact-output case.
- Playwright `tests/tool-page-shell-script-linter.spec.ts`: real output assertion, a `?param=`
  deep-link, a non-default select state, and the JSON format path.
- `python3 scripts/check-tool-hygiene.py shell-script-linter` exits 0.
