## Catch broken markup before it ships

Missing a single `</div>` or crossing two tags can push a whole layout off, break a
component, or confuse a scraper — and the browser silently "fixes" it, so the mistake
never shows up until something downstream breaks. This validator scans your HTML and
reports exactly what is wrong and **where**, so you can fix it at the source.

Paste a full page or a single snippet, and every issue comes back with its **line and
column** and a plain-language explanation of what was expected.

## What it checks

- **Unclosed tags** — an element that is opened but never closed by the end of the input
  (`<div><p>text` → `<p>` and `<div>` are never closed).
- **Nesting issues** — overlapping / misnested tags where a closing tag crosses another
  still-open element (`<b><i>hi</b></i>`), and stray closing tags with no matching open
  (`</span>` on its own).
- **Syntax errors** — an unterminated tag with no closing `>`, an unterminated `<!-- … -->`
  comment, and tags with no name.

It understands **void elements** (`br`, `img`, `hr`, `input`, `meta`, `link`, …) that
never take a closing tag, **self-closing** tags (`<hr/>`), quoted attribute values (so a
`>` inside `alt="a > b"` is safe), and the raw contents of `<script>`, `<style>`,
`<textarea>`, and `<pre>` — a `<` inside JavaScript or text is not treated as a tag.

## Worked example

Input:

```html
<div><p>Hello<span>world</div>
```

Report:

```
Invalid HTML: 3 error(s), 0 warning(s) in 0 element(s).

  error   line 1:6   `<p>` (opened at line 1:6) is not closed before `</div>` — overlapping/misnested tags
  error   line 1:14  `<span>` (opened at line 1:14) is not closed before `</div>` — overlapping/misnested tags
  error   line 1:1   `<div>` is never closed with `</div>`
```

## Output formats

Choose **report** for a readable, line-numbered issue list, or **json** for a
machine-readable `{ valid, errors, warnings, elements, issues[] }` object you can drop into
a test, a CI check, or an editor plugin. Each `issues[]` entry carries `severity`, `line`,
`column`, and `message`.

## Privacy

Everything runs locally in your browser through WebAssembly. Your HTML is never uploaded to
a server.

## FAQ

<details>
<summary>Is this the same as the W3C validator?</summary>

No. The W3C Nu checker validates conformance to the full living HTML standard — attribute
whitelists, ARIA rules, duplicate ids, and more — against a live schema. This tool is a
fast, local **structural** validator focused on the mistakes that actually break rendering:
unclosed tags, misnested tags, and raw syntax errors. It never uploads your markup.

</details>

<details>
<summary>What is the difference between an error and a warning?</summary>

An **error** means the markup is structurally broken — an unclosed tag, a misnested pair, a
stray closing tag, or an unterminated tag/comment. A **warning** flags something that is
technically tolerated but wrong, such as a closing tag on a void element (`</br>`). The
document is reported as *valid* only when there are no errors.

</details>

<details>
<summary>Does a `<` inside my JavaScript or text cause false errors?</summary>

No. The scanner treats the contents of `<script>`, `<style>`, `<textarea>`, and `<pre>` as
raw text, and a `<` that is not followed by a letter, `/`, `!`, or `?` (like `a < b`) is
read as plain text — not a tag. Quoted attribute values containing `>` are handled too.

</details>

<details>
<summary>Can I validate a fragment instead of a whole page?</summary>

Yes. You can paste a single component, a table row, or one `<div>` — you do not need a
`<!DOCTYPE html>` or `<html>` wrapper. The validator only reports on the tags you give it.

</details>

<details>
<summary>What are the limits?</summary>

This is a structural/syntax validator, not a full HTML conformance checker: it does not
verify attribute names, required attributes (like `alt`), accessibility rules, or CSS. It
also does not auto-correct — it reports issues so you can fix them at the source. Line and
column are 1-based.

</details>
