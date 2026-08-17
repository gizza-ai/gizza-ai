# diff-extract-versions competitor analysis (2026-08-17)

Tool: `diff-extract-versions` — reconstruct the full before-text and after-text from a unified
diff alone, when the diff carries enough context.

Research method: web search for "reconstruct original and new file from a unified diff",
"split a patch into before/after text", plus a scan of the reference implementations developers
actually reach for. Five real, reachable references were profiled. Everything below is
**paraphrased**; no competitor copy, branding, or trademark was reused.

## Sources scanned

1. **patchutils** (`twaugh/patchutils`, the CLI suite) — <https://github.com/twaugh/patchutils>.
   The closest functional relatives: `filterdiff` (keep the portions of a patch matching a shell
   wildcard), `lsdiff` (list the files a patch touches, optionally with the line number where
   each entry starts), `splitdiff` (break a multi-file/multi-change patch into single patches),
   `recountdiff` (repair wrong `@@` counts and offsets), `unwrapdiff` (repair word-wrapped
   diffs), `grepdiff`, `interdiff`, `combinediff`, `flipdiff`, `rediff`. No single command
   prints the two reconstructed versions — that is assembled by hand from context + `-`/`+`
   lines.
2. **unidiff (Python library)** — <https://pypi.org/project/unidiff/>. The one reference that
   exposes exactly our function: `PatchSet` → `PatchedFile` → `Hunk`, with `source_lines()` /
   `target_lines()` (the before/after line streams), `source_file`/`target_file` paths,
   `is_added_file`/`is_removed_file`/`is_rename`/`is_binary_file`, `added`/`removed` counts,
   per-line `source_line_no`/`target_line_no`, and explicit handling of
   `\ No newline at end of file` and CR-bearing lines. Library only — no browser surface,
   no gap reporting when hunks do not cover the whole file.
3. **Diff Patch Applier** (dev-toolbox.tech) — apply a unified diff to pasted original text, in
   the browser. Options: reverse apply (swap `+`/`-` to undo a change), per-hunk accept/reject
   review, fuzzy context matching, configurable context when generating. Copy button, keyboard
   shortcuts, eight worked example scenarios, "runs locally" privacy note. **Requires the
   original file** — it cannot recover anything from the patch alone.
4. **Diff/Patch Tool** (io-tools.com) — paste original text + unified diff, click a preview
   button, get the patched text. Validates hunk headers, context alignment and line counts, and
   stops with a mismatch message instead of emitting a wrong result. "Load sample" button, copy
   button, FAQ covering what a unified diff is and why context mismatches happen. Again:
   original file required.
5. **Online Diff / Patch Tool** (fannon.github.io) — three-pane layout (original · patched ·
   patch) on jsdiff; generates a patch from two texts or applies one to a pasted file, with
   copy-per-pane and share-via-URL. States plainly that a patch built against a different
   original can conflict and the output needs checking. No reverse/reconstruct-from-patch mode.

Also reviewed as format references: a unified-diff format guide (diffchecker.pro/blog) covering
`@@ -l,s +l,s @@` semantics, the omitted `,1` count, start-of-0 for pure insertions,
`\ No newline at end of file`, `/dev/null` for created/deleted files, `diff --git`/`index`/
`new file mode`/rename/binary extended headers, and the classic pitfalls (`-p0` vs `-p1`,
LF vs CRLF, tab/space drift, stale context).

## Positioning gap found

**Every browser tool in this space applies a patch to a file you already have.** None of the
five reconstructs both versions from the patch by itself, which is what you need when all you
were sent is a `.patch` file, a code-review email, or a diff pasted into a chat log. The only
prior art for that is a Python library (unidiff) and hand-assembly with patchutils. That is the
niche this tool fills, and it is the SEO angle for the page copy.

## Table stakes and decisions

| Capability / UX pattern | Seen in | In gizza model? | Decision |
| --- | --- | --- | --- |
| Parse `git diff` / `diff -u` / `git format-patch` / `svn diff` | all | Yes | One `diff` textarea; `diff --git`, `index`, mode, and mail preamble lines are skipped. |
| Reconstruct the before-text | unidiff `source_lines()` | Yes | `output=before`. |
| Reconstruct the after-text | unidiff `target_lines()` | Yes | `output=after`. |
| Both versions in one shot | nothing ships this | Yes | `output=both` (default) — labelled `===== BEFORE … =====` / `===== AFTER … =====` sections. |
| Machine-readable result | unidiff object model | Yes | `output=json`: per file paths, status, hunk/added/removed counts, `complete`, gap list, both texts. |
| Repair wrong `@@` counts | `recountdiff` | Yes | Body is authoritative; header counts are only used to detect gaps, and a mismatch is reported in JSON instead of failing. |
| Honour `\ No newline at end of file` | unidiff, format guides | Yes | Per-side; the reconstructed side loses its final newline. |
| Preserve CRLF | unidiff (`newline='\n'`) | Yes | Content after the marker char is copied byte-for-byte, so a CR survives. |
| Created / deleted files (`/dev/null`) | format guides, unidiff | Yes | Empty side reconstructs as empty text; JSON carries `status: added|deleted`. |
| Renames | unidiff `is_rename` | Yes | `rename from/to` captured as the before/after paths; `status: renamed`. |
| Binary / combined (`@@@`) diffs | unidiff `is_binary_file` | Partly | Reported as an explicit unsupported-entry error, never silently dropped. |
| Multi-file patch, pick one path | `filterdiff` shell wildcards | Yes | `file` param: exact path, bare filename, substring, or a `*`/`?` glob. |
| List what a patch touches | `lsdiff` | Yes | `output=json` is the inventory; an ambiguous `file` filter error names every path found. |
| Line numbers alongside the text | `lsdiff --line-number` | Yes | `line_numbers=true` prefixes each line with its number in that version. |
| Strip `a/` `b/` prefixes | `-p1` convention | Yes | Stripped automatically for display; no `-p` knob (schema bloat for one convention). |
| Say what is missing when context is short | nobody does this | Yes | `gaps=marker` (default) inserts a counted `[... N lines not in the diff ...]` marker; `omit` splices hunks together; `error` refuses. |
| Worked examples / sample loader | io-tools, dev-toolbox | Yes | Four `[[example]]` preset chips on the page. |
| Copy / download / share-by-URL | fannon, dev-toolbox | Yes | Platform-provided: Copy result, Download, and `?param=` deep links. |
| Local-only processing claim | all | Yes | True here — wasm in the browser tab. |
| Apply a patch to a pasted file | dev-toolbox, io-tools, fannon | In-model, already built | `apply-patch` covers it (with fuzz, reverse, conflicts). Not duplicated here. |
| Generate a diff from two texts | dev-toolbox, fannon, iotools.cloud | In-model, already built | `text-diff` / `diff-code`. |
| Side-by-side rendering of the patch | fannon | In-model, already built | `diff-viewer`. |
| Per-hunk accept/reject review | dev-toolbox | In-model, already built | `diff-hunk-selector`. |
| interdiff / combinediff / flipdiff between two patches | patchutils | Considered, rejected | Two-patch algebra is a different tool shape (two required inputs, different failure modes); a focused reconstructor beats a patchutils clone. |
| `unwrapdiff` (repair mail-wrapped diffs) | patchutils | Considered, rejected | Guessing where a wrapped line was split can silently corrupt reconstructed text — the tool errors with the offending line number instead. Empty lines that lost their trailing space ARE tolerated, since that repair is unambiguous. |
| Fuzz / offset search | dev-toolbox, `patch(1)` | Out of scope | Meaningless without a source file to match against. |
| Editing hunks in place | `editdiff` | Out-of-model | Needs an interactive editor surface. |
| Accounts, cloud history, API keys | some listicle tools | Out-of-model | gizza is no-account, no-server. |

## Defaults chosen

- `output=both` — the headline promise is *both* versions, and it makes the page show something
  useful with one paste and no other input.
- `gaps=marker` — an honest, counted placeholder beats either silently splicing unrelated regions
  together (`omit`) or refusing a perfectly useful partial reconstruction (`error`).
- `line_numbers=false` — the common use is copying the reconstructed text somewhere; numbers
  would have to be stripped again.
- `file=""` — most pasted diffs touch one file; a multi-file diff without a filter still works
  (per-file banners), so nothing is blocked.

## Honest limits stated on the page

- Content **after the last hunk** is unknowable from a diff — a diff never records the file's
  total length — so the reconstruction ends where the diff ends.
- Content between hunks is only recoverable when the diff carries it as context; with a normal
  3-line-context diff it is not there. `git diff -U100000` produces a fully reconstructable diff.
- Context-format (`diff -c`) and normal (`diff`) output are not unified diffs and are rejected
  with a message saying so.
