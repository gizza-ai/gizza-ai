# files-to-prompt — competitor analysis (2026-07-07)

Scan done BEFORE implementing. Sources paraphrased only — no competitor copy,
branding, or trademarks reproduced. gizza's `files-to-prompt` is a **paste-based**,
browser-local, pure-Rust tool: you paste your files (each preceded by a header line
naming its path) and get one LLM-ready digest. It does NOT crawl a repo or read a
filesystem (a browser page can't), which is the main fit-to-model boundary vs the CLI
competitors below.

## Competitors skimmed (top 3 real ones + 1 extra)

1. **files-to-prompt** (simonw) — the namesake Python CLI. Walks a directory, prints each
   file preceded by its relative path + separator. Output formats: default plain
   (`path`\n`---`\n`contents`\n`---`), `--markdown` (bare path line + language-fenced code
   block, escalates backtick count when contents contain triple backticks), `--cxml`
   (Claude `<documents>`/`<document index>`/`<source>`/`<document_contents>` wrapper),
   `--line-numbers`. Filtering: `--extension`, `--include-hidden`, `--ignore <glob>`,
   `--ignore-gitignore`, `--ignore-files-only`.
2. **repomix** (yamadashy) — Node CLI. Packs a repo into one AI-friendly file. Output
   styles xml (default)/markdown/plain/json. Prepends a **file-summary** section and a
   **directory-structure tree** (both toggleable). **Token counts** per file + whole repo
   (real tiktoken), `--token-count-tree`. `--output-show-line-numbers`, `--header-text`,
   custom instructions, `--top-files-len` (default 5), `--compress` (remove code bodies
   via tree-sitter).
3. **code2prompt** (mufeedvh) — Rust CLI. Source tree + Handlebars prompt templating +
   token counting; git diff/log integration; glob include/exclude.
4. **gitingest** — paste a repo URL → a text digest with a Summary, a directory tree, and
   concatenated file contents.

## Table-stakes → decision

| Table-stake (competitor) | Decision | Where |
|---|---|---|
| Multiple output formats: plain / markdown-fenced / Claude XML | IN-MODEL | `format` enum `markdown`(default)/`xml`/`plain` |
| Directory tree / structure block | IN-MODEL | `include_tree` bool, default true |
| Token count of the digest | IN-MODEL as an **estimate** (~chars÷4) | always in the summary footer |
| Exact model tokenizer (tiktoken o200k/cl100k) | OUT-OF-MODEL | needs the BPE rank tables (multi-MB wasm bloat); estimate only, clearly labelled |
| Line numbers | IN-MODEL | `line_numbers` bool, default false |
| File summary / count section | IN-MODEL | summary footer (N files, chars, ~tokens) |
| Language-detected code fences | IN-MODEL | extension→language map in core |
| Backtick-fence escalation when content has ``` | IN-MODEL | fence width = longest backtick run + 1 (min 3) |
| Recursive directory / repo crawl | OUT-OF-MODEL | no filesystem in a browser page — paste-based input |
| `.gitignore` / ignore-glob / extension / hidden filters | OUT-OF-MODEL | no directory walk to filter; the user pastes exactly what they want |
| Prompt templating (Handlebars) | OUT-OF-MODEL | template engine out of scope |
| Git diff/log integration | OUT-OF-MODEL | no git in the browser |
| Comment/body stripping (`--compress`) | OUT-OF-MODEL | needs per-language parsers (tree-sitter) → wasm bloat |
| Custom header / instructions text | Considered, rejected | schema bloat; the user can prepend their own instructions to the paste |
| Top-N largest files list | Considered, rejected | low value without a full crawl; the summary already gives totals |

## UX control patterns to match (all IN-MODEL)

- Output-format **dropdown** (`<select>`) — competitors expose plain/markdown/xml.
- **Checkboxes** for the tree toggle and line numbers.
- **Preset chips** (`[[example]]`) for the common shapes: markdown digest, Claude XML,
  plain + line numbers, tree only.
- Big **multiline textarea** for the pasted files.

## Feasibility spikes (before tagging out-of-model)

- **Directory tree from paths** — pure string work (split on `/`, nested map, box-drawing
  connectors). No dep. IN-MODEL. ✅
- **Token estimate** — `chars ÷ 4` heuristic (the widely-used rule of thumb). Pure. ✅
- **Exact tokenizer** — `tiktoken-rs` embeds/loads multi-MB BPE ranks; rejected for wasm
  size. Confirmed OUT-OF-MODEL. ✅
- **Language detection** — static extension→language table, no dep. IN-MODEL. ✅
