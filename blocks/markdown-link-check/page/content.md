## About this tool

**Markdown Link Check** scans a pasted Markdown document and reports link problems that are fully
decidable offline. It does not fetch URLs, read your filesystem or upload the document anywhere;
the same Rust engine runs in the CLI, chat block and browser page.

Use it before publishing a README, changelog or docs page to catch the mistakes that normal spell
checkers miss:

- malformed inline links such as `[text] (url)`, `(text)[url]`, unclosed `(` and empty `()` targets;
- undefined, duplicate or unused reference-style definitions;
- in-document `#anchors` that do not match any heading, custom `{#id}` or HTML `id="..."` anchor;
- empty link text, missing image alt text, unencoded spaces in destinations and malformed `mailto:`
  links;
- optional `http://` hygiene warnings.

Fenced code blocks and inline code spans are ignored, so examples in snippets do not create noise.
Line and column numbers are 1-based.

### Worked example

Input:

```markdown
# Install

See [setup](#setup), [site](https://example.com), and [missing][ref].

[ref]: https://example.com/one
[ref]: https://example.com/two
```

Output:

```text
Issues
  3:5  error  ML007  broken anchor '#setup' — no heading in this document produces that id
  6:1  error  ML005  duplicate reference definition [ref] — the first definition wins, this one is ignored

2 error(s), 0 warning(s) in 3 link(s) checked.
```

### Controls

- **Link kind** filters the report to anchors, external URLs, relative paths, mail links, images,
  empty targets or reference links.
- **Report format** chooses plain text, a Markdown table for issue comments, or JSON for scripts.
- **Also list passing links** adds every checked link to the report, not just the findings.
- **Check in-document anchors** can be disabled when your renderer uses custom slug rules.
- **Warn about `http://` links** is off by default because some docs deliberately cite legacy URLs.

### Limits and edge cases

- Maximum input size is **1 MB**.
- External URLs are **not fetched**; this tool reports structural and hygiene issues, not HTTP
  status codes.
- Relative links are classified but not checked on disk, because the browser/wasm surface has no
  project filesystem.
- Heading anchors follow GitHub-style slugs, including duplicate headings as `-1`, `-2`; custom
  `{#id}` and raw HTML `id="..."` / `name="..."` anchors are also recognized.
- Reference labels are case-insensitive and whitespace-normalized.

## FAQ

<details>
<summary>Does this replace a network link checker?</summary>

No. It catches local Markdown mistakes before a network checker runs: malformed syntax, duplicate
reference definitions and broken in-document anchors. It deliberately does not request external
URLs, so it is fast, private and deterministic, but it cannot tell whether `https://example.com`
currently returns 200 or 404.

</details>

<details>
<summary>Why are some relative file links only listed as links, not errors?</summary>

A relative target such as `./docs/install.md` may be valid in your repository, but the browser page
cannot see that repository. The tool classifies it as a relative link and can include it in JSON or
`show_ok` output, while leaving filesystem existence to your docs build or CI checkout.

</details>

<details>
<summary>Which heading anchor style is used?</summary>

GitHub-style slugs: text is lower-cased, punctuation is dropped, spaces become dashes and duplicate
headings get numeric suffixes (`#usage`, then `#usage-1`). The scanner also honors explicit
`{#custom-id}` suffixes and raw HTML `id="..."` / `name="..."` anchors.

</details>

<details>
<summary>How do I use the JSON output in CI?</summary>

Set report format to JSON and parse the `errors` count. A non-zero count means the document has
structural link errors. Warnings are separated so you can decide whether missing image alt text or
`http://` links should fail your own pipeline.

</details>
