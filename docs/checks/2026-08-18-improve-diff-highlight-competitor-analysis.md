# diff-highlight — competitor analysis (2026-08-18)

Scan run **before** implementing, per `/create-next-tool` step 4. Competitors were studied for
*features, parameters, defaults, and UX patterns only*. No competitor copy, branding, wording, or
trademark is reproduced here or in the tool — every string we ship is original.

Backlog row: `diff-highlight — Render a unified diff or two texts as a polished,
syntax-highlighted side-by-side image for sharing in PRs and chats. (pure)`

## Viability check (done first)

The requested output is a **syntax-highlighted image**, which raised the question of whether it
needs a browser/canvas/headless renderer. It does not: `blocks/code-screenshot` already ships the
exact pure-Rust stack in this repo — `syntect` (pure-Rust `fancy-regex` engine + bundled binary-dump
syntaxes/themes) for highlighting, `fontdue` (pure-Rust TrueType rasterizer, embedded font, no
system fonts) for glyph rasterization, and `png` for encoding. All three instantiate under
wasm32-wasip1. So a diff *image* is in-model; only the diff alignment + two-column layout is new
work. Built on the same shape: `Input::None` + `build_media_envelope`, **chat + CLI, no page**
(image-bytes-from-text has no page render mode — same as `code-screenshot`).

## Not a duplicate of an existing block

| Existing block | What it does | Why `diff-highlight` is distinct |
| --- | --- | --- |
| `diff-viewer` | Unified diff → inline / side-by-side / stats / JSON **text** | Text output only; no image |
| `diff-code` | Two snippets → side-by-side / patch / word-diff **text** | Text output only; no image |
| `text-diff`, `html-diff` | Text-level diffs, text output | No image, no syntax highlighting |
| `syntax-highlighter` | Code → styled HTML / ANSI | No diff; no image |
| `code-screenshot` | One code snippet → PNG | No diff — single snippet only, no two-column alignment, no add/remove tinting |

The gap is the intersection: **a rendered picture of a diff**. No existing block produces one.

## Competitors reviewed (top 3 real tools)

### 1. delta (git-delta) — terminal pager
- Side-by-side layout with old on the left, new on the right, aligned line by line.
- Word-level (intra-line) highlighting of exactly which characters changed.
- Always-visible, per-side line numbers.
- 20+ syntax themes; shares the `syntect` theme engine we already use.
- Toggles surfaced as flags: `--side-by-side`, `--line-numbers`, `--syntax-theme`.

### 2. git-split-diffs — terminal, GitHub-style split diffs
- Config knobs and defaults observed: `wrap-lines` (default on), `highlight-line-changes`
  (default on), `syntax-highlighting-theme` (empty disables), `min-line-width` (default 80 — below
  2× this it falls back to a unified layout), `theme-name` (default dark), plus a custom theme
  directory.
- Ships light *and* dark theme pairs (arctic / github dark+light / solarized dark+light /
  monochrome dark+light).
- Automatic unified fallback when the viewport is too narrow for two columns.

### 3. Browser patch/diff viewer (scrapfly patch-viewer) — paste-a-patch web tool
- Accepts a pasted git diff / patch.
- Side-by-side **and** unified view modes.
- Syntax highlighting across languages; file-tree navigation for multi-file patches.
- An "ignore whitespace" toggle for review focus.
- Fully client-side processing (the same privacy posture as ours).
- Sharing via permalinks and an HTML download.

## Table-stakes → decisions

Every table-stake below lands in the descriptor or in the out-of-model list. Nothing is dropped
silently.

| Table-stake | Seen in | Fit | Where it landed |
| --- | --- | --- | --- |
| Side-by-side two-column layout | all 3 | in-model | `layout = "side-by-side"` (default) |
| Unified / inline layout as an alternative | git-split-diffs, patch-viewer | in-model | `layout = "unified"` |
| Accept a pasted unified diff / .patch | delta, patch-viewer | in-model | `diff` param |
| Accept two raw texts to compare | (the backlog row; delta via `diff` input) | in-model | `left` + `right` params |
| Syntax highlighting by language | all 3 | in-model | `language` param (aliases + plaintext fallback) |
| Selectable themes, light **and** dark | delta, git-split-diffs | in-model | `theme` param (syntect theme names; dark default) |
| Per-side line numbers | delta, patch-viewer | in-model | `line_numbers` (default on) |
| Word-level intra-line change highlight | delta, git-split-diffs | in-model | `word_highlight` (default on) |
| Ignore-whitespace comparison | patch-viewer | in-model | `ignore_whitespace` (default off) |
| Context lines around changes | delta, patch-viewer (hunk collapsing) | in-model | `context` (default 3) |
| Multi-file patch handling | patch-viewer (file tree) | partly in-model | File **header rows** are rendered per file in the image; interactive tree navigation is not (see out-of-model) |
| Narrow-viewport unified fallback | git-split-diffs | n/a | An image has no viewport; the caller picks `layout` explicitly |
| Long-line wrapping | git-split-diffs (`wrap-lines`) | rejected | See below |
| Shareable permalinks | patch-viewer | out-of-model | Needs a backend to host the link |
| HTML download of the rendering | patch-viewer | out-of-model here | Already covered by `syntax-highlighter` (code → self-contained styled HTML) and `diff-viewer` (diff → text views) |
| Interactive file-tree navigation | patch-viewer | out-of-model | Requires an interactive UI; the output here is a single static image |
| Custom user theme directories | git-split-diffs | out-of-model | wasm has no host filesystem; only bundled themes are loadable |
| Terminal/ANSI output | delta, git-split-diffs | out-of-model here | `syntax-highlighter` already emits ANSI |

**Considered, rejected — long-line wrapping.** git-split-diffs wraps to fit a fixed terminal width.
An image has no fixed width: our canvas grows to fit the content instead, so wrapping would only
add a re-flow mode with no benefit. Long lines are instead truncated at a stated per-column cap
with a visible `…` marker, and the cap is documented on the tool. This keeps a pathological
minified-JS line from producing a multi-megapixel PNG.

## Stated limits (documented in the descriptor + errors)

- Max 400 rendered rows, 120 columns per side (truncated with `…`), 8 MiB output PNG.
- Exactly one input mode: either `diff`, or both `left` and `right` — supplying neither or both is
  a descriptive error naming what was expected.
- Renders `+`/`-`/context lines and file headers; binary-file patch stanzas are shown as a header
  row (there is nothing textual to render).
