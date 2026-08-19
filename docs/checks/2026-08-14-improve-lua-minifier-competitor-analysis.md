# lua-minifier — competitor analysis (2026-08-14)

Scan run BEFORE implementing, per `create-next-tool` step 4. One web search
("Lua minifier online compress Lua source strip comments rename locals"), then the
top competitor tools were skimmed directly. All notes below are **paraphrased
observations of capability** — no competitor copy, wording, branding or trademark is
reused anywhere in this block.

## Tools skimmed

| # | Tool | Reachable | What it is |
|---|------|-----------|------------|
| 1 | `stravant/LuaMinify` (GitHub, and the `emilk` fork) | yes | The reference open-source Lua minifier: a real Lua parser + formatter backends |
| 2 | minifier.org — Lua minifier | yes | Paste-and-minify web page, renames variables |
| 3 | devtoollab.com — Lua obfuscator | yes | Checkbox-driven strip/minify/rename/encode |
| 4 | goonlinetools.com — Lua minifier | yes | Minimal paste → Minify → Copy page |

`codeshack.io/lua-minifier/` was in the search results but returned HTTP 403 to the
fetcher, so it was replaced by a fourth reachable tool (devtoollab) rather than
running the scan with fewer competitors. Its search-result blurb (token-stream rather
than find-and-replace, no renaming) is consistent with tool 4's positioning and did
not add a distinct table-stake.

## Table stakes observed

| Capability | Seen in | Verdict | Where it landed |
|---|---|---|---|
| Strip line comments (`-- …`) and long comments (`--[[ … ]]`) | 1,2,3,4 | **in-model** | `remove_comments` (default `true`) |
| Collapse whitespace / indentation / blank lines to minimal spacing | 1,2,3,4 | **in-model** | always on; that is the tool |
| Join the whole script onto one line | 1,2,4 | **in-model** | `line_breaks = "strip"` (default) |
| Semantic renaming of local variables + function parameters to short names | 1,2,3 | **in-model** | `rename_locals` (default `false`) |
| Preserve a license / copyright banner while stripping comments | (JS/CSS minifier convention; not seen in the Lua tools) | **in-model**, cheap | `keep_license` (default `true`) |
| Never rename globals, table fields, method names or string contents | 1,3 | **in-model** | enforced by the resolver; documented in the FAQ |
| Paste a sample script with one click ("insert sample") | 4 | **in-model** | three `[[example]]` preset chips |
| Copy result / download result | 2,3,4 | **in-model** | generator ships Copy + Download on every `format = "text"` page |
| Runs client-side, nothing uploaded | 2,3,4 | **in-model** | already true — wasm in the browser; stated on the page |
| File upload for the source | 3 | **out-of-model here** | the pure-tool page takes a paste field; the CLI reads a file via the shell (`code="$(cat f.lua)"`) |
| Original vs minified size and "% saved" readout | 2 (partially: input line/byte count) | **deliberately not adopted** | the tool's output IS the minified script; mixing a stats header into it would break copy-paste and the CLI's exact-output contract. Reported per-surface size belongs to the page shell, not the block payload |
| Encode string literals as `string.char(…)` sequences | 3 | **out of scope** | that is obfuscation, not minification — it makes the output *larger* and is explicitly "not cryptographically secure" even in the tool that ships it. `blocks/lua-minifier` stays a size-reducing, behavior-preserving transform |
| Full-parser guarantees (AST reprint, constant folding, dead-code removal) | 1 | **out-of-model** | needs a complete Lua parser/AST; this repo has no wasm-instantiable Lua parser (see the `lua-runner` skiplist entry). The token-aware approach is stated honestly on the page and in the descriptor |

## UX control patterns adopted

- **Checkbox row** matching the pattern tool 3 uses (strip comments / minify / rename):
  `remove_comments`, `keep_license`, `rename_locals` all render as checkboxes from
  `Param::boolean`.
- **`<select>` with friendly labels** (`[input.labels]`) for `line_breaks`, so the
  "one long line" vs "keep the line structure" choice is legible without reading docs.
- **Preset chips** (`[[example]]`) replace the "insert sample" button pattern: minify a
  module, rename locals, keep line structure.
- **Multiline paste field** with a real Lua placeholder; Copy + Download + Reset come
  from the generator.

## Decisions

1. `rename_locals` defaults to **false**. Every competitor that renames does it
   unconditionally, but renaming is the one transform that can change behavior if the
   scope analysis is wrong (or if the script reflects over local names). Opt-in with a
   documented safety model is the honest default; the chips make it one click.
2. Renaming is **scope-aware and monotonic** — each local/parameter in the file gets a
   unique short alias, so an inner block can never shadow an outer name that is still
   referenced. Aliases also avoid every global name used anywhere in the file.
3. If block structure does not balance (a missing `end`), renaming **refuses with an
   explicit error** rather than emitting plausible-but-wrong code. Whitespace/comment
   minification still works on such input.
4. No competitor copy was read into the page: `page/content.md` and `meta.toml` were
   written from the capability list above.
