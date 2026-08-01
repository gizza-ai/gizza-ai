# string-literal-extractor — competitor analysis (2026-07-31)

Tool function: parse pasted source code and list every **string literal** it
contains (single-, double-, and backtick/template-quoted, with escape handling),
so developers can audit hard-coded strings for i18n externalization or scan for
secret-looking values. Pure-Rust, runs locally.

## Competitors scanned (paraphrased; no copy/branding reused)

1. **i18n-ally / lokalise (VS Code, "Hardcoded Strings Extraction")** — inline
   detects hard-coded strings in JS/TS/Vue/HTML templates and offers to extract
   them into a locale file. Table stakes: per-language quote awareness, ignores
   strings inside comments, lets you filter which strings count (min length /
   skip trivial), shows the source location.
2. **i18n-lint (jwarby, CLI/lib/Grunt)** — flags possible hard-coded strings in
   HTML/template files; reports file + line + column of each literal; has an
   attribute/ignore list. Table stakes: line/column reporting, list output.
3. **JetBrains IDEA "Hardcoded string literals" inspection** — highlights every
   string literal in code and can externalize it; language-aware (treats a Java
   `'c'` as a char literal, not a string); works across many languages.
4. **oliviertassinari/i18n-extract & aseemk gist (Node scripts)** — regex/AST
   scripts that walk source and collect string-literal arguments; support
   dedup/unique key lists and JSON output.

Sources: JetBrains IDEA docs (Hard-coded string literals), lokalise/i18n-ally
wiki (Hardcoded Strings Extraction), jwarby/i18n-lint, oliviertassinari/i18n-extract.

## Table-stakes params / behaviour → decision

| Capability | In/out of model | Where it landed |
|---|---|---|
| Quote-aware extraction (`"` `'` backtick) with escape handling | in-model | core tokenizer |
| Ignore literals inside `//`, `#`, `/* */` comments | in-model | per-language comment rules |
| Per-language rules (single-quote = char literal in C/Java/Rust/Go/C#) | in-model | `language` enum + profiles |
| Line (and column) reporting | in-model | `line_numbers` + JSON/CSV columns |
| Filter by quote style | in-model | `quotes` param |
| Minimum length filter (skip trivial strings) | in-model | `min_length` param |
| Dedup / unique list | in-model | `unique` param |
| Decode escape sequences to actual value | in-model | `decode_escapes` param |
| Output as list / JSON / CSV | in-model | `format` param |
| Preset examples (i18n audit, secret scan) | in-model | `[[example]]` chips |

## Out-of-model (listed, not built)

- **AST-grade parsing** (tree-sitter / language compilers) — the wasm sandbox
  can't host the C grammars; we use a deterministic quote/comment tokenizer, so
  exotic constructs (C# verbatim `@"..."`, Rust byte strings `b"..."` nuances,
  Ruby `%w[]`/heredocs, PHP heredoc/nowdoc) may be missed. Stated as a limit.
- **Writing extracted strings back into `.properties`/locale files** — this tool
  only lists; externalization/replacement is out of scope.
- **Detecting i18n *call keys*** (`t('key')` argument extraction) — the inverse
  of hard-coded-string detection; a distinct tool.

## Defaults chosen

`language=auto` (Python vs a generic C/JS profile), `quotes=all`,
`format=list`, `decode_escapes=false` (show raw source text), `unique=false`,
`min_length=0`, `line_numbers=true`.
