## What this tool does

Paste terminal output, a build log, or any text peppered with ANSI escape codes,
and get back clean, readable plain text. Those `[31m`, `[0m`, and `[2K`
sequences your shell uses to paint colors and move the cursor get stripped out —
right in your browser. Nothing is uploaded to a server, it works offline, and it
needs no sign-up.

## What are ANSI codes?

Terminals color and format text with **ANSI escape sequences** — short control
strings that start with the escape character (`ESC`, shown as `\x1b` or `^[`).
The most common are **SGR** codes for color and style (`\x1b[31m` = red,
`\x1b[1m` = bold, `\x1b[0m` = reset). Others move the cursor or erase parts of the
screen (`\x1b[2J`, `\x1b[H`), or set the window title and clickable links
(**OSC** strings). When you copy colored output into a file or a ticket, those
codes come along as visual noise — this tool removes them.

## Strip modes

| Strip | What it removes | What it keeps |
| --- | --- | --- |
| **all** (default) | Every ANSI escape sequence — colors, styles, cursor moves, screen-erase, and OSC titles/hyperlinks | Plain text, Unicode, and line breaks |
| **color** | Only SGR color and style codes (`\x1b[…m`) | Cursor and erase control sequences stay intact |

Use **all** to get clean copy-pasteable text. Use **color** when you want to drop
the coloring but preserve cursor/erase control — for example, feeding output to a
program that still relies on the positioning codes.

## Examples

| Input | Strip | Output |
| --- | --- | --- |
| `\x1b[1;32m✓ build passed\x1b[0m` | all | `✓ build passed` |
| `\x1b[31mERROR\x1b[0m: not found` | all | `ERROR: not found` |
| `\x1b[2J\x1b[H\x1b[33mwarn\x1b[0m` | color | `\x1b[2J\x1b[H` + `warn` |
| `\x1b]8;;https://x.com\x1b\\link\x1b]8;;\x1b\\` | all | `link` |

## Common uses

- Clean up **CI / build logs** copied from GitHub Actions, GitLab, or Jenkins
  before pasting them into an issue.
- Strip color from **`grep --color`, `ls`, `npm`, `cargo`, or `pytest`** output
  redirected to a file.
- Make a **diff or test failure** readable in a plain-text editor or chat.
- Pre-process logs so a search, a script, or an LLM sees the text without escape
  noise.

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

</details>

<details>
<summary>Does it keep my Unicode and line breaks?</summary>

Yes. Only the ANSI escape sequences
are removed; accents, emoji, tabs, and newlines are preserved exactly.

</details>

<details>
<summary>What's the difference between "all" and "color"?</summary>

**all** removes every escape
sequence for fully clean text. **color** removes only the SGR color/style codes
and leaves cursor-movement and screen-erase sequences in place.

</details>

<details>
<summary>Does it handle OSC hyperlinks and window titles?</summary>

Yes — in **all** mode, OSC
strings (terminated by BEL or ST) such as `\x1b]8` hyperlinks and `\x1b]0` window
titles are removed, leaving just the visible link text.

</details>

<details>
<summary>Why are there still odd characters left?</summary>

This tool removes ANSI/VT escape
sequences. Truly raw control bytes that aren't part of an escape sequence (for
example a stray backspace used for overstrike) are left as-is.

</details>
