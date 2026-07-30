## Highlight logs and command output with repeatable rules

Text Colorizer applies a stack of `color: regex` rules to every line of pasted text. It is useful for
turning plain CI logs, command output, server logs, or grep results into terminal-colored ANSI text or
self-contained HTML you can paste into a report.

Rules are evaluated from top to bottom. Each rule uses the text before the first colon as the style and
the text after it as a Rust regular expression. Earlier rules win where matches overlap, so put the most
important patterns first.

## Worked example

Paste this text:

```text
INFO service started
WARN retrying request
ERROR database timeout
```

Then use these rules:

```text
bold red: \bERROR\b
yellow: \bWARN(ING)?\b
green: \bINFO\b
```

Choose **ANSI terminal escapes** to paste into a compatible terminal or **HTML `<pre>`** to produce a
small themed block for a web page or incident note.

## Rule syntax

- `red: ERROR` colors matching substrings red.
- `bold red: \bERROR\b` adds attributes before the foreground color.
- `green on black: \bOK\b` sets a background with `on <color>`.
- `#3465a4 underline: https?://\S+` uses a custom hex color.
- Turn on **Color whole matching lines** when you want a matching `ERROR` or `FAIL` to color the entire line.

## Limits and edge cases

- Input text is capped at 512 KiB and rules are capped at 200 lines.
- Rules use Rust regular expressions, not JavaScript-specific regex features.
- HTML output escapes unmatched text before wrapping styled matches, so pasted `<` and `&` are safe text.
- ANSI output includes escape characters; use HTML output when a destination does not interpret ANSI.

## FAQ

<details>
<summary>What colors and attributes can I use?</summary>

Named colors include black, red, green, yellow, blue, magenta, cyan, white, gray/grey, and bright variants
such as brightred and brightyellow. You can also use `#rgb` or `#rrggbb` hex colors. Attributes include
bold, dim, italic, underline, blink, reverse, and strike.

</details>

<details>
<summary>What happens when two rules match the same text?</summary>

The first rule in the list wins for the overlapping characters. Put high-priority rules such as `ERROR`
before broad catch-all patterns like `timeout|failed|error`.

</details>

<details>
<summary>Can I color an entire log line instead of just the matching word?</summary>

Yes. Enable **Color whole matching lines**. The first rule that matches anywhere on a line styles the
whole line, which is useful for CI statuses or severity-based log highlighting.

</details>

<details>
<summary>When should I choose ANSI versus HTML?</summary>

Choose ANSI when the result will be viewed in a terminal or a tool that understands escape codes. Choose
HTML when you want a portable preview block for documentation, incident notes, or web pages.

</details>
