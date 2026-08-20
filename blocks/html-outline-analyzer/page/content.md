## About this tool

`html-outline-analyzer` reads pasted HTML and reports two things that are useful for SEO, accessibility reviews, documentation cleanups, and migrations: the H1-H6 heading outline, and the element tags used in the source.

The outline keeps document order and records each heading's level, text, `id`, line/column, and hidden state. It also flags common structural problems: missing H1, multiple H1s, skipped heading levels, empty headings, duplicate heading text, and headings hidden with markup such as `hidden`, `aria-hidden="true"`, or inline `display:none`.

### Worked example

Input:

```html
<h1>Getting started</h1>
<h2 id="install">Install</h2>
<p>Text</p>
<h3>macOS</h3>
<h3>Windows</h3>
<h2>Configure</h2>
<h4>Advanced flags</h4>
```

Default output includes:

```text
HTML OUTLINE — 6 heading(s)
Levels: h1:1  h2:2  h3:2  h4:1

OUTLINE (6 headings)
    1  h1  Getting started
    2    h2  Install  #install
    4      h3  macOS
    5      h3  Windows
    6    h2  Configure
    7        h4  Advanced flags
```

The skipped `h2` → `h4` jump is listed in the issues section, and the tag-count section shows how many `h1`, `h2`, `p`, and other element tags were written.

### Options and limits

- **Output** can be `tree`, `markdown`, `json`, or `csv`.
- **Minimum/maximum heading level** filter the rendered outline without changing the total heading counts.
- **Include outline issues** toggles the issue section.
- **Include tag counts** toggles the total/distinct tag count section.
- **Tag count rows** limits the number of distinct tag names listed, from 1 to 500.
- The input limit is 5 MB and the first 5,000 headings are recorded.
- Counts are based on literal opening/self-closing tags in the source. The scanner does not invent implied DOM elements such as browser-added `<tbody>` or `<body>`.
- It does not execute JavaScript, fetch linked pages, apply CSS, validate full HTML conformance, or compute the browser accessibility tree.

## FAQ

<details>
<summary>Is this the same as a browser DOM outline?</summary>

No. It scans the literal HTML you paste. That means tag counts reflect the source, not the DOM after a browser inserts implied elements or JavaScript mutates the page. This is useful for audits of generated markup and templates.

</details>

<details>
<summary>Does it use the old HTML5 outline algorithm?</summary>

No. The old sectioning algorithm is not implemented by browsers or assistive technology in a way authors can rely on. This tool reports the practical H1-H6 heading sequence and flags skipped levels.

</details>

<details>
<summary>Why is a hidden heading reported?</summary>

Hidden headings can affect audits and screen-reader behavior depending on how they are hidden. The analyzer marks headings with `hidden`, `aria-hidden="true"`, or inline styles such as `display:none` and `visibility:hidden` so you can review them explicitly.

</details>

<details>
<summary>Can it analyze a live URL?</summary>

No. Paste the HTML source. Keeping the tool paste-only avoids network fetching, login/session problems, and server-side rendering differences. Use your browser's “View Source” or a crawler to collect markup first.

</details>

<details>
<summary>Is my HTML uploaded?</summary>

No. The Rust scanner runs locally in the WebAssembly page and in the CLI. Your markup is processed in your browser or terminal.

</details>
