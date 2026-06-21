# unwrap-text — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/unwrap-text` — remove hard line breaks inside paragraphs to
rejoin wrapped text into continuous lines. Pure-Rust, dependency-free. Pure-text
input → text output: chat + CLI + a page.

## What competitors do

- **Online "remove line breaks" tools** — paste text, get it unwrapped. Useful,
  but **the text is sent to a third-party page**, and many naively strip *all*
  newlines (destroying paragraph and list structure).
- **Editor macros / regex** (`:%s/\n/ /` in vim, find-replace in editors) — local
  but fiddly, and a blunt regex flattens paragraphs and lists too.
- **`fmt`/`par` (Unix)** — reflow tools, but oriented toward *re-wrapping* to a
  width; "un-wrap to one line per paragraph" needs the right flags and isn't
  obvious.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: chat Service
   Worker, CLI, and in-browser page. The text never leaves the device.
2. **Structure-aware, not a blunt strip.** It rejoins lines *within* a paragraph
   but **keeps blank-line paragraph breaks**, and **collapses runs of blank lines**
   to a single break — so the document's shape survives.
3. **Protects lists and quotes.** Lines starting with `-`, `*`, `+`, `>`, or an
   ordered marker like `1.` stay on their own line by default (`keep_list_breaks`),
   so bullet lists and quoted email aren't mangled — the thing naive removers get
   wrong. Turn it off to join everything.
4. **Normalises line endings** (CRLF/CR → LF) and trims stray whitespace while
   joining.
5. **Same everywhere.** Identical via chat, CLI, and a `?text=…` page.

## Honest scope

- **Heuristic list/quote detection** (common Markdown-ish markers); unusual
  bullet styles may not be recognised.
- **Joins to one line per paragraph** — it does not re-wrap to a target column
  width (that's the inverse operation).

## Tests

6 core unit tests: joins a 3-line wrapped paragraph into one line; preserves a
blank-line paragraph break; collapses multiple blank lines to one; keeps unordered
**and** ordered list items on their own lines; `keep_list_breaks=false` joins
everything; and single-line / empty input pass through unchanged. Plus the block
drift-guard schema test. **CLI verified** end-to-end. **Page** verified with
Playwright. `wafer build` instantiates the chat block.
