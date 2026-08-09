# apply-patch — competitor analysis (2026-08-09)

Scan run **before** implementing, per the create-next-tool recipe. Everything below is
paraphrased from public documentation; no competitor copy, branding, or trademark text was
copied, and no competitor asset was reused.

## Scope

Tool intent (backlog row): *apply a unified diff/patch to a pasted source file and return the
patched result, and detect conflicts.*

Dup check before building — the repo already has diff blocks, but none of them **applies** a
patch:

| Existing block | What it does | Overlap |
| --- | --- | --- |
| `text-diff` | Computes an LCS diff of two texts → unified diff / JSON | Inverse operation (produces a patch) |
| `diff-viewer` | Parses and renders an existing unified diff (inline / side-by-side / stats / JSON) | Display only, never rewrites the source |
| `diff-hunk-selector` | Numbers, filters, and splits the hunks of a patch into a smaller patch | Patch-to-patch, no source text input |
| `merge-conflict-resolver` | Rewrites `<<<<<<<`/`=======`/`>>>>>>>` markers already in a file | Conflict markers, not a diff |
| `find-replace` | Literal/regex substitution | Unrelated |

None takes `source + patch → patched source`, so this is a genuinely new capability and the
build proceeded.

## Competitors reviewed (top 3)

### 1. Diff Patch Applier — dev-toolbox.tech
- Two panels: original text and a unified diff; an Apply action produces the patched text below.
- Reverse-apply toggle that swaps the meaning of the `+` and `-` sides to unapply a change or
  reconstruct the original from a patched file.
- Per-hunk accept/reject checkboxes, `git add -p`-style cherry picking.
- Fuzzy matching: when a hunk's context is not exactly at the header's line number, it searches a
  nearby window.
- Failure handling is per hunk — it names the hunks that could not be applied rather than aborting
  the whole run.
- Accepts `git diff`, `git format-patch`, `svn diff`, and plain `diff -u` output, including
  `---`/`+++` file headers and `@@` hunk headers.
- Ships eight worked scenarios (simple add, simple removal, function edit, multi-hunk refactor,
  config change, reverse apply, context mismatch, diff generation).
- Everything runs client-side; no stated size limit.

### 2. Diff/Patch Tool — io-tools.com
- Same two inputs (original text + unified diff) and a single apply/preview action, plus a
  "load sample" button and a copy-output button.
- Validates hunk headers, declared line counts, context lines, and the `-`/`+` markers before
  emitting anything.
- Strict by design: on any context mismatch it stops with a mismatch message instead of returning a
  half-applied result.
- No reverse apply, no fuzz control, no whitespace-insensitive matching, no multi-file handling, no
  alternate output shape.
- FAQ covers: what a unified diff is, what causes a context mismatch, whether files leave the
  browser, and the sample.

### 3. `git apply` / GNU `patch` (the reference CLI implementations)
The behaviour every online applier is measured against:

| Option | Effect |
| --- | --- |
| `-R` / `--reverse` | Apply the patch backwards |
| `--check` / `--dry-run` | Report whether the patch would apply; change nothing |
| `--reject` | Apply what applies and set the failed hunks aside as a `.rej` patch |
| `--ignore-whitespace` | Ignore whitespace differences when matching context |
| `-F` / `--fuzz <n>` | Drop up to *n* lines of leading/trailing context to find a match (GNU `patch` default 2) |
| offset search | A hunk may apply some lines away from its header position; the offset is reported |
| `--recount` | Trust the hunk body over the declared `@@` counts |
| `--stat` / `--numstat` | Report instead of applying |
| `-p<n>`, `--include`, `--exclude` | Path selection inside a multi-file patch |
| `--unidiff-zero` | Allow zero-context patches |

## Table stakes → decision

| Capability | Source | Decision |
| --- | --- | --- |
| Original text + unified diff in, patched text out | all three | **Built** — `source`, `patch`, default `output = patched` |
| Reverse / unapply | DevToolbox, `git apply -R` | **Built** — `reverse` checkbox |
| Dry-run / check without producing output | `git apply --check` | **Built** — `output = report` (and `json`) apply nothing |
| Per-hunk status: applied / failed, with reason | DevToolbox, io-tools | **Built** — `report` and `json` list every hunk with status, offset, fuzz, and the reason a failure occurred |
| Strict abort on mismatch | io-tools (its whole model) | **Built** — `on_conflict = fail` is the default |
| Partial apply, failures set aside | GNU `patch --reject` | **Built** — `on_conflict = skip` + `output = rejects` emits the failed hunks as a valid standalone patch |
| Fuzzy / offset matching | DevToolbox, GNU `patch` | **Built** — unlimited offset search (nearest match wins, ties prefer the earlier line) plus a `fuzz` slider (0–3, default 2, matching GNU `patch`) |
| Whitespace-insensitive context match | `git apply --ignore-whitespace` | **Built** — `ignore_whitespace` checkbox (matching only; emitted lines stay verbatim) |
| Multi-file patch support | DevToolbox, `git apply --include` | **Built** — `file` filter; a single-file patch is auto-selected, a multi-file patch without a filter errors and lists the paths it found |
| Machine-readable result | none of the three | **Built** — `output = json` (a differentiator: hunk statuses, offsets, and the patched text as data) |
| Tolerate wrong `@@` counts | `git apply --recount` | **Built, always on** — the hunk body is authoritative, so mail-mangled counts still parse. No param; documented in the FAQ |
| Preserve CRLF and "no newline at end of file" | GNU `patch` | **Built** — line ending and final-newline state of the source are preserved; `\ No newline…` markers are honoured |
| Per-hunk accept/reject checkboxes | DevToolbox | **Considered, not built here** — an interactive per-hunk UI is a different tool shape; the repo already ships `diff-hunk-selector` for narrowing a patch to chosen hunks before applying it. Documented on the page as the two-step workflow |
| Generate a diff from two texts | DevToolbox (second mode) | **Out of scope** — already `text-diff` in this repo; linked in the page copy instead of duplicated |
| Path strip levels (`-p<n>`), rename/mode/binary entries | `git apply` | **Out of model** — there is no working tree here; the tool patches one pasted text, so paths are only used to pick which hunks apply. Stated in the page limits |
| 3-way merge (`--3way`) | `git apply` | **Out of model** — needs the original blobs from a repository object store |

## UX patterns adopted

- Preset example chips for the common flows (apply, reverse/unapply, dry-run report, partial
  apply with rejects) — the declarative `[[example]]` chips the generator already supports.
- A slider for `fuzz` (small bounded numeric range) rather than a bare number box.
- Friendly `<select>` labels via `[input.labels]` for both enums.
- Multiline textareas for both text inputs so pasted newlines survive.
- Errors name the hunk number, the expected line, and the line actually found.

## Limits stated on the page

1 MB per input; single pasted file (multi-file patches need the `file` filter); no working tree, so
renames, mode changes, and binary hunks are reported as unsupported rather than silently dropped;
combined merge diffs (`@@@`) are rejected.
