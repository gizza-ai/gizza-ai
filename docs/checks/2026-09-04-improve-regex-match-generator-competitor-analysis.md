# regex-match-generator — competitor analysis (2026-09-04)

Scan run before finishing implementation. These notes paraphrase public tool behaviour and docs; no competitor wording, branding, or trademarked copy is reused in the block.

## Tools reviewed

1. **RandExp-style JavaScript demos**
   Browser demos built around regex-to-string libraries commonly expose a single pattern field and a generate button. They handle literals, classes, ranges, alternation and bounded repeats, usually returning one random string at a time. They tend to treat unbounded repeats with a hard default cap and do not explain unsupported constructs well.
2. **Generate Strings From Regex / reverse-regex utilities**
   Online developer utilities generally offer count, delimiter or output-format controls, with examples such as order IDs, phone numbers and hex strings. Their strongest UX pattern is showing multiple samples at once and making the random output repeatable enough for fixtures.
3. **Regex testers with sample generators**
   Regex test sites primarily validate matches but often include sample text or example generators around common patterns. Their table stakes are clear error messages, visible regex examples, and support notes for engine-specific features such as lookaround, backreferences and anchors.
4. **Property-based testing generators**
   Libraries used by test frameworks generate matching strings for regex-like patterns and emphasize determinism through a seed. They usually surface size limits so an expression with `.*` cannot allocate unbounded output.

## Table stakes → where each one landed

| Capability | Seen in | Decision |
|---|---|---|
| Pattern text input | All reviewed tools | `pattern` required text input with examples and placeholder |
| Multiple samples per run | Utility generators, property-test generators | `count` (1–200), default 5 |
| Deterministic random mode | Property-test generators | `style=random` plus `seed` default 42 |
| Systematic/non-random generation | Test-data libraries | `style=sequential`, plus `shortest` and `longest` modes |
| Repeat cap for infinite languages | RandExp-style demos, testing libs | `max_repeat` default 4, capped 1–50 |
| Per-sample length guard | Testing libs | `max_length` default 200, capped 1–2000 |
| Unique sample toggle | Fixture utilities | `unique` default true, can be turned off for exact counts |
| Lines / JSON / CSV output | Developer utilities | `output = lines | json | csv` |
| Common regex subset | All reviewed tools | literals, escapes, `.`, classes/ranges, shorthands, groups, alternation, anchors, quantifiers |
| Honest unsupported-feature errors | Regex testers | clear rejections for lookaround, backreferences, inline flags, atomic/possessive quantifiers, POSIX classes, Unicode properties and word boundaries |
| Preset examples | Browser demos | page `[[example]]` chips for order codes, hex digests, clock times, email-like values and shortest/longest checks |

## Out of model (listed, not built)

- Full PCRE/ECMAScript regex semantics, including backtracking-sensitive lookaround and backreferences. These features depend on engine state and are not a good fit for a small deterministic pure-Rust/WASM generator.
- Unicode property/category generation such as `\p{Letter}`. Correct support would require a large Unicode database and flavour-specific semantics.
- Validating against every regex flavour. The block is a generator for a documented subset, not a compatibility oracle for JavaScript, Rust, PCRE, Python and .NET simultaneously.
- Infinite exhaustive enumeration. The tool exposes `max_repeat`, `max_length` and `count` to make unbounded languages finite and browser-safe.

## UX decisions

- The default is seeded random generation because that matches the common "make me fixtures" use case while staying reproducible.
- Sequential, shortest and longest modes make the tool useful even when random samples hide important boundaries.
- Output formats are machine-friendly so generated samples can be piped into tests or checked into fixture files.
