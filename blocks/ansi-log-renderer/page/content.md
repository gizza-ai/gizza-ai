## What this tool does

Paste raw terminal output, build logs, or CI output that contains ANSI escape
sequences and convert it into clean HTML. Color and style codes such as
`\x1b[31m` (red), `\x1b[1m` (bold), and `\x1b[0m` (reset) become styled
`<span>` elements inside a `<pre>`. If you only need readable text, switch the
output to **Plain text** to remove the escape codes instead.

Everything runs locally in the browser. The input is not uploaded anywhere.

## Supported ANSI features

| Feature | Supported |
| --- | --- |
| 16 basic and bright SGR colors | Yes |
| xterm 256-color codes (`38;5;n`, `48;5;n`) | Yes |
| 24-bit truecolor (`38;2;r;g;b`, `48;2;r;g;b`) | Yes |
| Bold, dim, italic, underline, inverse, strike, conceal | Yes |
| Cursor movement, screen erase, OSC titles/hyperlinks | Dropped, not replayed |
| Full terminal emulation / progress-bar rewrites | Not attempted |

The renderer is best for captured logs where preserving the color semantics is
more important than replaying an interactive terminal session.

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `\x1b[1;32m✓ build passed\x1b[0m` | HTML, dark, inline | A dark `<pre>` with a bold green success line |
| `\x1b[31mERROR\x1b[0m: not found` | HTML, dark, inline | Red `ERROR`, followed by plain text |
| `\x1b[38;5;196mred\x1b[0m` | HTML, light, classes | A light `<pre>` with the 256-color red span |
| `\x1b[2J\x1b[H\x1b[33mwarn\x1b[0m` | Plain text | `warn` |

## Output modes

- **Colored HTML** returns markup you can paste into a report, documentation, or
  an HTML page. Choose **Inline styles** for a fully self-contained fragment, or
  **CSS classes + style block** when you prefer class names for the common ANSI
  palette.
- **Plain text** strips escape sequences and leaves the readable log text,
  Unicode, tabs, and line breaks.

## FAQ

<details>
<summary>Does this execute or upload my log?</summary>

No. It treats your input as text, parses escape sequences locally, and renders the
result in the browser. No command is run and nothing is sent to a server.

</details>

<details>
<summary>Will the HTML output safely escape markup in my log?</summary>

Yes. Text characters such as `<`, `>`, `&`, and quotes are HTML-escaped before
being placed inside the output. ANSI style spans are generated separately.

</details>

<details>
<summary>Does it support truecolor and 256-color terminal output?</summary>

Yes. It understands the common SGR forms for 16-color, bright-color, xterm
256-color, and 24-bit RGB foreground/background colors.

</details>

<details>
<summary>Why do cursor movement effects not appear?</summary>

This is a log renderer, not a full terminal emulator. Cursor movement, screen
clear, OSC title/link strings, and similar non-SGR control sequences are removed
instead of being replayed.

</details>
