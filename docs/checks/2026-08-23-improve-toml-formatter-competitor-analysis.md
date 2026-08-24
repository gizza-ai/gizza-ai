# toml-formatter — competitor analysis (2026-08-23)

Scan run **before** implementing, per the create-next-tool recipe. All notes below are
paraphrased observations of publicly documented behaviour; no competitor copy, branding or
trademarks are reproduced or reused.

## Scan

One web search (`TOML formatter online beautify prettify TOML tool`) plus a skim of the
reachable results. Four sources were read in full:

| # | Source | What it is | Notes |
|---|--------|-----------|-------|
| 1 | `taplo.tamasfe.dev` formatter options | The de-facto Rust TOML formatter/LSP; the reference feature set every online tool is a subset of | Full documented option matrix with defaults |
| 2 | `spoold.com/tools/toml/format` | Browser-side format + validate, live/debounced | Zero formatting options; colour-coded valid/invalid status; explicitly documents that comments are lost |
| 3 | `toolsfyi.com/.../toml-formatter` | Browser-side "normalize" formatter | Three fixed rules only; explicitly does not validate and does not reorder |
| 4 | `string.is/toml-formatter` | Browser-side formatter | Page copy advertises customizable key sorting + syntax highlighting; no option matrix exposed in the served markup (thin) |

Also listed but not read in depth (same shape as 2–4, no additional documented options):
freecodingtools.org, jsontotable.org, fastminify.com, elysiatools.com, devtoys.pro (a Prettier +
`prettier-plugin-toml` wrapper, so effectively the Prettier option set: print width + tab width).

## Table-stakes inventory

Every item below ends up either in our descriptor or in the explicit "not built" list — nothing is
dropped silently.

### In-model — shipped in the descriptor

| Capability | Seen at | Our param | Our default |
|---|---|---|---|
| Parse + **validate**, report line/column on bad input | 2 (status indicator), taplo | (always on — invalid input is a hard error, not silent passthrough) | — |
| Normalize spacing around `=` | 1 (`compact_entries`), 3, 4 | `spacing` = `standard` \| `compact` | `standard` (`key = value`) |
| Indentation control | 1 (`indent_string`, `indent_entries`, `indent_tables`), devtoys/Prettier tab width | `indent` (0–8 spaces, slider) | `0` — flat, matching the Cargo.toml/pyproject.toml convention and taplo's own defaults |
| Alphabetical key sorting | 1 (`reorder_keys`), 4 | `sort_keys` = `preserve` \| `asc` \| `desc` | `preserve` |
| Array expand / collapse | 1 (`array_auto_expand`, `array_auto_collapse`, `compact_arrays`) | `array_style` = `auto` \| `expand` \| `collapse` | `auto` |
| Line-width budget that decides when an array expands | 1 (`column_width` = 80), devtoys/Prettier print width | `column_width` (20–200, slider) | `80` |
| Align `=` across a run of entries | 1 (`align_entries`) | `align_values` (checkbox) | `false` |
| Blank line before each `[section]` header | 3 | `blank_line_before_tables` (checkbox) | `true` |
| Keep or strip comments | 1 (`align_comments`); 2 documents comment loss as a limitation | `keep_comments` (checkbox) | `true` |
| Strip trailing whitespace | 3 | always on | — |
| Trailing newline at EOF | 1 (`trailing_newline`) | always on | — |
| Trailing comma in expanded arrays | 1 (`array_trailing_comma` = true) | always on when expanded | — |
| Preset examples / "load a sample" | 2, 4 | four `[[example]]` chips on the page | — |

**Deliberate differentiator:** sources 2 and (by construction) most value-model round-trip
formatters *drop comments*. We format over `toml_edit`'s syntax tree, so own-line comments,
end-of-line comments on entries and headers, and the file's dangling trailing comments all
survive by default. That is the single biggest quality gap in the field and it is closed here.

**Second differentiator:** source 3 explicitly does not validate; source 4 exposes no option
matrix. Ours validates first and refuses to emit anything for input that is not valid TOML,
reporting the exact line and column — same contract as taplo, in a one-shot tool.

### In-model but intentionally not built (listed, not dropped)

| Capability | Seen at | Why not |
|---|---|---|
| `align_comments` (align end-of-line comments in a column) | 1 | Cosmetic second-order alignment; `align_values` already covers the common ask, and a second alignment knob would be the 10th parameter on the form. Comments are preserved at their entry, just not column-aligned. |
| `reorder_arrays`, `reorder_inline_tables` | 1 | Reordering array *values* changes document meaning for anyone treating array order as significant (it usually is in TOML — e.g. path lists). `sort_keys` already sorts inline-table keys, which are unordered by spec. |
| `crlf` (Windows line endings) | 1 | Output is always LF; a browser/CLI copy-paste target does not need CRLF, and editors normalize on save. |
| `allowed_blank_lines` (configurable 0–N) | 1 | We collapse any run of blank lines to at most one, which is the only value anyone changes it to. Not exposed as a knob. |
| `indent_tables` / `indent_entries` as *separate* switches | 1 | Folded into the single `indent` slider (0 = neither, >0 = both), which is what users actually mean. |
| Syntax highlighting in the output pane | 4, 2 | Presentation of the shared page shell, not a block capability. The page ships a Copy button and (for `format = "text"`) a Download link instead. |
| Live/debounced revalidate as you type | 2 | The shared page runtime already re-runs on every field change; nothing tool-specific to build. |
| Format history panel | 2 | Site-shell feature, out of scope for this repo (which renders unbranded pages). |

### Out-of-model (cannot be done by this block at all)

| Capability | Seen at | Why |
|---|---|---|
| Preserve comments *inside* arrays and inline tables verbatim in `collapse` mode | 1 | A collapsed single-line array physically cannot carry a `#` comment — everything after `#` would swallow the rest of the line. Comments inside arrays are preserved only when the array is expanded (`auto` forces expansion when an inner comment exists); `collapse` drops them, and the page says so. |
| Format-on-save / editor LSP integration | 1 | taplo is an LSP; this is a one-shot function with a page + CLI + chat surface. |
| Multi-file / directory formatting | 1 (`taplo fmt .`) | One document in, one document out. Shell out in a loop for batches (the page shows the CLI form). |

## UX control patterns adopted

- Sliders (`kind = "slider"`) for the two bounded numeric knobs (`indent`, `column_width`) rather
  than bare number boxes — competitors that expose width use steppers, a slider reads better and
  the canonical number box is still there.
- `<select>` for all three fixed-choice params (`sort_keys`, `spacing`, `array_style`) via
  `Param::enumv`, with `[input.labels]` giving them plain-English labels.
- Checkboxes for the three booleans, defaults chosen so the no-touch run is a safe, faithful
  reformat (comments kept, sections spaced, nothing aligned or reordered).
- `[[example]]` preset chips replacing the "load a sample" button that sources 2 and 4 ship:
  a messy Cargo-style snippet, a sort-keys run, an expand-arrays run, and a strip-comments run.
- `multiline = true` on the input so pasted files keep their newlines.

## Decisions recorded

1. **`indent` defaults to 0**, not 2. Real-world TOML (Cargo.toml, pyproject.toml, netlify.toml)
   is flat, and taplo — the tool that actually formats most TOML on disk — defaults both
   `indent_tables` and `indent_entries` to false. Defaulting to 2 would make the untouched run
   surprising for the exact files people paste.
2. **Table ordering is normalized structurally**: a table's own entries are emitted under its
   header, then its sub-tables, so `[a.b]` declared before `[a]` comes back in tree order. This is
   semantics-preserving and is the "table ordering" half of the backlog description.
3. **Key quoting is normalized** to bare form whenever the key is a legal bare key
   (`A-Za-z0-9_-`), otherwise a basic-quoted string with minimal escapes. String *values* keep
   their original literal form (`'...'` stays literal) — rewriting them would mangle Windows
   paths and regexes.
4. **Scalar literals are preserved verbatim** (`0xFF` stays `0xFF`, `1_000_000` keeps its
   underscores, dates keep their offset spelling). Re-emitting from a value model would silently
   rewrite them; formatting must not change what a value looks like.
