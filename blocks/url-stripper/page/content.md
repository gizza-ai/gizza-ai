## About this tool

**URL Stripper** removes web links from a block of text so what's left reads as clean
prose. Paste an email, a chat log, scraped copy, or an AI-generated draft, and it deletes
every `http://`, `https://`, and `ftp://` URL — and, by default, scheme-less links that
start with `www.` — leaving the surrounding sentences intact.

You control exactly what happens to each match:

- **Replace with nothing (default):** the link is deleted outright.
- **Replace with a placeholder:** type something like `[link]` into *Replace each link
  with* and every URL becomes that token instead of vanishing.
- **Also remove emails:** tick *Also remove email addresses* to strip bare addresses like
  `name@example.com` in the same pass.
- **Also remove www. links:** on by default; untick it to delete only true scheme URLs and
  leave `www.example.com` in place.

By default the tool then **tidies the spacing** left behind — it collapses the double
spaces a deleted URL leaves, drops a space sitting before a comma or period, removes
brackets that are now empty (`Source (https://a.com/x) here` → `Source here`), and trims
each line. Newlines and blank lines are kept, so paragraph structure survives. It even
keeps the sentence's own punctuation when a URL is glued to it: `See https://x.com/y.`
becomes `See.`, not `See`.

Everything runs locally in your browser via WebAssembly — nothing is uploaded, and no link
is ever fetched or followed. The same engine powers the command-line and chat versions.

### Worked example

Input:

```
Read the docs at https://example.com/guide and email us at hi@example.com — more at www.example.com.
```

With **Also remove email addresses** on and the replacement left blank:

```
Read the docs at and email us at — more at.
```

Leave emails off (the default) and the address stays; switch the replacement to `[link]`
and each URL becomes `[link]` instead of disappearing.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions with a blank line inside each. -->

<details>
<summary>What kinds of links does it remove?</summary>

Scheme URLs — `http://`, `https://`, and `ftp://` — always. By default it also removes
scheme-less links that begin with `www.` (like `www.example.com`); untick **Also remove
www. links** to keep those. Bare email addresses are removed only when you tick **Also
remove email addresses**. A URL stops at the first whitespace, bracket, or quote, so a link
inside `(…)` or `"…"` is removed without eating the punctuation around it.

</details>

<details>
<summary>Can I replace links with a placeholder instead of deleting them?</summary>

Yes. Type anything into **Replace each link with** — for example `[link]`, `[url]`, or
`***` — and every removed URL/email becomes that token. Leave the field blank (the default)
to delete each link entirely. The placeholder is inserted verbatim, so you can even use an
empty-looking marker or a full label.

</details>

<details>
<summary>Will removing a link leave weird gaps or broken punctuation?</summary>

Not with **Tidy the spacing left behind** on (the default). It collapses the double space a
deleted URL leaves, drops a stray space before a comma/period, removes brackets left empty
once their link is gone, and trims each line — while keeping newlines and blank lines so
paragraphs stay intact. Trailing punctuation stuck to a URL is preserved too, so
`See https://x.com/y.` becomes `See.`. Turn the option off if you want the raw gaps left
exactly where the links were.

</details>

<details>
<summary>Is my text uploaded anywhere?</summary>

No. The stripping runs entirely inside your browser via WebAssembly — the text never leaves
your device, and no link is fetched or followed. The same engine also powers the
command-line and chat versions.

</details>

## Limits & edge cases

- **Detection is text-pattern based:** links are matched by shape (`scheme://…`, `www.…`,
  `name@host.tld`), not by validating that a host really resolves. A URL missing its scheme
  and not starting with `www.` (e.g. a bare `example.com/path`) is left alone, and a word
  that merely looks like an address could match.
- **A URL ends at the first delimiter:** whitespace, quotes, or brackets terminate a match.
  A link containing a literal space (unusual, usually percent-encoded) is only removed up to
  that space.
- **Counts are not de-duplicated:** the same URL appearing twice counts as two removals.
- **No rich-text/markdown awareness:** it strips the raw URL text. In markdown like
  `[docs](https://x.com)` the URL inside the parentheses is removed, but the `[docs]` label
  and brackets remain — it does not rewrite link syntax.
- **Tidy pass is conservative:** it collapses horizontal spaces and trims lines but never
  merges separate lines or removes blank lines, so intentional formatting is preserved.
