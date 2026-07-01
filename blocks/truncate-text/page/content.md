## What this tool does

Truncate (shorten) text to a maximum number of **characters** or **words**, right
in your browser. When the text is longer than the limit it is cut and an ellipsis
(`…` by default) is appended; text that already fits is returned unchanged. Nothing
is sent to a server — it runs locally, works offline, and needs no sign-up.

Paste your text, set a **Limit**, choose whether to measure that limit in
characters or words, and tune the options.

## How it truncates

- **By characters** (default): the result is kept at or below your limit. By
  default the cut is backed up to the last space so a word is never split in half,
  and the ellipsis counts toward the limit so the *whole* output — marker included —
  fits within it.
- **By words**: the first *N* whitespace-separated words are kept and the ellipsis
  is appended.
- If the text is already within the limit, it is returned **as-is, with no
  ellipsis**.

## Options

| Option | What it does |
| --- | --- |
| **Limit** | The maximum number of units to keep (default 100). |
| **Measure limit in** | Count the limit in `characters` (default) or whole `words`. |
| **Ellipsis / suffix** | The marker appended when text is cut (default `…`). Type `...` for three dots, or your own suffix such as ` (read more)`. |
| **Ellipsis counts toward the limit** (default on) | When on, room is reserved for the marker so the whole result fits within the limit. Turn off to keep the limit characters *plus* the marker. Only applies to character truncation. |
| **Allow breaking words mid-cut** (default off) | When off, the cut backs up to the last space so a word is never split. Turn on to cut exactly at the character limit. Only applies to character truncation. |

## Examples

| Input | Limit | Output |
| --- | --- | --- |
| `the quick brown fox` | 12 characters | `the quick…` |
| `the quick brown fox` | 12 characters, break words on | `the quick b…` |
| `the quick brown fox jumps` | 3 words | `the quick brown…` |
| `hello` | 20 characters | `hello` (unchanged) |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your text never leaves your device, and it keeps
working offline once the page has loaded.

</details>

<details>
<summary>Does it count by characters or display width?</summary>

It counts Unicode characters
(scalar values), which is exact for plain prose. Wide CJK glyphs and combining
marks may render wider than their character count.

</details>

<details>
<summary>Can I truncate with no ellipsis at all?</summary>

On this page, clearing the ellipsis
field falls back to the default `…`. To cut with no marker, use the CLI or chat
tool and pass an empty `ellipsis`.

</details>

<details>
<summary>Is this good for meta descriptions or previews?</summary>

Yes — set the limit to your
target length (for example 155–160 characters for a meta description) and the
word-safe cut keeps the snippet readable.

</details>
