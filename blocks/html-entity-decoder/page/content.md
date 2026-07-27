## What this tool does

Paste text that contains **HTML character entities** and get back the real
characters they stand for. It decodes:

- **Named entities** across the full HTML5 set — `&amp;` → `&`, `&copy;` → ©,
  `&mdash;` → —, `&hellip;` → …, `&nbsp;` → a non-breaking space, `&eacute;` → é,
  and roughly 2,000 more.
- **Decimal numeric references** — `&#169;` → ©, `&#8364;` → €.
- **Hexadecimal numeric references** — `&#xA9;` or `&#XA9;` → ©.

Everything runs locally in your browser. Nothing is uploaded, it works offline
once loaded, and there is no sign-up.

## Worked example

**Input**

```
Fish &amp; Chips &mdash; &#169; 2026 &#x2122;
```

**Output**

```
Fish & Chips — © 2026 ™
```

The named `&amp;`, the named `&mdash;`, the decimal `&#169;`, and the hex
`&#x2122;` are all resolved in one pass.

## Unknown references

The **Unknown references** option decides what happens when a reference can't be
decoded — an unrecognized name or a malformed numeric reference:

| Setting | Behavior |
| --- | --- |
| **keep** (default) | Leaves the text exactly as written, so `&notreal;` stays `&notreal;`. Best for real-world HTML where a stray `&` shouldn't break the whole string. |
| **error** | Stops and names the first reference it can't decode. Use it to validate that every entity in a snippet is legitimate. |

## Details worth knowing

- **Legacy Windows code points.** Per the HTML standard, numeric references
  128–159 are read as Windows-1252, not raw Unicode — so `&#151;` decodes to an
  em dash (—) and `&#128;` to the euro sign (€), matching every browser.
- **Legacy entities without a semicolon.** A small historical set decodes even
  without the trailing `;` (`&copy` → ©). Modern names still need their `;`, so
  `&mdash` on its own is left untouched.
- **Invalid code points** (null, surrogates, or anything above U+10FFFF) decode
  to the Unicode replacement character `�` (U+FFFD), again matching browsers.
- **A bare ampersand is safe.** `Tom & Jerry` is returned unchanged — only real
  references are touched.

## Common entities

| Entity | Character | | Entity | Character |
| --- | --- | --- | --- | --- |
| `&amp;` | & | | `&mdash;` | — |
| `&lt;` | < | | `&ndash;` | – |
| `&gt;` | > | | `&hellip;` | … |
| `&quot;` | " | | `&copy;` | © |
| `&apos;` | ' | | `&reg;` | ® |
| `&nbsp;` | (non-breaking space) | | `&trade;` | ™ |
| `&euro;` | € | | `&pound;` | £ |
| `&ldquo;` / `&rdquo;` | " " | | `&lsquo;` / `&rsquo;` | ' ' |

## FAQ

<details>
<summary>Is my text uploaded anywhere?</summary>

No. Decoding happens entirely in your browser with WebAssembly — your input
never leaves your device, and the tool keeps working offline once the page has
loaded.

</details>

<details>
<summary>What's the difference between named and numeric entities?</summary>

A **named** entity uses a keyword between `&` and `;` (`&copy;`), while a
**numeric** reference uses a code point — decimal (`&#169;`) or hexadecimal
(`&#xA9;` / `&#XA9;`). All three forms above decode to the same character, ©,
and this tool handles all of them.

</details>

<details>
<summary>Why did <code>&#151;</code> turn into an em dash instead of a control character?</summary>

Numeric references in the range 128–159 are interpreted as **Windows-1252**
bytes by the HTML standard, so `&#151;` is an em dash (—) and `&#128;` is the
euro sign (€). Every browser does the same remap, and this tool follows it.

</details>

<details>
<summary>What happens to an entity I mistyped, like <code>&amp;notreal;</code>?</summary>

With **Unknown references** set to **keep** (the default) it's left exactly as
written, so the rest of your text still decodes cleanly. Switch to **error** if
you'd rather the tool stop and tell you which reference it couldn't resolve.

</details>

<details>
<summary>Is the decoded output safe to drop straight into a web page?</summary>

Treat it as untrusted. Decoding turns `&lt;script&gt;` back into
`<script>`, so decoded text can become **active HTML** if you inject it into a
page. Escape or sanitize it again before rendering it as markup.

</details>

<details>
<summary>How do I go the other way and encode characters into entities?</summary>

This tool only decodes. To turn characters like `<`, `>`, and `&` into entities,
use the companion **string-escaper** tool with its HTML escape mode.

</details>
