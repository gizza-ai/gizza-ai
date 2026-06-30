# Competitor analysis: regex-tester

Date: 2026-06-29
Tool: `gizza-ai/regex-tester`

## Goal

Provide a regular-expression *tester / debugger* across chat, CLI, and browser-page surfaces.
Unlike the companion `regex-extract` (which returns a flat list of one chosen group's matches),
the tester surfaces the full structured breakdown for debugging a pattern: every match with its
character start/end positions, and within each match the value and span of every capture group —
both numbered `(…)` and named `(?<name>…)`. Toggle case-insensitive, multiline and dot-all modes.
Position it as an offline, privacy-preserving regex debugger built on the linear-time Rust `regex`
engine.

## Competitors reviewed

1. regex101
   - URL: https://regex101.com/
   - Notes: The reference online regex tester. Live highlighting, per-match capture-group table,
     match positions, an explanation pane, a regex debugger, a substitution tab, a quick reference,
     and multiple flavours (PCRE/JS/Python/Go/Rust/Java/.NET).
   - Gap analysis: gizza matches the core in-model value — every match with positions, and the
     value/span of each numbered and named group, with the i/m/s flags. The explanation pane,
     step debugger, multi-flavour selector, and code-generation are larger UI features out of scope
     for a single pure-compute block. gizza's edge: runs locally with no upload, plus chat + CLI
     surfaces.

2. RegExr
   - URL: https://regexr.com/
   - Notes: Live match highlighting, capture-group details on hover, a reference/cheatsheet, saved
     community patterns, and a tools (list/replace/details) panel. JS/PCRE flavours.
   - Gap analysis: gizza covers match + group breakdown and flags. Community pattern library and
     live in-text highlighting are page-UX features beyond the current generated page model; the
     structured positional report is the in-model equivalent and is also available via CLI/chat.

3. Debuggex
   - URL: https://www.debuggex.com/
   - Notes: Renders a railroad/state-machine diagram of the pattern and animates a match against
     test input. JS/Python/PCRE.
   - Gap analysis: The railroad visualisation is out of model (no diagramming in a text block).
     gizza instead reports the exact character spans of each match and group, which is the
     debugging signal most users actually need.

4. IHateRegex playground
   - URL: https://ihateregex.io/playground
   - Notes: Simple tester with a curated library of common patterns (email, IP, URL…) and a small
     match view.
   - Gap analysis: gizza does not ship a pattern library, but its per-group positional report is
     richer than IHateRegex's match view. Common-pattern presets could be a future page nicety;
     not built to avoid bloating the block.

5. Rust Playground / `regex` crate docs
   - URL: https://docs.rs/regex/
   - Notes: The canonical reference for the exact flavour gizza implements — Unicode-aware,
     linear-time, no look-around/backreferences, named groups via `(?<name>…)`.
   - Gap analysis: gizza is a thin, purpose-built UI over this engine. It intentionally inherits
     the engine's guarantees (no catastrophic backtracking) and limitations (no backreferences or
     look-around), and documents the Rust flavour explicitly so expectations match behaviour.

## Fit-to-model decisions

Built in model:
- Pure-Rust core over the `regex` crate returning a structured `Report` (match_count, group_count,
  group_names, and per-match positions + per-group value/span).
- Numbered and named capture groups, with non-participating optional groups reported as `(no match)`
  rather than dropped.
- The i (ignore_case), m (multiline) and s (dotall) flags as booleans on every surface.
- Unicode-aware character offsets (positions count characters, not UTF-8 bytes).
- Errors for empty and syntactically invalid patterns.
- Chat schema drift guard, browser wrapper, page content, CLI usage, and Playwright page tests.
- Cross-link to the companion `regex-extract` tool for users who only want the match list.

Intentionally not built / out of model:
- Live in-text match highlighting / a railroad-diagram or step debugger (page-UX beyond a generated
  text block).
- Multi-flavour selector (PCRE/JS/.NET) — the engine is Rust `regex`; documented as such.
- A substitution/replace tab and code generation.
- A curated saved-pattern library or explanation pane.
- Look-around / backreferences — unsupported by the linear-time engine by design.

## Verification snapshot

- `cargo test --workspace` in `blocks/regex-tester/`: passed (10 tests).
- `wafer build` in `blocks/regex-tester/`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/regex-tester/web --target web --release --out-dir pkg`: passed.
- `cargo install --path cli`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed and rendered `tools/regex-tester/`.
- CLI surface: `gizza tool regex-tester text=… pattern='(?<year>\d{4})-(\d{2})-(?<day>\d{2})'`
  returned the structured match/group report for two dates.
- Playwright page test: `xvfb-run npx playwright test tool-page-regex-tester.spec.ts` passed (4 tests).
