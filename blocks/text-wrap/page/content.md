## What this tool does

Hard-wrap (reflow) text to a fixed column width, right in your browser. Every
output line is kept at or below the width you choose. Nothing is sent to a
server — it runs locally, works offline, and needs no sign-up. Paste your text,
set a **Line width**, and optionally toggle **Preserve indentation** or
**Break long words**.

## How it wraps

- Each source line is reflowed **independently**, so existing hard line breaks
  and blank lines are kept — your paragraphs and spacing survive.
- Within a line, words are packed greedily: each word is added until the next one
  would overflow the width, then a new line starts.
- Runs of spaces or tabs between words collapse to a single space on the wrapped
  output.

## Options

| Option | What it does |
| --- | --- |
| **Line width** | The maximum number of characters per line (default 80). No output line will exceed it. |
| **Preserve indentation** (default on) | Detects the leading whitespace of each source line and re-applies it to every continuation line, so indented blocks and list items stay aligned. Turn off to flush everything to the left margin. |
| **Break long words** (default on) | Hard-splits a single word that is longer than the width at the column boundary. Turn off to keep such a word intact on its own over-length line (useful for URLs or long identifiers you don't want chopped). |

## Examples

| Input | Width | Output |
| --- | --- | --- |
| `the quick brown fox jumps over the lazy dog` | 15 | `the quick brown` / `fox jumps over` / `the lazy dog` |
| `supercalifragilisticexpialidocious` | 10 (break on) | wrapped into 10-char chunks |
| `supercalifragilisticexpialidocious` | 10 (break off) | left on one over-length line |

## FAQ

**Is it free and private?** Yes — your text never leaves your device, and it
keeps working offline once the page has loaded.

**Does it count by characters or display width?** It counts Unicode characters
(scalar values), which is exact for plain prose. Wide CJK glyphs and combining
marks may render wider than their character count.

**Will it merge my paragraphs?** No. Blank lines and existing line breaks are
preserved — only the text within each line is reflowed. To remove hard breaks
inside paragraphs first, use the Unwrap Text tool, then wrap.
