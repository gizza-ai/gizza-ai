# c-vuln-pattern-scanner — competitor analysis (2026-08-29)

Scan run **before** implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased from public documentation and manual pages — **no competitor
copy, branding, trademarks or rule databases are reproduced** in the block, its descriptor,
its page copy, or this document.

## Tool under build

`c-vuln-pattern-scanner` — heuristically scans pasted C/C++ source for vulnerability
patterns such as format-string bugs, integer overflow and off-by-one risks. Pure,
deterministic, local: **no compiler, no clang/LLVM dependency, no code execution**.

## Top 3 competitors reviewed

| # | Tool | Kind | What it does |
|---|------|------|--------------|
| 1 | **Flawfinder** (<https://dwheeler.com/flawfinder/>) | CLI, Python, open source | Lexical (token-level) scanner over C/C++ with a built-in database of dangerous functions: buffer copies (`gets`, `strcpy`, `strcat`, `sprintf`, the `scanf` family), format-string sinks, race conditions, shell-metacharacter/exec risks, weak randomness. Every hit carries a **risk level 0–5** and a **CWE identifier**; officially CWE-compatible with 17 CWEs covered, 9 of them on the CWE/SANS Top 25. Filtering: `--minlevel`, `--inputs` (only functions taking external data), `--falsepositive`, `--neverignore`, `--regex`. Output: default text with `--context` (show the source line), `--singleline`, `--columns`, `--dataonly`, plus `--csv`, `--html`, `--sarif`, `--sonar`. Honors an in-source suppression comment (an "ignore" marker on the hit line or the line above). Hitlist diffing (`--savehitlist`/`--diffhitlist`) and `--patch` (scan only lines a unified diff touched). Documented limitation: no control flow, data flow, type, namespace or scope information, so it "will necessarily produce many false positives" and also false negatives; user-defined functions sharing a name with a database entry misfire; `#if`/`#ifdef` are ignored, so dead code is still reported. |
| 2 | **Cppcheck** (<https://cppcheck.sourceforge.io/>) | CLI + GUI, C++, open source | Parse-based analyzer aimed at bugs compilers miss, with an explicit design goal of near-zero false positives. Detects dead pointers, division by zero, **integer overflow**, invalid bit-shift operands and conversions, memory-management errors, null-pointer dereference, **out-of-bounds** access, uninitialized variables, writes to const data. Requires real preprocessing/parsing of a translation unit; ships severity classes (error/warning/style/performance/portability/information) and machine-readable output templates. Has found real CVEs (e.g. a long-lived stack overflow in X.org). |
| 3 | **RATS — Rough Auditing Tool for Security** (<https://manpages.ubuntu.com/manpages/trusty/man1/rats.1.html>) | CLI, C, open source | Multi-language (C, C++, Perl, PHP, Python) rough scanner over a vulnerability database — the C database alone holds ~334 entries. Reports are grouped and sorted **by severity**, with a `-w` warning-level switch: `1` = high only, `2` = medium and above (default), `3` = include low. Finds buffer overflows, format-string problems, out-of-bounds reads, shell execution and insecure temporary files. Explicitly positioned as an aid that **prioritizes** spots for a human audit rather than proving a bug. |

Also sighted for context (not one of the three deep reads): CScanner
(<https://github.com/karimmd/CScanner>), a small C source scanner that flags a fixed list
of dangerous functions for buffer-overflow risk, and the Clang Static Analyzer / Coverity
class of path-sensitive analyzers referenced by OWASP's buffer-overflow guidance.

## Table-stakes → in-model / out-of-model

Every table-stake below lands in the descriptor or is listed as out-of-model. Nothing is
dropped silently.

### In-model (built into this tool)

| # | Table-stake | Seen in | Where it lands |
|---|-------------|---------|----------------|
| 1 | Paste source, scan **without compiling or running** it | 1, 3 | Core is a pure lexical/regex pass over the text. No compiler, no clang, no linker, no filesystem, no network. |
| 2 | **Dangerous-function database** (`gets`, `strcpy`, `strcat`, `sprintf`, `scanf` family, `system`, `popen`, `exec*`, `tmpnam`, `mktemp`, `rand`) | 1, 3, CScanner | Rules `GETS`, `BANNED-COPY`, `BOUNDED-COPY`, `SCANF-UNBOUNDED`, `COMMAND-EXEC`, `TEMP-FILE`, `WEAK-RANDOM`, `WEAK-CRYPTO`. |
| 3 | **Format-string** detection | 1, 3 | `FORMAT-STRING`: a `printf`/`fprintf`/`sprintf`/`snprintf`/`syslog`/`vprintf`-family call whose format argument is **not** a string literal. String literals are masked to `""` before matching, so a literal format is provably distinguishable from a variable one. |
| 4 | **Integer overflow** in size computation | 1, 2 | `INT-OVERFLOW`: allocation size argument containing `*` or `+` arithmetic (`n * sizeof(T)`, `len + 1 + extra`) with no visible guard. |
| 5 | **Off-by-one / out-of-bounds** risk | 2, OWASP | `OFF-BY-ONE` (index exactly equal to the declared array length; `<=` loop bound against a declared length; `malloc(strlen(s))` with no `+ 1`) and `BUFFER-OVERRUN` (a literal size or index provably **larger** than the declared array size — a real overflow, not a heuristic). |
| 6 | **Per-finding severity** | 1 (0–5 risk), 3 (high/medium/low) | Four levels — `critical`, `high`, `medium`, `low` — one per finding, shown in every output format. |
| 7 | **Minimum-severity filter** (`--minlevel`, `-w`) | 1, 3 | `min_severity` enum: `all`, `low`, `medium`, `high`, `critical`. |
| 8 | **CWE identifier per finding**, CWE-compatible reporting | 1 | Every rule carries a fixed CWE id (CWE-242, 120, 787, 193, 134, 190, 195, 770, 476, 401, 416, 78, 377, 330, 327, 367, 467) rendered in text, JSON and CSV. |
| 9 | **Show the offending source line** (`--context`) | 1 | `include_context` boolean, default `true`. Off gives one compact line per finding for grepping/diffing. |
| 10 | **CSV output** (`--csv`) and machine-readable output | 1, 2 | `format` enum: `text`, `json`, `csv`. JSON carries language, per-severity counts and a findings array; CSV is `line,severity,code,cwe,message,source` with RFC-4180 quoting. |
| 11 | **Suppress a reviewed finding from inside the source** | 1 (ignore comment) | A `// vuln-scan: ignore` comment on the finding's line, or on the line immediately above, suppresses that line. Our own marker — a competitor's marker string is not reproduced. |
| 12 | **Suppress a whole rule family** (`--regex` filtering, RATS DB selection) | 1, 3 | `ignore` — comma/space-separated rule codes, e.g. `WEAK-RANDOM, TEMP-FILE`. |
| 13 | **Scan a subset of checks** by category | 3 (per-language/per-DB selection) | `profile` enum: `all`, `memory`, `injection`, `crypto`, `banned` — a positive rule-family filter that composes with `ignore`. |
| 14 | **C vs C++ awareness** | 1 (C++ handled as the C subset), 2, 3 | `language` enum: `auto`, `c`, `cpp`. `auto` detects C++ from `#include <iostream>`, `std::`, `class`/`namespace`/`template`/`using namespace`. C++ mode adds `CPP-STREAM` (unbounded `cin >>` into a `char` array); C mode does not fire it. |
| 15 | **Summary roll-up / exit-worthy counts** (`--error-level`) | 1 | Header line and JSON `summary` carry total plus per-severity counts, so a caller can gate on them. |
| 16 | Dead-code / comment noise control | 1 (a documented weakness: `#if` bodies still reported) | Comments and string/char literal bodies are masked before matching, so a dangerous name inside a comment or a string does not fire. `#if 0` blocks are still scanned — stated on the page as a known limit, matching the class of tool. |

### Out-of-model (listed, deliberately not built)

| # | Feature | Seen in | Why it is out of model |
|---|---------|---------|------------------------|
| A | **Path-sensitive / data-flow analysis** (taint tracking, `--inputs`-style "only functions reading external data") | 1, 2, Clang/Coverity | Requires a parser, a type system and an interprocedural CFG. This tool is a single deterministic lexical pass by design; whole-program analysis belongs in a compiler-backed analyzer, and the page says so. |
| B | **Preprocessing / real parsing** of a translation unit (`#include` resolution, macro expansion, `#if` evaluation) | 2 | No filesystem and no include path exist in a wasm block; a snippet is scanned as-is. Consequence (documented): code inside `#if 0` is still reported, and macro-hidden calls are missed. |
| C | **Multi-file / whole-project scan**, directory walking, symlink policy (`--allowlink`, `--followdotdir`) | 1, 2, 3 | The input is one pasted snippet or file's text. No directory traversal in the model. |
| D | **SARIF / SonarQube / HTML report** output (`--sarif`, `--sonar`, `--html`) | 1 | JSON + CSV cover machine consumption; SARIF is a large schema whose value is CI-platform ingestion, which this toolkit does not own. Listed, not built. |
| E | **Hitlist persistence and diffing** (`--savehitlist`, `--diffhitlist`, `--patch`) | 1 | Needs durable state across runs and a unified-diff input; blocks here are stateless single-shot functions. |
| F | **Multi-language scanning** (Perl, PHP, Python) | 3 | Out of scope for a C/C++ tool. Adjacent languages are covered by other blocks in this toolkit (e.g. the shell and SQL-injection scanners). |
| G | **Automatic fix / rewrite** to the safe API | — | A correct rewrite needs an AST and exact source ranges. The tool names the safer API in the message and leaves the edit to the developer. |
| H | Column numbers per hit (`--columns`) | 1 | Line-level granularity plus the echoed source line is enough to locate a hit in a pasted snippet; column tracking through the masking pass would be misleading where literals were rewritten. |
| I | **Risk score 0–5** numeric scale | 1 | Collapsed to four named severities (`low`…`critical`), which map cleanly onto the page filter and JSON. A finer numeric scale would imply calibration this heuristic does not have. |

## False-positive / false-negative posture (stated on the page)

Every tool in this class is explicitly heuristic — Flawfinder's own documentation says it
has no control-flow, data-flow or type information and "will necessarily produce many
false positives", and RATS positions itself as prioritizing spots for a *manual* audit.
This tool inherits the same limits and states them on the page rather than implying
soundness:

- **No proof of exploitability.** A finding means "this pattern is worth a human look",
  not "this is a vulnerability". A `strcpy` into a provably large-enough buffer is still
  reported by `BANNED-COPY`.
- **Known false-positive sources:** a user-defined function whose name collides with a
  flagged libc name; code inside `#if 0`/`#ifdef` branches that never compiles;
  macro-defined wrappers; `USE-AFTER-FREE` and `UNCHECKED-ALLOC`, which use a bounded
  line window inside one function body and cannot see aliasing or early returns.
- **Known false negatives:** anything hidden behind a macro or a function pointer,
  overflow reachable only through arithmetic this pass does not evaluate, and any bug
  needing cross-function reasoning.
- Two rules are deliberately *exact* rather than heuristic — `BUFFER-OVERRUN` fires only
  when a literal size/index is provably larger than a literal declared array bound, so
  those hits are high-confidence.

## Non-duplicate confirmation

Checked against existing blocks before building:

- `blocks/sql-injection-scanner` — flags SQL statements assembled from variables
  (concatenation, interpolation, `format`/`sprintf` into SQL) across many languages. It is
  about **SQL text construction**, not C memory safety; it has no notion of buffers,
  allocation sizes, array bounds or libc APIs.
- `blocks/secret-scanner` — hardcoded API keys/tokens/private keys by provider pattern +
  entropy. Different signal entirely.
- `blocks/shell-script-linter` — bash/sh pitfalls. Different language, different rules.
- `blocks/code-metrics-analyzer`, `blocks/code-outline-extractor`,
  `blocks/code-comment-extractor`, `blocks/code-language-detect` — structure/metrics/
  extraction over source, no security rules.
- `blocks/dependency-risk-auditor` — dependency manifests and advisory risk, not source code.
- `blocks/memory-strings` — extracts printable strings from a binary dump.
- `blocks/svg-security-linter`, `blocks/iam-policy-linter`, `blocks/sql-linter`,
  `blocks/markdown-lint`, `blocks/prose-linter` — other input languages.

No existing block scans C/C++ source for memory-safety or vulnerability patterns.
Not a duplicate.

## Sources

- [Flawfinder home page](https://dwheeler.com/flawfinder/)
- [flawfinder(1) manual page](https://manpages.ubuntu.com/manpages/noble/man1/flawfinder.1.html)
- [Cppcheck](https://cppcheck.sourceforge.io/)
- [rats(1) manual page](https://manpages.ubuntu.com/manpages/trusty/man1/rats.1.html)
- [CScanner](https://github.com/karimmd/CScanner)
- [OWASP — Buffer Overflow](https://owasp.org/www-community/vulnerabilities/Buffer_Overflow)
