# Competitor analysis: graphql-formatter

Date: 2026-08-13
Tool: `graphql-formatter`

## Sources checked

- jsonformatter.org GraphQL Formatter
- FastMinify GraphQL beautifier
- Convert Case GraphQL formatter
- Additional search-result checks for Flipper File and CaseChange GraphQL formatter/minifier pages

## Table-stakes capabilities

| Capability | Observed competitor pattern | Model fit | Decision |
| --- | --- | --- | --- |
| Paste GraphQL and beautify it | All competitors center the page on a large paste box and formatted output | In-model | Built as `input` text/textarea with formatted output |
| Validate syntax | Formatter/validator wording is common; broken GraphQL should not be silently printed | In-model | Core parser returns line/column syntax errors |
| Queries, mutations, fragments, SDL schemas | Competitors describe queries and schemas, not only a single operation type | In-model | Parser covers executable definitions and SDL definitions |
| Indentation control | Online formatters commonly default to two-space indentation; some expose indentation variants | In-model | Added enum `indent` with `2`, `4`, `8`, and `tab` |
| Minify/compact output | Several tools pair beautify with minify | In-model | Added `mode=format|minify`; minify strips ignored characters |
| Remove comments | Minifiers drop comments; readable comment stripping is useful for sharing examples | In-model | Added `remove_comments` boolean for format mode; minify always removes comments |
| Sort fields | Developer tools often optimize for stable formatting/diffs | In-model | Added optional `sort_fields` for selections and SDL object/input fields |
| Copy/download output | Web tools usually expose copy/download controls | Platform/generator | Text output uses the generic page text surface and generated controls |
| Run examples/presets | Competitor pages often include sample input buttons or prefilled examples | In-model/page | Added example chips for formatting a query and minifying a schema |
| Execute GraphQL against an endpoint | Some GraphQL playground products can send queries to a server | Out-of-model | Not built; this repository's block is local, deterministic formatting/validation only |
| Schema introspection or autocomplete | Full IDE/playground tools offer remote schema-aware help | Out-of-model | Not built; requires network endpoint/model/IDE state outside this tool model |

## UX decisions

- Use a multiline text field for the GraphQL document so pasted queries and schemas keep newlines.
- Use enums for indentation and mode so the page renders select controls rather than free-form strings.
- Keep boolean controls explicit for sorting and comment stripping; Playwright should cover a non-default boolean path.
- Include examples that exercise a query and an SDL schema so users see both supported inputs.

## Verification targets

- Exact CLI output for a compact query formatted with default settings.
- CLI minify path and advertised enum values (`indent`, `mode`).
- CLI boolean paths for `sort_fields` and `remove_comments`.
- Page real-output assertion and a deep-link case for `mode=minify` and a non-default checkbox state.
- Hygiene gate to catch placeholder text, stale manifest parameters, and FAQ/meta requirements.
