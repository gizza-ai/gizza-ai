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
