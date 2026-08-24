# text-file-inspector — competitor analysis (2026-08-21)

Scan run before implementation per the tool-loop recipe. Notes below paraphrase observed public behavior and avoid competitor copy or branding language.

Backlog row: `text-file-inspector` — "Reports a text file's detected encoding, BOM presence, line-ending style (LF/CRLF/mixed), line count, and longest line." Type hint: pure.

## Duplicate check

Nearby blocks checked:

| Existing block | What it does | Why this tool is distinct |
| --- | --- | --- |
| `text-encoding-converter` | Converts bytes between encodings and reports conversion diagnostics. | Conversion tool, not a status report; it does not summarize line endings, final newline, indentation or longest lines. |
| `data-format-sniffer` | Guesses whether data is CSV/JSON/etc. | Format classification, not text-file hygiene. |
| `line-ending-converter` / text cleanup family (where present in backlog/blocks) | Transform text. | This block is read-only and reports byte/line facts before deciding what to change. |

Verdict: not a duplicate. It combines editor status-bar facts and lint-style text hygiene in a single report.

## Competitors surveyed

1. Online newline / line-ending checkers: common controls are paste/upload text, show LF vs CRLF vs CR counts, identify mixed endings, and sometimes normalize or copy converted output.
2. Online encoding/BOM detectors: upload bytes, show UTF-8/UTF-16/legacy guesses, BOM bytes, invalid sequences and file size.
3. Text statistics / line counter tools: paste text, report line count, character count, longest line, blank lines and sometimes word count.
4. Editor status bars and lint plugins: surface final newline, mixed tabs/spaces, trailing whitespace, maximum line length and control characters.

## Table stakes → decision

| Capability | Decision |
| --- | --- |
| Detect BOM and report BOM bytes | In — UTF-8/UTF-16/UTF-32 BOMs are recognized and listed. |
| Detect encoding | In — ASCII/valid UTF-8/BOM/UTF-16 heuristic/chardetng guess are reported with detection method. |
| Count LF, CRLF and CR endings, flag mixed | In — counts, percentages and dominant style. |
| Line count and final-newline state | In — total, terminated/unterminated and final newline. |
| Longest line and optional longest-line list | In — `longest_lines` 0-50. |
| Maximum line-length lint check | In — `max_line_length` flags line numbers. |
| Blank, whitespace-only and trailing-whitespace lines | In — counts plus capped line-number lists. |
| Tab/space indentation mix | In — tab-indented, space-indented and mixed-indent line list. |
| Character classes | In — total, non-ASCII, control chars, NUL bytes and U+2028/U+2029 separators. |
| Preview with visible line-ending markers | In — `preview_lines` for first N lines. |
| JSON output | In — stable object for automation. |
| Exact file-byte upload on the browser page | Out of current page model — generic pure pages expose text fields; exact BOM/CRLF/legacy bytes are covered by CLI/chat `input_format=base64` or `hex`. |
| Rewrite/normalize line endings or encoding | Out of model for this block — read-only diagnostic; conversion belongs in dedicated converter tools. |

## Surface choices

- Chat/CLI: exact byte inspection via `input_format=text|base64|hex`.
- Page: pasted text inspection with report/json output, sliders for common caps and examples for mixed endings.
