# accent-stripper competitor analysis (2026-08-20)

## Sources scanned

- Online accent/diacritic removal tools for SEO slugs and data cleanup.
- Unicode normalization examples that decompose accents and remove combining marks.
- Transliteration libraries/tools that map non-Latin scripts and special Latin letters to ASCII approximations.

## Table-stakes mapped to this tool

| Capability / UX expectation | In model? | Decision in this tool |
| --- | --- | --- |
| Paste arbitrary text and remove accents | Yes | Required `input` textarea; default output is converted text only. |
| Handle more than simple combining marks (`ß`, `ø`, `Æ`, `Ł`, Cyrillic, Greek, CJK) | Yes | Default `mode=transliterate` uses a wasm-safe Rust transliteration table. |
| Conservative marks-only normalization | Yes | `mode=marks-only` decomposes Unicode and drops combining marks only. |
| Strict ASCII output option | Yes | `unmapped=remove|replace` handles characters left non-ASCII after conversion. |
| Preserve specific language letters | Yes | `keep` protects literal characters such as `ñ` from conversion. |
| Lowercase / whitespace cleanup for slug prep | Yes | `lowercase` and `collapse_whitespace` run after conversion. |
| Audit what happened | Yes | `include_report` returns JSON counts and unmapped samples. |
| Full URL slugification with punctuation policies and uniqueness | No | Out of model for this tool; page recommends chaining with a slug/regex cleanup tool. |
| Human language translation | No | Transliteration approximates characters; it does not translate words. |
| Locale-specific romanization choices | No | The transliteration table is deterministic and generic, not a locale-aware romanizer. |

## UX controls implemented

- Multiline textarea for source text.
- Select controls for conversion mode and unmapped policy.
- Tag-list for protected characters.
- Checkboxes for lowercase, whitespace collapse, and JSON report.
- Preset examples for recipe names, slug prep, marks-only behavior, Spanish `ñ`, and audit mode.

## Verification focus

Tests cover exact transliteration, conservative marks-only mode, unmapped remove/replace, protected characters, lowercase and whitespace collapse, report JSON, input cap, descriptor drift, CLI exact output, generated page output, and query-parameter deep links.
