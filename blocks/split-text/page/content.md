## What this tool does

Split a block of text on a delimiter and get back one item per line — ready to
paste into a list, a spreadsheet column, or another tool. Everything runs locally
in your browser: nothing is sent to a server, it works offline, and there is no
sign-up. Type your text, choose how to split it, and the result updates as you go.

## How to split

| Mode | What it does | Delimiter used? |
| --- | --- | --- |
| **literal** (default) | Splits on every occurrence of the **Delimiter** you type | Yes |
| **whitespace** | Splits on runs of spaces, tabs, and newlines; ignores leading/trailing whitespace | No |
| **chars** | Puts each character on its own line | No |

## The delimiter

In **literal** mode, the **Delimiter** is the exact substring to split on — a
comma `,`, a semicolon `;`, a pipe `|`, ` - `, or any multi-character string. A
few common escapes are recognised so you can type them on one line:

| Type this | Splits on |
| --- | --- |
| `\n` | a newline |
| `\t` | a tab |
| `\r` | a carriage return |
| `\\` | a literal backslash |

Anything else after a backslash is kept as-is (so `\d` splits on a literal
`\d`).

## Tidy up the items

- **Trim each item** — removes leading and trailing whitespace from every item,
  so `a, b , c` becomes `a`, `b`, `c` instead of `a`, ` b `, ` c`.
- **Remove empty items** — drops blank items. Combined with a trailing delimiter
  (`a,b,`) or double delimiters (`a,,b`), this gives you a clean list. When
  **Trim** is also on, items that are only whitespace count as empty and are
  dropped too.

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `apple, banana, cherry` | literal `,` · trim | `apple` / `banana` / `cherry` |
| `id\tname\temail` | literal `\t` | `id` / `name` / `email` |
| `the quick   brown fox` | whitespace | `the` / `quick` / `brown` / `fox` |
| `héllo` | chars | `h` / `é` / `l` / `l` / `o` |
| `a,,b,` | literal `,` · remove empty | `a` / `b` |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your text never leaves your device, and the
tool keeps working offline once the page has loaded.

</details>

<details>
<summary>How do I split a comma-separated list into lines?</summary>

Leave the delimiter as `,`
and turn on **Trim each item** to drop the spaces after each comma.

</details>

<details>
<summary>How do I split on tabs (from a spreadsheet)?</summary>

Type `\t` as the delimiter in
**literal** mode, or just use **whitespace** mode if any whitespace should split.

</details>

<details>
<summary>How do I count or list every character?</summary>

Use **chars** mode — each Unicode
character (including emoji and accented letters) goes on its own line.

</details>

<details>
<summary>My list has blank lines — how do I remove them?</summary>

Turn on **Remove empty
items**; add **Trim each item** as well if some "blank" lines actually contain
spaces.

</details>
