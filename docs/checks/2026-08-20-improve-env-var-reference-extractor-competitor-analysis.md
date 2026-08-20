# Competitor analysis — env-var-reference-extractor

Date: 2026-08-20
Tool: `env-var-reference-extractor`
Backlog prompt: find all environment-variable references (`$VAR`, `${VAR}`, `%VAR%`) used in shell scripts, Dockerfiles or configs.

## Competitors scanned

Search terms used by the builder: environment variable extractor, dotenv example generator, shell script env var scanner.

1. dotenv-linter / dotenv checkers
   - Table stakes: validate `.env`-style names, flag missing/duplicated keys, produce machine-readable diagnostics.
   - Gap/fit: these start from `.env` files; this tool starts from scripts/configs and can optionally compare against a `.env` body.
2. ShellCheck-style shell analyzers
   - Table stakes: understand shell variable references, line numbers, comments/escapes and undefined-ish variable warnings.
   - Gap/fit: full shell interpretation is out of scope; deterministic scanning with clear caveats is in model.
3. Secret/config inventory scripts and CI linters
   - Table stakes: extract `process.env.*`, Dockerfile `ARG`/`ENV`, CI config variables, CSV/JSON/Markdown reports, `.env.example` output.
   - Gap/fit: broad accessor coverage and report formats are in model; repository crawling is not, because gizza tools receive pasted text.

## Feature decisions

| Capability | In model? | Decision |
|---|---:|---|
| Shell `$VAR` and `${VAR}` | Yes | Implemented, including common parameter expansion forms and fallback defaults. |
| Windows `%VAR%` and `!VAR!` | Yes | Implemented with escaped `%%` handling and `set`/`setx` definitions. |
| Dockerfile `ARG`/`ENV` definitions | Yes | Implemented under dockerfile/auto syntax so defined status is meaningful. |
| Code accessors | Yes | Implemented for common JavaScript, Python, Java, Go, Rust, C/POSIX, Ruby and PHP forms. |
| Undefined-only output | Yes | `defined`, `include_defined_in_source` and `only_undefined` cover this workflow. |
| Multiple output formats | Yes | Names, aligned table, JSON, Markdown, CSV, `.env.example`, stats. |
| Ignore list/wildcards | Yes | Exact names and `PREFIX*` wildcards. |
| Full shell parser | No | Out of model for this lightweight wasm scanner; documented as deterministic scanning. |
| Crawl a repository/folder | No | Tools operate on pasted text/page input rather than filesystem traversal. |

## UX decisions

The page exposes preset chips for common workflows: shell table, Dockerfile missing variables, `.env.example` generation, Windows batch, and code accessors. Enum labels spell out the syntax families and output formats. Comment skipping, definitions-in-source and undefined-only are checkboxes because they materially change audit behavior.

## Verification plan

- Core unit tests cover shell, Dockerfile, Windows, code accessors, comments/escapes, nested defaults, sorting, JSON/CSV/Markdown/table/env-template/stats outputs, ignore patterns, supplied definitions and caps.
- Descriptor drift guard checks the generated chat schema.
- CLI checks should include an exact names/table output and at least one non-default checkbox state (`only_undefined=true` or `include_defined_in_source=false`).
- Page checks should run preset/deep-link cases and assert real output, including one `?output=env-template` case.

## Limits and caveats

The scanner intentionally does not interpret shell quoting/heredocs/arithmetic like a real shell. It ignores positional/special parameters (`$1`, `$@`, `$?`, `$$`, `$#`) and caps a run at 20,000 references.
