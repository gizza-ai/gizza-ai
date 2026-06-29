# Slugify competitor analysis (2026-06-29)

Tool: `slugify`

## Competitors reviewed

1. slugify.online / "URL Slug Generator" sites
   - Paste a title, get a hyphenated lowercase slug with a copy button.
   - Convenient, but most fold accents poorly (drop them instead of transliterating) and offer no per-line batch mode.
2. npm `slugify` (sindresorhus / simov)
   - The de-facto library: replacement map, custom separator, lower/strict options, locale transliteration.
   - Library-only — requires writing code; no paste-and-read page and no shareable link.
3. CyberChef "To Snake/Kebab case" + "Remove Diacritics"
   - Chainable recipe that can lower-case, strip accents, and join words.
   - Powerful but multi-step; no single "slug" primitive, and Unicode scripts (CJK) are not romanised.
4. Django `slugify` / Python `python-slugify`
   - Server-side helpers used in CMS permalinks; `python-slugify` adds Unicode transliteration and `max_length` on a word boundary.
   - Backend code, not an interactive tool; no browser UI.
5. Various "kebab case converter" pages
   - Convert spaces/camelCase to hyphenated lowercase.
   - Treat input as ASCII; accents and non-Latin scripts pass through unchanged, producing broken slugs.

## In-model gaps and actions taken

- Unicode transliteration: implemented `deunicode` so accents and non-Latin scripts fold to ASCII (`Crème Brûlée` → `creme-brulee`, `北京` → `bei-jing`, `Münchner Straße` → `munchner-strasse`), beating the many ASCII-only competitors.
- Apostrophe handling: drop apostrophes (including typographic `’`) so contractions/possessives join (`Bob's` → `bobs`, not `bob-s`).
- Custom separator: `separator` accepts `-` (default), `_`, or any non-alphanumeric string, with validation that rejects ASCII-letter/digit separators that would corrupt the slug.
- Case control: `lowercase` defaults true but can be turned off to preserve the original capitalisation.
- Length cap on a word boundary: `max_length` (0 = no limit, clamped to a 2048 hard cap) truncates without splitting a word, matching `python-slugify`'s most useful option.
- Batch mode: `per_line` slugifies each line independently and rejoins with newlines, a feature the single-box competitors lack; the page input is multiline so a list of titles can be pasted at once.
- Three surfaces: the same parameters drive the chat/LLM tool, the `gizza` CLI, and the page with `?text=` deep links so a slug is shareable.

## Out-of-model or intentionally not implemented

- Custom replacement/transliteration maps (e.g. `&` → `and`, locale-specific rules): out of scope; `deunicode`'s default romanisation is used uniformly.
- Stop-word removal / SEO keyword trimming: a separate editorial concern, not part of a deterministic slugifier.
- Uniqueness suffixes (`-2`, `-3`) against an existing set: requires external state (a database of taken slugs) the tool does not have.
- camelCase word splitting: input is treated as words separated by whitespace/punctuation; `fooBar` stays `foobar` rather than `foo-bar`.

## Verification snapshot

- `cargo test --workspace` from `blocks/slugify`: passed.
- `wafer build` from `blocks/slugify`: passed and produced `target/block.wasm`.
- `wasm-pack build blocks/slugify/web --target web --release --out-dir pkg`: passed.
- `cargo run --manifest-path tools/generator/Cargo.toml -- .`: passed; rendered `tools/slugify/`.
- `cargo install --path cli`: passed.
- `gizza tool slugify text='10 Tips for Crème Brûlée!'`: passed, returning `10-tips-for-creme-brulee`.
- `cd tests && xvfb-run npx playwright test tool-page-slugify.spec.ts`: passed.
- `npm run test`: passed.
