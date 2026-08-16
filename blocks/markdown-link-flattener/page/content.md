## About this tool

Markdown links are great while you are editing, but they get in the way when the text is headed to
plain email, a CMS field, a spreadsheet, an LLM prompt, or a legal/redline review that should show
the visible words instead of link syntax. This tool flattens inline Markdown links in place:
`[the docs](https://example.com/docs)` can become `the docs`, `the docs (https://example.com/docs)`,
or just `https://example.com/docs`.

The surrounding text stays in the same order. Code spans and fenced code blocks are preserved by
default, so README examples that intentionally show Markdown syntax are not rewritten by accident.
Images and reference definition lines have their own controls because teams disagree on whether
`![alt](image.png)` should become alt text, a citation, nothing at all, or stay as image syntax.

### Worked example

Input:

```md
Read [the docs](https://example.com/docs) and see ![diagram](diagram.png).

[old-ref]: https://example.com/old

`[example](kept)`
```

With the default settings, the output is:

```md
Read the docs and see diagram.



`[example](kept)`
```

Switch **Inline links become** to **Text with URL** and the first sentence becomes
`Read the docs (https://example.com/docs) and see diagram.`. Switch **Images become** to
**Keep image Markdown** and the image syntax stays untouched while normal links are still
flattened.

### What it handles

- Inline links: `[label](https://example.com)`, including destinations wrapped in `<...>` and an
  optional quoted title after the destination.
- Image syntax: `![alt text](image.png)`, with controls for alt text, alt text plus URL, drop, or
  keep Markdown.
- Reference definition lines: `[id]: https://example.com`, which can be dropped after the inline
  links are flattened or kept for review.
- Code spans and fenced code blocks, preserved by default.

### Limits and edge cases

- Up to **1,000,000 bytes** of Markdown per run. Split larger documents and run the parts separately.
- This is a syntax flattener, not a full CommonMark renderer. It does not resolve reference-style
  link uses such as `[label][id]` to their URLs; it can only drop or keep the definition lines.
- Nested brackets in link labels are handled, but malformed links are left exactly as written rather
  than guessed at.
- Relative URLs, fragments, mailto links and paths are treated as plain destinations. The tool does
  not fetch or validate them.
- Everything runs in your browser. No upload, no account, no network call, and no host clock.

## FAQ

<details>
<summary>Will it remove the visible link text?</summary>

Only if you choose **URL only**. The default keeps the visible label, so `[docs](https://example.com)`
becomes `docs`. The **Text with URL** mode keeps both pieces as `docs (https://example.com)`, which
is often the safest format for plain email or review notes.

</details>

<details>
<summary>What happens to images?</summary>

Images are controlled separately from normal links. By default `![diagram](diagram.png)` becomes
`diagram`, because alt text is the closest plain-text replacement. You can instead write
`diagram (diagram.png)`, remove images entirely, or keep the original image Markdown unchanged.

</details>

<details>
<summary>Does it change links inside code examples?</summary>

Not by default. Backtick code spans and fenced code blocks are copied through unchanged, so a README
example like `` `[x](y)` `` remains an example. Turn **Preserve code spans and fenced code blocks**
off only when you intentionally want to flatten every link-looking string in the document.

</details>

<details>
<summary>Can it resolve reference-style links?</summary>

It can drop or keep reference definition lines like `[docs]: https://example.com`, but it does not
rewrite `[the docs][docs]` to `the docs (https://example.com)`. Resolving references correctly needs
a fuller Markdown document model. This tool keeps that use visible rather than silently guessing.

</details>

<details>
<summary>Are URLs checked or fetched?</summary>

No. Destinations are treated as text. The tool does not follow redirects, validate domains, download
images, or contact the network, which keeps it deterministic and safe for drafts that contain
private URLs.

</details>
