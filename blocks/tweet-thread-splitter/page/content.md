## What this tool does

Paste a long piece of text and split it into a numbered Twitter/X **thread** of
character-limit-safe tweets — without ever breaking a word in half. Everything
runs locally in your browser: nothing is uploaded, it works offline once loaded,
and there is no sign-up.

## How it works

1. Paste your text.
2. Pick the **characters per tweet** (default **280**, X/Twitter's standard
   limit). Use a smaller number for a tighter thread, or a larger one for X
   Premium long posts (up to 25,000).
3. Choose a **numbering style** (or turn it off).
4. Choose how length is **counted** (see below).
5. Keep **sentence-boundary breaks** on (default) so tweets rarely end
   mid-thought.

The splitter packs whole words greedily into each tweet, so a tweet is filled as
much as possible while always staying at or under the limit — and a word is never
split across two tweets. The counter is included in the count, so the final tweet
text never exceeds your limit.

## Numbering styles

| Style | Looks like |
| --- | --- |
| **parens** (default) | `…your text (1/5)` |
| **slash** | `…your text 1/5` |
| **dotted** | `1. your text` (numbered-list style, prepended) |
| **none** | no counter at all |

## Counting: characters vs. UTF-16

| Count | What it measures | When to use |
| --- | --- | --- |
| **chars** (default) | Unicode characters — one emoji counts as 1 | Simple, predictable splitting |
| **utf16** | UTF-16 code units — an emoji or other astral character counts as 2 | Matching how X and most JavaScript clients weigh emoji |

If your text has lots of emoji and you want the count to line up with what X
actually shows, choose **utf16**.

## Sentence-aware breaks

With **Prefer breaking on sentence boundaries** on (the default), the splitter
starts a fresh tweet after a sentence ends (`.`, `!`, `?`) whenever it can, so a
tweet rarely cuts off mid-thought. A single sentence that is longer than one whole
tweet still falls back to word-packing — and a word is never broken.

## Long words and URLs

A "word" longer than a whole tweet — for example a very long URL — is hard-split
across tweets on safe character boundaries, so nothing is ever dropped from your
text.

## Examples

| Input | Settings | Result |
| --- | --- | --- |
| A 600-character paragraph | 280 · parens | 3 tweets ending `(1/3)`, `(2/3)`, `(3/3)` |
| `hello world` | 280 · parens | `hello world (1/1)` |
| `just a short note` | 280 · none | `just a short note` (one tweet, no counter) |
| `First point. Second point.` | 280 · dotted · sentences on | `1. First point.` / `2. Second point.` |

## FAQ

**Is it free and private?** Yes — your text never leaves your device, and the
tool keeps working offline once the page has loaded.

**Does it break words in the middle?** No. Whole words are kept together; the
only thing ever split is a single "word" that is by itself longer than one whole
tweet (like a long URL), so no content is lost.

**Does the counter count toward the limit?** Yes. The numbering is included in the
per-tweet count, so every emitted tweet — counter included — stays at or under your
chosen limit.

**Which numbering style should I use?** `parens` (`(1/5)`) is the most common
Twitter convention; `slash` (`1/5`) is the same without parentheses; `dotted`
(`1.`) reads like a numbered list; `none` drops the counter entirely.

**What limit should I use?** Leave it at **280** for standard X/Twitter posts. Use
a higher value if you post with X Premium's longer limit, or a lower value to keep
each tweet short.
