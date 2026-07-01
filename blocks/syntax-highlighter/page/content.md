## About this tool

The **syntax highlighter** turns a plain code snippet into colored, styled output you can
paste straight into a web page, an email, a blog post, or a terminal. Pick the language and a
color theme, choose **HTML** or **ANSI**, and copy the result. Everything runs locally in your
browser with WebAssembly — your code is never uploaded.

### HTML output

The **HTML** format returns a self-contained `<pre>` block with inline `style` attributes on
every colored token, plus a background color from the theme. Because the colors are inlined,
there is **no external stylesheet to ship** — it renders the same in a CMS, an email client, a
static site, or a sandboxed iframe. Just paste the markup where you need it.

### ANSI output

The **ANSI** format emits 24-bit ("true color") terminal escape codes and ends with a reset.
Pipe or `echo` it into a terminal that supports true color to see the highlighted snippet, or
embed it in CLI help text and demo scripts.

### Languages and themes

Highlighting is powered by [syntect](https://github.com/trishume/syntect) with its bundled
Sublime-Text syntax definitions, covering 100+ languages — use a hint like `rust`, `python`,
`javascript`, `typescript`, `bash`, `json`, `yaml`, `go`, `c`, `cpp`, `html`, or `markdown`.
An unknown or omitted language falls back to uncolored plain text. Themes include
`base16-ocean.dark` (the default), `base16-ocean.light`, `Solarized (dark)`,
`Solarized (light)`, `base16-eighties.dark`, `base16-mocha.dark`, and `InspiredGitHub`; an
unknown theme falls back to the default.

### Privacy

This tool is pure client-side WebAssembly. Your code and the highlighted output stay in your
browser — nothing is sent to a server.

## FAQ

<details>
<summary>Why did my code come out uncolored?</summary>

The language hint didn't resolve. Resolution tries friendly aliases
(`rust`/`rs`, `c++`/`cpp`, `bash`/`sh`, …), then file-extension, token and
full syntax names, case-insensitively — and anything still unknown falls back
to plain text rather than erroring. Try the language's usual file extension
(`ts`, `go`, `yaml`) if the full name didn't take.

</details>

<details>
<summary>Is there a size limit on the snippet?</summary>

Yes — 512 KiB of source. Larger input is rejected with an explicit
"too large" error (and empty input errors too) so a runaway paste can't hang
the highlighter. For a whole file that big, split it into sections.

</details>

<details>
<summary>Do I need to add a stylesheet for the HTML output?</summary>

No. Every colored token carries an inline `style` attribute and the `<pre>`
gets the theme's background color, so the block is fully self-contained — it
renders identically in a CMS, an email client, or a sandboxed iframe with no
external CSS.

</details>

<details>
<summary>My terminal shows garbage instead of colors for ANSI output — why?</summary>

The ANSI format uses 24-bit "true color" escape sequences. Older terminals
(or multiplexer configs) that only support 256 colors won't interpret them —
check that your terminal advertises truecolor support (e.g.
`COLORTERM=truecolor`). The output ends with a reset code so it won't bleed
styling into your prompt.

</details>
