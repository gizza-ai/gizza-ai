# xml-diff — competitor analysis (2026-08-23)

Scan run **before** implementation, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All findings are paraphrased observations of publicly documented behaviour — no competitor
copy, branding or trademarks were reused.

## Tools reviewed (top 3 reachable)

| # | Tool | Reachable | Shape |
|---|------|-----------|-------|
| 1 | comparexml.com | yes | Browser-side semantic XML diff, colour-coded, path context |
| 2 | diffchecker.dev/xml | yes | Browser-side XML-aware diff, categorised + filterable results |
| 3 | semanticdiff.com/online-diff/xml | yes | Server-side semantic diff, side-by-side alignment |
| — | toolxml.com/xml-diff-compare-two-files | **no** (HTTP 403) | replaced by #3 |

## Table stakes observed → our decision

| Capability | Seen on | Fit | Where it landed |
|---|---|---|---|
| Parse both docs and diff **semantically** (not as text lines) | 1, 2, 3 | in-model | core: both sides parsed into element trees with `quick-xml` |
| **Insignificant whitespace ignored** (indentation, line breaks, leading/trailing text whitespace) | 1, 2, 3 | in-model | `ignore_whitespace` param, default **true** (collapses runs, drops whitespace-only text) |
| **Attribute order ignored** | 1, 3 | in-model | inherent — attributes are compared as a sorted map, never positionally |
| **Full element path per change** (`/catalog/book[3]/author`) | 1, 2 | in-model | XPath-style paths incl. `[n]` predicates, `/@attr` for attributes, `/text()` for text |
| **Child matching strategy**: by index / LCS / unordered | 1 | in-model | `match` enum = `lcs` (default) \| `index` \| `unordered` |
| **Comments ignored** by default | 1 | in-model | `ignore_comments` param, default **true**; when false, comments diff as `comment()[n]` nodes |
| **CDATA treated as plain text** | 1 | in-model | inherent — CDATA content is folded into the element's text |
| **Numeric text compared numerically** (`1` == `1.0`) | 1 | in-model | `numeric_text` param, default **false** (opt-in, since XML text is nominally a string) |
| **Namespace handling** called out in FAQ | 2 | in-model | `ignore_namespaces` param, default false; when true, prefixes and `xmlns*` declarations are ignored |
| Change **classification** added / removed / changed with counts | 1, 2, 3 | in-model | report has `equal`, `added`, `removed`, `changed`, `changes[]` |
| Reordering an element must not create false positives | 1, 2 | in-model | `match=unordered` (set-like sibling matching); `lcs` absorbs pure insertions/deletions |
| Runs **entirely client-side**, nothing uploaded | 1, 2 | in-model | our page runs the same Rust core compiled to wasm in the browser |
| Worked examples / preset inputs | 1, 2 | in-model | three `[[example]]` preset chips on the page + a worked example in the copy |

## Out-of-model (listed, not built)

These are UI/rendering features of a hosted diff *viewer*, not capabilities of a
deterministic tool that returns one text result — they are intentionally not built here:

- **Side-by-side colour-coded panes with click-to-jump navigation** (1, 2, 3) — needs a bespoke
  two-pane editor; our surfaces (chat, CLI, single-output page) render one result body.
- **Filter/sort the result list by change type in the UI** (2) — the report already carries
  `kind` per change, so a caller filters downstream; there is no interactive result grid.
- **Edit-in-place conflict resolution / merge** (2) — an editor feature, out of scope for a diff.
- **File upload of two documents** (1, 3) — the page takes two pasted documents; the tool has no
  dual-file input model (single-source inputs only).
- **XSD/schema-aware comparison** (Altova-class desktop tools) — needs an XSD validator that
  instantiates under wasm; already ruled out repo-wide, see `docs/tool-skiplist.txt` `xml-validator`.

## Extras we ship that the scanned tools do not

- A **machine-readable JSON report** (`format=json`, default) usable in CI, plus a compact
  human `format=text` rendering — the scanned tools only render a visual diff.
- The same engine on **three surfaces** (chat block, `gizza` CLI, standalone page) with an
  identical parameter contract.
- Explicit, documented **limits** (1 MB per document, 500 nesting levels) with actionable errors.

## Known limits (stated on the page)

- Mixed content: an element's direct text nodes are folded into one value, so moving text
  around sibling elements inside the same parent is not reported positionally.
- The XML declaration, DOCTYPE and processing instructions are not compared.
- Custom (DTD-defined) entities are compared as written when they cannot be resolved.
