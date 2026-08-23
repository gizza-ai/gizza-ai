# toggle-line-comments — competitor analysis (2026-08-23)

Scan run **before** implementation, per `/improve-tool` Phase 2. All findings are
**paraphrased** — no competitor copy, branding or assets were reused.

## Scope of the scan

Two distinct product categories serve this job, and both were sampled:

1. **Editor/IDE comment-toggle commands** — the behavioral gold standard users already
   have muscle memory for (`Ctrl+/`). These define what "correct" means.
2. **Browser-based line-prefix text utilities** — the closest *online* analogues; they are
   generic prefix appenders with no language awareness.

## Competitors reviewed

### 1. JetBrains Rider / ReSharper — Comment/Uncomment with Line Comment (reachable)

- **Toggle semantics.** One command both comments and uncomments; commented input is
  uncommented, otherwise it is commented. With a selection, every line the selection
  *touches* is affected, even partly-selected ones.
- **Marker placement is a real setting.** A "don't indent comments started at first column"
  preference decides between putting the marker at **column 0** and **indenting it to match
  the code**. Configurable per language.
- **Separate block-comment command** (`Ctrl+Shift+/`) wraps the selection in `/* */`, and
  uncomments when the caret is already inside a block comment.
- Does not document blank-line handling or whether a space follows the marker.
- **Table stakes extracted:** toggle-by-default; a column-0 vs match-indent choice; per-language
  marker.

### 2. nvim-comment (reachable)

- **Explicit toggle rule, stated:** *if every line in the range is already commented, the
  range is uncommented; otherwise it is commented.* This is the rule to implement — "any
  uncommented line present ⇒ comment everything".
- **Options + defaults observed:**
  - `marker_padding` (default on) — a space between the marker and the code, "for linter
    compliance".
  - `comment_empty` (default on) — whether blank/whitespace-only lines get a marker too.
  - `comment_empty_trim_whitespace` (default on) — trailing whitespace is trimmed off a
    commented blank line.
  - Marker itself comes from the filetype's comment string, and is user-overridable.
- Indentation/alignment behavior is not documented.
- **Table stakes extracted:** a space-after-marker toggle; a comment-blank-lines toggle; a
  user override for the marker string.

### 3. Browserling — add prefix/suffix to lines (reachable)

- The generic online form: one textarea, a free-text prefix field, a free-text suffix field,
  one action button, plus undo. No defaults are pre-filled.
- **Zero language awareness** — the user must know and type `//`, `#` or `--` themselves, and
  there is no uncomment direction at all.
- **UX patterns worth matching:** paste-and-go single textarea, instant output, copy/undo
  affordances, no account.
- Sibling utilities in the same family (line-prefix tools generally) add *skip empty lines* and
  *trim whitespace* checkboxes; one such tool (codeshack) returned HTTP 403 to the scan and could
  not be verified first-hand — its options are recorded here as second-hand and were not relied
  on for any decision.

## Gap table — what shipped in this tool

| Table stake | Source | Decision |
| --- | --- | --- |
| Toggle by default, with all-commented ⇒ uncomment | nvim-comment, Rider | **In model** — `mode = toggle` (default), plus explicit `comment` / `uncomment` |
| Per-language comment marker | Rider, nvim-comment | **In model** — 31 languages + `auto` via `Param::enumv` |
| User override for the marker | nvim-comment | **In model** — `marker` param (wins over the language) |
| Space after the marker | nvim-comment `marker_padding` | **In model** — `space_after_marker`, default on |
| Comment blank lines or skip them | nvim-comment `comment_empty` | **In model** — `comment_blank_lines`, default **off** (editor-like; nvim's default-on is the outlier) |
| Column 0 vs match-the-indent marker placement | Rider setting | **In model** — `align = indent` (default) / `column0` |
| Block-pair fallback for languages with no line comment | Rider block-comment command | **In model** — `css`/`html`/`xml` wrap each line in `/* */` or `<!-- -->` |
| Trim trailing whitespace on a commented blank line | nvim-comment | **In model** — done implicitly: a blank line is commented as the bare marker, no trailing pad |
| Paste-and-go single textarea, copy button, no account | Browserling | **In model** — the shared page runtime already provides multiline input, Copy result and Reset |

### Considered, not built (out of model)

- **Editor keybindings / caret-follows-toggle / partial-line selections.** These need a live
  editor buffer and a cursor; gizza's surface is a text field, a CLI and a chat call.
- **Block-comment *span* mode** (one `/*` before the block and one `*/` after, rather than
  per-line). Distinct enough to be its own tool; this one is named for *line* comments, and the
  per-line pair fallback already covers the CSS/HTML case a user actually hits.
- **Full-parse language detection** (tree-sitter and friends). Those grammars are C libraries
  that do not instantiate in the wasm sandbox — the same conclusion `code-comment-extractor`
  already recorded. `auto` uses a deterministic shebang/keyword/existing-marker heuristic and the
  page tells the user to pick a language when it guesses wrong.
- **Per-project persisted preferences / accounts.** No backend, by design.

### Considered, rejected on judgment

- **A `suffix` field** (Browserling's second box). Line comments have no suffix in 28 of the 31
  languages, and the three that need one (`css`, `html`, `xml`) get it automatically from the
  language profile. A user-facing suffix box would be dead UI on almost every selection.
- **A separate "trim trailing whitespace" checkbox.** There is no configuration in which the
  opposite behavior is wanted, so it is simply always done rather than added as schema bloat.

## Non-duplication check

`blocks/code-comment-extractor` *reads* comments out of source (or strips them); this tool
*writes* comment markers on and off a block. `blocks/html-comment-stripper` removes HTML
comments only. No overlap in capability, so no skiplist entry.

## Sources

- <https://www.jetbrains.com/help/rider/Coding_Assistance__Comment_Uncomment_Code.html>
- <https://github.com/terrortylor/nvim-comment>
- <https://www.browserling.com/tools/prefix-suffix-lines>
- <https://codeshack.io/add-prefix-suffix-to-lines/> (HTTP 403 — not reachable for the scan)
