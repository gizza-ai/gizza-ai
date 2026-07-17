# regex-search — competitor analysis (2026-07-18)

Tool function: a grep-style search over pasted text — return the **whole lines**
that match a regex or literal pattern, with line numbers, surrounding context
lines, and match highlighting. Distinct from `regex-extract` (returns matched
*substrings*/capture groups) and `regex-tester` (returns match spans + all
capture groups for debugging a pattern). This tool is the browser equivalent of
`grep -n -C`.

## Competitors scanned

1. **Browserling — Grep Text** (browserling.com/tools/grep). Minimalist "paste
   text → Grep" flow. Regex pattern field. Ships an **Invert matches** toggle
   (show the non-matching lines). No stated line-number/context/whole-word
   options — deliberately bare.
2. **Awesome Toolkit — Text Search** (awesometoolkit.com/en/tools/text-search).
   Search by **literal string OR regular expression**. Result shows a **match
   count**, **line numbers**, and **surrounding context**. Runs fully in the
   browser, nothing uploaded.
3. **Galaxy / Magica — Grep Text Lines** (galaxy.ai/grep-text-lines). Advertises
   "powerful regular expressions with **Unix-like grep flags**, **line numbers**,
   **context lines**, real-time results." (JS-gated page; feature list taken from
   the search snapshot.)
4. **EasyStackTools — Grep Text** (easystacktools.com/text-tools/grep-text).
   Search text for lines matching a pattern (**plain text or regex**), show only
   the matching lines, **optional line numbers**.
5. **GNU grep (canonical reference)** — the feature vocabulary every tool above
   imitates: `-i` ignore-case, `-v` invert, `-w` whole-word, `-F` fixed-string
   (literal), `-n` line numbers, `-A/-B/-C` context lines, `-c` count, `--color`
   highlighting.

## Table-stakes → decision

| Capability (grep flag) | Competitors | Decision |
|---|---|---|
| Regex pattern matching | all | ✅ in descriptor (`pattern`) |
| Literal / fixed-string mode (`-F`) | Awesome, EasyStack | ✅ `literal` (escapes metachars) |
| Case-insensitive (`-i`) | grep, Galaxy | ✅ `ignore_case` |
| Invert match (`-v`) | Browserling, grep, Galaxy | ✅ `invert` |
| Whole-word match (`-w`) | grep, Galaxy | ✅ `whole_word` (wraps `\b…\b`) |
| Line numbers (`-n`) | Awesome, EasyStack, Galaxy | ✅ `line_numbers` (default **on**) |
| Context lines (`-A/-B/-C`) | Awesome, Galaxy, grep | ✅ `context` (before+after, grep `-C N`) |
| Match count (`-c`) | Awesome, grep | ✅ returned as `match_count` + rendered header |
| Highlight matches (`--color`) | grep, Galaxy (implied) | ✅ `highlight` (wraps matches in `«…»` markers — text-safe) |
| Multiline flag (`^/$` per line) | — | ✅ implicit: search is line-oriented, `^`/`$` already anchor each line |

Every table-stake lands in the descriptor. No out-of-model items: grep is pure
string work, fully in gizza's browser-local wasm model.

## UX patterns adopted (ideas only — no copy/branding reused)

- Line-numbers ON by default (matches the "grep -n" default users expect from a
  line search tool; competitors that hide numbers feel less useful).
- Preset example chips for the common shapes: plain literal search, case-
  insensitive regex, invert (exclude lines), and context window.
- Classic grep output grammar: `n:` prefixes hit lines, `n-` prefixes context
  lines, `--` separates non-adjacent context groups — instantly familiar.

## Out-of-model / considered-not-built

- **File upload of very large logs** — the page/CLI already accept pasted text;
  streaming multi-MB file search is a site-repo concern, out of scope here. The
  existing `log-parser` block covers log-file-specific searching.
- **Only-matching output (`-o`)** — deliberately omitted; that is exactly
  `regex-extract`'s job (substring extraction), and duplicating it would blur the
  line between the two tools. `regex-search` stays line-oriented.
